use crate::model::BeatmapSet;
pub use crate::model::BeatmapSetWithAudio;
use anyhow::{Context, Result};
use radio_core::import_types::ImportedBeatmapSet;
use radio_db::repositories::BeatmapSetRepository;

pub struct BeatmapSetService<'a> {
    pub(crate) repository: BeatmapSetRepository<'a>,
}
impl BeatmapSetService<'_> {
    pub async fn get(&self, id: i32) -> Result<Option<BeatmapSet>> {
        self.repository.get(id).await
    }
    pub async fn for_installation(&self, id: i32) -> Result<Vec<BeatmapSet>> {
        self.repository.for_installation(id).await
    }
    pub async fn all_with_audio_sources(&self) -> Result<Vec<BeatmapSetWithAudio>> {
        self.repository
            .all_with_audio_sources()
            .await
            .context("Failed to load beatmap sets with their audio sources")
    }
    pub(crate) async fn add(
        &self,
        installation_id: i32,
        imported: &ImportedBeatmapSet,
    ) -> Result<BeatmapSet> {
        self.repository.insert(installation_id, imported).await
    }
    pub(crate) async fn delete_for_installation(&self, id: i32) -> Result<()> {
        self.repository.delete_for_installation(id).await
    }
}
