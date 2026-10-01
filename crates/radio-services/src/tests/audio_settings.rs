use super::*;
use crate::{AudioSettings, AudioSettingsError};

#[allow(clippy::too_many_lines)] // One durable scenario covers projections, reimport identity and settings retention.
pub(super) async fn contracts(database: &TestDatabase, other: &TestDatabase, url: &str) {
    let installation = register(database, "audio-settings").await;
    let imported = snapshot("Volume song", "/volume/source");
    database
        .osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let track = database
        .beatmap_sets()
        .search_tracks("Volume song")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(track.volume_percent, None);
    assert_eq!(
        database.audio_settings().get().await.unwrap(),
        AudioSettings::default()
    );
    let settings = database
        .audio_settings()
        .update(Some(true), Some(20))
        .await
        .unwrap();
    assert!(settings.individual_volume_enabled);
    database
        .audio_settings()
        .set_volume(track.audio_source_id, Some(10))
        .await
        .unwrap();
    let maps = track
        .difficulties
        .iter()
        .map(|d| d.beatmap_id)
        .collect::<Vec<_>>();
    let playlist = database
        .playlists()
        .create("Volume playlist")
        .await
        .unwrap();
    let playlist = database
        .playlists()
        .add_items(playlist.id, &maps)
        .await
        .unwrap();
    assert!(playlist.items.iter().all(|i| i.volume_percent == Some(10)));
    let duplicate = database.playlists().create("Shared audio").await.unwrap();
    let duplicate = database
        .playlists()
        .add_items(duplicate.id, &maps)
        .await
        .unwrap();
    assert!(duplicate.items.iter().all(|i| i.volume_percent == Some(10)));
    let launch = database.playlists().play(playlist.id, None).await.unwrap();
    assert_eq!(launch.track.unwrap().volume_percent, Some(10));
    database
        .audio_settings()
        .update(None, Some(40))
        .await
        .unwrap();
    assert_eq!(
        other
            .queue()
            .playback()
            .await
            .unwrap()
            .track
            .unwrap()
            .volume_percent,
        Some(10)
    );
    database
        .audio_settings()
        .update(Some(false), None)
        .await
        .unwrap();
    assert_eq!(
        other
            .audio_settings()
            .get()
            .await
            .unwrap()
            .global_volume_percent,
        40
    );
    database
        .audio_settings()
        .update(Some(true), None)
        .await
        .unwrap();
    assert_eq!(
        other
            .beatmap_sets()
            .search_tracks("Volume song")
            .await
            .unwrap()[0]
            .volume_percent,
        Some(10)
    );
    let reopened = Services::connect(url).await.unwrap();
    reopened.migrate().await.unwrap();
    assert_eq!(
        reopened.audio_settings().get().await.unwrap(),
        AudioSettings {
            individual_volume_enabled: true,
            global_volume_percent: 40
        }
    );
    database
        .osu_installations()
        .replace_snapshot(installation, &[])
        .await
        .unwrap();
    assert!(
        database
            .audio_sources()
            .get(track.audio_source_id)
            .await
            .unwrap()
            .is_none()
    );
    let unavailable = database
        .playlists()
        .get(playlist.id)
        .await
        .unwrap()
        .unwrap();
    assert!(unavailable.items.iter().all(|i| i.volume_percent.is_none()));
    database
        .osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let returned = database
        .beatmap_sets()
        .search_tracks("Volume song")
        .await
        .unwrap()
        .remove(0);
    assert_ne!(returned.audio_source_id, track.audio_source_id);
    assert_eq!(returned.volume_percent, Some(10));
    assert!(
        database
            .playlists()
            .get(playlist.id)
            .await
            .unwrap()
            .unwrap()
            .items
            .iter()
            .all(|i| i.volume_percent == Some(10))
    );
    database
        .audio_settings()
        .set_volume(returned.audio_source_id, None)
        .await
        .unwrap();
    assert_eq!(
        other
            .beatmap_sets()
            .search_tracks("Volume song")
            .await
            .unwrap()[0]
            .volume_percent,
        None
    );
    for percent in [0, 100] {
        database
            .audio_settings()
            .set_volume(returned.audio_source_id, Some(percent))
            .await
            .unwrap();
    }
    let before = database.audio_settings().get().await.unwrap();
    let error = database
        .audio_settings()
        .update(Some(false), Some(101))
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref(),
        Some(&AudioSettingsError::InvalidPercent)
    );
    assert_eq!(database.audio_settings().get().await.unwrap(), before);
    assert!(
        database
            .audio_settings()
            .set_volume(returned.audio_source_id, Some(101))
            .await
            .is_err()
    );
    let error = database
        .audio_settings()
        .set_volume(i32::MAX, Some(20))
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref(),
        Some(&AudioSettingsError::UnknownAudio(i32::MAX))
    );
    database
        .osu_installations()
        .delete(installation)
        .await
        .unwrap();
}
