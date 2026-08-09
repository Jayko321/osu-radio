use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BeatmapSet {
    pub id: i32,
    pub online_id: Option<i32>,
    pub hash: Option<String>,
    pub audio_sources: Vec<AudioSource>,
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
    pub fn label(label: Option<String>) -> Self {
        Self {
            label: Some(label),
            enabled: None,
        }
    }

    pub fn enabled(enabled: bool) -> Self {
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
    pub fn was_created(&self) -> bool {
        matches!(self, Self::Created(_))
    }

    pub fn folder(&self) -> &OsuFolder {
        match self {
            Self::Created(folder) | Self::AlreadyRegistered(folder) => folder,
        }
    }

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
