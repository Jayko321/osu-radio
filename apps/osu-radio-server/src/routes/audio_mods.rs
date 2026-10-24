use crate::{error::ApiError, state::AppState};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
};
use radio_services::{AudioModsError, AudioModsSettings};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) enum AudioModsMode {
    None,
    Dt,
    Ht,
}
impl From<radio_services::AudioModsMode> for AudioModsMode {
    fn from(mode: radio_services::AudioModsMode) -> Self {
        match mode {
            radio_services::AudioModsMode::None => Self::None,
            radio_services::AudioModsMode::Dt => Self::Dt,
            radio_services::AudioModsMode::Ht => Self::Ht,
        }
    }
}
impl From<AudioModsMode> for radio_services::AudioModsMode {
    fn from(mode: AudioModsMode) -> Self {
        match mode {
            AudioModsMode::None => Self::None,
            AudioModsMode::Dt => Self::Dt,
            AudioModsMode::Ht => Self::Ht,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
/// none requires 100/false, DT 101–200, HT 50–99. NC is DT with `adjust_pitch=true`.
pub(crate) struct AudioModsProfile {
    mode: AudioModsMode,
    #[cfg_attr(feature = "docs", schema(minimum = 50, maximum = 200))]
    speed_percent: u16,
    adjust_pitch: bool,
}
impl From<radio_services::AudioModsProfile> for AudioModsProfile {
    fn from(profile: radio_services::AudioModsProfile) -> Self {
        Self {
            mode: profile.mode.into(),
            speed_percent: profile.speed_percent,
            adjust_pitch: profile.adjust_pitch,
        }
    }
}
impl From<AudioModsProfile> for radio_services::AudioModsProfile {
    fn from(profile: AudioModsProfile) -> Self {
        Self {
            mode: profile.mode.into(),
            speed_percent: profile.speed_percent,
            adjust_pitch: profile.adjust_pitch,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct AudioModsResponse {
    individual_mods_enabled: bool,
    global_profile: AudioModsProfile,
}
impl From<AudioModsSettings> for AudioModsResponse {
    fn from(settings: AudioModsSettings) -> Self {
        Self {
            individual_mods_enabled: settings.individual_mods_enabled,
            global_profile: settings.global_profile.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
/// Omitted fields are retained and the merged profile is validated before writing. Null is invalid.
pub(crate) struct AudioModsPatch {
    #[serde(default, deserialize_with = "super::audio_settings::present")]
    #[cfg_attr(feature = "docs", schema(nullable = false))]
    individual_mods_enabled: Option<bool>,
    #[serde(default, deserialize_with = "super::audio_settings::present")]
    #[cfg_attr(feature = "docs", schema(nullable = false))]
    global_mode: Option<AudioModsMode>,
    #[serde(default, deserialize_with = "super::audio_settings::present")]
    #[cfg_attr(
        feature = "docs",
        schema(nullable = false, minimum = 50, maximum = 200)
    )]
    global_speed_percent: Option<u16>,
    #[serde(default, deserialize_with = "super::audio_settings::present")]
    #[cfg_attr(feature = "docs", schema(nullable = false))]
    global_adjust_pitch: Option<bool>,
}

fn mods_error(error: anyhow::Error) -> ApiError {
    match error.downcast_ref::<AudioModsError>() {
        Some(reason @ AudioModsError::UnknownAudio(_)) => ApiError::not_found(reason.to_string()),
        Some(reason) => ApiError::bad_request(reason.to_string()),
        None => error.into(),
    }
}

#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/user-data/audio-mods", tag = "user-data",
    responses((status = OK, body = AudioModsResponse), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn get(
    State(state): State<AppState>,
) -> Result<Json<AudioModsResponse>, ApiError> {
    Ok(Json(state.services().audio_mods().get().await?.into()))
}

#[cfg_attr(feature = "docs", utoipa::path(patch, path = "/api/user-data/audio-mods", tag = "user-data", request_body = AudioModsPatch,
    responses((status = OK, body = AudioModsResponse), (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn patch(
    State(state): State<AppState>,
    body: Result<Json<AudioModsPatch>, JsonRejection>,
) -> Result<Json<AudioModsResponse>, ApiError> {
    let Json(body) = body.map_err(|error| ApiError::bad_request(error.body_text()))?;
    let settings = state
        .services()
        .audio_mods()
        .update(radio_services::AudioModsPatch {
            individual_mods_enabled: body.individual_mods_enabled,
            global_mode: body.global_mode.map(Into::into),
            global_speed_percent: body.global_speed_percent,
            global_adjust_pitch: body.global_adjust_pitch,
        })
        .await
        .map_err(mods_error)?;
    Ok(Json(settings.into()))
}

#[cfg_attr(feature = "docs", utoipa::path(put, path = "/api/audio-sources/{id}/mods", tag = "user-data", params(("id" = i32, Path)), request_body = AudioModsProfile,
    responses((status = NO_CONTENT), (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = NOT_FOUND, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn put(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    body: Result<Json<AudioModsProfile>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(body) = body.map_err(|error| ApiError::bad_request(error.body_text()))?;
    state
        .services()
        .audio_mods()
        .set_profile(id, Some(body.into()))
        .await
        .map_err(mods_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg_attr(feature = "docs", utoipa::path(delete, path = "/api/audio-sources/{id}/mods", tag = "user-data", params(("id" = i32, Path)),
    responses((status = NO_CONTENT), (status = NOT_FOUND, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    state
        .services()
        .audio_mods()
        .set_profile(id, None)
        .await
        .map_err(mods_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(all(test, feature = "sqlite"))]
mod tests;
