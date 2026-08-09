#![allow(clippy::expect_used)]

use std::{env, path::PathBuf};

use osu_radio_client::{ServerOptions, Session};

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
#[ignore = "needs a built osu-radio-server and the .env it loads"]
async fn the_embedded_server_answers_on_the_port_it_reports() {
    let options = ServerOptions {
        binary: Some(server_binary()),
        working_directory: Some(workspace_root()),
        ..ServerOptions::default()
    };

    let session = Session::start(options)
        .await
        .expect("the embedded server should start");

    assert!(
        session.base_url().starts_with("http://127.0.0.1:"),
        "unexpected base url: {}",
        session.base_url()
    );
    assert!(
        !session.base_url().ends_with(":0"),
        "the reported address should be the port the OS assigned"
    );

    let user_data = session
        .api()
        .user_data()
        .await
        .expect("the settings row should be readable");
    assert_eq!(user_data.id, 1);

    session
        .api()
        .beatmap_sets()
        .await
        .expect("beatmap sets should be readable");

    session
        .shutdown()
        .await
        .expect("the embedded server should stop");
}
