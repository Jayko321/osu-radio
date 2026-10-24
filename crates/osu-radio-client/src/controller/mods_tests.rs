use super::*;
use crate::{ModsAction, ModsEdit, models::AudioModsSettings};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
#[allow(clippy::too_many_lines)] // Keep the ordered HTTP and event-loop scenario together.
async fn mods_load_retry_and_event_loop_save_completion_use_the_real_api() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for index in 0..4 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                header.push(stream.read_u8().await.unwrap());
            }
            let header = String::from_utf8(header).unwrap();
            let length = header
                .lines()
                .find_map(|l| l.strip_prefix("content-length: "))
                .map_or(0, |l| l.parse::<usize>().unwrap());
            let mut body = vec![0; length];
            stream.read_exact(&mut body).await.unwrap();
            requests.push((header.lines().next().unwrap().to_owned(), body));
            let payload = match index {
                0 => r#"{"entries":[],"current_entry_id":null,"history_len":0,"mode":"stopped","waiting_for_check":false,"revision":0,"playback_token":0}"#.to_owned(),
                1 => r#"{"error":"load offline"}"#.to_owned(),
                // Deliberately echo the old profile on save: ACK must not replace local DT.
                _ => serde_json::to_string(&AudioModsSettings::default()).unwrap(),
            };
            let status = if index == 1 {
                "500 Internal Server Error"
            } else {
                "200 OK"
            };
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len()).as_bytes()).await.unwrap();
        }
        requests
    });
    let (mut state, _) = tests::controller();
    let (send, mut updates) = mpsc::unbounded_channel();
    state.emit = Arc::new(move |update| {
        let _ = send.send(update);
    });
    state.session = Some(
        Session::start(ServerOptions {
            remote_url: Some(format!("http://{address}")),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    state.settings.mods.loaded = false;
    // The playback stream is out of scope here; keep the fixture's request order.
    state.settings.volume.stream_started = true;
    let (commands, receiver) = mpsc::unbounded_channel();
    state.load_mods();
    let controller = tokio::spawn(state.run(receiver));
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let AppUpdate::AudioMods(s) = updates.recv().await.unwrap()
                && s.error.contains("load offline")
            {
                assert!(!s.loaded);
                break;
            }
        }
        commands.send(AppCommand::Mods(ModsAction::Retry)).unwrap();
        let target = loop {
            if let AppUpdate::AudioMods(s) = updates.recv().await.unwrap()
                && s.loaded
            {
                break s.target.unwrap();
            }
        };
        commands
            .send(AppCommand::Mods(ModsAction::Edit {
                target,
                edit: ModsEdit::CycleDt,
            }))
            .unwrap();
        let mut saving = false;
        loop {
            if let AppUpdate::AudioMods(s) = updates.recv().await.unwrap() {
                saving |= s.saving;
                if saving && !s.saving {
                    assert_eq!(s.selected_profile.speed_percent, 150);
                    assert!(s.error.is_empty());
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    commands.send(AppCommand::Shutdown).unwrap();
    tokio::time::timeout(Duration::from_secs(3), controller)
        .await
        .unwrap()
        .unwrap();
    let requests = server.await.unwrap();
    assert!(
        requests
            .get(1)
            .unwrap()
            .0
            .starts_with("GET /api/user-data/audio-mods ")
    );
    assert!(
        requests
            .get(2)
            .unwrap()
            .0
            .starts_with("GET /api/user-data/audio-mods ")
    );
    assert!(
        requests
            .get(3)
            .unwrap()
            .0
            .starts_with("PATCH /api/user-data/audio-mods ")
    );
    let body: serde_json::Value = serde_json::from_slice(&requests.get(3).unwrap().1).unwrap();
    assert_eq!(body.get("global_speed_percent").unwrap(), 150);
}
