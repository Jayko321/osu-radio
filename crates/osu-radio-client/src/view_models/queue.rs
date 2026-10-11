use super::Track;
use crate::controller::workflow::{Effect, Workflow};
use crate::controller::{AppUpdate, Completed};
use crate::describe;
use crate::models::PlaybackCommand;
#[cfg(test)]
use crate::models::{PlaybackAssignment, PlaybackMode};
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct QueueView {
    pub tracks: Vec<Track>,
    pub loading: bool,
    pub message: String,
}

#[derive(Default)]
pub struct QueueViewModel {
    pub(crate) view: QueueView,
    pub(crate) open: bool,
    pub(crate) request: u64,
    pub(crate) task: Option<tokio::task::AbortHandle>,
}

#[derive(Clone, Copy)]
pub(crate) enum PlaylistPlayback {
    Play(i32, Option<i32>),
    Append(i32),
}

#[derive(Clone)]
pub(crate) struct PendingPlayback {
    pub(crate) command: PlaybackCommand,
    pub(crate) playlist: Option<PlaylistPlayback>,
    pub(crate) expected_token: Option<u64>,
}

#[cfg(test)]
mod tests;

impl QueueViewModel {
    pub(crate) fn emit_queue(&self, _input: &QueueInput<'_>, work: &mut Workflow<'_>) {
        work.emit(AppUpdate::Queue(self.view.clone()));
    }
    pub(crate) fn set_queue_visible(
        &mut self,
        visible: bool,
        input: &QueueInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.open = visible;
        if visible {
            self.refresh_queue(input, work);
        } else {
            work.effect(Effect::QueueClosed);
            self.request = self.request.wrapping_add(1);
            if let Some(task) = self.task.take() {
                task.abort();
            }
            work.effect(Effect::ReconcileMedia);
        }
    }
    pub(crate) fn refresh_queue(&mut self, input: &QueueInput<'_>, work: &mut Workflow<'_>) {
        if !self.open {
            return;
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.request = self.request.wrapping_add(1);
        let request = self.request;
        let Some(api) = work.api.clone() else {
            self.view = QueueView {
                message: "Connect to the server to view the queue.".into(),
                ..QueueView::default()
            };
            self.emit_queue(input, work);
            return;
        };
        self.view.loading = true;
        self.view.message.clear();
        self.emit_queue(input, work);
        self.task = Some(work.task(async move {
            Completed::Queue {
                request,
                result: api.queue().await.map_err(|error| describe(&error)),
            }
        }));
    }
    pub(crate) fn queue_loaded(
        &mut self,
        request: u64,
        result: Result<crate::models::QueueState, String>,
        input: &QueueInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if request != self.request || !self.open {
            return;
        }
        self.task = None;
        self.view.loading = false;
        match result {
            Ok(queue) => {
                if input
                    .selection
                    .player
                    .assignment
                    .as_ref()
                    .is_some_and(|state| state.revision > queue.revision)
                {
                    self.refresh_queue(input, work);
                    return;
                }
                self.view.tracks = queue
                    .upcoming_tracks
                    .into_iter()
                    .map(|wire| {
                        let mut track = Track::from(wire);
                        track.apply_name_preferences(input.preferences);
                        track.duration = input
                            .durations
                            .get(&track.audio_source_id)
                            .copied()
                            .flatten()
                            .or_else(|| {
                                input
                                    .selection
                                    .track(track.audio_source_id)
                                    .and_then(|old| old.duration)
                            });
                        track
                    })
                    .collect();
                self.view.message.clear();
            }
            Err(error) => {
                self.view.tracks.clear();
                self.view.message = error;
            }
        }
        work.effect(Effect::QueueChanged);
        self.emit_queue(input, work);
    }
    #[must_use]
    pub const fn state(&self) -> &QueueView {
        &self.view
    }
}

pub(crate) struct QueueInput<'a> {
    pub(crate) selection: crate::controller::media::MediaInput<'a>,
    pub(crate) preferences: crate::TrackNamePreferences,
    pub(crate) durations: &'a std::collections::HashMap<i32, Option<Duration>>,
}

impl QueueViewModel {
    pub(crate) fn apply_preferences(&mut self, preferences: crate::TrackNamePreferences) {
        for track in &mut self.view.tracks {
            track.apply_name_preferences(preferences);
        }
    }
}

impl QueueViewModel {
    pub(crate) fn set_duration(&mut self, audio_id: i32, duration: Option<Duration>) {
        for track in &mut self.view.tracks {
            if track.audio_source_id == audio_id {
                track.duration = duration;
            }
        }
    }
}
