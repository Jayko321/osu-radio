use super::{AppUpdate, Completed, Controller};
use crate::{
    Loading, OsuFolder, describe,
    models::{DiscoverFolders, DiscoveryDepth, FolderDiscoveryEvent},
};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
};
use tokio::task::AbortHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FolderAction {
    Add,
    Remove,
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
pub(super) struct SelectionWork {
    pub state: FolderSelection,
    epoch: u64,
    scan: u64,
    scans: HashMap<u64, Scan>,
    queue: VecDeque<String>,
    counts: HashMap<String, AbortHandle>,
    changed: bool,
}
struct Scan {
    task: AbortHandle,
    scoped: bool,
    markers: Vec<String>,
}
pub(super) enum FolderEvent {
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
    const fn epoch(&self) -> u64 {
        match self {
            Self::Candidate { epoch, .. }
            | Self::Discovered { epoch, .. }
            | Self::Count { epoch, .. }
            | Self::Applied { epoch, .. }
            | Self::ApplyDone { epoch } => *epoch,
        }
    }
}
impl Controller {
    fn emit_selection_state(&self) {
        (self.emit)(AppUpdate::FolderSelection(self.selection.state.clone()));
    }
    pub(super) fn cancel_selection(&mut self) {
        for scan in self.selection.scans.drain().map(|(_, scan)| scan) {
            scan.task.abort();
        }
        for task in self.selection.counts.drain().map(|(_, task)| task) {
            task.abort();
        }
        self.selection.queue.clear();
        self.selection.epoch = self.selection.epoch.wrapping_add(1);
    }
    pub(super) fn open_selection(&mut self) {
        if self.session.is_none() || self.selection.state.open {
            return;
        }
        self.cancel_selection();
        self.selection.state = FolderSelection {
            open: true,
            ..Default::default()
        };
        self.reconcile_selection();
        self.discover_selection(DiscoverFolders::default(), false);
    }
    pub(super) fn close_selection(&mut self) {
        if self.selection.state.applying {
            return;
        }
        self.cancel_selection();
        self.selection.state = FolderSelection::default();
        self.emit_selection_state();
    }
    pub(super) fn reconcile_selection(&mut self) {
        if !self.selection.state.open {
            return;
        }
        for folder in self.folders.clone() {
            if let Some(row) = self
                .selection
                .state
                .rows
                .iter_mut()
                .find(|row| row.registered_id == Some(folder.id))
            {
                if row.pending == Some(FolderAction::Add) {
                    row.pending = None;
                }
            } else {
                self.merge_candidate(
                    folder.kind,
                    folder.root_path,
                    folder.marker_path,
                    Some(folder.id),
                );
            }
        }
        self.emit_selection_state();
    }
    fn merge_candidate(
        &mut self,
        kind: String,
        root_path: String,
        marker_path: String,
        registered_id: Option<i32>,
    ) {
        if let Some(row) = self.selection.state.rows.iter_mut().find(|row| {
            row.marker_path == marker_path
                || registered_id.is_some() && row.registered_id == registered_id
        }) {
            // The server resolves aliases. Replace the seeded marker and restart its preview once.
            if row.marker_path != marker_path {
                if let Some(task) = self.selection.counts.remove(&row.marker_path) {
                    task.abort();
                }
                self.selection.queue.retain(|path| *path != row.marker_path);
                row.marker_path.clone_from(&marker_path);
                row.root_path = root_path;
                row.count = Loading::Pending;
                self.selection.queue.push_back(marker_path);
            }
            if registered_id.is_some() {
                row.registered_id = registered_id;
                if row.pending == Some(FolderAction::Add) {
                    row.pending = None;
                }
            }
            return;
        }
        self.selection.queue.push_back(marker_path.clone());
        self.selection.state.rows.push(FolderSelectionRow {
            kind,
            root_path,
            marker_path,
            registered_id,
            pending: None,
            count: Loading::Pending,
            error: None,
        });
    }
    fn discover_selection(&mut self, request: DiscoverFolders, scoped: bool) {
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            return;
        };
        self.selection.scan = self.selection.scan.wrapping_add(1);
        let scan = self.selection.scan;
        let epoch = self.selection.epoch;
        let sender = self.folder_sender.clone();
        let task = self.task(async move {
            let result = api
                .discover_osu_folders(&request, |event| {
                    let _ = sender.send(FolderEvent::Candidate { epoch, scan, event });
                })
                .await
                .map_err(|error| describe(&error));
            // Preserve channel ordering: all candidates must arrive before completion.
            let _ = sender.send(FolderEvent::Discovered {
                epoch,
                scan,
                result,
            });
            Completed::Cancelled
        });
        self.selection.scans.insert(
            scan,
            Scan {
                task,
                scoped,
                markers: Vec::new(),
            },
        );
        self.selection.state.discovering = true;
        self.emit_selection_state();
    }
    pub(super) fn browse_selection(&mut self) {
        let state = &mut self.selection.state;
        if !state.open || state.applying || state.picking {
            return;
        }
        state.picking = true;
        self.emit_selection_state();
        (self.emit)(AppUpdate::FolderSelectionPickerRequested(
            self.selection.epoch,
        ));
    }
    pub(super) fn selection_picked(&mut self, epoch: u64, path: Option<PathBuf>) {
        if epoch != self.selection.epoch
            || !self.selection.state.open
            || !self.selection.state.picking
        {
            return;
        }
        self.selection.state.picking = false;
        if let Some(path) = path {
            self.selection.state.message.clear();
            self.discover_selection(
                DiscoverFolders {
                    roots: vec![path],
                    depth: DiscoveryDepth::Known,
                },
                true,
            );
        } else {
            self.emit_selection_state();
        }
    }
    pub(super) fn toggle_selection(&mut self, path: &str) {
        if !self.selection.state.open || self.selection.state.applying {
            return;
        }
        if let Some(row) = self
            .selection
            .state
            .rows
            .iter_mut()
            .find(|row| row.marker_path == path)
        {
            row.pending = if row.pending.is_some() {
                None
            } else if row.registered_id.is_some() {
                Some(FolderAction::Remove)
            } else {
                Some(FolderAction::Add)
            };
            row.error = None;
            self.emit_selection_state();
        }
    }
    pub(super) fn retry_count(&mut self, path: &str) {
        if !self.selection.state.open || self.selection.state.applying {
            return;
        }
        if let Some(row) = self
            .selection
            .state
            .rows
            .iter_mut()
            .find(|row| row.marker_path == path && matches!(row.count, Loading::Failed(_)))
        {
            row.count = Loading::Pending;
            self.selection.queue.push_back(path.into());
            self.emit_selection_state();
        }
    }
    pub(super) fn start_counts(&mut self) {
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            return;
        };
        while self.selection.state.open && self.selection.counts.len() < 2 {
            let Some(path) = self.selection.queue.pop_front() else {
                break;
            };
            let api = api.clone();
            let epoch = self.selection.epoch;
            let key = path.clone();
            let task = self.task(async move {
                Completed::FolderSelection(FolderEvent::Count {
                    epoch,
                    path: path.clone(),
                    result: api
                        .osu_folder_metadata(&path)
                        .await
                        .map(|metadata| metadata.beatmap_count)
                        .map_err(|error| describe(&error)),
                })
            });
            self.selection.counts.insert(key, task);
        }
    }
    pub(super) fn apply_selection(&mut self) {
        if !self.selection.state.open
            || self.selection.state.applying
            || self.selection.state.picking
        {
            return;
        }
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            return;
        };
        let mut actions: Vec<_> = self
            .selection
            .state
            .rows
            .iter()
            .filter_map(|row| {
                row.pending
                    .map(|action| (action, row.marker_path.clone(), row.registered_id))
            })
            .collect();
        actions.sort_by_key(|(action, _, _)| *action == FolderAction::Remove);
        self.selection.changed = false;
        self.selection.state.applying = true;
        self.selection.state.message.clear();
        self.emit_selection_state();
        let epoch = self.selection.epoch;
        let sender = self.folder_sender.clone();
        self.task(async move {
            for (action, path, id) in actions {
                let result = match action {
                    FolderAction::Add => api.import_osu_folder(&path).await.map(Some),
                    FolderAction::Remove => {
                        if let Some(id) = id {
                            api.remove_osu_folder(id).await.map(|()| None)
                        } else {
                            Err(crate::ApiError::Protocol(
                                "Missing registered folder ID.".into(),
                            ))
                        }
                    }
                }
                .map_err(|error| describe(&error));
                let _ = sender.send(FolderEvent::Applied {
                    epoch,
                    path,
                    result,
                });
            }
            let _ = sender.send(FolderEvent::ApplyDone { epoch });
            Completed::Cancelled
        });
    }
    fn selection_applied(&mut self, path: &str, result: Result<Option<OsuFolder>, String>) {
        if let Some(row) = self
            .selection
            .state
            .rows
            .iter_mut()
            .find(|row| row.marker_path == path)
        {
            match result {
                Ok(folder) => {
                    self.selection.changed = true;
                    row.pending = None;
                    row.error = None;
                    if let Some(folder) = folder {
                        row.registered_id = Some(folder.id);
                        if let Some(stored) = self
                            .folders
                            .iter_mut()
                            .find(|stored| stored.id == folder.id)
                        {
                            *stored = folder;
                        } else {
                            self.folders.push(folder);
                        }
                    } else {
                        let id = row.registered_id.take();
                        self.folders.retain(|folder| Some(folder.id) != id);
                    }
                    (self.emit)(AppUpdate::FoldersReplaced(self.folders.clone()));
                }
                Err(error) => row.error = Some(error),
            }
        }
    }
    pub(super) fn selection_event(&mut self, event: FolderEvent) {
        if event.epoch() != self.selection.epoch || !self.selection.state.open {
            return;
        }
        match event {
            FolderEvent::Candidate {
                scan,
                event:
                    FolderDiscoveryEvent::Candidate {
                        kind,
                        root_path,
                        marker_path,
                        registered_id,
                    },
                ..
            } => {
                let Some(scan) = self.selection.scans.get_mut(&scan) else {
                    return;
                };
                if !scan.markers.contains(&marker_path) {
                    scan.markers.push(marker_path.clone());
                }
                // Discovery's registration snapshot can predate an Apply removal.
                let registered_id =
                    registered_id.filter(|id| self.folders.iter().any(|folder| folder.id == *id));
                self.merge_candidate(kind, root_path, marker_path, registered_id);
            }
            FolderEvent::Candidate { .. } => return,
            FolderEvent::Discovered { scan, result, .. } => {
                let Some(scan) = self.selection.scans.remove(&scan) else {
                    return;
                };
                self.selection.state.discovering = !self.selection.scans.is_empty();
                match result {
                    Err(error) => self.selection.state.message = error,
                    Ok(()) if scan.scoped => {
                        self.selection.state.message = match scan.markers.len() {
                            0 => "No osu! installation found in the selected folder.".into(),
                            1 => {
                                if !self.selection.state.applying
                                    && let Some(row) =
                                        self.selection.state.rows.iter_mut().find(|row| {
                                            Some(&row.marker_path) == scan.markers.first()
                                                && row.registered_id.is_none()
                                        })
                                {
                                    row.pending = Some(FolderAction::Add);
                                    row.error = None;
                                }
                                String::new()
                            }
                            _ => "Multiple installations found. Select each folder to add.".into(),
                        };
                    }
                    Ok(()) => {}
                }
            }
            FolderEvent::Count { path, result, .. } => {
                self.selection.counts.remove(&path);
                if let Some(row) = self
                    .selection
                    .state
                    .rows
                    .iter_mut()
                    .find(|row| row.marker_path == path)
                {
                    row.count = match result {
                        Ok(count) => Loading::Ready(count),
                        Err(error) => Loading::Failed(error),
                    };
                }
            }
            FolderEvent::Applied { path, result, .. } => {
                self.selection_applied(&path, result);
            }
            FolderEvent::ApplyDone { .. } => {
                self.selection.state.applying = false;
                if self.selection.changed {
                    self.refresh_folders();
                    self.refresh_library();
                }
                if self
                    .selection
                    .state
                    .rows
                    .iter()
                    .all(|row| row.pending.is_none())
                {
                    self.close_selection();
                    return;
                }
                self.selection.state.message = "Some changes failed. Successful changes are saved; Apply retries the remaining rows.".into();
            }
        }
        self.emit_selection_state();
    }
}

#[cfg(test)]
#[path = "folders_tests.rs"]
mod tests;
