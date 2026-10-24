use super::*;
use crate::controller::{
    AppCommand, AppController,
    tests::{controller, track},
    view_model_tests,
};
use crate::models::{PlaybackMode, QueueEntryId};
use crate::playback::{Command, Playback, PlaybackParameters, PlayerState, Worker};
use osu_radio_media_controls::{Backend, LaunchKey, PlaybackStatus, test_backend};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, watch},
    task::JoinHandle,
};

const QUEUE: &str = r#"{"entries":[],"current_entry_id":null,"history_len":0,"mode":"stopped","waiting_for_check":false,"revision":0,"playback_token":0}"#;
const ASSIGNMENT: &str = r#"{"current_entry_id":"1","current_audio_source_id":1,"current_playlist_item_id":31,"track":null,"duration_ms":null,"waiting_for_check":false,"mode":"playing","revision":1,"playback_token":5,"can_next":true,"can_previous":false}"#;

// Existing HTTP fixture pattern, using a remote session so these checks run on Windows too.
struct HttpFixture {
    requests: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
    task: JoinHandle<()>,
}
impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn connected() -> (Controller, HttpFixture) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                header.push(stream.read_u8().await.unwrap());
            }
            let header = String::from_utf8(header).unwrap();
            let length = header
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .map_or(0, |length| length.parse::<usize>().unwrap());
            let mut body = vec![0; length];
            stream.read_exact(&mut body).await.unwrap();
            let body = if body.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::from_slice(&body).unwrap()
            };
            captured
                .lock()
                .unwrap()
                .push((header.lines().next().unwrap().to_owned(), body));
            let payload = if header.starts_with("GET /api/queue ") {
                QUEUE
            } else if header.starts_with("POST ") {
                ASSIGNMENT
            } else if header.starts_with("PATCH ") {
                r#"{"individual_volume_enabled":false,"global_volume_percent":40}"#
            } else if header.contains("/cover ") {
                "encoded artwork fixture"
            } else {
                ""
            };
            let status = if header.starts_with("GET /api/beatmaps/303/cover ") {
                "404 Not Found"
            } else {
                "200 OK"
            };
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len()).as_bytes()).await.unwrap();
        }
    });
    let session = crate::Session::start(crate::ServerOptions {
        remote_url: Some(format!("http://{address}")),
        binary: Some("/must-not-start-a-local-server".into()),
        ..Default::default()
    })
    .await
    .unwrap();
    let (mut state, _) = controller();
    state.session = Some(session);
    state.replace_tracks(vec![track(1), track(2)], true);
    let mut assignment = view_model_tests::assignment(PlaybackMode::Playing);
    assignment.current_entry_id = Some(QueueEntryId::new(1).unwrap());
    state.player.assignment = Some(assignment);
    let mut current = track(1);
    current.duration = Some(Duration::from_secs(60));
    state.player.current_track = Some(current);
    state.player.playback = Playback {
        current_entry_id: Some(QueueEntryId::new(1).unwrap()),
        current_audio_id: Some(1),
        playback_token: 5,
        has_source: true,
        ..Default::default()
    };
    state.player.playback.snapshot.state = PlayerState::Playing;
    state.player.playback.snapshot.position = Duration::from_secs(9);
    (state, HttpFixture { requests, task })
}

fn attach(state: &mut Controller) -> Backend {
    let (controls, backend, events) = test_backend(state.system_media_snapshot());
    state.system_media = Some(controls);
    state.system_events = Some(events);
    assert!(backend.connected());
    state.reconcile_media();
    state.publish_system_media();
    backend
}

async fn detach(state: &mut Controller, mut backend: Backend) {
    let cleanup = tokio::spawn(async move {
        backend.stop.wait_for(|stop| *stop).await.unwrap();
    });
    state.disconnect_system_media().await;
    cleanup.await.unwrap();
}

async fn update_until(state: &mut Controller, matches: impl Fn(&Playback) -> bool + Send + Sync) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let playback = state.worker_updates.recv().await.unwrap();
            let done = matches(&playback);
            state.worker_update(playback);
            if done {
                break;
            }
        }
    })
    .await
    .unwrap();
}

async fn fake_audio(state: &mut Controller) {
    let sender = state.worker_sender.clone();
    let (worker, _) = Worker::spawn_fake(Arc::new(move |playback| {
        let _ = sender.send(playback);
    }))
    .unwrap();
    worker.send(Command::Assign {
        generation: worker.generation(),
        id: Some(1),
        entry_id: state.player.assignment.as_ref().unwrap().current_entry_id,
        playback_token: 5,
        waiting_for_check: false,
        mode: PlaybackMode::Playing,
        volume: 0.5,
        parameters: PlaybackParameters::default(),
    });
    worker.send(Command::Loaded {
        generation: worker.generation(),
        id: 1,
        result: Ok(tempfile::NamedTempFile::new().unwrap()),
    });
    state.playback = Some(worker);
    update_until(state, |playback| playback.has_source).await;
}

fn capture_updates(state: &mut Controller) -> Arc<Mutex<Vec<crate::controller::AppUpdate>>> {
    let updates = Arc::new(Mutex::new(Vec::new()));
    let sink = updates.clone();
    state.emit = Arc::new(move |update| sink.lock().unwrap().push(update));
    updates
}

async fn finish_media_http(state: &mut Controller) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(result) = state.tasks.join_next().await {
            if let Ok(result) = result {
                state.complete(result);
            }
        }
    })
    .await
    .unwrap();
    state.publish_system_media();
}

#[tokio::test]
async fn offscreen_current_artwork_uses_one_http_pipeline_without_a_qt_ack() {
    let (mut state, http) = connected().await;
    state.command(AppCommand::SelectTrack(Some(2)));
    state.replace_tracks(vec![track(2)], false);
    state.set_visible_media(Vec::new());
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(7);
    let mut duplicate = track(1);
    duplicate.cover_beatmap_id = Some(999);
    state.playlists.tracks = vec![crate::view_models::PlaylistTrack {
        item_id: 31,
        track: duplicate,
    }];
    assert_eq!(state.track(1).unwrap().cover_beatmap_id, Some(999));
    assert!(!state.upcoming.open);
    let updates = capture_updates(&mut state);
    let backend = attach(&mut state);
    state.start_media();
    assert_eq!(state.media.jobs.len(), 1);
    assert_eq!(state.media.jobs.values().next().unwrap().ticket.audio_id, 1);
    finish_media_http(&mut state).await;
    let snapshot = backend.snapshots.borrow().clone();
    assert_eq!(snapshot.metadata.unwrap().title.as_deref(), Some("1"));
    assert_eq!(
        snapshot.artwork.unwrap().bytes.as_deref(),
        Some(b"encoded artwork fixture".as_slice())
    );
    assert!(
        state.media.jobs.is_empty(),
        "system-only delivery needs no GUI acknowledgement"
    );
    assert!(
        !updates
            .lock()
            .unwrap()
            .iter()
            .any(|update| matches!(update, crate::controller::AppUpdate::Artwork { .. }))
    );
    state.start_media();
    assert!(state.media.jobs.is_empty() && state.tasks.is_empty());
    assert_eq!(
        http.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(header, _)| header.starts_with("GET /api/beatmaps/101/cover "))
            .count(),
        1
    );
    assert_eq!(
        state
            .player
            .selected_track
            .as_ref()
            .unwrap()
            .audio_source_id,
        2
    );
    detach(&mut state, backend).await;
}

#[tokio::test]
async fn current_shares_inflight_gui_artwork_and_four_slots_wait_for_real_qt_acks() {
    let (mut state, http) = connected().await;
    state.replace_tracks((1..=5).map(track).collect(), false);
    state.command(AppCommand::SelectTrack(Some(2)));
    state.set_visible_media((1..=5).map(|id| (id, true)).collect());
    let updates = capture_updates(&mut state);
    state.start_media(); // GUI work is already in flight before the system attaches.
    let backend = attach(&mut state);
    state.start_media();
    assert_eq!(state.media.jobs.len(), 4);
    finish_media_http(&mut state).await;
    let delivered = updates
        .lock()
        .unwrap()
        .iter()
        .find_map(|update| match update {
            crate::controller::AppUpdate::Artwork {
                ticket,
                cover_id: 101,
                bytes,
            } => Some((*ticket, bytes.clone())),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        backend
            .snapshots
            .borrow()
            .artwork
            .as_ref()
            .unwrap()
            .bytes
            .as_deref(),
        Some(delivered.1.as_slice())
    );
    assert!(state.media.jobs.values().all(|job| job.decoding));
    state.start_media();
    assert_eq!(state.media.jobs.len(), 4);
    assert!(
        state.tasks.is_empty(),
        "system delivery must not acknowledge GUI decoding"
    );
    assert_eq!(
        http.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|(header, _)| header.starts_with("GET /api/beatmaps/101/cover "))
            .count(),
        1
    );
    state.command(AppCommand::MediaInstalled {
        ticket: delivered.0,
        available: true,
    });
    state.start_media();
    assert_eq!(
        state.media.jobs.len(),
        4,
        "the real ACK releases exactly one slot for row five"
    );
    finish_media_http(&mut state).await;
    let tickets: Vec<_> = state.media.jobs.values().map(|job| job.ticket).collect();
    state.advance_media(false);
    state.start_media();
    assert_eq!(
        state.media.jobs.len(),
        4,
        "stale decoders still own their slots"
    );
    assert!(state.tasks.is_empty());
    for ticket in tickets {
        state.command(AppCommand::MediaInstalled {
            ticket,
            available: false,
        });
    }
    assert!(state.media.jobs.is_empty());
    state.start_media();
    finish_media_http(&mut state).await;
    assert!(
        backend
            .snapshots
            .borrow()
            .artwork
            .as_ref()
            .unwrap()
            .bytes
            .is_some()
    );
    detach(&mut state, backend).await;
}

#[tokio::test]
async fn late_artwork_rejects_changed_audio_token_entry_cover_and_generation() {
    for changed in ["audio", "token", "entry", "cover", "generation"] {
        let (mut state, _http) = connected().await;
        state.command(AppCommand::SelectTrack(Some(2)));
        state.set_visible_media(Vec::new());
        let backend = attach(&mut state);
        state.start_media();
        let old = state
            .media
            .jobs
            .values()
            .find(|job| job.ticket.audio_id == 1)
            .unwrap()
            .ticket;
        match changed {
            "audio" => {
                state
                    .player
                    .assignment
                    .as_mut()
                    .unwrap()
                    .current_audio_source_id = Some(3);
                state.player.current_track = Some(track(3));
            }
            "token" => state.player.assignment.as_mut().unwrap().playback_token += 1,
            "entry" => {
                state.player.assignment.as_mut().unwrap().current_entry_id =
                    Some(QueueEntryId::new(2).unwrap());
            }
            "cover" => {
                state
                    .player
                    .current_track
                    .as_mut()
                    .unwrap()
                    .cover_beatmap_id = Some(777);
            }
            "generation" => state.advance_media(false),
            _ => panic!("unknown artwork transition: {changed}"),
        }
        state.artwork_completed(old, Some(b"old launch".to_vec()));
        state.publish_system_media();
        assert!(
            backend
                .snapshots
                .borrow()
                .artwork
                .as_ref()
                .unwrap()
                .bytes
                .is_none(),
            "{changed}"
        );
        state.start_media();
        let current_id = state.player.current_track.as_ref().unwrap().audio_source_id;
        let current = state
            .media
            .jobs
            .values()
            .find(|job| job.ticket.audio_id == current_id)
            .unwrap()
            .ticket;
        assert_ne!(current, old);
        state.artwork_completed(
            crate::controller::MediaTicket {
                generation: current.generation.wrapping_add(1),
                ..current
            },
            Some(b"forged generation".to_vec()),
        );
        assert!(
            state
                .system_media_snapshot()
                .artwork
                .unwrap()
                .bytes
                .is_none()
        );
        state.artwork_completed(current, Some(b"current launch".to_vec()));
        state.publish_system_media();
        let snapshot = backend.snapshots.borrow().clone();
        let artwork = snapshot.artwork.unwrap();
        assert_eq!(Some(artwork.key.launch), snapshot.launch);
        assert_eq!(
            artwork.key.cover_id,
            state
                .player
                .current_track
                .as_ref()
                .unwrap()
                .cover_beatmap_id
                .unwrap()
        );
        assert_eq!(artwork.key.generation, state.media.generation);
        assert_eq!(artwork.bytes.as_deref(), Some(b"current launch".as_slice()));
        state.tasks.abort_all();
        finish_media_http(&mut state).await;
        detach(&mut state, backend).await;
    }
}

#[tokio::test]
async fn repeated_launch_clears_artwork_before_http_and_old_qt_ack_cannot_restore_it() {
    let (mut state, _http) = connected().await;
    state.set_visible_media(vec![(1, true)]);
    let backend = attach(&mut state);
    state.start_media();
    finish_media_http(&mut state).await;
    let old = state.media.jobs.values().next().unwrap().ticket;
    assert!(
        backend
            .snapshots
            .borrow()
            .artwork
            .as_ref()
            .unwrap()
            .bytes
            .is_some()
    );
    state.player.assignment.as_mut().unwrap().playback_token += 1;
    state.publish_system_media();
    assert!(
        backend.snapshots.borrow().artwork.is_none(),
        "clear without waiting for HTTP or Qt"
    );
    state.start_media();
    let new = state
        .media
        .jobs
        .values()
        .find(|job| job.artwork_pending)
        .unwrap()
        .ticket;
    assert_ne!(old, new);
    assert_eq!(
        state.media.jobs.len(),
        2,
        "old decoder retains its own slot"
    );
    state.command(AppCommand::MediaInstalled {
        ticket: old,
        available: true,
    });
    state.publish_system_media();
    assert!(
        backend
            .snapshots
            .borrow()
            .artwork
            .as_ref()
            .unwrap()
            .bytes
            .is_none()
    );
    finish_media_http(&mut state).await;
    let artwork = backend.snapshots.borrow().artwork.clone().unwrap();
    assert_eq!(artwork.key.launch.playback_token, 6);
    state.command(AppCommand::MediaInstalled {
        ticket: old,
        available: false,
    });
    assert_eq!(backend.snapshots.borrow().artwork, Some(artwork));
    assert!(state.media.jobs.get(&new.serial).unwrap().decoding);
    state.command(AppCommand::MediaInstalled {
        ticket: new,
        available: true,
    });
    assert!(state.media.jobs.is_empty());
    detach(&mut state, backend).await;
}

#[tokio::test]
async fn missing_empty_or_failed_current_artwork_keeps_metadata_and_transport() {
    for http_failure in [true, false] {
        let (mut state, _http) = connected().await;
        state.command(AppCommand::SelectTrack(Some(2)));
        state.set_visible_media(Vec::new());
        let backend = attach(&mut state);
        state.start_media();
        finish_media_http(&mut state).await;
        assert!(
            backend
                .snapshots
                .borrow()
                .artwork
                .as_ref()
                .unwrap()
                .bytes
                .is_some()
        );
        state.player.assignment.as_mut().unwrap().playback_token += 1;
        state
            .player
            .current_track
            .as_mut()
            .unwrap()
            .cover_beatmap_id = None;
        state.publish_system_media();
        assert!(backend.snapshots.borrow().artwork.is_none());
        state.start_media();
        assert!(state.media.jobs.is_empty());
        state
            .player
            .current_track
            .as_mut()
            .unwrap()
            .cover_beatmap_id = Some(303);
        state.start_media();
        let ticket = state.media.jobs.values().next().unwrap().ticket;
        if http_failure {
            finish_media_http(&mut state).await; // Real 404 through the existing API bounds/error path.
        } else {
            state.artwork_completed(ticket, Some(Vec::new()));
        }
        state.publish_system_media();
        let snapshot = backend.snapshots.borrow().clone();
        assert!(snapshot.artwork.unwrap().bytes.is_none());
        assert_eq!(snapshot.metadata.unwrap().title.as_deref(), Some("1"));
        assert!(state.system_media_active() && snapshot.capabilities.pause);
        assert!(state.media.unavailable.contains(&303));
        assert!(state.media.jobs.is_empty());
        state.start_media();
        assert!(
            state.media.jobs.is_empty(),
            "failed cover is not retried every tick"
        );
        state.system_media_event(Event::Pause);
        assert_eq!(
            state.player.commands.back().unwrap().command,
            crate::models::PlaybackCommand::Pause
        );
        state.tasks.abort_all();
        finish_media_http(&mut state).await;
        detach(&mut state, backend).await;
    }
}

#[tokio::test]
async fn selection_from_library_and_playlist_preserves_current_snapshot_and_transport() {
    let (mut state, http) = connected().await;
    let backend = attach(&mut state);
    let mut initial = backend.snapshots.borrow().clone();
    state.command(AppCommand::SelectTrack(Some(2)));
    state.publish_system_media();
    assert_eq!(*backend.snapshots.borrow(), initial);
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(7);
    state.install_playlist(view_model_tests::playlist());
    state.playlist_action(crate::view_models::PlaylistAction::SelectItem(32));
    state.publish_system_media();
    // Replacing playlist rows advances media delivery generation, retaining current A.
    initial.artwork.as_mut().unwrap().key.generation = state.media.generation;
    assert_eq!(*backend.snapshots.borrow(), initial);
    for (event, expected) in [
        (Event::Pause, crate::models::PlaybackCommand::Pause),
        (Event::PlayPause, crate::models::PlaybackCommand::Pause),
        (
            Event::Play,
            crate::models::PlaybackCommand::Play {
                audio_source_id: None,
            },
        ),
        (Event::Stop, crate::models::PlaybackCommand::Stop),
        (Event::Next, crate::models::PlaybackCommand::Next),
    ] {
        assert!(backend.events.send(event));
        let event = state.system_events.as_mut().unwrap().recv().await.unwrap();
        state.system_media_event(event);
        let pending = state.player.commands.pop_back().unwrap();
        assert_eq!(pending.command, expected);
        assert!(pending.playlist.is_none() && pending.queue.is_none());
    }
    state.system_media_event(Event::Previous);
    assert!(state.player.commands.is_empty());
    // Resume goes over the existing HTTP lane without selecting library B or playlist item 32.
    state.system_media_event(Event::Play);
    state.start_playback_command();
    let result = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(result);
    assert!(state.player.commands.is_empty());
    assert!(http.requests.lock().unwrap().iter().any(|(header, body)| {
        header.starts_with("POST /api/playback/commands ")
            && *body == serde_json::json!({"command": "play"})
    }));
    assert_eq!(
        state
            .player
            .assignment
            .as_ref()
            .unwrap()
            .current_audio_source_id,
        Some(1)
    );
    detach(&mut state, backend).await;
}

#[tokio::test]
async fn capability_and_status_matrix_does_not_borrow_old_worker_state() {
    let (mut state, _http) = connected().await;
    for (mode, worker, status, pause, stop) in [
        (
            PlaybackMode::Playing,
            PlayerState::Playing,
            PlaybackStatus::Playing,
            true,
            true,
        ),
        (
            PlaybackMode::Paused,
            PlayerState::Paused,
            PlaybackStatus::Paused,
            false,
            true,
        ),
        (
            PlaybackMode::Stopped,
            PlayerState::Stopped,
            PlaybackStatus::Stopped,
            false,
            false,
        ),
    ] {
        state.player.assignment.as_mut().unwrap().mode = mode;
        state.player.playback.snapshot.state = worker;
        state.player.playback.has_source = mode != PlaybackMode::Stopped;
        let snapshot = state.system_media_snapshot();
        assert_eq!(snapshot.status, status);
        assert_eq!(snapshot.capabilities.pause, pause);
        assert_eq!(snapshot.capabilities.stop, stop);
        assert_eq!(snapshot.capabilities.seek, mode != PlaybackMode::Stopped);
        assert!(snapshot.capabilities.play && snapshot.capabilities.next);
        // The server has no previous entry, but a sounding track past 3 s restarts locally.
        assert_eq!(
            snapshot.capabilities.previous,
            mode != PlaybackMode::Stopped
        );
        state.system_media_event(Event::PlayPause);
        let expected = if pause {
            crate::models::PlaybackCommand::Pause
        } else {
            crate::models::PlaybackCommand::Play {
                audio_source_id: None,
            }
        };
        assert_eq!(state.player.commands.pop_back().unwrap().command, expected);
    }
}

#[tokio::test]
async fn modes_reach_snapshot_and_system_requests_become_commands() {
    use crate::models::{PlaybackCommand, RepeatMode};
    use osu_radio_media_controls::Repeat;
    let (mut state, _http) = connected().await;
    let snapshot = state.system_media_snapshot();
    assert!(snapshot.capabilities.set_modes);
    assert!(!snapshot.shuffle && snapshot.repeat == Repeat::Off);
    for (mode, repeat) in [
        (RepeatMode::Off, Repeat::Off),
        (RepeatMode::All, Repeat::All),
        (RepeatMode::One, Repeat::One),
    ] {
        let assignment = state.player.assignment.as_mut().unwrap();
        assignment.shuffle = mode == RepeatMode::All;
        assignment.repeat = mode;
        let snapshot = state.system_media_snapshot();
        assert_eq!(snapshot.repeat, repeat);
        assert_eq!(snapshot.shuffle, mode == RepeatMode::All);
    }
    state.system_media_event(Event::SetShuffle(true));
    assert_eq!(
        state.player.commands.pop_back().unwrap().command,
        PlaybackCommand::SetShuffle { enabled: true }
    );
    state.system_media_event(Event::SetRepeat(Repeat::One));
    assert_eq!(
        state.player.commands.pop_back().unwrap().command,
        PlaybackCommand::SetRepeat {
            mode: RepeatMode::One
        }
    );
    // Not ready: no assignment disables the capability and drops the request.
    state.player.assignment = None;
    assert!(!state.system_media_snapshot().capabilities.set_modes);
    state.system_media_event(Event::SetShuffle(false));
    state.system_media_event(Event::SetRepeat(Repeat::All));
    assert!(state.player.commands.is_empty());
}

#[tokio::test]
async fn unconfirmed_worker_and_empty_queue_have_safe_status_and_commands() {
    let (mut state, _http) = connected().await;
    state.player.assignment.as_mut().unwrap().mode = PlaybackMode::Playing;
    state.player.playback.snapshot.state = PlayerState::Playing;
    state.player.playback.has_source = true;
    for phase in 0..5 {
        state.player.playback.loading_audio_id = (phase == 0).then_some(1);
        state.player.assignment.as_mut().unwrap().waiting_for_check = phase == 1;
        state.player.playback.error = (phase == 2).then(|| "device failed".into());
        state.player.playback.playback_token = if phase == 3 { 4 } else { 5 };
        state.player.playback.current_audio_id = Some(if phase == 4 { 2 } else { 1 });
        let snapshot = state.system_media_snapshot();
        assert_eq!(snapshot.status, PlaybackStatus::Stopped, "phase {phase}");
        assert_eq!(snapshot.position, Duration::ZERO, "phase {phase}");
        assert!(!snapshot.capabilities.seek);
        assert!(snapshot.capabilities.pause);
        state.system_media_event(Event::PlayPause);
        assert_eq!(
            state.player.commands.pop_back().unwrap().command,
            crate::models::PlaybackCommand::Pause
        );
    }
    state.player.assignment.as_mut().unwrap().can_previous = true;
    state.system_media_event(Event::Previous);
    assert_eq!(
        state.player.commands.pop_back().unwrap().command,
        crate::models::PlaybackCommand::Previous
    );
    state.player.assignment.as_mut().unwrap().mode = PlaybackMode::Paused;
    for waiting in [false, true] {
        state.player.assignment.as_mut().unwrap().waiting_for_check = waiting;
        state.player.playback.loading_audio_id = (!waiting).then_some(1);
        state.system_media_event(Event::PlayPause);
        assert_eq!(
            state.player.commands.pop_back().unwrap().command,
            crate::models::PlaybackCommand::Play {
                audio_source_id: None
            }
        );
    }
    let assignment = state.player.assignment.as_mut().unwrap();
    assignment.current_entry_id = None;
    assignment.current_audio_source_id = None;
    assignment.can_next = false;
    assignment.can_previous = false;
    state.player.current_track = None;
    let empty = state.system_media_snapshot();
    assert!(empty.launch.is_none() && empty.metadata.is_none() && empty.duration.is_none());
    assert!(!empty.capabilities.play && !empty.capabilities.pause && !empty.capabilities.stop);
    for event in [
        Event::Play,
        Event::Pause,
        Event::PlayPause,
        Event::Stop,
        Event::Next,
        Event::Previous,
    ] {
        state.system_media_event(event);
    }
    assert!(state.player.commands.is_empty());
    state.settings.volume.settings = None;
    assert_eq!(
        state.system_media_snapshot().capabilities,
        osu_radio_media_controls::Capabilities::default()
    );
}

#[tokio::test]
async fn metadata_uses_source_fields_and_new_launch_clears_old_position_and_duration() {
    let (mut state, _http) = connected().await;
    let current = state.player.current_track.as_mut().unwrap();
    current.title_original = Some("Title".into());
    current.title_unicode = Some("曲".into());
    current.artist_original = Some(" ".into());
    current.artist_unicode = None;
    assert_eq!(
        state
            .system_media_snapshot()
            .metadata
            .unwrap()
            .title
            .as_deref(),
        Some("Title")
    );
    state.set_name_preferences(crate::TrackNamePreferences {
        use_unicode_titles: true,
        use_unicode_artists: true,
    });
    let metadata = state.system_media_snapshot().metadata.unwrap();
    assert_eq!(metadata.title.as_deref(), Some("曲"));
    assert_eq!(metadata.artist, None);
    assert_eq!(state.player.assignment.as_ref().unwrap().playback_token, 5);
    let current = state.player.current_track.as_mut().unwrap();
    current.title_unicode = Some(" ".into());
    assert_eq!(
        state
            .system_media_snapshot()
            .metadata
            .unwrap()
            .title
            .as_deref(),
        Some("Title")
    );
    let current = state.player.current_track.as_mut().unwrap();
    current.title_original = None;
    assert_eq!(state.system_media_snapshot().metadata.unwrap().title, None);
    state.player.assignment.as_mut().unwrap().current_entry_id =
        Some(QueueEntryId::new(2).unwrap());
    state.player.assignment.as_mut().unwrap().playback_token = 6;
    state.player.current_track = Some(track(1));
    let next = state.system_media_snapshot();
    assert_eq!(next.status, PlaybackStatus::Stopped);
    assert_eq!(next.position, Duration::ZERO);
    assert_eq!(next.duration, None);
    assert_eq!(next.seek_serial, 0);
    assert_eq!(next.artwork, None);
}

#[tokio::test]
async fn system_volume_edits_current_audio_and_uses_existing_save_lane() {
    let (mut state, http) = connected().await;
    state.command(AppCommand::SelectTrack(Some(2)));
    state.set_individual_volume(true);
    state.system_media_event(Event::SetVolume(0.17));
    assert_eq!(
        state.player.current_track.as_ref().unwrap().volume_percent,
        Some(17)
    );
    assert_eq!(state.settings.volume.overrides.get(&1), Some(&Some(17)));
    assert!(!state.settings.volume.overrides.contains_key(&2));
    // The GUI guard still rejects a gesture for A while B is selected.
    state.set_track_volume(1, Some(99));
    assert_eq!(state.settings.volume.overrides.get(&1), Some(&Some(17)));
    let pending = state.settings.volume.pending.len();
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 1.01] {
        state.system_media_event(Event::SetVolume(invalid));
    }
    assert_eq!(state.settings.volume.pending.len(), pending);
    assert_eq!(state.settings.volume.overrides.get(&1), Some(&Some(17)));
    state.set_individual_volume(false);
    state.system_media_event(Event::SetVolume(0.4));
    assert_eq!(
        state
            .settings
            .volume
            .settings
            .unwrap()
            .global_volume_percent,
        40
    );
    state.flush_volume_saves().await;
    let requests = http.requests.lock().unwrap();
    assert!(requests.iter().any(|(header, body)| {
        header.starts_with("PUT /api/audio-sources/1/volume ") && body["volume_percent"] == 17
    }));
    assert!(
        !requests
            .iter()
            .any(|(header, _)| header.starts_with("PUT /api/audio-sources/2/volume "))
    );
    assert!(requests.iter().any(|(header, body)| {
        header.starts_with("PATCH /api/user-data/audio-settings ")
            && body["global_volume_percent"] == 40
    }));
    drop(requests);
}

#[tokio::test]
async fn rate_is_the_sounding_launchs_applied_speed_with_source_coordinates() {
    let (mut state, _http) = connected().await;
    let fast = |state: &mut Controller, speed| {
        state.player.playback.snapshot.parameters = PlaybackParameters {
            speed,
            adjust_pitch: false,
        };
    };
    assert!((state.system_media_snapshot().rate - 1.0).abs() < f64::EPSILON);
    state.player.playback.snapshot.duration = Some(Duration::from_secs(240));
    // B selected (library row 2) must not change A's published rate or time scale.
    state.command(AppCommand::SelectTrack(Some(2)));
    let backend = attach(&mut state);
    for (speed, status) in [
        (1.2, PlaybackStatus::Playing),
        (1.5, PlaybackStatus::Playing),
        (0.75, PlaybackStatus::Playing),
        (1.5, PlaybackStatus::Paused),
    ] {
        fast(&mut state, speed);
        state.player.playback.snapshot.state = if status == PlaybackStatus::Paused {
            PlayerState::Paused
        } else {
            PlayerState::Playing
        };
        state.publish_system_media();
        let published = backend.snapshots.borrow().clone();
        assert!((published.rate - f64::from(speed)).abs() < 1e-6);
        assert_eq!(published.status, status);
        // Source coordinates: never divided by the speed.
        assert_eq!(published.position, Duration::from_secs(9));
        assert_eq!(published.duration, Some(Duration::from_secs(240)));
        assert_eq!(published.launch, backend.events.current_launch());
    }
    // A worker that is not the confirmed current launch publishes the neutral rate.
    state.player.playback.playback_token = 4;
    assert!((state.system_media_snapshot().rate - 1.0).abs() < f64::EPSILON);
    detach(&mut state, backend).await;
}

#[tokio::test]
async fn seek_is_current_launch_bound_and_confirmed_worker_seek_is_published() {
    let (mut state, _http) = connected().await;
    fake_audio(&mut state).await;
    state.command(AppCommand::SelectTrack(Some(2)));
    let backend = attach(&mut state);
    let launch = backend.events.current_launch().unwrap();
    assert!(backend.events.send(Event::SeekAbsolute {
        launch,
        position: Duration::from_secs(12)
    }));
    let event = state.system_events.as_mut().unwrap().recv().await.unwrap();
    state.system_media_event(event);
    update_until(&mut state, |p| p.seek_serial == 1).await;
    state.publish_system_media();
    assert_eq!(backend.snapshots.borrow().position, Duration::from_secs(12));
    assert_eq!(backend.snapshots.borrow().seek_serial, 1);
    // Relative offsets are based on the accepted current position, with safe bounds.
    state.system_media_event(Event::SeekRelative {
        launch,
        offset_micros: i64::MIN,
    });
    update_until(&mut state, |p| p.seek_serial == 2).await;
    assert_eq!(state.player.playback.snapshot.position, Duration::ZERO);
    state.system_media_event(Event::SeekRelative {
        launch,
        offset_micros: 60_000_000,
    });
    update_until(&mut state, |p| p.seek_serial == 3).await;
    assert_eq!(
        state.player.playback.snapshot.position,
        Duration::from_secs(60)
    );
    let mut stale_launch = launch;
    for (entry, token) in [(1, 6), (2, 6)] {
        assert!(backend.events.send(Event::SeekAbsolute {
            launch: stale_launch,
            position: Duration::from_secs(22)
        }));
        state.player.assignment.as_mut().unwrap().current_entry_id =
            Some(QueueEntryId::new(entry).unwrap());
        state.player.assignment.as_mut().unwrap().playback_token = token;
        let worker = state.playback.as_ref().unwrap();
        worker.send(Command::Assign {
            generation: worker.generation(),
            id: Some(1),
            entry_id: Some(QueueEntryId::new(entry).unwrap()),
            playback_token: token,
            waiting_for_check: false,
            mode: PlaybackMode::Playing,
            volume: 0.5,
            parameters: PlaybackParameters::default(),
        });
        update_until(&mut state, |p| {
            p.playback_token == token
                && p.current_entry_id == Some(QueueEntryId::new(entry).unwrap())
                && p.has_source
        })
        .await;
        let serial = state.player.playback.seek_serial;
        let event = state.system_events.as_mut().unwrap().recv().await.unwrap();
        state.system_media_event(event);
        // A worker barrier proves that the stale event did not cause any seek.
        state
            .playback
            .as_ref()
            .unwrap()
            .send(Command::SetVolume(0.31));
        update_until(&mut state, |p| {
            p.snapshot.volume.to_bits() == 0.31_f32.to_bits()
        })
        .await;
        assert_eq!(state.player.playback.seek_serial, serial);
        assert_eq!(state.player.playback.snapshot.position, Duration::ZERO);
        stale_launch = state.system_media_snapshot().launch.unwrap();
    }
    detach(&mut state, backend).await;
    state.playback.take().unwrap().shutdown();
}

#[tokio::test]
async fn relative_seek_beyond_end_advances_only_the_current_seekable_launch_with_next() {
    let (mut state, _http) = connected().await;
    state.command(AppCommand::SelectTrack(Some(2)));
    let mut launch = state.system_media_snapshot().launch.unwrap();
    state.system_media_event(Event::SeekRelative {
        launch,
        offset_micros: i64::MAX,
    });
    let pending = state.player.commands.pop_front().unwrap();
    assert_eq!(pending.command, crate::models::PlaybackCommand::Next);
    assert!(pending.playlist.is_none() && pending.queue.is_none());
    for (entry, token) in [(1, 6), (2, 6)] {
        let assignment = state.player.assignment.as_mut().unwrap();
        assignment.current_entry_id = Some(QueueEntryId::new(entry).unwrap());
        assignment.playback_token = token;
        state.player.playback.current_entry_id = assignment.current_entry_id;
        state.player.playback.playback_token = token;
        assert!(state.system_media_snapshot().capabilities.seek);
        state.system_media_event(Event::SeekRelative {
            launch,
            offset_micros: i64::MAX,
        });
        assert!(
            state.player.commands.is_empty(),
            "stale beyond-end seek must not advance a repeated audio launch"
        );
        launch = state.system_media_snapshot().launch.unwrap();
    }
    state.player.playback.loading_audio_id = Some(1);
    assert!(!state.system_media_snapshot().capabilities.seek);
    state.system_media_event(Event::SeekRelative {
        launch,
        offset_micros: i64::MAX,
    });
    assert!(
        state.player.commands.is_empty(),
        "seek capability must gate beyond-end Next too"
    );
    state.player.playback.loading_audio_id = None;
    state.player.assignment.as_mut().unwrap().can_next = false;
    state.system_media_event(Event::SeekRelative {
        launch,
        offset_micros: i64::MAX,
    });
    assert!(
        state.player.commands.is_empty(),
        "without next, beyond-end seek is a no-op"
    );
    assert!(
        state.playback.is_none(),
        "beyond-end seek never reaches the audio worker"
    );
}

#[tokio::test]
async fn queued_overflow_seek_keeps_its_launch_token_until_http_dispatch() {
    let (mut state, http) = connected().await;
    state.command(AppCommand::SelectTrack(Some(2)));
    let launch = state.system_media_snapshot().launch.unwrap();
    state.player.command_busy = true;
    state.system_media_event(Event::SeekRelative {
        launch,
        offset_micros: i64::MAX,
    });
    assert_eq!(
        state.player.commands.front().unwrap().expected_token,
        Some(5)
    );
    state.start_playback_command();
    assert!(
        state.tasks.is_empty(),
        "busy lane must retain the queued seek"
    );
    state.player.assignment.as_mut().unwrap().playback_token = 6;
    state.player.playback.playback_token = 6;
    state.player.command_busy = false;
    state.start_playback_command();
    assert!(state.player.commands.is_empty() && state.tasks.is_empty());
    assert!(
        !http
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|(header, _)| header.starts_with("POST "))
    );

    let launch = state.system_media_snapshot().launch.unwrap();
    state.player.command_busy = true;
    state.system_media_event(Event::SeekRelative {
        launch,
        offset_micros: i64::MAX,
    });
    assert_eq!(
        state.player.commands.front().unwrap().expected_token,
        Some(6)
    );
    state.player.command_busy = false;
    state.start_playback_command();
    let result = state.tasks.join_next().await.unwrap().unwrap();
    assert!(matches!(
        result,
        crate::controller::Completed::PlaybackCommand { result: Ok(_), .. }
    ));
    let requests = http.requests.lock().unwrap();
    let posts: Vec<_> = requests
        .iter()
        .filter(|(header, _)| header.starts_with("POST "))
        .collect();
    assert_eq!(posts.len(), 1);
    assert!(
        posts
            .first()
            .unwrap()
            .0
            .starts_with("POST /api/playback/commands ")
    );
    assert_eq!(
        posts.first().unwrap().1,
        serde_json::json!({"command": "next"})
    );
    drop(requests);
    // Explicit Next keeps its established, unguarded transport semantics.
    state.system_media_event(Event::Next);
    assert_eq!(state.player.commands.front().unwrap().expected_token, None);
}

#[tokio::test]
async fn event_loop_acknowledges_early_disconnect_and_repeats_cleanup_on_shutdown() {
    let (mut state, _http) = connected().await;
    let mut backend = attach(&mut state);
    let events = backend.events.clone();
    let (commands, receiver) = mpsc::unbounded_channel();
    let (done, stopped) = watch::channel(false);
    let app = AppController { commands, stopped };
    let task = tokio::spawn(async move {
        state.run(receiver).await;
        done.send_replace(true);
    });
    app.send(AppCommand::SelectTrack(Some(2)));
    app.publish_system_media();
    tokio::time::timeout(Duration::from_secs(3), backend.snapshots.changed())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        backend.snapshots.borrow().launch,
        Some(LaunchKey {
            entry_id: 1,
            playback_token: 5
        })
    );
    assert!(events.send(Event::SetVolume(0.23)));
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            backend.snapshots.changed().await.unwrap();
            if (backend.snapshots.borrow().volume - 0.23).abs() < 0.000_001 {
                break;
            }
        }
    })
    .await
    .unwrap();
    let disconnect = tokio::spawn({
        let app = app.clone();
        async move { app.disconnect_system_media().await }
    });
    backend.stop.wait_for(|stop| *stop).await.unwrap();
    assert!(
        !disconnect.is_finished(),
        "acknowledgement must await native backend cleanup"
    );
    assert!(!events.send(Event::Next));
    drop(backend);
    disconnect.await.unwrap();
    app.disconnect_system_media().await;
    app.shutdown().await;
    task.await.unwrap();
    app.disconnect_system_media().await;
    app.shutdown().await;
}

#[tokio::test]
async fn fatal_backend_error_is_terminal_but_client_keeps_handling_commands() {
    let (mut state, _http) = connected().await;
    let backend = attach(&mut state);
    backend.fail(Error::new("fixture connection failed"));
    assert!(state.system_events.as_mut().unwrap().recv().await.is_none());
    state.system_media_ended();
    state.command(AppCommand::SelectTrack(Some(2)));
    assert_eq!(state.library.selected, Some(2));
    assert!(state.connect_system_media(NativeContext::Linux).is_err());
    assert!(state.system_events.is_none());
    drop(backend);
    state.disconnect_system_media().await;
    assert!(state.connect_system_media(NativeContext::Linux).is_err());
    state.disconnect_system_media().await;
}

#[tokio::test]
async fn ordinary_controller_shutdown_also_waits_for_backend_cleanup() {
    let (mut state, _http) = connected().await;
    let mut backend = attach(&mut state);
    let events = backend.events.clone();
    let (commands, receiver) = mpsc::unbounded_channel();
    commands.send(AppCommand::Shutdown).unwrap();
    let task = tokio::spawn(state.run(receiver));
    backend.stop.wait_for(|stop| *stop).await.unwrap();
    assert!(!task.is_finished());
    assert!(!events.send(Event::Play));
    drop(backend);
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn mode_commands_derive_their_target_from_the_last_assignment_and_share_the_playback_lane() {
    use crate::models::{PlaybackCommand, RepeatMode};
    let (mut state, _http) = connected().await;
    state.command(AppCommand::Next);
    state.command(AppCommand::ToggleShuffle);
    let next = state.player.commands.pop_front().unwrap();
    let shuffle = state.player.commands.pop_front().unwrap();
    assert_eq!(next.command, PlaybackCommand::Next);
    assert_eq!(
        shuffle.command,
        PlaybackCommand::SetShuffle { enabled: true }
    );
    assert!(shuffle.playlist.is_none() && shuffle.queue.is_none());

    state.player.assignment.as_mut().unwrap().shuffle = true;
    state.command(AppCommand::ToggleShuffle);
    let off = state.player.commands.pop_front().unwrap();
    assert_eq!(off.command, PlaybackCommand::SetShuffle { enabled: false });

    for (current, target) in [
        (RepeatMode::Off, RepeatMode::All),
        (RepeatMode::All, RepeatMode::One),
        (RepeatMode::One, RepeatMode::Off),
    ] {
        state.player.assignment.as_mut().unwrap().repeat = current;
        state.command(AppCommand::CycleRepeat);
        let pending = state.player.commands.pop_front().unwrap();
        assert_eq!(pending.command, PlaybackCommand::SetRepeat { mode: target });
    }
}

#[test]
fn mode_commands_are_not_sent_without_a_connection_or_assignment() {
    let (mut state, _) = controller();
    for command in [AppCommand::ToggleShuffle, AppCommand::CycleRepeat] {
        state.command(command);
        assert!(state.player.commands.is_empty() && state.tasks.is_empty());
    }
    state.player.assignment = Some(view_model_tests::assignment(PlaybackMode::Playing));
    state.command(AppCommand::ToggleShuffle);
    assert!(
        state.player.commands.is_empty(),
        "an assignment alone does not make an offline client send"
    );
}

#[tokio::test]
async fn previous_restarts_locally_beyond_three_shown_seconds_for_button_and_system_controls() {
    use crate::models::PlaybackCommand;
    let (mut state, http) = connected().await;
    state.player.assignment.as_mut().unwrap().can_previous = true;
    // (source position, speed, state, restarts locally)
    for (position, speed, player_state, restarts) in [
        (5.0, 1.0, PlayerState::Playing, true),
        (5.0, 1.0, PlayerState::Paused, true),
        (3.0, 1.0, PlayerState::Playing, false),
        (2.0, 1.0, PlayerState::Playing, false),
        // The threshold uses the shown position: 5 s of source at 2x is 2.5 s.
        (5.0, 2.0, PlayerState::Playing, false),
        (3.5, 0.5, PlayerState::Playing, true),
    ] {
        state.player.playback.snapshot.position = Duration::from_secs_f64(position);
        state.player.playback.snapshot.parameters.speed = speed;
        state.player.playback.snapshot.state = player_state;
        for system in [false, true] {
            if system {
                state.system_media_event(Event::Previous);
            } else {
                state.command(AppCommand::Previous);
            }
            let queued = state.player.commands.pop_back();
            assert_eq!(
                queued.map(|pending| pending.command),
                (!restarts).then_some(PlaybackCommand::Previous),
                "{position}s at {speed}x {player_state:?}, system: {system}"
            );
        }
    }
    // Not sounding in the current worker: always the server's Previous.
    state.player.playback.snapshot.state = PlayerState::Playing;
    state.player.playback.snapshot.position = Duration::from_secs(9);
    state.player.playback.snapshot.parameters.speed = 1.0;
    for stale in 0..3 {
        let playback = &mut state.player.playback;
        playback.loading_audio_id = (stale == 0).then_some(1);
        playback.has_source = stale != 1;
        playback.playback_token = if stale == 2 { 4 } else { 5 };
        state.command(AppCommand::Previous);
        assert_eq!(
            state.player.commands.pop_back().unwrap().command,
            PlaybackCommand::Previous,
            "case {stale}"
        );
    }
    assert!(
        !http
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|(header, _)| header.starts_with("POST "))
    );
}

#[tokio::test]
async fn previous_stays_available_on_the_first_entry_while_a_local_restart_applies() {
    let (mut state, _http) = connected().await;
    state.player.assignment.as_mut().unwrap().can_previous = false;
    state.player.playback.can_previous = false;
    state.player.playback.snapshot.state = PlayerState::Playing;
    let presented = |state: &Controller| {
        state
            .player
            .presentation(
                None,
                false,
                true,
                true,
                crate::models::AudioModsProfile::default(),
            )
            .can_previous
    };
    for (position, available) in [(5, true), (3, false), (2, false)] {
        state.player.playback.snapshot.position = Duration::from_secs(position);
        assert_eq!(
            state.system_media_snapshot().capabilities.previous,
            available,
            "{position}s system capability"
        );
        assert_eq!(presented(&state), available, "{position}s button");
        // With no server previous entry the event is dropped unless the restart applies.
        state.system_media_event(Event::Previous);
        assert!(state.player.commands.is_empty());
    }
    // Not sounding: the server's answer alone decides.
    state.player.playback.snapshot.position = Duration::from_secs(9);
    state.player.playback.has_source = false;
    assert!(!state.system_media_snapshot().capabilities.previous && !presented(&state));
}

#[tokio::test]
async fn local_previous_restart_seeks_the_current_launch_without_a_server_command() {
    let (mut state, http) = connected().await;
    fake_audio(&mut state).await;
    state.player.playback.snapshot.position = Duration::from_secs(5);
    state.command(AppCommand::Seek(Duration::from_secs(5)));
    update_until(&mut state, |p| p.seek_serial == 1).await;
    assert_eq!(
        state.player.playback.snapshot.position,
        Duration::from_secs(5)
    );
    let playing = state.player.playback.snapshot.state;
    state.command(AppCommand::Previous);
    update_until(&mut state, |p| p.seek_serial == 2).await;
    assert_eq!(state.player.playback.snapshot.position, Duration::ZERO);
    assert_eq!(state.player.playback.snapshot.state, playing);
    assert!(state.player.commands.is_empty() && state.player.pending_start.is_none());
    assert!(
        !http
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|(header, _)| header.starts_with("POST "))
    );
    state.playback.take().unwrap().shutdown();
}
