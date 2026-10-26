//! Notification-area icon through `tray-icon`. It must live on the Qt thread: its hidden
//! window receives messages from Qt's loop, and its handlers run there.
use super::{ICON_SIZE, call, icon_rgba, transport};
use osu_radio_client::media_controls::Event;
use std::cell::RefCell;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

thread_local! {
    static TRAY: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
}

fn build() -> Result<TrayIcon, String> {
    let size = u32::try_from(ICON_SIZE).map_err(|error| error.to_string())?;
    let icon = Icon::from_rgba(icon_rgba(), size, size).map_err(|error| error.to_string())?;
    let menu = Menu::with_items(&[
        &MenuItem::with_id("open", "Open osu! radio", true, None),
        &MenuItem::with_id("toggle", "Play / Pause", true, None),
        &MenuItem::with_id("previous", "Previous", true, None),
        &MenuItem::with_id("next", "Next", true, None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id("quit", "Quit", true, None),
    ])
    .map_err(|error| error.to_string())?;
    TrayIconBuilder::new()
        .with_tooltip("osu! radio")
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|error| error.to_string())
}

pub fn start() {
    TrayIconEvent::set_event_handler(Some(|event: TrayIconEvent| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            call("toggleFromTray");
        }
    }));
    MenuEvent::set_event_handler(Some(|event: MenuEvent| match event.id.0.as_str() {
        "open" => call("showFromTray"),
        "toggle" => transport(&Event::PlayPause),
        "previous" => transport(&Event::Previous),
        "next" => transport(&Event::Next),
        "quit" => call("quitFromTray"),
        _ => {}
    }));
    match build() {
        Ok(tray) => {
            TRAY.with(|slot| *slot.borrow_mut() = Some(tray));
            call("trayOnline");
        }
        Err(error) => eprintln!("System tray unavailable: {error}"),
    }
}

pub fn stop() {
    // Dropping removes the icon; a leftover one lingers until the mouse passes over it.
    TRAY.with(|slot| slot.borrow_mut().take());
}
