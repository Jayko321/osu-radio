use std::path::{Path, PathBuf};

use crate::OsuKind;

#[derive(Debug, Clone, Default)]
pub struct ImportedSnapshot {
    pub beatmap_sets: Vec<ImportedBeatmapSet>,
    pub collections: Vec<ImportedCollection>,
}

#[derive(Debug, Clone)]
pub struct ImportedCollection {
    pub source_id: String,
    pub name: String,
    pub beatmap_md5_hashes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ImportedBeatmapSet {
    pub source: OsuKind,
    pub online_id: Option<i32>,
    pub hash: Option<String>,
    pub files: Vec<RealmNamedFileUsage>,
    pub beatmaps: Vec<ImportedBeatmap>,
}

impl ImportedBeatmapSet {
    #[must_use]
    pub fn resolved_audio_path(&self, beatmap: &ImportedBeatmap) -> Option<&Path> {
        self.resolved_file(beatmap.metadata.as_ref()?.audio_file.as_deref()?)
    }

    #[must_use]
    pub fn resolved_background_path(&self, beatmap: &ImportedBeatmap) -> Option<&Path> {
        self.resolved_file(beatmap.metadata.as_ref()?.background_file.as_deref()?)
    }

    fn resolved_file(&self, filename: &str) -> Option<&Path> {
        self.files
            .iter()
            .find(|usage| usage.filename.as_deref() == Some(filename))
            .and_then(|usage| usage.file.as_ref())
            .and_then(|file| file.resolved_path.as_deref())
    }
}

#[derive(Debug, Clone)]
pub struct ImportedBeatmap {
    pub md5_hash: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn beatmap() -> ImportedBeatmap {
        ImportedBeatmap {
            md5_hash: None,
            difficulty_name: None,
            bpm: None,
            hash: None,
            metadata: Some(BeatmapMetadata {
                title: None,
                title_unicode: None,
                artist: None,
                artist_unicode: None,
                author: None,
                source: None,
                tags: None,
                user_tags: Vec::new(),
                preview_time: None,
                audio_file: Some("audio.mp3".into()),
                background_file: Some("cover.jpg".into()),
            }),
        }
    }

    fn usage(name: &str, path: &str) -> RealmNamedFileUsage {
        RealmNamedFileUsage {
            filename: Some(name.into()),
            file: Some(RealmFile {
                hash: None,
                resolved_path: Some(path.into()),
            }),
        }
    }

    fn set(files: Vec<RealmNamedFileUsage>) -> ImportedBeatmapSet {
        ImportedBeatmapSet {
            source: OsuKind::Lazer,
            online_id: None,
            hash: None,
            files,
            beatmaps: Vec::new(),
        }
    }

    #[test]
    fn resolves_audio_and_background_independently_without_reading_files() {
        let set = set(vec![
            usage("cover.jpg", "missing/曲/cover.jpg"),
            usage("audio.mp3", "missing/曲/audio.mp3"),
        ]);
        let beatmap = beatmap();
        assert_eq!(
            set.resolved_audio_path(&beatmap),
            Some(Path::new("missing/曲/audio.mp3"))
        );
        assert_eq!(
            set.resolved_background_path(&beatmap),
            Some(Path::new("missing/曲/cover.jpg"))
        );
        // Resolution uses only metadata/file usages; membership in set.beatmaps is not required.
        assert!(set.beatmaps.is_empty());
    }

    #[test]
    fn absent_metadata_and_filenames_have_no_resolved_paths() {
        let set = set(vec![
            usage("audio.mp3", "audio"),
            usage("cover.jpg", "cover"),
        ]);
        let mut beatmap = beatmap();
        if let Some(metadata) = &mut beatmap.metadata {
            metadata.audio_file = None;
            metadata.background_file = None;
        }
        assert_eq!(set.resolved_audio_path(&beatmap), None);
        assert_eq!(set.resolved_background_path(&beatmap), None);
        beatmap.metadata = None;
        assert_eq!(set.resolved_audio_path(&beatmap), None);
        assert_eq!(set.resolved_background_path(&beatmap), None);
    }

    #[test]
    fn filenames_match_exactly_including_case_and_whitespace() {
        let beatmap = beatmap();
        let unmatched = set(vec![
            usage("AUDIO.MP3", "wrong-case"),
            usage("audio.mp3 ", "wrong-space"),
            usage("cover.JPG", "wrong-cover"),
        ]);
        assert_eq!(unmatched.resolved_audio_path(&beatmap), None);
        assert_eq!(unmatched.resolved_background_path(&beatmap), None);
        assert_eq!(set(Vec::new()).resolved_audio_path(&beatmap), None);
    }

    #[test]
    fn first_matching_usage_wins_even_when_its_file_or_path_is_absent() {
        let beatmap = beatmap();
        for filename in ["audio.mp3", "cover.jpg"] {
            for first in [
                RealmNamedFileUsage {
                    filename: Some(filename.into()),
                    file: None,
                },
                RealmNamedFileUsage {
                    filename: Some(filename.into()),
                    file: Some(RealmFile {
                        hash: None,
                        resolved_path: None,
                    }),
                },
                usage(filename, "first"),
            ] {
                let expected = first
                    .file
                    .as_ref()
                    .and_then(|file| file.resolved_path.clone());
                let set = set(vec![
                    RealmNamedFileUsage {
                        filename: None,
                        file: None,
                    },
                    first,
                    usage(filename, "later"),
                ]);
                let resolved = if filename == "audio.mp3" {
                    set.resolved_audio_path(&beatmap)
                } else {
                    set.resolved_background_path(&beatmap)
                };
                assert_eq!(resolved, expected.as_deref());
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn resolved_paths_preserve_native_non_unicode_bytes() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        let path = PathBuf::from(OsString::from_vec(b"source/\xff.mp3".to_vec()));
        let set = set(vec![RealmNamedFileUsage {
            filename: Some("audio.mp3".into()),
            file: Some(RealmFile {
                hash: None,
                resolved_path: Some(path.clone()),
            }),
        }]);
        assert_eq!(set.resolved_audio_path(&beatmap()), Some(path.as_path()));
    }
}
