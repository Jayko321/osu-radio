use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const ASSIGNMENT: &str = r#"{"current_audio_source_id":7,"track":{"audio_source_id":7,"title":"曲","title_unicode":null,"artist":null,"artist_unicode":null,"cover_beatmap_id":77,"difficulties":[]},"duration_ms":1234,"mode":"playing","revision":2,"playback_token":5,"can_next":true,"can_previous":true}"#;

async fn request(stream: &mut tokio::net::TcpStream) {
    let mut request = Vec::new();
    while !request.ends_with(b"\r\n\r\n") {
        request.push(stream.read_u8().await.unwrap());
    }
    assert!(request.starts_with(b"GET /api/playback/events HTTP/1.1"));
}
#[tokio::test]
async fn playback_stream_splits_utf8_skips_heartbeats_and_cancellation_closes_it() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = ApiClient::new(format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let (sent, mut updates) = tokio::sync::mpsc::unbounded_channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        request(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
            .await
            .unwrap();
        for byte in format!("\n{ASSIGNMENT}\n\n").as_bytes() {
            stream
                .write_all(&[b'1', b'\r', b'\n', *byte, b'\r', b'\n'])
                .await
                .unwrap();
        }
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(3), stream.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    });
    let stream = tokio::spawn(async move {
        api.playback_events(move |assignment| {
            let _ = sent.send(assignment);
        })
        .await
    });
    let assignment = tokio::time::timeout(std::time::Duration::from_secs(3), updates.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(assignment.playback_token, 5);
    assert_eq!(assignment.track.unwrap().title.as_deref(), Some("曲"));
    assert!(updates.try_recv().is_err());
    stream.abort();
    assert!(stream.await.unwrap_err().is_cancelled());
    server.await.unwrap();
}
#[tokio::test]
async fn playback_stream_overrides_thirty_second_timeout_and_rejects_invalid_or_truncated_events() {
    for body in [
        format!("{ASSIGNMENT}\n"),
        "{invalid}\n".into(),
        ASSIGNMENT.into(),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = ApiClient::new(format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let (ready, waiting) = tokio::sync::oneshot::channel();
        let (release, held) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            request(&mut stream).await;
            ready.send(()).unwrap();
            held.await.unwrap();
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let stream = tokio::spawn(async move { api.playback_events(|_| {}).await });
        waiting.await.unwrap();
        tokio::time::pause();
        tokio::time::advance(std::time::Duration::from_secs(31)).await;
        tokio::task::yield_now().await;
        assert!(!stream.is_finished());
        tokio::time::resume();
        release.send(()).unwrap();
        assert!(stream.await.unwrap().is_err());
        server.await.unwrap();
    }
}

#[test]
fn playback_command_wire_shapes_are_explicit_and_token_bound() {
    use crate::models::PlaybackCommand;
    for (command, value) in [
        (
            PlaybackCommand::Play {
                audio_source_id: None,
            },
            serde_json::json!({"command":"play"}),
        ),
        (
            PlaybackCommand::Play {
                audio_source_id: Some(7),
            },
            serde_json::json!({"command":"play","audio_source_id":7}),
        ),
        (
            PlaybackCommand::Pause,
            serde_json::json!({"command":"pause"}),
        ),
        (PlaybackCommand::Stop, serde_json::json!({"command":"stop"})),
        (PlaybackCommand::Next, serde_json::json!({"command":"next"})),
        (
            PlaybackCommand::Previous,
            serde_json::json!({"command":"previous"}),
        ),
        (
            PlaybackCommand::Finished { playback_token: 8 },
            serde_json::json!({"command":"finished","playback_token":8}),
        ),
        (
            PlaybackCommand::Failed { playback_token: 8 },
            serde_json::json!({"command":"failed","playback_token":8}),
        ),
    ] {
        assert_eq!(serde_json::to_value(command).unwrap(), value);
    }
}
