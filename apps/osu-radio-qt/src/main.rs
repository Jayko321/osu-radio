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

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QUrl};
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
            "Usage: osu-radio-qt [--component-gallery] [--help]\n\nSongs and the component gallery use bundled mock content. Playback is unavailable."
        );
        return ExitCode::SUCCESS;
    }
    let mut app = QGuiApplication::new();
    let mut engine = QQmlApplicationEngine::new();
    let (Some(mut app), Some(mut engine)) = (app.as_mut(), engine.as_mut()) else {
        eprintln!("Could not initialize Qt.");
        return ExitCode::FAILURE;
    };
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
    if smoke {
        engine
            .as_mut()
            .load(&QUrl::from("qrc:/qt/qml/OsuRadio/tests/AdapterProbe.qml"));
    }
    let root = if mode == LaunchMode::Songs {
        "Songs"
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
    ExitCode::from(u8::try_from(app.as_mut().exec()).unwrap_or(1))
}
