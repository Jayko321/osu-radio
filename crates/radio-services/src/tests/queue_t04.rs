use super::*;
use crate::{
    PlaybackCommand as Command, PlaybackMode as Mode, QueueAvailability as Availability,
    QueueItemRef, QueuePlacement,
};
use radio_core::import_types::{ImportedCollection, ImportedSnapshot};

pub(super) async fn contracts(db: &TestDatabase, other: &TestDatabase) {
    let original = db
        .playlists()
        .all()
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect::<std::collections::HashSet<_>>();
    rebinding_and_callbacks(db, other).await;
    waiting_intent(db).await;
    exact_keys_and_shared_audio(db).await;
    rollback_and_visibility(db, other).await;
    for playlist in db.playlists().all().await.unwrap() {
        if !original.contains(&playlist.id) {
            db.playlists().delete(playlist.id).await.unwrap();
        }
    }
}

fn imported(path: &str) -> ImportedSnapshot {
    let mut sets = snapshot("T04 saved", path);
    for (index, map) in sets[0].beatmaps.iter_mut().enumerate() {
        map.hash = Some(format!("t04-native-{index}"));
        map.md5_hash = Some(format!("t04-md5-{index}"));
    }
    ImportedSnapshot {
        beatmap_sets: sets,
        collections: vec![ImportedCollection {
            source_id: "t04-collection".into(),
            name: "T04 collection".into(),
            beatmap_md5_hashes: vec!["t04-md5-0".into()],
        }],
    }
}

async fn maps(db: &TestDatabase, installation: i32) -> Vec<crate::model::Beatmap> {
    let sets = db
        .beatmap_sets()
        .for_installation(installation)
        .await
        .unwrap();
    db.beatmaps().for_set(sets[0].id).await.unwrap()
}

async fn play(db: &TestDatabase) -> crate::PlaybackAssignment {
    db.queue()
        .command(Command::Play {
            audio_source_id: None,
        })
        .await
        .unwrap()
}

async fn stale_checks(db: &TestDatabase, checks: Vec<crate::QueueCheck>) {
    let before = db.queue().get().await.unwrap();
    for check in checks {
        db.queue().apply_check(check.clone(), true).await.unwrap();
        db.queue().apply_check(check, false).await.unwrap();
    }
    assert_eq!(db.queue().get().await.unwrap(), before);
}

#[allow(clippy::too_many_lines)] // Follow the same entries through removal, restoration and late callbacks.
async fn rebinding_and_callbacks(db: &TestDatabase, other: &TestDatabase) {
    db.queue().clear().await.unwrap();
    let initial = imported("/t04/original");
    let installation = db
        .osu_installations()
        .register_imported_snapshot(&marker("t04-rebind"), &initial)
        .await
        .unwrap()
        .into_installation()
        .id;
    let maps = maps(db, installation).await;
    let audio = maps[0].audio_source_id.unwrap();
    let playlist = db.playlists().create("T04 detached").await.unwrap();
    let playlist = db
        .playlists()
        .add_items(playlist.id, &[maps[0].id, maps[1].id])
        .await
        .unwrap();
    // MD5 identity must keep working without its original playlist item.
    sql(
        db,
        &format!(
            "UPDATE playlist_items SET hash_kind = 'md5', beatmap_hash = 't04-md5-0' WHERE id = {}",
            playlist.items[0].id
        ),
    )
    .await;
    db.queue()
        .insert(
            vec![
                QueueItemRef::PlaylistItem {
                    playlist_id: playlist.id,
                    item_id: playlist.items[0].id,
                },
                QueueItemRef::PlaylistItem {
                    playlist_id: playlist.id,
                    item_id: playlist.items[1].id,
                },
                QueueItemRef::PlaylistItem {
                    playlist_id: playlist.id,
                    item_id: playlist.items[0].id,
                },
                QueueItemRef::Audio(audio),
            ],
            QueuePlacement::End,
        )
        .await
        .unwrap();
    let old_checks = db.queue().pending_checks().await.unwrap();
    super::queue::complete_checks(db).await;
    db.audio_settings()
        .set_volume(audio, Some(23))
        .await
        .unwrap();
    let launch = play(db).await;
    let before = db.queue().get().await.unwrap();
    db.playlists().delete(playlist.id).await.unwrap();
    // Even when beatmap and cover IDs change, the checked source stays checked.
    db.osu_installations()
        .replace_imported_snapshot(installation, &initial)
        .await
        .unwrap();
    let unchanged = db.queue().get().await.unwrap();
    assert_eq!(unchanged.launch_source, before.launch_source);
    for (old, new) in before.entries.iter().zip(&unchanged.entries) {
        assert_eq!(new.id, old.id);
        assert_eq!(new.check_generation, old.check_generation);
        assert_eq!(new.availability, Availability::Available);
        assert_ne!(new.beatmap_id, old.beatmap_id);
    }
    let mut changed = imported("/t04/rebound");
    for map in &mut changed.beatmap_sets[0].beatmaps {
        map.metadata.as_mut().unwrap().title = Some("New library name".into());
    }
    db.osu_installations()
        .replace_imported_snapshot(installation, &changed)
        .await
        .unwrap();
    let rebound = other.queue().get().await.unwrap();
    assert_eq!(rebound.revision, unchanged.revision.checked_add(1).unwrap());
    assert_eq!(rebound.playback_token, launch.playback_token);
    assert_eq!(rebound.launch_source, before.launch_source);
    assert!(!rebound.waiting_for_check);
    let new_audio = rebound.entries[0].audio_source_id.unwrap();
    assert_ne!(new_audio, audio);
    for (index, (old, new)) in before.entries.iter().zip(&rebound.entries).enumerate() {
        assert_eq!(new.id, old.id);
        assert_eq!(new.position, old.position);
        assert_eq!(new.title, old.title);
        assert_eq!(new.difficulty_name, old.difficulty_name);
        assert_eq!(new.playlist_source, old.playlist_source);
        assert_eq!(new.playlist_item_id, old.playlist_item_id);
        assert_eq!(
            new.check_generation,
            old.check_generation.checked_add(1).unwrap()
        );
        assert_eq!(
            new.availability,
            if index == 3 {
                Availability::Unavailable
            } else {
                Availability::Checking
            }
        );
    }
    stale_checks(db, old_checks).await;
    let pending = db.queue().pending_checks().await.unwrap();
    assert!(pending.iter().all(|c| c.audio_source_id == new_audio));
    let started = db
        .queue()
        .command(Command::Started {
            playback_token: launch.playback_token,
        })
        .await
        .unwrap();
    assert_eq!(started.current_audio_source_id, Some(audio));
    assert_eq!(started.volume_percent, Some(23));
    assert!(started.track.unwrap().last_played_at_ms.is_some());
    let dates = db
        .database
        .listening_history()
        .for_sources(&[
            SourceType::Local("/t04/original".into()),
            SourceType::Local("/t04/rebound".into()),
        ])
        .await
        .unwrap();
    assert!(dates.contains_key(&("local".into(), "/t04/original".into())));
    assert!(!dates.contains_key(&("local".into(), "/t04/rebound".into())));
    db.queue()
        .apply_check(pending[0].clone(), true)
        .await
        .unwrap();
    let failed = db
        .queue()
        .command(Command::Failed {
            playback_token: launch.playback_token,
        })
        .await
        .unwrap();
    assert_eq!(failed.current_entry_id, Some(rebound.entries[1].id));
    assert!(failed.waiting_for_check);
    assert_eq!(
        db.queue().get().await.unwrap().entries[0].availability,
        Availability::Available
    );
    db.osu_installations()
        .replace_snapshot(installation, &[])
        .await
        .unwrap();
    let removed = db.queue().get().await.unwrap();
    assert_eq!(removed.current_entry_id, None);
    assert_eq!(removed.mode, Mode::Stopped);
    assert!(removed.entries.iter().all(|e| e.audio_source_id.is_none()
        && e.beatmap_id.is_none()
        && e.beatmap_set_id.is_none()
        && e.cover_beatmap_id.is_none()
        && e.availability == Availability::Unavailable));
    stale_checks(db, pending.clone()).await;
    db.osu_installations()
        .replace_imported_snapshot(installation, &initial)
        .await
        .unwrap();
    let restored = db.queue().get().await.unwrap();
    assert_eq!(restored.current_entry_id, None);
    assert_eq!(restored.mode, Mode::Stopped);
    assert_eq!(restored.playback_token, removed.playback_token);
    assert!(
        restored
            .entries
            .iter()
            .all(|e| e.availability == Availability::Checking
                && e.audio_source_id.is_some_and(|id| id != audio))
    );
    stale_checks(db, pending).await;
    super::queue::complete_checks(db).await;
    assert_eq!(db.queue().get().await.unwrap().mode, Mode::Stopped);
    // Reconciliation with no library changes does not rewrite/revise the queue.
    let stable = db.queue().get().await.unwrap();
    reject_step(db, "queue_entries", "INSERT").await;
    assert!(!db.osu_installations().delete(i32::MAX).await.unwrap());
    assert_eq!(db.queue().get().await.unwrap(), stable);
    allow_step(db, "queue_entries").await;
    // Recovery binds the current source again, pauses and invalidates the old token.
    play(db).await;
    db.osu_installations()
        .replace_imported_snapshot(installation, &changed)
        .await
        .unwrap();
    let active = db.queue().get().await.unwrap();
    db.queue().recover().await.unwrap();
    let recovered = db.queue().get().await.unwrap();
    assert_eq!(recovered.mode, Mode::Paused);
    assert!(recovered.waiting_for_check);
    assert!(recovered.launch_source.is_none());
    assert!(recovered.playback_token > active.playback_token);
    super::queue::complete_checks(db).await;
    assert_eq!(
        db.queue()
            .get()
            .await
            .unwrap()
            .launch_source
            .as_ref()
            .unwrap()
            .audio_source_id,
        recovered.current_entry().unwrap().audio_source_id.unwrap()
    );
    db.queue().clear().await.unwrap();
    db.osu_installations().delete(installation).await.unwrap();
}

async fn waiting_intent(db: &TestDatabase) {
    let first = register(db, "t04-wait-a").await;
    let second = register(db, "t04-wait-b").await;
    db.osu_installations()
        .replace_snapshot(first, &snapshot("Wait A", "/t04/wait-a"))
        .await
        .unwrap();
    db.osu_installations()
        .replace_snapshot(second, &snapshot("Wait B", "/t04/wait-b"))
        .await
        .unwrap();
    let b = maps(db, second).await[0].audio_source_id.unwrap();
    for mode in [Mode::Playing, Mode::Paused] {
        let a = maps(db, first).await[0].audio_source_id.unwrap();
        db.queue().clear().await.unwrap();
        db.queue().append(vec![a, b]).await.unwrap();
        play(db).await;
        if mode == Mode::Paused {
            db.queue().command(Command::Pause).await.unwrap();
        }
        let before = db.queue().get().await.unwrap();
        let pending = db.queue().pending_checks().await.unwrap();
        db.osu_installations()
            .replace_snapshot(first, &[])
            .await
            .unwrap();
        let after = db.queue().get().await.unwrap();
        assert_eq!(after.current_entry_id, Some(before.entries[1].id));
        assert_eq!(after.mode, mode);
        assert!(after.waiting_for_check);
        assert!(after.launch_source.is_none());
        assert_eq!(after.revision, before.revision.checked_add(1).unwrap());
        stale_checks(db, vec![pending[0].clone()]).await;
        db.osu_installations()
            .replace_snapshot(first, &snapshot("Wait A", "/t04/wait-a"))
            .await
            .unwrap();
        super::queue::complete_checks(db).await;
        assert_eq!(db.queue().get().await.unwrap().mode, mode);
    }
    db.queue().clear().await.unwrap();
    db.osu_installations().delete(first).await.unwrap();
    db.osu_installations().delete(second).await.unwrap();
}

async fn exact_keys_and_shared_audio(db: &TestDatabase) {
    let initial = imported("/t04/shared");
    let a = db
        .osu_installations()
        .register_imported_snapshot(&marker("t04-key-a"), &initial)
        .await
        .unwrap()
        .into_installation()
        .id;
    let b = db
        .osu_installations()
        .register_imported_snapshot(&marker("t04-key-b"), &initial)
        .await
        .unwrap()
        .into_installation()
        .id;
    let map = maps(db, a).await.remove(0);
    let playlist = db.playlists().create("T04 keys").await.unwrap();
    db.playlists()
        .add_items(playlist.id, &[map.id])
        .await
        .unwrap();
    db.playlists().append_to_queue(playlist.id).await.unwrap();
    db.queue()
        .append(vec![map.audio_source_id.unwrap()])
        .await
        .unwrap();
    super::queue::complete_checks(db).await;
    let before = db.queue().get().await.unwrap();
    db.osu_installations().delete(a).await.unwrap();
    let shared = db.queue().get().await.unwrap();
    assert!(
        shared
            .entries
            .iter()
            .all(|e| e.audio_source_id == map.audio_source_id
                && e.availability == Availability::Available
                && e.check_generation == 0)
    );
    assert_ne!(shared.entries[0].beatmap_id, before.entries[0].beatmap_id);
    // Same text in MD5 and native hash namespaces, or another installation kind, is not identity.
    let mut collision = initial.clone();
    collision.beatmap_sets[0].beatmaps[0].hash = Some("different-native".into());
    collision.beatmap_sets[0].beatmaps[0].md5_hash = Some("t04-native-0".into());
    db.osu_installations()
        .replace_imported_snapshot(b, &collision)
        .await
        .unwrap();
    assert_eq!(
        db.queue().get().await.unwrap().entries[0].availability,
        Availability::Unavailable
    );
    let mut stable = initial.clone();
    stable.beatmap_sets[0].source = OsuKind::Stable;
    let stable_marker = OsuMarker {
        kind: OsuKind::Stable,
        root_path: "/t04/stable".into(),
        marker_path: "/t04/stable/osu!.db".into(),
    };
    let c = db
        .osu_installations()
        .register_imported_snapshot(&stable_marker, &stable)
        .await
        .unwrap()
        .into_installation()
        .id;
    assert_eq!(
        db.queue().get().await.unwrap().entries[0].availability,
        Availability::Unavailable
    );
    // An exact library source key includes its kind, even when location is identical.
    let mut state = db.queue().get().await.unwrap();
    state.entries[1].audio_source = Some(SourceType::Copied("/t04/shared".into()));
    let tx = db.database.begin().await.unwrap();
    tx.user_data().lock().await.unwrap();
    tx.queue().save(&state).await.unwrap();
    tx.commit().await.unwrap();
    db.osu_installations()
        .replace_imported_snapshot(b, &initial)
        .await
        .unwrap();
    let restored = db.queue().get().await.unwrap();
    assert_eq!(restored.entries[0].availability, Availability::Checking);
    assert_eq!(restored.entries[1].availability, Availability::Unavailable);
    assert_eq!(restored.current_entry_id, None);
    // A migrated numeric ID with no saved key cannot be rebound to a library row.
    let mut legacy = restored;
    legacy.entries[1].audio_source = None;
    legacy.entries[1].migration_audio_source_id = map.audio_source_id;
    let tx = db.database.begin().await.unwrap();
    tx.user_data().lock().await.unwrap();
    tx.queue().save(&legacy).await.unwrap();
    tx.commit().await.unwrap();
    db.osu_installations().delete(i32::MAX).await.unwrap();
    assert_eq!(
        db.queue().get().await.unwrap().entries[1].audio_source_id,
        None
    );
    db.queue().clear().await.unwrap();
    db.playlists().delete(playlist.id).await.unwrap();
    db.osu_installations().delete(b).await.unwrap();
    db.osu_installations().delete(c).await.unwrap();
}

#[cfg(feature = "sqlite")]
async fn reject_step(db: &TestDatabase, table: &str, event: &str) {
    sql(db, &format!("CREATE TRIGGER t04_failure BEFORE {event} ON {table} BEGIN SELECT RAISE(ABORT, 't04 forced failure'); END")).await;
}
#[cfg(feature = "postgres")]
async fn reject_step(db: &TestDatabase, table: &str, event: &str) {
    sql(db, "CREATE FUNCTION t04_failure_fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 't04 forced failure'; END $$").await;
    sql(db, &format!("CREATE TRIGGER t04_failure BEFORE {event} ON {table} FOR EACH ROW EXECUTE FUNCTION t04_failure_fn()")).await;
}
async fn allow_step(db: &TestDatabase, table: &str) {
    #[cfg(feature = "sqlite")]
    {
        let _ = table;
        sql(db, "DROP TRIGGER t04_failure").await;
    }
    #[cfg(feature = "postgres")]
    {
        sql(db, &format!("DROP TRIGGER t04_failure ON {table}")).await;
        sql(db, "DROP FUNCTION t04_failure_fn()").await;
    }
}

#[allow(clippy::too_many_lines)] // Exercise failures at every late write in the same complete snapshot fixture.
async fn rollback_and_visibility(db: &TestDatabase, other: &TestDatabase) {
    let existing_playlists = db
        .playlists()
        .all()
        .await
        .unwrap()
        .into_iter()
        .map(|p| p.id)
        .collect::<std::collections::HashSet<_>>();
    let initial = imported("/t04/rollback-old");
    let installation = db
        .osu_installations()
        .register_imported_snapshot(&marker("t04-rollback"), &initial)
        .await
        .unwrap()
        .into_installation()
        .id;
    let audio = maps(db, installation).await[0].audio_source_id.unwrap();
    db.queue().append(vec![audio, audio]).await.unwrap();
    let mut replacement = imported("/t04/rollback-new");
    replacement.collections[0].name = "Changed collection".into();
    replacement.collections[0]
        .beatmap_md5_hashes
        .push("t04-md5-1".into());
    let collection_id = db
        .playlists()
        .all()
        .await
        .unwrap()
        .into_iter()
        .find(|p| !existing_playlists.contains(&p.id))
        .unwrap()
        .id;
    let old_collection = db.playlists().get(collection_id).await.unwrap();
    let old_installation = db.osu_installations().get(installation).await.unwrap();
    let old_sets = db
        .beatmap_sets()
        .for_installation(installation)
        .await
        .unwrap();
    let old_maps = maps(db, installation).await;
    for step in [
        "queue",
        "timestamp",
        "cleanup",
        "revision",
        "generation",
        "delete",
    ] {
        let (table, event) = match step {
            "queue" | "delete" => ("queue_entries", "INSERT"),
            "timestamp" => ("osu_installations", "UPDATE OF last_scanned_at"),
            _ => ("audio_sources", "DELETE"),
        };
        match step {
            "revision" => {
                sql(
                    db,
                    &format!("UPDATE playback_queue SET revision = {}", i64::MAX),
                )
                .await;
            }
            "generation" => {
                sql(
                    db,
                    &format!("UPDATE queue_entries SET check_generation = {}", i64::MAX),
                )
                .await;
            }
            _ => reject_step(db, table, event).await,
        }
        let before = db.queue().get().await.unwrap();
        let result = if step == "delete" {
            db.osu_installations()
                .delete(installation)
                .await
                .map(|_| ())
        } else {
            db.osu_installations()
                .replace_imported_snapshot(installation, &replacement)
                .await
                .map(|_| ())
        };
        assert!(result.is_err(), "{step}");
        assert_eq!(other.queue().get().await.unwrap(), before, "{step}");
        assert_eq!(
            other.osu_installations().get(installation).await.unwrap(),
            old_installation,
            "{step}"
        );
        assert_eq!(
            other
                .beatmap_sets()
                .for_installation(installation)
                .await
                .unwrap(),
            old_sets,
            "{step}"
        );
        assert_eq!(maps(other, installation).await, old_maps, "{step}");
        assert_eq!(
            other.playlists().get(collection_id).await.unwrap(),
            old_collection,
            "{step}"
        );
        assert!(
            other
                .audio_sources()
                .find(&SourceType::Local("/t04/rollback-new".into()))
                .await
                .unwrap()
                .is_none()
        );
        match step {
            "revision" => sql(db, "UPDATE playback_queue SET revision = 0").await,
            "generation" => sql(db, "UPDATE queue_entries SET check_generation = 0").await,
            _ => allow_step(db, table).await,
        }
    }
    let before = db.queue().get().await.unwrap();
    // The shared workflow has reached its commit barrier; another pool still sees the old snapshot.
    let tx = db.database.begin().await.unwrap();
    crate::OsuInstallationService::replace_imported_snapshot_in(&tx, installation, &replacement)
        .await
        .unwrap();
    assert_ne!(tx.queue().get().await.unwrap(), before);
    assert_eq!(other.queue().get().await.unwrap(), before);
    assert_eq!(maps(other, installation).await, old_maps);
    assert_eq!(
        other.playlists().get(collection_id).await.unwrap(),
        old_collection
    );
    assert_eq!(
        other.osu_installations().get(installation).await.unwrap(),
        old_installation
    );
    tx.commit().await.unwrap();
    let committed = other.queue().get().await.unwrap();
    assert_eq!(committed.revision, before.revision.checked_add(1).unwrap());
    assert!(
        committed
            .entries
            .iter()
            .all(|e| e.availability == Availability::Unavailable)
    );
    assert_ne!(maps(other, installation).await, old_maps);
    assert_ne!(
        other.playlists().get(collection_id).await.unwrap(),
        old_collection
    );
    // Cancelling after all library/queue/timestamp writes must roll back all of them.
    let services = db.services.clone();
    let cancelled_snapshot = initial.clone();
    let (reached, barrier) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let tx = services.database.begin().await.unwrap();
        crate::OsuInstallationService::replace_imported_snapshot_in(
            &tx,
            installation,
            &cancelled_snapshot,
        )
        .await
        .unwrap();
        reached.send(()).unwrap();
        std::future::pending::<()>().await;
        tx.commit().await.unwrap();
    });
    barrier.await.unwrap();
    let committed_installation = db.osu_installations().get(installation).await.unwrap();
    let committed_maps = maps(db, installation).await;
    let committed_collection = db.playlists().get(collection_id).await.unwrap();
    assert_eq!(other.queue().get().await.unwrap(), committed);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    // Acquire the same writer lock to wait for the dropped transaction's rollback.
    let tx = db.database.begin().await.unwrap();
    tx.user_data().lock().await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(other.queue().get().await.unwrap(), committed);
    assert_eq!(maps(other, installation).await, committed_maps);
    assert_eq!(
        other.osu_installations().get(installation).await.unwrap(),
        committed_installation
    );
    assert_eq!(
        other.playlists().get(collection_id).await.unwrap(),
        committed_collection
    );
    // Queue save failure must also roll back a newly registered installation and its collections.
    let returned_marker = marker("t04-registration-return");
    let collections_before = db.playlists().all().await.unwrap();
    reject_step(db, "queue_entries", "INSERT").await;
    assert!(
        db.osu_installations()
            .register_imported_snapshot(&returned_marker, &initial)
            .await
            .is_err()
    );
    assert_eq!(other.queue().get().await.unwrap(), committed);
    assert_eq!(other.playlists().all().await.unwrap(), collections_before);
    assert!(
        other
            .osu_installations()
            .all()
            .await
            .unwrap()
            .iter()
            .all(|i| i.marker_path != returned_marker.marker_path)
    );
    assert!(
        other
            .audio_sources()
            .find(&SourceType::Local("/t04/rollback-old".into()))
            .await
            .unwrap()
            .is_none()
    );
    allow_step(db, "queue_entries").await;
    let returned = db
        .osu_installations()
        .register_imported_snapshot(&returned_marker, &initial)
        .await
        .unwrap()
        .into_installation();
    let restored = other.queue().get().await.unwrap();
    assert!(
        restored
            .entries
            .iter()
            .all(|e| e.availability == Availability::Checking
                && e.audio_source_id.is_some_and(|id| id != audio))
    );
    assert_eq!(restored.current_entry_id, None);
    assert_eq!(restored.mode, Mode::Stopped);
    assert_eq!(
        restored.revision,
        committed.revision.checked_add(1).unwrap()
    );
    db.queue().clear().await.unwrap();
    db.osu_installations().delete(returned.id).await.unwrap();
    db.osu_installations().delete(installation).await.unwrap();
}
