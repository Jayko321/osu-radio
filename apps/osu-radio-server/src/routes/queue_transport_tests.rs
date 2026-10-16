use super::*;
use osu_radio_client::models::{QueueAvailability, QueueEntryId, QueueItemRef, QueuePlacement};

async fn audio_ids(state: &AppState) -> Vec<i32> {
    state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.audio_source_id)
        .collect()
}

async fn queue_json(state: &AppState) -> Value {
    let (status, body) = request(state, "GET", "/api/queue", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    body
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One client/server scenario follows entry identities through every mutation.
async fn api_client_roundtrip_covers_entries_placements_and_both_clears() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone.mp3");
    let mut first = beatmap_set(1, fixture.to_str().unwrap());
    first.beatmaps[0].metadata.as_mut().unwrap().title_unicode = Some("曲 🎵".into());
    first.beatmaps[0].metadata.as_mut().unwrap().artist_unicode = Some("演奏者".into());
    let state = state_with(&[first, beatmap_set(2, "/missing-transport.mp3")]).await;
    let ids = audio_ids(&state).await;
    let a = ids[0];
    let b = ids[1];
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api =
        osu_radio_client::ApiClient::new(format!("http://{}", listener.local_addr().unwrap()))
            .unwrap();
    let server_state = state.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, crate::routes::router(server_state))
            .await
            .unwrap();
    });
    api.append_queue(&[a, a, b]).await.unwrap();
    let initial = api.queue().await.unwrap();
    assert_eq!(initial.entries.len(), 3);
    let [one, two, three] = initial
        .entries
        .iter()
        .map(|e| e.id)
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    assert_ne!(one, two);
    assert_eq!(initial.entries[0].title_unicode.as_deref(), Some("曲 🎵"));
    assert_eq!(initial.entries[0].artist_unicode.as_deref(), Some("演奏者"));
    assert_eq!(initial.entries[0].difficulty_name.as_deref(), Some("Easy"));
    let waiting = api.play_queue_entry(one).await.unwrap();
    assert_eq!(waiting.current_entry_id, Some(one));
    assert_eq!(waiting.availability, Some(QueueAvailability::Checking));
    assert!(waiting.waiting_for_check && waiting.duration_ms.is_none());
    assert_eq!(
        state
            .services()
            .queue()
            .pending_checks()
            .await
            .unwrap()
            .len(),
        3,
        "GET and Play do not check files"
    );
    complete_checks(&state).await;
    state
        .services()
        .audio_settings()
        .set_volume(a, Some(17))
        .await
        .unwrap();
    let launch = api.playback().await.unwrap();
    assert!(!launch.waiting_for_check);
    assert!(launch.duration_ms.unwrap() > 0);
    api.playback_command(&osu_radio_client::models::PlaybackCommand::Started {
        playback_token: launch.playback_token,
    })
    .await
    .unwrap();
    let listening = api.queue().await.unwrap();
    assert!(listening.entries[0].last_played_at_ms.unwrap() > 0);
    assert_eq!(listening.entries[0].volume_percent, Some(17));
    assert_eq!(
        listening.entries[1].last_played_at_ms,
        listening.entries[0].last_played_at_ms
    );
    let before = state.services().queue().get().await.unwrap();
    assert_eq!(api.queue().await.unwrap(), listening);
    assert_eq!(state.services().queue().get().await.unwrap(), before);
    api.play_queue_entry(two).await.unwrap();
    let full = api.queue().await.unwrap();
    assert_eq!(full.history_len, 1);
    assert_eq!(
        full.entries.iter().map(|e| e.id).collect::<Vec<_>>(),
        [one, two, three]
    );
    let moved = api.move_queue_entry(two, None).await.unwrap();
    assert_eq!(moved.current_entry_id, Some(two));
    assert_eq!(moved.playback_token, full.playback_token);
    assert_eq!(api.queue().await.unwrap().history_len, 2);
    api.move_queue_entry(two, Some(three)).await.unwrap();
    api.insert_queue(
        &[QueueItemRef::Audio { audio_source_id: b }],
        QueuePlacement::Next,
    )
    .await
    .unwrap();
    let next = api.queue().await.unwrap();
    assert_eq!(next.entries[2].audio_source_id, Some(b));
    assert_eq!(next.entries[3].id, three);
    api.remove_queue_entry(one).await.unwrap();
    api.clear_upcoming_queue().await.unwrap();
    let remaining = api.queue().await.unwrap();
    assert_eq!(remaining.entries.len(), 1);
    assert_eq!(remaining.current_entry_id, Some(two));
    api.append_queue(&[b]).await.unwrap();
    api.playback_command(&osu_radio_client::models::PlaybackCommand::Pause)
        .await
        .unwrap();
    let advanced = api.remove_queue_entry(two).await.unwrap();
    assert_eq!(
        advanced.mode,
        osu_radio_client::models::PlaybackMode::Paused
    );
    assert!(advanced.waiting_for_check);
    assert_ne!(advanced.playback_token, remaining.playback_token);
    assert_eq!(advanced.current_audio_source_id, Some(b));
    api.clear_queue().await.unwrap();
    assert!(api.queue().await.unwrap().entries.is_empty());
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Rejections must preserve both public state and the private issued-ID counter.
async fn malformed_requests_and_all_missing_references_are_atomic_and_silent() {
    let state = state_with(&[beatmap_set(1, "/transport-atomic.mp3")]).await;
    let audio = audio_ids(&state).await[0];
    state.services().queue().append(vec![audio]).await.unwrap();
    let id = state.services().queue().get().await.unwrap().entries[0]
        .id
        .to_string();
    let playlist = state
        .services()
        .playlists()
        .create("references")
        .await
        .unwrap();
    let track = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()
        .remove(0);
    let playlist = state
        .services()
        .playlists()
        .add_items(playlist.id, &[track.difficulties[0].beatmap_id])
        .await
        .unwrap();
    let item = playlist.items[0].id;
    let other = state
        .services()
        .playlists()
        .create("foreign")
        .await
        .unwrap();
    let online = state
        .services()
        .audio_sources()
        .get_or_insert(&SourceType::Online("https://example.invalid/hidden".into()))
        .await
        .unwrap();
    let before = state.services().queue().get().await.unwrap();
    let wire = queue_json(&state).await;
    let mut notifications = state.subscribe_playback();
    notifications.borrow_and_update();
    let insert_path = "/api/queue/items";
    let move_path = format!("/api/queue/items/{id}");
    for (method, path, body, expected) in [
        (
            "POST",
            insert_path.to_owned(),
            json!({"items":[{"type":"audio","audio_source_id":audio},{"type":"audio","audio_source_id":i32::MAX}],"placement":"end"}),
            StatusCode::NOT_FOUND,
        ),
        (
            "POST",
            insert_path.to_owned(),
            json!({"items":[{"type":"audio","audio_source_id":audio},{"type":"audio","audio_source_id":online.id}],"placement":"next"}),
            StatusCode::NOT_FOUND,
        ),
        (
            "POST",
            insert_path.to_owned(),
            json!({"items":[{"type":"audio","audio_source_id":audio},{"type":"playlist_item","playlist_id":other.id,"item_id":item}],"placement":"end"}),
            StatusCode::NOT_FOUND,
        ),
        (
            "POST",
            insert_path.to_owned(),
            json!({"items":[{"type":"playlist_item","playlist_id":i32::MAX,"item_id":item}],"placement":"next"}),
            StatusCode::NOT_FOUND,
        ),
        (
            "POST",
            insert_path.to_owned(),
            json!({"items":[{"type":"playlist_item","playlist_id":playlist.id,"item_id":i32::MAX}],"placement":"end"}),
            StatusCode::NOT_FOUND,
        ),
        (
            "POST",
            "/api/playlists/2147483647/queue".into(),
            Value::Null,
            StatusCode::NOT_FOUND,
        ),
        (
            "POST",
            "/api/playlists/2147483647/play".into(),
            json!({}),
            StatusCode::NOT_FOUND,
        ),
        (
            "POST",
            format!("/api/playlists/{}/play", playlist.id),
            json!({"start_item_id":i32::MAX}),
            StatusCode::NOT_FOUND,
        ),
        (
            "POST",
            "/api/queue/items/9223372036854775807/play".into(),
            Value::Null,
            StatusCode::NOT_FOUND,
        ),
        (
            "DELETE",
            "/api/queue/items/9223372036854775807".into(),
            Value::Null,
            StatusCode::NOT_FOUND,
        ),
        (
            "PATCH",
            "/api/queue/items/9223372036854775807".into(),
            json!({"before_entry_id":null}),
            StatusCode::NOT_FOUND,
        ),
        (
            "PATCH",
            move_path.clone(),
            json!({"before_entry_id":"9223372036854775807"}),
            StatusCode::NOT_FOUND,
        ),
        (
            "PATCH",
            move_path.clone(),
            json!({}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "PATCH",
            move_path.clone(),
            json!({"before_entry_id":1}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "PATCH",
            move_path.clone(),
            json!({"before_entry_id":"9223372036854775808"}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "POST",
            insert_path.into(),
            json!({"items":[]}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "POST",
            insert_path.into(),
            json!({"placement":"end"}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "POST",
            insert_path.into(),
            json!({"items":[],"placement":"first"}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "POST",
            insert_path.into(),
            json!({"items":[{"type":"unknown","audio_source_id":audio}],"placement":"end"}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "POST",
            insert_path.into(),
            json!({"items":[{"type":"audio","audio_source_id":"1"}],"placement":"end"}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "POST",
            insert_path.into(),
            json!({"items":[{"type":"playlist_item","item_id":item}],"placement":"next"}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            "POST",
            insert_path.into(),
            Value::Null,
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        assert_eq!(
            request(&state, method, &path, body).await.0,
            expected,
            "{method} {path}"
        );
        assert_eq!(state.services().queue().get().await.unwrap(), before);
        assert_eq!(queue_json(&state).await, wire);
        assert!(
            !notifications.has_changed().unwrap(),
            "rollback cannot publish"
        );
    }
    for invalid in ["0", "-1", "9223372036854775808", "abc"] {
        assert_eq!(
            request(
                &state,
                "DELETE",
                &format!("/api/queue/items/{invalid}"),
                Value::Null
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    for body in ["", "{broken"] {
        let response = crate::routes::router(state.clone())
            .oneshot(
                Request::post(insert_path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let (status, _) = request(&state, "PATCH", &move_path, json!({"before_entry_id":null})).await;
    assert_eq!(status, StatusCode::OK, "explicit null anchor is accepted");
    let (_, added) = request(
        &state,
        "POST",
        insert_path,
        json!({"items":[{"type":"audio","audio_source_id":audio}],"placement":"end"}),
    )
    .await;
    let after = state.services().queue().get().await.unwrap();
    assert_eq!(after.entries[1].id.get(), before.last_entry_id + 1);
    assert_eq!(*notifications.borrow(), added["revision"].as_u64().unwrap());
}

#[tokio::test]
async fn playlist_append_and_r7_preserve_unavailable_saved_entries() {
    let mut second = beatmap_set(2, "/remaining.mp3");
    for map in &mut second.beatmaps {
        map.hash = Some(format!(
            "second-{}",
            map.difficulty_name.as_deref().unwrap()
        ));
    }
    let state = state_with(&[beatmap_set(1, "/unavailable-saved.mp3"), second.clone()]).await;
    let tracks = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap();
    let playlist = state
        .services()
        .playlists()
        .create("whole playlist")
        .await
        .unwrap();
    let maps = tracks
        .iter()
        .flat_map(|t| t.difficulties.iter().map(|d| d.beatmap_id))
        .collect::<Vec<_>>();
    let playlist = state
        .services()
        .playlists()
        .add_items(playlist.id, &maps)
        .await
        .unwrap();
    let installation = state.services().osu_installations().all().await.unwrap()[0].id;
    state
        .services()
        .osu_installations()
        .replace_snapshot(installation, &[second])
        .await
        .unwrap();
    let ids = audio_ids(&state).await;
    state
        .services()
        .queue()
        .append(vec![ids[0]; 3])
        .await
        .unwrap();
    complete_checks(&state).await;
    command_request(&state, json!({"command":"play"})).await;
    command_request(&state, json!({"command":"next"})).await;
    let before = state.services().queue().get().await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api =
        osu_radio_client::ApiClient::new(format!("http://{}", listener.local_addr().unwrap()))
            .unwrap();
    let cloned = state.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, crate::routes::router(cloned))
            .await
            .unwrap();
    });
    let appended = api.append_playlist_to_queue(playlist.id).await.unwrap();
    assert_eq!(appended.playback_token, before.playback_token);
    let queue = api.queue().await.unwrap();
    assert_eq!(queue.entries.len(), 7);
    let unavailable = &queue.entries[3];
    assert_eq!(unavailable.availability, QueueAvailability::Unavailable);
    assert!(
        unavailable.audio_source_id.is_none()
            && unavailable.beatmap_id.is_none()
            && unavailable.beatmap_set_id.is_none()
            && unavailable.cover_beatmap_id.is_none()
    );
    assert_eq!(unavailable.title.as_deref(), Some("Song"));
    assert_eq!(unavailable.playlist_item_id, Some(playlist.items[0].id));
    let osu_radio_client::ApiError::Status { status, .. } =
        api.play_queue_entry(unavailable.id).await.unwrap_err()
    else {
        panic!("HTTP error expected")
    };
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(api.queue().await.unwrap(), queue);
    let started = api.play_playlist(playlist.id, None).await.unwrap();
    let replaced = api.queue().await.unwrap();
    assert_eq!(replaced.entries.len(), 6);
    assert_eq!(
        replaced.entries[0].id.to_string(),
        before.entries[0].id.to_string()
    );
    assert_eq!(
        replaced.entries[1].id.to_string(),
        before.entries[1].id.to_string()
    );
    assert_eq!(
        replaced.history_len, 4,
        "R7 retains past/current, skips unavailable playlist prefix"
    );
    assert!(started.waiting_for_check);
    assert_eq!(started.current_playlist_item_id, Some(playlist.items[2].id));
    assert_ne!(started.playback_token, before.playback_token);
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // Follow one launch across worker completion, reimport and late callbacks.
async fn worker_and_reimport_stream_preserve_the_assigned_source_and_late_callbacks() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone.mp3");
    let state = state_with(&[beatmap_set(1, fixture.to_str().unwrap())]).await;
    let audio = audio_ids(&state).await[0];
    state
        .services()
        .audio_settings()
        .set_volume(audio, Some(29))
        .await
        .unwrap();
    state.services().queue().append(vec![audio]).await.unwrap();
    let id = state.services().queue().get().await.unwrap().entries[0]
        .id
        .to_string();
    let (_, waiting) = request(
        &state,
        "POST",
        &format!("/api/queue/items/{id}/play"),
        Value::Null,
    )
    .await;
    let mut stream = events(State(state.clone())).await.unwrap().into_body();
    assert_eq!(assignment_frame(&mut stream).await, waiting);
    assert_eq!(waiting["availability"], "checking");
    assert_eq!(waiting["waiting_for_check"], true);
    assert_eq!(waiting["duration_ms"], Value::Null);
    let worker = crate::queue_check::QueueCheckWorker::start(state.clone());
    let launched = assignment_frame(&mut stream).await;
    assert_eq!(launched["availability"], "available");
    assert_eq!(launched["waiting_for_check"], false);
    assert!(launched["duration_ms"].as_u64().unwrap() > 0);
    assert_eq!(launched["current_entry_id"], id);
    worker.shutdown().await.unwrap();
    let started = command_request(
        &state,
        json!({"command":"started", "playback_token": launched["playback_token"]}),
    )
    .await;
    assert_eq!(assignment_frame(&mut stream).await, started);
    let date = started["track"]["last_played_at_ms"].clone();
    let installation = state.services().osu_installations().all().await.unwrap()[0].id;
    // Same playlist key, new audio identity: the old assignment still owns its source/date/volume.
    let playlist = state.services().playlists().create("rebind").await.unwrap();
    let map = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()[0]
        .difficulties[0]
        .beatmap_id;
    let playlist = state
        .services()
        .playlists()
        .add_items(playlist.id, &[map])
        .await
        .unwrap();
    let (_, selected) = request(&state, "POST", "/api/queue/items", json!({"items":[{"type":"playlist_item","playlist_id":playlist.id,"item_id":playlist.items[0].id}],"placement":"end"})).await;
    assert_eq!(assignment_frame(&mut stream).await, selected);
    let selected_id = queue_json(&state).await["entries"][1]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    complete_checks(&state).await;
    let (_, selected) = request(
        &state,
        "POST",
        &format!("/api/queue/items/{selected_id}/play"),
        Value::Null,
    )
    .await;
    assert_eq!(assignment_frame(&mut stream).await, selected);
    state
        .services()
        .osu_installations()
        .replace_snapshot(installation, &[beatmap_set(1, "/rebound-source.mp3")])
        .await
        .unwrap();
    let assignment = state.services().queue().playback().await.unwrap();
    // Same post-commit transport boundary used by library workflows, without a real Realm reader.
    let Json(response) = committed_response(&state, assignment).await.unwrap();
    let rebound = serde_json::to_value(response).unwrap();
    assert_eq!(assignment_frame(&mut stream).await, rebound);
    assert_eq!(rebound["current_entry_id"], selected_id);
    assert_eq!(rebound["current_audio_source_id"], audio);
    assert_eq!(rebound["playback_token"], selected["playback_token"]);
    assert_eq!(rebound["track"]["last_played_at_ms"], date);
    assert_eq!(rebound["volume_percent"], 29);
    assert_eq!(rebound["availability"], "checking");
    assert_eq!(rebound["waiting_for_check"], false);
    assert_eq!(rebound["duration_ms"], Value::Null);
    let queue = queue_json(&state).await;
    assert_ne!(queue["entries"][1]["audio_source_id"], audio);
    assert_eq!(queue["entries"][1]["last_played_at_ms"], Value::Null);
    let late_started = command_request(
        &state,
        json!({"command":"started","playback_token":rebound["playback_token"]}),
    )
    .await;
    assert_eq!(late_started["current_audio_source_id"], audio);
    assert!(late_started["track"]["last_played_at_ms"].as_i64().unwrap() >= date.as_i64().unwrap());
    assert_eq!(
        queue_json(&state).await["entries"][1]["last_played_at_ms"],
        Value::Null
    );
    let failed = command_request(
        &state,
        json!({"command":"failed","playback_token":rebound["playback_token"]}),
    )
    .await;
    assert_eq!(failed["mode"], "stopped");
    assert_eq!(
        command_request(
            &state,
            json!({"command":"failed","playback_token":rebound["playback_token"]})
        )
        .await,
        failed
    );
    let worker = crate::queue_check::QueueCheckWorker::start(state.clone());
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let queue = queue_json(&state).await;
            if queue["entries"][1]["availability"] == "unavailable" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    worker.shutdown().await.unwrap();
    drop(stream);
    assert_eq!(state.playback_subscriber_count(), 0);
}

#[tokio::test]
async fn maximum_entry_id_roundtrips_in_queue_playback_and_ndjson_encoding() {
    let state = state_with(&[beatmap_set(1, "/max-wire.mp3")]).await;
    let audio = audio_ids(&state).await[0];
    state.services().queue().append(vec![audio]).await.unwrap();
    let mut snapshot = state.services().queue().snapshot().await.unwrap();
    let maximum = radio_services::QueueEntryId::new(i64::MAX).unwrap();
    snapshot.entries[0].entry.id = maximum;
    snapshot.current_entry_id = Some(maximum);
    let wire = serde_json::to_value(QueueResponse::from(snapshot)).unwrap();
    let client: osu_radio_client::models::QueueState =
        serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(
        client.current_entry_id,
        Some(QueueEntryId::new(i64::MAX).unwrap())
    );
    assert_eq!(wire["entries"][0]["id"], "9223372036854775807");
    let mut assignment = state.services().queue().playback().await.unwrap();
    assignment.current_entry_id = Some(maximum);
    let response = PlaybackResponse::from_assignment(&state, assignment)
        .await
        .unwrap();
    let client: osu_radio_client::models::PlaybackAssignment =
        serde_json::from_str(&encode_assignment(&response).unwrap()).unwrap();
    assert_eq!(client.current_entry_id.unwrap().get(), i64::MAX);
}

#[cfg(feature = "docs")]
#[tokio::test]
async fn openapi_queue_contract_matches_required_nullable_fields_and_statuses() {
    let state = state_with(&[beatmap_set(1, "/schema-entry")]).await;
    state
        .services()
        .queue()
        .append(vec![audio_ids(&state).await[0]])
        .await
        .unwrap();
    let response = crate::routes::router(state.clone())
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
        ("/api/queue/items", "post"),
        ("/api/queue/items/{entry_id}/play", "post"),
        ("/api/queue/items/{entry_id}", "patch"),
        ("/api/queue/items/{entry_id}", "delete"),
        ("/api/playlists/{id}/queue", "post"),
    ] {
        for status in ["200", "404", "500"] {
            assert!(
                document["paths"][path][method]["responses"][status].is_object(),
                "{method} {path}: {status}"
            );
        }
    }
    let schemas = &document["components"]["schemas"];
    assert_eq!(schemas["EntryId"]["type"], "string");
    let required = schemas["MoveRequest"]["required"].as_array().unwrap();
    assert!(required.contains(&json!("before_entry_id")));
    assert!(
        schemas["MoveRequest"]["properties"]["before_entry_id"]
            .to_string()
            .contains("null")
    );
    assert_eq!(
        schemas["AppendRequest"]["required"],
        json!(["items", "placement"])
    );
    assert_eq!(schemas["Placement"]["enum"], json!(["end", "next"]));
    for (name, wire) in [
        ("QueueResponse", queue_json(&state).await),
        (
            "QueueEntryResponse",
            queue_json(&state).await["entries"][0].clone(),
        ),
        (
            "PlaybackResponse",
            request(&state, "GET", "/api/playback", Value::Null).await.1,
        ),
    ] {
        let schema = &schemas[name];
        let required = schema["required"].as_array().unwrap();
        for (key, value) in wire.as_object().unwrap() {
            assert!(
                required.contains(&json!(key)),
                "{name}.{key} must be required"
            );
            if value.is_null() {
                assert!(
                    schema["properties"][key].to_string().contains("null"),
                    "{name}.{key} must be nullable"
                );
            }
        }
        assert_eq!(
            schema["properties"].as_object().unwrap().len(),
            wire.as_object().unwrap().len()
        );
    }
    for internal in [
        "audio_source",
        "playlist_source",
        "check_generation",
        "migration_audio_source_id",
        "migration_playlist_item_id",
    ] {
        assert!(
            schemas["QueueEntryResponse"]["properties"]
                .get(internal)
                .is_none()
        );
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One roundtrip retains typed item identity across disappearance and both playlist commands.
async fn typed_playlist_insertion_keeps_unavailable_items_and_empty_append_is_a_noop() {
    let state = state_with(&[beatmap_set(1, "/typed-playlist.mp3")]).await;
    let playlist = state.services().playlists().create("typed").await.unwrap();
    let track = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()
        .remove(0);
    let playlist = state
        .services()
        .playlists()
        .add_items(playlist.id, &[track.difficulties[0].beatmap_id])
        .await
        .unwrap();
    let empty = state.services().playlists().create("empty").await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api =
        osu_radio_client::ApiClient::new(format!("http://{}", listener.local_addr().unwrap()))
            .unwrap();
    let cloned = state.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, crate::routes::router(cloned))
            .await
            .unwrap();
    });
    let item = QueueItemRef::PlaylistItem {
        playlist_id: playlist.id,
        item_id: playlist.items[0].id,
    };
    api.insert_queue(&[item, item], QueuePlacement::Next)
        .await
        .unwrap();
    let original = api.queue().await.unwrap();
    assert_eq!(original.entries.len(), 2);
    assert!(
        original
            .entries
            .iter()
            .all(|e| e.playlist_item_id == Some(playlist.items[0].id))
    );
    api.append_playlist_to_queue(empty.id).await.unwrap();
    assert_eq!(api.queue().await.unwrap(), original);
    assert!(api.play_playlist(empty.id, None).await.is_err());
    assert_eq!(api.queue().await.unwrap(), original);
    let installation = state.services().osu_installations().all().await.unwrap()[0].id;
    request(
        &state,
        "DELETE",
        &format!("/api/user-data/osu-folders/{installation}"),
        Value::Null,
    )
    .await;
    let stopped = api.play_playlist(playlist.id, None).await.unwrap();
    assert_eq!(
        stopped.mode,
        osu_radio_client::models::PlaybackMode::Stopped
    );
    assert!(
        stopped.current_entry_id.is_none()
            && stopped.availability.is_none()
            && !stopped.waiting_for_check
    );
    let unavailable = api.queue().await.unwrap();
    assert_eq!(unavailable.history_len, 0);
    assert_eq!(unavailable.entries.len(), 1);
    assert_eq!(
        unavailable.entries[0].availability,
        QueueAvailability::Unavailable
    );
    api.insert_queue(&[item], QueuePlacement::End)
        .await
        .unwrap();
    let before = state.services().queue().get().await.unwrap();
    let (_, wire) = request(
        &state,
        "POST",
        &format!("/api/playlists/{}/play", playlist.id),
        json!({"start_item_id": playlist.items[0].id}),
    )
    .await;
    assert!(wire["error"].is_string());
    assert_eq!(state.services().queue().get().await.unwrap(), before);
    api.remove_queue_entry(unavailable.entries[0].id)
        .await
        .unwrap();
    assert_eq!(api.queue().await.unwrap().entries.len(), 1);
    for body in [json!({"start_item_id":"wrong type"}), Value::Null] {
        assert_eq!(
            request(
                &state,
                "POST",
                &format!("/api/playlists/{}/play", playlist.id),
                body
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
