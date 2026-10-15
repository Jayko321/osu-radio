//! Bounded IPC frames; the crate supplies Activity serialization and outgoing JSON framing.
use super::{APPLICATION_ID, Desired, IO_TIMEOUT};
use discord_rich_presence::{DiscordIpc, error::Error};
use std::{future::Future, io};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    runtime::Handle,
    sync::watch,
};
#[cfg(unix)]
type Socket = tokio::net::UnixStream;
#[cfg(windows)]
type Socket = tokio::net::windows::named_pipe::NamedPipeClient;

pub(super) struct Transport {
    runtime: Handle,
    state: watch::Receiver<Desired>,
    socket: Option<Socket>,
    first_byte: Option<u8>,
    nonce: u64,
    #[cfg(all(test, unix))]
    test_path: Option<std::path::PathBuf>,
    pub(super) cancellable: bool,
}
const MAX_PAYLOAD: u32 = 64 * 1024;
impl Transport {
    pub(super) const fn new(runtime: Handle, state: watch::Receiver<Desired>) -> Self {
        Self {
            runtime,
            state,
            socket: None,
            first_byte: None,
            nonce: 0,
            #[cfg(all(test, unix))]
            test_path: None,
            cancellable: true,
        }
    }
    pub(super) async fn ready(&mut self) -> Result<(), Error> {
        if self.first_byte.is_none() {
            let socket = self.socket.as_mut().ok_or(Error::NotConnected)?;
            let mut byte = [0];
            // read is cancellation-safe; readiness alone can be a false positive.
            if socket.read(&mut byte).await.map_err(Error::ReadError)? == 0 {
                return Err(Error::ReadError(io::ErrorKind::UnexpectedEof.into()));
            }
            self.first_byte = byte.first().copied();
        }
        Ok(())
    }
    pub(super) fn next_nonce(&mut self) -> String {
        self.nonce = self.nonce.wrapping_add(1);
        self.nonce.to_string()
    }
    pub(super) fn recv_raw(&mut self) -> Result<(u32, Vec<u8>), Error> {
        let socket = self.socket.as_mut().ok_or(Error::NotConnected)?;
        let first_byte = self.first_byte.take();
        bounded(&self.runtime, &mut self.state, self.cancellable, async {
            let mut header = [[0; 4]; 2];
            let bytes = header.as_flattened_mut();
            if let Some(byte) = first_byte {
                let (head, tail) = bytes.split_at_mut(1);
                head.copy_from_slice(&[byte]);
                socket.read_exact(tail).await?;
            } else {
                socket.read_exact(bytes).await?;
            }
            let [opcode, length] = header;
            let opcode = u32::from_le_bytes(opcode);
            let length = u32::from_le_bytes(length);
            if length > MAX_PAYLOAD {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Discord IPC frame exceeds 64 KiB",
                ));
            }
            let length = usize::try_from(length).map_err(io::Error::other)?;
            let mut payload = vec![0; length];
            socket.read_exact(&mut payload).await?;
            Ok((opcode, payload))
        })
        .map_err(Error::ReadError)
    }
    pub(super) fn pong(&mut self, payload: &[u8]) -> Result<(), Error> {
        let length = u32::try_from(payload.len())
            .map_err(|error| Error::WriteError(io::Error::other(error)))?;
        let mut frame = Vec::with_capacity(8_usize.saturating_add(payload.len()));
        frame.extend_from_slice(&4_u32.to_le_bytes());
        frame.extend_from_slice(&length.to_le_bytes());
        frame.extend_from_slice(payload);
        self.write(&frame)
    }
    #[cfg(all(test, unix))]
    pub(super) fn with_path(
        runtime: Handle,
        state: watch::Receiver<Desired>,
        path: std::path::PathBuf,
    ) -> Self {
        let mut transport = Self::new(runtime, state);
        transport.test_path = Some(path);
        transport
    }
}
fn bounded<T>(
    runtime: &Handle,
    state: &mut watch::Receiver<Desired>,
    cancellable: bool,
    operation: impl Future<Output = io::Result<T>>,
) -> io::Result<T> {
    runtime.block_on(async {
        tokio::select! {
            biased;
            _ = state.wait_for(|value| value.shutdown || !value.enabled), if cancellable =>
                Err(io::Error::new(io::ErrorKind::Interrupted, "RPC disabled")),
            result = tokio::time::timeout(IO_TIMEOUT, operation) =>
                result.unwrap_or_else(|_| Err(io::Error::new(io::ErrorKind::TimedOut, "Discord IPC timeout"))),
        }
    })
}
impl DiscordIpc for Transport {
    fn get_client_id(&self) -> &str {
        APPLICATION_ID
    }
    fn connect_ipc(&mut self) -> Result<(), Error> {
        #[cfg(all(test, unix))]
        if let Some(path) = &self.test_path {
            self.socket = Some(
                bounded(&self.runtime, &mut self.state, true, Socket::connect(path))
                    .map_err(Error::ReadError)?,
            );
            return Ok(());
        }
        self.socket =
            Some(bounded(&self.runtime, &mut self.state, true, open()).map_err(Error::ReadError)?);
        Ok(())
    }
    fn send_handshake(&mut self) -> Result<(), Error> {
        self.send(
            serde_json::json!({ "v": 1, "client_id": APPLICATION_ID }),
            0,
        )?;
        let (opcode, payload) = self.recv()?;
        if opcode == 1 && payload.get("evt").and_then(serde_json::Value::as_str) == Some("READY") {
            Ok(())
        } else {
            Err(Error::ReadError(io::Error::new(
                io::ErrorKind::InvalidData,
                "Discord handshake expected READY",
            )))
        }
    }
    fn recv(&mut self) -> Result<(u32, serde_json::Value), Error> {
        let (opcode, payload) = self.recv_raw()?;
        let value = serde_json::from_slice(&payload).map_err(|_| Error::JsonParseResponse)?;
        Ok((opcode, value))
    }
    fn read(&mut self, buffer: &mut [u8]) -> Result<(), Error> {
        let socket = self.socket.as_mut().ok_or(Error::NotConnected)?;
        bounded(
            &self.runtime,
            &mut self.state,
            self.cancellable,
            socket.read_exact(buffer),
        )
        .map_err(Error::ReadError)?;
        Ok(())
    }
    fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        let socket = self.socket.as_mut().ok_or(Error::NotConnected)?;
        bounded(
            &self.runtime,
            &mut self.state,
            self.cancellable,
            socket.write_all(data),
        )
        .map_err(Error::WriteError)
    }
    fn close(&mut self) -> Result<(), Error> {
        // Dropping the socket is sufficient and cannot block on an unresponsive peer.
        self.socket = None;
        self.first_byte = None;
        Ok(())
    }
}
// Same compatible layouts as discord-rich-presence, including Vesktop and sandboxes.
#[cfg(unix)]
const SUBPATHS: [&str; 9] = [
    "",
    "app/com.discordapp.Discord",
    "app/dev.vencord.Vesktop",
    ".flatpak/com.discordapp.Discord/xdg-run",
    ".flatpak/dev.vencord.Vesktop/xdg-run",
    ".flatpak/com.discordapp.Discord",
    ".flatpak/dev.vencord.Vesktop",
    "snap.discord",
    "snap.discord-canary",
];

#[cfg(unix)]
async fn open() -> io::Result<Socket> {
    let mut roots: Vec<std::path::PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(Into::into)
        .collect();
    if std::env::var_os("SNAP").is_some()
        && let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR")
        && let Some(parent) = std::path::Path::new(&runtime).parent()
    {
        roots.push(parent.to_owned());
    }
    roots.push(std::env::temp_dir());
    for root in roots {
        for subpath in SUBPATHS {
            for index in 0..10 {
                if let Ok(socket) =
                    Socket::connect(root.join(subpath).join(format!("discord-ipc-{index}"))).await
                {
                    return Ok(socket);
                }
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Discord IPC unavailable",
    ))
}
#[cfg(windows)]
async fn open() -> io::Result<Socket> {
    for index in 0..10 {
        if let Ok(socket) = tokio::net::windows::named_pipe::ClientOptions::new()
            .open(format!(r"\\.\pipe\discord-ipc-{index}"))
        {
            return Ok(socket);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "Discord IPC unavailable",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn disconnected_operations_and_wrapping_nonces_preserve_transport_state() {
        let (_sender, receiver) = watch::channel(Desired::default());
        let mut transport = Transport::new(Handle::current(), receiver);
        assert_eq!(transport.get_client_id(), APPLICATION_ID);
        assert!(matches!(transport.ready().await, Err(Error::NotConnected)));
        assert!(matches!(transport.recv_raw(), Err(Error::NotConnected)));
        assert!(matches!(transport.read(&mut [0]), Err(Error::NotConnected)));
        assert!(matches!(transport.write(&[1]), Err(Error::NotConnected)));
        assert!(matches!(transport.pong(&[2]), Err(Error::NotConnected)));
        assert!(matches!(
            crate::discord::Rpc::clear(&mut transport),
            Err(Error::NotConnected)
        ));
        assert!(
            transport.cancellable,
            "a failed clear restores normal cancellation"
        );
        transport.nonce = u64::MAX;
        assert_eq!(transport.next_nonce(), "0");
        assert_eq!(transport.next_nonce(), "1");
        DiscordIpc::close(&mut transport).unwrap();
        DiscordIpc::close(&mut transport).unwrap();
        assert!(transport.socket.is_none() && transport.first_byte.is_none());
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[allow(clippy::too_many_lines)]
    async fn unix_discovery_uses_only_child_environment_roots_and_reports_absence() {
        const PROBE: &str = "OSU_RADIO_TEST_DISCORD_SOCKET";
        const MISSING: &str = "OSU_RADIO_TEST_DISCORD_MISSING";
        if let Some(path) = std::env::var_os(PROBE) {
            let (_sender, receiver) = watch::channel(Desired {
                enabled: true,
                ..Desired::default()
            });
            let mut transport = Transport::new(Handle::current(), receiver);
            if std::env::var_os(MISSING).is_some() {
                let error = tokio::task::spawn_blocking(move || transport.connect_ipc())
                    .await
                    .unwrap()
                    .unwrap_err();
                assert!(
                    matches!(error, Error::ReadError(ref error) if error.kind() == io::ErrorKind::NotFound)
                );
                return;
            }
            let path = std::path::PathBuf::from(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let listener = tokio::net::UnixListener::bind(&path).unwrap();
            let task = tokio::task::spawn_blocking(move || {
                transport.connect_ipc().unwrap();
                let mut bytes = [0; 3];
                transport.read(&mut bytes).unwrap();
                assert_eq!(bytes, [3, 2, 1]);
                DiscordIpc::close(&mut transport).unwrap();
                assert!(matches!(
                    transport.read(&mut bytes),
                    Err(Error::NotConnected)
                ));
            });
            let (mut peer, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
            peer.write_all(&[3, 2, 1]).await.unwrap();
            tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap();
            return;
        }

        for (root_name, subpath, index) in [
            ("XDG_RUNTIME_DIR", "", 0),
            ("TMPDIR", "app/dev.vencord.Vesktop", 9),
            ("TMP", ".flatpak/com.discordapp.Discord/xdg-run", 4),
            ("TEMP", "snap.discord-canary", 8),
            ("SNAP", ".flatpak/dev.vencord.Vesktop", 1),
            ("MISSING", "", 0),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "discord::transport::tests::unix_discovery_uses_only_child_environment_roots_and_reports_absence", "--nocapture", "--test-threads=1"])
                .env_clear().current_dir(directory.path()).kill_on_drop(true);
            let mut selected = directory.path().join("unused");
            for name in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
                let path = directory.path().join(name);
                std::fs::create_dir_all(&path).unwrap();
                command.env(name, &path);
                if name == root_name {
                    selected = path;
                }
            }
            if root_name == "SNAP" {
                selected = directory.path().to_owned();
                command.env("SNAP", "fixture");
            }
            if root_name == "MISSING" {
                command.env(MISSING, "1");
            }
            command.env(
                PROBE,
                selected.join(subpath).join(format!("discord-ipc-{index}")),
            );
            if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
                command.env("LLVM_PROFILE_FILE", profile);
            }
            let output = tokio::time::timeout(Duration::from_secs(10), command.output())
                .await
                .unwrap()
                .unwrap();
            assert!(
                output.status.success(),
                "{root_name}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        }
    }
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancelling_first_byte_wait_preserves_frame_and_close_discards_saved_byte() {
        let (_sender, receiver) = watch::channel(Desired {
            enabled: true,
            ..Desired::default()
        });
        let mut transport = Transport::new(Handle::current(), receiver);
        let (socket, mut peer) = Socket::pair().unwrap();
        transport.socket = Some(socket);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), transport.ready())
                .await
                .is_err()
        );
        peer.write_all(&[3, 0, 0, 0, 2, 0, 0, 0, 0xff, 0])
            .await
            .unwrap();
        transport.ready().await.unwrap();
        let mut transport = tokio::task::spawn_blocking(move || {
            assert_eq!(transport.recv_raw().unwrap(), (3, vec![0xff, 0]));
            transport
        })
        .await
        .unwrap();
        peer.write_all(&[1]).await.unwrap();
        transport.ready().await.unwrap();
        assert_eq!(transport.first_byte, Some(1));
        transport.close().unwrap();
        assert_eq!(transport.first_byte, None);
        assert!(matches!(transport.ready().await, Err(Error::NotConnected)));
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unresponsive_io_times_out_and_disable_interrupts_an_inflight_read() {
        let (sender, receiver) = watch::channel(Desired {
            enabled: true,
            ..Desired::default()
        });
        let runtime = Handle::current();
        let mut timeout_state = receiver.clone();
        let runtime_copy = runtime.clone();
        let result = tokio::task::spawn_blocking(move || {
            bounded(
                &runtime_copy,
                &mut timeout_state,
                true,
                std::future::pending::<io::Result<()>>(),
            )
        })
        .await
        .unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        let mut cancel_state = receiver;
        let task = tokio::task::spawn_blocking(move || {
            bounded(
                &runtime,
                &mut cancel_state,
                true,
                std::future::pending::<io::Result<()>>(),
            )
        });
        sender.send_modify(|value| value.enabled = false);
        let result = tokio::time::timeout(Duration::from_millis(200), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
    }
}
