#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistSummary {
    pub id: i32,
    pub name: String,
    pub item_count: u64,
    pub cover_beatmap_id: Option<i32>,
    pub custom_cover_revision: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistItem {
    pub last_played_at_ms: Option<i64>,
    pub volume_percent: Option<u8>,
    pub id: i32,
    pub playlist_id: i32,
    pub source_kind: String,
    pub beatmap_hash: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub difficulty_name: Option<String>,
    pub beatmap_id: Option<i32>,
    pub beatmap_set_id: Option<i32>,
    pub audio_source_id: Option<i32>,
    pub cover_beatmap_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    pub id: i32,
    pub name: String,
    pub items: Vec<PlaylistItem>,
}

/// Library projection used for atomic additions and batched stable-key resolution.
#[derive(Debug, Clone)]
pub struct PlaylistBeatmap {
    pub beatmap_id: i32,
    pub beatmap_set_id: i32,
    pub source_kind: String,
    pub beatmap_hash: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub difficulty_name: Option<String>,
    pub audio_source_id: Option<i32>,
    pub cover_beatmap_id: Option<i32>,
}
