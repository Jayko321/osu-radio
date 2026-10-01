use super::{AppUpdate, Controller, Track};
use crate::models::Playlist;
use std::cmp::Reverse;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub enum TrackSort {
    #[default]
    TitleAsc,
    ArtistAsc,
    RecentlyPlayed,
}

impl TrackSort {
    pub(crate) fn key(
        self,
        title: &str,
        artist: &str,
        last_played_at_ms: Option<i64>,
        id: i64,
    ) -> (Reverse<Option<i64>>, String, String, i64) {
        let (first, second) = if self == Self::ArtistAsc {
            (artist, title)
        } else {
            (title, artist)
        };
        (
            Reverse(
                (self == Self::RecentlyPlayed)
                    .then_some(last_played_at_ms)
                    .flatten(),
            ),
            first.to_lowercase(),
            second.to_lowercase(),
            id,
        )
    }

    pub(crate) fn sort_tracks(self, tracks: &mut [Track]) {
        tracks.sort_by_cached_key(|track| {
            self.key(
                &track.title,
                &track.artist,
                track.last_played_at_ms,
                i64::from(track.audio_source_id),
            )
        });
    }

    pub(crate) fn sort_playlist(self, playlist: &mut Playlist, tracks: &mut Vec<Track>) {
        let mut rows: Vec<_> = std::mem::take(&mut playlist.items)
            .into_iter()
            .zip(std::mem::take(tracks))
            .collect();
        rows.sort_by_cached_key(|(item, track)| {
            self.key(
                &track.title,
                &track.artist,
                track.last_played_at_ms,
                i64::from(item.id),
            )
        });
        (playlist.items, *tracks) = rows.into_iter().unzip();
    }
}

impl Controller {
    pub(super) fn merge_last_played(&mut self, tracks: &mut [Track]) {
        for track in tracks.iter() {
            if track.audio_source_id >= 0
                && let Some(date) = track.last_played_at_ms
            {
                let known = self
                    .last_played
                    .entry(track.audio_source_id)
                    .or_insert(date);
                *known = (*known).max(date);
            }
        }
        for track in tracks {
            if track.audio_source_id >= 0 {
                track.last_played_at_ms = self.last_played.get(&track.audio_source_id).copied();
            }
        }
    }

    pub(super) fn assignment_last_played(&mut self) {
        if let Some(track) = &mut self.current_track {
            if let Some(date) = track.last_played_at_ms {
                let known = self
                    .last_played
                    .entry(track.audio_source_id)
                    .or_insert(date);
                *known = (*known).max(date);
            }
            track.last_played_at_ms = self.last_played.get(&track.audio_source_id).copied();
        }
        let mut changed = false;
        for track in self.tracks.iter_mut().chain(&mut self.playlists.tracks) {
            if track.audio_source_id < 0 {
                continue;
            }
            let date = self.last_played.get(&track.audio_source_id).copied();
            changed |= track.last_played_at_ms != date;
            track.last_played_at_ms = date;
        }
        if let Some(playlist) = &mut self.playlists.view.active {
            for item in &mut playlist.items {
                if let Some(id) = item.audio_source_id {
                    item.last_played_at_ms = self.last_played.get(&id).copied();
                }
            }
        }
        if changed {
            self.reorder_tracks();
        }
    }

    pub(super) fn set_track_sort(&mut self, sort: TrackSort) {
        if self.track_sort == sort {
            return;
        }
        self.track_sort = sort;
        (self.emit)(AppUpdate::TrackSort(sort));
        self.reorder_tracks();
        self.emit_selection();
    }

    fn reorder_tracks(&mut self) {
        self.track_sort.sort_tracks(&mut self.tracks);
        self.track_indices = self
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| (track.audio_source_id, index))
            .collect();
        if let Some(playlist) = &mut self.playlists.view.active {
            self.track_sort
                .sort_playlist(playlist, &mut self.playlists.tracks);
        }
        let tracks = if self.playlists.view.showing_detail() {
            // Adapters pair rows with playlist item IDs by index. Publish that order first.
            self.emit_playlists();
            self.playlists.tracks.clone()
        } else {
            self.tracks.clone()
        };
        (self.emit)(AppUpdate::TracksReordered(tracks));
    }
}

#[cfg(test)]
mod tests;
