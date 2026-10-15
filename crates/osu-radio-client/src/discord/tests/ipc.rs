//! The production worker and transport talking to an isolated, real Unix socket.
use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};

struct Fixture {
    _directory: tempfile::TempDir,
    listener: UnixListener,
    sender: watch::Sender<Desired>,
    task: JoinHandle<()>,
}
impl Fixture {
    fn start() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rpc");
        let listener = UnixListener::bind(&path).unwrap();
        let (track, playback) = sample(PlayerState::Playing);
        let (sender, receiver) = watch::channel(Desired {
            enabled: true,
            presence: Presence::from_playback(Some(&track), &playback),
            shutdown: false,
        });
        let runtime = tokio::runtime::Handle::current();
        let transport = Transport::with_path(runtime.clone(), receiver.clone(), path);
        let task = tokio::task::spawn_blocking(move || run(transport, receiver, &runtime));
        Self {
            _directory: directory,
            listener,
            sender,
            task,
        }
    }
    async fn accept(&self) -> UnixStream {
        tokio::time::timeout(Duration::from_secs(5), self.listener.accept())
            .await
            .unwrap()
            .unwrap()
            .0
    }
    async fn ready(&self) -> UnixStream {
        let mut peer = self.accept().await;
        let (opcode, payload) = read_frame(&mut peer).await;
        assert_eq!(opcode, 0);
        let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(json["client_id"], APPLICATION_ID);
        assert_eq!(json["v"], 1);
        write_json(&mut peer, &serde_json::json!({ "evt": "READY" })).await;
        peer
    }
    fn title(&self, title: &str) {
        self.sender
            .send_modify(|state| state.presence.as_mut().unwrap().key.title = title.into());
    }
    async fn shutdown(mut self) {
        self.sender.send_modify(|state| state.shutdown = true);
        tokio::time::timeout(Duration::from_millis(500), &mut self.task)
            .await
            .unwrap()
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.sender.send_modify(|state| state.shutdown = true);
    }
}
fn frame(opcode: u32, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.extend_from_slice(&opcode.to_le_bytes());
    frame.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
    frame.extend_from_slice(payload);
    frame
}
async fn read_frame(peer: &mut UnixStream) -> (u32, Vec<u8>) {
    tokio::time::timeout(Duration::from_secs(4), async {
        let mut header = [[0; 4]; 2];
        peer.read_exact(header.as_flattened_mut()).await.unwrap();
        let [opcode, length] = header;
        let opcode = u32::from_le_bytes(opcode);
        let length = u32::from_le_bytes(length);
        assert!(length <= 64 * 1024);
        let mut payload = vec![0; usize::try_from(length).unwrap()];
        peer.read_exact(&mut payload).await.unwrap();
        (opcode, payload)
    })
    .await
    .unwrap()
}
async fn activity(peer: &mut UnixStream) -> serde_json::Value {
    let (opcode, payload) = read_frame(peer).await;
    assert_eq!(opcode, 1);
    let json: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(json["cmd"], "SET_ACTIVITY");
    assert_eq!(json["args"]["pid"], std::process::id());
    assert!(!json["nonce"].as_str().unwrap().is_empty());
    json
}
async fn write_json(peer: &mut UnixStream, json: &serde_json::Value) {
    peer.write_all(&frame(1, &serde_json::to_vec(json).unwrap()))
        .await
        .unwrap();
}
async fn ack(peer: &mut UnixStream, request: &serde_json::Value) {
    write_json(peer, &serde_json::json!({ "cmd": "SET_ACTIVITY", "evt": null, "nonce": request["nonce"], "data": {} })).await;
}
async fn reject(peer: &mut UnixStream, request: &serde_json::Value) {
    write_json(peer, &serde_json::json!({ "cmd": "SET_ACTIVITY", "evt": "ERROR", "nonce": request["nonce"], "data": { "code": 4000, "message": "Invalid activity" } })).await;
}
async fn quiet(peer: &mut UnixStream, duration: Duration) {
    let mut byte = [0];
    assert!(
        tokio::time::timeout(duration, peer.read(&mut byte))
            .await
            .is_err(),
        "unexpected frame or disconnect"
    );
}
async fn closed(peer: &mut UnixStream) {
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), peer.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
}
async fn ping_barrier(peer: &mut UnixStream) {
    peer.write_all(&frame(3, b"barrier")).await.unwrap();
    assert_eq!(read_frame(peer).await, (4, b"barrier".to_vec()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ready_and_ack_silence_beyond_io_timeout_keeps_worker_connected() {
    let fixture = Fixture::start();
    let mut peer = fixture.ready().await;
    let initial = activity(&mut peer).await;
    quiet(
        &mut peer,
        IO_TIMEOUT.saturating_add(Duration::from_millis(150)),
    )
    .await;
    fixture.title("after READY silence");
    let request = activity(&mut peer).await;
    assert_ne!(request["nonce"], initial["nonce"]);
    ack(&mut peer, &request).await;
    quiet(
        &mut peer,
        IO_TIMEOUT.saturating_add(Duration::from_millis(150)),
    )
    .await;
    fixture.title("after ACK silence");
    assert_eq!(
        activity(&mut peer).await["args"]["activity"]["details"],
        "after ACK silence"
    );
    fixture.sender.send_modify(|state| state.enabled = false);
    assert!(activity(&mut peer).await["args"]["activity"].is_null());
    closed(&mut peer).await;
    fixture.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fragmented_handshake_and_response_and_byte_exact_ping_pong() {
    let fixture = Fixture::start();
    let mut peer = fixture.accept().await;
    read_frame(&mut peer).await;
    for byte in frame(1, br#"{"evt":"READY"}"#) {
        peer.write_all(&[byte]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    let request = activity(&mut peer).await;
    let reply =
        serde_json::json!({ "cmd": "SET_ACTIVITY", "nonce": request["nonce"], "evt": null });
    for byte in frame(1, &serde_json::to_vec(&reply).unwrap()) {
        peer.write_all(&[byte]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    for payload in [
        b"".as_slice(),
        b"\x00\xff\x80",
        br#"{ "json" : true }"#,
        &vec![0xab; 64 * 1024],
    ] {
        peer.write_all(&frame(3, payload)).await.unwrap();
        assert_eq!(read_frame(&mut peer).await, (4, payload.to_vec()));
        peer.write_all(&frame(4, payload)).await.unwrap();
    }
    ping_barrier(&mut peer).await;
    fixture.title("still connected");
    assert_eq!(
        activity(&mut peer).await["args"]["activity"]["details"],
        "still connected"
    );
    fixture.shutdown().await;
    assert!(activity(&mut peer).await["args"]["activity"].is_null());
    closed(&mut peer).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn oversized_and_malformed_frames_close_without_allocating_or_reusing_stream() {
    let mut too_large = 1_u32.to_le_bytes().to_vec();
    too_large.extend_from_slice(&(64_u32 * 1024 + 1).to_le_bytes());
    let mut huge = 1_u32.to_le_bytes().to_vec();
    huge.extend_from_slice(&u32::MAX.to_le_bytes());
    for bytes in [
        too_large,
        huge,
        frame(1, b"not JSON"),
        frame(1, b"\xff"),
        frame(99, b"{}"),
        frame(2, b"{}"),
    ] {
        let fixture = Fixture::start();
        let mut peer = fixture.ready().await;
        activity(&mut peer).await;
        peer.write_all(&bytes).await.unwrap();
        closed(&mut peer).await;
        fixture.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handshake_also_rejects_oversize_bad_json_wrong_ready_and_times_out() {
    let mut oversized = 1_u32.to_le_bytes().to_vec();
    oversized.extend_from_slice(&u32::MAX.to_le_bytes());
    for bytes in [
        oversized,
        frame(1, b"bad JSON"),
        frame(1, br#"{"evt":"ERROR"}"#),
        vec![],
    ] {
        let fixture = Fixture::start();
        let mut peer = fixture.accept().await;
        read_frame(&mut peer).await;
        peer.write_all(&bytes).await.unwrap();
        closed(&mut peer).await;
        fixture.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incomplete_header_body_and_eof_drop_the_old_connection_and_replay_latest() {
    for bytes in [vec![1], frame(1, b"{}").into_iter().take(9).collect()] {
        let fixture = Fixture::start();
        let mut peer = fixture.ready().await;
        activity(&mut peer).await;
        peer.write_all(&bytes).await.unwrap();
        closed(&mut peer).await;
        fixture.title("latest after timeout");
        let mut next = fixture.ready().await;
        assert_eq!(
            activity(&mut next).await["args"]["activity"]["details"],
            "latest after timeout"
        );
        drop(next);
        fixture.title("latest after EOF");
        let mut next = fixture.ready().await;
        assert_eq!(
            activity(&mut next).await["args"]["activity"]["details"],
            "latest after EOF"
        );
        fixture.shutdown().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disable_and_shutdown_cancel_idle_handshake_and_partial_frame_reads() {
    for shutdown in [false, true] {
        // Idle waiting for the first byte, stalled handshake, partial header and body.
        for bytes in [
            None,
            Some(vec![]),
            Some(vec![1]),
            Some(frame(1, b"{}").into_iter().take(9).collect()),
        ] {
            let fixture = Fixture::start();
            let handshake = bytes.as_ref().is_some_and(Vec::is_empty);
            let mut peer = if handshake {
                fixture.accept().await
            } else {
                fixture.ready().await
            };
            if handshake {
                read_frame(&mut peer).await;
            } else {
                activity(&mut peer).await;
            }
            if let Some(bytes) = &bytes {
                peer.write_all(bytes).await.unwrap();
            }
            if bytes.as_ref().is_some_and(|bytes| !bytes.is_empty()) {
                // Let the worker start completing the partial frame before cancellation.
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
            fixture.sender.send_modify(|state| {
                state.shutdown = shutdown;
                state.enabled = false;
            });
            if bytes.is_none() {
                assert!(activity(&mut peer).await["args"]["activity"].is_null());
            }
            tokio::time::timeout(Duration::from_millis(500), closed(&mut peer))
                .await
                .unwrap();
            fixture.shutdown().await;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn artwork_is_disabled_only_after_matching_fallback_ack_and_clear_error_stays_connected() {
    let fixture = Fixture::start();
    let mut peer = fixture.ready().await;
    let original = activity(&mut peer).await;
    assert!(original["args"]["activity"].get("assets").is_some());
    reject(&mut peer, &original).await;
    let fallback = activity(&mut peer).await;
    assert_ne!(original["nonce"], fallback["nonce"]);
    assert!(fallback["args"]["activity"].get("assets").is_none());
    ack(&mut peer, &fallback).await;
    ping_barrier(&mut peer).await;
    fixture.title("new track without artwork");
    let next = activity(&mut peer).await;
    assert!(next["args"]["activity"].get("assets").is_none());
    reject(&mut peer, &original).await;
    ack(&mut peer, &original).await;
    ping_barrier(&mut peer).await;
    quiet(&mut peer, Duration::from_millis(30)).await;
    fixture.sender.send_modify(|state| state.presence = None);
    let clear = activity(&mut peer).await;
    assert!(clear["args"]["activity"].is_null());
    reject(&mut peer, &clear).await;
    ping_barrier(&mut peer).await;
    quiet(&mut peer, Duration::from_millis(30)).await;
    let (track, playback) = sample(PlayerState::Playing);
    fixture
        .sender
        .send_modify(|state| state.presence = Presence::from_playback(Some(&track), &playback));
    assert!(
        activity(&mut peer).await["args"]["activity"]
            .get("assets")
            .is_none()
    );
    fixture.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_fallback_blocks_same_key_but_keeps_artwork_for_new_key_and_new_session() {
    let fixture = Fixture::start();
    let mut peer = fixture.ready().await;
    let original = activity(&mut peer).await;
    reject(&mut peer, &original).await;
    let fallback = activity(&mut peer).await;
    reject(&mut peer, &fallback).await;
    ping_barrier(&mut peer).await;
    fixture.sender.send_modify(|_| {});
    quiet(&mut peer, Duration::from_millis(50)).await;
    fixture.title("changed key");
    let next = activity(&mut peer).await;
    assert!(next["args"]["activity"].get("assets").is_some());
    reject(&mut peer, &next).await;
    let fallback = activity(&mut peer).await;
    reject(&mut peer, &fallback).await;
    ping_barrier(&mut peer).await;
    drop(peer);
    let mut peer = fixture.ready().await;
    let replay = activity(&mut peer).await;
    assert_eq!(replay["args"]["activity"]["details"], "changed key");
    assert!(replay["args"]["activity"].get("assets").is_some());
    fixture.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_errors_and_fallback_acks_do_not_modify_new_activity() {
    let fixture = Fixture::start();
    let mut peer = fixture.ready().await;
    let original = activity(&mut peer).await;
    fixture.title("second");
    let second = activity(&mut peer).await;
    reject(&mut peer, &original).await;
    ack(&mut peer, &original).await;
    ping_barrier(&mut peer).await;
    quiet(&mut peer, Duration::from_millis(30)).await;
    reject(&mut peer, &second).await;
    let fallback = activity(&mut peer).await;
    fixture.title("third");
    let third = activity(&mut peer).await;
    assert!(third["args"]["activity"].get("assets").is_some());
    ack(&mut peer, &fallback).await;
    reject(&mut peer, &fallback).await;
    ping_barrier(&mut peer).await;
    quiet(&mut peer, Duration::from_millis(30)).await;
    fixture.title("fourth");
    assert!(
        activity(&mut peer).await["args"]["activity"]
            .get("assets")
            .is_some()
    );
    fixture.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ready_close_grows_backoff_ack_resets_it_and_disable_shutdown_interrupt_it() {
    let fixture = Fixture::start();
    let mut peer = fixture.ready().await;
    activity(&mut peer).await;
    for delay in [1, 2, 4] {
        peer.write_all(&frame(2, b"{}")).await.unwrap();
        closed(&mut peer).await;
        let start = Instant::now();
        peer = fixture.ready().await;
        assert!(start.elapsed() >= Duration::from_millis(delay * 900));
        let request = activity(&mut peer).await;
        if delay == 4 {
            ack(&mut peer, &request).await;
            ping_barrier(&mut peer).await;
        }
    }
    peer.write_all(&frame(2, b"{}")).await.unwrap();
    closed(&mut peer).await;
    let start = Instant::now();
    peer = fixture.ready().await;
    assert!(start.elapsed() < Duration::from_secs(2));
    activity(&mut peer).await;
    peer.write_all(&frame(2, b"{}")).await.unwrap();
    closed(&mut peer).await;
    fixture.sender.send_modify(|state| state.enabled = false);
    assert!(
        tokio::time::timeout(Duration::from_millis(100), fixture.listener.accept())
            .await
            .is_err()
    );
    fixture.sender.send_modify(|state| state.enabled = true);
    let start = Instant::now();
    peer = fixture.ready().await;
    assert!(start.elapsed() < Duration::from_millis(500));
    activity(&mut peer).await;
    peer.write_all(&frame(2, b"{}")).await.unwrap();
    closed(&mut peer).await;
    fixture.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clear_and_unrelated_errors_do_not_retry_or_disable_artwork() {
    let fixture = Fixture::start();
    let mut peer = fixture.ready().await;
    activity(&mut peer).await;
    fixture.sender.send_modify(|state| state.presence = None);
    let clear = activity(&mut peer).await;
    reject(&mut peer, &clear).await;
    ping_barrier(&mut peer).await;
    quiet(&mut peer, Duration::from_millis(30)).await;
    let (track, playback) = sample(PlayerState::Playing);
    fixture
        .sender
        .send_modify(|state| state.presence = Presence::from_playback(Some(&track), &playback));
    let next = activity(&mut peer).await;
    assert!(next["args"]["activity"].get("assets").is_some());
    for (cmd, nonce) in [
        ("OTHER", next["nonce"].clone()),
        ("SET_ACTIVITY", serde_json::json!("unknown nonce")),
    ] {
        write_json(&mut peer, &serde_json::json!({ "cmd": cmd, "evt": "ERROR", "nonce": nonce, "data": { "code": 4000, "message": "unrelated" } })).await;
    }
    for reply in [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!({ "cmd": "SET_ACTIVITY", "evt": null }),
        serde_json::json!({ "cmd": "SET_ACTIVITY", "nonce": 42 }),
        serde_json::json!({ "cmd": "SET_ACTIVITY", "nonce": next["nonce"], "evt": "UNRELATED" }),
        serde_json::json!({ "cmd": "OTHER", "evt": "ERROR", "data": [] }),
    ] {
        write_json(&mut peer, &reply).await;
    }
    ping_barrier(&mut peer).await;
    quiet(&mut peer, Duration::from_millis(30)).await;
    fixture.title("artwork still enabled");
    assert!(
        activity(&mut peer).await["args"]["activity"]
            .get("assets")
            .is_some()
    );
    fixture.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn song_change_during_fragmented_error_never_retries_old_activity() {
    let fixture = Fixture::start();
    let mut peer = fixture.ready().await;
    let original = activity(&mut peer).await;
    let error = serde_json::json!({ "cmd": "SET_ACTIVITY", "evt": "ERROR", "nonce": original["nonce"], "data": { "code": 4000, "message": "Invalid activity" } });
    let bytes = frame(1, &serde_json::to_vec(&error).unwrap());
    let (head, tail) = bytes.split_at(1);
    peer.write_all(head).await.unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    fixture.title("changed during read");
    peer.write_all(tail).await.unwrap();
    let next = activity(&mut peer).await;
    assert_eq!(next["args"]["activity"]["details"], "changed during read");
    assert!(next["args"]["activity"].get("assets").is_some());
    ping_barrier(&mut peer).await;
    quiet(&mut peer, Duration::from_millis(30)).await;
    fixture.shutdown().await;
}
