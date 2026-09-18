//! Bundled, in-memory demonstration state. No session, persistence, media I/O or toolkit.
//!
//! The order of the four samples is stable so frontends can associate their own artwork.
//! Durations are demonstration values; selecting a song does not start playback.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MockTrack {
    pub id: u32,
    pub title: String,
    pub artist: String,
    pub subtitle: String,
    pub duration_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MockState {
    pub tracks: Vec<MockTrack>,
    pub selected: usize,
    pub search: String,
    pub gallery: GalleryState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GalleryState {
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

/// Frontends translate their events to these toolkit-independent actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    SelectTrack(usize),
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
        Self {
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
                },
            )
            .collect(),
            selected: 0,
            search: String::new(),
            gallery: GalleryState::default(),
        }
    }
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
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
            Action::Search(value) => replace(&mut self.search, value),
            Action::GalleryDisabled(value) => replace(&mut self.gallery.disabled, value),
            _ if self.gallery.disabled => false,
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
    fn selection_starts_at_first_sample_and_ignores_invalid_or_repeated_indices() {
        let mut state = MockState::default();
        assert_eq!(state.tracks.len(), 4);
        assert_eq!(state.selected, 0);
        assert_eq!(state.tracks.first().unwrap().title, "Karakara");
        assert!(!state.apply(Action::SelectTrack(0)));
        assert!(state.apply(Action::SelectTrack(2)));
        assert_eq!(
            state.tracks.get(state.selected).unwrap().title,
            "Rabbit Hole"
        );
        assert_eq!(
            state.tracks.get(state.selected).unwrap().duration_seconds,
            161
        );
        let selected = state.clone();
        for index in [2, state.tracks.len(), usize::MAX] {
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
