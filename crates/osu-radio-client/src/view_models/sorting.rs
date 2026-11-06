use super::Track;
#[cfg(test)]
use crate::controller::AppUpdate;
use crate::models::Playlist;
use std::cmp::Reverse;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub enum TrackSort {
    #[default]
    TitleAsc,
    ArtistAsc,
    RecentlyPlayed,
    RecentlyListened,
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
                matches!(self, Self::RecentlyPlayed | Self::RecentlyListened)
                    .then_some(last_played_at_ms)
                    .flatten(),
            ),
            first.to_lowercase(),
            second.to_lowercase(),
            id,
        )
    }

    fn date(self, track: &Track) -> Option<i64> {
        match self {
            Self::RecentlyPlayed => track
                .difficulties
                .iter()
                .filter_map(|map| map.osu_last_played_at_ms)
                .max(),
            Self::RecentlyListened => track.last_played_at_ms,
            _ => None,
        }
    }

    pub(crate) fn sort_tracks(self, tracks: &mut [Track]) {
        tracks.sort_by_cached_key(|track| {
            self.key(
                &track.title,
                &track.artist,
                self.date(track),
                i64::from(track.audio_source_id),
            )
        });
    }

    pub(crate) fn sort_playlist(
        self,
        playlist: &mut Playlist,
        tracks: &mut [super::PlaylistTrack],
    ) {
        tracks.sort_by_cached_key(|row| {
            self.key(
                &row.track.title,
                &row.track.artist,
                self.date(&row.track),
                i64::from(row.item_id),
            )
        });
        let order: std::collections::HashMap<_, _> = tracks
            .iter()
            .enumerate()
            .map(|(index, row)| (row.item_id, index))
            .collect();
        playlist
            .items
            .sort_by_key(|item| order.get(&item.id).copied());
    }
}

#[cfg(test)]
mod tests;
