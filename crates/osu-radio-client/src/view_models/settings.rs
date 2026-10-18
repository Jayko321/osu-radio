//! Settings state stays usable without a server; adapters own storage I/O only.
use super::folders::{FolderEvent, Scan};
use super::volume::{Change, Save};
use super::{
    AudioSettingsState, FolderAction, FolderSelection, FolderSelectionRow, Track,
    TrackNamePreferences, TrackSort, folders::SelectionWork, volume::VolumeWork,
};
use crate::controller::workflow::{Effect, Workflow};
use crate::controller::{AppUpdate, Completed, OperationStatus, SettingsRetry, failure};
use crate::models::{AudioSettings, PlaybackAssignment};
use crate::{DiscoverFolders, DiscoveryDepth, FolderDiscoveryEvent, RegisterOsuFolder, describe};
use crate::{Loading, OsuFolder};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingsPreferences {
    pub name_preferences: TrackNamePreferences,
    pub discord_enabled: bool,
    pub unicode_error: String,
    pub discord_error: String,
}
#[derive(Default)]
pub struct SettingsViewModel {
    pub(crate) preferences: SettingsPreferences,
    pub(crate) sort: TrackSort,
    pub(crate) volume: VolumeWork,
    pub(crate) selection: SelectionWork,
    pub(crate) folders: Vec<OsuFolder>,
    pub(crate) selected_folder: Option<i32>,
    pub(crate) status: OperationStatus,
    pub(crate) picking: bool,
}
impl SettingsViewModel {
    #[must_use]
    pub const fn preferences(&self) -> &SettingsPreferences {
        &self.preferences
    }
    #[must_use]
    pub fn folders(&self) -> &[OsuFolder] {
        &self.folders
    }
    #[must_use]
    pub const fn selected_folder_id(&self) -> Option<i32> {
        self.selected_folder
    }
    pub(crate) fn set_name_preferences(&mut self, preferences: TrackNamePreferences) -> bool {
        if self.preferences.name_preferences == preferences {
            return false;
        }
        self.preferences.name_preferences = preferences;
        self.preferences.unicode_error.clear();
        true
    }
    pub(crate) fn set_discord_enabled(&mut self, enabled: bool) {
        self.preferences.discord_enabled = enabled;
        self.preferences.discord_error.clear();
    }
    pub(crate) fn report_storage_error(&mut self, group: SettingsGroup, message: String) {
        match group {
            SettingsGroup::Unicode => self.preferences.unicode_error = message,
            SettingsGroup::Discord => self.preferences.discord_error = message,
        }
    }
    pub(crate) fn replace_folder_rows(&mut self, folders: Vec<OsuFolder>) {
        self.selected_folder = self
            .selected_folder
            .filter(|id| folders.iter().any(|folder| folder.id == *id))
            .or_else(|| folders.first().map(|folder| folder.id));
        self.folders = folders;
    }
    pub(crate) fn select_folder(&mut self, id: Option<i32>) -> bool {
        if id.is_some() && !self.folders.iter().any(|folder| Some(folder.id) == id) {
            return false;
        }
        self.selected_folder = id;
        true
    }
}

impl SettingsViewModel {
    pub(crate) fn cancel_selection(&mut self) {
        for scan in self.selection.scans.drain().map(|(_, scan)| scan) {
            scan.task.abort();
        }
        for task in self.selection.counts.drain().map(|(_, task)| task) {
            task.abort();
        }
        self.selection.queue.clear();
        self.selection.epoch = self.selection.epoch.wrapping_add(1);
    }
}

impl SettingsViewModel {
    pub(crate) fn merge_candidate(
        &mut self,
        kind: String,
        root_path: String,
        marker_path: String,
        registered_id: Option<i32>,
    ) {
        if let Some(row) = self.selection.state.rows.iter_mut().find(|row| {
            row.marker_path == marker_path
                || registered_id.is_some() && row.registered_id == registered_id
        }) {
            // The server resolves aliases. Replace the seeded marker and restart its preview once.
            if row.marker_path != marker_path {
                if let Some(task) = self.selection.counts.remove(&row.marker_path) {
                    task.abort();
                }
                self.selection.queue.retain(|path| *path != row.marker_path);
                row.marker_path.clone_from(&marker_path);
                row.root_path = root_path;
                row.count = Loading::Pending;
                self.selection.queue.push_back(marker_path);
            }
            if registered_id.is_some() {
                row.registered_id = registered_id;
                if row.pending == Some(FolderAction::Add) {
                    row.pending = None;
                }
            }
            return;
        }
        self.selection.queue.push_back(marker_path.clone());
        self.selection.state.rows.push(FolderSelectionRow {
            kind,
            root_path,
            marker_path,
            registered_id,
            pending: None,
            count: Loading::Pending,
            error: None,
        });
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SettingsGroup {
    Unicode,
    Discord,
}

impl SettingsViewModel {
    pub(crate) fn emit_selection_state(&self, _input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        work.emit(AppUpdate::FolderSelection(self.selection.state.clone()));
    }
    pub(crate) fn open_selection(&mut self, input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        if work.api.is_none() || self.selection.state.open {
            return;
        }
        self.cancel_selection();
        self.selection.state = FolderSelection {
            open: true,
            ..Default::default()
        };
        self.reconcile_selection(input, work);
        self.discover_selection(DiscoverFolders::default(), false, input, work);
    }
    pub(crate) fn close_selection(&mut self, input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        if self.selection.state.applying {
            return;
        }
        self.cancel_selection();
        self.selection.state = FolderSelection::default();
        self.emit_selection_state(input, work);
    }
    pub(crate) fn reconcile_selection(
        &mut self,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if !self.selection.state.open {
            return;
        }
        for folder in self.folders.clone() {
            if let Some(row) = self
                .selection
                .state
                .rows
                .iter_mut()
                .find(|row| row.registered_id == Some(folder.id))
            {
                if row.pending == Some(FolderAction::Add) {
                    row.pending = None;
                }
            } else {
                self.merge_candidate(
                    folder.kind,
                    folder.root_path,
                    folder.marker_path,
                    Some(folder.id),
                );
            }
        }
        self.emit_selection_state(input, work);
    }
    pub(crate) fn discover_selection(
        &mut self,
        request: DiscoverFolders,
        scoped: bool,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let Some(api) = work.api.clone() else {
            return;
        };
        self.selection.scan = self.selection.scan.wrapping_add(1);
        let scan = self.selection.scan;
        let epoch = self.selection.epoch;
        let sender = input.folder_sender.clone();
        let task = work.task(async move {
            let result = api
                .discover_osu_folders(&request, |event| {
                    let _ = sender.send(FolderEvent::Candidate { epoch, scan, event });
                })
                .await
                .map_err(|error| describe(&error));
            // Preserve channel ordering: all candidates must arrive before completion.
            let _ = sender.send(FolderEvent::Discovered {
                epoch,
                scan,
                result,
            });
            Completed::Cancelled
        });
        self.selection.scans.insert(
            scan,
            Scan {
                task,
                scoped,
                markers: Vec::new(),
            },
        );
        self.selection.state.discovering = true;
        self.emit_selection_state(input, work);
    }
    pub(crate) fn browse_selection(&mut self, input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        let state = &mut self.selection.state;
        if !state.open || state.applying || state.picking {
            return;
        }
        state.picking = true;
        self.emit_selection_state(input, work);
        work.emit(AppUpdate::FolderSelectionPickerRequested(
            self.selection.epoch,
        ));
    }
    pub(crate) fn selection_picked(
        &mut self,
        epoch: u64,
        path: Option<PathBuf>,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if epoch != self.selection.epoch
            || !self.selection.state.open
            || !self.selection.state.picking
        {
            return;
        }
        self.selection.state.picking = false;
        if let Some(path) = path {
            self.selection.state.message.clear();
            self.discover_selection(
                DiscoverFolders {
                    roots: vec![path],
                    depth: DiscoveryDepth::Known,
                },
                true,
                input,
                work,
            );
        } else {
            self.emit_selection_state(input, work);
        }
    }
    pub(crate) fn toggle_selection(
        &mut self,
        path: &str,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.stage_selection(path, false, input, work);
    }
    pub(crate) fn refresh_selection(
        &mut self,
        path: &str,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.stage_selection(path, true, input, work);
    }
    pub(crate) fn stage_selection(
        &mut self,
        path: &str,
        refresh: bool,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if !self.selection.state.open || self.selection.state.applying {
            return;
        }
        if let Some(row) = self
            .selection
            .state
            .rows
            .iter_mut()
            .find(|row| row.marker_path == path)
        {
            let action = if row.registered_id.is_some() {
                if refresh {
                    FolderAction::Refresh
                } else {
                    FolderAction::Remove
                }
            } else if refresh {
                return;
            } else {
                FolderAction::Add
            };
            row.pending = if row.pending == Some(action) {
                None
            } else {
                Some(action)
            };
            row.error = None;
            self.emit_selection_state(input, work);
        }
    }
    pub(crate) fn retry_count(
        &mut self,
        path: &str,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if !self.selection.state.open || self.selection.state.applying {
            return;
        }
        if let Some(row) = self
            .selection
            .state
            .rows
            .iter_mut()
            .find(|row| row.marker_path == path && matches!(row.count, Loading::Failed(_)))
        {
            row.count = Loading::Pending;
            self.selection.queue.push_back(path.into());
            self.emit_selection_state(input, work);
        }
    }
    pub(crate) fn start_counts(&mut self, _input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        let Some(api) = work.api.clone() else {
            return;
        };
        while self.selection.state.open && self.selection.counts.len() < 2 {
            let Some(path) = self.selection.queue.pop_front() else {
                break;
            };
            let api = api.clone();
            let epoch = self.selection.epoch;
            let key = path.clone();
            let task = work.task(async move {
                Completed::FolderSelection(FolderEvent::Count {
                    epoch,
                    path: path.clone(),
                    result: api
                        .osu_folder_metadata(&path)
                        .await
                        .map(|metadata| metadata.beatmap_count)
                        .map_err(|error| describe(&error)),
                })
            });
            self.selection.counts.insert(key, task);
        }
    }
    pub(crate) fn apply_selection(&mut self, input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        if !self.selection.state.open
            || self.selection.state.applying
            || self.selection.state.picking
        {
            return;
        }
        let Some(api) = work.api.clone() else {
            return;
        };
        let mut actions: Vec<_> = self
            .selection
            .state
            .rows
            .iter()
            .filter_map(|row| {
                row.pending
                    .map(|action| (action, row.marker_path.clone(), row.registered_id))
            })
            .collect();
        actions.sort_by_key(|(action, _, _)| *action == FolderAction::Remove);
        self.selection.changed = false;
        self.selection.state.applying = true;
        self.selection.state.message.clear();
        self.emit_selection_state(input, work);
        let epoch = self.selection.epoch;
        let sender = input.folder_sender.clone();
        work.task(async move {
            for (action, path, id) in actions {
                let result = match action {
                    FolderAction::Add => api.import_osu_folder(&path).await.map(Some),
                    FolderAction::Refresh => {
                        if let Some(id) = id {
                            api.reimport_osu_folder(id).await.map(Some)
                        } else {
                            Err(crate::ApiError::Protocol(
                                "Missing registered folder ID.".into(),
                            ))
                        }
                    }
                    FolderAction::Remove => {
                        if let Some(id) = id {
                            api.remove_osu_folder(id).await.map(|()| None)
                        } else {
                            Err(crate::ApiError::Protocol(
                                "Missing registered folder ID.".into(),
                            ))
                        }
                    }
                }
                .map_err(|error| describe(&error));
                let _ = sender.send(FolderEvent::Applied {
                    epoch,
                    path,
                    result,
                });
            }
            let _ = sender.send(FolderEvent::ApplyDone { epoch });
            Completed::Cancelled
        });
    }
    pub(crate) fn selection_applied(
        &mut self,
        path: &str,
        result: Result<Option<OsuFolder>, String>,
        _input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if let Some(row) = self
            .selection
            .state
            .rows
            .iter_mut()
            .find(|row| row.marker_path == path)
        {
            match result {
                Ok(folder) => {
                    self.selection.changed = true;
                    row.pending = None;
                    row.error = None;
                    if let Some(folder) = folder {
                        row.registered_id = Some(folder.id);
                        if let Some(stored) = self
                            .folders
                            .iter_mut()
                            .find(|stored| stored.id == folder.id)
                        {
                            *stored = folder;
                        } else {
                            self.folders.push(folder);
                        }
                    } else {
                        let id = row.registered_id.take();
                        self.folders.retain(|folder| Some(folder.id) != id);
                    }
                    work.emit(AppUpdate::FoldersReplaced(self.folders.clone()));
                }
                Err(error) => row.error = Some(error),
            }
        }
    }
    pub(crate) fn selection_event(
        &mut self,
        event: FolderEvent,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if event.epoch() != self.selection.epoch || !self.selection.state.open {
            return;
        }
        match event {
            FolderEvent::Candidate {
                scan,
                event:
                    FolderDiscoveryEvent::Candidate {
                        kind,
                        root_path,
                        marker_path,
                        registered_id,
                    },
                ..
            } => {
                let Some(scan) = self.selection.scans.get_mut(&scan) else {
                    return;
                };
                if !scan.markers.contains(&marker_path) {
                    scan.markers.push(marker_path.clone());
                }
                // Discovery's registration snapshot can predate an Apply removal.
                let registered_id =
                    registered_id.filter(|id| self.folders.iter().any(|folder| folder.id == *id));
                self.merge_candidate(kind, root_path, marker_path, registered_id);
            }
            FolderEvent::Candidate { .. } => return,
            FolderEvent::Discovered { scan, result, .. } => {
                let Some(scan) = self.selection.scans.remove(&scan) else {
                    return;
                };
                self.selection.state.discovering = !self.selection.scans.is_empty();
                match result {
                    Err(error) => self.selection.state.message = error,
                    Ok(()) if scan.scoped => {
                        self.selection.state.message = match scan.markers.len() {
                            0 => "No osu! installation found in the selected folder.".into(),
                            1 => {
                                if !self.selection.state.applying
                                    && let Some(row) =
                                        self.selection.state.rows.iter_mut().find(|row| {
                                            Some(&row.marker_path) == scan.markers.first()
                                                && row.registered_id.is_none()
                                        })
                                {
                                    row.pending = Some(FolderAction::Add);
                                    row.error = None;
                                }
                                String::new()
                            }
                            _ => "Multiple installations found. Select each folder to add.".into(),
                        };
                    }
                    Ok(()) => {}
                }
            }
            FolderEvent::Count { path, result, .. } => {
                self.selection.counts.remove(&path);
                if let Some(row) = self
                    .selection
                    .state
                    .rows
                    .iter_mut()
                    .find(|row| row.marker_path == path)
                {
                    row.count = match result {
                        Ok(count) => Loading::Ready(count),
                        Err(error) => Loading::Failed(error),
                    };
                }
            }
            FolderEvent::Applied { path, result, .. } => {
                self.selection_applied(&path, result, input, work);
            }
            FolderEvent::ApplyDone { .. } => {
                self.selection.state.applying = false;
                if self.selection.changed {
                    self.refresh_folders(input, work);
                    work.effect(Effect::RefreshLibrary);
                    work.effect(Effect::RefreshPlaylists);
                }
                if self
                    .selection
                    .state
                    .rows
                    .iter()
                    .all(|row| row.pending.is_none())
                {
                    self.close_selection(input, work);
                    return;
                }
                self.selection.state.message = "Some changes failed. Successful changes are saved; Apply retries the remaining rows.".into();
            }
        }
        self.emit_selection_state(input, work);
    }
    pub(crate) fn refresh_folders(&mut self, _input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        let Some(api) = work.api.clone() else {
            work.effect(Effect::Connect);
            return;
        };
        if self.status.loading || self.status.busy {
            return;
        }
        self.status = OperationStatus {
            loading: true,
            message: "Loading folders…".into(),
            ..Default::default()
        };
        work.emit(AppUpdate::FolderStatus(self.status.clone()));
        work.task(async move {
            Completed::Folders(api.osu_folders().await.map_err(|error| describe(&error)))
        });
    }
    pub(crate) fn begin_pick(&mut self, _input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        if work.api.is_none() || self.status.loading || self.status.busy {
            return;
        }
        self.picking = true;
        self.status.busy = true;
        work.emit(AppUpdate::FolderStatus(self.status.clone()));
        work.emit(AppUpdate::FolderPickerRequested);
    }
    pub(crate) fn complete_pick(
        &mut self,
        path: Option<PathBuf>,
        _input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if !self.picking {
            return;
        }
        self.picking = false;
        self.status.busy = false;
        let Some(path) = path else {
            work.emit(AppUpdate::FolderStatus(self.status.clone()));
            return;
        };
        let Some(api) = work.api.clone() else {
            return;
        };
        self.status = OperationStatus {
            busy: true,
            message: "Adding osu! folder…".into(),
            ..Default::default()
        };
        work.emit(AppUpdate::FolderStatus(self.status.clone()));
        work.task(async move {
            Completed::Registered(
                api.register_osu_folder(&RegisterOsuFolder::new(path))
                    .await
                    .map(crate::RegisteredFolder::into_folder)
                    .map_err(|error| describe(&error)),
            )
        });
    }
    pub(crate) fn replace_folders(
        &mut self,
        folders: Vec<OsuFolder>,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.replace_folder_rows(folders);
        self.reconcile_selection(input, work);
        work.emit(AppUpdate::FoldersReplaced(self.folders.clone()));
        work.emit(AppUpdate::FolderSelected(self.selected_folder));
    }
    pub(crate) fn load_audio_settings(
        &mut self,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if self.volume.loading || self.volume.settings.is_some() {
            return;
        }
        let Some(api) = work.api.clone() else {
            return;
        };
        self.volume.loading = true;
        self.volume.error.clear();
        self.emit_volume(input, work);
        work.task(async move {
            Completed::AudioSettings(api.audio_settings().await.map_err(|error| describe(&error)))
        });
    }
    pub(crate) fn audio_settings_loaded(
        &mut self,
        result: Result<AudioSettings, String>,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.volume.loading = false;
        match result {
            Ok(settings) => {
                self.volume.settings = Some(settings);
                self.volume.error.clear();
                if let Some(assignment) = self.volume.deferred_assignment.take() {
                    work.effect(Effect::Assignment(assignment));
                }
                if !self.volume.stream_started {
                    self.volume.stream_started = true;
                    work.effect(Effect::StartPlaybackStream);
                }
            }
            Err(error) => {
                self.volume.error = format!("Failed to load volume settings: {error}");
            }
        }
        self.emit_volume(input, work);
    }
    pub(crate) fn emit_volume(&self, input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        let id = Self::selected_volume_id(input);
        let stored = id.and_then(|id| self.track_override(id, input));
        work.emit(AppUpdate::AudioSettings(AudioSettingsState {
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
    pub(crate) fn enqueue_volume_save(
        &mut self,
        change: Change,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
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
        work.effect(Effect::CurrentVolume);
        self.emit_volume(input, work);
    }
    pub(crate) fn set_individual_volume(
        &mut self,
        enabled: bool,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let Some(mut settings) = self.volume.settings else {
            return;
        };
        if settings.individual_volume_enabled == enabled {
            return;
        }
        settings.individual_volume_enabled = enabled;
        self.volume.settings = Some(settings);
        self.enqueue_volume_save(Change::Settings(settings), input, work);
    }
    pub(crate) fn set_global_volume(
        &mut self,
        percent: u8,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let Some(mut settings) = self.volume.settings else {
            return;
        };
        if percent > 100 || settings.global_volume_percent == percent {
            return;
        }
        settings.global_volume_percent = percent;
        self.volume.settings = Some(settings);
        self.enqueue_volume_save(Change::Settings(settings), input, work);
    }
    pub(crate) fn set_selected_volume(
        &mut self,
        volume: f32,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
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
            if let Some(id) = Self::selected_volume_id(input) {
                self.set_track_volume(id, Some(percent), input, work);
            }
        } else {
            self.set_global_volume(percent, input, work);
        }
    }
    pub(crate) fn set_track_volume(
        &mut self,
        id: i32,
        percent: Option<u8>,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if !self
            .volume
            .settings
            .is_some_and(|s| s.individual_volume_enabled)
            || percent.is_some_and(|p| p > 100)
            || Self::selected_volume_id(input) != Some(id)
        {
            return;
        }
        self.save_track_volume(id, percent, input, work);
    }
    pub(crate) fn set_current_volume(
        &mut self,
        volume: f64,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return;
        }
        let Some(settings) = self.volume.settings else {
            return;
        };
        #[allow(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        // Validated finite 0..1, rounded to 0..100 before any state change.
        let percent = (volume * 100.0).round() as u8;
        if settings.individual_volume_enabled {
            if let Some(id) = input
                .selection
                .player
                .assignment
                .as_ref()
                .and_then(|assignment| assignment.current_audio_source_id)
                .filter(|id| *id >= 0)
            {
                self.save_track_volume(id, Some(percent), input, work);
            }
        } else {
            self.set_global_volume(percent, input, work);
        }
    }
    fn save_track_volume(
        &mut self,
        id: i32,
        percent: Option<u8>,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.volume.overrides.insert(id, percent);
        work.effect(Effect::TrackVolumeChanged {
            audio_id: id,
            percent,
        });
        self.enqueue_volume_save(Change::Track(id, percent), input, work);
    }
    pub(crate) fn retry_audio_settings(
        &mut self,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if self.volume.settings.is_none() {
            self.load_audio_settings(input, work);
        } else {
            self.volume.error.clear();
            self.finish_volume_editing();
            self.emit_volume(input, work);
        }
    }
    pub(crate) fn start_volume_save(&mut self, input: &SettingsInput<'_>, work: &mut Workflow<'_>) {
        if self
            .volume_deadline()
            .is_none_or(|deadline| deadline > Instant::now())
        {
            return;
        }
        let Some(api) = work.api.clone() else {
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
        self.emit_volume(input, work);
    }
    pub(crate) fn volume_saved(
        &mut self,
        result: Result<(), String>,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
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
        self.emit_volume(input, work);
    }
    pub(crate) fn selected_volume_id(input: &SettingsInput<'_>) -> Option<i32> {
        if input.selection.playlists.view.showing_detail() {
            input
                .selection
                .playlists
                .view
                .selected_item()
                .and_then(|item| item.audio_source_id)
        } else {
            input
                .selection
                .library
                .selected
                .filter(|id| *id >= 0 && input.selection.track(*id).is_some())
        }
    }
    pub(crate) fn track_override(&self, id: i32, input: &SettingsInput<'_>) -> Option<u8> {
        self.volume.overrides.get(&id).copied().unwrap_or_else(|| {
            input
                .selection
                .track(id)
                .and_then(|track| track.volume_percent)
        })
    }
    pub(crate) fn effective_percent(&self, id: Option<i32>, stored: Option<u8>) -> u8 {
        let settings = self.volume.settings.unwrap_or_default();
        if settings.individual_volume_enabled {
            id.and_then(|id| self.volume.overrides.get(&id).copied().unwrap_or(stored))
                .unwrap_or(settings.global_volume_percent)
        } else {
            settings.global_volume_percent
        }
    }
    pub(crate) fn assignment_volume(&self, input: &SettingsInput<'_>) -> f32 {
        f32::from(
            self.effective_percent(
                input
                    .selection
                    .player
                    .assignment
                    .as_ref()
                    .and_then(|a| a.current_audio_source_id),
                input
                    .selection
                    .player
                    .current_track
                    .as_ref()
                    .and_then(|t| t.volume_percent),
            ),
        ) / 100.0
    }
    pub(crate) fn merge_volume_overrides(&self, tracks: &mut [Track]) {
        for track in tracks {
            if let Some(percent) = self.volume.overrides.get(&track.audio_source_id) {
                track.volume_percent = *percent;
            }
        }
    }
    pub(crate) fn begin_volume_flush(&mut self) {
        self.volume.error.clear();
        self.finish_volume_editing();
    }
    pub(crate) const fn take_volume_save_task(
        &mut self,
    ) -> Option<tokio::task::JoinHandle<Result<(), String>>> {
        self.volume.save_task.take()
    }
    pub(crate) fn skip_failed_volume_save(&mut self) -> Option<String> {
        if self.volume.error.is_empty() {
            return None;
        }
        // Attempt every independent target once at close, even if one final write fails.
        self.volume.pending.pop_front();
        Some(std::mem::take(&mut self.volume.error))
    }
    pub(crate) fn finish_volume_editing(&mut self) {
        for pending in &mut self.volume.pending {
            pending.deadline = Instant::now();
        }
    }
    pub(crate) fn volume_deadline(&self) -> Option<Instant> {
        if self.volume.save_task.is_some() || !self.volume.error.is_empty() {
            return None;
        }
        self.volume.pending.front().map(|save| save.deadline)
    }
}

pub(crate) struct SettingsInput<'a> {
    pub(crate) selection: crate::controller::media::MediaInput<'a>,
    pub(crate) folder_sender: &'a tokio::sync::mpsc::UnboundedSender<super::folders::FolderEvent>,
}

impl SettingsViewModel {
    pub(crate) fn folders_loaded(
        &mut self,
        result: Result<Vec<OsuFolder>, String>,
        input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.status = OperationStatus::default();
        match result {
            Ok(folders) => self.replace_folders(folders, input, work),
            Err(error) => self.status = failure(error, SettingsRetry::Load),
        }
        work.emit(AppUpdate::FolderStatus(self.status.clone()));
    }
    pub(crate) fn folder_registered(
        &mut self,
        result: Result<OsuFolder, String>,
        _input: &SettingsInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.status = OperationStatus::default();
        match result {
            Ok(folder) => {
                self.selected_folder = Some(folder.id);
                if let Some(existing) = self.folders.iter_mut().find(|row| row.id == folder.id) {
                    *existing = folder;
                } else {
                    self.folders.push(folder);
                }
                work.emit(AppUpdate::FoldersReplaced(self.folders.clone()));
                work.emit(AppUpdate::FolderSelected(self.selected_folder));
            }
            Err(error) => self.status = failure(error, SettingsRetry::Browse),
        }
        work.emit(AppUpdate::FolderStatus(self.status.clone()));
    }
    pub(crate) fn defer_assignment(&mut self, assignment: PlaybackAssignment) {
        if self
            .volume
            .deferred_assignment
            .as_ref()
            .is_none_or(|old| assignment.revision > old.revision)
        {
            self.volume.deferred_assignment = Some(assignment);
        }
    }
    pub(crate) fn set_sort(&mut self, sort: TrackSort) -> bool {
        if self.sort == sort {
            return false;
        }
        self.sort = sort;
        true
    }
}

impl SettingsViewModel {
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
