//! Request execution and ordered calls back to the single application coordinator.
use super::{AppUpdate, Completed};
use crate::ApiClient;
use std::future::Future;
use tokio::{
    sync::watch,
    task::{AbortHandle, JoinSet},
};

pub(crate) enum Effect {
    Update(AppUpdate),
    DurationChanged {
        audio_id: i32,
        duration: Option<std::time::Duration>,
    },
    Connect,
    RefreshLibrary,
    RefreshPlaylists,
    StartPlaybackStream,
    Assignment(crate::models::PlaybackAssignment),
    CurrentVolume,
    TrackVolumeChanged {
        audio_id: i32,
        percent: Option<u8>,
    },
    QueueClosed,
    QueueChanged,
    ReconcileMedia,
    DeferAssignment(crate::models::PlaybackAssignment),
    AssignmentHistory,
    SelectPlayingTrack {
        audio_id: Option<i32>,
        playlist_item_id: Option<i32>,
    },
    RememberDuration {
        audio_id: i32,
        duration: Option<std::time::Duration>,
    },
    ResetDiscord,
    UpdateDiscord,
    RefreshQueue,
    CancelAudio,
    AssignWorker,
    Download(crate::playback::Download),
    PlayerCommand(crate::playback::Command),
    RefreshOpenPlaylist,
    Selection,
    FinishVolume,
    AdvanceMedia(bool),
    PlaylistPlayback(crate::view_models::queue::PlaylistPlayback),
    TogglePlayback,
    RememberDates(Vec<(i32, i64)>),
}
pub(crate) struct Workflow<'a> {
    pub(crate) api: Option<ApiClient>,
    pub(crate) tasks: &'a mut JoinSet<Completed>,
    pub(crate) cancel: &'a watch::Sender<bool>,
    pub(crate) effects: Vec<Effect>,
}
impl Workflow<'_> {
    pub(crate) fn emit(&mut self, update: AppUpdate) {
        self.effects.push(Effect::Update(update));
    }
    pub(crate) fn effect(&mut self, effect: Effect) {
        self.effects.push(effect);
    }
    pub(crate) fn task(
        &mut self,
        future: impl Future<Output = Completed> + Send + 'static,
    ) -> AbortHandle {
        let mut cancel = self.cancel.subscribe();
        self.tasks.spawn(async move {
            tokio::select! {
                result = future => result,
                _ = cancel.wait_for(|value| *value) => Completed::Cancelled,
            }
        })
    }
}
