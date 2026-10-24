use super::*;
use crate::{
    PlaybackCommand as Command, PlaybackMode as Mode, Playlist, PlaylistError,
    QueueAvailability as Availability, QueueState,
};

#[allow(clippy::too_many_lines)] // Keep the complete playlist snapshot and restoration fixture together.
pub(super) async fn contracts(db: &TestDatabase, other: &TestDatabase) {
    let installation = register(db, "queue-t03").await;
    let mut imported = snapshot("T03 snapshots", "/queue-t03/shared");
    let mut missing = imported[0].beatmaps[0].clone();
    missing.difficulty_name = Some("Lost".into());
    imported[0].beatmaps.push(missing);
    for (i, map) in imported[0].beatmaps.iter_mut().enumerate() {
        map.hash = Some(format!("t03-{i}"));
        map.md5_hash = Some(format!("t03-md5-{i}"));
        map.metadata.as_mut().unwrap().artist_unicode = Some("歌手".into());
    }
    db.osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let set = db
        .beatmap_sets()
        .for_installation(installation)
        .await
        .unwrap()
        .remove(0);
    let maps = db.beatmaps().for_set(set.id).await.unwrap();
    let playlist = db.playlists().create("T03").await.unwrap();
    let playlist = db
        .playlists()
        .add_items(playlist.id, &maps.iter().map(|m| m.id).collect::<Vec<_>>())
        .await
        .unwrap();
    sql(
        db,
        &format!(
            "UPDATE playlist_items SET hash_kind = 'md5', beatmap_hash = 't03-md5-0' WHERE id = {}",
            playlist.items[0].id
        ),
    )
    .await;
    sql(
        db,
        &format!(
            "UPDATE playlist_items SET beatmap_hash = 't03-missing' WHERE id = {}",
            playlist.items[2].id
        ),
    )
    .await;
    let playlist = db.playlists().get(playlist.id).await.unwrap().unwrap();
    let audio = playlist.items[0].audio_source_id.unwrap();
    append_states(db, &playlist, audio).await;
    play_boundaries(db, &playlist, audio).await;
    validation(db, &playlist).await;
    rollback_and_writers(db, other, &playlist).await;
    history_limit(db, &playlist, audio).await;
    detached(db, &playlist).await;
    // A wholly unavailable nonempty playlist remains future, even after sources return.
    let gone = db.playlists().create("T03 all gone").await.unwrap();
    db.playlists()
        .add_items(gone.id, &maps.iter().map(|m| m.id).collect::<Vec<_>>())
        .await
        .unwrap();
    db.queue().clear().await.unwrap();
    db.queue().append(vec![audio, audio]).await.unwrap();
    db.queue()
        .command(Command::Play {
            audio_source_id: None,
        })
        .await
        .unwrap();
    super::queue::complete_checks(db).await;
    let before = db.queue().get().await.unwrap();
    db.osu_installations()
        .replace_snapshot(installation, &[])
        .await
        .unwrap();
    let stopped = db.playlists().play(gone.id, None).await.unwrap();
    assert_eq!(stopped.mode, Mode::Stopped);
    assert_eq!(stopped.current_entry_id, None);
    assert!(!stopped.waiting_for_check);
    assert_eq!(
        stopped.playback_token,
        before.playback_token.checked_add(1).unwrap()
    );
    let state = db.queue().get().await.unwrap();
    assert_eq!(state.history_len, 1);
    assert_eq!(state.entries.len(), 4);
    assert_eq!(state.entries[0].id, before.entries[0].id);
    assert!(
        state.entries[1..]
            .iter()
            .all(|e| e.availability == Availability::Unavailable)
    );
    assert_eq!(state.revision, before.revision.checked_add(2).unwrap());
    stale(db, before.playback_token).await;
    db.osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let restored = db.queue().get().await.unwrap();
    assert_eq!(restored.current_entry_id, None);
    assert_eq!(restored.mode, Mode::Stopped);
    assert_eq!(restored.playback_token, stopped.playback_token);
    assert!(
        restored
            .entries
            .iter()
            .all(|e| e.availability == Availability::Checking)
    );
    db.queue().clear().await.unwrap();
    db.playlists().delete(gone.id).await.unwrap();
    db.osu_installations().delete(installation).await.unwrap();
}

fn assert_items(state: &QueueState, playlist: &Playlist, offset: usize) {
    let end = offset.checked_add(playlist.items.len()).unwrap();
    for (entry, item) in state.entries[offset..end].iter().zip(&playlist.items) {
        assert_eq!(entry.playlist_item_id, Some(item.id));
        assert_eq!(entry.audio_source_id, item.audio_source_id);
        let key = entry.playlist_source.as_ref().unwrap();
        assert_eq!(key.installation_kind, item.source_kind);
        assert_eq!(key.hash_kind, item.hash_kind);
        assert_eq!(key.beatmap_hash, item.beatmap_hash);
        assert_eq!(entry.title, item.title);
        assert_eq!(entry.title_unicode, item.title_unicode);
        assert_eq!(entry.artist, item.artist);
        assert_eq!(entry.artist_unicode, item.artist_unicode);
        assert_eq!(entry.difficulty_name, item.difficulty_name);
        assert_eq!(entry.beatmap_id, item.beatmap_id);
        assert_eq!(entry.beatmap_set_id, item.beatmap_set_id);
        assert_eq!(entry.cover_beatmap_id, item.cover_beatmap_id);
        assert_eq!(entry.audio_source.is_some(), item.audio_source_id.is_some());
        assert_eq!(
            entry.availability,
            if item.audio_source_id.is_some() {
                Availability::Checking
            } else {
                Availability::Unavailable
            }
        );
    }
}

async fn append_states(db: &TestDatabase, playlist: &Playlist, audio: i32) {
    for mode in ["empty", "exhausted", "playing", "paused"] {
        db.queue().clear().await.unwrap();
        if mode != "empty" {
            db.queue().append(vec![audio]).await.unwrap();
            db.queue()
                .command(Command::Play {
                    audio_source_id: None,
                })
                .await
                .unwrap();
            super::queue::complete_checks(db).await;
            if mode == "exhausted" {
                db.queue().command(Command::Next).await.unwrap();
            } else if mode == "paused" {
                db.queue().command(Command::Pause).await.unwrap();
            }
        }
        let before = db.queue().get().await.unwrap();
        let assignment = db.playlists().append_to_queue(playlist.id).await.unwrap();
        let state = db.queue().get().await.unwrap();
        assert_eq!(
            state.entries.len(),
            before.entries.len().checked_add(3).unwrap()
        );
        assert_eq!(&state.entries[..before.entries.len()], before.entries);
        assert_eq!(state.current_entry_id, before.current_entry_id);
        assert_eq!(state.history_len, before.history_len);
        assert_eq!(assignment.mode, before.mode);
        assert_eq!(state.waiting_for_check, before.waiting_for_check);
        assert_eq!(state.playback_token, before.playback_token);
        assert_eq!(state.revision, before.revision.checked_add(1).unwrap());
        assert_items(&state, playlist, before.entries.len());
        let repeated = db.playlists().append_to_queue(playlist.id).await.unwrap();
        let twice = db.queue().get().await.unwrap();
        assert_eq!(
            twice.entries.len(),
            state.entries.len().checked_add(3).unwrap()
        );
        assert_items(&twice, playlist, state.entries.len());
        assert_eq!(&twice.entries[..state.entries.len()], state.entries);
        assert_eq!(
            twice
                .entries
                .iter()
                .map(|e| e.id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            twice.entries.len()
        );
        let dates = db
            .queue()
            .snapshot()
            .await
            .unwrap()
            .entries
            .iter()
            .map(|e| e.last_played_at_ms)
            .collect::<Vec<_>>();
        super::queue::complete_checks(db).await;
        let checked = db.queue().playback().await.unwrap();
        assert_eq!(checked.mode, repeated.mode);
        assert_eq!(checked.playback_token, repeated.playback_token);
        assert_eq!(checked.current_entry_id, repeated.current_entry_id);
        assert_eq!(
            db.queue()
                .snapshot()
                .await
                .unwrap()
                .entries
                .iter()
                .map(|e| e.last_played_at_ms)
                .collect::<Vec<_>>(),
            dates
        );
        if mode == "exhausted" {
            let play = db
                .queue()
                .command(Command::Play {
                    audio_source_id: None,
                })
                .await
                .unwrap();
            assert_eq!(
                play.current_entry_id,
                Some(twice.entries[before.history_len].id)
            );
        }
    }
}

async fn stale(db: &TestDatabase, token: u64) {
    let before = db.queue().snapshot().await.unwrap();
    for command in [
        Command::Started {
            playback_token: token,
        },
        Command::Finished {
            playback_token: token,
        },
        Command::Failed {
            playback_token: token,
        },
    ] {
        db.queue().command(command).await.unwrap();
        assert_eq!(db.queue().snapshot().await.unwrap(), before);
    }
}

#[allow(clippy::too_many_lines)] // Boundary variants share launch, snapshot and stale-callback assertions.
async fn play_boundaries(db: &TestDatabase, playlist: &Playlist, audio: i32) {
    for (position, selected) in [(1, None), (1, Some(1)), (4, None), (0, None), (5, None)] {
        db.queue().clear().await.unwrap();
        db.queue().append(vec![audio; 4]).await.unwrap();
        db.queue()
            .command(Command::Play {
                audio_source_id: None,
            })
            .await
            .unwrap();
        super::queue::complete_checks(db).await;
        for _ in 0..position.max(1) {
            db.queue().command(Command::Next).await.unwrap();
        }
        if position == 0 {
            db.queue().command(Command::Previous).await.unwrap();
        }
        if position == 5 {
            db.queue().append(vec![audio, audio]).await.unwrap();
        }
        let before = db.queue().get().await.unwrap();
        let removed_check = db
            .queue()
            .pending_checks()
            .await
            .unwrap()
            .into_iter()
            .find(|check| {
                before
                    .entries
                    .iter()
                    .skip(before.history_len)
                    .any(|e| e.id == check.entry_id)
            });
        let boundary = before
            .current_index()
            .map_or(before.history_len, |i| i.checked_add(1).unwrap());
        let started = db
            .playlists()
            .play(playlist.id, selected.map(|i| playlist.items[i].id))
            .await
            .unwrap();
        let after = db.queue().get().await.unwrap();
        assert_eq!(&after.entries[..boundary], &before.entries[..boundary]);
        assert_eq!(after.entries.len(), boundary.checked_add(3).unwrap());
        assert_items(&after, playlist, boundary);
        assert!(
            after.entries[boundary..]
                .iter()
                .all(|e| e.id.get() > before.last_entry_id)
        );
        assert_eq!(
            after.last_entry_id,
            before.last_entry_id.checked_add(3).unwrap()
        );
        assert_eq!(after.revision, before.revision.checked_add(1).unwrap());
        assert_eq!(
            after.current_index(),
            Some(boundary.checked_add(selected.unwrap_or(0)).unwrap())
        );
        assert!(started.waiting_for_check);
        assert_eq!(started.mode, Mode::Playing);
        assert_eq!(
            started.playback_token,
            before.playback_token.checked_add(1).unwrap()
        );
        stale(db, before.playback_token).await;
        if let Some(check) = removed_check {
            let snapshot = db.queue().snapshot().await.unwrap();
            db.queue().apply_check(check, true).await.unwrap();
            assert_eq!(db.queue().snapshot().await.unwrap(), snapshot);
        }
        db.queue()
            .command(Command::Started {
                playback_token: started.playback_token,
            })
            .await
            .unwrap();
        assert!(
            db.queue()
                .snapshot()
                .await
                .unwrap()
                .entries
                .iter()
                .all(|e| e.last_played_at_ms.is_none())
        );
        super::queue::complete_checks(db).await;
        let ready = db.queue().playback().await.unwrap();
        assert!(!ready.waiting_for_check);
        assert_eq!(ready.playback_token, started.playback_token);
        assert!(
            db.queue()
                .snapshot()
                .await
                .unwrap()
                .entries
                .iter()
                .all(|e| e.last_played_at_ms.is_none())
        );
    }
    // A lost first source is retained before the first resolvable start.
    sql(
        db,
        &format!(
            "UPDATE playlist_items SET beatmap_hash = 't03-lost-first' WHERE id = {}",
            playlist.items[0].id
        ),
    )
    .await;
    let started = db.playlists().play(playlist.id, None).await.unwrap();
    assert_eq!(started.current_playlist_item_id, Some(playlist.items[1].id));
    sql(
        db,
        &format!(
            "UPDATE playlist_items SET beatmap_hash = 't03-md5-0' WHERE id = {}",
            playlist.items[0].id
        ),
    )
    .await;
}

async fn validation(db: &TestDatabase, playlist: &Playlist) {
    let empty = db.playlists().create("T03 empty").await.unwrap();
    let before = db.queue().snapshot().await.unwrap();
    let assignment = db.queue().playback().await.unwrap();
    assert_eq!(
        db.playlists().append_to_queue(empty.id).await.unwrap(),
        assignment
    );
    assert_eq!(db.queue().snapshot().await.unwrap(), before);
    for (id, start, expected) in [
        (empty.id, None, PlaylistError::NoAvailableItems),
        (
            empty.id,
            Some(playlist.items[0].id),
            PlaylistError::ItemNotFound(playlist.items[0].id),
        ),
        (
            playlist.id,
            Some(i32::MAX),
            PlaylistError::ItemNotFound(i32::MAX),
        ),
        (
            playlist.id,
            Some(playlist.items[2].id),
            PlaylistError::UnavailableItem(playlist.items[2].id),
        ),
        (i32::MAX, None, PlaylistError::NotFound(i32::MAX)),
    ] {
        let error = db.playlists().play(id, start).await.unwrap_err();
        assert_eq!(error.downcast_ref::<PlaylistError>(), Some(&expected));
        assert_eq!(db.queue().snapshot().await.unwrap(), before);
    }
    assert_eq!(
        db.playlists()
            .append_to_queue(i32::MAX)
            .await
            .unwrap_err()
            .downcast_ref::<PlaylistError>(),
        Some(&PlaylistError::NotFound(i32::MAX))
    );
    db.playlists().delete(empty.id).await.unwrap();
}

async fn rollback_and_writers(db: &TestDatabase, other: &TestDatabase, playlist: &Playlist) {
    let before = db.queue().snapshot().await.unwrap();
    let reject_id = before.last_entry_id.checked_add(2).unwrap();
    #[cfg(feature = "sqlite")]
    sql(db, &format!("CREATE TRIGGER reject_t03 BEFORE INSERT ON queue_entries WHEN NEW.id = {reject_id} BEGIN SELECT RAISE(ABORT, 'T03 rollback'); END")).await;
    #[cfg(feature = "postgres")]
    {
        sql(db, &format!("CREATE FUNCTION reject_t03_fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id = {reject_id} THEN RAISE EXCEPTION 'T03 rollback'; END IF; RETURN NEW; END $$")).await;
        sql(db, "CREATE TRIGGER reject_t03 BEFORE INSERT ON queue_entries FOR EACH ROW EXECUTE FUNCTION reject_t03_fn()").await;
    }
    assert!(db.playlists().append_to_queue(playlist.id).await.is_err());
    assert_eq!(other.queue().snapshot().await.unwrap(), before);
    assert!(db.playlists().play(playlist.id, None).await.is_err());
    assert_eq!(other.queue().snapshot().await.unwrap(), before);
    #[cfg(feature = "sqlite")]
    sql(db, "DROP TRIGGER reject_t03").await;
    #[cfg(feature = "postgres")]
    {
        sql(db, "DROP TRIGGER reject_t03 ON queue_entries").await;
        sql(db, "DROP FUNCTION reject_t03_fn()").await;
    }
    let old_check = db.queue().pending_checks().await.unwrap().remove(0);
    db.queue().clear().await.unwrap();
    let first = db.playlists();
    let second = other.playlists();
    let (a, b) = tokio::join!(
        first.append_to_queue(playlist.id),
        second.append_to_queue(playlist.id)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.revision.abs_diff(b.revision), 1);
    let after = other.queue().get().await.unwrap();
    assert_eq!(after.entries.len(), 6);
    assert_items(&after, playlist, 0);
    assert_items(&after, playlist, 3);
    assert_eq!(after.current_entry_id, None);
    assert_eq!(after.mode, Mode::Stopped);
    let before_stale = db.queue().snapshot().await.unwrap();
    db.queue().apply_check(old_check, true).await.unwrap();
    assert_eq!(db.queue().snapshot().await.unwrap(), before_stale);
    // Concurrent Play followed by append, or append followed by Play: no partial playlist.
    let (play, append) = tokio::join!(
        first.play(playlist.id, None),
        second.append_to_queue(playlist.id)
    );
    let play = play.unwrap();
    let append = append.unwrap();
    let after = other.queue().get().await.unwrap();
    assert_eq!(
        after.entries.len(),
        if play.revision < append.revision {
            6
        } else {
            3
        }
    );
    assert_eq!(after.current_entry_id, play.current_entry_id);
    assert_eq!(after.mode, Mode::Playing);
    assert_items(&after, playlist, 0);
}

async fn history_limit(db: &TestDatabase, playlist: &Playlist, audio: i32) {
    let empty = db.playlists().create("T03 oversized no-op").await.unwrap();
    for operation in ["append", "play", "selected", "unavailable"] {
        db.queue().clear().await.unwrap();
        db.queue().append(vec![audio]).await.unwrap();
        let mut state = db.queue().get().await.unwrap();
        let template = state.entries.remove(0);
        for position in 0..5002 {
            let mut entry = template.clone();
            entry.id = state.allocate_id().unwrap();
            entry.position = position;
            entry.availability = Availability::Unavailable;
            state.entries.push(entry);
        }
        state.current_entry_id = Some(state.entries[5001].id);
        state.history_len = 5001;
        state.mode = Mode::Paused;
        let tx = db.database.begin().await.unwrap();
        tx.user_data().lock().await.unwrap();
        tx.queue().save(&state).await.unwrap();
        tx.commit().await.unwrap();
        let assignment = db.queue().playback().await.unwrap();
        assert_eq!(
            db.playlists().append_to_queue(empty.id).await.unwrap(),
            assignment
        );
        assert_eq!(db.queue().get().await.unwrap(), state);
        let last_item_id = playlist.items.last().unwrap().id;
        let mut playlist = playlist.clone();
        if operation == "unavailable" {
            for item in &mut playlist.items {
                item.audio_source_id = None;
            }
        }
        let tx = db.database.begin().await.unwrap();
        tx.user_data().lock().await.unwrap();
        if operation == "append" {
            crate::QueueService::append_playlist_in(&tx, playlist.items)
                .await
                .unwrap();
        } else {
            let start = match operation {
                "selected" => Some(1),
                "unavailable" => None,
                _ => Some(0),
            };
            crate::QueueService::replace_in(&tx, playlist.items, start, operation == "selected")
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();
        let after = db.queue().get().await.unwrap();
        assert_eq!(after.history_len, 5000);
        let removed = match operation {
            "append" => 1,
            "selected" => 3,
            _ => 2,
        };
        assert_eq!(after.entries[0].id, state.entries[removed].id);
        assert_eq!(
            after.entries.last().unwrap().playlist_item_id,
            Some(last_item_id)
        );
        if operation == "append" {
            assert_eq!(after.current_entry_id, state.current_entry_id);
            assert_eq!(after.playback_token, state.playback_token);
        }
    }
    db.playlists().delete(empty.id).await.unwrap();
}

async fn detached(db: &TestDatabase, playlist: &Playlist) {
    db.queue().clear().await.unwrap();
    db.playlists().play(playlist.id, None).await.unwrap();
    let before = db.queue().snapshot().await.unwrap();
    db.playlists()
        .rename(playlist.id, "Edited T03")
        .await
        .unwrap();
    db.playlists()
        .remove_item(playlist.id, playlist.items[0].id)
        .await
        .unwrap();
    sql(db, &format!("UPDATE playlist_items SET title = 'Edited', title_unicode = 'Edited', difficulty_name = 'Edited' WHERE playlist_id = {}", playlist.id)).await;
    db.playlists().delete(playlist.id).await.unwrap();
    assert_eq!(db.queue().snapshot().await.unwrap(), before);
    super::queue::complete_checks(db).await;
    let next = db.queue().command(Command::Next).await.unwrap();
    assert_eq!(next.current_playlist_item_id, Some(playlist.items[1].id));
    assert_eq!(next.track.as_ref().unwrap().title, playlist.items[1].title);
    assert_eq!(
        next.track.as_ref().unwrap().difficulties[0].difficulty_name,
        playlist.items[1].difficulty_name
    );
    db.queue()
        .play_entry(before.entries[0].entry.id)
        .await
        .unwrap();
    db.queue()
        .move_entry(before.entries[2].entry.id, Some(before.entries[1].entry.id))
        .await
        .unwrap();
    assert_eq!(
        db.queue()
            .command(Command::Next)
            .await
            .unwrap()
            .current_playlist_item_id,
        Some(playlist.items[1].id)
    );
    db.queue()
        .remove_entry(before.entries[0].entry.id)
        .await
        .unwrap();
    assert_eq!(db.queue().get().await.unwrap().entries.len(), 2);
}
