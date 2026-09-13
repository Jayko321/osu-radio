use crate::model::BeatmapMetadata;
use anyhow::Result;
use radio_core::import_types::BeatmapMetadata as ImportedMetadata;
use radio_db::repositories::BeatmapMetadataRepository;

pub struct BeatmapMetadataService<'a> {
    pub(crate) repository: BeatmapMetadataRepository<'a>,
}
impl BeatmapMetadataService<'_> {
    pub async fn get(&self, hash: &str) -> Result<Option<BeatmapMetadata>> {
        self.repository.get(hash).await
    }
    pub async fn get_or_insert(&self, imported: &ImportedMetadata) -> Result<BeatmapMetadata> {
        self.repository.get_or_insert(imported).await
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        self.repository.cleanup().await
    }
}
