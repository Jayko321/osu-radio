//! Local, single-track audio playback. Call from the client's dedicated audio worker.
//!
//! This crate knows only local paths. The caller owns downloaded files and must keep
//! the current file until replacement succeeds or the player has been dropped.

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Source, mixer::Mixer};
mod stream;
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};
use stream::Stream;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerState {
    #[default]
    Empty,
    Paused,
    Playing,
    Stopped,
    Ended,
}

/// Engine parameters, independent of mods or track identity. Speed is a ratio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackParameters {
    /// Finite playback ratio in 0.5..=2.0; defaults to 1.0.
    pub speed: f32,
    /// True follows Rodio's speed/pitch coupling; false preserves source pitch.
    pub adjust_pitch: bool,
}

impl Default for PlaybackParameters {
    fn default() -> Self {
        Self {
            speed: 1.0,
            adjust_pitch: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub state: PlayerState,
    /// Source coordinates, independent of the current speed and DSP lookahead.
    pub position: Duration,
    /// Original source duration; never divided by speed.
    pub duration: Option<Duration>,
    pub volume: f32,
    /// Applied pair (or prepared pair before the first sample), not a pending edit.
    pub parameters: PlaybackParameters,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            state: PlayerState::Empty,
            position: Duration::ZERO,
            duration: None,
            volume: 1.0,
            parameters: PlaybackParameters::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("Unable to open audio output: {0}")]
    Device(#[from] rodio::DeviceSinkError),
    #[error("Audio output failed: {0}")]
    Output(String),
    #[error("Unable to open audio file: {0}")]
    Open(#[from] std::io::Error),
    #[error("Unable to decode audio: {0}")]
    Decode(#[from] rodio::decoder::DecoderError),
    #[error("Unable to seek audio: {0}")]
    Seek(#[from] rodio::source::SeekError),
    #[error("No audio file is loaded")]
    Empty,
    #[error("Volume must be a finite value between 0 and 1")]
    InvalidVolume,
    #[error("Speed must be a finite ratio between 0.5 and 2")]
    InvalidSpeed,
}

/// Owns one output stream and one local track; never opens a device until constructed.
pub struct Player {
    sink: rodio::Player,
    source: Option<Arc<Mutex<Option<Stream>>>>,
    output_error: Arc<Mutex<Option<String>>>,
    mixer: Mixer,
    // Drop the sink before the output stream; Drop reclaims the decoder first.
    _device: Option<MixerDeviceSink>,
    path: Option<PathBuf>,
    state: PlayerState,
    duration: Option<Duration>,
    offset: Duration,
    volume: f32,
    parameters: PlaybackParameters,
}

impl Player {
    pub fn new() -> Result<Self, PlayerError> {
        let output_error = Arc::new(Mutex::new(None));
        let callback_error = output_error.clone();
        let mut device = DeviceSinkBuilder::from_default_device()?
            .with_error_callback(move |error| {
                *lock(&callback_error) = Some(error.to_string());
            })
            .open_sink_or_fallback()?;
        device.log_on_drop(false);
        let mixer = device.mixer().clone();
        let mut player = Self::with_output(mixer, Some(device));
        player.output_error = output_error;
        Ok(player)
    }

    fn with_output(mixer: Mixer, device: Option<MixerDeviceSink>) -> Self {
        Self {
            sink: rodio::Player::connect_new(&mixer),
            source: None,
            output_error: Arc::new(Mutex::new(None)),
            mixer,
            _device: device,
            path: None,
            state: PlayerState::Empty,
            duration: None,
            offset: Duration::ZERO,
            volume: 1.0,
            parameters: PlaybackParameters::default(),
        }
    }

    /// Prepare a file paused. Opening/decoding failures preserve the previous track.
    pub fn load(&mut self, path: impl AsRef<Path>) -> Result<(), PlayerError> {
        self.check_output()?;
        let decoder = Self::decode(path.as_ref())?;
        let duration = decoder.total_duration();
        self.replace(decoder, Duration::ZERO, PlayerState::Paused);
        self.path = Some(path.as_ref().to_owned());
        self.duration = duration;
        Ok(())
    }

    pub fn play(&mut self) -> Result<(), PlayerError> {
        self.check_output()?;
        self.path.as_ref().ok_or(PlayerError::Empty)?;
        let snapshot = self.snapshot();
        let at_end = snapshot
            .duration
            .is_some_and(|duration| snapshot.position >= duration);
        if at_end || matches!(snapshot.state, PlayerState::Ended | PlayerState::Stopped) {
            self.prepare_at(Duration::ZERO, PlayerState::Playing)?;
        } else {
            self.sink.play();
            self.state = PlayerState::Playing;
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), PlayerError> {
        self.check_output()?;
        self.path.as_ref().ok_or(PlayerError::Empty)?;
        if self.snapshot().state == PlayerState::Playing {
            self.sink.pause();
            self.state = PlayerState::Paused;
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), PlayerError> {
        self.path.as_ref().ok_or(PlayerError::Empty)?;
        self.release_source();
        self.offset = Duration::ZERO;
        self.state = PlayerState::Stopped;
        self.check_output()
    }

    /// Seek preserves playing/paused status; seeking after EOF prepares a paused source.
    pub fn seek(&mut self, position: Duration) -> Result<(), PlayerError> {
        self.check_output()?;
        let state = if self.snapshot().state == PlayerState::Playing {
            PlayerState::Playing
        } else {
            PlayerState::Paused
        };
        self.prepare_at(position, state)
    }

    pub fn set_volume(&mut self, volume: f32) -> Result<(), PlayerError> {
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return Err(PlayerError::InvalidVolume);
        }
        self.volume = volume;
        self.sink.set_volume(volume);
        Ok(())
    }

    /// Apply at the next PCM block boundary, or prepare immediately while paused.
    /// Retains source position and pause status.
    /// May be called before load/play; invalid values leave all state unchanged.
    pub fn set_parameters(&mut self, parameters: PlaybackParameters) -> Result<(), PlayerError> {
        if !parameters.speed.is_finite() || !(0.5..=2.0).contains(&parameters.speed) {
            return Err(PlayerError::InvalidSpeed);
        }
        self.check_output()?;
        let paused = self.snapshot().state != PlayerState::Playing;
        if let Some(source) = &self.source
            && let Some(stream) = lock(source).as_mut()
        {
            stream.set_parameters(parameters, paused);
        }
        self.parameters = parameters;
        Ok(())
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let ended = self.state == PlayerState::Playing && self.sink.empty();
        let state = if self.output_error().is_some() {
            PlayerState::Stopped
        } else if ended {
            PlayerState::Ended
        } else {
            self.state
        };
        let (elapsed, parameters) = self
            .source
            .as_ref()
            .and_then(|source| {
                lock(source)
                    .as_ref()
                    .map(|stream| (stream.position(), stream.parameters()))
            })
            .unwrap_or((Duration::ZERO, self.parameters));
        let position = if state == PlayerState::Stopped {
            Duration::ZERO
        } else if ended {
            self.duration
                .unwrap_or_else(|| self.offset.saturating_add(elapsed))
        } else {
            let position = self.offset.saturating_add(elapsed);
            self.duration
                .map_or(position, |duration| position.min(duration))
        };
        Snapshot {
            state,
            position,
            duration: self.duration,
            volume: self.volume,
            parameters,
        }
    }

    /// An asynchronous output-device error, also returned by subsequent transport operations.
    #[must_use]
    pub fn output_error(&self) -> Option<String> {
        lock(&self.output_error).clone()
    }

    fn check_output(&self) -> Result<(), PlayerError> {
        self.output_error()
            .map_or(Ok(()), |error| Err(PlayerError::Output(error)))
    }

    fn release_source(&mut self) {
        self.sink.stop();
        if let Some(source) = self.source.take() {
            // Reclaim the decoder directly, even when device callbacks have stopped.
            // The mixer retains only an empty source shell, never an open file.
            lock(&source).take();
        }
    }

    fn decode(path: &Path) -> Result<Decoder<BufReader<File>>, PlayerError> {
        // TryFrom<File> uses content detection and supplies length/seekability.
        Ok(Decoder::try_from(File::open(path)?)?)
    }

    fn prepare_at(&mut self, position: Duration, state: PlayerState) -> Result<(), PlayerError> {
        let path = self.path.as_ref().ok_or(PlayerError::Empty)?;
        let mut decoder = Self::decode(path)?;
        let position = self
            .duration
            .map_or(position, |duration| position.min(duration));
        if !position.is_zero() {
            decoder.try_seek(position)?;
        }
        self.replace(decoder, position, state);
        Ok(())
    }

    fn replace(&mut self, decoder: Decoder<BufReader<File>>, offset: Duration, state: PlayerState) {
        let sink = rodio::Player::connect_new(&self.mixer);
        sink.pause();
        sink.set_volume(self.volume);
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        let duration = decoder.total_duration();
        let source = Arc::new(Mutex::new(Some(Stream::new(decoder, self.parameters))));
        sink.append(ReleasableSource {
            source: source.clone(),
            channels,
            sample_rate,
            duration,
        });
        self.release_source();
        self.sink = sink;
        self.source = Some(source);
        self.offset = offset;
        self.state = state;
        if state == PlayerState::Playing {
            self.sink.play();
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.release_source();
    }
}

type FileDecoder = Decoder<BufReader<File>>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The worker can close a decoder independently of Rodio's callback/queue lifetime.
struct ReleasableSource {
    source: Arc<Mutex<Option<Stream>>>,
    channels: rodio::ChannelCount,
    sample_rate: rodio::SampleRate,
    duration: Option<Duration>,
}
impl Iterator for ReleasableSource {
    type Item = rodio::Sample;
    fn next(&mut self) -> Option<Self::Item> {
        lock(&self.source).as_mut().and_then(Iterator::next)
    }
}
impl Source for ReleasableSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        self.channels
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.duration
    }
}

#[cfg(test)]
mod tests;
