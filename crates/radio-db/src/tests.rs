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
    repository_transaction_contracts(&database).await;
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
