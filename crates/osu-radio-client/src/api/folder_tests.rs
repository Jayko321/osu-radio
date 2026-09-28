#![allow(clippy::indexing_slicing)]
use super::*;
use crate::models::{DiscoverFolders, FolderDiscoveryEvent};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn request(stream: &mut tokio::net::TcpStream) -> String {
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        header.push(stream.read_u8().await.unwrap());
    }
    let header = String::from_utf8(header).unwrap();
    let length = header
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("content-length: ")
                .map(str::to_owned)
        })
        .unwrap_or_default()
        .parse()
        .unwrap_or(0);
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await.unwrap();
    header + &String::from_utf8(body).unwrap()
}
async fn listener() -> (ApiClient, tokio::net::TcpListener) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    (
        ApiClient::new(format!("http://{}", listener.local_addr().unwrap())).unwrap(),
        listener,
    )
}

#[tokio::test]
async fn discovery_handles_arbitrary_chunks_and_emits_before_completion() {
    let (api, listener) = listener().await;
    let (seen, observed) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = request(&mut stream).await;
        assert!(request.starts_with("POST /api/user-data/osu-folders/discover HTTP/1.1"));
        assert!(request.ends_with(r#"{"roots":[],"depth":"full"}"#));
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
            .await
            .unwrap();
        let event = "{\"event\":\"candidate\",\"kind\":\"stable\",\"root_path\":\"/曲\",\"marker_path\":\"/曲/osu!.db\",\"registered_id\":null}\n";
        for byte in event.as_bytes() {
            stream
                .write_all(&[b'1', b'\r', b'\n', *byte, b'\r', b'\n'])
                .await
                .unwrap();
        }
        // The client must deliver the candidate while the server withholds completion.
        tokio::time::timeout(std::time::Duration::from_secs(3), observed)
            .await
            .unwrap()
            .unwrap();
        let end = b"{\"event\":\"complete\"}\n";
        stream
            .write_all(format!("{:x}\r\n", end.len()).as_bytes())
            .await
            .unwrap();
        stream.write_all(end).await.unwrap();
        stream.write_all(b"\r\n0\r\n\r\n").await.unwrap();
    });
    let mut seen = Some(seen);
    let mut events = Vec::new();
    api.discover_osu_folders(&DiscoverFolders::default(), |event| {
        if matches!(event, FolderDiscoveryEvent::Candidate { .. }) {
            let _ = seen.take().unwrap().send(());
        }
        events.push(event);
    })
    .await
    .unwrap();
    assert_eq!(events.len(), 2);
    assert!(
        matches!(events[0], FolderDiscoveryEvent::Candidate { ref root_path, .. } if root_path == "/曲")
    );
    assert_eq!(events[1], FolderDiscoveryEvent::Complete);
    server.await.unwrap();
}

#[tokio::test]
async fn incomplete_or_invalid_discovery_is_an_error_and_cancellation_closes_the_response() {
    for body in ["", "{bad json}\n", "{\"event\":\"complete\"}"] {
        let (api, listener) = listener().await;
        let body = body.to_owned();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            request(&mut stream).await;
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
        assert!(
            api.discover_osu_folders(&DiscoverFolders::default(), |_| {})
                .await
                .is_err()
        );
        server.await.unwrap();
    }
    let (api, listener) = listener().await;
    let (ready, waiting) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        request(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\n\r\n")
            .await
            .unwrap();
        ready.send(()).unwrap();
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(3), stream.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    });
    let discovery = tokio::spawn(async move {
        api.discover_osu_folders(&DiscoverFolders::default(), |_| {})
            .await
    });
    waiting.await.unwrap();
    discovery.abort();
    assert!(discovery.await.unwrap_err().is_cancelled());
    server.await.unwrap();
}

#[tokio::test]
async fn folder_operations_override_the_ordinary_thirty_second_deadline() {
    for operation in ["metadata", "import", "discover"] {
        let (api, listener) = listener().await;
        let (ready, waiting) = tokio::sync::oneshot::channel();
        let (release, held) = tokio::sync::oneshot::channel();
        let operation = operation.to_owned();
        let route = operation.clone();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            assert!(
                request(&mut stream)
                    .await
                    .starts_with(&format!("POST /api/user-data/osu-folders/{route} HTTP/1.1"))
            );
            ready.send(()).unwrap();
            held.await.unwrap();
            let body = match route.as_str() {
                "metadata" => r#"{"beatmap_count":0}"#,
                "import" => {
                    r#"{"id":1,"kind":"stable","root_path":"/osu","marker_path":"/osu/osu!.db","label":null,"enabled":true,"last_scanned_at":null}"#
                }
                _ => "{\"event\":\"complete\"}\n",
            };
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
        let operation = tokio::spawn(async move {
            match operation.as_str() {
                "metadata" => api.osu_folder_metadata("/osu/osu!.db").await.map(|_| ()),
                "import" => api.import_osu_folder("/osu/osu!.db").await.map(|_| ()),
                _ => {
                    api.discover_osu_folders(&DiscoverFolders::default(), |_| {})
                        .await
                }
            }
        });
        waiting.await.unwrap();
        tokio::time::pause();
        tokio::time::advance(std::time::Duration::from_secs(31)).await;
        tokio::task::yield_now().await;
        assert!(!operation.is_finished());
        // Real socket delivery must not auto-advance the paused clock to the one-hour limit.
        tokio::time::resume();
        release.send(()).unwrap();
        operation.await.unwrap().unwrap();
        server.await.unwrap();
    }
}
