use super::*;
use crate::{PlaybackAssignment, PlaybackCommand as Command, PlaybackMode as Mode};

async fn started(database: &Services, token: u64) -> PlaybackAssignment {
    database
        .queue()
        .command(Command::Started {
            playback_token: token,
        })
        .await
        .unwrap()
}

#[allow(clippy::too_many_lines)] // End-to-end acknowledgement, projections and source lifetime share one persisted scenario.
pub(super) async fn contracts(database: &TestDatabase, other: &TestDatabase, url: &str) {
    let installation = register(database, "listening-history").await;
    let imported = snapshot("History song", "/history/source");
    database
        .osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let track = database
        .beatmap_sets()
        .search_tracks("History song")
        .await
        .unwrap()
        .remove(0);
    let audio = track.audio_source_id;
    assert_eq!(track.last_played_at_ms, None);
    let playlist = database
        .playlists()
        .create("History playlist")
        .await
        .unwrap();
    let maps = track
        .difficulties
        .iter()
        .map(|map| map.beatmap_id)
        .collect::<Vec<_>>();
    let playlist = database
        .playlists()
        .add_items(playlist.id, &maps)
        .await
        .unwrap();
    assert!(
        playlist
            .items
            .iter()
            .all(|item| item.last_played_at_ms.is_none())
    );
    let launch = database.playlists().play(playlist.id, None).await.unwrap();
    assert_eq!(
        launch.track.as_ref().unwrap().last_played_at_ms,
        None,
        "assignment alone never counts a listen"
    );
    assert_eq!(
        started(database, launch.playback_token.checked_add(1).unwrap()).await,
        launch
    );
    let paused = database.queue().command(Command::Pause).await.unwrap();
    let left = database.queue();
    let right = other.queue();
    let acknowledgment = Command::Started {
        playback_token: launch.playback_token,
    };
    let (one, two) = tokio::join!(left.command(acknowledgment), right.command(acknowledgment));
    let accounted = one.unwrap();
    assert_eq!(two.unwrap(), accounted);
    assert_eq!(
        accounted.mode,
        Mode::Paused,
        "delayed acknowledgement can arrive after pause"
    );
    assert_eq!(accounted.playback_token, launch.playback_token);
    assert_eq!(accounted.revision, paused.revision.checked_add(1).unwrap());
    let timestamp = accounted.track.as_ref().unwrap().last_played_at_ms.unwrap();
    assert!(timestamp > 0);
    assert_eq!(started(database, launch.playback_token).await, accounted);
    let tracks = database
        .beatmap_sets()
        .search_tracks("History")
        .await
        .unwrap();
    assert_eq!(tracks[0].last_played_at_ms, Some(timestamp));
    let saved = database
        .playlists()
        .get(playlist.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        saved
            .items
            .iter()
            .all(|item| item.last_played_at_ms == Some(timestamp)),
        "all difficulties sharing audio receive its date"
    );
    assert!(
        database
            .playlists()
            .add_items(playlist.id, &maps)
            .await
            .unwrap()
            .items
            .iter()
            .all(|item| item.last_played_at_ms == Some(timestamp))
    );
    let resumed = database
        .queue()
        .command(Command::Play {
            audio_source_id: None,
        })
        .await
        .unwrap();
    assert_eq!(
        started(database, resumed.playback_token).await,
        resumed,
        "pause/resume retains the accounted launch"
    );
    let repeated = database.queue().command(Command::Next).await.unwrap();
    assert_eq!(repeated.current_audio_source_id, Some(audio));
    assert_ne!(repeated.playback_token, launch.playback_token);
    assert_eq!(started(database, launch.playback_token).await, repeated);
    let repeated_started = started(database, repeated.playback_token).await;
    assert_eq!(
        repeated_started.revision,
        repeated.revision.checked_add(1).unwrap()
    );
    let latest = repeated_started
        .track
        .as_ref()
        .unwrap()
        .last_played_at_ms
        .unwrap();
    assert!(latest >= timestamp);
    let stopped = database.queue().command(Command::Stop).await.unwrap();
    assert_eq!(started(database, stopped.playback_token).await, stopped);
    let reopened = Services::connect(url).await.unwrap();
    reopened.migrate().await.unwrap();
    assert_eq!(reopened.queue().playback().await.unwrap(), stopped);
    database.queue().clear().await.unwrap();
    assert_eq!(
        database
            .listening_history()
            .for_audio_sources(&[audio])
            .await
            .unwrap()
            .get(&audio),
        Some(&latest),
        "queue clear retains listening history"
    );
    database
        .osu_installations()
        .replace_snapshot(installation, &[])
        .await
        .unwrap();
    assert!(database.audio_sources().get(audio).await.unwrap().is_none());
    let unavailable = database
        .playlists()
        .get(playlist.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        unavailable
            .items
            .iter()
            .all(|item| item.last_played_at_ms.is_none())
    );
    database
        .osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let returned = database
        .beatmap_sets()
        .search_tracks("History song")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(returned.last_played_at_ms, Some(latest));
    assert!(
        database
            .playlists()
            .get(playlist.id)
            .await
            .unwrap()
            .unwrap()
            .items
            .iter()
            .all(|item| item.last_played_at_ms == Some(latest))
    );
    let removed_launch = database
        .queue()
        .append(vec![returned.audio_source_id])
        .await
        .unwrap();
    database
        .osu_installations()
        .delete(installation)
        .await
        .unwrap();
    let missing = database.queue().playback().await.unwrap();
    assert_eq!(
        started(database, removed_launch.playback_token).await,
        missing,
        "removed current source cannot acknowledge playback"
    );
    let reinstallation = register(database, "listening-history-return").await;
    database
        .osu_installations()
        .replace_snapshot(reinstallation, &imported)
        .await
        .unwrap();
    assert_eq!(
        database
            .beatmap_sets()
            .search_tracks("History song")
            .await
            .unwrap()[0]
            .last_played_at_ms,
        Some(latest)
    );
    let relocated = snapshot("History song", "/history/relocated");
    database
        .osu_installations()
        .replace_snapshot(reinstallation, &relocated)
        .await
        .unwrap();
    assert_eq!(
        database
            .beatmap_sets()
            .search_tracks("History song")
            .await
            .unwrap()[0]
            .last_played_at_ms,
        None,
        "relocated source has a distinct identity"
    );
    database.queue().clear().await.unwrap();
    let unplayed = database
        .beatmap_sets()
        .search_tracks("History song")
        .await
        .unwrap()
        .remove(0);
    let failed_launch = database
        .queue()
        .append(vec![unplayed.audio_source_id])
        .await
        .unwrap();
    database
        .queue()
        .command(Command::Failed {
            playback_token: failed_launch.playback_token,
        })
        .await
        .unwrap();
    assert_eq!(
        database
            .beatmap_sets()
            .search_tracks("History song")
            .await
            .unwrap()[0]
            .last_played_at_ms,
        None,
        "failed download or decode without Started never counts a listen"
    );
    database.reset().await.unwrap();
    let reset_installation = register(database, "listening-history-reset").await;
    database
        .osu_installations()
        .replace_snapshot(reset_installation, &imported)
        .await
        .unwrap();
    assert_eq!(
        database
            .beatmap_sets()
            .search_tracks("History song")
            .await
            .unwrap()[0]
            .last_played_at_ms,
        None
    );
    database
        .osu_installations()
        .delete(reset_installation)
        .await
        .unwrap();
}
