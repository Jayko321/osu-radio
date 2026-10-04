//! Shared application workflows. Adapters own presentation and decoded artwork, and acknowledge
//! every artwork delivery only after decoding and cache installation (or stale-result discard).
use crate::{
    OsuFolder, RegisterOsuFolder, ServerOptions, Session, Track, TrackNamePreferences, describe,
    view_models::selection_after_refresh,
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    future::Future,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::{
    runtime::Handle,
    sync::{mpsc, watch},
    task::{AbortHandle, JoinSet},
};
mod folders;
mod playlists;
mod preferences;
mod queue;
mod sorting;
mod volume;
pub use folders::{FolderAction, FolderSelection, FolderSelectionRow};
pub use playlists::{PlaylistAction, PlaylistCandidate, PlaylistsState};
pub use queue::QueueView;
pub use sorting::TrackSort;
pub use volume::AudioSettingsState;

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
    Playlist(PlaylistAction),
    Connect,
    RefreshLibrary,
    SearchLibrary(String),
    SetTrackSort(TrackSort),
    SetTrackNamePreferences(TrackNamePreferences),
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
    SetIndividualVolumeEnabled(bool),
    SetGlobalVolume(u8),
    SetTrackVolume {
        audio_id: i32,
        volume_percent: Option<u8>,
    },
    RetryAudioSettings,
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
    TracksReplaced {
        tracks: Vec<Track>,
        invalidate_artwork: bool,
    },
    TracksReordered(Vec<Track>),
    TrackSort(TrackSort),
    TrackChanged(Track),
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
}

enum Completed {
    Queue {
        request: u64,
        result: Result<crate::models::QueueState, String>,
    },
    AudioSettings(Result<crate::models::AudioSettings, String>),
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
struct MediaJob {
    ticket: MediaTicket,
    cover: Option<i32>,
    decoding: bool,
    artwork_pending: bool,
    duration_pending: bool,
    tasks: Vec<AbortHandle>,
}
#[allow(clippy::struct_excessive_bools)] // Independent async workflows have separate pending flags.
struct Controller {
    upcoming: queue::QueueWork,
    volume: volume::VolumeWork,
    playlists: playlists::PlaylistWork,
    selection: folders::SelectionWork,
    folder_events: mpsc::UnboundedReceiver<folders::FolderEvent>,
    folder_sender: mpsc::UnboundedSender<folders::FolderEvent>,
    options: ServerOptions,
    emit: Arc<dyn Fn(AppUpdate) + Send + Sync>,
    session: Option<Session>,
    connecting: bool,
    tasks: JoinSet<Completed>,
    cancel: watch::Sender<bool>,
    tracks: Vec<Track>,
    track_sort: TrackSort,
    name_preferences: TrackNamePreferences,
    last_played: HashMap<i32, i64>,
    track_indices: HashMap<i32, usize>,
    selected: Option<i32>,
    current_track: Option<Track>,
    assignment: Option<crate::models::PlaybackAssignment>,
    playback_commands: VecDeque<queue::PendingPlayback>,
    playback_command_busy: bool,
    pending_start: Option<u64>,
    assignments: mpsc::UnboundedReceiver<Result<crate::models::PlaybackAssignment, String>>,
    assignment_sender: mpsc::UnboundedSender<Result<crate::models::PlaybackAssignment, String>>,
    worker_state: crate::playback::Playback,
    worker_updates: mpsc::UnboundedReceiver<crate::playback::Playback>,
    worker_sender: mpsc::UnboundedSender<crate::playback::Playback>,
    folders: Vec<OsuFolder>,
    selected_folder: Option<i32>,
    library: OperationStatus,
    library_query: String,
    library_request: u64,
    library_task: Option<AbortHandle>,
    library_deadline: Option<tokio::time::Instant>,
    library_invalidation_pending: bool,
    folder_status: OperationStatus,
    picking: bool,
    generation: u64,
    serial: u64,
    queue: VecDeque<(i32, bool)>,
    visible_media: Option<Vec<(i32, bool)>>,
    selected_media: Option<Track>,
    selected_artwork_missing: Option<bool>,
    queue_media: HashMap<i32, bool>,
    jobs: HashMap<u64, MediaJob>,
    durations: HashMap<i32, Option<Duration>>,
    unavailable: HashSet<i32>,
    playback: Option<crate::playback::Worker>,
    downloads: Option<mpsc::UnboundedReceiver<crate::playback::Message>>,
    audio_task: Option<AbortHandle>,
    audio_gate: Arc<tokio::sync::Semaphore>,
}
impl Controller {
    fn new(options: ServerOptions, emit: Arc<dyn Fn(AppUpdate) + Send + Sync>) -> Self {
        let (cancel, _) = watch::channel(false);
        let (folder_sender, folder_events) = mpsc::unbounded_channel();
        let (assignment_sender, assignments) = mpsc::unbounded_channel();
        let (worker_sender, worker_updates) = mpsc::unbounded_channel();
        Self {
            upcoming: queue::QueueWork::default(),
            volume: volume::VolumeWork::default(),
            playlists: playlists::PlaylistWork::default(),
            selection: folders::SelectionWork::default(),
            folder_sender,
            folder_events,
            options,
            emit,
            session: None,
            connecting: false,
            tasks: JoinSet::new(),
            cancel,
            tracks: Vec::new(),
            track_sort: TrackSort::default(),
            name_preferences: TrackNamePreferences::default(),
            last_played: HashMap::new(),
            track_indices: HashMap::new(),
            selected: None,
            current_track: None,
            assignment: None,
            playback_commands: VecDeque::new(),
            playback_command_busy: false,
            pending_start: None,
            assignment_sender,
            assignments,
            worker_sender,
            worker_state: crate::playback::Playback::default(),
            worker_updates,
            folders: Vec::new(),
            selected_folder: None,
            library: OperationStatus::default(),
            library_query: String::new(),
            library_request: 0,
            library_task: None,
            library_deadline: None,
            library_invalidation_pending: true,
            folder_status: OperationStatus::default(),
            picking: false,
            generation: 0,
            serial: 0,
            queue: VecDeque::new(),
            visible_media: None,
            selected_media: None,
            selected_artwork_missing: None,
            queue_media: HashMap::new(),
            jobs: HashMap::new(),
            durations: HashMap::new(),
            unavailable: HashSet::new(),
            playback: None,
            downloads: None,
            audio_task: None,
            audio_gate: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<AppCommand>) {
        loop {
            let volume_deadline = self.volume_deadline();
            tokio::select! {
                () = async {
                    match volume_deadline {
                        Some(deadline) => tokio::time::sleep_until(deadline).await,
                        None => std::future::pending().await,
                    }
                } => self.start_volume_save(),
                result = async {
                    match &mut self.volume.save_task {
                        Some(task) => task.await.unwrap_or_else(|error| Err(error.to_string())),
                        None => std::future::pending().await,
                    }
                } => self.volume_saved(result),
                command = commands.recv() => match command {
                    Some(AppCommand::Shutdown) | None => break,
                    Some(command) => self.command(command),
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
        }
        self.cancel_audio();
        self.flush_volume_saves().await;
        self.cancel_selection();
        let _ = self.cancel.send(true);
        if let Some(mut playback) = self.playback.take() {
            let _ = tokio::task::spawn_blocking(move || playback.shutdown()).await;
        }
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
    fn command(&mut self, command: AppCommand) {
        match command {
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
            AppCommand::SetVolume(volume) => self.set_selected_volume(volume),
            AppCommand::SetIndividualVolumeEnabled(enabled) => self.set_individual_volume(enabled),
            AppCommand::SetGlobalVolume(percent) => self.set_global_volume(percent),
            AppCommand::SetTrackVolume {
                audio_id,
                volume_percent,
            } => self.set_track_volume(audio_id, volume_percent),
            AppCommand::RetryAudioSettings => self.retry_audio_settings(),
            AppCommand::Connect => self.connect(),
            AppCommand::RefreshLibrary => self.refresh_library(),
            AppCommand::SetTrackSort(sort) => self.set_track_sort(sort),
            AppCommand::SetTrackNamePreferences(preferences) => {
                self.set_name_preferences(preferences);
            }
            AppCommand::SearchLibrary(query) => {
                if self.library_query != query {
                    self.library_query = query;
                    self.load_library(Duration::from_millis(200), false);
                }
            }
            AppCommand::RefreshFolders => self.refresh_folders(),
            AppCommand::SelectTrack(id) => {
                self.finish_volume_editing();
                if id.is_none_or(|id| self.track(id).is_some()) {
                    self.selected = id;
                    self.emit_selection();
                    if let Some(id) = id
                        && let Some(index) = self.queue.iter().position(|(queued, _)| *queued == id)
                        && let Some(request) = self.queue.remove(index)
                    {
                        self.queue.push_front(request);
                    }
                }
            }
            AppCommand::SelectFolder(id) => {
                if id.is_none() || self.folders.iter().any(|folder| Some(folder.id) == id) {
                    self.selected_folder = id;
                    (self.emit)(AppUpdate::FolderSelected(id));
                }
            }
            AppCommand::BeginFolderPick => self.begin_pick(),
            AppCommand::CompleteFolderPick(path) => self.complete_pick(path),
            AppCommand::RetryFolders => match self.folder_status.retry {
                Some(SettingsRetry::Browse) => self.begin_pick(),
                _ => self.refresh_folders(),
            },
            AppCommand::RequestMedia {
                audio_id,
                artwork_missing,
            } => self.request_media(audio_id, artwork_missing),
            AppCommand::SetVisibleMedia(visible) => self.set_visible_media(visible),
            AppCommand::MediaInstalled { ticket, available } => {
                if let Some(job) = self.jobs.get_mut(&ticket.serial)
                    && job.ticket == ticket
                    && job.decoding
                {
                    job.decoding = false;
                    if ticket.generation == self.generation && available {
                        if self
                            .selected_media
                            .as_ref()
                            .is_some_and(|track| track.audio_source_id == ticket.audio_id)
                        {
                            self.selected_artwork_missing = Some(false);
                        }
                        if let Some(missing) = self.queue_media.get_mut(&ticket.audio_id) {
                            *missing = false;
                        }
                        if let Some(visible) = &mut self.visible_media {
                            for (id, missing) in visible {
                                if *id == ticket.audio_id {
                                    *missing = false;
                                }
                            }
                        }
                    }
                    if ticket.generation == self.generation
                        && !available
                        && let Some(cover) = job.cover
                    {
                        self.unavailable.insert(cover);
                    }
                    self.finish_media(ticket);
                }
            }
            AppCommand::Shutdown => {}
        }
    }
    fn seek_selected(&mut self, position: Duration) {
        if self.playlists.view.showing_detail()
            && self.assignment.as_ref().is_none_or(|state| {
                state.current_playlist_item_id != self.playlists.view.selected_item_id
            })
        {
            return;
        }
        let expected_id = if self.playlists.view.showing_detail() {
            self.playlists
                .view
                .selected_item()
                .and_then(|item| item.audio_source_id)
        } else {
            self.selected
        };
        self.player_command(crate::playback::Command::Seek {
            expected_id,
            playback_token: self
                .assignment
                .as_ref()
                .map_or(0, |state| state.playback_token),
            position,
        });
    }
    fn ensure_playback(&mut self) -> bool {
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
                    (self.emit)(AppUpdate::Playback(crate::playback::Playback {
                        error: Some(error.to_string()),
                        ..Default::default()
                    }));
                    return false;
                }
            }
        }
        true
    }
    fn player_command(&mut self, command: crate::playback::Command) {
        if self.ensure_playback()
            && let Some(worker) = &self.playback
        {
            worker.send(command);
        }
    }
    fn cancel_audio(&mut self) {
        if let Some(worker) = &self.playback {
            worker.invalidate();
        }
        if let Some(task) = self.audio_task.take() {
            task.abort();
        }
    }
    fn download(&mut self, request: crate::playback::Download) {
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
    fn connect(&mut self) {
        if self.connecting || self.session.is_some() {
            return;
        }
        self.connecting = true;
        (self.emit)(AppUpdate::Connection(ConnectionStatus::Connecting));
        self.library = OperationStatus {
            loading: true,
            message: "Connecting…".into(),
            ..Default::default()
        };
        self.folder_status = self.library.clone();
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
    fn refresh_library(&mut self) {
        self.load_library(Duration::ZERO, true);
    }
    fn load_library(&mut self, delay: Duration, invalidate_artwork: bool) {
        self.library_invalidation_pending |= invalidate_artwork;
        let invalidate_artwork = self.library_invalidation_pending;
        self.library_deadline = tokio::time::Instant::now().checked_add(delay);
        self.library_request = self.library_request.wrapping_add(1);
        if let Some(task) = self.library_task.take() {
            task.abort();
        }
        let Some(session) = &self.session else {
            self.connect();
            return;
        };
        let api = session.api().clone();
        let query = self.library_query.clone();
        let request = self.library_request;
        self.library = OperationStatus {
            loading: true,
            message: "Loading library…".into(),
            ..Default::default()
        };
        (self.emit)(AppUpdate::LibraryStatus(self.library.clone()));
        self.library_task = Some(self.task(async move {
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
    fn refresh_folders(&mut self) {
        let Some(session) = &self.session else {
            self.connect();
            return;
        };
        if self.folder_status.loading || self.folder_status.busy {
            return;
        }
        let api = session.api().clone();
        self.folder_status = OperationStatus {
            loading: true,
            message: "Loading folders…".into(),
            ..Default::default()
        };
        (self.emit)(AppUpdate::FolderStatus(self.folder_status.clone()));
        self.task(async move {
            Completed::Folders(api.osu_folders().await.map_err(|error| describe(&error)))
        });
    }
    fn begin_pick(&mut self) {
        if self.session.is_none() || self.folder_status.loading || self.folder_status.busy {
            return;
        }
        self.picking = true;
        self.folder_status.busy = true;
        (self.emit)(AppUpdate::FolderStatus(self.folder_status.clone()));
        (self.emit)(AppUpdate::FolderPickerRequested);
    }
    fn complete_pick(&mut self, path: Option<PathBuf>) {
        if !self.picking {
            return;
        }
        self.picking = false;
        self.folder_status.busy = false;
        let Some(path) = path else {
            (self.emit)(AppUpdate::FolderStatus(self.folder_status.clone()));
            return;
        };
        let Some(session) = &self.session else {
            return;
        };
        let api = session.api().clone();
        self.folder_status = OperationStatus {
            busy: true,
            message: "Adding osu! folder…".into(),
            ..Default::default()
        };
        (self.emit)(AppUpdate::FolderStatus(self.folder_status.clone()));
        self.task(async move {
            Completed::Registered(
                api.register_osu_folder(&RegisterOsuFolder::new(path))
                    .await
                    .map(crate::RegisteredFolder::into_folder)
                    .map_err(|error| describe(&error)),
            )
        });
    }
    #[allow(clippy::too_many_lines)] // Keep completion routing and stale-result checks together.
    fn complete(&mut self, result: Completed) {
        match result {
            Completed::Queue { request, result } => self.queue_loaded(request, result),
            Completed::AudioSettings(result) => self.audio_settings_loaded(result),
            Completed::Playlist(event) => self.playlist_event(event),
            Completed::PlaybackRetry(pending) => self.playback_commands.push_front(pending),
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
                self.library.loading = false;
                self.folder_status.loading = false;
                match result {
                    Ok(session) => {
                        self.session = Some(session);
                        (self.emit)(AppUpdate::Connection(ConnectionStatus::Connected));
                        let delay = self.library_deadline.map_or(Duration::ZERO, |deadline| {
                            deadline.saturating_duration_since(tokio::time::Instant::now())
                        });
                        self.load_library(delay, true);
                        self.refresh_folders();
                        self.refresh_playlists();
                        self.load_audio_settings();
                        self.refresh_queue();
                    }
                    Err(error) => {
                        self.library = failure(error.clone(), SettingsRetry::Load);
                        self.folder_status = self.library.clone();
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
                if request != self.library_request {
                    return;
                }
                self.library_task = None;
                self.library = OperationStatus::default();
                match result {
                    Ok(tracks) => {
                        self.library_invalidation_pending = false;
                        self.replace_tracks(tracks, invalidate_artwork);
                        if self.playlists.view.active_id.is_some() {
                            self.playlist_action(PlaylistAction::Refresh);
                        }
                    }
                    Err(error) => self.library = failure(error, SettingsRetry::Load),
                }
                (self.emit)(AppUpdate::LibraryStatus(self.library.clone()));
            }
            Completed::Folders(result) => {
                self.folder_status = OperationStatus::default();
                match result {
                    Ok(folders) => self.replace_folders(folders),
                    Err(error) => self.folder_status = failure(error, SettingsRetry::Load),
                }
                (self.emit)(AppUpdate::FolderStatus(self.folder_status.clone()));
            }
            Completed::Registered(result) => {
                self.folder_status = OperationStatus::default();
                match result {
                    Ok(folder) => {
                        self.selected_folder = Some(folder.id);
                        if let Some(existing) =
                            self.folders.iter_mut().find(|row| row.id == folder.id)
                        {
                            *existing = folder;
                        } else {
                            self.folders.push(folder);
                        }
                        (self.emit)(AppUpdate::FoldersReplaced(self.folders.clone()));
                        (self.emit)(AppUpdate::FolderSelected(self.selected_folder));
                    }
                    Err(error) => self.folder_status = failure(error, SettingsRetry::Browse),
                }
                (self.emit)(AppUpdate::FolderStatus(self.folder_status.clone()));
            }
            Completed::MediaArtwork { ticket, bytes } => self.artwork_completed(ticket, bytes),
            Completed::MediaDuration { ticket, duration } => {
                self.duration_completed(ticket, duration);
            }
            Completed::Cancelled => {}
        }
    }
    fn replace_tracks(&mut self, mut tracks: Vec<Track>, invalidate_artwork: bool) {
        self.advance_media(invalidate_artwork);
        self.apply_name_preferences(&mut tracks);
        self.restore_durations(&mut tracks);
        self.merge_last_played(&mut tracks);
        self.merge_volume_overrides(&mut tracks);
        self.track_sort.sort_tracks(&mut tracks);
        if self.selected
            != self
                .current_track
                .as_ref()
                .map(|track| track.audio_source_id)
            || self.selected.is_none()
        {
            self.selected = selection_after_refresh(&tracks, self.selected);
        }
        self.track_indices = tracks
            .iter()
            .enumerate()
            .map(|(index, track)| (track.audio_source_id, index))
            .collect();
        self.tracks = tracks;
        if self.tracks.is_empty() {
            self.library.message = if self.library_query.trim().is_empty() {
                "No songs yet. Import an osu! folder to populate your library."
            } else {
                "Nothing found."
            }
            .into();
        }
        (self.emit)(AppUpdate::TracksReplaced {
            tracks: if self.playlists.view.showing_detail() {
                self.playlists.tracks.clone()
            } else {
                self.tracks.clone()
            },
            invalidate_artwork,
        });
        self.emit_selection();
    }
    fn replace_folders(&mut self, folders: Vec<OsuFolder>) {
        self.selected_folder = self
            .selected_folder
            .filter(|id| folders.iter().any(|folder| folder.id == *id))
            .or_else(|| folders.first().map(|folder| folder.id));
        self.folders = folders;
        self.reconcile_selection();
        (self.emit)(AppUpdate::FoldersReplaced(self.folders.clone()));
        (self.emit)(AppUpdate::FolderSelected(self.selected_folder));
    }
    fn emit_selection(&mut self) {
        // Detail loading has no replacement selection yet; retain the displayed track.
        if self.playlists.view.showing_detail() && self.playlists.view.active.is_none() {
            self.reconcile_media();
            return;
        }
        self.emit_volume();
        let track = if self.playlists.view.showing_detail() {
            self.playlists
                .view
                .active
                .as_ref()
                .and_then(|playlist| {
                    playlist
                        .items
                        .iter()
                        .position(|item| Some(item.id) == self.playlists.view.selected_item_id)
                })
                .and_then(|index| self.playlists.tracks.get(index))
                .cloned()
        } else {
            self.selected.and_then(|id| self.track(id)).cloned()
        };
        if self
            .selected_media
            .as_ref()
            .map(|old| (old.audio_source_id, old.cover_beatmap_id))
            != track
                .as_ref()
                .map(|row| (row.audio_source_id, row.cover_beatmap_id))
        {
            self.selected_artwork_missing = None;
        }
        self.selected_media.clone_from(&track);
        (self.emit)(AppUpdate::TrackSelected(track));
        self.reconcile_media();
    }
    fn emit_statuses(&self) {
        (self.emit)(AppUpdate::LibraryStatus(self.library.clone()));
        (self.emit)(AppUpdate::FolderStatus(self.folder_status.clone()));
    }
    fn track(&self, id: i32) -> Option<&Track> {
        if self.playlists.view.showing_detail() {
            let item_index = self.playlists.view.active.as_ref().and_then(|playlist| {
                playlist
                    .items
                    .iter()
                    .position(|item| Some(item.id) == self.playlists.view.selected_item_id)
            });
            if let Some(track) = item_index
                .and_then(|index| self.playlists.tracks.get(index))
                .filter(|track| track.audio_source_id == id)
                .or_else(|| {
                    self.playlists
                        .tracks
                        .iter()
                        .find(|track| track.audio_source_id == id)
                })
            {
                return Some(track);
            }
        }
        self.current_track
            .as_ref()
            .filter(|track| track.audio_source_id == id)
            .or_else(|| {
                self.track_indices
                    .get(&id)
                    .and_then(|index| self.tracks.get(*index))
            })
            .or_else(|| {
                self.upcoming
                    .view
                    .tracks
                    .iter()
                    .find(|track| track.audio_source_id == id)
            })
            .or_else(|| {
                self.selected_media
                    .as_ref()
                    .filter(|track| track.audio_source_id == id)
            })
    }
    fn restore_durations(&self, tracks: &mut [Track]) {
        for track in tracks {
            if let Some(duration) = self.durations.get(&track.audio_source_id) {
                track.duration = *duration;
            }
        }
    }
    fn advance_media(&mut self, invalidate_artwork: bool) {
        self.generation = self.generation.wrapping_add(1);
        self.queue.clear();
        if let Some(visible) = &mut self.visible_media {
            visible.clear();
        }
        for job in self.jobs.values_mut() {
            for task in job.tasks.drain(..) {
                task.abort();
            }
            job.artwork_pending = false;
            job.duration_pending = false;
        }
        // A started decoder cannot be cancelled, and continues owning its pipeline slot.
        self.jobs.retain(|_, job| job.decoding);
        if invalidate_artwork {
            self.durations.clear();
            self.unavailable.clear();
            self.selected_artwork_missing = self.selected_media.as_ref().map(|_| true);
            for missing in self.queue_media.values_mut() {
                *missing = true;
            }
        }
    }
    fn media_wanted(&self, id: i32) -> bool {
        self.selected_media
            .as_ref()
            .is_some_and(|track| track.audio_source_id == id)
            || (self.upcoming.open && self.queue_media.contains_key(&id))
            || self
                .visible_media
                .as_ref()
                .is_none_or(|visible| visible.iter().any(|(audio_id, _)| *audio_id == id))
    }
    fn set_visible_media(&mut self, visible: Vec<(i32, bool)>) {
        let mut snapshot = Vec::<(i32, bool)>::new();
        for (id, missing) in visible {
            if id < 0 || self.track(id).is_none() {
                continue;
            }
            if let Some((_, old_missing)) = snapshot.iter_mut().find(|(old_id, _)| *old_id == id) {
                *old_missing |= missing;
            } else {
                snapshot.push((id, missing));
            }
        }
        self.visible_media = Some(snapshot);
        self.reconcile_media();
    }
    fn reconcile_media(&mut self) {
        let unwanted: Vec<_> = self
            .jobs
            .values()
            .filter(|job| {
                job.ticket.generation != self.generation || !self.media_wanted(job.ticket.audio_id)
            })
            .map(|job| job.ticket.serial)
            .collect();
        for serial in unwanted {
            if let Some(job) = self.jobs.get_mut(&serial) {
                for task in job.tasks.drain(..) {
                    task.abort();
                }
                job.artwork_pending = false;
                job.duration_pending = false;
                if !job.decoding {
                    self.jobs.remove(&serial);
                }
            }
        }
        if let Some(visible) = self.visible_media.clone() {
            self.queue.clear();
            for (id, missing) in visible {
                self.enqueue_media(id, missing);
            }
        }
        if self.upcoming.open {
            for (id, missing) in self.queue_media.clone() {
                self.enqueue_media(id, missing);
            }
        }
        if let Some(id) = self
            .selected_media
            .as_ref()
            .map(|track| track.audio_source_id)
            && let Some(missing) = self.selected_artwork_missing
        {
            self.enqueue_media(id, missing);
        }
    }
    fn request_media(&mut self, id: i32, missing: bool) {
        if id < 0 || self.track(id).is_none() {
            return;
        }
        if self
            .selected_media
            .as_ref()
            .is_some_and(|track| track.audio_source_id == id)
        {
            self.selected_artwork_missing = Some(missing);
        }
        if self.upcoming.open
            && self
                .upcoming
                .view
                .tracks
                .iter()
                .any(|track| track.audio_source_id == id)
        {
            self.queue_media.insert(id, missing);
        }
        if self.media_wanted(id) {
            self.enqueue_media(id, missing);
        }
    }
    fn enqueue_media(&mut self, id: i32, missing: bool) {
        if id < 0 || self.track(id).is_none() {
            return;
        }
        let requested_cover = self
            .track(id)
            .and_then(|track| track.cover_beatmap_id)
            .filter(|cover| missing && !self.unavailable.contains(cover));
        if let Some((ticket, previous_cover, active_artwork)) = self
            .jobs
            .values()
            .find(|job| job.ticket.audio_id == id && job.ticket.generation == self.generation)
            .map(|job| (job.ticket, job.cover, job.artwork_pending || job.decoding))
        {
            if let Some(cover) = requested_cover.filter(|_| !active_artwork) {
                if previous_cover.is_some() {
                    // A second delivery needs a fresh ticket so a repeated old ACK cannot release it.
                    if let Some(job) = self.jobs.remove(&ticket.serial) {
                        for task in job.tasks {
                            task.abort();
                        }
                    }
                } else {
                    if let Some(api) = self.session.as_ref().map(|session| session.api().clone()) {
                        let task = self.task(async move {
                            Completed::MediaArtwork {
                                ticket,
                                bytes: api.cover(cover).await.ok().flatten(),
                            }
                        });
                        if let Some(job) = self.jobs.get_mut(&ticket.serial) {
                            job.cover = Some(cover);
                            job.artwork_pending = true;
                            job.tasks.push(task);
                        }
                    }
                    return;
                }
            } else {
                return;
            }
        }
        let selected = self
            .selected_media
            .as_ref()
            .map(|track| track.audio_source_id)
            .or(self.selected)
            == Some(id);
        if let Some(index) = self.queue.iter().position(|(queued, _)| *queued == id) {
            if let Some(request) = self.queue.get_mut(index) {
                request.1 |= missing;
            }
            if selected && let Some(request) = self.queue.remove(index) {
                self.queue.push_front(request);
            }
            return;
        }
        if requested_cover.is_none() && self.durations.contains_key(&id) {
            return;
        }
        if selected {
            self.queue.push_front((id, missing));
        } else {
            self.queue.push_back((id, missing));
        }
    }
    fn start_media(&mut self) {
        if self.library.loading {
            return;
        }
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            return;
        };
        while self.jobs.len() < 4 {
            let Some((id, missing)) = self.queue.pop_front() else {
                break;
            };
            if !self.media_wanted(id) {
                continue;
            }
            let Some(track) = self.track(id) else {
                continue;
            };
            let cover = track
                .cover_beatmap_id
                .filter(|cover| missing && !self.unavailable.contains(cover));
            let duration_requested = !self.durations.contains_key(&id);
            if cover.is_none() && !duration_requested {
                continue;
            }
            self.serial = self.serial.wrapping_add(1);
            let ticket = MediaTicket {
                generation: self.generation,
                audio_id: id,
                serial: self.serial,
            };
            let mut tasks = Vec::new();
            if let Some(cover_id) = cover {
                let api = api.clone();
                tasks.push(self.task(async move {
                    Completed::MediaArtwork {
                        ticket,
                        bytes: api.cover(cover_id).await.ok().flatten(),
                    }
                }));
            }
            if duration_requested {
                let api = api.clone();
                tasks.push(self.task(async move {
                    Completed::MediaDuration {
                        ticket,
                        duration: api
                            .audio_duration(id)
                            .await
                            .ok()
                            .and_then(|result| result.duration_ms),
                    }
                }));
            }
            self.jobs.insert(
                ticket.serial,
                MediaJob {
                    ticket,
                    cover,
                    decoding: false,
                    artwork_pending: cover.is_some(),
                    duration_pending: duration_requested,
                    tasks,
                },
            );
        }
    }
    fn finish_media(&mut self, ticket: MediaTicket) {
        if self.jobs.get(&ticket.serial).is_some_and(|job| {
            job.ticket == ticket && !job.artwork_pending && !job.duration_pending && !job.decoding
        }) {
            self.jobs.remove(&ticket.serial);
        }
    }
    fn artwork_completed(&mut self, ticket: MediaTicket, bytes: Option<Vec<u8>>) {
        let Some(job) = self
            .jobs
            .get_mut(&ticket.serial)
            .filter(|job| job.ticket == ticket && job.artwork_pending)
        else {
            return;
        };
        job.artwork_pending = false;
        if ticket.generation == self.generation {
            match (job.cover, bytes) {
                (Some(cover_id), Some(bytes)) => {
                    job.decoding = true;
                    (self.emit)(AppUpdate::Artwork {
                        ticket,
                        cover_id,
                        bytes,
                    });
                }
                (Some(cover), None) => {
                    self.unavailable.insert(cover);
                }
                _ => {}
            }
        }
        self.finish_media(ticket);
    }
    fn duration_completed(&mut self, ticket: MediaTicket, duration: Option<u64>) {
        let Some(job) = self
            .jobs
            .get_mut(&ticket.serial)
            .filter(|job| job.ticket == ticket && job.duration_pending)
        else {
            return;
        };
        job.duration_pending = false;
        if ticket.generation == self.generation {
            let duration = duration.map(Duration::from_millis);
            self.durations.insert(ticket.audio_id, duration);
            for track in self
                .tracks
                .iter_mut()
                .chain(&mut self.playlists.tracks)
                .chain(&mut self.upcoming.view.tracks)
            {
                if track.audio_source_id == ticket.audio_id {
                    track.duration = duration;
                }
            }
            if let Some(track) = self
                .current_track
                .as_mut()
                .filter(|track| track.audio_source_id == ticket.audio_id)
            {
                track.duration = duration;
            }
            if let Some(track) = self
                .selected_media
                .as_mut()
                .filter(|track| track.audio_source_id == ticket.audio_id)
            {
                track.duration = duration;
            }
            if let Some(track) = self.track(ticket.audio_id) {
                (self.emit)(AppUpdate::TrackChanged(track.clone()));
            }
            if self
                .selected_media
                .as_ref()
                .is_some_and(|track| track.audio_source_id == ticket.audio_id)
            {
                self.emit_selection();
            }
            if self.upcoming.open {
                self.emit_queue();
            }
        }
        self.finish_media(ticket);
    }
    #[cfg(test)]
    fn media_completed(
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
}
fn failure(message: String, retry: SettingsRetry) -> OperationStatus {
    OperationStatus {
        message,
        retry: Some(retry),
        ..Default::default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    pub(super) fn controller() -> (Controller, Arc<Mutex<Vec<AppUpdate>>>) {
        let updates = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&updates);
        (
            {
                let mut controller = Controller::new(
                    ServerOptions::default(),
                    Arc::new(move |update| sink.lock().unwrap().push(update)),
                );
                controller.volume.settings = Some(crate::models::AudioSettings::default());
                controller
            },
            updates,
        )
    }
    pub(super) fn track(id: i32) -> Track {
        Track {
            audio_source_id: id,
            last_played_at_ms: None,
            volume_percent: None,
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
    pub(super) async fn test_session() -> (tempfile::TempDir, Session) {
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
        assert!(state.picking);
        assert!(state.folder_status.busy);
        assert!(matches!(
            updates.lock().unwrap().last(),
            Some(AppUpdate::FolderPickerRequested)
        ));
        state.command(AppCommand::CompleteFolderPick(None));
        assert!(!state.folder_status.busy);
        state.complete(Completed::Folders(Err("list failed".into())));
        state.command(AppCommand::RetryFolders);
        assert!(state.folder_status.loading);
        assert_eq!(state.tasks.len(), 1);
        state.command(AppCommand::BeginFolderPick);
        assert!(!state.picking, "loading prevents another picker");
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
        assert!(state.library.loading);
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
        assert!(state.jobs.is_empty(), "refresh pauses new media pipelines");
        assert_eq!(state.queue, VecDeque::from([(2, true), (1, true)]));
        state.complete(Completed::Library {
            request: state.library_request,
            invalidate_artwork: true,
            result: Err("refresh failed".into()),
        });
        assert_eq!(state.selected, Some(2));
        assert_eq!(state.tracks, vec![track(1), track(2)]);
        state.start_media();
        assert!(state.queue.is_empty());
        assert_eq!(state.jobs.len(), 2);
        assert_eq!(
            state.jobs.get(&1).unwrap().ticket.audio_id,
            2,
            "selection keeps priority"
        );
        assert_eq!(state.jobs.get(&2).unwrap().ticket.audio_id, 1);
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
        state.selected = Some(2);
        // Paused time lets us check the delay without relying on wall-clock scheduling.
        tokio::time::pause();
        state.command(AppCommand::SearchLibrary("r".into()));
        let old = state.library_request;
        let old_task = state.library_task.clone().unwrap();
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
            assert_eq!(state.tracks, vec![track(1), track(2)]);
            assert_eq!(state.selected, Some(2));
            assert!(state.library.loading);
            assert_eq!(updates.lock().unwrap().len(), notifications);
        }
        // At 200 ms the fake session's refused connection completes the current task.
        tokio::time::advance(Duration::from_millis(1)).await;
        let completed = state.tasks.join_next().await.unwrap().unwrap();
        state.complete(completed);
        assert!(!state.library.loading);
        assert_eq!(state.library.retry, Some(SettingsRetry::Load));
        state.command(AppCommand::RefreshLibrary);
        assert_eq!(state.library_query, "roc");
        state.complete(Completed::Library {
            request: state.library_request,
            invalidate_artwork: true,
            result: Ok(vec![track(2)]),
        });
        assert_eq!(state.selected, Some(2));
        assert_eq!(state.library.retry, None);
        state.command(AppCommand::SearchLibrary("nothing".into()));
        state.complete(Completed::Library {
            request: state.library_request,
            invalidate_artwork: true,
            result: Ok(vec![]),
        });
        assert_eq!(state.library.message, "Nothing found.");
        let before_clear = state.library_request;
        state.command(AppCommand::SearchLibrary(String::new()));
        state.complete(Completed::Library {
            request: before_clear,
            invalidate_artwork: true,
            result: Err("late".into()),
        });
        assert!(state.library.loading);
        state.complete(Completed::Library {
            request: state.library_request,
            invalidate_artwork: true,
            result: Ok(vec![track(1), track(2)]),
        });
        assert_eq!(state.tracks.len(), 2);
        assert!(state.library.message.is_empty());
        state.tasks.abort_all();
        tokio::time::resume();
        state.session.take().unwrap().shutdown().await.unwrap();
    }

    #[test]
    fn selection_survives_refresh_shrinking_and_empty_library() {
        let (mut state, updates) = controller();
        state.replace_tracks(vec![track(1), track(2)], true);
        assert_eq!(state.selected, Some(1));
        state.command(AppCommand::SelectTrack(Some(2)));
        state.replace_tracks(vec![track(2), track(1)], true);
        assert_eq!(state.selected, Some(2));
        state.replace_tracks(vec![track(1)], true);
        assert_eq!(state.selected, Some(1));
        state.command(AppCommand::SelectTrack(None));
        assert_eq!(state.selected, None);
        state.command(AppCommand::SelectTrack(Some(404)));
        assert_eq!(state.selected, None);
        state.replace_tracks(vec![], true);
        assert!(state.library.message.contains("No songs"));
        assert!(matches!(
            updates.lock().unwrap().last(),
            Some(AppUpdate::TrackSelected(None))
        ));
    }
    #[test]
    fn list_failures_are_independent_and_preserve_previous_rows() {
        let (mut state, _) = controller();
        state.replace_tracks(vec![track(1)], true);
        state.complete(Completed::Folders(Err("folders failed".into())));
        assert_eq!(state.folder_status.retry, Some(SettingsRetry::Load));
        assert_eq!(state.library.retry, None);
        state.complete(Completed::Library {
            request: state.library_request,
            invalidate_artwork: true,
            result: Err("library failed".into()),
        });
        assert_eq!(state.tracks, vec![track(1)]);
        assert_eq!(state.selected, Some(1));
        state.complete(Completed::Folders(Ok(vec![folder(3)])));
        assert_eq!(state.selected_folder, Some(3));
        assert_eq!(state.folder_status.retry, None);
        assert_eq!(state.library.retry, Some(SettingsRetry::Load));
        state.complete(Completed::Library {
            request: state.library_request,
            invalidate_artwork: true,
            result: Ok(vec![track(2)]),
        });
        assert_eq!(state.library.retry, None);
        assert_eq!(state.selected, Some(2));
    }
    #[test]
    fn folder_cancel_duplicate_and_selection_preserve_stored_values() {
        let (mut state, _) = controller();
        state.replace_folders(vec![folder(1), folder(2)]);
        state.command(AppCommand::SelectFolder(Some(2)));
        state.replace_folders(vec![folder(2), folder(1)]);
        assert_eq!(state.selected_folder, Some(2));
        state.picking = true;
        state.folder_status.busy = true;
        state.complete_pick(None);
        assert_eq!(state.selected_folder, Some(2));
        assert!(!state.folder_status.busy);
        state.complete(Completed::Registered(Ok(folder(3))));
        state.complete(Completed::Registered(Ok(folder(1))));
        assert_eq!(state.selected_folder, Some(1));
        assert_eq!(state.folders, vec![folder(2), folder(1), folder(3)]);
        state.complete(Completed::Registered(Err("registration failed".into())));
        assert_eq!(state.folder_status.retry, Some(SettingsRetry::Browse));
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
            state.queue,
            VecDeque::from([(1, true), (2, true), (3, true)])
        );
        state.command(AppCommand::SelectTrack(Some(3)));
        assert_eq!(state.queue.front(), Some(&(3, true)));
    }
    #[test]
    fn evicted_cover_refetch_does_not_repeat_duration() {
        let (mut state, _) = controller();
        state.replace_tracks(vec![track(1)], true);
        state.durations.insert(1, None);
        state.request_media(1, false);
        assert!(state.queue.is_empty());
        state.request_media(1, true);
        assert_eq!(state.queue.pop_front(), Some((1, true)));
        assert!(state.durations.contains_key(&1));
        state.unavailable.insert(101);
        state.request_media(1, true);
        assert!(state.queue.is_empty());
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
            state.jobs.insert(
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
        assert_eq!(state.jobs.len(), 4);
        assert!(state.jobs.values().all(|job| job.decoding));
        for invalidate in [false, true, false] {
            state.replace_tracks((1..=5).map(track).collect(), invalidate);
        }
        assert_eq!(
            state.jobs.len(),
            4,
            "refresh cannot cancel a blocking decoder"
        );
        state.serial = 4;
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
        assert!(state.jobs.is_empty());
        assert!(
            state.unavailable.is_empty(),
            "stale decode failure cannot poison current cache state"
        );
        assert!(updates.lock().unwrap().is_empty());
        state.start_media();
        assert_eq!(
            state.jobs.len(),
            2,
            "released slots serve the retained selection and the newly visible row"
        );
        assert_eq!(
            state.jobs.get(&5).unwrap().ticket.audio_id,
            1,
            "selection keeps priority"
        );
        assert_eq!(state.jobs.get(&6).unwrap().ticket.audio_id, 5);
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
        state.jobs.insert(
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
            state.jobs.len(),
            1,
            "HTTP pipeline needs no installation ack yet"
        );
        state.replace_tracks(vec![track(1)], true);
        updates.lock().unwrap().clear();
        state.media_completed(ticket, Some(vec![3]), Some(6000), true);
        assert!(state.jobs.is_empty());
        assert_eq!(state.tracks.first().unwrap().duration, None);
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
}

#[cfg(test)]
mod benchmarks;

#[cfg(test)]
mod media_tests {
    use super::tests::{controller, track};
    use super::*;

    fn pipeline(state: &mut Controller, audio_id: i32, serial: u64) -> MediaTicket {
        let ticket = MediaTicket {
            generation: state.generation,
            audio_id,
            serial,
        };
        state.jobs.insert(
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
            state.current_track = Some(track(1));
            state.request_media(1, true);
            state.set_visible_media(vec![(1, true)]);
            state.upcoming.open = true;
            state.queue_media.insert(1, true);
            let ticket = pipeline(&mut state, 1, 1);
            updates.lock().unwrap().clear();
            state.artwork_completed(ticket, Some(vec![1, 2, 3]));
            assert!(state.jobs.get(&1).unwrap().decoding);
            assert!(state.jobs.get(&1).unwrap().duration_pending);
            assert_eq!(state.tracks.first().unwrap().duration, None);
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
                state.jobs.get(&1).unwrap().decoding,
                "forged ACK cannot release a slot"
            );
            if duration_first {
                state.duration_completed(ticket, Some(42_000));
                assert_eq!(state.jobs.len(), 1, "duration cannot release a decoder");
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
                !state.unavailable.contains(&101),
                "repeated ACK cannot poison a successful image"
            );
            if !duration_first {
                assert_eq!(
                    state.jobs.len(),
                    1,
                    "ACK cannot release pending duration HTTP"
                );
                state.duration_completed(ticket, Some(42_000));
            }
            assert!(state.jobs.is_empty());
            assert_eq!(
                state.current_track.as_ref().unwrap().duration,
                Some(Duration::from_secs(42))
            );
            assert_eq!(
                state.selected_media.as_ref().unwrap().duration,
                Some(Duration::from_secs(42))
            );
            state.duration_completed(ticket, Some(99_000));
            assert_eq!(
                state.durations.get(&1),
                Some(&Some(Duration::from_secs(42)))
            );
            state.set_visible_media(vec![(1, false)]);
            assert!(
                state.queue.is_empty(),
                "successful installation consumes all artwork demand"
            );
            assert_eq!(state.selected_artwork_missing, Some(false));
            assert_eq!(state.queue_media.get(&1), Some(&false));
        }
    }

    #[test]
    fn viewport_snapshots_replace_historical_demand_and_deduplicate_audio() {
        let (mut state, _) = controller();
        state.replace_tracks((1..=100).map(track).collect(), true);
        state.command(AppCommand::SelectTrack(None));
        state.set_visible_media((1..=30).map(|id| (id, true)).collect());
        state.set_visible_media(vec![(90, false), (91, true), (90, true)]);
        assert_eq!(state.visible_media, Some(vec![(90, true), (91, true)]));
        assert_eq!(state.queue, VecDeque::from([(90, true), (91, true)]));
        state.request_media(2, true);
        assert_eq!(
            state.queue.len(),
            2,
            "offscreen compatibility request cannot extend an active snapshot"
        );
        state.command(AppCommand::SelectTrack(Some(50)));
        state.request_media(50, true);
        state.set_visible_media(Vec::new());
        assert_eq!(
            state.queue,
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
        state.jobs.get_mut(&1).unwrap().tasks.push(http.clone());
        state.jobs.get_mut(&2).unwrap().tasks.push(duration.clone());
        state.artwork_completed(second, Some(vec![2]));
        state.set_visible_media(vec![(3, true)]);
        assert!(!state.jobs.contains_key(&1));
        assert!(state.jobs.get(&2).unwrap().decoding);
        assert!(!state.jobs.get(&2).unwrap().duration_pending);
        while state.tasks.join_next().await.is_some() {}
        assert!(http.is_finished() && duration.is_finished());
        state.artwork_completed(first, Some(vec![1]));
        state.replace_tracks((1..=3).map(track).collect(), false);
        state.duration_completed(second, Some(123_000));
        assert!(
            state.jobs.get(&2).unwrap().decoding,
            "late duration cannot release old decoder"
        );
        state.command(AppCommand::MediaInstalled {
            ticket: second,
            available: false,
        });
        assert!(state.jobs.is_empty());
        assert!(state.unavailable.is_empty() && state.durations.is_empty());
    }

    #[test]
    fn navigation_preserves_metadata_and_true_invalidation_clears_it() {
        let (mut state, updates) = controller();
        state.replace_tracks(vec![track(1)], true);
        updates.lock().unwrap().clear();
        state.durations.insert(1, Some(Duration::from_secs(42)));
        state.unavailable.insert(101);
        state.replace_tracks(vec![track(1)], false);
        assert_eq!(
            state.tracks.first().unwrap().duration,
            Some(Duration::from_secs(42))
        );
        state.playlist_action(PlaylistAction::ShowPlaylists);
        state.playlist_action(PlaylistAction::ShowLibrary);
        assert!(state.unavailable.contains(&101));
        assert_eq!(
            state.durations.get(&1),
            Some(&Some(Duration::from_secs(42)))
        );
        assert_eq!(
            state.generation, 4,
            "each replacement advances the adapter ticket contract"
        );
        let replacements: Vec<_> = updates
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| {
                if let AppUpdate::TracksReplaced {
                    invalidate_artwork, ..
                } = event
                {
                    Some(*invalidate_artwork)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(replacements, [false, false, false]);
        state.replace_tracks(vec![track(1)], true);
        assert!(state.unavailable.is_empty() && state.durations.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn superseding_search_retains_refresh_invalidation_until_success() {
        let (_directory, session) = super::tests::test_session().await;
        let (mut state, updates) = controller();
        state.session = Some(session);
        state.replace_tracks(vec![track(1)], true);
        state.library_invalidation_pending = false;
        state.durations.insert(1, Some(Duration::from_secs(42)));
        state.command(AppCommand::RefreshLibrary);
        let refresh = state.library_request;
        state.command(AppCommand::SearchLibrary("latest".into()));
        state.complete(Completed::Library {
            request: refresh,
            invalidate_artwork: true,
            result: Ok(Vec::new()),
        });
        assert_eq!(state.tracks.len(), 1);
        state.complete(Completed::Library {
            request: state.library_request,
            invalidate_artwork: true,
            result: Err("offline".into()),
        });
        assert_eq!(
            state.durations.get(&1),
            Some(&Some(Duration::from_secs(42)))
        );
        assert!(
            state.library_invalidation_pending,
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
            request: state.library_request,
            invalidate_artwork: true,
            result: Ok(vec![track(1)]),
        });
        assert!(!state.library_invalidation_pending && state.durations.is_empty());
        assert!(updates.lock().unwrap().iter().any(|event| matches!(
            event,
            AppUpdate::TracksReplaced {
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
        let original = state.jobs.get(&1).unwrap().ticket;
        assert!(state.jobs.get(&1).unwrap().cover.is_none());
        state.request_media(1, true);
        assert_eq!(state.jobs.len(), 1);
        assert!(state.jobs.get(&1).unwrap().artwork_pending);
        state.artwork_completed(original, Some(vec![1]));
        state.command(AppCommand::MediaInstalled {
            ticket: original,
            available: true,
        });
        assert!(state.jobs.get(&1).unwrap().duration_pending);
        state.request_media(1, true); // The installed cover was evicted while duration HTTP remained pending.
        state.start_media();
        let replacement = state.jobs.get(&2).unwrap().ticket;
        assert_ne!(replacement.serial, original.serial);
        assert_eq!(state.jobs.len(), 1);
        state.artwork_completed(replacement, Some(vec![2]));
        state.command(AppCommand::MediaInstalled {
            ticket: original,
            available: false,
        });
        state.duration_completed(original, Some(99_000));
        assert!(state.jobs.get(&2).unwrap().decoding);
        assert!(state.jobs.get(&2).unwrap().duration_pending);
        assert!(!state.unavailable.contains(&101));
        state.duration_completed(replacement, Some(42_000));
        state.command(AppCommand::MediaInstalled {
            ticket: replacement,
            available: true,
        });
        state.set_visible_media(vec![(1, false)]);
        assert!(state.jobs.is_empty() && state.queue.is_empty());
        state.tasks.abort_all();
        state.session.take().unwrap().shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_cancels_http_without_waiting_for_missing_decode_ack() {
        let (mut state, _) = controller();
        state.replace_tracks(vec![track(1)], true);
        let ticket = pipeline(&mut state, 1, 1);
        let task = state.task(std::future::pending());
        state.jobs.get_mut(&1).unwrap().tasks.push(task.clone());
        state.artwork_completed(ticket, Some(vec![1]));
        let (sender, receiver) = mpsc::unbounded_channel();
        sender.send(AppCommand::Shutdown).unwrap();
        tokio::time::timeout(Duration::from_secs(2), state.run(receiver))
            .await
            .unwrap();
        assert!(task.is_finished());
    }
}
