use super::BeatmapSetWithAudio;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryTrack {
    pub audio_source_id: i32,
    pub last_played_at_ms: Option<i64>,
    pub volume_percent: Option<u8>,
    pub mods: Option<crate::AudioModsProfile>,
    pub title: Option<String>,
    pub title_unicode: Option<String>,
    pub artist: Option<String>,
    pub artist_unicode: Option<String>,
    pub cover_beatmap_id: Option<i32>,
    pub difficulties: Vec<TrackDifficulty>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackDifficulty {
    pub beatmap_id: i32,
    pub beatmap_set_id: i32,
    pub beatmap_set_online_id: Option<i32>,
    pub difficulty_name: Option<String>,
    pub set_has_multiple_audio_sources: bool,
}

pub(crate) fn from_sets(sets: Vec<BeatmapSetWithAudio>) -> Vec<LibraryTrack> {
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
                maps.push((
                    map,
                    set.beatmap_set.id,
                    set.beatmap_set.online_id,
                    set.has_multiple_audio_sources,
                ));
            }
        }
    }
    order
        .into_iter()
        .map(|id| {
            let mut maps = groups.remove(&id).unwrap_or_default();
            maps.sort_by_key(|(map, _, _, _)| map.id);
            let representative = maps.first().map(|(map, _, _, _)| map);
            LibraryTrack {
                audio_source_id: id,
                last_played_at_ms: None,
                volume_percent: None,
                mods: None,
                title: representative.and_then(|map| map.title.clone()),
                title_unicode: representative.and_then(|map| map.title_unicode.clone()),
                artist: representative.and_then(|map| map.artist.clone()),
                artist_unicode: representative.and_then(|map| map.artist_unicode.clone()),
                cover_beatmap_id: maps
                    .iter()
                    .find(|(map, _, _, _)| map.has_cover)
                    .map(|(map, _, _, _)| map.id),
                difficulties: maps
                    .into_iter()
                    .map(|(map, set_id, online_id, split)| TrackDifficulty {
                        beatmap_id: map.id,
                        beatmap_set_id: set_id,
                        beatmap_set_online_id: online_id,
                        difficulty_name: map.difficulty_name,
                        set_has_multiple_audio_sources: split,
                    })
                    .collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AudioSource, BeatmapDetails, BeatmapSet, SourceType};

    fn detail(id: i32, audio: i32, title: &str, cover: bool) -> BeatmapDetails {
        BeatmapDetails {
            id,
            audio_source_id: Some(audio),
            difficulty_name: Some(format!("Difficulty {id}")),
            title: Some(title.into()),
            title_unicode: Some(format!("曲 {title}")),
            artist: Some(format!("Artist {title}")),
            artist_unicode: Some(format!("演奏者 {title}")),
            has_cover: cover,
        }
    }

    #[test]
    fn merges_global_audio_with_first_seen_order_lowest_id_labels_and_independent_cover() {
        assert!(from_sets(vec![]).is_empty());
        let audio = |id| AudioSource {
            id,
            s_type: SourceType::Local(format!("/audio/{id}")),
        };
        let sets = vec![
            BeatmapSetWithAudio {
                beatmap_set: BeatmapSet {
                    id: 100,
                    installation_id: 7,
                    online_id: Some(1000),
                    hash: None,
                },
                audio_sources: vec![audio(20), audio(10)],
                has_multiple_audio_sources: true,
                beatmaps: vec![
                    detail(31, 10, "Other", false),
                    detail(23, 20, "Later", true),
                    detail(21, 20, "Middle", false),
                ],
            },
            BeatmapSetWithAudio {
                beatmap_set: BeatmapSet {
                    id: 200,
                    installation_id: 8,
                    online_id: None,
                    hash: None,
                },
                audio_sources: vec![audio(20)],
                has_multiple_audio_sources: false,
                beatmaps: vec![detail(1, 20, "Earliest", false)],
            },
        ];
        assert_eq!(
            from_sets(sets),
            [
                LibraryTrack {
                    audio_source_id: 20,
                    last_played_at_ms: None,
                    volume_percent: None,
                    mods: None,
                    title: Some("Earliest".into()),
                    title_unicode: Some("曲 Earliest".into()),
                    artist: Some("Artist Earliest".into()),
                    artist_unicode: Some("演奏者 Earliest".into()),
                    cover_beatmap_id: Some(23),
                    difficulties: vec![
                        TrackDifficulty {
                            beatmap_id: 1,
                            beatmap_set_id: 200,
                            beatmap_set_online_id: None,
                            difficulty_name: Some("Difficulty 1".into()),
                            set_has_multiple_audio_sources: false
                        },
                        TrackDifficulty {
                            beatmap_id: 21,
                            beatmap_set_id: 100,
                            beatmap_set_online_id: Some(1000),
                            difficulty_name: Some("Difficulty 21".into()),
                            set_has_multiple_audio_sources: true
                        },
                        TrackDifficulty {
                            beatmap_id: 23,
                            beatmap_set_id: 100,
                            beatmap_set_online_id: Some(1000),
                            difficulty_name: Some("Difficulty 23".into()),
                            set_has_multiple_audio_sources: true
                        },
                    ],
                },
                LibraryTrack {
                    audio_source_id: 10,
                    last_played_at_ms: None,
                    volume_percent: None,
                    mods: None,
                    title: Some("Other".into()),
                    title_unicode: Some("曲 Other".into()),
                    artist: Some("Artist Other".into()),
                    artist_unicode: Some("演奏者 Other".into()),
                    cover_beatmap_id: None,
                    difficulties: vec![TrackDifficulty {
                        beatmap_id: 31,
                        beatmap_set_id: 100,
                        beatmap_set_online_id: Some(1000),
                        difficulty_name: Some("Difficulty 31".into()),
                        set_has_multiple_audio_sources: true
                    }],
                },
            ]
        );
    }
}
