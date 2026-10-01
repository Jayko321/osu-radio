use super::{AppUpdate, Completed, Controller, Duration, Track, describe};
use crate::models::{AudioSettings, PlaybackAssignment};
use std::collections::{HashMap, VecDeque};
use tokio::{task::JoinHandle, time::Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Independent load/save and selected-track flags for adapters.
pub struct AudioSettingsState {
    pub settings: AudioSettings,
    pub loaded: bool,
    pub loading: bool,
    pub saving: bool,
    pub error: String,
    pub selected_audio_id: Option<i32>,
    pub selected_volume_percent: u8,
    pub selected_has_override: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Change {
    Settings(AudioSettings),
    Track(i32, Option<u8>),
}
impl Change {
    fn same_target(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Settings(_), Self::Settings(_)) => true,
            (Self::Track(a, _), Self::Track(b, _)) => a == b,
            _ => false,
        }
    }
}
#[derive(Clone)]
struct Save {
    change: Change,
    deadline: Instant,
}

#[derive(Default)]
pub(super) struct VolumeWork {
    pub settings: Option<AudioSettings>,
    loading: bool,
    error: String,
    // Local values win over reads started before the edit, including a pending DELETE (None).
    overrides: HashMap<i32, Option<u8>>,
    pending: VecDeque<Save>,
    saving: Option<Save>,
    pub save_task: Option<JoinHandle<Result<(), String>>>,
    stream_started: bool,
    pub deferred_assignment: Option<PlaybackAssignment>,
}

impl Controller {
    pub(super) fn load_audio_settings(&mut self) {
        if self.volume.loading || self.volume.settings.is_some() {
            return;
        }
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            return;
        };
        self.volume.loading = true;
        self.volume.error.clear();
        self.emit_volume();
        self.task(async move {
            Completed::AudioSettings(api.audio_settings().await.map_err(|error| describe(&error)))
        });
    }
    pub(super) fn audio_settings_loaded(&mut self, result: Result<AudioSettings, String>) {
        self.volume.loading = false;
        match result {
            Ok(settings) => {
                self.volume.settings = Some(settings);
                self.volume.error.clear();
                if let Some(assignment) = self.volume.deferred_assignment.take() {
                    self.apply_assignment(assignment);
                }
                if !self.volume.stream_started {
                    self.volume.stream_started = true;
                    self.start_playback_stream();
                }
            }
            Err(error) => {
                self.volume.error = format!("Failed to load volume settings: {error}");
            }
        }
        self.emit_volume();
    }
    pub(super) fn selected_volume_id(&self) -> Option<i32> {
        if self.playlists.view.showing_detail() {
            self.playlists
                .view
                .selected_item()
                .and_then(|item| item.audio_source_id)
        } else {
            self.selected
                .filter(|id| *id >= 0 && self.track(*id).is_some())
        }
    }
    fn track_override(&self, id: i32) -> Option<u8> {
        self.volume
            .overrides
            .get(&id)
            .copied()
            .unwrap_or_else(|| self.track(id).and_then(|track| track.volume_percent))
    }
    fn effective_percent(&self, id: Option<i32>, stored: Option<u8>) -> u8 {
        let settings = self.volume.settings.unwrap_or_default();
        if settings.individual_volume_enabled {
            id.and_then(|id| self.volume.overrides.get(&id).copied().unwrap_or(stored))
                .unwrap_or(settings.global_volume_percent)
        } else {
            settings.global_volume_percent
        }
    }
    pub(super) fn assignment_volume(&self) -> f32 {
        f32::from(
            self.effective_percent(
                self.assignment
                    .as_ref()
                    .and_then(|a| a.current_audio_source_id),
                self.current_track.as_ref().and_then(|t| t.volume_percent),
            ),
        ) / 100.0
    }
    pub(super) fn emit_volume(&self) {
        let id = self.selected_volume_id();
        let stored = id.and_then(|id| self.track_override(id));
        (self.emit)(AppUpdate::AudioSettings(AudioSettingsState {
            settings: self.volume.settings.unwrap_or_default(),
            loaded: self.volume.settings.is_some(),
            loading: self.volume.loading,
            saving: self.volume.saving.is_some() || !self.volume.pending.is_empty(),
            error: self.volume.error.clone(),
            selected_audio_id: id,
            selected_volume_percent: self.effective_percent(id, stored),
            selected_has_override: stored.is_some(),
        }));
    }
    pub(super) fn merge_volume_overrides(&self, tracks: &mut [Track]) {
        for track in tracks {
            if let Some(percent) = self.volume.overrides.get(&track.audio_source_id) {
                track.volume_percent = *percent;
            }
        }
    }
    fn apply_current_volume(&self) {
        if let Some(worker) = &self.playback {
            worker.send(crate::playback::Command::SetVolume(
                self.assignment_volume(),
            ));
        }
    }
    fn enqueue_volume_save(&mut self, change: Change) {
        let save = Save {
            change,
            deadline: Instant::now()
                .checked_add(Duration::from_millis(200))
                .unwrap_or_else(Instant::now),
        };
        if let Some(pending) = self
            .volume
            .pending
            .iter_mut()
            .find(|p| p.change.same_target(&save.change))
        {
            *pending = save;
        } else {
            self.volume.pending.push_back(save);
        }
        self.volume.error.clear();
        self.apply_current_volume();
        self.emit_volume();
    }
    pub(super) fn set_individual_volume(&mut self, enabled: bool) {
        let Some(mut settings) = self.volume.settings else {
            return;
        };
        if settings.individual_volume_enabled == enabled {
            return;
        }
        settings.individual_volume_enabled = enabled;
        self.volume.settings = Some(settings);
        self.enqueue_volume_save(Change::Settings(settings));
    }
    pub(super) fn set_global_volume(&mut self, percent: u8) {
        let Some(mut settings) = self.volume.settings else {
            return;
        };
        if percent > 100 || settings.global_volume_percent == percent {
            return;
        }
        settings.global_volume_percent = percent;
        self.volume.settings = Some(settings);
        self.enqueue_volume_save(Change::Settings(settings));
    }
    pub(super) fn set_selected_volume(&mut self, volume: f32) {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return;
        }
        #[allow(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )] // Validated finite 0..1, rounded to 0..100.
        let percent = (volume * 100.0).round() as u8;
        if self
            .volume
            .settings
            .is_some_and(|s| s.individual_volume_enabled)
        {
            if let Some(id) = self.selected_volume_id() {
                self.set_track_volume(id, Some(percent));
            }
        } else {
            self.set_global_volume(percent);
        }
    }
    pub(super) fn set_track_volume(&mut self, id: i32, percent: Option<u8>) {
        if !self
            .volume
            .settings
            .is_some_and(|s| s.individual_volume_enabled)
            || percent.is_some_and(|p| p > 100)
            || self.selected_volume_id() != Some(id)
        {
            return;
        }
        self.volume.overrides.insert(id, percent);
        for track in self
            .tracks
            .iter_mut()
            .chain(&mut self.playlists.tracks)
            .chain(&mut self.current_track)
        {
            if track.audio_source_id == id {
                track.volume_percent = percent;
            }
        }
        if let Some(playlist) = &mut self.playlists.view.active {
            for item in &mut playlist.items {
                if item.audio_source_id == Some(id) {
                    item.volume_percent = percent;
                }
            }
        }
        self.enqueue_volume_save(Change::Track(id, percent));
    }
    pub(super) fn finish_volume_editing(&mut self) {
        for pending in &mut self.volume.pending {
            pending.deadline = Instant::now();
        }
    }
    pub(super) fn retry_audio_settings(&mut self) {
        if self.volume.settings.is_none() {
            self.load_audio_settings();
        } else {
            self.volume.error.clear();
            self.finish_volume_editing();
            self.emit_volume();
        }
    }
    pub(super) fn volume_deadline(&self) -> Option<Instant> {
        if self.volume.save_task.is_some() || !self.volume.error.is_empty() {
            return None;
        }
        self.volume.pending.front().map(|save| save.deadline)
    }
    pub(super) fn start_volume_save(&mut self) {
        if self
            .volume_deadline()
            .is_none_or(|deadline| deadline > Instant::now())
        {
            return;
        }
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            return;
        };
        let Some(save) = self.volume.pending.pop_front() else {
            return;
        };
        let change = save.change.clone();
        self.volume.saving = Some(save);
        // This write has its own handle: shutdown awaits it before stopping the backend.
        self.volume.save_task = Some(tokio::spawn(async move {
            match change {
                Change::Settings(settings) => {
                    api.update_audio_settings(&settings).await.map(|_| ())
                }
                Change::Track(id, percent) => api.set_audio_volume(id, percent).await,
            }
            .map_err(|error| describe(&error))
        }));
        self.emit_volume();
    }
    pub(super) fn volume_saved(&mut self, result: Result<(), String>) {
        self.volume.save_task = None;
        let Some(save) = self.volume.saving.take() else {
            return;
        };
        if let Err(error) = result
            && !self
                .volume
                .pending
                .iter()
                .any(|p| p.change.same_target(&save.change))
        {
            self.volume.pending.push_front(save);
            self.volume.error = format!("Failed to save volume: {error}");
        }
        // Responses acknowledge writes only; they never overwrite a newer local edit.
        self.emit_volume();
    }
    pub(super) async fn flush_volume_saves(&mut self) {
        self.volume.error.clear();
        self.finish_volume_editing();
        loop {
            self.start_volume_save();
            let Some(task) = self.volume.save_task.take() else {
                break;
            };
            self.volume_saved(task.await.unwrap_or_else(|error| Err(error.to_string())));
            if !self.volume.error.is_empty() {
                eprintln!("{}", self.volume.error);
                // Attempt every independent target once at close, even if one final write fails.
                self.volume.pending.pop_front();
                self.volume.error.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests;
