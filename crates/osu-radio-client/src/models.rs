use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioModsMode {
    #[default]
    None,
    Dt,
    Ht,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct AudioModsProfile {
    pub mode: AudioModsMode,
    pub speed_percent: u16,
    pub adjust_pitch: bool,
}
impl Default for AudioModsProfile {
    fn default() -> Self {
        Self {
            mode: AudioModsMode::None,
            speed_percent: 100,
            adjust_pitch: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct AudioModsSettings {
    pub individual_mods_enabled: bool,
    pub global_profile: AudioModsProfile,
}

/// Only present fields are sent. The server validates the complete merged profile.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct AudioModsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub individual_mods_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_mode: Option<AudioModsMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_speed_percent: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_adjust_pitch: Option<bool>,
}

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
    fn mode_wire_types_match_the_server_contract_and_default_to_off() {
        use serde_json::json;
        assert_eq!(
            serde_json::to_value(PlaybackCommand::SetShuffle { enabled: true }).unwrap(),
            json!({"command":"set_shuffle","enabled":true})
        );
        assert_eq!(
            serde_json::to_value(PlaybackCommand::SetRepeat {
                mode: RepeatMode::One
            })
            .unwrap(),
            json!({"command":"set_repeat","mode":"one"})
        );
        let legacy = r#"{"current_entry_id":null,"availability":null,"waiting_for_check":false,"mods":null,"current_audio_source_id":null,"track":null,"duration_ms":null,"mode":"stopped","revision":1,"playback_token":0,"can_next":false,"can_previous":false}"#;
        let assignment: PlaybackAssignment = serde_json::from_str(legacy).unwrap();
        assert!(!assignment.shuffle && assignment.repeat == RepeatMode::Off);
        let modern = legacy.replace(
            r#""mode":"stopped""#,
            r#""mode":"stopped","shuffle":true,"repeat":"all""#,
        );
        let assignment: PlaybackAssignment = serde_json::from_str(&modern).unwrap();
        assert!(assignment.shuffle && assignment.repeat == RepeatMode::All);
    }

    #[test]
    fn mods_wire_types_preserve_profiles_and_patch_omissions() {
        use serde_json::json;
        assert_eq!(
            serde_json::to_value(AudioModsSettings::default()).unwrap(),
            json!({
                "individual_mods_enabled":false,
                "global_profile":{"mode":"none","speed_percent":100,"adjust_pitch":false}
            })
        );
        assert_eq!(
            serde_json::to_value(AudioModsPatch::default()).unwrap(),
            json!({})
        );
        let patch = AudioModsPatch {
            individual_mods_enabled: Some(false),
            global_mode: Some(AudioModsMode::Dt),
            global_speed_percent: Some(120),
            global_adjust_pitch: Some(true),
        };
        assert_eq!(
            serde_json::to_value(patch).unwrap(),
            json!({
                "individual_mods_enabled":false,"global_mode":"dt",
                "global_speed_percent":120,"global_adjust_pitch":true
            })
        );
        let expected = AudioModsProfile {
            mode: AudioModsMode::Dt,
            speed_percent: 120,
            adjust_pitch: true,
        };
        for mods in [None, Some(expected)] {
            let row: LibraryTrack = serde_json::from_value(json!({
                "audio_source_id":7,"mods":mods,"difficulties":[]
            }))
            .unwrap();
            assert_eq!(row.mods, mods);
            let track = crate::Track::from(row);
            assert_eq!(track.mods, mods);
            assert_eq!(track.duration, None);
        }
        for profile in [
            json!({"mode":"nc","speed_percent":120,"adjust_pitch":true}),
            json!({"mode":"dt","speed_percent":120.5,"adjust_pitch":true}),
            json!({"mode":"dt","speed_percent":120,"adjust_pitch":null}),
            json!({"mode":"dt","speed_percent":120}),
        ] {
            assert!(serde_json::from_value::<AudioModsProfile>(profile).is_err());
        }
    }

    #[test]
    fn an_absent_label_and_a_cleared_label_serialize_differently() {
        let untouched = serde_json::to_string(&OsuFolderChanges::enabled(false)).expect("json");
        let cleared = serde_json::to_string(&OsuFolderChanges::label(None)).expect("json");

        assert_eq!(untouched, r#"{"enabled":false}"#);
        assert_eq!(cleared, r#"{"label":null}"#);
        assert_eq!(
            serde_json::to_value(OsuFolderChanges::default()).unwrap(),
            serde_json::json!({})
        );
        assert_eq!(
            serde_json::to_value(OsuFolderChanges {
                label: Some(Some("曲".into())),
                enabled: Some(true),
            })
            .unwrap(),
            serde_json::json!({"label": "曲", "enabled": true})
        );
    }

    #[test]
    fn every_volume_dto_accepts_only_integer_percentages_including_both_endpoints() {
        use serde_json::{Value, json};

        let fixtures = [
            (
                "global_volume_percent",
                json!({"individual_volume_enabled": true}),
                false,
            ),
            (
                "volume_percent",
                json!({"audio_source_id": 1, "difficulties": []}),
                true,
            ),
            (
                "volume_percent",
                json!({"waiting_for_check": false, "mode": "paused", "revision": 1,
                "playback_token": 2, "can_next": false, "can_previous": false}),
                true,
            ),
            (
                "volume_percent",
                json!({"id": 3, "playlist_id": 4,
                "source_kind": "stable", "beatmap_hash": "hash"}),
                true,
            ),
        ];
        for (index, (field, base, optional)) in fixtures.into_iter().enumerate() {
            let decode = |value: Value| -> Result<Option<u8>, serde_json::Error> {
                match index {
                    0 => serde_json::from_value::<AudioSettings>(value)
                        .map(|s| Some(s.global_volume_percent)),
                    1 => serde_json::from_value::<LibraryTrack>(value).map(|s| s.volume_percent),
                    2 => serde_json::from_value::<PlaybackAssignment>(value)
                        .map(|s| s.volume_percent),
                    _ => serde_json::from_value::<PlaylistItem>(value).map(|s| s.volume_percent),
                }
            };
            for percent in [0, 1, 99, 100] {
                let mut value = base.clone();
                value
                    .as_object_mut()
                    .unwrap()
                    .insert(field.into(), json!(percent));
                assert_eq!(decode(value).unwrap(), Some(percent), "DTO {index}");
            }
            for invalid in [
                json!(-1),
                json!(101),
                json!(255),
                json!(256),
                json!(0.5),
                json!("50"),
                json!(true),
            ] {
                let mut value = base.clone();
                value
                    .as_object_mut()
                    .unwrap()
                    .insert(field.into(), invalid.clone());
                assert!(decode(value).is_err(), "DTO {index} accepted {invalid}");
            }
            let mut null = base.clone();
            null.as_object_mut()
                .unwrap()
                .insert(field.into(), Value::Null);
            if optional {
                assert_eq!(decode(base).unwrap(), None);
                assert_eq!(decode(null).unwrap(), None);
            } else {
                assert!(decode(base).is_err());
                assert!(decode(null).is_err());
            }
        }
        assert_eq!(
            AudioSettings::default(),
            AudioSettings {
                individual_volume_enabled: false,
                global_volume_percent: 100,
            }
        );
    }

    #[test]
    fn playback_modes_and_boundary_tokens_keep_the_wire_contract() {
        use serde_json::json;
        for (command, expected) in [
            (
                PlaybackCommand::PauseIfCurrent { playback_token: 0 },
                json!({"command": "pause", "playback_token": 0}),
            ),
            (
                PlaybackCommand::PauseIfCurrent {
                    playback_token: u64::MAX,
                },
                json!({"command": "pause", "playback_token": u64::MAX}),
            ),
            (
                PlaybackCommand::Started {
                    playback_token: u64::MAX,
                },
                json!({"command": "started", "playback_token": u64::MAX}),
            ),
            (
                PlaybackCommand::Finished {
                    playback_token: u64::MAX,
                },
                json!({"command": "finished", "playback_token": u64::MAX}),
            ),
            (
                PlaybackCommand::Failed {
                    playback_token: u64::MAX,
                },
                json!({"command": "failed", "playback_token": u64::MAX}),
            ),
        ] {
            assert_eq!(serde_json::to_value(command).unwrap(), expected);
        }
        for (mode, expected) in [
            (PlaybackMode::Stopped, "stopped"),
            (PlaybackMode::Paused, "paused"),
            (PlaybackMode::Playing, "playing"),
        ] {
            assert_eq!(serde_json::to_value(mode).unwrap(), json!(expected));
            assert_eq!(
                serde_json::from_value::<PlaybackMode>(json!(expected)).unwrap(),
                mode
            );
        }
        assert!(serde_json::from_str::<PlaybackMode>("\"buffering\"").is_err());
        assert_eq!(PlaybackMode::default(), PlaybackMode::Stopped);
    }

    #[test]
    fn folder_requests_and_registration_outcomes_retain_paths_labels_and_identity() {
        use serde_json::json;
        let path = PathBuf::from("fixture root/曲/client.realm");
        assert_eq!(
            serde_json::to_value(RegisterOsuFolder::new(&path)).unwrap(),
            json!({"path": path, "label": null})
        );
        assert_eq!(
            serde_json::to_value(RegisterOsuFolder::labelled(&path, "曲")).unwrap(),
            json!({"path": path, "label": "曲"})
        );
        for (depth, expected) in [
            (DiscoveryDepth::Known, "known"),
            (DiscoveryDepth::Shallow, "shallow"),
            (DiscoveryDepth::Full, "full"),
        ] {
            assert_eq!(
                serde_json::to_value(DiscoverFolders {
                    roots: vec![path.clone()],
                    depth
                })
                .unwrap(),
                json!({"roots": [path], "depth": expected})
            );
        }
        assert_eq!(
            serde_json::to_value(DiscoverFolders::default()).unwrap(),
            json!({"roots": [], "depth": "full"})
        );
        assert_eq!(
            serde_json::to_value(FolderMarker {
                marker_path: "曲/client.realm".into()
            })
            .unwrap(),
            json!({"marker_path": "曲/client.realm"})
        );
        let folder: OsuFolder = serde_json::from_value(json!({"id": 8, "kind": "lazer", "root_path": "fixture root/曲",
            "marker_path": "fixture root/曲/client.realm", "label": "曲", "enabled": false, "last_scanned_at": "2026-10-15T00:00:00Z"})).unwrap();
        for (result, created) in [
            (RegisteredFolder::Created(folder.clone()), true),
            (RegisteredFolder::AlreadyRegistered(folder.clone()), false),
        ] {
            assert_eq!(result.was_created(), created);
            assert_eq!(result.folder(), &folder);
            assert_eq!(result.into_folder(), folder);
        }
    }

    #[test]
    fn older_response_shapes_default_only_the_optional_extension_fields() {
        use serde_json::json;
        let set: BeatmapSet =
            serde_json::from_value(json!({"id": 2, "audio_sources": []})).unwrap();
        assert!(!set.has_multiple_audio_sources);
        assert!(set.beatmaps.is_empty());
        assert_eq!((set.online_id, set.hash), (None, None));
        let summary: PlaylistSummary =
            serde_json::from_value(json!({"id": 5, "name": "曲"})).unwrap();
        assert_eq!(
            summary,
            PlaylistSummary {
                id: 5,
                name: "曲".into(),
                item_count: 0,
                cover_beatmap_id: None,
                custom_cover_revision: None
            }
        );
        let item: PlaylistItem = serde_json::from_value(json!({"id": 6, "playlist_id": 5,
            "source_kind": "stable", "beatmap_hash": "hash"}))
        .unwrap();
        assert_eq!(
            (
                item.last_played_at_ms,
                item.volume_percent,
                item.title_unicode,
                item.artist_unicode
            ),
            (None, None, None, None)
        );
        assert!(serde_json::from_value::<QueueState>(json!({"audio_source_ids": []})).is_err());
        assert!(serde_json::from_value::<BeatmapSet>(json!({"id": 2})).is_err());
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
    pub mods: Option<AudioModsProfile>,
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
    #[serde(default)]
    pub beatmap_set_online_id: Option<i32>,
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

/// Independent client identity; serialize to a string before crossing any Qt/QML boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct QueueEntryId(i64);
impl QueueEntryId {
    pub const fn new(value: i64) -> Result<Self, &'static str> {
        if value > 0 {
            Ok(Self(value))
        } else {
            Err("Queue entry ID must be positive")
        }
    }
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}
impl TryFrom<String> for QueueEntryId {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Expected a positive decimal i64 string");
        }
        value
            .parse::<i64>()
            .map_err(|_| "Expected a positive decimal i64 string")
            .and_then(Self::new)
    }
}
impl From<QueueEntryId> for String {
    fn from(id: QueueEntryId) -> Self {
        id.to_string()
    }
}
impl std::fmt::Display for QueueEntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueAvailability {
    Checking,
    Available,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct QueueEntry {
    pub id: QueueEntryId,
    pub position: usize,
    pub audio_source_id: Option<i32>,
    pub playlist_item_id: Option<i32>,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub difficulty_name: Option<String>,
    pub beatmap_id: Option<i32>,
    pub beatmap_set_id: Option<i32>,
    pub cover_beatmap_id: Option<i32>,
    pub availability: QueueAvailability,
    pub last_played_at_ms: Option<i64>,
    #[serde(deserialize_with = "optional_volume_percent")]
    pub volume_percent: Option<u8>,
    pub mods: Option<AudioModsProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueueItemRef {
    Audio { audio_source_id: i32 },
    PlaylistItem { playlist_id: i32, item_id: i32 },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueuePlacement {
    End,
    Next,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct QueueState {
    pub entries: Vec<QueueEntry>,
    pub current_entry_id: Option<QueueEntryId>,
    pub history_len: usize,
    pub mode: PlaybackMode,
    pub waiting_for_check: bool,
    pub revision: u64,
    pub playback_token: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}
impl RepeatMode {
    /// The button cycle: Off → All → One → Off.
    #[must_use]
    pub const fn cycled(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

impl From<RepeatMode> for osu_radio_media_controls::Repeat {
    fn from(mode: RepeatMode) -> Self {
        match mode {
            RepeatMode::Off => Self::Off,
            RepeatMode::All => Self::All,
            RepeatMode::One => Self::One,
        }
    }
}
impl From<osu_radio_media_controls::Repeat> for RepeatMode {
    fn from(repeat: osu_radio_media_controls::Repeat) -> Self {
        match repeat {
            osu_radio_media_controls::Repeat::Off => Self::Off,
            osu_radio_media_controls::Repeat::All => Self::All,
            osu_radio_media_controls::Repeat::One => Self::One,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Mirrors independent JSON flags of the server contract.
pub struct PlaybackAssignment {
    pub current_entry_id: Option<QueueEntryId>,
    pub availability: Option<QueueAvailability>,
    pub waiting_for_check: bool,
    #[serde(default, deserialize_with = "optional_volume_percent")]
    pub volume_percent: Option<u8>,
    pub mods: Option<AudioModsProfile>,
    pub current_audio_source_id: Option<i32>,
    #[serde(default)]
    pub current_playlist_item_id: Option<i32>,
    pub track: Option<LibraryTrack>,
    pub duration_ms: Option<u64>,
    pub mode: PlaybackMode,
    /// Absent from servers that predate queue modes.
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub repeat: RepeatMode,
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
    SetShuffle {
        enabled: bool,
    },
    SetRepeat {
        mode: RepeatMode,
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
    pub mods: Option<AudioModsProfile>,
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
