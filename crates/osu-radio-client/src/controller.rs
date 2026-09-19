//! Shared application workflows. Adapters own presentation and decoded artwork, and acknowledge
//! every artwork delivery only after decoding and cache installation (or stale-result discard).
use crate::{
    ApiClient, OsuFolder, RegisterOsuFolder, ServerOptions, Session, Track, describe,
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
    Connect,
    RefreshLibrary,
    SearchLibrary(String),
    RefreshFolders,
    SelectTrack(Option<i32>),
    SelectFolder(Option<i32>),
    BeginFolderPick,
    CompleteFolderPick(Option<PathBuf>),
    RetryFolders,
    RequestMedia {
        audio_id: i32,
        artwork_missing: bool,
    },
    MediaInstalled {
        ticket: MediaTicket,
        available: bool,
    },
    Shutdown,
}
#[derive(Clone, Debug)]
pub enum AppUpdate {
    Connection(ConnectionStatus),
    LibraryStatus(OperationStatus),
    FolderStatus(OperationStatus),
    TracksReplaced(Vec<Track>),
    TrackChanged(Track),
    TrackSelected(Option<Track>),
    FoldersReplaced(Vec<OsuFolder>),
    FolderSelected(Option<i32>),
    FolderPickerRequested,
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
    Connected(Result<Session, String>),
    Library {
        request: u64,
        result: Result<Vec<Track>, String>,
    },
    Folders(Result<Vec<OsuFolder>, String>),
    Registered(Result<OsuFolder, String>),
    Media {
        ticket: MediaTicket,
        bytes: Option<Vec<u8>>,
        duration: Option<u64>,
        duration_requested: bool,
    },
    Cancelled,
}
struct MediaJob {
    ticket: MediaTicket,
    cover: Option<i32>,
    decoding: bool,
}
struct Controller {
    options: ServerOptions,
    emit: Arc<dyn Fn(AppUpdate) + Send + Sync>,
    session: Option<Session>,
    connecting: bool,
    tasks: JoinSet<Completed>,
    cancel: watch::Sender<bool>,
    tracks: Vec<Track>,
    track_indices: HashMap<i32, usize>,
    selected: Option<i32>,
    folders: Vec<OsuFolder>,
    selected_folder: Option<i32>,
    library: OperationStatus,
    library_query: String,
    library_request: u64,
    library_task: Option<AbortHandle>,
    library_deadline: Option<tokio::time::Instant>,
    folder_status: OperationStatus,
    picking: bool,
    generation: u64,
    serial: u64,
    queue: VecDeque<(i32, bool)>,
    jobs: HashMap<u64, MediaJob>,
    duration_done: HashSet<i32>,
    unavailable: HashSet<i32>,
}
impl Controller {
    fn new(options: ServerOptions, emit: Arc<dyn Fn(AppUpdate) + Send + Sync>) -> Self {
        let (cancel, _) = watch::channel(false);
        Self {
            options,
            emit,
            session: None,
            connecting: false,
            tasks: JoinSet::new(),
            cancel,
            tracks: Vec::new(),
            track_indices: HashMap::new(),
            selected: None,
            folders: Vec::new(),
            selected_folder: None,
            library: OperationStatus::default(),
            library_query: String::new(),
            library_request: 0,
            library_task: None,
            library_deadline: None,
            folder_status: OperationStatus::default(),
            picking: false,
            generation: 0,
            serial: 0,
            queue: VecDeque::new(),
            jobs: HashMap::new(),
            duration_done: HashSet::new(),
            unavailable: HashSet::new(),
        }
    }
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<AppCommand>) {
        loop {
            tokio::select! {
                command = commands.recv() => match command {
                    Some(AppCommand::Shutdown) | None => break,
                    Some(command) => self.command(command),
                },
                result = self.tasks.join_next(), if !self.tasks.is_empty() => {
                    if let Some(Ok(result)) = result { self.complete(result); }
                }
            }
            self.start_media();
        }
        let _ = self.cancel.send(true);
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
    fn command(&mut self, command: AppCommand) {
        match command {
            AppCommand::Connect => self.connect(),
            AppCommand::RefreshLibrary => self.refresh_library(),
            AppCommand::SearchLibrary(query) => {
                if self.library_query != query {
                    self.library_query = query;
                    self.load_library(Duration::from_millis(200));
                }
            }
            AppCommand::RefreshFolders => self.refresh_folders(),
            AppCommand::SelectTrack(id) => {
                if id.is_none_or(|id| self.track_indices.contains_key(&id)) {
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
            AppCommand::MediaInstalled { ticket, available } => {
                if self
                    .jobs
                    .get(&ticket.serial)
                    .is_some_and(|job| job.ticket == ticket && job.decoding)
                    && let Some(job) = self.jobs.remove(&ticket.serial)
                    && ticket.generation == self.generation
                    && !available
                    && let Some(cover) = job.cover
                {
                    self.unavailable.insert(cover);
                }
            }
            AppCommand::Shutdown => {}
        }
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
        self.load_library(Duration::ZERO);
    }
    fn load_library(&mut self, delay: Duration) {
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
    fn complete(&mut self, result: Completed) {
        match result {
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
                        self.load_library(delay);
                        self.refresh_folders();
                    }
                    Err(error) => {
                        self.library = failure(error.clone(), SettingsRetry::Load);
                        self.folder_status = self.library.clone();
                        (self.emit)(AppUpdate::Connection(ConnectionStatus::Failed(error)));
                        self.emit_statuses();
                    }
                }
            }
            Completed::Library { request, result } => {
                if request != self.library_request {
                    return;
                }
                self.library_task = None;
                self.library = OperationStatus::default();
                match result {
                    Ok(tracks) => self.replace_tracks(tracks),
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
            Completed::Media {
                ticket,
                bytes,
                duration,
                duration_requested,
            } => self.media_completed(ticket, bytes, duration, duration_requested),
            Completed::Cancelled => {}
        }
    }
    fn replace_tracks(&mut self, tracks: Vec<Track>) {
        self.generation = self.generation.wrapping_add(1);
        self.queue.clear();
        self.duration_done.clear();
        self.unavailable.clear();
        self.selected = selection_after_refresh(&tracks, self.selected);
        self.track_indices = tracks
            .iter()
            .enumerate()
            .map(|(index, track)| (track.audio_source_id, index))
            .collect();
        self.tracks = tracks;
        if self.tracks.is_empty() {
            self.library.message = if self.library_query.trim().is_empty() {
                "No songs yet. Import a registered folder with the CLI."
            } else {
                "Nothing found."
            }
            .into();
        }
        (self.emit)(AppUpdate::TracksReplaced(self.tracks.clone()));
        self.emit_selection();
    }
    fn replace_folders(&mut self, folders: Vec<OsuFolder>) {
        self.selected_folder = self
            .selected_folder
            .filter(|id| folders.iter().any(|folder| folder.id == *id))
            .or_else(|| folders.first().map(|folder| folder.id));
        self.folders = folders;
        (self.emit)(AppUpdate::FoldersReplaced(self.folders.clone()));
        (self.emit)(AppUpdate::FolderSelected(self.selected_folder));
    }
    fn emit_selection(&self) {
        (self.emit)(AppUpdate::TrackSelected(
            self.selected.and_then(|id| self.track(id)).cloned(),
        ));
    }
    fn emit_statuses(&self) {
        (self.emit)(AppUpdate::LibraryStatus(self.library.clone()));
        (self.emit)(AppUpdate::FolderStatus(self.folder_status.clone()));
    }
    fn track(&self, id: i32) -> Option<&Track> {
        self.track_indices
            .get(&id)
            .and_then(|index| self.tracks.get(*index))
    }
    fn request_media(&mut self, id: i32, missing: bool) {
        if self.track(id).is_none() {
            return;
        }
        if self
            .jobs
            .values()
            .any(|job| job.ticket.audio_id == id && job.ticket.generation == self.generation)
        {
            return;
        }
        if let Some((_, artwork_missing)) = self.queue.iter_mut().find(|(queued, _)| *queued == id)
        {
            *artwork_missing |= missing;
            return;
        }
        let needs_cover = missing
            && self
                .track(id)
                .and_then(|track| track.cover_beatmap_id)
                .is_some_and(|cover| !self.unavailable.contains(&cover));
        if !needs_cover && self.duration_done.contains(&id) {
            return;
        }
        if self.selected == Some(id) {
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
            let Some(track) = self.track(id) else {
                continue;
            };
            let cover = track
                .cover_beatmap_id
                .filter(|cover| missing && !self.unavailable.contains(cover));
            let duration_requested = !self.duration_done.contains(&id);
            if cover.is_none() && !duration_requested {
                continue;
            }
            self.serial = self.serial.wrapping_add(1);
            let ticket = MediaTicket {
                generation: self.generation,
                audio_id: id,
                serial: self.serial,
            };
            self.jobs.insert(
                ticket.serial,
                MediaJob {
                    ticket,
                    cover,
                    decoding: false,
                },
            );
            self.task(fetch_media(api.clone(), ticket, cover, duration_requested));
        }
    }
    fn media_completed(
        &mut self,
        ticket: MediaTicket,
        bytes: Option<Vec<u8>>,
        duration: Option<u64>,
        duration_requested: bool,
    ) {
        let Some(job) = self.jobs.get_mut(&ticket.serial) else {
            return;
        };
        if ticket.generation != self.generation {
            self.jobs.remove(&ticket.serial);
            return;
        }
        if duration_requested {
            self.duration_done.insert(ticket.audio_id);
            if let Some(index) = self.track_indices.get(&ticket.audio_id)
                && let Some(track) = self.tracks.get_mut(*index)
            {
                track.duration = duration.map(Duration::from_millis);
                (self.emit)(AppUpdate::TrackChanged(track.clone()));
                if self.selected == Some(ticket.audio_id) {
                    (self.emit)(AppUpdate::TrackSelected(Some(track.clone())));
                }
            }
        }
        match (job.cover, bytes) {
            (Some(cover_id), Some(bytes)) => {
                job.decoding = true;
                (self.emit)(AppUpdate::Artwork {
                    ticket,
                    cover_id,
                    bytes,
                });
            }
            (cover, _) => {
                if let Some(cover) = cover {
                    self.unavailable.insert(cover);
                }
                self.jobs.remove(&ticket.serial);
            }
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
async fn fetch_media(
    api: ApiClient,
    ticket: MediaTicket,
    cover: Option<i32>,
    duration_requested: bool,
) -> Completed {
    let bytes = match cover {
        Some(id) => api.cover(id).await.ok().flatten(),
        None => None,
    };
    let duration = if duration_requested {
        api.audio_duration(ticket.audio_id)
            .await
            .ok()
            .and_then(|result| result.duration_ms)
    } else {
        None
    };
    Completed::Media {
        ticket,
        bytes,
        duration,
        duration_requested,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn controller() -> (Controller, Arc<Mutex<Vec<AppUpdate>>>) {
        let updates = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&updates);
        (
            Controller::new(
                ServerOptions::default(),
                Arc::new(move |update| sink.lock().unwrap().push(update)),
            ),
            updates,
        )
    }
    fn track(id: i32) -> Track {
        Track {
            audio_source_id: id,
            cover_beatmap_id: Some(id.saturating_add(100)),
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
    async fn test_session() -> (tempfile::TempDir, Session) {
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
        state.replace_tracks(vec![track(1), track(2)]);
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
        state.replace_tracks(vec![track(1), track(2)]);
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
            result: Ok(vec![track(2)]),
        });
        assert_eq!(state.selected, Some(2));
        assert_eq!(state.library.retry, None);
        state.command(AppCommand::SearchLibrary("nothing".into()));
        state.complete(Completed::Library {
            request: state.library_request,
            result: Ok(vec![]),
        });
        assert_eq!(state.library.message, "Nothing found.");
        let before_clear = state.library_request;
        state.command(AppCommand::SearchLibrary(String::new()));
        state.complete(Completed::Library {
            request: before_clear,
            result: Err("late".into()),
        });
        assert!(state.library.loading);
        state.complete(Completed::Library {
            request: state.library_request,
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
        state.replace_tracks(vec![track(1), track(2)]);
        assert_eq!(state.selected, Some(1));
        state.command(AppCommand::SelectTrack(Some(2)));
        state.replace_tracks(vec![track(2), track(1)]);
        assert_eq!(state.selected, Some(2));
        state.replace_tracks(vec![track(1)]);
        assert_eq!(state.selected, Some(1));
        state.command(AppCommand::SelectTrack(None));
        assert_eq!(state.selected, None);
        state.command(AppCommand::SelectTrack(Some(404)));
        assert_eq!(state.selected, None);
        state.replace_tracks(vec![]);
        assert!(state.library.message.contains("No songs"));
        assert!(matches!(
            updates.lock().unwrap().last(),
            Some(AppUpdate::TrackSelected(None))
        ));
    }
    #[test]
    fn list_failures_are_independent_and_preserve_previous_rows() {
        let (mut state, _) = controller();
        state.replace_tracks(vec![track(1)]);
        state.complete(Completed::Folders(Err("folders failed".into())));
        assert_eq!(state.folder_status.retry, Some(SettingsRetry::Load));
        assert_eq!(state.library.retry, None);
        state.complete(Completed::Library {
            request: state.library_request,
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
        state.replace_tracks(vec![track(1), track(2), track(3)]);
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
        state.replace_tracks(vec![track(1)]);
        state.duration_done.insert(1);
        state.request_media(1, false);
        assert!(state.queue.is_empty());
        state.request_media(1, true);
        assert_eq!(state.queue.pop_front(), Some((1, true)));
        assert!(state.duration_done.contains(&1));
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
        state.replace_tracks((1..=5).map(track).collect());
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
                },
            );
            state.media_completed(ticket, Some(vec![1, 2]), Some(42), true);
        }
        assert_eq!(state.jobs.len(), 4);
        assert!(state.jobs.values().all(|job| job.decoding));
        for _ in 0..3 {
            state.replace_tracks((1..=5).map(track).collect());
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
            1,
            "the acknowledged slot can now run current media"
        );
        assert_eq!(state.tasks.len(), 1);
        state.tasks.abort_all();
        state.session.take().unwrap().shutdown().await.unwrap();
    }
    #[test]
    fn stale_network_results_and_forged_acks_do_not_change_current_tracks() {
        let (mut state, updates) = controller();
        state.replace_tracks(vec![track(1)]);
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
        state.replace_tracks(vec![track(1)]);
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
