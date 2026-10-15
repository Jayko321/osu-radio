use super::*;
use crate::model::{PlaybackMode, QueueState};
use sea_orm_migration::MigratorTrait;

#[allow(clippy::too_many_lines)] // One migrated queue fixture checks reopen, rollback and rejected saves.
pub(super) async fn migration_contracts(database: &Database, url: &str) {
    crate::migrations::Migrator::up(&database.connection, Some(4))
        .await
        .unwrap();
    let installation = database
        .osu_installations()
        .register(&marker("queue-upgrade"), None)
        .await
        .unwrap()
        .into_installation();
    let imported = snapshot("Before queue", "/queue/existing");
    let set = database
        .beatmap_sets()
        .insert(installation.id, &imported[0])
        .await
        .unwrap();
    let meta = database
        .beatmap_metadata()
        .get_or_insert(&metadata("Before queue"))
        .await
        .unwrap();
    let audio = database
        .audio_sources()
        .get_or_insert(&SourceType::Local("/queue/existing".into()))
        .await
        .unwrap();
    let map = insert_legacy_beatmap(
        database,
        set.id,
        &imported[0].beatmaps[0],
        Some(meta.hash),
        Some(audio.id),
        Some("/queue/cover".into()),
    )
    .await
    .unwrap();
    let other = Database::connect(url).await.unwrap();
    let (first, second) = tokio::join!(database.migrate(), other.migrate());
    first.unwrap();
    second.unwrap();
    assert_eq!(database.beatmaps().get(map.id).await.unwrap(), Some(map));
    assert_eq!(
        database.beatmap_sets().get(set.id).await.unwrap(),
        Some(set)
    );
    let empty = QueueState {
        audio_source_ids: vec![],
        playlist_item_ids: vec![],
        current_index: None,
        mode: PlaybackMode::Stopped,
        revision: 0,
        playback_token: 0,
    };
    assert_eq!(database.queue().get().await.unwrap(), empty);
    let state = QueueState {
        audio_source_ids: vec![audio.id, audio.id],
        playlist_item_ids: vec![None, None],
        current_index: Some(1),
        mode: PlaybackMode::Playing,
        revision: 1,
        playback_token: 1,
    };
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    transaction.queue().save(&state).await.unwrap();
    transaction.commit().await.unwrap();
    let reopened = Database::connect(url).await.unwrap();
    reopened.migrate().await.unwrap();
    assert_eq!(reopened.queue().get().await.unwrap(), state);
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    transaction.queue().save(&empty).await.unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(other.queue().get().await.unwrap(), state);
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    transaction.queue().save(&empty).await.unwrap();
    drop(transaction);
    assert_eq!(database.queue().get().await.unwrap(), state);
    let mut invalid = state.clone();
    invalid.current_index = Some(3);
    assert!(database.queue().save(&invalid).await.is_err());
    invalid.current_index = Some(2);
    assert!(database.queue().save(&invalid).await.is_err());
    let mut mismatched = state.clone();
    mismatched.playlist_item_ids.pop();
    for invalid in [
        mismatched,
        QueueState {
            current_index: None,
            ..state.clone()
        },
        QueueState {
            revision: u64::MAX,
            ..state.clone()
        },
        QueueState {
            playback_token: u64::MAX,
            ..state.clone()
        },
        QueueState {
            mode: PlaybackMode::Paused,
            ..empty.clone()
        },
        QueueState {
            current_index: Some(0),
            ..empty.clone()
        },
    ] {
        assert!(
            database.queue().save(&invalid).await.is_err(),
            "{invalid:?}"
        );
        assert_eq!(
            database.queue().get().await.unwrap(),
            state,
            "a rejected save must be atomic"
        );
    }
    database.reset().await.unwrap();
    assert_eq!(database.queue().get().await.unwrap(), empty);
    drop_application_tables(database).await;
}
