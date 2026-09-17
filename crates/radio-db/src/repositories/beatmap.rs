use anyhow::Result;
use radio_core::import_types::ImportedBeatmap;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};

use crate::{entities::beatmap, model::Beatmap};

pub struct BeatmapRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}

impl BeatmapRepository<'_> {
    pub async fn insert(
        &self,
        set_id: i32,
        imported: &ImportedBeatmap,
        metadata_hash: Option<String>,
        audio_source_id: Option<i32>,
        background_path: Option<String>,
    ) -> Result<Beatmap> {
        Ok(into_model(
            beatmap::ActiveModel {
                difficulty_name: Set(imported.difficulty_name.clone()),
                bpm: Set(imported.bpm),
                hash: Set(imported.hash.clone()),
                beatmap_set_id: Set(set_id),
                metadata_hash: Set(metadata_hash),
                audio_source_id: Set(audio_source_id),
                background_path: Set(background_path),
                ..Default::default()
            }
            .insert(&self.connection)
            .await?,
        ))
    }

    pub async fn get(&self, id: i32) -> Result<Option<Beatmap>> {
        Ok(beatmap::Entity::find_by_id(id)
            .one(&self.connection)
            .await?
            .map(into_model))
    }

    pub async fn for_set(&self, set_id: i32) -> Result<Vec<Beatmap>> {
        Ok(beatmap::Entity::find()
            .filter(beatmap::Column::BeatmapSetId.eq(set_id))
            .order_by_asc(beatmap::Column::Id)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(into_model)
            .collect())
    }
}

fn into_model(value: beatmap::Model) -> Beatmap {
    Beatmap {
        id: value.id,
        difficulty_name: value.difficulty_name,
        bpm: value.bpm,
        hash: value.hash,
        beatmap_set_id: value.beatmap_set_id,
        metadata_hash: value.metadata_hash,
        audio_source_id: value.audio_source_id,
        background_path: value.background_path,
    }
}
