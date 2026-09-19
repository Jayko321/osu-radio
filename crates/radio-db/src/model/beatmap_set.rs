#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeatmapSet {
    pub id: i32,
    pub online_id: Option<i32>,
    pub hash: Option<String>,
    pub installation_id: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
pub struct BeatmapSetWithAudio {
    pub has_multiple_audio_sources: bool,
    pub beatmap_set: BeatmapSet,
    pub audio_sources: Vec<super::AudioSource>,
    pub beatmaps: Vec<BeatmapDetails>,
}

/// Minimal audio-bearing relationships used to match a library snapshot.
#[derive(Debug)]
pub struct SearchDifficulty {
    pub set_id: i32,
    pub audio_source_id: i32,
    pub metadata_hash: Option<String>,
    pub difficulty_name: Option<String>,
}

/// Shared searchable text, loaded once per metadata identity.
#[derive(Debug)]
pub struct SearchMetadata {
    pub hash: String,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
}
