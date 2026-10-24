#![allow(clippy::panic_in_result_fn)]

use crate::{
    Artwork, ArtworkKey, ArtworkStore, Error, Event, LaunchKey, MediaControls, NativeContext,
    Snapshot, Status, test_backend,
};
use std::{error::Error as StdError, num::NonZeroIsize, sync::Arc, time::Duration};
use tokio::time::timeout;

type TestResult = Result<(), Box<dyn StdError>>;
const FIRST: LaunchKey = LaunchKey {
    entry_id: 1,
    playback_token: 7,
};
// Same queue entry/audio, a new launch.
const REPEATED: LaunchKey = LaunchKey {
    entry_id: 1,
    playback_token: 8,
};
const COVER: ArtworkKey = ArtworkKey {
    launch: FIRST,
    cover_id: 3,
    generation: 10,
};

fn snapshot(launch: LaunchKey) -> Snapshot {
    Snapshot {
        launch: Some(launch),
        ..Snapshot::default()
    }
}

#[test]
#[allow(clippy::float_cmp)] // The neutral default and protocol bounds must be exact.
fn playback_rate_defaults_to_one_and_rejects_invalid_values() {
    assert_eq!(Snapshot::default().rate, 1.0);
    for rate in [0.5, 0.75, 1.0, 1.2, 1.5, 2.0] {
        assert!(
            Snapshot {
                rate,
                ..Snapshot::default()
            }
            .validate_rate()
            .is_ok()
        );
    }
    for rate in [
        f64::NAN,
        f64::NEG_INFINITY,
        f64::INFINITY,
        -1.0,
        0.0,
        0.49,
        2.01,
    ] {
        assert!(
            Snapshot {
                rate,
                ..Snapshot::default()
            }
            .validate_rate()
            .is_err()
        );
    }
}

#[tokio::test]
async fn slow_snapshots_coalesce_but_commands_keep_order_and_callback_identity() -> TestResult {
    let (mut controls, mut backend, mut events) = test_backend(snapshot(FIRST));
    assert!(backend.connected());
    assert_eq!(*controls.status().borrow(), Status::Connected);
    let launch = backend.events.current_launch().ok_or("missing launch")?;
    let seek = Event::SeekRelative {
        launch,
        offset_micros: -2_000_000,
    };
    assert!(backend.events.send(Event::Play));
    assert!(backend.events.send(seek.clone()));
    for position in 0..10_000 {
        let mut next = snapshot(REPEATED);
        next.position = Duration::from_millis(position);
        assert!(controls.publish(next));
    }
    assert_eq!(
        backend.snapshots.borrow_and_update().position,
        Duration::from_millis(9_999)
    );
    assert_eq!(backend.events.current_launch(), Some(REPEATED));
    assert!(backend.events.send(Event::Pause));
    assert_eq!(events.recv().await, Some(Event::Play));
    assert_eq!(events.recv().await, Some(seek));
    assert_eq!(events.recv().await, Some(Event::Pause));
    // No queued intermediate snapshots after taking the latest one.
    assert!(!backend.snapshots.has_changed()?);
    let task = tokio::spawn(async move {
        backend.stop.changed().await?;
        Ok::<_, tokio::sync::watch::error::RecvError>(())
    });
    timeout(Duration::from_secs(1), controls.shutdown()).await?;
    task.await??;
    Ok(())
}

#[tokio::test]
async fn shutdown_waits_for_cleanup_rejects_queued_and_late_callbacks_and_is_repeatable()
-> TestResult {
    let (mut controls, mut backend, mut events) = test_backend(snapshot(FIRST));
    backend.artwork.set_current(Some(COVER));
    assert!(backend.artwork.install(COVER, b"cover bytes")?);
    let path = backend
        .artwork
        .path()
        .ok_or("missing artwork file")?
        .to_path_buf();
    let callback = backend.events.clone();
    assert!(callback.send(Event::Next));
    // Cancellation of the wait must not lose the acknowledgement receiver.
    assert!(
        timeout(Duration::from_millis(10), controls.shutdown())
            .await
            .is_err()
    );
    assert!(*backend.stop.borrow());
    assert!(!callback.send(Event::Play));
    assert_eq!(events.recv().await, None);
    assert!(!controls.publish(snapshot(REPEATED)));
    assert!(path.exists());
    drop(backend);
    timeout(Duration::from_secs(1), controls.shutdown()).await?;
    timeout(Duration::from_secs(1), controls.shutdown()).await?;
    assert!(!path.exists());
    assert_eq!(*controls.status().borrow(), Status::Disconnected);
    assert!(!callback.send(Event::Stop));
    Ok(())
}

#[tokio::test]
async fn drop_requests_shutdown_and_wakes_an_idle_event_receiver() -> TestResult {
    let (controls, backend, mut events) = test_backend(snapshot(FIRST));
    drop(controls);
    assert!(*backend.stop.borrow());
    assert_eq!(timeout(Duration::from_secs(1), events.recv()).await?, None);
    assert!(!backend.events.send(Event::Next));
    Ok(())
}

#[tokio::test]
async fn fatal_failure_is_sticky_and_keeps_the_session_disabled() -> TestResult {
    let (mut controls, backend, mut events) = test_backend(snapshot(FIRST));
    assert!(backend.connected());
    let failure = Error::new("native API failed");
    backend.fail(failure.clone());
    assert!(!backend.connected());
    assert!(!controls.publish(snapshot(REPEATED)));
    assert!(!backend.events.send(Event::Play));
    assert_eq!(events.recv().await, None);
    drop(backend);
    controls.shutdown().await;
    controls.shutdown().await;
    assert_eq!(*controls.status().borrow(), Status::Failed(failure));
    Ok(())
}

#[tokio::test]
async fn unexpected_backend_exit_is_a_diagnostic_failure() -> TestResult {
    let (mut controls, backend, _) = test_backend(Snapshot::default());
    drop(backend);
    controls.shutdown().await;
    assert!(matches!(*controls.status().borrow(), Status::Failed(_)));
    assert!(!controls.publish(Snapshot::default()));
    Ok(())
}

#[tokio::test]
async fn native_connect_failure_is_observable_without_desktop_or_retry() -> TestResult {
    // Deliberately choose the other OS so this remains valid after native adapters land.
    let context = if cfg!(target_os = "windows") {
        NativeContext::Linux
    } else {
        NativeContext::Windows {
            hwnd: NonZeroIsize::new(1).ok_or("zero HWND")?,
        }
    };
    let (mut controls, mut events) = MediaControls::connect(context, Snapshot::default())?;
    let mut status = controls.status();
    timeout(Duration::from_secs(1), async {
        while *status.borrow_and_update() == Status::Connecting {
            status.changed().await?;
        }
        Ok::<_, tokio::sync::watch::error::RecvError>(())
    })
    .await??;
    assert!(matches!(*status.borrow(), Status::Failed(_)));
    let failed = status.borrow().clone();
    assert_eq!(events.recv().await, None);
    controls.shutdown().await;
    assert!(!controls.publish(Snapshot::default()));
    assert_eq!(*status.borrow(), failed);
    Ok(())
}

#[test]
fn artwork_replacement_stale_results_and_repeated_audio_launch() -> TestResult {
    let mut store = ArtworkStore::default();
    store.set_current(Some(COVER));
    assert!(store.install(COVER, b"first")?);
    let first = store.path().ok_or("missing first file")?.to_path_buf();
    assert_eq!(std::fs::read(&first)?, b"first");
    let next = ArtworkKey {
        launch: REPEATED,
        ..COVER
    };
    store.set_current(Some(next));
    assert!(!first.exists());
    assert!(!store.install(COVER, b"late result")?);
    assert!(store.path().is_none());
    assert!(store.install(next, b"second")?);
    let second = store.path().ok_or("missing second file")?.to_path_buf();
    let new_generation = ArtworkKey {
        generation: 11,
        ..next
    };
    store.set_current(Some(new_generation));
    assert!(!second.exists());
    assert!(!store.install(next, b"late generation")?);
    let new_cover = ArtworkKey {
        cover_id: 4,
        ..new_generation
    };
    store.set_current(Some(new_cover));
    assert!(!store.install(new_generation, b"late cover")?);
    assert!(store.install(new_cover, b"current")?);
    let current = store.path().ok_or("missing current file")?.to_path_buf();
    drop(store);
    assert!(!current.exists());
    Ok(())
}

#[tokio::test]
async fn artwork_failure_keeps_commands_and_other_metadata_working() -> TestResult {
    let mut initial = snapshot(FIRST);
    initial.artwork = Some(Artwork {
        key: COVER,
        bytes: Some(Arc::from(&b"cover"[..])),
    });
    let (mut controls, mut backend, mut events) = test_backend(initial.clone());
    assert!(backend.connected());
    backend.artwork.update(&initial)?;
    let first = backend.artwork.path().ok_or("missing file")?.to_path_buf();
    backend.artwork.update(&initial)?;
    assert_eq!(backend.artwork.path(), Some(first.as_path()));
    let mut invalid = initial.clone();
    invalid.artwork = Some(Artwork {
        key: ArtworkKey {
            generation: 11,
            ..COVER
        },
        bytes: Some(Arc::from([])),
    });
    assert!(backend.artwork.update(&invalid).is_err());
    assert!(!first.exists());
    assert!(backend.artwork.path().is_none());
    assert_eq!(*controls.status().borrow(), Status::Connected);
    assert!(backend.events.send(Event::PlayPause));
    assert_eq!(events.recv().await, Some(Event::PlayPause));
    // A late result attached to a new launch must never become its cover.
    initial.launch = Some(REPEATED);
    backend.artwork.update(&initial)?;
    assert!(backend.artwork.path().is_none());
    // Missing artwork clears an installed cover too.
    backend.artwork.set_current(Some(COVER));
    backend.artwork.install(COVER, b"current")?;
    let current = backend.artwork.path().ok_or("missing file")?.to_path_buf();
    backend.artwork.update(&Snapshot::default())?;
    assert!(!current.exists());
    backend.fail(Error::new("test complete"));
    drop(backend);
    controls.shutdown().await;
    Ok(())
}
