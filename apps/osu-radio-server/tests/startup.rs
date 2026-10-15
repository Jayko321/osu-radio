#![cfg(feature = "sqlite")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{env, fs, process::Stdio, time::Duration};

use tempfile::TempDir;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    time::timeout,
};

#[tokio::test]
async fn server_starts_from_environment_without_dotenv() {
    let directory = TempDir::new().expect("temporary working directory");
    let mut command = isolated_command(&directory);
    let mut child = command.spawn().expect("launch server");
    let mut lines = BufReader::new(child.stdout.take().expect("server stdout")).lines();
    let ready = timeout(Duration::from_secs(30), lines.next_line()).await;
    // Always kill and reap the server before asserting, including failed readiness checks.
    child.kill().await.expect("kill and reap server");
    let output = child
        .wait_with_output()
        .await
        .expect("capture server stderr");
    let line = ready
        .expect("server readiness deadline")
        .expect("read readiness");
    assert!(
        line.is_some(),
        "server exited: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let line = line.expect("server readiness line");
    let address = line
        .strip_prefix("osu-radio server listening on http://")
        .expect("server readiness marker")
        .parse::<std::net::SocketAddr>()
        .expect("bound server address");
    assert_eq!(address.ip(), std::net::Ipv4Addr::LOCALHOST);
    assert_ne!(address.port(), 0);
}

#[tokio::test]
async fn startup_configuration_errors_exit_unsuccessfully_without_announcing_readiness() {
    for (contents, expected) in [
        (None, "SQLITE_DATABASE_URL must be set"),
        (
            Some(b"INVALID='unterminated\n".as_slice()),
            "Failed to load .env",
        ),
        (Some([0xff].as_slice()), "Failed to load .env"),
    ] {
        let directory = TempDir::new().unwrap();
        let mut command = isolated_command(&directory);
        if let Some(contents) = contents {
            fs::write(directory.path().join(".env"), contents).unwrap();
        } else {
            command.env_remove("SQLITE_DATABASE_URL");
        }
        assert_startup_failure(command, expected).await;
        assert!(!directory.path().join("library.sqlite").exists());
    }
}

#[tokio::test]
async fn startup_database_connection_failure_is_reported_before_binding() {
    let directory = TempDir::new().unwrap();
    let mut command = isolated_command(&directory);
    command.env("SQLITE_DATABASE_URL", "missing-parent/library.sqlite");
    assert_startup_failure(command, "Failed to connect to the configured database").await;
    assert!(!directory.path().join("missing-parent").exists());
}

#[tokio::test]
async fn startup_rejects_invalid_and_occupied_addresses() {
    let occupied = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    for address in [
        "invalid-address".to_owned(),
        occupied.local_addr().unwrap().to_string(),
    ] {
        let directory = TempDir::new().unwrap();
        let mut command = isolated_command(&directory);
        command.env("OSU_RADIO_SERVER_ADDRESS", &address);
        assert_startup_failure(command, &format!("Failed to bind `{address}`")).await;
    }
}

async fn assert_startup_failure(mut command: Command, expected: &str) {
    let output = timeout(Duration::from_secs(10), command.output())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        output.stdout.is_empty(),
        "failed startup announced readiness: {output:?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected),
        "{output:?}"
    );
}

fn isolated_command(directory: &TempDir) -> Command {
    for ancestor in directory.path().ancestors() {
        assert!(
            !ancestor.join(".env").exists(),
            "fixture must not discover an ancestor .env: {}",
            ancestor.display()
        );
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_osu-radio-server"));
    command
        .current_dir(directory.path())
        .env_clear()
        .env("SQLITE_DATABASE_URL", ":memory:")
        .env("OSU_RADIO_SERVER_ADDRESS", "127.0.0.1:0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for name in ["SystemRoot", "PATH", "LLVM_PROFILE_FILE"] {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    command
}
