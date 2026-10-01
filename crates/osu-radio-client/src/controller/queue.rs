use super::{AppUpdate, Completed, Controller, Duration, Track, describe};
use crate::models::{PlaybackAssignment, PlaybackCommand, PlaybackMode};

#[derive(Clone)]
pub(super) struct PendingPlayback {
    command: PlaybackCommand,
    expected_token: Option<u64>,
}
impl Controller {
    pub(super) fn queue_playback(&mut self, command: PlaybackCommand) {
        if self.session.is_none() {
            self.playback_error("Connect to the server before playing a track.".into());
            return;
        }
        self.playback_commands.push_back(PendingPlayback {
            command,
            expected_token: None,
        });
    }
    pub(super) fn start_playback_command(&mut self) {
        if self.playback_command_busy {
            return;
        }
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            return;
        };
        while let Some(pending) = self.playback_commands.pop_front() {
            if pending.expected_token.is_some_and(|token| {
                self.assignment
                    .as_ref()
                    .is_none_or(|state| state.playback_token != token)
            }) {
                continue;
            }
            self.playback_command_busy = true;
            self.task(async move {
                let result = api
                    .playback_command(&pending.command)
                    .await
                    .map_err(|error| describe(&error));
                Completed::PlaybackCommand { pending, result }
            });
            break;
        }
    }
    pub(super) fn playback_command_completed(
        &mut self,
        pending: PendingPlayback,
        result: Result<PlaybackAssignment, String>,
    ) {
        self.playback_command_busy = false;
        match result {
            Ok(assignment) => self.apply_assignment(assignment),
            Err(error) => {
                self.playback_error(error);
                // Completion callbacks are token-bound and idempotent, so retrying cannot skip twice.
                if matches!(
                    pending.command,
                    PlaybackCommand::Finished { .. }
                        | PlaybackCommand::Failed { .. }
                        | PlaybackCommand::PauseIfCurrent { .. }
                ) {
                    self.task(async move {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        Completed::PlaybackRetry(pending)
                    });
                }
            }
        }
    }
    pub(super) fn start_playback_stream(&mut self) {
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            return;
        };
        let sender = self.assignment_sender.clone();
        self.task(async move {
            loop {
                // Re-read before each reconnect. Equal revisions must not restart a source.
                let snapshot = api.playback().await.map_err(|error| describe(&error));
                let _ = sender.send(snapshot);
                let stream_sender = sender.clone();
                let result = api
                    .playback_events(move |assignment| {
                        let _ = stream_sender.send(Ok(assignment));
                    })
                    .await;
                if let Err(error) = result {
                    let _ = sender.send(Err(describe(&error)));
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        });
    }
    pub(super) fn assignment_result(&mut self, result: Result<PlaybackAssignment, String>) {
        match result {
            Ok(assignment) => self.apply_assignment(assignment),
            Err(error) => self.playback_error(error),
        }
    }
    pub(super) fn apply_assignment(&mut self, assignment: PlaybackAssignment) {
        if let Some(current) = &self.assignment
            && current.revision >= assignment.revision
        {
            if current.revision == assignment.revision
                && current.playback_token == assignment.playback_token
            {
                // The worker owns decoder/device errors; a healthy sync clears only transport errors.
                self.worker_update(self.worker_state.clone());
            }
            return;
        }
        let restart = self.assignment.as_ref().is_none_or(|current| {
            current.playback_token != assignment.playback_token
                || current.current_audio_source_id != assignment.current_audio_source_id
        });
        self.current_track = assignment.track.clone().map(|track| {
            let mut track = Track::from(track);
            track.duration = assignment.duration_ms.map(Duration::from_millis);
            track
        });
        if restart && assignment.mode != PlaybackMode::Stopped && self.current_track.is_some() {
            self.selected = assignment.current_audio_source_id;
        }
        let id = assignment.current_audio_source_id;
        let token = assignment.playback_token;
        let mode = assignment.mode;
        if self
            .current_track
            .as_ref()
            .is_some_and(|track| track.duration.is_some())
            && let Some(id) = id
        {
            self.duration_done.insert(id);
        }
        self.assignment = Some(assignment);
        if restart {
            self.cancel_audio();
        }
        if self.ensure_playback() {
            if let Some(worker) = &self.playback {
                let generation = worker.generation();
                worker.send(crate::playback::Command::Assign {
                    generation,
                    id,
                    playback_token: token,
                    mode,
                });
            }
        } else if mode == PlaybackMode::Playing {
            self.playback_commands.push_back(PendingPlayback {
                command: PlaybackCommand::PauseIfCurrent {
                    playback_token: token,
                },
                expected_token: Some(token),
            });
        }
        self.emit_selection();
    }
    pub(super) fn worker_update(&mut self, mut playback: crate::playback::Playback) {
        if let Some(assignment) = &self.assignment {
            if playback.playback_token != assignment.playback_token {
                return;
            }
            playback.can_next = assignment.can_next;
            playback.can_previous = assignment.can_previous;
        }
        self.worker_state = playback.clone();
        (self.emit)(AppUpdate::Playback(playback));
    }
    fn playback_error(&self, error: String) {
        let assignment = self.assignment.as_ref();
        let mut playback = self.worker_state.clone();
        playback.current_audio_id = assignment.and_then(|state| state.current_audio_source_id);
        playback.playback_token = assignment.map_or(0, |state| state.playback_token);
        playback.can_next = assignment.is_some_and(|state| state.can_next);
        playback.can_previous = assignment.is_some_and(|state| state.can_previous);
        playback.error = Some(error);
        (self.emit)(AppUpdate::Playback(playback));
    }
    pub(super) fn worker_message(&mut self, message: crate::playback::Message) {
        use crate::playback::Message;
        let (command, token) = match message {
            Message::Download(download) => {
                self.download(download);
                return;
            }
            Message::Finished(token) => (
                PlaybackCommand::Finished {
                    playback_token: token,
                },
                token,
            ),
            Message::Failed(token) => (
                PlaybackCommand::Failed {
                    playback_token: token,
                },
                token,
            ),
            Message::DeviceFailure(token) => {
                if self
                    .assignment
                    .as_ref()
                    .is_none_or(|state| state.mode != PlaybackMode::Playing)
                {
                    return;
                }
                (
                    PlaybackCommand::PauseIfCurrent {
                        playback_token: token,
                    },
                    token,
                )
            }
        };
        if self
            .assignment
            .as_ref()
            .is_some_and(|state| state.playback_token == token)
        {
            self.playback_commands.push_back(PendingPlayback {
                command,
                expected_token: Some(token),
            });
        }
    }
}

#[cfg(test)]
mod tests;
