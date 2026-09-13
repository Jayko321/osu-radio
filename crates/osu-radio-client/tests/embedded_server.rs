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
#[ignore = "needs a built SQLite osu-radio-server; unset SQLITE_DATABASE_URL and POSTGRES_DATABASE_URL"]
async fn the_embedded_server_answers_on_the_port_it_reports() {
    assert!(
        env::var_os("SQLITE_DATABASE_URL").is_none(),
        "unset SQLITE_DATABASE_URL to isolate this test"
    );
    assert!(
        env::var_os("POSTGRES_DATABASE_URL").is_none(),
        "unset POSTGRES_DATABASE_URL to isolate this test"
    );
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join(".env"),
        "SQLITE_DATABASE_URL=library.sqlite\n",
    )
    .unwrap();
    let folder = directory.path().join("osu");
    std::fs::create_dir(&folder).unwrap();
    let marker = folder.join("client.realm");
    std::fs::write(&marker, b"source stays intact").unwrap();
    let options = ServerOptions {
        binary: Some(server_binary()),
        working_directory: Some(directory.path().to_path_buf()),
        ..ServerOptions::default()
    };
    let session = Session::start(options.clone()).await.unwrap();
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
    session.shutdown().await.unwrap();

    let session = Session::start(options).await.unwrap();
    assert_eq!(session.api().osu_folders().await.unwrap(), vec![updated]);
    session.api().remove_osu_folder(id).await.unwrap();
    assert!(session.api().osu_folders().await.unwrap().is_empty());
    assert!(session.api().beatmap_sets().await.unwrap().is_empty());
    assert_eq!(std::fs::read(marker).unwrap(), b"source stays intact");
    session.shutdown().await.unwrap();
}
