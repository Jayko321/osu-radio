use super::playlist_tests::read_request;
use super::*;
use crate::models::{AudioSettings, DiscoverFolders, PlaybackCommand};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

fn response(status: &str, body: &[u8]) -> Vec<u8> {
    let mut bytes = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

async fn respond(bytes: Vec<u8>) -> (ApiClient, tokio::task::JoinHandle<(String, Vec<u8>)>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = ApiClient::new(format!("http://{}///", listener.local_addr().unwrap())).unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let request = read_request(&mut stream).await;
        // Bound rejections can close the response before the writer finishes.
        let _ = stream.write_all(&bytes).await;
        request
    });
    (api, server)
}

async fn call(api: &ApiClient, operation: &str) -> Result<(), ApiError> {
    match operation {
        "sets" => api.beatmap_sets().await.map(drop),
        "search_sets" => api.search_beatmap_sets("query").await.map(drop),
        "tracks" => api.search_tracks("query").await.map(drop),
        "user" => api.user_data().await.map(drop),
        "folders" => api.osu_folders().await.map(drop),
        "register" => api
            .register_osu_folder(&RegisterOsuFolder::new("fixture"))
            .await
            .map(drop),
        "update_folder" => api
            .update_osu_folder(7, &OsuFolderChanges::enabled(false))
            .await
            .map(drop),
        "remove_folder" => api.remove_osu_folder(7).await,
        "metadata" => api.osu_folder_metadata("fixture/osu!.db").await.map(drop),
        "import" => api.import_osu_folder("fixture/osu!.db").await.map(drop),
        "reimport" => api.reimport_osu_folder(7).await.map(drop),
        "discover" => {
            api.discover_osu_folders(&DiscoverFolders::default(), |_| {
                panic!("error response emitted a candidate")
            })
            .await
        }
        "settings" => api.audio_settings().await.map(drop),
        "save_settings" => api
            .update_audio_settings(&AudioSettings::default())
            .await
            .map(drop),
        "volume" => api.set_audio_volume(7, Some(0)).await,
        "reset_volume" => api.set_audio_volume(7, None).await,
        "queue" => api.queue().await.map(drop),
        "append" => api.append_queue(&[7, 7]).await.map(drop),
        "clear" => api.clear_queue().await.map(drop),
        "playback" => api.playback().await.map(drop),
        "command" => api.playback_command(&PlaybackCommand::Stop).await.map(drop),
        "events" => {
            api.playback_events(|_| panic!("error response emitted an assignment"))
                .await
        }
        "playlists" => api.playlists().await.map(drop),
        "playlist" => api.playlist(7).await.map(drop),
        "create" => api.create_playlist("曲").await.map(drop),
        "rename" => api.rename_playlist(7, "曲").await.map(drop),
        "delete" => api.delete_playlist(7).await,
        "add_items" => api.add_playlist_items(7, &[8, 9]).await.map(drop),
        "remove_item" => api.remove_playlist_item(7, 8).await,
        "play_playlist" => api.play_playlist(7, Some(8)).await.map(drop),
        "playlist_cover" => api.playlist_cover(7).await.map(drop),
        "set_cover" => api.set_playlist_cover(7, vec![0]).await.map(drop),
        "reset_cover" => api.reset_playlist_cover(7).await,
        "cover" => api.cover(7).await.map(drop),
        "duration" => api.audio_duration(7).await.map(drop),
        "audio" => api.download_audio(7).await.map(drop),
        _ => panic!("unknown test operation: {operation}"),
    }
}

#[tokio::test]
async fn every_http_wrapper_retains_method_path_status_and_server_error() {
    for (operation, method, path) in [
        ("sets", "GET", "/api/beatmap-sets"),
        ("search_sets", "GET", "/api/beatmap-sets"),
        ("tracks", "GET", "/api/tracks"),
        ("user", "GET", "/api/user-data"),
        ("folders", "GET", "/api/user-data/osu-folders"),
        ("register", "POST", "/api/user-data/osu-folders"),
        ("update_folder", "PATCH", "/api/user-data/osu-folders/7"),
        ("remove_folder", "DELETE", "/api/user-data/osu-folders/7"),
        ("metadata", "POST", "/api/user-data/osu-folders/metadata"),
        ("import", "POST", "/api/user-data/osu-folders/import"),
        ("reimport", "POST", "/api/user-data/osu-folders/7/import"),
        ("discover", "POST", "/api/user-data/osu-folders/discover"),
        ("settings", "GET", "/api/user-data/audio-settings"),
        ("save_settings", "PATCH", "/api/user-data/audio-settings"),
        ("volume", "PUT", "/api/audio-sources/7/volume"),
        ("reset_volume", "DELETE", "/api/audio-sources/7/volume"),
        ("queue", "GET", "/api/queue"),
        ("append", "POST", "/api/queue/items"),
        ("clear", "DELETE", "/api/queue"),
        ("playback", "GET", "/api/playback"),
        ("command", "POST", "/api/playback/commands"),
        ("events", "GET", "/api/playback/events"),
        ("playlists", "GET", "/api/playlists"),
        ("playlist", "GET", "/api/playlists/7"),
        ("create", "POST", "/api/playlists"),
        ("rename", "PATCH", "/api/playlists/7"),
        ("delete", "DELETE", "/api/playlists/7"),
        ("add_items", "POST", "/api/playlists/7/items"),
        ("remove_item", "DELETE", "/api/playlists/7/items/8"),
        ("play_playlist", "POST", "/api/playlists/7/play"),
        ("playlist_cover", "GET", "/api/playlists/7/cover"),
        ("set_cover", "PUT", "/api/playlists/7/cover"),
        ("reset_cover", "DELETE", "/api/playlists/7/cover"),
        ("cover", "GET", "/api/beatmaps/7/cover"),
        ("duration", "GET", "/api/audio-sources/7/duration"),
        ("audio", "GET", "/api/audio-sources/7/audio"),
    ] {
        let (api, server) = respond(response(
            "422 Unprocessable Entity",
            br#"{"error":"fixture boundary"}"#,
        ))
        .await;
        let error = tokio::time::timeout(Duration::from_secs(3), call(&api, operation))
            .await
            .unwrap()
            .unwrap_err();
        assert!(error.source().is_none());
        assert!(
            matches!(error, ApiError::Status { path: ref failed_path, status: StatusCode::UNPROCESSABLE_ENTITY, ref message }
            if failed_path == path && message == "fixture boundary"),
            "{operation}: {error}"
        );
        let (headers, body) = server.await.unwrap();
        let mut request = headers.lines().next().unwrap().split_whitespace();
        assert_eq!(request.next(), Some(method), "{operation}");
        let target = request.next().unwrap();
        if matches!(operation, "search_sets" | "tracks") {
            assert_eq!(target, format!("{path}?q=query"));
        } else {
            assert_eq!(target, path, "{operation}");
        }
        let expected_body = match operation {
            "register" => Some(serde_json::json!({"path": "fixture", "label": null})),
            "update_folder" => Some(serde_json::json!({"enabled": false})),
            "metadata" | "import" => Some(serde_json::json!({"marker_path": "fixture/osu!.db"})),
            "discover" => Some(serde_json::json!({"roots": [], "depth": "full"})),
            "save_settings" => Some(
                serde_json::json!({"individual_volume_enabled": false, "global_volume_percent": 100}),
            ),
            "volume" => Some(serde_json::json!({"volume_percent": 0})),
            "append" => Some(serde_json::json!({"audio_source_ids": [7, 7]})),
            "command" => Some(serde_json::json!({"command": "stop"})),
            "create" | "rename" => Some(serde_json::json!({"name": "曲"})),
            "add_items" => Some(serde_json::json!({"beatmap_ids": [8, 9]})),
            "play_playlist" => Some(serde_json::json!({"start_item_id": 8})),
            _ => None,
        };
        if let Some(expected) = expected_body {
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                expected,
                "{operation}"
            );
        } else if operation == "set_cover" {
            assert_eq!(body, [0]);
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("content-type: image/png\r\n")
            );
        } else {
            assert!(body.is_empty(), "{operation}");
        }
    }
}

#[tokio::test]
async fn missing_mutation_and_legacy_search_success_contracts_preserve_payloads() {
    let body = br#"{"mode":"stopped","revision":17,"playback_token":23,"can_next":false,"can_previous":false}"#;
    let (api, server) = respond(response("200 OK", body)).await;
    assert!(!api.base_url().ends_with('/'));
    let assignment = api.clear_queue().await.unwrap();
    assert_eq!((assignment.revision, assignment.playback_token), (17, 23));
    assert_eq!(assignment.mode, crate::models::PlaybackMode::Stopped);
    assert_eq!(assignment.current_audio_source_id, None);
    let (headers, body) = server.await.unwrap();
    assert!(headers.starts_with("DELETE /api/queue HTTP/1.1\r\n"));
    assert!(body.is_empty());

    for percent in [Some(0), Some(100), None] {
        let (api, server) = respond(response("204 No Content", b"")).await;
        api.set_audio_volume(7, percent).await.unwrap();
        let (headers, body) = server.await.unwrap();
        if let Some(percent) = percent {
            assert!(headers.starts_with("PUT /api/audio-sources/7/volume HTTP/1.1\r\n"));
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                serde_json::json!({"volume_percent": percent})
            );
        } else {
            assert!(headers.starts_with("DELETE /api/audio-sources/7/volume HTTP/1.1\r\n"));
            assert!(body.is_empty());
        }
    }
    let (api, server) = respond(response("200 OK", br#"[{"id":5,"audio_sources":[]}]"#)).await;
    let sets = api.search_beatmap_sets("曲 a+b&?#").await.unwrap();
    assert_eq!(sets.first().unwrap().id, 5);
    assert!(
        server
            .await
            .unwrap()
            .0
            .starts_with("GET /api/beatmap-sets?q=%E6%9B%B2+a%2Bb%26%3F%23 HTTP/1.1\r\n")
    );
}

#[tokio::test]
async fn status_body_fallback_decode_and_transport_errors_remain_distinct() {
    for (body, expected) in [
        (br#"{"message":"legacy error"}"#.as_slice(), "legacy error"),
        (br#"{"error":""}"#.as_slice(), ""),
        (b"not JSON".as_slice(), "503 Service Unavailable"),
        (br#"{"error":42}"#.as_slice(), "503 Service Unavailable"),
        (b"{}".as_slice(), "503 Service Unavailable"),
    ] {
        let (api, server) = respond(response("503 Service Unavailable", body)).await;
        let error = api.user_data().await.unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("`/api/user-data` answered 503 Service Unavailable: {expected}")
        );
        assert!(
            matches!(error, ApiError::Status { status: StatusCode::SERVICE_UNAVAILABLE, ref message, .. } if message == expected)
        );
        server.await.unwrap();
    }
    for body in [
        b"invalid JSON".as_slice(),
        br#"{"id":"wrong type","osu_folders":[]}"#.as_slice(),
    ] {
        let (api, server) = respond(response("200 OK", body)).await;
        let error = api.user_data().await.unwrap_err();
        assert!(matches!(error, ApiError::Decode(_)));
        assert!(error.source().is_some());
        assert!(
            error
                .to_string()
                .starts_with("the server sent an unexpected response: ")
        );
        server.await.unwrap();
    }
    for (operation, status) in [
        ("register", "201 Created"),
        ("register", "409 Conflict"),
        ("metadata", "200 OK"),
        ("import", "200 OK"),
        ("update_folder", "200 OK"),
        ("create", "201 Created"),
    ] {
        let (api, server) = respond(response(status, b"invalid JSON")).await;
        assert!(
            matches!(call(&api, operation).await, Err(ApiError::Decode(_))),
            "{operation}: {status}"
        );
        server.await.unwrap();
    }
    let error = ApiClient::new("invalid URL")
        .unwrap()
        .user_data()
        .await
        .unwrap_err();
    assert!(matches!(error, ApiError::Transport(_)));
    assert!(error.source().is_some());
    assert!(
        error
            .to_string()
            .starts_with("the server could not be reached: ")
    );
}

#[tokio::test]
async fn temporary_audio_files_keep_io_errors_exact_limits_and_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let (api, server) = respond(response("200 OK", b"audio")).await;
    let error = api
        .download_audio_bounded(7, 100, Some(&directory.path().join("missing")))
        .await
        .unwrap_err();
    assert!(
        matches!(error, ApiError::File(ref cause) if cause.kind() == std::io::ErrorKind::NotFound)
    );
    assert!(error.source().is_some());
    assert!(
        error
            .to_string()
            .starts_with("could not save downloaded audio: ")
    );
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    server.await.unwrap();
    for bytes in [b"12345678".as_slice(), b"".as_slice()] {
        let (api, server) = respond(response("200 OK", bytes)).await;
        let file = api
            .download_audio_bounded(
                7,
                u64::try_from(bytes.len()).unwrap(),
                Some(directory.path()),
            )
            .await
            .unwrap();
        assert_eq!(std::fs::read(file.path()).unwrap(), bytes);
        drop(file);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        server.await.unwrap();
    }
    let error = ApiError::AudioTooLarge;
    assert_eq!(
        error.to_string(),
        "audio exceeds the 256 MiB download limit"
    );
    assert!(error.source().is_none());
}

#[tokio::test]
async fn ordinary_cover_bounds_accept_the_limit_and_discard_header_or_stream_excess() {
    let limit = 16 * 1024 * 1024;
    let exact = vec![0xab; limit];
    for bytes in [
        response("404 Not Found", b""),
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            limit + 1
        )
        .into_bytes(),
        {
            let mut bytes = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:x}\r\n", limit + 1).into_bytes();
            bytes.extend_from_slice(&exact);
            bytes.extend_from_slice(b"x\r\n0\r\n\r\n");
            bytes
        },
    ] {
        let (api, server) = respond(bytes).await;
        assert_eq!(api.cover(7).await.unwrap(), None);
        assert!(
            server
                .await
                .unwrap()
                .0
                .starts_with("GET /api/beatmaps/7/cover HTTP/1.1\r\n")
        );
    }
    let (api, server) = respond(response("200 OK", &exact)).await;
    assert_eq!(api.cover(7).await.unwrap().unwrap(), exact);
    server.await.unwrap();
}

#[tokio::test]
async fn discovery_and_playback_bound_individual_lines_including_heartbeats() {
    let limit = 1024 * 1024;
    for discovery in [false, true] {
        for oversized in [false, true] {
            let mut body = vec![b' '; limit - 1];
            if oversized {
                body.extend_from_slice(b"  ");
            }
            body.push(b'\n');
            body.extend_from_slice(if discovery {
                b"{\"event\":\"complete\"}\n"
            } else {
                b"{\"mode\":\"stopped\",\"revision\":1,\"playback_token\":0,\"can_next\":false,\"can_previous\":false}\n"
            });
            let (api, server) = respond(response("200 OK", &body)).await;
            let mut emitted = 0;
            let result = if discovery {
                api.discover_osu_folders(&DiscoverFolders::default(), |_| emitted += 1)
                    .await
            } else {
                api.playback_events(|_| emitted += 1).await
            };
            assert_eq!(emitted, usize::from(!oversized));
            if oversized {
                let expected = if discovery {
                    "Discovery event exceeds 1 MiB."
                } else {
                    "Playback event exceeds 1 MiB."
                };
                assert!(
                    matches!(result, Err(ApiError::Protocol(ref message)) if message == expected)
                );
            } else if discovery {
                result.unwrap();
            } else {
                assert!(
                    matches!(result, Err(ApiError::Protocol(ref message)) if message == "Playback stream ended; reconnecting.")
                );
            }
            server.await.unwrap();
        }
    }
}

#[tokio::test]
async fn partial_response_bodies_and_invalid_stream_json_keep_their_error_kinds() {
    for operation in ["events", "discover", "cover", "playlist_cover"] {
        let (api, server) = respond(
            b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\npartial".to_vec(),
        )
        .await;
        let error = call(&api, operation).await.unwrap_err();
        assert!(
            matches!(error, ApiError::Transport(_)),
            "{operation}: {error}"
        );
        assert!(error.source().is_some());
        server.await.unwrap();
    }
    let (api, server) = respond(response("200 OK", b"{bad JSON}\n")).await;
    let error = api
        .playback_events(|_| panic!("invalid JSON emitted an assignment"))
        .await
        .unwrap_err();
    assert!(matches!(error, ApiError::StreamDecode(_)));
    assert!(error.source().is_some());
    assert!(error.to_string().starts_with("Invalid stream event: "));
    server.await.unwrap();
    assert!(ApiError::Protocol("fixture".into()).source().is_none());
}
