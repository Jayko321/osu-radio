#![cfg(unix)]

use super::tests::{assignment, read_request, session_at};
use super::*;
use crate::controller::{
    AppCommand,
    tests::{controller, queue_row, track},
};
use std::sync::Arc;

#[test]
fn selecting_a_search_result_is_presentation_only_and_explicit_play_targets_that_audio() {
    let (mut state, _) = controller();
    state.replace_tracks(vec![track(1), track(2), track(3)], true);
    let mut current = assignment(1, 10, 1, PlaybackMode::Playing);
    current.current_entry_id = Some(QueueEntryId::new(1).unwrap());
    state.player.assignment = Some(current.clone());
    state.upcoming.view.entries = vec![queue_row(track(1), 1), queue_row(track(3), 2)];
    state.library.query = "B".into();
    state.command(AppCommand::SelectTrack(Some(2)));
    assert!(state.player.commands.is_empty() && !state.player.command_busy);
    assert_eq!(state.player.assignment, Some(current));
    assert_eq!(
        state
            .upcoming
            .view
            .entries
            .iter()
            .map(|row| row.audio_source_id)
            .collect::<Vec<_>>(),
        [Some(1), Some(3)]
    );
    assert!(matches!(
        state
            .player
            .primary_action(state.player.selected_track(), None),
        Some(crate::view_models::player::PrimaryAction::Playback(
            PlaybackCommand::Play {
                audio_source_id: Some(2)
            }
        ))
    ));
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // One ordered HTTP scenario covers all four transport states.
async fn insert_and_check_responses_keep_empty_exhausted_playing_and_paused_launches() {
    use tokio::io::AsyncWriteExt;
    for (mode, exhausted) in [
        (PlaybackMode::Stopped, false),
        (PlaybackMode::Stopped, true),
        (PlaybackMode::Playing, false),
        (PlaybackMode::Paused, false),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (_directory, session) =
            session_at(&format!("http://{}", listener.local_addr().unwrap())).await;
        let (mut state, _) = controller();
        state.session = Some(session);
        let (worker, downloads) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
        state.playback = Some(worker);
        state.downloads = Some(downloads);
        let mut initial = assignment(1, 10, 7, mode);
        if mode == PlaybackMode::Stopped {
            initial.current_audio_source_id = None;
            initial.track = None;
        } else {
            initial.current_entry_id = Some(QueueEntryId::new(1).unwrap());
            initial.availability = Some(QueueAvailability::Available);
        }
        state.apply_assignment(initial.clone());
        state.replace_tracks(vec![track(42)], true);
        state.command(AppCommand::SelectTrack(Some(42)));
        if exhausted {
            state.upcoming.view.entries = vec![queue_row(track(7), 1), queue_row(track(8), 2)];
            state.upcoming.view.history_len = 2;
        }
        let confirmed = state.upcoming.view.entries.clone();
        let generation = state.playback.as_ref().unwrap().generation();
        let response = serde_json::json!({
            "current_entry_id": initial.current_entry_id,
            "current_audio_source_id": initial.current_audio_source_id,
            "track": initial.current_audio_source_id.map(|audio| serde_json::json!({
                "audio_source_id": audio, "title":"Track 7", "title_unicode":null,
                "artist":null, "artist_unicode":null, "cover_beatmap_id":null, "difficulties":[]
            })),
            "availability":initial.availability, "waiting_for_check":false,
            "mode":mode, "revision":1, "playback_token":10, "can_next":true, "can_previous":true
        });
        let server = tokio::spawn(async move {
            for index in 0..5 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let request = read_request(&mut stream).await;
                assert!(request.starts_with("POST /api/queue/items "), "{request}");
                let body: serde_json::Value =
                    serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
                assert_eq!(
                    body.get("placement").unwrap(),
                    if index % 2 == 0 { "end" } else { "next" }
                );
                assert_eq!(
                    *body.get("items").unwrap(),
                    if index < 2 || index == 4 {
                        serde_json::json!([{"type":"audio","audio_source_id":42}])
                    } else {
                        serde_json::json!([{"type":"playlist_item","playlist_id":3,"item_id":4}])
                    }
                );
                let (status, body) = if index == 4 {
                    (
                        "409 Conflict",
                        serde_json::json!({"error":"insert rejected"}).to_string(),
                    )
                } else {
                    let mut next = response.clone();
                    *next.get_mut("revision").unwrap() = serde_json::json!(index + 2);
                    ("200 OK", next.to_string())
                };
                stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
        });
        for index in 0..5 {
            let item = if index < 2 || index == 4 {
                QueueItemRef::Audio {
                    audio_source_id: 42,
                }
            } else {
                QueueItemRef::PlaylistItem {
                    playlist_id: 3,
                    item_id: 4,
                }
            };
            state.command(AppCommand::Queue(QueueAction::Insert {
                items: vec![item],
                placement: if index % 2 == 0 {
                    QueuePlacement::End
                } else {
                    QueuePlacement::Next
                },
            }));
            state.start_playback_command();
            let completed = tokio::time::timeout(Duration::from_secs(2), state.tasks.join_next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            state.complete(completed);
            assert_eq!(state.upcoming.view.entries, confirmed);
            assert_eq!(state.library.selected, Some(42));
            let launch = state.player.assignment.as_ref().unwrap();
            assert_eq!(
                (launch.current_entry_id, launch.playback_token, launch.mode),
                (initial.current_entry_id, initial.playback_token, mode)
            );
            assert_eq!(state.playback.as_ref().unwrap().generation(), generation);
        }
        assert!(
            state
                .upcoming
                .view
                .operation_error
                .as_ref()
                .unwrap()
                .contains("insert rejected")
        );
        // Check completion for inserted entries sends a newer revision of the same launch.
        let mut checked = initial.clone();
        checked.revision = 10;
        state.apply_assignment(checked);
        assert_eq!(state.playback.as_ref().unwrap().generation(), generation);
        assert_eq!(state.library.selected, Some(42));
        assert_eq!(state.player.assignment.as_ref().unwrap().mode, mode);
        server.await.unwrap();
        state.playback.take().unwrap().shutdown();
        state.session.take().unwrap().shutdown().await.unwrap();
    }
}
