//! Read-only native import of the osu!stable database and background references.

mod media;
mod reader;
#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use radio_core::{
    OsuKind,
    import_types::{ImportedBeatmapSet, ImportedSnapshot, RealmFile, RealmNamedFileUsage},
};

/// Import a complete stable snapshot. Audio and artwork remain source references.
pub async fn import_from_stable_db(db_path: &Path) -> Result<Vec<ImportedBeatmapSet>> {
    Ok(import_snapshot_from_stable_db(db_path).await?.beatmap_sets)
}

pub async fn import_snapshot_from_stable_db(db_path: &Path) -> Result<ImportedSnapshot> {
    let db_path = db_path.to_path_buf();
    tokio::task::spawn_blocking(move || import(&db_path))
        .await
        .context("stable database import task failed")?
}

struct Group {
    set: ImportedBeatmapSet,
    filenames: HashSet<String>,
}

fn import(db_path: &Path) -> Result<ImportedSnapshot> {
    let db_path = std::path::absolute(db_path)
        .with_context(|| format!("failed to resolve `{}`", db_path.display()))?;
    let bytes = fs::read(&db_path)
        .with_context(|| format!("failed to read stable database `{}`", db_path.display()))?;
    let records = reader::decode(&bytes, &db_path)?;
    let installation = db_path.parent().context("database path has no parent")?;
    let collection_path = installation.join("collection.db");
    let collections = match fs::read(&collection_path) {
        Ok(bytes) => reader::decode_collections(&bytes, &collection_path)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read `{}`", collection_path.display()));
        }
    };
    let songs = media::songs_directory(installation)?;
    let mut indexes = HashMap::<PathBuf, usize>::new();
    let mut groups = Vec::<Group>::new();
    for mut record in records {
        let context = || {
            format!(
                "{}: record {}, byte offset {}",
                db_path.display(),
                record.index,
                record.offset
            )
        };
        let folder = media::relative_path(&record.folder).with_context(context)?;
        let directory = songs.join(folder);
        let osu_file = media::relative_path(&record.osu_file).with_context(context)?;
        let index = *indexes.entry(directory.clone()).or_insert_with(|| {
            let index = groups.len();
            groups.push(Group {
                set: ImportedBeatmapSet {
                    source: OsuKind::Stable,
                    online_id: record.online_id,
                    hash: None,
                    files: Vec::new(),
                    beatmaps: Vec::new(),
                },
                filenames: HashSet::new(),
            });
            index
        });
        let group = groups
            .get_mut(index)
            .context("missing stable folder group")?;
        if group.set.online_id != record.online_id {
            group.set.online_id = None;
        }
        let metadata = record
            .beatmap
            .metadata
            .as_mut()
            .context("missing stable metadata")?;
        if let Some(audio) = &mut metadata.audio_file {
            *audio = media::relative_path(audio).with_context(context)?;
            add_file(group, &directory, audio);
        }
        match media::background(&directory.join(osu_file)) {
            Ok(Some(background)) => {
                let background = media::relative_path(&background).with_context(context)?;
                add_file(group, &directory, &background);
                metadata.background_file = Some(background);
            }
            Ok(None) => {}
            Err(error) => eprintln!("{}: artwork unavailable: {error:#}", context()),
        }
        group.set.beatmaps.push(record.beatmap);
    }
    Ok(ImportedSnapshot {
        beatmap_sets: groups.into_iter().map(|group| group.set).collect(),
        collections,
    })
}

fn add_file(group: &mut Group, directory: &Path, filename: &str) {
    if group.filenames.insert(filename.to_owned()) {
        group.set.files.push(RealmNamedFileUsage {
            filename: Some(filename.to_owned()),
            file: Some(RealmFile {
                hash: None,
                resolved_path: Some(directory.join(filename)),
            }),
        });
    }
}
