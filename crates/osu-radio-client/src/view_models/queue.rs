use super::Track;
use crate::controller::workflow::{Effect, Workflow};
use crate::controller::{AppUpdate, Completed};
use crate::describe;
#[cfg(test)]
use crate::models::{PlaybackAssignment, PlaybackMode};
use crate::models::{
    PlaybackCommand, QueueAvailability, QueueEntry, QueueEntryId, QueueItemRef, QueuePlacement,
};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueRow {
    pub id: QueueEntryId,
    pub position: usize,
    pub audio_source_id: Option<i32>,
    pub playlist_item_id: Option<i32>,
    pub availability: QueueAvailability,
    pub track: Track,
}
impl From<QueueEntry> for QueueRow {
    fn from(entry: QueueEntry) -> Self {
        let track = Track::from(crate::models::LibraryTrack {
            audio_source_id: entry.audio_source_id.unwrap_or(-1),
            title: entry.title,
            title_unicode: entry.title_unicode,
            artist: entry.artist,
            artist_unicode: entry.artist_unicode,
            cover_beatmap_id: entry.cover_beatmap_id,
            last_played_at_ms: entry.last_played_at_ms,
            volume_percent: entry.volume_percent,
            mods: entry.mods,
            difficulties: entry
                .beatmap_id
                .zip(entry.beatmap_set_id)
                .map(
                    |(beatmap_id, beatmap_set_id)| crate::models::TrackDifficulty {
                        beatmap_id,
                        beatmap_set_id,
                        beatmap_set_online_id: None,
                        difficulty_name: entry.difficulty_name.clone(),
                        set_has_multiple_audio_sources: false,
                    },
                )
                .into_iter()
                .collect(),
        });
        let mut track = track;
        if let Some(name) = entry.difficulty_name.filter(|name| !name.trim().is_empty()) {
            track.subtitle_suffix = format!(" | {name}");
            track.apply_name_preferences(crate::TrackNamePreferences::default());
        }
        Self {
            id: entry.id,
            position: entry.position,
            audio_source_id: entry.audio_source_id,
            playlist_item_id: entry.playlist_item_id,
            availability: entry.availability,
            track,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueueAction {
    Refresh,
    Play(QueueEntryId),
    Remove(QueueEntryId),
    Move {
        id: QueueEntryId,
        before_entry_id: Option<QueueEntryId>,
    },
    ClearUpcoming,
    ClearAll,
    Insert {
        items: Vec<QueueItemRef>,
        placement: QueuePlacement,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueueView {
    pub entries: Vec<QueueRow>,
    pub current_entry_id: Option<QueueEntryId>,
    pub history_len: usize,
    pub mode: crate::models::PlaybackMode,
    pub waiting_for_check: bool,
    pub revision: u64,
    pub playback_token: u64,
    pub loading: bool,
    pub busy: bool,
    pub message: String,
    pub operation_error: Option<String>,
}
impl Default for QueueView {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            current_entry_id: None,
            history_len: 0,
            mode: crate::models::PlaybackMode::Stopped,
            waiting_for_check: false,
            revision: 0,
            playback_token: 0,
            loading: false,
            busy: false,
            message: String::new(),
            operation_error: None,
        }
    }
}
impl QueueView {
    /// Compatibility projection for the pre-T08 panel. Never feed history to its Repeater.
    pub fn upcoming(&self) -> impl Iterator<Item = &QueueRow> {
        self.entries.iter().skip(
            self.history_len
                .saturating_add(usize::from(self.current_entry_id.is_some())),
        )
    }
}

#[derive(Default)]
pub struct QueueViewModel {
    pub(crate) view: QueueView,
    pub(crate) open: bool,
    pub(crate) request: u64,
    pub(crate) task: Option<tokio::task::AbortHandle>,
}

#[derive(Clone, Copy)]
pub(crate) enum PlaylistPlayback {
    Play(i32, Option<i32>),
    Append(i32),
}

#[derive(Clone)]
pub(crate) struct PendingPlayback {
    pub(crate) command: PlaybackCommand,
    pub(crate) playlist: Option<PlaylistPlayback>,
    pub(crate) queue: Option<QueueAction>,
    pub(crate) expected_token: Option<u64>,
}

#[cfg(test)]
mod tests;

impl QueueViewModel {
    pub(crate) fn emit_queue(&self, _input: &QueueInput<'_>, work: &mut Workflow<'_>) {
        work.emit(AppUpdate::Queue(self.view.clone()));
    }
    pub(crate) fn set_queue_visible(
        &mut self,
        visible: bool,
        input: &QueueInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.open = visible;
        if visible {
            self.refresh_queue(input, work);
        } else {
            self.view.loading = false;
            work.effect(Effect::QueueClosed);
            self.request = self.request.wrapping_add(1);
            if let Some(task) = self.task.take() {
                task.abort();
            }
            work.effect(Effect::ReconcileMedia);
        }
    }
    pub(crate) fn refresh_queue(&mut self, input: &QueueInput<'_>, work: &mut Workflow<'_>) {
        if !self.open {
            return;
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.request = self.request.wrapping_add(1);
        let request = self.request;
        let Some(api) = work.api.clone() else {
            self.view.loading = false;
            self.view.message = "Connect to the server to view the queue.".into();
            self.emit_queue(input, work);
            return;
        };
        self.view.loading = true;
        self.view.message.clear();
        self.emit_queue(input, work);
        self.task = Some(work.task(async move {
            Completed::Queue {
                request,
                result: api.queue().await.map_err(|error| describe(&error)),
            }
        }));
    }
    pub(crate) fn queue_loaded(
        &mut self,
        request: u64,
        result: Result<crate::models::QueueState, String>,
        input: &QueueInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if request != self.request || !self.open {
            return;
        }
        self.task = None;
        self.view.loading = false;
        match result {
            Ok(queue) => {
                if input
                    .selection
                    .player
                    .assignment
                    .as_ref()
                    .is_some_and(|state| state.revision > queue.revision)
                {
                    self.refresh_queue(input, work);
                    return;
                }
                self.view.current_entry_id = queue.current_entry_id;
                self.view.history_len = queue.history_len;
                self.view.mode = queue.mode;
                self.view.waiting_for_check = queue.waiting_for_check;
                self.view.revision = queue.revision;
                self.view.playback_token = queue.playback_token;
                self.view.entries = queue
                    .entries
                    .into_iter()
                    .map(|entry| {
                        let mut row = QueueRow::from(entry);
                        row.track.apply_name_preferences(input.preferences);
                        row.track.duration =
                            row.audio_source_id.and_then(|id| {
                                input.durations.get(&id).copied().flatten().or_else(|| {
                                    input.selection.track(id).and_then(|old| old.duration)
                                })
                            });
                        row
                    })
                    .collect();
                self.view.message.clear();
            }
            Err(error) => {
                self.view.message = error;
            }
        }
        work.effect(Effect::QueueChanged);
        self.emit_queue(input, work);
    }
    #[must_use]
    pub const fn state(&self) -> &QueueView {
        &self.view
    }
}

pub(crate) struct QueueInput<'a> {
    pub(crate) selection: crate::controller::media::MediaInput<'a>,
    pub(crate) preferences: crate::TrackNamePreferences,
    pub(crate) durations: &'a std::collections::HashMap<i32, Option<Duration>>,
}

impl QueueViewModel {
    pub(crate) fn apply_preferences(&mut self, preferences: crate::TrackNamePreferences) {
        for track in self.view.entries.iter_mut().map(|row| &mut row.track) {
            track.apply_name_preferences(preferences);
        }
    }
}

impl QueueViewModel {
    pub(crate) fn set_duration(&mut self, audio_id: i32, duration: Option<Duration>) {
        for track in self.view.entries.iter_mut().map(|row| &mut row.track) {
            if track.audio_source_id == audio_id {
                track.duration = duration;
            }
        }
    }
}

impl QueueViewModel {
    pub(crate) fn operation_status(
        &mut self,
        busy: bool,
        error: Option<String>,
        input: &QueueInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.view.busy = busy;
        self.view.operation_error = error;
        self.emit_queue(input, work);
    }
    pub(crate) fn action(
        &mut self,
        action: QueueAction,
        input: &QueueInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if action == QueueAction::Refresh {
            self.refresh_queue(input, work);
            return;
        }
        if work.api.is_none() {
            self.view.operation_error = Some("Connect to the server to change the queue.".into());
        } else {
            self.view.busy = true;
            self.view.operation_error = None;
            work.effect(Effect::QueueAction(action));
        }
        self.emit_queue(input, work);
    }
}

#[cfg(test)]
mod t07_tests;
#[cfg(test)]
mod t09_tests;
