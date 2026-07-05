use std::path::PathBuf;

use crate::OsuKind;

#[derive(Debug, Clone)]
pub struct ImportedBeatmap {
    pub source: OsuKind,
    pub difficulty_name: Option<String>,
    pub bpm: Option<f64>,
    pub hash: Option<String>,
    pub metadata: Option<BeatmapMetadata>,
    pub beatmap_set: Option<BeatmapSet>,
}

#[derive(Debug, Clone)]
pub struct BeatmapMetadata {
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub author: Option<RealmUser>,
    pub source: Option<String>,
    pub tags: Option<String>,
    pub user_tags: Vec<String>,
    pub preview_time: Option<i32>,
    pub audio_file: Option<String>,
    pub background_file: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BeatmapSet {
    pub online_id: Option<i32>,
    pub hash: Option<String>,
    pub files: Vec<RealmNamedFileUsage>,
}

#[derive(Debug, Clone)]
pub struct RealmNamedFileUsage {
    pub filename: Option<String>,
    pub file: Option<RealmFile>,
}

#[derive(Debug, Clone)]
pub struct RealmFile {
    pub hash: Option<String>,
    pub resolved_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RealmUser {
    pub online_id: Option<i32>,
    pub username: Option<String>,
    pub country_code: Option<String>,
}
