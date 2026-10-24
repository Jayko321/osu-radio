use super::*;
use crate::controller::{
    AppCommand, Controller,
    tests::{controller, track},
};
use std::sync::{Arc, Mutex};

type Updates = Arc<Mutex<Vec<AppUpdate>>>;
fn latest(updates: &Updates) -> AudioModsState {
    updates
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find_map(|u| {
            if let AppUpdate::AudioMods(state) = u {
                Some(state.clone())
            } else {
                None
            }
        })
        .unwrap()
}
fn edit(state: &mut Controller, updates: &Updates, edit: ModsEdit) {
    let target = latest(updates).target.unwrap();
    state.command(AppCommand::Mods(ModsAction::Edit { target, edit }));
}
fn current(state: &Controller, id: i32, stored: Option<AudioModsProfile>) -> AudioModsProfile {
    state.settings.effective_mods(Some(id), stored)
}
fn acknowledge(state: &mut Controller, result: Result<(), String>) {
    state.settings.mods.saving = state.settings.mods.pending.pop_front();
    state.mods_saved(result);
}

#[test]
fn profiles_survive_modes_and_selected_b_does_not_change_assignment_a() {
    let (mut state, updates) = controller();
    state.replace_tracks(vec![track(1), track(2)], true);
    edit(&mut state, &updates, ModsEdit::CycleDt);
    edit(&mut state, &updates, ModsEdit::SetSpeed(130));
    let global = latest(&updates).selected_profile;
    assert_eq!(global.speed_percent, 130);
    state.mods_action(ModsAction::SetIndividual(true));
    assert_eq!(
        latest(&updates).selected_profile,
        AudioModsProfile::default()
    );
    assert_eq!(current(&state, 2, None), AudioModsProfile::default());
    edit(&mut state, &updates, ModsEdit::CycleDt);
    edit(&mut state, &updates, ModsEdit::SetSpeed(120));
    edit(&mut state, &updates, ModsEdit::CycleDt);
    let nc = latest(&updates).selected_profile;
    assert!(nc.adjust_pitch);
    assert_eq!(nc.speed_percent, 120);
    let assignment: PlaybackAssignment = serde_json::from_value(serde_json::json!({
        "current_audio_source_id": 1, "mode": "paused", "revision": 1, "waiting_for_check": false,
        "playback_token": 1, "can_next": false, "can_previous": false, "mods": null
    }))
    .unwrap();
    state.player.assignment = Some(assignment.clone());
    state.command(AppCommand::SelectTrack(Some(2)));
    edit(&mut state, &updates, ModsEdit::ToggleHt);
    assert_eq!(latest(&updates).selected_profile.speed_percent, 75);
    assert_eq!(
        state.settings.assignment_mods(&assignment),
        nc,
        "B must not change A"
    );
    assert_eq!(state.player.assignment.as_ref(), Some(&assignment));
    assert!(state.playback.is_none(), "editing must not start playback");
    assert_eq!(
        current(&state, 1, None),
        nc,
        "shared audio ID shares mods across difficulties/playlists"
    );
    state.mods_action(ModsAction::SetIndividual(false));
    assert_eq!(state.settings.assignment_mods(&assignment), global);
    state.mods_action(ModsAction::SetIndividual(true));
    assert_eq!(state.settings.assignment_mods(&assignment), nc);
    assert_eq!(latest(&updates).selected_profile.speed_percent, 75);
    assert_eq!(
        state.settings.volume.settings,
        Some(crate::models::AudioSettings::default())
    );
    // A stale library projection after a local DELETE cannot resurrect its stored HT.
    let mut b = track(2);
    b.mods = Some(latest(&updates).selected_profile);
    edit(&mut state, &updates, ModsEdit::Clear);
    state.replace_tracks(vec![track(1), b], true);
    assert_eq!(current(&state, 2, Some(nc)), AudioModsProfile::default());
    assert!(!latest(&updates).selected_has_override);
}

#[test]
fn keys_bounds_and_editor_epochs_cover_mouse_and_keyboard_rules() {
    let (mut state, updates) = controller();
    state.replace_tracks(vec![track(1), track(2)], true);
    for speed in [0, 50, 100, 150, 200] {
        edit(&mut state, &updates, ModsEdit::SetSpeed(speed));
        assert_eq!(
            latest(&updates).selected_profile,
            AudioModsProfile::default()
        );
    }
    edit(&mut state, &updates, ModsEdit::SetAdjustPitch(true));
    assert_eq!(
        latest(&updates).selected_profile,
        AudioModsProfile::default()
    );
    edit(&mut state, &updates, ModsEdit::CycleDt);
    for speed in [101, 120, 200] {
        edit(&mut state, &updates, ModsEdit::SetSpeed(speed));
        assert_eq!(latest(&updates).selected_profile.speed_percent, speed);
    }
    for speed in [0, 99, 100, 201, u16::MAX] {
        edit(&mut state, &updates, ModsEdit::SetSpeed(speed));
        assert_eq!(latest(&updates).selected_profile.speed_percent, 200);
    }
    edit(&mut state, &updates, ModsEdit::CycleDt);
    assert!(latest(&updates).selected_profile.adjust_pitch);
    assert_eq!(latest(&updates).selected_profile.speed_percent, 200);
    edit(&mut state, &updates, ModsEdit::CycleDt);
    assert_eq!(
        latest(&updates).selected_profile,
        AudioModsProfile::default()
    );
    edit(&mut state, &updates, ModsEdit::ToggleHt);
    assert_eq!(latest(&updates).selected_profile.speed_percent, 75);
    for speed in [50, 51, 99] {
        edit(&mut state, &updates, ModsEdit::SetSpeed(speed));
        assert_eq!(latest(&updates).selected_profile.speed_percent, speed);
    }
    for speed in [0, 49, 100, 150, u16::MAX] {
        edit(&mut state, &updates, ModsEdit::SetSpeed(speed));
        assert_eq!(latest(&updates).selected_profile.speed_percent, 99);
    }
    edit(&mut state, &updates, ModsEdit::SetAdjustPitch(true));
    edit(&mut state, &updates, ModsEdit::ToggleHt);
    assert_eq!(
        latest(&updates).selected_profile,
        AudioModsProfile::default()
    );
    state.mods_action(ModsAction::SetIndividual(true));
    let target = latest(&updates).target.unwrap();
    state.command(AppCommand::SelectTrack(Some(2)));
    state.command(AppCommand::SelectTrack(Some(1)));
    state.mods_action(ModsAction::Edit {
        target,
        edit: ModsEdit::CycleDt,
    });
    assert_eq!(
        latest(&updates).selected_profile,
        AudioModsProfile::default()
    );
    let target = latest(&updates).target.unwrap();
    state.mods_action(ModsAction::SetIndividual(false));
    state.mods_action(ModsAction::SetIndividual(true));
    state.mods_action(ModsAction::Edit {
        target,
        edit: ModsEdit::ToggleHt,
    });
    assert_eq!(
        latest(&updates).selected_profile,
        AudioModsProfile::default()
    );
    let target = latest(&updates).target.unwrap();
    state.mods_action(ModsAction::Finish(target));
    state.mods_action(ModsAction::Edit {
        target,
        edit: ModsEdit::CycleDt,
    });
    assert_eq!(
        latest(&updates).selected_profile,
        AudioModsProfile::default()
    );
    state.command(AppCommand::SelectTrack(None));
    assert!(latest(&updates).target.is_none());
}

#[tokio::test(start_paused = true)]
async fn debounce_ack_delete_retry_and_independent_targets() {
    let (mut state, updates) = controller();
    state.replace_tracks(vec![track(1), track(2)], true);
    edit(&mut state, &updates, ModsEdit::CycleDt);
    let deadline = state.settings.mods_deadline().unwrap();
    tokio::time::advance(Duration::from_millis(100)).await;
    edit(&mut state, &updates, ModsEdit::SetSpeed(120));
    assert_eq!(state.settings.mods.pending.len(), 1);
    assert_eq!(
        state.settings.mods_deadline(),
        Some(deadline + Duration::from_millis(100))
    );
    state.settings.mods.saving = state.settings.mods.pending.pop_front();
    edit(&mut state, &updates, ModsEdit::SetSpeed(140));
    state.mods_saved(Ok(()));
    assert_eq!(latest(&updates).selected_profile.speed_percent, 140);
    state.settings.mods.saving = state.settings.mods.pending.pop_front();
    edit(&mut state, &updates, ModsEdit::SetSpeed(160));
    state.mods_saved(Err("superseded failure".into()));
    assert!(latest(&updates).error.is_empty());
    acknowledge(&mut state, Err("offline".into()));
    assert!(state.settings.mods_deadline().is_none());
    assert!(latest(&updates).error.contains("offline"));
    state.mods_action(ModsAction::Retry);
    assert_eq!(state.settings.mods_deadline(), Some(Instant::now()));
    assert_eq!(latest(&updates).selected_profile.speed_percent, 160);
    state.mods_action(ModsAction::SetIndividual(true));
    edit(&mut state, &updates, ModsEdit::CycleDt);
    // Simulate a PUT in flight, followed by a local DELETE.
    let a = state.settings.mods.pending.pop_back().unwrap();
    assert!(matches!(a.change, Change::Track(1, Some(_))));
    state.settings.mods.saving = Some(a);
    edit(&mut state, &updates, ModsEdit::Clear);
    state.mods_saved(Ok(()));
    assert_eq!(
        latest(&updates).selected_profile,
        AudioModsProfile::default()
    );
    assert!(matches!(
        state.settings.mods.pending.back().unwrap().change,
        Change::Track(1, None)
    ));
    // A DELETE ACK also cannot undo the next local activation.
    state.settings.mods.saving = state.settings.mods.pending.pop_back();
    edit(&mut state, &updates, ModsEdit::ToggleHt);
    state.mods_saved(Ok(()));
    assert_eq!(latest(&updates).selected_profile.speed_percent, 75);
    edit(&mut state, &updates, ModsEdit::Clear);
    state.command(AppCommand::SelectTrack(Some(2)));
    edit(&mut state, &updates, ModsEdit::ToggleHt);
    assert_eq!(
        state.settings.mods.pending.len(),
        3,
        "settings and independent A/B targets"
    );
    assert!(state.settings.volume.pending.is_empty());
    let target = latest(&updates).target.unwrap();
    state.mods_action(ModsAction::Finish(target));
    assert!(
        state
            .settings
            .mods
            .pending
            .iter()
            .all(|p| p.deadline == Instant::now())
    );
}

#[test]
fn load_epochs_and_revisions_keep_newer_edits_and_errors_visible() {
    let (mut state, updates) = controller();
    state.replace_tracks(vec![track(1)], true);
    state.settings.mods.loaded = false;
    state.settings.mods.loading = true;
    state.settings.mods.load_request = 2;
    state.complete(Completed::AudioMods {
        request: 1,
        revision: 0,
        result: Ok(AudioModsSettings::default()),
    });
    assert!(state.settings.mods.loading && !state.settings.mods_loaded());
    state.complete(Completed::AudioMods {
        request: 2,
        revision: 0,
        result: Err("load offline".into()),
    });
    assert!(!state.settings.mods_loaded());
    assert!(latest(&updates).target.is_none());
    assert!(latest(&updates).error.contains("load offline"));
    state.settings.mods.loading = true;
    state.settings.mods.load_request = 3;
    state.complete(Completed::AudioMods {
        request: 3,
        revision: 0,
        result: Ok(AudioModsSettings::default()),
    });
    assert!(state.settings.mods_loaded());
    edit(&mut state, &updates, ModsEdit::CycleDt);
    // A late duplicate load/ACK is never a new authoritative state.
    state.complete(Completed::AudioMods {
        request: 3,
        revision: 0,
        result: Ok(AudioModsSettings::default()),
    });
    assert_eq!(latest(&updates).selected_profile.speed_percent, 150);
    state.settings.mods.loading = true;
    state.settings.mods.load_request = 4;
    state.complete(Completed::AudioMods {
        request: 4,
        revision: 0,
        result: Ok(AudioModsSettings::default()),
    });
    assert_eq!(latest(&updates).selected_profile.speed_percent, 150);
}

// Reuse the existing volume fixture pattern: private HTTP server + a disposable child session.
#[cfg(unix)]
#[tokio::test]
async fn shutdown_awaits_inflight_and_pending_api_writes_and_bounds_final_failures() {
    for fail in [false, true] {
        shutdown_case(fail).await;
    }
}
#[cfg(unix)]
#[allow(clippy::too_many_lines)] // One ordered fixture covers in-flight shutdown and each final failure.
async fn shutdown_case(fail: bool) {
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (entered, received) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        let mut entered = Some(entered);
        let mut released = Some(released);
        for index in 0..3 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                header.push(stream.read_u8().await.unwrap());
            }
            let header = String::from_utf8(header).unwrap();
            let length = header
                .lines()
                .find_map(|l| l.strip_prefix("content-length: "))
                .map_or(0, |l| l.parse::<usize>().unwrap());
            let mut body = vec![0; length];
            stream.read_exact(&mut body).await.unwrap();
            requests.push((header.lines().next().unwrap().to_owned(), body));
            if index == 0 {
                entered.take().unwrap().send(()).unwrap();
                released.take().unwrap().await.unwrap();
            }
            let payload = if fail {
                r#"{"error":"fixture save failed"}"#
            } else if header.starts_with("PATCH") {
                r#"{"individual_mods_enabled":true,"global_profile":{"mode":"dt","speed_percent":120,"adjust_pitch":false}}"#
            } else {
                ""
            };
            let status = if fail {
                "500 Internal Server Error"
            } else {
                "200 OK"
            };
            stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len()).as_bytes()).await.unwrap();
        }
        requests
    });
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("server");
    std::fs::write(
        &binary,
        format!("#!/bin/sh\necho 'listening on http://{address}'\nexec sleep 30\n"),
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let (mut state, updates) = controller();
    state.session = Some(
        crate::Session::start(crate::ServerOptions {
            binary: Some(binary),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    state.replace_tracks(vec![track(1), track(2)], true);
    edit(&mut state, &updates, ModsEdit::CycleDt);
    edit(&mut state, &updates, ModsEdit::SetSpeed(120));
    state.mods_action(ModsAction::SetIndividual(true));
    edit(&mut state, &updates, ModsEdit::CycleDt);
    // Coalesce A to DELETE; B retains its own HT write.
    edit(&mut state, &updates, ModsEdit::Clear);
    state.command(AppCommand::SelectTrack(Some(2)));
    edit(&mut state, &updates, ModsEdit::ToggleHt);
    state.start_mods_save();
    tokio::time::timeout(Duration::from_secs(2), received)
        .await
        .unwrap()
        .unwrap();
    assert!(
        state.settings.mods.save_task.is_some(),
        "first write already in flight"
    );
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(3), state.flush_mods_saves())
        .await
        .unwrap();
    assert!(state.settings.mods.pending.is_empty() && state.settings.mods.save_task.is_none());
    let requests = server.await.unwrap();
    assert!(
        requests
            .first()
            .unwrap()
            .0
            .starts_with("PATCH /api/user-data/audio-mods ")
    );
    let body: serde_json::Value = serde_json::from_slice(&requests.first().unwrap().1).unwrap();
    assert_eq!(body.get("global_speed_percent").unwrap(), 120);
    assert_eq!(body.get("individual_mods_enabled").unwrap(), true);
    assert!(
        requests
            .get(1)
            .unwrap()
            .0
            .starts_with("DELETE /api/audio-sources/1/mods ")
    );
    assert!(
        requests
            .get(2)
            .unwrap()
            .0
            .starts_with("PUT /api/audio-sources/2/mods ")
    );
    let b: AudioModsProfile = serde_json::from_slice(&requests.get(2).unwrap().1).unwrap();
    assert_eq!(b.speed_percent, 75);
    assert_eq!(current(&state, 1, None), AudioModsProfile::default());
    assert_eq!(current(&state, 2, None), b);
    if fail {
        assert!(latest(&updates).error.contains("fixture save failed"));
        assert!(state.settings.mods_deadline().is_none());
        assert!(updates.lock().unwrap().iter().any(
            |u| matches!(u, AppUpdate::AudioMods(s) if s.error.contains("fixture save failed"))
        ));
    }
    state.session.take().unwrap().shutdown().await.unwrap();
}
