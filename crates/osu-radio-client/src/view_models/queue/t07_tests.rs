use super::tests::{assignment, queue_entry};
use super::*;
use crate::controller::{
    AppCommand,
    tests::{controller, queue_row, track},
};
use crate::models::{QueueAvailability, QueueEntryId, QueueItemRef, QueuePlacement, QueueState};
use std::sync::Arc;

fn id(value: i64) -> QueueEntryId {
    QueueEntryId::new(value).unwrap()
}
fn snapshot(entries: Vec<QueueEntry>, revision: u64) -> QueueState {
    QueueState {
        entries,
        current_entry_id: Some(id(2)),
        history_len: 1,
        mode: PlaybackMode::Paused,
        waiting_for_check: false,
        revision,
        playback_token: 10,
    }
}

#[test]
fn full_projection_keeps_history_missing_ids_captions_and_order_on_errors() {
    let (mut state, _) = controller();
    state.upcoming.open = true;
    let mut entries: Vec<_> = [7, 7, 99, 7]
        .into_iter()
        .enumerate()
        .map(|(pos, audio)| {
            queue_entry(
                assignment(1, 10, audio, PlaybackMode::Paused)
                    .track
                    .unwrap(),
                pos,
            )
        })
        .collect();
    entries.get_mut(0).unwrap().audio_source_id = None;
    entries.get_mut(0).unwrap().title = Some("Saved missing source".into());
    entries.get_mut(0).unwrap().difficulty_name = Some("Hard".into());
    entries.get_mut(0).unwrap().availability = QueueAvailability::Unavailable;
    entries.get_mut(2).unwrap().availability = QueueAvailability::Checking;
    state.queue_loaded(0, Ok(snapshot(entries, 1)));
    let confirmed = state.upcoming.view.clone();
    assert_eq!(
        confirmed
            .entries
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        [id(1), id(2), id(3), id(4)]
    );
    assert_eq!(confirmed.entries.first().unwrap().audio_source_id, None);
    assert_eq!(
        confirmed.entries.first().unwrap().track.title,
        "Saved missing source"
    );
    assert!(
        confirmed
            .entries
            .first()
            .unwrap()
            .track
            .subtitle
            .ends_with(" | Hard")
    );
    assert_eq!(
        confirmed.upcoming().map(|row| row.id).collect::<Vec<_>>(),
        [id(3), id(4)]
    );
    assert!(state.media.queue.is_empty(), "GET alone creates no demand");
    state.queue_loaded(0, Err("GET failed".into()));
    assert_eq!(state.upcoming.view.entries, confirmed.entries);
    assert_eq!(
        state.upcoming.view.current_entry_id,
        confirmed.current_entry_id
    );
    assert_eq!(state.upcoming.view.message, "GET failed");
}

#[test]
fn five_thousand_history_rows_only_schedule_visible_entries_and_share_audio() {
    let (mut state, _) = controller();
    state.set_visible_media(Vec::new());
    state.upcoming.open = true;
    let entries = (0..5004)
        .map(|pos| {
            queue_entry(
                assignment(
                    1,
                    10,
                    if pos >= 5000 {
                        42
                    } else {
                        i32::try_from(pos).unwrap() + 100
                    },
                    PlaybackMode::Paused,
                )
                .track
                .unwrap(),
                pos,
            )
        })
        .collect();
    let mut queue = snapshot(entries, 1);
    queue.history_len = 5000;
    queue.current_entry_id = Some(id(5001));
    state.queue_loaded(0, Ok(queue));
    assert!(state.media.queue.is_empty());
    state.command(AppCommand::SetVisibleQueueMedia(vec![
        (id(1), true),
        (id(5002), false),
        (id(5003), true),
        (id(8000), true),
    ]));
    assert_eq!(state.media.queue.len(), 2);
    assert_eq!(state.media.queue_media.len(), 2);
    assert_eq!(state.media.queue_media.get(&42), Some(&true));
    state.command(AppCommand::SetVisibleQueueMedia(vec![(id(5004), false)]));
    assert_eq!(
        state.media.queue,
        std::collections::VecDeque::from([(42, false)])
    );
    state.request_media(100, true);
    assert_eq!(
        state.media.queue.len(),
        1,
        "legacy requests cannot extend a viewport snapshot"
    );
    state.set_queue_visible(false);
    assert!(state.media.queue.is_empty() && state.media.queue_media.is_empty());
}

#[tokio::test]
async fn paused_recovery_preserves_library_and_playlist_selection_and_rejects_waiting_callbacks() {
    let (mut state, _) = controller();
    state.replace_tracks(vec![track(1), track(2)], true);
    state.command(AppCommand::SelectTrack(Some(2)));
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    let mut restored = assignment(1, 10, 99, PlaybackMode::Paused);
    restored.current_entry_id = Some(id(9));
    restored.waiting_for_check = true;
    restored.availability = Some(QueueAvailability::Checking);
    state.apply_assignment(restored.clone());
    assert_eq!(state.library.selected, Some(2));
    assert_eq!(
        state
            .player
            .selected_track
            .as_ref()
            .unwrap()
            .audio_source_id,
        2
    );
    let generation = state.playback.as_ref().unwrap().generation();
    for feedback in [
        crate::playback::Message::Started(10),
        crate::playback::Message::Failed(10),
        crate::playback::Message::Finished(10),
    ] {
        state.worker_message(feedback);
    }
    assert!(state.player.commands.is_empty() && state.player.pending_start.is_none());
    assert_eq!(state.discord.current_token(), None);
    assert!(state.library.last_played.is_empty());
    restored.revision += 1;
    restored.waiting_for_check = false;
    restored.availability = Some(QueueAvailability::Available);
    state.apply_assignment(restored);
    assert_eq!(state.playback.as_ref().unwrap().generation(), generation);
    assert_eq!(state.library.selected, Some(2));
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(state.downloads.as_mut().unwrap().try_recv().is_err());
    state.playback.take().unwrap().shutdown();
}

#[tokio::test]
async fn reimport_queue_rebinding_keeps_assignment_source_date_volume_and_generation() {
    let (mut state, _) = controller();
    state.upcoming.open = false;
    state.replace_tracks(vec![track(7)], true);
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    let mut launch = assignment(1, 10, 7, PlaybackMode::Paused);
    launch.current_entry_id = Some(id(2));
    launch.volume_percent = Some(23);
    launch.track.as_mut().unwrap().last_played_at_ms = Some(100);
    state.apply_assignment(launch.clone());
    let generation = state.playback.as_ref().unwrap().generation();
    state.upcoming.open = true;
    let mut rebound = queue_entry(
        assignment(2, 10, 99, PlaybackMode::Paused).track.unwrap(),
        1,
    );
    rebound.availability = QueueAvailability::Checking;
    state.queue_loaded(
        0,
        Ok(snapshot(
            vec![queue_entry(launch.track.clone().unwrap(), 0), rebound],
            2,
        )),
    );
    state.upcoming.open = false;
    launch.revision = 2;
    launch.availability = Some(QueueAvailability::Checking);
    state.apply_assignment(launch);
    assert_eq!(
        state.upcoming.view.entries.get(1).unwrap().audio_source_id,
        Some(99)
    );
    let current = state.player.current_track.as_ref().unwrap();
    assert_eq!(
        (
            current.audio_source_id,
            current.last_played_at_ms,
            current.volume_percent
        ),
        (7, Some(100), Some(23))
    );
    assert_eq!(state.playback.as_ref().unwrap().generation(), generation);
    assert!(!state.library.last_played.contains_key(&99));
    state.playback.take().unwrap().shutdown();
}

#[cfg(unix)]
#[tokio::test]
#[allow(clippy::too_many_lines)] // One ordered wire scenario includes all queue mutations.
async fn mixed_queue_commands_are_serial_and_errors_and_noops_keep_confirmed_state() {
    use super::tests::{read_request, response, session_at};
    use tokio::io::AsyncWriteExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (_directory, session) =
        session_at(&format!("http://{}", listener.local_addr().unwrap())).await;
    let (mut state, _) = controller();
    state.session = Some(session);
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    state.player.assignment = Some(assignment(1, 10, 7, PlaybackMode::Paused));
    state.upcoming.view.entries = vec![queue_row(track(42), 3)];
    let confirmed = state.upcoming.view.entries.clone();
    let actions = [
        QueueAction::Insert {
            items: vec![
                QueueItemRef::Audio { audio_source_id: 7 },
                QueueItemRef::PlaylistItem {
                    playlist_id: 3,
                    item_id: 4,
                },
            ],
            placement: QueuePlacement::Next,
        },
        QueueAction::Move {
            id: id(3),
            before_entry_id: None,
        },
        QueueAction::Play(id(3)),
        QueueAction::Remove(id(3)),
        QueueAction::ClearUpcoming,
        QueueAction::ClearAll,
        QueueAction::Insert {
            items: vec![
                QueueItemRef::PlaylistItem {
                    playlist_id: 3,
                    item_id: 4,
                },
                QueueItemRef::Audio { audio_source_id: 7 },
            ],
            placement: QueuePlacement::End,
        },
        QueueAction::Move {
            id: id(3),
            before_entry_id: Some(id(2)),
        },
    ];
    let server = tokio::spawn(async move {
        for (index, prefix) in [
            "POST /api/queue/items ",
            "PATCH /api/queue/items/3 ",
            "POST /api/queue/items/3/play ",
            "DELETE /api/queue/items/3 ",
            "DELETE /api/queue/upcoming ",
            "DELETE /api/queue ",
            "POST /api/queue/items ",
            "PATCH /api/queue/items/3 ",
        ]
        .into_iter()
        .enumerate()
        {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            assert!(request.starts_with(prefix), "{request}");
            if index == 0 || index == 6 {
                let body: serde_json::Value =
                    serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
                assert_eq!(
                    body.get("placement").unwrap(),
                    if index == 0 { "next" } else { "end" }
                );
                assert_eq!(body.get("items").unwrap().as_array().unwrap().len(), 2);
            }
            if index == 1 {
                assert!(request.ends_with(r#"{"before_entry_id":null}"#));
            }
            if index == 7 {
                assert!(request.ends_with(r#"{"before_entry_id":"2"}"#));
            }
            assert!(
                tokio::time::timeout(Duration::from_millis(15), listener.accept())
                    .await
                    .is_err(),
                "parallel queue HTTP"
            );
            if index == 0 {
                let body = r#"{"error":"insert rejected"}"#;
                stream.write_all(format!("HTTP/1.1 409 Conflict\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            } else {
                response(&mut stream, 1, 10, 7, false).await;
            }
        }
        assert!(
            tokio::time::timeout(Duration::from_millis(30), listener.accept())
                .await
                .is_err(),
            "closed panel must not GET or retry insert"
        );
    });
    for action in actions {
        state.command(AppCommand::Queue(action));
    }
    assert!(state.upcoming.view.busy);
    for index in 0..8 {
        state.start_playback_command();
        state.start_playback_command();
        let completed = tokio::time::timeout(Duration::from_secs(2), state.tasks.join_next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        state.complete(completed);
        assert_eq!(state.upcoming.view.entries, confirmed);
        assert_eq!(state.player.assignment.as_ref().unwrap().playback_token, 10);
        if index == 0 {
            assert!(
                state
                    .upcoming
                    .view
                    .operation_error
                    .as_ref()
                    .unwrap()
                    .contains("insert rejected")
            );
        }
    }
    assert!(!state.upcoming.view.busy && state.upcoming.view.operation_error.is_none());
    server.await.unwrap();
    state.playback.take().unwrap().shutdown();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn successful_noop_refreshes_open_panel_and_close_discards_its_late_get() {
    use super::tests::{read_request, response, session_at};
    use tokio::io::AsyncWriteExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (_directory, session) =
        session_at(&format!("http://{}", listener.local_addr().unwrap())).await;
    let (mut state, _) = controller();
    state.session = Some(session);
    state.player.assignment = Some(assignment(1, 10, 7, PlaybackMode::Paused));
    state.upcoming.open = true;
    state.upcoming.view.entries = vec![queue_row(track(7), 1)];
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, held) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        assert!(
            read_request(&mut stream)
                .await
                .starts_with("DELETE /api/queue/upcoming ")
        );
        response(&mut stream, 1, 10, 7, false).await;
        let (mut get, _) = listener.accept().await.unwrap();
        assert!(read_request(&mut get).await.starts_with("GET /api/queue "));
        entered.send(()).unwrap();
        held.await.unwrap();
        let body = r#"{"entries":[],"current_entry_id":null,"history_len":0,"mode":"stopped","waiting_for_check":false,"revision":2,"playback_token":11}"#;
        let _ = get
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await;
    });
    state.command(AppCommand::Queue(QueueAction::ClearUpcoming));
    state.start_playback_command();
    let completed = state.tasks.join_next().await.unwrap().unwrap();
    state.complete(completed);
    tokio::time::timeout(Duration::from_secs(2), ready)
        .await
        .unwrap()
        .unwrap();
    assert!(state.upcoming.view.loading);
    let request = state.upcoming.request;
    state.set_queue_visible(false);
    release.send(()).unwrap();
    server.await.unwrap();
    while state.tasks.join_next().await.is_some() {}
    state.queue_loaded(request, Ok(snapshot(Vec::new(), 2)));
    assert_eq!(state.upcoming.view.entries.len(), 1);
    assert_eq!(state.player.assignment.as_ref().unwrap().revision, 1);
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn visible_queue_media_uses_four_pipelines_and_downloads_duplicates_once() {
    use super::tests::{read_request, session_at};
    use tokio::io::AsyncWriteExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (_directory, session) =
        session_at(&format!("http://{}", listener.local_addr().unwrap())).await;
    let (mut state, _) = controller();
    state.session = Some(session);
    state.set_visible_media(Vec::new());
    state.upcoming.open = true;
    let entries = (0..5007)
        .map(|pos| {
            queue_entry(
                assignment(
                    1,
                    10,
                    if pos >= 5000 {
                        i32::try_from(pos.min(5005)).unwrap()
                    } else {
                        42
                    },
                    PlaybackMode::Paused,
                )
                .track
                .unwrap(),
                pos,
            )
        })
        .collect();
    let mut queue = snapshot(entries, 1);
    queue.history_len = 5000;
    queue.current_entry_id = Some(id(5001));
    state.queue_loaded(0, Ok(queue));
    assert!(state.tasks.is_empty());
    let server = tokio::spawn(async move {
        let mut paths = std::collections::HashMap::<String, usize>::new();
        for _ in 0..12 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;
            let path = request.split_whitespace().nth(1).unwrap().to_owned();
            *paths.entry(path.clone()).or_default() += 1;
            let body = if path.ends_with("/duration") {
                r#"{"duration_ms":125000}"#
            } else {
                ""
            };
            let status = if body.is_empty() {
                "404 Not Found"
            } else {
                "200 OK"
            };
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        }
        paths
    });
    state.set_visible_queue_media((5001..=5007).map(|entry| (id(entry), true)).collect());
    state.start_media();
    assert_eq!(state.media.jobs.len(), 4);
    assert_eq!(
        state.tasks.len(),
        8,
        "four cover and four duration requests"
    );
    while let Some(completed) = state.tasks.join_next().await {
        state.complete(completed.unwrap());
    }
    state.start_media();
    assert_eq!(state.media.jobs.len(), 2);
    while let Some(completed) = state.tasks.join_next().await {
        state.complete(completed.unwrap());
    }
    let paths = server.await.unwrap();
    assert_eq!(paths.len(), 12);
    assert!(paths.values().all(|count| *count == 1));
    assert!(
        paths
            .keys()
            .all(|path| !path.contains("/42/") && !path.ends_with("/142/cover")),
        "history must not download"
    );
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[tokio::test]
async fn first_paused_playlist_assignment_keeps_the_selected_item() {
    use crate::models::{Playlist, PlaylistItem};
    let (mut state, _) = controller();
    state.replace_tracks(vec![track(1), track(2)], true);
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(7);
    state.install_playlist(Playlist {
        id: 7,
        name: "Demo".into(),
        items: (1..=2)
            .map(|audio| PlaylistItem {
                id: audio,
                playlist_id: 7,
                source_kind: "stable".into(),
                beatmap_hash: audio.to_string(),
                title: Some(format!("Song {audio}")),
                title_unicode: None,
                artist: None,
                artist_unicode: None,
                difficulty_name: None,
                beatmap_id: Some(audio),
                beatmap_set_id: Some(1),
                audio_source_id: Some(audio),
                cover_beatmap_id: None,
                last_played_at_ms: None,
                volume_percent: None,
            })
            .collect(),
    });
    state.playlist_action(crate::view_models::PlaylistAction::SelectItem(2));
    let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.downloads = Some(downloads);
    let mut restored = assignment(1, 10, 1, PlaybackMode::Paused);
    restored.current_entry_id = Some(id(1));
    restored.current_playlist_item_id = Some(1);
    state.apply_assignment(restored);
    assert_eq!(state.playlists.view.selected_item_id, Some(2));
    assert_eq!(
        state
            .player
            .selected_track
            .as_ref()
            .unwrap()
            .audio_source_id,
        2
    );
    state.playback.take().unwrap().shutdown();
}

#[test]
fn waiting_toggle_uses_server_intention_before_worker_has_a_local_source() {
    let (mut state, _) = controller();
    let mut waiting = assignment(1, 10, 7, PlaybackMode::Playing);
    waiting.waiting_for_check = true;
    waiting.current_entry_id = Some(id(1));
    state.player.assignment = Some(waiting.clone());
    let selected = track(7);
    assert!(matches!(
        state.player.primary_action(Some(&selected), None),
        Some(crate::view_models::player::PrimaryAction::Playback(
            PlaybackCommand::Pause
        ))
    ));
    waiting.mode = PlaybackMode::Paused;
    state.player.assignment = Some(waiting);
    assert!(matches!(
        state.player.primary_action(Some(&selected), None),
        Some(crate::view_models::player::PrimaryAction::Playback(
            PlaybackCommand::Play {
                audio_source_id: None
            }
        ))
    ));
    let presentation = state.player.presentation(None, true, true, true);
    assert!(
        presentation.waiting_for_check
            && !presentation.can_seek
            && !presentation.selected_is_playing
    );
    assert_eq!(presentation.message, "Checking audio…");
}
