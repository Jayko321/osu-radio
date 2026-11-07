use super::*;
use crate::model::AudioSettings;
use sea_orm_migration::MigratorTrait;

#[allow(clippy::too_many_lines)] // One populated upgrade, snapshot, rollback and identity scenario.
pub(super) async fn migration_contracts(database: &Database, url: &str) {
    crate::migrations::Migrator::up(&database.connection, Some(7))
        .await
        .unwrap();
    let source = SourceType::Local("{\"Unix\":[255,47,97]}".into());
    let audio = database
        .audio_sources()
        .get_or_insert(&source)
        .await
        .unwrap();
    database
        .listening_history()
        .record_started(&source, 8, 1234)
        .await
        .unwrap();
    let other = Database::connect(url).await.unwrap();
    let (one, two) = tokio::join!(database.migrate(), other.migrate());
    one.unwrap();
    two.unwrap();
    assert_eq!(
        database.user_data().audio_settings().await.unwrap(),
        AudioSettings::default()
    );
    assert_eq!(
        database.audio_sources().get(audio.id).await.unwrap(),
        Some(audio.clone())
    );
    assert_eq!(
        database
            .listening_history()
            .for_audio_sources(&[audio.id])
            .await
            .unwrap()
            .get(&audio.id),
        Some(&1234)
    );
    assert!(
        database
            .audio_volume()
            .for_audio_sources(&[audio.id])
            .await
            .unwrap()
            .is_empty()
    );
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    transaction
        .user_data()
        .update_audio_settings(Some(true), Some(20))
        .await
        .unwrap();
    transaction.audio_volume().set(&source, 10).await.unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(
        other.user_data().audio_settings().await.unwrap(),
        AudioSettings {
            individual_volume_enabled: true,
            global_volume_percent: 20
        }
    );
    assert_eq!(
        other
            .audio_volume()
            .for_audio_sources(&vec![audio.id; 1002])
            .await
            .unwrap()
            .get(&audio.id),
        Some(&10)
    );
    let reader = database.begin_read().await.unwrap();
    assert_eq!(
        reader
            .user_data()
            .audio_settings()
            .await
            .unwrap()
            .global_volume_percent,
        20
    );
    other.audio_volume().set(&source, 30).await.unwrap();
    assert_eq!(
        reader
            .audio_volume()
            .for_audio_sources(&[audio.id])
            .await
            .unwrap()
            .get(&audio.id),
        Some(&10)
    );
    reader.commit().await.unwrap();
    let transaction = database.begin().await.unwrap();
    transaction.audio_volume().set(&source, 80).await.unwrap();
    transaction
        .user_data()
        .update_audio_settings(None, Some(40))
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(
        other
            .audio_volume()
            .for_audio_sources(&[audio.id])
            .await
            .unwrap()
            .get(&audio.id),
        Some(&30)
    );
    assert_eq!(
        other
            .user_data()
            .audio_settings()
            .await
            .unwrap()
            .global_volume_percent,
        20
    );
    for percent in [0, 100] {
        database.audio_volume().set(&source, percent).await.unwrap();
        database
            .user_data()
            .update_audio_settings(None, Some(percent))
            .await
            .unwrap();
    }
    assert!(database.audio_volume().set(&source, 101).await.is_err());
    assert!(
        database
            .user_data()
            .update_audio_settings(Some(false), Some(101))
            .await
            .is_err()
    );
    assert!(
        database
            .connection
            .execute_unprepared("UPDATE user_data SET global_volume_percent = -1")
            .await
            .is_err()
    );
    assert!(
        database
            .connection
            .execute_unprepared("UPDATE audio_volume SET volume_percent = 101")
            .await
            .is_err()
    );
    database.audio_sources().cleanup().await.unwrap();
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
    assert_ne!(audio.id, returned.id);
    let volumes = other
        .audio_volume()
        .for_audio_sources(&[copied.id, returned.id])
        .await
        .unwrap();
    assert_eq!(volumes.get(&returned.id), Some(&100));
    assert!(!volumes.contains_key(&copied.id));
    other.audio_volume().delete(&source).await.unwrap();
    assert!(
        database
            .audio_volume()
            .for_audio_sources(&[returned.id])
            .await
            .unwrap()
            .is_empty()
    );
    database.audio_volume().set(&source, 10).await.unwrap();
    database.reset().await.unwrap();
    assert_eq!(
        database.user_data().audio_settings().await.unwrap(),
        AudioSettings::default()
    );
    assert!(
        database
            .audio_volume()
            .for_audio_sources(&[])
            .await
            .unwrap()
            .is_empty()
    );
    drop_application_tables(database).await;
}
