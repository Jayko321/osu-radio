use anyhow::Result;
use radio_core::{OsuMarker, import_types::ImportedBeatmapSet};
use radio_services::{ImportSummary, Services, model::OsuInstallation};
use serde_json::json;

use crate::{
    commands::{
        database::open_database,
        helpers::{
            beatmap_count, import_error, marker_to_json, print_json, select_marker, source_name,
        },
    },
    types::StoreArgs,
};

pub(crate) async fn store(args: StoreArgs) -> Result<()> {
    let marker = select_marker(&args.filters, args.index, args.marker.as_deref()).await?;

    let beatmap_sets = match radio_scanner::get_beatmap_sets(marker.clone()).await {
        Ok(beatmap_sets) => beatmap_sets,
        Err(error) => return Err(import_error(&marker, error)),
    };

    let database = open_database().await?;
    prepare_schema(&database, args.clear).await?;

    let registered = database.osu_installations().register(&marker, None).await?;
    let newly_registered = registered.was_created();
    let installation = registered.into_installation();

    let summary = database
        .osu_installations()
        .replace_snapshot(installation.id, &beatmap_sets)
        .await?;

    if args.json {
        print_store_json(&marker, &installation, &beatmap_sets, &summary, &args)?;
    } else {
        print_store_summary(&marker, &installation, newly_registered, &summary, &args);
    }

    Ok(())
}

async fn prepare_schema(database: &Services, clear: bool) -> Result<()> {
    if clear {
        database.reset().await
    } else {
        database.migrate().await
    }
}

fn print_store_summary(
    marker: &OsuMarker,
    installation: &OsuInstallation,
    newly_registered: bool,
    summary: &ImportSummary,
    args: &StoreArgs,
) {
    if args.clear {
        println!("Cleared every previously stored beatmap and registered osu! folder.");
    }

    if newly_registered {
        println!(
            "Registered osu! folder #{} at `{}`.",
            installation.id,
            installation.root_path.display()
        );
    } else {
        println!(
            "Importing into already registered osu! folder #{}.",
            installation.id
        );
    }

    println!(
        "Stored {} beatmap set(s), {} beatmap(s), referencing {} distinct audio source(s) from {} marker `{}`.",
        summary.beatmap_sets,
        summary.beatmaps,
        summary.audio_sources,
        source_name(marker.kind),
        marker.marker_path.display()
    );

    if args.verbose {
        println!(
            "Audio is referenced in place: each stored source points at the osu! file path, and nothing was copied."
        );
        println!("Re-running replaces this installation's complete library snapshot.");
    }
}

fn print_store_json(
    marker: &OsuMarker,
    installation: &OsuInstallation,
    beatmap_sets: &[ImportedBeatmapSet],
    summary: &ImportSummary,
    args: &StoreArgs,
) -> Result<()> {
    print_json(&json!({
        "marker": marker_to_json(0, marker),
        "installation_id": installation.id,
        "cleared": args.clear,
        "discovered_beatmap_sets": beatmap_sets.len(),
        "discovered_beatmaps": beatmap_count(beatmap_sets),
        "stored_beatmap_sets": summary.beatmap_sets,
        "stored_beatmaps": summary.beatmaps,
        "stored_audio_sources": summary.audio_sources,
    }))
}
