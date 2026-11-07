#![allow(clippy::indexing_slicing)]
use super::*;
use crate::playback::Snapshot;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

fn sample(state: PlayerState) -> (Track, Playback) {
    let mut track = Track::new("crystallized", "Camellia", Duration::from_secs(252));
    track.audio_source_id = 42;
    let playback = Playback {
        current_audio_id: Some(42),
        has_source: true,
        playback_token: 7,
        snapshot: Snapshot {
            state,
            position: Duration::from_secs(84),
            duration: Some(Duration::from_secs(252)),
            ..Snapshot::default()
        },
        ..Playback::default()
    };
    (track, playback)
}
#[test]
fn listening_metadata_progress_pause_resume_and_unknown_duration() {
    let (track, mut playback) = sample(PlayerState::Playing);
    let presence = Presence::from_playback(Some(&track), &playback).unwrap();
    let json = serde_json::to_value(presence.activity()).unwrap();
    assert_eq!(json["type"], 2);
    assert_eq!(json["details"], "crystallized");
    assert_eq!(json["state"], "Camellia");
    assert_eq!(
        json["timestamps"]["end"]
            .as_i64()
            .unwrap()
            .checked_sub(json["timestamps"]["start"].as_i64().unwrap())
            .unwrap(),
        252
    );
    assert!(json.get("audio_source_id").is_none() && json.get("playback_token").is_none());
    playback.snapshot.state = PlayerState::Paused;
    let paused = Presence::from_playback(Some(&track), &playback).unwrap();
    let json = serde_json::to_value(paused.activity()).unwrap();
    assert!(json.get("timestamps").is_none());
    assert!(json["state"].as_str().unwrap().contains("Paused"));
    playback.snapshot.state = PlayerState::Playing;
    assert!(
        serde_json::to_value(
            Presence::from_playback(Some(&track), &playback)
                .unwrap()
                .activity()
        )
        .unwrap()
        .get("timestamps")
        .is_some()
    );
    playback.snapshot.duration = None;
    assert!(
        serde_json::to_value(
            Presence::from_playback(Some(&track), &playback)
                .unwrap()
                .activity()
        )
        .unwrap()
        .get("timestamps")
        .is_none()
    );
    playback.snapshot.duration = Some(Duration::ZERO);
    assert!(
        Presence::from_playback(Some(&track), &playback)
            .unwrap()
            .key
            .duration
            .is_none()
    );
}
#[test]
fn speed_scales_source_time_once_and_rate_only_change_republishes_even_paused() {
    let (track, mut playback) = sample(PlayerState::Playing);
    let fast = |playback: &mut Playback, speed| {
        playback.snapshot.parameters = crate::playback::PlaybackParameters {
            speed,
            adjust_pitch: false,
        };
    };
    playback.snapshot.duration = Some(Duration::from_secs(240));
    playback.snapshot.position = Duration::from_secs(90);
    fast(&mut playback, 1.5);
    let presence = Presence::from_playback(Some(&track), &playback).unwrap();
    assert_eq!(presence.key.duration, Some(Duration::from_secs(160)));
    assert_eq!(presence.position, Duration::from_secs(60));
    let json = serde_json::to_value(presence.activity()).unwrap();
    assert_eq!(
        json["timestamps"]["end"].as_i64().unwrap() - json["timestamps"]["start"].as_i64().unwrap(),
        160
    );
    playback.snapshot.state = PlayerState::Paused;
    let paused = Presence::from_playback(Some(&track), &playback).unwrap();
    assert!(paused.paused_label.starts_with("Paused • 1:00 • "));
    assert!(
        serde_json::to_value(paused.activity())
            .unwrap()
            .get("timestamps")
            .is_none()
    );
    // Rate-only change: same state/seek/token/duration source, new key and a recomputed label.
    let mut service = Service::default();
    service.update(Some(&track), &playback);
    let (sender, receiver) = watch::channel(service.desired.clone());
    service.sender = Some(sender);
    fast(&mut playback, 0.75);
    service.update(Some(&track), &playback);
    assert!(
        receiver.has_changed().unwrap(),
        "rate change wakes IPC while paused"
    );
    let slow = service.desired.presence.as_ref().unwrap();
    assert!(slow.paused_label.starts_with("Paused • 2:00 • "));
    assert_eq!(slow.key.duration, Some(Duration::from_secs(320)));
    // Unknown end stays timestamp-free at any speed.
    playback.snapshot.state = PlayerState::Playing;
    playback.snapshot.duration = None;
    assert!(
        serde_json::to_value(
            Presence::from_playback(Some(&track), &playback)
                .unwrap()
                .activity()
        )
        .unwrap()
        .get("timestamps")
        .is_none()
    );
}
#[test]
fn empty_unicode_and_long_metadata_never_leak_paths_or_ids() {
    let (mut track, playback) = sample(PlayerState::Playing);
    track.title = "  ".into();
    track.artist = String::new();
    let presence = Presence::from_playback(Some(&track), &playback).unwrap();
    assert_eq!(presence.key.title, "Unknown title");
    assert_eq!(presence.key.artist, "Unknown artist");
    track.title_unicode = Some("曲".repeat(100));
    track.artist_unicode = Some("かめりあ".into());
    track.apply_name_preferences(crate::TrackNamePreferences {
        use_unicode_titles: true,
        use_unicode_artists: true,
    });
    let presence = Presence::from_playback(Some(&track), &playback).unwrap();
    assert!(presence.key.title.len() <= 128 && presence.key.title.starts_with('曲'));
    assert_eq!(presence.key.artist, "かめりあ");
    assert_eq!(presence.key.title, "曲".repeat(42));
    assert_eq!(field(&"a".repeat(129), "unused"), "a".repeat(128));
    assert_eq!(field("  曲  ", "unused"), "曲");
}

#[test]
fn eof_and_extreme_decoder_times_never_publish_invalid_timestamps() {
    let (track, mut playback) = sample(PlayerState::Playing);
    for (position, duration) in [
        (Duration::from_secs(252), Duration::from_secs(252)),
        (Duration::from_secs(253), Duration::from_secs(252)),
        (Duration::MAX, Duration::from_secs(252)),
        (Duration::from_secs(1), Duration::MAX),
    ] {
        playback.snapshot.position = position;
        playback.snapshot.duration = Some(duration);
        let presence = Presence::from_playback(Some(&track), &playback).unwrap();
        let json = serde_json::to_value(presence.activity()).unwrap();
        assert!(json.get("timestamps").is_none());
        assert_eq!(json["details"], "crystallized");
        assert_eq!(json["state"], "Camellia");
    }
}
#[test]
fn source_loading_errors_stop_end_and_wrong_id_clear_presence() {
    let (track, playback) = sample(PlayerState::Playing);
    assert!(Presence::from_playback(None, &playback).is_none());
    for state in [PlayerState::Stopped, PlayerState::Empty, PlayerState::Ended] {
        let mut playback = playback.clone();
        playback.snapshot.state = state;
        assert!(Presence::from_playback(Some(&track), &playback).is_none());
    }
    let mut loading = playback.clone();
    loading.loading_audio_id = Some(42);
    assert!(Presence::from_playback(Some(&track), &loading).is_none());
    let mut error = playback.clone();
    error.error = Some("decoder error".into());
    assert!(Presence::from_playback(Some(&track), &error).is_none());
    let mut wrong = playback;
    wrong.current_audio_id = Some(43);
    assert!(Presence::from_playback(Some(&track), &wrong).is_none());
    wrong.current_audio_id = Some(42);
    wrong.has_source = false;
    assert!(Presence::from_playback(Some(&track), &wrong).is_none());
}
#[test]
fn position_ticks_coalesce_but_seek_token_state_and_metadata_changes_publish() {
    let (mut track, mut playback) = sample(PlayerState::Playing);
    let mut service = Service::default();
    service.update(Some(&track), &playback);
    let (sender, mut receiver) = watch::channel(service.desired.clone());
    service.sender = Some(sender);
    playback.snapshot.position = Duration::from_secs(90);
    service.update(Some(&track), &playback);
    assert_eq!(
        receiver.borrow().presence.as_ref().unwrap().position,
        playback.snapshot.position
    );
    assert!(
        !receiver.has_changed().unwrap(),
        "position tick does not wake IPC"
    );
    playback.seek_serial = 1;
    service.update(Some(&track), &playback);
    assert!(receiver.has_changed().unwrap());
    assert_eq!(
        receiver
            .borrow_and_update()
            .presence
            .as_ref()
            .unwrap()
            .key
            .seek,
        1
    );
    assert_eq!(
        service.desired.presence.as_ref().unwrap().position,
        playback.snapshot.position
    );
    playback.playback_token = 8;
    service.update(Some(&track), &playback);
    assert!(receiver.has_changed().unwrap());
    assert_eq!(
        receiver
            .borrow_and_update()
            .presence
            .as_ref()
            .unwrap()
            .key
            .token,
        8
    );
    assert_eq!(service.desired.presence.as_ref().unwrap().key.token, 8);
    track.title = "次の曲".into();
    service.update(Some(&track), &playback);
    assert!(receiver.has_changed().unwrap());
    assert_eq!(
        receiver
            .borrow_and_update()
            .presence
            .as_ref()
            .unwrap()
            .key
            .title,
        "次の曲"
    );
    assert_eq!(
        service.desired.presence.as_ref().unwrap().key.title,
        "次の曲"
    );
    playback.snapshot.state = PlayerState::Paused;
    service.update(Some(&track), &playback);
    assert!(receiver.has_changed().unwrap());
    assert_eq!(
        receiver
            .borrow_and_update()
            .presence
            .as_ref()
            .unwrap()
            .key
            .state,
        PlayerState::Paused
    );
    assert_eq!(
        service.desired.presence.as_ref().unwrap().key.state,
        PlayerState::Paused
    );
    playback.snapshot.duration = Some(Duration::from_secs(300));
    service.update(Some(&track), &playback);
    assert!(receiver.has_changed().unwrap());
    assert_eq!(
        receiver
            .borrow_and_update()
            .presence
            .as_ref()
            .unwrap()
            .key
            .duration,
        Some(Duration::from_secs(300))
    );
    playback.has_source = false;
    service.update(Some(&track), &playback);
    assert!(receiver.has_changed().unwrap());
    assert!(receiver.borrow_and_update().presence.is_none());
    service.update(None, &playback);
    assert!(!receiver.has_changed().unwrap());
}
#[tokio::test]
async fn disabled_default_never_starts_worker_and_shutdown_is_idempotent() {
    let mut service = Service::default();
    service.enabled(false);
    assert!(service.worker.is_none() && service.sender.is_none());
    service.shutdown().await;
    service.shutdown().await;
}
#[derive(Default)]
struct MockState {
    events: Mutex<Vec<String>>,
    available: AtomicBool,
    fail_set: AtomicBool,
    broken: AtomicBool,
    notify: tokio::sync::Notify,
}
struct Mock(Arc<MockState>);
impl Rpc for Mock {
    fn connect(&mut self) -> Result<(), Error> {
        self.0.events.lock().unwrap().push("connect".into());
        if self.0.available.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(Error::IPCNotFound)
        }
    }
    fn set(&mut self, presence: &Presence, _artwork: bool) -> Result<String, Error> {
        self.0
            .events
            .lock()
            .unwrap()
            .push(format!("set:{}", presence.key.title));
        if self.0.fail_set.swap(false, Ordering::SeqCst) {
            Err(Error::IPCConnectionFailed)
        } else {
            Ok(presence.key.title.clone())
        }
    }
    fn clear(&mut self) -> Result<String, Error> {
        self.0.events.lock().unwrap().push("clear".into());
        Ok("clear".into())
    }
    fn close(&mut self) {
        self.0.events.lock().unwrap().push("close".into());
    }
    async fn incoming(&mut self) -> Result<(), Error> {
        self.0.notify.notified().await;
        Ok(())
    }
    fn receive(&mut self) -> Result<Reply, Error> {
        if self.0.broken.load(Ordering::SeqCst) {
            Err(Error::IPCConnectionFailed)
        } else {
            Ok(Reply::Other)
        }
    }
}
async fn wait_event(state: &MockState, name: &str, count: usize) {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            if state
                .events
                .lock()
                .unwrap()
                .iter()
                .filter(|value| value.as_str() == name)
                .count()
                >= count
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn absent_discord_retry_restores_latest_state_idle_disconnect_disable_reenable_shutdown() {
    let (track, playback) = sample(PlayerState::Playing);
    let (sender, receiver) = watch::channel(Desired {
        enabled: true,
        presence: Presence::from_playback(Some(&track), &playback),
        shutdown: false,
    });
    let state = Arc::new(MockState::default());
    let mock = Mock(state.clone());
    let runtime = tokio::runtime::Handle::current();
    let task = tokio::task::spawn_blocking(move || run(mock, receiver, &runtime));
    wait_event(&state, "connect", 1).await;
    sender.send_modify(|value| value.presence.as_mut().unwrap().key.title = "latest".into());
    state.available.store(true, Ordering::SeqCst);
    wait_event(&state, "set:latest", 1).await;
    assert!(
        !state
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|value| value == "set:crystallized")
    );
    sender.send_modify(|_| {}); // Equivalent snapshot is never resent.
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(
        state
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|value| value.as_str() == "set:latest")
            .count(),
        1
    );
    state.broken.store(true, Ordering::SeqCst);
    state.notify.notify_one();
    wait_event(&state, "set:latest", 2).await;
    state.broken.store(false, Ordering::SeqCst);
    sender.send_modify(|value| value.enabled = false);
    wait_event(&state, "clear", 1).await;
    let connects = state
        .events
        .lock()
        .unwrap()
        .iter()
        .filter(|value| value.as_str() == "connect")
        .count();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        state
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|value| value.as_str() == "connect")
            .count(),
        connects
    );
    sender.send_modify(|value| value.enabled = true);
    wait_event(&state, "set:latest", 3).await;
    sender.send_modify(|value| value.shutdown = true);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.events.lock().unwrap().last().unwrap(), "close");
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_interrupts_reconnect_backoff() {
    let (sender, receiver) = watch::channel(Desired {
        enabled: true,
        ..Desired::default()
    });
    let state = Arc::new(MockState::default());
    let mock = Mock(state.clone());
    let runtime = tokio::runtime::Handle::current();
    let task = tokio::task::spawn_blocking(move || run(mock, receiver, &runtime));
    wait_event(&state, "connect", 1).await;
    sender.send_modify(|value| value.shutdown = true);
    tokio::time::timeout(Duration::from_millis(200), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn service_reuses_one_worker_and_both_teardown_paths_stop_it() {
    for drop_service in [false, true] {
        let (track, playback) = sample(PlayerState::Playing);
        let mut service = Service::default();
        service.update(Some(&track), &playback);
        let (sender, receiver) = watch::channel(service.desired.clone());
        let state = Arc::new(MockState::default());
        state.available.store(true, Ordering::SeqCst);
        let mock = Mock(state.clone());
        let runtime = tokio::runtime::Handle::current();
        // Install the existing mock seam so enabling cannot search a user's Discord.
        service.worker = Some(tokio::task::spawn_blocking(move || {
            run(mock, receiver, &runtime);
        }));
        service.sender = Some(sender);
        let worker_id = service.worker.as_ref().unwrap().id();
        service.enabled(true);
        wait_event(&state, "set:crystallized", 1).await;
        service.enabled(true);
        assert_eq!(service.worker.as_ref().unwrap().id(), worker_id);
        service.enabled(false);
        wait_event(&state, "clear", 1).await;
        service.enabled(true);
        wait_event(&state, "set:crystallized", 2).await;
        if drop_service {
            let task = service.worker.take().unwrap();
            drop(service);
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .unwrap()
                .unwrap();
        } else {
            tokio::time::timeout(Duration::from_secs(2), service.shutdown())
                .await
                .unwrap();
            assert!(service.worker.is_none() && service.sender.is_none());
            service.shutdown().await;
        }
        let events = state.events.lock().unwrap();
        assert_eq!(events.iter().filter(|event| *event == "connect").count(), 2);
        assert_eq!(events.last().unwrap(), "close");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_activity_write_reconnects_and_replays_current_snapshot() {
    let (track, playback) = sample(PlayerState::Playing);
    let (sender, receiver) = watch::channel(Desired {
        enabled: true,
        presence: Presence::from_playback(Some(&track), &playback),
        shutdown: false,
    });
    let state = Arc::new(MockState::default());
    state.available.store(true, Ordering::SeqCst);
    state.fail_set.store(true, Ordering::SeqCst);
    let mock = Mock(state.clone());
    let runtime = tokio::runtime::Handle::current();
    let task = tokio::task::spawn_blocking(move || run(mock, receiver, &runtime));
    wait_event(&state, "set:crystallized", 1).await;
    sender.send_modify(|value| {
        value.presence.as_mut().unwrap().key.title = "new after failure".into();
    });
    wait_event(&state, "set:new after failure", 1).await;
    assert_eq!(
        state
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|value| value.as_str() == "connect")
            .count(),
        2
    );
    sender.send_modify(|value| value.shutdown = true);
    task.await.unwrap();
}

#[test]
fn long_paused_artists_preserve_time_and_utf8_limit() {
    let (mut track, playback) = sample(PlayerState::Paused);
    for artist in ["a".repeat(200), "かめりあ".repeat(100)] {
        track.artist = artist;
        let presence = Presence::from_playback(Some(&track), &playback).unwrap();
        assert!(presence.paused_label.starts_with("Paused • 1:24 • "));
        assert!(presence.paused_label.len() <= 128);
        assert_eq!(
            serde_json::to_value(presence.activity()).unwrap()["state"],
            presence.paused_label
        );
    }
    track.title = "a".into();
    track.artist = "曲".into();
    let presence = Presence::from_playback(Some(&track), &playback).unwrap();
    assert_eq!(presence.key.title, "a");
    assert_eq!(presence.key.artist, "曲");
}

#[test]
fn cover_comes_from_first_difficulty_with_a_public_set_id() {
    let (mut track, playback) = sample(PlayerState::Playing);
    let presence = |track: &Track| Presence::from_playback(Some(track), &playback).unwrap();
    assert_eq!(presence(&track).cover_url(), None);
    let difficulty = |online| crate::models::TrackDifficulty {
        beatmap_id: 1,
        beatmap_set_id: 9,
        beatmap_set_online_id: online,
        difficulty_name: None,
        set_has_multiple_audio_sources: false,
    };
    track.difficulties = vec![difficulty(None), difficulty(Some(123))];
    assert_eq!(
        presence(&track).cover_url().as_deref(),
        Some("https://assets.ppy.sh/beatmaps/123/covers/list@2x.jpg")
    );
}

#[cfg(unix)]
mod ipc;

#[tokio::test]
async fn every_transport_failure_grows_retry_up_to_thirty_seconds() {
    let mut worker = Worker {
        rpc: Mock(Arc::new(MockState::default())),
        connected: true,
        sent: None,
        pending: None,
        use_assets: true,
        retry: Duration::from_secs(1),
        deadline: Instant::now(),
    };
    for delay in [1, 2, 4, 8, 16, 30, 30] {
        let before = Instant::now();
        worker.failed("test failure", &Error::IPCConnectionFailed);
        assert!(worker.deadline.saturating_duration_since(before) >= Duration::from_secs(delay));
        assert_eq!(
            worker.retry,
            Duration::from_secs(delay)
                .saturating_mul(2)
                .min(Duration::from_secs(30))
        );
        assert!(!worker.connected);
    }
}
