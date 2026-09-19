use std::{collections::HashMap, time::Duration};

use crate::{BeatmapSet, models::BeatmapDetails};

/// A library row is one stored audio source, even when several sets reference it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub audio_source_id: i32,
    pub cover_beatmap_id: Option<i32>,
    pub title: String,
    pub artist: String,
    pub subtitle: String,
    pub duration: Option<Duration>,
    pub difficulties: Vec<crate::models::TrackDifficulty>,
}

impl Track {
    pub fn new(title: impl Into<String>, artist: impl Into<String>, duration: Duration) -> Self {
        let artist = artist.into();
        Self {
            audio_source_id: 0,
            cover_beatmap_id: None,
            title: title.into(),
            subtitle: artist.clone(),
            artist,
            duration: Some(duration),
            difficulties: Vec::new(),
        }
    }

    #[must_use]
    pub fn duration_label(&self) -> String {
        self.duration.map_or_else(
            || "--:--".to_owned(),
            |duration| {
                let seconds = duration.as_secs();
                format!("{:02}:{:02}", seconds / 60, seconds % 60)
            },
        )
    }

    #[must_use]
    pub fn meta(&self) -> String {
        format!("{} // {}", self.subtitle, self.duration_label())
    }
}

/// Preserves server set/audio order; metadata and cover choice use beatmap ID order.
#[must_use]
pub fn library_tracks(sets: &[BeatmapSet]) -> Vec<Track> {
    let mut order = Vec::new();
    let mut groups: HashMap<i32, Vec<(&BeatmapDetails, i32, bool)>> = HashMap::new();
    for set in sets {
        for audio in &set.audio_sources {
            if !groups.contains_key(&audio.id) {
                order.push(audio.id);
            }
            groups.entry(audio.id).or_default();
        }
        let split = set.has_multiple_audio_sources || set.audio_sources.len() > 1;
        for map in &set.beatmaps {
            if let Some(id) = map.audio_source_id
                && let Some(maps) = groups.get_mut(&id)
            {
                maps.push((map, set.id, split));
            }
        }
    }
    order
        .into_iter()
        .map(|id| {
            let mut maps = groups.remove(&id).unwrap_or_default();
            maps.sort_by_key(|(map, _, _)| map.id);
            let representative = maps.first().map(|(map, _, _)| *map);
            Track::from(crate::models::LibraryTrack {
                audio_source_id: id,
                title: representative.and_then(|map| map.title.clone()),
                title_unicode: representative.and_then(|map| map.title_unicode.clone()),
                artist: representative.and_then(|map| map.artist.clone()),
                artist_unicode: representative.and_then(|map| map.artist_unicode.clone()),
                cover_beatmap_id: maps
                    .iter()
                    .find(|(map, _, _)| map.has_cover)
                    .map(|(map, _, _)| map.id),
                difficulties: maps
                    .into_iter()
                    .map(|(map, set_id, split)| crate::models::TrackDifficulty {
                        beatmap_id: map.id,
                        beatmap_set_id: set_id,
                        difficulty_name: map.difficulty_name.clone(),
                        set_has_multiple_audio_sources: split,
                    })
                    .collect(),
            })
        })
        .collect()
}

fn text(ordinary: Option<&str>, unicode: Option<&str>, unknown: &str) -> String {
    ordinary
        .filter(|s| !s.trim().is_empty())
        .or_else(|| unicode.filter(|s| !s.trim().is_empty()))
        .unwrap_or(unknown)
        .to_owned()
}

#[must_use]
pub fn selection_after_refresh(tracks: &[Track], selected: Option<i32>) -> Option<i32> {
    selected
        .filter(|id| tracks.iter().any(|track| track.audio_source_id == *id))
        .or_else(|| tracks.first().map(|track| track.audio_source_id))
}

impl From<crate::models::LibraryTrack> for Track {
    fn from(track: crate::models::LibraryTrack) -> Self {
        let title = text(
            track.title.as_deref(),
            track.title_unicode.as_deref(),
            "Unknown title",
        );
        let artist = text(
            track.artist.as_deref(),
            track.artist_unicode.as_deref(),
            "Unknown artist",
        );
        let mut names = Vec::new();
        for map in &track.difficulties {
            if map.set_has_multiple_audio_sources {
                let name = text(map.difficulty_name.as_deref(), None, "Unknown difficulty");
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        let subtitle = if names.is_empty() {
            artist.clone()
        } else {
            format!("{artist} | {}", names.join(", "))
        };
        Self {
            audio_source_id: track.audio_source_id,
            cover_beatmap_id: track.cover_beatmap_id,
            title,
            artist,
            subtitle,
            duration: None,
            difficulties: track.difficulties,
        }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::AudioSource;

    fn set(ids: &[i32], maps: Vec<BeatmapDetails>) -> BeatmapSet {
        BeatmapSet {
            has_multiple_audio_sources: ids.len() > 1,
            id: 1,
            online_id: None,
            hash: None,
            beatmaps: maps,
            audio_sources: ids
                .iter()
                .map(|id| AudioSource {
                    id: *id,
                    kind: "local".into(),
                    location: String::new(),
                })
                .collect(),
        }
    }
    fn map(id: i32, audio: i32, name: &str) -> BeatmapDetails {
        BeatmapDetails {
            id,
            audio_source_id: Some(audio),
            difficulty_name: Some(name.into()),
            title: Some("Title".into()),
            artist: Some("Artist".into()),
            ..BeatmapDetails::default()
        }
    }
    #[test]
    fn groups_globally_and_only_annotates_split_audio() {
        let mut cover = map(2, 10, "Hard");
        cover.has_cover = true;
        let tracks = library_tracks(&[
            set(
                &[10, 20],
                vec![
                    cover,
                    map(1, 10, "Easy"),
                    map(3, 20, "Insane"),
                    map(6, 10, "Hard"),
                ],
            ),
            set(&[10], vec![map(4, 10, "Another")]),
            set(&[30], vec![map(5, 30, "Normal")]),
        ]);
        assert_eq!(
            tracks.iter().map(|t| t.audio_source_id).collect::<Vec<_>>(),
            [10, 20, 30]
        );
        assert_eq!(tracks[0].subtitle, "Artist | Easy, Hard");
        assert_eq!(
            tracks[0]
                .difficulties
                .iter()
                .map(|map| map.beatmap_id)
                .collect::<Vec<_>>(),
            [1, 2, 4, 6]
        );
        assert_eq!(
            tracks[0]
                .difficulties
                .iter()
                .filter(|map| map.difficulty_name.as_deref() == Some("Hard"))
                .count(),
            2
        );
        assert_eq!(tracks[0].cover_beatmap_id, Some(2));
        assert_eq!(tracks[2].subtitle, "Artist");
        assert_eq!(selection_after_refresh(&tracks, Some(20)), Some(20));
        assert_eq!(selection_after_refresh(&tracks, Some(99)), Some(10));
        assert_eq!(selection_after_refresh(&[], Some(10)), None);
    }
    #[test]
    fn filtered_audio_keeps_its_title_cover_and_difficulty_annotation() {
        let mut cover = map(2, 10, "Hard");
        cover.has_cover = true;
        cover.title = Some("Alternate".into());
        let mut original = set(
            &[10, 20],
            vec![map(1, 10, "Easy"), cover, map(3, 20, "Insane")],
        );
        let before = library_tracks(&[original.clone()]);
        original.audio_sources.retain(|audio| audio.id == 10);
        original
            .beatmaps
            .retain(|map| map.audio_source_id == Some(10));
        assert_eq!(library_tracks(&[original]), vec![before[0].clone()]);
    }

    #[test]
    fn missing_metadata_and_duration_are_explicit() {
        let map = BeatmapDetails {
            id: 1,
            audio_source_id: Some(7),
            title: Some(" ".into()),
            title_unicode: Some("曲".into()),
            ..BeatmapDetails::default()
        };
        let tracks = library_tracks(&[set(&[7, 8], vec![map])]);
        assert_eq!(tracks[0].title, "曲");
        assert_eq!(tracks[0].artist, "Unknown artist");
        assert_eq!(tracks[1].title, "Unknown title");
        assert_eq!(tracks[0].duration_label(), "--:--");
        let track = Track::new("Title", "Artist", Duration::from_secs(3607));
        assert_eq!(track.duration_label(), "60:07");
    }
}
