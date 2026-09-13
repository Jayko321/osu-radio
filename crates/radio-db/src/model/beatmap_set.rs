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

#[derive(Debug)]
pub struct BeatmapSetWithAudio {
    pub beatmap_set: BeatmapSet,
    pub audio_sources: Vec<super::AudioSource>,
    pub beatmaps: Vec<BeatmapDetails>,
}
