use std::path::Path;

use radio_core::{OsuKind, OsuMarker, import_types::ImportedBeatmap};
use radio_scanner::ScannerError;
use serde_json::{Value, json};

use crate::{
    commands::helpers::{
        discover_markers, marker_from_path, marker_to_json, print_json, print_markers_table,
        print_table, source_name,
    },
    consts::IMPORT_TABLE_WIDTHS,
    types::ImportArgs,
};

pub(crate) async fn import(args: ImportArgs) -> Result<(), String> {
    let marker = select_marker(&args)?;

    match radio_scanner::get_beatmaps(marker.clone()).await {
        Ok(beatmaps) => {
            if args.json {
                print_import_json(&marker, &beatmaps, args.limit)?;
            } else {
                print_import_table(&marker, &beatmaps, args.limit, args.verbose);
            }
            Ok(())
        }
        Err(error) => Err(import_error_message(&marker, error)),
    }
}

fn select_marker(args: &ImportArgs) -> Result<OsuMarker, String> {
    if args.index.is_some() && args.marker.is_some() {
        return Err("Use either `--index` or `--marker`, not both.".to_string());
    }

    if let Some(marker_path) = &args.marker {
        return marker_from_path(marker_path, args.filters.source);
    }

    let markers = discover_markers(&args.filters)?;

    if let Some(index) = args.index {
        if index == 0 {
            return Err(
                "`--index` is 1-based. Use an index from `osu-radio-cli scan`.".to_string(),
            );
        }

        return markers.get(index - 1).cloned().ok_or_else(|| {
            format!(
                "No discovered osu! installation has index {index}. Run `osu-radio-cli scan` to see available indexes."
            )
        });
    }

    match markers.as_slice() {
        [] => Err(
            "No osu! installations found. Run `osu-radio-cli scan` to inspect discovery results."
                .to_string(),
        ),
        [marker] => Ok(marker.clone()),
        _ => {
            print_markers_table(&markers);
            Err("Multiple osu! installations found. Choose one with `osu-radio-cli import --index <INDEX>`.".to_string())
        }
    }
}

fn import_error_message(marker: &OsuMarker, error: ScannerError) -> String {
    match error {
        ScannerError::UnsupportedSource(OsuKind::Stable) => {
            "Stable osu! installations are detected, but stable importing is not supported yet. Try a lazer source or run `osu-radio-cli scan --source lazer`.".to_string()
        }
        ScannerError::UnsupportedSource(kind) => {
            format!("The selected osu! source is not supported yet: {kind:?}.")
        }
        ScannerError::Import(error) => format!(
            "Failed to import beatmaps from {} marker `{}`: {error}",
            source_name(marker.kind),
            marker.marker_path.display()
        ),
    }
}

fn print_import_table(
    marker: &OsuMarker,
    beatmaps: &[ImportedBeatmap],
    limit: usize,
    verbose: bool,
) {
    println!(
        "Imported {} beatmap(s) from {} marker `{}`.",
        beatmaps.len(),
        source_name(marker.kind),
        marker.marker_path.display()
    );

    if beatmaps.is_empty() || limit == 0 {
        return;
    }

    let rows = beatmaps
        .iter()
        .take(limit)
        .enumerate()
        .map(|(index, beatmap)| {
            vec![
                (index + 1).to_string(),
                source_name(beatmap.source).to_string(),
                beatmap_artist(beatmap),
                beatmap_title(beatmap),
                beatmap
                    .difficulty_name
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
                beatmap_bpm(beatmap),
                audio_status(beatmap).to_string(),
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

    if beatmaps.len() > limit {
        println!(
            "Showing first {limit} of {} beatmap(s). Use `--limit` to show more.",
            beatmaps.len()
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
    beatmaps: &[ImportedBeatmap],
    limit: usize,
) -> Result<(), String> {
    let shown = beatmaps.iter().take(limit).collect::<Vec<_>>();
    print_json(&json!({
        "marker": marker_to_json(0, marker),
        "total_beatmaps": beatmaps.len(),
        "shown_beatmaps": shown.len(),
        "beatmaps": shown.into_iter().map(beatmap_to_json).collect::<Vec<_>>(),
    }))
}

fn beatmap_to_json(beatmap: &ImportedBeatmap) -> Value {
    json!({
        "source": source_name(beatmap.source),
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
        "beatmap_set": beatmap.beatmap_set.as_ref().map(|beatmap_set| json!({
            "online_id": beatmap_set.online_id,
            "hash": beatmap_set.hash,
            "files": beatmap_set.files.iter().map(|usage| json!({
                "filename": usage.filename,
                "file": usage.file.as_ref().map(|file| json!({
                    "hash": file.hash,
                    "resolved_path": file.resolved_path.as_ref().map(|path| path.display().to_string()),
                })),
            })).collect::<Vec<_>>(),
        })),
        "resolved_audio_path": resolved_audio_path(beatmap).map(|path| path.display().to_string()),
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
        .map(|bpm| format!("{bpm:.1}"))
        .unwrap_or_else(|| "-".to_string())
}

fn audio_status(beatmap: &ImportedBeatmap) -> &'static str {
    match resolved_audio_path(beatmap) {
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

fn resolved_audio_path(beatmap: &ImportedBeatmap) -> Option<&Path> {
    let metadata = beatmap.metadata.as_ref()?;
    let audio_file = metadata.audio_file.as_ref()?;
    let beatmap_set = beatmap.beatmap_set.as_ref()?;

    beatmap_set
        .files
        .iter()
        .find(|usage| usage.filename.as_ref() == Some(audio_file))
        .and_then(|usage| usage.file.as_ref())
        .and_then(|file| file.resolved_path.as_deref())
}
