#![allow(clippy::panic_in_result_fn)]

use super::*;
use crate::{Capabilities, test_backend};

const FIRST: LaunchKey = LaunchKey {
    entry_id: 4,
    playback_token: 10,
};
const SECOND: LaunchKey = LaunchKey {
    entry_id: 4,
    playback_token: 11,
};

#[test]
fn transport_and_status_translate_without_playback_rules() {
    for (button, event) in [
        (SystemMediaTransportControlsButton::Play, Event::Play),
        (SystemMediaTransportControlsButton::Pause, Event::Pause),
        (SystemMediaTransportControlsButton::Stop, Event::Stop),
        (SystemMediaTransportControlsButton::Next, Event::Next),
        (
            SystemMediaTransportControlsButton::Previous,
            Event::Previous,
        ),
    ] {
        assert_eq!(button_event(button), Some(event));
    }
    // SMTC has separate Play/Pause buttons, no PlayPause enum member.
    for button in [
        SystemMediaTransportControlsButton::Record,
        SystemMediaTransportControlsButton::FastForward,
        SystemMediaTransportControlsButton::Rewind,
        SystemMediaTransportControlsButton(999),
    ] {
        assert_eq!(button_event(button), None);
    }
    assert_eq!(
        playback_status(PlaybackStatus::Playing),
        MediaPlaybackStatus::Playing
    );
    assert_eq!(
        playback_status(PlaybackStatus::Paused),
        MediaPlaybackStatus::Paused
    );
    assert_eq!(
        playback_status(PlaybackStatus::Stopped),
        MediaPlaybackStatus::Stopped
    );
}

#[test]
fn timeline_units_bounds_missing_duration_and_seek_availability() {
    let mut snapshot = Snapshot {
        launch: Some(FIRST),
        duration: Some(Duration::from_secs(120)),
        position: Duration::from_millis(1_234),
        capabilities: Capabilities {
            seek: true,
            ..Capabilities::default()
        },
        ..Snapshot::default()
    };
    assert_eq!(
        timeline(&snapshot).map(|value| value.Duration),
        [1_200_000_000, 12_340_000, 0, 1_200_000_000]
    );
    snapshot.position = Duration::from_secs(130);
    assert_eq!(
        timeline(&snapshot).map(|value| value.Duration),
        [1_200_000_000, 1_200_000_000, 0, 1_200_000_000]
    );
    snapshot.capabilities.seek = false;
    snapshot.position = Duration::from_secs(5);
    assert_eq!(
        timeline(&snapshot).map(|value| value.Duration),
        [1_200_000_000, 50_000_000, 50_000_000, 50_000_000]
    );
    snapshot.duration = None;
    assert_eq!(timeline(&snapshot).map(|value| value.Duration), [0; 4]);
    snapshot.duration = Some(Duration::from_secs(120));
    snapshot.launch = None;
    assert_eq!(timeline(&snapshot).map(|value| value.Duration), [0; 4]);
    assert_eq!(to_timespan(Duration::MAX).Duration, i64::MAX);
    assert_eq!(to_timespan(Duration::from_nanos(199)).Duration, 1);
    assert_eq!(from_timespan(TimeSpan { Duration: -1 }), Duration::ZERO);
    assert_eq!(
        from_timespan(TimeSpan { Duration: 12_345 }),
        Duration::from_nanos(1_234_500)
    );
    assert_eq!(
        to_timespan(from_timespan(TimeSpan { Duration: i64::MAX })).Duration,
        i64::MAX
    );
}

#[tokio::test]
async fn callback_captures_launch_and_rejects_late_events() -> Result<(), Box<dyn std::error::Error>>
{
    let (mut controls, backend, mut receiver) = test_backend(Snapshot {
        launch: Some(FIRST),
        ..Snapshot::default()
    });
    let callbacks = Callbacks {
        events: backend.events.clone(),
        active: Arc::new(AtomicBool::new(true)),
    };
    // Native callback captures before the next snapshot, consumer executes afterward.
    let received_launch = callbacks.events.current_launch();
    assert!(controls.publish(Snapshot {
        launch: Some(SECOND),
        ..Snapshot::default()
    }));
    assert!(callbacks.position(
        received_launch,
        TimeSpan {
            Duration: 50_000_000
        }
    ));
    assert_eq!(
        receiver.recv().await,
        Some(Event::SeekAbsolute {
            launch: FIRST,
            position: Duration::from_secs(5)
        })
    );
    assert!(!callbacks.position(None, TimeSpan::default()));
    assert!(callbacks.button(SystemMediaTransportControlsButton::Pause));
    assert_eq!(receiver.recv().await, Some(Event::Pause));
    callbacks.active.store(false, Ordering::Release);
    assert!(!callbacks.button(SystemMediaTransportControlsButton::Play));
    assert!(!callbacks.position(Some(SECOND), TimeSpan::default()));
    // The real owner drops Backend only after native cleanup; exercise that boundary.
    let shutdown = tokio::spawn(async move {
        controls.shutdown().await;
        controls
    });
    tokio::task::yield_now().await;
    drop(backend);
    let mut controls = shutdown.await?;
    controls.shutdown().await;
    assert_eq!(receiver.recv().await, None);
    Ok(())
}

#[test]
#[ignore = "opt-in Windows desktop probe: real HWND, native registrations and temporary artwork"]
fn native_close_rejects_retained_callbacks_and_releases_file()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::{Artwork, ArtworkKey, Metadata};
    use windows::{
        Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
        },
        core::w,
    };

    struct Window(HWND);
    impl Drop for Window {
        fn drop(&mut self) {
            let _ = unsafe { DestroyWindow(self.0) };
        }
    }
    let _apartment = Apartment::new()?;
    let window = Window(unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!("SMTC cleanup probe"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            100,
            100,
            None,
            None,
            None,
            None,
        )?
    });
    let hwnd = NonZeroIsize::new(isize::from_ne_bytes(
        window.0.0.expose_provenance().to_ne_bytes(),
    ))
    .ok_or("null HWND")?;
    let snapshot = Snapshot {
        launch: Some(FIRST),
        metadata: Some(Metadata {
            title: Some("cleanup fixture".into()),
            artist: None,
        }),
        artwork: Some(Artwork {
            key: ArtworkKey {
                launch: FIRST,
                cover_id: 1,
                generation: 1,
            },
            bytes: Some(Arc::from(
                include_bytes!("../../tests/windows/cover.png").as_slice(),
            )),
        }),
        ..Snapshot::default()
    };
    let (mut controls, mut backend, _receiver) = test_backend(snapshot.clone());
    let mut session = Session::new(hwnd, backend.events.clone())?;
    session.publish(&snapshot, &mut backend.artwork)?;
    let path = backend
        .artwork
        .path()
        .ok_or("missing native artwork")?
        .to_owned();
    assert!(path.exists());
    let callbacks = session.callbacks.clone();
    assert!(callbacks.button(SystemMediaTransportControlsButton::Pause));
    assert!(callbacks.position(
        Some(FIRST),
        TimeSpan {
            Duration: 10_000_000
        }
    ));
    session.close()?;
    session.close()?;
    assert!(!session.controls.IsEnabled()?);
    assert!(!callbacks.button(SystemMediaTransportControlsButton::Pause));
    assert!(!callbacks.position(
        Some(FIRST),
        TimeSpan {
            Duration: 10_000_000
        }
    ));
    drop(session);
    // This is the production acknowledgement boundary after releasing native references.
    backend.fail(Error::new("test cleanup"));
    drop(backend);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(controls.shutdown());
    assert!(!path.exists());
    assert!(unsafe { IsWindow(Some(window.0)) }.as_bool());
    Ok(())
}
