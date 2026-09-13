use anyhow::{Context, Result};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, QueryTrait, Set,
    sea_query::OnConflict,
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
        cleanup(&self.connection).await
    }

    pub async fn get(&self, id: i32) -> Result<Option<AudioSource>> {
        audio_source::Entity::find_by_id(id)
            .one(&self.connection)
            .await?
            .map(into_model)
            .transpose()
    }

    pub async fn find(&self, source: &SourceType) -> Result<Option<AudioSource>> {
        find(&self.connection, source).await
    }

    pub async fn get_or_insert(&self, source: &SourceType) -> Result<AudioSource> {
        get_or_insert(&self.connection, source).await
    }
}

async fn find(
    connection: &impl ConnectionTrait,
    source: &SourceType,
) -> Result<Option<AudioSource>> {
    audio_source::Entity::find()
        .filter(audio_source::Column::Kind.eq(source.kind()))
        .filter(audio_source::Column::Location.eq(source.location()))
        .one(connection)
        .await?
        .map(into_model)
        .transpose()
}

async fn get_or_insert(
    connection: &impl ConnectionTrait,
    source: &SourceType,
) -> Result<AudioSource> {
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
    .exec_without_returning(connection)
    .await?;
    find(connection, source)
        .await?
        .context("audio source missing after insertion")
}

async fn cleanup(connection: &impl ConnectionTrait) -> Result<()> {
    audio_source::Entity::delete_many()
        .filter(
            audio_source::Column::Id.not_in_subquery(
                beatmap::Entity::find()
                    .select_only()
                    .column(beatmap::Column::AudioSourceId)
                    .filter(beatmap::Column::AudioSourceId.is_not_null())
                    .into_query(),
            ),
        )
        .exec(connection)
        .await?;
    Ok(())
}

pub(crate) fn into_model(value: audio_source::Model) -> Result<AudioSource> {
    Ok(AudioSource {
        id: value.id,
        s_type: SourceType::from_parts(&value.kind, value.location)?,
    })
}
