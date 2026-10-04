use super::{AppUpdate, Controller, Track, TrackNamePreferences};

impl Controller {
    pub(super) fn apply_name_preferences(&self, tracks: &mut [Track]) {
        for track in tracks {
            track.apply_name_preferences(self.name_preferences);
        }
    }

    pub(super) fn set_name_preferences(&mut self, preferences: TrackNamePreferences) {
        if self.name_preferences == preferences {
            return;
        }
        self.name_preferences = preferences;
        for track in self
            .tracks
            .iter_mut()
            .chain(&mut self.playlists.tracks)
            .chain(&mut self.upcoming.view.tracks)
            .chain(&mut self.current_track)
            .chain(&mut self.selected_media)
        {
            track.apply_name_preferences(preferences);
        }
        self.reorder_tracks();
        self.emit_queue();
        if self.playlists.view.showing_detail() && self.playlists.view.active.is_none() {
            // Loading retains the old caption; adapters keep the selection unavailable.
            (self.emit)(AppUpdate::TrackSelected(self.selected_media.clone()));
        } else {
            self.emit_selection();
        }
    }
}

#[cfg(test)]
mod tests;
