use anyhow::{Context, Result, ensure};
use radio_db::{Database, Transaction};
use std::{collections::HashSet, error::Error, fmt};

use crate::{LibraryTrack, ListeningHistoryService, PlaylistItem};
pub use radio_db::model::{
    PlaybackMode, QueueAvailability, QueueEntry, QueueEntryId, QueueLaunchSource,
    QueuePlaylistSource, QueueState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueSnapshotEntry {
    pub entry: QueueEntry,
    pub last_played_at_ms: Option<i64>,
    pub volume_percent: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueSnapshot {
    pub current_entry_id: Option<QueueEntryId>,
    pub history_len: usize,
    pub mode: PlaybackMode,
    pub waiting_for_check: bool,
    pub revision: u64,
    pub playback_token: u64,
    pub last_entry_id: i64,
    pub entries: Vec<QueueSnapshotEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueItemRef {
    Audio(i32),
    PlaylistItem { playlist_id: i32, item_id: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuePlacement {
    End,
    Next,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueCheck {
    pub entry_id: QueueEntryId,
    pub generation: u64,
    pub audio_source_id: i32,
    pub source: radio_db::model::SourceType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackAssignment {
    pub current_entry_id: Option<QueueEntryId>,
    pub availability: Option<QueueAvailability>,
    pub waiting_for_check: bool,
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
    InvalidPlaylistItem { playlist_id: i32, item_id: i32 },
    UnknownEntry(QueueEntryId),
    UnavailableEntry(QueueEntryId),
}
impl fmt::Display for QueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAudioSource(id) => write!(
                f,
                "Audio source {id} is missing or unsupported for playback"
            ),
            Self::InvalidPlaylistItem {
                playlist_id,
                item_id,
            } => write!(
                f,
                "Item {item_id} does not belong to playlist {playlist_id}"
            ),
            Self::UnknownEntry(id) => write!(f, "Queue entry {id} does not exist"),
            Self::UnavailableEntry(id) => write!(f, "Queue entry {id} is unavailable"),
        }
    }
}
impl Error for QueueError {}

pub struct QueueService<'a> {
    pub(crate) database: &'a Database,
}

enum Change {
    Insert(Vec<QueueItemRef>, QueuePlacement),
    PlayEntry(QueueEntryId),
    Move(QueueEntryId, Option<QueueEntryId>),
    Remove(QueueEntryId),
    ClearUpcoming,
    Clear,
    Check(QueueCheck, bool),
    Command(PlaybackCommand),
    Recover,
}

impl QueueService<'_> {
    /// Caller owns the library transaction and singleton writer lock; never commits here.
    pub(crate) async fn reconcile_sources_in(transaction: &Transaction) -> Result<()> {
        let mut state = transaction.queue().get().await?;
        let original = state.clone();
        let keys = state
            .entries
            .iter()
            .filter_map(|e| e.playlist_source.clone())
            .collect::<Vec<_>>();
        let resolved = transaction.playlists().resolve_sources(&keys).await?;
        for entry in &mut state.entries {
            let (audio_id, source, beatmap_id, set_id, cover_id) =
                if let Some(key) = &entry.playlist_source {
                    if let Some(map) = resolved.get(key) {
                        let audio = transaction
                            .audio_sources()
                            .get(map.audio_source_id.context("Resolved source missing")?)
                            .await?
                            .context("Resolved source disappeared")?;
                        (
                            Some(audio.id),
                            Some(audio.s_type),
                            Some(map.beatmap_id),
                            Some(map.beatmap_set_id),
                            map.cover_beatmap_id,
                        )
                    } else {
                        (None, None, None, None, None)
                    }
                } else if let Some(source) = &entry.audio_source {
                    if let Some(audio) = transaction
                        .audio_sources()
                        .find(source)
                        .await?
                        .filter(|a| !matches!(a.s_type, radio_db::model::SourceType::Online(_)))
                    {
                        let track = track_in(transaction, audio.id).await?;
                        let map = track.difficulties.first();
                        (
                            Some(audio.id),
                            Some(audio.s_type),
                            map.map(|m| m.beatmap_id),
                            map.map(|m| m.beatmap_set_id),
                            track.cover_beatmap_id,
                        )
                    } else {
                        (None, None, None, None, None)
                    }
                } else {
                    // Numeric migration evidence cannot recover a lost source identity.
                    (None, None, None, None, None)
                };
            if entry.audio_source_id != audio_id
                || source
                    .as_ref()
                    .is_some_and(|s| Some(s) != entry.audio_source.as_ref())
            {
                entry.check_generation = entry
                    .check_generation
                    .checked_add(1)
                    .context("Queue check generation exhausted")?;
                entry.availability = if audio_id.is_some() {
                    QueueAvailability::Checking
                } else {
                    QueueAvailability::Unavailable
                };
            } else if audio_id.is_none() {
                entry.availability = QueueAvailability::Unavailable;
            }
            entry.audio_source_id = audio_id;
            if source.is_some() {
                entry.audio_source = source;
            }
            entry.beatmap_id = beatmap_id;
            entry.beatmap_set_id = set_id;
            entry.cover_beatmap_id = cover_id;
        }
        if state.waiting_for_check
            && state
                .current_entry()
                .is_some_and(|e| e.availability == QueueAvailability::Unavailable)
        {
            let start = next_index(&state);
            let mode = state.mode;
            let playable = transaction
                .audio_sources()
                .playable_ids(&state.audio_source_ids())
                .await?;
            state.waiting_for_check = false;
            forward(&mut state, start, mode, &playable)?;
        }
        if state != original {
            trim_history(&mut state);
            bump_revision(&mut state)?;
            transaction.queue().save(&state).await?;
        }
        Ok(())
    }

    pub(crate) async fn append_playlist_in(
        transaction: &Transaction,
        items: Vec<PlaylistItem>,
    ) -> Result<PlaybackAssignment> {
        let mut state = transaction.queue().get().await?;
        if !items.is_empty() {
            for item in items {
                let entry = entry_for_playlist_item(transaction, &mut state, item).await?;
                state.entries.push(entry);
            }
            state.reindex();
            trim_history(&mut state);
            bump_revision(&mut state)?;
            transaction.queue().save(&state).await?;
        }
        let playable = transaction
            .audio_sources()
            .playable_ids(&state.audio_source_ids())
            .await?;
        assignment(transaction, &state, &playable).await
    }

    pub(crate) async fn replace_in(
        transaction: &Transaction,
        items: Vec<PlaylistItem>,
        start: Option<usize>,
    ) -> Result<PlaybackAssignment> {
        ensure!(!items.is_empty(), "Playlist queue is empty");
        ensure!(
            start.is_none_or(|i| i < items.len()),
            "Playlist queue start is out of bounds"
        );
        let mut state = transaction.queue().get().await?;
        let boundary = next_index(&state);
        state.entries.truncate(boundary);
        state.current_entry_id = None;
        state.waiting_for_check = false;
        state.history_len = boundary;
        for item in items {
            let entry = entry_for_playlist_item(transaction, &mut state, item).await?;
            state.entries.push(entry);
        }
        state.reindex();
        if let Some(start) = start {
            launch(
                &mut state,
                boundary
                    .checked_add(start)
                    .context("Queue position exhausted")?,
                PlaybackMode::Playing,
            )?;
        } else {
            state.mode = PlaybackMode::Stopped;
            invalidate(&mut state)?;
        }
        trim_history(&mut state);
        bump_revision(&mut state)?;
        let playable = transaction
            .audio_sources()
            .playable_ids(&state.audio_source_ids())
            .await?;
        transaction.queue().save(&state).await?;
        assignment(transaction, &state, &playable).await
    }

    pub async fn get(&self) -> Result<QueueState> {
        self.database.queue().get().await
    }

    pub async fn snapshot(&self) -> Result<QueueSnapshot> {
        let transaction = self.database.begin_read().await?;
        let state = transaction.queue().get().await?;
        let sources = state
            .entries
            .iter()
            .filter_map(|e| e.audio_source.clone())
            .collect::<Vec<_>>();
        let dates = ListeningHistoryService::for_sources_in(&transaction, &sources).await?;
        let volumes = crate::AudioSettingsService::for_sources_in(&transaction, &sources).await?;
        let entries = state
            .entries
            .into_iter()
            .map(|entry| {
                let key = entry
                    .audio_source
                    .as_ref()
                    .map(|s| (s.kind().to_owned(), s.location().to_owned()));
                QueueSnapshotEntry {
                    entry,
                    last_played_at_ms: key.as_ref().and_then(|key| dates.get(key).copied()),
                    volume_percent: key.as_ref().and_then(|key| volumes.get(key).copied()),
                }
            })
            .collect();
        transaction.commit().await?;
        Ok(QueueSnapshot {
            current_entry_id: state.current_entry_id,
            history_len: state.history_len,
            mode: state.mode,
            waiting_for_check: state.waiting_for_check,
            revision: state.revision,
            playback_token: state.playback_token,
            last_entry_id: state.last_entry_id,
            entries,
        })
    }

    /// Legacy upcoming projection; the full, unfiltered list is available through `snapshot()`.
    pub async fn upcoming(&self) -> Result<(QueueState, Vec<LibraryTrack>)> {
        let transaction = self.database.begin_read().await?;
        let state = transaction.queue().get().await?;
        let playable = transaction
            .audio_sources()
            .playable_ids(&state.audio_source_ids())
            .await?;
        let mut tracks = Vec::new();
        for entry in state.entries.iter().skip(next_index(&state)) {
            if entry_playable(entry, &playable)
                && let Some(id) = entry.audio_source_id
            {
                tracks.push(track_for_entry(&transaction, entry, id).await?);
            }
        }
        transaction.commit().await?;
        Ok((state, tracks))
    }

    pub async fn playback(&self) -> Result<PlaybackAssignment> {
        let transaction = self.database.begin_read().await?;
        let state = transaction.queue().get().await?;
        let playable = transaction
            .audio_sources()
            .playable_ids(&state.audio_source_ids())
            .await?;
        let result = assignment(&transaction, &state, &playable).await?;
        transaction.commit().await?;
        Ok(result)
    }

    /// Persisted unfinished checks, with the current waiting launch first.
    pub async fn pending_checks(&self) -> Result<Vec<QueueCheck>> {
        let state = self.get().await?;
        let mut checks = state
            .entries
            .iter()
            .filter(|e| e.availability == QueueAvailability::Checking)
            .filter_map(|e| {
                Some(QueueCheck {
                    entry_id: e.id,
                    generation: e.check_generation,
                    audio_source_id: e.audio_source_id?,
                    source: e.audio_source.clone()?,
                })
            })
            .collect::<Vec<_>>();
        if state.waiting_for_check {
            checks.sort_by_key(|c| Some(c.entry_id) != state.current_entry_id);
        }
        Ok(checks)
    }

    pub async fn apply_check(
        &self,
        check: QueueCheck,
        available: bool,
    ) -> Result<PlaybackAssignment> {
        self.change(Change::Check(check, available)).await
    }
    pub async fn insert(
        &self,
        items: Vec<QueueItemRef>,
        placement: QueuePlacement,
    ) -> Result<PlaybackAssignment> {
        self.change(Change::Insert(items, placement)).await
    }
    pub async fn append(&self, ids: Vec<i32>) -> Result<PlaybackAssignment> {
        self.insert(
            ids.into_iter().map(QueueItemRef::Audio).collect(),
            QueuePlacement::End,
        )
        .await
    }
    pub async fn play_entry(&self, id: QueueEntryId) -> Result<PlaybackAssignment> {
        self.change(Change::PlayEntry(id)).await
    }
    pub async fn move_entry(
        &self,
        id: QueueEntryId,
        before_id: Option<QueueEntryId>,
    ) -> Result<PlaybackAssignment> {
        self.change(Change::Move(id, before_id)).await
    }
    pub async fn remove_entry(&self, id: QueueEntryId) -> Result<PlaybackAssignment> {
        self.change(Change::Remove(id)).await
    }
    pub async fn clear_upcoming(&self) -> Result<PlaybackAssignment> {
        self.change(Change::ClearUpcoming).await
    }
    pub async fn clear(&self) -> Result<PlaybackAssignment> {
        self.change(Change::Clear).await
    }
    pub async fn command(&self, command: PlaybackCommand) -> Result<PlaybackAssignment> {
        self.change(Change::Command(command)).await
    }
    /// Startup retains the list, pauses the selection and invalidates old process callbacks.
    pub async fn recover(&self) -> Result<()> {
        self.change(Change::Recover).await?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)] // All mutations share one writer lock, rollback and outer commit.
    async fn change(&self, change: Change) -> Result<PlaybackAssignment> {
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        let mut state = transaction.queue().get().await?;
        let original = state.clone();
        let recovering = matches!(change, Change::Recover);
        let mut history_changed = false;
        let mut ids = state.audio_source_ids();
        if let Change::Command(PlaybackCommand::Play {
            audio_source_id: Some(id),
        }) = &change
        {
            ids.push(*id);
        }
        let mut playable = transaction.audio_sources().playable_ids(&ids).await?;
        match change {
            Change::Insert(items, placement) => {
                let index = match placement {
                    QueuePlacement::End => state.entries.len(),
                    QueuePlacement::Next => next_index(&state),
                };
                let mut entries = Vec::with_capacity(items.len());
                for item in items {
                    entries.push(entry_for_ref(&transaction, &mut state, item).await?);
                }
                state.entries.splice(index..index, entries);
                state.reindex();
                playable = transaction
                    .audio_sources()
                    .playable_ids(&state.audio_source_ids())
                    .await?;
            }
            Change::PlayEntry(id) => {
                let index = entry_index(&state, id)?;
                if !entry_playable(
                    state.entries.get(index).context("Queue position missing")?,
                    &playable,
                ) {
                    return Err(QueueError::UnavailableEntry(id).into());
                }
                if state.current_entry_id == Some(id) && state.mode != PlaybackMode::Stopped {
                    state.mode = PlaybackMode::Playing;
                } else {
                    launch(&mut state, index, PlaybackMode::Playing)?;
                }
            }
            Change::Move(id, before) => {
                let from = entry_index(&state, id)?;
                let to = before
                    .map(|id| entry_index(&state, id))
                    .transpose()?
                    .unwrap_or(state.entries.len());
                if from != to && from.saturating_add(1) != to {
                    let entry = state.entries.remove(from);
                    state
                        .entries
                        .insert(if from < to { to.saturating_sub(1) } else { to }, entry);
                    state.reindex();
                }
            }
            Change::Remove(id) => {
                let index = entry_index(&state, id)?;
                let current = state.current_entry_id == Some(id);
                state.entries.remove(index);
                if current {
                    let mode = transition_mode(&state);
                    state.waiting_for_check = false;
                    state.reindex();
                    // Invalidate even when removing the last current entry.
                    forward(&mut state, index, mode, &playable)?;
                } else {
                    if index < state.history_len {
                        state.history_len = state.history_len.saturating_sub(1);
                    }
                    state.reindex();
                }
            }
            Change::ClearUpcoming => {
                state.entries.truncate(next_index(&state));
            }
            Change::Clear => {
                if !state.entries.is_empty() {
                    state.entries.clear();
                    state.current_entry_id = None;
                    state.history_len = 0;
                    state.mode = PlaybackMode::Stopped;
                    state.waiting_for_check = false;
                    invalidate(&mut state)?;
                }
            }
            Change::Check(check, available) => {
                if let Some(index) = state.entries.iter().position(|e| {
                    e.id == check.entry_id
                        && e.check_generation == check.generation
                        && e.audio_source_id == Some(check.audio_source_id)
                        && e.audio_source.as_ref() == Some(&check.source)
                        && e.availability == QueueAvailability::Checking
                }) {
                    state
                        .entries
                        .get_mut(index)
                        .context("Queue position missing")?
                        .availability = if available {
                        QueueAvailability::Available
                    } else {
                        QueueAvailability::Unavailable
                    };
                    if state.current_entry_id == Some(check.entry_id) && state.waiting_for_check {
                        state.waiting_for_check = false;
                        if available {
                            bind_launch_source(&mut state);
                        } else {
                            let mode = state.mode;
                            forward(&mut state, index.saturating_add(1), mode, &playable)?;
                        }
                    }
                }
            }
            Change::Command(PlaybackCommand::Started { playback_token }) => {
                if state.playback_token == playback_token
                    && state.mode != PlaybackMode::Stopped
                    && !state.waiting_for_check
                    && let Some(launch) = &state.launch_source
                {
                    history_changed = ListeningHistoryService::record_started(
                        &transaction,
                        &launch.source,
                        playback_token,
                    )
                    .await?;
                }
            }
            Change::Command(PlaybackCommand::Play {
                audio_source_id: Some(id),
            }) => {
                validate(id, &playable)?;
                if state
                    .current_entry()
                    .is_some_and(|e| e.audio_source_id == Some(id) && e.playlist_source.is_none())
                {
                    play(&mut state, &playable)?;
                } else {
                    let index = next_index(&state);
                    let entry = new_entry(&transaction, &mut state, id).await?;
                    state.entries.insert(index, entry);
                    state.reindex();
                    launch(&mut state, index, PlaybackMode::Playing)?;
                }
            }
            Change::Command(command) => apply(&mut state, command, &playable)?,
            Change::Recover => {
                if state.current_entry_id.is_some() {
                    state.mode = PlaybackMode::Paused;
                    state.waiting_for_check = state
                        .current_entry()
                        .is_some_and(|e| e.availability == QueueAvailability::Checking);
                    invalidate(&mut state)?;
                    bind_launch_source(&mut state);
                }
            }
        }
        if state != original || history_changed || recovering {
            trim_history(&mut state);
        }
        if state != original || history_changed {
            bump_revision(&mut state)?;
            transaction.queue().save(&state).await?;
        }
        let result = assignment(&transaction, &state, &playable).await?;
        transaction.commit().await?;
        Ok(result)
    }
}

fn entry_index(state: &QueueState, id: QueueEntryId) -> Result<usize> {
    state
        .entries
        .iter()
        .position(|e| e.id == id)
        .ok_or_else(|| QueueError::UnknownEntry(id).into())
}
fn validate(id: i32, playable: &HashSet<i32>) -> Result<()> {
    if !playable.contains(&id) {
        return Err(QueueError::InvalidAudioSource(id).into());
    }
    Ok(())
}
fn bump_revision(state: &mut QueueState) -> Result<()> {
    state.revision = state
        .revision
        .checked_add(1)
        .context("Playback revision exhausted")?;
    Ok(())
}
fn invalidate(state: &mut QueueState) -> Result<()> {
    state.launch_source = None;
    state.playback_token = state
        .playback_token
        .checked_add(1)
        .context("Playback token exhausted")?;
    Ok(())
}
fn trim_history(state: &mut QueueState) {
    let excess = state.history_len.saturating_sub(5000);
    if excess > 0 {
        state.entries.drain(..excess);
        state.history_len = state.history_len.saturating_sub(excess);
        state.reindex();
    }
}
fn launch(state: &mut QueueState, index: usize, mode: PlaybackMode) -> Result<()> {
    let entry = state.entries.get(index).context("Queue position missing")?;
    state.current_entry_id = Some(entry.id);
    state.history_len = index;
    state.mode = mode;
    state.waiting_for_check = entry.availability == QueueAvailability::Checking;
    invalidate(state)?;
    bind_launch_source(state);
    Ok(())
}
fn bind_launch_source(state: &mut QueueState) {
    state.launch_source = state
        .current_entry()
        .filter(|e| {
            !state.waiting_for_check
                && state.mode != PlaybackMode::Stopped
                && e.availability == QueueAvailability::Available
        })
        .and_then(|e| {
            Some(QueueLaunchSource {
                audio_source_id: e.audio_source_id?,
                source: e.audio_source.clone()?,
                check_generation: e.check_generation,
            })
        });
}
fn exhaust(state: &mut QueueState) -> Result<()> {
    if state.current_entry_id.is_some()
        || state.history_len != state.entries.len()
        || state.mode != PlaybackMode::Stopped
    {
        state.current_entry_id = None;
        state.history_len = state.entries.len();
        state.mode = PlaybackMode::Stopped;
        state.waiting_for_check = false;
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
        .entries
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(i, e)| entry_playable(e, playable).then_some(i))
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
        PlaybackCommand::Started { .. } => {}
        PlaybackCommand::Play { .. } => play(state, playable)?,
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
                state.waiting_for_check = false;
                invalidate(state)?;
            }
        }
        PlaybackCommand::Next => {
            forward(state, next_index(state), transition_mode(state), playable)?;
        }
        PlaybackCommand::Previous => {
            let before = state.current_index().unwrap_or(state.history_len);
            let previous = state
                .entries
                .iter()
                .enumerate()
                .take(before)
                .rev()
                .find_map(|(i, e)| entry_playable(e, playable).then_some(i));
            if let Some(index) = previous {
                launch(state, index, transition_mode(state))?;
            } else {
                forward(state, 0, transition_mode(state), playable)?;
            }
        }
        PlaybackCommand::Finished { playback_token } => {
            if state.mode == PlaybackMode::Playing
                && !state.waiting_for_check
                && state.playback_token == playback_token
            {
                forward(state, next_index(state), PlaybackMode::Playing, playable)?;
            }
        }
        PlaybackCommand::Failed { playback_token } => {
            if state.mode != PlaybackMode::Stopped
                && state.playback_token == playback_token
                && let Some(index) = state.current_index()
            {
                let entry = state.entries.get(index).context("Queue position missing")?;
                let matches_launch = state.launch_source.as_ref().is_some_and(|launch| {
                    entry.audio_source_id == Some(launch.audio_source_id)
                        && entry.audio_source.as_ref() == Some(&launch.source)
                        && entry.check_generation == launch.check_generation
                });
                if matches_launch {
                    state
                        .entries
                        .get_mut(index)
                        .context("Queue position missing")?
                        .availability = QueueAvailability::Unavailable;
                }
                forward(state, index.saturating_add(1), state.mode, playable)?;
            }
        }
    }
    Ok(())
}
fn next_index(state: &QueueState) -> usize {
    state
        .current_index()
        .map_or(state.history_len, |i| i.saturating_add(1))
}
fn entry_playable(entry: &QueueEntry, playable: &HashSet<i32>) -> bool {
    entry.availability != QueueAvailability::Unavailable
        && entry
            .audio_source_id
            .is_some_and(|id| playable.contains(&id))
}
fn play(state: &mut QueueState, playable: &HashSet<i32>) -> Result<()> {
    if let Some(index) = state.current_index() {
        if state.mode != PlaybackMode::Stopped {
            state.mode = PlaybackMode::Playing;
            return Ok(());
        }
        return forward(state, index, PlaybackMode::Playing, playable);
    }
    let start = if state.history_len < state.entries.len() {
        state.history_len
    } else {
        0
    };
    forward(state, start, PlaybackMode::Playing, playable)
}
async fn assignment(
    transaction: &Transaction,
    state: &QueueState,
    playable: &HashSet<i32>,
) -> Result<PlaybackAssignment> {
    let entry = state.current_entry();
    let current = state
        .launch_source
        .as_ref()
        .map(|s| s.audio_source_id)
        .or_else(|| entry.and_then(|e| e.audio_source_id.or(e.migration_audio_source_id)));
    let mut track = if let Some((entry, id)) = entry.zip(current) {
        Some(track_for_entry(transaction, entry, id).await?)
    } else {
        None
    };
    if let Some(track) = &mut track
        && let Some(source) = state
            .launch_source
            .as_ref()
            .map(|s| &s.source)
            .or_else(|| entry.and_then(|e| e.audio_source.as_ref()))
    {
        let key = (source.kind().to_owned(), source.location().to_owned());
        track.last_played_at_ms =
            ListeningHistoryService::for_sources_in(transaction, std::slice::from_ref(source))
                .await?
                .get(&key)
                .copied();
        track.volume_percent =
            crate::AudioSettingsService::for_sources_in(transaction, std::slice::from_ref(source))
                .await?
                .get(&key)
                .copied();
    }
    Ok(PlaybackAssignment {
        current_entry_id: state.current_entry_id,
        availability: entry.map(|e| e.availability),
        waiting_for_check: state.waiting_for_check,
        volume_percent: track.as_ref().and_then(|t| t.volume_percent),
        current_audio_source_id: current,
        current_playlist_item_id: entry
            .and_then(|e| e.playlist_item_id.or(e.migration_playlist_item_id)),
        track,
        mode: state.mode,
        revision: state.revision,
        playback_token: state.playback_token,
        can_next: state
            .entries
            .iter()
            .skip(next_index(state))
            .any(|e| entry_playable(e, playable)),
        can_previous: state
            .entries
            .iter()
            .take(
                state
                    .current_index()
                    .map_or(state.history_len, |i| i.saturating_add(1)),
            )
            .any(|e| entry_playable(e, playable)),
    })
}

async fn entry_for_ref(
    transaction: &Transaction,
    state: &mut QueueState,
    item: QueueItemRef,
) -> Result<QueueEntry> {
    match item {
        QueueItemRef::Audio(id) => {
            validate(id, &transaction.audio_sources().playable_ids(&[id]).await?)?;
            new_entry(transaction, state, id).await
        }
        QueueItemRef::PlaylistItem {
            playlist_id,
            item_id,
        } => {
            let item = transaction
                .playlists()
                .item(item_id)
                .await?
                .filter(|item| item.playlist_id == playlist_id)
                .ok_or(QueueError::InvalidPlaylistItem {
                    playlist_id,
                    item_id,
                })?;
            entry_for_playlist_item(transaction, state, item).await
        }
    }
}

async fn entry_for_playlist_item(
    transaction: &Transaction,
    state: &mut QueueState,
    item: PlaylistItem,
) -> Result<QueueEntry> {
    let source = if let Some(id) = item.audio_source_id {
        Some(
            transaction
                .audio_sources()
                .get(id)
                .await?
                .context("Queue source disappeared")?
                .s_type,
        )
    } else {
        None
    };
    Ok(QueueEntry {
        id: state.allocate_id()?,
        position: state.entries.len(),
        audio_source: source,
        playlist_source: Some(QueuePlaylistSource {
            installation_kind: item.source_kind,
            hash_kind: item.hash_kind,
            beatmap_hash: item.beatmap_hash,
        }),
        audio_source_id: item.audio_source_id,
        playlist_item_id: Some(item.id),
        migration_audio_source_id: None,
        migration_playlist_item_id: None,
        title: item.title,
        title_unicode: item.title_unicode,
        artist: item.artist,
        artist_unicode: item.artist_unicode,
        difficulty_name: item.difficulty_name,
        beatmap_id: item.beatmap_id,
        beatmap_set_id: item.beatmap_set_id,
        cover_beatmap_id: item.cover_beatmap_id,
        availability: if item.audio_source_id.is_some() {
            QueueAvailability::Checking
        } else {
            QueueAvailability::Unavailable
        },
        check_generation: 0,
    })
}

async fn track_in(transaction: &Transaction, id: i32) -> Result<LibraryTrack> {
    Ok(crate::beatmap_set::tracks::from_sets(
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
    }))
}

async fn new_entry(
    transaction: &Transaction,
    state: &mut QueueState,
    audio_id: i32,
) -> Result<QueueEntry> {
    let source = transaction
        .audio_sources()
        .get(audio_id)
        .await?
        .context("Queue source disappeared")?;
    let track = track_in(transaction, audio_id).await?;
    let difficulty = track.difficulties.first();
    Ok(QueueEntry {
        id: state.allocate_id()?,
        position: state.entries.len(),
        audio_source: Some(source.s_type.clone()),
        playlist_source: None,
        audio_source_id: Some(audio_id),
        playlist_item_id: None,
        migration_audio_source_id: None,
        migration_playlist_item_id: None,
        title: track.title,
        title_unicode: track.title_unicode,
        artist: track.artist,
        artist_unicode: track.artist_unicode,
        difficulty_name: difficulty.and_then(|d| d.difficulty_name.clone()),
        beatmap_id: difficulty.map(|d| d.beatmap_id),
        beatmap_set_id: difficulty.map(|d| d.beatmap_set_id),
        cover_beatmap_id: track.cover_beatmap_id,
        availability: if matches!(source.s_type, radio_db::model::SourceType::Online(_)) {
            QueueAvailability::Unavailable
        } else {
            QueueAvailability::Checking
        },
        check_generation: 0,
    })
}
fn track_from_entry(entry: &QueueEntry, audio_source_id: i32) -> LibraryTrack {
    LibraryTrack {
        audio_source_id,
        last_played_at_ms: None,
        volume_percent: None,
        title: entry.title.clone(),
        title_unicode: entry.title_unicode.clone(),
        artist: entry.artist.clone(),
        artist_unicode: entry.artist_unicode.clone(),
        cover_beatmap_id: entry.cover_beatmap_id,
        difficulties: entry
            .beatmap_id
            .zip(entry.beatmap_set_id)
            .map(|(beatmap_id, beatmap_set_id)| crate::TrackDifficulty {
                beatmap_id,
                beatmap_set_id,
                difficulty_name: entry.difficulty_name.clone(),
                set_has_multiple_audio_sources: true,
            })
            .into_iter()
            .collect(),
    }
}

async fn track_for_entry(
    transaction: &Transaction,
    entry: &QueueEntry,
    id: i32,
) -> Result<LibraryTrack> {
    let mut track = if entry.playlist_source.is_some() {
        track_from_entry(entry, id)
    } else {
        let library = track_in(transaction, id).await?;
        if library.difficulties.is_empty() {
            track_from_entry(entry, id)
        } else {
            library
        }
    };
    track.title.clone_from(&entry.title);
    track.title_unicode.clone_from(&entry.title_unicode);
    track.artist.clone_from(&entry.artist);
    track.artist_unicode.clone_from(&entry.artist_unicode);
    Ok(track)
}
