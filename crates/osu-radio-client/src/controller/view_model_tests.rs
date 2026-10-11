use super::tests::{controller, track};
use super::*;
use crate::models::{PlaybackAssignment, PlaybackCommand, PlaybackMode, Playlist, PlaylistItem};
use crate::view_models::player;

fn playlist() -> Playlist {
    Playlist {
        id: 7,
        name: "Duplicates".into(),
        items: [
            (31, "Z", "Easy", Some(1)),
            (32, "A", "Hard", Some(1)),
            (33, "Missing", "Old", None),
        ]
        .into_iter()
        .map(|(id, title, difficulty, audio)| PlaylistItem {
            id,
            playlist_id: 7,
            source_kind: "stable".into(),
            beatmap_hash: id.to_string(),
            title: Some(title.into()),
            title_unicode: None,
            artist: Some("Artist".into()),
            artist_unicode: None,
            difficulty_name: Some(difficulty.into()),
            beatmap_id: audio.map(|_| id),
            beatmap_set_id: audio.map(|_| 10),
            audio_source_id: audio,
            cover_beatmap_id: None,
            last_played_at_ms: None,
            volume_percent: None,
        })
        .collect(),
    }
}
fn assignment(mode: PlaybackMode) -> PlaybackAssignment {
    PlaybackAssignment {
        current_audio_source_id: Some(1),
        current_playlist_item_id: Some(31),
        track: None,
        duration_ms: None,
        mode,
        revision: 1,
        playback_token: 5,
        can_next: true,
        can_previous: false,
        volume_percent: None,
    }
}

#[test]
fn independent_lists_keep_search_selection_and_atomic_playlist_identity() {
    let (mut state, updates) = controller();
    state.replace_tracks(vec![track(1), track(2)], true);
    state.command(AppCommand::SelectTrack(Some(2)));
    state.library.query = "library search".into();
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(7);
    state.install_playlist(playlist());
    state.playlist_action(PlaylistAction::SelectItem(31));
    state.playlist_action(PlaylistAction::Search("playlist search".into()));
    let rows = state.playlists.tracks.clone();
    updates.lock().unwrap().clear();
    state.replace_tracks(vec![track(2), track(3)], true);
    assert_eq!(state.library.selected_id(), Some(2));
    assert_eq!(state.playlists.state().selected_item_id, Some(31));
    assert_eq!(state.playlists.tracks(), rows.as_slice());
    assert!(
        updates
            .lock()
            .unwrap()
            .iter()
            .any(|event| matches!(event, AppUpdate::LibrarySelected(Some(2))))
    );
    assert!(
        updates
            .lock()
            .unwrap()
            .iter()
            .all(|event| !matches!(event, AppUpdate::PlaylistTracksReplaced { .. }))
    );
    state.playlist_action(PlaylistAction::ShowLibrary);
    assert_eq!(state.player.selected_track().unwrap().audio_source_id, 2);
    state.playlist_action(PlaylistAction::ShowPlaylists);
    assert!(
        state
            .player
            .selected_track()
            .unwrap()
            .subtitle
            .contains("Easy")
    );
    assert_eq!(state.library.query(), "library search");
    assert_eq!(state.playlists.state().query, "playlist search");
    assert!(
        state.player.commands.is_empty(),
        "selection must never start playback"
    );
    updates.lock().unwrap().clear();
    state.install_playlist(playlist());
    let tracks = updates
        .lock()
        .unwrap()
        .iter()
        .find_map(|event| match event {
            AppUpdate::PlaylistTracksReplaced { tracks, .. } => Some(tracks.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        tracks.iter().map(|row| row.item_id).collect::<Vec<_>>(),
        [32, 33, 31]
    );
    assert_eq!(tracks.first().unwrap().track.audio_source_id, 1);
    assert!(tracks.first().unwrap().track.subtitle.contains("Hard"));
    assert_eq!(tracks.get(2).unwrap().track.audio_source_id, 1);
    assert!(tracks.get(2).unwrap().track.subtitle.contains("Easy"));
}

#[test]
fn primary_action_preserves_local_library_loading_and_playlist_assignment_rules() {
    let mut player = PlayerViewModel {
        assignment: Some(assignment(PlaybackMode::Playing)),
        ..PlayerViewModel::default()
    };
    player.playback.current_audio_id = Some(1);
    player.playback.loading_audio_id = Some(1);
    let selected = track(1);
    assert!(matches!(
        player.primary_action(Some(&selected), None),
        Some(player::PrimaryAction::Playback(PlaybackCommand::Play {
            audio_source_id: None
        }))
    ));
    player.playback.snapshot.state = crate::playback::PlayerState::Playing;
    assert!(matches!(
        player.primary_action(Some(&selected), None),
        Some(player::PrimaryAction::Playback(PlaybackCommand::Pause))
    ));
    player.playback.snapshot.state = crate::playback::PlayerState::Paused;
    player.playback.error = Some("device failure".into());
    assert!(matches!(
        player.primary_action(Some(&selected), None),
        Some(player::PrimaryAction::Playback(PlaybackCommand::Play {
            audio_source_id: None
        }))
    ));
    assert!(matches!(
        player.primary_action(Some(&track(2)), None),
        Some(player::PrimaryAction::Playback(PlaybackCommand::Play {
            audio_source_id: Some(2)
        }))
    ));
    assert!(matches!(
        player.primary_action(Some(&selected), Some((7, 31))),
        Some(player::PrimaryAction::Playback(PlaybackCommand::Pause))
    ));
    assert!(matches!(
        player.primary_action(Some(&selected), Some((7, 32))),
        Some(player::PrimaryAction::Playlist(
            queue::PlaylistPlayback::Play(7, Some(32))
        ))
    ));
    assert!(
        player
            .primary_action(Some(&track(-1)), Some((7, 33)))
            .is_none()
    );
}

#[test]
fn position_ticks_only_update_player_and_duplicate_audio_does_not_enable_seek() {
    let (mut state, updates) = controller();
    state.replace_tracks(vec![track(1)], true);
    let mut playback = crate::playback::Playback {
        current_audio_id: Some(1),
        has_source: true,
        ..Default::default()
    };
    playback.snapshot.state = crate::playback::PlayerState::Playing;
    playback.snapshot.duration = Some(Duration::from_secs(60));
    updates.lock().unwrap().clear();
    for seconds in 0..20 {
        playback.snapshot.position = Duration::from_secs(seconds);
        state.worker_update(playback.clone());
    }
    let events = updates.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AppUpdate::Player(_)))
            .count(),
        20
    );
    assert!(
        events
            .iter()
            .all(|event| matches!(event, AppUpdate::Player(_) | AppUpdate::Playback(_)))
    );
    drop(events);
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(7);
    state.install_playlist(playlist());
    state.playlist_action(PlaylistAction::SelectItem(32));
    state.player.assignment = Some(assignment(PlaybackMode::Playing));
    playback.playback_token = 5;
    state.worker_update(playback);
    let presentation = state.player.presentation(Some(32), true, true, true);
    assert!(!presentation.can_seek);
    assert!(!presentation.selected_is_playing);
    assert_eq!(presentation.position, Duration::ZERO);
}

#[test]
fn offline_settings_keep_session_choices_and_errors_in_their_own_groups() {
    let (mut state, updates) = controller();
    state.command(AppCommand::ReportSettingsError {
        group: SettingsGroup::Discord,
        message: "Discord write failed".into(),
    });
    let preferences = TrackNamePreferences {
        use_unicode_titles: true,
        use_unicode_artists: false,
    };
    state.command(AppCommand::SetTrackNamePreferences(preferences));
    assert_eq!(state.settings.preferences().name_preferences, preferences);
    assert_eq!(
        state.settings.preferences().discord_error,
        "Discord write failed"
    );
    state.command(AppCommand::ReportSettingsError {
        group: SettingsGroup::Unicode,
        message: "Unicode write failed".into(),
    });
    state.command(AppCommand::SetDiscordRichPresence(false));
    assert_eq!(
        state.settings.preferences().unicode_error,
        "Unicode write failed"
    );
    assert!(state.settings.preferences().discord_error.is_empty());
    assert!(state.session.is_none());
    assert!(updates.lock().unwrap().iter().any(|event|matches!(event,AppUpdate::SettingsPreferences(value) if value.name_preferences==preferences)));
}

#[cfg(unix)]
#[tokio::test]
async fn toggle_command_waits_for_audio_settings_and_uses_the_selected_item_identity() {
    let (mut state, _) = controller();
    state.replace_tracks(vec![track(1), track(2)], true);
    state.command(AppCommand::SelectTrack(Some(2)));
    state.command(AppCommand::TogglePlayback);
    assert!(state.player.commands.is_empty());
    let (_directory, session) = tests::test_session().await;
    state.session = Some(session);
    state.settings.volume.settings = None;
    state.command(AppCommand::TogglePlayback);
    assert!(state.player.commands.is_empty());
    state.settings.volume.settings = Some(crate::models::AudioSettings::default());
    state.command(AppCommand::TogglePlayback);
    let pending = state.player.commands.pop_front().unwrap();
    assert_eq!(
        pending.command,
        PlaybackCommand::Play {
            audio_source_id: Some(2)
        }
    );
    state.player.playback.current_audio_id = Some(2);
    state.player.playback.loading_audio_id = Some(2);
    state.command(AppCommand::TogglePlayback);
    assert_eq!(
        state.player.commands.pop_front().unwrap().command,
        PlaybackCommand::Play {
            audio_source_id: None
        }
    );
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(7);
    state.install_playlist(playlist());
    state.playlist_action(PlaylistAction::SelectItem(32));
    state.player.assignment = Some(assignment(PlaybackMode::Playing));
    state.command(AppCommand::TogglePlayback);
    assert!(matches!(
        state.player.commands.pop_front().unwrap().playlist,
        Some(queue::PlaylistPlayback::Play(7, Some(32)))
    ));
    state.playlist_action(PlaylistAction::SelectItem(33));
    state.command(AppCommand::TogglePlayback);
    assert!(state.player.commands.is_empty());
    state.tasks.abort_all();
    state.session.take().unwrap().shutdown().await.unwrap();
}

#[test]
fn worker_start_failure_updates_player_state_and_presentation() {
    let (mut state, updates) = controller();
    state.replace_tracks(vec![track(1)], true);
    state.player.assignment = Some(assignment(PlaybackMode::Playing));
    state.player.transport_error = Some("older transport failure".into());
    updates.lock().unwrap().clear();
    state.worker_start_failed("cannot create playback worker".into());
    assert_eq!(
        state.player.playback().error.as_deref(),
        Some("cannot create playback worker")
    );
    assert!(state.player.transport_error.is_none());
    let events = updates.lock().unwrap().clone();
    assert!(events.iter().any(|event| matches!(event, AppUpdate::Playback(value) if value.error.as_deref()==Some("cannot create playback worker"))));
    assert!(events.iter().any(|event| matches!(event, AppUpdate::Player(value) if value.message=="cannot create playback worker" && !value.can_seek)));
    updates.lock().unwrap().clear();
    state.emit_player();
    assert!(updates.lock().unwrap().iter().any(|event| matches!(event, AppUpdate::Player(value) if value.message=="cannot create playback worker")));
    state.player.device_failed(5);
    let pending = state.player.commands.pop_front().unwrap();
    assert_eq!(
        pending.command,
        PlaybackCommand::PauseIfCurrent { playback_token: 5 }
    );
    assert_eq!(pending.expected_token, Some(5));
}
