use crate::model::BeatmapSet;
pub use crate::model::BeatmapSetWithAudio;
use anyhow::{Context, Result};
use radio_core::import_types::ImportedBeatmapSet;
use radio_db::{
    Database, Transaction,
    repositories::{BeatmapSetRepository, beatmap_set::DateField},
};
#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashSet;

pub(crate) mod tracks;
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
        let transaction = self.database.begin_read().await?;
        let sets = Self::search_in(&transaction, query).await?;
        transaction.commit().await?;
        Ok(sets)
    }

    async fn search_in(transaction: &Transaction, query: &str) -> Result<Vec<BeatmapSetWithAudio>> {
        if query.split_whitespace().next().is_none() {
            return transaction.beatmap_sets().all_with_audio_sources().await;
        }
        let ids = Self::matching_audio_ids(transaction, query).await?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let multiple_audio_sets = transaction.beatmap_sets().multiple_audio_set_ids().await?;
        transaction
            .beatmap_sets()
            .for_audio_sources(&ids, &multiple_audio_sets)
            .await
    }

    async fn matching_audio_ids(transaction: &Transaction, query: &str) -> Result<Vec<i32>> {
        Self::matching_audio_ids_at(transaction, query, chrono::Utc::now().timestamp_millis()).await
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) async fn matching_audio_ids_at(
        transaction: &Transaction,
        query: &str,
        now: i64,
    ) -> Result<Vec<i32>> {
        let search = crate::search::parse(query);
        let mut ids: Option<HashSet<i32>> = None;
        for (index, condition) in search.conditions.iter().enumerate() {
            let mut matching: HashSet<_> = transaction
                .beatmap_sets()
                .matching_text(&condition.criterion, condition.phrase, |text| {
                    !condition.phrase
                        || crate::search::phrase_matches(text, &condition.criterion.value)
                })
                .await?
                .into_iter()
                .collect();
            if search.numeric_condition == Some(index)
                && let Some(online_id) = search.online_id
            {
                matching.extend(
                    transaction
                        .beatmap_sets()
                        .matching_online_set_id(online_id)
                        .await?,
                );
            }
            let current = match &mut ids {
                Some(ids) => ids,
                None if condition.negative => ids.insert(
                    transaction
                        .beatmap_sets()
                        .audio_source_ids()
                        .await?
                        .into_iter()
                        .collect(),
                ),
                None => ids.insert(matching.clone()),
            };
            if condition.negative {
                current.retain(|id| !matching.contains(id));
            } else {
                current.retain(|id| matching.contains(id));
            }
            if current.is_empty() {
                return Ok(Vec::new());
            }
        }
        for field in [
            radio_db::repositories::beatmap_set::NumericField::Bpm,
            radio_db::repositories::beatmap_set::NumericField::Stars,
            radio_db::repositories::beatmap_set::NumericField::Length,
        ] {
            let criteria: Vec<_> = search
                .numeric
                .iter()
                .copied()
                .filter(|c| c.field == field)
                .collect();
            if criteria.is_empty() {
                continue;
            }
            let matching: HashSet<_> = transaction
                .beatmap_sets()
                .matching_numeric(field, &criteria)
                .await?
                .into_iter()
                .collect();
            if let Some(ids) = &mut ids {
                ids.retain(|id| matching.contains(id));
            } else {
                ids = Some(matching);
            }
            if ids.as_ref().is_some_and(HashSet::is_empty) {
                return Ok(Vec::new());
            }
        }
        for (statuses, negative) in &search.statuses {
            let matching: HashSet<_> = transaction
                .beatmap_sets()
                .matching_status(statuses)
                .await?
                .into_iter()
                .collect();
            let current = match &mut ids {
                Some(ids) => ids,
                None if *negative => ids.insert(
                    transaction
                        .beatmap_sets()
                        .audio_source_ids()
                        .await?
                        .into_iter()
                        .collect(),
                ),
                None => ids.insert(matching.clone()),
            };
            current.retain(|id| matching.contains(id) != *negative);
        }
        for field in [DateField::Ranked, DateField::Submitted] {
            let criteria: Vec<_> = search
                .dates
                .iter()
                .copied()
                .filter(|c| c.field == field)
                .collect();
            if !criteria.is_empty() {
                let matching: HashSet<_> = transaction
                    .beatmap_sets()
                    .matching_dates(field, &criteria)
                    .await?
                    .into_iter()
                    .collect();
                if let Some(ids) = &mut ids {
                    ids.retain(|id| matching.contains(id));
                } else {
                    ids = Some(matching);
                }
            }
        }
        if !search.played.is_empty() || !search.ages.is_empty() {
            let matching: HashSet<_> = transaction
                .beatmap_sets()
                .matching_played(&search.played, &search.ages, now)
                .await?
                .into_iter()
                .collect();
            if let Some(ids) = &mut ids {
                ids.retain(|id| matching.contains(id));
            } else {
                ids = Some(matching);
            }
        }
        let mut ids: Vec<_> = match ids {
            Some(ids) => ids.into_iter().collect(),
            None => transaction.beatmap_sets().audio_source_ids().await?,
        };
        ids.sort_unstable();
        Ok(ids)
    }

    pub async fn search_tracks(&self, query: &str) -> Result<Vec<LibraryTrack>> {
        let transaction = self.database.begin_read().await?;
        let ids = Self::matching_audio_ids(&transaction, query).await?;
        let multiple_audio_sets = transaction.beatmap_sets().multiple_audio_set_ids().await?;
        let mut tracks = Vec::new();
        for chunk in ids.chunks(500) {
            let sets = transaction
                .beatmap_sets()
                .for_audio_sources(chunk, &multiple_audio_sets)
                .await?;
            tracks.extend(tracks::from_sets(sets));
        }
        tracks.sort_by_key(|track| {
            (
                track
                    .difficulties
                    .iter()
                    .map(|map| map.beatmap_set_id)
                    .min(),
                track.audio_source_id,
            )
        });
        crate::ListeningHistoryService::fill_tracks(&transaction, &mut tracks).await?;
        crate::AudioSettingsService::fill_tracks(&transaction, &mut tracks).await?;
        crate::AudioModsService::fill_tracks(&transaction, &mut tracks).await?;
        transaction.commit().await?;
        Ok(tracks)
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
#[cfg(test)]
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

#[cfg(test)]
fn match_text(matches: &mut [bool], words: &[String], text: &str) {
    for (matched, word) in matches.iter_mut().zip(words) {
        *matched |= text.contains(word);
    }
}

#[cfg(test)]
mod benchmarks;
