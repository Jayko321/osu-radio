use crate::{
    controller::{PlaylistAction, PlaylistCandidate, PlaylistsState},
    models::{Playlist, PlaylistItem, PlaylistSummary},
};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MockPlaylists {
    pub view: PlaylistsState,
    #[serde(skip)]
    storage: Vec<Playlist>,
}

fn item(id: i32, playlist_id: i32, name: &str, available: bool) -> PlaylistItem {
    PlaylistItem {
        id,
        playlist_id,
        source_kind: "stable".into(),
        beatmap_hash: format!("demo-{id}"),
        title: Some("Karakara".into()),
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
                active_id: Some(1),
                active: Some(playlist.clone()),
                selected_item_id: Some(1),
                target_id: Some(1),
                playlists: vec![PlaylistSummary {
                    id: 1,
                    name: playlist.name.clone(),
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
        }
    }
}
impl MockPlaylists {
    #[allow(clippy::too_many_lines)] // One offline reducer mirrors the public playlist actions.
    pub fn apply(&mut self, action: PlaylistAction) -> bool {
        let before = self.clone();
        self.view.message.clear();
        match action {
            PlaylistAction::Open | PlaylistAction::OpenAdd => {
                self.view.open = true;
                self.view.adding = matches!(action, PlaylistAction::OpenAdd);
            }
            PlaylistAction::Close => self.view.open = false,
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
                if self.view.active_id == Some(id) {
                    self.view.active_id = None;
                }
            }
            PlaylistAction::Select(id) => {
                self.view.active_id = id;
                self.view.open = false;
            }
            PlaylistAction::SelectItem(id) => self.view.selected_item_id = Some(id),
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
            PlaylistAction::Play { .. } | PlaylistAction::TogglePlayback => {
                self.view.message = "Playback is unavailable in the offline gallery.".into();
            }
            PlaylistAction::Refresh => {}
        }
        self.view.playlists = self
            .storage
            .iter()
            .map(|playlist| PlaylistSummary {
                id: playlist.id,
                name: playlist.name.clone(),
            })
            .collect();
        self.view.active = self
            .storage
            .iter()
            .find(|playlist| Some(playlist.id) == self.view.active_id)
            .cloned();
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
    fn offline_playlists_show_duplicate_audio_unavailable_entries_and_edit_in_memory() {
        let mut demo = MockPlaylists::default();
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
}
