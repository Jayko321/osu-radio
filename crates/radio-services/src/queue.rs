use anyhow::{Context, Result};
use radio_db::{Database, Transaction};
use std::{collections::HashSet, error::Error, fmt};

use crate::LibraryTrack;
pub use radio_db::model::{PlaybackMode, QueueState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackAssignment {
    pub current_audio_source_id: Option<i32>,
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
    pub async fn get(&self) -> Result<QueueState> {
        self.database.queue().get().await
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
        match change {
            Change::Append(incoming) => {
                for id in &incoming {
                    validate(*id, &playable)?;
                }
                if !incoming.is_empty() {
                    let old_len = state.audio_source_ids.len();
                    let exhausted = state.current_index.is_none_or(|index| index >= old_len);
                    state.audio_source_ids.extend(incoming);
                    if exhausted {
                        launch(&mut state, old_len, PlaybackMode::Playing)?;
                    }
                }
            }
            Change::Clear => {
                if !state.audio_source_ids.is_empty() {
                    state.audio_source_ids.clear();
                    state.current_index = None;
                    state.mode = PlaybackMode::Stopped;
                    invalidate(&mut state)?;
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
        if state != original {
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
        if current != Some(id) {
            let index = state.current_index.map_or(0, |index| {
                index.saturating_add(1).min(state.audio_source_ids.len())
            });
            state.audio_source_ids.insert(index, id);
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
    let track = if let Some(id) = current {
        Some(
            crate::beatmap_set::tracks::from_sets(
                transaction.beatmap_sets().for_audio_source(id).await?,
            )
            .into_iter()
            .next()
            .unwrap_or(LibraryTrack {
                audio_source_id: id,
                title: None,
                title_unicode: None,
                artist: None,
                artist_unicode: None,
                cover_beatmap_id: None,
                difficulties: Vec::new(),
            }),
        )
    } else {
        None
    };
    let next = state
        .current_index
        .map_or(0, |index| index.saturating_add(1));
    let previous = state.current_index.unwrap_or(0).saturating_add(1);
    Ok(PlaybackAssignment {
        current_audio_source_id: current,
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
