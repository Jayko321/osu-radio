use std::{fs, path::PathBuf};

use radio_core::import_types::{
    BeatmapMetadata, ImportedBeatmap, ImportedBeatmapSet, RealmFile, RealmNamedFileUsage,
};
use radio_core::{OsuKind, OsuMarker};
use tempfile::TempDir;

use crate::state::AppState;

pub(crate) fn beatmap(difficulty_name: &str, audio_file: &str) -> ImportedBeatmap {
    ImportedBeatmap {
        difficulty_name: Some(difficulty_name.to_owned()),
        bpm: Some(180.0),
        hash: Some(format!("hash-{difficulty_name}")),
        metadata: Some(BeatmapMetadata {
            title: Some("Song".to_owned()),
            title_unicode: None,
            artist: Some("Artist".to_owned()),
            artist_unicode: None,
            author: None,
            source: None,
            tags: None,
            user_tags: Vec::new(),
            preview_time: Some(4200),
            audio_file: Some(audio_file.to_owned()),
            background_file: None,
        }),
    }
}

pub(crate) fn beatmap_set(online_id: i32, audio_path: &str) -> ImportedBeatmapSet {
    ImportedBeatmapSet {
        source: OsuKind::Lazer,
        online_id: Some(online_id),
        hash: Some(format!("set-hash-{online_id}")),
        files: vec![RealmNamedFileUsage {
            filename: Some("audio.mp3".to_owned()),
            file: Some(RealmFile {
                hash: Some(format!("hash-{online_id}")),
                resolved_path: Some(PathBuf::from(audio_path)),
            }),
        }],
        beatmaps: vec![beatmap("Easy", "audio.mp3"), beatmap("Hard", "audio.mp3")],
    }
}

pub(crate) async fn state_with(beatmap_sets: &[ImportedBeatmapSet]) -> AppState {
    let state = empty_state().await;
    let installation = state
        .database()
        .osu_installations()
        .register(
            &OsuMarker {
                kind: OsuKind::Lazer,
                root_path: PathBuf::from("/test/osu"),
                marker_path: PathBuf::from("/test/osu/client.realm"),
            },
            None,
        )
        .await
        .expect("test installation should register")
        .into_installation();
    state
        .database()
        .osu_installations()
        .replace_snapshot(installation.id, beatmap_sets)
        .await
        .expect("beatmap sets should store");

    state
}

pub(crate) async fn empty_state() -> AppState {
    AppState::connect(":memory:")
        .await
        .expect("in-memory SQLite database should connect")
}

/// A directory that discovery recognizes as a lazer installation.
pub(crate) fn lazer_folder() -> TempDir {
    let folder = TempDir::new().expect("temp dir");
    fs::write(folder.path().join("client.realm"), b"").expect("write client.realm");

    folder
}

/// Adds a stable marker to an existing folder, so one root holds two installations.
pub(crate) fn stable_folder(folder: &TempDir) {
    fs::write(folder.path().join("osu!.db"), b"").expect("write osu!.db");
}
