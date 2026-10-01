use super::playback::{PlaybackResponse, committed_response};
use crate::{error::ApiError, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use radio_services::{Playlist, PlaylistError, PlaylistItem, PlaylistSummary};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct PlaylistSummaryResponse {
    id: i32,
    name: String,
}
impl From<PlaylistSummary> for PlaylistSummaryResponse {
    fn from(value: PlaylistSummary) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct PlaylistItemResponse {
    id: i32,
    playlist_id: i32,
    source_kind: String,
    beatmap_hash: String,
    title: Option<String>,
    artist: Option<String>,
    difficulty_name: Option<String>,
    beatmap_id: Option<i32>,
    beatmap_set_id: Option<i32>,
    audio_source_id: Option<i32>,
    cover_beatmap_id: Option<i32>,
}
impl From<PlaylistItem> for PlaylistItemResponse {
    fn from(item: PlaylistItem) -> Self {
        Self {
            id: item.id,
            playlist_id: item.playlist_id,
            source_kind: item.source_kind,
            beatmap_hash: item.beatmap_hash,
            title: item.title,
            artist: item.artist,
            difficulty_name: item.difficulty_name,
            beatmap_id: item.beatmap_id,
            beatmap_set_id: item.beatmap_set_id,
            audio_source_id: item.audio_source_id,
            cover_beatmap_id: item.cover_beatmap_id,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct PlaylistResponse {
    id: i32,
    name: String,
    items: Vec<PlaylistItemResponse>,
}
impl From<Playlist> for PlaylistResponse {
    fn from(value: Playlist) -> Self {
        Self {
            id: value.id,
            name: value.name,
            items: value.items.into_iter().map(Into::into).collect(),
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct NameRequest {
    name: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct ItemsRequest {
    beatmap_ids: Vec<i32>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct PlayRequest {
    start_item_id: Option<i32>,
}

fn playlist_error(error: anyhow::Error) -> ApiError {
    if let Some(reason) = error.downcast_ref::<PlaylistError>() {
        return match reason {
            PlaylistError::NotFound(_) | PlaylistError::ItemNotFound(_) => {
                ApiError::not_found(reason.to_string())
            }
            _ => ApiError::bad_request(reason.to_string()),
        };
    }
    error.into()
}

#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/playlists", tag = "playlists",
    responses((status = OK, body = Vec<PlaylistSummaryResponse>))))]
pub(crate) async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<PlaylistSummaryResponse>>, ApiError> {
    Ok(Json(
        state
            .services()
            .playlists()
            .all()
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}
#[cfg_attr(feature = "docs", utoipa::path(post, path = "/api/playlists", tag = "playlists", request_body = NameRequest,
    responses((status = CREATED, body = PlaylistSummaryResponse), (status = BAD_REQUEST))))]
pub(crate) async fn create(
    State(state): State<AppState>,
    Json(body): Json<NameRequest>,
) -> Result<(StatusCode, Json<PlaylistSummaryResponse>), ApiError> {
    let playlist = state
        .services()
        .playlists()
        .create(&body.name)
        .await
        .map_err(playlist_error)?;
    Ok((StatusCode::CREATED, Json(playlist.into())))
}
#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/playlists/{id}", tag = "playlists", params(("id" = i32, Path)),
    responses((status = OK, body = PlaylistResponse), (status = NOT_FOUND))))]
pub(crate) async fn get(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<PlaylistResponse>, ApiError> {
    let playlist = state
        .services()
        .playlists()
        .get(id)
        .await?
        .ok_or_else(|| ApiError::not_found("Playlist was not found."))?;
    Ok(Json(playlist.into()))
}
#[cfg_attr(feature = "docs", utoipa::path(patch, path = "/api/playlists/{id}", tag = "playlists", params(("id" = i32, Path)), request_body = NameRequest,
    responses((status = OK, body = PlaylistSummaryResponse), (status = NOT_FOUND), (status = BAD_REQUEST))))]
pub(crate) async fn rename(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<NameRequest>,
) -> Result<Json<PlaylistSummaryResponse>, ApiError> {
    Ok(Json(
        state
            .services()
            .playlists()
            .rename(id, &body.name)
            .await
            .map_err(playlist_error)?
            .into(),
    ))
}
#[cfg_attr(feature = "docs", utoipa::path(delete, path = "/api/playlists/{id}", tag = "playlists", params(("id" = i32, Path)),
    responses((status = NO_CONTENT), (status = NOT_FOUND))))]
pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode, ApiError> {
    state
        .services()
        .playlists()
        .delete(id)
        .await
        .map_err(playlist_error)?;
    Ok(StatusCode::NO_CONTENT)
}
#[cfg_attr(feature = "docs", utoipa::path(post, path = "/api/playlists/{id}/items", tag = "playlists", params(("id" = i32, Path)), request_body = ItemsRequest,
    responses((status = OK, body = PlaylistResponse), (status = NOT_FOUND), (status = BAD_REQUEST))))]
pub(crate) async fn add_items(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(body): Json<ItemsRequest>,
) -> Result<Json<PlaylistResponse>, ApiError> {
    Ok(Json(
        state
            .services()
            .playlists()
            .add_items(id, &body.beatmap_ids)
            .await
            .map_err(playlist_error)?
            .into(),
    ))
}
#[cfg_attr(feature = "docs", utoipa::path(delete, path = "/api/playlists/{id}/items/{item_id}", tag = "playlists", params(("id" = i32, Path), ("item_id" = i32, Path)),
    responses((status = NO_CONTENT), (status = NOT_FOUND))))]
pub(crate) async fn remove_item(
    State(state): State<AppState>,
    Path((id, item_id)): Path<(i32, i32)>,
) -> Result<StatusCode, ApiError> {
    state
        .services()
        .playlists()
        .remove_item(id, item_id)
        .await
        .map_err(playlist_error)?;
    Ok(StatusCode::NO_CONTENT)
}
#[cfg_attr(feature = "docs", utoipa::path(post, path = "/api/playlists/{id}/play", tag = "playlists", params(("id" = i32, Path)), request_body = PlayRequest,
    responses((status = OK, body = PlaybackResponse), (status = NOT_FOUND), (status = BAD_REQUEST))))]
pub(crate) async fn play(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    body: Option<Json<PlayRequest>>,
) -> Result<Json<PlaybackResponse>, ApiError> {
    let assignment = state
        .services()
        .playlists()
        .play(id, body.and_then(|body| body.start_item_id))
        .await
        .map_err(playlist_error)?;
    committed_response(&state, assignment).await
}

#[cfg(all(test, feature = "sqlite"))]
mod tests;
