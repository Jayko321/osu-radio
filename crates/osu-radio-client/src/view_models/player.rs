//! Selected/current presentation and the common Play/Pause/Resume rules.
use super::{
    Track,
    queue::{PendingPlayback, PlaylistPlayback},
};
use crate::controller::workflow::{Effect, Workflow};
use crate::controller::{AppUpdate, Completed};
use crate::describe;
use crate::{
    models::{PlaybackAssignment, PlaybackCommand, PlaybackMode},
    playback::{Playback, PlayerState},
};
use std::{collections::VecDeque, time::Duration};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Independent presentation and transport availability flags.
pub struct PlayerPresentation {
    pub selected_track: Option<Track>,
    pub selected_playlist_item_id: Option<i32>,
    pub selected_is_playing: bool,
    pub can_play: bool,
    pub can_seek: bool,
    pub can_next: bool,
    pub can_previous: bool,
    pub position: Duration,
    pub duration: Option<Duration>,
    pub message: String,
}
#[derive(Default)]
pub struct PlayerViewModel {
    pub(crate) current_track: Option<Track>,
    pub(crate) selected_track: Option<Track>,
    pub(crate) assignment: Option<PlaybackAssignment>,
    pub(crate) commands: VecDeque<PendingPlayback>,
    pub(crate) command_busy: bool,
    pub(crate) pending_start: Option<u64>,
    pub(crate) playback: Playback,
    pub(crate) transport_error: Option<String>,
}
pub(crate) enum PrimaryAction {
    Playback(PlaybackCommand),
    Playlist(PlaylistPlayback),
}
impl PlayerViewModel {
    pub(crate) fn merge_current_last_played(&mut self, library: &mut super::LibraryViewModel) {
        if let Some(track) = &mut self.current_track {
            library.merge_last_played(std::slice::from_mut(track));
        }
    }
    #[must_use]
    pub const fn selected_track(&self) -> Option<&Track> {
        self.selected_track.as_ref()
    }
    #[must_use]
    pub const fn playback(&self) -> &Playback {
        &self.playback
    }
    pub(crate) fn primary_action(
        &self,
        selected: Option<&Track>,
        playlist: Option<(i32, i32)>,
    ) -> Option<PrimaryAction> {
        let selected = selected.filter(|track| track.audio_source_id >= 0)?;
        let current = if let Some((_, item)) = playlist {
            self.assignment
                .as_ref()
                .is_some_and(|assignment| assignment.current_playlist_item_id == Some(item))
        } else {
            self.playback.current_audio_id == Some(selected.audio_source_id)
        };
        if current {
            let playing = if playlist.is_some() {
                self.assignment
                    .as_ref()
                    .is_some_and(|assignment| assignment.mode == PlaybackMode::Playing)
            } else {
                self.playback.snapshot.state == PlayerState::Playing
            };
            Some(PrimaryAction::Playback(if playing {
                PlaybackCommand::Pause
            } else {
                PlaybackCommand::Play {
                    audio_source_id: None,
                }
            }))
        } else if let Some((id, item)) = playlist {
            Some(PrimaryAction::Playlist(PlaylistPlayback::Play(
                id,
                Some(item),
            )))
        } else {
            Some(PrimaryAction::Playback(PlaybackCommand::Play {
                audio_source_id: Some(selected.audio_source_id),
            }))
        }
    }

    #[must_use]
    pub fn presentation(
        &self,
        selected_item_id: Option<i32>,
        selection_available: bool,
        connected: bool,
        audio_settings_loaded: bool,
    ) -> PlayerPresentation {
        let selected = self.selected_track.as_ref();
        let selected_current = selection_available
            && selected.is_some_and(|track| {
                self.playback.current_audio_id == Some(track.audio_source_id)
                    && selected_item_id
                        .is_none_or(|item| self.playback.current_playlist_item_id == Some(item))
            });
        let duration = selected_current
            .then(|| {
                self.playback
                    .snapshot
                    .duration
                    .or_else(|| selected.and_then(|track| track.duration))
            })
            .flatten();
        PlayerPresentation {
            selected_track: self.selected_track.clone(),
            selected_playlist_item_id: selected_item_id,
            selected_is_playing: selected_current
                && self.playback.snapshot.state == PlayerState::Playing,
            can_play: connected
                && audio_settings_loaded
                && selection_available
                && selected.is_some_and(|track| track.audio_source_id >= 0),
            can_seek: selected_current
                && self.playback.has_source
                && duration.is_some_and(|duration| !duration.is_zero()),
            can_next: self.playback.can_next,
            can_previous: self.playback.can_previous,
            position: if selected_current {
                self.playback.snapshot.position
            } else {
                Duration::ZERO
            },
            duration,
            message: self.playback.error.clone().unwrap_or_else(|| {
                if self.playback.loading_audio_id.is_some() {
                    "Loading audio…".into()
                } else {
                    String::new()
                }
            }),
        }
    }
    pub(crate) fn accept_worker(&mut self, mut playback: Playback) -> Option<Playback> {
        if let Some(assignment) = &self.assignment {
            if playback.playback_token != assignment.playback_token {
                return None;
            }
            playback.can_next = assignment.can_next;
            playback.can_previous = assignment.can_previous;
            playback.current_playlist_item_id = assignment.current_playlist_item_id;
        }
        self.playback = playback.clone();
        Some(playback)
    }
}

impl PlayerViewModel {
    pub(crate) fn queue_playback(
        &mut self,
        command: PlaybackCommand,
        input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if work.api.is_none() {
            self.playback_error(
                "Connect to the server before playing a track.".into(),
                input,
                work,
            );
            return;
        }
        self.commands.push_back(PendingPlayback {
            command,
            playlist: None,
            expected_token: None,
        });
    }

    pub(crate) fn queue_playlist(
        &mut self,
        playlist: PlaylistPlayback,
        input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if work.api.is_none() {
            self.playback_error(
                "Connect to the server to queue a playlist.".into(),
                input,
                work,
            );
            return;
        }
        self.commands.push_back(PendingPlayback {
            command: PlaybackCommand::Play {
                audio_source_id: None,
            },
            playlist: Some(playlist),
            expected_token: None,
        });
    }
    pub(crate) fn start_playback_command(
        &mut self,
        input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if self.command_busy || input.settings.volume.settings.is_none() {
            return;
        }
        let Some(api) = work.api.clone() else {
            return;
        };
        for _ in 0..self.commands.len() {
            let Some(pending) = self.commands.pop_front() else {
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
                self.commands.push_back(pending);
                continue;
            }
            self.command_busy = true;
            work.task(async move {
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
    pub(crate) fn playback_command_completed(
        &mut self,
        pending: PendingPlayback,
        result: Result<PlaybackAssignment, String>,
        input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.command_busy = false;
        match result {
            Ok(assignment) => {
                if matches!(pending.command, PlaybackCommand::Started { playback_token } if self.pending_start == Some(playback_token))
                {
                    self.pending_start = None;
                }
                self.apply_assignment(assignment, input, work);
            }
            Err(error) => {
                self.playback_error(error, input, work);
                // Completion callbacks are token-bound and idempotent, so retrying cannot skip twice.
                if matches!(
                    pending.command,
                    PlaybackCommand::Finished { .. }
                        | PlaybackCommand::Failed { .. }
                        | PlaybackCommand::Started { .. }
                        | PlaybackCommand::PauseIfCurrent { .. }
                ) {
                    work.task(async move {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        Completed::PlaybackRetry(pending)
                    });
                }
            }
        }
    }
    pub(crate) fn start_playback_stream(input: &PlayerInput<'_>, work: &mut Workflow<'_>) {
        let Some(api) = work.api.clone() else {
            return;
        };
        let sender = input.assignment_sender.clone();
        work.task(async move {
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
    pub(crate) fn assignment_result(
        &mut self,
        result: Result<PlaybackAssignment, String>,
        input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        match result {
            Ok(assignment) => self.apply_assignment(assignment, input, work),
            Err(error) => self.playback_error(error, input, work),
        }
    }
    pub(crate) fn apply_assignment(
        &mut self,
        assignment: PlaybackAssignment,
        input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if input.settings.volume.settings.is_none() {
            work.effect(Effect::DeferAssignment(assignment));
            return;
        }
        if let Some(current) = &self.assignment
            && current.revision >= assignment.revision
        {
            if current.revision == assignment.revision
                && current.playback_token == assignment.playback_token
            {
                // The worker owns decoder/device errors; a healthy sync clears only transport errors.
                self.worker_update(self.playback.clone(), input, work);
            }
            return;
        }
        let restart = self.assignment.as_ref().is_none_or(|current| {
            current.playback_token != assignment.playback_token
                || current.current_audio_source_id != assignment.current_audio_source_id
        });
        self.current_track = assignment.track.clone().map(|track| {
            let mut track = Track::from(track);
            track.apply_name_preferences(input.settings.preferences.name_preferences);
            track.volume_percent = assignment.volume_percent.or(track.volume_percent);
            input
                .media
                .restore_durations(std::slice::from_mut(&mut track));
            track.duration = assignment
                .duration_ms
                .map(Duration::from_millis)
                .or(track.duration);
            track
        });
        if let Some(mut track) = self.current_track.take() {
            input
                .settings
                .merge_volume_overrides(std::slice::from_mut(&mut track));
            self.current_track = Some(track);
        }
        work.effect(Effect::AssignmentHistory);
        if restart && assignment.mode != PlaybackMode::Stopped && self.current_track.is_some() {
            work.effect(Effect::SelectPlayingTrack {
                audio_id: assignment.current_audio_source_id,
                playlist_item_id: assignment.current_playlist_item_id,
            });
        }
        let id = assignment.current_audio_source_id;
        if self
            .current_track
            .as_ref()
            .is_some_and(|track| track.duration.is_some())
            && let Some(id) = id
        {
            work.effect(Effect::RememberDuration {
                audio_id: id,
                duration: self.current_track.as_ref().and_then(|track| track.duration),
            });
        }
        self.assignment = Some(assignment);
        // A new assignment cannot borrow the old local track's Playing state.
        if restart {
            work.effect(Effect::ResetDiscord);
        }
        work.effect(Effect::RefreshQueue);
        if restart {
            work.effect(Effect::FinishVolume);
            self.pending_start = None;
            work.effect(Effect::CancelAudio);
        }
        work.effect(Effect::AssignWorker);
        work.effect(Effect::Selection);
    }
    pub(crate) fn worker_update(
        &mut self,
        playback: crate::playback::Playback,
        input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let Some(playback) = self.accept_worker(playback) else {
            return;
        };
        self.transport_error = None;
        work.effect(Effect::UpdateDiscord);
        work.emit(AppUpdate::Playback(playback));
        self.emit_presentation(input, work);
    }
    pub(crate) fn worker_start_failed(
        &mut self,
        error: String,
        input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.playback = Playback {
            error: Some(error),
            ..Default::default()
        };
        self.transport_error = None;
        work.emit(AppUpdate::Playback(self.playback.clone()));
        self.emit_presentation(input, work);
    }
    pub(crate) fn playback_error(
        &mut self,
        error: String,
        input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let assignment = self.assignment.as_ref();
        let mut playback = self.playback.clone();
        playback.current_audio_id = assignment.and_then(|state| state.current_audio_source_id);
        playback.current_playlist_item_id =
            assignment.and_then(|state| state.current_playlist_item_id);
        playback.playback_token = assignment.map_or(0, |state| state.playback_token);
        playback.can_next = assignment.is_some_and(|state| state.can_next);
        playback.can_previous = assignment.is_some_and(|state| state.can_previous);
        playback.error = Some(error.clone());
        self.transport_error = Some(error);
        work.emit(AppUpdate::Playback(playback));
        self.emit_presentation(input, work);
    }
    pub(crate) fn worker_message(
        &mut self,
        message: crate::playback::Message,
        _input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        use crate::playback::Message;
        let (command, token) = match message {
            Message::Download(download) => {
                work.effect(Effect::Download(download));
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
            self.commands.push_back(PendingPlayback {
                command,
                playlist: None,
                expected_token: Some(token),
            });
        }
    }
    pub(crate) fn seek_selected(
        &self,
        position: Duration,
        input: &PlayerInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if input.playlists.view.showing_detail()
            && self.assignment.as_ref().is_none_or(|state| {
                state.current_playlist_item_id != input.playlists.view.selected_item_id
            })
        {
            return;
        }
        let expected_id = if input.playlists.view.showing_detail() {
            input
                .playlists
                .view
                .selected_item()
                .and_then(|item| item.audio_source_id)
        } else {
            input.library.selected
        };
        work.effect(Effect::PlayerCommand(crate::playback::Command::Seek {
            expected_id,
            playback_token: self
                .assignment
                .as_ref()
                .map_or(0, |state| state.playback_token),
            position,
        }));
    }
}

pub(crate) struct PlayerInput<'a> {
    pub(crate) library: &'a super::LibraryViewModel,
    pub(crate) playlists: &'a super::PlaylistsViewModel,
    pub(crate) settings: &'a super::SettingsViewModel,
    pub(crate) media: &'a crate::controller::media::MediaScheduler,
    pub(crate) assignment_sender:
        &'a tokio::sync::mpsc::UnboundedSender<Result<PlaybackAssignment, String>>,
}
impl PlayerViewModel {
    fn emit_presentation(&self, input: &PlayerInput<'_>, work: &mut Workflow<'_>) {
        let detail = input.playlists.view.showing_detail();
        let mut presentation = self.presentation(
            detail
                .then_some(input.playlists.view.selected_item_id)
                .flatten(),
            !detail || input.playlists.view.selected_item().is_some(),
            work.api.is_some(),
            input.settings.volume.settings.is_some(),
        );
        if let Some(error) = &self.transport_error {
            presentation.message.clone_from(error);
        }
        work.emit(AppUpdate::Player(presentation));
    }
}

impl PlayerViewModel {
    pub(crate) fn apply_preferences(&mut self, preferences: crate::TrackNamePreferences) {
        for track in self
            .current_track
            .iter_mut()
            .chain(&mut self.selected_track)
        {
            track.apply_name_preferences(preferences);
        }
    }
    pub(crate) fn select_track(&mut self, track: Option<Track>) {
        self.selected_track = track;
    }
    pub(crate) fn device_failed(&mut self, token: u64) {
        self.commands.push_back(PendingPlayback {
            command: PlaybackCommand::PauseIfCurrent {
                playback_token: token,
            },
            playlist: None,
            expected_token: Some(token),
        });
    }
}

impl PlayerViewModel {
    pub(crate) fn toggle_playback(&mut self, input: &PlayerInput<'_>, work: &mut Workflow<'_>) {
        if work.api.is_none() || input.settings.volume.settings.is_none() {
            return;
        }
        let detail = input.playlists.view.showing_detail();
        let selected = if detail {
            input.playlists.selected_track()
        } else {
            self.selected_track.as_ref()
        };
        let playlist = if detail {
            let Some(item) = input.playlists.view.selected_item() else {
                return;
            };
            Some((item.playlist_id, item.id))
        } else {
            None
        };
        match self.primary_action(selected, playlist) {
            Some(PrimaryAction::Playback(command)) => self.queue_playback(command, input, work),
            Some(PrimaryAction::Playlist(playlist)) => self.queue_playlist(playlist, input, work),
            None => {}
        }
    }
}

impl PlayerViewModel {
    pub(crate) fn set_duration(&mut self, audio_id: i32, duration: Option<Duration>) {
        for track in self
            .current_track
            .iter_mut()
            .chain(&mut self.selected_track)
        {
            if track.audio_source_id == audio_id {
                track.duration = duration;
            }
        }
    }
    pub(crate) const fn set_track_volume(&mut self, audio_id: i32, percent: Option<u8>) {
        if let Some(track) = &mut self.current_track
            && track.audio_source_id == audio_id
        {
            track.volume_percent = percent;
        }
    }
}
