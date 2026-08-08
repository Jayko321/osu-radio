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
                (index + 1).to_string(),
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
            remaining -= take;
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
        .map(|bpm| format!("{bpm:.1}"))
        .unwrap_or_else(|| "-".to_string())
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
