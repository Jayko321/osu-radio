use anyhow::{Result, anyhow};
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
        into_model(
            beatmap::ActiveModel {
                osu_last_played_at_ms: Set(imported.osu_last_played_at_ms),
                stars: Set(imported.stars),
                status: Set(imported.status.map(|status| status.as_str().to_owned())),
                difficulty_name: Set(imported.difficulty_name.clone()),
                search_difficulty_name: Set(imported
                    .difficulty_name
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()),
                bpm: Set(imported.bpm),
                hash: Set(imported.hash.clone()),
                md5_hash: Set(imported.md5_hash.clone()),
                beatmap_set_id: Set(set_id),
                metadata_hash: Set(metadata_hash),
                audio_source_id: Set(audio_source_id),
                background_path: Set(background_path),
                ..Default::default()
            }
            .insert(&self.connection)
            .await?,
        )
    }

    pub async fn get(&self, id: i32) -> Result<Option<Beatmap>> {
        beatmap::Entity::find_by_id(id)
            .one(&self.connection)
            .await?
            .map(into_model)
            .transpose()
    }

    pub async fn for_set(&self, set_id: i32) -> Result<Vec<Beatmap>> {
        beatmap::Entity::find()
            .filter(beatmap::Column::BeatmapSetId.eq(set_id))
            .order_by_asc(beatmap::Column::Id)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(into_model)
            .collect()
    }
}

fn into_model(value: beatmap::Model) -> Result<Beatmap> {
    Ok(Beatmap {
        id: value.id,
        osu_last_played_at_ms: value.osu_last_played_at_ms,
        stars: value.stars,
        status: value
            .status
            .map(|status| {
                status
                    .parse()
                    .map_err(|_| anyhow!("unknown beatmap status `{status}`"))
            })
            .transpose()?,
        difficulty_name: value.difficulty_name,
        bpm: value.bpm,
        hash: value.hash,
        md5_hash: value.md5_hash,
        beatmap_set_id: value.beatmap_set_id,
        metadata_hash: value.metadata_hash,
        audio_source_id: value.audio_source_id,
        background_path: value.background_path,
    })
}
