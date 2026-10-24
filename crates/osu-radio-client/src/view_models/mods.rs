//! Mod editing rules and its independent, serialized persistence lane.
use super::settings::{SettingsInput, SettingsViewModel};
use crate::controller::{AppUpdate, Completed, workflow::Workflow};
use crate::models::{
    AudioModsMode, AudioModsPatch, AudioModsProfile, AudioModsSettings, PlaybackAssignment,
};
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tokio::{task::JoinHandle, time::Instant};

/// Capture from the latest state when opening an editor; never rebuild it from an audio ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModsEditTarget {
    pub audio_id: Option<i32>,
    epoch: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModsEdit {
    CycleDt,
    ToggleHt,
    SetSpeed(u16),
    SetAdjustPitch(bool),
    Clear,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModsAction {
    SetIndividual(bool),
    Edit {
        target: ModsEditTarget,
        edit: ModsEdit,
    },
    /// Finish keeps local edits and makes their writes due; it also invalidates late UI events.
    Finish(ModsEditTarget),
    Retry,
}
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Independent load/save/override flags for adapters.
pub struct AudioModsState {
    pub settings: AudioModsSettings,
    pub loaded: bool,
    pub loading: bool,
    pub saving: bool,
    pub error: String,
    pub target: Option<ModsEditTarget>,
    pub selected_profile: AudioModsProfile,
    pub selected_has_override: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Settings(AudioModsSettings),
    Track(i32, Option<AudioModsProfile>),
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

/// Shared by the live Settings VM and the memory-only gallery.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Editor {
    pub settings: AudioModsSettings,
    pub overrides: HashMap<i32, Option<AudioModsProfile>>,
    selected: Option<i32>,
    epoch: u64,
}
impl Editor {
    pub fn select(&mut self, id: Option<i32>) {
        if self.selected != id {
            self.invalidate();
            self.selected = id;
        }
    }
    pub const fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
    }
    pub const fn target(&self) -> Option<ModsEditTarget> {
        let individual = self.settings.individual_mods_enabled;
        if individual && self.selected.is_none() {
            return None;
        }
        Some(ModsEditTarget {
            audio_id: if individual { self.selected } else { None },
            epoch: self.epoch,
        })
    }
    pub fn stored(&self, id: i32, stored: Option<AudioModsProfile>) -> Option<AudioModsProfile> {
        self.overrides.get(&id).copied().unwrap_or(stored)
    }
    pub fn effective(&self, id: Option<i32>, stored: Option<AudioModsProfile>) -> AudioModsProfile {
        if self.settings.individual_mods_enabled {
            id.and_then(|id| self.stored(id, stored))
                .unwrap_or_default()
        } else {
            self.settings.global_profile
        }
    }
    pub const fn set_individual(&mut self, enabled: bool) -> Option<Change> {
        if self.settings.individual_mods_enabled == enabled {
            return None;
        }
        self.settings.individual_mods_enabled = enabled;
        self.invalidate();
        Some(Change::Settings(self.settings))
    }
    pub fn edit(
        &mut self,
        target: ModsEditTarget,
        edit: ModsEdit,
        stored: Option<AudioModsProfile>,
    ) -> Option<Change> {
        if self.target() != Some(target) {
            return None;
        }
        let old = self.effective(self.selected, stored);
        let profile = edited_profile(old, edit)?;
        if let Some(id) = target.audio_id {
            let profile = (profile.mode != AudioModsMode::None).then_some(profile);
            if self.stored(id, stored) == profile {
                return None;
            }
            self.overrides.insert(id, profile);
            Some(Change::Track(id, profile))
        } else {
            if old == profile {
                return None;
            }
            self.settings.global_profile = profile;
            Some(Change::Settings(self.settings))
        }
    }
}
fn edited_profile(mut profile: AudioModsProfile, edit: ModsEdit) -> Option<AudioModsProfile> {
    use AudioModsMode::{Dt, Ht, None as NoMod};
    match edit {
        ModsEdit::CycleDt => {
            profile = match (profile.mode, profile.adjust_pitch) {
                (Dt, false) => AudioModsProfile {
                    adjust_pitch: true,
                    ..profile
                },
                (Dt, true) => AudioModsProfile::default(),
                _ => AudioModsProfile {
                    mode: Dt,
                    speed_percent: 150,
                    adjust_pitch: false,
                },
            };
        }
        ModsEdit::ToggleHt => {
            profile = if profile.mode == Ht {
                AudioModsProfile::default()
            } else {
                AudioModsProfile {
                    mode: Ht,
                    speed_percent: 75,
                    adjust_pitch: false,
                }
            };
        }
        ModsEdit::SetSpeed(speed) => {
            let valid = match profile.mode {
                Dt => (101..=200).contains(&speed),
                Ht => (50..=99).contains(&speed),
                NoMod => false,
            };
            if !valid {
                return None;
            }
            profile.speed_percent = speed;
        }
        ModsEdit::SetAdjustPitch(pitch) => {
            if profile.mode == NoMod {
                return None;
            }
            profile.adjust_pitch = pitch;
        }
        ModsEdit::Clear => profile = AudioModsProfile::default(),
    }
    Some(profile)
}
#[derive(Clone)]
pub(crate) struct Save {
    pub change: Change,
    pub deadline: Instant,
}
#[derive(Default)]
pub(crate) struct ModsWork {
    pub editor: Editor,
    pub loaded: bool,
    pub loading: bool,
    pub error: String,
    pub pending: VecDeque<Save>,
    pub saving: Option<Save>,
    pub save_task: Option<JoinHandle<Result<(), String>>>,
    pub load_request: u64,
    pub revision: u64,
}
impl SettingsViewModel {
    /// Individual absence is always 1x. Local writes (including DELETE) win over projections.
    #[must_use]
    pub fn effective_mods(
        &self,
        audio_id: Option<i32>,
        stored: Option<AudioModsProfile>,
    ) -> AudioModsProfile {
        self.mods.editor.effective(audio_id, stored)
    }
    /// Profile whose speed scales the selected track's displayed duration.
    #[must_use]
    pub fn selected_mods(&self, track: Option<&super::Track>) -> AudioModsProfile {
        self.effective_mods(
            track.map(|track| track.audio_source_id),
            track.and_then(|track| track.mods),
        )
    }
    #[must_use]
    pub const fn mods_loaded(&self) -> bool {
        self.mods.loaded
    }

    #[must_use]
    pub fn assignment_mods(&self, assignment: &PlaybackAssignment) -> AudioModsProfile {
        self.effective_mods(assignment.current_audio_source_id, assignment.mods)
    }
    pub(crate) fn emit_mods(&mut self, input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        let id = Self::selected_volume_id(input);
        self.mods.editor.select(id);
        let stored = id.and_then(|id| input.selection.track(id).and_then(|track| track.mods));
        work.emit(AppUpdate::AudioMods(AudioModsState {
            settings: self.mods.editor.settings,
            loaded: self.mods.loaded,
            loading: self.mods.loading,
            saving: self.mods.saving.is_some() || !self.mods.pending.is_empty(),
            error: self.mods.error.clone(),
            target: self
                .mods
                .loaded
                .then(|| self.mods.editor.target())
                .flatten(),
            selected_profile: self.effective_mods(id, stored),
            selected_has_override: id
                .and_then(|id| self.mods.editor.stored(id, stored))
                .is_some(),
        }));
    }
    pub(crate) fn mods_action(
        &mut self,
        action: ModsAction,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.mods.editor.select(Self::selected_volume_id(input));
        if action == ModsAction::Retry {
            if self.mods.loaded {
                self.mods.error.clear();
                self.finish_mods_editing();
                self.emit_mods(input, work);
            } else {
                self.load_mods(input, work);
            }
            return;
        }
        if !self.mods.loaded {
            return;
        }
        let change = match action {
            ModsAction::SetIndividual(enabled) => {
                if self.mods.editor.settings.individual_mods_enabled != enabled {
                    self.finish_mods_editing();
                }
                self.mods.editor.set_individual(enabled)
            }
            ModsAction::Edit { target, edit } => {
                let stored = target
                    .audio_id
                    .and_then(|id| input.selection.track(id).and_then(|t| t.mods));
                self.mods.editor.edit(target, edit, stored)
            }
            ModsAction::Finish(target) => {
                if self.mods.editor.target() == Some(target) {
                    self.finish_mods_editing();
                    self.emit_mods(input, work);
                }
                return;
            }
            ModsAction::Retry => return,
        };
        if let Some(change) = change {
            self.mods.revision = self.mods.revision.wrapping_add(1);
            let save = Save {
                change,
                deadline: Instant::now()
                    .checked_add(Duration::from_millis(200))
                    .unwrap_or_else(Instant::now),
            };
            if let Some(pending) = self
                .mods
                .pending
                .iter_mut()
                .find(|p| p.change.same_target(&save.change))
            {
                *pending = save;
            } else {
                self.mods.pending.push_back(save);
            }
            self.mods.error.clear();
            self.emit_mods(input, work);
        }
    }
    pub(crate) fn load_mods(&mut self, input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        if self.mods.loading || self.mods.loaded {
            return;
        }
        let Some(api) = work.api.clone() else {
            return;
        };
        self.mods.loading = true;
        self.mods.error.clear();
        self.mods.load_request = self.mods.load_request.wrapping_add(1);
        let request = self.mods.load_request;
        let revision = self.mods.revision;
        self.emit_mods(input, work);
        work.task(async move {
            Completed::AudioMods {
                request,
                revision,
                result: api.audio_mods().await.map_err(|e| crate::describe(&e)),
            }
        });
    }
    pub(crate) fn mods_loaded_result(
        &mut self,
        request: u64,
        revision: u64,
        result: Result<AudioModsSettings, String>,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if request != self.mods.load_request || !self.mods.loading {
            return;
        }
        self.mods.loading = false;
        match result {
            Ok(settings) => {
                if revision == self.mods.revision {
                    self.mods.editor.settings = settings;
                }
                self.mods.loaded = true;
                self.mods.error.clear();
                self.release_playback_gate(work);
            }
            Err(error) => self.mods.error = format!("Failed to load mods: {error}"),
        }
        self.emit_mods(input, work);
    }
    pub(crate) fn mods_deadline(&self) -> Option<Instant> {
        if self.mods.save_task.is_some() || !self.mods.error.is_empty() {
            return None;
        }
        self.mods.pending.front().map(|save| save.deadline)
    }
    pub(crate) fn start_mods_save(&mut self, input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        if self
            .mods_deadline()
            .is_none_or(|deadline| deadline > Instant::now())
        {
            return;
        }
        let Some(api) = work.api.clone() else {
            return;
        };
        let Some(save) = self.mods.pending.pop_front() else {
            return;
        };
        let change = save.change.clone();
        self.mods.saving = Some(save);
        self.mods.save_task = Some(tokio::spawn(async move {
            match change {
                Change::Settings(settings) => api
                    .update_audio_mods(&AudioModsPatch {
                        individual_mods_enabled: Some(settings.individual_mods_enabled),
                        global_mode: Some(settings.global_profile.mode),
                        global_speed_percent: Some(settings.global_profile.speed_percent),
                        global_adjust_pitch: Some(settings.global_profile.adjust_pitch),
                    })
                    .await
                    .map(|_| ()),
                Change::Track(id, profile) => api.set_audio_mods(id, profile).await,
            }
            .map_err(|e| crate::describe(&e))
        }));
        self.emit_mods(input, work);
    }
    pub(crate) fn mods_saved(
        &mut self,
        result: Result<(), String>,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.mods.save_task = None;
        let Some(save) = self.mods.saving.take() else {
            return;
        };
        if let Err(error) = result
            && !self
                .mods
                .pending
                .iter()
                .any(|p| p.change.same_target(&save.change))
        {
            self.mods.pending.push_front(save);
            self.mods.error = format!("Failed to save mods: {error}");
        }
        // ACKs never install profiles: a newer edit or DELETE may already be visible.
        self.emit_mods(input, work);
    }
    pub(crate) fn finish_mods_editing(&mut self) {
        self.mods.editor.invalidate();
        for pending in &mut self.mods.pending {
            pending.deadline = Instant::now();
        }
    }
}

#[cfg(test)]
mod tests;
