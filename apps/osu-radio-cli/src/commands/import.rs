use anyhow::Result;
use radio_core::{
    OsuMarker,
    import_types::{ImportedBeatmap, ImportedBeatmapSet},
};
use serde_json::{Value, json};

use crate::{
    commands::helpers::{
        beatmap_count, import_error, marker_to_json, print_json, print_table, select_marker,
        source_name,
    },
    consts::IMPORT_TABLE_WIDTHS,
    types::ImportArgs,
};

pub(crate) async fn import(args: ImportArgs) -> Result<()> {
    let marker = select_marker(&args.filters, args.index, args.marker.as_deref()).await?;

    let beatmap_sets = match radio_scanner::get_beatmap_sets(marker.clone()).await {
        Ok(beatmap_sets) => beatmap_sets,
        Err(error) => return Err(import_error(&marker, error)),
    };

    if args.json {
        print_import_json(&marker, &beatmap_sets, args.limit)?;
    } else {
        print_import_table(&marker, &beatmap_sets, args.limit, args.verbose);
    }

    Ok(())
}

fn print_import_table(
    marker: &OsuMarker,
    beatmap_sets: &[ImportedBeatmapSet],
    limit: usize,
    verbose: bool,
) {
    println!(
        "Imported {} beatmap set(s) and {} beatmap(s) from {} marker `{}`.",
        beatmap_sets.len(),
        beatmap_count(beatmap_sets),
        source_name(marker.kind),
        marker.marker_path.display()
    );

    if beatmap_sets.is_empty() || limit == 0 {
        return;
    }

    let rows = beatmap_sets
        .iter()
        .flat_map(|beatmap_set| {
            beatmap_set
                .beatmaps
                .iter()
                .map(move |beatmap| (beatmap_set, beatmap))
        })
        .take(limit)
        .enumerate()
        .map(|(index, (beatmap_set, beatmap))| {
            vec![
                index.saturating_add(1).to_string(),
                source_name(beatmap_set.source).to_string(),
                beatmap_artist(beatmap),
                beatmap_title(beatmap),
                beatmap
                    .difficulty_name
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                beatmap_bpm(beatmap),
                audio_status(beatmap_set, beatmap).to_string(),
            ]
        })
        .collect::<Vec<_>>();

    print_table(
        &[
            "Index",
            "Source",
            "Artist",
            "Title",
            "Difficulty",
            "BPM",
            "Audio",
        ],
        &rows,
        IMPORT_TABLE_WIDTHS,
    );

    if beatmap_count(beatmap_sets) > limit {
        println!(
            "Showing first {limit} of {} beatmap(s). Use `--limit` to show more.",
            beatmap_count(beatmap_sets)
        );
    }

    if verbose {
        println!(
            "Audio status is based on whether the imported record includes a resolved audio file reference."
        );
    }
}

fn print_import_json(
    marker: &OsuMarker,
    beatmap_sets: &[ImportedBeatmapSet],
    limit: usize,
) -> Result<()> {
    let mut remaining = limit;
    let shown = beatmap_sets
        .iter()
        .filter_map(|beatmap_set| {
            let take = remaining.min(beatmap_set.beatmaps.len());
            remaining = remaining.saturating_sub(take);
            (take > 0).then(|| beatmap_set_to_json(beatmap_set, take))
        })
        .collect::<Vec<_>>();

    print_json(&json!({
        "marker": marker_to_json(0, marker),
        "total_beatmap_sets": beatmap_sets.len(),
        "total_beatmaps": beatmap_count(beatmap_sets),
        "shown_beatmaps": limit.min(beatmap_count(beatmap_sets)),
        "beatmap_sets": shown,
    }))
}

fn beatmap_set_to_json(beatmap_set: &ImportedBeatmapSet, beatmap_limit: usize) -> Value {
    json!({
        "source": source_name(beatmap_set.source),
        "online_id": beatmap_set.online_id,
        "hash": beatmap_set.hash,
        "files": beatmap_set.files.iter().map(|usage| json!({
            "filename": usage.filename,
            "file": usage.file.as_ref().map(|file| json!({
                "hash": file.hash,
                "resolved_path": file.resolved_path.as_ref().map(|path| path.display().to_string()),
            })),
        })).collect::<Vec<_>>(),
        "beatmaps": beatmap_set.beatmaps.iter().take(beatmap_limit).map(|beatmap| beatmap_to_json(beatmap_set, beatmap)).collect::<Vec<_>>(),
    })
}

fn beatmap_to_json(beatmap_set: &ImportedBeatmapSet, beatmap: &ImportedBeatmap) -> Value {
    json!({
        "difficulty_name": beatmap.difficulty_name,
        "bpm": beatmap.bpm,
        "hash": beatmap.hash,
        "metadata": beatmap.metadata.as_ref().map(|metadata| json!({
            "title": metadata.title,
            "title_unicode": metadata.title_unicode,
            "artist": metadata.artist,
            "artist_unicode": metadata.artist_unicode,
            "author": metadata.author.as_ref().map(|author| json!({
                "online_id": author.online_id,
                "username": author.username,
                "country_code": author.country_code,
            })),
            "source": metadata.source,
            "tags": metadata.tags,
            "user_tags": metadata.user_tags,
            "preview_time": metadata.preview_time,
            "audio_file": metadata.audio_file,
            "background_file": metadata.background_file,
        })),
        "resolved_audio_path": beatmap_set.resolved_audio_path(beatmap).map(|path| path.display().to_string()),
    })
}

fn beatmap_artist(beatmap: &ImportedBeatmap) -> String {
    beatmap
        .metadata
        .as_ref()
        .and_then(|metadata| {
            metadata
                .artist_unicode
                .as_ref()
                .or(metadata.artist.as_ref())
        })
        .cloned()
        .unwrap_or_else(|| "-".to_string())
}

fn beatmap_title(beatmap: &ImportedBeatmap) -> String {
    beatmap
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.title_unicode.as_ref().or(metadata.title.as_ref()))
        .cloned()
        .unwrap_or_else(|| "-".to_string())
}

fn beatmap_bpm(beatmap: &ImportedBeatmap) -> String {
    beatmap
        .bpm
        .map_or_else(|| "-".to_string(), |bpm| format!("{bpm:.1}"))
}

fn audio_status(beatmap_set: &ImportedBeatmapSet, beatmap: &ImportedBeatmap) -> &'static str {
    match beatmap_set.resolved_audio_path(beatmap) {
        Some(_) => "resolved",
        None if beatmap
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.audio_file.as_ref())
            .is_some() =>
        {
            "unresolved"
        }
        None => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use radio_core::{
        OsuKind,
        import_types::{BeatmapMetadata, RealmFile, RealmNamedFileUsage, RealmUser},
    };
    use std::path::PathBuf;

    fn fixture() -> (ImportedBeatmapSet, ImportedBeatmap) {
        let beatmap = ImportedBeatmap {
            md5_hash: None,
            difficulty_name: Some("Hard".into()),
            bpm: Some(123.45),
            hash: Some("map-hash".into()),
            metadata: Some(BeatmapMetadata {
                title: Some("Song".into()),
                title_unicode: Some("曲".into()),
                artist: Some("Artist".into()),
                artist_unicode: Some("歌手".into()),
                author: Some(RealmUser {
                    online_id: Some(42),
                    username: Some("Mapper".into()),
                    country_code: Some("JP".into()),
                }),
                source: Some("Game".into()),
                tags: Some("tag two".into()),
                user_tags: vec!["user tag".into()],
                preview_time: Some(1234),
                audio_file: Some("audio.mp3".into()),
                background_file: Some("cover.jpg".into()),
            }),
        };
        let set = ImportedBeatmapSet {
            source: OsuKind::Stable,
            online_id: Some(123),
            hash: Some("set-hash".into()),
            files: vec![RealmNamedFileUsage {
                filename: Some("audio.mp3".into()),
                file: Some(RealmFile {
                    hash: Some("audio-hash".into()),
                    resolved_path: Some(PathBuf::from("fixture/audio.mp3")),
                }),
            }],
            beatmaps: vec![beatmap.clone(), beatmap.clone()],
        };
        (set, beatmap)
    }

    #[test]
    fn table_prefers_unicode_with_ordinary_and_unknown_fallbacks() {
        let (_, mut beatmap) = fixture();
        assert_eq!(beatmap_artist(&beatmap), "歌手");
        assert_eq!(beatmap_title(&beatmap), "曲");
        assert_eq!(beatmap_bpm(&beatmap), "123.5");
        let metadata = beatmap.metadata.as_mut().unwrap();
        metadata.artist_unicode = None;
        metadata.title_unicode = None;
        assert_eq!(beatmap_artist(&beatmap), "Artist");
        assert_eq!(beatmap_title(&beatmap), "Song");
        let metadata = beatmap.metadata.as_mut().unwrap();
        metadata.artist = None;
        metadata.title = None;
        assert_eq!(beatmap_artist(&beatmap), "-");
        assert_eq!(beatmap_title(&beatmap), "-");
        beatmap.metadata = None;
        beatmap.bpm = None;
        assert_eq!(beatmap_artist(&beatmap), "-");
        assert_eq!(beatmap_title(&beatmap), "-");
        assert_eq!(beatmap_bpm(&beatmap), "-");
    }

    #[test]
    fn audio_status_distinguishes_a_reference_from_an_unresolved_or_unknown_file() {
        let (mut set, mut beatmap) = fixture();
        assert_eq!(audio_status(&set, &beatmap), "resolved");
        set.files.clear();
        assert_eq!(audio_status(&set, &beatmap), "unresolved");
        beatmap.metadata.as_mut().unwrap().audio_file = None;
        assert_eq!(audio_status(&set, &beatmap), "unknown");
        beatmap.metadata = None;
        assert_eq!(audio_status(&set, &beatmap), "unknown");
    }

    #[test]
    fn json_preserves_metadata_file_references_and_limits_difficulties() {
        let (set, _) = fixture();
        let value = beatmap_set_to_json(&set, 1);
        assert_eq!(value["source"], "stable");
        assert_eq!(value["online_id"], 123);
        assert_eq!(value["hash"], "set-hash");
        assert_eq!(
            value["files"],
            json!([{
                "filename": "audio.mp3", "file": {
                    "hash": "audio-hash", "resolved_path": PathBuf::from("fixture/audio.mp3").display().to_string(),
                },
            }])
        );
        assert_eq!(value["beatmaps"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["beatmaps"][0],
            json!({
                "difficulty_name": "Hard", "bpm": 123.45, "hash": "map-hash",
                "metadata": {
                    "title": "Song", "title_unicode": "曲", "artist": "Artist", "artist_unicode": "歌手",
                    "author": { "online_id": 42, "username": "Mapper", "country_code": "JP" },
                    "source": "Game", "tags": "tag two", "user_tags": ["user tag"],
                    "preview_time": 1234, "audio_file": "audio.mp3", "background_file": "cover.jpg",
                },
                "resolved_audio_path": PathBuf::from("fixture/audio.mp3").display().to_string(),
            })
        );
        assert_eq!(beatmap_count(&[set]), 2);
        assert_eq!(beatmap_count(&[]), 0);
    }

    #[test]
    fn json_retains_null_fields_and_empty_file_records() {
        let (mut set, mut beatmap) = fixture();
        beatmap.metadata = None;
        beatmap.difficulty_name = None;
        beatmap.hash = None;
        beatmap.bpm = None;
        assert_eq!(
            beatmap_to_json(&set, &beatmap),
            json!({
                "difficulty_name": null, "bpm": null, "hash": null,
                "metadata": null, "resolved_audio_path": null,
            })
        );
        set.files = vec![
            RealmNamedFileUsage {
                filename: None,
                file: None,
            },
            RealmNamedFileUsage {
                filename: Some("missing".into()),
                file: Some(RealmFile {
                    hash: None,
                    resolved_path: None,
                }),
            },
        ];
        let value = beatmap_set_to_json(&set, 0);
        assert_eq!(value["beatmaps"], json!([]));
        assert_eq!(
            value["files"],
            json!([
                { "filename": null, "file": null },
                { "filename": "missing", "file": { "hash": null, "resolved_path": null } },
            ])
        );
        let metadata = set.beatmaps[0].metadata.as_mut().unwrap();
        metadata.author = None;
        assert_eq!(
            beatmap_set_to_json(&set, usize::MAX)["beatmaps"][0]["metadata"]["author"],
            Value::Null
        );
    }
}
