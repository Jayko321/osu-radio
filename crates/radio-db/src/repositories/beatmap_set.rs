use std::collections::BTreeMap;

use anyhow::{Context, Result};
use radio_core::import_types::ImportedBeatmapSet;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, Set,
    sea_query::{Alias, Expr, ExprTrait, Order, Query},
};

use crate::{
    entities::{audio_source, beatmap, beatmap_set},
    model::{AudioSource, BeatmapSet, SourceType},
};

pub struct BeatmapSetRepository<'a> {
    pub(crate) connection: &'a sea_orm::DatabaseConnection,
}

impl BeatmapSetRepository<'_> {
    pub async fn get(&self, id: i32) -> Result<Option<BeatmapSet>> {
        Ok(beatmap_set::Entity::find_by_id(id)
            .one(self.connection)
            .await?
            .map(into_model))
    }

    pub async fn for_installation(&self, installation_id: i32) -> Result<Vec<BeatmapSet>> {
        Ok(beatmap_set::Entity::find()
            .filter(beatmap_set::Column::InstallationId.eq(installation_id))
            .order_by_asc(beatmap_set::Column::Id)
            .all(self.connection)
            .await?
            .into_iter()
            .map(into_model)
            .collect())
    }

    pub async fn all_with_audio_sources(&self) -> Result<Vec<(BeatmapSet, Vec<AudioSource>)>> {
        use audio_source::Column as Audio;
        use beatmap::Column as Map;
        use beatmap_set::Column as SetColumn;

        // One query gives the entire aggregate a consistent database snapshot.
        let query = Query::select()
            .distinct()
            .columns([
                (beatmap_set::Entity, SetColumn::Id),
                (beatmap_set::Entity, SetColumn::OnlineId),
                (beatmap_set::Entity, SetColumn::Hash),
                (beatmap_set::Entity, SetColumn::InstallationId),
            ])
            .expr_as(
                Expr::col((audio_source::Entity, Audio::Id)),
                Alias::new("audio_id"),
            )
            .expr_as(
                Expr::col((audio_source::Entity, Audio::Kind)),
                Alias::new("audio_kind"),
            )
            .expr_as(
                Expr::col((audio_source::Entity, Audio::Location)),
                Alias::new("audio_location"),
            )
            .from(beatmap_set::Entity)
            .left_join(
                beatmap::Entity,
                Expr::col((beatmap_set::Entity, SetColumn::Id))
                    .equals((beatmap::Entity, Map::BeatmapSetId)),
            )
            .left_join(
                audio_source::Entity,
                Expr::col((beatmap::Entity, Map::AudioSourceId))
                    .equals((audio_source::Entity, Audio::Id)),
            )
            .order_by((beatmap_set::Entity, SetColumn::Id), Order::Asc)
            .order_by((audio_source::Entity, Audio::Id), Order::Asc)
            .to_owned();
        let rows =
            SetWithAudio::find_by_statement(self.connection.get_database_backend().build(&query))
                .all(self.connection)
                .await?;
        let mut sets = BTreeMap::<i32, (BeatmapSet, Vec<AudioSource>)>::new();
        for row in rows {
            let (_, audio_sources) = sets.entry(row.id).or_insert_with(|| {
                (
                    BeatmapSet {
                        id: row.id,
                        online_id: row.online_id,
                        hash: row.hash,
                        installation_id: row.installation_id,
                    },
                    Vec::new(),
                )
            });
            if let Some(id) = row.audio_id {
                audio_sources.push(AudioSource {
                    id,
                    s_type: SourceType::from_parts(
                        &row.audio_kind.context("joined audio source has no kind")?,
                        row.audio_location
                            .context("joined audio source has no location")?,
                    )?,
                });
            }
        }
        Ok(sets.into_values().collect())
    }
}

#[derive(FromQueryResult)]
struct SetWithAudio {
    id: i32,
    online_id: Option<i32>,
    hash: Option<String>,
    installation_id: i32,
    audio_id: Option<i32>,
    audio_kind: Option<String>,
    audio_location: Option<String>,
}

pub(crate) async fn insert(
    connection: &impl ConnectionTrait,
    installation_id: i32,
    imported: &ImportedBeatmapSet,
) -> Result<BeatmapSet> {
    Ok(into_model(
        beatmap_set::ActiveModel {
            online_id: Set(imported.online_id),
            hash: Set(imported.hash.clone()),
            installation_id: Set(installation_id),
            ..Default::default()
        }
        .insert(connection)
        .await?,
    ))
}

pub(crate) async fn delete_for_installation(
    connection: &impl ConnectionTrait,
    installation_id: i32,
) -> Result<()> {
    beatmap_set::Entity::delete_many()
        .filter(beatmap_set::Column::InstallationId.eq(installation_id))
        .exec(connection)
        .await?;
    Ok(())
}

fn into_model(value: beatmap_set::Model) -> BeatmapSet {
    BeatmapSet {
        id: value.id,
        online_id: value.online_id,
        hash: value.hash,
        installation_id: value.installation_id,
    }
}
