use std::sync::Arc;

use osu_radio_client::{ServerOptions, Session, describe};
use tokio::runtime::Handle;
use vizia::prelude::*;

use crate::{assets, sample, views};

pub fn run(runtime: Handle) -> Result<(), ApplicationError> {
    Application::new(move |cx| {
        assets::register(cx);
        views::styles(cx);

        let state = UiState::new();

        AppData {
            session: None,
            runtime,
            state,
        }
        .build(cx);

        cx.emit(AppEvent::Connect);

        views::shell(cx, state);
    })
    .title("osu! radio")
    .inner_size((1440u32, 952u32))
    .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Songs,
    Settings,
}

#[derive(Debug, Clone, Copy)]
pub struct UiState {
    pub tab: Signal<Tab>,
    pub playing: Signal<usize>,
    pub song_query: Signal<String>,
    pub settings_query: Signal<String>,
    pub status: Signal<String>,
}

impl UiState {
    fn new() -> Self {
        Self {
            tab: Signal::new(Tab::Songs),
            playing: Signal::new(sample::PLAYING),
            song_query: Signal::new(String::new()),
            settings_query: Signal::new(String::new()),
            status: Signal::new("starting the embedded server...".to_owned()),
        }
    }
}

struct AppData {
    session: Option<Arc<Session>>,
    runtime: Handle,
    state: UiState,
}

pub enum AppEvent {
    Connect,
    Connected(Arc<Session>),
    Failed(String),
    SelectTab(Tab),
    SelectTrack(usize),
}

impl Model for AppData {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|app_event, _| match app_event {
            AppEvent::Connect => self.connect(cx),
            AppEvent::Connected(session) => {
                self.session = Some(Arc::clone(session));
                self.state.status.set(String::new());
            }
            AppEvent::Failed(reason) => self.state.status.set(reason.clone()),
            AppEvent::SelectTab(tab) => self.state.tab.set(*tab),
            AppEvent::SelectTrack(index) => self.state.playing.set(*index),
        });

        event.map(|window_event, _| {
            if matches!(window_event, WindowEvent::WindowClose) {
                self.stop();
            }
        });
    }
}

impl AppData {
    fn connect(&self, cx: &EventContext) {
        let mut proxy = cx.get_proxy();

        self.runtime.spawn(async move {
            let session = match Session::start(ServerOptions::default()).await {
                Ok(session) => session,
                Err(error) => {
                    let _ = proxy.emit(AppEvent::Failed(describe(&error)));
                    return;
                }
            };

            let reachable = session.api().user_data().await;

            let event = match reachable {
                Ok(_) => AppEvent::Connected(Arc::new(session)),
                Err(error) => AppEvent::Failed(describe(&error)),
            };

            let _ = proxy.emit(event);
        });
    }

    /// Stops the server on window close rather than waiting for the drop that follows it, so the
    /// child is gone before the process starts tearing down.
    fn stop(&mut self) {
        let Some(session) = self.session.take() else {
            return;
        };

        self.runtime.spawn(async move {
            if let Err(error) = session.shutdown().await {
                eprintln!("The embedded server could not be stopped: {error}");
            }
        });
    }
}
