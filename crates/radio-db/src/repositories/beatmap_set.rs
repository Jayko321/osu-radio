use std::collections::BTreeMap;

use anyhow::{Context, Result};
use radio_core::import_types::ImportedBeatmapSet;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, Set,
    sea_query::{Alias, Expr, ExprTrait, Order, Query},
};

use crate::{
    entities::{audio_source, beatmap, beatmap_metadata, beatmap_set},
    model::{AudioSource, BeatmapDetails, BeatmapSet, BeatmapSetWithAudio, SourceType},
};

pub struct BeatmapSetRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}

impl BeatmapSetRepository<'_> {
    pub async fn insert(
        &self,
        installation_id: i32,
        imported: &ImportedBeatmapSet,
    ) -> Result<BeatmapSet> {
        insert(&self.connection, installation_id, imported).await
    }
    pub async fn delete_for_installation(&self, installation_id: i32) -> Result<()> {
        delete_for_installation(&self.connection, installation_id).await
    }

    pub async fn get(&self, id: i32) -> Result<Option<BeatmapSet>> {
        Ok(beatmap_set::Entity::find_by_id(id)
            .one(&self.connection)
            .await?
            .map(into_model))
    }

    pub async fn for_installation(&self, installation_id: i32) -> Result<Vec<BeatmapSet>> {
        Ok(beatmap_set::Entity::find()
            .filter(beatmap_set::Column::InstallationId.eq(installation_id))
            .order_by_asc(beatmap_set::Column::Id)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(into_model)
            .collect())
    }

    #[allow(clippy::too_many_lines)] // Keep the snapshot-consistent aggregate query together.
    pub async fn all_with_audio_sources(&self) -> Result<Vec<BeatmapSetWithAudio>> {
        use audio_source::Column as Audio;
        use beatmap::Column as Map;
        use beatmap_metadata::Column as Meta;
        use beatmap_set::Column as SetColumn;

        // One query gives the entire aggregate a consistent database snapshot.
        let query = Query::select()
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
            .expr_as(Expr::col((beatmap::Entity, Map::Id)), Alias::new("map_id"))
            .expr_as(
                Expr::col((beatmap::Entity, Map::DifficultyName)),
                Alias::new("difficulty_name"),
            )
            .expr_as(
                Expr::col((beatmap::Entity, Map::BackgroundPath)),
                Alias::new("background_path"),
            )
            .columns([
                (beatmap_metadata::Entity, Meta::Title),
                (beatmap_metadata::Entity, Meta::TitleUnicode),
                (beatmap_metadata::Entity, Meta::Artist),
                (beatmap_metadata::Entity, Meta::ArtistUnicode),
            ])
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
            .left_join(
                beatmap_metadata::Entity,
                Expr::col((beatmap::Entity, Map::MetadataHash))
                    .equals((beatmap_metadata::Entity, Meta::Hash)),
            )
            .order_by((beatmap_set::Entity, SetColumn::Id), Order::Asc)
            .order_by((audio_source::Entity, Audio::Id), Order::Asc)
            .order_by((beatmap::Entity, Map::Id), Order::Asc)
            .to_owned();
        let rows =
            SetWithAudio::find_by_statement(self.connection.get_database_backend().build(&query))
                .all(&self.connection)
                .await?;
        let mut sets = BTreeMap::<i32, BeatmapSetWithAudio>::new();
        for row in rows {
            let set = sets.entry(row.id).or_insert_with(|| BeatmapSetWithAudio {
                beatmap_set: BeatmapSet {
                    id: row.id,
                    online_id: row.online_id,
                    hash: row.hash,
                    installation_id: row.installation_id,
                },
                audio_sources: Vec::new(),
                beatmaps: Vec::new(),
            });
            if let Some(id) = row.audio_id
                && set.audio_sources.last().is_none_or(|audio| audio.id != id)
            {
                set.audio_sources.push(AudioSource {
                    id,
                    s_type: SourceType::from_parts(
                        &row.audio_kind.context("joined audio source has no kind")?,
                        row.audio_location
                            .context("joined audio source has no location")?,
                    )?,
                });
            }
            if let Some(id) = row.map_id {
                set.beatmaps.push(BeatmapDetails {
                    id,
                    audio_source_id: row.audio_id,
                    difficulty_name: row.difficulty_name,
                    title: row.title,
                    title_unicode: row.title_unicode,
                    artist: row.artist,
                    artist_unicode: row.artist_unicode,
                    has_cover: row.background_path.is_some(),
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
    map_id: Option<i32>,
    difficulty_name: Option<String>,
    background_path: Option<String>,
    title: Option<String>,
    title_unicode: Option<String>,
    artist: Option<String>,
    artist_unicode: Option<String>,
}

async fn insert(
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

async fn delete_for_installation(
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
