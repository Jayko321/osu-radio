use super::*;
use crate::test_support::{beatmap_set, empty_state, state_with};
use radio_services::{
    PlaybackCommand, PlaybackMode, QueueAvailability, QueueItemRef, QueuePlacement,
};
use std::sync::mpsc;
use tokio::{sync::mpsc as async_mpsc, time::timeout};

const DEADLINE: Duration = Duration::from_secs(5);

struct Operation {
    check: QueueCheck,
    result: mpsc::Sender<Option<bool>>,
}

impl Operation {
    fn finish(self, available: bool) {
        self.result.send(Some(available)).unwrap();
    }
}

fn controlled(state: AppState) -> (QueueCheckWorker, async_mpsc::UnboundedReceiver<Operation>) {
    let (sender, receiver) = async_mpsc::unbounded_channel();
    let worker = QueueCheckWorker::start_with_check(state, move |check| {
        let (result, completed) = mpsc::channel();
        sender
            .send(Operation {
                check: check.clone(),
                result,
            })
            .unwrap();
        completed
            .recv_timeout(DEADLINE)
            .unwrap()
            .expect("injected task failure")
    });
    (worker, receiver)
}

async fn next(receiver: &mut async_mpsc::UnboundedReceiver<Operation>) -> Operation {
    timeout(DEADLINE, receiver.recv()).await.unwrap().unwrap()
}

async fn add(state: &AppState, source: SourceType, count: usize) {
    let audio = state
        .services()
        .audio_sources()
        .get_or_insert(&source)
        .await
        .unwrap();
    state
        .services()
        .queue()
        .append(vec![audio.id; count])
        .await
        .unwrap();
}

async fn settled(state: &AppState) -> radio_services::QueueState {
    timeout(DEADLINE, async {
        loop {
            let queue = state.services().queue().get().await.unwrap();
            if queue
                .entries
                .iter()
                .all(|entry| entry.availability != QueueAvailability::Checking)
            {
                return queue;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}

async fn play(state: &AppState) {
    state
        .services()
        .queue()
        .command(PlaybackCommand::Play {
            audio_source_id: None,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn files_are_checked_without_decoding_and_append_never_starts() {
    let directory = tempfile::tempdir().unwrap();
    let regular = directory.path().join("not-audio");
    std::fs::write(&regular, b"not encoded audio").unwrap();
    let state = empty_state().await;
    for source in [
        SourceType::Local(regular.to_str().unwrap().into()),
        SourceType::Copied(regular.to_str().unwrap().into()),
        SourceType::Local(directory.path().join("absent").to_str().unwrap().into()),
        SourceType::Local(directory.path().to_str().unwrap().into()),
    ] {
        add(&state, source, 1).await;
    }
    let initial = state.services().queue().get().await.unwrap();
    let worker = QueueCheckWorker::start(state.clone());
    let queue = settled(&state).await;
    worker.shutdown().await.unwrap();
    assert_eq!(
        queue
            .entries
            .iter()
            .map(|e| e.availability)
            .collect::<Vec<_>>(),
        vec![
            QueueAvailability::Available,
            QueueAvailability::Available,
            QueueAvailability::Unavailable,
            QueueAvailability::Unavailable,
        ]
    );
    assert_eq!(queue.mode, PlaybackMode::Stopped);
    assert_eq!(queue.playback_token, initial.playback_token);
    assert!(queue.current_entry_id.is_none());
    assert!(
        state
            .services()
            .queue()
            .snapshot()
            .await
            .unwrap()
            .entries
            .iter()
            .all(|e| e.last_played_at_ms.is_none())
    );
}

#[cfg(unix)]
#[test]
fn fifo_is_rejected_before_open_and_regression_is_unblocked() {
    let directory = tempfile::tempdir().unwrap();
    let fifo = directory.path().join("audio-fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let (sender, receiver) = mpsc::channel();
    let path = fifo.clone();
    let operation = std::thread::spawn(move || {
        sender.send(open_audio_file(&path).is_ok()).unwrap();
    });
    let result = receiver.recv_timeout(Duration::from_secs(2));
    if result.is_err() {
        drop(
            std::fs::File::options()
                .read(true)
                .write(true)
                .open(&fifo)
                .unwrap(),
        );
    }
    operation.join().unwrap();
    assert!(!result.unwrap());
}

#[tokio::test]
async fn four_slots_no_duplicates_and_new_waiting_entry_has_priority() {
    let state = empty_state().await;
    add(&state, SourceType::Local("/controlled".into()), 7).await;
    let entries = state.services().queue().get().await.unwrap().entries;
    let (worker, mut operations) = controlled(state.clone());
    let mut first = Vec::new();
    for _ in 0..4 {
        let operation = next(&mut operations).await;
        first.push(operation);
    }
    // Blocking tasks may start in any order; compare the complete first batch below.
    let mut ids = first.iter().map(|o| o.check.entry_id).collect::<Vec<_>>();
    ids.sort();
    assert_eq!(
        ids,
        entries.iter().take(4).map(|e| e.id).collect::<Vec<_>>()
    );
    for _ in 0..3 {
        state.wake_queue_checks();
    }
    assert!(
        timeout(Duration::from_millis(60), operations.recv())
            .await
            .is_err()
    );
    state
        .services()
        .queue()
        .play_entry(entries[6].id)
        .await
        .unwrap();
    first.pop().unwrap().finish(true);
    let current = next(&mut operations).await;
    assert_eq!(current.check.entry_id, entries[6].id);
    current.finish(true);
    for operation in first {
        operation.finish(true);
    }
    for _ in 0..2 {
        next(&mut operations).await.finish(true);
    }
    settled(&state).await;
    worker.shutdown().await.unwrap();
    assert!(operations.recv().await.is_none());
}

#[tokio::test]
async fn task_failure_retries_on_tick_without_false_revision() {
    let state = empty_state().await;
    add(&state, SourceType::Local("/panic".into()), 1).await;
    let (worker, mut operations) = controlled(state.clone());
    let first = next(&mut operations).await;
    let revision = state.services().queue().get().await.unwrap().revision;
    let mut updates = state.subscribe_playback();
    // Synchronize with publication of the pre-existing append.
    if *updates.borrow_and_update() < revision {
        updates.changed().await.unwrap();
    }
    updates.borrow_and_update();
    first.result.send(None).unwrap();
    assert!(
        timeout(Duration::from_millis(100), operations.recv())
            .await
            .is_err()
    );
    assert_eq!(
        state.services().queue().get().await.unwrap().revision,
        revision
    );
    assert!(!updates.has_changed().unwrap());
    let retry = next(&mut operations).await;
    assert_eq!(retry.check, first.check);
    retry.finish(true);
    let queue = settled(&state).await;
    assert_eq!(queue.revision, revision.checked_add(1).unwrap());
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn shutdown_waits_for_io_discards_late_results_and_drop_stops_supervisor() {
    let state = empty_state().await;
    add(&state, SourceType::Local("/shutdown".into()), 2).await;
    let (worker, mut operations) = controlled(state.clone());
    let first = next(&mut operations).await;
    let second = next(&mut operations).await;
    let before = state.services().queue().get().await.unwrap();
    let shutdown = worker.shutdown();
    tokio::pin!(shutdown);
    assert!(
        timeout(Duration::from_millis(40), &mut shutdown)
            .await
            .is_err()
    );
    first.finish(true);
    assert!(
        timeout(Duration::from_millis(40), &mut shutdown)
            .await
            .is_err()
    );
    second.finish(false);
    timeout(DEADLINE, shutdown).await.unwrap().unwrap();
    assert!(operations.recv().await.is_none());
    assert_eq!(state.services().queue().get().await.unwrap(), before);

    let (worker, mut operations) = controlled(state.clone());
    let first = next(&mut operations).await;
    let second = next(&mut operations).await;
    drop(worker);
    first.finish(true);
    second.finish(true);
    assert!(
        timeout(DEADLINE, operations.recv())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(state.services().queue().get().await.unwrap(), before);
}

#[tokio::test]
async fn restart_picks_only_checking_and_periodic_scan_recovers_lost_wakeup() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("queue.sqlite");
    let state = AppState::connect(database.to_str().unwrap()).await.unwrap();
    add(&state, SourceType::Local("/restart".into()), 3).await;
    let checks = state.services().queue().pending_checks().await.unwrap();
    state
        .services()
        .queue()
        .apply_check(checks[0].clone(), true)
        .await
        .unwrap();
    state
        .services()
        .queue()
        .apply_check(checks[1].clone(), false)
        .await
        .unwrap();
    play(&state).await;
    drop(state);
    let state = AppState::connect(database.to_str().unwrap()).await.unwrap();
    assert_eq!(
        state.services().queue().get().await.unwrap().mode,
        PlaybackMode::Paused
    );
    let (worker, mut operations) = controlled(state.clone());
    let operation = next(&mut operations).await;
    assert_eq!(operation.check, checks[2]);
    operation.finish(true);
    settled(&state).await;
    assert!(
        timeout(Duration::from_millis(40), operations.recv())
            .await
            .is_err()
    );
    add(&state, SourceType::Local("/lost-notification".into()), 1).await;
    next(&mut operations).await.finish(false);
    settled(&state).await;
    worker.shutdown().await.unwrap();
}

#[tokio::test]
async fn waiting_success_and_failure_preserve_playing_and_paused_without_started() {
    for mode in [PlaybackMode::Playing, PlaybackMode::Paused] {
        for available in [true, false] {
            let state = empty_state().await;
            add(&state, SourceType::Local("/waiting".into()), 2).await;
            play(&state).await;
            if mode == PlaybackMode::Paused {
                state
                    .services()
                    .queue()
                    .command(PlaybackCommand::Pause)
                    .await
                    .unwrap();
            }
            let before = state.services().queue().get().await.unwrap();
            let (worker, mut operations) = controlled(state.clone());
            let a = next(&mut operations).await;
            let b = next(&mut operations).await;
            let (current, following) = if a.check.entry_id == before.current_entry_id.unwrap() {
                (a, b)
            } else {
                (b, a)
            };
            following.finish(true);
            current.finish(available);
            let queue = settled(&state).await;
            worker.shutdown().await.unwrap();
            assert_eq!(queue.mode, mode);
            assert!(!queue.waiting_for_check);
            if available {
                assert_eq!(queue.current_entry_id, before.current_entry_id);
                assert_eq!(queue.playback_token, before.playback_token);
            } else {
                assert_eq!(queue.current_entry_id, Some(before.entries[1].id));
                assert!(queue.playback_token > before.playback_token);
            }
            assert!(
                state
                    .services()
                    .queue()
                    .snapshot()
                    .await
                    .unwrap()
                    .entries
                    .iter()
                    .all(|e| e.last_played_at_ms.is_none())
            );
        }
    }
}

#[tokio::test]
async fn removed_and_cleared_entries_ignore_inflight_results_without_notifications() {
    for clear in [false, true] {
        let state = empty_state().await;
        add(&state, SourceType::Local("/removed".into()), 1).await;
        let (worker, mut operations) = controlled(state.clone());
        let operation = next(&mut operations).await;
        if clear {
            state.services().queue().clear().await.unwrap();
        } else {
            state
                .services()
                .queue()
                .remove_entry(operation.check.entry_id)
                .await
                .unwrap();
        }
        let before = state.services().queue().get().await.unwrap();
        state.publish_playback(before.revision);
        let updates = state.subscribe_playback();
        operation.finish(true);
        assert!(
            timeout(Duration::from_millis(80), operations.recv())
                .await
                .is_err()
        );
        assert_eq!(state.services().queue().get().await.unwrap(), before);
        assert!(!updates.has_changed().unwrap());
        add(&state, SourceType::Local("/after-remove".into()), 1).await;
        let new = next(&mut operations).await;
        assert!(new.check.entry_id.get() > before.last_entry_id);
        assert_eq!(
            state.services().queue().get().await.unwrap().revision,
            before.revision.checked_add(1).unwrap()
        );
        new.finish(true);
        settled(&state).await;
        worker.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn reimport_rejects_old_generation_and_source_without_rebinding_active_launch() {
    let state = state_with(&[beatmap_set(1, "/old")]).await;
    let maps = state
        .services()
        .beatmaps()
        .for_set(
            state
                .services()
                .beatmap_sets()
                .all_with_audio_sources()
                .await
                .unwrap()[0]
                .beatmap_set
                .id,
        )
        .await
        .unwrap();
    let playlist = state
        .services()
        .playlists()
        .create("checking")
        .await
        .unwrap();
    let playlist = state
        .services()
        .playlists()
        .add_items(playlist.id, &[maps[0].id])
        .await
        .unwrap();
    state
        .services()
        .queue()
        .insert(
            vec![QueueItemRef::PlaylistItem {
                playlist_id: playlist.id,
                item_id: playlist.items[0].id,
            }],
            QueuePlacement::End,
        )
        .await
        .unwrap();
    let (worker, mut operations) = controlled(state.clone());
    next(&mut operations).await.finish(true);
    settled(&state).await;
    play(&state).await;
    let launch = state.services().queue().get().await.unwrap();
    state
        .services()
        .osu_installations()
        .replace_snapshot(1, &[beatmap_set(1, "/new")])
        .await
        .unwrap();
    state.wake_queue_checks();
    let old = next(&mut operations).await;
    state
        .services()
        .osu_installations()
        .replace_snapshot(1, &[beatmap_set(1, "/newer")])
        .await
        .unwrap();
    state.wake_queue_checks();
    let new = next(&mut operations).await;
    assert_eq!(old.check.entry_id, new.check.entry_id);
    assert!(old.check.generation < new.check.generation);
    assert_ne!(old.check.source, new.check.source);
    new.finish(true);
    let before = settled(&state).await;
    state.publish_playback(before.revision);
    let mut updates = state.subscribe_playback();
    old.finish(false);
    assert!(
        timeout(Duration::from_millis(80), updates.changed())
            .await
            .is_err()
    );
    worker.shutdown().await.unwrap();
    assert_eq!(state.services().queue().get().await.unwrap(), before);
    assert_eq!(before.launch_source, launch.launch_source);
    assert_eq!(before.playback_token, launch.playback_token);
    assert!(!before.waiting_for_check);
}

async fn playlist(state: &AppState) -> i32 {
    let track = &state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()[0];
    let playlist = state
        .services()
        .playlists()
        .create("worker playlist")
        .await
        .unwrap();
    state
        .services()
        .playlists()
        .add_items(playlist.id, &[track.difficulties[0].beatmap_id])
        .await
        .unwrap();
    playlist.id
}

#[tokio::test]
async fn restored_fully_unavailable_playlist_stays_stopped_and_old_result_is_ignored() {
    let state = state_with(&[beatmap_set(1, "/playlist")]).await;
    let playlist = playlist(&state).await;
    state
        .services()
        .playlists()
        .play(playlist, None)
        .await
        .unwrap();
    let (worker, mut operations) = controlled(state.clone());
    let old = next(&mut operations).await;
    state
        .services()
        .osu_installations()
        .replace_snapshot(1, &[])
        .await
        .unwrap();
    let stopped = state
        .services()
        .playlists()
        .play(playlist, None)
        .await
        .unwrap();
    assert_eq!(stopped.mode, PlaybackMode::Stopped);
    let before = state.services().queue().get().await.unwrap();
    old.finish(true);
    assert!(
        timeout(Duration::from_millis(80), operations.recv())
            .await
            .is_err()
    );
    assert_eq!(state.services().queue().get().await.unwrap(), before);
    state
        .services()
        .osu_installations()
        .replace_snapshot(1, &[beatmap_set(1, "/playlist")])
        .await
        .unwrap();
    // Intentionally omit the wakeup, as with a cancelled import response after commit.
    // Play retained the old launch in history, so restoration checks both queue entries.
    let count = state
        .services()
        .queue()
        .pending_checks()
        .await
        .unwrap()
        .len();
    for _ in 0..count {
        next(&mut operations).await.finish(true);
    }
    let restored = settled(&state).await;
    worker.shutdown().await.unwrap();
    assert_eq!(restored.mode, PlaybackMode::Stopped);
    assert_eq!(restored.playback_token, stopped.playback_token);
    assert!(restored.current_entry_id.is_none());
    assert_eq!(restored.entries[0].id, before.entries[0].id);
    assert_eq!(
        restored.entries[0].availability,
        QueueAvailability::Available
    );
}

#[tokio::test]
async fn committed_revisions_reach_ndjson_and_folder_deletion_is_published_without_checks() {
    use axum::{body::Body, extract::State, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn frame(body: &mut Body) -> serde_json::Value {
        let bytes = timeout(DEADLINE, body.frame())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .into_data()
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    let state = state_with(&[beatmap_set(1, "/events")]).await;
    let audio = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()[0]
        .audio_source_id;
    let (worker, mut operations) = controlled(state.clone());
    let mut body = crate::routes::playback::events(State(state.clone()))
        .await
        .unwrap()
        .into_body();
    let initial = frame(&mut body).await;
    let response = crate::routes::router(state.clone())
        .oneshot(
            Request::post("/api/queue/items")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"items": [{"type":"audio", "audio_source_id":audio}], "placement":"end"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(response.status().is_success());
    let operation = next(&mut operations).await;
    let appended = frame(&mut body).await;
    assert!(appended["revision"].as_u64().unwrap() > initial["revision"].as_u64().unwrap());
    operation.finish(true);
    settled(&state).await;
    let available = frame(&mut body).await;
    assert!(available["revision"].as_u64().unwrap() > appended["revision"].as_u64().unwrap());
    assert_eq!(available["mode"], "stopped");
    assert_eq!(available["playback_token"], initial["playback_token"]);
    let rejected = crate::routes::router(state.clone())
        .oneshot(
            Request::post("/api/queue/items")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({"items":[{"type":"audio", "audio_source_id":i32::MAX}], "placement":"end"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), axum::http::StatusCode::NOT_FOUND);
    let unchanged = state.services().queue().get().await.unwrap();
    let mut watch = state.subscribe_playback();
    state.services().queue().append(Vec::new()).await.unwrap();
    state.wake_queue_checks();
    assert!(
        timeout(Duration::from_millis(80), watch.changed())
            .await
            .is_err()
    );
    assert_eq!(state.services().queue().get().await.unwrap(), unchanged);
    // A direct service commit simulates a lost route notification; no checking work remains.
    state
        .services()
        .osu_installations()
        .delete(1)
        .await
        .unwrap();
    timeout(DEADLINE, watch.changed()).await.unwrap().unwrap();
    let removed = state.services().queue().get().await.unwrap();
    assert_eq!(*watch.borrow(), removed.revision);
    assert_eq!(
        removed.entries[0].availability,
        QueueAvailability::Unavailable
    );
    let notification = frame(&mut body).await;
    assert_eq!(notification["revision"], removed.revision);
    assert!(removed.revision > unchanged.revision);
    worker.shutdown().await.unwrap();
    assert_eq!(state.playback_subscriber_count(), 2);
    drop(body);
    drop(watch);
    assert_eq!(state.playback_subscriber_count(), 0);
}

#[tokio::test]
async fn folder_route_wakes_worker_and_late_check_cannot_restore_deleted_source() {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    let state = state_with(&[beatmap_set(1, "/deleted-folder")]).await;
    let audio = state
        .services()
        .beatmap_sets()
        .search_tracks("")
        .await
        .unwrap()[0]
        .audio_source_id;
    state.services().queue().append(vec![audio]).await.unwrap();
    let (worker, mut operations) = controlled(state.clone());
    let operation = next(&mut operations).await;
    let response = crate::routes::router(state.clone())
        .oneshot(
            Request::delete("/api/user-data/osu-folders/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);
    let before = state.services().queue().get().await.unwrap();
    assert_eq!(
        before.entries[0].availability,
        QueueAvailability::Unavailable
    );
    assert!(before.entries[0].audio_source_id.is_none());
    let mut watch = state.subscribe_playback();
    if *watch.borrow_and_update() < before.revision {
        timeout(Duration::from_millis(500), watch.changed())
            .await
            .unwrap()
            .unwrap();
    }
    watch.borrow_and_update();
    operation.finish(true);
    assert!(
        timeout(Duration::from_millis(80), watch.changed())
            .await
            .is_err()
    );
    assert_eq!(state.services().queue().get().await.unwrap(), before);
    worker.shutdown().await.unwrap();
}
