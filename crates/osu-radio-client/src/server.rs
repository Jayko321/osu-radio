use std::{
    env,
    error::Error,
    fmt, io,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Mutex,
    time::Duration,
};

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    time,
};

const BINARY_ENV: &str = "OSU_RADIO_SERVER_BIN";
const BINARY_NAME: &str = "osu-radio-server";
const ADDRESS_ENV: &str = "OSU_RADIO_SERVER_ADDRESS";

/// Asking for port 0 makes the OS pick a free port, so an embedded server never collides with a
/// separately running one. The server prints the address it actually bound, which is how the
/// supervisor learns the port.
const EPHEMERAL_ADDRESS: &str = "127.0.0.1:0";
const READY_MARKER: &str = "listening on ";

#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// Connect to this existing server instead of starting a child process.
    pub remote_url: Option<String>,
    pub binary: Option<PathBuf>,
    pub working_directory: Option<PathBuf>,
    pub address: String,
    pub startup_timeout: Duration,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            remote_url: None,
            binary: None,
            working_directory: None,
            address: EPHEMERAL_ADDRESS.to_owned(),
            startup_timeout: Duration::from_secs(30),
        }
    }
}

/// A child `osu-radio-server` owned by this process. Dropping it kills the server.
#[derive(Debug)]
pub struct EmbeddedServer {
    child: Mutex<Option<Child>>,
    base_url: String,
}

impl EmbeddedServer {
    pub async fn start(options: ServerOptions) -> Result<Self, ServerError> {
        let (_cancel, receiver) = tokio::sync::watch::channel(false);
        Self::start_cancellable(options, receiver).await
    }

    pub(crate) async fn start_cancellable(
        options: ServerOptions,
        mut cancelled: tokio::sync::watch::Receiver<bool>,
    ) -> Result<Self, ServerError> {
        let binary = resolve_binary(options.binary.as_deref())?;

        let mut command = Command::new(&binary);
        command
            .env(ADDRESS_ENV, &options.address)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        if let Some(directory) = &options.working_directory {
            command.current_dir(directory);
        }

        let mut child = command.spawn().map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                ServerError::BinaryNotFound(binary.clone())
            } else {
                ServerError::Spawn {
                    binary: binary.clone(),
                    error,
                }
            }
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or(ServerError::MissingPipe("stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(ServerError::MissingPipe("stderr"))?;

        forward(stderr, "server");

        let mut lines = BufReader::new(stdout).lines();
        // The deadline includes EOF followed by a still-live child. Cancellation and every
        // startup failure reap the child while the caller's runtime is still alive.
        let ready = async {
            match read_ready_line(&mut lines).await {
                Ok(Some(url)) => Ok(url),
                Ok(None) => {
                    let status = child.wait().await.ok();
                    Err(ServerError::Exited(status.map(|status| status.to_string())))
                }
                Err(error) => Err(ServerError::ReadOutput(error)),
            }
        };
        let result = tokio::select! {
            result = time::timeout(options.startup_timeout, ready) => {
                result.unwrap_or(Err(ServerError::StartupTimedOut(options.startup_timeout)))
            }
            _ = cancelled.wait_for(|value| *value) => Err(ServerError::Cancelled),
        };
        let base_url = match result {
            Ok(url) => url,
            Err(error) => {
                let _ = child.kill().await;
                return Err(error);
            }
        };

        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                println!("[server] {line}");
            }
        });

        Ok(Self {
            child: Mutex::new(Some(child)),
            base_url,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Stops the server and waits for it to exit. Dropping the value kills it too, but without
    /// waiting, so prefer this wherever the caller can await.
    pub async fn shutdown(&self) -> Result<(), ServerError> {
        match self.take_child() {
            Some(mut child) => child.kill().await.map_err(ServerError::Shutdown),
            None => Ok(()),
        }
    }

    fn take_child(&self) -> Option<Child> {
        match self.child.lock() {
            Ok(mut guard) => guard.take(),
            Err(poisoned) => poisoned.into_inner().take(),
        }
    }
}

impl Drop for EmbeddedServer {
    fn drop(&mut self) {
        if let Some(mut child) = self.take_child() {
            let _ = child.start_kill();
        }
    }
}

async fn read_ready_line<R>(
    lines: &mut tokio::io::Lines<BufReader<R>>,
) -> io::Result<Option<String>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    while let Some(line) = lines.next_line().await? {
        println!("[server] {line}");

        if let Some(base_url) = parse_ready_line(&line) {
            return Ok(Some(base_url));
        }
    }

    Ok(None)
}

fn parse_ready_line(line: &str) -> Option<String> {
    let address = line.split_once(READY_MARKER)?.1.trim();

    if address.starts_with("http://") || address.starts_with("https://") {
        Some(address.to_owned())
    } else {
        None
    }
}

fn resolve_binary(configured: Option<&Path>) -> Result<PathBuf, ServerError> {
    if let Some(binary) = configured {
        return Ok(binary.to_path_buf());
    }

    if let Some(binary) = env::var_os(BINARY_ENV) {
        return Ok(PathBuf::from(binary));
    }

    let file_name = format!("{BINARY_NAME}{}", env::consts::EXE_SUFFIX);
    let alongside = env::current_exe()
        .map_err(ServerError::LocateSelf)?
        .parent()
        .map(|directory| directory.join(&file_name));

    match alongside {
        Some(binary) if binary.is_file() => Ok(binary),
        _ => Ok(PathBuf::from(file_name)),
    }
}

fn forward<R>(reader: R, label: &'static str)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();

        while let Ok(Some(line)) = lines.next_line().await {
            eprintln!("[{label}] {line}");
        }
    });
}

#[derive(Debug)]
pub enum ServerError {
    BinaryNotFound(PathBuf),
    LocateSelf(io::Error),
    Spawn { binary: PathBuf, error: io::Error },
    MissingPipe(&'static str),
    ReadOutput(io::Error),
    StartupTimedOut(Duration),
    Exited(Option<String>),
    Shutdown(io::Error),
    Cancelled,
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinaryNotFound(binary) => write!(
                formatter,
                "`{}` was not found. Build it with `cargo build -p osu-radio-server`, or point {BINARY_ENV} at it.",
                binary.display()
            ),
            Self::LocateSelf(error) => write!(
                formatter,
                "the running executable could not be located: {error}"
            ),
            Self::Spawn { binary, error } => write!(
                formatter,
                "`{}` could not be started: {error}",
                binary.display()
            ),
            Self::MissingPipe(pipe) => {
                write!(formatter, "the server was started without a {pipe} pipe")
            }
            Self::ReadOutput(error) => {
                write!(formatter, "the server output could not be read: {error}")
            }
            Self::StartupTimedOut(timeout) => write!(
                formatter,
                "the server did not report an address within {timeout:?}"
            ),
            Self::Exited(Some(status)) => write!(
                formatter,
                "the server stopped before it reported an address ({status}). SQLITE_DATABASE_URL must be set in the .env it loads."
            ),
            Self::Exited(None) => write!(
                formatter,
                "the server stopped before it reported an address. SQLITE_DATABASE_URL must be set in the .env it loads."
            ),
            Self::Cancelled => write!(formatter, "server startup was cancelled"),
            Self::Shutdown(error) => write!(formatter, "the server could not be stopped: {error}"),
        }
    }
}

impl Error for ServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LocateSelf(error)
            | Self::Spawn { error, .. }
            | Self::ReadOutput(error)
            | Self::Shutdown(error) => Some(error),
            Self::BinaryNotFound(_)
            | Self::MissingPipe(_)
            | Self::StartupTimedOut(_)
            | Self::Exited(_)
            | Self::Cancelled => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_selection_uses_explicit_override_environment_sibling_then_path() {
        const EXPECTED: &str = "OSU_RADIO_TEST_RESOLVED_BINARY";
        const EXPLICIT: &str = "OSU_RADIO_TEST_EXPLICIT_BINARY";
        if let Some(expected) = env::var_os(EXPECTED) {
            let explicit = env::var_os(EXPLICIT).map(PathBuf::from);
            assert_eq!(
                resolve_binary(explicit.as_deref()).unwrap(),
                PathBuf::from(expected)
            );
            return;
        }
        let executable = env::current_exe().unwrap();
        let directory = tempfile::tempdir_in(executable.parent().unwrap()).unwrap();
        let probe = directory
            .path()
            .join(format!("probe{}", env::consts::EXE_SUFFIX));
        // A private hard link changes current_exe's directory without copying or editing the test binary.
        std::fs::hard_link(&executable, &probe).unwrap();
        let file_name = format!("osu-radio-server{}", env::consts::EXE_SUFFIX);
        let sibling = directory.path().join(&file_name);
        std::fs::write(&sibling, b"resolution fixture, never executed").unwrap();
        for (explicit, environment, expected) in [
            (
                Some(PathBuf::from("explicit/server")),
                Some("environment/server"),
                PathBuf::from("explicit/server"),
            ),
            (
                None,
                Some("environment/server"),
                PathBuf::from("environment/server"),
            ),
            (None, None, sibling.clone()),
            (None, None, PathBuf::from(file_name)),
        ] {
            if expected.file_name() == sibling.file_name() && expected.is_relative() {
                std::fs::remove_file(&sibling).unwrap();
            }
            let mut command = std::process::Command::new(&probe);
            command.args(["--exact", "server::tests::binary_selection_uses_explicit_override_environment_sibling_then_path", "--nocapture"])
                .env_clear().env(EXPECTED, &expected);
            if let Some(explicit) = explicit {
                command.env(EXPLICIT, explicit);
            }
            if let Some(environment) = environment {
                command.env(BINARY_ENV, environment);
            }
            if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
                command.env("LLVM_PROFILE_FILE", profile);
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn the_bound_address_is_read_back_from_the_startup_line() {
        let line = "osu-radio server listening on http://127.0.0.1:54321";

        assert_eq!(
            parse_ready_line(line).as_deref(),
            Some("http://127.0.0.1:54321")
        );
    }

    #[test]
    fn other_startup_lines_are_not_mistaken_for_the_address() {
        assert_eq!(
            parse_ready_line("GET http://127.0.0.1:3000/api/beatmap-sets returns every set."),
            None
        );
        assert_eq!(
            parse_ready_line("Scalar API reference: http://x/docs"),
            None
        );
        assert_eq!(parse_ready_line("listening on ftp://fixture"), None);
        assert_eq!(parse_ready_line("listening on    "), None);
        assert_eq!(
            parse_ready_line("listening on  https://fixture:443  ").as_deref(),
            Some("https://fixture:443")
        );
    }

    #[test]
    fn errors_keep_actionable_context_and_only_io_variants_have_a_source() {
        for (error, expected, has_source) in [
            (
                ServerError::BinaryNotFound("fixture-binary".into()),
                "`fixture-binary` was not found. Build it with `cargo build -p osu-radio-server`, or point OSU_RADIO_SERVER_BIN at it.",
                false,
            ),
            (
                ServerError::LocateSelf(io::Error::other("fixture io")),
                "the running executable could not be located: fixture io",
                true,
            ),
            (
                ServerError::Spawn {
                    binary: "fixture-binary".into(),
                    error: io::Error::other("fixture io"),
                },
                "`fixture-binary` could not be started: fixture io",
                true,
            ),
            (
                ServerError::MissingPipe("stdout"),
                "the server was started without a stdout pipe",
                false,
            ),
            (
                ServerError::ReadOutput(io::Error::other("fixture io")),
                "the server output could not be read: fixture io",
                true,
            ),
            (
                ServerError::StartupTimedOut(Duration::from_secs(2)),
                "the server did not report an address within 2s",
                false,
            ),
            (
                ServerError::Exited(Some("fixture exit".into())),
                "the server stopped before it reported an address (fixture exit). SQLITE_DATABASE_URL must be set in the .env it loads.",
                false,
            ),
            (
                ServerError::Exited(None),
                "the server stopped before it reported an address. SQLITE_DATABASE_URL must be set in the .env it loads.",
                false,
            ),
            (
                ServerError::Shutdown(io::Error::other("fixture io")),
                "the server could not be stopped: fixture io",
                true,
            ),
            (
                ServerError::Cancelled,
                "server startup was cancelled",
                false,
            ),
        ] {
            assert_eq!(error.to_string(), expected);
            assert_eq!(error.source().is_some(), has_source);
            if let Some(source) = error.source() {
                assert_eq!(source.to_string(), "fixture io");
            }
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod lifecycle_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fixture(body: &str) -> (tempfile::TempDir, ServerOptions) {
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("server");
        std::fs::write(&binary, format!("#!/bin/sh\necho $$ > pid\n{body}\n")).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let options = ServerOptions {
            binary: Some(binary),
            working_directory: Some(directory.path().into()),
            startup_timeout: Duration::from_millis(120),
            ..Default::default()
        };
        (directory, options)
    }
    fn assert_reaped(directory: &Path) {
        let pid = std::fs::read_to_string(directory.join("pid")).unwrap();
        assert!(
            !Path::new(&format!("/proc/{}", pid.trim())).exists(),
            "child must already be reaped when cleanup completes"
        );
    }
    #[tokio::test]
    async fn startup_timeout_reaps_silent_child() {
        let (directory, options) = fixture("exec sleep 30");
        let result = time::timeout(Duration::from_secs(3), EmbeddedServer::start(options))
            .await
            .unwrap();
        assert!(
            matches!(result, Err(ServerError::StartupTimedOut(_))),
            "{result:?}"
        );
        assert_reaped(directory.path());
    }
    #[tokio::test]
    async fn stdout_eof_does_not_bypass_startup_timeout() {
        let (directory, options) = fixture("exec 1>&-\nexec sleep 30");
        let result = time::timeout(Duration::from_secs(3), EmbeddedServer::start(options))
            .await
            .unwrap();
        assert!(
            matches!(result, Err(ServerError::StartupTimedOut(_))),
            "{result:?}"
        );
        assert_reaped(directory.path());
    }
    #[tokio::test]
    async fn controller_close_during_startup_awaits_child_cleanup() {
        let (directory, mut options) = fixture("exec 1>&-\nexec sleep 30");
        options.startup_timeout = Duration::from_secs(30);
        let controller = crate::controller::AppController::spawn(
            &tokio::runtime::Handle::current(),
            options,
            |_| {},
        );
        controller.send(crate::controller::AppCommand::Connect);
        time::timeout(Duration::from_secs(3), async {
            while !directory.path().join("pid").exists() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        time::timeout(Duration::from_secs(3), controller.shutdown())
            .await
            .unwrap();
        assert_reaped(directory.path());
    }
    #[tokio::test]
    async fn ready_child_is_reaped_before_shutdown_returns() {
        let (directory, options) =
            fixture("echo 'listening on http://127.0.0.1:54321'\nexec sleep 30");
        let server = EmbeddedServer::start(options).await.unwrap();
        server.shutdown().await.unwrap();
        server.shutdown().await.unwrap();
        assert_reaped(directory.path());
    }

    #[tokio::test]
    async fn missing_and_nonexecutable_binaries_have_distinct_spawn_errors() {
        let (directory, mut options) = fixture("exit 0");
        let binary = options.binary.clone().unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o600)).unwrap();
        let error = EmbeddedServer::start(options.clone()).await.unwrap_err();
        assert!(
            matches!(error, ServerError::Spawn { binary: ref actual, ref error }
            if actual == &binary && error.kind() == io::ErrorKind::PermissionDenied)
        );
        options.binary = Some(directory.path().join("missing"));
        let error = EmbeddedServer::start(options).await.unwrap_err();
        assert!(
            matches!(error, ServerError::BinaryNotFound(ref actual) if actual == &directory.path().join("missing"))
        );
        assert!(!directory.path().join("pid").exists());
    }

    #[tokio::test]
    async fn early_exit_and_invalid_utf8_output_are_reported_after_reaping() {
        for invalid_output in [false, true] {
            let body = if invalid_output {
                "printf '\\377\\n'\nexec sleep 30"
            } else {
                "exit 7"
            };
            let (directory, options) = fixture(body);
            let error = time::timeout(Duration::from_secs(3), EmbeddedServer::start(options))
                .await
                .unwrap()
                .unwrap_err();
            if invalid_output {
                assert!(
                    matches!(error, ServerError::ReadOutput(ref error) if error.kind() == io::ErrorKind::InvalidData)
                );
            } else {
                assert!(
                    matches!(error, ServerError::Exited(Some(ref status)) if status == "exit status: 7")
                );
            }
            assert_reaped(directory.path());
        }
    }

    #[tokio::test]
    async fn session_uses_reported_address_and_acknowledges_repeated_shutdown() {
        let (directory, options) =
            fixture("echo 'listening on http://127.0.0.1:54321'\nexec sleep 30");
        let session = crate::Session::start(options).await.unwrap();
        assert_eq!(session.base_url(), "http://127.0.0.1:54321");
        assert_eq!(session.api().base_url(), session.base_url());
        session.shutdown().await.unwrap();
        session.shutdown().await.unwrap();
        assert_reaped(directory.path());
    }

    #[tokio::test]
    async fn dropping_ready_server_kills_its_child_without_explicit_shutdown() {
        let (directory, options) =
            fixture("echo 'listening on http://127.0.0.1:54321'\nexec sleep 30");
        let server = EmbeddedServer::start(options).await.unwrap();
        let pid = std::fs::read_to_string(directory.path().join("pid")).unwrap();
        let process = PathBuf::from(format!("/proc/{}", pid.trim()));
        assert!(process.exists());
        drop(server);
        time::timeout(Duration::from_secs(3), async {
            while process.exists() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_reaped(directory.path());
    }
}
