use super::user_data::{OsuFolderResponse, registration_error};
use crate::{error::ApiError, state::AppState};
use axum::{
    Json,
    body::Body,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use radio_services::{DiscoveryDepth, DiscoveryOptions};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct DiscoverRequest {
    #[serde(default)]
    #[cfg_attr(feature = "docs", schema(value_type = Vec<String>))]
    roots: Vec<PathBuf>,
    #[serde(default)]
    depth: Depth,
}
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
enum Depth {
    Known,
    Shallow,
    #[default]
    Full,
}

#[derive(Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) enum DiscoveryEvent {
    Candidate {
        kind: String,
        root_path: String,
        marker_path: String,
        registered_id: Option<i32>,
    },
    Complete,
}
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct MarkerRequest {
    #[cfg_attr(feature = "docs", schema(value_type = String))]
    marker_path: PathBuf,
}
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct MetadataResponse {
    beatmap_count: usize,
}

#[cfg_attr(feature = "docs", utoipa::path(post, path = "/api/user-data/osu-folders/discover", tag = "user-data",
    request_body = DiscoverRequest, responses((status = OK, body = DiscoveryEvent, content_type = "application/x-ndjson"),
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn discover(
    State(state): State<AppState>,
    Json(request): Json<DiscoverRequest>,
) -> Result<Response, ApiError> {
    if request.roots.iter().any(|root| !root.is_absolute()) {
        return Err(ApiError::bad_request(
            "Discovery roots must be absolute paths.",
        ));
    }
    let discovery = state
        .services()
        .osu_installations()
        .discover_folders(DiscoveryOptions {
            roots: request.roots,
            depth: match request.depth {
                Depth::Known => DiscoveryDepth::Known,
                Depth::Shallow => DiscoveryDepth::Shallow,
                Depth::Full => DiscoveryDepth::Full,
            },
            ..DiscoveryOptions::default()
        })
        .await?;
    let stream = futures_util::stream::unfold(Some(discovery), |discovery| async move {
        let mut discovery = discovery?;
        let (event, next) = if let Some(candidate) = discovery.next().await {
            (
                DiscoveryEvent::Candidate {
                    kind: candidate.marker.kind.as_str().into(),
                    root_path: candidate.marker.root_path.to_string_lossy().into_owned(),
                    marker_path: candidate.marker.marker_path.to_string_lossy().into_owned(),
                    registered_id: candidate.registered_id,
                },
                Some(discovery),
            )
        } else {
            (DiscoveryEvent::Complete, None)
        };
        // All fields are strings/integers; serialization cannot fail for this event.
        let chunk = serde_json::to_string(&event).map(|mut json| {
            json.push('\n');
            json
        });
        Some((chunk, next))
    });
    Ok((
        [(header::CONTENT_TYPE, "application/x-ndjson")],
        Body::from_stream(stream),
    )
        .into_response())
}

#[cfg_attr(feature = "docs", utoipa::path(post, path = "/api/user-data/osu-folders/metadata", tag = "user-data",
    request_body = MarkerRequest, responses((status = OK, body = MetadataResponse),
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn metadata(
    State(state): State<AppState>,
    Json(request): Json<MarkerRequest>,
) -> Result<Json<MetadataResponse>, ApiError> {
    let beatmap_count = state
        .services()
        .osu_installations()
        .folder_metadata(request.marker_path)
        .await
        .map_err(registration_error)?;
    Ok(Json(MetadataResponse { beatmap_count }))
}

#[cfg_attr(feature = "docs", utoipa::path(post, path = "/api/user-data/osu-folders/import", tag = "user-data",
    request_body = MarkerRequest, responses((status = OK, body = OsuFolderResponse),
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn import(
    State(state): State<AppState>,
    Json(request): Json<MarkerRequest>,
) -> Result<Json<OsuFolderResponse>, ApiError> {
    let folder = state
        .services()
        .osu_installations()
        .import_folder(request.marker_path)
        .await
        .map_err(registration_error)?
        .into_installation();
    Ok(Json(folder.into()))
}

#[cfg(all(test, feature = "sqlite"))]
#[path = "folder_selection_tests.rs"]
mod tests;
