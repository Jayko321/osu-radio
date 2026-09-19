use super::BeatmapSetWithAudio;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub struct LibraryTrack {
    pub audio_source_id: i32,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub cover_beatmap_id: Option<i32>,
    pub difficulties: Vec<TrackDifficulty>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TrackDifficulty {
    pub beatmap_id: i32,
    pub beatmap_set_id: i32,
    pub difficulty_name: Option<String>,
    pub set_has_multiple_audio_sources: bool,
}

pub(super) fn from_sets(sets: Vec<BeatmapSetWithAudio>) -> Vec<LibraryTrack> {
    let mut order = Vec::new();
    let mut groups = HashMap::<_, Vec<_>>::new();
    for set in sets {
        for audio in set.audio_sources {
            if let std::collections::hash_map::Entry::Vacant(entry) = groups.entry(audio.id) {
                order.push(audio.id);
                entry.insert(Vec::new());
            }
        }
        for map in set.beatmaps {
            if let Some(maps) = map.audio_source_id.and_then(|id| groups.get_mut(&id)) {
                maps.push((map, set.beatmap_set.id, set.has_multiple_audio_sources));
            }
        }
    }
    order
        .into_iter()
        .map(|id| {
            let mut maps = groups.remove(&id).unwrap_or_default();
            maps.sort_by_key(|(map, _, _)| map.id);
            let representative = maps.first().map(|(map, _, _)| map);
            LibraryTrack {
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
                    .map(|(map, set_id, split)| TrackDifficulty {
                        beatmap_id: map.id,
                        beatmap_set_id: set_id,
                        difficulty_name: map.difficulty_name,
                        set_has_multiple_audio_sources: split,
                    })
                    .collect(),
            }
        })
        .collect()
}
