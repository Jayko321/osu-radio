#![allow(clippy::indexing_slicing)]
use super::*;
use crate::controller::AppCommand;
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
    state.playlists.view.active_id = Some(id);
    state.install_playlist(playlist(id));
}

#[test]
fn separate_difficulties_selection_and_stale_responses_preserve_the_current_view() {
    let (mut state, updates) = super::super::tests::controller();
    state.replace_tracks(vec![super::super::tests::track(20)]);
    install(&mut state, 7);
    assert_eq!(state.playlists.view.tracks().len(), 3);
    assert_ne!(
        state.playlists.tracks[0].subtitle,
        state.playlists.tracks[1].subtitle
    );
    assert!(state.playlists.tracks[2].subtitle.contains("Недоступно"));
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
        matches!(updates.lock().unwrap().iter().rev().find(|event| matches!(event, AppUpdate::TracksReplaced(_))), Some(AppUpdate::TracksReplaced(tracks)) if tracks.len() == 1)
    );
}

#[test]
fn playlist_duration_notifications_work_outside_the_library_search_results() {
    let (mut state, updates) = super::super::tests::controller();
    state.replace_tracks(Vec::new());
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

#[cfg(unix)]
#[tokio::test]
async fn playback_uses_item_identity_and_mutations_ignore_a_different_open_playlist() {
    let (_directory, session) = super::super::tests::test_session().await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    install(&mut state, 7);
    state.assignment = Some(PlaybackAssignment {
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
    assert_eq!(
        state.playback_commands.pop_front().unwrap().playlist,
        Some((7, Some(2)))
    );
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
    state.replace_tracks(vec![track.clone()]);
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
    state.replace_tracks(vec![super::super::tests::track(999)]);
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
