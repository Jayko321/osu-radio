use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use osu_radio_client::{
    OsuFolder, RegisterOsuFolder, ServerOptions, Session, Track, describe,
    view_models::{library_tracks, selection_after_refresh},
};
use tokio::{runtime::Handle, task::JoinHandle};
use vizia::prelude::*;

use crate::{assets, views};

pub fn run(runtime: Handle) -> Result<(), ApplicationError> {
    Application::new(move |cx| {
        crate::input::install(cx);
        assets::register(cx);
        views::styles(cx);
        let state = UiState::new();
        AppData {
            session: None,
            runtime,
            state,
            generation: 0,
            closed: false,
            connecting: false,
            jobs: HashMap::new(),
            queue: VecDeque::new(),
            duration_done: HashSet::new(),
            missing_covers: HashSet::new(),
        }
        .build(cx);
        cx.emit(AppEvent::Connect);
        views::shell(cx, state);
    })
    .title("osu! radio")
    .inner_size((1440u32, 952u32))
    .min_inner_size(Some((1024u32, 640u32)))
    .decorations(false)
    .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Songs,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRetry {
    Load,
    Browse,
}

#[derive(Debug, Clone, Copy)]
pub struct UiState {
    pub tab: Signal<Tab>,
    pub playing: Signal<Option<i32>>,
    pub selected: Signal<Option<Track>>,
    pub tracks: Signal<Vec<Track>>,
    pub artwork_revision: Signal<u64>,
    pub library_revision: Signal<u64>,
    pub library_message: Signal<String>,
    pub library_loading: Signal<bool>,
    pub settings_message: Signal<String>,
    pub settings_loading: Signal<bool>,
    pub settings_retry: Signal<Option<SettingsRetry>>,
    pub connected: Signal<bool>,
    pub folders: Signal<Vec<OsuFolder>>,
    pub selected_folder: Signal<Option<i32>>,
    pub busy: Signal<bool>,
    pub browsing: Signal<bool>,
    pub song_query: Signal<String>,
    pub settings_query: Signal<String>,
    pub status: Signal<String>,
    /// OS-side maximize is not exposed by Vizia and may desynchronize this toggle.
    pub maximized: Signal<bool>,
}
impl UiState {
    fn new() -> Self {
        Self {
            tab: Signal::new(Tab::Songs),
            playing: Signal::new(None),
            selected: Signal::new(None),
            tracks: Signal::new(Vec::new()),
            artwork_revision: Signal::new(0),
            library_revision: Signal::new(0),
            library_message: Signal::new("Connecting…".into()),
            library_loading: Signal::new(false),
            settings_message: Signal::new("Connecting…".into()),
            settings_loading: Signal::new(false),
            settings_retry: Signal::new(None),
            connected: Signal::new(false),
            folders: Signal::new(Vec::new()),
            selected_folder: Signal::new(None),
            busy: Signal::new(false),
            browsing: Signal::new(false),
            song_query: Signal::new(String::new()),
            settings_query: Signal::new(String::new()),
            status: Signal::new("starting the embedded server...".into()),
            maximized: Signal::new(false),
        }
    }

    fn set_folders(self, folders: Vec<OsuFolder>) {
        let selected = self.selected_folder.get();
        self.selected_folder.set(
            selected
                .filter(|id| folders.iter().any(|folder| folder.id == *id))
                .or_else(|| folders.first().map(|folder| folder.id)),
        );
        self.folders.set(folders);
    }

    fn folder_registered(self, folder: OsuFolder) {
        let mut folders = self.folders.get();
        self.selected_folder.set(Some(folder.id));
        if let Some(existing) = folders.iter_mut().find(|row| row.id == folder.id) {
            *existing = folder;
        } else {
            folders.push(folder);
        }
        self.set_folders(folders);
    }
}

struct AppData {
    session: Option<Arc<Session>>,
    runtime: Handle,
    state: UiState,
    generation: u64,
    closed: bool,
    connecting: bool,
    jobs: HashMap<i32, JoinHandle<()>>,
    queue: VecDeque<i32>,
    duration_done: HashSet<i32>,
    missing_covers: HashSet<i32>,
}

pub enum AppEvent {
    Connect,
    Connected(Arc<Session>),
    Failed(String),
    RefreshLibrary,
    LibraryLoaded(u64, Result<Vec<Track>, String>),
    RefreshSettings,
    SettingsLoaded(Result<Vec<OsuFolder>, String>),
    RequestMedia(i32),
    MediaLoaded(u64, i32, Option<i32>, Option<vizia::vg::Image>, Option<u64>),
    Browse,
    Browsed(Option<PathBuf>),
    FolderRegistered(Result<OsuFolder, String>),
    SelectTab(Tab),
    SelectTrack(i32),
    DragWindow,
    MinimizeWindow,
    ToggleMaximizeWindow,
    CloseWindow,
}

impl Model for AppData {
    #[allow(clippy::too_many_lines)] // UI event dispatch keeps state transitions in one place.
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|app_event, _| {
            if self.closed {
                return;
            }
            match app_event {
                AppEvent::Connect => self.connect(cx),
                AppEvent::Connected(session) => {
                    self.connecting = false;
                    self.state.connected.set(true);
                    self.session = Some(Arc::clone(session));
                    self.state.status.set(String::new());
                    self.refresh_library(cx);
                    self.refresh_settings(cx);
                }
                AppEvent::Failed(reason) => {
                    self.connecting = false;
                    self.state.connected.set(false);
                    self.state.settings_retry.set(Some(SettingsRetry::Load));
                    self.state.status.set(reason.clone());
                    self.state.library_message.set(reason.clone());
                    self.state.settings_message.set(reason.clone());
                }
                AppEvent::RefreshLibrary => self.refresh_library(cx),
                AppEvent::LibraryLoaded(generation, result) if *generation == self.generation => {
                    self.state.library_loading.set(false);
                    match result {
                        Ok(tracks) => {
                            self.duration_done.clear();
                            self.missing_covers.clear();
                            assets::clear_covers();
                            self.state
                                .artwork_revision
                                .set(self.state.artwork_revision.get().wrapping_add(1));
                            self.state.library_message.set(if tracks.is_empty() {
                                "No songs yet. Import a registered folder with the CLI.".into()
                            } else {
                                String::new()
                            });
                            self.state
                                .playing
                                .set(selection_after_refresh(tracks, self.state.playing.get()));
                            self.state.tracks.set(tracks.clone());
                            self.state
                                .library_revision
                                .set(self.state.library_revision.get().wrapping_add(1));
                            self.update_selection(cx);
                        }
                        Err(error) => self.state.library_message.set(error.clone()),
                    }
                }
                AppEvent::RefreshSettings => self.refresh_settings(cx),
                AppEvent::SettingsLoaded(result) => {
                    self.state.settings_loading.set(false);
                    match result {
                        Ok(folders) => {
                            self.state.settings_message.set(String::new());
                            self.state.set_folders(folders.clone());
                        }
                        Err(error) => {
                            self.state.settings_retry.set(Some(SettingsRetry::Load));
                            self.state.settings_message.set(error.clone());
                        }
                    }
                }
                AppEvent::RequestMedia(id) => self.request_media(cx, *id),
                AppEvent::MediaLoaded(generation, id, cover, image, duration)
                    if *generation == self.generation =>
                {
                    self.jobs.remove(id);
                    self.duration_done.insert(*id);
                    if let Some(cover) = cover {
                        if let Some(image) = image {
                            assets::cache_cover(*cover, image.clone());
                        } else if !assets::has_cover(*cover) {
                            self.missing_covers.insert(*cover);
                        }
                    }
                    let mut tracks = self.state.tracks.get();
                    if let Some(track) = tracks.iter_mut().find(|t| t.audio_source_id == *id)
                        && let Some(ms) = duration
                    {
                        track.duration = Some(Duration::from_millis(*ms));
                    }
                    self.state.tracks.set(tracks);
                    self.state
                        .artwork_revision
                        .set(self.state.artwork_revision.get().wrapping_add(1));
                    self.update_selection(cx);
                    self.start_media(cx);
                }
                AppEvent::Browse
                    if self.session.is_some()
                        && !self.state.settings_loading.get()
                        && !self.state.browsing.get()
                        && !self.state.busy.get() =>
                {
                    self.state.browsing.set(true);
                    let mut proxy = cx.get_proxy();
                    self.runtime.spawn(async move {
                        let path = rfd::AsyncFileDialog::new()
                            .set_title("Select osu! folder")
                            .pick_folder()
                            .await
                            .map(|folder| folder.path().to_path_buf());
                        let _ = proxy.emit(AppEvent::Browsed(path));
                    });
                }
                AppEvent::Browsed(path) => {
                    self.state.browsing.set(false);
                    if let Some(path) = path {
                        self.register_folder(cx, path.clone());
                    }
                }
                AppEvent::FolderRegistered(result) => {
                    self.state.busy.set(false);
                    match result {
                        Ok(folder) => {
                            self.state.folder_registered(folder.clone());
                            self.state.settings_message.set(String::new());
                        }
                        Err(error) => {
                            self.state.settings_retry.set(Some(SettingsRetry::Browse));
                            self.state.settings_message.set(error.clone());
                        }
                    }
                }
                AppEvent::SelectTab(tab) => self.state.tab.set(*tab),
                AppEvent::SelectTrack(id) => {
                    self.state.playing.set(Some(*id));
                    self.update_selection(cx);
                }
                AppEvent::DragWindow => cx.emit(WindowEvent::DragWindow),
                AppEvent::MinimizeWindow => cx.emit(WindowEvent::SetMinimized(true)),
                AppEvent::ToggleMaximizeWindow => {
                    let maximized = !self.state.maximized.get();
                    self.state.maximized.set(maximized);
                    cx.emit(WindowEvent::SetMaximized(maximized));
                }
                AppEvent::CloseWindow => cx.emit(WindowEvent::WindowClose),
                _ => {}
            }
        });
        event.map(|window_event, _| {
            if matches!(window_event, WindowEvent::WindowClose) {
                self.stop();
            }
        });
    }
}

impl AppData {
    fn connect(&mut self, cx: &EventContext) {
        if self.connecting || self.session.is_some() {
            return;
        }
        self.connecting = true;
        self.state.settings_retry.set(None);
        self.state.settings_message.set("Connecting…".into());
        let mut proxy = cx.get_proxy();
        self.runtime.spawn(async move {
            let event = match Session::start(ServerOptions::default()).await {
                Ok(session) => AppEvent::Connected(Arc::new(session)),
                Err(error) => AppEvent::Failed(describe(&error)),
            };
            let _ = proxy.emit(event);
        });
    }
    fn refresh_library(&mut self, cx: &EventContext) {
        if self.state.library_loading.get() || self.state.busy.get() {
            return;
        }
        let Some(session) = &self.session else {
            self.connect(cx);
            return;
        };
        self.generation = self.generation.wrapping_add(1);
        for (_, job) in self.jobs.drain() {
            job.abort();
        }
        self.queue.clear();
        self.state.library_loading.set(true);
        self.state.library_message.set("Loading library…".into());
        let api = session.api().clone();
        let generation = self.generation;
        let mut proxy = cx.get_proxy();
        self.runtime.spawn(async move {
            let result = api
                .beatmap_sets()
                .await
                .map(|sets| library_tracks(&sets))
                .map_err(|e| describe(&e));
            let _ = proxy.emit(AppEvent::LibraryLoaded(generation, result));
        });
    }
    fn refresh_settings(&mut self, cx: &EventContext) {
        if self.state.settings_loading.get() || self.state.busy.get() || self.state.browsing.get() {
            return;
        }
        let Some(session) = &self.session else {
            self.connect(cx);
            return;
        };
        self.state.settings_loading.set(true);
        self.state.settings_retry.set(None);
        self.state.settings_message.set("Loading folders…".into());
        let api = session.api().clone();
        let mut proxy = cx.get_proxy();
        self.runtime.spawn(async move {
            let result = api.osu_folders().await.map_err(|e| describe(&e));
            let _ = proxy.emit(AppEvent::SettingsLoaded(result));
        });
    }
    fn update_selection(&mut self, cx: &EventContext) {
        let selected = self
            .state
            .tracks
            .get()
            .into_iter()
            .find(|t| Some(t.audio_source_id) == self.state.playing.get());
        self.state.selected.set(selected.clone());
        if let Some(track) = selected {
            self.request_media(cx, track.audio_source_id);
        }
    }
    fn request_media(&mut self, cx: &EventContext, id: i32) {
        if self.state.library_loading.get()
            || self.jobs.contains_key(&id)
            || self.queue.contains(&id)
        {
            return;
        }
        let Some(track) = self
            .state
            .tracks
            .get()
            .into_iter()
            .find(|t| t.audio_source_id == id)
        else {
            return;
        };
        let needs_cover = track.cover_beatmap_id.is_some_and(|cover| {
            !assets::has_cover(cover) && !self.missing_covers.contains(&cover)
        });
        if !needs_cover && self.duration_done.contains(&id) {
            return;
        }
        if self.state.playing.get() == Some(id) {
            self.queue.push_front(id);
        } else {
            self.queue.push_back(id);
        }
        self.start_media(cx);
    }
    fn start_media(&mut self, cx: &EventContext) {
        let Some(session) = &self.session else {
            return;
        };
        while self.jobs.len() < 4 {
            let Some(id) = self.queue.pop_front() else {
                break;
            };
            let Some(track) = self
                .state
                .tracks
                .get()
                .into_iter()
                .find(|t| t.audio_source_id == id)
            else {
                continue;
            };
            let cover = track
                .cover_beatmap_id
                .filter(|cover| !assets::has_cover(*cover) && !self.missing_covers.contains(cover));
            let duration_needed = !self.duration_done.contains(&id);
            let api = session.api().clone();
            let generation = self.generation;
            let mut proxy = cx.get_proxy();
            let job = self.runtime.spawn(async move {
                let image = if let Some(cover) = cover {
                    if let Ok(Some(bytes)) = api.cover(cover).await {
                        tokio::task::spawn_blocking(move || assets::decode_cover(&bytes))
                            .await
                            .ok()
                            .flatten()
                    } else {
                        None
                    }
                } else {
                    None
                };
                let duration = if duration_needed {
                    api.audio_duration(id)
                        .await
                        .ok()
                        .and_then(|d| d.duration_ms)
                } else {
                    None
                };
                let _ = proxy.emit(AppEvent::MediaLoaded(
                    generation, id, cover, image, duration,
                ));
            });
            self.jobs.insert(id, job);
        }
    }
    fn register_folder(&self, cx: &EventContext, path: PathBuf) {
        if self.state.busy.get() || self.state.settings_loading.get() || self.state.browsing.get() {
            return;
        }
        let Some(session) = &self.session else {
            return;
        };
        let registration = RegisterOsuFolder::new(path);
        self.state.busy.set(true);
        self.state.settings_retry.set(None);
        self.state
            .settings_message
            .set("Adding osu! folder…".into());
        let api = session.api().clone();
        let mut proxy = cx.get_proxy();
        self.runtime.spawn(async move {
            let result = api
                .register_osu_folder(&registration)
                .await
                .map(osu_radio_client::RegisteredFolder::into_folder)
                .map_err(|e| describe(&e));
            let _ = proxy.emit(AppEvent::FolderRegistered(result));
        });
    }
    fn stop(&mut self) {
        self.closed = true;
        self.generation = self.generation.wrapping_add(1);
        for (_, job) in self.jobs.drain() {
            job.abort();
        }
        self.queue.clear();
        assets::clear_covers();
        if let Some(session) = self.session.take() {
            self.runtime.spawn(async move {
                if let Err(error) = session.shutdown().await {
                    eprintln!("The embedded server could not be stopped: {error}");
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_selection_survives_refresh_and_duplicate_registration() {
        let state = UiState::new();
        let folder = |id| OsuFolder {
            id,
            kind: "lazer".into(),
            root_path: format!("/test/osu-{id}"),
            marker_path: format!("/test/osu-{id}/client.realm"),
            label: Some("Old label".into()),
            enabled: false,
            last_scanned_at: None,
        };

        state.set_folders(vec![]);
        assert_eq!(state.selected_folder.get(), None);
        state.set_folders(vec![folder(1), folder(2)]);
        assert_eq!(state.selected_folder.get(), Some(1));

        state.selected_folder.set(Some(2));
        state.set_folders(vec![folder(2), folder(1)]);
        assert_eq!(state.selected_folder.get(), Some(2));

        state.folder_registered(folder(3));
        assert_eq!(state.selected_folder.get(), Some(3));
        state.folder_registered(folder(1));
        assert_eq!(state.selected_folder.get(), Some(1));
        assert_eq!(state.folders.get(), vec![folder(2), folder(1), folder(3)]);

        state.set_folders(vec![folder(2)]);
        assert_eq!(state.selected_folder.get(), Some(2));
        state.set_folders(vec![]);
        assert_eq!(state.selected_folder.get(), None);
    }
}
