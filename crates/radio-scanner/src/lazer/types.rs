use std::path::PathBuf;

use anyhow::{Context, Result};
use radio_core::{
    OsuKind,
    import_types::{
        BeatmapMetadata, ImportedBeatmap, ImportedBeatmapSet, RealmFile, RealmNamedFileUsage,
        RealmUser,
    },
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
enum LazerSource {
    Lazer,
}

#[derive(Debug, Deserialize)]
struct LazerBeatmapSetRecord {
    source: LazerSource,
    online_id: Option<i32>,
    hash: Option<String>,
    #[serde(default)]
    files: Vec<LazerRealmNamedFileUsage>,
    beatmaps: Vec<LazerBeatmapRecord>,
}

#[derive(Debug, Deserialize)]
struct LazerBeatmapRecord {
    difficulty_name: Option<String>,
    bpm: Option<f64>,
    hash: Option<String>,
    metadata: Option<LazerBeatmapMetadata>,
}

#[derive(Debug, Deserialize)]
struct LazerBeatmapMetadata {
    title: Option<String>,
    title_unicode: Option<String>,
    artist: Option<String>,
    artist_unicode: Option<String>,
    author: Option<LazerRealmUser>,
    source: Option<String>,
    tags: Option<String>,
    #[serde(default)]
    user_tags: Vec<String>,
    preview_time: Option<i32>,
    audio_file: Option<String>,
    background_file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LazerRealmNamedFileUsage {
    filename: Option<String>,
    file: Option<LazerRealmFile>,
}

#[derive(Debug, Deserialize)]
struct LazerRealmFile {
    hash: Option<String>,
    resolved_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct LazerRealmUser {
    online_id: Option<i32>,
    username: Option<String>,
    country_code: Option<String>,
}

pub(crate) fn parse_lazer_beatmap_set_line(line: &str) -> Result<ImportedBeatmapSet> {
    let record: LazerBeatmapSetRecord =
        serde_json::from_str(line).context("failed to parse osu!lazer beatmap set JSON")?;

    Ok(record.into())
}

impl From<LazerBeatmapSetRecord> for ImportedBeatmapSet {
    fn from(record: LazerBeatmapSetRecord) -> Self {
        let LazerBeatmapSetRecord {
            source,
            online_id,
            hash,
            files,
            beatmaps,
        } = record;

        Self {
            source: match source {
                LazerSource::Lazer => OsuKind::Lazer,
            },
            online_id,
            hash,
            files: files.into_iter().map(Into::into).collect(),
            beatmaps: beatmaps.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<LazerBeatmapRecord> for ImportedBeatmap {
    fn from(record: LazerBeatmapRecord) -> Self {
        Self {
            difficulty_name: record.difficulty_name,
            bpm: record.bpm,
            hash: record.hash,
            metadata: record.metadata.map(Into::into),
        }
    }
}

impl From<LazerBeatmapMetadata> for BeatmapMetadata {
    fn from(metadata: LazerBeatmapMetadata) -> Self {
        Self {
            title: metadata.title,
            title_unicode: metadata.title_unicode,
            artist: metadata.artist,
            artist_unicode: metadata.artist_unicode,
            author: metadata.author.map(Into::into),
            source: metadata.source,
            tags: metadata.tags,
            user_tags: metadata.user_tags,
            preview_time: metadata.preview_time,
            audio_file: metadata.audio_file,
            background_file: metadata.background_file,
        }
    }
}

impl From<LazerRealmNamedFileUsage> for RealmNamedFileUsage {
    fn from(usage: LazerRealmNamedFileUsage) -> Self {
        Self {
            filename: usage.filename,
            file: usage.file.map(Into::into),
        }
    }
}

impl From<LazerRealmFile> for RealmFile {
    fn from(file: LazerRealmFile) -> Self {
        Self {
            hash: file.hash,
            resolved_path: file.resolved_path,
        }
    }
}

impl From<LazerRealmUser> for RealmUser {
    fn from(user: LazerRealmUser) -> Self {
        Self {
            online_id: user.online_id,
            username: user.username,
            country_code: user.country_code,
        }
    }
}
