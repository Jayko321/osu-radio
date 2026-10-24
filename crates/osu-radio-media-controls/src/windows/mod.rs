use crate::{
    ArtworkStore, Backend, Error, Event, EventSender, LaunchKey, PlaybackStatus, Repeat, Snapshot,
};
use std::{
    num::NonZeroIsize,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::{oneshot, watch};
use windows::{
    Foundation::{TimeSpan, TypedEventHandler},
    Media::{
        AutoRepeatModeChangeRequestedEventArgs, MediaPlaybackAutoRepeatMode, MediaPlaybackStatus,
        MediaPlaybackType, PlaybackPositionChangeRequestedEventArgs,
        ShuffleEnabledChangeRequestedEventArgs, SystemMediaTransportControls,
        SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
        SystemMediaTransportControlsDisplayUpdater, SystemMediaTransportControlsTimelineProperties,
    },
    Storage::{StorageFile, Streams::RandomAccessStreamReference},
    Win32::{
        Foundation::{E_INVALIDARG, HWND},
        System::WinRT::{
            ISystemMediaTransportControlsInterop, RO_INIT_MULTITHREADED, RoInitialize,
            RoUninitialize,
        },
        UI::WindowsAndMessaging::{GA_ROOT, GetAncestor, GetWindowThreadProcessId, IsWindow},
    },
    core::{HSTRING, Result as WinResult, factory},
};

pub(crate) async fn run(hwnd: NonZeroIsize, backend: &mut Backend) -> Result<(), Error> {
    let snapshots = backend.snapshots.clone();
    let stop = backend.stop.clone();
    let events = backend.events.clone();
    let artwork = std::mem::take(&mut backend.artwork);
    let (ready, connected) = oneshot::channel();
    // Every WinRT call, including destruction, stays on this initialized MTA thread.
    let worker = tokio::task::spawn_blocking(move || {
        run_blocking(hwnd, snapshots, stop, events, artwork, ready)
    });
    if connected.await.is_ok() {
        let _ = backend.connected();
    }
    worker
        .await
        .map_err(|error| Error::new(format!("SMTC worker: {error}")))?
}

fn run_blocking(
    hwnd: NonZeroIsize,
    mut snapshots: watch::Receiver<Snapshot>,
    mut stop: watch::Receiver<bool>,
    events: EventSender,
    mut artwork: ArtworkStore,
    ready: oneshot::Sender<()>,
) -> Result<(), Error> {
    let _apartment = Apartment::new().map_err(native_error)?;
    if *stop.borrow() {
        return Ok(());
    }
    let mut session = Session::new(hwnd, events).map_err(native_error)?;
    let initial = snapshots.borrow_and_update().clone();
    session
        .publish(&initial, &mut artwork)
        .map_err(native_error)?;
    session.controls.SetIsEnabled(true).map_err(native_error)?;
    let _ = ready.send(());
    let runtime = tokio::runtime::Handle::current();
    let result = loop {
        if *stop.borrow() {
            break Ok(());
        }
        let changed = runtime.block_on(async {
            tokio::select! {
                biased;
                _ = stop.changed() => false,
                changed = snapshots.changed() => changed.is_ok(),
            }
        });
        if !changed || *stop.borrow() {
            break Ok(());
        }
        let snapshot = snapshots.borrow_and_update().clone();
        if let Err(error) = session.publish(&snapshot, &mut artwork) {
            break Err(error);
        }
    };
    let cleanup = session.close();
    drop(session);
    artwork.clear();
    result.and(cleanup).map_err(native_error)
}

#[allow(clippy::needless_pass_by_value)] // Used by Result::map_err, which consumes the error.
fn native_error(error: windows::core::Error) -> Error {
    Error::new(format!("Windows SMTC: {error}"))
}

struct Apartment;
impl Apartment {
    fn new() -> WinResult<Self> {
        // SMTC is documented ThreadingModel.MTA; a Tokio future may migrate threads.
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED)?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // Balanced on the same blocking thread, after all WinRT objects are dropped.
        unsafe {
            RoUninitialize();
        }
    }
}

#[derive(Clone)]
struct Callbacks {
    events: EventSender,
    active: Arc<AtomicBool>,
}
impl Callbacks {
    fn button(&self, button: SystemMediaTransportControlsButton) -> bool {
        if !self.active.load(Ordering::Acquire) {
            return false;
        }
        button_event(button).is_some_and(|event| self.events.send(event))
    }

    fn shuffle(&self, enabled: bool) -> bool {
        self.active.load(Ordering::Acquire) && self.events.send(Event::SetShuffle(enabled))
    }

    fn repeat(&self, mode: MediaPlaybackAutoRepeatMode) -> bool {
        self.active.load(Ordering::Acquire)
            && repeat_from_native(mode)
                .is_some_and(|repeat| self.events.send(Event::SetRepeat(repeat)))
    }

    fn position(&self, launch: Option<LaunchKey>, position: TimeSpan) -> bool {
        self.active.load(Ordering::Acquire)
            && launch.is_some_and(|launch| {
                self.events.send(Event::SeekAbsolute {
                    launch,
                    position: from_timespan(position),
                })
            })
    }
}

struct Session {
    controls: SystemMediaTransportControls,
    updater: SystemMediaTransportControlsDisplayUpdater,
    callbacks: Callbacks,
    buttons: Option<i64>,
    position: Option<i64>,
    shuffle: Option<i64>,
    repeat: Option<i64>,
    last: Option<Snapshot>,
    closed: bool,
}
impl Session {
    fn new(hwnd: NonZeroIsize, events: EventSender) -> WinResult<Self> {
        let hwnd = HWND(std::ptr::with_exposed_provenance_mut(
            hwnd.get().cast_unsigned(),
        ));
        let mut process = 0;
        // The facade borrows a live top-level window of this process until shutdown.
        unsafe {
            if !IsWindow(Some(hwnd)).as_bool()
                || GetAncestor(hwnd, GA_ROOT) != hwnd
                || GetWindowThreadProcessId(hwnd, Some(&raw mut process)) == 0
                || process != std::process::id()
            {
                return Err(windows::core::Error::new(
                    E_INVALIDARG,
                    "SMTC requires a live top-level HWND owned by this process",
                ));
            }
        }
        let interop =
            factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
        let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(hwnd)? };
        let updater = controls.DisplayUpdater()?;
        let mut session = Self {
            controls,
            updater,
            callbacks: Callbacks {
                events,
                active: Arc::new(AtomicBool::new(true)),
            },
            buttons: None,
            position: None,
            shuffle: None,
            repeat: None,
            last: None,
            closed: false,
        };
        session.controls.SetIsEnabled(false)?;
        let callbacks = session.callbacks.clone();
        session.buttons =
            Some(session.controls.ButtonPressed(&TypedEventHandler::<
                SystemMediaTransportControls,
                SystemMediaTransportControlsButtonPressedEventArgs,
            >::new(move |_, args| {
                if let Some(args) = args.as_ref() {
                    let _ = callbacks.button(args.Button()?);
                }
                Ok(())
            }))?);
        let callbacks = session.callbacks.clone();
        session.position = Some(session.controls.PlaybackPositionChangeRequested(
            &TypedEventHandler::<
                SystemMediaTransportControls,
                PlaybackPositionChangeRequestedEventArgs,
            >::new(move |_, args| {
                // Capture before reading the native argument or queueing the command.
                let launch = callbacks.events.current_launch();
                if let Some(args) = args.as_ref() {
                    let _ = callbacks.position(launch, args.RequestedPlaybackPosition()?);
                }
                Ok(())
            }),
        )?);
        let callbacks = session.callbacks.clone();
        session.shuffle = Some(session.controls.ShuffleEnabledChangeRequested(
            &TypedEventHandler::<
                SystemMediaTransportControls,
                ShuffleEnabledChangeRequestedEventArgs,
            >::new(move |_, args| {
                if let Some(args) = args.as_ref() {
                    let _ = callbacks.shuffle(args.RequestedShuffleEnabled()?);
                }
                Ok(())
            }),
        )?);
        let callbacks = session.callbacks.clone();
        session.repeat = Some(session.controls.AutoRepeatModeChangeRequested(
            &TypedEventHandler::<
                SystemMediaTransportControls,
                AutoRepeatModeChangeRequestedEventArgs,
            >::new(move |_, args| {
                if let Some(args) = args.as_ref() {
                    let _ = callbacks.repeat(args.RequestedAutoRepeatMode()?);
                }
                Ok(())
            }),
        )?);
        Ok(session)
    }

    fn publish(&mut self, snapshot: &Snapshot, artwork: &mut ArtworkStore) -> WinResult<()> {
        snapshot
            .validate_rate()
            .map_err(|error| windows::core::Error::new(E_INVALIDARG, error.to_string()))?;
        let capabilities = snapshot.capabilities;
        self.controls.SetIsPlayEnabled(capabilities.play)?;
        self.controls.SetIsPauseEnabled(capabilities.pause)?;
        self.controls.SetIsStopEnabled(capabilities.stop)?;
        self.controls.SetIsNextEnabled(capabilities.next)?;
        self.controls.SetIsPreviousEnabled(capabilities.previous)?;
        self.controls.SetIsFastForwardEnabled(false)?;
        self.controls.SetIsRewindEnabled(false)?;
        self.controls.SetIsRecordEnabled(false)?;
        self.controls.SetIsChannelUpEnabled(false)?;
        self.controls.SetIsChannelDownEnabled(false)?;
        self.controls
            .SetPlaybackStatus(playback_status(snapshot.status))?;
        self.controls.SetPlaybackRate(snapshot.rate)?;
        self.controls.SetShuffleEnabled(snapshot.shuffle)?;
        self.controls
            .SetAutoRepeatMode(repeat_to_native(snapshot.repeat))?;

        let display_changed = self.last.as_ref().is_none_or(|last| {
            last.launch != snapshot.launch
                || last.metadata != snapshot.metadata
                || last.artwork != snapshot.artwork
        });
        if display_changed {
            // Release the native reference before replacing/deleting the temporary file.
            self.updater.ClearAll()?;
            self.updater.SetType(MediaPlaybackType::Music)?;
            let music = self.updater.MusicProperties()?;
            let metadata = snapshot.metadata.as_ref();
            music.SetTitle(&HSTRING::from(
                metadata
                    .and_then(|data| data.title.as_deref())
                    .unwrap_or_default(),
            ))?;
            music.SetArtist(&HSTRING::from(
                metadata
                    .and_then(|data| data.artist.as_deref())
                    .unwrap_or_default(),
            ))?;
            // Commit the cleared cover before any file I/O for the new track.
            self.updater.Update()?;
            if let Err(error) = self.set_artwork(snapshot, artwork) {
                eprintln!("SMTC artwork unavailable: {error}");
                self.updater
                    .SetThumbnail(None::<&RandomAccessStreamReference>)?;
                artwork.clear();
            }
            self.updater.Update()?;
        }
        let [end, position, min_seek, max_seek] = timeline(snapshot);
        let properties = SystemMediaTransportControlsTimelineProperties::new()?;
        properties.SetStartTime(TimeSpan::default())?;
        properties.SetEndTime(end)?;
        properties.SetPosition(position)?;
        properties.SetMinSeekTime(min_seek)?;
        properties.SetMaxSeekTime(max_seek)?;
        self.controls.UpdateTimelineProperties(&properties)?;
        self.last = Some(snapshot.clone());
        Ok(())
    }

    fn set_artwork(
        &self,
        snapshot: &Snapshot,
        artwork: &mut ArtworkStore,
    ) -> Result<(), Box<dyn std::error::Error>> {
        artwork.update(snapshot)?;
        if let Some(path) = artwork.path() {
            let file =
                StorageFile::GetFileFromPathAsync(&HSTRING::from(path.as_os_str()))?.join()?;
            let thumbnail = RandomAccessStreamReference::CreateFromFile(&file)?;
            self.updater.SetThumbnail(&thumbnail)?;
        }
        Ok(())
    }

    #[allow(clippy::or_fun_call)] // Cleanup must attempt every call even after an error.
    fn close(&mut self) -> WinResult<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.callbacks.active.store(false, Ordering::Release);
        // Attempt every release even when an earlier native call fails.
        let mut result = Ok(());
        if let Some(token) = self.buttons.take() {
            result = result.and(self.controls.RemoveButtonPressed(token));
        }
        if let Some(token) = self.position.take() {
            result = result.and(self.controls.RemovePlaybackPositionChangeRequested(token));
        }
        if let Some(token) = self.shuffle.take() {
            result = result.and(self.controls.RemoveShuffleEnabledChangeRequested(token));
        }
        if let Some(token) = self.repeat.take() {
            result = result.and(self.controls.RemoveAutoRepeatModeChangeRequested(token));
        }
        result = result.and(self.controls.SetIsEnabled(false));
        result = result.and(self.controls.SetPlaybackStatus(MediaPlaybackStatus::Closed));
        result = result.and(self.updater.ClearAll());
        result.and(self.updater.Update())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("SMTC cleanup failed: {error}");
        }
    }
}

const fn button_event(button: SystemMediaTransportControlsButton) -> Option<Event> {
    match button {
        SystemMediaTransportControlsButton::Play => Some(Event::Play),
        SystemMediaTransportControlsButton::Pause => Some(Event::Pause),
        SystemMediaTransportControlsButton::Stop => Some(Event::Stop),
        SystemMediaTransportControlsButton::Next => Some(Event::Next),
        SystemMediaTransportControlsButton::Previous => Some(Event::Previous),
        _ => None,
    }
}

const fn repeat_to_native(repeat: Repeat) -> MediaPlaybackAutoRepeatMode {
    match repeat {
        Repeat::Off => MediaPlaybackAutoRepeatMode::None,
        Repeat::One => MediaPlaybackAutoRepeatMode::Track,
        Repeat::All => MediaPlaybackAutoRepeatMode::List,
    }
}

const fn repeat_from_native(mode: MediaPlaybackAutoRepeatMode) -> Option<Repeat> {
    match mode {
        MediaPlaybackAutoRepeatMode::None => Some(Repeat::Off),
        MediaPlaybackAutoRepeatMode::Track => Some(Repeat::One),
        MediaPlaybackAutoRepeatMode::List => Some(Repeat::All),
        _ => None,
    }
}

const fn playback_status(status: PlaybackStatus) -> MediaPlaybackStatus {
    match status {
        PlaybackStatus::Stopped => MediaPlaybackStatus::Stopped,
        PlaybackStatus::Playing => MediaPlaybackStatus::Playing,
        PlaybackStatus::Paused => MediaPlaybackStatus::Paused,
    }
}

fn to_timespan(duration: Duration) -> TimeSpan {
    TimeSpan {
        Duration: i64::try_from(duration.as_nanos().saturating_div(100)).unwrap_or(i64::MAX),
    }
}

fn from_timespan(position: TimeSpan) -> Duration {
    Duration::from_nanos(u64::try_from(position.Duration).unwrap_or_default()).saturating_mul(100)
}

fn timeline(snapshot: &Snapshot) -> [TimeSpan; 4] {
    let Some(duration) = snapshot.duration.filter(|_| snapshot.launch.is_some()) else {
        // Unknown duration clears the preceding track's range instead of inventing an end.
        return [TimeSpan::default(); 4];
    };
    let end = to_timespan(duration);
    let position = to_timespan(snapshot.position.min(duration));
    let (min_seek, max_seek) = if snapshot.capabilities.seek {
        (TimeSpan::default(), end)
    } else {
        (position, position)
    };
    [end, position, min_seek, max_seek]
}

#[cfg(test)]
mod tests;
