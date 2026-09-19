#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BeatmapMetadata {
    pub hash: String,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub author: Option<serde_json::Value>,
    pub source: Option<String>,
    pub preview_time: Option<i32>,
    pub audio_file: Option<String>,
    pub background_file: Option<String>,
}
