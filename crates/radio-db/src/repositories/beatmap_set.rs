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
        Ok(into_model(
            beatmap_set::ActiveModel {
                online_id: Set(imported.online_id),
                hash: Set(imported.hash.clone()),
                installation_id: Set(installation_id),
                ..Default::default()
            }
            .insert(&self.connection)
            .await?,
        ))
    }
    pub async fn delete_for_installation(&self, installation_id: i32) -> Result<()> {
        beatmap_set::Entity::delete_many()
            .filter(beatmap_set::Column::InstallationId.eq(installation_id))
            .exec(&self.connection)
            .await?;
        Ok(())
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

    pub async fn search_difficulties(&self) -> Result<Vec<crate::model::SearchDifficulty>> {
        let statement = sea_orm::Statement::from_string(
            self.connection.get_database_backend(),
            "SELECT beatmap_set_id AS set_id, audio_source_id, metadata_hash, difficulty_name \
             FROM beatmaps WHERE audio_source_id IS NOT NULL",
        );
        Ok(SearchDifficultyRow::find_by_statement(statement)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(|row| crate::model::SearchDifficulty {
                set_id: row.set_id,
                audio_source_id: row.audio_source_id,
                metadata_hash: row.metadata_hash,
                difficulty_name: row.difficulty_name,
            })
            .collect())
    }

    pub async fn all_with_audio_sources(&self) -> Result<Vec<BeatmapSetWithAudio>> {
        self.load_with_audio_sources(None).await
    }

    /// Caller supplies multiplicity from the unfiltered search snapshot.
    /// Chunking is safe only on a transaction-bound repository for a consistent read.
    pub async fn for_audio_sources(
        &self,
        ids: &[i32],
        multiple_audio_sets: &std::collections::HashSet<i32>,
    ) -> Result<Vec<BeatmapSetWithAudio>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut sets = self.load_with_audio_sources(Some(ids)).await?;
        for set in &mut sets {
            set.has_multiple_audio_sources = multiple_audio_sets.contains(&set.beatmap_set.id);
        }
        Ok(sets)
    }

    #[allow(clippy::too_many_lines)] // Keep the aggregate query and materialization together.
    async fn load_with_audio_sources(
        &self,
        ids: Option<&[i32]>,
    ) -> Result<Vec<BeatmapSetWithAudio>> {
        use audio_source::Column as Audio;
        use beatmap::Column as Map;
        use beatmap_metadata::Column as Meta;
        use beatmap_set::Column as SetColumn;

        // Blank queries use one aggregate query; filtered callers hold a read transaction.
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
        let backend = self.connection.get_database_backend();
        let rows = if let Some(ids) = ids {
            let mut rows = Vec::new();
            for chunk in ids.chunks(500) {
                let mut filtered = query.clone();
                filtered.and_where(
                    Expr::col((beatmap::Entity, Map::AudioSourceId)).is_in(chunk.iter().copied()),
                );
                rows.extend(
                    SetWithAudio::find_by_statement(backend.build(&filtered))
                        .all(&self.connection)
                        .await?,
                );
            }
            rows.sort_by_key(|row| (row.id, row.audio_id, row.map_id));
            rows.dedup_by_key(|row| (row.id, row.map_id));
            rows
        } else {
            SetWithAudio::find_by_statement(backend.build(&query))
                .all(&self.connection)
                .await?
        };
        let mut sets = Vec::<BeatmapSetWithAudio>::new();
        for row in rows {
            if sets.last().is_none_or(|set| set.beatmap_set.id != row.id) {
                sets.push(BeatmapSetWithAudio {
                    beatmap_set: BeatmapSet {
                        id: row.id,
                        online_id: row.online_id,
                        hash: row.hash,
                        installation_id: row.installation_id,
                    },
                    has_multiple_audio_sources: false,
                    audio_sources: Vec::new(),
                    beatmaps: Vec::new(),
                });
            }
            let set = sets.last_mut().context("aggregate group missing")?;
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
        for set in &mut sets {
            set.has_multiple_audio_sources = set.audio_sources.len() > 1;
        }
        Ok(sets)
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

fn into_model(value: beatmap_set::Model) -> BeatmapSet {
    BeatmapSet {
        id: value.id,
        online_id: value.online_id,
        hash: value.hash,
        installation_id: value.installation_id,
    }
}

#[derive(FromQueryResult)]
struct SearchDifficultyRow {
    set_id: i32,
    audio_source_id: i32,
    metadata_hash: Option<String>,
    difficulty_name: Option<String>,
}
