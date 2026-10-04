use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct AudioSettings {
    pub individual_volume_enabled: bool,
    #[serde(deserialize_with = "volume_percent")]
    pub global_volume_percent: u8,
}
impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            individual_volume_enabled: false,
            global_volume_percent: 100,
        }
    }
}
fn volume_percent<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u8, D::Error> {
    let percent = u8::deserialize(d)?;
    if percent > 100 {
        return Err(serde::de::Error::custom("Volume exceeds 100%"));
    }
    Ok(percent)
}

fn optional_volume_percent<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u8>, D::Error> {
    let percent = Option::<u8>::deserialize(d)?;
    if percent.is_some_and(|p| p > 100) {
        return Err(serde::de::Error::custom("Volume exceeds 100%"));
    }
    Ok(percent)
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiscoverFolders {
    pub roots: Vec<PathBuf>,
    pub depth: DiscoveryDepth,
}
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryDepth {
    Known,
    Shallow,
    #[default]
    Full,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum FolderDiscoveryEvent {
    Candidate {
        kind: String,
        root_path: String,
        marker_path: String,
        registered_id: Option<i32>,
    },
    Complete,
}
#[derive(Debug, Clone, Serialize)]
pub struct FolderMarker {
    pub marker_path: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FolderMetadata {
    pub beatmap_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BeatmapSet {
    #[serde(default)]
    pub has_multiple_audio_sources: bool,
    pub id: i32,
    pub online_id: Option<i32>,
    pub hash: Option<String>,
    pub audio_sources: Vec<AudioSource>,
    #[serde(default)]
    pub beatmaps: Vec<BeatmapDetails>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AudioSource {
    pub id: i32,
    pub kind: String,
    pub location: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct UserData {
    pub id: i32,
    pub osu_folders: Vec<OsuFolder>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct OsuFolder {
    pub id: i32,
    pub kind: String,
    pub root_path: String,
    pub marker_path: String,
    pub label: Option<String>,
    pub enabled: bool,
    pub last_scanned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RegisterOsuFolder {
    pub path: PathBuf,
    pub label: Option<String>,
}

impl RegisterOsuFolder {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            label: None,
        }
    }

    pub fn labelled(path: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            label: Some(label.into()),
        }
    }
}

/// An absent field is left alone by the server; `label: Some(None)` clears the label.
#[derive(Debug, Clone, Default, Serialize)]
pub struct OsuFolderChanges {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

impl OsuFolderChanges {
    #[must_use]
    pub const fn label(label: Option<String>) -> Self {
        Self {
            label: Some(label),
            enabled: None,
        }
    }

    #[must_use]
    pub const fn enabled(enabled: bool) -> Self {
        Self {
            label: None,
            enabled: Some(enabled),
        }
    }
}

/// The server answers `201` for a new folder and `409` for one it already knows, both with the
/// same body, so a caller can tell the two apart without inspecting the status itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisteredFolder {
    Created(OsuFolder),
    AlreadyRegistered(OsuFolder),
}

impl RegisteredFolder {
    #[must_use]
    pub const fn was_created(&self) -> bool {
        matches!(self, Self::Created(_))
    }

    #[must_use]
    pub const fn folder(&self) -> &OsuFolder {
        match self {
            Self::Created(folder) | Self::AlreadyRegistered(folder) => folder,
        }
    }

    #[must_use]
    pub fn into_folder(self) -> OsuFolder {
        match self {
            Self::Created(folder) | Self::AlreadyRegistered(folder) => folder,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_label_and_a_cleared_label_serialize_differently() {
        let untouched = serde_json::to_string(&OsuFolderChanges::enabled(false)).expect("json");
        let cleared = serde_json::to_string(&OsuFolderChanges::label(None)).expect("json");

        assert_eq!(untouched, r#"{"enabled":false}"#);
        assert_eq!(cleared, r#"{"label":null}"#);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct BeatmapDetails {
    pub id: i32,
    pub audio_source_id: Option<i32>,
    pub difficulty_name: Option<String>,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub has_cover: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AudioDuration {
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LibraryTrack {
    pub audio_source_id: i32,
    #[serde(default)]
    pub last_played_at_ms: Option<i64>,
    #[serde(default, deserialize_with = "optional_volume_percent")]
    pub volume_percent: Option<u8>,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub cover_beatmap_id: Option<i32>,
    pub difficulties: Vec<TrackDifficulty>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TrackDifficulty {
    pub beatmap_id: i32,
    pub beatmap_set_id: i32,
    pub difficulty_name: Option<String>,
    pub set_has_multiple_audio_sources: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackMode {
    #[default]
    Stopped,
    Paused,
    Playing,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct QueueState {
    #[serde(default)]
    pub upcoming_tracks: Vec<LibraryTrack>,
    pub audio_source_ids: Vec<i32>,
    #[serde(default)]
    pub playlist_item_ids: Vec<Option<i32>>,
    pub current_index: Option<usize>,
    pub mode: PlaybackMode,
    pub revision: u64,
    pub playback_token: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PlaybackAssignment {
    #[serde(default, deserialize_with = "optional_volume_percent")]
    pub volume_percent: Option<u8>,
    pub current_audio_source_id: Option<i32>,
    #[serde(default)]
    pub current_playlist_item_id: Option<i32>,
    pub track: Option<LibraryTrack>,
    pub duration_ms: Option<u64>,
    pub mode: PlaybackMode,
    pub revision: u64,
    pub playback_token: u64,
    pub can_next: bool,
    pub can_previous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum PlaybackCommand {
    Play {
        #[serde(skip_serializing_if = "Option::is_none")]
        audio_source_id: Option<i32>,
    },
    Pause,
    #[serde(rename = "pause")]
    PauseIfCurrent {
        playback_token: u64,
    },
    Stop,
    Next,
    Previous,
    Started {
        playback_token: u64,
    },
    Finished {
        playback_token: u64,
    },
    Failed {
        playback_token: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlaylistSummary {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub item_count: u64,
    #[serde(default)]
    pub cover_beatmap_id: Option<i32>,
    #[serde(default)]
    pub custom_cover_revision: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PlaylistItem {
    pub id: i32,
    #[serde(default)]
    pub last_played_at_ms: Option<i64>,
    #[serde(default, deserialize_with = "optional_volume_percent")]
    pub volume_percent: Option<u8>,
    pub playlist_id: i32,
    pub source_kind: String,
    pub beatmap_hash: String,
    pub title: Option<String>,
    #[serde(default)]
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    #[serde(default)]
    pub artist_unicode: Option<String>,
    pub difficulty_name: Option<String>,
    pub beatmap_id: Option<i32>,
    pub beatmap_set_id: Option<i32>,
    pub audio_source_id: Option<i32>,
    pub cover_beatmap_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Playlist {
    pub id: i32,
    pub name: String,
    pub items: Vec<PlaylistItem>,
}
