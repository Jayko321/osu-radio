//! Local system media integration. The client owns playback rules; adapters only translate I/O.
mod artwork;
mod lifecycle;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

pub use artwork::ArtworkStore;
pub use lifecycle::{Backend, EventReceiver, EventSender, MediaControls, Status, test_backend};

use std::{fmt, num::NonZeroIsize, sync::Arc, time::Duration};

/// Opaque launch identity. A repeated audio source must have a new playback token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LaunchKey {
    pub entry_id: i64,
    pub playback_token: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
}

/// Computed by the client, never inferred from selection by an adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Independent capabilities of the system API.
pub struct Capabilities {
    pub play: bool,
    pub pause: bool,
    pub stop: bool,
    pub next: bool,
    pub previous: bool,
    pub seek: bool,
    pub set_volume: bool,
    /// The system may request shuffle and repeat changes.
    pub set_modes: bool,
}

/// Source-neutral repeat mode; adapters map it to their platform's vocabulary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Repeat {
    #[default]
    Off,
    One,
    All,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub title: Option<String>,
    pub artist: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArtworkKey {
    pub launch: LaunchKey,
    pub cover_id: i32,
    pub generation: u64,
}

/// Bytes are immutable within a key; a refreshed cover uses a new generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artwork {
    pub key: ArtworkKey,
    /// None means this identity is pending or unavailable, not the previous cover.
    pub bytes: Option<Arc<[u8]>>,
}

/// Latest observed state, with no GUI placeholders or resources.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub launch: Option<LaunchKey>,
    pub metadata: Option<Metadata>,
    pub duration: Option<Duration>,
    pub status: PlaybackStatus,
    pub position: Duration,
    /// Actual playback speed, finite and within 0.5..=2.0. Times stay in source coordinates.
    /// An invalid rate fails the native adapter through `Status::Failed`.
    pub rate: f64,
    pub seek_serial: u64,
    /// Effective internal player volume, normalized to 0..=1.
    pub volume: f64,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub capabilities: Capabilities,
    pub artwork: Option<Artwork>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            launch: None,
            metadata: None,
            duration: None,
            status: PlaybackStatus::default(),
            position: Duration::ZERO,
            rate: 1.0,
            seek_serial: 0,
            volume: 0.0,
            shuffle: false,
            repeat: Repeat::Off,
            capabilities: Capabilities::default(),
            artwork: None,
        }
    }
}

impl Snapshot {
    pub(crate) fn validate_rate(&self) -> Result<(), Error> {
        if (0.5..=2.0).contains(&self.rate) {
            Ok(())
        } else {
            Err(Error::new(format!(
                "media playback rate must be finite and within 0.5..=2.0: {}",
                self.rate
            )))
        }
    }
}

/// Commands retain arrival order independently of coalesced snapshots.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    SeekAbsolute {
        launch: LaunchKey,
        position: Duration,
    },
    SeekRelative {
        launch: LaunchKey,
        offset_micros: i64,
    },
    SetVolume(f64),
    SetShuffle(bool),
    SetRepeat(Repeat),
}

/// The window owner must keep HWND valid until acknowledged shutdown completes.
/// Linux requires no native window. No toolkit objects cross this boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeContext {
    Linux,
    Windows { hwnd: NonZeroIsize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    message: String,
}
impl Error {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Error {}

async fn run_native(context: NativeContext, backend: &mut Backend) -> Result<(), Error> {
    #[cfg(target_os = "linux")]
    if context == NativeContext::Linux {
        return linux::run(backend).await;
    }
    #[cfg(target_os = "windows")]
    if let NativeContext::Windows { hwnd } = context {
        return windows::run(hwnd, backend).await;
    }
    let _ = (context, backend);
    Err(Error::new("native media context is unsupported on this OS"))
}

#[cfg(test)]
mod tests;
