use super::*;
use crate::test_support::{beatmap_set, state_with};
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

const SETTINGS: &str = "/api/user-data/audio-mods";

async fn request(state: &AppState, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = crate::routes::router(state.clone())
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
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
#[allow(clippy::too_many_lines)] // Check atomic rejection and merged PATCH through both routers.
async fn validation_preserves_settings_and_profiles_without_partial_writes() {
    let state = state_with(&[beatmap_set(1, "/mods.mp3")]).await;
    let (_, defaults) = request(&state, "GET", SETTINGS, Value::Null).await;
    assert_eq!(
        defaults,
        json!({"individual_mods_enabled":false,"global_profile":{"mode":"none","speed_percent":100,"adjust_pitch":false}})
    );
    let id = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()[0]
        .audio_source_id;
    let path = format!("/api/audio-sources/{id}/mods");
    let profile = json!({"mode":"dt","speed_percent":120,"adjust_pitch":true});
    assert_eq!(
        request(&state, "PUT", &path, profile.clone()).await.0,
        StatusCode::NO_CONTENT
    );
    let (status, expected) = request(&state, "PATCH", SETTINGS,
        json!({"individual_mods_enabled":true,"global_mode":"ht","global_speed_percent":75,"global_adjust_pitch":true})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        request(&state, "PATCH", SETTINGS, json!({})).await,
        (StatusCode::OK, expected.clone())
    );
    for patch in [
        json!({"global_mode":"nc"}),
        json!({"global_mode":null}),
        json!({"global_mode":1}),
        json!({"individual_mods_enabled":null}),
        json!({"individual_mods_enabled":"false"}),
        json!({"global_adjust_pitch":null}),
        json!({"global_adjust_pitch":0}),
        json!({"global_speed_percent":null}),
        json!({"global_speed_percent":"75"}),
        json!({"global_speed_percent":75.5}),
        json!({"global_speed_percent":-1}),
        json!({"global_speed_percent":65536}),
        json!({"global_speed_percent":49}),
        json!({"global_mode":"dt","global_speed_percent":201}),
        json!({"global_mode":"dt","global_speed_percent":100}),
        json!({"global_speed_percent":100,"individual_mods_enabled":false}),
        json!({"global_mode":"none","global_speed_percent":100,"global_adjust_pitch":true}),
        json!({"global_mode":"dt"}),
        json!({"unknown":true}),
        json!({"global_profile":{"mode":"none","speed_percent":100,"adjust_pitch":false}}),
    ] {
        assert_eq!(
            request(&state, "PATCH", SETTINGS, patch.clone()).await.0,
            StatusCode::BAD_REQUEST,
            "{patch}"
        );
        assert_eq!(
            request(&state, "GET", SETTINGS, Value::Null).await,
            (StatusCode::OK, expected.clone())
        );
    }
    for invalid in [
        json!({}),
        json!({"mode":"dt","speed_percent":120}),
        json!({"mode":"nc","speed_percent":120,"adjust_pitch":true}),
        json!({"mode":null,"speed_percent":120,"adjust_pitch":true}),
        json!({"mode":"dt","speed_percent":null,"adjust_pitch":true}),
        json!({"mode":"dt","speed_percent":"120","adjust_pitch":true}),
        json!({"mode":"dt","speed_percent":120.5,"adjust_pitch":true}),
        json!({"mode":"dt","speed_percent":-1,"adjust_pitch":true}),
        json!({"mode":"dt","speed_percent":201,"adjust_pitch":true}),
        json!({"mode":"dt","speed_percent":100,"adjust_pitch":true}),
        json!({"mode":"ht","speed_percent":100,"adjust_pitch":false}),
        json!({"mode":"ht","speed_percent":49,"adjust_pitch":false}),
        json!({"mode":"none","speed_percent":100,"adjust_pitch":true}),
        json!({"mode":"none","speed_percent":99,"adjust_pitch":false}),
        json!({"mode":"dt","speed_percent":120,"adjust_pitch":null}),
        json!({"mode":"dt","speed_percent":120,"adjust_pitch":"true"}),
        json!({"mode":"dt","speed_percent":120,"adjust_pitch":true,"unknown":false}),
    ] {
        assert_eq!(
            request(&state, "PUT", &path, invalid.clone()).await.0,
            StatusCode::BAD_REQUEST,
            "{invalid}"
        );
        let (_, tracks) = request(&state, "GET", "/api/tracks", Value::Null).await;
        assert_eq!(tracks[0]["mods"], profile);
    }
    for (mode, speed, pitch) in [
        ("dt", 101, false),
        ("dt", 200, true),
        ("ht", 50, false),
        ("ht", 99, true),
        ("none", 100, false),
    ] {
        assert_eq!(
            request(
                &state,
                "PUT",
                &path,
                json!({"mode":mode,"speed_percent":speed,"adjust_pitch":pitch})
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(
        request(&state, "PUT", "/api/audio-sources/999999/mods", profile)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &state,
            "DELETE",
            "/api/audio-sources/999999/mods",
            Value::Null
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for _ in 0..2 {
        assert_eq!(
            request(&state, "DELETE", &path, Value::Null).await.0,
            StatusCode::NO_CONTENT
        );
    }
    let (status, updated) = request(
        &state,
        "PATCH",
        SETTINGS,
        json!({"global_speed_percent":80}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        updated["global_profile"],
        json!({"mode":"ht","speed_percent":80,"adjust_pitch":true})
    );
    assert_eq!(updated["individual_mods_enabled"], true);
}

#[tokio::test]
async fn stored_profiles_reach_rest_and_ndjson_without_changing_the_launch() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone.mp3");
    let state = state_with(&[beatmap_set(1, fixture.to_str().unwrap())]).await;
    let track = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()
        .remove(0);
    let id = track.audio_source_id;
    let playlist = state.services().playlists().create("Mods").await.unwrap();
    state
        .services()
        .playlists()
        .add_items(playlist.id, &[track.difficulties[0].beatmap_id])
        .await
        .unwrap();
    state.services().queue().append(vec![id]).await.unwrap();
    for check in state.services().queue().pending_checks().await.unwrap() {
        state
            .services()
            .queue()
            .apply_check(check, true)
            .await
            .unwrap();
    }
    let (_, launch) = request(
        &state,
        "POST",
        "/api/playback/commands",
        json!({"command":"play"}),
    )
    .await;
    let duration = launch["duration_ms"].as_u64().unwrap();
    let path = format!("/api/audio-sources/{id}/mods");
    let playlist_path = format!("/api/playlists/{}", playlist.id);
    let profile = json!({"mode":"dt","speed_percent":120,"adjust_pitch":true});
    for mods in [Value::Null, profile, Value::Null] {
        let method = if mods.is_null() { "DELETE" } else { "PUT" };
        assert_eq!(
            request(&state, method, &path, mods.clone()).await.0,
            StatusCode::NO_CONTENT
        );
        // A global setting never replaces the saved individual projection.
        assert_eq!(
            request(
                &state,
                "PATCH",
                SETTINGS,
                json!({"global_mode":"ht","global_speed_percent":75})
            )
            .await
            .0,
            StatusCode::OK
        );
        let (_, tracks) = request(&state, "GET", "/api/tracks", Value::Null).await;
        let (_, playlist) = request(&state, "GET", &playlist_path, Value::Null).await;
        let (_, queue) = request(&state, "GET", "/api/queue", Value::Null).await;
        let (_, assignment) = request(&state, "GET", "/api/playback", Value::Null).await;
        for projection in [
            &tracks[0],
            &playlist["items"][0],
            &queue["entries"][0],
            &assignment,
            &assignment["track"],
        ] {
            assert_eq!(projection.get("mods"), Some(&mods));
        }
        assert_eq!(assignment["duration_ms"], duration);
        for key in ["revision", "playback_token"] {
            assert_eq!(queue[key], launch[key]);
            assert_eq!(assignment[key], launch[key]);
        }
        let response = crate::routes::router(state.clone())
            .oneshot(
                Request::get("/api/playback/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let mut body = response.into_body();
        let frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let event: Value = serde_json::from_slice(frame.data_ref().unwrap()).unwrap();
        assert_eq!(event, assignment);
    }
}

#[cfg(feature = "docs")]
#[tokio::test]
async fn openapi_documents_required_nullable_profiles_and_strict_patch_fields() {
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
    let schemas = &document["components"]["schemas"];
    for name in [
        "TrackResponse",
        "PlaylistItemResponse",
        "QueueEntryResponse",
        "PlaybackResponse",
    ] {
        assert!(
            schemas[name]["required"]
                .as_array()
                .unwrap()
                .contains(&json!("mods")),
            "{name}"
        );
        let property = &schemas[name]["properties"]["mods"];
        assert!(
            property["oneOf"]
                .as_array()
                .unwrap()
                .contains(&json!({"type":"null"})),
            "{property}"
        );
        assert!(
            property["oneOf"]
                .as_array()
                .unwrap()
                .contains(&json!({"$ref":"#/components/schemas/AudioModsProfile"}))
        );
    }
    assert_eq!(
        schemas["AudioModsProfile"]["properties"]["speed_percent"]["minimum"],
        50
    );
    assert_eq!(
        schemas["AudioModsProfile"]["properties"]["speed_percent"]["maximum"],
        200
    );
    assert_eq!(schemas["AudioModsProfile"]["additionalProperties"], false);
    assert_eq!(
        schemas["AudioModsMode"]["enum"],
        json!(["none", "dt", "ht"])
    );
    assert_eq!(schemas["AudioModsPatch"]["additionalProperties"], false);
    for key in [
        "individual_mods_enabled",
        "global_mode",
        "global_speed_percent",
        "global_adjust_pitch",
    ] {
        assert!(schemas["AudioModsPatch"]["properties"][key].is_object());
        assert!(
            !schemas["AudioModsPatch"]["required"]
                .as_array()
                .is_some_and(|fields| fields.contains(&json!(key)))
        );
    }
    for (path, methods) in [
        (SETTINGS, ["get", "patch"]),
        ("/api/audio-sources/{id}/mods", ["put", "delete"]),
    ] {
        for method in methods {
            assert!(document["paths"][path][method].is_object());
        }
    }
}

#[tokio::test]
async fn api_client_reads_and_saves_profiles_through_the_real_router() {
    use osu_radio_client::models::{
        AudioModsMode, AudioModsPatch, AudioModsProfile, AudioModsSettings,
    };
    let state = state_with(&[beatmap_set(1, "/client-mods.mp3")]).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api =
        osu_radio_client::ApiClient::new(format!("http://{}", listener.local_addr().unwrap()))
            .unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, crate::routes::router(state))
            .await
            .unwrap();
    });
    assert_eq!(
        api.audio_mods().await.unwrap(),
        AudioModsSettings::default()
    );
    let profile = AudioModsProfile {
        mode: AudioModsMode::Dt,
        speed_percent: 120,
        adjust_pitch: true,
    };
    let saved = api
        .update_audio_mods(&AudioModsPatch {
            global_mode: Some(profile.mode),
            global_speed_percent: Some(120),
            global_adjust_pitch: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(saved.global_profile, profile);
    let saved = api
        .update_audio_mods(&AudioModsPatch {
            individual_mods_enabled: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(saved.global_profile, profile);
    assert!(saved.individual_mods_enabled);
    let tracks = api.search_tracks("").await.unwrap();
    let id = tracks[0].audio_source_id;
    assert_eq!(tracks[0].mods, None);
    api.set_audio_mods(id, Some(profile)).await.unwrap();
    let tracks = api.search_tracks("").await.unwrap();
    assert_eq!(
        osu_radio_client::Track::from(tracks[0].clone()).mods,
        Some(profile)
    );
    let playlist = api.create_playlist("Mods").await.unwrap();
    let playlist = api
        .add_playlist_items(playlist.id, &[tracks[0].difficulties[0].beatmap_id])
        .await
        .unwrap();
    assert_eq!(playlist.items[0].track().mods, Some(profile));
    api.append_queue(&[id]).await.unwrap();
    let entry = api.queue().await.unwrap().entries.remove(0);
    assert_eq!(
        osu_radio_client::view_models::QueueRow::from(entry)
            .track
            .mods,
        Some(profile)
    );
    for _ in 0..2 {
        api.set_audio_mods(id, None).await.unwrap();
    }
    assert_eq!(api.search_tracks("").await.unwrap()[0].mods, None);
    server.abort();
    let _ = server.await;
}
