use std::path::{Path, PathBuf};

use crate::OsuKind;

#[derive(Debug, Clone)]
pub struct ImportedBeatmapSet {
    pub source: OsuKind,
    pub online_id: Option<i32>,
    pub hash: Option<String>,
    pub files: Vec<RealmNamedFileUsage>,
    pub beatmaps: Vec<ImportedBeatmap>,
}

impl ImportedBeatmapSet {
    pub fn resolved_audio_path(&self, beatmap: &ImportedBeatmap) -> Option<&Path> {
        let audio_file = beatmap.metadata.as_ref()?.audio_file.as_ref()?;

        self.files
            .iter()
            .find(|usage| usage.filename.as_ref() == Some(audio_file))
            .and_then(|usage| usage.file.as_ref())
            .and_then(|file| file.resolved_path.as_deref())
    }
}

#[derive(Debug, Clone)]
pub struct ImportedBeatmap {
    pub difficulty_name: Option<String>,
    pub bpm: Option<f64>,
    pub hash: Option<String>,
    pub metadata: Option<BeatmapMetadata>,
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
