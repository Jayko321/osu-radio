use crate::{error::ApiError, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use radio_services::{AudioSettings, AudioSettingsError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct AudioSettingsResponse {
    individual_volume_enabled: bool,
    global_volume_percent: u8,
}
impl From<AudioSettings> for AudioSettingsResponse {
    fn from(settings: AudioSettings) -> Self {
        Self {
            individual_volume_enabled: settings.individual_volume_enabled,
            global_volume_percent: settings.global_volume_percent,
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct SettingsPatch {
    #[serde(default, deserialize_with = "present")]
    individual_volume_enabled: Option<bool>,
    #[serde(default, deserialize_with = "present")]
    #[cfg_attr(feature = "docs", schema(minimum = 0, maximum = 100))]
    global_volume_percent: Option<i32>,
}
fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct VolumeRequest {
    #[cfg_attr(feature = "docs", schema(minimum = 0, maximum = 100))]
    volume_percent: i32,
}
fn percent(value: i32) -> Result<u8, ApiError> {
    u8::try_from(value)
        .ok()
        .filter(|v| *v <= 100)
        .ok_or_else(|| ApiError::bad_request(AudioSettingsError::InvalidPercent.to_string()))
}
fn settings_error(error: anyhow::Error) -> ApiError {
    match error.downcast_ref::<AudioSettingsError>() {
        Some(reason @ AudioSettingsError::UnknownAudio(_)) => {
            ApiError::not_found(reason.to_string())
        }
        Some(reason) => ApiError::bad_request(reason.to_string()),
        None => error.into(),
    }
}
#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/user-data/audio-settings", tag = "user-data",
    responses((status = OK, body = AudioSettingsResponse), (status = INTERNAL_SERVER_ERROR))))]
pub(crate) async fn get(
    State(state): State<AppState>,
) -> Result<Json<AudioSettingsResponse>, ApiError> {
    Ok(Json(state.services().audio_settings().get().await?.into()))
}
#[cfg_attr(feature = "docs", utoipa::path(patch, path = "/api/user-data/audio-settings", tag = "user-data", request_body = SettingsPatch,
    responses((status = OK, body = AudioSettingsResponse), (status = BAD_REQUEST), (status = INTERNAL_SERVER_ERROR))))]
pub(crate) async fn patch(
    State(state): State<AppState>,
    Json(body): Json<SettingsPatch>,
) -> Result<Json<AudioSettingsResponse>, ApiError> {
    let settings = state
        .services()
        .audio_settings()
        .update(
            body.individual_volume_enabled,
            body.global_volume_percent.map(percent).transpose()?,
        )
        .await
        .map_err(settings_error)?;
    Ok(Json(settings.into()))
}
#[cfg_attr(feature = "docs", utoipa::path(put, path = "/api/audio-sources/{id}/volume", tag = "user-data", params(("id" = i32, Path)), request_body = VolumeRequest,
    responses((status = NO_CONTENT), (status = BAD_REQUEST), (status = NOT_FOUND), (status = INTERNAL_SERVER_ERROR))))]
pub(crate) async fn put_volume(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<VolumeRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .services()
        .audio_settings()
        .set_volume(id, Some(percent(body.volume_percent)?))
        .await
        .map_err(settings_error)?;
    Ok(StatusCode::NO_CONTENT)
}
#[cfg_attr(feature = "docs", utoipa::path(delete, path = "/api/audio-sources/{id}/volume", tag = "user-data", params(("id" = i32, Path)),
    responses((status = NO_CONTENT), (status = NOT_FOUND), (status = INTERNAL_SERVER_ERROR))))]
pub(crate) async fn delete_volume(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    state
        .services()
        .audio_settings()
        .set_volume(id, None)
        .await
        .map_err(settings_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::test_support::{beatmap_set, state_with};
    use axum::{body::Body, http::Request};
    use serde_json::{Value, json};
    use tower::ServiceExt;

    async fn request(
        state: &AppState,
        method: &str,
        path: &str,
        body: Value,
    ) -> (StatusCode, Value) {
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
    #[cfg(feature = "docs")]
    #[tokio::test]
    async fn served_document_contains_volume_routes_bounds_and_optional_fields() {
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
        for (path, methods) in [
            ("/api/user-data/audio-settings", ["get", "patch"]),
            ("/api/audio-sources/{id}/volume", ["put", "delete"]),
        ] {
            for method in methods {
                assert!(document["paths"][path][method].is_object());
            }
        }
        let schemas = &document["components"]["schemas"];
        assert_eq!(
            schemas["VolumeRequest"]["properties"]["volume_percent"]["maximum"],
            100
        );
        for schema in ["TrackResponse", "PlaylistItemResponse", "PlaybackResponse"] {
            assert!(schemas[schema]["properties"]["volume_percent"].is_object());
        }
    }
    #[tokio::test]
    #[allow(clippy::too_many_lines)] // Both router variants exercise validation and projections through public URLs.
    async fn settings_routes_preserve_absolute_values_and_validate_percentages() {
        let state = state_with(&[beatmap_set(1, "/volume.mp3")]).await;
        let settings_path = "/api/user-data/audio-settings";
        let (status, settings) = request(&state, "GET", settings_path, Value::Null).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            settings,
            json!({"individual_volume_enabled":false, "global_volume_percent":100})
        );
        let (_, settings) = request(
            &state,
            "PATCH",
            settings_path,
            json!({"individual_volume_enabled":true,"global_volume_percent":20}),
        )
        .await;
        assert_eq!(settings["global_volume_percent"], 20);
        let tracks = state
            .services()
            .beatmap_sets()
            .search_tracks("")
            .await
            .unwrap();
        let id = tracks[0].audio_source_id;
        let volume_path = format!("/api/audio-sources/{id}/volume");
        assert_eq!(
            request(&state, "PUT", &volume_path, json!({"volume_percent":10}))
                .await
                .0,
            StatusCode::NO_CONTENT
        );
        let (_, settings) = request(
            &state,
            "PATCH",
            settings_path,
            json!({"global_volume_percent":40}),
        )
        .await;
        assert_eq!(settings["individual_volume_enabled"], true);
        let (_, tracks) = request(&state, "GET", "/api/tracks", Value::Null).await;
        assert_eq!(tracks[0]["volume_percent"], 10);
        let playlist = state.services().playlists().create("Volume").await.unwrap();
        state
            .services()
            .playlists()
            .add_items(
                playlist.id,
                &[tracks[0]["difficulties"][0]["beatmap_id"]
                    .as_i64()
                    .and_then(|id| i32::try_from(id).ok())
                    .unwrap()],
            )
            .await
            .unwrap();
        let (_, playlist_body) = request(
            &state,
            "GET",
            &format!("/api/playlists/{}", playlist.id),
            Value::Null,
        )
        .await;
        assert_eq!(playlist_body["items"][0]["volume_percent"], 10);
        let (_, playback) = request(
            &state,
            "POST",
            "/api/playback/commands",
            json!({"command":"play", "audio_source_id":id}),
        )
        .await;
        assert_eq!(playback["track"]["volume_percent"], 10);
        assert_eq!(playback["volume_percent"], 10);
        for value in [json!(-1), json!(101), json!(1.5), json!("10"), Value::Null] {
            let status = request(&state, "PUT", &volume_path, json!({"volume_percent":value}))
                .await
                .0;
            assert!(status.is_client_error());
            assert!(
                request(
                    &state,
                    "PATCH",
                    settings_path,
                    json!({"global_volume_percent":value})
                )
                .await
                .0
                .is_client_error()
            );
        }
        assert_eq!(
            request(
                &state,
                "PUT",
                "/api/audio-sources/999999/volume",
                json!({"volume_percent":10})
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            request(
                &state,
                "DELETE",
                "/api/audio-sources/999999/volume",
                Value::Null
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        for percent in [0, 100] {
            assert_eq!(
                request(
                    &state,
                    "PUT",
                    &volume_path,
                    json!({"volume_percent":percent})
                )
                .await
                .0,
                StatusCode::NO_CONTENT
            );
        }
        assert_eq!(
            request(&state, "DELETE", &volume_path, Value::Null).await.0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            request(&state, "DELETE", &volume_path, Value::Null).await.0,
            StatusCode::NO_CONTENT
        );
        let (_, tracks) = request(&state, "GET", "/api/tracks", Value::Null).await;
        assert!(tracks[0]["volume_percent"].is_null());
    }
}
