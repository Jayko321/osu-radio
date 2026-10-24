//! Async orchestration and interactions between the owned view models.
use super::{
    AppUpdate, Arc, Completed, ConnectionStatus, Controller, Duration, LibraryViewModel,
    MediaScheduler, MediaTicket, OsuFolder, PathBuf, PlayerViewModel, PlaylistAction,
    QueueViewModel, Session, SettingsViewModel, Track, TrackNamePreferences, TrackSort, describe,
    media, workflow,
};
#[cfg(test)]
use crate::models::Playlist;
use crate::models::{AudioSettings, PlaybackAssignment, PlaybackCommand, PlaybackMode};
use crate::view_models::folders::FolderEvent;
use crate::view_models::playlists::PlaylistEvent;
use crate::view_models::queue::{PendingPlayback, PlaylistPlayback};
use crate::view_models::{library, player, playlists, queue, settings};
use tokio::time::Instant;
impl Controller {
    pub(crate) fn system_player_event(
        &mut self,
        event: osu_radio_media_controls::Event,
        snapshot: &osu_radio_media_controls::Snapshot,
    ) {
        self.player_work(|player, input, work| player.system_event(event, snapshot, input, work));
    }

    pub(crate) fn set_current_volume(&mut self, volume: f64) {
        self.settings_work(|settings, input, work| {
            settings.set_current_volume(volume, input, work);
        });
    }

    pub(crate) fn folders_loaded(&mut self, result: Result<Vec<OsuFolder>, String>) {
        self.settings_work(|settings, input, work| settings.folders_loaded(result, input, work));
    }
    pub(crate) fn folder_registered(&mut self, result: Result<OsuFolder, String>) {
        self.settings_work(|settings, input, work| settings.folder_registered(result, input, work));
    }

    fn library_work(
        &mut self,
        operation: impl FnOnce(
            &mut LibraryViewModel,
            &library::LibraryInput<'_>,
            &mut workflow::Workflow<'_>,
        ),
    ) {
        let input = library::LibraryInput {
            player: &self.player,
            settings: &self.settings,
            media: &self.media,
        };
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        operation(&mut self.library, &input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }
    pub(crate) fn library_loaded(
        &mut self,
        request: u64,
        invalidate_artwork: bool,
        result: Result<Vec<Track>, String>,
    ) {
        self.library_work(|library, input, work| {
            library.loaded(request, invalidate_artwork, result, input, work);
        });
    }

    fn assign_worker(&mut self) {
        let Some(assignment) = &self.player.assignment else {
            return;
        };
        let (id, token, mode) = (
            assignment.current_audio_source_id,
            assignment.playback_token,
            assignment.mode,
        );
        let entry_id = assignment.current_entry_id;
        let waiting_for_check = assignment.waiting_for_check;
        if self.ensure_playback() {
            if let Some(worker) = &self.playback {
                worker.send(crate::playback::Command::Assign {
                    generation: worker.generation(),
                    id,
                    playback_token: token,
                    entry_id,
                    waiting_for_check,
                    mode,
                    volume: self.assignment_volume(),
                    parameters: self.assignment_parameters(),
                });
            }
        } else if mode == PlaybackMode::Playing && !waiting_for_check {
            self.player.device_failed(token);
        }
    }

    fn player_work(
        &mut self,
        operation: impl FnOnce(
            &mut PlayerViewModel,
            &player::PlayerInput<'_>,
            &mut workflow::Workflow<'_>,
        ),
    ) {
        let input = player::PlayerInput {
            library: &self.library,
            playlists: &self.playlists,
            settings: &self.settings,
            media: &self.media,
            assignment_sender: &self.assignment_sender,
        };
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        operation(&mut self.player, &input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }

    fn queue_work(
        &mut self,
        operation: impl FnOnce(&mut QueueViewModel, &queue::QueueInput<'_>, &mut workflow::Workflow<'_>),
    ) {
        let input = queue::QueueInput {
            selection: media::MediaInput {
                library: &self.library,
                playlists: &self.playlists,
                player: &self.player,
                upcoming: None,
            },
            preferences: self.settings.preferences.name_preferences,
            durations: &self.media.durations,
        };
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        operation(&mut self.upcoming, &input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }

    pub(crate) const fn settings_input(&self) -> settings::SettingsInput<'_> {
        settings::SettingsInput {
            selection: media::MediaInput {
                library: &self.library,
                playlists: &self.playlists,
                player: &self.player,
                upcoming: Some(&self.upcoming),
            },
            folder_sender: &self.folder_sender,
        }
    }
    fn settings_work(
        &mut self,
        operation: impl FnOnce(
            &mut SettingsViewModel,
            &settings::SettingsInput<'_>,
            &mut workflow::Workflow<'_>,
        ),
    ) {
        let input = settings::SettingsInput {
            selection: media::MediaInput {
                library: &self.library,
                playlists: &self.playlists,
                player: &self.player,
                upcoming: Some(&self.upcoming),
            },
            folder_sender: &self.folder_sender,
        };
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        operation(&mut self.settings, &input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }
    pub(crate) fn cancel_selection(&mut self) {
        self.settings.cancel_selection();
    }
    #[cfg(test)]
    pub(crate) fn merge_candidate(
        &mut self,
        kind: String,
        root_path: String,
        marker_path: String,
        registered_id: Option<i32>,
    ) {
        self.settings
            .merge_candidate(kind, root_path, marker_path, registered_id);
    }

    fn duration_changed(&mut self, audio_id: i32, duration: Option<Duration>) {
        self.library.set_duration(audio_id, duration);
        self.playlists.set_duration(audio_id, duration);
        self.upcoming.set_duration(audio_id, duration);
        self.player.set_duration(audio_id, duration);
        if let Some(track) = self
            .library
            .tracks
            .iter()
            .find(|track| track.audio_source_id == audio_id)
        {
            (self.emit)(AppUpdate::LibraryTrackChanged(track.clone()));
        }
        for row in self
            .playlists
            .tracks
            .iter()
            .filter(|row| row.track.audio_source_id == audio_id)
        {
            (self.emit)(AppUpdate::PlaylistTrackChanged(row.clone()));
        }
        if self
            .player
            .selected_track
            .as_ref()
            .is_some_and(|track| track.audio_source_id == audio_id)
        {
            self.emit_selection();
        }
        if self.upcoming.open {
            self.emit_queue();
        }
    }

    fn media_work(
        &mut self,
        operation: impl FnOnce(&mut MediaScheduler, &media::MediaInput<'_>, &mut workflow::Workflow<'_>),
    ) {
        self.media.sync_system_artwork(self.system_artwork_key());
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        let input = media::MediaInput {
            library: &self.library,
            playlists: &self.playlists,
            player: &self.player,
            upcoming: Some(&self.upcoming),
        };
        operation(&mut self.media, &input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }

    pub(crate) fn apply_effects(&mut self, effects: Vec<workflow::Effect>) {
        for effect in effects {
            match effect {
                workflow::Effect::Update(update) => (self.emit)(update),
                workflow::Effect::DurationChanged { audio_id, duration } => {
                    self.duration_changed(audio_id, duration);
                }
                workflow::Effect::Connect => self.connect(),
                workflow::Effect::RefreshLibrary => self.refresh_library(),
                workflow::Effect::RefreshPlaylists => self.refresh_playlist_views(),
                workflow::Effect::StartPlaybackStream => self.start_playback_stream(),
                workflow::Effect::Assignment(assignment) => self.apply_assignment(assignment),
                workflow::Effect::CurrentVolume => {
                    if let Some(worker) = &self.playback {
                        worker.send(crate::playback::Command::SetVolume(
                            self.assignment_volume(),
                        ));
                    }
                }
                workflow::Effect::TrackVolumeChanged { audio_id, percent } => {
                    self.library.set_track_volume(audio_id, percent);
                    self.playlists.set_track_volume(audio_id, percent);
                    self.player.set_track_volume(audio_id, percent);
                }
                workflow::Effect::QueueAction(action) => {
                    self.player.commands.push_back(PendingPlayback {
                        command: PlaybackCommand::Play {
                            audio_source_id: None,
                        },
                        playlist: None,
                        queue: Some(action),
                        expected_token: None,
                    });
                }
                workflow::Effect::QueueClosed => self.media.close_queue_demand(),
                workflow::Effect::QueueChanged => {
                    self.reconcile_media();
                }
                workflow::Effect::ReconcileMedia => self.reconcile_media(),
                workflow::Effect::DeferAssignment(assignment) => {
                    self.settings.defer_assignment(assignment);
                }
                workflow::Effect::AssignmentHistory => self.assignment_last_played(),
                workflow::Effect::SelectPlayingTrack {
                    audio_id,
                    playlist_item_id,
                } => {
                    if self.playlists.view.showing_detail() {
                        if let Some(item_id) = playlist_item_id
                            && self.playlists.select_item(item_id)
                        {
                            self.emit_playlists();
                        }
                    } else {
                        self.library.select(audio_id, true);
                        (self.emit)(AppUpdate::LibrarySelected(audio_id));
                    }
                }
                workflow::Effect::RememberDuration { audio_id, duration } => {
                    self.media.remember_duration(audio_id, duration);
                }
                workflow::Effect::ResetDiscord => self.discord.update(None, &self.player.playback),
                workflow::Effect::UpdateDiscord => self.update_discord(),
                workflow::Effect::RefreshQueue => self.refresh_queue(),
                workflow::Effect::CancelAudio => self.cancel_audio(),
                workflow::Effect::AssignWorker => self.assign_worker(),
                workflow::Effect::Download(download) => self.download(download),
                workflow::Effect::PlayerCommand(command) => self.player_command(command),
                workflow::Effect::RefreshOpenPlaylist => {
                    if self.playlists.view.active_id.is_some() {
                        self.playlist_action(PlaylistAction::Refresh);
                    }
                }
                workflow::Effect::Selection => self.emit_selection(),
                workflow::Effect::FinishVolume => self.finish_volume_editing(),
                workflow::Effect::AdvanceMedia(invalidate) => self.advance_media(invalidate),
                workflow::Effect::PlaylistPlayback(playlist) => self.queue_playlist(playlist),
                workflow::Effect::TogglePlayback => self.toggle_playback(),
                workflow::Effect::RememberDates(dates) => self.library.remember_dates(dates),
            }
        }
    }

    pub(crate) fn emit_player(&self) {
        let detail = self.playlists.view.showing_detail();
        let mut presentation = self.player.presentation(
            detail
                .then_some(self.playlists.view.selected_item_id)
                .flatten(),
            !detail || self.playlists.view.selected_item().is_some(),
            self.session.is_some(),
            self.settings.playback_ready(),
            self.settings
                .selected_mods(self.player.selected_track.as_ref()),
        );
        if let Some(error) = &self.player.transport_error {
            presentation.message.clone_from(error);
        }
        (self.emit)(AppUpdate::Player(presentation));
    }
    pub(crate) fn toggle_playback(&mut self) {
        self.player_work(crate::view_models::player::PlayerViewModel::toggle_playback);
    }

    pub(crate) fn set_name_preferences(&mut self, preferences: TrackNamePreferences) {
        if !self.settings.set_name_preferences(preferences) {
            return;
        }
        (self.emit)(AppUpdate::SettingsPreferences(
            self.settings.preferences.clone(),
        ));
        self.library.apply_preferences(preferences);
        self.playlists.apply_preferences(preferences);
        self.upcoming.apply_preferences(preferences);
        self.player.apply_preferences(preferences);
        self.update_discord();
        self.reorder_tracks();
        self.emit_queue();
        if self.playlists.view.showing_detail() && self.playlists.view.active.is_none() {
            // Loading retains the old caption; adapters keep the selection unavailable.
            (self.emit)(AppUpdate::TrackSelected(self.player.selected_track.clone()));
            self.emit_player();
        } else {
            self.emit_selection();
        }
    }

    pub(crate) fn assignment_last_played(&mut self) {
        self.player.merge_current_last_played(&mut self.library);
        let library_changed = self.library.apply_known_dates();
        let playlist_changed = self.playlists.apply_known_dates(&self.library.last_played);
        if library_changed || playlist_changed {
            self.reorder_tracks();
        }
    }

    pub(crate) fn set_track_sort(&mut self, sort: TrackSort) {
        if !self.settings.set_sort(sort) {
            return;
        }
        (self.emit)(AppUpdate::TrackSort(sort));
        self.reorder_tracks();
        self.emit_selection();
    }

    pub(crate) fn reorder_tracks(&mut self) {
        self.library.reorder(self.settings.sort);
        self.playlists.reorder(self.settings.sort);
        (self.emit)(AppUpdate::LibraryTracksReordered(
            self.library.tracks.clone(),
        ));
        self.emit_playlists();
        (self.emit)(AppUpdate::PlaylistTracksReordered(
            self.playlists.tracks.clone(),
        ));
    }

    pub(crate) async fn flush_volume_saves(&mut self) {
        self.settings.begin_volume_flush();
        loop {
            self.start_volume_save();
            let Some(task) = self.settings.take_volume_save_task() else {
                break;
            };
            self.volume_saved(task.await.unwrap_or_else(|error| Err(error.to_string())));
            if let Some(error) = self.settings.skip_failed_volume_save() {
                eprintln!("{error}");
            }
        }
    }

    pub(crate) fn update_discord(&mut self) {
        let track = self.player.current_track.as_ref().filter(|_| {
            self.player.assignment.as_ref().is_some_and(|assignment| {
                assignment.playback_token == self.player.playback.playback_token
                    && assignment.current_entry_id == self.player.playback.current_entry_id
                    && !assignment.waiting_for_check
                    && assignment.current_audio_source_id == self.player.playback.current_audio_id
            })
        });
        self.discord.update(track, &self.player.playback);
    }

    pub(crate) fn ensure_playback(&mut self) -> bool {
        if self.playback.is_none() {
            let sender = self.worker_sender.clone();
            match crate::playback::Worker::spawn(Arc::new(move |state| {
                let _ = sender.send(state);
            })) {
                Ok((worker, downloads)) => {
                    self.playback = Some(worker);
                    self.downloads = Some(downloads);
                }
                Err(error) => {
                    self.worker_start_failed(error.to_string());
                    return false;
                }
            }
        }
        true
    }
    pub(crate) fn worker_start_failed(&mut self, error: String) {
        self.player_work(|player, input, work| player.worker_start_failed(error, input, work));
    }
    pub(crate) fn player_command(&mut self, command: crate::playback::Command) {
        if self.ensure_playback()
            && let Some(worker) = &self.playback
        {
            worker.send(command);
        }
    }
    pub(crate) fn cancel_audio(&mut self) {
        if let Some(worker) = &self.playback {
            worker.invalidate();
        }
        if let Some(task) = self.audio_task.take() {
            task.abort();
        }
    }
    pub(crate) fn download(&mut self, request: crate::playback::Download) {
        let Some(worker) = &self.playback else {
            return;
        };
        if !worker.is_current(request.generation) {
            return;
        }
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            worker.send(crate::playback::Command::Loaded {
                generation: request.generation,
                id: request.id,
                result: Err("Connect to the server before playing a track.".into()),
            });
            return;
        };
        let gate = self.audio_gate.clone();
        self.audio_task = Some(self.task(async move {
            let Ok(_permit) = gate.acquire_owned().await else {
                return Completed::Cancelled;
            };
            Completed::Audio {
                generation: request.generation,
                id: request.id,
                result: api
                    .download_audio(request.id)
                    .await
                    .map_err(|error| describe(&error)),
            }
        }));
    }
    pub(crate) fn connect(&mut self) {
        if self.connecting || self.session.is_some() {
            return;
        }
        self.connecting = true;
        (self.emit)(AppUpdate::Connection(ConnectionStatus::Connecting));
        self.library.connection_pending();
        self.settings.connection_pending();
        self.emit_statuses();
        let options = self.options.clone();
        let cancel = self.cancel.subscribe();
        self.tasks.spawn(async move {
            Completed::Connected(
                Session::start_cancellable(options, cancel)
                    .await
                    .map_err(|error| describe(&error)),
            )
        });
    }
    pub(crate) fn refresh_library(&mut self) {
        self.load_library(Duration::ZERO, true);
    }

    pub(crate) fn emit_selection(&mut self) {
        // Detail loading has no replacement selection yet; retain the displayed track.
        if self.playlists.view.showing_detail() && self.playlists.view.active.is_none() {
            self.emit_player();
            self.reconcile_media();
            return;
        }
        self.emit_volume();
        self.settings_work(SettingsViewModel::emit_mods);
        let track = if self.playlists.view.showing_detail() {
            self.playlists.selected_track().cloned()
        } else {
            self.library.selected.and_then(|id| self.track(id)).cloned()
        };
        self.media
            .selection_changed(self.player.selected_track.as_ref(), track.as_ref());
        self.player.select_track(track.clone());
        (self.emit)(AppUpdate::TrackSelected(track));
        self.emit_player();
        self.reconcile_media();
    }
    pub(crate) fn emit_statuses(&self) {
        (self.emit)(AppUpdate::LibraryStatus(self.library.status.clone()));
        (self.emit)(AppUpdate::FolderStatus(self.settings.status.clone()));
    }
    pub(crate) fn track(&self, id: i32) -> Option<&Track> {
        if self.playlists.view.showing_detail()
            && let Some(track) = self
                .playlists
                .selected_track()
                .filter(|track| track.audio_source_id == id)
                .or_else(|| {
                    self.playlists
                        .tracks
                        .iter()
                        .find(|row| row.track.audio_source_id == id)
                        .map(|row| &row.track)
                })
        {
            return Some(track);
        }
        self.player
            .current_track
            .as_ref()
            .filter(|track| track.audio_source_id == id)
            .or_else(|| {
                self.library
                    .indices
                    .get(&id)
                    .and_then(|index| self.library.tracks.get(*index))
            })
            .or_else(|| {
                self.upcoming
                    .view
                    .entries
                    .iter()
                    .find(|row| row.audio_source_id == Some(id))
                    .map(|row| &row.track)
            })
            .or_else(|| {
                self.player
                    .selected_track
                    .as_ref()
                    .filter(|track| track.audio_source_id == id)
            })
    }

    pub(crate) fn advance_media(&mut self, invalidate_artwork: bool) {
        self.media
            .advance(invalidate_artwork, self.player.selected_track.is_some());
        (self.emit)(AppUpdate::MediaGeneration {
            generation: self.media.generation,
            invalidate_artwork,
        });
    }

    #[cfg(test)]
    pub(crate) fn media_completed(
        &mut self,
        ticket: MediaTicket,
        bytes: Option<Vec<u8>>,
        duration: Option<u64>,
        duration_requested: bool,
    ) {
        self.artwork_completed(ticket, bytes);
        if duration_requested {
            self.duration_completed(ticket, duration);
        }
    }
    pub(crate) fn playlist_action(&mut self, action: PlaylistAction) {
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        let input = playlists::PlaylistInput {
            library: &self.library,
            preferences: self.settings.preferences.name_preferences,
            sort: self.settings.sort,
            durations: &self.media.durations,
            last_played: &self.library.last_played,
            overrides: &self.settings.volume.overrides,
        };
        self.playlists.playlist_action(action, &input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }
    pub(crate) fn start_playlist_images(&mut self) {
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        let input = playlists::PlaylistInput {
            library: &self.library,
            preferences: self.settings.preferences.name_preferences,
            sort: self.settings.sort,
            durations: &self.media.durations,
            last_played: &self.library.last_played,
            overrides: &self.settings.volume.overrides,
        };
        self.playlists.start_playlist_images(&input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }
    pub(crate) fn refresh_playlist_views(&mut self) {
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        let input = playlists::PlaylistInput {
            library: &self.library,
            preferences: self.settings.preferences.name_preferences,
            sort: self.settings.sort,
            durations: &self.media.durations,
            last_played: &self.library.last_played,
            overrides: &self.settings.volume.overrides,
        };
        self.playlists.refresh_playlist_views(&input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }
    pub(crate) fn refresh_playlists(&mut self) {
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        let input = playlists::PlaylistInput {
            library: &self.library,
            preferences: self.settings.preferences.name_preferences,
            sort: self.settings.sort,
            durations: &self.media.durations,
            last_played: &self.library.last_played,
            overrides: &self.settings.volume.overrides,
        };
        self.playlists.refresh_playlists(&input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }

    pub(crate) fn playlist_event(&mut self, event: PlaylistEvent) {
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        let input = playlists::PlaylistInput {
            library: &self.library,
            preferences: self.settings.preferences.name_preferences,
            sort: self.settings.sort,
            durations: &self.media.durations,
            last_played: &self.library.last_played,
            overrides: &self.settings.volume.overrides,
        };
        self.playlists.playlist_event(event, &input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }
    #[cfg(test)]
    pub(crate) fn install_playlist(&mut self, playlist: Playlist) {
        let mut work = workflow::Workflow {
            api: self.session.as_ref().map(|session| session.api().clone()),
            tasks: &mut self.tasks,
            cancel: &self.cancel,
            effects: Vec::new(),
        };
        let input = playlists::PlaylistInput {
            library: &self.library,
            preferences: self.settings.preferences.name_preferences,
            sort: self.settings.sort,
            durations: &self.media.durations,
            last_played: &self.library.last_played,
            overrides: &self.settings.volume.overrides,
        };
        self.playlists.install_playlist(playlist, &input, &mut work);
        let effects = work.effects;
        self.apply_effects(effects);
    }
    pub(crate) fn emit_playlists(&self) {
        (self.emit)(AppUpdate::Playlists(self.playlists.view.clone()));
    }
    pub(crate) fn set_visible_queue_media(
        &mut self,
        visible: Vec<(crate::models::QueueEntryId, bool)>,
    ) {
        self.media_work(|media, input, work| media.set_visible_queue_media(visible, input, work));
    }
    pub(crate) fn queue_action(&mut self, action: crate::view_models::QueueAction) {
        self.queue_work(|queue, input, work| queue.action(action, input, work));
    }
    pub(crate) fn set_visible_media(&mut self, visible: Vec<(i32, bool)>) {
        self.media_work(|media, input, work| media.set_visible_media(visible, input, work));
    }
    pub(crate) fn reconcile_media(&mut self) {
        self.media_work(super::media::MediaScheduler::reconcile_media);
    }
    pub(crate) fn request_media(&mut self, id: i32, missing: bool) {
        self.media_work(|media, input, work| media.request_media(id, missing, input, work));
    }

    pub(crate) fn start_media(&mut self) {
        self.media_work(|media, input, work| {
            media.reconcile_media(input, work);
            media.start_media(input, work);
        });
    }
    pub(crate) fn artwork_completed(&mut self, ticket: MediaTicket, bytes: Option<Vec<u8>>) {
        self.media_work(|media, input, work| media.artwork_completed(ticket, bytes, input, work));
    }
    pub(crate) fn duration_completed(&mut self, ticket: MediaTicket, duration: Option<u64>) {
        self.media_work(|media, input, work| {
            media.duration_completed(ticket, duration, input, work);
        });
    }
    pub(crate) fn open_selection(&mut self) {
        self.settings_work(crate::view_models::settings::SettingsViewModel::open_selection);
    }
    pub(crate) fn close_selection(&mut self) {
        self.settings_work(crate::view_models::settings::SettingsViewModel::close_selection);
    }

    pub(crate) fn browse_selection(&mut self) {
        self.settings_work(crate::view_models::settings::SettingsViewModel::browse_selection);
    }
    pub(crate) fn selection_picked(&mut self, epoch: u64, path: Option<PathBuf>) {
        self.settings_work(|settings, input, work| {
            settings.selection_picked(epoch, path, input, work);
        });
    }
    pub(crate) fn toggle_selection(&mut self, path: &str) {
        self.settings_work(|settings, input, work| settings.toggle_selection(path, input, work));
    }
    pub(crate) fn refresh_selection(&mut self, path: &str) {
        self.settings_work(|settings, input, work| settings.refresh_selection(path, input, work));
    }
    pub(crate) fn retry_count(&mut self, path: &str) {
        self.settings_work(|settings, input, work| settings.retry_count(path, input, work));
    }
    pub(crate) fn start_counts(&mut self) {
        self.settings_work(crate::view_models::settings::SettingsViewModel::start_counts);
    }
    pub(crate) fn apply_selection(&mut self) {
        self.settings_work(crate::view_models::settings::SettingsViewModel::apply_selection);
    }
    pub(crate) fn selection_event(&mut self, event: FolderEvent) {
        self.settings_work(|settings, input, work| settings.selection_event(event, input, work));
    }
    pub(crate) fn refresh_folders(&mut self) {
        self.settings_work(crate::view_models::settings::SettingsViewModel::refresh_folders);
    }
    pub(crate) fn begin_pick(&mut self) {
        self.settings_work(crate::view_models::settings::SettingsViewModel::begin_pick);
    }
    pub(crate) fn complete_pick(&mut self, path: Option<PathBuf>) {
        self.settings_work(|settings, input, work| settings.complete_pick(path, input, work));
    }
    #[cfg(test)]
    pub(crate) fn replace_folders(&mut self, folders: Vec<OsuFolder>) {
        self.settings_work(|settings, input, work| settings.replace_folders(folders, input, work));
    }
    pub(crate) fn load_audio_settings(&mut self) {
        self.settings_work(crate::view_models::settings::SettingsViewModel::load_audio_settings);
    }
    pub(crate) fn audio_settings_loaded(&mut self, result: Result<AudioSettings, String>) {
        self.settings_work(|settings, input, work| {
            settings.audio_settings_loaded(result, input, work);
        });
    }
    pub(crate) fn emit_volume(&mut self) {
        self.settings_work(|settings, input, work| settings.emit_volume(input, work));
    }
    pub(crate) fn set_individual_volume(&mut self, enabled: bool) {
        self.settings_work(|settings, input, work| {
            settings.set_individual_volume(enabled, input, work);
        });
    }
    pub(crate) fn set_global_volume(&mut self, percent: u8) {
        self.settings_work(|settings, input, work| {
            settings.set_global_volume(percent, input, work);
        });
    }
    pub(crate) fn set_selected_volume(&mut self, volume: f32) {
        self.settings_work(|settings, input, work| {
            settings.set_selected_volume(volume, input, work);
        });
    }
    pub(crate) fn set_track_volume(&mut self, id: i32, percent: Option<u8>) {
        self.settings_work(|settings, input, work| {
            settings.set_track_volume(id, percent, input, work);
        });
    }
    pub(crate) fn retry_audio_settings(&mut self) {
        self.settings_work(crate::view_models::settings::SettingsViewModel::retry_audio_settings);
    }
    pub(crate) fn start_volume_save(&mut self) {
        self.settings_work(crate::view_models::settings::SettingsViewModel::start_volume_save);
    }
    pub(crate) fn volume_saved(&mut self, result: Result<(), String>) {
        self.settings_work(|settings, input, work| settings.volume_saved(result, input, work));
    }
    #[cfg(test)]
    pub(crate) fn selected_volume_id(&self) -> Option<i32> {
        SettingsViewModel::selected_volume_id(&self.settings_input())
    }
    pub(crate) fn assignment_parameters(&self) -> crate::playback::PlaybackParameters {
        self.player
            .assignment
            .as_ref()
            .map_or_else(Default::default, |a| {
                self.settings.assignment_mods(a).into()
            })
    }
    pub(crate) fn assignment_volume(&self) -> f32 {
        self.settings.assignment_volume(&self.settings_input())
    }

    pub(crate) fn finish_volume_editing(&mut self) {
        self.settings.finish_volume_editing();
        self.settings.finish_mods_editing();
        self.settings_work(SettingsViewModel::emit_mods);
    }
    pub(crate) fn volume_deadline(&self) -> Option<Instant> {
        self.settings.volume_deadline()
    }
    pub(crate) fn emit_queue(&self) {
        (self.emit)(AppUpdate::Queue(self.upcoming.view.clone()));
    }
    pub(crate) fn set_queue_visible(&mut self, visible: bool) {
        self.queue_work(|queue, input, work| queue.set_queue_visible(visible, input, work));
    }
    pub(crate) fn refresh_queue(&mut self) {
        self.queue_work(crate::view_models::queue::QueueViewModel::refresh_queue);
    }
    pub(crate) fn queue_loaded(
        &mut self,
        request: u64,
        result: Result<crate::models::QueueState, String>,
    ) {
        self.queue_work(|queue, input, work| queue.queue_loaded(request, result, input, work));
    }
    pub(crate) fn queue_playback(&mut self, command: PlaybackCommand) {
        self.player_work(|player, input, work| player.queue_playback(command, input, work));
    }
    pub(crate) fn queue_playlist(&mut self, playlist: PlaylistPlayback) {
        if matches!(playlist, PlaylistPlayback::Append(_)) {
            let connected = self.session.is_some();
            self.queue_work(|queue, input, work| {
                queue.operation_status(
                    connected,
                    (!connected).then(|| "Connect to the server to queue a playlist.".into()),
                    input,
                    work,
                );
            });
            if !connected {
                return;
            }
        }
        self.player_work(|player, input, work| player.queue_playlist(playlist, input, work));
    }
    pub(crate) fn start_playback_command(&mut self) {
        self.player_work(crate::view_models::player::PlayerViewModel::start_playback_command);
    }
    pub(crate) fn playback_command_completed(
        &mut self,
        pending: PendingPlayback,
        result: Result<PlaybackAssignment, String>,
    ) {
        let queue_operation = pending.queue.is_some()
            || matches!(pending.playlist, Some(PlaylistPlayback::Append(_)));
        let previous_revision = self.player.assignment.as_ref().map(|state| state.revision);
        let success = result.is_ok();
        let operation_error = result.as_ref().err().cloned();
        self.player_work(|player, input, work| {
            player.playback_command_completed(pending, result, input, work);
        });
        if queue_operation {
            let busy = self.player.commands.iter().any(|pending| {
                pending.queue.is_some()
                    || matches!(pending.playlist, Some(PlaylistPlayback::Append(_)))
            });
            self.queue_work(|queue, input, work| {
                queue.operation_status(busy, operation_error, input, work);
            });
        }
        // Even a successful no-op must refresh the open panel.
        if success
            && previous_revision == self.player.assignment.as_ref().map(|state| state.revision)
        {
            self.refresh_queue();
        }
    }
    pub(crate) fn start_playback_stream(&mut self) {
        self.player_work(|_player, input, work| {
            crate::view_models::player::PlayerViewModel::start_playback_stream(input, work);
        });
    }
    pub(crate) fn assignment_result(&mut self, result: Result<PlaybackAssignment, String>) {
        self.player_work(|player, input, work| player.assignment_result(result, input, work));
    }
    pub(crate) fn apply_assignment(&mut self, assignment: PlaybackAssignment) {
        self.player_work(|player, input, work| player.apply_assignment(assignment, input, work));
    }
    pub(crate) fn worker_update(&mut self, playback: crate::playback::Playback) {
        self.player_work(|player, input, work| player.worker_update(playback, input, work));
    }
    pub(crate) fn worker_message(&mut self, message: crate::playback::Message) {
        self.player_work(|player, input, work| player.worker_message(message, input, work));
    }
    pub(crate) fn seek_selected(&mut self, position: Duration) {
        self.player_work(|player, input, work| player.seek_selected(position, input, work));
    }
    pub(crate) fn load_library(&mut self, delay: Duration, invalidate_artwork: bool) {
        self.library_work(|library, input, work| {
            library.load_library(delay, invalidate_artwork, input, work);
        });
    }
    #[cfg(test)]
    pub(crate) fn replace_tracks(&mut self, tracks: Vec<Track>, invalidate_artwork: bool) {
        self.library_work(|library, input, work| {
            library.replace_tracks(tracks, invalidate_artwork, input, work);
        });
    }
}

impl Controller {
    pub(crate) fn load_mods(&mut self) {
        self.settings_work(SettingsViewModel::load_mods);
    }
    pub(crate) fn mods_loaded_result(
        &mut self,
        request: u64,
        revision: u64,
        result: Result<crate::models::AudioModsSettings, String>,
    ) {
        self.settings_work(|settings, input, work| {
            settings.mods_loaded_result(request, revision, result, input, work);
        });
    }
    pub(crate) fn mods_action(&mut self, action: crate::view_models::ModsAction) {
        self.settings_work(|settings, input, work| settings.mods_action(action, input, work));
        // A recomputes from its own launch profile (same entry/token: no restart); B's scale follows its profile.
        if self.playback.is_some() {
            self.assign_worker();
        }
        self.emit_player();
    }
    pub(crate) fn start_mods_save(&mut self) {
        self.settings_work(SettingsViewModel::start_mods_save);
    }
    pub(crate) fn mods_saved(&mut self, result: Result<(), String>) {
        self.settings_work(|settings, input, work| settings.mods_saved(result, input, work));
    }
    pub(crate) async fn flush_mods_saves(&mut self) {
        self.settings.mods.error.clear();
        let mut errors = Vec::new();
        self.settings.finish_mods_editing();
        loop {
            self.start_mods_save();
            let Some(task) = self.settings.mods.save_task.take() else {
                break;
            };
            self.mods_saved(task.await.unwrap_or_else(|error| Err(error.to_string())));
            if !self.settings.mods.error.is_empty() {
                // Attempt each target once on shutdown; keep the final error visible.
                self.settings.mods.pending.pop_front();
                eprintln!("{}", self.settings.mods.error);
                errors.push(std::mem::take(&mut self.settings.mods.error));
            }
        }
        self.settings.mods.error = errors.join("\n");
        self.settings_work(SettingsViewModel::emit_mods);
    }
}
