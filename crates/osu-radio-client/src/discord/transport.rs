//! Socket I/O only. Handshake, frame encoding, Activity, clear and decode belong to the crate.
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
    pub(super) cancellable: bool,
    pub(super) use_assets: bool,
}
impl Transport {
    pub(super) const fn new(runtime: Handle, state: watch::Receiver<Desired>) -> Self {
        Self {
            runtime,
            state,
            socket: None,
            cancellable: true,
            use_assets: true,
        }
    }
    pub(super) async fn ready(&self) {
        if let Some(socket) = &self.socket {
            let _ = socket.readable().await;
        }
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
        self.socket = Some(
            bounded(&self.runtime, &mut self.state, true, open())
                .map_err(|_| Error::IPCConnectionFailed)?,
        );
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
            Err(Error::IPCConnectionFailed)
        }
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
