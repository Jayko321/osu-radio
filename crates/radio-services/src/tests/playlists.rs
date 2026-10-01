use super::*;
use crate::{PlaybackCommand, PlaybackMode, PlaylistError};

pub(super) async fn cover_contracts(database: &TestDatabase, other: &TestDatabase, url: &str) {
    let playlist = database.playlists().create("Cover").await.unwrap();
    assert_eq!(playlist.item_count, 0);
    assert_eq!(playlist.custom_cover_revision, None);
    let png = cover_png(512, 512);
    let source = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(source.path(), &png).unwrap();
    let saved = database
        .playlists()
        .set_cover(playlist.id, &std::fs::read(source.path()).unwrap())
        .await
        .unwrap();
    source.close().unwrap();
    assert_eq!(saved.custom_cover_revision, Some(1));
    assert_eq!(
        other.playlists().cover(playlist.id).await.unwrap(),
        Some(png.clone())
    );
    let reopened = Services::connect(url).await.unwrap();
    reopened.migrate().await.unwrap();
    assert_eq!(
        reopened.playlists().cover(playlist.id).await.unwrap(),
        Some(png.clone())
    );
    assert_eq!(
        reopened
            .playlists()
            .all()
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.id == playlist.id),
        Some(saved)
    );
    let queue = database.queue().get().await.unwrap();
    for invalid in [
        b"broken PNG".to_vec(),
        png[..png.len() / 2].to_vec(),
        cover_png(511, 512),
        cover_png(513, 512),
        vec![0; crate::MAX_PLAYLIST_COVER_BYTES + 1],
    ] {
        assert!(
            database
                .playlists()
                .set_cover(playlist.id, &invalid)
                .await
                .is_err()
        );
        assert_eq!(
            database.playlists().cover(playlist.id).await.unwrap(),
            Some(png.clone())
        );
    }
    assert_eq!(database.queue().get().await.unwrap(), queue);
    database.playlists().clear_cover(playlist.id).await.unwrap();
    database.playlists().clear_cover(playlist.id).await.unwrap();
    assert_eq!(database.playlists().cover(playlist.id).await.unwrap(), None);
    assert_eq!(
        database
            .playlists()
            .rename(playlist.id, "Renamed cover")
            .await
            .unwrap()
            .custom_cover_revision,
        None
    );
    assert_eq!(
        database
            .playlists()
            .set_cover(playlist.id, &png)
            .await
            .unwrap()
            .custom_cover_revision,
        Some(2)
    );
    database.playlists().delete(playlist.id).await.unwrap();
    assert_eq!(database.playlists().cover(playlist.id).await.unwrap(), None);
    assert!(
        database
            .playlists()
            .set_cover(playlist.id, &png)
            .await
            .is_err()
    );
    assert!(database.playlists().clear_cover(playlist.id).await.is_err());
}

fn cover_png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(width, height)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

#[allow(clippy::too_many_lines)] // One scenario verifies persistence and playback across snapshot changes.
pub(super) async fn contracts(database: &TestDatabase, other: &TestDatabase, url: &str) {
    let installation = register(database, "playlist").await;
    let mut imported = snapshot("Playlist song", "/playlist/shared");
    for map in &mut imported[0].beatmaps {
        map.hash = map.hash.as_ref().map(|hash| format!("playlist-{hash}"));
    }
    database
        .osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let set = database
        .beatmap_sets()
        .for_installation(installation)
        .await
        .unwrap()
        .remove(0);
    let maps = database.beatmaps().for_set(set.id).await.unwrap();
    let ids = maps.iter().map(|map| map.id).collect::<Vec<_>>();
    assert!(database.playlists().create(" \n\t ").await.is_err());
    let playlist = database.playlists().create("  Favourites  ").await.unwrap();
    assert_eq!(playlist.name, "Favourites");
    let duplicate_name = database.playlists().create("Favourites").await.unwrap();
    assert_ne!(duplicate_name.id, playlist.id);
    assert!(database.playlists().rename(playlist.id, " ").await.is_err());
    assert_eq!(
        database
            .playlists()
            .rename(playlist.id, " Evening ")
            .await
            .unwrap()
            .name,
        "Evening"
    );
    let error = database
        .playlists()
        .add_items(playlist.id, &[ids[0], i32::MAX])
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<PlaylistError>(),
        Some(&PlaylistError::InvalidBeatmap(i32::MAX))
    );
    assert!(
        database
            .playlists()
            .get(playlist.id)
            .await
            .unwrap()
            .unwrap()
            .items
            .is_empty()
    );
    let first_service = database.playlists();
    let second_service = other.playlists();
    let (first, second) = tokio::join!(
        first_service.add_items(playlist.id, &ids),
        second_service.add_items(playlist.id, &ids)
    );
    let populated = first.unwrap();
    assert_eq!(second.unwrap(), populated);
    assert_eq!(populated.items.len(), 2);
    let summary = database
        .playlists()
        .all()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == playlist.id)
        .unwrap();
    assert_eq!(summary.item_count, 2);
    assert_eq!(
        summary.cover_beatmap_id,
        populated.items[0].cover_beatmap_id
    );
    assert_eq!(
        database
            .playlists()
            .rename(playlist.id, "Evening")
            .await
            .unwrap(),
        summary
    );
    assert_eq!(
        populated
            .items
            .iter()
            .map(|item| item.beatmap_id.unwrap())
            .collect::<Vec<_>>(),
        ids
    );
    assert_eq!(
        populated.items[0].audio_source_id,
        populated.items[1].audio_source_id
    );
    assert_eq!(populated.items[0].cover_beatmap_id, Some(ids[0]));
    assert_eq!(
        populated.items[0].cover_beatmap_id, populated.items[1].cover_beatmap_id,
        "shared audio uses the same representative cover as the library"
    );
    let reopened = Services::connect(url).await.unwrap();
    reopened.migrate().await.unwrap();
    assert_eq!(
        reopened.playlists().get(playlist.id).await.unwrap(),
        Some(populated.clone())
    );
    let audio = populated.items[0].audio_source_id.unwrap();
    database
        .queue()
        .append(vec![audio, audio, audio])
        .await
        .unwrap();
    let first = database.playlists().play(playlist.id, None).await.unwrap();
    assert_eq!(first.current_playlist_item_id, Some(populated.items[0].id));
    assert_eq!(
        database.queue().get().await.unwrap().audio_source_ids,
        [audio, audio]
    );
    let second = database
        .queue()
        .command(PlaybackCommand::Next)
        .await
        .unwrap();
    assert_eq!(second.current_playlist_item_id, Some(populated.items[1].id));
    assert_eq!(
        second.current_audio_source_id,
        first.current_audio_source_id
    );
    assert!(second.playback_token > first.playback_token);
    assert_eq!(
        second.track.as_ref().unwrap().difficulties[0]
            .difficulty_name
            .as_deref(),
        Some("Hard")
    );
    let stale = database
        .queue()
        .command(PlaybackCommand::Finished {
            playback_token: first.playback_token,
        })
        .await
        .unwrap();
    assert_eq!(stale, second);
    let previous = database
        .queue()
        .command(PlaybackCommand::Previous)
        .await
        .unwrap();
    assert_eq!(
        previous.current_playlist_item_id,
        first.current_playlist_item_id
    );
    let selected = database
        .playlists()
        .play(playlist.id, Some(populated.items[1].id))
        .await
        .unwrap();
    assert_eq!(
        selected.current_playlist_item_id,
        second.current_playlist_item_id
    );
    let paused = database
        .queue()
        .command(PlaybackCommand::Pause)
        .await
        .unwrap();
    let resumed = database
        .queue()
        .command(PlaybackCommand::Play {
            audio_source_id: None,
        })
        .await
        .unwrap();
    assert_eq!(paused.mode, PlaybackMode::Paused);
    assert_eq!(resumed.playback_token, selected.playback_token);
    assert_eq!(
        resumed.current_playlist_item_id,
        selected.current_playlist_item_id
    );
    assert_eq!(reopened.queue().playback().await.unwrap(), resumed);
    reopened.queue().recover().await.unwrap();
    let restored = reopened.queue().playback().await.unwrap();
    assert_eq!(restored.mode, PlaybackMode::Paused);
    assert_eq!(
        restored.current_playlist_item_id,
        resumed.current_playlist_item_id
    );
    assert!(restored.playback_token > resumed.playback_token);
    assert_eq!(
        database
            .queue()
            .command(PlaybackCommand::Finished {
                playback_token: resumed.playback_token,
            })
            .await
            .unwrap(),
        restored,
        "recovery invalidates pre-restart playlist callbacks"
    );
    let before = database.queue().get().await.unwrap();
    assert!(
        database
            .playlists()
            .play(duplicate_name.id, None)
            .await
            .is_err()
    );
    assert!(
        database
            .playlists()
            .play(playlist.id, Some(i32::MAX))
            .await
            .is_err()
    );
    assert_eq!(database.queue().get().await.unwrap(), before);
    database
        .playlists()
        .remove_item(playlist.id, populated.items[1].id)
        .await
        .unwrap();
    assert_eq!(
        database.queue().get().await.unwrap(),
        before,
        "editing retains the queue snapshot"
    );
    database
        .playlists()
        .add_items(playlist.id, &[ids[1]])
        .await
        .unwrap();
    let populated = database
        .playlists()
        .get(playlist.id)
        .await
        .unwrap()
        .unwrap();
    database
        .osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let reimported = database
        .playlists()
        .get(playlist.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        reimported
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        populated
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>()
    );
    assert_ne!(
        reimported.items[0].beatmap_id,
        populated.items[0].beatmap_id
    );
    database
        .osu_installations()
        .delete(installation)
        .await
        .unwrap();
    let unavailable = database
        .playlists()
        .get(playlist.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unavailable.items.len(), 2);
    let summary = database
        .playlists()
        .all()
        .await
        .unwrap()
        .into_iter()
        .find(|row| row.id == playlist.id)
        .unwrap();
    assert_eq!(summary.item_count, 2);
    assert_eq!(summary.cover_beatmap_id, None);
    assert!(
        unavailable
            .items
            .iter()
            .all(|item| item.beatmap_id.is_none() && item.audio_source_id.is_none())
    );
    assert_eq!(unavailable.items[0].title.as_deref(), Some("Playlist song"));
    let before = database.queue().get().await.unwrap();
    assert!(database.playlists().play(playlist.id, None).await.is_err());
    assert_eq!(database.queue().get().await.unwrap(), before);
    let returned = register(database, "playlist-returned").await;
    database
        .osu_installations()
        .replace_snapshot(returned, &imported)
        .await
        .unwrap();
    assert!(
        database
            .playlists()
            .get(playlist.id)
            .await
            .unwrap()
            .unwrap()
            .items
            .iter()
            .all(|item| item.audio_source_id.is_some())
    );
    let mut no_hash = imported.clone();
    no_hash[0].beatmaps[0].hash = None;
    database
        .osu_installations()
        .replace_snapshot(returned, &no_hash)
        .await
        .unwrap();
    let set = database
        .beatmap_sets()
        .for_installation(returned)
        .await
        .unwrap()
        .remove(0);
    let missing_hash = database.beatmaps().for_set(set.id).await.unwrap()[0].id;
    let error = database
        .playlists()
        .add_items(duplicate_name.id, &[missing_hash])
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<PlaylistError>(),
        Some(&PlaylistError::MissingHash(missing_hash))
    );
    database.playlists().delete(playlist.id).await.unwrap();
    assert!(
        database
            .playlists()
            .get(playlist.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        database
            .playlists()
            .remove_item(duplicate_name.id, populated.items[0].id)
            .await
            .is_err()
    );
    database
        .playlists()
        .delete(duplicate_name.id)
        .await
        .unwrap();
    database.osu_installations().delete(returned).await.unwrap();
    database.queue().clear().await.unwrap();
    copy_resolution_and_batches(database).await;
}

#[allow(clippy::too_many_lines)] // Test stable source keys, copy priority and query chunks in one isolated fixture.
async fn copy_resolution_and_batches(database: &TestDatabase) {
    let mut imported = snapshot("Copies", "/playlist-copies/first");
    imported[0].beatmaps.truncate(1);
    imported[0].beatmaps[0].hash = Some("playlist-copy-'_%".into());
    let mut missing = imported.clone();
    missing[0].files.clear();
    let unavailable = register(database, "playlist-copy-unavailable").await;
    database
        .osu_installations()
        .replace_snapshot(unavailable, &missing)
        .await
        .unwrap();
    let set = database
        .beatmap_sets()
        .for_installation(unavailable)
        .await
        .unwrap()
        .remove(0);
    let missing_id = database.beatmaps().for_set(set.id).await.unwrap()[0].id;
    let playlist = database.playlists().create("Copies").await.unwrap();
    assert!(
        database
            .playlists()
            .add_items(playlist.id, &[missing_id])
            .await
            .unwrap()
            .items[0]
            .audio_source_id
            .is_none()
    );
    let first = register(database, "playlist-copy-first").await;
    database
        .osu_installations()
        .replace_snapshot(first, &imported)
        .await
        .unwrap();
    let set = database
        .beatmap_sets()
        .for_installation(first)
        .await
        .unwrap()
        .remove(0);
    let first_map = database.beatmaps().for_set(set.id).await.unwrap().remove(0);
    let second = register(database, "playlist-copy-second").await;
    let second_snapshot = snapshot_with_location(&imported, "/playlist-copies/second");
    database
        .osu_installations()
        .replace_snapshot(second, &second_snapshot)
        .await
        .unwrap();
    let set = database
        .beatmap_sets()
        .for_installation(second)
        .await
        .unwrap()
        .remove(0);
    let second_map = database.beatmaps().for_set(set.id).await.unwrap().remove(0);
    assert_eq!(
        database
            .playlists()
            .get(playlist.id)
            .await
            .unwrap()
            .unwrap()
            .items[0]
            .beatmap_id,
        Some(first_map.id)
    );
    sql(
        database,
        &format!(
            "UPDATE audio_sources SET kind = 'online' WHERE id = {}",
            first_map.audio_source_id.unwrap()
        ),
    )
    .await;
    assert_eq!(
        database
            .playlists()
            .get(playlist.id)
            .await
            .unwrap()
            .unwrap()
            .items[0]
            .beatmap_id,
        Some(second_map.id)
    );
    sql(
        database,
        &format!(
            "UPDATE audio_sources SET kind = 'local' WHERE id = {}",
            first_map.audio_source_id.unwrap()
        ),
    )
    .await;
    let stable = database
        .osu_installations()
        .register(
            &OsuMarker {
                kind: OsuKind::Stable,
                root_path: "/test/playlist-stable-copy".into(),
                marker_path: "/test/playlist-stable-copy/osu!.db".into(),
            },
            None,
        )
        .await
        .unwrap()
        .into_installation();
    let mut stable_snapshot = snapshot_with_location(&imported, "/playlist-copies/stable");
    stable_snapshot[0].source = OsuKind::Stable;
    database
        .osu_installations()
        .replace_snapshot(stable.id, &stable_snapshot)
        .await
        .unwrap();
    let set = database
        .beatmap_sets()
        .for_installation(stable.id)
        .await
        .unwrap()
        .remove(0);
    let stable_id = database.beatmaps().for_set(set.id).await.unwrap()[0].id;
    let mixed = database
        .playlists()
        .add_items(playlist.id, &[stable_id, first_map.id, second_map.id])
        .await
        .unwrap();
    assert_eq!(
        mixed.items.len(),
        2,
        "same hash in distinct source kinds stays distinct"
    );
    database.osu_installations().delete(first).await.unwrap();
    database.osu_installations().delete(second).await.unwrap();
    let mixed = database
        .playlists()
        .get(playlist.id)
        .await
        .unwrap()
        .unwrap();
    assert!(mixed.items[0].audio_source_id.is_none());
    let skipped = database.playlists().play(playlist.id, None).await.unwrap();
    assert_eq!(skipped.current_playlist_item_id, Some(mixed.items[1].id));
    let before = database.queue().get().await.unwrap();
    assert!(
        database
            .playlists()
            .play(playlist.id, Some(mixed.items[0].id))
            .await
            .is_err()
    );
    assert_eq!(database.queue().get().await.unwrap(), before);
    database.playlists().delete(playlist.id).await.unwrap();
    for installation in [unavailable, stable.id] {
        database
            .osu_installations()
            .delete(installation)
            .await
            .unwrap();
    }
    let large = register(database, "playlist-batches").await;
    let mut batch = snapshot("Batches", "/playlist-batches/audio");
    batch[0].beatmaps = (0..503)
        .map(|index| ImportedBeatmap {
            difficulty_name: Some(format!("Diff-{index}")),
            hash: Some(format!("playlist-batch-{index}'_%")),
            ..batch[0].beatmaps[0].clone()
        })
        .collect();
    database
        .osu_installations()
        .replace_snapshot(large, &batch)
        .await
        .unwrap();
    let set = database
        .beatmap_sets()
        .for_installation(large)
        .await
        .unwrap()
        .remove(0);
    let ids: Vec<_> = database
        .beatmaps()
        .for_set(set.id)
        .await
        .unwrap()
        .into_iter()
        .rev()
        .map(|map| map.id)
        .collect();
    let playlist = database.playlists().create("Large").await.unwrap();
    let saved = database
        .playlists()
        .add_items(playlist.id, &ids)
        .await
        .unwrap();
    assert_eq!(saved.items.len(), 503);
    assert_eq!(
        saved
            .items
            .iter()
            .map(|item| item.beatmap_id.unwrap())
            .collect::<Vec<_>>(),
        ids
    );
    let last = saved.items.last().unwrap().id;
    assert_eq!(
        database
            .playlists()
            .play(playlist.id, Some(last))
            .await
            .unwrap()
            .current_playlist_item_id,
        Some(last)
    );
    assert_eq!(
        database
            .queue()
            .get()
            .await
            .unwrap()
            .playlist_item_ids
            .len(),
        503
    );
    database.playlists().delete(playlist.id).await.unwrap();
    database.osu_installations().delete(large).await.unwrap();
    database.queue().clear().await.unwrap();
}

fn snapshot_with_location(imported: &[ImportedBeatmapSet], path: &str) -> Vec<ImportedBeatmapSet> {
    let mut snapshot = imported.to_vec();
    snapshot[0].files[0].file.as_mut().unwrap().resolved_path = Some(path.into());
    snapshot
}
