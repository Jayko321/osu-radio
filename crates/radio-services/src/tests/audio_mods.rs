use super::*;
use crate::{
    AudioModsError, AudioModsMode as Mode, AudioModsPatch as Patch, AudioModsProfile as Profile,
    AudioModsSettings,
};

#[allow(clippy::too_many_lines)] // One source lifecycle covers shared projections, mode retention, partial writes and independence from volume/queue.
pub(super) async fn contracts(db: &TestDatabase, other: &TestDatabase, url: &str) {
    let installation = register(db, "audio-mods").await;
    let imported = snapshot("Mods song", "/mods/source");
    db.osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let original = db
        .beatmap_sets()
        .search_tracks("Mods song")
        .await
        .unwrap()
        .remove(0);
    assert_eq!(original.mods, None);
    assert_eq!(
        db.audio_mods().get().await.unwrap(),
        AudioModsSettings::default()
    );
    let volume = db.audio_settings().get().await.unwrap();
    let dt = Profile {
        mode: Mode::Dt,
        speed_percent: 120,
        adjust_pitch: true,
    };
    let ht = Profile {
        mode: Mode::Ht,
        speed_percent: 75,
        adjust_pitch: false,
    };
    let settings = db
        .audio_mods()
        .update(Patch {
            global_mode: Some(dt.mode),
            global_speed_percent: Some(dt.speed_percent),
            global_adjust_pitch: Some(dt.adjust_pitch),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(settings.global_profile, dt);
    assert_eq!(
        other
            .beatmap_sets()
            .search_tracks("Mods song")
            .await
            .unwrap()[0]
            .mods,
        None,
        "stored individual profile never inherits global mods"
    );
    db.audio_mods()
        .update(Patch {
            individual_mods_enabled: Some(true),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        other
            .beatmap_sets()
            .search_tracks("Mods song")
            .await
            .unwrap()[0]
            .mods,
        None
    );
    db.audio_mods()
        .set_profile(original.audio_source_id, Some(ht))
        .await
        .unwrap();
    let maps = original
        .difficulties
        .iter()
        .map(|d| d.beatmap_id)
        .collect::<Vec<_>>();
    assert_eq!(maps.len(), 2);
    let mut playlists = Vec::new();
    for name in ["Mods playlist", "Shared mods"] {
        let list = db.playlists().create(name).await.unwrap();
        let list = db.playlists().add_items(list.id, &maps).await.unwrap();
        assert!(list.items.iter().all(|i| i.mods == Some(ht)));
        playlists.push(list);
    }
    let launch = db.playlists().play(playlists[0].id, None).await.unwrap();
    assert_eq!(launch.mods, Some(ht));
    assert_eq!(launch.track.unwrap().mods, Some(ht));
    let before_queue = db.queue().get().await.unwrap();
    for enabled in [false, true] {
        let settings = db
            .audio_mods()
            .update(Patch {
                individual_mods_enabled: Some(enabled),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(settings.global_profile, dt);
        assert!(
            other
                .queue()
                .snapshot()
                .await
                .unwrap()
                .entries
                .iter()
                .all(|e| e.mods == Some(ht))
        );
        assert_eq!(other.queue().playback().await.unwrap().mods, Some(ht));
        assert_eq!(
            other
                .beatmap_sets()
                .search_tracks("Mods song")
                .await
                .unwrap()[0]
                .mods,
            Some(ht)
        );
    }
    // Partial changes preserve every omitted field, and validate the merged profile before writing.
    let before = db
        .audio_mods()
        .update(Patch {
            global_speed_percent: Some(130),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        before.global_profile,
        Profile {
            speed_percent: 130,
            ..dt
        }
    );
    assert_eq!(
        db.audio_mods().update(Patch::default()).await.unwrap(),
        before
    );
    let error = db
        .audio_mods()
        .update(Patch {
            individual_mods_enabled: Some(false),
            global_mode: Some(Mode::Ht),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(error.downcast_ref(), Some(&AudioModsError::InvalidProfile));
    assert_eq!(other.audio_mods().get().await.unwrap(), before);
    let error = db
        .audio_mods()
        .set_profile(
            original.audio_source_id,
            Some(Profile {
                speed_percent: 100,
                ..ht
            }),
        )
        .await
        .unwrap_err();
    assert_eq!(error.downcast_ref(), Some(&AudioModsError::InvalidProfile));
    for value in [Some(ht), None] {
        let error = db
            .audio_mods()
            .set_profile(i32::MAX, value)
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref(),
            Some(&AudioModsError::UnknownAudio(i32::MAX))
        );
    }
    assert_eq!(
        other.queue().get().await.unwrap(),
        before_queue,
        "mods writes cannot mutate queue tokens/revisions or launch"
    );
    assert_eq!(other.audio_settings().get().await.unwrap(), volume);
    assert_eq!(
        other
            .beatmap_sets()
            .search_tracks("Mods song")
            .await
            .unwrap()[0]
            .mods,
        Some(ht)
    );
    let reopened = Services::connect(url).await.unwrap();
    reopened.migrate().await.unwrap();
    assert_eq!(reopened.audio_mods().get().await.unwrap(), before);
    assert_eq!(reopened.queue().playback().await.unwrap().mods, Some(ht));
    db.osu_installations()
        .replace_snapshot(installation, &[])
        .await
        .unwrap();
    assert!(
        db.audio_sources()
            .get(original.audio_source_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        db.playlists()
            .get(playlists[0].id)
            .await
            .unwrap()
            .unwrap()
            .items
            .iter()
            .all(|i| i.mods.is_none())
    );
    // The full queue retains the saved source even while the library no longer resolves it.
    assert!(
        db.queue()
            .snapshot()
            .await
            .unwrap()
            .entries
            .iter()
            .all(|e| e.mods == Some(ht))
    );
    db.osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let returned = db
        .beatmap_sets()
        .search_tracks("Mods song")
        .await
        .unwrap()
        .remove(0);
    assert_ne!(returned.audio_source_id, original.audio_source_id);
    assert_eq!(returned.mods, Some(ht));
    for list in &playlists {
        assert!(
            db.playlists()
                .get(list.id)
                .await
                .unwrap()
                .unwrap()
                .items
                .iter()
                .all(|i| i.mods == Some(ht))
        );
    }
    // Changing either component of the key must not reuse a retained profile.
    for source in [
        SourceType::Copied("/mods/source".into()),
        SourceType::Local("/mods/moved".into()),
    ] {
        let audio = db
            .database
            .audio_sources()
            .get_or_insert(&source)
            .await
            .unwrap();
        assert!(
            db.database
                .audio_mods()
                .for_audio_sources(&[audio.id])
                .await
                .unwrap()
                .is_empty()
        );
    }
    db.audio_mods()
        .set_profile(returned.audio_source_id, Some(Profile::default()))
        .await
        .unwrap();
    assert_eq!(
        other
            .beatmap_sets()
            .search_tracks("Mods song")
            .await
            .unwrap()[0]
            .mods,
        Some(Profile::default())
    );
    for _ in 0..2 {
        db.audio_mods()
            .set_profile(returned.audio_source_id, None)
            .await
            .unwrap();
        assert_eq!(
            other
                .beatmap_sets()
                .search_tracks("Mods song")
                .await
                .unwrap()[0]
                .mods,
            None
        );
    }
    assert_eq!(
        db.audio_mods().get().await.unwrap(),
        before,
        "deleting an individual profile preserves global mods"
    );
    db.osu_installations().delete(installation).await.unwrap();
}
