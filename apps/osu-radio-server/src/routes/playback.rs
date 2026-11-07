use std::time::Duration;

use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
};
use radio_services::{PlaybackAssignment, PlaybackCommand, QueueError, QueueSnapshot};
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) enum RepeatMode {
    Off,
    All,
    One,
}

impl From<radio_services::RepeatMode> for RepeatMode {
    fn from(mode: radio_services::RepeatMode) -> Self {
        match mode {
            radio_services::RepeatMode::Off => Self::Off,
            radio_services::RepeatMode::All => Self::All,
            radio_services::RepeatMode::One => Self::One,
        }
    }
}

impl From<RepeatMode> for radio_services::RepeatMode {
    fn from(mode: RepeatMode) -> Self {
        match mode {
            RepeatMode::Off => Self::Off,
            RepeatMode::All => Self::All,
            RepeatMode::One => Self::One,
        }
    }
}

/// A positive BIGINT carried as a decimal string, never a JSON number.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "docs", schema(value_type = String, pattern = "^0*[1-9][0-9]*$", example = "9223372036854775807"))]
pub(crate) struct EntryId(radio_services::QueueEntryId);

impl TryFrom<String> for EntryId {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Expected a positive decimal i64 string");
        }
        value
            .parse::<i64>()
            .ok()
            .and_then(|id| radio_services::QueueEntryId::new(id).ok())
            .map(Self)
            .ok_or("Expected a positive decimal i64 string")
    }
}
impl From<EntryId> for String {
    fn from(id: EntryId) -> Self {
        id.0.to_string()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) enum Availability {
    Checking,
    Available,
    Unavailable,
}
impl From<radio_services::QueueAvailability> for Availability {
    fn from(value: radio_services::QueueAvailability) -> Self {
        match value {
            radio_services::QueueAvailability::Checking => Self::Checking,
            radio_services::QueueAvailability::Available => Self::Available,
            radio_services::QueueAvailability::Unavailable => Self::Unavailable,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct QueueEntryResponse {
    id: EntryId,
    position: usize,
    #[cfg_attr(feature = "docs", schema(required = true))]
    audio_source_id: Option<i32>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    playlist_item_id: Option<i32>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    title: Option<String>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    title_unicode: Option<String>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    artist: Option<String>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    artist_unicode: Option<String>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    difficulty_name: Option<String>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    beatmap_id: Option<i32>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    beatmap_set_id: Option<i32>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    cover_beatmap_id: Option<i32>,
    availability: Availability,
    #[cfg_attr(feature = "docs", schema(required = true))]
    last_played_at_ms: Option<i64>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    volume_percent: Option<u8>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    mods: Option<super::audio_mods::AudioModsProfile>,
}
impl From<radio_services::QueueSnapshotEntry> for QueueEntryResponse {
    fn from(snapshot: radio_services::QueueSnapshotEntry) -> Self {
        let entry = snapshot.entry;
        Self {
            id: EntryId(entry.id),
            position: entry.position,
            audio_source_id: entry.audio_source_id,
            playlist_item_id: entry.playlist_item_id,
            title: entry.title,
            title_unicode: entry.title_unicode,
            artist: entry.artist,
            artist_unicode: entry.artist_unicode,
            difficulty_name: entry.difficulty_name,
            beatmap_id: entry.beatmap_id,
            beatmap_set_id: entry.beatmap_set_id,
            cover_beatmap_id: entry.cover_beatmap_id,
            availability: entry.availability.into(),
            last_played_at_ms: snapshot.last_played_at_ms,
            volume_percent: snapshot.volume_percent,
            mods: snapshot.mods.map(Into::into),
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct QueueResponse {
    entries: Vec<QueueEntryResponse>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    current_entry_id: Option<EntryId>,
    history_len: usize,
    mode: PlaybackMode,
    waiting_for_check: bool,
    shuffle: bool,
    repeat: RepeatMode,
    revision: u64,
    playback_token: u64,
}
impl From<QueueSnapshot> for QueueResponse {
    fn from(queue: QueueSnapshot) -> Self {
        Self {
            entries: queue.entries.into_iter().map(Into::into).collect(),
            current_entry_id: queue.current_entry_id.map(EntryId),
            history_len: queue.history_len,
            mode: queue.mode.into(),
            waiting_for_check: queue.waiting_for_check,
            shuffle: queue.shuffle,
            repeat: queue.repeat.into(),
            revision: queue.revision,
            playback_token: queue.playback_token,
        }
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
#[allow(clippy::struct_excessive_bools)] // Mirrors independent JSON flags of the wire contract.
pub(crate) struct PlaybackResponse {
    #[cfg_attr(feature = "docs", schema(required = true))]
    current_entry_id: Option<EntryId>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    availability: Option<Availability>,
    waiting_for_check: bool,
    #[cfg_attr(feature = "docs", schema(required = true))]
    volume_percent: Option<u8>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    mods: Option<super::audio_mods::AudioModsProfile>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    current_audio_source_id: Option<i32>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    current_playlist_item_id: Option<i32>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    track: Option<TrackResponse>,
    #[cfg_attr(feature = "docs", schema(required = true))]
    duration_ms: Option<u64>,
    mode: PlaybackMode,
    shuffle: bool,
    repeat: RepeatMode,
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
        let duration_ms = if assignment.availability
            == Some(radio_services::QueueAvailability::Available)
            && !assignment.waiting_for_check
            && let Some(id) = assignment.current_audio_source_id
        {
            if let Some(source) = state.services().audio_sources().get(id).await? {
                probe_source_duration(source.s_type).await?
            } else {
                None
            }
        } else {
            None
        };
        Ok(Self {
            current_entry_id: assignment.current_entry_id.map(EntryId),
            availability: assignment.availability.map(Into::into),
            waiting_for_check: assignment.waiting_for_check,
            volume_percent: assignment.volume_percent,
            mods: assignment.mods.map(Into::into),
            current_audio_source_id: assignment.current_audio_source_id,
            current_playlist_item_id: assignment.current_playlist_item_id,
            track: assignment.track.map(TrackResponse::from),
            duration_ms,
            mode: assignment.mode.into(),
            shuffle: assignment.shuffle,
            repeat: assignment.repeat.into(),
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
    items: Vec<QueueItem>,
    placement: Placement,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) enum QueueItem {
    Audio { audio_source_id: i32 },
    PlaylistItem { playlist_id: i32, item_id: i32 },
}
impl From<QueueItem> for radio_services::QueueItemRef {
    fn from(item: QueueItem) -> Self {
        match item {
            QueueItem::Audio { audio_source_id } => Self::Audio(audio_source_id),
            QueueItem::PlaylistItem {
                playlist_id,
                item_id,
            } => Self::PlaylistItem {
                playlist_id,
                item_id,
            },
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) enum Placement {
    End,
    Next,
}
impl From<Placement> for radio_services::QueuePlacement {
    fn from(placement: Placement) -> Self {
        match placement {
            Placement::End => Self::End,
            Placement::Next => Self::Next,
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct MoveRequest {
    #[serde(deserialize_with = "required_anchor")]
    #[cfg_attr(feature = "docs", schema(required = true))]
    before_entry_id: Option<EntryId>,
}
fn required_anchor<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<EntryId>, D::Error> {
    Option::deserialize(d)
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
    SetShuffle { enabled: bool },
    SetRepeat { mode: RepeatMode },
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
            CommandRequest::SetShuffle { enabled } => Self::SetShuffle { enabled },
            CommandRequest::SetRepeat { mode } => Self::SetRepeat(mode.into()),
        }
    }
}

fn queue_error(error: anyhow::Error) -> ApiError {
    if let Some(reason) = error.downcast_ref::<QueueError>() {
        return match reason {
            QueueError::UnavailableEntry(_) => ApiError::bad_request(reason.to_string()),
            _ => ApiError::not_found(reason.to_string()),
        };
    }
    error.into()
}

pub(super) async fn committed_response(
    state: &AppState,
    assignment: PlaybackAssignment,
) -> Result<Json<PlaybackResponse>, ApiError> {
    // Notify immediately after commit, before duration probing can fail or be cancelled.
    state.publish_playback(assignment.revision);
    state.wake_queue_checks();
    Ok(Json(
        PlaybackResponse::from_assignment(state, assignment).await?,
    ))
}

#[cfg_attr(feature = "docs", utoipa::path(get, path = "/api/queue", tag = "playback",
    responses((status = OK, body = QueueResponse), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn get_queue(
    State(state): State<AppState>,
) -> Result<Json<QueueResponse>, ApiError> {
    Ok(Json(state.services().queue().snapshot().await?.into()))
}

#[cfg_attr(feature = "docs", utoipa::path(post, path = "/api/queue/items", tag = "playback",
    request_body = AppendRequest, responses((status = OK, body = PlaybackResponse),
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = NOT_FOUND, body = crate::error::ApiErrorBody), (status = UNPROCESSABLE_ENTITY), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn append_items(
    State(state): State<AppState>,
    Json(request): Json<AppendRequest>,
) -> Result<Json<PlaybackResponse>, ApiError> {
    let assignment = state
        .services()
        .queue()
        .insert(
            request.items.into_iter().map(Into::into).collect(),
            request.placement.into(),
        )
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

#[cfg_attr(feature = "docs", utoipa::path(post, path = "/api/queue/items/{entry_id}/play", tag = "playback",
    params(("entry_id" = EntryId, Path)), responses((status = OK, body = PlaybackResponse),
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = NOT_FOUND, body = crate::error::ApiErrorBody),
    (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn play_entry(
    State(state): State<AppState>,
    Path(entry_id): Path<EntryId>,
) -> Result<Json<PlaybackResponse>, ApiError> {
    let assignment = state
        .services()
        .queue()
        .play_entry(entry_id.0)
        .await
        .map_err(queue_error)?;
    committed_response(&state, assignment).await
}

#[cfg_attr(feature = "docs", utoipa::path(patch, path = "/api/queue/items/{entry_id}", tag = "playback",
    params(("entry_id" = EntryId, Path)), request_body = MoveRequest, responses((status = OK, body = PlaybackResponse),
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = NOT_FOUND, body = crate::error::ApiErrorBody),
    (status = UNPROCESSABLE_ENTITY), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn move_entry(
    State(state): State<AppState>,
    Path(entry_id): Path<EntryId>,
    Json(request): Json<MoveRequest>,
) -> Result<Json<PlaybackResponse>, ApiError> {
    let assignment = state
        .services()
        .queue()
        .move_entry(entry_id.0, request.before_entry_id.map(|id| id.0))
        .await
        .map_err(queue_error)?;
    committed_response(&state, assignment).await
}

#[cfg_attr(feature = "docs", utoipa::path(delete, path = "/api/queue/items/{entry_id}", tag = "playback",
    params(("entry_id" = EntryId, Path)), responses((status = OK, body = PlaybackResponse),
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = NOT_FOUND, body = crate::error::ApiErrorBody),
    (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn remove_entry(
    State(state): State<AppState>,
    Path(entry_id): Path<EntryId>,
) -> Result<Json<PlaybackResponse>, ApiError> {
    let assignment = state
        .services()
        .queue()
        .remove_entry(entry_id.0)
        .await
        .map_err(queue_error)?;
    committed_response(&state, assignment).await
}

#[cfg_attr(feature = "docs", utoipa::path(delete, path = "/api/queue/upcoming", tag = "playback",
    responses((status = OK, body = PlaybackResponse), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
pub(crate) async fn clear_upcoming(
    State(state): State<AppState>,
) -> Result<Json<PlaybackResponse>, ApiError> {
    let assignment = state
        .services()
        .queue()
        .clear_upcoming()
        .await
        .map_err(queue_error)?;
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
    (status = BAD_REQUEST, body = crate::error::ApiErrorBody), (status = NOT_FOUND, body = crate::error::ApiErrorBody), (status = UNPROCESSABLE_ENTITY), (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody))))]
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
                let revision = match state.services().queue().revision().await {
                    Ok(revision) => revision,
                    Err(error) => {
                        eprintln!("Playback stream revision read failed: {error:#}");
                        return None;
                    }
                };
                if last_revision.is_some_and(|last| revision <= last) {
                    if heartbeat_tick {
                        return Some((
                            Ok("\n".to_owned()),
                            (state, changes, heartbeat, None, last_revision),
                        ));
                    }
                    continue;
                }
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
