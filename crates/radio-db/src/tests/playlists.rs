use super::*;
use crate::model::{PlaybackMode, QueueState};
use sea_orm_migration::MigratorTrait;

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
    let map = database
        .beatmaps()
        .insert(set.id, &imported[0].beatmaps[0], None, None, None)
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
