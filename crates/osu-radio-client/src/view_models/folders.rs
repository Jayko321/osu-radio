#[cfg(test)]
use crate::controller::Controller;
use crate::{Loading, OsuFolder, models::FolderDiscoveryEvent};
use std::collections::{HashMap, VecDeque};
use tokio::task::AbortHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FolderAction {
    Add,
    Remove,
    Refresh,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderSelectionRow {
    pub kind: String,
    pub root_path: String,
    pub marker_path: String,
    pub registered_id: Option<i32>,
    pub pending: Option<FolderAction>,
    pub count: Loading<u64>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FolderSelection {
    pub open: bool,
    pub applying: bool,
    pub discovering: bool,
    pub picking: bool,
    pub rows: Vec<FolderSelectionRow>,
    pub message: String,
}
#[derive(Default)]
pub(crate) struct SelectionWork {
    pub state: FolderSelection,
    pub(crate) epoch: u64,
    pub(crate) scan: u64,
    pub(crate) scans: HashMap<u64, Scan>,
    pub(crate) queue: VecDeque<String>,
    pub(crate) counts: HashMap<String, AbortHandle>,
    pub(crate) changed: bool,
}
pub(crate) struct Scan {
    pub(crate) task: AbortHandle,
    pub(crate) scoped: bool,
    pub(crate) markers: Vec<String>,
}
pub(crate) enum FolderEvent {
    Candidate {
        epoch: u64,
        scan: u64,
        event: FolderDiscoveryEvent,
    },
    Discovered {
        epoch: u64,
        scan: u64,
        result: Result<(), String>,
    },
    Count {
        epoch: u64,
        path: String,
        result: Result<u64, String>,
    },
    Applied {
        epoch: u64,
        path: String,
        result: Result<Option<OsuFolder>, String>,
    },
    ApplyDone {
        epoch: u64,
    },
}
impl FolderEvent {
    pub(crate) const fn epoch(&self) -> u64 {
        match self {
            Self::Candidate { epoch, .. }
            | Self::Discovered { epoch, .. }
            | Self::Count { epoch, .. }
            | Self::Applied { epoch, .. }
            | Self::ApplyDone { epoch } => *epoch,
        }
    }
}

#[cfg(test)]
#[path = "folders_tests.rs"]
mod tests;
