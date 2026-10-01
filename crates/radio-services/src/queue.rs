use anyhow::{Context, Result};
use radio_db::{Database, Transaction};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

use crate::{LibraryTrack, ListeningHistoryService};
pub use radio_db::model::{PlaybackMode, QueueState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackAssignment {
    pub volume_percent: Option<u8>,
    pub current_audio_source_id: Option<i32>,
    pub current_playlist_item_id: Option<i32>,
    pub track: Option<LibraryTrack>,
    pub mode: PlaybackMode,
    pub revision: u64,
    pub playback_token: u64,
    pub can_next: bool,
    pub can_previous: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackCommand {
    Play { audio_source_id: Option<i32> },
    Pause,
    PauseIfCurrent { playback_token: u64 },
    Stop,
    Next,
    Previous,
    Started { playback_token: u64 },
    Finished { playback_token: u64 },
    Failed { playback_token: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueError {
    InvalidAudioSource(i32),
}

impl fmt::Display for QueueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAudioSource(id) => write!(
                formatter,
                "Audio source {id} is missing or unsupported for playback"
            ),
        }
    }
}
impl Error for QueueError {}

pub struct QueueService<'a> {
    pub(crate) database: &'a Database,
}

enum Change {
    Append(Vec<i32>),
    Clear,
    Command(PlaybackCommand),
    Recover,
}

impl QueueService<'_> {
    pub(crate) async fn replace_in(
        transaction: &Transaction,
        ids: Vec<i32>,
        item_ids: Vec<Option<i32>>,
        start: usize,
    ) -> Result<PlaybackAssignment> {
        let mut state = transaction.queue().get().await?;
        state.audio_source_ids = ids;
        state.playlist_item_ids = item_ids;
        launch(&mut state, start, PlaybackMode::Playing)?;
        state.revision = state
            .revision
            .checked_add(1)
            .context("Playback revision exhausted")?;
        let playable = transaction
            .audio_sources()
            .playable_ids(&state.audio_source_ids)
            .await?;
        transaction.queue().save(&state).await?;
        assignment(transaction, &state, &playable).await
    }
    pub async fn get(&self) -> Result<QueueState> {
        self.database.queue().get().await
    }

    /// Pending playable entries and their metadata from the same queue snapshot.
    pub async fn upcoming(&self) -> Result<(QueueState, Vec<LibraryTrack>)> {
        let transaction = self.database.begin_read().await?;
        let state = transaction.queue().get().await?;
        let start = state
            .current_index
            .map_or(0, |index| index.saturating_add(1));
        let ids = state.audio_source_ids.get(start..).unwrap_or_default();
        let playable = transaction.audio_sources().playable_ids(ids).await?;
        let mut metadata = HashMap::new();
        let mut tracks = Vec::new();
        for id in ids {
            if !playable.contains(id) {
                continue;
            }
            let track = match metadata.entry(*id) {
                std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(track_in(&transaction, *id, None).await?)
                }
            };
            tracks.push(track.clone());
        }
        transaction.commit().await?;
        Ok((state, tracks))
    }

    pub async fn playback(&self) -> Result<PlaybackAssignment> {
        let transaction = self.database.begin_read().await?;
        let state = transaction.queue().get().await?;
        let playable = transaction
            .audio_sources()
            .playable_ids(&state.audio_source_ids)
            .await?;
        let assignment = assignment(&transaction, &state, &playable).await?;
        transaction.commit().await?;
        Ok(assignment)
    }

    pub async fn append(&self, ids: Vec<i32>) -> Result<PlaybackAssignment> {
        self.change(Change::Append(ids)).await
    }

    pub async fn clear(&self) -> Result<PlaybackAssignment> {
        self.change(Change::Clear).await
    }

    pub async fn command(&self, command: PlaybackCommand) -> Result<PlaybackAssignment> {
        self.change(Change::Command(command)).await
    }

    /// Startup recovery retains position/history and invalidates the old process's callbacks.
    pub async fn recover(&self) -> Result<()> {
        self.change(Change::Recover).await?;
        Ok(())
    }

    async fn change(&self, change: Change) -> Result<PlaybackAssignment> {
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        let mut state = transaction.queue().get().await?;
        let original = state.clone();
        let mut ids = state.audio_source_ids.clone();
        match &change {
            Change::Append(incoming) => ids.extend(incoming),
            Change::Command(PlaybackCommand::Play {
                audio_source_id: Some(id),
            }) => ids.push(*id),
            _ => {}
        }
        let playable = transaction.audio_sources().playable_ids(&ids).await?;
        let mut history_changed = false;
        match change {
            Change::Append(incoming) => {
                for id in &incoming {
                    validate(*id, &playable)?;
                }
                if !incoming.is_empty() {
                    let old_len = state.audio_source_ids.len();
                    let exhausted = state.current_index.is_none_or(|index| index >= old_len);
                    state
                        .playlist_item_ids
                        .extend(std::iter::repeat_n(None, incoming.len()));
                    state.audio_source_ids.extend(incoming);
                    if exhausted {
                        launch(&mut state, old_len, PlaybackMode::Playing)?;
                    }
                }
            }
            Change::Clear => {
                if !state.audio_source_ids.is_empty() {
                    state.audio_source_ids.clear();
                    state.playlist_item_ids.clear();
                    state.current_index = None;
                    state.mode = PlaybackMode::Stopped;
                    invalidate(&mut state)?;
                }
            }
            Change::Command(PlaybackCommand::Started { playback_token }) => {
                if state.playback_token == playback_token
                    && state.mode != PlaybackMode::Stopped
                    && let Some(id) = state
                        .current_index
                        .and_then(|index| state.audio_source_ids.get(index))
                    && playable.contains(id)
                    && let Some(source) = transaction.audio_sources().get(*id).await?
                {
                    history_changed = ListeningHistoryService::record_started(
                        &transaction,
                        &source.s_type,
                        playback_token,
                    )
                    .await?;
                }
            }
            Change::Command(command) => apply(&mut state, command, &playable)?,
            Change::Recover => {
                if state
                    .current_index
                    .is_some_and(|index| index < state.audio_source_ids.len())
                {
                    state.mode = PlaybackMode::Paused;
                    invalidate(&mut state)?;
                }
            }
        }
        if state != original || history_changed {
            state.revision = state
                .revision
                .checked_add(1)
                .context("Playback revision exhausted")?;
            transaction.queue().save(&state).await?;
        }
        let result = assignment(&transaction, &state, &playable).await?;
        transaction.commit().await?;
        Ok(result)
    }
}

fn validate(id: i32, playable: &HashSet<i32>) -> Result<()> {
    if !playable.contains(&id) {
        return Err(QueueError::InvalidAudioSource(id).into());
    }
    Ok(())
}

fn invalidate(state: &mut QueueState) -> Result<()> {
    state.playback_token = state
        .playback_token
        .checked_add(1)
        .context("Playback token exhausted")?;
    Ok(())
}

fn launch(state: &mut QueueState, index: usize, mode: PlaybackMode) -> Result<()> {
    state.current_index = Some(index);
    state.mode = mode;
    invalidate(state)
}

fn exhaust(state: &mut QueueState) -> Result<()> {
    let index = (!state.audio_source_ids.is_empty()).then_some(state.audio_source_ids.len());
    if state.current_index != index || state.mode != PlaybackMode::Stopped {
        state.current_index = index;
        state.mode = PlaybackMode::Stopped;
        invalidate(state)?;
    }
    Ok(())
}

fn forward(
    state: &mut QueueState,
    start: usize,
    mode: PlaybackMode,
    playable: &HashSet<i32>,
) -> Result<()> {
    if let Some(index) = state
        .audio_source_ids
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(index, id)| playable.contains(id).then_some(index))
    {
        launch(state, index, mode)
    } else {
        exhaust(state)
    }
}

fn transition_mode(state: &QueueState) -> PlaybackMode {
    if state.mode == PlaybackMode::Playing {
        PlaybackMode::Playing
    } else {
        PlaybackMode::Paused
    }
}

fn apply(state: &mut QueueState, command: PlaybackCommand, playable: &HashSet<i32>) -> Result<()> {
    match command {
        PlaybackCommand::Started { .. } => {} // History is recorded in the outer queue transaction.
        PlaybackCommand::Play { audio_source_id } => play(state, audio_source_id, playable)?,
        PlaybackCommand::PauseIfCurrent { playback_token }
            if state.playback_token != playback_token => {}
        PlaybackCommand::Pause | PlaybackCommand::PauseIfCurrent { .. } => {
            if state.mode == PlaybackMode::Playing {
                state.mode = PlaybackMode::Paused;
            }
        }
        PlaybackCommand::Stop => {
            if state.mode != PlaybackMode::Stopped {
                state.mode = PlaybackMode::Stopped;
                invalidate(state)?;
            }
        }
        PlaybackCommand::Next => {
            let mode = transition_mode(state);
            forward(
                state,
                state
                    .current_index
                    .map_or(0, |index| index.saturating_add(1)),
                mode,
                playable,
            )?;
        }
        PlaybackCommand::Previous => {
            let before = state.current_index.unwrap_or(0).saturating_sub(1);
            let previous = state
                .audio_source_ids
                .iter()
                .enumerate()
                .take(before.saturating_add(1))
                .rev()
                .find_map(|(index, id)| playable.contains(id).then_some(index));
            if let Some(index) = previous {
                launch(state, index, transition_mode(state))?;
            } else {
                let mode = transition_mode(state);
                forward(state, 0, mode, playable)?;
            }
        }
        PlaybackCommand::Finished { playback_token } => {
            if state.mode == PlaybackMode::Playing && state.playback_token == playback_token {
                forward(
                    state,
                    state
                        .current_index
                        .map_or(0, |index| index.saturating_add(1)),
                    PlaybackMode::Playing,
                    playable,
                )?;
            }
        }
        PlaybackCommand::Failed { playback_token } => {
            if state.mode != PlaybackMode::Stopped && state.playback_token == playback_token {
                let mode = state.mode;
                forward(
                    state,
                    state
                        .current_index
                        .map_or(0, |index| index.saturating_add(1)),
                    mode,
                    playable,
                )?;
            }
        }
    }
    Ok(())
}

fn play(state: &mut QueueState, selected: Option<i32>, playable: &HashSet<i32>) -> Result<()> {
    if let Some(id) = selected {
        validate(id, playable)?;
        let current = state
            .current_index
            .and_then(|index| state.audio_source_ids.get(index))
            .copied();
        let playlist_item = state
            .current_index
            .and_then(|index| state.playlist_item_ids.get(index))
            .copied()
            .flatten();
        if current != Some(id) || playlist_item.is_some() {
            let index = state.current_index.map_or(0, |index| {
                index.saturating_add(1).min(state.audio_source_ids.len())
            });
            state.audio_source_ids.insert(index, id);
            state.playlist_item_ids.insert(index, None);
            return launch(state, index, PlaybackMode::Playing);
        }
    }
    let start = state
        .current_index
        .filter(|index| *index < state.audio_source_ids.len())
        .unwrap_or(0);
    let current_valid = state
        .audio_source_ids
        .get(start)
        .is_some_and(|id| playable.contains(id));
    if current_valid && state.current_index == Some(start) && state.mode != PlaybackMode::Stopped {
        state.mode = PlaybackMode::Playing;
        return Ok(());
    }
    forward(state, start, PlaybackMode::Playing, playable)
}

async fn assignment(
    transaction: &Transaction,
    state: &QueueState,
    playable: &HashSet<i32>,
) -> Result<PlaybackAssignment> {
    let current = state
        .current_index
        .and_then(|index| state.audio_source_ids.get(index))
        .copied();
    let current_playlist_item_id = state
        .current_index
        .and_then(|index| state.playlist_item_ids.get(index))
        .copied()
        .flatten();
    let mut track = if let Some(id) = current {
        Some(track_in(transaction, id, current_playlist_item_id).await?)
    } else {
        None
    };
    if let Some(track) = &mut track {
        ListeningHistoryService::fill_tracks(transaction, std::slice::from_mut(track)).await?;
        crate::AudioSettingsService::fill_tracks(transaction, std::slice::from_mut(track)).await?;
    }
    let next = state
        .current_index
        .map_or(0, |index| index.saturating_add(1));
    let previous = state.current_index.unwrap_or(0).saturating_add(1);
    Ok(PlaybackAssignment {
        volume_percent: track.as_ref().and_then(|track| track.volume_percent),
        current_audio_source_id: current,
        current_playlist_item_id,
        track,
        mode: state.mode,
        revision: state.revision,
        playback_token: state.playback_token,
        can_next: state
            .audio_source_ids
            .iter()
            .skip(next)
            .any(|id| playable.contains(id)),
        can_previous: state
            .audio_source_ids
            .iter()
            .take(previous)
            .any(|id| playable.contains(id)),
    })
}

async fn track_in(
    transaction: &Transaction,
    id: i32,
    playlist_item_id: Option<i32>,
) -> Result<LibraryTrack> {
    let playlist_item = if let Some(id) = playlist_item_id {
        transaction.playlists().item(id).await?
    } else {
        None
    };
    Ok(if let Some(item) = playlist_item {
        LibraryTrack {
            audio_source_id: id,
            last_played_at_ms: None,
            volume_percent: None,
            title: item.title,
            title_unicode: None,
            artist: item.artist,
            artist_unicode: None,
            cover_beatmap_id: item.cover_beatmap_id,
            difficulties: item
                .beatmap_id
                .zip(item.beatmap_set_id)
                .map(|(beatmap_id, beatmap_set_id)| crate::TrackDifficulty {
                    beatmap_id,
                    beatmap_set_id,
                    difficulty_name: item.difficulty_name,
                    set_has_multiple_audio_sources: true,
                })
                .into_iter()
                .collect(),
        }
    } else {
        crate::beatmap_set::tracks::from_sets(
            transaction.beatmap_sets().for_audio_source(id).await?,
        )
        .into_iter()
        .next()
        .unwrap_or(LibraryTrack {
            audio_source_id: id,
            last_played_at_ms: None,
            volume_percent: None,
            title: None,
            title_unicode: None,
            artist: None,
            artist_unicode: None,
            cover_beatmap_id: None,
            difficulties: Vec::new(),
        })
    })
}
