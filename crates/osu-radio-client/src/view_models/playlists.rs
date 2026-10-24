use super::queue::PlaylistPlayback;
use super::{LibraryViewModel, Track, TrackNamePreferences, TrackSort};
#[cfg(test)]
use crate::controller::Controller;
use crate::controller::workflow::{Effect, Workflow};
use crate::controller::{AppUpdate, Completed};
use crate::describe;
use crate::models::{Playlist, PlaylistItem, PlaylistSummary, TrackDifficulty};
use std::collections::HashMap;
use std::collections::{HashSet, VecDeque};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaylistAction {
    ShowLibrary,
    ShowPlaylists,
    Search(String),
    BeginCreate,
    BeginEdit(i32),
    SetEditorName(String),
    CancelEditor,
    SaveEditor,
    CompleteCoverPick {
        epoch: u64,
        png: Result<Option<Vec<u8>>, String>,
    },
    ResetEditorCover,
    RequestCover {
        id: i32,
        revision: i64,
    },
    RequestArtwork {
        id: i32,
        beatmap_id: i32,
    },
    Open,
    Close,
    Refresh,
    Create(String),
    Rename {
        id: i32,
        name: String,
    },
    Delete(i32),
    AddToQueue(i32),
    Select(Option<i32>),
    SelectItem(i32),
    Play {
        id: i32,
        start_item_id: Option<i32>,
    },
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
    pub visible: bool,
    pub query: String,
    pub editor_open: bool,
    pub editor_id: Option<i32>,
    pub editor_name: String,
    pub editor_epoch: u64,
    #[serde(skip)]
    pub editor_cover_png: Option<Vec<u8>>,
    pub editor_cover_reset: bool,
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
    pub const fn showing_detail(&self) -> bool {
        self.visible && self.active_id.is_some()
    }
    #[must_use]
    pub fn filtered_playlists(&self) -> Vec<&PlaylistSummary> {
        let query = self.query.to_lowercase();
        self.playlists
            .iter()
            .filter(|playlist| playlist.name.to_lowercase().contains(&query))
            .collect()
    }
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
            last_played_at_ms: self.last_played_at_ms,
            volume_percent: self.volume_percent,
            mods: self.mods,
            title: self.title.clone(),
            title_unicode: self.title_unicode.clone(),
            artist: self.artist.clone(),
            artist_unicode: self.artist_unicode.clone(),
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
            .filter(|name| !name.trim().is_empty())
            .unwrap_or("Unknown difficulty");
        track.subtitle_suffix = format!(
            " | {difficulty}{}",
            if self.audio_source_id.is_none() {
                " · Unavailable"
            } else {
                ""
            }
        );
        track.apply_name_preferences(crate::TrackNamePreferences::default());
        track
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistTrack {
    pub item_id: i32,
    pub track: Track,
}
impl std::ops::Deref for PlaylistTrack {
    type Target = Track;
    fn deref(&self) -> &Track {
        &self.track
    }
}
impl std::ops::DerefMut for PlaylistTrack {
    fn deref_mut(&mut self) -> &mut Track {
        &mut self.track
    }
}
#[derive(Default)]
pub struct PlaylistsViewModel {
    pub(crate) view: PlaylistsState,
    pub(crate) tracks: Vec<PlaylistTrack>,
    pub(crate) list_epoch: u64,
    pub(crate) detail_epoch: u64,
    pub(crate) refresh_pending: bool,
    pub(crate) candidates_epoch: u64,
    pub(crate) images: HashSet<PlaylistImage>,
    pub(crate) pending_images: VecDeque<PlaylistImage>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PlaylistImage {
    Custom { id: i32, revision: i64 },
    Automatic { id: i32, beatmap_id: i32 },
}
pub(crate) enum PlaylistEvent {
    Image {
        request: PlaylistImage,
        result: Result<Option<Vec<u8>>, String>,
    },
    EditorSaved {
        epoch: u64,
        saved: Option<PlaylistSummary>,
        result: Result<PlaylistSummary, String>,
    },
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
pub(crate) enum Mutation {
    Created(PlaylistSummary),
    Renamed(PlaylistSummary),
    Deleted(i32),
    Items(Playlist),
    Removed { playlist_id: i32, item_id: i32 },
}

#[cfg(test)]
mod tests;

impl PlaylistsViewModel {
    pub(crate) fn begin_playlist_editor(&mut self, id: Option<i32>) {
        if self.view.busy {
            return;
        }
        let name = match id {
            Some(id) => {
                let Some(playlist) = self
                    .view
                    .playlists
                    .iter()
                    .find(|playlist| playlist.id == id)
                else {
                    return;
                };
                playlist.name.clone()
            }
            None => String::new(),
        };
        self.view.editor_epoch = self.view.editor_epoch.wrapping_add(1);
        self.view.editor_open = true;
        self.view.editor_id = id;
        self.view.editor_name = name;
        self.view.editor_cover_png = None;
        self.view.editor_cover_reset = false;
        self.view.message.clear();
    }
}

impl PlaylistsViewModel {
    pub(crate) fn close_playlist_editor(&mut self) {
        self.view.editor_epoch = self.view.editor_epoch.wrapping_add(1);
        self.view.editor_open = false;
        self.view.editor_id = None;
        self.view.editor_name.clear();
        self.view.editor_cover_png = None;
        self.view.editor_cover_reset = false;
        self.view.message.clear();
    }
}

impl PlaylistsViewModel {
    pub(crate) fn install_playlist_summary(&mut self, summary: PlaylistSummary) {
        if let Some(active) = self
            .view
            .active
            .as_mut()
            .filter(|active| active.id == summary.id)
        {
            active.name.clone_from(&summary.name);
        }
        if let Some(row) = self
            .view
            .playlists
            .iter_mut()
            .find(|row| row.id == summary.id)
        {
            *row = summary;
        } else {
            self.view.playlists.push(summary);
        }
    }
}

impl PlaylistsViewModel {
    pub(crate) fn install_candidates(&mut self, track: Track) {
        self.view.candidates = track
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
}

impl PlaylistsViewModel {
    pub(crate) fn playlist_image_current(&self, request: PlaylistImage) -> bool {
        self.view.playlists.iter().any(|playlist| match request {
            PlaylistImage::Custom { id, revision } => {
                playlist.id == id && playlist.custom_cover_revision == Some(revision)
            }
            PlaylistImage::Automatic { id, beatmap_id } => {
                playlist.id == id && playlist.cover_beatmap_id == Some(beatmap_id)
            }
        })
    }
}

impl PlaylistsViewModel {
    pub(crate) fn request_playlist_image(&mut self, request: PlaylistImage) {
        if self.playlist_image_current(request)
            && !self.images.contains(&request)
            && !self.pending_images.contains(&request)
        {
            self.pending_images.push_back(request);
        }
    }
}

impl PlaylistsViewModel {
    #[must_use]
    pub const fn state(&self) -> &PlaylistsState {
        &self.view
    }
    #[must_use]
    pub fn tracks(&self) -> &[PlaylistTrack] {
        &self.tracks
    }
    pub(crate) fn selected_track(&self) -> Option<&Track> {
        self.tracks
            .iter()
            .find(|row| Some(row.item_id) == self.view.selected_item_id)
            .map(|row| &row.track)
    }
    pub(crate) const fn set_visible(&mut self, visible: bool) -> bool {
        if self.view.visible == visible {
            return false;
        }
        self.view.visible = visible;
        true
    }
    pub(crate) fn clear_detail(&mut self, id: Option<i32>) {
        if id.is_some() {
            self.view.visible = true;
        }
        self.detail_epoch = self.detail_epoch.wrapping_add(1);
        self.view.active_id = id;
        self.view.active = None;
        self.tracks.clear();
        self.view.selected_item_id = None;
        self.view.open = false;
    }
    pub(crate) fn replace_detail(
        &mut self,
        playlist: Playlist,
        tracks: Vec<Track>,
        sort: super::TrackSort,
    ) {
        let selected = self.view.selected_item_id;
        self.tracks = playlist
            .items
            .iter()
            .zip(tracks)
            .map(|(item, track)| PlaylistTrack {
                item_id: item.id,
                track,
            })
            .collect();
        self.view.active = Some(playlist);
        self.reorder(sort);
        self.view.selected_item_id = selected
            .filter(|id| self.tracks.iter().any(|row| row.item_id == *id))
            .or_else(|| self.tracks.first().map(|row| row.item_id));
    }
    pub(crate) fn reorder(&mut self, sort: super::TrackSort) {
        if let Some(playlist) = &mut self.view.active {
            sort.sort_playlist(playlist, &mut self.tracks);
        }
    }
}

impl PlaylistsViewModel {
    #[allow(clippy::too_many_lines)] // Keep command guards and state transitions together.
    pub(crate) fn playlist_action(
        &mut self,
        action: PlaylistAction,
        input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if matches!(
            action,
            PlaylistAction::Select(_)
                | PlaylistAction::SelectItem(_)
                | PlaylistAction::ShowLibrary
                | PlaylistAction::ShowPlaylists
        ) {
            work.effect(Effect::FinishVolume);
        }
        match action {
            PlaylistAction::ShowLibrary => self.show_playlists(false, input, work),
            PlaylistAction::ShowPlaylists | PlaylistAction::Open => {
                self.show_playlists(true, input, work);
                self.refresh_playlists(input, work);
                if let Some(id) = self.view.active_id
                    && self.view.active.is_none()
                {
                    self.load_playlist(id, input, work);
                }
            }
            PlaylistAction::Search(query) => self.view.query = query,
            PlaylistAction::BeginCreate => self.begin_playlist_editor(None),
            PlaylistAction::BeginEdit(id) => self.begin_playlist_editor(Some(id)),
            PlaylistAction::SetEditorName(name) => {
                if self.view.editor_open && !self.view.busy {
                    self.view.editor_name = name;
                }
            }
            PlaylistAction::CancelEditor => {
                if !self.view.busy {
                    self.close_playlist_editor();
                }
            }
            PlaylistAction::SaveEditor => self.save_playlist_editor(input, work),
            PlaylistAction::CompleteCoverPick { epoch, png } => {
                if epoch == self.view.editor_epoch && self.view.editor_open && !self.view.busy {
                    match png {
                        Ok(Some(bytes)) if bytes.len() <= 2 * 1024 * 1024 => {
                            self.view.editor_epoch = self.view.editor_epoch.wrapping_add(1);
                            self.view.editor_cover_png = Some(bytes);
                            self.view.editor_cover_reset = false;
                            self.view.message.clear();
                        }
                        Ok(Some(_)) => {
                            self.view.message = "Playlist cover exceeds 2 MiB.".into();
                        }
                        Ok(None) => {}
                        Err(error) => self.view.message = error,
                    }
                }
            }
            PlaylistAction::ResetEditorCover => {
                if self.view.editor_open && !self.view.busy {
                    self.view.editor_epoch = self.view.editor_epoch.wrapping_add(1);
                    self.view.editor_cover_png = None;
                    self.view.editor_cover_reset = true;
                    self.view.message.clear();
                }
            }
            PlaylistAction::RequestCover { id, revision } => {
                self.request_playlist_image(PlaylistImage::Custom { id, revision });
                return;
            }
            PlaylistAction::RequestArtwork { id, beatmap_id } => {
                self.request_playlist_image(PlaylistImage::Automatic { id, beatmap_id });
                return;
            }
            PlaylistAction::Close => {
                if !self.view.busy {
                    self.view.open = false;
                    self.view.adding = false;
                    self.candidates_epoch = self.candidates_epoch.wrapping_add(1);
                }
            }
            PlaylistAction::Refresh => {
                self.refresh_playlists(input, work);
                if let Some(id) = self.view.active_id {
                    self.load_playlist(id, input, work);
                }
            }
            PlaylistAction::Select(id) => self.select_playlist(id, input, work),
            PlaylistAction::SelectItem(id) => {
                if self.view.showing_detail()
                    && self
                        .view
                        .active
                        .as_ref()
                        .is_some_and(|playlist| playlist.items.iter().any(|item| item.id == id))
                {
                    self.view.selected_item_id = Some(id);
                    work.effect(Effect::Selection);
                }
            }
            PlaylistAction::Play { id, start_item_id } => {
                work.effect(Effect::PlaylistPlayback(PlaylistPlayback::Play(
                    id,
                    start_item_id,
                )));
            }
            PlaylistAction::AddToQueue(id) => {
                work.effect(Effect::PlaylistPlayback(PlaylistPlayback::Append(id)));
            }
            PlaylistAction::TogglePlayback => self.toggle_playlist_playback(input, work),
            PlaylistAction::OpenAdd => self.open_playlist_add(input, work),
            PlaylistAction::ChooseTarget(id) => {
                if self.view.playlists.iter().any(|playlist| playlist.id == id) {
                    self.view.target_id = Some(id);
                }
            }
            PlaylistAction::ToggleDifficulty(id) => {
                if !self.view.busy
                    && let Some(candidate) = self
                        .view
                        .candidates
                        .iter_mut()
                        .find(|candidate| candidate.beatmap_id == id)
                {
                    candidate.checked = !candidate.checked;
                }
            }
            action => self.mutate_playlist(action, input, work),
        }
        self.emit_playlists(input, work);
    }
    pub(crate) fn show_playlists(
        &mut self,
        visible: bool,
        input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if !self.set_visible(visible) {
            return;
        }
        work.effect(Effect::AdvanceMedia(false));
        self.emit_playlists(input, work);
        work.effect(Effect::Selection);
    }
    pub(crate) fn save_playlist_editor(
        &mut self,
        _input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if !self.view.editor_open || self.view.busy {
            return;
        }
        let name = self.view.editor_name.trim().to_owned();
        if name.is_empty() {
            self.view.message = "Playlist name cannot be empty.".into();
            return;
        }
        let Some(api) = work.api.clone() else {
            self.view.message = "Connect to the server to edit playlists.".into();
            return;
        };
        let epoch = self.view.editor_epoch;
        let id = self.view.editor_id;
        let png = self.view.editor_cover_png.clone();
        let reset = self.view.editor_cover_reset;
        self.view.busy = true;
        self.view.loading = false;
        self.view.message.clear();
        self.list_epoch = self.list_epoch.wrapping_add(1);
        self.detail_epoch = self.detail_epoch.wrapping_add(1);
        work.task(async move {
            let saved = match id {
                Some(id) => api.rename_playlist(id, &name).await,
                None => api.create_playlist(&name).await,
            }
            .map_err(|error| describe(&error));
            let result = match &saved {
                Err(error) => Err(error.clone()),
                Ok(playlist) => {
                    if let Some(png) = png {
                        api.set_playlist_cover(playlist.id, png)
                            .await
                            .map_err(|error| describe(&error))
                    } else if reset {
                        api.reset_playlist_cover(playlist.id)
                            .await
                            .map_err(|error| describe(&error))
                            .map(|()| {
                                let mut summary = playlist.clone();
                                summary.custom_cover_revision = None;
                                summary
                            })
                    } else {
                        Ok(playlist.clone())
                    }
                }
            };
            Completed::Playlist(PlaylistEvent::EditorSaved {
                epoch,
                saved: saved.ok(),
                result,
            })
        });
    }
    pub(crate) fn start_playlist_images(
        &mut self,
        _input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let Some(api) = work.api.clone() else {
            return;
        };
        while self.images.len() < 4 {
            let Some(request) = self.pending_images.pop_front() else {
                break;
            };
            if !self.playlist_image_current(request) {
                continue;
            }
            self.images.insert(request);
            let api = api.clone();
            work.task(async move {
                let result = match request {
                    PlaylistImage::Custom { id, .. } => api.playlist_cover(id).await,
                    PlaylistImage::Automatic { beatmap_id, .. } => api.cover(beatmap_id).await,
                }
                .map_err(|error| describe(&error));
                Completed::Playlist(PlaylistEvent::Image { request, result })
            });
        }
    }
    pub(crate) fn toggle_playlist_playback(
        &self,
        _input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if self.view.showing_detail() {
            work.effect(Effect::TogglePlayback);
        }
    }
    pub(crate) fn open_playlist_add(&mut self, input: &PlaylistInput<'_>, work: &mut Workflow<'_>) {
        let selected = if self.view.showing_detail() {
            self.view
                .selected_item()
                .and_then(|item| item.audio_source_id)
        } else {
            input.library.selected
        };
        let Some(id) = selected else {
            return;
        };
        self.candidates_epoch = self.candidates_epoch.wrapping_add(1);
        self.view.open = true;
        self.view.adding = true;
        self.view.candidates.clear();
        self.refresh_playlists(input, work);
        if let Some(track) = input
            .library
            .indices
            .get(&id)
            .and_then(|index| input.library.tracks.get(*index))
            .cloned()
        {
            self.install_candidates(track);
        } else if let Some(api) = work.api.clone() {
            // Search results may omit a playlist song; reload its complete difficulty group.
            let epoch = self.candidates_epoch;
            self.view.message = "Loading difficulties…".into();
            work.task(async move {
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
    pub(crate) fn refresh_playlist_views(
        &mut self,
        input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        // Import changes membership even if the library request or a pending edit fails.
        self.list_epoch = self.list_epoch.wrapping_add(1);
        self.detail_epoch = self.detail_epoch.wrapping_add(1);
        self.refresh_pending = self.view.busy;
        if self.refresh_pending {
            return;
        }
        self.refresh_playlists(input, work);
        if let Some(id) = self.view.active_id {
            self.load_playlist(id, input, work);
        }
        self.emit_playlists(input, work);
    }
    pub(crate) fn refresh_playlists(
        &mut self,
        _input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if self.view.busy {
            return;
        }
        let Some(api) = work.api.clone() else {
            self.view.message = "Connect to the server to load playlists.".into();
            return;
        };
        self.list_epoch = self.list_epoch.wrapping_add(1);
        let epoch = self.list_epoch;
        self.view.loading = true;
        self.view.message.clear();
        work.task(async move {
            Completed::Playlist(PlaylistEvent::List {
                epoch,
                result: api.playlists().await.map_err(|error| describe(&error)),
            })
        });
    }
    pub(crate) fn select_playlist(
        &mut self,
        id: Option<i32>,
        input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.clear_detail(id);
        work.effect(Effect::AdvanceMedia(false));
        self.emit_playlists(input, work);
        if let Some(id) = id {
            self.load_playlist(id, input, work);
        }
        work.emit(AppUpdate::PlaylistTracksReplaced {
            tracks: Vec::new(),
            invalidate_artwork: false,
        });
        work.effect(Effect::Selection);
    }
    pub(crate) fn load_playlist(
        &mut self,
        id: i32,
        _input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let Some(api) = work.api.clone() else {
            return;
        };
        self.detail_epoch = self.detail_epoch.wrapping_add(1);
        let epoch = self.detail_epoch;
        if self.view.message.is_empty() {
            self.view.message = "Loading playlist…".into();
        }
        work.task(async move {
            Completed::Playlist(PlaylistEvent::Detail {
                epoch,
                id,
                result: api.playlist(id).await.map_err(|error| describe(&error)),
            })
        });
    }
    pub(crate) fn mutate_playlist(
        &mut self,
        action: PlaylistAction,
        _input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if self.view.busy {
            return;
        }
        let Some(api) = work.api.clone() else {
            self.view.message = "Connect to the server to edit playlists.".into();
            return;
        };
        let active = self.view.active_id;
        let target = self.view.target_id;
        let ids: Vec<_> = self
            .view
            .candidates
            .iter()
            .filter(|candidate| candidate.checked)
            .map(|candidate| candidate.beatmap_id)
            .collect();
        if matches!(action, PlaylistAction::Add) && (target.is_none() || ids.is_empty()) {
            self.view.message = "Choose a playlist and at least one difficulty.".into();
            return;
        }
        self.list_epoch = self.list_epoch.wrapping_add(1);
        self.detail_epoch = self.detail_epoch.wrapping_add(1);
        self.view.busy = true;
        self.view.loading = false;
        self.view.message.clear();
        work.task(async move {
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
    pub(crate) fn playlist_event(
        &mut self,
        event: PlaylistEvent,
        input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        match event {
            PlaylistEvent::Image { request, result } => {
                self.images.remove(&request);
                if self.playlist_image_current(request)
                    && let Ok(Some(bytes)) = result
                {
                    work.emit(match request {
                        PlaylistImage::Custom { id, revision } => AppUpdate::PlaylistCover {
                            id,
                            revision,
                            bytes,
                        },
                        PlaylistImage::Automatic { id, beatmap_id } => AppUpdate::PlaylistArtwork {
                            id,
                            beatmap_id,
                            bytes,
                        },
                    });
                }
                return;
            }
            PlaylistEvent::EditorSaved {
                epoch,
                saved,
                result,
            } => {
                if epoch != self.view.editor_epoch || !self.view.editor_open {
                    return;
                }
                self.view.busy = false;
                if let Some(saved) = saved {
                    self.view.editor_id = Some(saved.id);
                    self.view.target_id = Some(saved.id);
                    self.install_playlist_summary(saved);
                }
                match result {
                    Ok(summary) => {
                        self.install_playlist_summary(summary);
                        self.close_playlist_editor();
                        self.refresh_playlists(input, work);
                    }
                    Err(error) => self.view.message = error,
                }
                if let Some(id) = self.view.active_id {
                    self.load_playlist(id, input, work);
                }
            }
            PlaylistEvent::Candidates { epoch, result } => {
                if epoch != self.candidates_epoch || !self.view.open || !self.view.adding {
                    return;
                }
                match result {
                    Ok(track) => {
                        self.install_candidates(track);
                        if self.view.message == "Loading difficulties…" {
                            self.view.message.clear();
                        }
                    }
                    Err(error) => self.view.message = error,
                }
            }
            PlaylistEvent::List { epoch, result } => {
                if epoch != self.list_epoch {
                    return;
                }
                self.view.loading = false;
                match result {
                    Ok(playlists) => {
                        self.view.target_id = self
                            .view
                            .target_id
                            .filter(|id| playlists.iter().any(|playlist| playlist.id == *id))
                            .or_else(|| playlists.first().map(|playlist| playlist.id));
                        self.view.playlists = playlists;
                        if self.view.active_id.is_some_and(|id| {
                            !self.view.playlists.iter().any(|playlist| playlist.id == id)
                        }) {
                            self.select_playlist(None, input, work);
                        }
                    }
                    Err(error) => self.view.message = error,
                }
            }
            PlaylistEvent::Detail { epoch, id, result } => {
                if epoch != self.detail_epoch || Some(id) != self.view.active_id {
                    return;
                }
                match result {
                    Ok(playlist) => {
                        self.install_playlist(playlist, input, work);
                        if self.view.message == "Loading playlist…" {
                            self.view.message.clear();
                        }
                    }
                    Err(error) => self.view.message = error,
                }
            }
            PlaylistEvent::Mutation(result) => {
                self.view.busy = false;
                match result {
                    Ok(mutation) => {
                        match mutation {
                            Mutation::Created(playlist) => {
                                self.view.target_id = Some(playlist.id);
                                self.view.playlists.push(playlist);
                            }
                            Mutation::Renamed(playlist) => {
                                self.install_playlist_summary(playlist);
                            }
                            Mutation::Deleted(id) => {
                                self.view.playlists.retain(|playlist| playlist.id != id);
                                if self.view.editor_id == Some(id) {
                                    self.close_playlist_editor();
                                }
                                if self.view.active_id == Some(id) {
                                    self.select_playlist(None, input, work);
                                }
                            }
                            Mutation::Items(playlist) => {
                                if self.view.active_id == Some(playlist.id) {
                                    self.install_playlist(playlist, input, work);
                                }
                                self.view.open = false;
                            }
                            Mutation::Removed {
                                playlist_id,
                                item_id,
                            } => {
                                if let Some(mut playlist) = self
                                    .view
                                    .active
                                    .clone()
                                    .filter(|active| active.id == playlist_id)
                                {
                                    playlist.items.retain(|item| item.id != item_id);
                                    self.install_playlist(playlist, input, work);
                                }
                            }
                        }
                        self.refresh_playlists(input, work);
                        if let Some(id) = self.view.active_id {
                            self.load_playlist(id, input, work);
                        }
                    }
                    Err(error) => self.view.message = error,
                }
            }
        }
        if self.refresh_pending && !self.view.busy {
            let message = std::mem::take(&mut self.view.message);
            self.refresh_playlist_views(input, work);
            if !message.is_empty() {
                self.view.message = message;
            }
        }
        self.emit_playlists(input, work);
        work.effect(Effect::Selection);
    }
    pub(crate) fn install_playlist(
        &mut self,
        mut playlist: Playlist,
        input: &PlaylistInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let mut tracks: Vec<_> = playlist.items.iter().map(PlaylistItem::track).collect();
        input.prepare_tracks(&mut tracks, work);
        for (item, track) in playlist.items.iter_mut().zip(&tracks) {
            item.last_played_at_ms = track.last_played_at_ms;
            item.volume_percent = track.volume_percent;
        }
        if self.view.showing_detail() {
            work.effect(Effect::AdvanceMedia(false));
        }
        self.replace_detail(playlist, tracks, input.sort);
        self.emit_playlists(input, work);
        work.emit(AppUpdate::PlaylistTracksReplaced {
            tracks: self.tracks.clone(),
            invalidate_artwork: false,
        });
    }
    pub(crate) fn emit_playlists(&self, _input: &PlaylistInput<'_>, work: &mut Workflow<'_>) {
        work.emit(AppUpdate::Playlists(self.view.clone()));
    }
}

pub(crate) struct PlaylistInput<'a> {
    pub(crate) library: &'a LibraryViewModel,
    pub(crate) preferences: TrackNamePreferences,
    pub(crate) sort: TrackSort,
    pub(crate) durations: &'a HashMap<i32, Option<Duration>>,
    pub(crate) last_played: &'a HashMap<i32, i64>,
    pub(crate) overrides: &'a HashMap<i32, Option<u8>>,
}
impl PlaylistInput<'_> {
    fn prepare_tracks(&self, tracks: &mut [Track], work: &mut Workflow<'_>) {
        let mut dates = Vec::new();
        for track in tracks {
            track.apply_name_preferences(self.preferences);
            if let Some(duration) = self.durations.get(&track.audio_source_id) {
                track.duration = *duration;
            }
            if track.audio_source_id >= 0 {
                track.last_played_at_ms = track
                    .last_played_at_ms
                    .max(self.last_played.get(&track.audio_source_id).copied());
                if let Some(date) = track.last_played_at_ms {
                    dates.push((track.audio_source_id, date));
                }
            }
            if let Some(volume) = self.overrides.get(&track.audio_source_id) {
                track.volume_percent = *volume;
            }
        }
        work.effect(Effect::RememberDates(dates));
    }
}

impl PlaylistsViewModel {
    pub(crate) fn select_item(&mut self, id: i32) -> bool {
        if !self.view.showing_detail() || !self.tracks.iter().any(|row| row.item_id == id) {
            return false;
        }
        self.view.selected_item_id = Some(id);
        true
    }
}

impl PlaylistsViewModel {
    pub(crate) fn apply_preferences(&mut self, preferences: TrackNamePreferences) {
        for row in &mut self.tracks {
            row.track.apply_name_preferences(preferences);
        }
    }
    pub(crate) fn apply_known_dates(&mut self, dates: &HashMap<i32, i64>) -> bool {
        let mut changed = false;
        for row in &mut self.tracks {
            if row.track.audio_source_id >= 0 {
                let date = dates.get(&row.track.audio_source_id).copied();
                changed |= row.track.last_played_at_ms != date;
                row.track.last_played_at_ms = date;
            }
        }
        if let Some(playlist) = &mut self.view.active {
            for item in &mut playlist.items {
                if let Some(id) = item.audio_source_id {
                    item.last_played_at_ms = dates.get(&id).copied();
                }
            }
        }
        changed
    }
}

impl PlaylistsViewModel {
    pub(crate) fn set_duration(&mut self, audio_id: i32, duration: Option<Duration>) {
        for track in self.tracks.iter_mut().map(|row| &mut row.track) {
            if track.audio_source_id == audio_id {
                track.duration = duration;
            }
        }
    }
    pub(crate) fn set_track_volume(&mut self, audio_id: i32, percent: Option<u8>) {
        for track in self.tracks.iter_mut().map(|row| &mut row.track) {
            if track.audio_source_id == audio_id {
                track.volume_percent = percent;
            }
        }
        if let Some(playlist) = &mut self.view.active {
            for item in &mut playlist.items {
                if item.audio_source_id == Some(audio_id) {
                    item.volume_percent = percent;
                }
            }
        }
    }
}
