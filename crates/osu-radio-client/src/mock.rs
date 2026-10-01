//! Bundled, in-memory demonstration state. No session, persistence, media I/O or toolkit.
//!
//! Sample IDs are stable so frontends can associate their own artwork after sorting.
//! Durations are demonstration values; selecting a song does not start playback.

use crate::controller::TrackSort;
use serde::Serialize;
mod playlists;
pub use playlists::MockPlaylists;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MockTrack {
    pub id: u32,
    pub title: String,
    pub artist: String,
    pub subtitle: String,
    pub duration_seconds: u32,
    pub last_played_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MockState {
    pub tracks: Vec<MockTrack>,
    pub track_sort: TrackSort,
    pub selected: usize,
    pub search: String,
    pub gallery: GalleryState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GalleryState {
    pub playlists: MockPlaylists,
    pub folder_rows: Vec<MockFolderRow>,
    pub folder_message: String,
    pub disabled: bool,
    pub presses: u32,
    pub field: String,
    pub filled: String,
    pub search: String,
    pub playlist_name: String,
    pub switches: [bool; 2],
    /// Exactly one of the three demonstration tabs is selected.
    pub tab: usize,
    pub tag: bool,
    /// Neutral (0), included (1), or excluded (2).
    pub filter: u8,
    pub menu_query: String,
    pub menu_selected: usize,
    pub menu_items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockFolderRow {
    pub kind: String,
    pub path: String,
    pub marker_path: String,
    pub registered: bool,
    pub action: String,
    pub count: String,
    pub count_pending: bool,
    pub count_error: String,
    pub error: String,
}
fn folder_rows() -> Vec<MockFolderRow> {
    [
        ("stable", true, "5324", ""),
        ("lazer", false, "0", ""),
        ("stable", false, "", "Preview failed"),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(index, (kind, registered, count, count_error))| MockFolderRow {
            kind: kind.into(),
            path: format!("/demo/osu/{index}"),
            marker_path: format!("/demo/osu/{index}/marker"),
            registered,
            action: String::new(),
            count: count.into(),
            count_pending: false,
            count_error: count_error.into(),
            error: String::new(),
        },
    )
    .collect()
}

/// Frontends translate their events to these toolkit-independent actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Playlist(crate::controller::PlaylistAction),
    FolderReset,
    FolderToggle(String),
    FolderCount(String),
    FolderBrowse,
    FolderApply,
    SelectTrack(usize),
    SetTrackSort(TrackSort),
    Search(String),
    GalleryDisabled(bool),
    Press,
    Field(String),
    Filled(String),
    GallerySearch(String),
    PlaylistName(String),
    ToggleSwitch(usize),
    SelectTab(usize),
    ToggleTag,
    CycleFilter,
    MenuQuery(String),
    SelectMenu(usize),
}

impl Default for MockState {
    fn default() -> Self {
        let mut state = Self {
            tracks: [
                (1, "Karakara", "Kessoku Band", "結束バンド", 265),
                (2, "Alice", "FELT", "Bundled sample", 238),
                (3, "Rabbit Hole", "DECO*27", "feat. Hatsune Miku", 161),
                (4, "Bling-Bang-Bang-Born", "Creepy Nuts", "B.B.B.B.", 168),
            ]
            .into_iter()
            .map(
                |(id, title, artist, subtitle, duration_seconds)| MockTrack {
                    id,
                    title: title.into(),
                    artist: artist.into(),
                    subtitle: subtitle.into(),
                    duration_seconds,
                    last_played_at_ms: match id {
                        1 | 4 => Some(2_000),
                        3 => Some(3_000),
                        _ => None,
                    },
                },
            )
            .collect(),
            selected: 0,
            track_sort: TrackSort::default(),
            search: String::new(),
            gallery: GalleryState::default(),
        };
        state.sort_tracks();
        state.selected = 0;
        state
    }
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            folder_rows: folder_rows(),
            playlists: MockPlaylists::default(),
            folder_message: String::new(),
            disabled: false,
            presses: 0,
            field: String::new(),
            filled: "A very long playlist title · 夜に駆ける · Музыка для вечера".into(),
            search: String::new(),
            playlist_name: String::new(),
            switches: [false, true],
            tab: 0,
            tag: false,
            filter: 0,
            menu_query: String::new(),
            menu_selected: 0,
            menu_items: (0..30)
                .map(|index| {
                    if index == 0 {
                        "A very long playlist name — favourites for a quiet evening / お気に入り / Избранное".into()
                    } else {
                        format!("Playlist {index:02} · late night radio")
                    }
                })
                .collect(),
        }
    }
}

impl GalleryState {
    fn apply_folders(&mut self) {
        for row in &mut self.folder_rows {
            if row.action.is_empty() {
                continue;
            }
            if row.path == "/demo/osu/2" && row.error.is_empty() {
                row.error = "Demo import failed. Apply again to retry.".into();
            } else {
                row.registered = row.action == "add";
                row.action.clear();
                row.error.clear();
            }
        }
        self.folder_message = if self.folder_rows.iter().any(|row| !row.action.is_empty()) {
            "Some changes failed. Successful changes are saved.".into()
        } else {
            String::new()
        };
    }
    /// Search results retain original indices, including after Unicode case conversion.
    #[must_use]
    pub fn filtered_menu(&self) -> Vec<(usize, &str)> {
        let query = self.menu_query.to_lowercase();
        self.menu_items
            .iter()
            .enumerate()
            .filter(|(_, label)| label.to_lowercase().contains(&query))
            .map(|(index, label)| (index, label.as_str()))
            .collect()
    }
}

impl MockState {
    fn sort_tracks(&mut self) {
        let selected = self.tracks.get(self.selected).map(|track| track.id);
        self.tracks.sort_by_cached_key(|track| {
            self.track_sort.key(
                &track.title,
                &track.artist,
                track.last_played_at_ms,
                i64::from(track.id),
            )
        });
        self.selected = self
            .tracks
            .iter()
            .position(|track| Some(track.id) == selected)
            .unwrap_or(0);
        self.gallery.playlists.set_track_sort(self.track_sort);
    }
    /// Applies an action and reports whether an observer needs a new snapshot.
    ///
    /// Invalid selections, unchanged values and disabled gallery actions are no-ops.
    /// Songs remain interactive when gallery controls are disabled. Song search only
    /// stores the entered text; it deliberately does not filter the sample list.
    pub fn apply(&mut self, action: Action) -> bool {
        match action {
            Action::SelectTrack(index) => {
                index < self.tracks.len() && replace(&mut self.selected, index)
            }
            Action::SetTrackSort(sort) => {
                if !replace(&mut self.track_sort, sort) {
                    return false;
                }
                self.sort_tracks();
                true
            }
            Action::Search(value) => replace(&mut self.search, value),
            Action::GalleryDisabled(value) => replace(&mut self.gallery.disabled, value),
            _ if self.gallery.disabled => false,
            Action::Playlist(action) => self.gallery.playlists.apply(action),
            Action::FolderReset => {
                self.gallery.folder_rows = folder_rows();
                self.gallery.folder_message.clear();
                true
            }
            Action::FolderToggle(path) => self
                .gallery
                .folder_rows
                .iter_mut()
                .find(|row| row.marker_path == path)
                .is_some_and(|row| {
                    row.action = if row.action.is_empty() {
                        if row.registered { "remove" } else { "add" }.into()
                    } else {
                        String::new()
                    };
                    row.error.clear();
                    true
                }),
            Action::FolderCount(path) => self
                .gallery
                .folder_rows
                .iter_mut()
                .find(|row| row.marker_path == path)
                .is_some_and(|row| {
                    row.count = "42".into();
                    row.count_error.clear();
                    true
                }),
            Action::FolderBrowse => {
                if self.gallery.folder_rows.iter().any(|row| row.count_pending) {
                    return false;
                }
                self.gallery.folder_rows.push(MockFolderRow {
                    kind: "lazer".into(),
                    path: "/demo/selected/osu".into(),
                    marker_path: "/demo/selected/osu/client.realm".into(),
                    registered: false,
                    action: "add".into(),
                    count: String::new(),
                    count_pending: true,
                    count_error: String::new(),
                    error: String::new(),
                });
                true
            }
            Action::FolderApply => {
                self.gallery.apply_folders();
                true
            }
            Action::Press => {
                let next = self.gallery.presses.saturating_add(1);
                replace(&mut self.gallery.presses, next)
            }
            Action::Field(value) => replace(&mut self.gallery.field, value),
            Action::Filled(value) => replace(&mut self.gallery.filled, value),
            Action::GallerySearch(value) => replace(&mut self.gallery.search, value),
            Action::PlaylistName(value) => replace(&mut self.gallery.playlist_name, value),
            Action::ToggleSwitch(index) => {
                self.gallery.switches.get_mut(index).is_some_and(|value| {
                    *value = !*value;
                    true
                })
            }
            Action::SelectTab(index) => index < 3 && replace(&mut self.gallery.tab, index),
            Action::ToggleTag => {
                self.gallery.tag = !self.gallery.tag;
                true
            }
            Action::CycleFilter => {
                self.gallery.filter = match self.gallery.filter {
                    0 => 1,
                    1 => 2,
                    _ => 0,
                };
                true
            }
            Action::MenuQuery(value) => replace(&mut self.gallery.menu_query, value),
            Action::SelectMenu(index) => {
                index < self.gallery.menu_items.len()
                    && replace(&mut self.gallery.menu_selected, index)
            }
        }
    }
}

fn replace<T: PartialEq>(slot: &mut T, value: T) -> bool {
    if *slot == value {
        return false;
    }
    *slot = value;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_sorts_all_modes_by_stable_id_and_preserves_selected_song() {
        let mut state = MockState::default();
        assert_eq!(
            state
                .tracks
                .iter()
                .map(|track| track.id)
                .collect::<Vec<_>>(),
            [2, 4, 1, 3]
        );
        state.apply(Action::SelectTrack(2));
        for (sort, expected) in [
            (TrackSort::ArtistAsc, vec![4, 3, 2, 1]),
            (TrackSort::RecentlyPlayed, vec![3, 4, 1, 2]),
            (TrackSort::TitleAsc, vec![2, 4, 1, 3]),
        ] {
            assert!(state.apply(Action::SetTrackSort(sort)));
            assert_eq!(
                state
                    .tracks
                    .iter()
                    .map(|track| track.id)
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(state.tracks.get(state.selected).unwrap().id, 1);
            assert_eq!(
                state.tracks.get(state.selected).unwrap().duration_seconds,
                265
            );
            assert!(!state.apply(Action::SetTrackSort(sort)));
        }
    }

    #[test]
    fn selection_starts_at_first_sample_and_ignores_invalid_or_repeated_indices() {
        let mut state = MockState::default();
        assert_eq!(state.tracks.len(), 4);
        assert_eq!(state.selected, 0);
        assert_eq!(state.tracks.first().unwrap().title, "Alice");
        assert!(!state.apply(Action::SelectTrack(0)));
        assert!(state.apply(Action::SelectTrack(3)));
        assert_eq!(
            state.tracks.get(state.selected).unwrap().title,
            "Rabbit Hole"
        );
        assert_eq!(
            state.tracks.get(state.selected).unwrap().duration_seconds,
            161
        );
        let selected = state.clone();
        for index in [3, state.tracks.len(), usize::MAX] {
            assert!(!state.apply(Action::SelectTrack(index)));
            assert_eq!(state, selected);
        }
    }

    #[test]
    fn song_search_preserves_samples_order_and_selection() {
        let mut state = MockState::default();
        assert!(state.apply(Action::SelectTrack(3)));
        let tracks = state.tracks.clone();
        for query in ["Rabbit", "no matching song", "夜に駆ける", ""] {
            assert!(state.apply(Action::Search(query.into())));
            assert_eq!(state.search, query);
            assert_eq!(state.tracks, tracks);
            assert_eq!(state.selected, 3);
            assert!(!state.apply(Action::Search(query.into())));
        }
        assert_eq!(state.gallery.search, "");
    }

    #[test]
    fn gallery_tags_cycle_and_tab_selection_is_exclusive() {
        let mut state = MockState::default();
        for expected in [1, 2, 0, 1] {
            assert!(state.apply(Action::CycleFilter));
            assert_eq!(state.gallery.filter, expected);
        }
        assert!(state.apply(Action::ToggleTag));
        assert!(state.gallery.tag);
        assert!(state.apply(Action::ToggleTag));
        assert!(!state.gallery.tag);
        for index in [1, 2, 0] {
            assert!(state.apply(Action::SelectTab(index)));
            assert_eq!(state.gallery.tab, index);
            assert!(!state.apply(Action::SelectTab(index)));
        }
        assert!(!state.apply(Action::SelectTab(3)));
        assert!(!state.apply(Action::SelectTab(usize::MAX)));
        assert_eq!(state.gallery.tab, 0);
    }

    #[test]
    fn menu_search_preserves_original_indices_and_selected_item() {
        let mut state = MockState::default();
        assert_eq!(state.gallery.filtered_menu().len(), 30);
        assert!(state.apply(Action::MenuQuery("PLAYLIST 12".into())));
        assert_eq!(
            state.gallery.filtered_menu(),
            [(12, "Playlist 12 · late night radio")]
        );
        assert!(state.apply(Action::SelectMenu(12)));
        assert!(state.apply(Action::MenuQuery("изБРАННОЕ".into())));
        assert_eq!(state.gallery.filtered_menu().first().unwrap().0, 0);
        assert_eq!(state.gallery.menu_selected, 12);
        assert!(state.apply(Action::MenuQuery("no matching menu option".into())));
        assert!(state.gallery.filtered_menu().is_empty());
        assert_eq!(state.gallery.menu_selected, 12);
        assert!(!state.apply(Action::SelectMenu(30)));
        assert!(!state.apply(Action::SelectMenu(usize::MAX)));
        state.gallery.menu_items.clear();
        assert!(!state.apply(Action::SelectMenu(0)));
        assert!(state.gallery.filtered_menu().is_empty());
    }

    #[test]
    fn fields_and_switches_are_independent_and_only_changes_notify() {
        let mut state = MockState::default();
        for action in [
            Action::Field("field".into()),
            Action::Filled("filled".into()),
            Action::GallerySearch("search".into()),
            Action::PlaylistName("playlist".into()),
        ] {
            assert!(state.apply(action.clone()));
            assert!(!state.apply(action));
        }
        assert_eq!(state.gallery.field, "field");
        assert_eq!(state.gallery.filled, "filled");
        assert_eq!(state.gallery.search, "search");
        assert_eq!(state.gallery.playlist_name, "playlist");
        assert!(state.search.is_empty());
        assert!(state.apply(Action::ToggleSwitch(0)));
        assert_eq!(state.gallery.switches, [true, true]);
        assert!(state.apply(Action::ToggleSwitch(1)));
        assert_eq!(state.gallery.switches, [true, false]);
        assert!(!state.apply(Action::ToggleSwitch(2)));
        assert!(!state.apply(Action::ToggleSwitch(usize::MAX)));
        assert_eq!(state.gallery.switches, [true, false]);
        assert!(state.apply(Action::Press));
        assert_eq!(state.gallery.presses, 1);
        state.gallery.presses = u32::MAX;
        assert!(!state.apply(Action::Press));
        assert_eq!(state.gallery.presses, u32::MAX);
    }

    #[test]
    fn disabled_gallery_suppresses_every_demo_action_and_can_be_reenabled() {
        let mut state = MockState::default();
        assert!(state.apply(Action::GalleryDisabled(true)));
        assert!(!state.apply(Action::GalleryDisabled(true)));
        let disabled = state.clone();
        for action in [
            Action::Press,
            Action::Field("new".into()),
            Action::Filled("new".into()),
            Action::GallerySearch("new".into()),
            Action::PlaylistName("new".into()),
            Action::ToggleSwitch(0),
            Action::ToggleSwitch(1),
            Action::SelectTab(1),
            Action::ToggleTag,
            Action::CycleFilter,
            Action::MenuQuery("new".into()),
            Action::SelectMenu(1),
        ] {
            assert!(!state.apply(action));
            assert_eq!(state, disabled);
        }
        assert!(state.apply(Action::SelectTrack(1)));
        assert!(state.apply(Action::Search("still editable".into())));
        assert_eq!(state.gallery, disabled.gallery);
        assert!(state.apply(Action::GalleryDisabled(false)));
        assert!(state.apply(Action::Press));
        assert_eq!(state.gallery.presses, 1);
    }
}
