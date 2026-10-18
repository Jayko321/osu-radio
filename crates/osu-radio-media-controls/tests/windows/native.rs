use osu_radio_media_controls::{
    Artwork, ArtworkKey, Capabilities, LaunchKey, MediaControls, Metadata, NativeContext,
    PlaybackStatus, Snapshot, Status,
};
use std::{error::Error, num::NonZeroIsize, sync::Arc, time::Duration};
use windows::{
    Media::{MediaPlaybackStatus, SystemMediaTransportControls},
    Win32::{
        Foundation::{HINSTANCE, HWND},
        System::{
            LibraryLoader::GetModuleHandleW,
            WinRT::{
                ISystemMediaTransportControlsInterop, RO_INIT_MULTITHREADED, RoInitialize,
                RoUninitialize,
            },
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, DispatchMessageW, GA_ROOT, GetAncestor, IsWindow,
            IsWindowVisible, MSG, PM_REMOVE, PeekMessageW, TranslateMessage, WINDOW_EX_STYLE,
            WS_OVERLAPPEDWINDOW,
        },
    },
    core::{factory, w},
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const FIRST: LaunchKey = LaunchKey {
    entry_id: 1,
    playback_token: 1,
};

// A real top-level HWND, owned and pumped on this test thread until acknowledged stop.
struct HiddenWindow(HWND);
impl HiddenWindow {
    fn new() -> TestResult<Self> {
        let instance = unsafe { GetModuleHandleW(None)? };
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("osu-radio SMTC native probe"),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                320,
                200,
                None,
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )?
        };
        assert!(unsafe { IsWindow(Some(hwnd)) }.as_bool());
        assert!(!unsafe { IsWindowVisible(hwnd) }.as_bool());
        assert_eq!(unsafe { GetAncestor(hwnd, GA_ROOT) }, hwnd);
        Ok(Self(hwnd))
    }

    fn context(&self) -> TestResult<NativeContext> {
        Ok(NativeContext::Windows {
            hwnd: NonZeroIsize::new(isize::from_ne_bytes(
                self.0.0.expose_provenance().to_ne_bytes(),
            ))
            .ok_or("CreateWindowEx returned null")?,
        })
    }
}
impl Drop for HiddenWindow {
    fn drop(&mut self) {
        if let Err(error) = unsafe { DestroyWindow(self.0) } {
            eprintln!("probe DestroyWindow failed: {error}");
        }
    }
}

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}

fn pump_messages() {
    let mut message = MSG::default();
    unsafe {
        while PeekMessageW(&raw mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&raw const message);
            DispatchMessageW(&raw const message);
        }
    }
}

async fn until(mut check: impl FnMut() -> TestResult<bool>) -> TestResult {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            pump_messages();
            if check()? {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await?
}

fn initial() -> Snapshot {
    Snapshot {
        launch: Some(FIRST),
        metadata: Some(Metadata {
            title: Some("曲 A".into()),
            artist: Some("Исполнитель A".into()),
        }),
        duration: Some(Duration::from_secs(120)),
        position: Duration::from_secs(12),
        status: PlaybackStatus::Playing,
        capabilities: Capabilities {
            play: true,
            pause: true,
            stop: true,
            next: true,
            seek: true,
            ..Capabilities::default()
        },
        ..Snapshot::default()
    }
}

#[test]
#[ignore = "opt-in Windows desktop probe: owns a real hidden top-level HWND"]
#[allow(clippy::too_many_lines)] // One native window/session lifecycle, including cleanup assertions.
fn real_hwnd_publish_clear_artwork_and_acknowledged_shutdown() -> TestResult {
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }
    let _apartment = Apartment;
    let window = HiddenWindow::new()?;
    let interop = factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
    let native: SystemMediaTransportControls = unsafe { interop.GetForWindow(window.0)? };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let mut snapshot = initial();
        let (mut controls, mut events) =
            MediaControls::connect(window.context()?, snapshot.clone())?;
        let status = controls.status();
        until(|| match &*status.borrow() {
            Status::Connected => Ok(true),
            Status::Failed(error) => Err(Box::new(error.clone())),
            _ => Ok(false),
        })
        .await?;
        assert!(native.IsEnabled()?);
        assert_eq!(native.PlaybackStatus()?, MediaPlaybackStatus::Playing);
        assert!(native.IsPlayEnabled()?);
        assert!(native.IsPauseEnabled()?);
        assert!(native.IsNextEnabled()?);
        assert!(!native.IsPreviousEnabled()?);
        let updater = native.DisplayUpdater()?;
        assert_eq!(updater.MusicProperties()?.Title()?, "曲 A");
        assert_eq!(updater.MusicProperties()?.Artist()?, "Исполнитель A");

        let png: &[u8] = include_bytes!("cover.png");
        snapshot.artwork = Some(Artwork {
            key: ArtworkKey {
                launch: FIRST,
                cover_id: 2,
                generation: 1,
            },
            bytes: Some(Arc::from(png)),
        });
        snapshot.metadata = Some(Metadata {
            title: Some("with cover".into()),
            artist: None,
        });
        assert!(controls.publish(snapshot.clone()));
        until(|| {
            Ok(updater.MusicProperties()?.Title()? == "with cover" && updater.Thumbnail().is_ok())
        })
        .await?;
        let thumbnail = updater.Thumbnail()?;
        let stream = thumbnail.OpenReadAsync()?.join()?;
        assert_eq!(stream.Size()?, u64::try_from(png.len())?);
        drop(stream);
        drop(thumbnail);

        snapshot.launch = Some(LaunchKey {
            entry_id: 1,
            playback_token: 2,
        });
        snapshot.metadata = Some(Metadata {
            title: Some("B".into()),
            artist: None,
        });
        snapshot.duration = None;
        snapshot.status = PlaybackStatus::Paused;
        snapshot.capabilities.next = false;
        snapshot.capabilities.previous = true;
        snapshot.capabilities.seek = false;
        snapshot.artwork = None;
        assert!(controls.publish(snapshot.clone()));
        until(|| Ok(updater.MusicProperties()?.Title()? == "B")).await?;
        assert_eq!(updater.MusicProperties()?.Artist()?, "");
        assert!(updater.Thumbnail().is_err());
        assert_eq!(native.PlaybackStatus()?, MediaPlaybackStatus::Paused);
        assert!(!native.IsNextEnabled()?);
        assert!(native.IsPreviousEnabled()?);

        // A cover write failure must leave the same native session usable.
        snapshot.artwork = Some(Artwork {
            key: ArtworkKey {
                launch: snapshot.launch.ok_or("launch missing")?,
                cover_id: 3,
                generation: 2,
            },
            bytes: Some(Arc::from([])),
        });
        snapshot.metadata = Some(Metadata {
            title: Some("bad cover".into()),
            artist: None,
        });
        assert!(controls.publish(snapshot.clone()));
        until(|| Ok(updater.MusicProperties()?.Title()? == "bad cover")).await?;
        assert!(updater.Thumbnail().is_err());
        assert_eq!(*status.borrow(), Status::Connected);
        assert!(native.IsEnabled()?);
        snapshot.metadata = None;
        snapshot.artwork = None;
        assert!(controls.publish(snapshot));
        until(|| Ok(updater.MusicProperties()?.Title()?.is_empty())).await?;

        tokio::time::timeout(Duration::from_secs(15), controls.shutdown()).await?;
        controls.shutdown().await;
        assert_eq!(*status.borrow(), Status::Disconnected);
        assert!(!native.IsEnabled()?);
        assert_eq!(native.PlaybackStatus()?, MediaPlaybackStatus::Closed);
        assert_eq!(events.recv().await, None);
        // The native window still exists after acknowledged teardown.
        assert!(unsafe { IsWindow(Some(window.0)) }.as_bool());
        Ok::<_, Box<dyn Error>>(())
    })?;
    drop(native);
    let hwnd = window.0;
    drop(window);
    assert!(!unsafe { IsWindow(Some(hwnd)) }.as_bool());
    Ok(())
}

#[tokio::test]
async fn invalid_hwnd_fails_once_and_acknowledges_shutdown() -> TestResult {
    let hwnd = NonZeroIsize::new(-1).ok_or("invalid nonzero fixture")?;
    let (mut controls, mut events) =
        MediaControls::connect(NativeContext::Windows { hwnd }, initial())?;
    let status = controls.status();
    until(|| Ok(matches!(*status.borrow(), Status::Failed(_)))).await?;
    assert!(!controls.publish(Snapshot::default()));
    assert_eq!(events.recv().await, None);
    tokio::time::timeout(Duration::from_secs(15), controls.shutdown()).await?;
    controls.shutdown().await;
    assert!(matches!(*status.borrow(), Status::Failed(_)));
    Ok(())
}
