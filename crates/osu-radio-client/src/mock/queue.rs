//! Memory-only queue scenarios for the gallery. No session, files or audio worker.
use crate::{
    models::{
        PlaybackMode, QueueAvailability, QueueEntry, QueueEntryId, QueueItemRef, QueuePlacement,
        RepeatMode,
    },
    view_models::{QueueAction, QueueRow, QueueView},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockQueue {
    pub view: QueueView,
    /// Display-only: the demonstration list is never reordered or wrapped.
    pub shuffle: bool,
    pub repeat: RepeatMode,
    next_id: i64,
}
impl Default for MockQueue {
    fn default() -> Self {
        let mut queue = Self {
            view: QueueView::default(),
            shuffle: false,
            repeat: RepeatMode::Off,
            next_id: 1,
        };
        for (audio, availability) in [
            (Some(1), QueueAvailability::Available),
            (Some(2), QueueAvailability::Available),
            (Some(1), QueueAvailability::Available),
            (Some(3), QueueAvailability::Checking),
            (None, QueueAvailability::Unavailable),
        ] {
            if let Ok(mut row) = queue.row(audio, None, "Demo song") {
                row.availability = availability;
                queue.view.entries.push(row);
            }
        }
        queue.view.current_entry_id = queue.view.entries.get(1).map(|row| row.id);
        queue.view.history_len = 1;
        queue.view.mode = PlaybackMode::Paused;
        queue.view.playback_token = 1;
        queue.reindex();
        queue
    }
}
impl MockQueue {
    fn row(
        &mut self,
        audio: Option<i32>,
        item: Option<i32>,
        title: &str,
    ) -> Result<QueueRow, &'static str> {
        // IDs are never recycled, including after ClearAll.
        let id = QueueEntryId::new(self.next_id)?;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or("Demo queue ID exhausted.")?;
        Ok(QueueRow::from(QueueEntry {
            id,
            position: 0,
            audio_source_id: audio,
            playlist_item_id: item,
            title: Some(title.into()),
            title_unicode: None,
            artist: Some("Demo artist".into()),
            artist_unicode: None,
            difficulty_name: None,
            beatmap_id: None,
            beatmap_set_id: None,
            cover_beatmap_id: None,
            last_played_at_ms: None,
            volume_percent: None,
            mods: None,
            availability: if audio.is_some() {
                QueueAvailability::Checking
            } else {
                QueueAvailability::Unavailable
            },
        }))
    }
    fn reindex(&mut self) {
        for (index, row) in self.view.entries.iter_mut().enumerate() {
            row.position = index;
        }
        if let Some(index) = self
            .view
            .entries
            .iter()
            .position(|row| Some(row.id) == self.view.current_entry_id)
        {
            self.view.history_len = index;
        }
        if self.view.history_len > 5000 {
            let remove = self.view.history_len.saturating_sub(5000);
            self.view.entries.drain(..remove);
            self.view.history_len = self.view.history_len.saturating_sub(remove);
            for (index, row) in self.view.entries.iter_mut().enumerate() {
                row.position = index;
            }
        }
    }
    fn next_index(&self) -> usize {
        self.view
            .history_len
            .saturating_add(usize::from(self.view.current_entry_id.is_some()))
    }
    fn forward(&mut self, start: usize, mode: PlaybackMode) {
        self.view.playback_token = self.view.playback_token.saturating_add(1);
        if let Some(row) = self
            .view
            .entries
            .iter()
            .skip(start)
            .find(|row| row.availability != QueueAvailability::Unavailable)
        {
            self.view.current_entry_id = Some(row.id);
            self.view.history_len = row.position;
            self.view.mode = mode;
            self.view.waiting_for_check = row.availability == QueueAvailability::Checking;
        } else {
            self.view.current_entry_id = None;
            self.view.history_len = self.view.entries.len();
            self.view.mode = PlaybackMode::Stopped;
            self.view.waiting_for_check = false;
        }
    }
    fn insert(&mut self, rows: Vec<QueueRow>, placement: QueuePlacement) {
        let index = match placement {
            QueuePlacement::End => self.view.entries.len(),
            QueuePlacement::Next => self.next_index(),
        };
        self.view.entries.splice(index..index, rows);
    }
    /// Invalid actions keep the confirmed list and transport intention intact.
    pub fn apply(&mut self, action: QueueAction, playlists: &super::MockPlaylists) {
        let mut before = self.clone();
        if let Err(error) = self.change(action, playlists) {
            *self = before;
            self.view.operation_error = Some(error.into());
            return;
        }
        self.view.operation_error = None;
        self.reindex();
        before.view.operation_error = None;
        if *self != before {
            self.view.revision = self.view.revision.saturating_add(1);
        }
    }
    fn index(&self, id: QueueEntryId) -> Result<usize, &'static str> {
        self.view
            .entries
            .iter()
            .position(|row| row.id == id)
            .ok_or("Queue entry no longer exists.")
    }
    fn change(
        &mut self,
        action: QueueAction,
        playlists: &super::MockPlaylists,
    ) -> Result<(), &'static str> {
        match action {
            QueueAction::Refresh => {}
            QueueAction::Play(id) => {
                let index = self.index(id)?;
                if self
                    .view
                    .entries
                    .get(index)
                    .is_some_and(|row| row.availability == QueueAvailability::Unavailable)
                {
                    return Err("Queue entry is unavailable.");
                }
                if self.view.current_entry_id == Some(id) {
                    self.view.mode = PlaybackMode::Playing;
                } else {
                    self.forward(index, PlaybackMode::Playing);
                }
            }
            QueueAction::Remove(id) => {
                let index = self.index(id)?;
                self.view.entries.remove(index);
                if self.view.current_entry_id == Some(id) {
                    let mode = if self.view.mode == PlaybackMode::Playing {
                        PlaybackMode::Playing
                    } else {
                        PlaybackMode::Paused
                    };
                    self.reindex();
                    self.forward(index, mode);
                } else if index < self.view.history_len {
                    self.view.history_len = self.view.history_len.saturating_sub(1);
                }
            }
            QueueAction::Move {
                id,
                before_entry_id,
            } => {
                let from = self.index(id)?;
                let to = before_entry_id
                    .map(|id| self.index(id))
                    .transpose()?
                    .unwrap_or(self.view.entries.len());
                if from != to && from.saturating_add(1) != to {
                    let row = self.view.entries.remove(from);
                    self.view
                        .entries
                        .insert(if from < to { to.saturating_sub(1) } else { to }, row);
                }
            }
            QueueAction::ClearUpcoming => {
                self.view.entries.truncate(self.next_index());
            }
            QueueAction::ClearAll => {
                if !self.view.entries.is_empty() {
                    self.view.entries.clear();
                    self.view.current_entry_id = None;
                    self.view.history_len = 0;
                    self.view.waiting_for_check = false;
                    self.view.mode = PlaybackMode::Stopped;
                    self.view.playback_token = self.view.playback_token.saturating_add(1);
                }
            }
            QueueAction::Insert { items, placement } => {
                let mut rows = Vec::new();
                for item in items {
                    rows.push(match item {
                        QueueItemRef::Audio { audio_source_id } => {
                            if !(1..=4).contains(&audio_source_id) {
                                return Err("Audio source no longer exists.");
                            }
                            self.row(Some(audio_source_id), None, "Demo song")?
                        }
                        QueueItemRef::PlaylistItem {
                            playlist_id,
                            item_id,
                        } => {
                            let item = playlists
                                .items(playlist_id)
                                .and_then(|items| items.iter().find(|item| item.id == item_id))
                                .ok_or("Playlist item no longer exists.")?;
                            self.playlist_row(item)?
                        }
                    });
                }
                self.insert(rows, placement);
            }
        }
        Ok(())
    }
    fn playlist_row(
        &mut self,
        item: &crate::models::PlaylistItem,
    ) -> Result<QueueRow, &'static str> {
        let mut row = self.row(
            item.audio_source_id,
            Some(item.id),
            item.title.as_deref().unwrap_or("Unknown title"),
        )?;
        row.track = item.track();
        Ok(row)
    }
    pub fn append_playlist(&mut self, id: i32, playlists: &super::MockPlaylists) {
        let Some(items) = playlists.items(id) else {
            self.view.operation_error = Some("Playlist no longer exists.".into());
            return;
        };
        let next_id = self.next_id;
        let rows = items
            .iter()
            .map(|item| self.playlist_row(item))
            .collect::<Result<Vec<_>, _>>();
        let rows = match rows {
            Ok(rows) => rows,
            Err(error) => {
                self.next_id = next_id;
                self.view.operation_error = Some(error.into());
                return;
            }
        };
        self.insert(rows, QueuePlacement::End);
        self.reindex();
        self.view.operation_error = None;
        if !items.is_empty() {
            self.view.revision = self.view.revision.saturating_add(1);
        }
    }
    /// Simulates the asynchronous check result without probing a source file.
    pub fn checked(&mut self, id: QueueEntryId, available: bool) {
        let Ok(index) = self.index(id) else {
            return;
        };
        let Some(row) = self.view.entries.get_mut(index) else {
            return;
        };
        if row.availability != QueueAvailability::Checking {
            return;
        }
        row.availability = if available {
            QueueAvailability::Available
        } else {
            QueueAvailability::Unavailable
        };
        if self.view.current_entry_id == Some(id) && self.view.waiting_for_check {
            if available {
                self.view.waiting_for_check = false;
            } else {
                self.forward(index.saturating_add(1), self.view.mode);
            }
        }
        self.reindex();
        self.view.revision = self.view.revision.saturating_add(1);
    }
    pub fn pause(&mut self) {
        if self.view.mode == PlaybackMode::Playing {
            self.view.mode = PlaybackMode::Paused;
            self.view.revision = self.view.revision.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{Action, MockState};
    fn id(value: i64) -> QueueEntryId {
        QueueEntryId::new(value).unwrap()
    }
    #[test]
    fn modes_toggle_offline_without_touching_the_list() {
        let mut state = MockState::default();
        let before = state.queue.view.clone();
        assert!(state.apply(Action::ToggleShuffle));
        assert!(state.queue.shuffle);
        for expected in [RepeatMode::All, RepeatMode::One, RepeatMode::Off] {
            assert!(state.apply(Action::CycleRepeat));
            assert_eq!(state.queue.repeat, expected);
        }
        assert!(state.apply(Action::ToggleShuffle) && !state.queue.shuffle);
        assert_eq!(state.queue.view, before);
    }
    #[test]
    fn memory_queue_preserves_history_duplicates_and_errors_and_all_edit_operations() {
        let mut state = MockState::default();
        let original = state.queue.view.clone();
        assert_eq!(original.history_len, 1);
        assert_eq!(
            original.entries.first().unwrap().audio_source_id,
            original.entries.get(2).unwrap().audio_source_id
        );
        assert_ne!(
            original.entries.first().unwrap().id,
            original.entries.get(2).unwrap().id
        );
        state.apply(Action::Queue(QueueAction::Insert {
            items: vec![
                QueueItemRef::Audio { audio_source_id: 3 },
                QueueItemRef::PlaylistItem {
                    playlist_id: 1,
                    item_id: 3,
                },
            ],
            placement: QueuePlacement::Next,
        }));
        assert_eq!(
            state
                .queue
                .view
                .entries
                .iter()
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            [id(1), id(2), id(6), id(7), id(3), id(4), id(5)]
        );
        assert_eq!(
            state.queue.view.entries.get(3).unwrap().availability,
            QueueAvailability::Unavailable
        );
        assert_eq!(
            (state.queue.view.mode, state.queue.view.playback_token),
            (PlaybackMode::Paused, original.playback_token)
        );
        state.apply(Action::Queue(QueueAction::Move {
            id: id(6),
            before_entry_id: Some(id(1)),
        }));
        assert_eq!(state.queue.view.history_len, 2);
        state.apply(Action::Queue(QueueAction::Remove(id(1))));
        assert_eq!(state.queue.view.history_len, 1);
        let confirmed = state.queue.view.clone();
        state.apply(Action::Queue(QueueAction::Insert {
            items: vec![
                QueueItemRef::Audio { audio_source_id: 1 },
                QueueItemRef::PlaylistItem {
                    playlist_id: 1,
                    item_id: 999,
                },
            ],
            placement: QueuePlacement::End,
        }));
        assert_eq!(state.queue.view.entries, confirmed.entries);
        assert_eq!(state.queue.view.revision, confirmed.revision);
        assert!(state.queue.view.operation_error.is_some());
        state.apply(Action::Queue(QueueAction::Move {
            id: id(6),
            before_entry_id: None,
        }));
        assert_eq!(state.queue.view.entries.last().unwrap().id, id(6));
        state.apply(Action::Playlist(
            crate::view_models::PlaylistAction::AddToQueue(1),
        ));
        let appended = state
            .queue
            .view
            .entries
            .get(state.queue.view.entries.len().saturating_sub(3)..)
            .unwrap();
        assert_eq!(
            appended
                .iter()
                .map(|row| row.playlist_item_id)
                .collect::<Vec<_>>(),
            [Some(1), Some(2), Some(3)]
        );
        assert_eq!(
            appended.get(2).unwrap().availability,
            QueueAvailability::Unavailable
        );
        assert_eq!(state.queue.view.mode, PlaybackMode::Paused);
        state.apply(Action::Queue(QueueAction::ClearUpcoming));
        assert_eq!(state.queue.view.entries.len(), 1);
        state.apply(Action::Queue(QueueAction::ClearAll));
        assert!(state.queue.view.entries.is_empty());
        state.apply(Action::Queue(QueueAction::Insert {
            items: vec![QueueItemRef::Audio { audio_source_id: 1 }],
            placement: QueuePlacement::End,
        }));
        assert_eq!(state.queue.view.mode, PlaybackMode::Stopped);
        assert!(state.queue.view.entries.first().unwrap().id > id(7));
    }
    #[test]
    fn insertion_checks_never_start_empty_or_exhausted_queues_and_play_uses_first_future() {
        for exhausted in [false, true] {
            for placement in [QueuePlacement::End, QueuePlacement::Next] {
                let mut state = MockState::default();
                if exhausted {
                    state.queue.view.entries.truncate(2);
                    state.queue.forward(2, PlaybackMode::Playing);
                } else {
                    state.apply(Action::Queue(QueueAction::ClearAll));
                }
                let before = state.queue.view.clone();
                let selected = state.selected;
                state.apply(Action::Queue(QueueAction::Insert {
                    items: vec![
                        QueueItemRef::Audio { audio_source_id: 3 },
                        QueueItemRef::Audio { audio_source_id: 4 },
                    ],
                    placement,
                }));
                let upcoming: Vec<_> = state.queue.view.upcoming().map(|row| row.id).collect();
                assert_eq!(upcoming.len(), 2);
                for id in &upcoming {
                    state.apply(Action::QueueChecked {
                        id: *id,
                        available: true,
                    });
                }
                assert_eq!(state.queue.view.mode, PlaybackMode::Stopped);
                assert_eq!(state.queue.view.current_entry_id, None);
                assert_eq!(state.queue.view.playback_token, before.playback_token);
                assert_eq!(state.queue.view.history_len, before.history_len);
                assert_eq!(state.selected, selected);
                state.apply(Action::Queue(QueueAction::Play(*upcoming.first().unwrap())));
                assert_eq!(state.queue.view.current_entry_id, upcoming.first().copied());
                assert_eq!(
                    state
                        .queue
                        .view
                        .entries
                        .get(state.queue.view.history_len)
                        .unwrap()
                        .audio_source_id,
                    Some(3)
                );
            }
        }
    }

    #[test]
    fn checking_results_preserve_pause_and_stale_results_do_not_affect_a_replacement() {
        let mut state = MockState::default();
        state.apply(Action::Queue(QueueAction::Play(id(4))));
        assert!(state.queue.view.waiting_for_check);
        assert_eq!(state.queue.view.mode, PlaybackMode::Playing);
        state.apply(Action::QueuePause);
        state.apply(Action::QueueChecked {
            id: id(4),
            available: true,
        });
        assert!(!state.queue.view.waiting_for_check);
        assert_eq!(state.queue.view.mode, PlaybackMode::Paused);
        assert!(
            state
                .queue
                .view
                .entries
                .iter()
                .all(|row| row.track.last_played_at_ms.is_none())
        );
        state.apply(Action::Queue(QueueAction::Play(id(5))));
        assert!(state.queue.view.operation_error.is_some());
        assert_eq!(state.queue.view.current_entry_id, Some(id(4)));
        state.apply(Action::Queue(QueueAction::Remove(id(4))));
        assert_eq!(state.queue.view.mode, PlaybackMode::Stopped);
        let confirmed = state.queue.view.clone();
        state.apply(Action::QueueChecked {
            id: id(4),
            available: false,
        });
        assert_eq!(state.queue.view, confirmed);
    }
    #[test]
    fn checking_failure_advances_with_intention_and_success_respects_playing() {
        for available in [false, true] {
            let mut state = MockState::default();
            state.apply(Action::Queue(QueueAction::Play(id(4))));
            state.apply(Action::QueueChecked {
                id: id(4),
                available,
            });
            assert!(!state.queue.view.waiting_for_check);
            assert_eq!(
                state.queue.view.mode,
                if available {
                    PlaybackMode::Playing
                } else {
                    PlaybackMode::Stopped
                }
            );
        }
    }
}
