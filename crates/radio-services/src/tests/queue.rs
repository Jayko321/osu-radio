use super::*;
use crate::{PlaybackAssignment, PlaybackCommand as Command, PlaybackMode as Mode, QueueError};

pub(super) async fn contracts(database: &TestDatabase, other: &TestDatabase, url: &str) {
    let mut installations = Vec::new();
    let mut ids = Vec::new();
    for title in ["A", "B", "C", "X"] {
        let installation = register(database, &format!("queue-{title}")).await;
        database
            .osu_installations()
            .replace_snapshot(installation, &snapshot(title, &format!("/queue/{title}")))
            .await
            .unwrap();
        installations.push(installation);
        ids.push(
            database
                .beatmap_sets()
                .search_tracks(title)
                .await
                .unwrap()
                .into_iter()
                .find(|track| track.title.as_deref() == Some(title))
                .unwrap()
                .audio_source_id,
        );
    }
    let [a, b, c, x] = <[i32; 4]>::try_from(ids).unwrap();
    validation_is_atomic(database, a).await;
    insertion_and_history(database, [a, b, c, x]).await;
    pause_stop_failures(database, [a, b, c]).await;
    device_pause_is_bound_to_launch(database, [a, b]).await;
    concurrent_callbacks_and_writes(database, other, [a, b, c, x]).await;
    recovery_and_reopen(database, url, a).await;
    missing_sources_are_skipped(database, [a, b, c], &installations).await;
    assignment_metadata(database, a, installations[0]).await;
    database.queue().clear().await.unwrap();
    for id in installations {
        database.osu_installations().delete(id).await.unwrap();
    }
}

async fn command(database: &TestDatabase, command: Command) -> PlaybackAssignment {
    database.queue().command(command).await.unwrap()
}

async fn validation_is_atomic(database: &TestDatabase, a: i32) {
    let online = database
        .audio_sources()
        .get_or_insert(&SourceType::Online("https://example.invalid/audio".into()))
        .await
        .unwrap()
        .id;
    for bad in [i32::MAX, online] {
        let before = database.queue().get().await.unwrap();
        let error = database.queue().append(vec![a, bad, a]).await.unwrap_err();
        assert_eq!(
            error.downcast_ref::<QueueError>(),
            Some(&QueueError::InvalidAudioSource(bad))
        );
        assert_eq!(database.queue().get().await.unwrap(), before);
        assert!(
            database
                .queue()
                .command(Command::Play {
                    audio_source_id: Some(bad)
                })
                .await
                .is_err()
        );
        assert_eq!(database.queue().get().await.unwrap(), before);
    }
    let before = database.queue().get().await.unwrap();
    database.queue().append(vec![]).await.unwrap();
    assert_eq!(database.queue().get().await.unwrap(), before);
    let copied = database
        .audio_sources()
        .get_or_insert(&SourceType::Copied("/queue/copied".into()))
        .await
        .unwrap()
        .id;
    assert_eq!(
        database
            .queue()
            .append(vec![copied])
            .await
            .unwrap()
            .current_audio_source_id,
        Some(copied)
    );
    database.queue().clear().await.unwrap();
}

#[allow(clippy::too_many_lines)]
async fn insertion_and_history(database: &TestDatabase, [a, b, c, x]: [i32; 4]) {
    let first = database.queue().append(vec![a, b, c]).await.unwrap();
    assert_eq!(first.current_audio_source_id, Some(a));
    assert_eq!(first.mode, Mode::Playing);
    assert!(first.can_next && first.can_previous);
    let same = command(
        database,
        Command::Play {
            audio_source_id: Some(a),
        },
    )
    .await;
    assert_eq!(same, first);
    let inserted = command(
        database,
        Command::Play {
            audio_source_id: Some(x),
        },
    )
    .await;
    assert_eq!(inserted.current_audio_source_id, Some(x));
    assert!(inserted.playback_token > first.playback_token);
    let queue = database.queue().get().await.unwrap();
    assert_eq!(queue.audio_source_ids, [a, x, b, c]);
    assert_eq!(queue.current_index, Some(1));
    let stale = command(
        database,
        Command::Finished {
            playback_token: first.playback_token,
        },
    )
    .await;
    assert_eq!(stale, inserted);
    let next = command(
        database,
        Command::Finished {
            playback_token: inserted.playback_token,
        },
    )
    .await;
    assert_eq!(next.current_audio_source_id, Some(b));
    assert_eq!(
        command(
            database,
            Command::Finished {
                playback_token: inserted.playback_token
            }
        )
        .await,
        next
    );
    let previous = command(database, Command::Previous).await;
    assert_eq!(previous.current_audio_source_id, Some(x));
    assert_eq!(
        command(database, Command::Previous)
            .await
            .current_audio_source_id,
        Some(a)
    );
    let restarted = command(database, Command::Previous).await;
    assert_eq!(restarted.current_audio_source_id, Some(a));
    assert!(restarted.playback_token > previous.playback_token);
    command(database, Command::Next).await;
    command(database, Command::Next).await;
    let last = command(database, Command::Next).await;
    assert_eq!(last.current_audio_source_id, Some(c));
    assert!(!last.can_next);
    let exhausted = command(
        database,
        Command::Finished {
            playback_token: last.playback_token,
        },
    )
    .await;
    assert_eq!(exhausted.mode, Mode::Stopped);
    assert_eq!(exhausted.current_audio_source_id, None);
    assert!(exhausted.can_previous);
    assert_eq!(database.queue().get().await.unwrap().current_index, Some(4));
    assert_eq!(command(database, Command::Next).await, exhausted);
    let appended = database.queue().append(vec![a, a]).await.unwrap();
    assert_eq!(appended.mode, Mode::Playing);
    assert_eq!(database.queue().get().await.unwrap().current_index, Some(4));
    let duplicate = command(
        database,
        Command::Finished {
            playback_token: appended.playback_token,
        },
    )
    .await;
    assert_eq!(duplicate.current_audio_source_id, Some(a));
    assert!(duplicate.playback_token > appended.playback_token);
    command(database, Command::Next).await;
    let replay = command(
        database,
        Command::Play {
            audio_source_id: None,
        },
    )
    .await;
    assert_eq!(replay.current_audio_source_id, Some(a));
    assert_eq!(database.queue().get().await.unwrap().current_index, Some(0));
    let cleared = database.queue().clear().await.unwrap();
    assert_eq!(cleared.mode, Mode::Stopped);
    assert!(cleared.playback_token > replay.playback_token);
    assert_eq!(cleared.current_audio_source_id, None);
    assert!(!cleared.can_next && !cleared.can_previous);
    assert_eq!(database.queue().clear().await.unwrap(), cleared);
}

async fn pause_stop_failures(database: &TestDatabase, [a, b, c]: [i32; 3]) {
    let first = database.queue().append(vec![a, b]).await.unwrap();
    let paused = command(database, Command::Pause).await;
    assert_eq!(paused.mode, Mode::Paused);
    assert_eq!(paused.playback_token, first.playback_token);
    assert_eq!(
        command(
            database,
            Command::Finished {
                playback_token: first.playback_token
            }
        )
        .await,
        paused
    );
    let appended = database.queue().append(vec![c]).await.unwrap();
    assert_eq!(appended.mode, Mode::Paused);
    assert_eq!(appended.current_audio_source_id, Some(a));
    assert_eq!(appended.playback_token, paused.playback_token);
    let resumed = command(
        database,
        Command::Play {
            audio_source_id: Some(a),
        },
    )
    .await;
    assert_eq!(resumed.mode, Mode::Playing);
    assert_eq!(resumed.playback_token, paused.playback_token);
    command(database, Command::Pause).await;
    let next = command(database, Command::Next).await;
    assert_eq!(next.mode, Mode::Paused);
    assert_eq!(next.current_audio_source_id, Some(b));
    let previous = command(database, Command::Previous).await;
    assert_eq!(previous.mode, Mode::Paused);
    assert_eq!(previous.current_audio_source_id, Some(a));
    let failed = command(
        database,
        Command::Failed {
            playback_token: previous.playback_token,
        },
    )
    .await;
    assert_eq!(failed.current_audio_source_id, Some(b));
    assert_eq!(failed.mode, Mode::Paused);
    assert_eq!(
        command(
            database,
            Command::Failed {
                playback_token: previous.playback_token
            }
        )
        .await,
        failed
    );
    let stopped = command(database, Command::Stop).await;
    assert_eq!(stopped.mode, Mode::Stopped);
    assert_eq!(stopped.current_audio_source_id, Some(b));
    assert!(stopped.playback_token > failed.playback_token);
    let restarted = command(
        database,
        Command::Play {
            audio_source_id: None,
        },
    )
    .await;
    assert_eq!(restarted.current_audio_source_id, Some(b));
    assert!(restarted.playback_token > stopped.playback_token);
    let failed = command(
        database,
        Command::Failed {
            playback_token: restarted.playback_token,
        },
    )
    .await;
    assert_eq!(failed.current_audio_source_id, Some(c));
    assert_eq!(failed.mode, Mode::Playing);
    let exhausted = command(
        database,
        Command::Failed {
            playback_token: failed.playback_token,
        },
    )
    .await;
    assert_eq!(exhausted.mode, Mode::Stopped);
    assert_eq!(
        command(database, Command::Previous)
            .await
            .current_audio_source_id,
        Some(c)
    );
    assert_eq!(
        database.queue().playback().await.unwrap().mode,
        Mode::Paused
    );
    database.queue().clear().await.unwrap();
}

async fn device_pause_is_bound_to_launch(database: &TestDatabase, [a, b]: [i32; 2]) {
    let first = database.queue().append(vec![a, b]).await.unwrap();
    let pause = Command::PauseIfCurrent {
        playback_token: first.playback_token,
    };
    let paused = command(database, pause).await;
    assert_eq!(paused.mode, Mode::Paused);
    assert_eq!(paused.playback_token, first.playback_token);
    assert_eq!(command(database, pause).await, paused);
    let next = command(database, Command::Next).await;
    let playing = command(
        database,
        Command::Play {
            audio_source_id: None,
        },
    )
    .await;
    assert_eq!(playing.current_audio_source_id, Some(b));
    assert_eq!(playing.mode, Mode::Playing);
    assert_eq!(playing.playback_token, next.playback_token);
    assert_eq!(command(database, pause).await, playing);
    database.queue().clear().await.unwrap();
}

async fn concurrent_callbacks_and_writes(
    database: &TestDatabase,
    other: &TestDatabase,
    [a, b, c, x]: [i32; 4],
) {
    let first = database.queue().append(vec![a, b, c]).await.unwrap();
    let left = database.queue();
    let right = other.queue();
    let finished = Command::Finished {
        playback_token: first.playback_token,
    };
    let (one, two) = tokio::join!(left.command(finished), right.command(finished));
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one, two);
    assert_eq!(one.current_audio_source_id, Some(b));
    assert_eq!(one.revision, first.revision.checked_add(1).unwrap());
    let (one, two) = tokio::join!(left.append(vec![a, x]), right.append(vec![c, b]));
    one.unwrap();
    two.unwrap();
    let queue = left.get().await.unwrap();
    assert!(
        queue.audio_source_ids == [a, b, c, a, x, c, b]
            || queue.audio_source_ids == [a, b, c, c, b, a, x]
    );
    assert_eq!(queue.revision, first.revision.checked_add(3).unwrap());
    let (one, two) = tokio::join!(
        left.clear(),
        right.command(Command::Finished {
            playback_token: queue.playback_token
        })
    );
    one.unwrap();
    two.unwrap();
    assert!(left.get().await.unwrap().audio_source_ids.is_empty());
    assert_eq!(left.playback().await.unwrap().mode, Mode::Stopped);
}

async fn recovery_and_reopen(database: &TestDatabase, url: &str, a: i32) {
    let playing = database.queue().append(vec![a, a]).await.unwrap();
    let old = database.queue().get().await.unwrap();
    let reopened = Services::connect(url).await.unwrap();
    reopened.migrate().await.unwrap();
    assert_eq!(reopened.queue().get().await.unwrap(), old);
    reopened.queue().recover().await.unwrap();
    let paused = reopened.queue().playback().await.unwrap();
    assert_eq!(paused.mode, Mode::Paused);
    assert!(paused.playback_token > playing.playback_token);
    assert_eq!(
        reopened.queue().get().await.unwrap().audio_source_ids,
        old.audio_source_ids
    );
    assert_eq!(
        reopened
            .queue()
            .command(Command::Failed {
                playback_token: playing.playback_token
            })
            .await
            .unwrap(),
        paused
    );
    reopened.queue().recover().await.unwrap();
    let again = reopened.queue().playback().await.unwrap();
    assert!(again.playback_token > paused.playback_token);
    let resumed = reopened
        .queue()
        .command(Command::Play {
            audio_source_id: None,
        })
        .await
        .unwrap();
    assert_eq!(resumed.mode, Mode::Playing);
    assert_eq!(resumed.playback_token, again.playback_token);
    let stopped = command(database, Command::Stop).await;
    assert_eq!(stopped.mode, Mode::Stopped);
    reopened.queue().recover().await.unwrap();
    let recovered_stop = reopened.queue().playback().await.unwrap();
    assert_eq!(recovered_stop.mode, Mode::Paused);
    assert_eq!(recovered_stop.current_audio_source_id, Some(a));
    assert!(recovered_stop.playback_token > stopped.playback_token);
    reopened.queue().clear().await.unwrap();
    let before = reopened.queue().get().await.unwrap();
    reopened.queue().recover().await.unwrap();
    assert_eq!(reopened.queue().get().await.unwrap(), before);
}

async fn missing_sources_are_skipped(
    database: &TestDatabase,
    [a, b, c]: [i32; 3],
    installations: &[i32],
) {
    database.queue().append(vec![a, b, c, a]).await.unwrap();
    database
        .osu_installations()
        .delete(installations[1])
        .await
        .unwrap();
    sql(
        database,
        &format!("UPDATE audio_sources SET kind = 'online' WHERE id = {c}"),
    )
    .await;
    assert_eq!(
        command(database, Command::Next)
            .await
            .current_audio_source_id,
        Some(a)
    );
    assert_eq!(database.queue().get().await.unwrap().current_index, Some(3));
    assert_eq!(
        command(database, Command::Previous)
            .await
            .current_audio_source_id,
        Some(a)
    );
    assert_eq!(database.queue().get().await.unwrap().current_index, Some(0));
    assert!(database.queue().playback().await.unwrap().can_next);
    database.queue().clear().await.unwrap();
    sql(
        database,
        &format!("UPDATE audio_sources SET kind = 'local' WHERE id = {c}"),
    )
    .await;
}

async fn assignment_metadata(database: &TestDatabase, a: i32, installation: i32) {
    let mut imported = snapshot("A", "/queue/A");
    let mut split = imported[0].beatmaps[0].clone();
    split.metadata.as_mut().unwrap().audio_file = Some("other.mp3".into());
    imported[0].beatmaps.push(split);
    imported[0].files.push(RealmNamedFileUsage {
        filename: Some("other.mp3".into()),
        file: Some(RealmFile {
            hash: None,
            resolved_path: Some("/queue/other".into()),
        }),
    });
    imported.extend(snapshot("Alias", "/queue/A"));
    database
        .osu_installations()
        .replace_snapshot(installation, &imported)
        .await
        .unwrap();
    let expected = database
        .beatmap_sets()
        .search_tracks("A")
        .await
        .unwrap()
        .into_iter()
        .find(|track| track.audio_source_id == a)
        .unwrap();
    assert_eq!(expected.difficulties.len(), 4);
    assert_eq!(
        expected
            .difficulties
            .iter()
            .filter(|difficulty| difficulty.set_has_multiple_audio_sources)
            .count(),
        2
    );
    assert!(expected.cover_beatmap_id.is_some());
    assert_eq!(
        database.queue().append(vec![a]).await.unwrap().track,
        Some(expected)
    );
    assert!(
        database
            .beatmap_sets()
            .search_tracks("unmatched text")
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        database
            .queue()
            .playback()
            .await
            .unwrap()
            .track
            .unwrap()
            .audio_source_id,
        a
    );
}
