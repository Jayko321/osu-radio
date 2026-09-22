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
}

pub(crate) enum Command {
    Request {
        generation: u64,
        id: i32,
    },
    Loaded {
        generation: u64,
        id: i32,
        result: Result<NamedTempFile, String>,
    },
    Pause(Option<i32>),
    Resume(Option<i32>),
    Stop,
    Seek {
        expected_id: Option<i32>,
        position: Duration,
    },
    SetVolume(f32),
    Shutdown,
}
#[derive(Clone, Copy)]
pub(crate) struct Download {
    pub generation: u64,
    pub id: i32,
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
    ) -> std::io::Result<(Self, tokio::sync::mpsc::UnboundedReceiver<Download>)> {
        Self::spawn_engine::<Player>(emit)
    }
    #[cfg(test)]
    pub(crate) fn spawn_fake(
        emit: Arc<dyn Fn(Playback) + Send + Sync>,
    ) -> std::io::Result<(Self, tokio::sync::mpsc::UnboundedReceiver<Download>)> {
        Self::spawn_engine::<tests::FakeEngine>(emit)
    }
    fn spawn_engine<E: Engine + 'static>(
        emit: Arc<dyn Fn(Playback) + Send + Sync>,
    ) -> std::io::Result<(Self, tokio::sync::mpsc::UnboundedReceiver<Download>)> {
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
                                let _ = downloads.send(request);
                            }
                            drop(guard);
                            emit(core.state.clone());
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let previous = core.state.clone();
                            core.refresh();
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
    fn stop(&mut self) -> Result<(), String>;
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
    fn stop(&mut self) -> Result<(), String> {
        self.stop().map_err(|e| crate::describe(&e))
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
    state: Playback,
}
impl<E> Default for Core<E> {
    fn default() -> Self {
        Self {
            player: None,
            file: None,
            state: Playback::default(),
        }
    }
}
impl<E: Engine> Core<E> {
    fn command(&mut self, command: Command, generation: u64) -> Option<Download> {
        let result = match command {
            Command::Request {
                generation: candidate,
                id,
            } => {
                if candidate != generation {
                    return None;
                }
                self.state.error = None;
                if self.state.current_audio_id == Some(id) {
                    self.state.loading_audio_id = None;
                    self.player.as_mut().map_or(Ok(()), Engine::play)
                } else {
                    self.state.loading_audio_id = Some(id);
                    self.refresh();
                    return Some(Download { generation, id });
                }
            }
            Command::Loaded {
                generation: candidate,
                id,
                result,
            } => {
                if candidate != generation {
                    return None;
                }
                self.state.loading_audio_id = None;
                result.and_then(|file| self.load(id, file))
            }
            Command::Pause(expected_id) => {
                if !self.matches_current(expected_id) {
                    return None;
                }
                self.player.as_mut().map_or(Ok(()), Engine::pause)
            }
            Command::Resume(expected_id) => {
                // The controller already invalidated any pending download for explicit Play.
                self.state.loading_audio_id = None;
                if !self.matches_current(expected_id) {
                    return None;
                }
                self.player.as_mut().map_or(Ok(()), Engine::play)
            }
            Command::Stop => {
                self.state.loading_audio_id = None;
                self.player.as_mut().map_or(Ok(()), Engine::stop)
            }
            Command::Seek {
                expected_id,
                position,
            } => {
                if !self.matches_current(expected_id) {
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
        self.state.error = result.err();
        self.refresh();
        None
    }
    // UI state can lag behind a committed load. Commands remain bound to the selected source
    // they were issued for; an absent selection retains the public controller's global controls.
    fn matches_current(&self, expected_id: Option<i32>) -> bool {
        expected_id.is_none_or(|id| self.state.current_audio_id == Some(id))
    }
    fn load(&mut self, id: i32, file: NamedTempFile) -> Result<(), String> {
        if self.player.is_none() {
            let mut player = E::new()?;
            player.volume(self.state.snapshot.volume)?;
            self.player = Some(player);
        }
        if let Some(player) = &mut self.player {
            player.load(file.path())?;
            // Successful load has released the previous decoder before its file is removed.
            self.file = Some(file);
            self.state.current_audio_id = Some(id);
            player.play()?;
        }
        Ok(())
    }
    fn refresh(&mut self) {
        if let Some(player) = &self.player {
            self.state.snapshot = player.snapshot();
            if let Some(error) = player.output_error() {
                self.state.error = Some(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            self.snapshot.state = PlayerState::Paused;
            Ok(())
        }
        fn stop(&mut self) -> Result<(), String> {
            self.snapshot.state = PlayerState::Stopped;
            self.snapshot.position = Duration::ZERO;
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
    fn load(core: &mut Core<FakeEngine>, generation: u64, id: i32) {
        assert!(
            core.command(Command::Request { generation, id }, generation)
                .is_some()
        );
        core.command(
            Command::Loaded {
                generation,
                id,
                result: Ok(file(b"mp3")),
            },
            generation,
        );
    }
    #[test]
    fn switching_waits_for_success_and_download_or_decode_failure_preserves_current() {
        let mut core = Core::<FakeEngine>::default();
        load(&mut core, 1, 10);
        let original = core.file.as_ref().unwrap().path().to_owned();
        core.command(
            Command::Request {
                generation: 2,
                id: 20,
            },
            2,
        );
        assert_eq!(core.state.current_audio_id, Some(10));
        assert_eq!(core.state.snapshot.state, PlayerState::Playing);
        assert_eq!(core.state.loading_audio_id, Some(20));
        core.command(
            Command::Loaded {
                generation: 2,
                id: 20,
                result: Err("download failed".into()),
            },
            2,
        );
        assert_eq!(core.state.current_audio_id, Some(10));
        core.command(
            Command::Loaded {
                generation: 3,
                id: 20,
                result: Ok(file(b"bad")),
            },
            3,
        );
        assert_eq!(core.state.current_audio_id, Some(10));
        assert_eq!(core.state.snapshot.state, PlayerState::Playing);
        assert!(original.exists());
        load(&mut core, 4, 20);
        assert_eq!(core.state.current_audio_id, Some(20));
        assert!(!original.exists());
        let current = core.file.as_ref().unwrap().path().to_owned();
        drop(core);
        assert!(!current.exists());
    }
    #[test]
    fn stale_results_are_discarded_before_engine_creation_and_remove_temporary_files() {
        let mut core = Core::<FakeEngine>::default();
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
        assert!(core.player.is_none());
        assert!(!path.exists());
        load(&mut core, 2, 20);
        core.command(
            Command::Request {
                generation: 1,
                id: 10,
            },
            2,
        );
        assert_eq!(core.state.current_audio_id, Some(20));
        assert_eq!(core.state.loading_audio_id, None);
    }
    #[test]
    fn volume_is_lazy_and_persists_and_cached_play_does_not_download_again() {
        let mut core = Core::<FakeEngine>::default();
        core.command(Command::SetVolume(0.4), 0);
        assert!(core.player.is_none());
        core.command(Command::SetVolume(f32::NAN), 0);
        assert_eq!(core.state.snapshot.volume.to_bits(), 0.4_f32.to_bits());
        assert!(core.state.error.is_some());
        load(&mut core, 1, 10);
        assert_eq!(core.state.snapshot.volume.to_bits(), 0.4_f32.to_bits());
        core.command(Command::Pause(None), 1);
        core.command(
            Command::Seek {
                expected_id: None,
                position: Duration::from_secs(12),
            },
            1,
        );
        assert_eq!(core.state.snapshot.state, PlayerState::Paused);
        assert_eq!(core.state.snapshot.position, Duration::from_secs(12));
        assert!(
            core.command(
                Command::Request {
                    generation: 2,
                    id: 10
                },
                2
            )
            .is_none()
        );
        assert_eq!(core.player.as_ref().unwrap().loads, 1);
        assert_eq!(core.state.snapshot.state, PlayerState::Playing);
        core.command(Command::Stop, 3);
        assert_eq!(core.state.snapshot.position, Duration::ZERO);
        load(&mut core, 4, 20);
        assert_eq!(core.state.snapshot.volume.to_bits(), 0.4_f32.to_bits());
    }
    #[test]
    fn resume_clears_pending_track_and_stale_load_cannot_replace_resumed_audio() {
        let mut core = Core::<FakeEngine>::default();
        load(&mut core, 1, 10);
        core.command(Command::Pause(None), 1);
        core.command(
            Command::Request {
                generation: 2,
                id: 20,
            },
            2,
        );
        core.command(Command::Resume(None), 3);
        core.command(
            Command::Loaded {
                generation: 2,
                id: 20,
                result: Ok(file(b"mp3")),
            },
            3,
        );
        assert_eq!(core.state.current_audio_id, Some(10));
        assert_eq!(core.state.loading_audio_id, None);
        assert_eq!(core.state.snapshot.state, PlayerState::Playing);
    }
    #[test]
    fn stale_selected_track_controls_do_not_change_a_newly_committed_source() {
        let mut core = Core::<FakeEngine>::default();
        load(&mut core, 1, 10);
        load(&mut core, 2, 20);
        let playing = core.state.clone();
        core.command(Command::Pause(Some(10)), 2);
        core.command(
            Command::Seek {
                expected_id: Some(10),
                position: Duration::from_secs(12),
            },
            2,
        );
        assert_eq!(core.state, playing);
        core.command(Command::Pause(Some(20)), 2);
        assert_eq!(core.state.snapshot.state, PlayerState::Paused);
        let paused = core.state.clone();
        core.command(Command::Resume(Some(10)), 3);
        assert_eq!(core.state, paused);
        core.command(
            Command::Seek {
                expected_id: Some(20),
                position: Duration::from_secs(12),
            },
            3,
        );
        assert_eq!(core.state.snapshot.position, Duration::from_secs(12));
        core.command(Command::Resume(Some(20)), 4);
        assert_eq!(core.state.snapshot.state, PlayerState::Playing);
    }
    #[test]
    fn snapshot_refresh_surfaces_asynchronous_output_failure() {
        let mut core = Core::<FakeEngine>::default();
        load(&mut core, 1, 10);
        let player = core.player.as_mut().unwrap();
        player.snapshot.state = PlayerState::Stopped;
        player.output_error = Some("audio device disconnected".into());
        core.refresh();
        assert_eq!(core.state.snapshot.state, PlayerState::Stopped);
        assert_eq!(
            core.state.error.as_deref(),
            Some("audio device disconnected")
        );
    }
}
