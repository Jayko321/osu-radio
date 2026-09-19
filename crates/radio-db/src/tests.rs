use radio_core::{
    OsuKind, OsuMarker,
    import_types::{
        BeatmapMetadata as ImportedMetadata, ImportedBeatmap, ImportedBeatmapSet, RealmFile,
        RealmNamedFileUsage, RealmUser,
    },
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, sea_query::Expr,
};

use crate::{
    Database,
    entities::{audio_source, beatmap, beatmap_metadata, beatmap_set},
    model::SourceType,
};

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sqlite_repository_contracts() {
    let directory = tempfile::tempdir().unwrap();
    let url = directory.path().join("contracts.sqlite");
    repository_contracts(url.to_str().unwrap()).await;

    let memory = Database::connect(":memory:").await.unwrap();
    memory.migrate().await.unwrap();
    assert_eq!(memory.user_data().get().await.unwrap().id, 1);
    memory.clone().check_connection().await.unwrap();
}

#[cfg(feature = "postgres")]
#[tokio::test]
#[ignore = "requires RADIO_DB_TEST_POSTGRES_URL pointing to a disposable radio_db_test_* database"]
async fn postgres_repository_contracts() {
    let url = std::env::var("RADIO_DB_TEST_POSTGRES_URL")
        .expect("set RADIO_DB_TEST_POSTGRES_URL to a disposable local test database");
    let database_name = url.rsplit('/').next().unwrap().split('?').next().unwrap();
    assert!(
        database_name.starts_with("radio_db_test_"),
        "test database name must start with radio_db_test_; never use the application database"
    );
    repository_contracts(&url).await;
}

async fn repository_contracts(url: &str) {
    let database = Database::connect(url).await.unwrap();
    database.check_connection().await.unwrap();
    concurrent_schema_changes(&database, url).await;
    native_path_migration(&database).await;
    tag_migration::contracts(&database, url).await;
    // A fresh database is expected: no configured application database is consulted.
    sql(
        &database,
        "CREATE TABLE unrelated_table (id INTEGER PRIMARY KEY)",
    )
    .await;
    sql(&database, "INSERT INTO unrelated_table VALUES (42)").await;
    sql(
        &database,
        "CREATE TABLE beatmap_sets (id INTEGER PRIMARY KEY)",
    )
    .await;
    let legacy_error = database.migrate().await.unwrap_err().to_string();
    assert!(
        legacy_error.to_lowercase().contains("reset"),
        "legacy error must explain explicit reset: {legacy_error}"
    );
    database.reset().await.unwrap();
    database.migrate().await.unwrap();
    assert_unrelated_table(&database).await;
    assert_eq!(database.user_data().get().await.unwrap().id, 1);
    assert_eq!(database.user_data().get().await.unwrap().id, 1);

    let other_pool = Database::connect(url).await.unwrap();
    other_pool.migrate().await.unwrap();
    native_path_registration(&database, &other_pool).await;
    aggregate_grouping_and_cleanup(&database).await;
    repository_transaction_contracts(&database).await;
    tag_repository_contracts(&database).await;
    metadata_corruption_is_rejected(&database).await;

    database.reset().await.unwrap();
    assert!(database.osu_installations().all().await.unwrap().is_empty());
    assert_counts(&database, [0, 0, 0, 0]).await;
    assert_unrelated_table(&database).await;
    assert_eq!(database.user_data().get().await.unwrap().id, 1);
}

async fn sql(database: &Database, statement: &str) {
    database
        .connection
        .execute_unprepared(statement)
        .await
        .unwrap();
}

async fn assert_unrelated_table(database: &Database) {
    let rows = database
        .connection
        .query_all_raw(sea_orm::Statement::from_string(
            database.connection.get_database_backend(),
            "SELECT id FROM unrelated_table",
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].try_get::<i32>("", "id").unwrap(), 42);
}

fn marker(name: &str) -> OsuMarker {
    OsuMarker {
        kind: OsuKind::Lazer,
        root_path: format!("/test/{name}").into(),
        marker_path: format!("/test/{name}/client.realm").into(),
    }
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
        user_tags: vec!["calm".to_owned(), "piano\nonly".to_owned()],
        preview_time: Some(1200),
        audio_file: Some("audio.mp3".to_owned()),
        background_file: Some("bg.jpg".to_owned()),
    }
}

fn snapshot(title: &str, path: &str) -> Vec<ImportedBeatmapSet> {
    vec![ImportedBeatmapSet {
        source: OsuKind::Lazer,
        online_id: Some(23),
        hash: Some(format!("set-{title}")),
        files: vec![RealmNamedFileUsage {
            filename: Some("audio.mp3".to_owned()),
            file: Some(RealmFile {
                hash: Some("file-hash".to_owned()),
                resolved_path: Some(path.into()),
            }),
        }],
        beatmaps: ["Easy", "Hard"]
            .into_iter()
            .map(|difficulty| ImportedBeatmap {
                difficulty_name: Some(difficulty.to_owned()),
                bpm: Some(180.5),
                hash: Some(format!("beatmap-{difficulty}")),
                metadata: Some(metadata(title)),
            })
            .collect(),
    }]
}

async fn assert_counts(database: &Database, expected: [u64; 4]) {
    assert_eq!(
        beatmap_set::Entity::find()
            .count(&database.connection)
            .await
            .unwrap(),
        expected[0]
    );
    assert_eq!(
        beatmap::Entity::find()
            .count(&database.connection)
            .await
            .unwrap(),
        expected[1]
    );
    assert_eq!(
        beatmap_metadata::Entity::find()
            .count(&database.connection)
            .await
            .unwrap(),
        expected[2]
    );
    assert_eq!(
        audio_source::Entity::find()
            .count(&database.connection)
            .await
            .unwrap(),
        expected[3]
    );
}

async fn metadata_corruption_is_rejected(database: &Database) {
    let imported = metadata("Immutable");
    let stored = database
        .beatmap_metadata()
        .get_or_insert(&imported)
        .await
        .unwrap();
    beatmap_metadata::Entity::update_many()
        .col_expr(beatmap_metadata::Column::Title, Expr::value("Corrupted"))
        .filter(beatmap_metadata::Column::Hash.eq(&stored.hash))
        .exec(&database.connection)
        .await
        .unwrap();
    let error = database
        .beatmap_metadata()
        .get_or_insert(&imported)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("hash"));
    assert_eq!(
        database
            .beatmap_metadata()
            .get(&stored.hash)
            .await
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("Corrupted")
    );
    database.beatmap_metadata().cleanup().await.unwrap();
    assert_counts(database, [0, 0, 0, 0]).await;
}

async fn repository_transaction_contracts(database: &Database) {
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    let installation = transaction
        .osu_installations()
        .register(&marker("transaction"), None)
        .await
        .unwrap()
        .into_installation();
    let imported = snapshot("Constraints", "/test/audio.mp3");
    let set = transaction
        .beatmap_sets()
        .insert(installation.id, &imported[0])
        .await
        .unwrap();
    let metadata = transaction
        .beatmap_metadata()
        .get_or_insert(&metadata("Constraints"))
        .await
        .unwrap();
    let audio = transaction
        .audio_sources()
        .get_or_insert(&SourceType::Local("/test/audio.mp3".into()))
        .await
        .unwrap();
    transaction
        .beatmaps()
        .insert(
            set.id,
            &imported[0].beatmaps[0],
            Some(metadata.hash.clone()),
            Some(audio.id),
            None,
        )
        .await
        .unwrap();
    transaction.commit().await.unwrap();
    assert!(
        beatmap_metadata::Entity::delete_by_id(metadata.hash)
            .exec(&database.connection)
            .await
            .is_err()
    );
    assert!(
        audio_source::Entity::delete_by_id(audio.id)
            .exec(&database.connection)
            .await
            .is_err()
    );
    let transaction = database.begin().await.unwrap();
    transaction
        .osu_installations()
        .delete(installation.id)
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    assert!(
        database
            .osu_installations()
            .get(installation.id)
            .await
            .unwrap()
            .is_some()
    );
    let transaction = database.begin().await.unwrap();
    transaction
        .osu_installations()
        .delete(installation.id)
        .await
        .unwrap();
    drop(transaction);
    assert!(
        database
            .osu_installations()
            .get(installation.id)
            .await
            .unwrap()
            .is_some()
    );
    database
        .osu_installations()
        .delete(installation.id)
        .await
        .unwrap();
    database.beatmap_metadata().cleanup().await.unwrap();
    database.audio_sources().cleanup().await.unwrap();
    assert_counts(database, [0, 0, 0, 0]).await;
}

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn background_migration_preserves_existing_rows() {
    use sea_orm_migration::MigratorTrait;
    let database = Database::connect(":memory:").await.unwrap();
    crate::migrations::Migrator::up(&database.connection, Some(1))
        .await
        .unwrap();
    sql(&database, "INSERT INTO osu_installations (kind, root_path, marker_path, enabled, user_data_id) VALUES ('lazer', '/osu', '/osu/client.realm', 1, 1)").await;
    sql(
        &database,
        "INSERT INTO beatmap_sets (installation_id) VALUES (1)",
    )
    .await;
    sql(
        &database,
        "INSERT INTO beatmaps (beatmap_set_id, difficulty_name) VALUES (1, 'Easy')",
    )
    .await;
    database.migrate().await.unwrap();
    let map = database.beatmaps().get(1).await.unwrap().unwrap();
    assert_eq!(map.difficulty_name.as_deref(), Some("Easy"));
    assert_eq!(map.background_path, None);
    database.migrate().await.unwrap();
    assert_eq!(database.beatmaps().get(1).await.unwrap(), Some(map));
}

async fn drop_application_tables(database: &Database) {
    for table in [
        "beatmap_set_tags",
        "tags",
        "beatmaps",
        "beatmap_sets",
        "beatmap_metadata",
        "audio_sources",
        "osu_installations",
        "user_data",
        "seaql_migrations",
    ] {
        sql(database, &format!("DROP TABLE IF EXISTS {table}")).await;
    }
}

async fn concurrent_schema_changes(database: &Database, url: &str) {
    use sea_orm_migration::MigratorTrait;
    let other = Database::connect(url).await.unwrap();
    for trial in 0..20 {
        // Exercise both empty databases and pending upgrades through independent pools.
        if trial % 2 == 1 {
            crate::migrations::Migrator::up(&database.connection, Some(1))
                .await
                .unwrap();
        }
        let (first, second) = tokio::join!(database.migrate(), other.migrate());
        first.unwrap();
        second.unwrap();
        assert_eq!(database.user_data().get().await.unwrap().id, 1);
        assert_eq!(
            crate::migrations::Migrator::get_applied_migrations(&database.connection)
                .await
                .unwrap()
                .len(),
            4
        );
        let (reset, migrate) = tokio::join!(database.reset(), other.migrate());
        reset.unwrap();
        migrate.unwrap();
        drop_application_tables(database).await;
    }
}

async fn native_path_migration(database: &Database) {
    use crate::entities::osu_installation;
    use sea_orm::{Set, sea_query::OnConflict};
    use sea_orm_migration::MigratorTrait;
    crate::migrations::Migrator::up(&database.connection, Some(2))
        .await
        .unwrap();
    // Include a literal JSON-shaped filename and a transient unique-key collision.
    let paths = [
        "/osu/曲",
        "\"/osu/曲\"",
        "{\"Unix\":[255]}",
        "C:\\osu\\client.realm",
    ];
    for path in paths {
        osu_installation::Entity::insert(osu_installation::ActiveModel {
            user_data_id: Set(1),
            kind: Set("lazer".into()),
            root_path: Set(path.into()),
            marker_path: Set(path.into()),
            enabled: Set(true),
            ..Default::default()
        })
        .on_conflict(OnConflict::new().do_nothing().to_owned())
        .exec(&database.connection)
        .await
        .unwrap();
    }
    database.migrate().await.unwrap();
    for (stored, original) in database
        .osu_installations()
        .all()
        .await
        .unwrap()
        .iter()
        .zip(paths)
    {
        assert_eq!(stored.root_path, std::path::Path::new(original));
        assert_eq!(stored.marker_path, std::path::Path::new(original));
        let duplicate = database
            .osu_installations()
            .register(
                &OsuMarker {
                    kind: OsuKind::Lazer,
                    root_path: original.into(),
                    marker_path: original.into(),
                },
                None,
            )
            .await
            .unwrap();
        assert!(!duplicate.was_created());
        assert_eq!(duplicate.installation().id, stored.id);
    }
    drop_application_tables(database).await;
}

async fn native_path_registration(database: &Database, other: &Database) {
    #[cfg(unix)]
    let roots = {
        use std::os::unix::ffi::OsStringExt;
        [
            std::ffi::OsString::from_vec(b"/osu/\xff".to_vec()),
            std::ffi::OsString::from_vec(b"/osu/\xfe".to_vec()),
        ]
    };
    #[cfg(windows)]
    let roots = {
        use std::os::windows::ffi::OsStringExt;
        [
            std::ffi::OsString::from_wide(&[67, 58, 92, 0xd800]),
            std::ffi::OsString::from_wide(&[67, 58, 92, 0xd801]),
        ]
    };
    assert_eq!(roots[0].to_string_lossy(), roots[1].to_string_lossy());
    let mut ids = Vec::new();
    for root in roots {
        let root = std::path::PathBuf::from(root);
        let marker = OsuMarker {
            kind: OsuKind::Lazer,
            marker_path: root.join("client.realm"),
            root_path: root,
        };
        let registered = database
            .osu_installations()
            .register(&marker, None)
            .await
            .unwrap();
        assert!(registered.was_created());
        let stored = other
            .osu_installations()
            .get(registered.installation().id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.root_path, marker.root_path);
        assert_eq!(stored.marker_path, marker.marker_path);
        let duplicate = other
            .osu_installations()
            .register(&marker, None)
            .await
            .unwrap();
        assert!(!duplicate.was_created());
        assert_eq!(duplicate.installation(), &stored);
        ids.push(stored.id);
    }
    assert_ne!(ids[0], ids[1]);
    for id in ids {
        database.osu_installations().delete(id).await.unwrap();
    }
}

async fn aggregate_grouping_and_cleanup(database: &Database) {
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    let installation = transaction
        .osu_installations()
        .register(&marker("groups"), None)
        .await
        .unwrap()
        .into_installation();
    let imported = &snapshot("Groups", "/audio")[0];
    let empty = transaction
        .beatmap_sets()
        .insert(installation.id, imported)
        .await
        .unwrap();
    let populated = transaction
        .beatmap_sets()
        .insert(installation.id, imported)
        .await
        .unwrap();
    let audio = transaction.audio_sources();
    let first = audio
        .get_or_insert(&SourceType::Local("/first".into()))
        .await
        .unwrap();
    let second = audio
        .get_or_insert(&SourceType::Local("/second".into()))
        .await
        .unwrap();
    let orphan = audio
        .get_or_insert(&SourceType::Local("/orphan".into()))
        .await
        .unwrap();
    let metadata = transaction.beatmap_metadata();
    let used = metadata
        .get_or_insert(&self::metadata("used"))
        .await
        .unwrap();
    let unused = metadata
        .get_or_insert(&self::metadata("unused"))
        .await
        .unwrap();
    for id in [Some(second.id), None, Some(first.id), Some(second.id)] {
        transaction
            .beatmaps()
            .insert(
                populated.id,
                &imported.beatmaps[0],
                id.map(|_| used.hash.clone()),
                id,
                None,
            )
            .await
            .unwrap();
    }
    metadata.cleanup().await.unwrap();
    audio.cleanup().await.unwrap();
    assert!(metadata.get(&used.hash).await.unwrap().is_some());
    assert!(metadata.get(&unused.hash).await.unwrap().is_none());
    assert!(audio.get(orphan.id).await.unwrap().is_none());
    let grouped = transaction
        .beatmap_sets()
        .all_with_audio_sources()
        .await
        .unwrap();
    assert_eq!(grouped.len(), 2);
    assert_eq!(grouped[0].beatmap_set, empty);
    assert!(grouped[0].beatmaps.is_empty());
    assert!(grouped[0].audio_sources.is_empty());
    assert_eq!(grouped[1].beatmap_set, populated);
    assert_eq!(grouped[1].beatmaps.len(), 4);
    assert_eq!(grouped[1].audio_sources, [first, second]);
    transaction.rollback().await.unwrap();
}

#[path = "tests/tag_migration.rs"]
mod tag_migration;

async fn tag_repository_contracts(database: &Database) {
    let repository = database.tags();
    assert_eq!(repository.get_or_insert("\u{2003}\t ").await.unwrap(), None);
    let rock = repository.get_or_insert(" Rock ").await.unwrap().unwrap();
    assert_eq!(rock.name, "rock");
    for name in ["ROCK", "rock", "\tRoCk\n"] {
        assert_eq!(
            repository.get_or_insert(name).await.unwrap(),
            Some(rock.clone())
        );
    }
    let transaction = database.begin().await.unwrap();
    let installation = transaction
        .osu_installations()
        .register(&marker("tag-constraints"), None)
        .await
        .unwrap()
        .into_installation();
    let imported = snapshot("Tags", "/audio/tags.mp3");
    let set = transaction
        .beatmap_sets()
        .insert(installation.id, &imported[0])
        .await
        .unwrap();
    transaction.tags().link(set.id, rock.id).await.unwrap();
    transaction.tags().link(set.id, rock.id).await.unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(
        repository.for_set(set.id).await.unwrap(),
        std::slice::from_ref(&rock)
    );
    for name in ["rocket", "ЁЖ", "a+b%_&?#", "quote'\\tail"] {
        let tag = repository.get_or_insert(name).await.unwrap().unwrap();
        repository.link(set.id, tag.id).await.unwrap();
    }
    for word in ["roc", "ёж", "%", "_", "a+b", "&?#", "'", "\\"] {
        assert_eq!(
            repository.matching_set_ids(word).await.unwrap(),
            [set.id],
            "{word}"
        );
    }
    for word in ["rock%", ".*", "' OR 1=1 --", "not-present"] {
        assert!(
            repository.matching_set_ids(word).await.unwrap().is_empty(),
            "{word}"
        );
    }
    assert!(repository.link(set.id, i32::MAX).await.is_err());
    assert!(repository.link(i32::MAX, rock.id).await.is_err());
    assert!(
        crate::entities::tag::Entity::delete_by_id(rock.id)
            .exec(&database.connection)
            .await
            .is_err()
    );
    repository.cleanup().await.unwrap();
    assert_eq!(repository.get(rock.id).await.unwrap(), Some(rock.clone()));
    database
        .osu_installations()
        .delete(installation.id)
        .await
        .unwrap();
    assert!(repository.for_set(set.id).await.unwrap().is_empty());
    repository.cleanup().await.unwrap();
    assert!(repository.all().await.unwrap().is_empty());
}
