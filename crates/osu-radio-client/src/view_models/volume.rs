use crate::models::{AudioSettings, PlaybackAssignment};
use std::collections::{HashMap, VecDeque};
#[cfg(test)]
use std::time::Duration;
use tokio::{task::JoinHandle, time::Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)] // Independent load/save and selected-track flags for adapters.
pub struct AudioSettingsState {
    pub settings: AudioSettings,
    pub loaded: bool,
    pub loading: bool,
    pub saving: bool,
    pub error: String,
    pub selected_audio_id: Option<i32>,
    pub selected_volume_percent: u8,
    pub selected_has_override: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Settings(AudioSettings),
    Track(i32, Option<u8>),
}
impl Change {
    pub(crate) fn same_target(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Settings(_), Self::Settings(_)) => true,
            (Self::Track(a, _), Self::Track(b, _)) => a == b,
            _ => false,
        }
    }
}
#[derive(Clone)]
pub(crate) struct Save {
    pub(crate) change: Change,
    pub(crate) deadline: Instant,
}

#[derive(Default)]
pub(crate) struct VolumeWork {
    pub settings: Option<AudioSettings>,
    pub(crate) loading: bool,
    pub(crate) error: String,
    // Local values win over reads started before the edit, including a pending DELETE (None).
    pub(crate) overrides: HashMap<i32, Option<u8>>,
    pub(crate) pending: VecDeque<Save>,
    pub(crate) saving: Option<Save>,
    pub save_task: Option<JoinHandle<Result<(), String>>>,
    pub(crate) stream_started: bool,
    pub deferred_assignment: Option<PlaybackAssignment>,
}

#[cfg(test)]
mod tests;
