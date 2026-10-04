use super::*;
use crate::model::{PlaybackMode, QueueState};
use sea_orm_migration::MigratorTrait;

#[allow(clippy::too_many_lines)] // One populated upgrade checks metadata, cover versions and retained queue.
pub(super) async fn cover_migration_contracts(database: &Database, url: &str) {
    crate::migrations::Migrator::up(&database.connection, Some(8))
        .await
        .unwrap();
    let installation = database
        .osu_installations()
        .register(&marker("cover-upgrade"), None)
        .await
        .unwrap()
        .into_installation();
    let imported = snapshot("Saved cover library", "/cover-upgrade/audio");
    let set = database
        .beatmap_sets()
        .insert(installation.id, &imported[0])
        .await
        .unwrap();
    let audio = database
        .audio_sources()
        .get_or_insert(&SourceType::Local("/cover-upgrade/audio".into()))
        .await
        .unwrap();
    let map = insert_legacy_beatmap(
        database,
        set.id,
        &imported[0].beatmaps[0],
        None,
        Some(audio.id),
        Some("/cover-upgrade/background".into()),
    )
    .await
    .unwrap();
    sql(
        database,
        "INSERT INTO playlists (id, name) VALUES (501, 'Existing'), (502, 'Empty')",
    )
    .await;
    sql(database, "INSERT INTO playlist_items (id, playlist_id, source_kind, beatmap_hash, title) VALUES (501, 501, 'lazer', 'missing-first', 'Unavailable first'), (502, 501, 'lazer', 'beatmap-Easy', 'Available second')").await;
    sql(database, "UPDATE playback_queue SET audio_source_ids = '[11]', playlist_item_ids = '[501]', current_index = 0, mode = 'paused', revision = 7, playback_token = 9").await;
    let queue = database.queue().get().await.unwrap();
    let other = Database::connect(url).await.unwrap();
    let (first, second) = tokio::join!(database.migrate(), other.migrate());
    first.unwrap();
    second.unwrap();
    assert_eq!(
        database.beatmaps().get(map.id).await.unwrap(),
        Some(map.clone())
    );
    assert_eq!(database.queue().get().await.unwrap(), queue);
    let summaries = database.playlists().all().await.unwrap();
    assert_eq!(summaries[0].item_count, 2);
    assert_eq!(
        summaries[0].cover_beatmap_id, None,
        "do not skip the first unavailable item"
    );
    assert_eq!(summaries[0].custom_cover_revision, None);
    assert_eq!(summaries[1].item_count, 0);
    assert_eq!(summaries[1].cover_beatmap_id, None);
    assert_eq!(
        database
            .playlists()
            .get(501)
            .await
            .unwrap()
            .unwrap()
            .items
            .len(),
        2
    );
    database.playlists().remove_item(501, 501).await.unwrap();
    let automatic = database.playlists().summary(501).await.unwrap().unwrap();
    assert_eq!(automatic.item_count, 1);
    assert_eq!(automatic.cover_beatmap_id, Some(map.id));
    assert_eq!(database.queue().get().await.unwrap(), queue);
    assert!(
        database
            .playlists()
            .set_cover(501, b"persisted PNG bytes")
            .await
            .unwrap()
    );
    assert_eq!(
        other.playlists().cover(501).await.unwrap().as_deref(),
        Some(b"persisted PNG bytes".as_slice())
    );
    assert_eq!(
        other
            .playlists()
            .summary(501)
            .await
            .unwrap()
            .unwrap()
            .custom_cover_revision,
        Some(1)
    );
    assert!(database.playlists().clear_cover(501).await.unwrap());
    assert_eq!(
        database
            .playlists()
            .summary(501)
            .await
            .unwrap()
            .unwrap()
            .custom_cover_revision,
        None
    );
    assert_eq!(database.playlists().cover(501).await.unwrap(), None);
    assert!(
        database
            .playlists()
            .set_cover(501, b"replacement")
            .await
            .unwrap()
    );
    assert_eq!(
        database
            .playlists()
            .summary(501)
            .await
            .unwrap()
            .unwrap()
            .custom_cover_revision,
        Some(2)
    );
    assert!(
        !database
            .playlists()
            .set_cover(i32::MAX, b"absent")
            .await
            .unwrap()
    );
    assert!(!database.playlists().clear_cover(i32::MAX).await.unwrap());
    database.reset().await.unwrap();
    assert!(database.playlists().all().await.unwrap().is_empty());
    drop_application_tables(database).await;
}

#[allow(clippy::too_many_lines)] // Verify the additive upgrade, old queue and cascade/reset in one database.
pub(super) async fn migration_contracts(database: &Database, url: &str) {
    crate::migrations::Migrator::up(&database.connection, Some(5))
        .await
        .unwrap();
    let installation = database
        .osu_installations()
        .register(&marker("playlist-upgrade"), None)
        .await
        .unwrap()
        .into_installation();
    let imported = snapshot("Existing library", "/playlist-upgrade/audio");
    let set = database
        .beatmap_sets()
        .insert(installation.id, &imported[0])
        .await
        .unwrap();
    let map = insert_legacy_beatmap(database, set.id, &imported[0].beatmaps[0], None, None, None)
        .await
        .unwrap();
    sql(database, "UPDATE playback_queue SET audio_source_ids = '[11,11]', current_index = 1, mode = 'paused', revision = 7, playback_token = 9").await;
    let other = Database::connect(url).await.unwrap();
    let (first, second) = tokio::join!(database.migrate(), other.migrate());
    first.unwrap();
    second.unwrap();
    assert_eq!(
        database.beatmaps().get(map.id).await.unwrap(),
        Some(map.clone())
    );
    let before = QueueState {
        audio_source_ids: vec![11, 11],
        playlist_item_ids: vec![None, None],
        current_index: Some(1),
        mode: PlaybackMode::Paused,
        revision: 7,
        playback_token: 9,
    };
    assert_eq!(database.queue().get().await.unwrap(), before);
    let playlist = database.playlists().create("Saved").await.unwrap();
    let projection = database
        .playlists()
        .beatmaps(&[map.id])
        .await
        .unwrap()
        .remove(0);
    database
        .playlists()
        .add(playlist.id, &projection)
        .await
        .unwrap();
    database
        .playlists()
        .add(playlist.id, &projection)
        .await
        .unwrap();
    assert_eq!(
        database
            .playlists()
            .get(playlist.id)
            .await
            .unwrap()
            .unwrap()
            .items
            .len(),
        1
    );
    let item = database
        .playlists()
        .get(playlist.id)
        .await
        .unwrap()
        .unwrap()
        .items
        .remove(0);
    let mut queued = before.clone();
    queued.playlist_item_ids = vec![Some(item.id), Some(item.id)];
    database.queue().save(&queued).await.unwrap();
    database
        .osu_installations()
        .delete(installation.id)
        .await
        .unwrap();
    assert_eq!(
        other
            .playlists()
            .get(playlist.id)
            .await
            .unwrap()
            .unwrap()
            .items
            .len(),
        1
    );
    database.playlists().delete(playlist.id).await.unwrap();
    assert!(
        database.playlists().item(item.id).await.unwrap().is_none(),
        "only playlist deletion cascades membership"
    );
    assert_eq!(
        database.queue().get().await.unwrap(),
        queued,
        "queue keeps the launch snapshot even after playlist deletion"
    );
    let mut invalid = queued;
    invalid.playlist_item_ids.clear();
    assert!(database.queue().save(&invalid).await.is_err());
    assert!(database.playlists().create(" ").await.is_err());
    assert!(
        database
            .playlists()
            .add(i32::MAX, &projection)
            .await
            .is_err()
    );
    database.reset().await.unwrap();
    assert!(database.playlists().all().await.unwrap().is_empty());
    drop_application_tables(database).await;
}

#[allow(clippy::too_many_lines)] // Populate old schema and verify both upgrade and downgrade preservation.
pub(super) async fn unicode_migration_contracts(database: &Database) {
    crate::migrations::Migrator::up(&database.connection, Some(9))
        .await
        .unwrap();
    let installation = database
        .osu_installations()
        .register(&marker("unicode-upgrade"), None)
        .await
        .unwrap()
        .into_installation();
    let imported = snapshot("Ordinary", "/unicode-upgrade/audio");
    let set = database
        .beatmap_sets()
        .insert(installation.id, &imported[0])
        .await
        .unwrap();
    let mut first = metadata("First");
    first.title_unicode = Some("最初".into());
    first.artist_unicode = Some("作者".into());
    let mut second = first.clone();
    second.title_unicode = Some("第二".into());
    let first = database
        .beatmap_metadata()
        .get_or_insert(&first)
        .await
        .unwrap();
    let second = database
        .beatmap_metadata()
        .get_or_insert(&second)
        .await
        .unwrap();
    let online = database
        .audio_sources()
        .get_or_insert(&SourceType::Online("https://example.test/audio".into()))
        .await
        .unwrap();
    let copied = database
        .audio_sources()
        .get_or_insert(&SourceType::Copied("/unicode-upgrade/copied".into()))
        .await
        .unwrap();
    let local = database
        .audio_sources()
        .get_or_insert(&SourceType::Local("/unicode-upgrade/local".into()))
        .await
        .unwrap();
    // Earlier missing/Online beatmaps cannot win over a usable Local/Copied candidate.
    for (audio, metadata) in [
        (None, &second),
        (Some(online.id), &second),
        (Some(copied.id), &first),
        (Some(local.id), &second),
    ] {
        insert_legacy_beatmap(
            database,
            set.id,
            &imported[0].beatmaps[0],
            Some(metadata.hash.clone()),
            audio,
            None,
        )
        .await
        .unwrap();
    }
    sql(
        database,
        "INSERT INTO playlists (id, name) VALUES (701, 'Unicode')",
    )
    .await;
    sql(database, "INSERT INTO playlist_items (id, playlist_id, source_kind, beatmap_hash, title, artist, difficulty_name) VALUES (701, 701, 'lazer', 'beatmap-Easy', 'Saved title', 'Saved artist', 'Easy'), (702, 701, 'lazer', 'missing', 'Unavailable', NULL, NULL), (703, 701, 'stable', 'beatmap-Easy', 'Other kind', NULL, NULL)").await;
    let queue = database.queue().get().await.unwrap();
    database.migrate().await.unwrap();
    let saved = database.playlists().get(701).await.unwrap().unwrap();
    assert_eq!(saved.items[0].title.as_deref(), Some("Saved title"));
    assert_eq!(saved.items[0].artist.as_deref(), Some("Saved artist"));
    assert_eq!(saved.items[0].difficulty_name.as_deref(), Some("Easy"));
    assert_eq!(saved.items[0].title_unicode.as_deref(), Some("最初"));
    assert_eq!(saved.items[0].artist_unicode.as_deref(), Some("作者"));
    for item in &saved.items[1..] {
        assert_eq!(item.title_unicode, None);
        assert_eq!(item.artist_unicode, None);
    }
    assert_eq!(database.queue().get().await.unwrap(), queue);
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    transaction
        .connection
        .execute_unprepared("UPDATE audio_sources SET kind = 'online' WHERE kind = 'copied'")
        .await
        .unwrap();
    transaction
        .playlists()
        .refresh_unicode_names()
        .await
        .unwrap();
    assert_eq!(
        transaction
            .playlists()
            .get(701)
            .await
            .unwrap()
            .unwrap()
            .items[0]
            .title_unicode
            .as_deref(),
        Some("第二")
    );
    transaction.rollback().await.unwrap();
    assert_eq!(
        database.playlists().get(701).await.unwrap(),
        Some(saved.clone())
    );
    crate::migrations::Migrator::down(&database.connection, Some(2))
        .await
        .unwrap();
    assert!(
        !sea_orm_migration::SchemaManager::new(&database.connection)
            .has_column("playlist_items", "title_unicode")
            .await
            .unwrap()
    );
    database.migrate().await.unwrap();
    assert_eq!(database.playlists().get(701).await.unwrap(), Some(saved));
    drop_application_tables(database).await;
}
