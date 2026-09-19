use crate::model::Tag;
use anyhow::Result;
use radio_core::import_types::ImportedBeatmapSet;
use radio_db::repositories::TagRepository;
use std::collections::BTreeSet;

pub struct TagService<'a> {
    pub(crate) repository: TagRepository<'a>,
}

impl TagService<'_> {
    pub async fn all(&self) -> Result<Vec<Tag>> {
        self.repository.all().await
    }
    pub async fn get(&self, id: i32) -> Result<Option<Tag>> {
        self.repository.get(id).await
    }
    pub async fn for_set(&self, id: i32) -> Result<Vec<Tag>> {
        self.repository.for_set(id).await
    }

    pub(crate) async fn import(&self, set_id: i32, imported: &ImportedBeatmapSet) -> Result<()> {
        let names: BTreeSet<_> = imported
            .beatmaps
            .iter()
            .filter_map(|map| map.metadata.as_ref())
            .flat_map(|metadata| {
                metadata
                    .tags
                    .as_deref()
                    .unwrap_or_default()
                    .split_whitespace()
                    .chain(metadata.user_tags.iter().map(String::as_str))
            })
            .map(|name| name.trim().to_lowercase())
            .filter(|name| !name.is_empty())
            .collect();
        for name in names {
            if let Some(tag) = self.repository.get_or_insert(&name).await? {
                self.repository.link(set_id, tag.id).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        self.repository.cleanup().await
    }
}
