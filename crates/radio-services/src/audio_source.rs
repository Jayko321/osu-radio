use crate::model::AudioSource;
use crate::model::SourceType;
use anyhow::Result;
use radio_db::repositories::AudioSourceRepository;

pub struct AudioSourceService<'a> {
    pub(crate) repository: AudioSourceRepository<'a>,
}
impl AudioSourceService<'_> {
    pub async fn get(&self, id: i32) -> Result<Option<AudioSource>> {
        self.repository.get(id).await
    }
    pub async fn find(&self, source: &SourceType) -> Result<Option<AudioSource>> {
        self.repository.find(source).await
    }
    pub async fn get_or_insert(&self, source: &SourceType) -> Result<AudioSource> {
        self.repository.get_or_insert(source).await
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        self.repository.cleanup().await
    }
}
