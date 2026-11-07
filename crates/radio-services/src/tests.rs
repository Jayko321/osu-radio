use crate::{FolderChanges, ImportSummary, Services, metadata_hash, model::SourceType};
use radio_core::{
    OsuKind, OsuMarker,
    import_types::{
        BeatmapMetadata as ImportedMetadata, BeatmapStatus, ImportedBeatmap, ImportedBeatmapSet,
        ImportedCollection, ImportedSnapshot, RealmFile, RealmNamedFileUsage, RealmUser,
    },
};
use sea_orm::ConnectionTrait;
#[cfg(feature = "sqlite")]
mod folder_selection;

struct TestDatabase {
    services: Services,
    connection: sea_orm::DatabaseConnection,
}
impl std::ops::Deref for TestDatabase {
    type Target = Services;
    fn deref(&self) -> &Services {
        &self.services
    }
}
impl TestDatabase {
    async fn connect(url: &str) -> Self {
        let services = Services::connect(url).await.unwrap();
        services.migrate().await.unwrap();
        #[cfg(feature = "sqlite")]
        let url = format!("sqlite://{url}?mode=rwc");
        let mut options = sea_orm::ConnectOptions::new(url);
        options.sqlx_logging(false);
        Self {
            services,
            connection: sea_orm::Database::connect(options).await.unwrap(),
        }
    }
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sqlite_service_contracts() {
    let directory = tempfile::tempdir().unwrap();
    contracts(directory.path().join("contracts.sqlite").to_str().unwrap()).await;
}

#[cfg(feature = "postgres")]
#[tokio::test]
#[ignore = "requires RADIO_DB_TEST_POSTGRES_URL pointing to a fresh disposable radio_db_test_* database"]
async fn postgres_service_contracts() {
    let url = std::env::var("RADIO_DB_TEST_POSTGRES_URL").unwrap();
    assert!(
        url.rsplit('/')
            .next()
            .unwrap()
            .split('?')
            .next()
            .unwrap()
            .starts_with("radio_db_test_")
    );
    contracts(&url).await;
}

async fn contracts(url: &str) {
    let database = TestDatabase::connect(url).await;
    let other_pool = TestDatabase::connect(url).await;
    registrations_and_updates(&database, &other_pool).await;
    snapshots_and_cleanup(&database, &other_pool).await;
    search_data_snapshot(&database).await;
    duration_snapshot(&database).await;
    tags::contracts(&database, &other_pool).await;
    search::contracts(&database, &other_pool).await;
    rollback_preserves_snapshot(&database).await;
    cancellation_preserves_snapshot(&database).await;
    concurrent_replacements_and_deletion(&database, &other_pool).await;
    shared_source_variants(&database).await;
    source_files_remain_untouched(&database).await;
    Box::pin(queue::contracts(&database, &other_pool, url)).await;
    queue_t03::contracts(&database, &other_pool).await;
    queue_t04::contracts(&database, &other_pool).await;
    playlists::contracts(&database, &other_pool, url).await;
    playlists::unicode_contracts(&database).await;
    playlists::cover_contracts(&database, &other_pool, url).await;
    collections::contracts(&database).await;
    listening_history::contracts(&database, &other_pool, url).await;
    audio_settings::contracts(&database, &other_pool, url).await;
    audio_mods::contracts(&database, &other_pool, url).await;
}

#[allow(clippy::too_many_lines)]
async fn duration_snapshot(database: &TestDatabase) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("audio");
    let wav = include_bytes!("../../../apps/osu-radio-server/tests/fixtures/tone.wav");
    let mut longer = Vec::new();
    longer.extend(b"RIFF");
    longer.extend(64036_u32.to_le_bytes());
    longer.extend(b"WAVEfmt ");
    longer.extend(16_u32.to_le_bytes());
    longer.extend(1_u16.to_le_bytes());
    longer.extend(1_u16.to_le_bytes());
    longer.extend(8000_u32.to_le_bytes());
    longer.extend(16000_u32.to_le_bytes());
    longer.extend(2_u16.to_le_bytes());
    longer.extend(16_u16.to_le_bytes());
    longer.extend(b"data");
    longer.extend(64000_u32.to_le_bytes());
    longer.extend(vec![0; 64000]);
    std::fs::write(&path, wav).unwrap();
    let source = path.to_str().unwrap();
    let id = register(database, "duration").await;
    let other = register(database, "duration-shared").await;
    let imported = snapshot("Duration", source);
    let prepared = crate::osu_installation::prepare_durations(&imported)
        .await
        .unwrap();
    assert_eq!(prepared.len(), 1);
    database
        .osu_installations()
        .replace_snapshot(id, &imported)
        .await
        .unwrap();
    database
        .osu_installations()
        .replace_snapshot(other, &imported)
        .await
        .unwrap();
    let audio = database
        .audio_sources()
        .find(&SourceType::Local(source.into()))
        .await
        .unwrap()
        .unwrap();
    let initial = audio.duration_ms.unwrap();
    assert!((200..=350).contains(&initial));
    let set = database
        .beatmap_sets()
        .for_installation(id)
        .await
        .unwrap()
        .remove(0);
    let maps = database.beatmaps().for_set(set.id).await.unwrap();
    assert!(maps.iter().all(|map| map.audio_source_id == Some(audio.id)));
    std::fs::write(&path, &longer).unwrap();
    database
        .osu_installations()
        .replace_snapshot(id, &imported)
        .await
        .unwrap();
    let current = database
        .audio_sources()
        .get(audio.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.duration_ms, Some(4000));
    assert_eq!(
        database
            .beatmap_sets()
            .search_tracks("Duration length>3s")
            .await
            .unwrap()
            .len(),
        1
    );
    assert_ne!(current.duration_ms, Some(initial));
    assert_eq!(
        current.duration_ms,
        crate::probe_source_duration(SourceType::Local(source.into()))
            .await
            .unwrap()
            .map(|v| i64::try_from(v).unwrap())
    );
    std::fs::write(&path, b"corrupt").unwrap();
    install_failure_trigger(database).await;
    let mut failed = imported.clone();
    failed[0].beatmaps[1].difficulty_name = Some("FAIL_IMPORT".into());
    let before = database.osu_installations().get(id).await.unwrap();
    assert!(
        database
            .osu_installations()
            .replace_snapshot(id, &failed)
            .await
            .is_err()
    );
    assert_eq!(
        database.audio_sources().get(audio.id).await.unwrap(),
        Some(current)
    );
    assert_eq!(database.osu_installations().get(id).await.unwrap(), before);
    assert_eq!(
        database
            .beatmap_sets()
            .for_installation(id)
            .await
            .unwrap()
            .len(),
        1
    );
    remove_failure_trigger(database).await;
    database
        .osu_installations()
        .replace_snapshot(id, &imported)
        .await
        .unwrap();
    assert_eq!(
        database
            .audio_sources()
            .get(audio.id)
            .await
            .unwrap()
            .unwrap()
            .duration_ms,
        None
    );
    assert!(
        database
            .beatmap_sets()
            .search_tracks("Duration length!=3s")
            .await
            .unwrap()
            .is_empty()
    );
    std::fs::remove_file(&path).unwrap();
    database
        .osu_installations()
        .replace_snapshot(id, &imported)
        .await
        .unwrap();
    assert_eq!(
        database
            .audio_sources()
            .get(audio.id)
            .await
            .unwrap()
            .unwrap()
            .duration_ms,
        None
    );
    database.osu_installations().delete(id).await.unwrap();
    database.osu_installations().delete(other).await.unwrap();
}

async fn search_data_snapshot(database: &TestDatabase) {
    let id = register(database, "search-data").await;
    let mut imported = snapshot("Search data", "/audio/search-data.mp3");
    imported[0].ranked_at_ms = Some(1_700_000_000_123);
    imported[0].submitted_at_ms = Some(-1234);
    imported[0].beatmaps[0].stars = Some(5.987);
    imported[0].beatmaps[0].status = Some(BeatmapStatus::Loved);
    database
        .osu_installations()
        .replace_snapshot(id, &imported)
        .await
        .unwrap();
    let set = database
        .beatmap_sets()
        .for_installation(id)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        (set.ranked_at_ms, set.submitted_at_ms),
        (Some(1_700_000_000_123), Some(-1234))
    );
    let listed = database
        .beatmap_sets()
        .all_with_audio_sources()
        .await
        .unwrap();
    assert_eq!(
        listed
            .iter()
            .find(|item| item.beatmap_set.id == set.id)
            .unwrap()
            .beatmap_set,
        set
    );
    let maps = database.beatmaps().for_set(set.id).await.unwrap();
    assert_eq!(
        (maps[0].stars, maps[0].status),
        (Some(5.987), Some(BeatmapStatus::Loved))
    );
    let audio_id = maps[0].audio_source_id.unwrap();
    database
        .osu_installations()
        .replace_snapshot(id, &snapshot("Search data", "/audio/search-data.mp3"))
        .await
        .unwrap();
    let set = database
        .beatmap_sets()
        .for_installation(id)
        .await
        .unwrap()
        .remove(0);
    assert_eq!((set.ranked_at_ms, set.submitted_at_ms), (None, None));
    let maps = database.beatmaps().for_set(set.id).await.unwrap();
    assert_eq!((maps[0].stars, maps[0].status), (None, None));
    assert_eq!(maps[0].audio_source_id, Some(audio_id));
    database.osu_installations().delete(id).await.unwrap();
}

fn snapshot_with_collection(title: &str, path: &str, collection_name: &str) -> ImportedSnapshot {
    let mut sets = snapshot(title, path);
    sets[0].ranked_at_ms = Some(111);
    sets[0].submitted_at_ms = Some(222);
    sets[0].beatmaps[0].stars = Some(4.25);
    sets[0].beatmaps[0].status = Some(BeatmapStatus::Ranked);
    sets[0].beatmaps[0].md5_hash = Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into());
    ImportedSnapshot {
        beatmap_sets: sets,
        collections: vec![ImportedCollection {
            source_id: "collection-id".into(),
            name: collection_name.into(),
            beatmap_md5_hashes: vec!["aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()],
        }],
    }
}

async fn assert_counts(database: &TestDatabase, expected: [i64; 4]) {
    for (table, expected) in [
        "beatmap_sets",
        "beatmaps",
        "beatmap_metadata",
        "audio_sources",
    ]
    .into_iter()
    .zip(expected)
    {
        let row = database
            .connection
            .query_one_raw(sea_orm::Statement::from_string(
                database.connection.get_database_backend(),
                format!("SELECT COUNT(*) AS count FROM {table}"),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.try_get::<i64>("", "count").unwrap(),
            expected,
            "{table}"
        );
    }
}

async fn sql(database: &TestDatabase, statement: &str) {
    database
        .connection
        .execute_unprepared(statement)
        .await
        .unwrap();
}

fn marker(name: &str) -> OsuMarker {
    OsuMarker {
        kind: OsuKind::Lazer,
        root_path: format!("/test/{name}").into(),
        marker_path: format!("/test/{name}/client.realm").into(),
    }
}

async fn register(database: &TestDatabase, name: &str) -> i32 {
    let registered = database
        .osu_installations()
        .register(&marker(name), None)
        .await
        .unwrap();
    assert!(registered.was_created());
    assert!(registered.installation().id > 0);
    assert_eq!(registered.installation().user_data_id, 1);
    registered.into_installation().id
}

fn metadata(title: &str) -> ImportedMetadata {
    ImportedMetadata {
        title: Some(title.to_owned()),
        title_unicode: Some("曲".to_owned()),
        artist: Some("Artist".to_owned()),
        artist_unicode: None,
        author: Some(RealmUser {
            online_id: Some(7),
            username: Some("Mapper".to_owned()),
            country_code: Some("JP".to_owned()),
        }),
        source: Some(String::new()),
        tags: None,
        user_tags: vec![
            "calm".to_owned(),
            "piano\nonly".to_owned(),
            title.to_owned(),
        ],
        preview_time: Some(1200),
        audio_file: Some("audio.mp3".to_owned()),
        background_file: Some("bg.jpg".to_owned()),
    }
}

fn snapshot(title: &str, path: &str) -> Vec<ImportedBeatmapSet> {
    vec![ImportedBeatmapSet {
        ranked_at_ms: None,
        submitted_at_ms: None,
        source: OsuKind::Lazer,
        online_id: Some(23),
        hash: Some(format!("set-{title}")),
        files: vec![
            RealmNamedFileUsage {
                filename: Some("audio.mp3".to_owned()),
                file: Some(RealmFile {
                    hash: Some("file-hash".to_owned()),
                    resolved_path: Some(path.into()),
                }),
            },
            RealmNamedFileUsage {
                filename: Some("bg.jpg".to_owned()),
                file: Some(RealmFile {
                    hash: None,
                    resolved_path: Some(format!("{path}.jpg").into()),
                }),
            },
        ],
        beatmaps: ["Easy", "Hard"]
            .into_iter()
            .map(|difficulty| ImportedBeatmap {
                stars: None,
                status: None,
                osu_last_played_at_ms: None,
                md5_hash: None,
                difficulty_name: Some(difficulty.to_owned()),
                bpm: Some(180.5),
                hash: Some(format!("beatmap-{difficulty}")),
                metadata: Some(metadata(title)),
            })
            .collect(),
    }]
}

#[allow(clippy::too_many_lines)]
async fn registrations_and_updates(database: &TestDatabase, other_pool: &TestDatabase) {
    let marker = marker("registration");
    let first_repo = database.osu_installations();
    let second_repo = other_pool.osu_installations();
    let (first, second) = tokio::join!(
        first_repo.register(&marker, Some("Original")),
        second_repo.register(&marker, Some("Original"))
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_ne!(first.was_created(), second.was_created());
    assert_eq!(first.installation(), second.installation());
    let original = first.into_installation();
    assert_eq!(
        database.osu_installations().all().await.unwrap(),
        vec![original.clone()]
    );
    let duplicate = database
        .osu_installations()
        .register(&marker, Some("Ignored"))
        .await
        .unwrap();
    assert!(!duplicate.was_created());
    assert_eq!(duplicate.installation().label.as_deref(), Some("Original"));
    assert_eq!(
        database
            .osu_installations()
            .update(original.id, FolderChanges::default())
            .await
            .unwrap(),
        Some(original.clone())
    );
    let updated = database
        .osu_installations()
        .update(
            original.id,
            FolderChanges {
                label: None,
                enabled: Some(false),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert!(!updated.enabled);
    assert_eq!(updated.label, original.label);
    let updated = database
        .osu_installations()
        .update(
            original.id,
            FolderChanges {
                label: Some(None),
                enabled: None,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.label, None);
    assert!(!updated.enabled);
    let updated = database
        .osu_installations()
        .update(
            original.id,
            FolderChanges {
                label: Some(Some(String::new())),
                enabled: Some(true),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.label.as_deref(), Some(""));
    assert!(updated.enabled);
    assert_eq!(
        database.osu_installations().get(original.id).await.unwrap(),
        Some(updated)
    );
    assert!(
        database
            .osu_installations()
            .get(i32::MAX)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        database
            .osu_installations()
            .update(i32::MAX, FolderChanges::default())
            .await
            .unwrap()
            .is_none()
    );
    assert!(!database.osu_installations().delete(i32::MAX).await.unwrap());
    assert!(
        database
            .osu_installations()
            .delete(original.id)
            .await
            .unwrap()
    );
}

#[allow(clippy::too_many_lines)]
async fn snapshots_and_cleanup(database: &TestDatabase, other_pool: &TestDatabase) {
    let first = register(database, "first").await;
    let second = register(database, "second").await;
    assert_ne!(first, second);
    let imported = snapshot("Shared", "/audio/first.mp3");
    let summary = database
        .osu_installations()
        .replace_snapshot(first, &imported)
        .await
        .unwrap();
    assert_eq!(
        summary,
        ImportSummary {
            beatmap_sets: 1,
            beatmaps: 2,
            audio_sources: 1
        }
    );
    assert!(
        database
            .osu_installations()
            .get(first)
            .await
            .unwrap()
            .unwrap()
            .last_scanned_at
            .is_some()
    );
    let first_sets = database
        .beatmap_sets()
        .for_installation(first)
        .await
        .unwrap();
    assert_eq!(first_sets.len(), 1);
    assert!(first_sets[0].id > 0);
    assert_eq!(
        database.beatmap_sets().get(first_sets[0].id).await.unwrap(),
        Some(first_sets[0].clone())
    );
    let first_maps = database.beatmaps().for_set(first_sets[0].id).await.unwrap();
    assert_eq!(first_maps.len(), 2);
    assert!(first_maps[0].id > 0);
    assert_ne!(first_maps[0].id, first_maps[1].id);
    assert_eq!(
        database.beatmaps().get(first_maps[0].id).await.unwrap(),
        Some(first_maps[0].clone())
    );
    assert_eq!(first_maps[0].bpm, Some(180.5));
    let shared_hash = metadata_hash(&metadata("Shared")).unwrap();
    assert_eq!(
        first_maps[0].metadata_hash.as_deref(),
        Some(shared_hash.as_str())
    );
    let shared_metadata = database
        .beatmap_metadata()
        .get(&shared_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        database
            .tags()
            .for_set(first_sets[0].id)
            .await
            .unwrap()
            .iter()
            .map(|tag| tag.name.as_str())
            .collect::<Vec<_>>(),
        ["calm", "piano\nonly", "shared"]
    );
    assert_eq!(
        shared_metadata.author,
        Some(serde_json::json!({"online_id":7,"username":"Mapper","country_code":"JP"}))
    );
    let first_audio = first_maps[0].audio_source_id.unwrap();
    let second_snapshot = snapshot("Shared", "/audio/second.mp3");
    other_pool
        .osu_installations()
        .replace_snapshot(second, &second_snapshot)
        .await
        .unwrap();
    assert_counts(database, [2, 4, 1, 2]).await;
    let second_sets = database
        .beatmap_sets()
        .for_installation(second)
        .await
        .unwrap();
    let second_maps = database
        .beatmaps()
        .for_set(second_sets[0].id)
        .await
        .unwrap();
    assert_eq!(second_maps[0].metadata_hash, first_maps[0].metadata_hash);
    let second_audio = second_maps[0].audio_source_id.unwrap();
    assert_ne!(first_audio, second_audio);
    assert_eq!(
        database
            .beatmap_metadata()
            .get_or_insert(&metadata("Shared"))
            .await
            .unwrap(),
        shared_metadata
    );

    let grouped = database
        .beatmap_sets()
        .all_with_audio_sources()
        .await
        .unwrap();
    assert_eq!(grouped.len(), 2);
    assert!(grouped.iter().all(|set| set.audio_sources.len() == 1));

    let replacement = snapshot("Replacement", "/audio/replacement.mp3");
    database
        .osu_installations()
        .replace_snapshot(first, &replacement)
        .await
        .unwrap();
    assert_counts(database, [2, 4, 2, 2]).await;
    assert!(
        database
            .audio_sources()
            .get(first_audio)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        database
            .beatmap_sets()
            .for_installation(second)
            .await
            .unwrap(),
        second_sets
    );
    assert_eq!(
        database
            .beatmaps()
            .for_set(second_sets[0].id)
            .await
            .unwrap(),
        second_maps
    );
    assert_eq!(
        database.beatmap_metadata().get(&shared_hash).await.unwrap(),
        Some(shared_metadata)
    );
    let cleared = database
        .osu_installations()
        .replace_snapshot(first, &[])
        .await
        .unwrap();
    assert_eq!(cleared, ImportSummary::default());
    assert_counts(database, [1, 2, 1, 1]).await;
    assert!(
        database
            .beatmap_sets()
            .for_installation(first)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(database.osu_installations().delete(second).await.unwrap());
    assert_counts(database, [0, 0, 0, 0]).await;
    assert!(
        database
            .audio_sources()
            .get(second_audio)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        database
            .beatmaps()
            .get(second_maps[0].id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(database.osu_installations().delete(first).await.unwrap());

    assert!(
        database
            .osu_installations()
            .replace_snapshot(i32::MAX, &imported)
            .await
            .is_err()
    );
    let stable = database
        .osu_installations()
        .register(
            &OsuMarker {
                kind: OsuKind::Stable,
                ..marker("stable")
            },
            None,
        )
        .await
        .unwrap()
        .into_installation();
    assert!(
        database
            .osu_installations()
            .replace_snapshot(stable.id, &imported)
            .await
            .is_err()
    );
    assert_eq!(
        database
            .osu_installations()
            .get(stable.id)
            .await
            .unwrap()
            .unwrap()
            .last_scanned_at,
        None
    );
    assert_counts(database, [0, 0, 0, 0]).await;
    database
        .osu_installations()
        .delete(stable.id)
        .await
        .unwrap();

    let no_audio = register(database, "no-audio").await;
    let mut unresolved = snapshot("Unresolved", "/unused.mp3");
    unresolved[0].files.clear();
    unresolved[0].beatmaps[1].metadata = None;
    let summary = database
        .osu_installations()
        .replace_snapshot(no_audio, &unresolved)
        .await
        .unwrap();
    assert_eq!(summary.audio_sources, 0);
    let grouped = database
        .beatmap_sets()
        .all_with_audio_sources()
        .await
        .unwrap();
    assert!(grouped[0].audio_sources.is_empty());
    let maps = database
        .beatmaps()
        .for_set(grouped[0].beatmap_set.id)
        .await
        .unwrap();
    assert!(maps.iter().all(|map| map.audio_source_id.is_none()));
    assert_eq!(
        maps.iter()
            .filter(|map| map.metadata_hash.is_some())
            .count(),
        1
    );
    database.osu_installations().delete(no_audio).await.unwrap();
}

#[allow(clippy::too_many_lines)] // Check the complete rollback contract together.
async fn rollback_preserves_snapshot(database: &TestDatabase) {
    let installation = register(database, "rollback").await;
    database
        .osu_installations()
        .replace_imported_snapshot(
            installation,
            &snapshot_with_collection("Old", "/audio/old.mp3", "Old collection"),
        )
        .await
        .unwrap();
    let old_installation = database
        .osu_installations()
        .get(installation)
        .await
        .unwrap();
    let old_sets = database
        .beatmap_sets()
        .for_installation(installation)
        .await
        .unwrap();
    let old_maps = database.beatmaps().for_set(old_sets[0].id).await.unwrap();
    let old_tags = database.tags().all().await.unwrap();
    let old_links = database.tags().for_set(old_sets[0].id).await.unwrap();
    let old_metadata = database
        .beatmap_metadata()
        .get(old_maps[0].metadata_hash.as_ref().unwrap())
        .await
        .unwrap();
    let old_audio = database
        .audio_sources()
        .get(old_maps[0].audio_source_id.unwrap())
        .await
        .unwrap();
    let playlist_id = database
        .playlists()
        .all()
        .await
        .unwrap()
        .into_iter()
        .find(|p| p.name == "Old collection")
        .unwrap()
        .id;
    let old_playlist = database.playlists().get(playlist_id).await.unwrap();
    assert_eq!(
        old_maps[0].background_path.as_deref(),
        Some("/audio/old.mp3.jpg")
    );
    install_failure_trigger(database).await;
    let mut replacement =
        snapshot_with_collection("Failed", "/audio/failed.mp3", "Failed collection");
    replacement.beatmap_sets[0].ranked_at_ms = None;
    replacement.beatmap_sets[0].submitted_at_ms = Some(999);
    replacement.beatmap_sets[0].beatmaps[0].stars = None;
    replacement.beatmap_sets[0].beatmaps[0].status = Some(BeatmapStatus::Loved);
    replacement.beatmap_sets[0].beatmaps[1].difficulty_name = Some("FAIL_IMPORT".to_owned());
    let error = database
        .osu_installations()
        .replace_imported_snapshot(installation, &replacement)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("forced import failure"));
    assert_eq!(
        database
            .osu_installations()
            .get(installation)
            .await
            .unwrap(),
        old_installation
    );
    assert_eq!(
        database
            .beatmap_sets()
            .for_installation(installation)
            .await
            .unwrap(),
        old_sets
    );
    assert_eq!(
        database.beatmaps().for_set(old_sets[0].id).await.unwrap(),
        old_maps
    );
    assert_eq!(
        database
            .beatmap_metadata()
            .get(old_maps[0].metadata_hash.as_ref().unwrap())
            .await
            .unwrap(),
        old_metadata
    );
    assert_eq!(
        database
            .audio_sources()
            .get(old_maps[0].audio_source_id.unwrap())
            .await
            .unwrap(),
        old_audio
    );
    assert!(
        database
            .beatmap_metadata()
            .get(&metadata_hash(&metadata("Failed")).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        database
            .audio_sources()
            .find(&SourceType::Local("/audio/failed.mp3".to_owned()))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(database.tags().all().await.unwrap(), old_tags);
    assert_eq!(
        database.playlists().get(playlist_id).await.unwrap(),
        old_playlist
    );
    assert_eq!(
        database.tags().for_set(old_sets[0].id).await.unwrap(),
        old_links
    );
    assert_counts(database, [1, 2, 1, 1]).await;
    remove_failure_trigger(database).await;
    database.playlists().delete(playlist_id).await.unwrap();
    database
        .osu_installations()
        .delete(installation)
        .await
        .unwrap();
}

#[cfg(feature = "sqlite")]
async fn install_failure_trigger(database: &TestDatabase) {
    sql(database, "CREATE TRIGGER fail_import BEFORE INSERT ON beatmaps WHEN NEW.difficulty_name = 'FAIL_IMPORT' BEGIN SELECT RAISE(ABORT, 'forced import failure'); END").await;
}

#[cfg(feature = "postgres")]
async fn install_failure_trigger(database: &TestDatabase) {
    sql(database, "CREATE FUNCTION fail_import() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.difficulty_name = 'FAIL_IMPORT' THEN RAISE EXCEPTION 'forced import failure'; END IF; RETURN NEW; END $$").await;
    sql(database, "CREATE TRIGGER fail_import BEFORE INSERT ON beatmaps FOR EACH ROW EXECUTE FUNCTION fail_import()").await;
}

async fn remove_failure_trigger(database: &TestDatabase) {
    #[cfg(feature = "sqlite")]
    sql(database, "DROP TRIGGER fail_import").await;
    #[cfg(feature = "postgres")]
    {
        sql(database, "DROP TRIGGER fail_import ON beatmaps").await;
        sql(database, "DROP FUNCTION fail_import()").await;
    }
}

async fn concurrent_replacements_and_deletion(database: &TestDatabase, other_pool: &TestDatabase) {
    let first = register(database, "concurrent-first").await;
    let second = register(database, "concurrent-second").await;
    let shared = snapshot("Concurrent", "/audio/shared.mp3");
    let first_repo = database.osu_installations();
    let second_repo = other_pool.osu_installations();
    let (a, b) = tokio::join!(
        first_repo.replace_snapshot(first, &shared),
        second_repo.replace_snapshot(second, &shared)
    );
    a.unwrap();
    b.unwrap();
    assert_counts(database, [2, 4, 1, 1]).await;
    let replacement = snapshot("Different", "/audio/different.mp3");
    let (a, b) = tokio::join!(
        first_repo.replace_snapshot(first, &shared),
        second_repo.replace_snapshot(first, &replacement)
    );
    a.unwrap();
    b.unwrap();
    let sets = database
        .beatmap_sets()
        .for_installation(first)
        .await
        .unwrap();
    assert_eq!(sets.len(), 1);
    let maps = database.beatmaps().for_set(sets[0].id).await.unwrap();
    assert_eq!(maps.len(), 2);
    assert_eq!(maps[0].metadata_hash, maps[1].metadata_hash);
    assert_eq!(maps[0].audio_source_id, maps[1].audio_source_id);
    assert!(matches!(
        sets[0].hash.as_deref(),
        Some("set-Concurrent" | "set-Different")
    ));
    // Shared cleanup cannot race another pool's insertion of the same shared rows.
    let (a, b) = tokio::join!(
        first_repo.delete(first),
        second_repo.replace_snapshot(second, &shared)
    );
    assert!(a.unwrap());
    b.unwrap();
    assert_counts(database, [1, 2, 1, 1]).await;
    first_repo.delete(second).await.unwrap();
    assert_counts(database, [0, 0, 0, 0]).await;
}

async fn shared_source_variants(database: &TestDatabase) {
    let mut ids = Vec::new();
    for source in [
        SourceType::Local("same".to_owned()),
        SourceType::Copied("same".to_owned()),
        SourceType::Online("same".to_owned()),
    ] {
        let stored = database
            .audio_sources()
            .get_or_insert(&source)
            .await
            .unwrap();
        assert_eq!(stored.s_type, source);
        assert!(stored.id > 0);
        assert!(!ids.contains(&stored.id));
        ids.push(stored.id);
        assert_eq!(
            database.audio_sources().find(&source).await.unwrap(),
            Some(stored.clone())
        );
        assert_eq!(
            database
                .audio_sources()
                .get_or_insert(&source)
                .await
                .unwrap(),
            stored
        );
    }
    // Cleanup only touches records, never source files; these locations need not exist.
    let installation = register(database, "cleanup-unreferenced").await;
    database
        .osu_installations()
        .replace_snapshot(installation, &[])
        .await
        .unwrap();
    assert_counts(database, [0, 0, 0, 0]).await;
    database
        .osu_installations()
        .delete(installation)
        .await
        .unwrap();
}

async fn source_files_remain_untouched(database: &TestDatabase) {
    let directory = tempfile::tempdir().unwrap();
    let audio = directory.path().join("audio.mp3");
    std::fs::write(&audio, b"source bytes").unwrap();
    let installation = register(database, "preserve-files").await;
    database
        .osu_installations()
        .replace_snapshot(installation, &snapshot("File", audio.to_str().unwrap()))
        .await
        .unwrap();
    database
        .osu_installations()
        .delete(installation)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&audio).unwrap(), b"source bytes");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    assert_counts(database, [0, 0, 0, 0]).await;
}

// Suspend after the complete service workflow, before commit, then cancel its owner.
// This checks rollback of sets, shared-row cleanup and timestamp together without timing sleeps.
#[allow(clippy::too_many_lines)] // Check the whole cancelled snapshot and collection together.
async fn cancellation_preserves_snapshot(database: &TestDatabase) {
    let id = register(database, "cancelled").await;
    database
        .osu_installations()
        .replace_imported_snapshot(
            id,
            &snapshot_with_collection(
                "Before cancel",
                "/audio/before.mp3",
                "Before cancel collection",
            ),
        )
        .await
        .unwrap();
    let old_installation = database.osu_installations().get(id).await.unwrap();
    let old_sets = database.beatmap_sets().for_installation(id).await.unwrap();
    let old_maps = database.beatmaps().for_set(old_sets[0].id).await.unwrap();
    let old_tags = database.tags().all().await.unwrap();
    let old_links = database.tags().for_set(old_sets[0].id).await.unwrap();
    let old_metadata = database
        .beatmap_metadata()
        .get(old_maps[0].metadata_hash.as_ref().unwrap())
        .await
        .unwrap();
    let old_audio = database
        .audio_sources()
        .get(old_maps[0].audio_source_id.unwrap())
        .await
        .unwrap();
    let playlist_id = database
        .playlists()
        .all()
        .await
        .unwrap()
        .into_iter()
        .find(|p| p.name == "Before cancel collection")
        .unwrap()
        .id;
    let old_playlist = database.playlists().get(playlist_id).await.unwrap();
    let services = database.services.clone();
    let (ready, wait) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut replacement =
            snapshot_with_collection("Cancelled", "/audio/cancelled.mp3", "Cancelled collection");
        replacement.beatmap_sets[0].ranked_at_ms = None;
        replacement.beatmap_sets[0].submitted_at_ms = Some(999);
        replacement.beatmap_sets[0].beatmaps[0].stars = None;
        replacement.beatmap_sets[0].beatmaps[0].status = Some(BeatmapStatus::Loved);
        let durations = crate::osu_installation::prepare_durations(&replacement.beatmap_sets)
            .await
            .unwrap();
        let transaction = services.database.begin().await.unwrap();
        crate::OsuInstallationService::replace_imported_snapshot_in(
            &transaction,
            id,
            &replacement,
            &durations,
        )
        .await
        .unwrap();
        ready.send(()).unwrap();
        std::future::pending::<()>().await;
        transaction.commit().await.unwrap();
    });
    wait.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(
        database.osu_installations().get(id).await.unwrap(),
        old_installation
    );
    assert_eq!(
        database.beatmap_sets().for_installation(id).await.unwrap(),
        old_sets
    );
    assert_eq!(
        database.beatmaps().for_set(old_sets[0].id).await.unwrap(),
        old_maps
    );
    assert_eq!(
        database
            .beatmap_metadata()
            .get(old_maps[0].metadata_hash.as_ref().unwrap())
            .await
            .unwrap(),
        old_metadata
    );
    assert_eq!(
        database
            .audio_sources()
            .get(old_maps[0].audio_source_id.unwrap())
            .await
            .unwrap(),
        old_audio
    );
    assert!(
        database
            .beatmap_metadata()
            .get(&metadata_hash(&metadata("Cancelled")).unwrap())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        database
            .audio_sources()
            .find(&SourceType::Local("/audio/cancelled.mp3".into()))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(database.tags().all().await.unwrap(), old_tags);
    assert_eq!(
        database.playlists().get(playlist_id).await.unwrap(),
        old_playlist
    );
    assert_eq!(
        database.tags().for_set(old_sets[0].id).await.unwrap(),
        old_links
    );
    assert_counts(database, [1, 2, 1, 1]).await;
    database.playlists().delete(playlist_id).await.unwrap();
    database.osu_installations().delete(id).await.unwrap();
}

#[path = "tests/tags.rs"]
mod tags;

mod search;

mod listening_history;
mod playlists;
mod queue;
mod queue_modes;
mod queue_repeat;
mod queue_t02;
mod queue_t03;
mod queue_t04;

mod audio_mods;
mod audio_settings;

mod collections;
