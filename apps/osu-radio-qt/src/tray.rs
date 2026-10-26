//! System tray. Window actions reach the Qt thread through `ffi::tray_call`; transport items use
//! the same client rules as media keys. The platform modules own only the native icon and menu.
use crate::runtime::{RuntimeContext, ffi};
use cxx_qt_lib::QString;
use osu_radio_client::{controller::AppCommand, media_controls::Event};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

const ICON_SIZE: i32 = 64;

/// Calls a QML function on the main window from any thread.
fn call(method: &str) {
    ffi::tray_call(&QString::from(method));
}

fn transport(event: &Event) {
    if let Some(context) = RuntimeContext::current() {
        for controller in context.controllers() {
            controller.send(AppCommand::SystemMedia(event.clone()));
        }
    }
}

/// `ICON_SIZE` square RGBA pixels, empty when the bundled logo cannot be decoded.
fn icon_rgba() -> Vec<u8> {
    ffi::tray_icon_rgba(ICON_SIZE).as_slice().to_vec()
}

/// Registers the tray from the Qt thread once the main window exists. Platforms without a tray
/// implementation never report one, so Close keeps closing the window.
pub fn start(runtime: &RuntimeContext) {
    // Probes must not register tray icons in the developer's session.
    if std::env::var_os("OSU_RADIO_QT_SMOKE_TEST").is_some() {
        return;
    }
    #[cfg(target_os = "linux")]
    runtime.track(runtime.handle.spawn(linux::run()));
    #[cfg(target_os = "windows")]
    windows::start();
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let _ = runtime;
}

/// Removes the native icon before Qt shuts down; Linux stops with the runtime tasks.
#[allow(
    clippy::missing_const_for_fn,
    reason = "Windows removes the icon at runtime"
)]
pub fn stop() {
    #[cfg(target_os = "windows")]
    windows::stop();
}
