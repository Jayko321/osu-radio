use crate::model::Beatmap;
use anyhow::Result;
use radio_core::import_types::ImportedBeatmap;
use radio_db::repositories::BeatmapRepository;

pub struct BeatmapService<'a> {
    pub(crate) repository: BeatmapRepository<'a>,
}
impl BeatmapService<'_> {
    pub async fn get(&self, id: i32) -> Result<Option<Beatmap>> {
        self.repository.get(id).await
    }
    pub async fn for_set(&self, id: i32) -> Result<Vec<Beatmap>> {
        self.repository.for_set(id).await
    }
    pub(crate) async fn add(
        &self,
        set_id: i32,
        imported: &ImportedBeatmap,
        metadata_hash: Option<String>,
        audio_source_id: Option<i32>,
        background_path: Option<String>,
    ) -> Result<Beatmap> {
        self.repository
            .insert(
                set_id,
                imported,
                metadata_hash,
                audio_source_id,
                background_path,
            )
            .await
    }
}
