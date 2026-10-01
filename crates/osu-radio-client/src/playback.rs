//! Toolkit-free playback state shared by frontend adapters.
use osu_radio_player::Player;
pub use osu_radio_player::{PlayerState, Snapshot};
use std::{
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};
use tempfile::NamedTempFile;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playback {
    pub current_audio_id: Option<i32>,
    pub loading_audio_id: Option<i32>,
    pub snapshot: Snapshot,
    pub error: Option<String>,
    pub has_source: bool,
    pub playback_token: u64,
    pub can_next: bool,
    pub can_previous: bool,
}

pub(crate) enum Command {
    Assign {
        generation: u64,
        id: Option<i32>,
        playback_token: u64,
        mode: crate::models::PlaybackMode,
    },
    Loaded {
        generation: u64,
        id: i32,
        result: Result<NamedTempFile, String>,
    },
    Seek {
        expected_id: Option<i32>,
        playback_token: u64,
        position: Duration,
    },
    SetVolume(f32),
    Shutdown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Download {
    pub generation: u64,
    pub id: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Message {
    Download(Download),
    Finished(u64),
    Failed(u64),
    DeviceFailure(u64),
}

/// The mutex linearizes a new request against the synchronous engine's load/play commit.
/// A response that has become stale can never replace the current source.
pub(crate) struct Worker {
    commands: mpsc::Sender<Command>,
    generation: Arc<Mutex<u64>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Worker {
    pub fn spawn(
        emit: Arc<dyn Fn(Playback) + Send + Sync>,
    ) -> std::io::Result<(Self, tokio::sync::mpsc::UnboundedReceiver<Message>)> {
        Self::spawn_engine::<Player>(emit)
    }
    #[cfg(test)]
    pub(crate) fn spawn_fake(
        emit: Arc<dyn Fn(Playback) + Send + Sync>,
    ) -> std::io::Result<(Self, tokio::sync::mpsc::UnboundedReceiver<Message>)> {
        Self::spawn_engine::<tests::FakeEngine>(emit)
    }
    fn spawn_engine<E: Engine + 'static>(
        emit: Arc<dyn Fn(Playback) + Send + Sync>,
    ) -> std::io::Result<(Self, tokio::sync::mpsc::UnboundedReceiver<Message>)> {
        let (commands, receiver) = mpsc::channel();
        let (downloads, requests) = tokio::sync::mpsc::unbounded_channel();
        let generation = Arc::new(Mutex::new(0));
        let worker_generation = generation.clone();
        let thread = thread::Builder::new()
            .name("osu-radio-audio".into())
            .spawn(move || {
                let mut core = Core::<E>::default();
                loop {
                    match receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(Command::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Ok(command) => {
                            let guard = worker_generation
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            if let Some(request) = core.command(command, *guard) {
                                let _ = downloads.send(Message::Download(request));
                            }
                            drop(guard);
                            if let Some(feedback) = core.feedback.take() {
                                let _ = downloads.send(feedback);
                            }
                            emit(core.state.clone());
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let previous = core.state.clone();
                            core.refresh();
                            if let Some(feedback) = core.feedback.take() {
                                let _ = downloads.send(feedback);
                            }
                            if previous != core.state
                                || core.state.snapshot.state == PlayerState::Playing
                            {
                                emit(core.state.clone());
                            }
                        }
                    }
                }
                // Source handles must close before the temporary path is removed (Windows too).
                core.player.take();
                core.file.take();
            })?;
        Ok((
            Self {
                commands,
                generation,
                thread: Some(thread),
            },
            requests,
        ))
    }
    pub fn invalidate(&self) -> u64 {
        let mut generation = self
            .generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *generation = generation.wrapping_add(1);
        *generation
    }
    pub fn generation(&self) -> u64 {
        *self
            .generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub fn is_current(&self, candidate: u64) -> bool {
        *self
            .generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            == candidate
    }
    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }
    pub fn shutdown(&mut self) {
        self.invalidate();
        self.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// Private seam for deterministic worker tests; the production player never crosses threads.
trait Engine: Sized {
    fn new() -> Result<Self, String>;
    fn load(&mut self, path: &Path) -> Result<(), String>;
    fn play(&mut self) -> Result<(), String>;
    fn pause(&mut self) -> Result<(), String>;
    fn seek(&mut self, position: Duration) -> Result<(), String>;
    fn volume(&mut self, volume: f32) -> Result<(), String>;
    fn snapshot(&self) -> Snapshot;
    fn output_error(&self) -> Option<String> {
        None
    }
}
impl Engine for Player {
    fn new() -> Result<Self, String> {
        Self::new().map_err(|e| crate::describe(&e))
    }
    fn load(&mut self, path: &Path) -> Result<(), String> {
        self.load(path).map_err(|e| crate::describe(&e))
    }
    fn play(&mut self) -> Result<(), String> {
        self.play().map_err(|e| crate::describe(&e))
    }
    fn pause(&mut self) -> Result<(), String> {
        self.pause().map_err(|e| crate::describe(&e))
    }
    fn seek(&mut self, position: Duration) -> Result<(), String> {
        self.seek(position).map_err(|e| crate::describe(&e))
    }
    fn volume(&mut self, volume: f32) -> Result<(), String> {
        self.set_volume(volume).map_err(|e| crate::describe(&e))
    }
    fn snapshot(&self) -> Snapshot {
        self.snapshot()
    }
    fn output_error(&self) -> Option<String> {
        self.output_error()
    }
}

struct Core<E> {
    // Keep field order: the engine drops before the file on every exit, including unwinding.
    player: Option<E>,
    file: Option<NamedTempFile>,
    file_id: Option<i32>,
    state: Playback,
    mode: crate::models::PlaybackMode,
    feedback: Option<Message>,
    reported: bool,
}
impl<E> Default for Core<E> {
    fn default() -> Self {
        Self {
            player: None,
            file: None,
            file_id: None,
            state: Playback::default(),
            mode: crate::models::PlaybackMode::Stopped,
            feedback: None,
            reported: false,
        }
    }
}
impl<E: Engine> Core<E> {
    #[allow(clippy::too_many_lines)]
    fn command(&mut self, command: Command, generation: u64) -> Option<Download> {
        use crate::models::PlaybackMode;
        let result = match command {
            Command::Assign {
                generation: candidate,
                id,
                playback_token,
                mode,
            } => {
                if candidate != generation {
                    return None;
                }
                let restart = playback_token != self.state.playback_token
                    || id != self.state.current_audio_id;
                if !restart && self.mode == mode {
                    self.refresh();
                    return None;
                }
                if self.mode != mode && mode == PlaybackMode::Playing {
                    self.reported = false;
                }
                self.mode = mode;
                if restart {
                    self.player.take();
                    if self.file_id != id {
                        self.file.take();
                        self.file_id = None;
                    }
                    self.state.current_audio_id = id;
                    self.state.loading_audio_id = None;
                    self.state.playback_token = playback_token;
                    if id.is_some() {
                        self.state.error = None;
                    }
                    self.state.has_source = false;
                    self.state.snapshot.position = Duration::ZERO;
                    self.state.snapshot.duration = None;
                    self.reported = false;
                }
                match mode {
                    PlaybackMode::Stopped => {
                        self.player.take();
                        self.state.has_source = false;
                        self.state.loading_audio_id = None;
                        self.state.snapshot.state = PlayerState::Stopped;
                        self.state.snapshot.position = Duration::ZERO;
                        Ok(())
                    }
                    PlaybackMode::Paused => {
                        self.state.snapshot.state = PlayerState::Paused;
                        self.player.as_mut().map_or(Ok(()), Engine::pause)
                    }
                    PlaybackMode::Playing => {
                        self.state.error = None;
                        if self
                            .player
                            .as_ref()
                            .is_some_and(|player| player.snapshot().state == PlayerState::Ended)
                        {
                            // Resuming EOF with the same token completes that launch; replay requires a new token.
                            self.report(Message::Finished(playback_token));
                            Ok(())
                        } else if self.player.is_some() {
                            self.player.as_mut().map_or(Ok(()), Engine::play)
                        } else if self.file_id == id && self.file.is_some() {
                            self.load_cached()
                        } else if self.state.loading_audio_id == id {
                            Ok(())
                        } else if let Some(id) = id {
                            self.state.loading_audio_id = Some(id);
                            self.state.snapshot.state = PlayerState::Playing;
                            return Some(Download { generation, id });
                        } else {
                            Ok(())
                        }
                    }
                }
            }
            Command::Loaded {
                generation: candidate,
                id,
                result,
            } => {
                if candidate != generation || self.state.current_audio_id != Some(id) {
                    return None;
                }
                self.state.loading_audio_id = None;
                match result {
                    Ok(file) => {
                        self.player.take();
                        self.file = Some(file);
                        self.file_id = Some(id);
                        self.load_cached()
                    }
                    Err(error) => {
                        self.report(Message::Failed(self.state.playback_token));
                        Err(error)
                    }
                }
            }
            Command::Seek {
                expected_id,
                playback_token,
                position,
            } => {
                if playback_token != self.state.playback_token
                    || expected_id.is_some_and(|id| self.state.current_audio_id != Some(id))
                {
                    return None;
                }
                self.player
                    .as_mut()
                    .map_or(Ok(()), |player| player.seek(position))
            }
            Command::SetVolume(volume) => {
                if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
                    Err("volume must be a finite value between 0 and 1".into())
                } else {
                    self.state.snapshot.volume = volume;
                    self.player
                        .as_mut()
                        .map_or(Ok(()), |player| player.volume(volume))
                }
            }
            Command::Shutdown => return None,
        };
        if let Err(error) = result {
            self.state.error = Some(error);
        }
        self.refresh();
        None
    }
    const fn report(&mut self, message: Message) {
        if !self.reported {
            self.feedback = Some(message);
            self.reported = true;
        }
    }
    fn load_cached(&mut self) -> Result<(), String> {
        if self.player.is_none() {
            let mut player = match E::new() {
                Ok(player) => player,
                Err(error) => {
                    self.report(Message::DeviceFailure(self.state.playback_token));
                    return Err(error);
                }
            };
            player.volume(self.state.snapshot.volume)?;
            self.player = Some(player);
        }
        let result = match (&mut self.player, &self.file) {
            (Some(player), Some(file)) => player.load(file.path()).and_then(|()| {
                if self.mode == crate::models::PlaybackMode::Playing {
                    player.play()
                } else {
                    Ok(())
                }
            }),
            _ => Ok(()),
        };
        self.state.has_source = result.is_ok();
        if result.is_err() {
            let device = self
                .player
                .as_ref()
                .and_then(Engine::output_error)
                .is_some();
            self.report(if device {
                Message::DeviceFailure(self.state.playback_token)
            } else {
                Message::Failed(self.state.playback_token)
            });
        }
        result
    }
    fn refresh(&mut self) {
        if let Some(player) = &self.player {
            self.state.snapshot = player.snapshot();
            if let Some(error) = player.output_error() {
                self.state.error = Some(error);
                self.report(Message::DeviceFailure(self.state.playback_token));
                // Release the failed device and decoder so explicit Play can open a new device.
                self.player.take();
                self.state.has_source = false;
            } else if self.state.snapshot.state == PlayerState::Ended
                && self.mode == crate::models::PlaybackMode::Playing
            {
                self.report(Message::Finished(self.state.playback_token));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PlaybackMode;
    use std::io::Write;

    #[derive(Default)]
    pub(super) struct FakeEngine {
        snapshot: Snapshot,
        loads: usize,
        output_error: Option<String>,
    }
    impl Engine for FakeEngine {
        fn new() -> Result<Self, String> {
            Ok(Self::default())
        }
        fn load(&mut self, path: &Path) -> Result<(), String> {
            if std::fs::read(path).unwrap() == b"bad" {
                return Err("decode failed".into());
            }
            self.loads = self.loads.saturating_add(1);
            self.snapshot.state = PlayerState::Paused;
            self.snapshot.position = Duration::ZERO;
            Ok(())
        }
        fn play(&mut self) -> Result<(), String> {
            self.snapshot.state = PlayerState::Playing;
            Ok(())
        }
        fn pause(&mut self) -> Result<(), String> {
            if self.snapshot.state == PlayerState::Playing {
                self.snapshot.state = PlayerState::Paused;
            }
            Ok(())
        }
        fn seek(&mut self, position: Duration) -> Result<(), String> {
            self.snapshot.position = position;
            Ok(())
        }
        fn volume(&mut self, volume: f32) -> Result<(), String> {
            self.snapshot.volume = volume;
            Ok(())
        }
        fn snapshot(&self) -> Snapshot {
            self.snapshot.clone()
        }
        fn output_error(&self) -> Option<String> {
            self.output_error.clone()
        }
    }
    fn file(bytes: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        file
    }
    fn assign(
        core: &mut Core<FakeEngine>,
        generation: u64,
        token: u64,
        id: i32,
        mode: PlaybackMode,
    ) -> Option<Download> {
        core.command(
            Command::Assign {
                generation,
                id: Some(id),
                playback_token: token,
                mode,
            },
            generation,
        )
    }
    fn load(core: &mut Core<FakeEngine>, token: u64, id: i32) {
        assert!(assign(core, token, token, id, PlaybackMode::Playing).is_some());
        core.command(
            Command::Loaded {
                generation: token,
                id,
                result: Ok(file(b"mp3")),
            },
            token,
        );
    }
    #[test]
    fn duplicate_id_new_token_restarts_and_same_token_reconnect_preserves_position() {
        let mut core = Core::<FakeEngine>::default();
        load(&mut core, 1, 10);
        core.command(
            Command::Seek {
                expected_id: Some(10),
                playback_token: 1,
                position: Duration::from_secs(12),
            },
            1,
        );
        assert!(assign(&mut core, 1, 1, 10, PlaybackMode::Playing).is_none());
        assert_eq!(core.state.snapshot.position, Duration::from_secs(12));
        assert!(assign(&mut core, 2, 2, 10, PlaybackMode::Playing).is_none());
        assert_eq!(core.state.snapshot.position, Duration::ZERO);
        core.command(
            Command::Seek {
                expected_id: Some(10),
                playback_token: 1,
                position: Duration::from_secs(99),
            },
            2,
        );
        assert_eq!(core.state.snapshot.position, Duration::ZERO);
    }
    #[test]
    fn stale_downloads_are_removed_and_pause_during_download_never_resumes() {
        let mut core = Core::<FakeEngine>::default();
        assign(&mut core, 1, 1, 10, PlaybackMode::Playing);
        assign(&mut core, 2, 2, 20, PlaybackMode::Playing);
        let stale = file(b"mp3");
        let path = stale.path().to_owned();
        core.command(
            Command::Loaded {
                generation: 1,
                id: 10,
                result: Ok(stale),
            },
            2,
        );
        assert!(!path.exists());
        assert!(core.player.is_none());
        assign(&mut core, 2, 2, 20, PlaybackMode::Paused);
        core.command(
            Command::Loaded {
                generation: 2,
                id: 20,
                result: Ok(file(b"mp3")),
            },
            2,
        );
        assert_eq!(core.state.snapshot.state, PlayerState::Paused);
    }
    #[test]
    fn finished_failed_and_device_feedback_captures_launch_token_and_emits_once() {
        let mut core = Core::<FakeEngine>::default();
        load(&mut core, 1, 10);
        core.player.as_mut().unwrap().snapshot.state = PlayerState::Ended;
        core.refresh();
        assert_eq!(core.feedback.take(), Some(Message::Finished(1)));
        core.refresh();
        assert!(core.feedback.take().is_none());
        assign(&mut core, 1, 1, 10, PlaybackMode::Playing);
        assert_eq!(core.state.snapshot.state, PlayerState::Ended);
        assign(&mut core, 1, 1, 10, PlaybackMode::Paused);
        assign(&mut core, 1, 1, 10, PlaybackMode::Playing);
        assert_eq!(
            core.state.snapshot.state,
            PlayerState::Ended,
            "same-token resume must not replay EOF"
        );
        core.player.as_mut().unwrap().snapshot.state = PlayerState::Ended;
        core.refresh();
        assert_eq!(core.feedback.take(), Some(Message::Finished(1)));
        assign(&mut core, 2, 2, 20, PlaybackMode::Playing);
        core.command(
            Command::Loaded {
                generation: 2,
                id: 20,
                result: Ok(file(b"bad")),
            },
            2,
        );
        assert_eq!(core.feedback.take(), Some(Message::Failed(2)));
        assign(&mut core, 3, 3, 30, PlaybackMode::Playing);
        core.command(
            Command::Loaded {
                generation: 3,
                id: 30,
                result: Err("download failed".into()),
            },
            3,
        );
        assert_eq!(core.feedback.take(), Some(Message::Failed(3)));
        load(&mut core, 4, 40);
        core.player.as_mut().unwrap().output_error = Some("device lost".into());
        core.refresh();
        assert_eq!(core.feedback.take(), Some(Message::DeviceFailure(4)));
        assert!(core.player.is_none());
        core.refresh();
        assert!(core.feedback.take().is_none());
    }
    std::thread_local! {
        static DEVICE_OPEN_FAILURES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    struct MissingDevice(FakeEngine);
    impl Engine for MissingDevice {
        fn new() -> Result<Self, String> {
            if DEVICE_OPEN_FAILURES.with(|remaining| {
                let failed = remaining.get() > 0;
                remaining.set(remaining.get().saturating_sub(1));
                failed
            }) {
                Err("device unavailable".into())
            } else {
                Ok(Self(FakeEngine::default()))
            }
        }
        fn load(&mut self, path: &Path) -> Result<(), String> {
            self.0.load(path)
        }
        fn play(&mut self) -> Result<(), String> {
            self.0.play()
        }
        fn pause(&mut self) -> Result<(), String> {
            self.0.pause()
        }
        fn seek(&mut self, position: Duration) -> Result<(), String> {
            self.0.seek(position)
        }
        fn volume(&mut self, volume: f32) -> Result<(), String> {
            self.0.volume(volume)
        }
        fn snapshot(&self) -> Snapshot {
            self.0.snapshot()
        }
    }
    #[test]
    fn explicit_resume_reports_each_device_open_failure_with_the_same_token() {
        DEVICE_OPEN_FAILURES.with(|remaining| remaining.set(2));
        let mut core = Core::<MissingDevice>::default();
        core.command(
            Command::Assign {
                generation: 1,
                id: Some(10),
                playback_token: 4,
                mode: PlaybackMode::Playing,
            },
            1,
        );
        core.command(
            Command::Loaded {
                generation: 1,
                id: 10,
                result: Ok(file(b"mp3")),
            },
            1,
        );
        assert_eq!(core.feedback.take(), Some(Message::DeviceFailure(4)));
        core.command(
            Command::Assign {
                generation: 1,
                id: Some(10),
                playback_token: 4,
                mode: PlaybackMode::Paused,
            },
            1,
        );
        core.command(
            Command::Assign {
                generation: 1,
                id: Some(10),
                playback_token: 4,
                mode: PlaybackMode::Playing,
            },
            1,
        );
        assert_eq!(core.feedback.take(), Some(Message::DeviceFailure(4)));
        core.command(
            Command::Assign {
                generation: 1,
                id: Some(10),
                playback_token: 4,
                mode: PlaybackMode::Playing,
            },
            1,
        );
        assert!(core.feedback.take().is_none());
        core.command(
            Command::Assign {
                generation: 1,
                id: Some(10),
                playback_token: 4,
                mode: PlaybackMode::Paused,
            },
            1,
        );
        core.command(
            Command::Assign {
                generation: 1,
                id: Some(10),
                playback_token: 4,
                mode: PlaybackMode::Playing,
            },
            1,
        );
        assert_eq!(core.state.snapshot.state, PlayerState::Playing);
        assert!(core.state.error.is_none());
    }
    #[test]
    fn paused_restore_is_lazy_and_volume_and_file_lifetime_are_preserved() {
        let mut core = Core::<FakeEngine>::default();
        core.command(Command::SetVolume(0.4), 0);
        assign(&mut core, 1, 1, 10, PlaybackMode::Paused);
        assert!(core.player.is_none());
        assert!(core.file.is_none());
        load(&mut core, 1, 10);
        let path = core.file.as_ref().unwrap().path().to_owned();
        assert_eq!(core.state.snapshot.volume.to_bits(), 0.4_f32.to_bits());
        core.command(Command::SetVolume(f32::NAN), 1);
        assert_eq!(core.state.snapshot.volume.to_bits(), 0.4_f32.to_bits());
        assign(&mut core, 2, 2, 10, PlaybackMode::Stopped);
        assert!(core.player.is_none());
        assert!(path.exists());
        assign(&mut core, 3, 3, 10, PlaybackMode::Playing);
        assert_eq!(core.state.snapshot.state, PlayerState::Playing);
        drop(core);
        assert!(!path.exists());
    }
}
