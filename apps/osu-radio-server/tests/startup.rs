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
        (
            None,
            if cfg!(any(target_os = "linux", windows)) {
                "Cannot determine SQLite data directory"
            } else {
                "SQLITE_DATABASE_URL must be set"
            },
        ),
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

#[cfg(any(target_os = "linux", windows))]
const DATA_HOME_ENV: &str = if cfg!(windows) {
    "LOCALAPPDATA"
} else {
    "XDG_DATA_HOME"
};

#[cfg(any(target_os = "linux", windows))]
#[tokio::test]
async fn default_directory_failure_exits_before_readiness() {
    let directory = TempDir::new().unwrap();
    let blocked = directory.path().join("blocked");
    fs::write(&blocked, b"file instead of directory").unwrap();
    let mut command = isolated_command(&directory);
    command
        .env_remove("SQLITE_DATABASE_URL")
        .env(DATA_HOME_ENV, blocked);
    assert_startup_failure(command, "Failed to create SQLite data directory").await;
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn default_database_survives_restart_without_dotenv_or_a_database_url() {
    use osu_radio_client::{ApiClient, RegisterOsuFolder};

    for xdg in [None, Some(""), Some("relative/data"), Some("absolute")] {
        let directory = TempDir::new().unwrap();
        let home = directory.path().join("home with spaces");
        let data = directory.path().join("data with spaces");
        let expected = if xdg == Some("absolute") {
            data.clone()
        } else {
            home.join(".local/share")
        }
        .join("osu-radio/osu-radio.db");
        let folder = directory.path().join("osu");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("client.realm"), b"source remains intact").unwrap();
        for restart in [false, true] {
            let mut command = isolated_command(&directory);
            command.env_remove("SQLITE_DATABASE_URL").env("HOME", &home);
            if let Some(value) = xdg {
                command.env(
                    "XDG_DATA_HOME",
                    if value == "absolute" {
                        data.as_os_str()
                    } else {
                        std::ffi::OsStr::new(value)
                    },
                );
            }
            let mut child = command.spawn().unwrap();
            let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
            let line = timeout(Duration::from_secs(30), lines.next_line())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let api = ApiClient::new(line.strip_prefix("osu-radio server listening on ").unwrap())
                .unwrap();
            if restart {
                let folders = api.osu_folders().await.unwrap();
                assert_eq!(folders.len(), 1);
                assert_eq!(folders.first().unwrap().label.as_deref(), Some("Persisted"));
            } else {
                api.register_osu_folder(&RegisterOsuFolder::labelled(&folder, "Persisted"))
                    .await
                    .unwrap();
            }
            child.kill().await.unwrap();
            assert!(expected.is_file());
            assert!(!directory.path().join("osu-radio.db").exists());
            assert!(!directory.path().join("library.sqlite").exists());
            assert_eq!(
                fs::read(folder.join("client.realm")).unwrap(),
                b"source remains intact"
            );
        }
    }
}

#[cfg(windows)]
#[tokio::test]
async fn windows_default_database_survives_restart_in_local_app_data() {
    use osu_radio_client::{ApiClient, RegisterOsuFolder};

    let directory = TempDir::new().unwrap();
    let local = directory.path().join("Local с пробелами");
    let expected = local.join("osu-radio").join("osu-radio.db");
    let folder = directory.path().join("osu");
    fs::create_dir(&folder).unwrap();
    for restart in [false, true] {
        let mut command = isolated_command(&directory);
        command
            .env_remove("SQLITE_DATABASE_URL")
            .env("LOCALAPPDATA", &local);
        let mut child = command.spawn().unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let line = timeout(Duration::from_secs(30), lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let api =
            ApiClient::new(line.strip_prefix("osu-radio server listening on ").unwrap()).unwrap();
        if restart {
            assert_eq!(api.osu_folders().await.unwrap().len(), 1);
        } else {
            api.register_osu_folder(&RegisterOsuFolder::labelled(&folder, "Persisted"))
                .await
                .unwrap();
        }
        child.kill().await.unwrap();
        assert!(expected.is_file());
        assert!(!directory.path().join("osu-radio.db").exists());
    }
}

#[tokio::test]
async fn startup_database_connection_failure_is_reported_before_binding() {
    let directory = TempDir::new().unwrap();
    let mut command = isolated_command(&directory);
    command.env("SQLITE_DATABASE_URL", "missing-parent/library.sqlite");
    command
        .env("XDG_DATA_HOME", directory.path().join("unused-data"))
        .env("LOCALAPPDATA", directory.path().join("unused-data"));
    assert_startup_failure(command, "Failed to connect to the configured database").await;
    assert!(!directory.path().join("missing-parent").exists());
    assert!(!directory.path().join("unused-data").exists());
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
