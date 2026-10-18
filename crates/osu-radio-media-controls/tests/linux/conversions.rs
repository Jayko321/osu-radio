#![allow(clippy::panic_in_result_fn)] // Assertions report failed test expectations.

use super::conversion::*;
use osu_radio_media_controls::{Capabilities, Error, Event, LaunchKey, Snapshot};
use std::time::Duration;

fn snapshot() -> Snapshot {
    Snapshot {
        launch: Some(LaunchKey {
            entry_id: i64::MIN,
            playback_token: u64::MAX,
        }),
        duration: Some(Duration::from_secs(10)),
        position: Duration::from_secs(3),
        capabilities: Capabilities {
            seek: true,
            next: true,
            ..Capabilities::default()
        },
        ..Snapshot::default()
    }
}

#[test]
fn track_identity_and_absolute_boundaries() -> Result<(), Error> {
    let mut state = snapshot();
    let Some(launch) = state.launch else {
        return Err(Error::new("missing fixture launch"));
    };
    let id = track_path("instance1_0", launch);
    assert!(
        id.as_str()
            .contains("entry_8000000000000000/token_ffffffffffffffff")
    );
    assert_eq!(
        absolute_seek(&state, "instance1_0", &id, 2_000_000),
        Some(Event::SeekAbsolute {
            launch,
            position: Duration::from_secs(2)
        })
    );
    assert!(absolute_seek(&state, "instance1_0", &id, -1).is_none());
    assert_eq!(
        absolute_seek(&state, "instance1_0", &id, 10_000_000),
        Some(Event::SeekAbsolute {
            launch,
            position: Duration::from_secs(10)
        })
    );
    assert!(absolute_seek(&state, "instance1_0", &id, 10_000_001).is_none());
    assert!(
        absolute_seek(
            &state,
            "instance1_0",
            "/org/mpris/MediaPlayer2/TrackList/NoTrack",
            0
        )
        .is_none()
    );
    state.launch = Some(LaunchKey {
        playback_token: 0,
        ..launch
    });
    assert!(absolute_seek(&state, "instance1_0", &id, 0).is_none());
    state.duration = None;
    let fresh = track_path(
        "instance1_0",
        state.launch.ok_or_else(|| Error::new("launch"))?,
    );
    assert!(absolute_seek(&state, "instance1_0", &fresh, i64::MAX).is_some());
    state.capabilities.seek = false;
    assert!(absolute_seek(&state, "instance1_0", &fresh, 0).is_none());
    Ok(())
}

#[test]
fn relative_seek_preserves_launch_and_signed_offset() -> Result<(), Error> {
    let mut state = snapshot();
    let launch = state.launch.ok_or_else(|| Error::new("fixture launch"))?;
    for next in [true, false] {
        state.capabilities.next = next;
        for duration in [Some(Duration::from_secs(10)), None] {
            state.duration = duration;
            for offset_micros in [i64::MIN, -1_000_000, 0, 7_000_000, 7_000_001, i64::MAX] {
                assert_eq!(
                    relative_seek(&state, offset_micros),
                    Some(Event::SeekRelative {
                        launch,
                        offset_micros
                    })
                );
            }
        }
    }
    state.capabilities.seek = false;
    assert!(relative_seek(&state, 1).is_none());
    state.capabilities.seek = true;
    state.launch = None;
    assert!(relative_seek(&state, i64::MAX).is_none());
    assert_eq!(micros(Duration::MAX), i64::MAX);
    assert_eq!(micros(Duration::from_nanos(1_999)), 1);
    state.position = Duration::MAX;
    state.duration = Some(Duration::from_secs(10));
    assert_eq!(position(&state), Duration::from_secs(10));
    state.duration = None;
    assert_eq!(position(&state), Duration::MAX);
    Ok(())
}

#[test]
fn volume_stays_in_the_internal_contract() {
    assert_eq!(volume(-0.2), Some(0.0));
    assert_eq!(volume(0.25), Some(0.25));
    assert_eq!(volume(2.0), Some(1.0));
    assert_eq!(volume(f64::NAN), None);
    assert_eq!(volume(f64::INFINITY), None);
}

#[test]
fn seek_notification_requires_confirmation_of_the_same_launch() {
    let previous = snapshot();
    let mut current = previous.clone();
    current.position = Duration::from_secs(4);
    assert_eq!(confirmed_seek(&previous, &current), None);
    current.seek_serial = 1;
    assert_eq!(
        confirmed_seek(&previous, &current),
        Some(Duration::from_secs(4))
    );
    current.position = Duration::from_secs(20);
    assert_eq!(
        confirmed_seek(&previous, &current),
        Some(Duration::from_secs(10))
    );
    current.launch = Some(LaunchKey {
        entry_id: i64::MIN,
        playback_token: 0,
    });
    assert_eq!(confirmed_seek(&previous, &current), None);
    current.launch = None;
    assert_eq!(confirmed_seek(&previous, &current), None);
}
