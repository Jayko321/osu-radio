use super::tests::{controller, queue_row, track};
use super::*;
use crate::models::{
    AudioModsProfile, PlaybackAssignment, PlaybackCommand, PlaybackMode, Playlist, PlaylistItem,
};
use crate::playback::PlaybackParameters;
use crate::view_models::player;

#[test]
fn media_and_controller_resolve_duplicate_audio_from_the_same_state_owner() {
    fn assert_title(state: &Controller, expected: Option<&str>) {
        assert_eq!(state.track(1).map(|track| track.title.as_str()), expected);
        let input = media::MediaInput {
            library: &state.library,
            playlists: &state.playlists,
            player: &state.player,
            upcoming: Some(&state.upcoming),
        };
        assert_eq!(input.track(1).map(|track| track.title.as_str()), expected);
        assert!(state.track(999).is_none() && input.track(999).is_none());
    }
    let (mut state, _) = controller();
    let mut library = track(1);
    library.title = "Library".into();
    library.title_original = Some("Library".into());
    state.replace_tracks(vec![library], true);
    state.playlists.view.visible = true;
    state.playlists.view.active_id = Some(7);
    state.install_playlist(playlist());
    state.playlist_action(PlaylistAction::SelectItem(31));
    assert_title(&state, Some("Z"));
    state.playlist_action(PlaylistAction::SelectItem(32));
    assert_title(&state, Some("A"));
    state.playlists.view.visible = false;
    let mut current = track(1);
    current.title = "Playing".into();
    state.player.current_track = Some(current);
    assert_title(&state, Some("Playing"));
    state.player.current_track = None;
    assert_title(&state, Some("Library"));
    let mut queued = track(1);
    queued.title = "Queued".into();
    state.upcoming.view.entries = vec![queue_row(queued, 1)];
    state.library.indices.insert(1, usize::MAX);
    assert_title(&state, Some("Queued"));
    state.upcoming.view.entries.clear();
    let mut selected = track(1);
    selected.title = "Selected".into();
    state.player.selected_track = Some(selected);
    assert_title(&state, Some("Selected"));
    state.player.selected_track = None;
    assert_title(&state, None);
}

#[test]
fn duration_effect_updates_duplicate_rows_without_changing_selection_or_playback() {
    for open in [false, true] {
        let (mut state, updates) = controller();
        state.replace_tracks(vec![track(1), track(2)], true);
        state.command(AppCommand::SelectTrack(Some(2)));
        state.playlists.view.visible = true;
        state.playlists.view.active_id = Some(7);
        state.install_playlist(playlist());
        state.playlist_action(PlaylistAction::SelectItem(31));
        state.player.current_track = Some(track(1));
        state.upcoming.open = open;
        state.upcoming.view.entries = vec![
            queue_row(track(1), 1),
            queue_row(track(2), 2),
            queue_row(track(1), 3),
        ];
        let playback = state.player.playback.clone();
        for duration in [Some(Duration::from_secs(42)), None] {
            updates.lock().unwrap().clear();
            state.apply_effects(vec![workflow::Effect::DurationChanged {
                audio_id: 1,
                duration,
            }]);
            assert_eq!(state.library.tracks.first().unwrap().duration, duration);
            assert_eq!(state.library.tracks.get(1).unwrap().duration, None);
            assert!(
                state
                    .playlists
                    .tracks
                    .iter()
                    .filter(|row| row.track.audio_source_id == 1)
                    .all(|row| row.track.duration == duration)
            );
            assert!(
                state
                    .upcoming
                    .view
                    .entries
                    .iter()
                    .filter(|row| row.audio_source_id == Some(1))
                    .all(|row| row.track.duration == duration)
            );
            assert_eq!(
                state.player.selected_track.as_ref().unwrap().duration,
                duration
            );
            assert_eq!(
                state.player.current_track.as_ref().unwrap().duration,
                duration
            );
            assert_eq!(state.library.selected, Some(2));
            assert_eq!(state.playlists.view.selected_item_id, Some(31));
            assert_eq!(state.player.playback, playback);
            assert!(state.player.commands.is_empty() && state.playback.is_none());
            let events = updates.lock().unwrap().clone();
            assert_eq!(events.iter().filter(|event| matches!(event, AppUpdate::LibraryTrackChanged(track) if track.audio_source_id == 1 && track.duration == duration)).count(), 1);
            let playlist_items: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    AppUpdate::PlaylistTrackChanged(row) => Some(row.item_id),
                    _ => None,
                })
                .collect();
            assert_eq!(playlist_items, [32, 31]);
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event, AppUpdate::Queue(_)))
                    .count(),
                usize::from(open)
            );
            assert!(events.iter().all(|event| !matches!(
                event,
                AppUpdate::LibraryTracksReplaced { .. }
                    | AppUpdate::PlaylistTracksReplaced { .. }
                    | AppUpdate::Playback(_)
            )));
        }
    }
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn audio_download_guards_preserve_current_work_and_seek_reaches_the_worker() {
    async fn message(
        receiver: &mut mpsc::UnboundedReceiver<crate::playback::Message>,
    ) -> crate::playback::Message {
        tokio::time::timeout(Duration::from_secs(5), receiver.recv())
            .await
            .unwrap()
            .unwrap()
    }
    async fn snapshot(
        receiver: &mut mpsc::UnboundedReceiver<crate::playback::Playback>,
        matches: impl Fn(&crate::playback::Playback) -> bool + Send + Sync,
    ) -> crate::playback::Playback {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let state = receiver.recv().await.unwrap();
                if matches(&state) {
                    return state;
                }
            }
        })
        .await
        .unwrap()
    }
    let (mut state, _) = controller();
    state.replace_tracks(vec![track(1)], true);
    state.download(crate::playback::Download {
        generation: 0,
        id: 1,
    });
    assert!(state.audio_task.is_none() && state.tasks.is_empty());
    let (sender, mut snapshots) = mpsc::unbounded_channel();
    let (worker, mut feedback) = crate::playback::Worker::spawn_fake(Arc::new(move |state| {
        let _ = sender.send(state);
    }))
    .unwrap();
    let first = worker.invalidate();
    worker.send(crate::playback::Command::Assign {
        entry_id: None,
        waiting_for_check: false,
        generation: first,
        id: Some(1),
        playback_token: 5,
        mode: PlaybackMode::Playing,
        volume: 0.4,
        parameters: PlaybackParameters::default(),
    });
    assert_eq!(
        message(&mut feedback).await,
        crate::playback::Message::Download(crate::playback::Download {
            generation: first,
            id: 1
        })
    );
    state.playback = Some(worker);
    state.download(crate::playback::Download {
        generation: 0,
        id: 1,
    });
    assert!(state.audio_task.is_none() && state.tasks.is_empty());
    state.download(crate::playback::Download {
        generation: first,
        id: 1,
    });
    assert_eq!(
        message(&mut feedback).await,
        crate::playback::Message::Failed(5)
    );
    let failed = snapshot(&mut snapshots, |state| state.error.is_some()).await;
    assert_eq!(
        failed.error.as_deref(),
        Some("Connect to the server before playing a track.")
    );
    assert!(!failed.has_source && failed.loading_audio_id.is_none());
    let worker = state.playback.as_ref().unwrap();
    let current = worker.invalidate();
    worker.send(crate::playback::Command::Assign {
        entry_id: None,
        waiting_for_check: false,
        generation: current,
        id: Some(1),
        playback_token: 6,
        mode: PlaybackMode::Playing,
        volume: 0.4,
        parameters: PlaybackParameters::default(),
    });
    assert_eq!(
        message(&mut feedback).await,
        crate::playback::Message::Download(crate::playback::Download {
            generation: current,
            id: 1
        })
    );
    let task = state.task(std::future::pending());
    state.audio_task = Some(task.clone());
    let expired_file = tempfile::NamedTempFile::new().unwrap();
    let expired_path = expired_file.path().to_owned();
    state.complete(Completed::Audio {
        generation: first,
        id: 1,
        result: Ok(expired_file),
    });
    assert!(!expired_path.exists());
    assert_eq!(state.audio_task.as_ref().unwrap().id(), task.id());
    assert!(!task.is_finished());
    let current_file = tempfile::NamedTempFile::new().unwrap();
    let current_path = current_file.path().to_owned();
    std::fs::write(&current_path, b"mp3").unwrap();
    state.complete(Completed::Audio {
        generation: current,
        id: 1,
        result: Ok(current_file),
    });
    assert!(state.audio_task.is_none());
    assert_eq!(
        message(&mut feedback).await,
        crate::playback::Message::Started(6)
    );
    let playing = snapshot(&mut snapshots, |state| state.has_source).await;
    assert_eq!(playing.current_audio_id, Some(1));
    assert!(playing.error.is_none() && current_path.exists());
    let mut assigned = assignment(PlaybackMode::Playing);
    assigned.playback_token = 6;
    state.player.assignment = Some(assigned);
    state.command(AppCommand::Seek(Duration::from_secs(3)));
    let sought = snapshot(&mut snapshots, |state| state.seek_serial == 1).await;
    assert_eq!(sought.snapshot.position, Duration::from_secs(3));
    assert_eq!(sought.playback_token, 6);
    state.tasks.abort_all();
    while state.tasks.join_next().await.is_some() {}
    state.playback.take().unwrap().shutdown();
    assert!(!current_path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn audio_gate_wait_is_cancelled_and_closed_gate_returns_without_http() {
    let (_directory, session) = super::tests::test_session().await;
    let (mut state, updates) = controller();
    state.session = Some(session);
    let (worker, _) = crate::playback::Worker::spawn_fake(Arc::new(|_| {})).unwrap();
    state.playback = Some(worker);
    state.audio_gate = Arc::new(tokio::sync::Semaphore::new(0));
    state.download(crate::playback::Download {
        generation: 0,
        id: 1,
    });
    let blocked = state.audio_task.as_ref().unwrap().clone();
    tokio::task::yield_now().await;
    assert!(!blocked.is_finished());
    assert_eq!(state.audio_gate.available_permits(), 0);
    state.cancel_audio();
    assert!(state.audio_task.is_none());
    assert!(
        matches!(state.tasks.join_next().await.unwrap(), Err(ref error) if error.is_cancelled())
    );
    assert!(blocked.is_finished());
    let current = state.playback.as_ref().unwrap().generation();
    assert_eq!(current, 1);
    state.audio_gate.close();
    state.download(crate::playback::Download {
        generation: current,
        id: 1,
    });
    let completed = state.tasks.join_next().await.unwrap().unwrap();
    assert!(matches!(completed, Completed::Cancelled));
    state.complete(completed);
    assert!(updates.lock().unwrap().is_empty());
    assert!(state.tasks.is_empty());
    state.playback.take().unwrap().shutdown();
    state.session.take().unwrap().shutdown().await.unwrap();
}

pub(super) fn playlist() -> Playlist {
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
            mods: None,
        })
        .collect(),
    }
}
pub(super) fn assignment(mode: PlaybackMode) -> PlaybackAssignment {
    PlaybackAssignment {
        current_entry_id: None,
        availability: None,
        waiting_for_check: false,
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
        mods: None,
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
    let presentation =
        state
            .player
            .presentation(Some(32), true, true, true, AudioModsProfile::default());
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

#[test]
fn player_presentation_gates_play_and_seek_without_borrowing_another_items_position() {
    let mut player = PlayerViewModel::default();
    let mut selected = track(1);
    selected.duration = Some(Duration::from_secs(42));
    player.select_track(Some(selected.clone()));
    player.playback.current_audio_id = Some(1);
    player.playback.current_playlist_item_id = Some(31);
    player.playback.has_source = true;
    player.playback.snapshot.state = crate::playback::PlayerState::Playing;
    player.playback.snapshot.position = Duration::from_secs(7);
    player.playback.snapshot.duration = Some(Duration::from_secs(60));
    player.playback.can_next = true;
    for connected in [false, true] {
        for loaded in [false, true] {
            for available in [false, true] {
                let view = player.presentation(
                    Some(31),
                    available,
                    connected,
                    loaded,
                    AudioModsProfile::default(),
                );
                assert_eq!(view.selected_track, Some(selected.clone()));
                assert_eq!(view.can_play, connected && loaded && available);
                assert_eq!(
                    view.can_seek, available,
                    "local seek survives HTTP disconnection"
                );
                assert_eq!(view.selected_is_playing, available);
                assert_eq!(
                    view.position,
                    if available {
                        Duration::from_secs(7)
                    } else {
                        Duration::ZERO
                    }
                );
                assert_eq!(view.duration, available.then_some(Duration::from_secs(60)));
                assert!(view.can_next && !view.can_previous);
            }
        }
    }
    let other_item = player.presentation(Some(32), true, true, true, AudioModsProfile::default());
    assert!(!other_item.can_seek && !other_item.selected_is_playing);
    assert_eq!(
        (other_item.position, other_item.duration),
        (Duration::ZERO, None)
    );
    player.playback.snapshot.duration = None;
    assert_eq!(
        player
            .presentation(None, true, true, true, AudioModsProfile::default())
            .duration,
        Some(Duration::from_secs(42))
    );
    player.playback.snapshot.duration = Some(Duration::ZERO);
    assert!(
        !player
            .presentation(None, true, true, true, AudioModsProfile::default())
            .can_seek
    );
    player.playback.snapshot.duration = Some(Duration::from_secs(60));
    player.playback.has_source = false;
    assert!(
        !player
            .presentation(None, true, true, true, AudioModsProfile::default())
            .can_seek
    );
    player.playback.loading_audio_id = Some(1);
    assert_eq!(
        player
            .presentation(None, true, true, true, AudioModsProfile::default())
            .message,
        "Loading audio…"
    );
    player.playback.error = Some("decoder failed".into());
    assert_eq!(
        player
            .presentation(None, true, true, true, AudioModsProfile::default())
            .message,
        "decoder failed"
    );
    player.select_track(Some(track(-1)));
    assert!(
        !player
            .presentation(None, true, true, true, AudioModsProfile::default())
            .can_play
    );
    player.select_track(None);
    assert!(
        !player
            .presentation(None, true, true, true, AudioModsProfile::default())
            .can_play
    );
    assert!(player.selected_track().is_none());
}

#[test]
fn seek_effect_keeps_the_launch_token_and_rejects_a_different_duplicate_audio_item() {
    for (detail, selected_item, assigned, expected) in [
        (false, None, true, Some((1, 5))),
        (false, None, false, Some((1, 0))),
        (true, Some(31), true, Some((1, 5))),
        (true, Some(32), true, None),
        (true, Some(33), true, None),
        (true, None, true, None),
        (true, Some(31), false, None),
    ] {
        let (mut state, _) = controller();
        state.replace_tracks(vec![track(1)], true);
        if detail {
            state.playlists.view.visible = true;
            state.playlists.view.active_id = Some(7);
            state.install_playlist(playlist());
            state.playlists.view.selected_item_id = selected_item;
        }
        state.player.assignment = assigned.then(|| assignment(PlaybackMode::Playing));
        let input = player::PlayerInput {
            library: &state.library,
            playlists: &state.playlists,
            settings: &state.settings,
            media: &state.media,
            assignment_sender: &state.assignment_sender,
        };
        let mut work = workflow::Workflow {
            api: None,
            tasks: &mut state.tasks,
            cancel: &state.cancel,
            effects: Vec::new(),
        };
        state
            .player
            .seek_selected(Duration::from_secs(17), &input, &mut work);
        if let Some((id, token)) = expected {
            assert!(
                matches!(work.effects.as_slice(), [workflow::Effect::PlayerCommand(crate::playback::Command::Seek {
                expected_id: Some(actual), playback_token, position,
            })] if *actual == id && *playback_token == token && *position == Duration::from_secs(17))
            );
        } else {
            assert!(work.effects.is_empty());
        }
        assert!(state.playback.is_none());
    }
}

#[test]
fn offline_transport_commands_report_errors_without_queuing_or_opening_a_worker() {
    let (mut state, updates) = controller();
    state.replace_tracks(vec![track(1)], true);
    for command in [
        AppCommand::PlayTrack(1),
        AppCommand::Pause,
        AppCommand::Resume,
        AppCommand::Stop,
        AppCommand::Next,
        AppCommand::Previous,
    ] {
        updates.lock().unwrap().clear();
        state.command(command);
        assert!(
            state.player.commands.is_empty() && state.tasks.is_empty() && state.playback.is_none()
        );
        assert_eq!(state.library.selected_id(), Some(1));
        assert!(
            updates
                .lock()
                .unwrap()
                .iter()
                .any(|event| matches!(event, AppUpdate::Player(view)
            if view.message == "Connect to the server before playing a track." && !view.can_play))
        );
    }
    for action in [
        PlaylistAction::Play {
            id: 7,
            start_item_id: Some(31),
        },
        PlaylistAction::AddToQueue(7),
    ] {
        updates.lock().unwrap().clear();
        let append = matches!(action, PlaylistAction::AddToQueue(_));
        state.playlist_action(action);
        assert!(state.player.commands.is_empty() && state.playback.is_none());
        if append {
            assert_eq!(
                state.upcoming.view.operation_error.as_deref(),
                Some("Connect to the server to queue a playlist.")
            );
            assert!(!state.upcoming.view.busy);
            continue;
        }
        assert!(
            updates
                .lock()
                .unwrap()
                .iter()
                .any(|event| matches!(event, AppUpdate::Player(view)
            if view.message == "Connect to the server to queue a playlist."))
        );
    }
}

#[test]
fn one_gate_holds_the_last_assignment_until_volume_and_mods_are_loaded() {
    for mods_first in [false, true] {
        let (mut state, _) = controller();
        state.settings.volume.settings = None;
        state.settings.mods.loaded = false;
        state.settings.mods.loading = true;
        state.settings.mods.load_request = 1;
        let mut last = assignment(PlaybackMode::Paused);
        last.revision = 2;
        state.apply_assignment(assignment(PlaybackMode::Paused));
        state.apply_assignment(last);
        let load_volume = |state: &mut Controller| {
            state.audio_settings_loaded(Ok(crate::models::AudioSettings::default()));
        };
        let load_mods = |state: &mut Controller| {
            state.mods_loaded_result(1, 0, Ok(crate::models::AudioModsSettings::default()));
        };
        assert!(state.player.assignment.is_none());
        if mods_first {
            load_mods(&mut state);
        } else {
            load_volume(&mut state);
        }
        assert!(
            state.player.assignment.is_none(),
            "one load is not admission"
        );
        if mods_first {
            load_volume(&mut state);
        } else {
            load_mods(&mut state);
        }
        assert_eq!(
            state.player.assignment.as_ref().map(|a| a.revision),
            Some(2)
        );
    }
    // A mods load error keeps the gate shut; the later successful Retry result opens it.
    let (mut state, _) = controller();
    state.settings.mods.loaded = false;
    state.settings.mods.loading = true;
    state.settings.mods.load_request = 1;
    state.apply_assignment(assignment(PlaybackMode::Paused));
    assert!(state.player.assignment.is_none());
    state.mods_loaded_result(1, 0, Err("offline".into()));
    assert!(state.player.assignment.is_none() && !state.settings.mods_loaded());
    state.settings.mods.loading = true;
    state.settings.mods.load_request = 2;
    state.mods_loaded_result(2, 0, Ok(crate::models::AudioModsSettings::default()));
    assert!(state.player.assignment.is_some());
}

fn mods(
    mode: crate::models::AudioModsMode,
    speed_percent: u16,
    adjust_pitch: bool,
) -> crate::models::AudioModsProfile {
    crate::models::AudioModsProfile {
        mode,
        speed_percent,
        adjust_pitch,
    }
}

#[tokio::test]
async fn mode_switch_resends_the_current_launchs_own_profile_without_restarting_it() {
    use crate::models::AudioModsMode::{Dt, Ht};
    let (mut state, _) = controller();
    state.replace_tracks(vec![track(1), track(2)], true);
    let mut sounding = assignment(PlaybackMode::Playing);
    sounding.mods = Some(mods(Dt, 120, true));
    state.player.assignment = Some(sounding);
    // B's stored profile differs, but it is the selection and not the sounding launch.
    state
        .library
        .tracks
        .iter_mut()
        .find(|t| t.audio_source_id == 2)
        .unwrap()
        .mods = Some(mods(Ht, 75, false));
    state.command(AppCommand::SelectTrack(Some(2)));
    let (sender, mut snapshots) = mpsc::unbounded_channel();
    let (worker, _downloads) = crate::playback::Worker::spawn_fake(Arc::new(move |state| {
        let _ = sender.send(state);
    }))
    .unwrap();
    state.playback = Some(worker);
    let pair = |speed, adjust_pitch| PlaybackParameters {
        speed,
        adjust_pitch,
    };
    // Global mode (default none) -> individual: A takes its own stored profile; back again.
    for (individual, expected) in [(true, pair(1.2, true)), (false, pair(1.0, false))] {
        state.mods_action(crate::view_models::ModsAction::SetIndividual(individual));
        assert_eq!(state.assignment_parameters(), expected);
        let seen = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let playback: crate::playback::Playback = snapshots.recv().await.unwrap();
                if playback.snapshot.parameters == expected {
                    return playback;
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(seen.playback_token, 5, "same launch, no restart");
    }
}

#[test]
fn presentation_time_follows_actual_speed_and_the_selected_tracks_own_profile() {
    let mut player = PlayerViewModel::default();
    let mut selected = track(1);
    selected.duration = Some(Duration::from_secs(240));
    player.select_track(Some(selected.clone()));
    player.playback.current_audio_id = Some(1);
    player.playback.has_source = true;
    player.playback.snapshot.state = crate::playback::PlayerState::Playing;
    player.playback.snapshot.position = Duration::from_secs(90);
    player.playback.snapshot.duration = Some(Duration::from_secs(240));
    player.playback.snapshot.parameters.speed = 1.5;
    let view = player.presentation(None, true, true, true, AudioModsProfile::default());
    assert_eq!(
        (
            view.position,
            view.duration,
            view.selected_duration,
            view.speed_percent
        ),
        (
            Duration::from_secs(60),
            Some(Duration::from_secs(160)),
            Some(Duration::from_secs(160)),
            150
        )
    );
    // The source place is kept across a speed change; only its presentation moves.
    for (speed, position, duration) in [(1.0, 90, 240), (1.2, 75, 200), (0.75, 120, 320)] {
        player.playback.snapshot.parameters.speed = speed;
        let view = player.presentation(None, true, true, true, AudioModsProfile::default());
        assert_eq!(
            (view.position, view.duration),
            (
                Duration::from_secs(position),
                Some(Duration::from_secs(duration))
            )
        );
    }
    // Selected B with another speed: its own profile scales its duration; unknown stays unknown.
    player.playback.current_audio_id = Some(9);
    let view = player.presentation(
        None,
        true,
        true,
        true,
        mods(crate::models::AudioModsMode::Dt, 150, false),
    );
    assert_eq!(
        (
            view.position,
            view.duration,
            view.selected_duration,
            view.speed_percent
        ),
        (Duration::ZERO, None, Some(Duration::from_secs(160)), 150)
    );
    selected.duration = None;
    player.select_track(Some(selected));
    let view = player.presentation(
        None,
        true,
        true,
        true,
        mods(crate::models::AudioModsMode::Dt, 150, false),
    );
    assert_eq!(view.selected_duration, None);
}

#[test]
fn ui_seek_converts_once_while_system_seek_stays_in_source_time() {
    // UI 1:00 at 1.5x is source 1:30, converted exactly once.
    let (mut state, _) = controller();
    state.replace_tracks(vec![track(1)], true);
    let mut assigned = assignment(PlaybackMode::Playing);
    assigned.current_entry_id = crate::models::QueueEntryId::new(1).ok();
    state.player.assignment = Some(assigned);
    state.player.playback.snapshot.parameters.speed = 1.5;
    let input = player::PlayerInput {
        library: &state.library,
        playlists: &state.playlists,
        settings: &state.settings,
        media: &state.media,
        assignment_sender: &state.assignment_sender,
    };
    let mut work = workflow::Workflow {
        api: None,
        tasks: &mut state.tasks,
        cancel: &state.cancel,
        effects: Vec::new(),
    };
    state
        .player
        .seek_selected(Duration::from_secs(60), &input, &mut work);
    assert!(matches!(
        work.effects.as_slice(),
        [workflow::Effect::PlayerCommand(crate::playback::Command::Seek { position, .. })]
            if *position == Duration::from_secs(90)
    ));
    // An external seek already carries source time and bypasses the UI conversion.
    work.effects.clear();
    let launch = state.player.system_launch().unwrap();
    let snapshot = osu_radio_media_controls::Snapshot {
        launch: Some(launch),
        duration: Some(Duration::from_secs(240)),
        rate: 1.5,
        capabilities: osu_radio_media_controls::Capabilities {
            seek: true,
            ..Default::default()
        },
        ..Default::default()
    };
    state.player.system_event(
        osu_radio_media_controls::Event::SeekAbsolute {
            launch,
            position: Duration::from_secs(60),
        },
        &snapshot,
        &input,
        &mut work,
    );
    assert!(matches!(
        work.effects.as_slice(),
        [workflow::Effect::PlayerCommand(crate::playback::Command::Seek { position, .. })]
            if *position == Duration::from_secs(60)
    ));
}
