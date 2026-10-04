use super::{AppUpdate, Completed, Controller, Duration, Track, describe};
use crate::models::{PlaybackAssignment, PlaybackCommand, PlaybackMode};

#[derive(Clone, Debug, Default)]
pub struct QueueView {
    pub tracks: Vec<Track>,
    pub loading: bool,
    pub message: String,
}

#[derive(Default)]
pub(super) struct QueueWork {
    pub(super) view: QueueView,
    pub(super) open: bool,
    request: u64,
    task: Option<tokio::task::AbortHandle>,
}

#[derive(Clone, Copy)]
pub(super) enum PlaylistPlayback {
    Play(i32, Option<i32>),
    Append(i32),
}

#[derive(Clone)]
pub(super) struct PendingPlayback {
    pub(super) command: PlaybackCommand,
    pub(super) playlist: Option<PlaylistPlayback>,
    expected_token: Option<u64>,
}
impl Controller {
    pub(super) fn emit_queue(&self) {
        (self.emit)(AppUpdate::Queue(self.upcoming.view.clone()));
    }
    pub(super) fn set_queue_visible(&mut self, visible: bool) {
        self.upcoming.open = visible;
        if visible {
            self.refresh_queue();
        } else {
            self.queue_media.clear();
            self.upcoming.request = self.upcoming.request.wrapping_add(1);
            if let Some(task) = self.upcoming.task.take() {
                task.abort();
            }
            self.reconcile_media();
        }
    }
    pub(super) fn refresh_queue(&mut self) {
        if !self.upcoming.open {
            return;
        }
        if let Some(task) = self.upcoming.task.take() {
            task.abort();
        }
        self.upcoming.request = self.upcoming.request.wrapping_add(1);
        let request = self.upcoming.request;
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            self.upcoming.view = QueueView {
                message: "Connect to the server to view the queue.".into(),
                ..QueueView::default()
            };
            self.emit_queue();
            return;
        };
        self.upcoming.view.loading = true;
        self.upcoming.view.message.clear();
        self.emit_queue();
        self.upcoming.task = Some(self.task(async move {
            Completed::Queue {
                request,
                result: api.queue().await.map_err(|error| describe(&error)),
            }
        }));
    }
    pub(super) fn queue_loaded(
        &mut self,
        request: u64,
        result: Result<crate::models::QueueState, String>,
    ) {
        if request != self.upcoming.request || !self.upcoming.open {
            return;
        }
        self.upcoming.task = None;
        self.upcoming.view.loading = false;
        match result {
            Ok(queue) => {
                if self
                    .assignment
                    .as_ref()
                    .is_some_and(|state| state.revision > queue.revision)
                {
                    self.refresh_queue();
                    return;
                }
                self.upcoming.view.tracks = queue
                    .upcoming_tracks
                    .into_iter()
                    .map(|wire| {
                        let mut track = Track::from(wire);
                        track.apply_name_preferences(self.name_preferences);
                        track.duration = self
                            .durations
                            .get(&track.audio_source_id)
                            .copied()
                            .flatten()
                            .or_else(|| {
                                self.track(track.audio_source_id)
                                    .and_then(|old| old.duration)
                            });
                        track
                    })
                    .collect();
                self.upcoming.view.message.clear();
            }
            Err(error) => {
                self.upcoming.view.tracks.clear();
                self.upcoming.view.message = error;
            }
        }
        self.queue_media.retain(|id, _| {
            self.upcoming
                .view
                .tracks
                .iter()
                .any(|track| track.audio_source_id == *id)
        });
        self.reconcile_media();
        self.emit_queue();
    }
    pub(super) fn queue_playback(&mut self, command: PlaybackCommand) {
        if self.session.is_none() {
            self.playback_error("Connect to the server before playing a track.".into());
            return;
        }
        self.playback_commands.push_back(PendingPlayback {
            command,
            playlist: None,
            expected_token: None,
        });
    }
    pub(super) fn queue_playlist_play(&mut self, id: i32, start: Option<i32>) {
        self.queue_playlist(PlaylistPlayback::Play(id, start));
    }
    pub(super) fn queue_playlist(&mut self, playlist: PlaylistPlayback) {
        if self.session.is_none() {
            self.playback_error("Connect to the server to queue a playlist.".into());
            return;
        }
        self.playback_commands.push_back(PendingPlayback {
            command: PlaybackCommand::Play {
                audio_source_id: None,
            },
            playlist: Some(playlist),
            expected_token: None,
        });
    }
    pub(super) fn start_playback_command(&mut self) {
        if self.playback_command_busy || self.volume.settings.is_none() {
            return;
        }
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            return;
        };
        for _ in 0..self.playback_commands.len() {
            let Some(pending) = self.playback_commands.pop_front() else {
                break;
            };
            if pending.expected_token.is_some_and(|token| {
                self.assignment
                    .as_ref()
                    .is_none_or(|state| state.playback_token != token)
            }) {
                continue;
            }
            if matches!(pending.command, PlaybackCommand::Finished { playback_token } | PlaybackCommand::Failed { playback_token } if self.pending_start == Some(playback_token))
            {
                // Finish may follow an immediate EOF; retain its start through callback retries.
                self.playback_commands.push_back(pending);
                continue;
            }
            self.playback_command_busy = true;
            self.task(async move {
                let result = match pending.playlist {
                    Some(PlaylistPlayback::Play(id, start)) => api.play_playlist(id, start).await,
                    Some(PlaylistPlayback::Append(id)) => {
                        async {
                            let playlist = api.playlist(id).await?;
                            let ids: Vec<_> = playlist
                                .items
                                .iter()
                                .filter_map(|item| item.audio_source_id)
                                .collect();
                            if ids.is_empty() {
                                return Err(crate::api::ApiError::Protocol(
                                    "Playlist has no available tracks.".into(),
                                ));
                            }
                            api.append_queue(&ids).await
                        }
                        .await
                    }
                    None => api.playback_command(&pending.command).await,
                }
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
            Ok(assignment) => {
                if matches!(pending.command, PlaybackCommand::Started { playback_token } if self.pending_start == Some(playback_token))
                {
                    self.pending_start = None;
                }
                self.apply_assignment(assignment);
            }
            Err(error) => {
                self.playback_error(error);
                // Completion callbacks are token-bound and idempotent, so retrying cannot skip twice.
                if matches!(
                    pending.command,
                    PlaybackCommand::Finished { .. }
                        | PlaybackCommand::Failed { .. }
                        | PlaybackCommand::Started { .. }
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
        if self.volume.settings.is_none() {
            if self
                .volume
                .deferred_assignment
                .as_ref()
                .is_none_or(|old| assignment.revision > old.revision)
            {
                self.volume.deferred_assignment = Some(assignment);
            }
            return;
        }
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
            track.apply_name_preferences(self.name_preferences);
            track.volume_percent = assignment.volume_percent.or(track.volume_percent);
            self.restore_durations(std::slice::from_mut(&mut track));
            track.duration = assignment
                .duration_ms
                .map(Duration::from_millis)
                .or(track.duration);
            track
        });
        if let Some(mut track) = self.current_track.take() {
            self.merge_volume_overrides(std::slice::from_mut(&mut track));
            self.current_track = Some(track);
        }
        self.assignment_last_played();
        if restart && assignment.mode != PlaybackMode::Stopped && self.current_track.is_some() {
            if self.playlists.view.showing_detail() {
                if self.playlists.view.active.as_ref().is_some_and(|playlist| {
                    playlist
                        .items
                        .iter()
                        .any(|item| Some(item.id) == assignment.current_playlist_item_id)
                }) {
                    self.playlists.view.selected_item_id = assignment.current_playlist_item_id;
                    self.emit_playlists();
                }
            } else {
                self.selected = assignment.current_audio_source_id;
            }
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
            self.durations.insert(
                id,
                self.current_track.as_ref().and_then(|track| track.duration),
            );
        }
        self.assignment = Some(assignment);
        self.refresh_queue();
        if restart {
            self.finish_volume_editing();
            self.pending_start = None;
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
                    volume: self.assignment_volume(),
                });
            }
        } else if mode == PlaybackMode::Playing {
            self.playback_commands.push_back(PendingPlayback {
                command: PlaybackCommand::PauseIfCurrent {
                    playback_token: token,
                },
                playlist: None,
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
        playback.current_playlist_item_id = self
            .assignment
            .as_ref()
            .and_then(|state| state.current_playlist_item_id);
        (self.emit)(AppUpdate::Playback(playback));
    }
    fn playback_error(&self, error: String) {
        let assignment = self.assignment.as_ref();
        let mut playback = self.worker_state.clone();
        playback.current_audio_id = assignment.and_then(|state| state.current_audio_source_id);
        playback.current_playlist_item_id =
            assignment.and_then(|state| state.current_playlist_item_id);
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
            Message::Started(token) => (
                PlaybackCommand::Started {
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
            if matches!(command, PlaybackCommand::Started { .. }) {
                self.pending_start = Some(token);
            }
            self.playback_commands.push_back(PendingPlayback {
                command,
                playlist: None,
                expected_token: Some(token),
            });
        }
    }
}

#[cfg(test)]
mod tests;
