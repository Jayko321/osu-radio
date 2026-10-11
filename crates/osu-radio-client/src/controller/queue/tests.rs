use super::*;
use crate::controller::AppCommand;
use crate::models::{LibraryTrack, TrackDifficulty};
use std::sync::Arc;

fn assignment(revision: u64, token: u64, id: i32, mode: PlaybackMode) -> PlaybackAssignment {
    PlaybackAssignment {
        volume_percent: None,
        current_audio_source_id: Some(id),
        current_playlist_item_id: None,
        track: Some(LibraryTrack {
            audio_source_id: id,
            last_played_at_ms: None,
            volume_percent: None,
            title: Some(format!("Track {id}")),
            title_unicode: None,
            artist: Some("Artist".into()),
            artist_unicode: None,
            cover_beatmap_id: Some(id.saturating_add(100)),
            difficulties: vec![TrackDifficulty {
                beatmap_id: id,
                beatmap_set_id: 1,
                difficulty_name: None,
                set_has_multiple_audio_sources: false,
            }],
        }),
        duration_ms: Some(100_000),
        mode,
        revision,
        playback_token: token,
        can_next: true,
        can_previous: true,
    }
}

#[tokio::test]
async fn queue_view_preserves_order_duplicates_and_media_outside_search() {
    let (mut state, updates) = super::super::tests::controller();
    state.assignment = Some(assignment(5, 9, 7, PlaybackMode::Paused));
    state.upcoming.open = true;
    state.upcoming.request = 3;
    let queue = crate::models::QueueState {
        upcoming_tracks: [42, 103, 42]
            .into_iter()
            .map(|id| assignment(5, 9, id, PlaybackMode::Paused).track.unwrap())
            .collect(),
        audio_source_ids: vec![7, 42, 103, 42],
        playlist_item_ids: vec![None; 4],
        current_index: Some(0),
        mode: PlaybackMode::Paused,
        revision: 5,
        playback_token: 9,
    };
    state.queue_loaded(2, Ok(queue.clone()));
    assert!(
        state.upcoming.view.tracks.is_empty(),
        "stale request ignored"
    );
    state.queue_loaded(3, Ok(queue.clone()));
    assert_eq!(
        state
            .upcoming
            .view
            .tracks
            .iter()
            .map(|track| track.audio_source_id)
            .collect::<Vec<_>>(),
        [42, 103, 42]
    );
    assert!(
        state.tracks.is_empty(),
        "queue metadata does not populate Songs search"
    );
    state.request_media(42, true);
    assert_eq!(state.queue.front(), Some(&(42, true)));
    let ticket = super::super::MediaTicket {
        generation: state.generation,
        audio_id: 42,
        serial: 1,
    };
    state.jobs.insert(
        1,
        super::super::MediaJob {
            ticket,
            cover: Some(142),
            decoding: false,
            artwork_pending: true,
            duration_pending: true,
            tasks: Vec::new(),
        },
    );
    state.media_completed(ticket, None, Some(261_000), true);
    assert_eq!(
        state.upcoming.view.tracks.first().unwrap().duration_label(),
        "04:21"
    );
    assert_eq!(
        state.upcoming.view.tracks.get(2).unwrap().duration_label(),
        "04:21"
    );
    assert!(matches!(
        updates.lock().unwrap().last(),
        Some(AppUpdate::Queue(_))
    ));
    let mut empty = queue.clone();
    empty.upcoming_tracks.clear();
    state.queue_loaded(3, Ok(empty));
    state.queue_loaded(3, Ok(queue));
    assert_eq!(
        state.upcoming.view.tracks.first().unwrap().duration_label(),
        "04:21",
        "rewinding the queue restores probed duration outside search"
    );
    state.set_queue_visible(false);
    state.queue_loaded(3, Err("late failure".into()));
    assert!(
        state.upcoming.view.message.is_empty(),
        "closed panel ignores late responses"
    );
    state.upcoming.open = true;
    state.queue_loaded(state.upcoming.request, Err("queue unavailable".into()));
    assert!(state.upcoming.view.tracks.is_empty());
    assert_eq!(state.upcoming.view.message, "queue unavailable");
}

#[tokio::test]
async fn assignments_order_reconnect_and_search_selection_are_independent() {
    let (mut state, updates) = super::super::tests::controller();
    state.replace_tracks(
        vec![super::super::tests::track(1), super::super::tests::track(2)],
        true,
    );
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    state.apply_assignment(assignment(5, 9, 99, PlaybackMode::Paused));
    assert_eq!(state.selected, Some(99));
    assert_eq!(
        state.track(99).unwrap().duration,
        Some(Duration::from_secs(100))
    );
    assert_eq!(state.track(99).unwrap().cover_beatmap_id, Some(199));
    let generation = state.playback.as_ref().unwrap().generation();
    state.replace_tracks(vec![], true);
    assert_eq!(state.selected, Some(99));
    assert!(
        matches!(updates.lock().unwrap().last(), Some(AppUpdate::TrackSelected(Some(track))) if track.audio_source_id == 99)
    );
    state.apply_assignment(assignment(5, 9, 99, PlaybackMode::Paused));
    state.apply_assignment(assignment(4, 7, 1, PlaybackMode::Playing));
    assert_eq!(state.playback.as_ref().unwrap().generation(), generation);
    assert_eq!(state.assignment.as_ref().unwrap().playback_token, 9);
    state.apply_assignment(assignment(6, 10, 99, PlaybackMode::Paused));
    assert_eq!(
        state.playback.as_ref().unwrap().generation(),
        generation.saturating_add(1)
    );
    state.replace_tracks(vec![super::super::tests::track(1)], true);
    state.command(AppCommand::SelectTrack(Some(1)));
    state.apply_assignment(assignment(7, 10, 99, PlaybackMode::Playing));
    assert_eq!(state.selected, Some(1), "resume is not a queue transition");
    state.playback.take().unwrap().shutdown();
}

#[tokio::test]
async fn same_audio_assignment_without_duration_preserves_probed_metadata() {
    let (mut state, updates) = super::super::tests::controller();
    state.replace_tracks(vec![super::super::tests::track(1)], true);
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    let mut initial = assignment(1, 4, 1, PlaybackMode::Paused);
    initial.duration_ms = None;
    state.apply_assignment(initial);
    state.durations.insert(1, Some(Duration::from_secs(42)));
    state.current_track.as_mut().unwrap().duration = Some(Duration::from_secs(42));
    state.emit_selection();
    let generation = state.playback.as_ref().unwrap().generation();
    let mut resumed = assignment(2, 4, 1, PlaybackMode::Paused);
    resumed.duration_ms = None;
    state.apply_assignment(resumed);
    assert_eq!(
        state.track(1).unwrap().duration,
        Some(Duration::from_secs(42))
    );
    assert_eq!(
        state.selected_media.as_ref().unwrap().duration,
        Some(Duration::from_secs(42))
    );
    assert_eq!(
        state.durations.get(&1),
        Some(&Some(Duration::from_secs(42)))
    );
    assert_eq!(state.playback.as_ref().unwrap().generation(), generation);
    assert!(matches!(
        updates.lock().unwrap().last(),
        Some(AppUpdate::TrackSelected(Some(track))) if track.duration == Some(Duration::from_secs(42))
    ));
    state.playback.take().unwrap().shutdown();
}

#[tokio::test]
async fn callbacks_keep_launch_token_and_device_failure_pauses_instead_of_skipping() {
    let (mut state, _) = super::super::tests::controller();
    state.assignment = Some(assignment(1, 4, 10, PlaybackMode::Playing));
    state.worker_message(crate::playback::Message::Started(3));
    state.worker_message(crate::playback::Message::Finished(3));
    assert!(state.playback_commands.is_empty());
    state.worker_message(crate::playback::Message::Started(4));
    state.worker_message(crate::playback::Message::Finished(4));
    state.worker_message(crate::playback::Message::Failed(4));
    state.worker_message(crate::playback::Message::DeviceFailure(4));
    assert_eq!(state.playback_commands.len(), 4);
    assert_eq!(
        state.playback_commands.pop_front().unwrap().command,
        PlaybackCommand::Started { playback_token: 4 }
    );
    assert_eq!(
        state.playback_commands.pop_front().unwrap().command,
        PlaybackCommand::Finished { playback_token: 4 }
    );
    assert_eq!(
        state.playback_commands.pop_front().unwrap().command,
        PlaybackCommand::Failed { playback_token: 4 }
    );
    assert_eq!(
        state.playback_commands.pop_front().unwrap().command,
        PlaybackCommand::PauseIfCurrent { playback_token: 4 }
    );
    state.assignment.as_mut().unwrap().mode = PlaybackMode::Paused;
    state.worker_message(crate::playback::Message::DeviceFailure(4));
    assert!(state.playback_commands.is_empty());
}

#[tokio::test]
async fn started_assignment_updates_recent_dates_without_restart_or_selection_change() {
    use crate::controller::TrackSort;
    let (mut state, _) = super::super::tests::controller();
    state.replace_tracks(
        vec![super::super::tests::track(1), super::super::tests::track(2)],
        true,
    );
    state.command(AppCommand::SetTrackSort(TrackSort::RecentlyPlayed));
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    state.apply_assignment(assignment(1, 4, 2, PlaybackMode::Paused));
    state.command(AppCommand::SelectTrack(Some(1)));
    let generation = state.playback.as_ref().unwrap().generation();
    let mut started = assignment(2, 4, 2, PlaybackMode::Paused);
    started.track.as_mut().unwrap().last_played_at_ms = Some(100);
    state.apply_assignment(started);
    assert_eq!(
        state
            .tracks
            .iter()
            .map(|track| track.audio_source_id)
            .collect::<Vec<_>>(),
        [2, 1]
    );
    assert_eq!(state.tracks.first().unwrap().last_played_at_ms, Some(100));
    assert_eq!(
        state.current_track.as_ref().unwrap().last_played_at_ms,
        Some(100)
    );
    assert_eq!(state.selected, Some(1));
    assert_eq!(state.playback.as_ref().unwrap().generation(), generation);
    state.apply_assignment(assignment(1, 4, 2, PlaybackMode::Paused));
    state.replace_tracks(
        vec![super::super::tests::track(1), super::super::tests::track(2)],
        true,
    );
    assert_eq!(state.tracks.first().unwrap().last_played_at_ms, Some(100));
    state.playback.take().unwrap().shutdown();
}

#[tokio::test]
async fn stale_worker_updates_do_not_replace_current_and_errors_keep_position_and_volume() {
    let (mut state, updates) = super::super::tests::controller();
    state.assignment = Some(assignment(1, 4, 10, PlaybackMode::Playing));
    state.worker_update(crate::playback::Playback {
        playback_token: 3,
        ..Default::default()
    });
    assert!(updates.lock().unwrap().is_empty());
    let mut playback = crate::playback::Playback {
        current_audio_id: Some(10),
        playback_token: 4,
        has_source: true,
        ..Default::default()
    };
    playback.snapshot.position = Duration::from_secs(20);
    playback.snapshot.volume = 0.3;
    playback.snapshot.state = crate::playback::PlayerState::Playing;
    state.worker_update(playback);
    state.playback_error("stream disconnected".into());
    let last = updates.lock().unwrap().last().cloned();
    let Some(AppUpdate::Playback(playback)) = last else {
        panic!("missing playback update")
    };
    assert_eq!(playback.snapshot.position, Duration::from_secs(20));
    assert_eq!(playback.snapshot.volume.to_bits(), 0.3_f32.to_bits());
    assert!(playback.can_next && playback.can_previous);
}

#[cfg(unix)]
#[tokio::test]
async fn delayed_callback_retry_is_dropped_after_a_new_token_and_does_not_block_commands() {
    let (_directory, session) = super::super::tests::test_session().await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    state.assignment = Some(assignment(1, 4, 10, PlaybackMode::Playing));
    state.playback_command_busy = true;
    state.playback_command_completed(
        PendingPlayback {
            playlist: None,
            command: PlaybackCommand::PauseIfCurrent { playback_token: 4 },
            expected_token: Some(4),
        },
        Err("connection failed".into()),
    );
    assert!(
        !state.playback_command_busy,
        "backoff must not hold the command lane"
    );
    state.assignment = Some(assignment(2, 5, 20, PlaybackMode::Playing));
    let retry = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(retry);
    state.start_playback_command();
    assert!(state.playback_commands.is_empty());
    assert!(!state.playback_command_busy);
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[cfg(unix)]
async fn session_at(url: &str) -> (tempfile::TempDir, crate::Session) {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("server");
    std::fs::write(
        &binary,
        format!("#!/bin/sh\necho 'listening on {url}'\nexec sleep 30\n"),
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let session = crate::Session::start(crate::ServerOptions {
        binary: Some(binary),
        ..Default::default()
    })
    .await
    .unwrap();
    (directory, session)
}
#[cfg(unix)]
async fn read_request(stream: &mut tokio::net::TcpStream) -> String {
    use tokio::io::AsyncReadExt;
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        header.push(stream.read_u8().await.unwrap());
    }
    let header = String::from_utf8(header).unwrap();
    let size = header
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .map(str::to_owned)
        })
        .and_then(|size| size.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; size];
    stream.read_exact(&mut body).await.unwrap();
    header + &String::from_utf8(body).unwrap()
}
#[cfg(unix)]
async fn response(
    stream: &mut tokio::net::TcpStream,
    revision: u64,
    token: u64,
    id: i32,
    newline: bool,
) {
    use tokio::io::AsyncWriteExt;
    let mut body = serde_json::json!({
        "current_audio_source_id": id, "track": {"audio_source_id":id, "title":"Track", "title_unicode":null,"artist":null,"artist_unicode":null,"cover_beatmap_id":null,"difficulties":[]},
        "duration_ms":100_000,"mode":"paused","revision":revision,"playback_token":token,"can_next":true,"can_previous":true,
    }).to_string();
    if newline {
        body.push('\n');
    }
    stream
        .write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
}
#[cfg(unix)]
#[tokio::test]
async fn controller_serializes_http_commands_until_the_previous_response_commits() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (_directory, session) =
        session_at(&format!("http://{}", listener.local_addr().unwrap())).await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    let (ready, waiting) = tokio::sync::oneshot::channel();
    let (release, held) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        let request = read_request(&mut first).await;
        assert!(request.starts_with("POST /api/playback/commands"));
        assert!(request.ends_with(r#"{"command":"play","audio_source_id":7}"#));
        ready.send(()).unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(50), listener.accept())
                .await
                .is_err(),
            "second HTTP command ran concurrently"
        );
        held.await.unwrap();
        response(&mut first, 1, 1, 7, false).await;
        let (mut second, _) = listener.accept().await.unwrap();
        assert!(
            read_request(&mut second)
                .await
                .ends_with(r#"{"command":"next"}"#)
        );
        response(&mut second, 2, 2, 8, false).await;
    });
    state.queue_playback(PlaybackCommand::Play {
        audio_source_id: Some(7),
    });
    state.queue_playback(PlaybackCommand::Next);
    state.start_playback_command();
    waiting.await.unwrap();
    state.start_playback_command();
    assert_eq!(state.playback_commands.len(), 1);
    release.send(()).unwrap();
    let first = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(first);
    assert_eq!(state.assignment.as_ref().unwrap().revision, 1);
    state.start_playback_command();
    let second = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(second);
    assert_eq!(
        state.assignment.as_ref().unwrap().current_audio_source_id,
        Some(8)
    );
    server.await.unwrap();
    state.playback.take().unwrap().shutdown();
    state.session.take().unwrap().shutdown().await.unwrap();
}
#[cfg(unix)]
#[tokio::test]
async fn playlist_append_preserves_pause_selection_order_and_duplicates_and_skips_unavailable() {
    use crate::controller::PlaylistAction;
    use tokio::io::AsyncWriteExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (_directory, session) =
        session_at(&format!("http://{}", listener.local_addr().unwrap())).await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    state.apply_assignment(assignment(1, 5, 7, PlaybackMode::Paused));
    state.replace_tracks(vec![super::super::tests::track(42)], true);
    state.command(AppCommand::SelectTrack(Some(42)));
    let server = tokio::spawn(async move {
        for ids in [vec![Some(42), None, Some(42), Some(103)], vec![None]] {
            let (mut stream, _) = listener.accept().await.unwrap();
            assert!(
                read_request(&mut stream)
                    .await
                    .starts_with("GET /api/playlists/3 ")
            );
            let items: Vec<_> = ids.iter().enumerate().map(|(index, audio_id)| serde_json::json!({
                "id": index, "playlist_id": 3, "source_kind": "stable", "beatmap_hash": format!("hash-{index}"),
                "title": "Track", "artist": null, "difficulty_name": null, "beatmap_id": audio_id,
                "beatmap_set_id": audio_id, "audio_source_id": audio_id, "cover_beatmap_id": null,
            })).collect();
            let body = serde_json::json!({"id": 3, "name": "Playlist", "items": items}).to_string();
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            if ids.iter().any(Option::is_some) {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request = read_request(&mut stream).await;
                assert!(request.starts_with("POST /api/queue/items "));
                assert!(request.ends_with(r#"{"audio_source_ids":[42,42,103]}"#));
                response(&mut stream, 2, 5, 7, false).await;
            } else {
                assert!(
                    tokio::time::timeout(Duration::from_millis(50), listener.accept())
                        .await
                        .is_err(),
                    "unavailable playlist must not append"
                );
            }
        }
    });
    for _ in 0..2 {
        state.command(AppCommand::Playlist(PlaylistAction::AddToQueue(3)));
        state.start_playback_command();
        let completed = state.tasks.join_next().await.unwrap().unwrap();
        state.complete(completed);
        let current = state.assignment.as_ref().unwrap();
        assert_eq!(current.current_audio_source_id, Some(7));
        assert_eq!(current.playback_token, 5);
        assert_eq!(current.mode, PlaybackMode::Paused);
        assert_eq!(state.selected, Some(42));
    }
    server.await.unwrap();
    state.playback.take().unwrap().shutdown();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn stream_reconnect_rereads_snapshot_without_new_generation_or_download() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (_directory, session) =
        session_at(&format!("http://{}", listener.local_addr().unwrap())).await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    let (done, held) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        for expected in [
            "GET /api/playback HTTP/1.1",
            "GET /api/playback/events HTTP/1.1",
            "GET /api/playback HTTP/1.1",
            "GET /api/playback/events HTTP/1.1",
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            assert!(read_request(&mut stream).await.starts_with(expected));
            response(&mut stream, 3, 9, 10, expected.contains("events")).await;
        }
        let _ = held.await;
    });
    state.start_playback_stream();
    let mut generation = None;
    let mut snapshots = 0;
    while snapshots < 4 {
        let snapshot = tokio::time::timeout(Duration::from_secs(3), state.assignments.recv())
            .await
            .unwrap()
            .unwrap();
        if snapshot.is_ok() {
            snapshots += 1;
        }
        state.assignment_result(snapshot);
        if let Some(previous) = generation {
            assert_eq!(state.playback.as_ref().unwrap().generation(), previous);
        } else {
            generation = Some(state.playback.as_ref().unwrap().generation());
        }
    }
    assert!(state.downloads.as_mut().unwrap().try_recv().is_err());
    state.tasks.abort_all();
    while state.tasks.join_next().await.is_some() {}
    done.send(()).unwrap();
    server.await.unwrap();
    state.playback.take().unwrap().shutdown();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[tokio::test]
async fn healthy_same_revision_reconnect_clears_only_transport_error_on_pause() {
    let (mut state, updates) = super::super::tests::controller();
    let paused = assignment(7, 9, 10, PlaybackMode::Paused);
    state.assignment = Some(paused.clone());
    let mut playback = crate::playback::Playback {
        playback_token: 9,
        current_audio_id: Some(10),
        has_source: true,
        ..Default::default()
    };
    playback.snapshot.state = crate::playback::PlayerState::Paused;
    playback.snapshot.position = Duration::from_secs(20);
    state.worker_update(playback.clone());
    state.assignment_result(Err("connection lost".into()));
    state.assignment_result(Ok(paused.clone()));
    let Some(AppUpdate::Playback(healthy)) = updates.lock().unwrap().last().cloned() else {
        panic!("missing playback")
    };
    assert!(healthy.error.is_none());
    assert_eq!(healthy.snapshot.position, Duration::from_secs(20));
    playback.error = Some("device disconnected".into());
    state.worker_update(playback);
    state.assignment_result(Err("connection lost".into()));
    state.assignment_result(Ok(paused));
    let Some(AppUpdate::Playback(failed)) = updates.lock().unwrap().last().cloned() else {
        panic!("missing playback")
    };
    assert_eq!(failed.error.as_deref(), Some("device disconnected"));
    let previous = updates.lock().unwrap().len();
    state.assignment_result(Ok(assignment(6, 8, 10, PlaybackMode::Paused)));
    assert_eq!(updates.lock().unwrap().len(), previous);
    assert!(
        state.playback.is_none(),
        "reconnect must not create a worker"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn device_pause_retries_a_lost_http_request_with_the_launch_token() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (_directory, session) =
        session_at(&format!("http://{}", listener.local_addr().unwrap())).await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    state.assignment = Some(assignment(1, 4, 10, PlaybackMode::Playing));
    let server = tokio::spawn(async move {
        {
            let (mut first, _) = listener.accept().await.unwrap();
            assert!(
                read_request(&mut first)
                    .await
                    .ends_with(r#"{"command":"pause","playback_token":4}"#)
            );
            // Drop before writing a response: the original device pause was not committed.
        }
        let (mut retry, _) = listener.accept().await.unwrap();
        assert!(
            read_request(&mut retry)
                .await
                .ends_with(r#"{"command":"pause","playback_token":4}"#)
        );
        response(&mut retry, 2, 4, 10, false).await;
    });
    state.worker_message(crate::playback::Message::DeviceFailure(4));
    state.start_playback_command();
    let failed = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(failed);
    let retry = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(retry);
    state.start_playback_command();
    let committed = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(committed);
    assert_eq!(
        state.assignment.as_ref().unwrap().mode,
        PlaybackMode::Paused
    );
    assert_eq!(state.assignment.as_ref().unwrap().playback_token, 4);
    server.await.unwrap();
    state.playback.take().unwrap().shutdown();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn started_retry_precedes_immediate_eof_without_blocking_user_pause() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (_directory, session) =
        session_at(&format!("http://{}", listener.local_addr().unwrap())).await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    state.assignment = Some(assignment(1, 4, 10, PlaybackMode::Playing));
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    let server = tokio::spawn(async move {
        {
            let (mut first, _) = listener.accept().await.unwrap();
            assert!(
                read_request(&mut first)
                    .await
                    .ends_with(r#"{"command":"started","playback_token":4}"#)
            );
        }
        for (expected, revision) in [
            (r#"{"command":"pause"}"#, 2),
            (r#"{"command":"started","playback_token":4}"#, 3),
            (r#"{"command":"finished","playback_token":4}"#, 4),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            assert!(read_request(&mut stream).await.ends_with(expected));
            response(&mut stream, revision, 4, 10, false).await;
        }
    });
    state.worker_message(crate::playback::Message::Started(4));
    state.worker_message(crate::playback::Message::Finished(4));
    state.start_playback_command();
    let failed = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(failed);
    state.start_playback_command();
    assert!(
        !state.playback_command_busy,
        "EOF waits for a successful start acknowledgment"
    );
    state.command(AppCommand::Pause);
    state.start_playback_command();
    let paused = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(paused);
    assert_eq!(state.pending_start, Some(4));
    let retry = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(retry);
    state.start_playback_command();
    let started = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(started);
    assert!(state.pending_start.is_none());
    state.start_playback_command();
    let finished = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(finished);
    server.await.unwrap();
    state.playback.take().unwrap().shutdown();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[test]
fn queue_media_demand_survives_empty_main_view_and_is_removed_on_close_or_new_queue() {
    let (mut state, _) = super::super::tests::controller();
    state.replace_tracks(Vec::new(), true);
    state.upcoming.open = true;
    state.upcoming.view.tracks = vec![
        super::super::tests::track(42),
        super::super::tests::track(103),
    ];
    state.request_media(42, true);
    state.set_visible_media(Vec::new());
    assert_eq!(state.queue, std::collections::VecDeque::from([(42, true)]));
    state.replace_tracks(Vec::new(), false);
    assert_eq!(
        state.queue,
        std::collections::VecDeque::from([(42, true)]),
        "navigation retains independent open-queue demand"
    );
    state.set_queue_visible(false);
    assert!(state.queue_media.is_empty() && state.queue.is_empty());
    state.upcoming.open = true;
    state.request_media(103, true);
    let refreshed = crate::models::QueueState {
        upcoming_tracks: Vec::new(),
        audio_source_ids: Vec::new(),
        playlist_item_ids: Vec::new(),
        current_index: None,
        mode: PlaybackMode::Stopped,
        revision: 0,
        playback_token: 0,
    };
    state.queue_loaded(state.upcoming.request, Ok(refreshed));
    assert!(
        state.queue_media.is_empty() && state.queue.is_empty(),
        "removed queue IDs cannot keep media work alive"
    );
}

#[tokio::test]
async fn late_queue_and_assignment_metadata_use_current_unicode_choices_without_restarting() {
    use crate::TrackNamePreferences;
    let (mut state, _) = super::super::tests::controller();
    let preferences = TrackNamePreferences {
        use_unicode_titles: true,
        use_unicode_artists: true,
    };
    let mut first = assignment(5, 9, 99, PlaybackMode::Paused);
    let wire = first.track.as_mut().unwrap();
    wire.title_unicode = Some("曲".into());
    wire.artist_unicode = Some("演奏者".into());
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    state.apply_assignment(first.clone());
    let worker_generation = state.playback.as_ref().unwrap().generation();
    state.command(AppCommand::SetTrackNamePreferences(preferences));
    assert_eq!(state.current_track.as_ref().unwrap().title, "曲");
    assert_eq!(state.assignment.as_ref().unwrap().playback_token, 9);
    assert_eq!(
        state.playback.as_ref().unwrap().generation(),
        worker_generation
    );
    state.upcoming.open = true;
    state.queue_loaded(
        state.upcoming.request,
        Ok(crate::models::QueueState {
            audio_source_ids: vec![99, 99],
            playlist_item_ids: vec![None, None],
            current_index: Some(0),
            mode: PlaybackMode::Paused,
            revision: 5,
            playback_token: 9,
            upcoming_tracks: vec![first.track.clone().unwrap(), first.track.clone().unwrap()],
        }),
    );
    assert!(
        state
            .upcoming
            .view
            .tracks
            .iter()
            .all(|row| row.title == "曲" && row.artist == "演奏者")
    );
    first.revision = 6;
    state.apply_assignment(first);
    assert_eq!(state.current_track.as_ref().unwrap().artist, "演奏者");
    assert_eq!(
        state.playback.as_ref().unwrap().generation(),
        worker_generation
    );
    state.cancel_audio();
}

#[test]
fn discord_uses_local_source_rejects_old_tokens_and_ignores_selection() {
    let (mut state, _) = super::super::tests::controller();
    let assigned = assignment(1, 9, 42, PlaybackMode::Playing);
    state.current_track = assigned.track.clone().map(Track::from);
    state.assignment = Some(assigned);
    let mut playback = crate::playback::Playback {
        current_audio_id: Some(42),
        playback_token: 9,
        has_source: true,
        snapshot: crate::playback::Snapshot {
            state: crate::playback::PlayerState::Playing,
            ..Default::default()
        },
        ..Default::default()
    };
    state.worker_update(playback.clone());
    assert_eq!(state.discord.current_token(), Some(9));
    state.selected = Some(777);
    state.worker_update(playback.clone());
    assert_eq!(state.discord.current_token(), Some(9));
    playback.playback_token = 8;
    playback.snapshot.state = crate::playback::PlayerState::Stopped;
    state.worker_update(playback.clone());
    assert_eq!(
        state.discord.current_token(),
        Some(9),
        "stale local stop cannot clear current RPC"
    );
    playback.playback_token = 9;
    state.worker_update(playback);
    assert_eq!(state.discord.current_token(), None);
}

#[test]
fn name_preference_change_cannot_republish_previous_token_for_the_same_audio() {
    let (mut state, _) = super::super::tests::controller();
    let assigned = assignment(1, 9, 42, PlaybackMode::Playing);
    state.current_track = assigned.track.clone().map(Track::from);
    state.assignment = Some(assigned);
    state.worker_update(crate::playback::Playback {
        current_audio_id: Some(42),
        playback_token: 9,
        has_source: true,
        snapshot: crate::playback::Snapshot {
            state: crate::playback::PlayerState::Playing,
            ..Default::default()
        },
        ..Default::default()
    });
    assert_eq!(state.discord.current_token(), Some(9));
    state.assignment = Some(assignment(2, 10, 42, PlaybackMode::Playing));
    state.set_name_preferences(crate::TrackNamePreferences {
        use_unicode_titles: true,
        ..Default::default()
    });
    assert_eq!(state.discord.current_token(), None);
}
