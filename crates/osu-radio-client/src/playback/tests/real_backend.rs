//! Real HTTP, imports, file checks and restart; only the local audio engine is fake.
#![allow(clippy::indexing_slicing, clippy::arithmetic_side_effects)]
use super::*;
use crate::{
    ApiClient, ServerOptions, Session,
    models::{
        AudioModsMode, AudioModsPatch, AudioModsProfile, PlaybackAssignment, PlaybackCommand,
        QueueAvailability, QueueItemRef, QueuePlacement, QueueState,
    },
    view_models::mods::Editor,
};

async fn settled(api: &ApiClient) -> QueueState {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let queue = api.queue().await.unwrap();
            if queue
                .entries
                .iter()
                .all(|e| e.availability != QueueAvailability::Checking)
            {
                return queue;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

async fn drive(core: &mut Core<FakeEngine>, api: &ApiClient, assignment: &PlaybackAssignment) {
    drive_with(core, api, assignment, PlaybackParameters::default()).await;
}

async fn drive_with(
    core: &mut Core<FakeEngine>,
    api: &ApiClient,
    assignment: &PlaybackAssignment,
    parameters: PlaybackParameters,
) {
    let generation = assignment.playback_token;
    let download = core.command(
        Command::Assign {
            generation,
            id: assignment.current_audio_source_id,
            entry_id: assignment.current_entry_id,
            waiting_for_check: assignment.waiting_for_check,
            playback_token: assignment.playback_token,
            mode: assignment.mode,
            volume: f32::from(assignment.volume_percent.unwrap_or(100)) / 100.0,
            parameters,
        },
        generation,
    );
    if let Some(download) = download {
        core.command(
            Command::Loaded {
                generation,
                id: download.id,
                result: Ok(api.download_audio(download.id).await.unwrap()),
            },
            generation,
        );
    }
}

async fn confirm(core: &mut Core<FakeEngine>, api: &ApiClient) -> PlaybackAssignment {
    let Some(Message::Started(playback_token)) = core.feedback.pop_front() else {
        panic!("only a successful engine start may confirm listening")
    };
    assert!(core.feedback.is_empty());
    api.playback_command(&PlaybackCommand::Started { playback_token })
        .await
        .unwrap()
}

fn seek(core: &mut Core<FakeEngine>, assignment: &PlaybackAssignment) {
    core.command(
        Command::Seek {
            expected_id: assignment.current_audio_source_id,
            playback_token: assignment.playback_token,
            position: Duration::from_secs(17),
        },
        assignment.playback_token,
    );
}

fn prepared() -> (tempfile::TempDir, ServerOptions) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let server = root
        .join("target/debug/osu-radio-server")
        .canonicalize()
        .unwrap();
    let fixture = root.join("apps/osu-radio-qt/tests/fixtures/queue_backend.py");
    let directory = tempfile::tempdir().unwrap();
    let output = std::process::Command::new("python3")
        .arg(fixture)
        .arg("prepare")
        .arg(server)
        .current_dir(directory.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.path().join("library.sqlite").is_file());
    assert!(!directory.path().join(".env").exists());
    let options = ServerOptions {
        binary: Some(directory.path().join("fixture-server")),
        working_directory: Some(directory.path().to_owned()),
        ..Default::default()
    };
    (directory, options)
}

#[tokio::test]
#[ignore = "needs a built SQLite osu-radio-server and python3; uses a private directory"]
#[allow(clippy::too_many_lines)] // One scenario follows real entry identity across every layer and restart.
async fn real_server_queue_and_fake_engine_preserve_launches_across_edits_and_reimport() {
    let (directory, options) = prepared();
    let session = Session::start(options.clone()).await.unwrap();
    let api = session.api();
    let original = api.queue().await.unwrap();
    let ids = original.entries.iter().map(|e| e.id).collect::<Vec<_>>();
    assert!(ids.iter().all(|id| id.get() > 9_007_199_254_740_992));
    let mut core = Core::<FakeEngine>::default();
    drive(&mut core, api, &api.playback().await.unwrap()).await;
    assert!(core.player.is_none() && core.feedback.is_empty());

    let playing = api.play_queue_entry(ids[0]).await.unwrap();
    drive(&mut core, api, &playing).await;
    let started = confirm(&mut core, api).await;
    assert!(started.track.as_ref().unwrap().last_played_at_ms.is_some());
    seek(&mut core, &started);
    let moved = api.move_queue_entry(ids[0], Some(ids[2])).await.unwrap();
    drive(&mut core, api, &moved).await;
    assert_eq!(moved.playback_token, playing.playback_token);
    let engine = core.player.as_ref().unwrap();
    assert_eq!(
        (engine.loads, engine.seeks, engine.play_volumes.len()),
        (1, 1, 1)
    );
    assert_eq!(engine.snapshot.position, Duration::from_secs(17));
    assert!(core.feedback.is_empty());
    let previous = api
        .playback_command(&PlaybackCommand::Previous)
        .await
        .unwrap();
    assert_eq!(previous.current_entry_id, Some(ids[1]));
    assert_eq!(
        previous.current_audio_source_id,
        playing.current_audio_source_id
    );
    assert_ne!(previous.playback_token, playing.playback_token);
    drive(&mut core, api, &previous).await;
    assert_eq!(core.state.snapshot.position, Duration::ZERO);
    assert_eq!(core.player.as_ref().unwrap().loads, 1);
    confirm(&mut core, api).await;
    let next = api.playback_command(&PlaybackCommand::Next).await.unwrap();
    assert_eq!(next.current_entry_id, Some(ids[0]));
    drive(&mut core, api, &next).await;
    confirm(&mut core, api).await;
    assert_eq!(
        api.queue().await.unwrap().entries.len(),
        4,
        "past Play/transport makes no copy"
    );

    let b = api.search_tracks("B").await.unwrap()[0].audio_source_id;
    let c = api.search_tracks("C").await.unwrap()[0].audio_source_id;
    let selected = api
        .playback_command(&PlaybackCommand::Play {
            audio_source_id: Some(b),
        })
        .await
        .unwrap();
    assert_ne!(
        selected.current_entry_id,
        Some(ids[2]),
        "library Play inserts its own occurrence"
    );
    assert!(selected.waiting_for_check);
    drive(&mut core, api, &selected).await;
    assert!(core.player.is_none() && core.feedback.is_empty());
    settled(api).await;
    let released = api.playback().await.unwrap();
    drive(&mut core, api, &released).await;
    confirm(&mut core, api).await;
    seek(&mut core, &released);
    let inserted = api
        .insert_queue(
            &[QueueItemRef::Audio { audio_source_id: c }],
            QueuePlacement::Next,
        )
        .await
        .unwrap();
    drive(&mut core, api, &inserted).await;
    assert_eq!(inserted.playback_token, released.playback_token);
    assert_eq!(core.state.snapshot.position, Duration::from_secs(17));
    assert!(core.feedback.is_empty());
    let queue = settled(api).await;
    let position = queue.history_len;
    assert_eq!(queue.entries[position + 1].audio_source_id, Some(c));
    api.playback_command(&PlaybackCommand::Pause).await.unwrap();
    let removed = api
        .remove_queue_entry(selected.current_entry_id.unwrap())
        .await
        .unwrap();
    drive(&mut core, api, &removed).await;
    assert_eq!(removed.mode, PlaybackMode::Paused);
    assert_eq!(removed.current_audio_source_id, Some(c));
    assert!(core.feedback.is_empty());

    let prefix = api.queue().await.unwrap();
    api.play_playlist(1, None).await.unwrap();
    let queue = settled(api).await;
    let retained = prefix.history_len + 1;
    assert_eq!(&queue.entries[..retained], &prefix.entries[..retained]);
    assert_eq!(queue.entries.len(), retained + 4);
    assert_eq!(
        queue.entries.last().unwrap().availability,
        QueueAvailability::Unavailable
    );
    let assigned = api.playback().await.unwrap();
    drive(&mut core, api, &assigned).await;
    confirm(&mut core, api).await;
    seek(&mut core, &assigned);
    let trimmed = api.clear_upcoming_queue().await.unwrap();
    drive(&mut core, api, &trimmed).await;
    assert_eq!(core.state.snapshot.position, Duration::from_secs(17));
    assert_eq!(trimmed.playback_token, assigned.playback_token);
    let library = api.search_tracks("").await.unwrap();
    let playlists = api.playlists().await.unwrap();
    let empty = api.clear_queue().await.unwrap();
    drive(&mut core, api, &empty).await;
    assert_eq!(empty.mode, PlaybackMode::Stopped);
    assert_eq!(api.search_tracks("").await.unwrap(), library);
    assert_eq!(api.playlists().await.unwrap(), playlists);

    // Hold the successful engine's Started until after a real HTTP reimport.
    api.append_playlist_to_queue(1).await.unwrap();
    let queue = settled(api).await;
    let entry = queue.entries[1].id;
    let assigned = api.play_queue_entry(entry).await.unwrap();
    drive(&mut core, api, &assigned).await;
    seek(&mut core, &assigned);
    let old_source = assigned.current_audio_source_id;
    let snapshot_path = directory.path().join("snapshot.ndjson");
    let snapshot = std::fs::read_to_string(&snapshot_path).unwrap();
    std::fs::write(&snapshot_path, snapshot.replace("A.wav", "rebound.wav")).unwrap();
    let folder = api.osu_folders().await.unwrap()[0].id;
    api.reimport_osu_folder(folder).await.unwrap();
    let rebound = api.playback().await.unwrap();
    assert_eq!(rebound.current_audio_source_id, old_source);
    assert_eq!(rebound.playback_token, assigned.playback_token);
    assert_eq!(rebound.volume_percent, Some(29));
    drive(&mut core, api, &rebound).await;
    assert_eq!(core.state.snapshot.position, Duration::from_secs(17));
    let queue = settled(api).await;
    let row = queue.entries.iter().find(|e| e.id == entry).unwrap();
    assert_ne!(row.audio_source_id, old_source);
    assert_eq!(row.last_played_at_ms, None);
    let late = confirm(&mut core, api).await;
    assert!(late.track.unwrap().last_played_at_ms.is_some());
    assert_eq!(
        api.queue().await.unwrap().entries[1].last_played_at_ms,
        None
    );
    api.playback_command(&PlaybackCommand::Failed {
        playback_token: assigned.playback_token,
    })
    .await
    .unwrap();
    assert_eq!(
        api.queue().await.unwrap().entries[1].availability,
        QueueAvailability::Available
    );
    let new_launch = api.play_queue_entry(entry).await.unwrap();
    assert_eq!(new_launch.current_audio_source_id, row.audio_source_id);
    assert_ne!(new_launch.playback_token, assigned.playback_token);
    drive(&mut core, api, &new_launch).await;
    confirm(&mut core, api).await;

    // Removing the installation and original playlist cannot remove queue snapshots.
    api.playback_command(&PlaybackCommand::Pause).await.unwrap();
    let before = api.queue().await.unwrap();
    api.remove_osu_folder(folder).await.unwrap();
    api.delete_playlist(1).await.unwrap();
    assert!(
        api.queue()
            .await
            .unwrap()
            .entries
            .iter()
            .all(|e| e.availability == QueueAvailability::Unavailable)
    );
    let marker = directory.path().join("source/client.realm");
    api.import_osu_folder(marker.to_str().unwrap())
        .await
        .unwrap();
    let restored = settled(api).await;
    assert_eq!(
        restored
            .entries
            .iter()
            .map(|e| (e.id, &e.title))
            .collect::<Vec<_>>(),
        before
            .entries
            .iter()
            .map(|e| (e.id, &e.title))
            .collect::<Vec<_>>()
    );
    assert_eq!(restored.current_entry_id, before.current_entry_id);
    assert_eq!(restored.mode, PlaybackMode::Paused);
    let source = restored.entries[1].audio_source_id;
    let token = restored.playback_token;
    session.shutdown().await.unwrap();
    let session = Session::start(options).await.unwrap();
    let api = session.api();
    let recovered = api.queue().await.unwrap();
    assert_eq!(recovered.entries, restored.entries);
    assert_eq!(recovered.current_entry_id, restored.current_entry_id);
    assert_eq!(recovered.history_len, restored.history_len);
    assert_eq!(recovered.mode, PlaybackMode::Paused);
    assert_ne!(recovered.playback_token, token);
    let mut recovered_core = Core::<FakeEngine>::default();
    drive(&mut recovered_core, api, &api.playback().await.unwrap()).await;
    assert!(recovered_core.player.is_none() && recovered_core.feedback.is_empty());
    let stale = api
        .playback_command(&PlaybackCommand::Started {
            playback_token: token,
        })
        .await
        .unwrap();
    assert_eq!(stale.playback_token, recovered.playback_token);
    let resumed = api
        .playback_command(&PlaybackCommand::Play {
            audio_source_id: None,
        })
        .await
        .unwrap();
    assert_eq!(resumed.current_audio_source_id, source);
    drive(&mut recovered_core, api, &resumed).await;
    confirm(&mut recovered_core, api).await;

    // Exhaustion keeps history; append C -> B stays silent until explicit Play.
    api.clear_upcoming_queue().await.unwrap();
    let current = api.playback().await.unwrap();
    let ended = api
        .playback_command(&PlaybackCommand::Finished {
            playback_token: current.playback_token,
        })
        .await
        .unwrap();
    assert_eq!(ended.mode, PlaybackMode::Stopped);
    let tracks = api.search_tracks("").await.unwrap();
    let c = tracks
        .iter()
        .find(|t| t.title.as_deref() == Some("C"))
        .unwrap()
        .audio_source_id;
    let b = tracks
        .iter()
        .find(|t| t.title.as_deref() == Some("B"))
        .unwrap()
        .audio_source_id;
    api.append_queue(&[c, b]).await.unwrap();
    let queue = settled(api).await;
    assert_eq!(queue.mode, PlaybackMode::Stopped);
    let first = api
        .playback_command(&PlaybackCommand::Play {
            audio_source_id: None,
        })
        .await
        .unwrap();
    assert_eq!(first.current_audio_source_id, Some(c));
    // A wholly unavailable playlist remains stopped even after import/checks restore it.
    api.clear_queue().await.unwrap();
    let folder = api.osu_folders().await.unwrap()[0].id;
    api.remove_osu_folder(folder).await.unwrap();
    let unavailable = api.play_playlist(2, None).await.unwrap();
    assert_eq!(unavailable.mode, PlaybackMode::Stopped);
    assert!(unavailable.current_entry_id.is_none());
    api.import_osu_folder(marker.to_str().unwrap())
        .await
        .unwrap();
    let restored = settled(api).await;
    assert_eq!(
        restored.entries[0].availability,
        QueueAvailability::Available
    );
    assert_eq!(restored.mode, PlaybackMode::Stopped);
    assert!(restored.current_entry_id.is_none());
    drive(&mut recovered_core, api, &api.playback().await.unwrap()).await;
    assert!(recovered_core.player.is_none() && recovered_core.feedback.is_empty());
    session.shutdown().await.unwrap();
    assert!(
        !directory.path().join("data").exists(),
        "explicit fixture URL must bypass XDG fallback"
    );
}

fn profile(mode: AudioModsMode, speed_percent: u16, adjust_pitch: bool) -> AudioModsProfile {
    AudioModsProfile {
        mode,
        speed_percent,
        adjust_pitch,
    }
}

async fn audio_of(api: &ApiClient, title: &str) -> i32 {
    let tracks = api.search_tracks("").await.unwrap();
    tracks
        .iter()
        .find(|t| t.title.as_deref() == Some(title))
        .unwrap()
        .audio_source_id
}

/// The client's own rule: individual mode never inherits the global set.
async fn effective(api: &ApiClient, assignment: &PlaybackAssignment) -> PlaybackParameters {
    let mut editor = Editor::default();
    editor.settings = api.audio_mods().await.unwrap();
    editor
        .effective(assignment.current_audio_source_id, assignment.mods)
        .into()
}

#[tokio::test]
#[ignore = "needs a built SQLite osu-radio-server and python3; uses a private directory"]
#[allow(clippy::too_many_lines)] // One scenario follows each profile across storage, server, worker and restart.
async fn real_server_mods_follow_each_launch_across_restart_reimport_and_live_edits() {
    let nc_120 = profile(AudioModsMode::Dt, 120, true);
    let dt_150 = profile(AudioModsMode::Dt, 150, false);
    let ht_75 = profile(AudioModsMode::Ht, 75, false);
    let ht_60 = profile(AudioModsMode::Ht, 60, false);
    let pair = |speed, adjust_pitch| PlaybackParameters {
        speed,
        adjust_pitch,
    };
    let (directory, options) = prepared();
    let marker = directory.path().join("source/client.realm");
    let session = Session::start(options.clone()).await.unwrap();
    let api = session.api();

    // Fixture: individual mode, global DT 1.30x, A = NC 1.20x, B and C without a profile.
    let (a, b, c) = (
        audio_of(api, "A").await,
        audio_of(api, "B").await,
        audio_of(api, "C").await,
    );
    let settings = api.audio_mods().await.unwrap();
    assert!(settings.individual_mods_enabled);
    assert_eq!(
        settings.global_profile,
        profile(AudioModsMode::Dt, 130, false)
    );
    let mods_of = |tracks: &[crate::models::LibraryTrack], id| {
        tracks
            .iter()
            .find(|t| t.audio_source_id == id)
            .unwrap()
            .mods
    };
    let tracks = api.search_tracks("").await.unwrap();
    assert_eq!(
        (
            mods_of(&tracks, a),
            mods_of(&tracks, b),
            mods_of(&tracks, c)
        ),
        (Some(nc_120), None, None)
    );

    // Toggling the mode erases neither set.
    let global_only = api
        .update_audio_mods(&AudioModsPatch {
            individual_mods_enabled: Some(false),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(global_only.global_profile, settings.global_profile);
    assert_eq!(
        mods_of(&api.search_tracks("").await.unwrap(), a),
        Some(nc_120)
    );
    api.update_audio_mods(&AudioModsPatch {
        individual_mods_enabled: Some(true),
        ..Default::default()
    })
    .await
    .unwrap();
    api.set_audio_mods(b, Some(ht_75)).await.unwrap();
    assert_eq!(
        mods_of(&api.search_tracks("").await.unwrap(), b),
        Some(ht_75)
    );

    // A (shared by two difficulties) starts from the queue with its own pair before the first sample.
    let ids = api
        .queue()
        .await
        .unwrap()
        .entries
        .iter()
        .map(|e| e.id)
        .collect::<Vec<_>>();
    let mut core = Core::<FakeEngine>::default();
    let playing = api.play_queue_entry(ids[1]).await.unwrap();
    assert_eq!(playing.mods, Some(nc_120));
    assert_eq!(effective(api, &playing).await, pair(1.2, true));
    drive_with(&mut core, api, &playing, effective(api, &playing).await).await;
    let started = confirm(&mut core, api).await;
    seek(&mut core, &playing);
    let engine = core.player.as_ref().unwrap();
    assert_eq!(engine.play_parameters, [pair(1.2, true)]);
    assert_eq!(engine.snapshot.parameters, pair(1.2, true));

    // Editing B while A sounds changes neither the launch nor the queue token/revision.
    let queue_before = api.queue().await.unwrap();
    api.set_audio_mods(b, Some(ht_60)).await.unwrap();
    let queue_after = api.queue().await.unwrap();
    assert_eq!(
        (queue_after.revision, queue_after.playback_token),
        (queue_before.revision, queue_before.playback_token)
    );
    let unchanged = api.playback().await.unwrap();
    assert_eq!(unchanged, started);
    drive_with(&mut core, api, &unchanged, effective(api, &unchanged).await).await;
    let engine = core.player.as_ref().unwrap();
    assert_eq!((engine.loads, engine.play_parameters.len()), (1, 1));
    assert_eq!(engine.snapshot.position, Duration::from_secs(17));
    assert!(core.feedback.is_empty());

    // Live edits of A keep the token, source, position and pause state.
    api.set_audio_mods(a, Some(dt_150)).await.unwrap();
    let live = api.playback().await.unwrap();
    assert_eq!(live.playback_token, playing.playback_token);
    assert_eq!(live.mods, Some(dt_150));
    drive_with(&mut core, api, &live, effective(api, &live).await).await;
    let engine = core.player.as_ref().unwrap();
    assert_eq!(engine.snapshot.parameters, pair(1.5, false));
    assert_eq!(engine.snapshot.position, Duration::from_secs(17));
    assert_eq!((engine.loads, engine.play_parameters.len()), (1, 1));
    api.playback_command(&PlaybackCommand::Pause).await.unwrap();
    api.set_audio_mods(a, Some(profile(AudioModsMode::Dt, 150, true)))
        .await
        .unwrap();
    let paused = api.playback().await.unwrap();
    assert_eq!(paused.mode, PlaybackMode::Paused);
    drive_with(&mut core, api, &paused, effective(api, &paused).await).await;
    let engine = core.player.as_ref().unwrap();
    assert_eq!(engine.snapshot.state, PlayerState::Paused);
    assert_eq!(engine.snapshot.parameters, pair(1.5, true));
    assert_eq!(engine.snapshot.position, Duration::from_secs(17));
    let resumed = api
        .playback_command(&PlaybackCommand::Play {
            audio_source_id: None,
        })
        .await
        .unwrap();
    assert_eq!(resumed.playback_token, playing.playback_token);
    drive_with(&mut core, api, &resumed, effective(api, &resumed).await).await;
    core.command(
        Command::Seek {
            expected_id: resumed.current_audio_source_id,
            playback_token: resumed.playback_token,
            position: Duration::from_secs(23),
        },
        resumed.playback_token,
    );
    let engine = core.player.as_ref().unwrap();
    assert_eq!(engine.snapshot.state, PlayerState::Playing);
    assert_eq!(engine.snapshot.parameters, pair(1.5, true));
    assert_eq!(engine.snapshot.position, Duration::from_secs(23));
    assert_eq!(engine.loads, 1);
    assert!(core.feedback.is_empty());

    // Restart: settings and both profile sets come back from the database.
    session.shutdown().await.unwrap();
    let session = Session::start(options).await.unwrap();
    let api = session.api();
    let restored = api.audio_mods().await.unwrap();
    assert!(restored.individual_mods_enabled);
    assert_eq!(restored.global_profile, settings.global_profile);
    let tracks = api.search_tracks("").await.unwrap();
    assert_eq!(
        (
            mods_of(&tracks, a),
            mods_of(&tracks, b),
            mods_of(&tracks, c)
        ),
        (
            Some(profile(AudioModsMode::Dt, 150, true)),
            Some(ht_60),
            None
        )
    );

    // Removing and importing the same source identity restores the profiles.
    let folder = api.osu_folders().await.unwrap()[0].id;
    api.remove_osu_folder(folder).await.unwrap();
    assert!(api.search_tracks("").await.unwrap().is_empty());
    api.import_osu_folder(marker.to_str().unwrap())
        .await
        .unwrap();
    let queue = settled(api).await;
    let (a, b, c) = (
        audio_of(api, "A").await,
        audio_of(api, "B").await,
        audio_of(api, "C").await,
    );
    let tracks = api.search_tracks("").await.unwrap();
    assert_eq!(
        (
            mods_of(&tracks, a),
            mods_of(&tracks, b),
            mods_of(&tracks, c)
        ),
        (
            Some(profile(AudioModsMode::Dt, 150, true)),
            Some(ht_60),
            None
        )
    );
    assert_eq!(
        queue
            .entries
            .iter()
            .filter(|e| e.audio_source_id == Some(a))
            .map(|e| e.mods)
            .collect::<Vec<_>>(),
        [Some(profile(AudioModsMode::Dt, 150, true)); 2],
        "both difficulties of the shared audio use one profile"
    );

    // Explicit Play of B starts with B's pair; C and A then start with their own, not the previous one.
    let mut core = Core::<FakeEngine>::default();
    let selected = api
        .playback_command(&PlaybackCommand::Play {
            audio_source_id: Some(b),
        })
        .await
        .unwrap();
    assert_eq!(selected.mods, Some(ht_60));
    drive(&mut core, api, &selected).await;
    settled(api).await;
    let released = api.playback().await.unwrap();
    drive_with(&mut core, api, &released, effective(api, &released).await).await;
    confirm(&mut core, api).await;
    assert_eq!(
        core.player.as_ref().unwrap().play_parameters,
        [pair(0.6, false)]
    );
    api.insert_queue(
        &[
            QueueItemRef::Audio { audio_source_id: c },
            QueueItemRef::Audio { audio_source_id: a },
        ],
        QueuePlacement::Next,
    )
    .await
    .unwrap();
    settled(api).await;
    let token = api.playback().await.unwrap().playback_token;
    for (audio, expected_mods, expected) in [
        (c, None, pair(1.0, false)),
        (
            a,
            Some(profile(AudioModsMode::Dt, 150, true)),
            pair(1.5, true),
        ),
    ] {
        let next = api
            .playback_command(&PlaybackCommand::Finished {
                playback_token: api.playback().await.unwrap().playback_token,
            })
            .await
            .unwrap();
        assert_eq!(next.current_audio_source_id, Some(audio));
        assert_ne!(next.playback_token, token);
        assert_eq!(next.mods, expected_mods);
        drive_with(&mut core, api, &next, effective(api, &next).await).await;
        confirm(&mut core, api).await;
        assert_eq!(core.player.as_ref().unwrap().play_parameters, [expected]);
    }
    session.shutdown().await.unwrap();
}
