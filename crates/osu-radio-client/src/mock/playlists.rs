use crate::{
    models::{Playlist, PlaylistItem, PlaylistSummary},
    view_models::{PlaylistAction, PlaylistCandidate, PlaylistsState, TrackSort},
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MockPlaylists {
    pub view: PlaylistsState,
    #[serde(skip)]
    storage: Vec<Playlist>,
    #[serde(skip)]
    track_sort: TrackSort,
    #[serde(skip)]
    covers: HashMap<i32, (i64, Vec<u8>)>,
    #[serde(skip)]
    cover_revision: i64,
}

fn item(id: i32, playlist_id: i32, name: &str, available: bool) -> PlaylistItem {
    PlaylistItem {
        id,
        playlist_id,
        last_played_at_ms: None,
        volume_percent: None,
        source_kind: "stable".into(),
        beatmap_hash: format!("demo-{id}"),
        title: Some("Karakara".into()),
        title_unicode: Some("カラカラ".into()),
        artist_unicode: Some("結束バンド".into()),
        artist: Some("Kessoku Band".into()),
        difficulty_name: Some(name.into()),
        beatmap_id: available.then_some(id),
        beatmap_set_id: available.then_some(1),
        audio_source_id: available.then_some(1),
        cover_beatmap_id: None,
    }
}
impl Default for MockPlaylists {
    fn default() -> Self {
        let playlist = Playlist {
            id: 1,
            name: "Evening".into(),
            items: vec![
                item(1, 1, "Easy", true),
                item(2, 1, "Hard", true),
                item(3, 1, "Insane", false),
            ],
        };
        Self {
            view: PlaylistsState {
                target_id: Some(1),
                playlists: vec![PlaylistSummary {
                    id: 1,
                    name: playlist.name.clone(),
                    item_count: u64::try_from(playlist.items.len()).unwrap_or(u64::MAX),
                    cover_beatmap_id: playlist
                        .items
                        .first()
                        .and_then(|item| item.cover_beatmap_id),
                    custom_cover_revision: None,
                }],
                candidates: vec![
                    PlaylistCandidate {
                        beatmap_id: 1,
                        name: "Easy".into(),
                        checked: true,
                    },
                    PlaylistCandidate {
                        beatmap_id: 2,
                        name: "Hard".into(),
                        checked: true,
                    },
                ],
                ..Default::default()
            },
            storage: vec![playlist],
            track_sort: TrackSort::default(),
            covers: HashMap::new(),
            cover_revision: 0,
        }
    }
}
impl MockPlaylists {
    #[must_use]
    pub fn cover(&self, id: i32, revision: i64) -> Option<&[u8]> {
        self.covers
            .get(&id)
            .filter(|(current, _)| *current == revision)
            .map(|(_, bytes)| bytes.as_slice())
    }

    fn begin_editor(&mut self, id: Option<i32>) {
        let name = match id {
            Some(id) => {
                let Some(playlist) = self.storage.iter().find(|playlist| playlist.id == id) else {
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

    fn close_editor(&mut self) {
        self.view.editor_epoch = self.view.editor_epoch.wrapping_add(1);
        self.view.editor_open = false;
        self.view.editor_id = None;
        self.view.editor_name.clear();
        self.view.editor_cover_png = None;
        self.view.editor_cover_reset = false;
        self.view.message.clear();
    }

    fn save_editor(&mut self) {
        if !self.view.editor_open {
            return;
        }
        let name = self.view.editor_name.trim().to_owned();
        if name.is_empty() {
            self.view.message = "Playlist name cannot be empty.".into();
            return;
        }
        let id = if let Some(id) = self.view.editor_id {
            let Some(playlist) = self.storage.iter_mut().find(|playlist| playlist.id == id) else {
                return;
            };
            playlist.name = name;
            id
        } else {
            let id = self
                .storage
                .iter()
                .map(|playlist| playlist.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            self.storage.push(Playlist {
                id,
                name,
                items: Vec::new(),
            });
            id
        };
        if let Some(png) = self.view.editor_cover_png.take() {
            self.cover_revision = self.cover_revision.saturating_add(1);
            self.covers.insert(id, (self.cover_revision, png));
        } else if self.view.editor_cover_reset {
            self.covers.remove(&id);
        }
        self.view.target_id = Some(id);
        self.close_editor();
    }
    pub(super) fn set_track_sort(&mut self, sort: TrackSort) {
        self.track_sort = sort;
        self.sort_active();
    }
    fn sort_active(&mut self) {
        if let Some(playlist) = &mut self.view.active {
            let mut tracks: Vec<_> = playlist
                .items
                .iter()
                .map(|item| crate::view_models::PlaylistTrack {
                    item_id: item.id,
                    track: item.track(),
                })
                .collect();
            self.track_sort.sort_playlist(playlist, &mut tracks);
        }
    }
    #[allow(clippy::too_many_lines)] // One offline reducer mirrors the public playlist actions.
    pub fn apply(&mut self, action: PlaylistAction) -> bool {
        let before = self.clone();
        if matches!(
            action,
            PlaylistAction::Create(_)
                | PlaylistAction::Rename { .. }
                | PlaylistAction::Delete(_)
                | PlaylistAction::Add
                | PlaylistAction::RemoveItem(_)
        ) {
            self.view.message.clear();
        }
        match action {
            PlaylistAction::ShowLibrary => self.view.visible = false,
            PlaylistAction::ShowPlaylists | PlaylistAction::Open => self.view.visible = true,
            PlaylistAction::Search(query) => self.view.query = query,
            PlaylistAction::BeginCreate => self.begin_editor(None),
            PlaylistAction::BeginEdit(id) => self.begin_editor(Some(id)),
            PlaylistAction::SetEditorName(name) => {
                if self.view.editor_open {
                    self.view.editor_name = name;
                }
            }
            PlaylistAction::CancelEditor => self.close_editor(),
            PlaylistAction::SaveEditor => self.save_editor(),
            PlaylistAction::CompleteCoverPick { epoch, png } => {
                if self.view.editor_open && epoch == self.view.editor_epoch {
                    match png {
                        Ok(Some(bytes)) if bytes.len() <= 2 * 1024 * 1024 => {
                            self.view.editor_epoch = self.view.editor_epoch.wrapping_add(1);
                            self.view.editor_cover_png = Some(bytes);
                            self.view.editor_cover_reset = false;
                            self.view.message.clear();
                        }
                        Ok(Some(_)) => self.view.message = "Playlist cover exceeds 2 MiB.".into(),
                        Ok(None) => {}
                        Err(error) => self.view.message = error,
                    }
                }
            }
            PlaylistAction::ResetEditorCover => {
                if self.view.editor_open {
                    self.view.editor_epoch = self.view.editor_epoch.wrapping_add(1);
                    self.view.editor_cover_png = None;
                    self.view.editor_cover_reset = true;
                    self.view.message.clear();
                }
            }
            PlaylistAction::RequestCover { .. }
            | PlaylistAction::RequestArtwork { .. }
            | PlaylistAction::Refresh => {}
            PlaylistAction::OpenAdd => {
                self.view.open = true;
                self.view.adding = true;
            }
            PlaylistAction::Close => {
                self.view.open = false;
                self.view.adding = false;
            }
            PlaylistAction::Create(name) => {
                let name = name.trim();
                if name.is_empty() {
                    self.view.message = "Playlist name cannot be empty.".into();
                } else {
                    let id = self
                        .storage
                        .iter()
                        .map(|playlist| playlist.id)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1);
                    self.storage.push(Playlist {
                        id,
                        name: name.into(),
                        items: Vec::new(),
                    });
                    self.view.target_id = Some(id);
                }
            }
            PlaylistAction::Rename { id, name } => {
                if name.trim().is_empty() {
                    self.view.message = "Playlist name cannot be empty.".into();
                } else if let Some(playlist) =
                    self.storage.iter_mut().find(|playlist| playlist.id == id)
                {
                    playlist.name = name.trim().into();
                }
            }
            PlaylistAction::Delete(id) => {
                self.storage.retain(|playlist| playlist.id != id);
                self.covers.remove(&id);
                if self.view.editor_id == Some(id) {
                    self.close_editor();
                }
                if self.view.active_id == Some(id) {
                    self.view.active_id = None;
                }
            }
            PlaylistAction::Select(id) => {
                if id.is_some() {
                    self.view.visible = true;
                }
                self.view.active_id = id;
                self.view.selected_item_id = None;
                self.view.open = false;
            }
            PlaylistAction::SelectItem(id) => {
                if self.view.showing_detail()
                    && self
                        .view
                        .active
                        .as_ref()
                        .is_some_and(|playlist| playlist.items.iter().any(|item| item.id == id))
                {
                    self.view.selected_item_id = Some(id);
                }
            }
            PlaylistAction::ChooseTarget(id) => self.view.target_id = Some(id),
            PlaylistAction::ToggleDifficulty(id) => {
                if let Some(candidate) = self
                    .view
                    .candidates
                    .iter_mut()
                    .find(|candidate| candidate.beatmap_id == id)
                {
                    candidate.checked = !candidate.checked;
                }
            }
            PlaylistAction::Add => {
                if let Some(playlist) = self
                    .storage
                    .iter_mut()
                    .find(|playlist| Some(playlist.id) == self.view.target_id)
                {
                    for candidate in self
                        .view
                        .candidates
                        .iter()
                        .filter(|candidate| candidate.checked)
                    {
                        if !playlist.items.iter().any(|item| {
                            item.beatmap_hash == format!("demo-{}", candidate.beatmap_id)
                        }) {
                            playlist.items.push(item(
                                candidate.beatmap_id,
                                playlist.id,
                                &candidate.name,
                                true,
                            ));
                        }
                    }
                    self.view.open = false;
                }
            }
            PlaylistAction::RemoveItem(id) => {
                if let Some(playlist) = self
                    .storage
                    .iter_mut()
                    .find(|playlist| Some(playlist.id) == self.view.active_id)
                {
                    playlist.items.retain(|item| item.id != id);
                }
            }
            PlaylistAction::AddToQueue(_)
            | PlaylistAction::Play { .. }
            | PlaylistAction::TogglePlayback => {
                self.view.message = "Playback is unavailable in the offline gallery.".into();
            }
        }
        self.view.playlists = self
            .storage
            .iter()
            .map(|playlist| PlaylistSummary {
                id: playlist.id,
                name: playlist.name.clone(),
                item_count: u64::try_from(playlist.items.len()).unwrap_or(u64::MAX),
                cover_beatmap_id: playlist
                    .items
                    .first()
                    .and_then(|item| item.cover_beatmap_id),
                custom_cover_revision: self.covers.get(&playlist.id).map(|(revision, _)| *revision),
            })
            .collect();
        self.view.active = self
            .storage
            .iter()
            .find(|playlist| Some(playlist.id) == self.view.active_id)
            .cloned();
        self.sort_active();
        self.view.selected_item_id = self
            .view
            .active
            .as_ref()
            .and_then(|playlist| {
                playlist
                    .items
                    .iter()
                    .find(|item| Some(item.id) == self.view.selected_item_id)
                    .or_else(|| playlist.items.first())
            })
            .map(|item| item.id);
        self.view.target_id = self
            .view
            .target_id
            .filter(|id| self.storage.iter().any(|playlist| playlist.id == *id))
            .or_else(|| self.storage.first().map(|playlist| playlist.id));
        self != &before
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    #[test]
    fn editor_cover_errors_stale_epochs_cancel_and_delete_preserve_storage() {
        let mut demo = MockPlaylists::default();
        let original = demo.storage.clone();
        assert!(!demo.apply(PlaylistAction::BeginEdit(999)));
        assert!(!demo.apply(PlaylistAction::SetEditorName("closed".into())));
        assert!(!demo.apply(PlaylistAction::SaveEditor));
        assert!(!demo.apply(PlaylistAction::ResetEditorCover));
        demo.apply(PlaylistAction::BeginEdit(1));
        let epoch = demo.view.editor_epoch;
        for (png, error) in [
            (
                Ok(Some(vec![0; 2 * 1024 * 1024 + 1])),
                "Playlist cover exceeds 2 MiB.",
            ),
            (Err("picker failed".into()), "picker failed"),
        ] {
            assert!(demo.apply(PlaylistAction::CompleteCoverPick { epoch, png }));
            assert_eq!(demo.view.message, error);
            assert!(demo.view.editor_cover_png.is_none());
            assert_eq!(demo.view.editor_epoch, epoch);
            assert_eq!(demo.storage, original);
        }
        let before = demo.clone();
        assert!(!demo.apply(PlaylistAction::CompleteCoverPick {
            epoch,
            png: Ok(None)
        }));
        assert_eq!(demo, before);
        let png = vec![42; 2 * 1024 * 1024];
        demo.apply(PlaylistAction::CompleteCoverPick {
            epoch,
            png: Ok(Some(png.clone())),
        });
        assert!(demo.view.message.is_empty());
        assert!(!demo.apply(PlaylistAction::CompleteCoverPick {
            epoch,
            png: Err("stale".into())
        }));
        let serialized = serde_json::to_value(&demo).unwrap();
        assert!(serialized.get("storage").is_none());
        assert!(serialized.get("covers").is_none());
        assert!(serialized["view"].get("editor_cover_png").is_none());
        demo.apply(PlaylistAction::SetEditorName("  夜 / Вечер  ".into()));
        demo.apply(PlaylistAction::SaveEditor);
        assert_eq!(demo.storage.first().unwrap().name, "夜 / Вечер");
        let revision = demo
            .view
            .playlists
            .first()
            .unwrap()
            .custom_cover_revision
            .unwrap();
        assert_eq!(demo.cover(1, revision), Some(png.as_slice()));
        assert!(demo.cover(1, revision + 1).is_none());
        assert!(demo.cover(999, revision).is_none());
        demo.apply(PlaylistAction::BeginEdit(1));
        demo.apply(PlaylistAction::ResetEditorCover);
        demo.apply(PlaylistAction::CancelEditor);
        assert_eq!(demo.cover(1, revision), Some(png.as_slice()));
        assert!(demo.view.editor_name.is_empty());
        assert!(!demo.view.editor_open && !demo.view.editor_cover_reset);
        demo.apply(PlaylistAction::BeginEdit(1));
        demo.apply(PlaylistAction::Delete(1));
        assert!(!demo.view.editor_open);
        assert!(demo.cover(1, revision).is_none());
        assert!(demo.storage.is_empty() && demo.view.playlists.is_empty());
        assert_eq!(demo.view.target_id, None);
    }

    #[test]
    fn sorting_selection_and_offline_controls_keep_saved_item_order() {
        let mut demo = MockPlaylists::default();
        let playlist = demo.storage.first_mut().unwrap();
        playlist.items[0].title = Some("Zulu".into());
        playlist.items[0].last_played_at_ms = Some(20);
        playlist.items[1].title = Some("Alpha".into());
        playlist.items[2].title = Some("Middle".into());
        playlist.items[2].last_played_at_ms = Some(30);
        let original = demo.storage.clone();
        assert!(demo.apply(PlaylistAction::Select(Some(1))));
        assert_eq!(
            demo.view
                .active
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            [2, 3, 1]
        );
        assert_eq!(demo.view.selected_item_id, Some(2));
        assert!(demo.apply(PlaylistAction::SelectItem(3)));
        assert!(!demo.apply(PlaylistAction::SelectItem(999)));
        demo.set_track_sort(TrackSort::RecentlyPlayed);
        assert_eq!(
            demo.view
                .active
                .as_ref()
                .unwrap()
                .items
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            [3, 1, 2]
        );
        assert_eq!(demo.view.selected_item_id, Some(3));
        assert_eq!(demo.storage, original);
        for action in [
            PlaylistAction::AddToQueue(3),
            PlaylistAction::Play {
                id: 1,
                start_item_id: Some(3),
            },
            PlaylistAction::TogglePlayback,
        ] {
            demo.apply(action);
            assert_eq!(
                demo.view.message,
                "Playback is unavailable in the offline gallery."
            );
            assert_eq!(demo.storage, original);
        }
        demo.apply(PlaylistAction::ShowLibrary);
        assert!(!demo.apply(PlaylistAction::SelectItem(1)));
        assert_eq!(demo.view.selected_item_id, Some(3));
        demo.apply(PlaylistAction::Select(Some(1)));
        assert_eq!(demo.view.selected_item_id, Some(3));
        demo.apply(PlaylistAction::SelectItem(2));
        assert_eq!(demo.view.selected_item_id, Some(2));
        demo.apply(PlaylistAction::RemoveItem(2));
        assert_eq!(demo.view.selected_item_id, Some(3));
        demo.apply(PlaylistAction::Create("  ".into()));
        assert_eq!(demo.view.message, "Playlist name cannot be empty.");
        demo.apply(PlaylistAction::Rename {
            id: 1,
            name: "  Renamed  ".into(),
        });
        assert_eq!(demo.view.active.as_ref().unwrap().name, "Renamed");
        demo.apply(PlaylistAction::ChooseTarget(999));
        assert_eq!(demo.view.target_id, Some(1));
        demo.apply(PlaylistAction::Select(Some(999)));
        assert!(demo.view.active.is_none() && demo.view.selected_item_id.is_none());
    }
    #[test]
    fn offline_playlists_show_duplicate_audio_unavailable_entries_and_edit_in_memory() {
        let mut demo = MockPlaylists::default();
        demo.apply(PlaylistAction::Select(Some(1)));
        let items = &demo.view.active.as_ref().unwrap().items;
        assert_eq!(items[0].audio_source_id, items[1].audio_source_id);
        assert!(items[2].audio_source_id.is_none());
        demo.apply(PlaylistAction::Create(" New ".into()));
        let id = demo.view.target_id.unwrap();
        demo.apply(PlaylistAction::Add);
        demo.apply(PlaylistAction::Select(Some(id)));
        assert_eq!(demo.view.active.as_ref().unwrap().items.len(), 2);
        demo.apply(PlaylistAction::Add);
        assert_eq!(demo.view.active.as_ref().unwrap().items.len(), 2);
        demo.apply(PlaylistAction::RemoveItem(1));
        assert_eq!(demo.view.active.as_ref().unwrap().items.len(), 1);
        demo.apply(PlaylistAction::Delete(id));
        assert!(demo.view.active.is_none());
    }
    #[test]
    fn offline_editor_counts_covers_tabs_and_selection_match_live_state() {
        let mut demo = MockPlaylists::default();
        assert!(demo.view.active.is_none());
        assert_eq!(demo.view.playlists[0].item_count, 3);
        demo.apply(PlaylistAction::Select(Some(1)));
        demo.apply(PlaylistAction::SelectItem(2));
        demo.apply(PlaylistAction::Search("evening".into()));
        demo.apply(PlaylistAction::ShowLibrary);
        assert_eq!(demo.view.selected_item_id, Some(2));
        demo.apply(PlaylistAction::ShowPlaylists);
        assert!(demo.view.showing_detail());
        assert_eq!(demo.view.query, "evening");
        demo.apply(PlaylistAction::BeginCreate);
        demo.apply(PlaylistAction::SaveEditor);
        assert!(demo.view.editor_open && !demo.view.message.is_empty());
        demo.apply(PlaylistAction::SetEditorName("New playlist".into()));
        let epoch = demo.view.editor_epoch;
        demo.apply(PlaylistAction::CompleteCoverPick {
            epoch,
            png: Ok(Some(vec![1, 2])),
        });
        demo.apply(PlaylistAction::SaveEditor);
        let id = demo.view.target_id.unwrap();
        let revision = demo
            .view
            .playlists
            .iter()
            .find(|playlist| playlist.id == id)
            .unwrap()
            .custom_cover_revision
            .unwrap();
        assert_eq!(demo.cover(id, revision), Some([1, 2].as_slice()));
        assert!(!demo.view.editor_open);
        demo.apply(PlaylistAction::BeginEdit(id));
        demo.apply(PlaylistAction::ResetEditorCover);
        demo.apply(PlaylistAction::CompleteCoverPick {
            epoch,
            png: Ok(Some(vec![3])),
        });
        assert!(demo.view.editor_cover_png.is_none());
        demo.apply(PlaylistAction::SaveEditor);
        assert!(demo.cover(id, revision).is_none());
        assert!(
            demo.view
                .playlists
                .iter()
                .find(|playlist| playlist.id == id)
                .unwrap()
                .custom_cover_revision
                .is_none()
        );
        demo.apply(PlaylistAction::Select(None));
        assert!(demo.view.visible && demo.view.active.is_none());
    }
}
