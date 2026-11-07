use super::*;
use crate::model::{Beatmap, BeatmapDetails, BeatmapMetadata, BeatmapSet, BeatmapSetWithAudio};

#[allow(clippy::too_many_lines)] // One rolled-back fixture checks persistence and every library projection together.
pub(super) async fn contracts(database: &Database) {
    let transaction = database.begin().await.unwrap();
    transaction.user_data().lock().await.unwrap();
    let installation = transaction
        .osu_installations()
        .register(&marker("model-projections"), Some("Projection fixture"))
        .await
        .unwrap()
        .into_installation();
    let mut imported = snapshot("Projection title", "/projection/audio.mp3").remove(0);
    let metadata = imported.beatmaps[0].metadata.as_mut().unwrap();
    metadata.artist_unicode = Some("演奏者".into());
    metadata.source = Some("Projection source".into());
    let hash = crate::metadata_hash(metadata).unwrap();
    let expected_metadata = BeatmapMetadata {
        hash: hash.clone(),
        title: Some("Projection title".into()),
        title_unicode: Some("曲".into()),
        artist: Some("Artist".into()),
        artist_unicode: Some("演奏者".into()),
        author: Some(serde_json::json!({"online_id":7,"username":"Mapper","country_code":"JP"})),
        source: Some("Projection source".into()),
        preview_time: Some(1200),
        audio_file: Some("audio.mp3".into()),
        background_file: Some("bg.jpg".into()),
    };
    assert_eq!(
        transaction
            .beatmap_metadata()
            .get_or_insert(metadata)
            .await
            .unwrap(),
        expected_metadata
    );
    assert_eq!(
        transaction.beatmap_metadata().get(&hash).await.unwrap(),
        Some(expected_metadata)
    );
    let audio = transaction
        .audio_sources()
        .get_or_insert(&SourceType::Local("/projection/audio.mp3".into()))
        .await
        .unwrap();
    let other_audio = transaction
        .audio_sources()
        .get_or_insert(&SourceType::Copied("/projection/second.ogg".into()))
        .await
        .unwrap();
    let empty_set = transaction
        .beatmap_sets()
        .insert(
            installation.id,
            &ImportedBeatmapSet {
                source: OsuKind::Lazer,
                online_id: None,
                hash: None,
                files: vec![],
                beatmaps: vec![],
            },
        )
        .await
        .unwrap();
    let expected_empty_set = BeatmapSet {
        id: empty_set.id,
        installation_id: installation.id,
        online_id: None,
        hash: None,
    };
    assert_eq!(empty_set, expected_empty_set);
    let set = transaction
        .beatmap_sets()
        .insert(installation.id, &imported)
        .await
        .unwrap();
    let expected_set = BeatmapSet {
        id: set.id,
        online_id: Some(23),
        hash: Some("set-Projection title".into()),
        installation_id: installation.id,
    };
    assert_eq!(set, expected_set);
    assert_eq!(
        transaction.beatmap_sets().get(set.id).await.unwrap(),
        Some(expected_set.clone())
    );
    assert_eq!(
        transaction
            .beatmap_sets()
            .for_installation(installation.id)
            .await
            .unwrap(),
        [expected_empty_set.clone(), expected_set.clone()]
    );
    imported.beatmaps[0].md5_hash = Some("abcdef0123456789abcdef0123456789".into());
    let map = transaction
        .beatmaps()
        .insert(
            set.id,
            &imported.beatmaps[0],
            Some(hash.clone()),
            Some(audio.id),
            Some("/projection/cover.jpg".into()),
        )
        .await
        .unwrap();
    let expected_map = Beatmap {
        id: map.id,
        beatmap_set_id: set.id,
        difficulty_name: Some("Easy".into()),
        bpm: Some(180.5),
        hash: Some("beatmap-Easy".into()),
        md5_hash: Some("abcdef0123456789abcdef0123456789".into()),
        metadata_hash: Some(hash.clone()),
        audio_source_id: Some(audio.id),
        background_path: Some("/projection/cover.jpg".into()),
    };
    assert_eq!(map, expected_map);
    assert_eq!(
        transaction.beatmaps().get(map.id).await.unwrap(),
        Some(expected_map.clone())
    );
    let empty = ImportedBeatmap {
        difficulty_name: None,
        bpm: None,
        hash: None,
        md5_hash: None,
        metadata: None,
    };
    let second = transaction
        .beatmaps()
        .insert(set.id, &empty, None, Some(other_audio.id), None)
        .await
        .unwrap();
    let silent = transaction
        .beatmaps()
        .insert(set.id, &empty, None, None, None)
        .await
        .unwrap();
    let expected_empty = Beatmap {
        id: silent.id,
        beatmap_set_id: set.id,
        difficulty_name: None,
        bpm: None,
        hash: None,
        md5_hash: None,
        metadata_hash: None,
        audio_source_id: None,
        background_path: None,
    };
    assert_eq!(
        transaction.beatmaps().get(silent.id).await.unwrap(),
        Some(expected_empty.clone())
    );
    assert_eq!(
        transaction.beatmaps().for_set(set.id).await.unwrap(),
        [
            expected_map,
            Beatmap {
                id: second.id,
                audio_source_id: Some(other_audio.id),
                ..expected_empty.clone()
            },
            expected_empty,
        ]
    );
    let expected_detail = BeatmapDetails {
        id: map.id,
        audio_source_id: Some(audio.id),
        difficulty_name: Some("Easy".into()),
        title: Some("Projection title".into()),
        title_unicode: Some("曲".into()),
        artist: Some("Artist".into()),
        artist_unicode: Some("演奏者".into()),
        has_cover: true,
    };
    let empty_detail = BeatmapDetails {
        id: silent.id,
        audio_source_id: None,
        difficulty_name: None,
        title: None,
        title_unicode: None,
        artist: None,
        artist_unicode: None,
        has_cover: false,
    };
    let mut grouped = transaction
        .beatmap_sets()
        .all_with_audio_sources()
        .await
        .unwrap();
    assert_eq!(grouped.len(), 2);
    assert_eq!(
        grouped.remove(0),
        BeatmapSetWithAudio {
            beatmap_set: expected_empty_set,
            audio_sources: vec![],
            beatmaps: vec![],
            has_multiple_audio_sources: false,
        }
    );
    // Unbound-audio NULL ordering differs between SQL backends; compare difficulty IDs.
    grouped[0].beatmaps.sort_by_key(|row| row.id);
    assert_eq!(
        grouped,
        [BeatmapSetWithAudio {
            beatmap_set: expected_set.clone(),
            audio_sources: vec![audio.clone(), other_audio],
            has_multiple_audio_sources: true,
            beatmaps: vec![
                expected_detail.clone(),
                BeatmapDetails {
                    id: second.id,
                    audio_source_id: Some(second.audio_source_id.unwrap()),
                    ..empty_detail.clone()
                },
                empty_detail
            ],
        }]
    );
    let expected_filtered = BeatmapSetWithAudio {
        beatmap_set: expected_set,
        audio_sources: vec![audio.clone()],
        beatmaps: vec![expected_detail],
        has_multiple_audio_sources: true,
    };
    assert_eq!(
        transaction
            .beatmap_sets()
            .for_audio_source(audio.id)
            .await
            .unwrap(),
        [expected_filtered]
    );
    let multiple = std::collections::HashSet::from([set.id]);
    let filtered = transaction
        .beatmap_sets()
        .for_audio_sources(&vec![audio.id; 501], &multiple)
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(
        filtered[0].beatmaps.len(),
        1,
        "duplicate IDs across chunks must not duplicate rows"
    );
    assert!(filtered[0].has_multiple_audio_sources);
    assert!(
        transaction
            .beatmap_sets()
            .for_audio_sources(&[], &multiple)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        transaction
            .beatmap_sets()
            .for_audio_source(i32::MAX)
            .await
            .unwrap()
            .is_empty()
    );
    let search = transaction
        .beatmap_metadata()
        .search_metadata()
        .await
        .unwrap();
    assert_eq!(
        transaction
            .beatmap_sets()
            .matching_audio_sources("projection")
            .await
            .unwrap(),
        [audio.id]
    );
    assert_eq!(
        transaction
            .beatmap_sets()
            .matching_audio_sources("easy")
            .await
            .unwrap(),
        [audio.id]
    );
    assert_eq!(
        transaction
            .beatmap_sets()
            .multiple_audio_set_ids()
            .await
            .unwrap(),
        multiple
    );
    assert_eq!(search.len(), 1);
    assert_eq!(
        (
            &search[0].hash,
            search[0].title.as_deref(),
            search[0].title_unicode.as_deref(),
            search[0].artist.as_deref(),
            search[0].artist_unicode.as_deref()
        ),
        (
            &hash,
            Some("Projection title"),
            Some("曲"),
            Some("Artist"),
            Some("演奏者")
        )
    );
    let mut difficulties = transaction
        .beatmap_sets()
        .search_difficulties()
        .await
        .unwrap();
    difficulties.sort_by_key(|row| row.audio_source_id);
    assert_eq!(
        difficulties.len(),
        2,
        "a difficulty without audio must be absent"
    );
    assert_eq!(
        (
            difficulties[0].set_id,
            difficulties[0].audio_source_id,
            difficulties[0].metadata_hash.as_deref(),
            difficulties[0].difficulty_name.as_deref()
        ),
        (set.id, audio.id, Some(hash.as_str()), Some("Easy"))
    );
    assert_eq!(
        (
            difficulties[1].set_id,
            difficulties[1].audio_source_id,
            difficulties[1].metadata_hash.as_deref(),
            difficulties[1].difficulty_name.as_deref()
        ),
        (set.id, second.audio_source_id.unwrap(), None, None)
    );
    playlist_projections(&transaction, map.id, set.id, audio.id, installation.id).await;
    transaction.rollback().await.unwrap();
    assert!(database.beatmap_sets().get(set.id).await.unwrap().is_none());
    assert!(
        database
            .beatmap_metadata()
            .get(&hash)
            .await
            .unwrap()
            .is_none()
    );
}

#[allow(clippy::too_many_lines)] // Assert complete available and detached playlist projections on the same fixture.
async fn playlist_projections(
    transaction: &crate::Transaction,
    map_id: i32,
    set_id: i32,
    audio_id: i32,
    installation_id: i32,
) {
    use crate::model::{Playlist, PlaylistItem, PlaylistSummary};

    let repository = transaction.playlists();
    let maps = repository.beatmaps(&[map_id]).await.unwrap();
    assert_eq!(maps.len(), 1);
    let map = &maps[0];
    assert_eq!(
        (
            map.beatmap_id,
            map.beatmap_set_id,
            map.source_kind.as_str(),
            map.beatmap_hash.as_deref(),
            map.md5_hash.as_deref(),
            map.title.as_deref(),
            map.title_unicode.as_deref(),
            map.artist.as_deref(),
            map.artist_unicode.as_deref(),
            map.difficulty_name.as_deref(),
            map.audio_source_id,
            map.cover_beatmap_id
        ),
        (
            map_id,
            set_id,
            "lazer",
            Some("beatmap-Easy"),
            Some("abcdef0123456789abcdef0123456789"),
            Some("Projection title"),
            Some("曲"),
            Some("Artist"),
            Some("演奏者"),
            Some("Easy"),
            Some(audio_id),
            Some(map_id)
        )
    );
    assert!(repository.beatmaps(&[]).await.unwrap().is_empty());
    assert!(repository.beatmaps(&[i32::MAX]).await.unwrap().is_empty());
    let playlist = repository.create("Projection playlist").await.unwrap();
    assert_eq!(
        playlist,
        PlaylistSummary {
            id: playlist.id,
            name: "Projection playlist".into(),
            item_count: 0,
            cover_beatmap_id: None,
            custom_cover_revision: None
        }
    );
    assert!(!repository.is_imported(playlist.id).await.unwrap());
    repository.add(playlist.id, map).await.unwrap();
    repository.add(playlist.id, map).await.unwrap();
    let saved = repository.get(playlist.id).await.unwrap().unwrap();
    assert_eq!(
        saved.items.len(),
        1,
        "adding the same stable key is idempotent"
    );
    let item_id = saved.items[0].id;
    let expected_item = PlaylistItem {
        id: item_id,
        playlist_id: playlist.id,
        source_kind: "lazer".into(),
        beatmap_hash: "beatmap-Easy".into(),
        hash_kind: "source".into(),
        title: Some("Projection title".into()),
        title_unicode: Some("曲".into()),
        artist: Some("Artist".into()),
        artist_unicode: Some("演奏者".into()),
        difficulty_name: Some("Easy".into()),
        beatmap_id: Some(map_id),
        beatmap_set_id: Some(set_id),
        audio_source_id: Some(audio_id),
        cover_beatmap_id: Some(map_id),
        last_played_at_ms: None,
        volume_percent: None,
        mods: None,
    };
    assert_eq!(
        saved,
        Playlist {
            id: playlist.id,
            name: "Projection playlist".into(),
            items: vec![expected_item.clone()]
        }
    );
    assert_eq!(
        repository.item(item_id).await.unwrap(),
        Some(expected_item.clone())
    );
    assert_eq!(
        repository.summary(playlist.id).await.unwrap(),
        Some(PlaylistSummary {
            id: playlist.id,
            name: "Projection playlist".into(),
            item_count: 1,
            cover_beatmap_id: Some(map_id),
            custom_cover_revision: None,
        })
    );
    assert!(
        repository
            .set_cover(playlist.id, b"fixture PNG bytes")
            .await
            .unwrap()
    );
    assert_eq!(
        repository.cover(playlist.id).await.unwrap().as_deref(),
        Some(b"fixture PNG bytes".as_slice())
    );
    assert_eq!(
        repository
            .summary(playlist.id)
            .await
            .unwrap()
            .unwrap()
            .custom_cover_revision,
        Some(1)
    );
    // Saved labels and stable identity survive removal of all live library references.
    transaction
        .beatmap_sets()
        .delete_for_installation(installation_id)
        .await
        .unwrap();
    assert_eq!(
        repository.item(item_id).await.unwrap(),
        Some(PlaylistItem {
            beatmap_id: None,
            beatmap_set_id: None,
            audio_source_id: None,
            cover_beatmap_id: None,
            ..expected_item
        })
    );
    assert_eq!(
        repository.summary(playlist.id).await.unwrap(),
        Some(PlaylistSummary {
            id: playlist.id,
            name: "Projection playlist".into(),
            item_count: 1,
            cover_beatmap_id: None,
            custom_cover_revision: Some(1),
        })
    );
}
