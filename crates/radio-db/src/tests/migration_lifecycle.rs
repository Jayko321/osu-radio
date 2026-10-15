use super::*;
use crate::migrations::Migrator;
use sea_orm::Statement;
use sea_orm_migration::{MigratorTrait, SchemaManager};

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sqlite_migration_lifecycle() {
    let database = Database::connect(":memory:").await.unwrap();
    contracts(&database).await;
}

pub(super) async fn contracts(database: &Database) {
    initial_schema_constraints_and_ownership(database).await;
    reversible_migrations_preserve_the_library(database).await;
    background_downgrade_and_native_path_guard(database).await;
}

#[allow(clippy::too_many_lines)] // Keep the complete frozen v1 ownership fixture together.
async fn initial_schema_constraints_and_ownership(database: &Database) {
    Migrator::up(&database.connection, Some(1)).await.unwrap();
    let manager = SchemaManager::new(&database.connection);
    for table in [
        "user_data",
        "osu_installations",
        "beatmap_sets",
        "beatmap_metadata",
        "audio_sources",
        "beatmaps",
    ] {
        assert!(manager.has_table(table).await.unwrap(), "{table}");
    }
    for (table, field) in [
        ("osu_installations", "user_data_id"),
        ("beatmap_sets", "installation_id"),
        ("beatmaps", "beatmap_set_id"),
        ("beatmaps", "metadata_hash"),
        ("beatmaps", "audio_source_id"),
    ] {
        assert!(
            manager
                .has_index(table, &format!("idx_{table}_{field}"))
                .await
                .unwrap()
        );
    }
    for column in [
        "title",
        "title_unicode",
        "artist",
        "artist_unicode",
        "author",
        "source",
        "tags",
        "user_tags",
        "preview_time",
        "audio_file",
        "background_file",
    ] {
        assert!(
            manager
                .has_column("beatmap_metadata", column)
                .await
                .unwrap()
        );
    }
    assert!(
        !manager
            .has_column("beatmaps", "background_path")
            .await
            .unwrap()
    );
    assert!(!manager.has_table("tags").await.unwrap());
    sql(database,"INSERT INTO osu_installations (id,user_data_id,kind,root_path,marker_path) VALUES (1,1,'stable','/fixture','/fixture/osu!.db')").await;
    sql(
        database,
        "INSERT INTO beatmap_sets (id,installation_id) VALUES (1,1)",
    )
    .await;
    sql(
        database,
        "INSERT INTO beatmap_metadata (hash,user_tags) VALUES ('one','[]')",
    )
    .await;
    sql(
        database,
        "INSERT INTO audio_sources (id,kind,location) VALUES (1,'local','/fixture/audio')",
    )
    .await;
    sql(database,"INSERT INTO beatmaps (id,beatmap_set_id,metadata_hash,audio_source_id) VALUES (1,1,'one',1)").await;
    for invalid in [
        "INSERT INTO user_data (id) VALUES (2)",
        "INSERT INTO osu_installations (id,user_data_id,kind,root_path,marker_path) VALUES (2,1,'unknown','/bad','/bad')",
        "INSERT INTO osu_installations (id,user_data_id,kind,root_path,marker_path) VALUES (2,99,'lazer','/bad','/bad')",
        "INSERT INTO osu_installations (id,user_data_id,kind,root_path,marker_path) VALUES (2,1,'lazer','/other','/fixture/osu!.db')",
        "INSERT INTO beatmap_sets (id,installation_id) VALUES (2,99)",
        "INSERT INTO beatmap_metadata (hash) VALUES ('missing-tags')",
        "INSERT INTO audio_sources (id,kind,location) VALUES (2,'unknown','/bad')",
        "INSERT INTO audio_sources (id,kind,location) VALUES (2,'local','/fixture/audio')",
        "INSERT INTO beatmaps (id,beatmap_set_id) VALUES (2,99)",
        "INSERT INTO beatmaps (id,beatmap_set_id,metadata_hash) VALUES (2,1,'missing')",
        "INSERT INTO beatmaps (id,beatmap_set_id,audio_source_id) VALUES (2,1,99)",
        "DELETE FROM user_data WHERE id=1",
        "DELETE FROM beatmap_metadata WHERE hash='one'",
        "DELETE FROM audio_sources WHERE id=1",
    ] {
        assert!(
            database
                .connection
                .execute_unprepared(invalid)
                .await
                .is_err(),
            "initial schema accepted {invalid}"
        );
    }
    sql(database, "DELETE FROM osu_installations WHERE id=1").await;
    for (table, expected) in [
        ("user_data", 1_i64),
        ("osu_installations", 0),
        ("beatmap_sets", 0),
        ("beatmaps", 0),
        ("beatmap_metadata", 1),
        ("audio_sources", 1),
    ] {
        let row = database
            .connection
            .query_one_raw(Statement::from_string(
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
    drop_application_tables(database).await;
}

async fn reversible_migrations_preserve_the_library(database: &Database) {
    Migrator::up(&database.connection, Some(4)).await.unwrap();
    sql(
        database,
        "CREATE TABLE migration_probe (id INTEGER PRIMARY KEY)",
    )
    .await;
    sql(database, "INSERT INTO migration_probe VALUES (42)").await;
    let installation = database
        .osu_installations()
        .register(&marker("migration-lifecycle"), Some("Keep me"))
        .await
        .unwrap()
        .into_installation();
    let imported = snapshot("Keep library", "/migration/audio");
    let set = database
        .beatmap_sets()
        .insert(installation.id, &imported[0])
        .await
        .unwrap();
    let map = insert_legacy_beatmap(
        database,
        set.id,
        &imported[0].beatmaps[0],
        None,
        None,
        Some("/migration/cover".into()),
    )
    .await
    .unwrap();
    assert_schema(database, 4).await;
    for version in 5..=11 {
        Migrator::up(&database.connection, Some(1)).await.unwrap();
        assert_schema(database, version).await;
        assert_library(database, &installation, set.id, map.id).await;
    }
    for version in (4..=10).rev() {
        Migrator::down(&database.connection, Some(1)).await.unwrap();
        assert_schema(database, version).await;
        assert_library(database, &installation, set.id, map.id).await;
    }
    let error = Migrator::down(&database.connection, Some(1))
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cannot restore the original metadata tag distribution")
    );
    assert_schema(database, 4).await;
    assert_library(database, &installation, set.id, map.id).await;
    database.migrate().await.unwrap();
    assert_schema(database, 11).await;
    assert_library(database, &installation, set.id, map.id).await;
    assert_eq!(
        database.queue().get().await.unwrap(),
        crate::model::QueueState {
            audio_source_ids: Vec::new(),
            playlist_item_ids: Vec::new(),
            current_index: None,
            mode: crate::model::PlaybackMode::Stopped,
            revision: 0,
            playback_token: 0,
        }
    );
    assert_eq!(
        database.user_data().audio_settings().await.unwrap(),
        crate::model::AudioSettings::default()
    );
    drop_application_tables(database).await;
    sql(database, "DROP TABLE migration_probe").await;
}

async fn assert_schema(database: &Database, version: usize) {
    let manager = SchemaManager::new(&database.connection);
    assert_eq!(
        Migrator::get_applied_migrations(&database.connection)
            .await
            .unwrap()
            .len(),
        version
    );
    for (table, since) in [
        ("playback_queue", 5),
        ("playlists", 6),
        ("playlist_items", 6),
        ("listening_history", 7),
        ("audio_volume", 8),
    ] {
        assert_eq!(
            manager.has_table(table).await.unwrap(),
            version >= since,
            "{table} at migration {version}"
        );
    }
    for (table, column, since) in [
        ("beatmaps", "background_path", 2),
        ("playback_queue", "playlist_item_ids", 6),
        ("user_data", "individual_volume_enabled", 8),
        ("user_data", "global_volume_percent", 8),
        ("playlists", "custom_cover_png", 9),
        ("playlists", "custom_cover_revision", 9),
        ("playlist_items", "title_unicode", 10),
        ("playlist_items", "artist_unicode", 10),
        ("beatmaps", "md5_hash", 11),
        ("playlist_items", "hash_kind", 11),
        ("playlists", "origin_source", 11),
        ("playlists", "origin_collection", 11),
    ] {
        assert_eq!(
            manager.has_column(table, column).await.unwrap(),
            version >= since,
            "{table}.{column} at migration {version}"
        );
    }
    for (table, index, since) in [
        ("beatmaps", "idx_beatmaps_source_hash", 6),
        ("beatmaps", "idx_beatmaps_md5_hash", 11),
        ("playlists", "uq_playlists_origin", 11),
    ] {
        assert_eq!(
            manager.has_index(table, index).await.unwrap(),
            version >= since,
            "{index} at migration {version}"
        );
    }
}

async fn assert_library(
    database: &Database,
    installation: &crate::model::OsuInstallation,
    set_id: i32,
    map_id: i32,
) {
    assert_eq!(
        database
            .osu_installations()
            .get(installation.id)
            .await
            .unwrap(),
        Some(installation.clone())
    );
    let row = database.connection.query_one_raw(Statement::from_string(database.connection.get_database_backend(),
        format!("SELECT difficulty_name, background_path, beatmap_set_id FROM beatmaps WHERE id = {map_id}")))
        .await.unwrap().unwrap();
    assert_eq!(
        row.try_get::<String>("", "difficulty_name").unwrap(),
        "Easy"
    );
    assert_eq!(
        row.try_get::<String>("", "background_path").unwrap(),
        "/migration/cover"
    );
    assert_eq!(row.try_get::<i32>("", "beatmap_set_id").unwrap(), set_id);
    let row = database
        .connection
        .query_one_raw(Statement::from_string(
            database.connection.get_database_backend(),
            "SELECT id FROM migration_probe",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i32>("", "id").unwrap(), 42);
}

async fn background_downgrade_and_native_path_guard(database: &Database) {
    Migrator::up(&database.connection, Some(2)).await.unwrap();
    sql(database, "INSERT INTO osu_installations (id,kind,root_path,marker_path,enabled,user_data_id) VALUES (1,'stable','/test/曲','/test/曲/osu!.db',TRUE,1)").await;
    sql(
        database,
        "INSERT INTO beatmap_sets (id,installation_id) VALUES (1,1)",
    )
    .await;
    sql(database, "INSERT INTO beatmaps (id,beatmap_set_id,difficulty_name,background_path) VALUES (1,1,'Keep difficulty','/cover')").await;
    Migrator::down(&database.connection, Some(1)).await.unwrap();
    let manager = SchemaManager::new(&database.connection);
    assert!(
        !manager
            .has_column("beatmaps", "background_path")
            .await
            .unwrap()
    );
    let row = database
        .connection
        .query_one_raw(Statement::from_string(
            database.connection.get_database_backend(),
            "SELECT difficulty_name FROM beatmaps WHERE id = 1",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        row.try_get::<String>("", "difficulty_name").unwrap(),
        "Keep difficulty"
    );
    Migrator::up(&database.connection, Some(2)).await.unwrap();
    let installation = database.osu_installations().get(1).await.unwrap().unwrap();
    assert_eq!(installation.root_path, std::path::Path::new("/test/曲"));
    assert_eq!(
        installation.marker_path,
        std::path::Path::new("/test/曲/osu!.db")
    );
    let error = Migrator::down(&database.connection, Some(1))
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cannot be downgraded to lossy text storage")
    );
    assert_eq!(
        Migrator::get_applied_migrations(&database.connection)
            .await
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        database.osu_installations().get(1).await.unwrap(),
        Some(installation)
    );
    assert!(
        manager
            .has_column("beatmaps", "background_path")
            .await
            .unwrap()
    );
    drop_application_tables(database).await;
}
