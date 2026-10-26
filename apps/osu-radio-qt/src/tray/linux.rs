//! `StatusNotifierItem` tray through `ksni`, running on the application runtime.
use super::{ICON_SIZE, call, icon_rgba, transport};
use ksni::{Icon, MenuItem, OfflineReason, Tray, TrayMethods, menu::StandardItem};
use osu_radio_client::media_controls::Event;

struct OsuRadioTray {
    icon: Vec<Icon>,
}

fn item(label: &str, action: fn()) -> MenuItem<OsuRadioTray> {
    StandardItem {
        label: label.into(),
        activate: Box::new(move |_| action()),
        ..Default::default()
    }
    .into()
}

impl Tray for OsuRadioTray {
    fn id(&self) -> String {
        "osu-radio".into()
    }
    fn title(&self) -> String {
        "osu! radio".into()
    }
    fn icon_pixmap(&self) -> Vec<Icon> {
        self.icon.clone()
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        call("toggleFromTray");
    }
    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            item("Open osu! radio", || call("showFromTray")),
            item("Play / Pause", || transport(&Event::PlayPause)),
            item("Previous", || transport(&Event::Previous)),
            item("Next", || transport(&Event::Next)),
            MenuItem::Separator,
            item("Quit", || call("quitFromTray")),
        ]
    }
    fn watcher_online(&self) {
        call("trayOnline");
    }
    fn watcher_offline(&self, _reason: OfflineReason) -> bool {
        // A hidden window needs a tray to return to; close behaves normally until it is back.
        call("trayOffline");
        true
    }
}

/// Registers the tray and keeps it alive until the task is aborted at runtime shutdown.
/// Without a `StatusNotifierWatcher` the tray stays unavailable and Close keeps closing the window.
pub async fn run() {
    // SNI wants ARGB32 in network byte order; Qt supplies RGBA.
    let mut data = icon_rgba();
    for pixel in data.as_chunks_mut::<4>().0 {
        pixel.rotate_right(1);
    }
    let icon = if data.is_empty() {
        Vec::new()
    } else {
        vec![Icon {
            width: ICON_SIZE,
            height: ICON_SIZE,
            data,
        }]
    };
    match (OsuRadioTray { icon }).spawn().await {
        Ok(_handle) => {
            call("trayOnline");
            std::future::pending::<()>().await;
        }
        Err(error) => eprintln!("System tray unavailable: {error}"),
    }
}
