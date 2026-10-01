use super::*;
use crate::test_support::{beatmap_set, state_with};
use axum::{
    body::Body,
    http::{Request, header},
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn request(
    state: &AppState,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    let body = body.map_or_else(Body::empty, |body| {
        Body::from(serde_json::to_vec(&body).unwrap())
    });
    let response = crate::routes::router(state.clone())
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn client_playlist_http_roundtrip_matches_the_server_contract() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone.mp3");
    let state = state_with(&[beatmap_set(1, fixture.to_str().unwrap())]).await;
    let maps = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()[0]
        .difficulties
        .clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api =
        osu_radio_client::ApiClient::new(format!("http://{}", listener.local_addr().unwrap()))
            .unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, crate::routes::router(state))
            .await
            .unwrap();
    });
    assert!(api.playlists().await.unwrap().is_empty());
    let created = api.create_playlist(" Test ").await.unwrap();
    assert_eq!(created.name, "Test");
    assert!(api.rename_playlist(created.id, " ").await.is_err());
    assert_eq!(
        api.rename_playlist(created.id, " Renamed ")
            .await
            .unwrap()
            .name,
        "Renamed"
    );
    let ids: Vec<_> = maps.iter().map(|map| map.beatmap_id).collect();
    let added = api.add_playlist_items(created.id, &ids).await.unwrap();
    assert_eq!(added.items.len(), 2);
    assert_eq!(
        api.add_playlist_items(created.id, &ids).await.unwrap(),
        added
    );
    let item = added.items[1].id;
    let assignment = api.play_playlist(created.id, Some(item)).await.unwrap();
    assert_eq!(assignment.current_playlist_item_id, Some(item));
    assert_eq!(
        api.queue().await.unwrap().playlist_item_ids,
        added
            .items
            .iter()
            .map(|item| Some(item.id))
            .collect::<Vec<_>>()
    );
    api.remove_playlist_item(created.id, item).await.unwrap();
    assert_eq!(api.playlist(created.id).await.unwrap().items.len(), 1);
    api.delete_playlist(created.id).await.unwrap();
    assert!(api.playlist(created.id).await.is_err());
    assert_eq!(api.playlists().await.unwrap().len(), 0);
    server.abort();
}

#[cfg(feature = "docs")]
#[tokio::test]
async fn every_playlist_route_and_item_assignment_is_documented() {
    let response = crate::routes::router(crate::test_support::empty_state().await)
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
        ("/api/playlists", "get"),
        ("/api/playlists", "post"),
        ("/api/playlists/{id}", "get"),
        ("/api/playlists/{id}", "patch"),
        ("/api/playlists/{id}", "delete"),
        ("/api/playlists/{id}/items", "post"),
        ("/api/playlists/{id}/items/{item_id}", "delete"),
        ("/api/playlists/{id}/play", "post"),
    ] {
        assert!(
            document["paths"][path][method].is_object(),
            "{method} {path}"
        );
    }
    assert!(document["components"]["schemas"]["PlaybackResponse"]["properties"]["current_playlist_item_id"].is_object());
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One HTTP scenario retains the playlist and queue through every operation.
async fn playlist_routes_cover_crud_validation_and_distinct_playback_items() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone.mp3");
    let state = state_with(&[beatmap_set(1, fixture.to_str().unwrap())]).await;
    let maps = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()[0]
        .difficulties
        .clone();
    let (status, _) = request(&state, "POST", "/api/playlists", Some(json!({"name":"  "}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, created) = request(
        &state,
        "POST",
        "/api/playlists",
        Some(json!({"name":"  Test  "})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["name"], "Test");
    let id = created["id"].as_i64().unwrap();
    let path = format!("/api/playlists/{id}");
    let (_, all) = request(&state, "GET", "/api/playlists", None).await;
    assert_eq!(all.as_array().unwrap().len(), 1);
    let (status, _) = request(&state, "PATCH", &path, Some(json!({"name":" Renamed "}))).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = request(&state, "POST", &format!("{path}/play"), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = request(
        &state,
        "POST",
        &format!("{path}/items"),
        Some(json!({"beatmap_ids":[maps[0].beatmap_id,i32::MAX]})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, playlist) = request(&state, "GET", &path, None).await;
    assert_eq!(playlist["items"], json!([]));
    let (status, playlist) = request(
        &state,
        "POST",
        &format!("{path}/items"),
        Some(json!({"beatmap_ids":maps.iter().map(|map|map.beatmap_id).collect::<Vec<_>>()})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let item = playlist["items"][1]["id"].clone();
    let (status, playing) = request(
        &state,
        "POST",
        &format!("{path}/play"),
        Some(json!({"start_item_id":item})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(playing["current_playlist_item_id"], item);
    assert!(playing["duration_ms"].as_u64().unwrap() > 0);
    let (status, _) = request(&state, "POST", &format!("{path}/play"), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, next) = request(
        &state,
        "POST",
        "/api/playback/commands",
        Some(json!({"command":"next"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(next["current_playlist_item_id"], item);
    let (status, _) = request(&state, "DELETE", &format!("{path}/items/{item}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, queue) = request(&state, "GET", "/api/queue", None).await;
    assert_eq!(queue["playlist_item_ids"].as_array().unwrap().len(), 2);
    let (status, _) = request(&state, "DELETE", &path, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    for (method, body) in [
        ("GET", None),
        ("PATCH", Some(json!({"name":"Lost"}))),
        ("DELETE", None),
    ] {
        assert_eq!(
            request(&state, method, &path, body).await.0,
            StatusCode::NOT_FOUND
        );
    }
}
