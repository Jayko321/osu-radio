#![allow(clippy::indexing_slicing)]
use super::*;
use crate::controller::{
    AppCommand,
    tests::{controller, track},
};
use crate::models::{LibraryTrack, PlaybackMode};

fn assignment(id: i32, percent: Option<u8>, revision: u64) -> PlaybackAssignment {
    PlaybackAssignment {
        volume_percent: percent,
        current_audio_source_id: Some(id),
        current_playlist_item_id: None,
        track: Some(LibraryTrack {
            audio_source_id: id,
            last_played_at_ms: None,
            volume_percent: percent,
            title: None,
            title_unicode: None,
            artist: None,
            artist_unicode: None,
            cover_beatmap_id: None,
            difficulties: Vec::new(),
        }),
        duration_ms: None,
        mode: PlaybackMode::Paused,
        revision,
        playback_token: revision,
        can_next: true,
        can_previous: true,
    }
}

#[test]
fn absolute_volumes_selection_reset_toggle_and_stale_reads() {
    let (mut state, _) = controller();
    state.replace_tracks(vec![track(1), track(2)], true);
    state.set_global_volume(20);
    state.set_individual_volume(true);
    state.set_selected_volume(0.1);
    state.current_track = Some(state.tracks[0].clone());
    state.assignment = Some(assignment(1, Some(10), 1));
    assert_eq!(state.assignment_volume().to_bits(), 0.1_f32.to_bits());
    state.set_global_volume(40);
    assert_eq!(state.assignment_volume().to_bits(), 0.1_f32.to_bits());
    state.command(AppCommand::SelectTrack(Some(2)));
    state.set_selected_volume(0.3);
    assert_eq!(state.track_override(2), Some(30));
    assert_eq!(
        state.assignment_volume().to_bits(),
        0.1_f32.to_bits(),
        "B edit leaves A alone"
    );
    state.set_track_volume(1, Some(99));
    assert_eq!(
        state.track_override(1),
        Some(10),
        "late slider gesture for A is rejected after selection B"
    );
    state.command(AppCommand::SelectTrack(Some(1)));
    state.set_track_volume(1, None);
    assert_eq!(state.assignment_volume().to_bits(), 0.4_f32.to_bits());
    state.set_track_volume(1, Some(10));
    state.set_individual_volume(false);
    assert_eq!(state.assignment_volume().to_bits(), 0.4_f32.to_bits());
    state.set_individual_volume(true);
    assert_eq!(state.assignment_volume().to_bits(), 0.1_f32.to_bits());
    state.replace_tracks(vec![track(1), track(2)], true);
    assert_eq!(state.tracks[0].volume_percent, Some(10));
    assert_eq!(state.tracks[1].volume_percent, Some(30));
    state.set_selected_volume(0.0);
    assert_eq!(state.track_override(1), Some(0));
    state.set_selected_volume(1.0);
    assert_eq!(state.track_override(1), Some(100));
    for invalid in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
        state.set_selected_volume(invalid);
    }
    state.set_global_volume(101);
    assert_eq!(state.track_override(1), Some(100));
    assert_eq!(state.volume.settings.unwrap().global_volume_percent, 40);
    state.command(AppCommand::SelectTrack(None));
    state.set_selected_volume(0.5);
    assert_eq!(state.selected_volume_id(), None);
    assert_eq!(state.track_override(1), Some(100));
}

#[tokio::test]
async fn loading_blocks_audio_and_queue_transitions_receive_their_volume() {
    let (mut state, _) = controller();
    state.volume.settings = None;
    state.apply_assignment(assignment(1, Some(10), 1));
    state.apply_assignment(assignment(2, None, 2));
    state.apply_assignment(assignment(1, None, 1));
    assert!(state.assignment.is_none() && state.playback.is_none());
    state.audio_settings_loaded(Err("offline".into()));
    assert!(state.assignment.is_none() && !state.volume.error.is_empty());
    state.audio_settings_loaded(Ok(AudioSettings {
        individual_volume_enabled: true,
        global_volume_percent: 20,
    }));
    assert_eq!(
        state.assignment.as_ref().unwrap().current_audio_source_id,
        Some(2)
    );
    assert_eq!(state.assignment_volume().to_bits(), 0.2_f32.to_bits());
    state.apply_assignment(assignment(1, Some(10), 3));
    assert_eq!(state.assignment_volume().to_bits(), 0.1_f32.to_bits());
    state.apply_assignment(assignment(2, None, 4));
    assert_eq!(state.assignment_volume().to_bits(), 0.2_f32.to_bits());
    state.playback.take().unwrap().shutdown();
}

#[tokio::test]
async fn debounce_keeps_latest_serializes_targets_and_retries_without_rollback() {
    let (mut state, _) = controller();
    state.replace_tracks(vec![track(1), track(2)], true);
    tokio::time::pause();
    state.set_global_volume(20);
    let first = state.volume_deadline().unwrap();
    tokio::time::advance(Duration::from_millis(100)).await;
    state.set_global_volume(40);
    assert_eq!(state.volume.pending.len(), 1);
    assert_eq!(
        state.volume_deadline().unwrap(),
        first + Duration::from_millis(100)
    );
    let save = state.volume.pending.pop_front().unwrap();
    state.volume.saving = Some(save);
    state.set_global_volume(60);
    state.volume_saved(Ok(()));
    assert_eq!(state.volume.settings.unwrap().global_volume_percent, 60);
    let save = state.volume.pending.pop_front().unwrap();
    state.volume.saving = Some(save);
    state.set_global_volume(70);
    state.volume_saved(Err("late failure".into()));
    assert!(
        state.volume.error.is_empty(),
        "newer write will persist the current value"
    );
    assert_eq!(state.volume.pending.len(), 1);
    let save = state.volume.pending.pop_front().unwrap();
    state.volume.saving = Some(save);
    state.volume_saved(Err("offline".into()));
    assert!(state.volume_deadline().is_none());
    assert!(state.volume.error.contains("offline"));
    state.retry_audio_settings();
    assert_eq!(state.volume_deadline(), Some(Instant::now()));
    assert_eq!(state.volume.settings.unwrap().global_volume_percent, 70);
    state.set_individual_volume(true);
    state.set_selected_volume(0.1);
    state.command(AppCommand::SelectTrack(Some(2)));
    state.set_selected_volume(0.3);
    assert_eq!(
        state.volume.pending.len(),
        3,
        "settings plus independent A/B targets"
    );
    tokio::time::resume();
}

#[cfg(unix)]
#[tokio::test]
async fn shutdown_flushes_pending_writes_in_order_through_the_api() {
    shutdown_flush_case(false).await;
    shutdown_flush_case(true).await;
}

#[cfg(unix)]
async fn shutdown_flush_case(fail_first: bool) {
    use std::os::unix::fs::PermissionsExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for index in 0..3 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                header.push(stream.read_u8().await.unwrap());
            }
            let header = String::from_utf8(header).unwrap();
            let length = header
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .map_or(0, |len| len.parse::<usize>().unwrap());
            let mut body = vec![0; length];
            stream.read_exact(&mut body).await.unwrap();
            requests.push((header.lines().next().unwrap().to_owned(), body));
            let failed = fail_first && index == 0;
            let status = if failed {
                "500 Internal Server Error"
            } else {
                "200 OK"
            };
            let payload = if failed {
                r#"{"error":"fixture save failed"}"#
            } else if header.starts_with("PATCH") {
                r#"{"individual_volume_enabled":true,"global_volume_percent":40}"#
            } else {
                ""
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
    let session = crate::Session::start(crate::ServerOptions {
        binary: Some(binary),
        ..Default::default()
    })
    .await
    .unwrap();
    let (mut state, _) = controller();
    state.session = Some(session);
    state.replace_tracks(vec![track(1), track(2)], true);
    state.set_global_volume(40);
    state.set_individual_volume(true);
    state.set_selected_volume(0.1);
    state.command(AppCommand::SelectTrack(Some(2)));
    state.set_selected_volume(0.3);
    state.flush_volume_saves().await;
    assert!(state.volume.pending.is_empty() && state.volume.save_task.is_none());
    let requests = server.await.unwrap();
    assert!(
        requests[0]
            .0
            .starts_with("PATCH /api/user-data/audio-settings ")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&requests[0].1).unwrap()["global_volume_percent"],
        40
    );
    assert!(
        requests[1]
            .0
            .starts_with("PUT /api/audio-sources/1/volume ")
    );
    assert!(
        requests[2]
            .0
            .starts_with("PUT /api/audio-sources/2/volume ")
    );
    state.session.take().unwrap().shutdown().await.unwrap();
}
