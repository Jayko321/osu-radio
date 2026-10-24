use super::queue_modes::{cmd, entry_ids, shuffle, start};
use super::*;
use crate::{PlaybackAssignment, PlaybackCommand as Command, PlaybackMode as Mode, RepeatMode};

pub(super) async fn contracts(db: &TestDatabase, [a, b, c]: [i32; 3]) {
    repeat_one(db, [a, b, c]).await;
    repeat_all(db, [a, b, c]).await;
    errors_and_exhaustion(db, [a, b, c]).await;
    db.queue().clear().await.unwrap();
    shuffle(db, false).await;
    cmd(db, Command::SetRepeat(RepeatMode::Off)).await;
}

async fn repeat(db: &TestDatabase, mode: RepeatMode) {
    cmd(db, Command::SetRepeat(mode)).await;
}
async fn set_kind(db: &TestDatabase, id: i32, kind: &str) {
    sql(
        db,
        &format!("UPDATE audio_sources SET kind = '{kind}' WHERE id = {id}"),
    )
    .await;
}
/// Acknowledges a launch and returns the listening date shown for it.
async fn listen(db: &TestDatabase, token: u64) -> Option<i64> {
    let assignment = cmd(
        db,
        Command::Started {
            playback_token: token,
        },
    )
    .await;
    assignment.track.unwrap().last_played_at_ms
}
fn current(assignment: &PlaybackAssignment) -> Option<i32> {
    assignment.current_audio_source_id
}
fn pause() {
    // The date has millisecond resolution; a change must be observable.
    std::thread::sleep(std::time::Duration::from_millis(10));
}

async fn repeat_one(db: &TestDatabase, [a, b, c]: [i32; 3]) {
    let first = start(db, &[a, b, c]).await;
    repeat(db, RepeatMode::One).await;
    let played = listen(db, first.playback_token).await;
    assert!(played.is_some());
    pause();

    let again = cmd(
        db,
        Command::Finished {
            playback_token: first.playback_token,
        },
    )
    .await;
    assert_eq!(again.current_entry_id, first.current_entry_id);
    assert!(again.playback_token > first.playback_token);
    assert_eq!(again.mode, Mode::Playing);
    assert!(db.queue().get().await.unwrap().repeat_launch);
    assert_eq!(listen(db, again.playback_token).await, played);
    // A stale callback of the finished run does nothing.
    let before = db.queue().playback().await.unwrap();
    assert_eq!(
        cmd(
            db,
            Command::Finished {
                playback_token: first.playback_token
            }
        )
        .await,
        before
    );

    // Manual transitions walk the list, keep the mode and count as new listens.
    let next = cmd(db, Command::Next).await;
    super::queue::complete_checks(db).await;
    assert_eq!((current(&next), next.repeat), (Some(b), RepeatMode::One));
    assert!(!db.queue().get().await.unwrap().repeat_launch);
    let b_before = db
        .queue()
        .playback()
        .await
        .unwrap()
        .track
        .unwrap()
        .last_played_at_ms;
    let b_played = listen(db, next.playback_token).await;
    assert!(b_played.is_some() && b_played != b_before);
    let previous = cmd(db, Command::Previous).await;
    assert_eq!(current(&previous), Some(a));
    assert_eq!(previous.repeat, RepeatMode::One);

    // A repeat of an unavailable current entry moves on instead.
    set_kind(db, a, "online").await;
    let moved = cmd(
        db,
        Command::Finished {
            playback_token: previous.playback_token,
        },
    )
    .await;
    set_kind(db, a, "local").await;
    assert_eq!(current(&moved), Some(b));
    assert!(!db.queue().get().await.unwrap().repeat_launch);

    // Failure and the end of the list do not wrap under One.
    let failed = cmd(
        db,
        Command::Failed {
            playback_token: moved.playback_token,
        },
    )
    .await;
    assert_eq!(current(&failed), Some(c));
    assert_eq!(
        cmd(db, Command::Next).await.mode,
        Mode::Stopped,
        "Next at the end must not wrap"
    );
    assert!(!db.queue().playback().await.unwrap().can_next);
    repeat(db, RepeatMode::Off).await;
}

async fn repeat_all(db: &TestDatabase, [a, b, c]: [i32; 3]) {
    for shuffled in [false, true] {
        start(db, &[a, b, c, a]).await;
        shuffle(db, shuffled).await;
        repeat(db, RepeatMode::All).await;
        let order = entry_ids(&db.queue().get().await.unwrap());
        for _ in 0..3 {
            cmd(db, Command::Next).await;
        }
        let last = db.queue().playback().await.unwrap();
        assert_eq!(last.current_entry_id, order.last().copied());
        assert!(last.can_next);

        let wrapped = cmd(db, Command::Next).await;
        assert_eq!(wrapped.current_entry_id, order.first().copied());
        assert!(wrapped.playback_token > last.playback_token);
        assert_eq!(entry_ids(&db.queue().get().await.unwrap()), order);

        // Natural end of the last entry wraps too; Previous on the first one restarts it.
        for _ in 0..3 {
            cmd(db, Command::Next).await;
        }
        let last = db.queue().playback().await.unwrap();
        let wrapped = cmd(
            db,
            Command::Finished {
                playback_token: last.playback_token,
            },
        )
        .await;
        assert_eq!(wrapped.current_entry_id, order.first().copied());
        assert_eq!(wrapped.mode, Mode::Playing);
        let restarted = cmd(db, Command::Previous).await;
        assert_eq!(restarted.current_entry_id, order.first().copied());
        assert!(restarted.playback_token > wrapped.playback_token);
        assert_eq!(db.queue().get().await.unwrap().current_index(), Some(0));
        assert_eq!(entry_ids(&db.queue().get().await.unwrap()), order);
    }
    shuffle(db, false).await;

    // Unavailable entries are skipped on the way around.
    start(db, &[a, b, c]).await;
    repeat(db, RepeatMode::All).await;
    set_kind(db, a, "online").await;
    cmd(db, Command::Next).await;
    cmd(db, Command::Next).await;
    let wrapped = cmd(db, Command::Next).await;
    set_kind(db, a, "local").await;
    assert_eq!(current(&wrapped), Some(b));

    // With Off the last entry is a dead end and `can_next` says so.
    repeat(db, RepeatMode::Off).await;
    let end = cmd(db, Command::Next).await;
    assert_eq!(current(&end), Some(c));
    assert!(!end.can_next);
    repeat(db, RepeatMode::All).await;
    assert!(db.queue().playback().await.unwrap().can_next);
    repeat(db, RepeatMode::Off).await;
}

async fn errors_and_exhaustion(db: &TestDatabase, [a, b, c]: [i32; 3]) {
    // Failure at the end wraps to an earlier available entry.
    let first = start(db, &[a, b]).await;
    repeat(db, RepeatMode::All).await;
    let at_b = cmd(db, Command::Next).await;
    let failed = cmd(
        db,
        Command::Failed {
            playback_token: at_b.playback_token,
        },
    )
    .await;
    assert_eq!(failed.current_entry_id, first.current_entry_id);
    assert_eq!(failed.mode, Mode::Playing);

    // Nothing available: stop instead of looping.
    set_kind(db, a, "online").await;
    set_kind(db, b, "online").await;
    let stopped = cmd(
        db,
        Command::Finished {
            playback_token: failed.playback_token,
        },
    )
    .await;
    set_kind(db, a, "local").await;
    set_kind(db, b, "local").await;
    assert_eq!(
        (stopped.mode, stopped.current_entry_id),
        (Mode::Stopped, None)
    );
    assert!(!stopped.can_next);
    assert!(db.queue().playback().await.unwrap().can_next);

    // A lone entry that fails is not retried, even when it is not marked unavailable.
    for stale_launch in [false, true] {
        let lone = start(db, &[a]).await;
        repeat(db, RepeatMode::All).await;
        if stale_launch {
            sql(
                db,
                "UPDATE queue_entries SET check_generation = check_generation + 1",
            )
            .await;
        }
        let stopped = cmd(
            db,
            Command::Failed {
                playback_token: lone.playback_token,
            },
        )
        .await;
        assert_eq!(
            (stopped.mode, stopped.current_entry_id),
            (Mode::Stopped, None),
            "stale_launch = {stale_launch}"
        );
    }

    // Removing the current last entry continues from the head under All.
    start(db, &[a, b, c]).await;
    repeat(db, RepeatMode::All).await;
    cmd(db, Command::Next).await;
    let last = cmd(db, Command::Next).await;
    let removed = db
        .queue()
        .remove_entry(last.current_entry_id.unwrap())
        .await
        .unwrap();
    assert_eq!(current(&removed), Some(a));
    assert_eq!(removed.mode, Mode::Playing);

    // A failed availability check of the last entry wraps as well.
    start(db, &[a, b]).await;
    db.queue().append(vec![c]).await.unwrap();
    cmd(db, Command::Next).await;
    let waiting = cmd(db, Command::Next).await;
    assert!(waiting.waiting_for_check);
    let check = db.queue().pending_checks().await.unwrap().remove(0);
    let wrapped = db.queue().apply_check(check, false).await.unwrap();
    assert_eq!(current(&wrapped), Some(a));
    repeat(db, RepeatMode::Off).await;
}
