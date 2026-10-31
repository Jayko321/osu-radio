use super::*;
use crate::{
    PlaybackCommand as Command, PlaybackMode as Mode, QueueAvailability as Availability,
    QueueEntryId, QueueError, QueueItemRef as Item, QueuePlacement as Placement, QueueState,
};

pub(super) async fn contracts(db: &TestDatabase, other: &TestDatabase, ids: [i32; 4]) {
    insertions(db, ids).await;
    identity_and_edits(db, ids).await;
    bounded_checks(db, ids[0]).await;
    waiting_and_results(db, other, ids).await;
    playlist_refs(db, ids[0]).await;
    detached_projection(db).await;
    history_limit(db, ids[0]).await;
    rollback(db, other, ids[0]).await;
    db.queue().clear().await.unwrap();
}

async fn save(db: &TestDatabase, state: &QueueState) {
    let tx = db.database.begin().await.unwrap();
    tx.user_data().lock().await.unwrap();
    tx.queue().save(state).await.unwrap();
    tx.commit().await.unwrap();
}

async fn play(db: &TestDatabase) -> crate::PlaybackAssignment {
    db.queue()
        .command(Command::Play {
            audio_source_id: None,
        })
        .await
        .unwrap()
}

async fn insertions(db: &TestDatabase, [a, b, c, x]: [i32; 4]) {
    for placement in [Placement::End, Placement::Next] {
        for mode in [
            None,
            Some(Mode::Stopped),
            Some(Mode::Paused),
            Some(Mode::Playing),
        ] {
            db.queue().clear().await.unwrap();
            db.queue().append(vec![a, b]).await.unwrap();
            if let Some(mode) = mode {
                play(db).await;
                if mode != Mode::Playing {
                    db.queue()
                        .command(if mode == Mode::Paused {
                            Command::Pause
                        } else {
                            Command::Stop
                        })
                        .await
                        .unwrap();
                }
            }
            let before = db.queue().get().await.unwrap();
            let result = db
                .queue()
                .insert(
                    vec![Item::Audio(c), Item::Audio(x), Item::Audio(c)],
                    placement,
                )
                .await
                .unwrap();
            let after = db.queue().get().await.unwrap();
            assert_eq!(after.current_entry_id, before.current_entry_id);
            assert_eq!(result.mode, before.mode);
            assert_eq!(result.playback_token, before.playback_token);
            assert_eq!(after.waiting_for_check, before.waiting_for_check);
            let index = match placement {
                Placement::End => 2,
                Placement::Next => usize::from(mode.is_some()),
            };
            assert_eq!(
                &after.audio_source_ids()[index..index.checked_add(3).unwrap()],
                [c, x, c]
            );
            assert_ne!(
                after.entries[index].id,
                after.entries[index.checked_add(2).unwrap()].id
            );
            super::queue::complete_checks(db).await;
            assert_eq!(db.queue().playback().await.unwrap().mode, before.mode);
        }
    }
    db.queue().clear().await.unwrap();
    db.queue().append(vec![a, b]).await.unwrap();
    play(db).await;
    db.queue().command(Command::Next).await.unwrap();
    db.queue().command(Command::Next).await.unwrap();
    assert_eq!(db.queue().get().await.unwrap().history_len, 2);
    db.queue().append(vec![c, x]).await.unwrap();
    db.queue()
        .insert(vec![Item::Audio(a)], Placement::Next)
        .await
        .unwrap();
    assert_eq!(
        db.queue().get().await.unwrap().audio_source_ids(),
        [a, b, a, c, x]
    );
    assert_eq!(play(db).await.current_audio_source_id, Some(a));
    db.queue().command(Command::Next).await.unwrap();
    assert_eq!(
        db.queue().playback().await.unwrap().current_audio_source_id,
        Some(c)
    );
    db.queue().command(Command::Next).await.unwrap();
    db.queue().command(Command::Next).await.unwrap();
    db.queue()
        .insert(vec![Item::Audio(b)], Placement::Next)
        .await
        .unwrap();
    assert_eq!(play(db).await.current_audio_source_id, Some(b));
    db.queue().command(Command::Next).await.unwrap();
    assert_eq!(
        play(db).await.current_audio_source_id,
        Some(a),
        "explicit replay at exhaustion"
    );
}

#[allow(clippy::too_many_lines)]
async fn identity_and_edits(db: &TestDatabase, [a, b, c, x]: [i32; 4]) {
    db.queue().clear().await.unwrap();
    db.queue().append(vec![a, b, c, x, a]).await.unwrap();
    let original = db.queue().get().await.unwrap();
    let ids = original.entries.iter().map(|e| e.id).collect::<Vec<_>>();
    db.queue().play_entry(ids[2]).await.unwrap();
    let first = db.queue().play_entry(ids[0]).await.unwrap();
    assert_eq!(db.queue().get().await.unwrap().entries, original.entries);
    let next = db.queue().command(Command::Next).await.unwrap();
    assert_eq!(next.current_entry_id, Some(ids[1]));
    assert_eq!(
        db.queue()
            .command(Command::Next)
            .await
            .unwrap()
            .current_entry_id,
        Some(ids[2])
    );
    let duplicate = db.queue().play_entry(ids[4]).await.unwrap();
    assert_eq!(
        duplicate.current_audio_source_id,
        first.current_audio_source_id
    );
    assert_ne!(duplicate.playback_token, first.playback_token);
    db.queue().command(Command::Pause).await.unwrap();
    assert_eq!(
        db.queue().play_entry(ids[4]).await.unwrap().playback_token,
        duplicate.playback_token
    );
    let moved = db.queue().move_entry(ids[4], Some(ids[1])).await.unwrap();
    assert_eq!(moved.current_entry_id, Some(ids[4]));
    assert_eq!(moved.playback_token, duplicate.playback_token);
    assert_eq!(db.queue().get().await.unwrap().history_len, 1);
    assert_eq!(
        db.queue()
            .command(Command::Next)
            .await
            .unwrap()
            .current_entry_id,
        Some(ids[1])
    );
    let before = db.queue().get().await.unwrap();
    let unknown = QueueEntryId::new(i64::MAX).unwrap();
    for error in [
        db.queue().play_entry(unknown).await.unwrap_err(),
        db.queue().remove_entry(unknown).await.unwrap_err(),
        db.queue()
            .move_entry(ids[0], Some(unknown))
            .await
            .unwrap_err(),
    ] {
        assert_eq!(
            error.downcast_ref::<QueueError>(),
            Some(&QueueError::UnknownEntry(unknown))
        );
        assert_eq!(db.queue().get().await.unwrap(), before);
    }
    db.queue().move_entry(ids[1], Some(ids[1])).await.unwrap();
    db.queue().move_entry(ids[1], Some(ids[2])).await.unwrap();
    assert_eq!(
        db.queue().get().await.unwrap(),
        before,
        "no-op moves preserve revision"
    );
    let token = before.playback_token;
    db.queue().remove_entry(ids[0]).await.unwrap();
    assert_eq!(db.queue().get().await.unwrap().history_len, 1);
    db.queue().remove_entry(ids[3]).await.unwrap();
    assert_eq!(db.queue().get().await.unwrap().playback_token, token);
    db.queue().clear_upcoming().await.unwrap();
    assert_eq!(db.queue().get().await.unwrap().entries.len(), 2);
    assert_eq!(db.queue().get().await.unwrap().playback_token, token);
    let same = db.queue().get().await.unwrap();
    db.queue().clear_upcoming().await.unwrap();
    assert_eq!(db.queue().get().await.unwrap(), same);
    db.queue().remove_entry(ids[1]).await.unwrap();
    assert_eq!(db.queue().get().await.unwrap().mode, Mode::Stopped);
    assert!(db.queue().get().await.unwrap().playback_token > token);
    let counter = db.queue().get().await.unwrap().last_entry_id;
    db.queue().clear().await.unwrap();
    assert_eq!(db.queue().get().await.unwrap().last_entry_id, counter);

    for mode in [Mode::Playing, Mode::Paused] {
        for availability in [
            Availability::Checking,
            Availability::Available,
            Availability::Unavailable,
        ] {
            db.queue().clear().await.unwrap();
            db.queue().append(vec![a, b, c]).await.unwrap();
            play(db).await;
            if mode == Mode::Paused {
                db.queue().command(Command::Pause).await.unwrap();
            }
            let mut state = db.queue().get().await.unwrap();
            state.entries[1].availability = availability;
            save(db, &state).await;
            let result = db.queue().remove_entry(state.entries[0].id).await.unwrap();
            assert_eq!(result.mode, mode);
            assert_eq!(
                result.current_entry_id,
                Some(
                    state.entries[if availability == Availability::Unavailable {
                        2
                    } else {
                        1
                    }]
                    .id
                )
            );
            assert_eq!(
                result.waiting_for_check,
                availability != Availability::Available
            );
        }
    }
    // Without a selection, moving preserves the count of past entries; removing past decreases it.
    db.queue().clear().await.unwrap();
    db.queue().append(vec![a, b, c]).await.unwrap();
    let mut state = db.queue().get().await.unwrap();
    state.history_len = 2;
    save(db, &state).await;
    db.queue()
        .move_entry(state.entries[0].id, None)
        .await
        .unwrap();
    assert_eq!(db.queue().get().await.unwrap().history_len, 2);
    db.queue().remove_entry(state.entries[1].id).await.unwrap();
    assert_eq!(db.queue().get().await.unwrap().history_len, 1);
}

#[allow(clippy::too_many_lines)]
async fn waiting_and_results(db: &TestDatabase, other: &TestDatabase, [a, b, c, _]: [i32; 4]) {
    db.queue().clear().await.unwrap();
    db.queue().append(vec![a, b, c, a]).await.unwrap();
    let state = db.queue().get().await.unwrap();
    let waiting = db.queue().play_entry(state.entries[1].id).await.unwrap();
    assert!(waiting.waiting_for_check);
    assert_eq!(
        db.queue().pending_checks().await.unwrap()[0].entry_id,
        state.entries[1].id
    );
    let before = db.queue().snapshot().await.unwrap();
    assert!(before.entries.iter().all(|e| e.last_played_at_ms.is_none()));
    for command in [
        Command::Started {
            playback_token: waiting.playback_token,
        },
        Command::Finished {
            playback_token: waiting.playback_token,
        },
    ] {
        db.queue().command(command).await.unwrap();
        assert_eq!(db.queue().snapshot().await.unwrap(), before);
    }
    db.queue().command(Command::Pause).await.unwrap();
    let check = db.queue().pending_checks().await.unwrap().remove(0);
    let ready = db.queue().apply_check(check.clone(), true).await.unwrap();
    assert!(!ready.waiting_for_check);
    assert_eq!(ready.mode, Mode::Paused);
    assert_eq!(ready.playback_token, waiting.playback_token);
    assert!(
        db.queue()
            .snapshot()
            .await
            .unwrap()
            .entries
            .iter()
            .all(|e| e.last_played_at_ms.is_none())
    );
    assert_eq!(db.queue().apply_check(check, false).await.unwrap(), ready);
    // Only the current launch's Started writes history, and duplicates are idempotent across pools.
    let command = Command::Started {
        playback_token: ready.playback_token,
    };
    let left = db.queue();
    let right = other.queue();
    let (one, two) = tokio::join!(left.command(command), right.command(command));
    assert_eq!(one.unwrap(), two.unwrap());
    db.audio_settings().set_volume(b, Some(37)).await.unwrap();
    let dated = db.queue().snapshot().await.unwrap();
    assert!(dated.entries[1].last_played_at_ms.is_some());
    assert_eq!(dated.entries[1].volume_percent, Some(37));
    db.queue()
        .move_entry(state.entries[1].id, None)
        .await
        .unwrap();
    assert_eq!(
        db.queue()
            .snapshot()
            .await
            .unwrap()
            .entries
            .last()
            .unwrap()
            .last_played_at_ms,
        dated.entries[1].last_played_at_ms
    );
    db.audio_settings().set_volume(b, None).await.unwrap();
    db.queue().clear().await.unwrap();
    db.queue().append(vec![a, b, c]).await.unwrap();
    let waiting = play(db).await;
    let check = db.queue().pending_checks().await.unwrap().remove(0);
    let failed = db.queue().apply_check(check, false).await.unwrap();
    assert_eq!(failed.current_audio_source_id, Some(b));
    assert_eq!(failed.mode, Mode::Playing);
    assert!(failed.waiting_for_check);
    assert!(failed.playback_token > waiting.playback_token);
    let unavailable = db.queue().get().await.unwrap().entries[0].id;
    let before = db.queue().get().await.unwrap();
    assert_eq!(
        db.queue()
            .play_entry(unavailable)
            .await
            .unwrap_err()
            .downcast_ref::<QueueError>(),
        Some(&QueueError::UnavailableEntry(unavailable))
    );
    assert_eq!(db.queue().get().await.unwrap(), before);
    db.queue().command(Command::Pause).await.unwrap();
    let check = db.queue().pending_checks().await.unwrap().remove(0);
    let paused_next = db.queue().apply_check(check, false).await.unwrap();
    assert_eq!(paused_next.current_audio_source_id, Some(c));
    assert_eq!(paused_next.mode, Mode::Paused);
    // An already assigned launch may be rechecked without waiting or interruption (T04 hook).
    super::queue::complete_checks(db).await;
    let mut assigned = db.queue().get().await.unwrap();
    let current = assigned.current_index().unwrap();
    assigned.entries[current].availability = Availability::Checking;
    assigned.entries[current].check_generation = assigned.entries[current]
        .check_generation
        .checked_add(1)
        .unwrap();
    save(db, &assigned).await;
    let check = db.queue().pending_checks().await.unwrap().remove(0);
    let still_assigned = db.queue().apply_check(check, false).await.unwrap();
    assert_eq!(still_assigned.current_entry_id, assigned.current_entry_id);
    assert_eq!(still_assigned.playback_token, assigned.playback_token);
    assert_eq!(still_assigned.mode, assigned.mode);
    let failed = db
        .queue()
        .command(Command::Failed {
            playback_token: still_assigned.playback_token,
        })
        .await
        .unwrap();
    assert_eq!(failed.mode, Mode::Stopped);
    for stale in [
        Command::Started {
            playback_token: still_assigned.playback_token,
        },
        Command::Finished {
            playback_token: still_assigned.playback_token,
        },
        Command::Failed {
            playback_token: still_assigned.playback_token,
        },
    ] {
        assert_eq!(db.queue().command(stale).await.unwrap(), failed);
    }
    db.queue().clear().await.unwrap();
    db.queue().append(vec![a, a]).await.unwrap();
    let first = play(db).await;
    let second = db
        .queue()
        .command(Command::Failed {
            playback_token: first.playback_token,
        })
        .await
        .unwrap();
    let duplicate_state = db.queue().get().await.unwrap();
    assert_eq!(
        duplicate_state.entries[0].availability,
        Availability::Checking
    );
    assert_eq!(
        duplicate_state.entries[1].availability,
        Availability::Checking
    );
    assert_eq!(second.current_audio_source_id, Some(a));
    assert_ne!(second.current_entry_id, first.current_entry_id);
    assert!(second.waiting_for_check);
    db.queue().clear().await.unwrap();
    db.queue().append(vec![a, b]).await.unwrap();
    let check = db.queue().pending_checks().await.unwrap().remove(0);
    let mut newer = db.queue().get().await.unwrap();
    newer.entries[0].check_generation = newer.entries[0].check_generation.checked_add(1).unwrap();
    save(db, &newer).await;
    let current_check = db.queue().pending_checks().await.unwrap().remove(0);
    let mut wrong_id = current_check.clone();
    wrong_id.audio_source_id = b;
    let mut wrong_source = current_check.clone();
    wrong_source.source = SourceType::Local("wrong".into());
    for stale in [check.clone(), wrong_id, wrong_source] {
        db.queue().apply_check(stale, true).await.unwrap();
        assert_eq!(db.queue().get().await.unwrap(), newer);
    }
    let check = db.queue().pending_checks().await.unwrap().remove(0);
    db.queue().remove_entry(check.entry_id).await.unwrap();
    let removed = db.queue().get().await.unwrap();
    db.queue().apply_check(check, true).await.unwrap();
    assert_eq!(db.queue().get().await.unwrap(), removed);
    let check = db.queue().pending_checks().await.unwrap().remove(0);
    db.queue().clear().await.unwrap();
    let cleared = db.queue().get().await.unwrap();
    db.queue().apply_check(check, true).await.unwrap();
    assert_eq!(db.queue().get().await.unwrap(), cleared);
    db.queue().append(vec![a]).await.unwrap();
    let old = play(db).await;
    db.queue().recover().await.unwrap();
    let recovered = db.queue().playback().await.unwrap();
    assert_eq!(recovered.mode, Mode::Paused);
    assert!(recovered.waiting_for_check);
    assert!(recovered.playback_token > old.playback_token);
    assert_eq!(
        db.queue()
            .command(Command::Started {
                playback_token: old.playback_token
            })
            .await
            .unwrap(),
        recovered
    );
}

async fn playlist_refs(db: &TestDatabase, audio: i32) {
    db.queue().clear().await.unwrap();
    let track = db
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.audio_source_id == audio)
        .unwrap();
    let playlist = db.playlists().create("T02 saved captions").await.unwrap();
    let playlist = db
        .playlists()
        .add_items(playlist.id, &[track.difficulties[0].beatmap_id])
        .await
        .unwrap();
    let item = &playlist.items[0];
    let reference = Item::PlaylistItem {
        playlist_id: playlist.id,
        item_id: item.id,
    };
    db.queue()
        .insert(vec![reference, reference], Placement::Next)
        .await
        .unwrap();
    let before = db.queue().get().await.unwrap();
    let error = db
        .queue()
        .insert(
            vec![
                Item::Audio(audio),
                Item::PlaylistItem {
                    playlist_id: -1,
                    item_id: item.id,
                },
            ],
            Placement::End,
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<QueueError>(),
        Some(&QueueError::InvalidPlaylistItem {
            playlist_id: -1,
            item_id: item.id
        })
    );
    assert_eq!(db.queue().get().await.unwrap(), before);
    // Unresolve the saved playlist key without deleting library audio used by other contracts.
    sql(
        db,
        &format!(
            "UPDATE playlist_items SET beatmap_hash = 'missing-t02' WHERE id = {}",
            item.id
        ),
    )
    .await;
    db.queue()
        .insert(vec![reference], Placement::End)
        .await
        .unwrap();
    let saved = db.queue().snapshot().await.unwrap();
    assert_eq!(
        saved.entries[2].entry.availability,
        Availability::Unavailable
    );
    assert_eq!(saved.entries[2].entry.audio_source_id, None);
    assert_eq!(saved.entries[2].entry.title, item.title);
    db.playlists().delete(playlist.id).await.unwrap();
    assert_eq!(
        db.queue().snapshot().await.unwrap(),
        saved,
        "queue captions and keys outlive the playlist"
    );
}

async fn seeded(
    db: &TestDatabase,
    audio: i32,
    count: usize,
    history: usize,
    current: bool,
) -> QueueState {
    db.queue().clear().await.unwrap();
    db.queue().append(vec![audio]).await.unwrap();
    let mut state = db.queue().get().await.unwrap();
    let entry = state.entries.remove(0);
    for position in 0..count {
        let mut entry = entry.clone();
        entry.id = state.allocate_id().unwrap();
        entry.position = position;
        entry.availability = if position < 100 {
            Availability::Unavailable
        } else {
            Availability::Available
        };
        state.entries.push(entry);
    }
    state.history_len = history;
    if current {
        state.current_entry_id = Some(state.entries[history].id);
        state.mode = Mode::Playing;
    }
    save(db, &state).await;
    state
}

#[allow(clippy::too_many_lines)] // Boundary transitions and no-op checks share oversized fixtures.
async fn history_limit(db: &TestDatabase, audio: i32) {
    for past in [4999_usize, 5000, 5001] {
        for operation in ["play", "move", "recover"] {
            let seed = seeded(
                db,
                audio,
                past.checked_add(2).unwrap(),
                if operation == "recover" { past } else { 0 },
                operation != "play",
            )
            .await;
            let target = if operation == "move" {
                seed.entries[0].id
            } else {
                seed.entries[past].id
            };
            match operation {
                "play" => {
                    db.queue().play_entry(target).await.unwrap();
                }
                "move" => {
                    db.queue()
                        .move_entry(target, Some(seed.entries[past.checked_add(1).unwrap()].id))
                        .await
                        .unwrap();
                }
                _ => {
                    db.queue().recover().await.unwrap();
                }
            }
            let state = db.queue().get().await.unwrap();
            assert_eq!(state.history_len, past.min(5000));
            assert_eq!(
                state.entries.len(),
                past.checked_add(2)
                    .unwrap()
                    .checked_sub(past.saturating_sub(5000))
                    .unwrap()
            );
            assert_eq!(state.current_entry_id, Some(target));
            assert_eq!(
                state.entries.last().unwrap().id,
                seed.entries.last().unwrap().id
            );
            if operation != "play" {
                assert_eq!(
                    state.playback_token,
                    seed.playback_token
                        .checked_add(u64::from(operation == "recover"))
                        .unwrap()
                );
            }
        }
    }
    let transition = seeded(db, audio, 5002, 5000, true).await;
    let next = db.queue().command(Command::Next).await.unwrap();
    assert_eq!(next.current_entry_id, Some(transition.entries[5001].id));
    assert_eq!(db.queue().get().await.unwrap().history_len, 5000);
    db.queue().command(Command::Next).await.unwrap();
    let exhausted = db.queue().get().await.unwrap();
    assert_eq!(exhausted.current_entry_id, None);
    assert_eq!(exhausted.history_len, 5000);
    assert_eq!(exhausted.entries[0].id, transition.entries[2].id);
    let seed = seeded(db, audio, 5001, 5001, false).await;
    assert_eq!(
        db.queue()
            .snapshot()
            .await
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.entry)
            .collect::<Vec<_>>(),
        seed.entries,
        "reads never trim"
    );
    db.queue().append(vec![]).await.unwrap();
    db.queue()
        .command(Command::Failed {
            playback_token: seed.playback_token,
        })
        .await
        .unwrap();
    db.queue().clear_upcoming().await.unwrap();
    assert_eq!(
        db.queue().get().await.unwrap(),
        seed,
        "no-op/stale never trim"
    );
    db.queue().recover().await.unwrap();
    let trimmed = db.queue().get().await.unwrap();
    assert_eq!(trimmed.history_len, 5000);
    assert_eq!(trimmed.entries[0].id, seed.entries[1].id);
    db.queue().append(vec![audio]).await.unwrap();
    let appended = db.queue().get().await.unwrap();
    assert_eq!(appended.history_len, 5000);
    assert_eq!(
        play(db).await.current_entry_id,
        Some(appended.entries[5000].id)
    );
}

#[allow(clippy::too_many_lines)] // Injection, overflow and independent writers share the same rollback fixture.
async fn rollback(db: &TestDatabase, other: &TestDatabase, audio: i32) {
    db.queue().clear().await.unwrap();
    db.queue().append(vec![audio, audio]).await.unwrap();
    play(db).await;
    let before = db.queue().snapshot().await.unwrap();
    #[cfg(feature = "sqlite")]
    sql(db, "CREATE TRIGGER reject_t02 BEFORE INSERT ON queue_entries BEGIN SELECT RAISE(ABORT, 'T02 rollback'); END").await;
    #[cfg(feature = "postgres")]
    {
        sql(db, "CREATE FUNCTION reject_t02_fn() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'T02 rollback'; END $$").await;
        sql(db, "CREATE TRIGGER reject_t02 BEFORE INSERT ON queue_entries FOR EACH ROW EXECUTE FUNCTION reject_t02_fn()").await;
    }
    let check = db.queue().pending_checks().await.unwrap().remove(0);
    assert!(db.queue().apply_check(check.clone(), true).await.is_err());
    assert_eq!(other.queue().snapshot().await.unwrap(), before);
    assert!(
        db.queue()
            .remove_entry(before.entries[0].entry.id)
            .await
            .is_err()
    );
    assert_eq!(other.queue().snapshot().await.unwrap(), before);
    #[cfg(feature = "sqlite")]
    sql(db, "DROP TRIGGER reject_t02").await;
    #[cfg(feature = "postgres")]
    {
        sql(db, "DROP TRIGGER reject_t02 ON queue_entries").await;
        sql(db, "DROP FUNCTION reject_t02_fn()").await;
    }
    let revision = before.revision;
    sql(
        db,
        &format!("UPDATE playback_queue SET revision = {}", i64::MAX),
    )
    .await;
    let overflow = db.queue().snapshot().await.unwrap();
    assert!(db.queue().apply_check(check.clone(), true).await.is_err());
    assert_eq!(other.queue().snapshot().await.unwrap(), overflow);
    sql(
        db,
        &format!("UPDATE playback_queue SET revision = {revision}"),
    )
    .await;
    db.queue().apply_check(check.clone(), true).await.unwrap();
    let revision = db.queue().get().await.unwrap().revision;
    sql(
        db,
        &format!("UPDATE playback_queue SET revision = {}", i64::MAX),
    )
    .await;
    let overflow = db.queue().snapshot().await.unwrap();
    assert!(
        db.queue()
            .command(Command::Started {
                playback_token: overflow.playback_token
            })
            .await
            .is_err()
    );
    assert_eq!(other.queue().snapshot().await.unwrap(), overflow);
    sql(
        db,
        &format!("UPDATE playback_queue SET revision = {revision}"),
    )
    .await;
    // Create a new unfinished generation for the independent-connection race.
    let mut checking = db.queue().get().await.unwrap();
    checking.entries[0].availability = Availability::Checking;
    checking.entries[0].check_generation =
        checking.entries[0].check_generation.checked_add(1).unwrap();
    save(db, &checking).await;
    let check = db.queue().pending_checks().await.unwrap().remove(0);
    // Independent connections serialize a check result racing removal; the survivor is unchanged.
    let left = db.queue();
    let right = other.queue();
    let (one, two) = tokio::join!(
        left.apply_check(check.clone(), true),
        right.remove_entry(check.entry_id)
    );
    one.unwrap();
    two.unwrap();
    let after = db.queue().get().await.unwrap();
    assert_eq!(after.entries.len(), 1);
    assert_ne!(after.entries[0].id, check.entry_id);
}

async fn detached_projection(db: &TestDatabase) {
    db.queue().clear().await.unwrap();
    let installation = register(db, "t02-detached").await;
    db.osu_installations()
        .replace_snapshot(installation, &snapshot("Detached", "/t02/detached"))
        .await
        .unwrap();
    let audio = db.beatmap_sets().search_tracks("Detached").await.unwrap()[0].audio_source_id;
    db.audio_settings()
        .set_volume(audio, Some(29))
        .await
        .unwrap();
    db.queue().append(vec![audio, audio]).await.unwrap();
    play(db).await;
    super::queue::complete_checks(db).await;
    let token = db.queue().get().await.unwrap().playback_token;
    db.queue()
        .command(Command::Started {
            playback_token: token,
        })
        .await
        .unwrap();
    let mut state = db.queue().get().await.unwrap();
    for entry in &mut state.entries {
        entry.audio_source_id = None;
        entry.availability = Availability::Unavailable;
    }
    save(db, &state).await;
    db.osu_installations().delete(installation).await.unwrap();
    let after = db.queue().snapshot().await.unwrap();
    assert!(
        after
            .entries
            .iter()
            .all(|e| e.entry.beatmap_id.is_none() && e.entry.cover_beatmap_id.is_none())
    );
    assert!(
        after
            .entries
            .iter()
            .all(|e| e.last_played_at_ms.is_some() && e.volume_percent == Some(29))
    );
    assert!(
        after
            .entries
            .iter()
            .all(|e| e.entry.title.as_deref() == Some("Detached"))
    );
    assert_eq!(
        after.entries[0].last_played_at_ms,
        after.entries[1].last_played_at_ms
    );
}

async fn bounded_checks(db: &TestDatabase, audio: i32) {
    db.queue().clear().await.unwrap();
    db.queue().append(vec![audio; 7]).await.unwrap();
    let state = db.queue().get().await.unwrap();
    db.queue().play_entry(state.entries[6].id).await.unwrap();
    let checks = db.queue().pending_checks().await.unwrap();
    assert_eq!(checks[0].entry_id, state.entries[6].id);
    assert_eq!(
        db.queue().pending_checks_excluding(&[], 1).await.unwrap(),
        checks[..1]
    );
    // Active/failed work must not consume the bounded result slots.
    assert_eq!(
        db.queue()
            .pending_checks_excluding(&checks[..4], 2)
            .await
            .unwrap(),
        checks[4..6]
    );
    assert!(
        db.queue()
            .pending_checks_excluding(&[], 0)
            .await
            .unwrap()
            .is_empty()
    );
    let mut outdated = checks[0].clone();
    outdated.generation = outdated.generation.checked_add(1).unwrap();
    assert_eq!(
        db.queue()
            .pending_checks_excluding(&[outdated], 1)
            .await
            .unwrap(),
        checks[..1]
    );
    let mut outdated = checks[0].clone();
    outdated.audio_source_id = outdated.audio_source_id.checked_add(1).unwrap();
    assert_eq!(
        db.queue()
            .pending_checks_excluding(&[outdated], 1)
            .await
            .unwrap(),
        checks[..1]
    );
    db.queue().clear().await.unwrap();
    db.queue().append(vec![audio; 1_003]).await.unwrap();
    let checks = db.queue().pending_checks().await.unwrap();
    assert_eq!(
        db.queue()
            .pending_checks_excluding(&checks[..1_001], 1)
            .await
            .unwrap(),
        checks[1_001..1_002]
    );
    assert_eq!(
        db.queue().revision().await.unwrap(),
        db.queue().get().await.unwrap().revision
    );
}
