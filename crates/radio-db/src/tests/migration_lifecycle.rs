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
    search_text_upgrade(database).await;
    library_search_upgrade(database).await;
}

#[allow(clippy::too_many_lines)] // One populated v18 fixture exercises migration and rollback on both backends.
async fn library_search_upgrade(database: &Database) {
    use sea_orm::sea_query::{Alias, Query};
    Migrator::up(&database.connection, Some(18)).await.unwrap();
    sql(database, "INSERT INTO osu_installations (id,user_data_id,kind,root_path,marker_path) VALUES (1,1,'stable','\"/v18\"','\"/v18/osu!.db\"')").await;
    sql(
        database,
        "INSERT INTO beatmap_sets (id,installation_id,online_id,hash) VALUES (1,1,42,'set-hash')",
    )
    .await;
    sql(
        database,
        "INSERT INTO audio_sources (id,kind,location) VALUES (1,'local','/v18/audio')",
    )
    .await;
    sql(
        database,
        "INSERT INTO playlists (id,name) VALUES (1,'Preserved playlist')",
    )
    .await;
    sql(database, "INSERT INTO playlist_items (id,playlist_id,source_kind,beatmap_hash,title) VALUES (1,1,'stable','map-hash','Saved')").await;
    sql(database, "INSERT INTO listening_history (audio_kind,source_location,last_played_at_ms,playback_token) VALUES ('local','/v18/audio',123456,7)").await;
    sql(
        database,
        "UPDATE user_data SET global_volume_percent=37 WHERE id=1",
    )
    .await;
    for id in 1..=257 {
        let (author, source) = legacy_search_values(id);
        database
            .connection
            .execute(
                Query::insert()
                    .into_table(Alias::new("beatmap_metadata"))
                    .columns(["hash", "title", "author", "source"].map(Alias::new))
                    .values_panic([
                        format!("legacy-{id:03}").into(),
                        "Original title".into(),
                        author.into(),
                        source.into(),
                    ]),
            )
            .await
            .unwrap();
        database
            .connection
            .execute(
                Query::insert()
                    .into_table(Alias::new("beatmaps"))
                    .columns(
                        [
                            "id",
                            "beatmap_set_id",
                            "audio_source_id",
                            "metadata_hash",
                            "hash",
                        ]
                        .map(Alias::new),
                    )
                    .values_panic([
                        id.into(),
                        1.into(),
                        1.into(),
                        format!("legacy-{id:03}").into(),
                        format!("map-{id}").into(),
                    ]),
            )
            .await
            .unwrap();
    }
    let identity = metadata("Migrated identity");
    let identity_hash = crate::metadata_hash(&identity).unwrap();
    database
        .connection
        .execute(
            Query::insert()
                .into_table(Alias::new("beatmap_metadata"))
                .columns(
                    [
                        "hash",
                        "title",
                        "title_unicode",
                        "artist",
                        "artist_unicode",
                        "search_title",
                        "search_title_unicode",
                        "search_artist",
                        "search_artist_unicode",
                        "author",
                        "source",
                        "preview_time",
                        "audio_file",
                        "background_file",
                    ]
                    .map(Alias::new),
                )
                .values_panic([
                    identity_hash.clone().into(),
                    identity.title.clone().into(),
                    identity.title_unicode.clone().into(),
                    identity.artist.clone().into(),
                    identity.artist_unicode.clone().into(),
                    identity
                        .title
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .into(),
                    identity
                        .title_unicode
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .into(),
                    identity
                        .artist
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .into(),
                    identity
                        .artist_unicode
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .into(),
                    serde_json::json!({"online_id": 7, "username": "Mapper", "country_code": "JP"})
                        .into(),
                    identity.source.clone().into(),
                    identity.preview_time.into(),
                    identity.audio_file.clone().into(),
                    identity.background_file.clone().into(),
                ]),
        )
        .await
        .unwrap();
    assert_legacy_search_data(database).await;
    #[cfg(feature = "sqlite")]
    sql(database, "CREATE TRIGGER fail_library_search BEFORE UPDATE ON beatmap_metadata WHEN NEW.hash = 'legacy-200' BEGIN SELECT RAISE(ABORT, 'forced backfill failure'); END").await;
    #[cfg(feature = "postgres")]
    {
        sql(database, "CREATE FUNCTION fail_library_search() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.hash = 'legacy-200' THEN RAISE EXCEPTION 'forced backfill failure'; END IF; RETURN NEW; END $$").await;
        sql(database, "CREATE TRIGGER fail_library_search BEFORE UPDATE ON beatmap_metadata FOR EACH ROW EXECUTE FUNCTION fail_library_search()").await;
    }
    assert!(database.migrate().await.is_err());
    assert_schema(database, 18).await;
    assert_legacy_search_data(database).await;
    #[cfg(feature = "sqlite")]
    sql(database, "DROP TRIGGER fail_library_search").await;
    #[cfg(feature = "postgres")]
    {
        sql(
            database,
            "DROP TRIGGER fail_library_search ON beatmap_metadata",
        )
        .await;
        sql(database, "DROP FUNCTION fail_library_search()").await;
    }
    for _ in 0..2 {
        database.migrate().await.unwrap();
        assert_schema(database, 19).await;
        assert_legacy_search_data(database).await;
        assert_eq!(
            database
                .beatmap_metadata()
                .get_or_insert(&identity)
                .await
                .unwrap()
                .hash,
            identity_hash
        );
        let rows = database.connection.query_all_raw(Statement::from_string(
            database.connection.get_database_backend(),
            "SELECT m.hash,m.search_mapper,m.search_source,b.id,b.stars,b.status,s.ranked_at_ms,s.submitted_at_ms,a.duration_ms \
             FROM beatmap_metadata m JOIN beatmaps b ON b.metadata_hash=m.hash \
             JOIN beatmap_sets s ON s.id=b.beatmap_set_id JOIN audio_sources a ON a.id=b.audio_source_id ORDER BY b.id",
        )).await.unwrap();
        assert_eq!(rows.len(), 257);
        for row in rows {
            let id = row.try_get::<i32>("", "id").unwrap();
            let (author, source) = legacy_search_values(id);
            let expected = author
                .as_ref()
                .and_then(|a| a.get("username"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_lowercase();
            assert_eq!(
                row.try_get::<String>("", "search_mapper").unwrap(),
                expected
            );
            assert_eq!(
                row.try_get::<String>("", "search_source").unwrap(),
                source.unwrap_or_default().to_lowercase()
            );
            assert_eq!(row.try_get::<Option<f64>>("", "stars").unwrap(), None);
            assert_eq!(row.try_get::<Option<String>>("", "status").unwrap(), None);
            for column in ["ranked_at_ms", "submitted_at_ms", "duration_ms"] {
                assert_eq!(row.try_get::<Option<i64>>("", column).unwrap(), None);
            }
        }
    }
    Migrator::down(&database.connection, Some(1)).await.unwrap();
    assert_schema(database, 18).await;
    assert_legacy_search_data(database).await;
    database.migrate().await.unwrap();
    assert_schema(database, 19).await;
    assert_legacy_search_data(database).await;
    drop_application_tables(database).await;
}

fn legacy_search_values(id: i32) -> (Option<serde_json::Value>, Option<String>) {
    match id % 4 {
        0 => (None, None),
        1 => (
            Some(serde_json::json!({"username": null, "online_id": 7})),
            Some(String::new()),
        ),
        2 => (Some(serde_json::json!({"username": ""})), None),
        _ => (
            Some(serde_json::json!({"username": "İSTANBUL ЁЖ ΟΣ", "online_id": 7})),
            Some("İSTANBUL ЁЖ ΟΣ".into()),
        ),
    }
}

async fn assert_legacy_search_data(database: &Database) {
    let rows = database.connection.query_all_raw(Statement::from_string(database.connection.get_database_backend(),
        "SELECT m.hash,m.title,m.author,m.source,b.id,b.hash AS map_hash,b.beatmap_set_id,b.audio_source_id FROM beatmap_metadata m JOIN beatmaps b ON b.metadata_hash=m.hash ORDER BY b.id")).await.unwrap();
    assert_eq!(rows.len(), 257);
    for row in rows {
        let id = row.try_get::<i32>("", "id").unwrap();
        let (author, source) = legacy_search_values(id);
        assert_eq!(
            row.try_get::<String>("", "hash").unwrap(),
            format!("legacy-{id:03}")
        );
        assert_eq!(
            row.try_get::<String>("", "title").unwrap(),
            "Original title"
        );
        assert_eq!(
            row.try_get::<Option<serde_json::Value>>("", "author")
                .unwrap(),
            author
        );
        assert_eq!(row.try_get::<Option<String>>("", "source").unwrap(), source);
        assert_eq!(
            row.try_get::<String>("", "map_hash").unwrap(),
            format!("map-{id}")
        );
        assert_eq!(row.try_get::<i32>("", "beatmap_set_id").unwrap(), 1);
        assert_eq!(row.try_get::<i32>("", "audio_source_id").unwrap(), 1);
    }
    let row = database.connection.query_one_raw(Statement::from_string(database.connection.get_database_backend(),
        "SELECT s.id AS set_id,s.online_id,s.hash AS set_hash,a.id AS audio_id,a.location,p.id AS playlist_id,p.name,i.id AS item_id,i.beatmap_hash,h.last_played_at_ms,h.playback_token,u.global_volume_percent FROM beatmap_sets s JOIN audio_sources a ON a.id=1 JOIN playlists p ON p.id=1 JOIN playlist_items i ON i.playlist_id=p.id JOIN listening_history h ON h.source_location=a.location JOIN user_data u ON u.id=1 WHERE s.id=1")).await.unwrap().unwrap();
    for column in ["set_id", "audio_id", "playlist_id", "item_id"] {
        assert_eq!(row.try_get::<i32>("", column).unwrap(), 1);
    }
    assert_eq!(row.try_get::<i32>("", "online_id").unwrap(), 42);
    assert_eq!(row.try_get::<String>("", "set_hash").unwrap(), "set-hash");
    assert_eq!(row.try_get::<String>("", "location").unwrap(), "/v18/audio");
    assert_eq!(
        row.try_get::<String>("", "name").unwrap(),
        "Preserved playlist"
    );
    assert_eq!(
        row.try_get::<String>("", "beatmap_hash").unwrap(),
        "map-hash"
    );
    assert_eq!(
        row.try_get::<i64>("", "last_played_at_ms").unwrap(),
        123_456
    );
    assert_eq!(row.try_get::<i64>("", "playback_token").unwrap(), 7);
    assert_eq!(row.try_get::<i32>("", "global_volume_percent").unwrap(), 37);
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
    let set = insert_legacy_set(database, installation.id, &imported[0]).await;
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
    for version in 5..=19 {
        Migrator::up(&database.connection, Some(1)).await.unwrap();
        assert_schema(database, version).await;
        assert_library(database, &installation, set.id, map.id).await;
    }
    for version in (4..=18).rev() {
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
    assert_schema(database, 19).await;
    assert_library(database, &installation, set.id, map.id).await;
    assert_eq!(
        database.queue().get().await.unwrap(),
        crate::model::QueueState::default()
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
        ("audio_mods", 15),
        ("queue_entries", 12),
    ] {
        assert_eq!(
            manager.has_table(table).await.unwrap(),
            version >= since,
            "{table} at migration {version}"
        );
    }
    for (table, column, since) in [
        ("beatmaps", "background_path", 2),
        ("beatmaps", "search_difficulty_name", 17),
        ("beatmaps", "osu_last_played_at_ms", 18),
        ("beatmaps", "stars", 19),
        ("beatmaps", "status", 19),
        ("beatmap_sets", "ranked_at_ms", 19),
        ("beatmap_sets", "submitted_at_ms", 19),
        ("audio_sources", "duration_ms", 19),
        ("beatmap_metadata", "search_mapper", 19),
        ("beatmap_metadata", "search_source", 19),
        ("beatmap_metadata", "search_title", 17),
        ("beatmap_metadata", "search_title_unicode", 17),
        ("beatmap_metadata", "search_artist", 17),
        ("beatmap_metadata", "search_artist_unicode", 17),
        ("playback_queue", "current_entry_id", 12),
        ("playback_queue", "waiting_for_check", 13),
        ("playback_queue", "launch_audio_source_id", 14),
        ("playback_queue", "launch_audio_kind", 14),
        ("playback_queue", "launch_audio_location", 14),
        ("playback_queue", "launch_check_generation", 14),
        ("playback_queue", "shuffle", 16),
        ("playback_queue", "repeat_mode", 16),
        ("playback_queue", "repeat_launch", 16),
        ("queue_entries", "shuffle_order", 16),
        ("user_data", "individual_volume_enabled", 8),
        ("user_data", "global_volume_percent", 8),
        ("user_data", "individual_mods_enabled", 15),
        ("user_data", "global_mods_mode", 15),
        ("user_data", "global_mods_speed_percent", 15),
        ("user_data", "global_mods_adjust_pitch", 15),
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
    for column in ["audio_source_ids", "playlist_item_ids", "current_index"] {
        let since = if column == "playlist_item_ids" { 6 } else { 5 };
        assert_eq!(
            manager.has_column("playback_queue", column).await.unwrap(),
            (since..12).contains(&version)
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

#[allow(clippy::too_many_lines)] // One frozen fixture covers paged backfill, rollback and reversible upgrades.
async fn search_text_upgrade(database: &Database) {
    use sea_orm::sea_query::{Alias, Query};
    Migrator::up(&database.connection, Some(16)).await.unwrap();
    sql(database, "INSERT INTO osu_installations (id,user_data_id,kind,root_path,marker_path) VALUES (1,1,'stable','\"/search-upgrade\"','\"/search-upgrade/osu!.db\"')").await;
    sql(
        database,
        "INSERT INTO beatmap_sets (id,installation_id) VALUES (1,1)",
    )
    .await;
    sql(
        database,
        "INSERT INTO audio_sources (id,kind,location) VALUES (1,'local','search-upgrade')",
    )
    .await;
    // Cross two backfill page boundaries and retain NULL fields and metadata identities.
    for id in 1..=257 {
        let text = (id % 2 == 1).then_some("İSTANBUL ЁЖ ΟΣ");
        database
            .connection
            .execute(
                Query::insert()
                    .into_table(Alias::new("beatmap_metadata"))
                    .columns(
                        ["hash", "title", "title_unicode", "artist", "artist_unicode"]
                            .map(Alias::new),
                    )
                    .values_panic([
                        format!("legacy-{id:03}").into(),
                        text.into(),
                        text.into(),
                        text.into(),
                        text.into(),
                    ]),
            )
            .await
            .unwrap();
        database
            .connection
            .execute(
                Query::insert()
                    .into_table(Alias::new("beatmaps"))
                    .columns(
                        [
                            "id",
                            "beatmap_set_id",
                            "audio_source_id",
                            "metadata_hash",
                            "difficulty_name",
                        ]
                        .map(Alias::new),
                    )
                    .values_panic([
                        id.into(),
                        1.into(),
                        1.into(),
                        format!("legacy-{id:03}").into(),
                        text.into(),
                    ]),
            )
            .await
            .unwrap();
    }
    #[cfg(feature = "sqlite")]
    {
        sql(database, "CREATE TRIGGER fail_search_upgrade BEFORE UPDATE ON beatmap_metadata BEGIN SELECT RAISE(ABORT, 'backfill failure'); END").await;
        assert!(database.migrate().await.is_err());
        assert_schema(database, 16).await;
        sql(database, "DROP TRIGGER fail_search_upgrade").await;
    }
    for _ in 0..2 {
        database.migrate().await.unwrap();
        assert_schema(database, 19).await;
        let rows = database.connection.query_all_raw(Statement::from_string(
            database.connection.get_database_backend(),
            "SELECT m.*, b.id, b.difficulty_name, b.search_difficulty_name FROM beatmap_metadata m \
             JOIN beatmaps b ON b.metadata_hash = m.hash ORDER BY b.id",
        )).await.unwrap();
        assert_eq!(rows.len(), 257);
        for row in rows {
            let id = row.try_get::<i32>("", "id").unwrap();
            assert_eq!(
                row.try_get::<String>("", "hash").unwrap(),
                format!("legacy-{id:03}")
            );
            for (original, normalized) in [
                ("title", "search_title"),
                ("title_unicode", "search_title_unicode"),
                ("artist", "search_artist"),
                ("artist_unicode", "search_artist_unicode"),
                ("difficulty_name", "search_difficulty_name"),
            ] {
                let original = row.try_get::<Option<String>>("", original).unwrap();
                assert_eq!(
                    original.as_deref(),
                    (id % 2 == 1).then_some("İSTANBUL ЁЖ ΟΣ")
                );
                assert_eq!(
                    row.try_get::<String>("", normalized).unwrap(),
                    original.unwrap_or_default().to_lowercase()
                );
            }
        }
        assert_eq!(
            database
                .beatmap_sets()
                .matching_audio_sources("i\u{0307}stan")
                .await
                .unwrap(),
            [1]
        );
        assert!(
            database
                .beatmap_sets()
                .matching_audio_sources("missing")
                .await
                .unwrap()
                .is_empty()
        );
        Migrator::down(&database.connection, Some(3)).await.unwrap();
        assert_schema(database, 16).await;
    }
    drop_application_tables(database).await;
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
