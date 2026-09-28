use super::*;
use crate::{routes::router, test_support::empty_state};
use axum::{
    body::to_bytes,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

#[tokio::test]
async fn discovery_stream_has_resolved_candidates_and_explicit_completion() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("osu!.db"), b"").unwrap();
    let state = empty_state().await;
    let body = serde_json::json!({"roots": [directory.path()], "depth": "full"}).to_string();
    let response = router(state.clone())
        .oneshot(
            Request::post("/api/user-data/osu-folders/discover")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/x-ndjson");
    let bytes = to_bytes(response.into_body(), 10000).await.unwrap();
    let events: Vec<serde_json::Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["event"], "candidate");
    assert_eq!(events[0]["kind"], "stable");
    assert_eq!(events[1]["event"], "complete");
    assert!(
        state
            .services()
            .osu_installations()
            .all()
            .await
            .unwrap()
            .is_empty()
    );
    for operation in ["metadata", "import"] {
        let response = router(state.clone())
            .oneshot(
                Request::post(format!("/api/user-data/osu-folders/{operation}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"marker_path":"relative/osu!.db"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn preview_and_import_http_roundtrip_is_read_only_then_atomic_and_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("osu!.db");
    let mut bytes = 20_250_108_i32.to_le_bytes().to_vec();
    bytes.extend(0_i32.to_le_bytes());
    bytes.push(1);
    bytes.extend([0; 8]);
    bytes.push(0);
    bytes.extend(0_i32.to_le_bytes());
    bytes.extend(1_i32.to_le_bytes());
    std::fs::write(&marker, bytes).unwrap();
    let state = empty_state().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api =
        osu_radio_client::ApiClient::new(format!("http://{}", listener.local_addr().unwrap()))
            .unwrap();
    let app = router(state.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let path = marker.to_str().unwrap();
    assert_eq!(
        api.osu_folder_metadata(path).await.unwrap().beatmap_count,
        0
    );
    assert!(api.osu_folders().await.unwrap().is_empty());
    let stored = api.import_osu_folder(path).await.unwrap();
    assert!(stored.last_scanned_at.is_some());
    assert_eq!(stored.kind, "stable");
    std::fs::write(&marker, b"broken source").unwrap();
    assert_eq!(api.import_osu_folder(path).await.unwrap(), stored);
    assert!(api.osu_folder_metadata(path).await.is_err());
    assert_eq!(api.osu_folders().await.unwrap(), [stored]);
    let mut events = Vec::new();
    api.discover_osu_folders(
        &osu_radio_client::DiscoverFolders {
            roots: vec![directory.path().to_path_buf()],
            depth: osu_radio_client::DiscoveryDepth::Known,
        },
        |event| events.push(event),
    )
    .await
    .unwrap();
    assert!(matches!(
        events.first(),
        Some(osu_radio_client::FolderDiscoveryEvent::Candidate {
            registered_id: Some(_),
            ..
        })
    ));
    assert_eq!(
        events.last(),
        Some(&osu_radio_client::FolderDiscoveryEvent::Complete)
    );
    server.abort();
    let _ = server.await;
}

#[cfg(feature = "docs")]
#[tokio::test]
async fn folder_selection_routes_are_in_the_served_openapi_document() {
    let response = router(empty_state().await)
        .oneshot(Request::get("/docs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let html = std::str::from_utf8(&body).unwrap();
    let spec = html
        .split_once("type=\"application/json\">")
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0;
    let document: serde_json::Value = serde_json::from_str(spec).unwrap();
    for operation in ["discover", "metadata", "import"] {
        assert!(
            document["paths"][format!("/api/user-data/osu-folders/{operation}")]["post"]
                .is_object()
        );
    }
}
