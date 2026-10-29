#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::unwrap_used,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects
    )
)]
mod launch;
use osu_radio_qt::runtime;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};
use launch::LaunchMode;
use std::{
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn main() -> ExitCode {
    let mode = match LaunchMode::parse(std::env::args_os().skip(1)) {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("{error}\nUsage: osu-radio-qt [--component-gallery] [--help]");
            return ExitCode::from(2);
        }
    };
    if mode == LaunchMode::Help {
        println!(
            "Usage: osu-radio-qt [--component-gallery] [--help]\n\nSet OSU_RADIO_SERVER_URL=http://host:3000 to connect to an existing server.\nWithout it, Songs starts a local server. The component gallery stays offline. Playback uses the server queue."
        );
        return ExitCode::SUCCESS;
    }
    let live = if mode == LaunchMode::Songs {
        match runtime::LiveRuntime::new() {
            Ok(runtime) => Some(runtime),
            Err(error) => {
                eprintln!("Could not initialize runtime: {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };
    let result = run(mode);
    if let Some(runtime) = live {
        runtime.shutdown();
    }
    result
}

fn run(mode: LaunchMode) -> ExitCode {
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    let (Some(mut app), Some(mut engine)) = (app.as_mut(), engine.as_mut()) else {
        eprintln!("Could not initialize Qt.");
        return ExitCode::FAILURE;
    };
    app.as_mut()
        .set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
    let created = Arc::new(AtomicBool::new(false));
    let loaded = Arc::clone(&created);
    engine
        .as_mut()
        .on_object_created(move |_, object, _| {
            loaded.store(!object.is_null(), Ordering::Relaxed);
        })
        .release();
    let smoke = std::env::var_os("OSU_RADIO_QT_SMOKE_TEST").is_some();
    runtime::ffi::configure_engine(engine.as_mut(), smoke);
    let root = if mode == LaunchMode::Songs {
        "Main"
    } else {
        "Gallery"
    };
    engine
        .as_mut()
        .load(&QUrl::from(&format!("qrc:/qt/qml/OsuRadio/qml/{root}.qml")));
    if !created.load(Ordering::Relaxed) {
        eprintln!("Failed to create the {root} QML root.");
        return ExitCode::FAILURE;
    }
    // Dropped before engine/app on every return, including failed probe loading.
    let _window_media =
        (mode == LaunchMode::Songs).then(|| runtime::ffi::connect_live_window(engine.as_mut()));
    if smoke {
        runtime::ffi::configure_probe(engine.as_mut(), mode == LaunchMode::ComponentGallery);
        let case = std::env::var("OSU_RADIO_QT_PROBE_CASE").unwrap_or_default();
        let probe = if case == "system-media-failed-launch" {
            "MissingProbe"
        } else if case.starts_with("system-media") {
            // The native lifecycle probe owns completion and SMTC readback.
            return ExitCode::from(u8::try_from(app.as_mut().exec()).unwrap_or(1));
        } else if case.starts_with("visual") || case.starts_with("native") {
            "VisualProbe"
        } else {
            "AdapterProbe"
        };
        engine.as_mut().load(&QUrl::from(&format!(
            "qrc:/qt/qml/OsuRadio/tests/{probe}.qml"
        )));
        if !created.load(Ordering::Relaxed) {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::from(u8::try_from(app.as_mut().exec()).unwrap_or(1))
}
