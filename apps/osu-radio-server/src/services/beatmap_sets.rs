use anyhow::{Context, Result};
use radio_db::model::{AudioSource, BeatmapSet};

use crate::state::AppState;

#[derive(Debug)]
pub(crate) struct BeatmapSetWithAudio {
    pub(crate) beatmap_set: BeatmapSet,
    pub(crate) audio_sources: Vec<AudioSource>,
}

pub(crate) struct BeatmapSetService<'a> {
    state: &'a AppState,
}

impl<'a> BeatmapSetService<'a> {
    pub(crate) const fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub(crate) async fn all_with_audio_sources(&self) -> Result<Vec<BeatmapSetWithAudio>> {
        let stored = self
            .state
            .database()
            .await
            .beatmap_sets_with_audio_sources()
            .await
            .context("Failed to load beatmap sets with their audio sources")?;

        Ok(stored
            .into_iter()
            .map(|(beatmap_set, audio_sources)| BeatmapSetWithAudio {
                beatmap_set,
                audio_sources,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use radio_db::model::SourceType;

    use crate::test_support::{beatmap_set, state_with};

    use super::*;

    fn locations(listed: &BeatmapSetWithAudio) -> Vec<&SourceType> {
        listed
            .audio_sources
            .iter()
            .map(|audio_source| &audio_source.s_type)
            .collect()
    }

    #[tokio::test]
    async fn returns_each_beatmap_set_with_the_audio_sources_it_references_once() {
        let state = state_with(&[
            beatmap_set(1, "/osu/files/a/ab/abc"),
            beatmap_set(2, "/osu/files/d/de/def"),
        ])
        .await;

        let listed = BeatmapSetService::new(&state)
            .all_with_audio_sources()
            .await
            .expect("beatmap sets should load");

        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].beatmap_set.online_id, Some(1));
        assert_eq!(
            locations(&listed[0]),
            vec![&SourceType::Local("/osu/files/a/ab/abc".to_owned())]
        );
        assert_eq!(
            locations(&listed[1]),
            vec![&SourceType::Local("/osu/files/d/de/def".to_owned())]
        );
    }

    #[tokio::test]
    async fn returns_nothing_for_an_empty_database() {
        let state = state_with(&[]).await;

        let listed = BeatmapSetService::new(&state)
            .all_with_audio_sources()
            .await
            .expect("beatmap sets should load");

        assert!(listed.is_empty());
    }
}
