use std::time::Duration;

use axum::{
    Json,
    body::Body,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use radio_services::{PlaybackAssignment, PlaybackCommand, QueueError, QueueState};
use serde::{Deserialize, Serialize};

use super::{media::probe_source_duration, tracks::TrackResponse};
use crate::{error::ApiError, state::AppState};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) enum PlaybackMode {
    Stopped,
    Paused,
    Playing,
}

impl From<radio_services::PlaybackMode> for PlaybackMode {
    fn from(mode: radio_services::PlaybackMode) -> Self {
        match mode {
            radio_services::PlaybackMode::Stopped => Self::Stopped,
            radio_services::PlaybackMode::Paused => Self::Paused,
            radio_services::PlaybackMode::Playing => Self::Playing,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct QueueResponse {
    upcoming_tracks: Vec<TrackResponse>,
    audio_source_ids: Vec<i32>,
    playlist_item_ids: Vec<Option<i32>>,
    current_index: Option<usize>,
    mode: PlaybackMode,
    revision: u64,
    playback_token: u64,
}

impl From<QueueState> for QueueResponse {
    fn from(queue: QueueState) -> Self {
        Self {
            upcoming_tracks: Vec::new(),
            audio_source_ids: queue.audio_source_ids,
            playlist_item_ids: queue.playlist_item_ids,
            current_index: queue.current_index,
            mode: queue.mode.into(),
            revision: queue.revision,
            playback_token: queue.playback_token,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct PlaybackResponse {
    volume_percent: Option<u8>,
    current_audio_source_id: Option<i32>,
    current_playlist_item_id: Option<i32>,
    track: Option<TrackResponse>,
    duration_ms: Option<u64>,
    mode: PlaybackMode,
    revision: u64,
    playback_token: u64,
    can_next: bool,
    can_previous: bool,
}

impl PlaybackResponse {
    async fn from_assignment(
        state: &AppState,
        assignment: PlaybackAssignment,
    ) -> Result<Self, ApiError> {
        let duration_ms = if let Some(id) = assignment.current_audio_source_id {
            if let Some(source) = state.services().audio_sources().get(id).await? {
                probe_source_duration(source.s_type).await?
            } else {
                None
            }
        } else {
            None
        };
        Ok(Self {
            volume_percent: assignment.volume_percent,
            current_audio_source_id: assignment.current_audio_source_id,
            current_playlist_item_id: assignment.current_playlist_item_id,
            track: assignment.track.map(TrackResponse::from),
            duration_ms,
            mode: assignment.mode.into(),
            revision: assignment.revision,
            playback_token: assignment.playback_token,
            can_next: assignment.can_next,
            can_previous: assignment.can_previous,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct AppendRequest {
    audio_source_ids: Vec<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) enum CommandRequest {
    Play { audio_source_id: Option<i32> },
    Pause { playback_token: Option<u64> },
    Stop,
    Next,
    Previous,
    Started { playback_token: u64 },
    Finished { playback_token: u64 },
    Failed { playback_token: u64 },
}

impl From<CommandRequest> for PlaybackCommand {
    fn from(command: CommandRequest) -> Self {
        match command {
            CommandRequest::Play { audio_source_id } => Self::Play { audio_source_id },
            CommandRequest::Pause {
                playback_token: None,
            } => Self::Pause,
            CommandRequest::Pause {
                playback_token: Some(playback_token),
            } => Self::PauseIfCurrent { playback_token },
            CommandRequest::Stop => Self::Stop,
            CommandRequest::Next => Self::Next,
            CommandRequest::Previous => Self::Previous,
            CommandRequest::Started { playback_token } => Self::Started { playback_token },
            CommandRequest::Finished { playback_token } => Self::Finished { playback_token },
            CommandRequest::Failed { playback_token } => Self::Failed { playback_token },
        }
    }
}

fn queue_error(error: anyhow::Error) -> ApiError {
    if let Some(QueueError::InvalidAudioSource(id)) = error.downcast_ref::<QueueError>() {
        return ApiError::bad_request(format!("Audio source {id} is unavailable or unsupported."));
    }
    error.into()
}

pub(super) async fn committed_response(
    state: &AppState,
    assignment: PlaybackAssignment,
) -> Result<Json<PlaybackResponse>, ApiError> {
    // Notify immediately after commit, before duration probing can fail or be cancelled.
    state.publish_playback(assignment.revision);
    Ok(Json(
        PlaybackResponse::from_assignment(state, assignment).await?,
    ))
}

#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/queue", tag = "playback",
    responses((status = OK, body = QueueResponse), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn get_queue(
    State(state): State<AppState>,
) -> Result<Json<QueueResponse>, ApiError> {
    let (queue, tracks) = state.services().queue().upcoming().await?;
    let mut response = QueueResponse::from(queue);
    response.upcoming_tracks = tracks.into_iter().map(TrackResponse::from).collect();
    Ok(Json(response))
}

#[cfg_attr(feature = "docs", utoipa::path(post, path = "/api/queue/items", tag = "playback",
    request_body = AppendRequest, responses((status = OK, body = PlaybackResponse),
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn append_items(
    State(state): State<AppState>,
    Json(request): Json<AppendRequest>,
) -> Result<Json<PlaybackResponse>, ApiError> {
    let assignment = state
        .services()
        .queue()
        .append(request.audio_source_ids)
        .await
        .map_err(queue_error)?;
    committed_response(&state, assignment).await
}

#[cfg_attr(feature = "docs", utoipa::path(delete, path = "/api/queue", tag = "playback",
    responses((status = OK, body = PlaybackResponse), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn clear_queue(
    State(state): State<AppState>,
) -> Result<Json<PlaybackResponse>, ApiError> {
    let assignment = state.services().queue().clear().await?;
    committed_response(&state, assignment).await
}

#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/playback", tag = "playback",
    responses((status = OK, body = PlaybackResponse), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn get_playback(
    State(state): State<AppState>,
) -> Result<Json<PlaybackResponse>, ApiError> {
    let assignment = state.services().queue().playback().await?;
    Ok(Json(
        PlaybackResponse::from_assignment(&state, assignment).await?,
    ))
}

#[cfg_attr(feature = "docs", utoipa::path(post, path = "/api/playback/commands", tag = "playback",
    request_body = CommandRequest, responses((status = OK, body = PlaybackResponse),
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn command(
    State(state): State<AppState>,
    Json(request): Json<CommandRequest>,
) -> Result<Json<PlaybackResponse>, ApiError> {
    let assignment = state
        .services()
        .queue()
        .command(request.into())
        .await
        .map_err(queue_error)?;
    committed_response(&state, assignment).await
}

#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/playback/events", tag = "playback",
    description = "Immediately returns the current assignment, then committed revisions as NDJSON. Blank-line heartbeats every 15 seconds. Reconnect to synchronize after the client one-hour stream timeout.",
    responses((status = OK, body = PlaybackResponse, content_type = "application/x-ndjson"),
    (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn events(State(state): State<AppState>) -> Result<Response, ApiError> {
    // Subscribe before reading: a commit during initial metadata probing is still observed.
    let changes = state.subscribe_playback();
    let initial =
        PlaybackResponse::from_assignment(&state, state.services().queue().playback().await?)
            .await?;
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.tick().await;
    let stream = futures_util::stream::unfold(
        (state, changes, heartbeat, Some(initial), None),
        |(state, mut changes, mut heartbeat, initial, mut last_revision)| async move {
            if let Some(initial) = initial {
                last_revision = Some(initial.revision);
                return Some((
                    encode_assignment(&initial),
                    (state, changes, heartbeat, None, last_revision),
                ));
            }
            loop {
                let heartbeat_tick = tokio::select! {
                    notification = changes.changed() => {
                        if notification.is_err() { return None; }
                        false
                    }
                    _ = heartbeat.tick() => true,
                };
                // Heartbeats also detect commits made by a different Services handle/process,
                // or a request cancelled after its commit but before notification.
                let assignment = match state.services().queue().playback().await {
                    Ok(assignment) => assignment,
                    Err(error) => {
                        eprintln!("Playback stream failed: {error:#}");
                        return None;
                    }
                };
                if last_revision.is_none_or(|revision| assignment.revision > revision) {
                    state.publish_playback(assignment.revision);
                    last_revision = Some(assignment.revision);
                    let response = match PlaybackResponse::from_assignment(&state, assignment).await
                    {
                        Ok(response) => response,
                        Err(error) => {
                            eprintln!("Playback stream failed: {error:?}");
                            return None;
                        }
                    };
                    return Some((
                        encode_assignment(&response),
                        (state, changes, heartbeat, None, last_revision),
                    ));
                }
                if heartbeat_tick {
                    return Some((
                        Ok("\n".to_owned()),
                        (state, changes, heartbeat, None, last_revision),
                    ));
                }
            }
        },
    );
    // The response owns the stream and its watch receiver. Cancellation drops both;
    // there is no detached task or transaction waiting for the next notification.
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-ndjson"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

fn encode_assignment(response: &PlaybackResponse) -> Result<String, serde_json::Error> {
    serde_json::to_string(response).map(|mut json| {
        json.push('\n');
        json
    })
}

#[cfg(all(test, feature = "sqlite"))]
#[path = "playback_tests.rs"]
mod tests;
