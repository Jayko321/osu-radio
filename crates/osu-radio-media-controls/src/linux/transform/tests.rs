#![allow(clippy::panic_in_result_fn)] // Assertions report failed test expectations.

use super::*;
use crate::{Capabilities, Metadata as Text};

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
fn metadata_is_optional_and_file_uri_is_escaped() -> Result<(), Error> {
    let mut state = snapshot();
    state.metadata = Some(Text {
        title: Some("曲名".into()),
        artist: Some(String::new()),
    });
    let published = Published::new(state, Some(Path::new("/tmp/a #%.png")), "instance1_0")?;
    assert_eq!(published.metadata.title(), Some("曲名"));
    assert!(published.metadata.artist().is_none());
    assert_eq!(
        published.metadata.art_url().as_deref(),
        Some("file:///tmp/a%20%23%25.png")
    );
    assert_eq!(
        published.metadata.length(),
        Some(Time::from_micros(10_000_000))
    );
    let empty = Published::new(Snapshot::default(), None, "instance1_0")?;
    assert!(empty.metadata.trackid().is_none());
    assert!(empty.metadata.title().is_none());
    assert!(empty.metadata.art_url().is_none());
    assert!(empty.metadata.length().is_none());
    Ok(())
}

#[test]
fn notification_diff_excludes_position_and_includes_changed_properties() -> Result<(), Error> {
    let old = Published::new(snapshot(), None, "instance1_0")?;
    let mut next = old.snapshot.clone();
    next.position = Duration::from_secs(4);
    let tick = Published::new(next.clone(), None, "instance1_0")?;
    assert!(changes(&old, &tick).is_empty());
    next.seek_serial = 1;
    next.launch = Some(LaunchKey {
        entry_id: 1,
        playback_token: 1,
    });
    next.status = PlaybackStatus::Paused;
    next.volume = 0.5;
    next.capabilities = Capabilities {
        play: true,
        pause: true,
        previous: true,
        ..Capabilities::default()
    };
    let changed = changes(&old, &Published::new(next, None, "instance1_0")?);
    assert!(
        changed
            .iter()
            .any(|property| matches!(property, Property::Metadata(_)))
    );
    for property in [
        Property::PlaybackStatus(mpris_server::PlaybackStatus::Paused),
        Property::Volume(0.5),
        Property::CanPlay(true),
        Property::CanPause(true),
        Property::CanGoNext(false),
        Property::CanGoPrevious(true),
        Property::CanSeek(false),
    ] {
        assert!(changed.contains(&property));
    }
    Ok(())
}

#[test]
fn rate_changes_preserve_source_coordinates_and_metadata() -> Result<(), Error> {
    let mut previous = Published::new(snapshot(), None, "instance1_0")?;
    for status in [PlaybackStatus::Playing, PlaybackStatus::Paused] {
        for rate in [0.75, 1.2, 1.5] {
            let mut next = previous.snapshot.clone();
            next.status = status;
            next.rate = rate;
            let current = Published::new(next, None, "instance1_0")?;
            assert!(changes(&previous, &current).contains(&Property::Rate(rate)));
            assert_eq!(current.metadata, previous.metadata);
            assert_eq!(current.snapshot.position, previous.snapshot.position);
            assert_eq!(current.snapshot.duration, previous.snapshot.duration);
            assert_eq!(current.snapshot.launch, previous.snapshot.launch);
            assert_eq!(
                conversion::confirmed_seek(&previous.snapshot, &current.snapshot),
                None
            );
            assert!(changes(&current, &current).is_empty());
            previous = current;
        }
    }
    let invalid = Snapshot {
        rate: f64::NAN,
        ..snapshot()
    };
    assert!(Published::new(invalid, None, "instance1_0").is_err());
    Ok(())
}
