//! Library presentation state and request/selection transitions.
use super::{Track, TrackNamePreferences, TrackSort, selection_after_refresh};
use crate::controller::workflow::{Effect, Workflow};
use crate::controller::{AppUpdate, Completed, OperationStatus, SettingsRetry, failure};
use crate::describe;
use std::{collections::HashMap, time::Duration};
use tokio::{task::AbortHandle, time::Instant};

pub struct LibraryViewModel {
    pub(crate) tracks: Vec<Track>,
    pub(crate) indices: HashMap<i32, usize>,
    pub(crate) selected: Option<i32>,
    pub(crate) status: OperationStatus,
    pub(crate) query: String,
    pub(crate) request: u64,
    pub(crate) task: Option<AbortHandle>,
    pub(crate) deadline: Option<Instant>,
    pub(crate) invalidation_pending: bool,
    pub(crate) last_played: HashMap<i32, i64>,
}
impl Default for LibraryViewModel {
    fn default() -> Self {
        Self {
            tracks: Vec::new(),
            indices: HashMap::new(),
            selected: None,
            status: OperationStatus::default(),
            query: String::new(),
            request: 0,
            task: None,
            deadline: None,
            invalidation_pending: true,
            last_played: HashMap::new(),
        }
    }
}
impl LibraryViewModel {
    #[must_use]
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }
    #[must_use]
    pub const fn selected_id(&self) -> Option<i32> {
        self.selected
    }
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }
    #[must_use]
    pub const fn status(&self) -> &OperationStatus {
        &self.status
    }
    pub(crate) fn search(&mut self, query: String) -> bool {
        if self.query == query {
            return false;
        }
        self.query = query;
        true
    }
    pub(crate) fn begin_load(&mut self, delay: Duration, invalidate: bool) -> (u64, bool) {
        self.invalidation_pending |= invalidate;
        self.deadline = Instant::now().checked_add(delay);
        self.request = self.request.wrapping_add(1);
        if let Some(task) = self.task.take() {
            task.abort();
        }
        (self.request, self.invalidation_pending)
    }
    pub(crate) fn accepts(&mut self, request: u64) -> bool {
        if request != self.request {
            return false;
        }
        self.task = None;
        self.status = OperationStatus::default();
        true
    }
    pub(crate) const fn select(&mut self, id: Option<i32>, available: bool) -> bool {
        if id.is_some() && !available {
            return false;
        }
        self.selected = id;
        true
    }
    pub(crate) fn replace(&mut self, tracks: Vec<Track>, current_id: Option<i32>) {
        if self.selected != current_id || self.selected.is_none() {
            self.selected = selection_after_refresh(&tracks, self.selected);
        }
        self.tracks = tracks;
        self.reindex();
        if self.tracks.is_empty() {
            self.status.message = if self.query.trim().is_empty() {
                "No songs yet. Import an osu! folder to populate your library."
            } else {
                "Nothing found."
            }
            .into();
        }
    }
    pub(crate) fn reindex(&mut self) {
        self.indices = self
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| (track.audio_source_id, index))
            .collect();
    }
    pub(crate) fn reorder(&mut self, sort: TrackSort) {
        sort.sort_tracks(&mut self.tracks);
        self.reindex();
    }
    pub(crate) fn apply_preferences(&mut self, preferences: TrackNamePreferences) {
        for track in &mut self.tracks {
            track.apply_name_preferences(preferences);
        }
    }
}

impl LibraryViewModel {
    pub(crate) fn load_library(
        &mut self,
        delay: Duration,
        invalidate_artwork: bool,
        _input: &LibraryInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let (request, invalidate_artwork) = self.begin_load(delay, invalidate_artwork);
        let Some(api) = work.api.clone() else {
            work.effect(Effect::Connect);
            return;
        };
        let query = self.query.clone();
        self.status = OperationStatus {
            loading: true,
            message: "Loading library…".into(),
            ..Default::default()
        };
        work.emit(AppUpdate::LibraryStatus(self.status.clone()));
        self.task = Some(work.task(async move {
            tokio::time::sleep(delay).await;
            Completed::Library {
                request,
                invalidate_artwork,
                result: api
                    .search_tracks(&query)
                    .await
                    .map(|tracks| tracks.into_iter().map(Track::from).collect())
                    .map_err(|error| describe(&error)),
            }
        }));
    }
    pub(crate) fn replace_tracks(
        &mut self,
        mut tracks: Vec<Track>,
        invalidate_artwork: bool,
        input: &LibraryInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        work.effect(Effect::AdvanceMedia(invalidate_artwork));
        for track in &mut tracks {
            track.apply_name_preferences(input.settings.preferences.name_preferences);
        }
        if !invalidate_artwork {
            input.media.restore_durations(&mut tracks);
        }
        self.merge_last_played(&mut tracks);
        input.settings.merge_volume_overrides(&mut tracks);
        input.settings.sort.sort_tracks(&mut tracks);
        self.replace(
            tracks,
            input
                .player
                .current_track
                .as_ref()
                .map(|track| track.audio_source_id),
        );
        work.emit(AppUpdate::LibraryTracksReplaced {
            tracks: self.tracks.clone(),
            invalidate_artwork,
        });
        work.emit(AppUpdate::LibrarySelected(self.selected));
        work.effect(Effect::Selection);
    }
    pub(crate) fn merge_last_played(&mut self, tracks: &mut [Track]) {
        for track in tracks.iter() {
            if track.audio_source_id >= 0
                && let Some(date) = track.last_played_at_ms
            {
                let known = self
                    .last_played
                    .entry(track.audio_source_id)
                    .or_insert(date);
                *known = (*known).max(date);
            }
        }
        for track in tracks {
            if track.audio_source_id >= 0 {
                track.last_played_at_ms = self.last_played.get(&track.audio_source_id).copied();
            }
        }
    }
}

pub(crate) struct LibraryInput<'a> {
    pub(crate) player: &'a super::PlayerViewModel,
    pub(crate) settings: &'a super::SettingsViewModel,
    pub(crate) media: &'a crate::controller::media::MediaScheduler,
}
impl LibraryViewModel {
    pub(crate) fn loaded(
        &mut self,
        request: u64,
        invalidate_artwork: bool,
        result: Result<Vec<Track>, String>,
        input: &LibraryInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if !self.accepts(request) {
            return;
        }
        match result {
            Ok(tracks) => {
                self.invalidation_pending = false;
                self.replace_tracks(tracks, invalidate_artwork, input, work);
                work.effect(Effect::RefreshOpenPlaylist);
            }
            Err(error) => self.status = failure(error, SettingsRetry::Load),
        }
        work.emit(AppUpdate::LibraryStatus(self.status.clone()));
    }
}

impl LibraryViewModel {
    pub(crate) fn remember_dates(&mut self, dates: Vec<(i32, i64)>) {
        for (id, date) in dates {
            let known = self.last_played.entry(id).or_insert(date);
            *known = (*known).max(date);
        }
    }
    pub(crate) fn apply_known_dates(&mut self) -> bool {
        let mut changed = false;
        for track in &mut self.tracks {
            if track.audio_source_id >= 0 {
                let date = self.last_played.get(&track.audio_source_id).copied();
                changed |= track.last_played_at_ms != date;
                track.last_played_at_ms = date;
            }
        }
        changed
    }
}

impl LibraryViewModel {
    pub(crate) fn connection_pending(&mut self) {
        self.status = OperationStatus {
            loading: true,
            message: "Connecting…".into(),
            ..Default::default()
        };
    }
    pub(crate) const fn connection_finished(&mut self) {
        self.status.loading = false;
    }
    pub(crate) fn connection_failed(&mut self, error: String) {
        self.status = failure(error, SettingsRetry::Load);
    }
}

impl LibraryViewModel {
    pub(crate) fn set_duration(&mut self, audio_id: i32, duration: Option<Duration>) {
        for track in &mut self.tracks {
            if track.audio_source_id == audio_id {
                track.duration = duration;
            }
        }
    }
    pub(crate) fn set_track_volume(&mut self, audio_id: i32, percent: Option<u8>) {
        for track in &mut self.tracks {
            if track.audio_source_id == audio_id {
                track.volume_percent = percent;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::tests::track;

    #[tokio::test]
    async fn new_load_cancels_old_work_and_stale_completion_preserves_current_status() {
        let mut library = LibraryViewModel::default();
        let old = tokio::spawn(std::future::pending::<()>());
        library.task = Some(old.abort_handle());
        library.request = u64::MAX;
        library.connection_pending();
        assert!(library.status().loading);
        assert_eq!(library.status().message, "Connecting…");
        let before = Instant::now();
        assert_eq!(library.begin_load(Duration::from_secs(1), false), (0, true));
        assert!(library.deadline.unwrap() >= before.checked_add(Duration::from_secs(1)).unwrap());
        assert!(
            library.deadline.unwrap()
                <= Instant::now().checked_add(Duration::from_secs(1)).unwrap()
        );
        assert!(old.await.unwrap_err().is_cancelled());
        assert!(!library.accepts(u64::MAX));
        assert!(
            library.status().loading,
            "stale completion cannot clear current status"
        );
        assert!(library.accepts(0));
        assert!(!library.status().loading);
        assert!(library.status().message.is_empty());
        library.invalidation_pending = false;
        assert_eq!(library.begin_load(Duration::ZERO, false), (1, false));
        assert_eq!(library.begin_load(Duration::ZERO, true), (2, true));
        assert_eq!(library.begin_load(Duration::ZERO, false), (3, true));
        library.connection_failed("offline".into());
        assert!(!library.status().loading);
        assert_eq!(library.status().message, "offline");
        assert_eq!(library.status().retry, Some(SettingsRetry::Load));
        library.connection_pending();
        library.connection_finished();
        assert!(!library.status().loading);
        assert!(library.status().retry.is_none());
    }

    #[test]
    fn refresh_preserves_current_selection_outside_results_and_rejects_unavailable_selection() {
        let mut library = LibraryViewModel::default();
        library.replace(vec![track(7), track(8)], None);
        assert_eq!(library.tracks().len(), 2);
        assert_eq!(library.selected_id(), Some(7));
        assert!(!library.select(Some(99), false));
        assert_eq!(library.selected_id(), Some(7));
        assert!(library.select(Some(8), true));
        library.replace(vec![track(7)], Some(8));
        assert_eq!(library.selected_id(), Some(8));
        library.replace(vec![track(7)], None);
        assert_eq!(library.selected_id(), Some(7));
        assert!(library.select(None, false));
        assert_eq!(library.selected_id(), None);
        library.replace(Vec::new(), None);
        assert_eq!(library.selected_id(), None);
        assert_eq!(
            library.status().message,
            "No songs yet. Import an osu! folder to populate your library."
        );
        assert!(library.search(" 曲 ".into()));
        assert!(!library.search(" 曲 ".into()));
        assert_eq!(library.query(), " 曲 ");
        library.replace(Vec::new(), None);
        assert_eq!(library.status().message, "Nothing found.");
        assert!(library.search(" \t ".into()));
        library.replace(Vec::new(), None);
        assert_eq!(
            library.status().message,
            "No songs yet. Import an osu! folder to populate your library."
        );
    }

    #[test]
    fn history_only_moves_forward_and_updates_skip_unrelated_and_unavailable_rows() {
        let mut library = LibraryViewModel::default();
        let mut missing = track(-1);
        missing.last_played_at_ms = Some(3);
        let mut rows = vec![track(7), track(8), missing];
        library.remember_dates(vec![(7, 20), (7, 10), (-1, 99)]);
        library.merge_last_played(&mut rows);
        assert_eq!(
            rows.iter().map(|r| r.last_played_at_ms).collect::<Vec<_>>(),
            [Some(20), None, Some(3)]
        );
        library.replace(rows, None);
        library.remember_dates(vec![(7, 15), (8, 30)]);
        assert!(library.apply_known_dates());
        assert!(!library.apply_known_dates());
        let before = library.tracks().to_vec();
        library.set_duration(7, None);
        library.set_track_volume(7, Some(0));
        assert_eq!(library.tracks().first().unwrap().duration, None);
        assert_eq!(library.tracks().first().unwrap().volume_percent, Some(0));
        assert_eq!(library.tracks().get(1..), before.get(1..));
        library.set_duration(7, Some(Duration::ZERO));
        library.set_track_volume(7, None);
        assert_eq!(
            library.tracks().first().unwrap().duration,
            Some(Duration::ZERO)
        );
        assert_eq!(library.tracks().first().unwrap().volume_percent, None);
    }
}
