use super::{AppUpdate, Completed, Controller, Track, describe};
use crate::models::{
    PlaybackCommand, PlaybackMode, Playlist, PlaylistItem, PlaylistSummary, TrackDifficulty,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaylistAction {
    Open,
    Close,
    Refresh,
    Create(String),
    Rename { id: i32, name: String },
    Delete(i32),
    Select(Option<i32>),
    SelectItem(i32),
    Play { id: i32, start_item_id: Option<i32> },
    TogglePlayback,
    OpenAdd,
    ChooseTarget(i32),
    ToggleDifficulty(i32),
    Add,
    RemoveItem(i32),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlaylistCandidate {
    pub beatmap_id: i32,
    pub name: String,
    pub checked: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[allow(clippy::struct_excessive_bools)] // Independent modal and request flags.
pub struct PlaylistsState {
    pub open: bool,
    pub adding: bool,
    pub loading: bool,
    pub busy: bool,
    pub message: String,
    pub playlists: Vec<PlaylistSummary>,
    pub active_id: Option<i32>,
    pub active: Option<Playlist>,
    pub selected_item_id: Option<i32>,
    pub target_id: Option<i32>,
    pub candidates: Vec<PlaylistCandidate>,
}
impl PlaylistsState {
    #[must_use]
    pub fn selected_item(&self) -> Option<&PlaylistItem> {
        self.active
            .as_ref()?
            .items
            .iter()
            .find(|item| Some(item.id) == self.selected_item_id)
    }
    pub fn tracks(&self) -> Vec<Track> {
        self.active.as_ref().map_or_else(Vec::new, |playlist| {
            playlist.items.iter().map(PlaylistItem::track).collect()
        })
    }
}
impl PlaylistItem {
    #[must_use]
    pub fn track(&self) -> Track {
        let mut track = Track::from(crate::models::LibraryTrack {
            audio_source_id: self.audio_source_id.unwrap_or(-1),
            title: self.title.clone(),
            title_unicode: None,
            artist: self.artist.clone(),
            artist_unicode: None,
            cover_beatmap_id: self.cover_beatmap_id,
            difficulties: self
                .beatmap_id
                .zip(self.beatmap_set_id)
                .map(|(beatmap_id, beatmap_set_id)| TrackDifficulty {
                    beatmap_id,
                    beatmap_set_id,
                    difficulty_name: self.difficulty_name.clone(),
                    set_has_multiple_audio_sources: true,
                })
                .into_iter()
                .collect(),
        });
        let difficulty = self
            .difficulty_name
            .as_deref()
            .unwrap_or("Unknown difficulty");
        track.subtitle = format!(
            "{} | {difficulty}{}",
            track.artist,
            if self.audio_source_id.is_none() {
                " · Недоступно"
            } else {
                ""
            }
        );
        track
    }
}

#[derive(Default)]
pub(super) struct PlaylistWork {
    pub view: PlaylistsState,
    pub tracks: Vec<Track>,
    list_epoch: u64,
    detail_epoch: u64,
    candidates_epoch: u64,
}
pub(super) enum PlaylistEvent {
    List {
        epoch: u64,
        result: Result<Vec<PlaylistSummary>, String>,
    },
    Detail {
        epoch: u64,
        id: i32,
        result: Result<Playlist, String>,
    },
    Candidates {
        epoch: u64,
        result: Result<Track, String>,
    },
    Mutation(Result<Mutation, String>),
}
pub(super) enum Mutation {
    Created(PlaylistSummary),
    Renamed(PlaylistSummary),
    Deleted(i32),
    Items(Playlist),
    Removed { playlist_id: i32, item_id: i32 },
}

impl Controller {
    pub(super) fn playlist_action(&mut self, action: PlaylistAction) {
        match action {
            PlaylistAction::Open => {
                self.playlists.view.open = true;
                self.playlists.view.adding = false;
                self.refresh_playlists();
            }
            PlaylistAction::Close => {
                if !self.playlists.view.busy {
                    self.playlists.view.open = false;
                }
            }
            PlaylistAction::Refresh => {
                self.refresh_playlists();
                if let Some(id) = self.playlists.view.active_id {
                    self.load_playlist(id);
                }
            }
            PlaylistAction::Select(id) => self.select_playlist(id),
            PlaylistAction::SelectItem(id) => {
                if self
                    .playlists
                    .view
                    .active
                    .as_ref()
                    .is_some_and(|playlist| playlist.items.iter().any(|item| item.id == id))
                {
                    self.playlists.view.selected_item_id = Some(id);
                    self.emit_selection();
                }
            }
            PlaylistAction::Play { id, start_item_id } => {
                self.queue_playlist_play(id, start_item_id);
            }
            PlaylistAction::TogglePlayback => self.toggle_playlist_playback(),
            PlaylistAction::OpenAdd => self.open_playlist_add(),
            PlaylistAction::ChooseTarget(id) => {
                if self
                    .playlists
                    .view
                    .playlists
                    .iter()
                    .any(|playlist| playlist.id == id)
                {
                    self.playlists.view.target_id = Some(id);
                }
            }
            PlaylistAction::ToggleDifficulty(id) => {
                if !self.playlists.view.busy
                    && let Some(candidate) = self
                        .playlists
                        .view
                        .candidates
                        .iter_mut()
                        .find(|candidate| candidate.beatmap_id == id)
                {
                    candidate.checked = !candidate.checked;
                }
            }
            action => self.mutate_playlist(action),
        }
        self.emit_playlists();
    }

    fn toggle_playlist_playback(&mut self) {
        let Some(item) = self.playlists.view.selected_item() else {
            return;
        };
        if item.audio_source_id.is_none() {
            return;
        }
        if let Some(assignment) = &self.assignment
            && assignment.current_playlist_item_id == Some(item.id)
        {
            self.queue_playback(if assignment.mode == PlaybackMode::Playing {
                PlaybackCommand::Pause
            } else {
                PlaybackCommand::Play {
                    audio_source_id: None,
                }
            });
        } else {
            self.queue_playlist_play(item.playlist_id, Some(item.id));
        }
    }

    fn open_playlist_add(&mut self) {
        let selected = if self.playlists.view.active_id.is_some() {
            self.playlists
                .view
                .selected_item()
                .and_then(|item| item.audio_source_id)
        } else {
            self.selected
        };
        let Some(id) = selected else {
            return;
        };
        self.playlists.candidates_epoch = self.playlists.candidates_epoch.wrapping_add(1);
        self.playlists.view.open = true;
        self.playlists.view.adding = true;
        self.playlists.view.candidates.clear();
        self.refresh_playlists();
        if let Some(track) = self
            .track_indices
            .get(&id)
            .and_then(|index| self.tracks.get(*index))
            .cloned()
        {
            self.install_candidates(track);
        } else if let Some(api) = self.session.as_ref().map(|session| session.api().clone()) {
            // Search results may omit a playlist song; reload its complete difficulty group.
            let epoch = self.playlists.candidates_epoch;
            self.playlists.view.message = "Loading difficulties…".into();
            self.task(async move {
                let result = api
                    .search_tracks("")
                    .await
                    .map_err(|error| describe(&error))
                    .and_then(|tracks| {
                        tracks
                            .into_iter()
                            .find(|track| track.audio_source_id == id)
                            .map(Track::from)
                            .ok_or_else(|| {
                                "This song is no longer available. Refresh the library.".into()
                            })
                    });
                Completed::Playlist(PlaylistEvent::Candidates { epoch, result })
            });
        }
    }

    fn install_candidates(&mut self, track: Track) {
        self.playlists.view.candidates = track
            .difficulties
            .into_iter()
            .map(|difficulty| PlaylistCandidate {
                beatmap_id: difficulty.beatmap_id,
                name: difficulty
                    .difficulty_name
                    .unwrap_or_else(|| "Unknown difficulty".into()),
                checked: true,
            })
            .collect();
    }

    pub(super) fn refresh_playlists(&mut self) {
        if self.playlists.view.busy {
            return;
        }
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            self.playlists.view.message = "Connect to the server to load playlists.".into();
            return;
        };
        self.playlists.list_epoch = self.playlists.list_epoch.wrapping_add(1);
        let epoch = self.playlists.list_epoch;
        self.playlists.view.loading = true;
        self.playlists.view.message.clear();
        self.task(async move {
            Completed::Playlist(PlaylistEvent::List {
                epoch,
                result: api.playlists().await.map_err(|error| describe(&error)),
            })
        });
    }

    fn select_playlist(&mut self, id: Option<i32>) {
        self.playlists.detail_epoch = self.playlists.detail_epoch.wrapping_add(1);
        self.playlists.view.active_id = id;
        self.playlists.view.active = None;
        self.playlists.tracks.clear();
        self.playlists.view.selected_item_id = None;
        self.playlists.view.open = false;
        self.generation = self.generation.wrapping_add(1);
        self.queue.clear();
        self.duration_done.clear();
        self.unavailable.clear();
        self.emit_playlists();
        if let Some(id) = id {
            self.load_playlist(id);
        }
        (self.emit)(AppUpdate::TracksReplaced(if id.is_some() {
            Vec::new()
        } else {
            self.tracks.clone()
        }));
        self.emit_selection();
    }

    fn load_playlist(&mut self, id: i32) {
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            return;
        };
        self.playlists.detail_epoch = self.playlists.detail_epoch.wrapping_add(1);
        let epoch = self.playlists.detail_epoch;
        self.playlists.view.message = "Loading playlist…".into();
        self.task(async move {
            Completed::Playlist(PlaylistEvent::Detail {
                epoch,
                id,
                result: api.playlist(id).await.map_err(|error| describe(&error)),
            })
        });
    }

    fn mutate_playlist(&mut self, action: PlaylistAction) {
        if self.playlists.view.busy {
            return;
        }
        let Some(api) = self.session.as_ref().map(|session| session.api().clone()) else {
            self.playlists.view.message = "Connect to the server to edit playlists.".into();
            return;
        };
        let active = self.playlists.view.active_id;
        let target = self.playlists.view.target_id;
        let ids: Vec<_> = self
            .playlists
            .view
            .candidates
            .iter()
            .filter(|candidate| candidate.checked)
            .map(|candidate| candidate.beatmap_id)
            .collect();
        if matches!(action, PlaylistAction::Add) && (target.is_none() || ids.is_empty()) {
            self.playlists.view.message = "Choose a playlist and at least one difficulty.".into();
            return;
        }
        self.playlists.list_epoch = self.playlists.list_epoch.wrapping_add(1);
        self.playlists.detail_epoch = self.playlists.detail_epoch.wrapping_add(1);
        self.playlists.view.busy = true;
        self.playlists.view.loading = false;
        self.playlists.view.message.clear();
        self.task(async move {
            let result = async {
                match action {
                    PlaylistAction::Create(name) => {
                        api.create_playlist(&name).await.map(Mutation::Created)
                    }
                    PlaylistAction::Rename { id, name } => {
                        api.rename_playlist(id, &name).await.map(Mutation::Renamed)
                    }
                    PlaylistAction::Delete(id) => api
                        .delete_playlist(id)
                        .await
                        .map(|()| Mutation::Deleted(id)),
                    PlaylistAction::Add => api
                        .add_playlist_items(target.unwrap_or(-1), &ids)
                        .await
                        .map(Mutation::Items),
                    PlaylistAction::RemoveItem(item_id) => api
                        .remove_playlist_item(active.unwrap_or(-1), item_id)
                        .await
                        .map(|()| Mutation::Removed {
                            playlist_id: active.unwrap_or(-1),
                            item_id,
                        }),
                    _ => Err(crate::ApiError::Protocol(
                        "Unsupported playlist action.".into(),
                    )),
                }
            }
            .await
            .map_err(|error| describe(&error));
            Completed::Playlist(PlaylistEvent::Mutation(result))
        });
    }

    #[allow(clippy::too_many_lines)] // Keep epoch checks and their state transitions together.
    pub(super) fn playlist_event(&mut self, event: PlaylistEvent) {
        match event {
            PlaylistEvent::Candidates { epoch, result } => {
                if epoch != self.playlists.candidates_epoch
                    || !self.playlists.view.open
                    || !self.playlists.view.adding
                {
                    return;
                }
                match result {
                    Ok(track) => {
                        self.install_candidates(track);
                        if self.playlists.view.message == "Loading difficulties…" {
                            self.playlists.view.message.clear();
                        }
                    }
                    Err(error) => self.playlists.view.message = error,
                }
            }
            PlaylistEvent::List { epoch, result } => {
                if epoch != self.playlists.list_epoch {
                    return;
                }
                self.playlists.view.loading = false;
                match result {
                    Ok(playlists) => {
                        self.playlists.view.target_id = self
                            .playlists
                            .view
                            .target_id
                            .filter(|id| playlists.iter().any(|playlist| playlist.id == *id))
                            .or_else(|| playlists.first().map(|playlist| playlist.id));
                        self.playlists.view.playlists = playlists;
                    }
                    Err(error) => self.playlists.view.message = error,
                }
            }
            PlaylistEvent::Detail { epoch, id, result } => {
                if epoch != self.playlists.detail_epoch || Some(id) != self.playlists.view.active_id
                {
                    return;
                }
                match result {
                    Ok(playlist) => {
                        self.install_playlist(playlist);
                        if self.playlists.view.message == "Loading playlist…" {
                            self.playlists.view.message.clear();
                        }
                    }
                    Err(error) => self.playlists.view.message = error,
                }
            }
            PlaylistEvent::Mutation(result) => {
                self.playlists.view.busy = false;
                match result {
                    Ok(mutation) => {
                        match mutation {
                            Mutation::Created(playlist) => {
                                self.playlists.view.target_id = Some(playlist.id);
                                self.playlists.view.playlists.push(playlist);
                            }
                            Mutation::Renamed(playlist) => {
                                if let Some(row) = self
                                    .playlists
                                    .view
                                    .playlists
                                    .iter_mut()
                                    .find(|row| row.id == playlist.id)
                                {
                                    row.name.clone_from(&playlist.name);
                                }
                                if let Some(active) = self
                                    .playlists
                                    .view
                                    .active
                                    .as_mut()
                                    .filter(|active| active.id == playlist.id)
                                {
                                    active.name = playlist.name;
                                }
                            }
                            Mutation::Deleted(id) => {
                                self.playlists
                                    .view
                                    .playlists
                                    .retain(|playlist| playlist.id != id);
                                if self.playlists.view.active_id == Some(id) {
                                    self.select_playlist(None);
                                }
                            }
                            Mutation::Items(playlist) => {
                                if self.playlists.view.active_id == Some(playlist.id) {
                                    self.install_playlist(playlist);
                                }
                                self.playlists.view.open = false;
                            }
                            Mutation::Removed {
                                playlist_id,
                                item_id,
                            } => {
                                if let Some(mut playlist) = self
                                    .playlists
                                    .view
                                    .active
                                    .clone()
                                    .filter(|active| active.id == playlist_id)
                                {
                                    playlist.items.retain(|item| item.id != item_id);
                                    self.install_playlist(playlist);
                                }
                            }
                        }
                        self.refresh_playlists();
                        if let Some(id) = self.playlists.view.active_id {
                            self.load_playlist(id);
                        }
                    }
                    Err(error) => self.playlists.view.message = error,
                }
            }
        }
        self.emit_playlists();
        self.emit_selection();
    }

    fn install_playlist(&mut self, playlist: Playlist) {
        self.generation = self.generation.wrapping_add(1);
        self.queue.clear();
        self.unavailable.clear();
        self.duration_done.clear();
        self.playlists.view.selected_item_id = self
            .playlists
            .view
            .selected_item_id
            .filter(|id| playlist.items.iter().any(|item| item.id == *id))
            .or_else(|| playlist.items.first().map(|item| item.id));
        self.playlists.view.active = Some(playlist);
        self.playlists.tracks = self.playlists.view.tracks();
        self.emit_playlists();
        (self.emit)(AppUpdate::TracksReplaced(self.playlists.tracks.clone()));
    }
    pub(super) fn emit_playlists(&self) {
        (self.emit)(AppUpdate::Playlists(self.playlists.view.clone()));
    }
}

#[cfg(test)]
mod tests;
