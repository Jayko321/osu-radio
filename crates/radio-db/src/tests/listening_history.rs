use super::*;
use crate::model::{PlaybackMode, QueueState};
use sea_orm_migration::MigratorTrait;

#[allow(clippy::too_many_lines)] // Upgrade preserves populated models, then verifies durable source identity and reset.
pub(super) async fn migration_contracts(database: &Database, url: &str) {
    crate::migrations::Migrator::up(&database.connection, Some(6))
        .await
        .unwrap();
    let installation = database
        .osu_installations()
        .register(&marker("history-upgrade"), None)
        .await
        .unwrap()
        .into_installation();
    let imported = snapshot("History upgrade", "/history-upgrade/audio");
    let set = database
        .beatmap_sets()
        .insert(installation.id, &imported[0])
        .await
        .unwrap();
    let source = SourceType::Local("{\"Unix\":[255,47,97]}".into());
    let audio = database
        .audio_sources()
        .get_or_insert(&source)
        .await
        .unwrap();
    let map = database
        .beatmaps()
        .insert(set.id, &imported[0].beatmaps[0], None, Some(audio.id), None)
        .await
        .unwrap();
    let playlist = database.playlists().create("Keep playlist").await.unwrap();
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
    let playlist = database
        .playlists()
        .get(playlist.id)
        .await
        .unwrap()
        .unwrap();
    let queue = QueueState {
        audio_source_ids: vec![audio.id],
        playlist_item_ids: vec![Some(playlist.items[0].id)],
        current_index: Some(0),
        mode: PlaybackMode::Playing,
        revision: 3,
        playback_token: 7,
    };
    database.queue().save(&queue).await.unwrap();
    let other = Database::connect(url).await.unwrap();
    let (one, two) = tokio::join!(database.migrate(), other.migrate());
    one.unwrap();
    two.unwrap();
    assert_eq!(database.beatmaps().get(map.id).await.unwrap(), Some(map));
    assert_eq!(
        database.playlists().get(playlist.id).await.unwrap(),
        Some(playlist)
    );
    assert_eq!(database.queue().get().await.unwrap(), queue);
    assert!(
        database
            .listening_history()
            .for_audio_sources(&[audio.id])
            .await
            .unwrap()
            .is_empty(),
        "upgrade does not backfill queue launches"
    );
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    assert!(
        transaction
            .listening_history()
            .record_started(&source, 7, 1234)
            .await
            .unwrap()
    );
    assert!(
        !transaction
            .listening_history()
            .record_started(&source, 7, 9999)
            .await
            .unwrap()
    );
    transaction.commit().await.unwrap();
    assert_eq!(
        other
            .listening_history()
            .for_audio_sources(&[audio.id])
            .await
            .unwrap()
            .get(&audio.id),
        Some(&1234)
    );
    let reader = database.begin_read().await.unwrap();
    assert_eq!(reader.queue().get().await.unwrap(), queue);
    let writer = other.begin().await.unwrap();
    writer.user_data().lock().await.unwrap();
    assert!(
        writer
            .listening_history()
            .record_started(&source, 8, 5678)
            .await
            .unwrap()
    );
    let mut updated = queue.clone();
    updated.revision = updated.revision.checked_add(1).unwrap();
    writer.queue().save(&updated).await.unwrap();
    writer.commit().await.unwrap();
    assert_eq!(
        reader
            .listening_history()
            .for_audio_sources(&vec![audio.id; 1002])
            .await
            .unwrap()
            .get(&audio.id),
        Some(&1234),
        "date batches and queue use the same read snapshot"
    );
    assert_eq!(reader.queue().get().await.unwrap(), queue);
    reader.commit().await.unwrap();
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    assert!(
        transaction
            .listening_history()
            .record_started(&source, 9, 9999)
            .await
            .unwrap()
    );
    transaction.rollback().await.unwrap();
    assert_eq!(
        other
            .listening_history()
            .for_audio_sources(&[audio.id])
            .await
            .unwrap()
            .get(&audio.id),
        Some(&5678)
    );
    database
        .osu_installations()
        .delete(installation.id)
        .await
        .unwrap();
    database.audio_sources().cleanup().await.unwrap();
    assert!(
        database
            .audio_sources()
            .get(audio.id)
            .await
            .unwrap()
            .is_none()
    );
    let copied = database
        .audio_sources()
        .get_or_insert(&SourceType::Copied(source.location().into()))
        .await
        .unwrap();
    let returned = database
        .audio_sources()
        .get_or_insert(&source)
        .await
        .unwrap();
    assert_ne!(returned.id, audio.id);
    let dates = other
        .listening_history()
        .for_audio_sources(&[copied.id, returned.id])
        .await
        .unwrap();
    assert_eq!(dates.get(&returned.id), Some(&5678));
    assert!(
        !dates.contains_key(&copied.id),
        "audio kind is part of identity"
    );
    assert!(
        database
            .listening_history()
            .record_started(&source, u64::MAX, 1)
            .await
            .is_err()
    );
    database.reset().await.unwrap();
    let returned = database
        .audio_sources()
        .get_or_insert(&source)
        .await
        .unwrap();
    assert!(
        database
            .listening_history()
            .for_audio_sources(&[returned.id])
            .await
            .unwrap()
            .is_empty()
    );
    drop_application_tables(database).await;
}
