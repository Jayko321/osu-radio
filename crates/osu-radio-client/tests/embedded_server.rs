#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{env, path::PathBuf};

use osu_radio_client::{OsuFolderChanges, RegisterOsuFolder, ServerOptions, Session};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .expect("the crate should sit two levels below the workspace root")
        .to_path_buf()
}

fn server_binary() -> PathBuf {
    workspace_root()
        .join("target")
        .join("debug")
        .join(format!("osu-radio-server{}", env::consts::EXE_SUFFIX))
}

#[tokio::test]
#[ignore = "needs a built SQLite osu-radio-server; uses an isolated subprocess"]
async fn the_embedded_server_answers_on_the_port_it_reports() {
    if env::var_os("OSU_RADIO_EMBEDDED_TEST_CHILD").is_none() {
        check_in_subprocess();
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let cwd = directory.path().join("child");
    std::fs::create_dir(&cwd).unwrap();
    let folder = directory.path().join("osu");
    std::fs::create_dir(&folder).unwrap();
    let marker = folder.join("client.realm");
    std::fs::write(&marker, b"source stays intact").unwrap();
    let options = ServerOptions {
        binary: Some(server_binary()),
        working_directory: Some(cwd.clone()),
        ..ServerOptions::default()
    };
    let session = Session::start(options.clone()).await.unwrap();
    assert!(!cwd.join("osu-radio.db").exists());
    assert!(!cwd.join("library.sqlite").exists());
    #[cfg(target_os = "linux")]
    assert!(
        PathBuf::from(env::var_os("XDG_DATA_HOME").unwrap())
            .join("osu-radio/osu-radio.db")
            .is_file()
    );
    assert!(session.base_url().starts_with("http://127.0.0.1:"));
    assert!(!session.base_url().ends_with(":0"));
    assert_eq!(session.api().user_data().await.unwrap().id, 1);
    assert!(session.api().beatmap_sets().await.unwrap().is_empty());
    let registered = session
        .api()
        .register_osu_folder(&RegisterOsuFolder::labelled(&folder, "Original"))
        .await
        .unwrap();
    assert!(registered.was_created());
    let id = registered.folder().id;
    let duplicate = session
        .api()
        .register_osu_folder(&RegisterOsuFolder::labelled(&folder, "Overwrite?"))
        .await
        .unwrap();
    assert!(!duplicate.was_created());
    assert_eq!(duplicate.folder().label.as_deref(), Some("Original"));
    let updated = session
        .api()
        .update_osu_folder(
            id,
            &OsuFolderChanges {
                label: Some(None),
                enabled: Some(false),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.label, None);
    assert!(!updated.enabled);
    assert!(session.api().cover(i32::MAX).await.unwrap().is_none());
    assert!(session.api().audio_duration(i32::MAX).await.is_err());
    let remote = Session::start(ServerOptions {
        remote_url: Some(session.base_url().to_owned()),
        binary: Some(directory.path().join("missing-local-server")),
        ..Default::default()
    })
    .await
    .unwrap();
    assert_eq!(
        remote.api().osu_folders().await.unwrap(),
        vec![updated.clone()]
    );
    assert!(remote.api().queue().await.unwrap().entries.is_empty());
    remote.shutdown().await.unwrap();
    drop(remote);
    assert_eq!(session.api().user_data().await.unwrap().id, 1);
    session.shutdown().await.unwrap();

    #[cfg(target_os = "linux")]
    for location in [directory.path(), cwd.as_path()] {
        for contents in [
            b"SQLITE_DATABASE_URL=must-not-open.sqlite\nOSU_RADIO_SERVER_ADDRESS=invalid\n"
                .as_slice(),
            b"INVALID='unterminated\n".as_slice(),
            &[0xff],
        ] {
            std::fs::write(location.join(".env"), contents).unwrap();
            let session = Session::start(options.clone()).await.unwrap();
            assert_eq!(
                session.api().osu_folders().await.unwrap(),
                vec![updated.clone()]
            );
            session.shutdown().await.unwrap();
            std::fs::remove_file(location.join(".env")).unwrap();
            assert!(!cwd.join("must-not-open.sqlite").exists());
        }
    }

    let session = Session::start(options).await.unwrap();
    assert_eq!(session.api().osu_folders().await.unwrap(), vec![updated]);
    session.api().remove_osu_folder(id).await.unwrap();
    assert!(session.api().osu_folders().await.unwrap().is_empty());
    assert!(session.api().beatmap_sets().await.unwrap().is_empty());
    assert_eq!(std::fs::read(marker).unwrap(), b"source stays intact");
    session.shutdown().await.unwrap();
}

fn check_in_subprocess() {
    let directory = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "the_embedded_server_answers_on_the_port_it_reports",
            "--ignored",
        ])
        .env_clear()
        .env("OSU_RADIO_EMBEDDED_TEST_CHILD", "1")
        .env("OSU_RADIO_SKIP_DOTENV", "0")
        .env("HOME", directory.path().join("home"))
        .env("XDG_DATA_HOME", directory.path().join("data with spaces"));
    #[cfg(not(target_os = "linux"))]
    command.env(
        "SQLITE_DATABASE_URL",
        directory.path().join("library.sqlite"),
    );
    for name in ["PATH", "SystemRoot", "LLVM_PROFILE_FILE"] {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
