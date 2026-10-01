#![allow(clippy::indexing_slicing)]
use super::*;
use crate::controller::{AppCommand, TrackSort};
use crate::models::PlaybackAssignment;

fn playlist(id: i32) -> Playlist {
    Playlist {
        id,
        name: format!("Playlist {id}"),
        items: ["Easy", "Hard", "Missing"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| {
                let item_id = i32::try_from(index).unwrap().saturating_add(1);
                PlaylistItem {
                    id: item_id,
                    last_played_at_ms: None,
                    volume_percent: None,
                    playlist_id: id,
                    source_kind: "stable".into(),
                    beatmap_hash: name.into(),
                    title: Some("Song".into()),
                    artist: Some("Artist".into()),
                    difficulty_name: Some(name.into()),
                    beatmap_id: (index < 2).then_some(item_id),
                    beatmap_set_id: (index < 2).then_some(10),
                    audio_source_id: (index < 2).then_some(20),
                    cover_beatmap_id: None,
                }
            })
            .collect(),
    }
}
fn install(state: &mut Controller, id: i32) {
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(id);
    state.install_playlist(playlist(id));
}

#[test]
fn separate_difficulties_selection_and_stale_responses_preserve_the_current_view() {
    let (mut state, updates) = super::super::tests::controller();
    state.replace_tracks(vec![super::super::tests::track(20)], true);
    install(&mut state, 7);
    assert_eq!(state.playlists.view.tracks().len(), 3);
    assert_ne!(
        state.playlists.tracks[0].subtitle,
        state.playlists.tracks[1].subtitle
    );
    assert!(state.playlists.tracks[2].subtitle.contains("Unavailable"));
    state.playlist_action(PlaylistAction::SelectItem(2));
    assert_eq!(state.playlists.view.selected_item_id, Some(2));
    assert!(
        state.assignment.is_none(),
        "selection never starts playback"
    );
    let before = updates.lock().unwrap().len();
    state.playlist_event(PlaylistEvent::Detail {
        epoch: 99,
        id: 7,
        result: Ok(playlist(8)),
    });
    state.playlist_event(PlaylistEvent::Detail {
        epoch: 0,
        id: 8,
        result: Err("old error".into()),
    });
    state.playlists.list_epoch = 2;
    state.playlist_event(PlaylistEvent::List {
        epoch: 1,
        result: Err("old list error".into()),
    });
    assert_eq!(updates.lock().unwrap().len(), before);
    assert_eq!(state.playlists.view.active.as_ref().unwrap().id, 7);
    state.playlist_action(PlaylistAction::Select(None));
    assert!(state.playlists.view.active_id.is_none());
    assert!(
        matches!(updates.lock().unwrap().iter().rev().find(|event| matches!(event, AppUpdate::TracksReplaced { .. })), Some(AppUpdate::TracksReplaced { tracks, .. }) if tracks.len() == 1)
    );
}

#[test]
fn playlist_duration_notifications_work_outside_the_library_search_results() {
    let (mut state, updates) = super::super::tests::controller();
    state.replace_tracks(Vec::new(), true);
    install(&mut state, 7);
    let ticket = super::super::MediaTicket {
        generation: state.generation,
        audio_id: 20,
        serial: 1,
    };
    state.jobs.insert(
        1,
        super::super::MediaJob {
            ticket,
            cover: None,
            decoding: false,
            artwork_pending: true,
            duration_pending: true,
            tasks: Vec::new(),
        },
    );
    updates.lock().unwrap().clear();
    state.media_completed(ticket, None, Some(42_000), true);
    assert_eq!(state.playlists.tracks[0].duration_label(), "00:42");
    assert_eq!(state.playlists.tracks[1].duration_label(), "00:42");
    assert!(state.playlists.tracks[0].subtitle.contains("Easy"));
    assert!(state.playlists.tracks[1].subtitle.contains("Hard"));
    assert!(updates.lock().unwrap().iter().any(|event| {
        matches!(event, AppUpdate::TrackChanged(track) if track.audio_source_id == 20 && track.duration_label() == "00:42")
    }));
}

#[test]
fn recent_assignment_reorders_shared_audio_rows_and_late_playlist_dates_cannot_regress() {
    let (mut state, updates) = super::super::tests::controller();
    state.replace_tracks(vec![super::super::tests::track(20)], true);
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(7);
    let mut incoming = playlist(7);
    for item in &mut incoming.items {
        item.title = Some(
            if item.audio_source_id.is_some() {
                "Zulu"
            } else {
                "Alpha"
            }
            .into(),
        );
    }
    incoming.items[2].last_played_at_ms = Some(3);
    let mut unavailable = incoming.items[2].clone();
    unavailable.id = 4;
    incoming.items.insert(0, unavailable);
    state.install_playlist(incoming.clone());
    assert_eq!(
        state
            .playlists
            .view
            .active
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        [3, 4, 1, 2]
    );
    state.playlist_action(PlaylistAction::SelectItem(2));
    state
        .playlists
        .tracks
        .iter_mut()
        .for_each(|track| track.duration = Some(std::time::Duration::from_secs(42)));
    let generation = state.generation;
    state.command(AppCommand::SetTrackSort(TrackSort::RecentlyPlayed));
    assert_eq!(
        state
            .playlists
            .view
            .active
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        [3, 4, 1, 2]
    );
    let mut started = super::super::tests::track(20);
    started.last_played_at_ms = Some(100);
    state.current_track = Some(started);
    updates.lock().unwrap().clear();
    state.assignment_last_played();
    let active = state.playlists.view.active.as_ref().unwrap();
    assert_eq!(
        active.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    assert_eq!(
        active
            .items
            .iter()
            .map(|item| item.last_played_at_ms)
            .collect::<Vec<_>>(),
        [Some(100), Some(100), Some(3), Some(3)]
    );
    assert_eq!(state.tracks[0].last_played_at_ms, Some(100));
    assert_eq!(state.playlists.view.selected_item_id, Some(2));
    assert_eq!(state.generation, generation);
    assert!(
        state
            .playlists
            .tracks
            .iter()
            .all(|track| track.duration == Some(std::time::Duration::from_secs(42)))
    );
    let events = updates.lock().unwrap();
    assert!(matches!(events.first(), Some(AppUpdate::Playlists(_))));
    assert!(matches!(events.get(1), Some(AppUpdate::TracksReordered(_))));
    drop(events);
    state.install_playlist(incoming);
    assert_eq!(state.playlists.view.selected_item_id, Some(2));
    assert_eq!(
        state
            .playlists
            .tracks
            .iter()
            .take(2)
            .map(|track| track.last_played_at_ms)
            .collect::<Vec<_>>(),
        [Some(100), Some(100)]
    );
    assert!(state.playlists.tracks[0].subtitle.contains("Easy"));
    assert!(state.playlists.tracks[1].subtitle.contains("Hard"));
}

#[cfg(unix)]
#[tokio::test]
async fn playback_uses_item_identity_and_mutations_ignore_a_different_open_playlist() {
    let (_directory, session) = super::super::tests::test_session().await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    install(&mut state, 7);
    state.assignment = Some(PlaybackAssignment {
        volume_percent: None,
        current_audio_source_id: Some(20),
        current_playlist_item_id: Some(1),
        track: None,
        duration_ms: None,
        mode: PlaybackMode::Paused,
        revision: 1,
        playback_token: 9,
        can_next: true,
        can_previous: true,
    });
    state.playlist_action(PlaylistAction::TogglePlayback);
    let resume = state.playback_commands.pop_front().unwrap();
    assert_eq!(
        resume.command,
        PlaybackCommand::Play {
            audio_source_id: None
        }
    );
    assert!(resume.playlist.is_none());
    state.playlist_action(PlaylistAction::SelectItem(2));
    state.playlist_action(PlaylistAction::TogglePlayback);
    assert!(matches!(
        state.playback_commands.pop_front().unwrap().playlist,
        Some(PlaylistPlayback::Play(7, Some(2)))
    ));
    state.playlist_action(PlaylistAction::SelectItem(3));
    state.playlist_action(PlaylistAction::TogglePlayback);
    assert!(state.playback_commands.is_empty());
    install(&mut state, 8);
    state.playlist_event(PlaylistEvent::Mutation(Ok(Mutation::Items(playlist(7)))));
    assert_eq!(state.playlists.view.active.as_ref().unwrap().id, 8);
    state.playlist_event(PlaylistEvent::Mutation(Ok(Mutation::Deleted(7))));
    assert_eq!(state.playlists.view.active_id, Some(8));
    state.tasks.abort_all();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn add_dialog_keeps_concrete_difficulties_and_failed_requests_keep_retryable_state() {
    let (_directory, session) = super::super::tests::test_session().await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    let mut track = super::super::tests::track(20);
    track.difficulties = playlist(7)
        .items
        .iter()
        .take(2)
        .map(|item| TrackDifficulty {
            beatmap_id: item.beatmap_id.unwrap(),
            beatmap_set_id: 10,
            difficulty_name: item.difficulty_name.clone(),
            set_has_multiple_audio_sources: false,
        })
        .collect();
    state.replace_tracks(vec![track.clone()], true);
    state.command(AppCommand::Playlist(PlaylistAction::OpenAdd));
    assert!(state.playlists.view.open && state.playlists.view.adding);
    assert_eq!(state.playlists.view.candidates.len(), 2);
    state.playlist_action(PlaylistAction::ToggleDifficulty(1));
    assert!(!state.playlists.view.candidates[0].checked);
    state.playlists.view.target_id = Some(7);
    state.playlist_action(PlaylistAction::Add);
    assert!(state.playlists.view.busy);
    state.playlist_action(PlaylistAction::Close);
    assert!(state.playlists.view.open);
    state.playlist_event(PlaylistEvent::Mutation(Err("source hash missing".into())));
    assert!(!state.playlists.view.busy && state.playlists.view.open);
    assert_eq!(state.playlists.view.message, "source hash missing");
    assert!(!state.playlists.view.candidates[0].checked);
    install(&mut state, 7);
    state.replace_tracks(vec![super::super::tests::track(999)], true);
    state.playlist_action(PlaylistAction::OpenAdd);
    assert!(state.playlists.view.candidates.is_empty());
    let epoch = state.playlists.candidates_epoch;
    state.playlist_event(PlaylistEvent::Candidates {
        epoch,
        result: Ok(track.clone()),
    });
    assert_eq!(state.playlists.view.candidates.len(), 2);
    state.playlist_action(PlaylistAction::OpenAdd);
    state.playlist_event(PlaylistEvent::Candidates {
        epoch,
        result: Err("stale candidates".into()),
    });
    assert_ne!(state.playlists.view.message, "stale candidates");
    state.playlist_action(PlaylistAction::Close);
    state.playlist_event(PlaylistEvent::Candidates {
        epoch: state.playlists.candidates_epoch,
        result: Ok(track),
    });
    assert!(state.playlists.view.candidates.is_empty());
    state.tasks.abort_all();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[test]
fn volume_is_shared_across_difficulties_playlists_and_stale_detail_refreshes() {
    let (mut state, _) = super::super::tests::controller();
    state.replace_tracks(vec![super::super::tests::track(20)], true);
    state.command(AppCommand::SetGlobalVolume(20));
    state.command(AppCommand::SetIndividualVolumeEnabled(true));
    install(&mut state, 7);
    state.command(AppCommand::SetVolume(0.1));
    for item in &state.playlists.view.active.as_ref().unwrap().items[..2] {
        assert_eq!(item.volume_percent, Some(10));
    }
    assert_eq!(state.playlists.tracks[0].volume_percent, Some(10));
    assert_eq!(state.playlists.tracks[1].volume_percent, Some(10));
    state.playlist_action(PlaylistAction::SelectItem(2));
    state.command(AppCommand::SetVolume(0.3));
    install(&mut state, 8);
    assert_eq!(state.playlists.tracks[0].volume_percent, Some(30));
    assert_eq!(state.playlists.tracks[1].volume_percent, Some(30));
    state.playlist_action(PlaylistAction::SelectItem(3));
    state.command(AppCommand::SetVolume(0.9));
    assert_eq!(state.playlists.tracks[2].volume_percent, None);
    state.playlist_action(PlaylistAction::Select(None));
    assert_eq!(state.tracks[0].volume_percent, Some(30));
}

fn summary(id: i32, revision: Option<i64>) -> PlaylistSummary {
    PlaylistSummary {
        id,
        name: "Evening".into(),
        item_count: 3,
        cover_beatmap_id: Some(100),
        custom_cover_revision: revision,
    }
}

#[test]
fn tabs_keep_independent_search_and_restore_the_playlist_item_without_playing() {
    let (mut state, updates) = super::super::tests::controller();
    state.replace_tracks(
        vec![
            super::super::tests::track(20),
            super::super::tests::track(21),
        ],
        true,
    );
    state.command(AppCommand::SelectTrack(Some(21)));
    state.library_query = "library query".into();
    install(&mut state, 7);
    state.playlist_action(PlaylistAction::SelectItem(2));
    state.playlist_action(PlaylistAction::Search("playlist query".into()));
    state.playlist_action(PlaylistAction::ShowLibrary);
    assert!(!state.playlists.view.showing_detail());
    assert_eq!(state.playlists.view.active_id, Some(7));
    assert_eq!(state.playlists.view.selected_item_id, Some(2));
    assert_eq!(state.selected_volume_id(), Some(21));
    assert_eq!(state.track(20).unwrap().subtitle, "Artist");
    state.command(AppCommand::SetTrackSort(
        crate::controller::TrackSort::ArtistAsc,
    ));
    assert!(
        matches!(updates.lock().unwrap().iter().rev().find(|event| matches!(event, AppUpdate::TracksReordered(_))), Some(AppUpdate::TracksReordered(tracks)) if tracks.len() == 2)
    );
    // A late detail refresh must never replace Songs while its tab is visible.
    updates.lock().unwrap().clear();
    state.install_playlist(playlist(7));
    assert!(
        !updates
            .lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event, AppUpdate::TracksReplaced { .. }))
    );
    state.playlist_action(PlaylistAction::ShowPlaylists);
    assert!(state.playlists.view.showing_detail());
    assert_eq!(state.playlists.view.selected_item_id, Some(2));
    assert_eq!(state.library_query, "library query");
    assert_eq!(state.playlists.view.query, "playlist query");
    assert!(state.assignment.is_none() && state.playback_commands.is_empty());
    state.playlist_action(PlaylistAction::Select(None));
    assert!(state.playlists.view.visible && state.playlists.view.active_id.is_none());
}

#[test]
fn editor_retains_created_id_after_cover_failure_and_rejects_stale_picker_results() {
    let (mut state, _) = super::super::tests::controller();
    state.playlist_action(PlaylistAction::BeginCreate);
    state.playlist_action(PlaylistAction::SetEditorName(" Evening ".into()));
    let epoch = state.playlists.view.editor_epoch;
    state.playlist_action(PlaylistAction::CompleteCoverPick {
        epoch,
        png: Ok(Some(vec![1, 2])),
    });
    let epoch = state.playlists.view.editor_epoch;
    state.playlist_event(PlaylistEvent::EditorSaved {
        epoch,
        saved: Some(summary(7, None)),
        result: Err("Upload failed".into()),
    });
    assert_eq!(state.playlists.view.editor_id, Some(7));
    assert_eq!(state.playlists.view.editor_name, " Evening ");
    assert_eq!(state.playlists.view.editor_cover_png, Some(vec![1, 2]));
    assert_eq!(state.playlists.view.playlists.len(), 1);
    assert!(state.playlists.view.editor_open && !state.playlists.view.busy);
    state.playlist_action(PlaylistAction::ResetEditorCover);
    state.playlist_action(PlaylistAction::CompleteCoverPick {
        epoch,
        png: Ok(Some(vec![3])),
    });
    assert!(state.playlists.view.editor_cover_png.is_none());
    let epoch = state.playlists.view.editor_epoch;
    state.playlist_action(PlaylistAction::CompleteCoverPick {
        epoch,
        png: Ok(Some(vec![4])),
    });
    let epoch = state.playlists.view.editor_epoch;
    state.playlist_action(PlaylistAction::CompleteCoverPick {
        epoch,
        png: Ok(None),
    });
    state.playlist_action(PlaylistAction::CompleteCoverPick {
        epoch,
        png: Err("Invalid PNG".into()),
    });
    assert_eq!(state.playlists.view.editor_cover_png, Some(vec![4]));
    assert_eq!(state.playlists.view.message, "Invalid PNG");
    state.playlist_action(PlaylistAction::CancelEditor);
    state.playlist_action(PlaylistAction::BeginCreate);
    state.playlist_action(PlaylistAction::CompleteCoverPick {
        epoch,
        png: Ok(Some(vec![5])),
    });
    assert!(state.playlists.view.editor_cover_png.is_none());
    state.playlist_action(PlaylistAction::SaveEditor);
    assert_eq!(
        state.playlists.view.message,
        "Playlist name cannot be empty."
    );
}

#[test]
fn cover_deliveries_require_current_revision_or_fallback_and_can_retry_after_eviction() {
    let (mut state, updates) = super::super::tests::controller();
    state.playlists.view.playlists.push(summary(7, Some(2)));
    let current = PlaylistImage::Custom { id: 7, revision: 2 };
    state.playlist_action(PlaylistAction::RequestCover { id: 7, revision: 2 });
    state.playlist_action(PlaylistAction::RequestCover { id: 7, revision: 2 });
    state.request_playlist_image(PlaylistImage::Custom { id: 7, revision: 1 });
    assert_eq!(state.playlists.pending_images.len(), 1);
    assert!(
        updates.lock().unwrap().is_empty(),
        "Image requests never re-emit editor state"
    );
    state.playlist_event(PlaylistEvent::Image {
        request: PlaylistImage::Custom { id: 7, revision: 1 },
        result: Ok(Some(vec![1])),
    });
    assert!(updates.lock().unwrap().is_empty());
    state.playlists.pending_images.clear();
    state.playlists.images.insert(current);
    state.playlist_event(PlaylistEvent::Image {
        request: current,
        result: Ok(Some(vec![2])),
    });
    assert!(matches!(
        updates.lock().unwrap().last(),
        Some(AppUpdate::PlaylistCover {
            id: 7,
            revision: 2,
            ..
        })
    ));
    state.request_playlist_image(current);
    assert_eq!(
        state.playlists.pending_images.len(),
        1,
        "Evicted artwork may be fetched again"
    );
    updates.lock().unwrap().clear();
    let automatic = PlaylistImage::Automatic {
        id: 7,
        beatmap_id: 100,
    };
    state.playlist_action(PlaylistAction::RequestArtwork {
        id: 7,
        beatmap_id: 100,
    });
    assert!(state.playlists.pending_images.contains(&automatic));
    assert!(updates.lock().unwrap().is_empty());
    state.playlist_event(PlaylistEvent::Image {
        request: automatic,
        result: Ok(Some(vec![4])),
    });
    assert!(
        matches!(
            updates.lock().unwrap().last(),
            Some(AppUpdate::PlaylistArtwork {
                id: 7,
                beatmap_id: 100,
                ..
            })
        ),
        "Automatic artwork is available for preview before resetting a saved custom cover"
    );
    assert_eq!(
        state.playlists.view.playlists[0].custom_cover_revision,
        Some(2)
    );
    updates.lock().unwrap().clear();
    state.playlists.view.playlists[0].custom_cover_revision = None;
    state.playlist_event(PlaylistEvent::Image {
        request: current,
        result: Ok(Some(vec![3])),
    });
    state.playlist_event(PlaylistEvent::Image {
        request: PlaylistImage::Automatic {
            id: 7,
            beatmap_id: 99,
        },
        result: Ok(Some(vec![3])),
    });
    assert!(updates.lock().unwrap().is_empty());
    state.playlist_event(PlaylistEvent::Image {
        request: automatic,
        result: Ok(Some(vec![4])),
    });
    assert!(matches!(
        updates.lock().unwrap().last(),
        Some(AppUpdate::PlaylistArtwork {
            id: 7,
            beatmap_id: 100,
            ..
        })
    ));
    updates.lock().unwrap().clear();
    state.playlists.view.playlists.clear();
    state.playlist_event(PlaylistEvent::Image {
        request: current,
        result: Ok(Some(vec![5])),
    });
    assert!(updates.lock().unwrap().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn playlist_images_share_four_slots_and_deduplicate_pending_requests() {
    let (_directory, session) = super::super::tests::test_session().await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    for id in 1..=8 {
        state
            .playlists
            .view
            .playlists
            .push(summary(id, (id % 2 == 0).then_some(1)));
        let request = if id % 2 == 0 {
            PlaylistImage::Custom { id, revision: 1 }
        } else {
            PlaylistImage::Automatic {
                id,
                beatmap_id: 100,
            }
        };
        state.request_playlist_image(request);
        state.request_playlist_image(request);
    }
    state.start_playlist_images();
    assert_eq!(state.playlists.images.len(), 4);
    assert_eq!(state.playlists.pending_images.len(), 4);
    assert_eq!(state.tasks.len(), 4);
    state.start_playlist_images();
    assert_eq!(state.tasks.len(), 4);
    state.tasks.abort_all();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
#[allow(clippy::too_many_lines)] // Exercise the complete create/upload/retry sequence against HTTP.
async fn a_failed_cover_upload_retries_the_created_playlist_instead_of_creating_a_duplicate() {
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("server");
    std::fs::write(
        &binary,
        format!("#!/bin/sh\necho 'listening on http://{address}'\nexec sleep 30\n"),
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let session = crate::Session::start(crate::ServerOptions {
        binary: Some(binary),
        ..Default::default()
    })
    .await
    .unwrap();
    let server = tokio::spawn(async move {
        for (method, path, status, body) in [
            (
                "POST",
                "/api/playlists",
                "200 OK",
                serde_json::to_string(&summary(7, None)).unwrap(),
            ),
            (
                "PUT",
                "/api/playlists/7/cover",
                "500 Internal Server Error",
                r#"{"error":"Upload failed"}"#.into(),
            ),
            (
                "PATCH",
                "/api/playlists/7",
                "200 OK",
                serde_json::to_string(&summary(7, None)).unwrap(),
            ),
            (
                "PUT",
                "/api/playlists/7/cover",
                "200 OK",
                serde_json::to_string(&summary(7, Some(1))).unwrap(),
            ),
        ] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                headers.push(stream.read_u8().await.unwrap());
            }
            let headers = String::from_utf8(headers).unwrap();
            assert!(headers.starts_with(&format!("{method} {path} HTTP/1.1\r\n")));
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap_or(0);
            let mut request = vec![0; length];
            stream.read_exact(&mut request).await.unwrap();
            if method == "PUT" {
                assert_eq!(request, vec![1, 2]);
            }
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        }
    });
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    state.playlist_action(PlaylistAction::BeginCreate);
    state.playlist_action(PlaylistAction::SetEditorName("Evening".into()));
    state.playlist_action(PlaylistAction::CompleteCoverPick {
        epoch: state.playlists.view.editor_epoch,
        png: Ok(Some(vec![1, 2])),
    });
    state.playlist_action(PlaylistAction::SaveEditor);
    let completed =
        tokio::time::timeout(std::time::Duration::from_secs(3), state.tasks.join_next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    state.complete(completed);
    assert_eq!(state.playlists.view.editor_id, Some(7));
    assert!(state.playlists.view.editor_open);
    assert!(state.playlists.view.message.contains("Upload failed"));
    assert_eq!(state.playlists.view.playlists.len(), 1);
    state.playlist_action(PlaylistAction::SaveEditor);
    let completed =
        tokio::time::timeout(std::time::Duration::from_secs(3), state.tasks.join_next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    state.complete(completed);
    assert!(!state.playlists.view.editor_open);
    assert_eq!(state.playlists.view.playlists.len(), 1);
    assert_eq!(
        state.playlists.view.playlists[0].custom_cover_revision,
        Some(1)
    );
    server.await.unwrap();
    state.tasks.abort_all();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn saving_before_detail_load_finishes_reloads_it_and_preserves_cover_errors() {
    let (_directory, session) = super::super::tests::test_session().await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    state.playlists.view.playlists.push(summary(7, None));
    for failure in [false, true] {
        state.playlist_action(PlaylistAction::Select(Some(7)));
        let opening_epoch = state.playlists.detail_epoch;
        state.playlist_action(PlaylistAction::BeginEdit(7));
        state.playlist_action(PlaylistAction::SetEditorName("Updated title".into()));
        state.playlist_action(PlaylistAction::SaveEditor);
        let editor_epoch = state.playlists.view.editor_epoch;
        let saving_epoch = state.playlists.detail_epoch;
        assert_ne!(saving_epoch, opening_epoch);
        state.playlist_event(PlaylistEvent::Detail {
            epoch: opening_epoch,
            id: 7,
            result: Ok(playlist(7)),
        });
        assert!(state.playlists.view.active.is_none());
        let mut renamed = summary(7, None);
        renamed.name = "Updated title".into();
        let before = state.tasks.len();
        state.playlist_event(PlaylistEvent::EditorSaved {
            epoch: editor_epoch,
            saved: Some(renamed.clone()),
            result: if failure {
                Err("Cover upload failed".into())
            } else {
                Ok(renamed)
            },
        });
        assert_ne!(
            state.playlists.detail_epoch, saving_epoch,
            "Saving must start a fresh detail request"
        );
        assert!(state.tasks.len() > before);
        assert_eq!(state.playlists.view.editor_open, failure);
        if failure {
            assert_eq!(state.playlists.view.message, "Cover upload failed");
        }
        let mut fresh = playlist(7);
        fresh.name = "Updated title".into();
        state.playlist_event(PlaylistEvent::Detail {
            epoch: state.playlists.detail_epoch,
            id: 7,
            result: Ok(fresh),
        });
        assert_eq!(
            state.playlists.view.active.as_ref().unwrap().name,
            "Updated title"
        );
        assert_eq!(state.playlists.view.selected_item_id, Some(1));
        if failure {
            assert_eq!(state.playlists.view.message, "Cover upload failed");
        } else {
            assert_ne!(state.playlists.view.message, "Loading playlist…");
        }
    }
    state.tasks.abort_all();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[test]
fn selected_playlist_media_and_pending_detail_keep_independent_demand_and_durations() {
    let (mut state, updates) = super::super::tests::controller();
    state.replace_tracks(
        vec![
            super::super::tests::track(1),
            super::super::tests::track(20),
        ],
        true,
    );
    state
        .durations
        .insert(20, Some(std::time::Duration::from_secs(42)));
    let mut loaded = playlist(7);
    for item in &mut loaded.items {
        item.cover_beatmap_id = item.audio_source_id.map(|id| id + 100);
    }
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(7);
    state.install_playlist(loaded);
    state.emit_selection();
    assert_eq!(
        state.selected,
        Some(1),
        "library selection remains distinct"
    );
    assert_eq!(state.selected_media.as_ref().unwrap().audio_source_id, 20);
    assert_eq!(
        state.playlists.tracks[0].duration,
        Some(std::time::Duration::from_secs(42))
    );
    assert_eq!(
        state.playlists.tracks[0].duration,
        state.playlists.tracks[1].duration
    );
    assert_ne!(
        state.playlists.view.active.as_ref().unwrap().items[0].id,
        state.playlists.view.active.as_ref().unwrap().items[1].id
    );
    state.request_media(20, true);
    state.set_visible_media(Vec::new());
    assert_eq!(state.queue.front().map(|(id, _)| *id), Some(20));
    let previous = state.selected_media.clone();
    updates.lock().unwrap().clear();
    state.playlist_action(PlaylistAction::Select(Some(8)));
    assert_eq!(
        state.selected_media, previous,
        "pending detail retains displayed selection"
    );
    assert!(
        !updates
            .lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event, AppUpdate::TrackSelected(None)))
    );
    assert_eq!(
        state.queue.front().map(|(id, _)| *id),
        Some(20),
        "retained selected demand restarts after view-generation change"
    );
    let mut empty = playlist(8);
    empty.items.clear();
    state.playlist_event(PlaylistEvent::Detail {
        epoch: state.playlists.detail_epoch,
        id: 8,
        result: Ok(empty),
    });
    assert!(
        state.selected_media.is_none(),
        "resolved empty detail clears selection"
    );
    assert!(
        updates
            .lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event, AppUpdate::TrackSelected(None)))
    );
}
