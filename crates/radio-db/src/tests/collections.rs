use super::*;
use sea_orm_migration::{MigratorTrait, SchemaManager};

pub(super) async fn migration_contracts(database: &Database) {
    crate::migrations::Migrator::up(&database.connection, Some(10))
        .await
        .unwrap();
    let installation = database
        .osu_installations()
        .register(&marker("collections-upgrade"), None)
        .await
        .unwrap()
        .into_installation();
    let imported = snapshot("Original", "/collections-upgrade/audio");
    let set = database
        .beatmap_sets()
        .insert(installation.id, &imported[0])
        .await
        .unwrap();
    let map = insert_legacy_beatmap(database, set.id, &imported[0].beatmaps[0], None, None, None)
        .await
        .unwrap();
    sql(
        database,
        "INSERT INTO playlists (id,name) VALUES (801,'Original')",
    )
    .await;
    sql(database, "INSERT INTO playlist_items (id,playlist_id,source_kind,beatmap_hash) VALUES (801,801,'lazer','beatmap-Easy')").await;
    database.migrate().await.unwrap();
    assert_eq!(database.beatmaps().get(map.id).await.unwrap(), Some(map));
    let row = database
        .connection
        .query_one_raw(sea_orm::Statement::from_string(
            database.connection.get_database_backend(),
            "SELECT hash_kind FROM playlist_items WHERE id = 801",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<String>("", "hash_kind").unwrap(), "source");
    // Downgrade removes new columns only when no collection identity or MD5 membership would be lost.
    crate::migrations::Migrator::down(&database.connection, Some(1))
        .await
        .unwrap();
    assert!(
        !SchemaManager::new(&database.connection)
            .has_column("beatmaps", "md5_hash")
            .await
            .unwrap()
    );
    database.migrate().await.unwrap();
    sql(database, "UPDATE playlists SET origin_source = '/encoded/source', origin_collection = 'guid' WHERE id = 801").await;
    assert!(database.connection.execute_unprepared("INSERT INTO playlists(name,origin_source,origin_collection) VALUES ('duplicate','/encoded/source','guid')").await.is_err());
    assert!(
        crate::migrations::Migrator::down(&database.connection, Some(1))
            .await
            .is_err()
    );
    assert!(
        SchemaManager::new(&database.connection)
            .has_column("playlists", "origin_source")
            .await
            .unwrap()
    );
    sql(
        database,
        "UPDATE playlists SET origin_source = NULL, origin_collection = NULL WHERE id = 801",
    )
    .await;
    sql(
        database,
        "UPDATE playlist_items SET hash_kind = 'md5' WHERE id = 801",
    )
    .await;
    assert!(
        crate::migrations::Migrator::down(&database.connection, Some(1))
            .await
            .is_err()
    );
    assert!(
        database
            .connection
            .execute_unprepared("UPDATE playlist_items SET hash_kind = 'sha256' WHERE id = 801")
            .await
            .is_err()
    );
    sql(
        database,
        "UPDATE playlist_items SET hash_kind = 'source' WHERE id = 801",
    )
    .await;
    // The original unique item key deliberately still ignores hash_kind.
    assert!(database.connection.execute_unprepared("INSERT INTO playlist_items(playlist_id,source_kind,beatmap_hash,hash_kind) VALUES (801,'lazer','beatmap-Easy','md5')").await.is_err());
    crate::migrations::Migrator::down(&database.connection, Some(1))
        .await
        .unwrap();
    database.migrate().await.unwrap();
    drop_application_tables(database).await;
}
