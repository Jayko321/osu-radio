use super::*;
use sea_orm::{
    Statement,
    sea_query::{Alias, ExprTrait, Query},
};
use sea_orm_migration::{MigratorTrait, SchemaManager};
use sha2::{Digest, Sha256};

#[allow(clippy::too_many_lines)] // Verify the complete populated upgrade in one fixture.
pub(super) async fn contracts(database: &Database, url: &str) {
    crate::migrations::Migrator::up(&database.connection, Some(3))
        .await
        .unwrap();
    let installation = database
        .osu_installations()
        .register(&marker("tags-upgrade"), None)
        .await
        .unwrap()
        .into_installation();
    let mut first = metadata("Shared title");
    first.tags = Some("  Rock\tjapanese\nÄPFEL ЁЖ İSTANBUL ".into());
    first.user_tags = vec![
        " Piano Only ".into(),
        "rock".into(),
        String::new(),
        " \t ".into(),
    ];
    let mut second = first.clone();
    second.tags = Some("ROCK zeta".into());
    second.user_tags = vec!["piano only".into(), " Rock ".into()];
    let first_hash = insert_old(database, &first).await;
    let second_hash = insert_old(database, &second).await;
    assert_ne!(first_hash, second_hash);
    let imported = snapshot("Shared title", "/audio/unchanged.mp3");
    let mut set_ids = Vec::new();
    let mut old_maps = Vec::new();
    for hashes in [
        vec![&first_hash, &second_hash, &first_hash],
        vec![&second_hash],
    ] {
        let set = database
            .beatmap_sets()
            .insert(installation.id, &imported[0])
            .await
            .unwrap();
        set_ids.push(set.id);
        for hash in hashes {
            old_maps.push(
                database
                    .beatmaps()
                    .insert(
                        set.id,
                        &imported[0].beatmaps[0],
                        Some(hash.clone()),
                        None,
                        Some("/unchanged.jpg".into()),
                    )
                    .await
                    .unwrap(),
            );
        }
        old_maps.push(
            database
                .beatmaps()
                .insert(set.id, &imported[0].beatmaps[0], None, None, None)
                .await
                .unwrap(),
        );
    }
    let before = database
        .osu_installations()
        .get(installation.id)
        .await
        .unwrap();
    let other = Database::connect(url).await.unwrap();
    let (a, b) = tokio::join!(database.migrate(), other.migrate());
    a.unwrap();
    b.unwrap();
    let all = database.tags().all().await.unwrap();
    assert_eq!(
        all.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
        [
            "i\u{307}stanbul",
            "japanese",
            "piano only",
            "rock",
            "zeta",
            "äpfel",
            "ёж"
        ]
    );
    assert_eq!(database.tags().for_set(set_ids[0]).await.unwrap(), all);
    let second_tags = database.tags().for_set(set_ids[1]).await.unwrap();
    assert_eq!(
        second_tags
            .iter()
            .map(|tag| tag.name.as_str())
            .collect::<Vec<_>>(),
        ["piano only", "rock", "zeta"]
    );
    for tag in second_tags {
        assert!(all.contains(&tag));
    }
    let new_hash = crate::metadata_hash(&first).unwrap();
    assert_eq!(new_hash, crate::metadata_hash(&second).unwrap());
    for mut map in old_maps {
        if map.metadata_hash.is_some() {
            map.metadata_hash = Some(new_hash.clone());
        }
        assert_eq!(database.beatmaps().get(map.id).await.unwrap(), Some(map));
    }
    assert_eq!(
        database
            .osu_installations()
            .get(installation.id)
            .await
            .unwrap(),
        before
    );
    assert_counts(database, [2, 6, 1, 0]).await;
    assert_eq!(
        database.beatmap_metadata().get(&first_hash).await.unwrap(),
        None
    );
    assert_eq!(
        database.beatmap_metadata().get(&second_hash).await.unwrap(),
        None
    );
    let stored = database
        .beatmap_metadata()
        .get(&new_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        database
            .beatmap_metadata()
            .get_or_insert(&first)
            .await
            .unwrap(),
        stored
    );
    let manager = SchemaManager::new(&database.connection);
    assert!(
        !manager
            .has_column("beatmap_metadata", "tags")
            .await
            .unwrap()
    );
    assert!(
        !manager
            .has_column("beatmap_metadata", "user_tags")
            .await
            .unwrap()
    );
    assert!(
        manager
            .has_index("beatmap_set_tags", "idx_beatmap_set_tags_tag_id")
            .await
            .unwrap()
    );
    assert!(
        crate::migrations::Migrator::down(&database.connection, Some(1))
            .await
            .is_err()
    );
    database.migrate().await.unwrap();
    assert_eq!(database.tags().all().await.unwrap(), all);
    assert_counts(database, [2, 6, 1, 0]).await;
    drop_application_tables(database).await;
    collision_rolls_back(database).await;
    drop_application_tables(database).await;
}

async fn insert_old(database: &Database, metadata: &ImportedMetadata) -> String {
    let author = metadata
        .author
        .as_ref()
        .map(|a| (a.online_id, &a.username, &a.country_code));
    let hash = hex::encode(Sha256::digest(
        serde_json::to_vec(&(
            "radio-db:metadata:v1",
            &metadata.title,
            &metadata.title_unicode,
            &metadata.artist,
            &metadata.artist_unicode,
            author,
            &metadata.source,
            &metadata.tags,
            &metadata.user_tags,
            metadata.preview_time,
            &metadata.audio_file,
            &metadata.background_file,
        ))
        .unwrap(),
    ));
    let author = metadata.author.as_ref().map(|a| {
        serde_json::json!({
            "online_id": a.online_id, "username": a.username, "country_code": a.country_code,
        })
    });
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
                        "author",
                        "source",
                        "tags",
                        "user_tags",
                        "preview_time",
                        "audio_file",
                        "background_file",
                    ]
                    .map(Alias::new),
                )
                .values_panic([
                    hash.clone().into(),
                    metadata.title.clone().into(),
                    metadata.title_unicode.clone().into(),
                    metadata.artist.clone().into(),
                    metadata.artist_unicode.clone().into(),
                    author.into(),
                    metadata.source.clone().into(),
                    metadata.tags.clone().into(),
                    serde_json::json!(metadata.user_tags).into(),
                    metadata.preview_time.into(),
                    metadata.audio_file.clone().into(),
                    metadata.background_file.clone().into(),
                ]),
        )
        .await
        .unwrap();
    hash
}

#[allow(clippy::too_many_lines)] // Check schema, data and history rollback together.
async fn collision_rolls_back(database: &Database) {
    crate::migrations::Migrator::up(&database.connection, Some(3))
        .await
        .unwrap();
    let original = metadata("Original");
    let old_hash = insert_old(database, &original).await;
    let installation = database
        .osu_installations()
        .register(&marker("failed-tag-migration"), None)
        .await
        .unwrap()
        .into_installation();
    let imported = snapshot("Original", "/audio/old.mp3");
    let set = database
        .beatmap_sets()
        .insert(installation.id, &imported[0])
        .await
        .unwrap();
    let old_map = database
        .beatmaps()
        .insert(
            set.id,
            &imported[0].beatmaps[0],
            Some(old_hash.clone()),
            None,
            None,
        )
        .await
        .unwrap();
    let corrupt = insert_old(database, &metadata("Corrupted")).await;
    let new_hash = crate::metadata_hash(&original).unwrap();
    database
        .connection
        .execute(
            Query::update()
                .table(Alias::new("beatmap_metadata"))
                .value(Alias::new("hash"), new_hash.clone())
                .and_where(Expr::col(Alias::new("hash")).eq(corrupt)),
        )
        .await
        .unwrap();
    let error = database.migrate().await.unwrap_err();
    assert!(format!("{error:#}").contains("collision"), "{error:#}");
    assert_eq!(
        database.beatmaps().get(old_map.id).await.unwrap(),
        Some(old_map)
    );
    let manager = SchemaManager::new(&database.connection);
    assert!(!manager.has_table("tags").await.unwrap());
    assert!(!manager.has_table("beatmap_set_tags").await.unwrap());
    assert!(
        manager
            .has_column("beatmap_metadata", "user_tags")
            .await
            .unwrap()
    );
    assert!(
        manager
            .has_column("beatmap_metadata", "tags")
            .await
            .unwrap()
    );
    assert_eq!(
        crate::migrations::Migrator::get_applied_migrations(&database.connection)
            .await
            .unwrap()
            .len(),
        3
    );
    let rows = database
        .connection
        .query_all_raw(Statement::from_string(
            database.connection.get_database_backend(),
            "SELECT hash, title FROM beatmap_metadata",
        ))
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|row| row.try_get::<String>("", "hash").unwrap() == old_hash)
    );
    database
        .connection
        .execute(
            Query::delete()
                .from_table(Alias::new("beatmap_metadata"))
                .and_where(Expr::col(Alias::new("hash")).eq(new_hash)),
        )
        .await
        .unwrap();
    database.migrate().await.unwrap();
    assert!(
        database
            .beatmap_metadata()
            .get(&crate::metadata_hash(&original).unwrap())
            .await
            .unwrap()
            .is_some()
    );
}
