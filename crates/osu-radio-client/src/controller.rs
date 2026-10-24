//! Shared application workflows. Adapters own presentation and decoded artwork, and acknowledge
//! every artwork delivery only after decoding and cache installation (or stale-result discard).
use crate::{OsuFolder, ServerOptions, Session, Track, TrackNamePreferences, describe};
use std::{future::Future, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    runtime::Handle,
    sync::{mpsc, oneshot, watch},
    task::{AbortHandle, JoinSet},
};
mod coordinator;
pub(crate) mod media;
mod system_media;
pub(crate) mod workflow;
use crate::view_models::{
    AudioSettingsState, FolderSelection, LibraryViewModel, PlayerPresentation, PlayerViewModel,
    PlaylistAction, PlaylistTrack, PlaylistsState, PlaylistsViewModel, QueueView, QueueViewModel,
    SettingsGroup, SettingsPreferences, SettingsViewModel, TrackSort, folders, playlists, queue,
};
#[cfg(test)]
pub(crate) use media::MediaJob;
use media::MediaScheduler;
#[cfg(test)]
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsRetry {
    Load,
    Browse,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OperationStatus {
    pub loading: bool,
    pub message: String,
    pub retry: Option<SettingsRetry>,
    pub busy: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MediaTicket {
    pub generation: u64,
    pub audio_id: i32,
    pub serial: u64,
}
#[derive(Debug)]
pub enum AppCommand {
    ConnectSystemMedia {
        context: osu_radio_media_controls::NativeContext,
        completed: oneshot::Sender<Result<(), osu_radio_media_controls::Error>>,
    },
    PublishSystemMedia,
    DisconnectSystemMedia {
        completed: oneshot::Sender<()>,
    },
    Queue(crate::view_models::QueueAction),
    Playlist(PlaylistAction),
    Connect,
    RefreshLibrary,
    SearchLibrary(String),
    SetTrackSort(TrackSort),
    SetTrackNamePreferences(TrackNamePreferences),
    SetDiscordRichPresence(bool),
    ReportSettingsError {
        group: SettingsGroup,
        message: String,
    },
    TogglePlayback,
    RefreshFolders,
    SelectTrack(Option<i32>),
    PlayTrack(i32),
    Pause,
    Resume,
    Stop,
    Next,
    Previous,
    SetQueueVisible(bool),
    Seek(Duration),
    SetVolume(f32),
    /// Session-only mute; never saved and never changes a volume setting.
    SetMuted(bool),
    SetIndividualVolumeEnabled(bool),
    SetGlobalVolume(u8),
    SetTrackVolume {
        audio_id: i32,
        volume_percent: Option<u8>,
    },
    RetryAudioSettings,
    Mods(crate::view_models::ModsAction),
    SelectFolder(Option<i32>),
    BeginFolderPick,
    CompleteFolderPick(Option<PathBuf>),
    RetryFolders,
    OpenFolderSelection,
    CloseFolderSelection,
    BrowseFolderSelection,
    CompleteFolderSelectionPick {
        epoch: u64,
        path: Option<PathBuf>,
    },
    ToggleFolderSelection(String),
    RefreshFolderSelection(String),
    RetryFolderCount(String),
    ApplyFolderSelection,
    RequestMedia {
        audio_id: i32,
        artwork_missing: bool,
    },
    SetVisibleMedia(Vec<(i32, bool)>),
    SetVisibleQueueMedia(Vec<(crate::models::QueueEntryId, bool)>),
    MediaInstalled {
        ticket: MediaTicket,
        available: bool,
    },
    Shutdown,
}
#[derive(Clone, Debug)]
pub enum AppUpdate {
    Queue(QueueView),
    AudioSettings(AudioSettingsState),
    AudioMods(crate::view_models::AudioModsState),
    Playlists(PlaylistsState),
    PlaylistCover {
        id: i32,
        revision: i64,
        bytes: Vec<u8>,
    },
    PlaylistArtwork {
        id: i32,
        beatmap_id: i32,
        bytes: Vec<u8>,
    },
    Playback(crate::playback::Playback),
    Connection(ConnectionStatus),
    LibraryStatus(OperationStatus),
    FolderStatus(OperationStatus),
    LibraryTracksReplaced {
        tracks: Vec<Track>,
        invalidate_artwork: bool,
    },
    LibraryTracksReordered(Vec<Track>),
    PlaylistTracksReplaced {
        tracks: Vec<PlaylistTrack>,
        invalidate_artwork: bool,
    },
    PlaylistTracksReordered(Vec<PlaylistTrack>),
    PlaylistTrackChanged(PlaylistTrack),
    MediaGeneration {
        generation: u64,
        invalidate_artwork: bool,
    },
    SettingsPreferences(SettingsPreferences),
    Player(PlayerPresentation),
    TrackSort(TrackSort),
    LibraryTrackChanged(Track),
    LibrarySelected(Option<i32>),
    TrackSelected(Option<Track>),
    FoldersReplaced(Vec<OsuFolder>),
    FolderSelected(Option<i32>),
    FolderPickerRequested,
    FolderSelection(FolderSelection),
    FolderSelectionPickerRequested(u64),
    Artwork {
        ticket: MediaTicket,
        cover_id: i32,
        bytes: Vec<u8>,
    },
}

/// Cloneable command endpoint. Keep a clone through GUI teardown and await `shutdown` before
/// dropping the runtime. Sending after shutdown is harmless; shutdown is idempotent.
#[derive(Clone)]
pub struct AppController {
    commands: mpsc::UnboundedSender<AppCommand>,
    stopped: watch::Receiver<bool>,
}
impl AppController {
    pub fn spawn(
        runtime: &Handle,
        options: ServerOptions,
        updates: impl Fn(AppUpdate) + Send + Sync + 'static,
    ) -> Self {
        let (commands, receiver) = mpsc::unbounded_channel();
        let (done, stopped) = watch::channel(false);
        runtime.spawn(async move {
            Controller::new(options, Arc::new(updates))
                .run(receiver)
                .await;
            let _ = done.send(true);
        });
        Self { commands, stopped }
    }
    pub fn send(&self, command: AppCommand) {
        let _ = self.commands.send(command);
    }
    pub async fn shutdown(&self) {
        self.send(AppCommand::Shutdown);
        let _ = self.stopped.clone().wait_for(|done| *done).await;
    }
    /// Starts native initialization with the current playback snapshot. Native failure is
    /// diagnosed asynchronously and disables this integration until application restart.
    pub async fn connect_system_media(
        &self,
        context: osu_radio_media_controls::NativeContext,
    ) -> Result<(), osu_radio_media_controls::Error> {
        let (completed, result) = oneshot::channel();
        self.send(AppCommand::ConnectSystemMedia { context, completed });
        result.await.unwrap_or_else(|_| {
            Err(osu_radio_media_controls::Error::new(
                "application controller stopped",
            ))
        })
    }
    /// Requests publication of the latest client-owned state; normal updates publish automatically.
    pub fn publish_system_media(&self) {
        self.send(AppCommand::PublishSystemMedia);
    }
    /// Keep the native window alive until this returns. Safe before or after ordinary shutdown.
    pub async fn disconnect_system_media(&self) {
        let (completed, result) = oneshot::channel();
        self.send(AppCommand::DisconnectSystemMedia { completed });
        // A closed controller also means its shutdown has completed native cleanup.
        if result.await.is_err() {
            let _ = self.stopped.clone().wait_for(|done| *done).await;
        }
    }
}

pub(crate) enum Completed {
    Queue {
        request: u64,
        result: Result<crate::models::QueueState, String>,
    },
    AudioSettings(Result<crate::models::AudioSettings, String>),
    AudioMods {
        request: u64,
        revision: u64,
        result: Result<crate::models::AudioModsSettings, String>,
    },
    Playlist(playlists::PlaylistEvent),
    PlaybackRetry(queue::PendingPlayback),
    PlaybackCommand {
        pending: queue::PendingPlayback,
        result: Result<crate::models::PlaybackAssignment, String>,
    },
    FolderSelection(folders::FolderEvent),
    Connected(Result<Session, String>),
    Library {
        request: u64,
        invalidate_artwork: bool,
        result: Result<Vec<Track>, String>,
    },
    Folders(Result<Vec<OsuFolder>, String>),
    Registered(Result<OsuFolder, String>),
    MediaArtwork {
        ticket: MediaTicket,
        bytes: Option<Vec<u8>>,
    },
    MediaDuration {
        ticket: MediaTicket,
        duration: Option<u64>,
    },
    Audio {
        generation: u64,
        id: i32,
        result: Result<tempfile::NamedTempFile, String>,
    },
    Cancelled,
}
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Controller {
    pub(crate) system_media: Option<osu_radio_media_controls::MediaControls>,
    system_events: Option<osu_radio_media_controls::EventReceiver>,
    system_media_failed: bool,
    pub(crate) discord: crate::discord::Service,
    pub(crate) library: LibraryViewModel,
    pub(crate) playlists: PlaylistsViewModel,
    pub(crate) settings: SettingsViewModel,
    pub(crate) player: PlayerViewModel,
    pub(crate) upcoming: QueueViewModel,
    pub(crate) media: MediaScheduler,
    folder_events: mpsc::UnboundedReceiver<folders::FolderEvent>,
    folder_sender: mpsc::UnboundedSender<folders::FolderEvent>,
    options: ServerOptions,
    emit: Arc<dyn Fn(AppUpdate) + Send + Sync>,
    pub(crate) session: Option<Session>,
    connecting: bool,
    pub(crate) tasks: JoinSet<Completed>,
    cancel: watch::Sender<bool>,
    pub(crate) assignments:
        mpsc::UnboundedReceiver<Result<crate::models::PlaybackAssignment, String>>,
    assignment_sender: mpsc::UnboundedSender<Result<crate::models::PlaybackAssignment, String>>,
    worker_updates: mpsc::UnboundedReceiver<crate::playback::Playback>,
    worker_sender: mpsc::UnboundedSender<crate::playback::Playback>,
    pub(crate) playback: Option<crate::playback::Worker>,
    pub(crate) downloads: Option<mpsc::UnboundedReceiver<crate::playback::Message>>,
    audio_task: Option<AbortHandle>,
    audio_gate: Arc<tokio::sync::Semaphore>,
}
impl Controller {
    pub(crate) fn new(options: ServerOptions, emit: Arc<dyn Fn(AppUpdate) + Send + Sync>) -> Self {
        let (cancel, _) = watch::channel(false);
        let (folder_sender, folder_events) = mpsc::unbounded_channel();
        let (assignment_sender, assignments) = mpsc::unbounded_channel();
        let (worker_sender, worker_updates) = mpsc::unbounded_channel();
        Self {
            system_media: None,
            system_events: None,
            system_media_failed: false,
            discord: crate::discord::Service::default(),
            library: LibraryViewModel::default(),
            playlists: PlaylistsViewModel::default(),
            settings: SettingsViewModel::default(),
            player: PlayerViewModel::default(),
            upcoming: QueueViewModel::default(),
            media: MediaScheduler::default(),
            folder_sender,
            folder_events,
            options,
            emit,
            session: None,
            connecting: false,
            tasks: JoinSet::new(),
            cancel,
            assignment_sender,
            assignments,
            worker_sender,
            worker_updates,
            playback: None,
            downloads: None,
            audio_task: None,
            audio_gate: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }

    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<AppCommand>) {
        loop {
            let volume_deadline = self.volume_deadline();
            let mods_deadline = self.settings.mods_deadline();
            tokio::select! {
                () = async {
                    match mods_deadline {
                        Some(deadline) => tokio::time::sleep_until(deadline).await,
                        None => std::future::pending().await,
                    }
                } => self.start_mods_save(),
                result = async {
                    match &mut self.settings.mods.save_task {
                        Some(task) => task.await.unwrap_or_else(|error| Err(error.to_string())),
                        None => std::future::pending().await,
                    }
                } => self.mods_saved(result),
                () = async {
                    match volume_deadline {
                        Some(deadline) => tokio::time::sleep_until(deadline).await,
                        None => std::future::pending().await,
                    }
                } => self.start_volume_save(),
                result = async {
                    match &mut self.settings.volume.save_task {
                        Some(task) => task.await.unwrap_or_else(|error| Err(error.to_string())),
                        None => std::future::pending().await,
                    }
                } => self.volume_saved(result),
                command = commands.recv() => match command {
                    Some(AppCommand::Shutdown) | None => break,
                    Some(AppCommand::DisconnectSystemMedia { completed }) => {
                        self.disconnect_system_media().await;
                        let _ = completed.send(());
                    }
                    Some(command) => self.command(command),
                },
                event = async {
                    match &mut self.system_events {
                        Some(events) => events.recv().await,
                        None => std::future::pending().await,
                    }
                } => match event {
                    Some(event) => self.system_media_event(event),
                    None => self.system_media_ended(),
                },
                Some(event) = self.folder_events.recv() => self.selection_event(event),
                Some(assignment) = self.assignments.recv() => self.assignment_result(assignment),
                Some(playback) = self.worker_updates.recv() => self.worker_update(playback),
                request = async {
                    match &mut self.downloads {
                        Some(downloads) => downloads.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    if let Some(request) = request { self.worker_message(request); }
                    else { self.downloads = None; }
                }
                result = self.tasks.join_next(), if !self.tasks.is_empty() => {
                    if let Some(Ok(result)) = result { self.complete(result); }
                }
            }
            self.start_playback_command();
            self.start_media();
            self.start_playlist_images();
            self.start_counts();
            self.publish_system_media();
        }
        self.disconnect_system_media().await;
        self.discord.begin_shutdown();
        self.cancel_audio();
        self.flush_volume_saves().await;
        self.flush_mods_saves().await;
        self.cancel_selection();
        let _ = self.cancel.send(true);
        if let Some(mut playback) = self.playback.take() {
            let _ = tokio::task::spawn_blocking(move || playback.shutdown()).await;
        }
        self.discord.shutdown().await;
        // Startup owns its child until it either installs a session or awaits cancellation cleanup.
        while let Some(result) = self.tasks.join_next().await {
            if let Ok(Completed::Connected(Ok(session))) = result {
                let _ = session.shutdown().await;
            }
        }
        if let Some(session) = self.session.take() {
            let _ = session.shutdown().await;
        }
    }
    fn task(&mut self, future: impl Future<Output = Completed> + Send + 'static) -> AbortHandle {
        let mut cancel = self.cancel.subscribe();
        self.tasks.spawn(async move {
            tokio::select! {
                result = future => result,
                _ = cancel.wait_for(|value| *value) => Completed::Cancelled,
            }
        })
    }
    #[allow(clippy::too_many_lines)] // Keep command-to-workflow routing in one place.
    pub(crate) fn command(&mut self, command: AppCommand) {
        match command {
            AppCommand::ConnectSystemMedia { context, completed } => {
                let result = self.connect_system_media(context);
                let _ = completed.send(result);
            }
            AppCommand::PublishSystemMedia => self.publish_system_media(),
            // Awaited by run so the window owner gets a cleanup acknowledgement.
            AppCommand::DisconnectSystemMedia { .. } | AppCommand::Shutdown => {}
            AppCommand::SetDiscordRichPresence(enabled) => {
                self.settings.set_discord_enabled(enabled);
                self.discord.enabled(enabled);
                (self.emit)(AppUpdate::SettingsPreferences(
                    self.settings.preferences.clone(),
                ));
            }
            AppCommand::ReportSettingsError { group, message } => {
                self.settings.report_storage_error(group, message);
                (self.emit)(AppUpdate::SettingsPreferences(
                    self.settings.preferences.clone(),
                ));
            }
            AppCommand::Queue(action) => self.queue_action(action),
            AppCommand::SetVisibleQueueMedia(visible) => self.set_visible_queue_media(visible),
            AppCommand::TogglePlayback => self.toggle_playback(),
            AppCommand::SetQueueVisible(visible) => self.set_queue_visible(visible),
            AppCommand::Playlist(action) => self.playlist_action(action),
            AppCommand::OpenFolderSelection => self.open_selection(),
            AppCommand::CloseFolderSelection => self.close_selection(),
            AppCommand::BrowseFolderSelection => self.browse_selection(),
            AppCommand::CompleteFolderSelectionPick { epoch, path } => {
                self.selection_picked(epoch, path);
            }
            AppCommand::ToggleFolderSelection(path) => self.toggle_selection(&path),
            AppCommand::RefreshFolderSelection(path) => self.refresh_selection(&path),
            AppCommand::RetryFolderCount(path) => self.retry_count(&path),
            AppCommand::ApplyFolderSelection => self.apply_selection(),
            AppCommand::PlayTrack(id) => {
                self.queue_playback(crate::models::PlaybackCommand::Play {
                    audio_source_id: Some(id),
                });
            }
            AppCommand::Pause => self.queue_playback(crate::models::PlaybackCommand::Pause),
            AppCommand::Resume => self.queue_playback(crate::models::PlaybackCommand::Play {
                audio_source_id: None,
            }),
            AppCommand::Stop => self.queue_playback(crate::models::PlaybackCommand::Stop),
            AppCommand::Next => self.queue_playback(crate::models::PlaybackCommand::Next),
            AppCommand::Previous => self.queue_playback(crate::models::PlaybackCommand::Previous),
            AppCommand::Seek(position) => self.seek_selected(position),
            AppCommand::SetVolume(volume) => {
                self.set_selected_volume(volume);
                self.set_muted(false);
            }
            AppCommand::SetMuted(muted) => self.set_muted(muted),
            AppCommand::SetIndividualVolumeEnabled(enabled) => self.set_individual_volume(enabled),
            AppCommand::SetGlobalVolume(percent) => self.set_global_volume(percent),
            AppCommand::SetTrackVolume {
                audio_id,
                volume_percent,
            } => {
                self.set_track_volume(audio_id, volume_percent);
                if volume_percent.is_some() {
                    self.set_muted(false);
                }
            }
            AppCommand::RetryAudioSettings => self.retry_audio_settings(),
            AppCommand::Mods(action) => self.mods_action(action),
            AppCommand::Connect => self.connect(),
            AppCommand::RefreshLibrary => self.refresh_library(),
            AppCommand::SetTrackSort(sort) => self.set_track_sort(sort),
            AppCommand::SetTrackNamePreferences(preferences) => {
                self.set_name_preferences(preferences);
            }
            AppCommand::SearchLibrary(query) => {
                if self.library.search(query) {
                    self.load_library(Duration::from_millis(200), false);
                }
            }
            AppCommand::RefreshFolders => self.refresh_folders(),
            AppCommand::SelectTrack(id) => {
                self.finish_volume_editing();
                let available = id.is_none_or(|id| self.track(id).is_some());
                if self.library.select(id, available) {
                    (self.emit)(AppUpdate::LibrarySelected(id));
                    self.emit_selection();
                    self.media.prioritize_selected(id);
                }
            }
            AppCommand::SelectFolder(id) => {
                if self.settings.select_folder(id) {
                    (self.emit)(AppUpdate::FolderSelected(id));
                }
            }
            AppCommand::BeginFolderPick => self.begin_pick(),
            AppCommand::CompleteFolderPick(path) => self.complete_pick(path),
            AppCommand::RetryFolders => match self.settings.status.retry {
                Some(SettingsRetry::Browse) => self.begin_pick(),
                _ => self.refresh_folders(),
            },
            AppCommand::RequestMedia {
                audio_id,
                artwork_missing,
            } => self.request_media(audio_id, artwork_missing),
            AppCommand::SetVisibleMedia(visible) => self.set_visible_media(visible),
            AppCommand::MediaInstalled { ticket, available } => self.media.installed(
                ticket,
                available,
                self.player
                    .selected_track
                    .as_ref()
                    .map(|track| track.audio_source_id),
                &self.upcoming.view,
            ),
        }
    }
    #[allow(clippy::too_many_lines)] // Keep completion routing and stale-result checks together.
    pub(crate) fn complete(&mut self, result: Completed) {
        match result {
            Completed::Queue { request, result } => self.queue_loaded(request, result),
            Completed::AudioSettings(result) => self.audio_settings_loaded(result),
            Completed::AudioMods {
                request,
                revision,
                result,
            } => self.mods_loaded_result(request, revision, result),
            Completed::Playlist(event) => self.playlist_event(event),
            Completed::PlaybackRetry(pending) => self.player.commands.push_front(pending),
            Completed::PlaybackCommand { pending, result } => {
                self.playback_command_completed(pending, result);
            }
            Completed::FolderSelection(event) => self.selection_event(event),
            Completed::Audio {
                generation,
                id,
                result,
            } => {
                if let Some(worker) = &self.playback
                    && worker.is_current(generation)
                {
                    self.audio_task = None;
                    worker.send(crate::playback::Command::Loaded {
                        generation,
                        id,
                        result,
                    });
                }
            }
            Completed::Connected(result) => {
                self.connecting = false;
                self.library.connection_finished();
                self.settings.connection_finished();
                match result {
                    Ok(session) => {
                        self.session = Some(session);
                        (self.emit)(AppUpdate::Connection(ConnectionStatus::Connected));
                        let delay = self.library.deadline.map_or(Duration::ZERO, |deadline| {
                            deadline.saturating_duration_since(tokio::time::Instant::now())
                        });
                        self.load_library(delay, true);
                        self.refresh_folders();
                        self.refresh_playlists();
                        self.load_audio_settings();
                        self.load_mods();
                        self.refresh_queue();
                    }
                    Err(error) => {
                        self.library.connection_failed(error.clone());
                        self.settings.connection_failed(error.clone());
                        (self.emit)(AppUpdate::Connection(ConnectionStatus::Failed(error)));
                        self.emit_statuses();
                    }
                }
            }
            Completed::Library {
                request,
                invalidate_artwork,
                result,
            } => {
                self.library_loaded(request, invalidate_artwork, result);
            }

            Completed::Folders(result) => self.folders_loaded(result),
            Completed::Registered(result) => self.folder_registered(result),
            Completed::MediaArtwork { ticket, bytes } => self.artwork_completed(ticket, bytes),
            Completed::MediaDuration { ticket, duration } => {
                self.duration_completed(ticket, duration);
            }
            Completed::Cancelled => {}
        }
    }
}

pub(crate) fn failure(message: String, retry: SettingsRetry) -> OperationStatus {
    OperationStatus {
        message,
        retry: Some(retry),
        ..Default::default()
    }
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    pub(crate) fn controller() -> (Controller, Arc<Mutex<Vec<AppUpdate>>>) {
        let updates = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&updates);
        (
            {
                let mut controller = Controller::new(
                    ServerOptions::default(),
                    Arc::new(move |update| sink.lock().unwrap().push(update)),
                );
                controller.settings.volume.settings = Some(crate::models::AudioSettings::default());
                controller.settings.mods.loaded = true;
                controller
            },
            updates,
        )
    }
    pub(crate) fn queue_row(track: Track, id: i64) -> crate::view_models::QueueRow {
        crate::view_models::QueueRow {
            id: crate::models::QueueEntryId::new(id).unwrap(),
            position: usize::try_from(id.saturating_sub(1)).unwrap(),
            audio_source_id: Some(track.audio_source_id),
            playlist_item_id: None,
            availability: crate::models::QueueAvailability::Available,
            track,
        }
    }
    pub(crate) fn track(id: i32) -> Track {
        Track {
            audio_source_id: id,
            last_played_at_ms: None,
            volume_percent: None,
            mods: None,
            cover_beatmap_id: Some(id.saturating_add(100)),
            title_original: Some(id.to_string()),
            title_unicode: None,
            artist_original: Some("Artist".into()),
            artist_unicode: None,
            subtitle_suffix: String::new(),
            title: id.to_string(),
            artist: "Artist".into(),
            subtitle: "Artist".into(),
            difficulties: Vec::new(),
            duration: None,
        }
    }
    fn folder(id: i32) -> OsuFolder {
        OsuFolder {
            id,
            kind: "lazer".into(),
            root_path: format!("/test/{id}"),
            marker_path: format!("/test/{id}/client.realm"),
            label: Some("stored label".into()),
            enabled: false,
            last_scanned_at: None,
        }
    }
    #[cfg(unix)]
    pub(crate) async fn test_session() -> (tempfile::TempDir, Session) {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("server");
        std::fs::write(
            &binary,
            "#!/bin/sh\necho 'listening on http://127.0.0.1:9'\nexec sleep 30\n",
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let session = Session::start(ServerOptions {
            binary: Some(binary),
            ..Default::default()
        })
        .await
        .unwrap();
        (directory, session)
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn retry_commands_reopen_picker_or_reload_failed_list() {
        let (_directory, session) = test_session().await;
        let (mut state, updates) = controller();
        state.session = Some(session);
        state.complete(Completed::Registered(Err("registration failed".into())));
        state.command(AppCommand::RetryFolders);
        assert!(state.settings.picking);
        assert!(state.settings.status.busy);
        assert!(matches!(
            updates.lock().unwrap().last(),
            Some(AppUpdate::FolderPickerRequested)
        ));
        state.command(AppCommand::CompleteFolderPick(None));
        assert!(!state.settings.status.busy);
        state.complete(Completed::Folders(Err("list failed".into())));
        state.command(AppCommand::RetryFolders);
        assert!(state.settings.status.loading);
        assert_eq!(state.tasks.len(), 1);
        state.command(AppCommand::BeginFolderPick);
        assert!(!state.settings.picking, "loading prevents another picker");
        state.tasks.abort_all();
        state.session.take().unwrap().shutdown().await.unwrap();
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn failed_refresh_resumes_visible_and_selected_media_queued_while_loading() {
        let (_directory, session) = test_session().await;
        let (mut state, _) = controller();
        state.session = Some(session);
        state.replace_tracks(vec![track(1), track(2)], true);
        state.command(AppCommand::RefreshLibrary);
        assert!(state.library.status.loading);
        state.command(AppCommand::RequestMedia {
            audio_id: 1,
            artwork_missing: true,
        });
        state.command(AppCommand::SelectTrack(Some(2)));
        state.command(AppCommand::RequestMedia {
            audio_id: 2,
            artwork_missing: true,
        });
        state.start_media();
        assert!(
            state.media.jobs.is_empty(),
            "refresh pauses new media pipelines"
        );
        assert_eq!(state.media.queue, VecDeque::from([(2, true), (1, true)]));
        state.complete(Completed::Library {
            request: state.library.request,
            invalidate_artwork: true,
            result: Err("refresh failed".into()),
        });
        assert_eq!(state.library.selected, Some(2));
        assert_eq!(state.library.tracks, vec![track(1), track(2)]);
        state.start_media();
        assert!(state.media.queue.is_empty());
        assert_eq!(state.media.jobs.len(), 2);
        assert_eq!(
            state.media.jobs.get(&1).unwrap().ticket.audio_id,
            2,
            "selection keeps priority"
        );
        assert_eq!(state.media.jobs.get(&2).unwrap().ticket.audio_id, 1);
        state.tasks.abort_all();
        state.session.take().unwrap().shutdown().await.unwrap();
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn search_debounces_cancels_and_ignores_late_success_and_error() {
        let (_directory, session) = test_session().await;
        let (mut state, updates) = controller();
        state.session = Some(session);
        state.replace_tracks(vec![track(1), track(2)], true);
        state.library.selected = Some(2);
        // Paused time lets us check the delay without relying on wall-clock scheduling.
        tokio::time::pause();
        state.command(AppCommand::SearchLibrary("r".into()));
        let old = state.library.request;
        let old_task = state.library.task.clone().unwrap();
        tokio::time::advance(Duration::from_millis(100)).await;
        state.command(AppCommand::SearchLibrary("roc".into()));
        tokio::task::yield_now().await;
        assert!(old_task.is_finished(), "superseded task is aborted");
        while state.tasks.try_join_next().is_some() {}
        tokio::time::advance(Duration::from_millis(199)).await;
        assert!(
            state.tasks.try_join_next().is_none(),
            "latest request still debouncing"
        );
        let notifications = updates.lock().unwrap().len();
        for result in [Ok(vec![track(99)]), Err("late failure".into())] {
            state.complete(Completed::Library {
                request: old,
                invalidate_artwork: true,
                result,
            });
            assert_eq!(state.library.tracks, vec![track(1), track(2)]);
            assert_eq!(state.library.selected, Some(2));
            assert!(state.library.status.loading);
            assert_eq!(updates.lock().unwrap().len(), notifications);
        }
        // At 200 ms the fake session's refused connection completes the current task.
        tokio::time::advance(Duration::from_millis(1)).await;
        let completed = state.tasks.join_next().await.unwrap().unwrap();
        state.complete(completed);
        assert!(!state.library.status.loading);
        assert_eq!(state.library.status.retry, Some(SettingsRetry::Load));
        state.command(AppCommand::RefreshLibrary);
        assert_eq!(state.library.query, "roc");
        state.complete(Completed::Library {
            request: state.library.request,
            invalidate_artwork: true,
            result: Ok(vec![track(2)]),
        });
        assert_eq!(state.library.selected, Some(2));
        assert_eq!(state.library.status.retry, None);
        state.command(AppCommand::SearchLibrary("nothing".into()));
        state.complete(Completed::Library {
            request: state.library.request,
            invalidate_artwork: true,
            result: Ok(vec![]),
        });
        assert_eq!(state.library.status.message, "Nothing found.");
        let before_clear = state.library.request;
        state.command(AppCommand::SearchLibrary(String::new()));
        state.complete(Completed::Library {
            request: before_clear,
            invalidate_artwork: true,
            result: Err("late".into()),
        });
        assert!(state.library.status.loading);
        state.complete(Completed::Library {
            request: state.library.request,
            invalidate_artwork: true,
            result: Ok(vec![track(1), track(2)]),
        });
        assert_eq!(state.library.tracks.len(), 2);
        assert!(state.library.status.message.is_empty());
        state.tasks.abort_all();
        tokio::time::resume();
        state.session.take().unwrap().shutdown().await.unwrap();
    }

    #[test]
    fn selection_survives_refresh_shrinking_and_empty_library() {
        let (mut state, updates) = controller();
        state.replace_tracks(vec![track(1), track(2)], true);
        assert_eq!(state.library.selected, Some(1));
        state.command(AppCommand::SelectTrack(Some(2)));
        state.replace_tracks(vec![track(2), track(1)], true);
        assert_eq!(state.library.selected, Some(2));
        state.replace_tracks(vec![track(1)], true);
        assert_eq!(state.library.selected, Some(1));
        state.command(AppCommand::SelectTrack(None));
        assert_eq!(state.library.selected, None);
        state.command(AppCommand::SelectTrack(Some(404)));
        assert_eq!(state.library.selected, None);
        state.replace_tracks(vec![], true);
        assert!(state.library.status.message.contains("No songs"));
        assert!(matches!(
            updates
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|event| matches!(event, AppUpdate::TrackSelected(_))),
            Some(AppUpdate::TrackSelected(None))
        ));
    }
    #[test]
    fn list_failures_are_independent_and_preserve_previous_rows() {
        let (mut state, _) = controller();
        state.replace_tracks(vec![track(1)], true);
        state.complete(Completed::Folders(Err("folders failed".into())));
        assert_eq!(state.settings.status.retry, Some(SettingsRetry::Load));
        assert_eq!(state.library.status.retry, None);
        state.complete(Completed::Library {
            request: state.library.request,
            invalidate_artwork: true,
            result: Err("library failed".into()),
        });
        assert_eq!(state.library.tracks, vec![track(1)]);
        assert_eq!(state.library.selected, Some(1));
        state.complete(Completed::Folders(Ok(vec![folder(3)])));
        assert_eq!(state.settings.selected_folder, Some(3));
        assert_eq!(state.settings.status.retry, None);
        assert_eq!(state.library.status.retry, Some(SettingsRetry::Load));
        state.complete(Completed::Library {
            request: state.library.request,
            invalidate_artwork: true,
            result: Ok(vec![track(2)]),
        });
        assert_eq!(state.library.status.retry, None);
        assert_eq!(state.library.selected, Some(2));
    }
    #[test]
    fn folder_cancel_duplicate_and_selection_preserve_stored_values() {
        let (mut state, _) = controller();
        state.replace_folders(vec![folder(1), folder(2)]);
        state.command(AppCommand::SelectFolder(Some(2)));
        state.replace_folders(vec![folder(2), folder(1)]);
        assert_eq!(state.settings.selected_folder, Some(2));
        assert_eq!(state.settings.folders(), &[folder(2), folder(1)]);
        state.command(AppCommand::SelectFolder(Some(99)));
        assert_eq!(state.settings.selected_folder_id(), Some(2));
        state.settings.picking = true;
        state.settings.status.busy = true;
        state.complete_pick(None);
        assert_eq!(state.settings.selected_folder, Some(2));
        assert!(!state.settings.status.busy);
        state.complete(Completed::Registered(Ok(folder(3))));
        state.complete(Completed::Registered(Ok(folder(1))));
        assert_eq!(state.settings.selected_folder, Some(1));
        assert_eq!(
            state.settings.folders,
            vec![folder(2), folder(1), folder(3)]
        );
        state.complete(Completed::Registered(Err("registration failed".into())));
        assert_eq!(state.settings.status.retry, Some(SettingsRetry::Browse));
        state.replace_folders(vec![folder(2)]);
        assert_eq!(state.settings.selected_folder_id(), Some(2));
        state.command(AppCommand::SelectFolder(None));
        assert_eq!(state.settings.selected_folder_id(), None);
        state.replace_folders(Vec::new());
        assert!(state.settings.folders().is_empty());
        assert_eq!(state.settings.selected_folder_id(), None);
    }
    #[test]
    fn selected_media_has_priority_and_duplicate_requests_merge() {
        let (mut state, _) = controller();
        state.replace_tracks(vec![track(1), track(2), track(3)], true);
        state.request_media(2, false);
        state.request_media(2, true);
        state.request_media(3, true);
        state.request_media(1, true);
        assert_eq!(
            state.media.queue,
            VecDeque::from([(1, true), (2, true), (3, true)])
        );
        state.command(AppCommand::SelectTrack(Some(3)));
        assert_eq!(state.media.queue.front(), Some(&(3, true)));
    }
    #[test]
    fn evicted_cover_refetch_does_not_repeat_duration() {
        let (mut state, _) = controller();
        state.replace_tracks(vec![track(1)], true);
        state.media.durations.insert(1, None);
        state.request_media(1, false);
        assert!(state.media.queue.is_empty());
        state.request_media(1, true);
        assert_eq!(state.media.queue.pop_front(), Some((1, true)));
        assert!(state.media.durations.contains_key(&1));
        state.media.unavailable.insert(101);
        state.request_media(1, true);
        assert!(state.media.queue.is_empty());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn delayed_decodes_keep_all_slots_across_repeated_refreshes() {
        let (_directory, session) = test_session().await;
        let (mut state, updates) = controller();
        state.session = Some(session);
        state.replace_tracks((1..=5).map(track).collect(), true);
        let mut tickets = Vec::new();
        for id in 1..=4 {
            let ticket = MediaTicket {
                generation: 1,
                audio_id: id,
                serial: u64::try_from(id).unwrap(),
            };
            tickets.push(ticket);
            state.media.jobs.insert(
                ticket.serial,
                MediaJob {
                    ticket,
                    cover: Some(id.saturating_add(100)),
                    decoding: false,
                    artwork_pending: true,
                    duration_pending: true,
                    tasks: Vec::new(),
                },
            );
            state.media_completed(ticket, Some(vec![1, 2]), Some(42), true);
        }
        assert_eq!(state.media.jobs.len(), 4);
        assert!(state.media.jobs.values().all(|job| job.decoding));
        for invalidate in [false, true, false] {
            state.replace_tracks((1..=5).map(track).collect(), invalidate);
        }
        assert_eq!(
            state.media.jobs.len(),
            4,
            "refresh cannot cancel a blocking decoder"
        );
        state.media.serial = 4;
        state.request_media(5, true);
        state.start_media();
        assert!(
            state.tasks.is_empty(),
            "all four slots still belong to decoders"
        );
        updates.lock().unwrap().clear();
        for ticket in tickets {
            state.command(AppCommand::MediaInstalled {
                ticket,
                available: false,
            });
        }
        assert!(state.media.jobs.is_empty());
        assert!(
            state.media.unavailable.is_empty(),
            "stale decode failure cannot poison current cache state"
        );
        assert!(updates.lock().unwrap().is_empty());
        state.start_media();
        assert_eq!(
            state.media.jobs.len(),
            2,
            "released slots serve the retained selection and the newly visible row"
        );
        assert_eq!(
            state.media.jobs.get(&5).unwrap().ticket.audio_id,
            1,
            "selection keeps priority"
        );
        assert_eq!(state.media.jobs.get(&6).unwrap().ticket.audio_id, 5);
        assert_eq!(
            state.tasks.len(),
            4,
            "cover and duration run independently per pipeline"
        );
        state.tasks.abort_all();
        state.session.take().unwrap().shutdown().await.unwrap();
    }
    #[test]
    fn stale_network_results_and_forged_acks_do_not_change_current_tracks() {
        let (mut state, updates) = controller();
        state.replace_tracks(vec![track(1)], true);
        let ticket = MediaTicket {
            generation: 1,
            audio_id: 1,
            serial: 7,
        };
        state.media.jobs.insert(
            7,
            MediaJob {
                ticket,
                cover: Some(101),
                decoding: false,
                artwork_pending: true,
                duration_pending: true,
                tasks: Vec::new(),
            },
        );
        state.command(AppCommand::MediaInstalled {
            ticket,
            available: false,
        });
        assert_eq!(
            state.media.jobs.len(),
            1,
            "HTTP pipeline needs no installation ack yet"
        );
        state.replace_tracks(vec![track(1)], true);
        updates.lock().unwrap().clear();
        state.media_completed(ticket, Some(vec![3]), Some(6000), true);
        assert!(state.media.jobs.is_empty());
        assert_eq!(state.library.tracks.first().unwrap().duration, None);
        assert!(updates.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn failed_connection_can_retry_and_shutdown_is_idempotent() {
        let (send, mut updates) = mpsc::unbounded_channel();
        let options = ServerOptions {
            binary: Some(PathBuf::from("/nonexistent/osu-radio-test-server")),
            ..Default::default()
        };
        let controller = AppController::spawn(&Handle::current(), options, move |update| {
            let _ = send.send(update);
        });
        for _ in 0..2 {
            controller.send(AppCommand::Connect);
            tokio::time::timeout(Duration::from_secs(2), async {
                while let Some(update) = updates.recv().await {
                    if matches!(update, AppUpdate::Connection(ConnectionStatus::Failed(_))) {
                        return;
                    }
                }
                panic!("controller ended without a failure");
            })
            .await
            .unwrap();
        }
        controller.shutdown().await;
        controller.shutdown().await;
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn connection_is_single_flight_and_preserves_search_deadline_on_install() {
        let (directory, session) = test_session().await;
        let (mut state, updates) = controller();
        state.options.binary = Some(directory.path().join("missing-server"));
        tokio::time::pause();
        state.command(AppCommand::SearchLibrary("pending query".into()));
        let deadline = state.library.deadline.unwrap();
        let request = state.library.request;
        state.command(AppCommand::Connect);
        assert!(state.connecting);
        assert_eq!(state.tasks.len(), 1);
        assert_eq!(
            updates
                .lock()
                .unwrap()
                .iter()
                .filter(|update| matches!(
                    update,
                    AppUpdate::Connection(ConnectionStatus::Connecting)
                ))
                .count(),
            1
        );
        state.tasks.abort_all();
        while state.tasks.join_next().await.is_some() {}
        tokio::time::advance(Duration::from_millis(50)).await;
        state.complete(Completed::Connected(Ok(session)));
        assert!(!state.connecting);
        assert_eq!(state.library.query, "pending query");
        assert_eq!(state.library.deadline, Some(deadline));
        assert_eq!(state.library.request, request.wrapping_add(1));
        assert!(state.library.status.loading && state.library.task.is_some());
        let pending = state.tasks.len();
        state.command(AppCommand::Connect);
        assert_eq!(state.tasks.len(), pending);
        assert!(
            updates
                .lock()
                .unwrap()
                .iter()
                .any(|update| matches!(update, AppUpdate::Connection(ConnectionStatus::Connected)))
        );
        tokio::time::advance(Duration::from_millis(149)).await;
        tokio::task::yield_now().await;
        assert!(!state.library.task.as_ref().unwrap().is_finished());
        state.tasks.abort_all();
        while state.tasks.join_next().await.is_some() {}
        tokio::time::resume();
        state.session.take().unwrap().shutdown().await.unwrap();
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn shutdown_reaps_a_session_that_finishes_starting_during_cancellation() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("server");
        std::fs::write(
            &binary,
            "#!/bin/sh\necho $$ > pid\necho 'listening on http://127.0.0.1:9'\nexec sleep 30\n",
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let session = Session::start(ServerOptions {
            binary: Some(binary),
            working_directory: Some(directory.path().to_owned()),
            ..Default::default()
        })
        .await
        .unwrap();
        let pid: u32 = std::fs::read_to_string(directory.path().join("pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let process = PathBuf::from(format!("/proc/{pid}"));
        assert!(process.exists());
        let (mut state, _) = controller();
        let mut cancelled = state.cancel.subscribe();
        state.tasks.spawn(async move {
            cancelled.wait_for(|cancelled| *cancelled).await.unwrap();
            Completed::Connected(Ok(session))
        });
        let (sender, receiver) = mpsc::unbounded_channel();
        sender.send(AppCommand::Shutdown).unwrap();
        tokio::time::timeout(Duration::from_secs(3), state.run(receiver))
            .await
            .unwrap();
        assert!(
            !process.exists(),
            "late startup must be reaped before shutdown returns"
        );
    }
}

#[cfg(test)]
mod benchmarks;

#[cfg(test)]
mod media_tests {
    use super::tests::{controller, track};
    use super::*;

    fn pipeline(state: &mut Controller, audio_id: i32, serial: u64) -> MediaTicket {
        let ticket = MediaTicket {
            generation: state.media.generation,
            audio_id,
            serial,
        };
        state.media.jobs.insert(
            serial,
            MediaJob {
                ticket,
                cover: Some(audio_id.saturating_add(100)),
                decoding: false,
                artwork_pending: true,
                duration_pending: true,
                tasks: Vec::new(),
            },
        );
        ticket
    }

    #[test]
    fn artwork_arrives_before_duration_and_slots_release_in_both_completion_orders() {
        for duration_first in [false, true] {
            let (mut state, updates) = controller();
            state.replace_tracks(vec![track(1)], true);
            state.player.current_track = Some(track(1));
            state.request_media(1, true);
            state.set_visible_media(vec![(1, true)]);
            state.upcoming.open = true;
            state.upcoming.view.entries = vec![super::tests::queue_row(track(1), 1)];
            state.set_visible_queue_media(vec![(
                crate::models::QueueEntryId::new(1).unwrap(),
                true,
            )]);
            let ticket = pipeline(&mut state, 1, 1);
            updates.lock().unwrap().clear();
            state.artwork_completed(ticket, Some(vec![1, 2, 3]));
            assert!(state.media.jobs.get(&1).unwrap().decoding);
            assert!(state.media.jobs.get(&1).unwrap().duration_pending);
            assert_eq!(state.library.tracks.first().unwrap().duration, None);
            assert!(
                matches!(updates.lock().unwrap().last(), Some(AppUpdate::Artwork { ticket: emitted, .. }) if *emitted == ticket)
            );
            state.command(AppCommand::MediaInstalled {
                ticket: MediaTicket {
                    audio_id: 2,
                    ..ticket
                },
                available: false,
            });
            assert!(
                state.media.jobs.get(&1).unwrap().decoding,
                "forged ACK cannot release a slot"
            );
            if duration_first {
                state.duration_completed(ticket, Some(42_000));
                assert_eq!(
                    state.media.jobs.len(),
                    1,
                    "duration cannot release a decoder"
                );
            }
            state.command(AppCommand::MediaInstalled {
                ticket,
                available: true,
            });
            state.command(AppCommand::MediaInstalled {
                ticket,
                available: false,
            });
            assert!(
                !state.media.unavailable.contains(&101),
                "repeated ACK cannot poison a successful image"
            );
            if !duration_first {
                assert_eq!(
                    state.media.jobs.len(),
                    1,
                    "ACK cannot release pending duration HTTP"
                );
                state.duration_completed(ticket, Some(42_000));
            }
            assert!(state.media.jobs.is_empty());
            assert_eq!(
                state.player.current_track.as_ref().unwrap().duration,
                Some(Duration::from_secs(42))
            );
            assert_eq!(
                state.player.selected_track.as_ref().unwrap().duration,
                Some(Duration::from_secs(42))
            );
            state.duration_completed(ticket, Some(99_000));
            assert_eq!(
                state.media.durations.get(&1),
                Some(&Some(Duration::from_secs(42)))
            );
            state.set_visible_media(vec![(1, false)]);
            assert!(
                state.media.queue.is_empty(),
                "successful installation consumes all artwork demand"
            );
            assert_eq!(state.media.selected_artwork_missing, Some(false));
            assert_eq!(state.media.queue_media.get(&1), Some(&false));
        }
    }

    #[test]
    fn viewport_snapshots_replace_historical_demand_and_deduplicate_audio() {
        let (mut state, _) = controller();
        state.replace_tracks((1..=100).map(track).collect(), true);
        state.command(AppCommand::SelectTrack(None));
        state.set_visible_media((1..=30).map(|id| (id, true)).collect());
        state.set_visible_media(vec![
            (-1, true),
            (999, true),
            (90, false),
            (91, true),
            (90, true),
        ]);
        assert_eq!(state.media.visible, Some(vec![(90, true), (91, true)]));
        assert_eq!(state.media.queue, VecDeque::from([(90, true), (91, true)]));
        state.request_media(2, true);
        assert_eq!(
            state.media.queue.len(),
            2,
            "offscreen compatibility request cannot extend an active snapshot"
        );
        state.command(AppCommand::SelectTrack(Some(50)));
        state.request_media(50, true);
        state.set_visible_media(Vec::new());
        assert_eq!(
            state.media.queue,
            VecDeque::from([(50, true)]),
            "selection remains independent"
        );
    }

    #[tokio::test]
    async fn obsolete_http_is_aborted_but_started_decode_waits_for_its_ack() {
        let (mut state, _) = controller();
        state.replace_tracks((1..=3).map(track).collect(), true);
        state.command(AppCommand::SelectTrack(None));
        let first = pipeline(&mut state, 1, 1);
        let second = pipeline(&mut state, 2, 2);
        let http = state.task(std::future::pending());
        let duration = state.task(std::future::pending());
        state
            .media
            .jobs
            .get_mut(&1)
            .unwrap()
            .tasks
            .push(http.clone());
        state
            .media
            .jobs
            .get_mut(&2)
            .unwrap()
            .tasks
            .push(duration.clone());
        state.artwork_completed(second, Some(vec![2]));
        state.set_visible_media(vec![(3, true)]);
        assert!(!state.media.jobs.contains_key(&1));
        assert!(state.media.jobs.get(&2).unwrap().decoding);
        assert!(!state.media.jobs.get(&2).unwrap().duration_pending);
        while state.tasks.join_next().await.is_some() {}
        assert!(http.is_finished() && duration.is_finished());
        state.artwork_completed(first, Some(vec![1]));
        state.replace_tracks((1..=3).map(track).collect(), false);
        state.duration_completed(second, Some(123_000));
        assert!(
            state.media.jobs.get(&2).unwrap().decoding,
            "late duration cannot release old decoder"
        );
        state.command(AppCommand::MediaInstalled {
            ticket: second,
            available: false,
        });
        assert!(state.media.jobs.is_empty());
        assert!(state.media.unavailable.is_empty() && state.media.durations.is_empty());
    }

    #[test]
    fn navigation_preserves_metadata_and_true_invalidation_clears_it() {
        let (mut state, updates) = controller();
        state.replace_tracks(vec![track(1)], true);
        updates.lock().unwrap().clear();
        state
            .media
            .durations
            .insert(1, Some(Duration::from_secs(42)));
        state.media.unavailable.insert(101);
        state.replace_tracks(vec![track(1)], false);
        assert_eq!(
            state.library.tracks.first().unwrap().duration,
            Some(Duration::from_secs(42))
        );
        state.playlist_action(PlaylistAction::ShowPlaylists);
        state.playlist_action(PlaylistAction::ShowLibrary);
        assert!(state.media.unavailable.contains(&101));
        assert_eq!(
            state.media.durations.get(&1),
            Some(&Some(Duration::from_secs(42)))
        );
        assert_eq!(
            state.media.generation, 4,
            "each replacement advances the adapter ticket contract"
        );
        let replacements: Vec<_> = updates
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| {
                if let AppUpdate::LibraryTracksReplaced {
                    invalidate_artwork, ..
                } = event
                {
                    Some(*invalidate_artwork)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(replacements, [false]);
        state.replace_tracks(vec![track(1)], true);
        assert!(state.media.unavailable.is_empty() && state.media.durations.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn superseding_search_retains_refresh_invalidation_until_success() {
        let (_directory, session) = super::tests::test_session().await;
        let (mut state, updates) = controller();
        state.session = Some(session);
        state.replace_tracks(vec![track(1)], true);
        state.library.invalidation_pending = false;
        state
            .media
            .durations
            .insert(1, Some(Duration::from_secs(42)));
        state.command(AppCommand::RefreshLibrary);
        let refresh = state.library.request;
        state.command(AppCommand::SearchLibrary("latest".into()));
        state.complete(Completed::Library {
            request: refresh,
            invalidate_artwork: true,
            result: Ok(Vec::new()),
        });
        assert_eq!(state.library.tracks.len(), 1);
        state.complete(Completed::Library {
            request: state.library.request,
            invalidate_artwork: true,
            result: Err("offline".into()),
        });
        assert_eq!(
            state.media.durations.get(&1),
            Some(&Some(Duration::from_secs(42)))
        );
        assert!(
            state.library.invalidation_pending,
            "failed refresh preserves both images and its invalidation intent"
        );
        state.command(AppCommand::SearchLibrary("newest".into()));
        // The current task itself must have carried the sticky reason through load_library.
        let mut completion = None;
        while let Some(result) = state.tasks.join_next().await {
            if let Ok(result @ Completed::Library { .. }) = result {
                completion = Some(result);
                break;
            }
        }
        assert!(matches!(
            completion,
            Some(Completed::Library {
                invalidate_artwork: true,
                ..
            })
        ));
        state.complete(Completed::Library {
            request: state.library.request,
            invalidate_artwork: true,
            result: Ok(vec![track(1)]),
        });
        assert!(!state.library.invalidation_pending && state.media.durations.is_empty());
        assert!(updates.lock().unwrap().iter().any(|event| matches!(
            event,
            AppUpdate::LibraryTracksReplaced {
                invalidate_artwork: true,
                ..
            }
        )));
        state.tasks.abort_all();
        state.session.take().unwrap().shutdown().await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn artwork_upgrade_and_reentry_use_one_slot_and_distinct_delivery_tickets() {
        let (_directory, session) = super::tests::test_session().await;
        let (mut state, _) = controller();
        state.session = Some(session);
        state.replace_tracks(vec![track(1)], true);
        state.request_media(1, false);
        state.start_media();
        let original = state.media.jobs.get(&1).unwrap().ticket;
        assert!(state.media.jobs.get(&1).unwrap().cover.is_none());
        state.request_media(1, true);
        assert_eq!(state.media.jobs.len(), 1);
        assert!(state.media.jobs.get(&1).unwrap().artwork_pending);
        state.artwork_completed(original, Some(vec![1]));
        state.command(AppCommand::MediaInstalled {
            ticket: original,
            available: true,
        });
        assert!(state.media.jobs.get(&1).unwrap().duration_pending);
        state.request_media(1, true); // The installed cover was evicted while duration HTTP remained pending.
        state.start_media();
        let replacement = state.media.jobs.get(&2).unwrap().ticket;
        assert_ne!(replacement.serial, original.serial);
        assert_eq!(state.media.jobs.len(), 1);
        state.artwork_completed(replacement, Some(vec![2]));
        state.command(AppCommand::MediaInstalled {
            ticket: original,
            available: false,
        });
        state.duration_completed(original, Some(99_000));
        assert!(state.media.jobs.get(&2).unwrap().decoding);
        assert!(state.media.jobs.get(&2).unwrap().duration_pending);
        assert!(!state.media.unavailable.contains(&101));
        state.duration_completed(replacement, Some(42_000));
        state.command(AppCommand::MediaInstalled {
            ticket: replacement,
            available: true,
        });
        state.set_visible_media(vec![(1, false)]);
        assert!(state.media.jobs.is_empty() && state.media.queue.is_empty());
        state.tasks.abort_all();
        state.session.take().unwrap().shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_cancels_http_without_waiting_for_missing_decode_ack() {
        let (mut state, _) = controller();
        state.replace_tracks(vec![track(1)], true);
        let ticket = pipeline(&mut state, 1, 1);
        let task = state.task(std::future::pending());
        state
            .media
            .jobs
            .get_mut(&1)
            .unwrap()
            .tasks
            .push(task.clone());
        state.artwork_completed(ticket, Some(vec![1]));
        let (sender, receiver) = mpsc::unbounded_channel();
        sender.send(AppCommand::Shutdown).unwrap();
        tokio::time::timeout(Duration::from_secs(2), state.run(receiver))
            .await
            .unwrap();
        assert!(task.is_finished());
    }

    #[tokio::test]
    async fn generation_wrap_aborts_http_retains_decode_and_invalidates_all_demands() {
        let (mut state, updates) = controller();
        state.replace_tracks(vec![track(1), track(2)], true);
        state.media.generation = u64::MAX;
        state.set_visible_media(vec![(1, false), (2, true)]);
        state.media.queue_media.insert(1, false);
        state.media.queue_media.insert(2, false);
        state.media.selected_artwork_missing = Some(false);
        state
            .media
            .remember_duration(1, Some(Duration::from_secs(42)));
        state.media.unavailable.insert(101);
        let network = pipeline(&mut state, 1, 1);
        let decode = pipeline(&mut state, 2, 2);
        let mut handles = Vec::new();
        for ticket in [network, decode] {
            let task = state.task(std::future::pending());
            state
                .media
                .jobs
                .get_mut(&ticket.serial)
                .unwrap()
                .tasks
                .push(task.clone());
            handles.push(task);
        }
        state.artwork_completed(decode, Some(vec![2]));
        updates.lock().unwrap().clear();
        state.advance_media(true);
        while let Some(result) = state.tasks.join_next().await {
            assert!(matches!(result, Err(ref error) if error.is_cancelled()));
        }
        assert!(handles.iter().all(tokio::task::AbortHandle::is_finished));
        assert_eq!(state.media.generation, 0);
        assert!(state.media.queue.is_empty());
        assert_eq!(state.media.visible, Some(Vec::new()));
        assert!(state.media.durations.is_empty() && state.media.unavailable.is_empty());
        assert_eq!(state.media.selected_artwork_missing, Some(true));
        assert!(state.media.queue_media.values().all(|missing| *missing));
        assert!(!state.media.jobs.contains_key(&network.serial));
        let job = state.media.jobs.get(&decode.serial).unwrap();
        assert!(job.decoding && job.tasks.is_empty());
        assert!(!job.artwork_pending && !job.duration_pending);
        assert!(matches!(
            updates.lock().unwrap().as_slice(),
            [AppUpdate::MediaGeneration {
                generation: 0,
                invalidate_artwork: true
            }]
        ));
        state.command(AppCommand::MediaInstalled {
            ticket: decode,
            available: true,
        });
        assert!(state.media.jobs.is_empty());
        assert_eq!(state.media.queue_media.get(&2), Some(&true));
        assert!(state.media.unavailable.is_empty());
    }

    #[test]
    fn missing_http_and_failed_decode_cache_unavailability_until_explicit_invalidation() {
        for decode_failure in [false, true] {
            let (mut state, updates) = controller();
            state.replace_tracks(vec![track(1)], true);
            state.request_media(1, true);
            let ticket = pipeline(&mut state, 1, 1);
            updates.lock().unwrap().clear();
            state.artwork_completed(ticket, decode_failure.then(|| vec![1, 2]));
            assert_eq!(state.media.jobs.get(&1).unwrap().decoding, decode_failure);
            state.duration_completed(ticket, None);
            if decode_failure {
                state.command(AppCommand::MediaInstalled {
                    ticket,
                    available: false,
                });
            }
            assert!(state.media.jobs.is_empty());
            assert_eq!(state.media.durations.get(&1), Some(&None));
            assert!(state.media.unavailable.contains(&101));
            state.media.queue.clear();
            state.request_media(1, true);
            assert!(state.media.queue.is_empty());
            assert_eq!(
                updates
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|update| matches!(update, AppUpdate::Artwork { .. }))
                    .count(),
                usize::from(decode_failure)
            );
            state.advance_media(false);
            assert!(state.media.unavailable.contains(&101));
            assert_eq!(state.media.durations.get(&1), Some(&None));
            state.advance_media(true);
            state.request_media(1, true);
            assert_eq!(state.media.queue, VecDeque::from([(1, true)]));
        }
    }
}

#[cfg(test)]
mod view_model_tests;

#[cfg(test)]
mod mods_tests;
