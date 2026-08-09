use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, QueryResult, SelectableHelper};
use diesel_async::{AsyncConnection, RunQueryDsl, scoped_futures::ScopedFutureExt};
use radio_core::import_types::{
    BeatmapMetadata as ImportedMetadata, ImportedBeatmap, ImportedBeatmapSet,
};

use crate::{
    connection::DatabaseConnection,
    model::{
        AudioSource, BeatmapSet, NewAudioSource, NewBeatmap, NewBeatmapMetadata, NewBeatmapSet,
        SourceType,
    },
    schema::{audio_sources, beatmap_metadata, beatmap_sets, beatmaps},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub beatmap_sets: usize,
    pub beatmaps: usize,
    pub audio_sources: usize,
    pub skipped_beatmap_sets: usize,
}

pub async fn all_beatmap_sets_with_audio_sources(
    connection: &mut DatabaseConnection,
) -> Result<Vec<(BeatmapSet, Vec<AudioSource>)>> {
    let beatmap_sets: Vec<BeatmapSet> = beatmap_sets::table
        .order(beatmap_sets::id.asc())
        .load(connection)
        .await?;

    let linked: Vec<(Option<i32>, AudioSource)> = beatmaps::table
        .inner_join(beatmap_metadata::table.inner_join(audio_sources::table))
        .select((beatmaps::beatmap_set_id, AudioSource::as_select()))
        .order((beatmaps::beatmap_set_id.asc(), audio_sources::id.asc()))
        .load(connection)
        .await?;

    let mut grouped: HashMap<i32, Vec<AudioSource>> = HashMap::new();
    for (beatmap_set_id, audio_source) in linked {
        let Some(beatmap_set_id) = beatmap_set_id else {
            continue;
        };

        let audio_sources = grouped.entry(beatmap_set_id).or_default();
        if !audio_sources
            .iter()
            .any(|stored| stored.id == audio_source.id)
        {
            audio_sources.push(audio_source);
        }
    }

    Ok(beatmap_sets
        .into_iter()
        .map(|beatmap_set| {
            let audio_sources = grouped.remove(&beatmap_set.id).unwrap_or_default();
            (beatmap_set, audio_sources)
        })
        .collect())
}

pub async fn insert_beatmap_sets(
    connection: &mut DatabaseConnection,
    imported: &[ImportedBeatmapSet],
    beatmap_set_limit: Option<usize>,
    installation_id: Option<i32>,
) -> Result<ImportSummary> {
    let storable = beatmap_set_limit.unwrap_or(usize::MAX).min(imported.len());

    let summary = connection
        .transaction::<_, diesel::result::Error, _>(|connection| {
            async move {
                let mut summary = ImportSummary {
                    skipped_beatmap_sets: imported.len() - storable,
                    ..ImportSummary::default()
                };
                let mut audio_source_ids = HashMap::new();

                for beatmap_set in &imported[..storable] {
                    let beatmap_set_id =
                        insert_beatmap_set(connection, beatmap_set, installation_id).await?;
                    summary.beatmap_sets += 1;

                    for beatmap in &beatmap_set.beatmaps {
                        let metadata_id = insert_metadata(
                            connection,
                            &mut audio_source_ids,
                            beatmap_set,
                            beatmap,
                        )
                        .await?;

                        insert_beatmap(
                            connection,
                            beatmap_set,
                            beatmap,
                            beatmap_set_id,
                            metadata_id,
                        )
                        .await?;
                        summary.beatmaps += 1;
                    }
                }

                summary.audio_sources = audio_source_ids.len();
                Ok(summary)
            }
            .scope_boxed()
        })
        .await?;

    Ok(summary)
}

async fn insert_beatmap_set(
    connection: &mut DatabaseConnection,
    beatmap_set: &ImportedBeatmapSet,
    installation_id: Option<i32>,
) -> QueryResult<i32> {
    diesel::insert_into(beatmap_sets::table)
        .values(NewBeatmapSet {
            online_id: beatmap_set.online_id,
            hash: beatmap_set.hash.as_deref(),
            installation_id,
        })
        .returning(beatmap_sets::id)
        .get_result(connection)
        .await
}

async fn insert_beatmap(
    connection: &mut DatabaseConnection,
    beatmap_set: &ImportedBeatmapSet,
    beatmap: &ImportedBeatmap,
    beatmap_set_id: i32,
    metadata_id: Option<i32>,
) -> QueryResult<i32> {
    diesel::insert_into(beatmaps::table)
        .values(NewBeatmap {
            source: beatmap_set.source.as_str(),
            difficulty_name: beatmap.difficulty_name.as_deref(),
            bpm: beatmap.bpm,
            hash: beatmap.hash.as_deref(),
            beatmap_set_id: Some(beatmap_set_id),
            metadata_id,
        })
        .returning(beatmaps::id)
        .get_result(connection)
        .await
}

async fn insert_metadata(
    connection: &mut DatabaseConnection,
    audio_source_ids: &mut HashMap<String, i32>,
    beatmap_set: &ImportedBeatmapSet,
    beatmap: &ImportedBeatmap,
) -> QueryResult<Option<i32>> {
    let Some(metadata) = beatmap.metadata.as_ref() else {
        return Ok(None);
    };

    let audio_source_id = match beatmap_set.resolved_audio_path(beatmap) {
        Some(path) => Some(audio_source_id(connection, audio_source_ids, path).await?),
        None => None,
    };

    let id = diesel::insert_into(beatmap_metadata::table)
        .values(new_metadata(metadata, audio_source_id))
        .returning(beatmap_metadata::id)
        .get_result(connection)
        .await?;

    Ok(Some(id))
}

async fn audio_source_id(
    connection: &mut DatabaseConnection,
    audio_source_ids: &mut HashMap<String, i32>,
    path: &Path,
) -> QueryResult<i32> {
    let location = path.to_string_lossy().into_owned();
    if let Some(id) = audio_source_ids.get(&location) {
        return Ok(*id);
    }

    let source = SourceType::Local(location.clone());
    let columns = NewAudioSource::from(&source);

    let stored = audio_sources::table
        .filter(audio_sources::kind.eq(columns.kind))
        .filter(audio_sources::location.eq(columns.location))
        .select(audio_sources::id)
        .first(connection)
        .await
        .optional()?;

    let id = match stored {
        Some(id) => id,
        None => {
            diesel::insert_into(audio_sources::table)
                .values(columns)
                .returning(audio_sources::id)
                .get_result(connection)
                .await?
        }
    };

    audio_source_ids.insert(location, id);
    Ok(id)
}

fn new_metadata(
    metadata: &ImportedMetadata,
    audio_source_id: Option<i32>,
) -> NewBeatmapMetadata<'_> {
    let author = metadata.author.as_ref();

    NewBeatmapMetadata {
        title: metadata.title.as_deref(),
        title_unicode: metadata.title_unicode.as_deref(),
        artist: metadata.artist.as_deref(),
        artist_unicode: metadata.artist_unicode.as_deref(),
        author_online_id: author.and_then(|author| author.online_id),
        author_username: author.and_then(|author| author.username.as_deref()),
        author_country_code: author.and_then(|author| author.country_code.as_deref()),
        source: metadata.source.as_deref(),
        tags: metadata.tags.as_deref(),
        user_tags: (!metadata.user_tags.is_empty()).then(|| metadata.user_tags.join("\n")),
        preview_time: metadata.preview_time,
        audio_source_id,
        background_file: metadata.background_file.as_deref(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use diesel_async::{AsyncConnection, SimpleAsyncConnection};
    use radio_core::OsuKind;
    use radio_core::import_types::{
        BeatmapMetadata, ImportedBeatmap, ImportedBeatmapSet, RealmFile, RealmNamedFileUsage,
    };

    use super::*;

    async fn connection() -> DatabaseConnection {
        let mut connection = DatabaseConnection::establish(":memory:")
            .await
            .expect("database connection should open");
        connection
            .batch_execute("PRAGMA foreign_keys = ON;")
            .await
            .expect("foreign keys should be enforced");
        connection
            .batch_execute(crate::CREATE_SCHEMA)
            .await
            .expect("initial schema migration should apply");

        connection
    }

    fn beatmap(difficulty_name: &str, audio_file: &str) -> ImportedBeatmap {
        ImportedBeatmap {
            difficulty_name: Some(difficulty_name.to_owned()),
            bpm: Some(180.0),
            hash: Some(format!("hash-{difficulty_name}")),
            metadata: Some(BeatmapMetadata {
                title: Some("Song".to_owned()),
                title_unicode: None,
                artist: Some("Artist".to_owned()),
                artist_unicode: None,
                author: None,
                source: None,
                tags: None,
                user_tags: vec!["calm".to_owned(), "piano only".to_owned()],
                preview_time: Some(4200),
                audio_file: Some(audio_file.to_owned()),
                background_file: None,
            }),
        }
    }

    fn beatmap_set(audio_file: &str, beatmaps: Vec<ImportedBeatmap>) -> ImportedBeatmapSet {
        ImportedBeatmapSet {
            source: OsuKind::Lazer,
            online_id: Some(1),
            hash: Some("set-hash".to_owned()),
            files: vec![RealmNamedFileUsage {
                filename: Some(audio_file.to_owned()),
                file: Some(RealmFile {
                    hash: Some("abc".to_owned()),
                    resolved_path: Some(PathBuf::from("/osu/files/a/ab/abc")),
                }),
            }],
            beatmaps,
        }
    }

    #[tokio::test]
    async fn stores_a_shared_audio_source_once_per_location() {
        let mut connection = connection().await;
        let sets = vec![beatmap_set(
            "audio.mp3",
            vec![beatmap("Easy", "audio.mp3"), beatmap("Hard", "audio.mp3")],
        )];

        let summary = insert_beatmap_sets(&mut connection, &sets, None, None)
            .await
            .expect("beatmap sets should store");

        assert_eq!(
            summary,
            ImportSummary {
                beatmap_sets: 1,
                beatmaps: 2,
                audio_sources: 1,
                skipped_beatmap_sets: 0,
            }
        );

        let stored_sources: i64 = audio_sources::table
            .count()
            .get_result(&mut connection)
            .await
            .expect("audio sources should count");
        assert_eq!(stored_sources, 1);
    }

    #[tokio::test]
    async fn stores_whole_beatmap_sets_until_the_limit_is_reached() {
        let mut connection = connection().await;
        let sets = vec![
            beatmap_set(
                "audio.mp3",
                vec![beatmap("Easy", "audio.mp3"), beatmap("Hard", "audio.mp3")],
            ),
            beatmap_set("other.mp3", vec![beatmap("Insane", "other.mp3")]),
        ];

        let summary = insert_beatmap_sets(&mut connection, &sets, Some(1), None)
            .await
            .expect("beatmap sets should store");

        assert_eq!(
            summary,
            ImportSummary {
                beatmap_sets: 1,
                beatmaps: 2,
                audio_sources: 1,
                skipped_beatmap_sets: 1,
            }
        );

        let stored_beatmaps: i64 = beatmaps::table
            .count()
            .get_result(&mut connection)
            .await
            .expect("beatmaps should count");
        assert_eq!(stored_beatmaps, 2);
    }

    #[tokio::test]
    async fn loads_every_beatmap_set_with_the_audio_sources_its_beatmaps_reference() {
        let mut connection = connection().await;
        let sets = vec![
            beatmap_set(
                "audio.mp3",
                vec![beatmap("Easy", "audio.mp3"), beatmap("Hard", "audio.mp3")],
            ),
            beatmap_set("other.mp3", vec![beatmap("Insane", "other.mp3")]),
        ];
        insert_beatmap_sets(&mut connection, &sets, None, None)
            .await
            .expect("beatmap sets should store");

        let loaded = all_beatmap_sets_with_audio_sources(&mut connection)
            .await
            .expect("beatmap sets should load");

        assert_eq!(loaded.len(), 2);
        assert!(loaded[0].0.id < loaded[1].0.id);
        assert_eq!(loaded[0].0.hash.as_deref(), Some("set-hash"));
        assert_eq!(
            loaded[0].1,
            vec![AudioSource {
                id: 1,
                s_type: SourceType::Local("/osu/files/a/ab/abc".to_owned()),
            }]
        );
    }

    #[tokio::test]
    async fn loads_a_beatmap_set_whose_beatmaps_reference_no_audio() {
        let mut connection = connection().await;
        let sets = vec![beatmap_set(
            "audio.mp3",
            vec![beatmap("Easy", "missing.mp3")],
        )];
        insert_beatmap_sets(&mut connection, &sets, None, None)
            .await
            .expect("beatmap sets should store");

        let loaded = all_beatmap_sets_with_audio_sources(&mut connection)
            .await
            .expect("beatmap sets should load");

        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].1.is_empty());
    }

    #[tokio::test]
    async fn deleting_an_installation_removes_the_beatmap_sets_imported_from_it() {
        use crate::repositories::user_data::{delete_installation, register_installation};
        use radio_core::OsuMarker;

        let mut connection = connection().await;
        let installation = register_installation(
            &mut connection,
            &OsuMarker {
                kind: OsuKind::Lazer,
                marker_path: PathBuf::from("/osu/client.realm"),
                root_path: PathBuf::from("/osu"),
            },
            None,
        )
        .await
        .expect("installation should register")
        .into_installation();

        let sets = vec![beatmap_set("audio.mp3", vec![beatmap("Easy", "audio.mp3")])];
        insert_beatmap_sets(&mut connection, &sets, None, Some(installation.id))
            .await
            .expect("beatmap sets should store");

        assert!(
            delete_installation(&mut connection, installation.id)
                .await
                .expect("installation should delete")
        );

        let remaining_sets: i64 = beatmap_sets::table
            .count()
            .get_result(&mut connection)
            .await
            .expect("beatmap sets should count");
        let remaining_beatmaps: i64 = beatmaps::table
            .count()
            .get_result(&mut connection)
            .await
            .expect("beatmaps should count");

        assert_eq!(remaining_sets, 0);
        assert_eq!(remaining_beatmaps, 0);
    }

    #[tokio::test]
    async fn stores_metadata_that_has_no_resolvable_audio_file() {
        let mut connection = connection().await;
        let sets = vec![beatmap_set(
            "audio.mp3",
            vec![beatmap("Easy", "missing.mp3")],
        )];

        let summary = insert_beatmap_sets(&mut connection, &sets, None, None)
            .await
            .expect("beatmap sets should store");

        assert_eq!(summary.beatmaps, 1);
        assert_eq!(summary.audio_sources, 0);

        let linked: Option<i32> = beatmap_metadata::table
            .select(beatmap_metadata::audio_source_id)
            .first(&mut connection)
            .await
            .expect("metadata should load");
        assert_eq!(linked, None);
    }
}
