use crate::model::BeatmapSet;
pub use crate::model::BeatmapSetWithAudio;
use anyhow::{Context, Result};
use radio_core::import_types::ImportedBeatmapSet;
use radio_db::{Database, repositories::BeatmapSetRepository};
use std::collections::{HashMap, HashSet};

mod tracks;
pub use tracks::{LibraryTrack, TrackDifficulty};

pub struct BeatmapSetService<'a> {
    pub(crate) database: &'a Database,
}
impl BeatmapSetService<'_> {
    pub async fn get(&self, id: i32) -> Result<Option<BeatmapSet>> {
        self.database.beatmap_sets().get(id).await
    }
    pub async fn for_installation(&self, id: i32) -> Result<Vec<BeatmapSet>> {
        self.database.beatmap_sets().for_installation(id).await
    }
    pub async fn all_with_audio_sources(&self) -> Result<Vec<BeatmapSetWithAudio>> {
        self.database
            .beatmap_sets()
            .all_with_audio_sources()
            .await
            .context("Failed to load beatmap sets with their audio sources")
    }
    pub async fn search_with_audio_sources(&self, query: &str) -> Result<Vec<BeatmapSetWithAudio>> {
        let mut seen = HashSet::new();
        let words: Vec<_> = query
            .split_whitespace()
            .map(str::to_lowercase)
            .filter(|word| seen.insert(word.clone()))
            .collect();
        if words.is_empty() {
            return self.all_with_audio_sources().await;
        }
        let transaction = self.database.begin_read().await?;
        let metadata = transaction.beatmap_metadata().search_metadata().await?;
        let difficulties = transaction.beatmap_sets().search_difficulties().await?;
        let mut tags = Vec::with_capacity(words.len());
        for word in &words {
            tags.push(transaction.tags().matching_set_ids(word).await?);
        }
        let (ids, multiple_audio_sets) = matching_audio(metadata, difficulties, &tags, &words);
        let sets = transaction
            .beatmap_sets()
            .for_audio_sources(&ids, &multiple_audio_sets)
            .await?;
        transaction.commit().await?;
        Ok(sets)
    }

    pub async fn search_tracks(&self, query: &str) -> Result<Vec<LibraryTrack>> {
        Ok(tracks::from_sets(
            self.search_with_audio_sources(query).await?,
        ))
    }

    pub(crate) async fn add(
        repository: BeatmapSetRepository<'_>,
        installation_id: i32,
        imported: &ImportedBeatmapSet,
    ) -> Result<BeatmapSet> {
        repository.insert(installation_id, imported).await
    }
    pub(crate) async fn delete_for_installation(
        repository: BeatmapSetRepository<'_>,
        id: i32,
    ) -> Result<()> {
        repository.delete_for_installation(id).await
    }
}

/// Match shared metadata once, then union word matches across every reference of global audio.
fn matching_audio(
    metadata: Vec<crate::model::SearchMetadata>,
    difficulties: Vec<crate::model::SearchDifficulty>,
    tags: &[Vec<i32>],
    words: &[String],
) -> (Vec<i32>, HashSet<i32>) {
    let metadata_matches: HashMap<_, _> = metadata
        .into_iter()
        .map(|meta| {
            let mut matches = vec![false; words.len()];
            for text in [
                meta.title,
                meta.title_unicode,
                meta.artist,
                meta.artist_unicode,
            ]
            .into_iter()
            .flatten()
            {
                match_text(&mut matches, words, &text.to_lowercase());
            }
            (meta.hash, matches)
        })
        .collect();
    let mut set_matches: HashMap<i32, Vec<bool>> = HashMap::new();
    for (index, ids) in tags.iter().enumerate() {
        for id in ids {
            let matches = set_matches
                .entry(*id)
                .or_insert_with(|| vec![false; words.len()]);
            if let Some(matched) = matches.get_mut(index) {
                *matched = true;
            }
        }
    }
    let mut first_audio = HashMap::new();
    let mut multiple_audio_sets = HashSet::new();
    let mut audio_matches: HashMap<i32, Vec<bool>> = HashMap::new();
    for map in difficulties {
        if *first_audio.entry(map.set_id).or_insert(map.audio_source_id) != map.audio_source_id {
            multiple_audio_sets.insert(map.set_id);
        }
        let matches = audio_matches
            .entry(map.audio_source_id)
            .or_insert_with(|| vec![false; words.len()]);
        for source in [
            set_matches.get(&map.set_id),
            map.metadata_hash
                .as_ref()
                .and_then(|hash| metadata_matches.get(hash)),
        ]
        .into_iter()
        .flatten()
        {
            for (matched, source) in matches.iter_mut().zip(source) {
                *matched |= source;
            }
        }
        if let Some(name) = map.difficulty_name {
            match_text(matches, words, &name.to_lowercase());
        }
    }
    let mut ids: Vec<_> = audio_matches
        .into_iter()
        .filter(|(_, matches)| matches.iter().all(|matched| *matched))
        .map(|(id, _)| id)
        .collect();
    ids.sort_unstable();
    (ids, multiple_audio_sets)
}

fn match_text(matches: &mut [bool], words: &[String], text: &str) {
    for (matched, word) in matches.iter_mut().zip(words) {
        *matched |= text.contains(word);
    }
}

#[cfg(test)]
mod benchmarks;
