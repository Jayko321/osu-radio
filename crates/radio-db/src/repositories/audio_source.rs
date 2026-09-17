use anyhow::{Context, Result};
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, QuerySelect, QueryTrait, Set,
    sea_query::{Expr, ExprTrait, OnConflict},
};

use crate::{
    entities::{audio_source, beatmap},
    model::{AudioSource, SourceType},
};

pub struct AudioSourceRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}

impl AudioSourceRepository<'_> {
    pub async fn cleanup(&self) -> Result<()> {
        audio_source::Entity::delete_many()
            .filter(
                Expr::exists(
                    beatmap::Entity::find()
                        .select_only()
                        .column(beatmap::Column::AudioSourceId)
                        .filter(
                            Expr::col((beatmap::Entity, beatmap::Column::AudioSourceId))
                                .equals((audio_source::Entity, audio_source::Column::Id)),
                        )
                        .into_query(),
                )
                .not(),
            )
            .exec(&self.connection)
            .await?;
        Ok(())
    }

    pub async fn get(&self, id: i32) -> Result<Option<AudioSource>> {
        audio_source::Entity::find_by_id(id)
            .one(&self.connection)
            .await?
            .map(into_model)
            .transpose()
    }

    pub async fn find(&self, source: &SourceType) -> Result<Option<AudioSource>> {
        audio_source::Entity::find()
            .filter(audio_source::Column::Kind.eq(source.kind()))
            .filter(audio_source::Column::Location.eq(source.location()))
            .one(&self.connection)
            .await?
            .map(into_model)
            .transpose()
    }

    pub async fn get_or_insert(&self, source: &SourceType) -> Result<AudioSource> {
        audio_source::Entity::insert(audio_source::ActiveModel {
            kind: Set(source.kind().to_owned()),
            location: Set(source.location().to_owned()),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([audio_source::Column::Kind, audio_source::Column::Location])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&self.connection)
        .await?;
        self.find(source)
            .await?
            .context("audio source missing after insertion")
    }
}

pub(crate) fn into_model(value: audio_source::Model) -> Result<AudioSource> {
    Ok(AudioSource {
        id: value.id,
        s_type: SourceType::from_parts(&value.kind, value.location)?,
    })
}
