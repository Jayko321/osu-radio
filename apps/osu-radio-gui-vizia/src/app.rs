use crate::{assets, views};
pub use osu_radio_client::controller::SettingsRetry;
use osu_radio_client::{
    OsuFolder, ServerOptions, Track,
    controller::{AppCommand, AppController, AppUpdate, ConnectionStatus, MediaTicket},
};
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;
use vizia::prelude::*;

pub fn run(runtime: &Handle) -> Result<(), ApplicationError> {
    let retained = Arc::new(Mutex::new(None::<AppController>));
    let launcher = Arc::clone(&retained);
    let worker = runtime.clone();
    let result = Application::new(move |cx| {
        crate::input::install(cx);
        assets::register(cx);
        views::styles(cx);
        let state = UiState::new();
        let proxy = Mutex::new(cx.get_proxy());
        let controller = AppController::spawn(&worker, ServerOptions::default(), move |update| {
            if let Ok(mut proxy) = proxy.lock() {
                let _ = proxy.emit(AppEvent::Update(update));
            }
        });
        if let Ok(mut retained) = retained.lock() {
            *retained = Some(controller.clone());
        }
        AppData {
            controller,
            runtime: worker,
            state,
            generation: 0,
            closed: false,
        }
        .build(cx);
        cx.emit(AppEvent::Connect);
        views::shell(cx, state);
    })
    .title("osu! radio")
    .inner_size((1440u32, 952u32))
    .min_inner_size(Some((1024u32, 640u32)))
    .decorations(false)
    .run();
    let controller = launcher
        .lock()
        .ok()
        .and_then(|mut retained| retained.take());
    if let Some(controller) = controller {
        runtime.block_on(controller.shutdown());
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Songs,
    Settings,
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
}

struct AppData {
    controller: AppController,
    runtime: Handle,
    state: UiState,
    generation: u64,
    closed: bool,
}
pub enum AppEvent {
    Connect,
    RefreshLibrary,
    SearchLibrary(String),
    RefreshSettings,
    RequestMedia(i32),
    Browse,
    Update(AppUpdate),
    ArtworkDecoded(MediaTicket, i32, Option<vizia::vg::Image>),
    SelectTab(Tab),
    SelectTrack(i32),
    SelectFolder(i32),
    DragWindow,
    MinimizeWindow,
    ToggleMaximizeWindow,
    CloseWindow,
}
impl Model for AppData {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|app_event, _| {
            if self.closed {
                return;
            }
            match app_event {
                AppEvent::Connect => self.controller.send(AppCommand::Connect),
                AppEvent::SearchLibrary(query) => self
                    .controller
                    .send(AppCommand::SearchLibrary(query.clone())),
                AppEvent::RefreshLibrary => self.controller.send(AppCommand::RefreshLibrary),
                AppEvent::RefreshSettings => self.controller.send(AppCommand::RefreshFolders),
                AppEvent::RequestMedia(id) => self.request_media(*id),
                AppEvent::Browse => self.controller.send(AppCommand::BeginFolderPick),
                AppEvent::SelectTrack(id) => {
                    self.controller.send(AppCommand::SelectTrack(Some(*id)));
                }
                AppEvent::SelectFolder(id) => {
                    self.controller.send(AppCommand::SelectFolder(Some(*id)));
                }
                AppEvent::SelectTab(tab) => self.state.tab.set(*tab),
                AppEvent::Update(update) => self.update(cx, update.clone()),
                AppEvent::ArtworkDecoded(ticket, cover, image) => {
                    let available = ticket.generation == self.generation && image.is_some();
                    if available && let Some(image) = image {
                        assets::cache_cover(*cover, image.clone());
                        self.artwork_changed();
                    }
                    self.controller.send(AppCommand::MediaInstalled {
                        ticket: *ticket,
                        available,
                    });
                }
                AppEvent::DragWindow => cx.emit(WindowEvent::DragWindow),
                AppEvent::MinimizeWindow => cx.emit(WindowEvent::SetMinimized(true)),
                AppEvent::ToggleMaximizeWindow => {
                    let maximized = !self.state.maximized.get();
                    self.state.maximized.set(maximized);
                    cx.emit(WindowEvent::SetMaximized(maximized));
                }
                AppEvent::CloseWindow => cx.emit(WindowEvent::WindowClose),
            }
        });
        event.map(|window_event, _| {
            if matches!(window_event, WindowEvent::WindowClose) {
                self.closed = true;
                assets::clear_covers();
                self.controller.send(AppCommand::Shutdown);
            }
        });
    }
}
impl AppData {
    fn artwork_changed(&self) {
        self.state
            .artwork_revision
            .set(self.state.artwork_revision.get().wrapping_add(1));
    }
    fn request_media(&self, id: i32) {
        let missing = self
            .state
            .tracks
            .get()
            .iter()
            .find(|track| track.audio_source_id == id)
            .and_then(|track| track.cover_beatmap_id)
            .is_some_and(|cover| !assets::has_cover(cover));
        self.controller.send(AppCommand::RequestMedia {
            audio_id: id,
            artwork_missing: missing,
        });
    }
    fn update(&mut self, cx: &EventContext, update: AppUpdate) {
        match update {
            AppUpdate::Connection(status) => {
                self.state
                    .connected
                    .set(matches!(status, ConnectionStatus::Connected));
                self.state.status.set(match status {
                    ConnectionStatus::Connecting => "Starting the embedded server…".into(),
                    ConnectionStatus::Failed(error) => error,
                    _ => String::new(),
                });
            }
            AppUpdate::LibraryStatus(status) => {
                self.state.library_loading.set(status.loading);
                self.state.library_message.set(status.message);
            }
            AppUpdate::FolderStatus(status) => {
                self.state.settings_loading.set(status.loading);
                self.state.settings_message.set(status.message);
                self.state.settings_retry.set(status.retry);
                self.state.busy.set(status.busy);
            }
            AppUpdate::TracksReplaced(tracks) => {
                self.generation = self.generation.wrapping_add(1);
                assets::clear_covers();
                self.artwork_changed();
                self.state.tracks.set(tracks);
                self.state
                    .library_revision
                    .set(self.state.library_revision.get().wrapping_add(1));
            }
            AppUpdate::TrackChanged(track) => {
                let mut tracks = self.state.tracks.get();
                if let Some(row) = tracks
                    .iter_mut()
                    .find(|row| row.audio_source_id == track.audio_source_id)
                {
                    *row = track;
                }
                self.state.tracks.set(tracks);
            }
            AppUpdate::TrackSelected(track) => {
                self.state
                    .playing
                    .set(track.as_ref().map(|track| track.audio_source_id));
                self.state.selected.set(track.clone());
                if let Some(track) = track {
                    self.request_media(track.audio_source_id);
                }
            }
            AppUpdate::FoldersReplaced(folders) => self.state.folders.set(folders),
            AppUpdate::FolderSelected(id) => self.state.selected_folder.set(id),
            AppUpdate::FolderPickerRequested => {
                let controller = self.controller.clone();
                self.runtime.spawn(async move {
                    let path = rfd::AsyncFileDialog::new()
                        .set_title("Select osu! folder")
                        .pick_folder()
                        .await
                        .map(|folder| folder.path().to_path_buf());
                    controller.send(AppCommand::CompleteFolderPick(path));
                });
            }
            AppUpdate::Artwork {
                ticket,
                cover_id,
                bytes,
            } => {
                if ticket.generation != self.generation {
                    self.controller.send(AppCommand::MediaInstalled {
                        ticket,
                        available: false,
                    });
                    return;
                }
                let mut proxy = cx.get_proxy();
                let controller = self.controller.clone();
                self.runtime.spawn(async move {
                    let image = tokio::task::spawn_blocking(move || assets::decode_cover(&bytes))
                        .await
                        .ok()
                        .flatten();
                    if proxy
                        .emit(AppEvent::ArtworkDecoded(ticket, cover_id, image))
                        .is_err()
                    {
                        controller.send(AppCommand::MediaInstalled {
                            ticket,
                            available: false,
                        });
                    }
                });
            }
        }
    }
}
