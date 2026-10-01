use super::*;
#[cfg(feature = "docs")]
use crate::test_support::empty_state;
use crate::test_support::{beatmap_set, state_with};
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use radio_services::model::SourceType;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn request(state: &AppState, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = crate::routes::router(state.clone())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

async fn command_request(state: &AppState, command: Value) -> Value {
    let (status, response) = request(state, "POST", "/api/playback/commands", command).await;
    assert_eq!(status, StatusCode::OK);
    response
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One HTTP scenario retains state across all queue commands.
async fn http_queue_validates_atomically_and_translates_every_command() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone.mp3");
    let mut first = beatmap_set(1, fixture.to_str().unwrap());
    first.beatmaps[0].metadata.as_mut().unwrap().title = Some("Current outside search".into());
    let state = state_with(&[
        first,
        beatmap_set(2, "/missing-second.mp3"),
        beatmap_set(3, "/missing-third.mp3"),
    ])
    .await;
    let tracks = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap();
    let a = tracks[0].audio_source_id;
    let b = tracks[1].audio_source_id;
    let x = tracks[2].audio_source_id;

    let (status, empty) = request(&state, "GET", "/api/queue", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["audio_source_ids"], json!([]));
    assert_eq!(empty["current_index"], Value::Null);
    assert_eq!(empty["mode"], "stopped");
    let (status, first) = request(
        &state,
        "POST",
        "/api/queue/items",
        json!({"audio_source_ids":[a,b]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["current_audio_source_id"], a);
    assert_eq!(first["mode"], "playing");
    assert!(first["duration_ms"].as_u64().unwrap() > 0);
    assert_eq!(first["track"]["title"], "Current outside search");
    assert_eq!(first["track"]["difficulties"].as_array().unwrap().len(), 2);
    assert!(first.get("audio_source_ids").is_none());
    assert_eq!(first["can_next"], true);
    let token = first["playback_token"].as_u64().unwrap();
    let (_, before) = request(&state, "GET", "/api/queue", Value::Null).await;
    let online = state
        .services()
        .audio_sources()
        .get_or_insert(&SourceType::Online("https://example.invalid/song".into()))
        .await
        .unwrap();
    for invalid in [i32::MAX, online.id] {
        let (status, error) = request(
            &state,
            "POST",
            "/api/queue/items",
            json!({"audio_source_ids":[x,invalid]}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            error["error"]
                .as_str()
                .unwrap()
                .contains(&invalid.to_string())
        );
        let (_, after) = request(&state, "GET", "/api/queue", Value::Null).await;
        assert_eq!(after, before);
    }
    let paused = command_request(&state, json!({"command":"pause"})).await;
    assert_eq!(paused["mode"], "paused");
    assert_eq!(paused["playback_token"], token);
    let next = command_request(&state, json!({"command":"next"})).await;
    assert_eq!(next["mode"], "paused");
    assert_eq!(next["current_audio_source_id"], b);
    let previous = command_request(&state, json!({"command":"previous"})).await;
    assert_eq!(previous["mode"], "paused");
    assert_eq!(previous["current_audio_source_id"], a);
    let immediate = command_request(&state, json!({"command":"play","audio_source_id":x})).await;
    assert_eq!(immediate["current_audio_source_id"], x);
    assert_eq!(immediate["mode"], "playing");
    assert_eq!(immediate["duration_ms"], Value::Null);
    let (_, queue) = request(&state, "GET", "/api/queue", Value::Null).await;
    assert_eq!(queue["audio_source_ids"], json!([a, x, b]));
    assert_eq!(queue["current_index"], 1);
    let token = immediate["playback_token"].as_u64().unwrap();
    let finished =
        command_request(&state, json!({"command":"finished","playback_token":token})).await;
    assert_eq!(finished["current_audio_source_id"], b);
    let repeated =
        command_request(&state, json!({"command":"finished","playback_token":token})).await;
    assert_eq!(repeated, finished);
    let failed = command_request(
        &state,
        json!({"command":"failed","playback_token":finished["playback_token"]}),
    )
    .await;
    assert_eq!(failed["mode"], "stopped");
    assert_eq!(failed["current_audio_source_id"], Value::Null);
    let restarted = command_request(&state, json!({"command":"play"})).await;
    assert_eq!(restarted["current_audio_source_id"], a);
    let stopped = command_request(&state, json!({"command":"stop"})).await;
    assert_eq!(stopped["mode"], "stopped");
    let (status, cleared) = request(&state, "DELETE", "/api/queue", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cleared["mode"], "stopped");
    let (_, queue) = request(&state, "GET", "/api/queue", Value::Null).await;
    assert_eq!(queue["audio_source_ids"], json!([]));
    let (status, _) = request(
        &state,
        "POST",
        "/api/playback/commands",
        json!({"command":"finished"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = request(
        &state,
        "POST",
        "/api/playback/commands",
        json!({"command":"unexpected"}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = request(
        &state,
        "POST",
        "/api/playback/commands",
        json!({"command":"play","audio_source_id":i32::MAX}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn device_pause_token_protects_a_newer_launch_and_allows_duplicate_delivery() {
    let state = state_with(&[
        beatmap_set(1, "/device-first.mp3"),
        beatmap_set(2, "/device-second.mp3"),
    ])
    .await;
    let tracks = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap();
    let a = tracks[0].audio_source_id;
    let b = tracks[1].audio_source_id;
    let (_, first) = request(
        &state,
        "POST",
        "/api/queue/items",
        json!({"audio_source_ids":[a,b]}),
    )
    .await;
    let token = first["playback_token"].as_u64().unwrap();
    let pause = json!({"command":"pause","playback_token":token});
    let paused = command_request(&state, pause.clone()).await;
    assert_eq!(paused["mode"], "paused");
    assert_eq!(command_request(&state, pause.clone()).await, paused);
    command_request(&state, json!({"command":"next"})).await;
    let playing = command_request(&state, json!({"command":"play"})).await;
    assert_eq!(playing["current_audio_source_id"], b);
    assert_eq!(playing["mode"], "playing");
    assert_eq!(command_request(&state, pause).await, playing);
}

async fn assignment_frame(body: &mut Body) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(2), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let bytes = frame.into_data().unwrap();
    assert!(bytes.ends_with(b"\n"));
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn events_synchronize_committed_changes_ignore_stale_notifications_and_drop_subscribers() {
    let state = state_with(&[beatmap_set(1, "/missing.mp3")]).await;
    let id = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()[0]
        .audio_source_id;
    let response = events(State(state.clone())).await.unwrap();
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/x-ndjson"
    );
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(state.playback_subscriber_count(), 1);
    let Json(playing) = append_items(
        State(state.clone()),
        Json(AppendRequest {
            audio_source_ids: vec![id, id],
        }),
    )
    .await
    .unwrap();
    // Commit after the response snapshot was built but before its first frame is polled.
    let mut body = response.into_body();
    let initial = assignment_frame(&mut body).await;
    assert_eq!(initial["mode"], "stopped");
    // A late handler cannot replace a newer revision in the watch channel.
    state.publish_playback(initial["revision"].as_u64().unwrap());
    let update = assignment_frame(&mut body).await;
    assert_eq!(update["revision"], playing.revision);
    assert_eq!(update["playback_token"], playing.playback_token);
    let Json(next) = command(State(state.clone()), Json(CommandRequest::Next))
        .await
        .unwrap();
    let update = assignment_frame(&mut body).await;
    assert_eq!(update["current_audio_source_id"], id);
    assert_ne!(update["playback_token"], playing.playback_token);
    assert_eq!(update["playback_token"], next.playback_token);
    drop(body);
    assert_eq!(state.playback_subscriber_count(), 0);
    let mut reconnected = events(State(state.clone())).await.unwrap().into_body();
    let resynchronized = assignment_frame(&mut reconnected).await;
    assert_eq!(resynchronized, update);
    drop(reconnected);
    assert_eq!(state.playback_subscriber_count(), 0);
}

#[tokio::test]
async fn event_heartbeat_detects_a_committed_write_without_notification() {
    let state = state_with(&[beatmap_set(1, "/missing.mp3")]).await;
    let id = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()[0]
        .audio_source_id;
    let mut body = events(State(state.clone())).await.unwrap().into_body();
    assignment_frame(&mut body).await;
    // Simulates an independent Services writer or cancellation after commit.
    let committed = state.services().queue().append(vec![id]).await.unwrap();
    let frame = tokio::time::timeout(
        HEARTBEAT_INTERVAL.saturating_add(Duration::from_secs(2)),
        body.frame(),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    let update: Value = serde_json::from_slice(&frame.into_data().unwrap()).unwrap();
    assert_eq!(update["revision"], committed.revision);
    let frame = tokio::time::timeout(
        HEARTBEAT_INTERVAL.saturating_add(Duration::from_secs(2)),
        body.frame(),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    assert_eq!(frame.into_data().unwrap().as_ref(), b"\n");
}

#[cfg(feature = "docs")]
#[tokio::test]
async fn playback_routes_and_tagged_commands_are_documented() {
    let response = crate::routes::router(empty_state().await)
        .oneshot(Request::get("/docs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let html = std::str::from_utf8(&bytes).unwrap();
    let spec = html
        .split_once("type=\"application/json\">")
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0;
    let document: Value = serde_json::from_str(spec).unwrap();
    for (path, method) in [
        ("/api/queue", "get"),
        ("/api/queue", "delete"),
        ("/api/queue/items", "post"),
        ("/api/playback", "get"),
        ("/api/playback/events", "get"),
        ("/api/playback/commands", "post"),
    ] {
        assert!(
            document["paths"][path][method].is_object(),
            "{method} {path}"
        );
    }
    let schema = &document["components"]["schemas"]["CommandRequest"];
    assert!(schema.to_string().contains("playback_token"));
    assert!(schema.to_string().contains("previous"));
}
