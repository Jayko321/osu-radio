#![cfg(feature = "sqlite")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{env, process::Stdio, time::Duration};

use tempfile::TempDir;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    time::timeout,
};

#[tokio::test]
async fn server_starts_from_environment_without_dotenv() {
    let directory = TempDir::new().expect("temporary working directory");
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
    for name in ["SystemRoot", "PATH"] {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
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
