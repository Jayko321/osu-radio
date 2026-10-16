//! Toolkit-free presentation state and domain workflows.
pub(crate) mod folders;
pub(crate) mod library;
pub(crate) mod player;
pub(crate) mod playlists;
#[cfg(test)]
mod preferences;
pub(crate) mod queue;
pub(crate) mod settings;
mod sorting;
mod track;
pub(crate) mod volume;

pub use folders::{FolderAction, FolderSelection, FolderSelectionRow};
pub use library::LibraryViewModel;
pub use player::{PlayerPresentation, PlayerViewModel};
pub use playlists::{
    PlaylistAction, PlaylistCandidate, PlaylistTrack, PlaylistsState, PlaylistsViewModel,
};
pub use queue::{QueueAction, QueueRow, QueueView, QueueViewModel};
pub use settings::{SettingsGroup, SettingsPreferences, SettingsViewModel};
pub use sorting::TrackSort;
pub use track::{Track, TrackNamePreferences, library_tracks, selection_after_refresh};
pub use volume::AudioSettingsState;
