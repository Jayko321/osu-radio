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
    pub binary: Option<PathBuf>,
    pub working_directory: Option<PathBuf>,
    pub address: String,
    pub startup_timeout: Duration,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
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
        let base_url =
            match time::timeout(options.startup_timeout, read_ready_line(&mut lines)).await {
                Ok(Ok(Some(base_url))) => base_url,
                Ok(Ok(None)) => {
                    let status = child.wait().await.ok();
                    return Err(ServerError::Exited(status.map(|status| status.to_string())));
                }
                Ok(Err(error)) => return Err(ServerError::ReadOutput(error)),
                Err(_) => return Err(ServerError::StartupTimedOut(options.startup_timeout)),
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
            | Self::Exited(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ready_line;

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
    }
}
