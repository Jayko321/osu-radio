use crate::{entities::audio_volume, model::SourceType};
use anyhow::Result;
use sea_orm::{ConnectionTrait, EntityTrait, Set, sea_query::OnConflict};
use std::collections::HashMap;

pub struct AudioVolumeRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}
impl AudioVolumeRepository<'_> {
    pub async fn set(&self, source: &SourceType, percent: u8) -> Result<()> {
        anyhow::ensure!(percent <= 100, "Volume must be between 0 and 100");
        audio_volume::Entity::insert(audio_volume::ActiveModel {
            audio_kind: Set(source.kind().to_owned()),
            source_location: Set(source.location().to_owned()),
            volume_percent: Set(i32::from(percent)),
        })
        .on_conflict(
            OnConflict::columns([
                audio_volume::Column::AudioKind,
                audio_volume::Column::SourceLocation,
            ])
            .update_column(audio_volume::Column::VolumePercent)
            .to_owned(),
        )
        .exec_without_returning(&self.connection)
        .await?;
        Ok(())
    }
    pub async fn delete(&self, source: &SourceType) -> Result<()> {
        audio_volume::Entity::delete_by_id((
            source.kind().to_owned(),
            source.location().to_owned(),
        ))
        .exec(&self.connection)
        .await?;
        Ok(())
    }
    pub async fn for_sources(
        &self,
        sources: &[SourceType],
    ) -> Result<HashMap<(String, String), u8>> {
        use sea_orm::{ColumnTrait, Condition, QueryFilter};
        let mut values = HashMap::new();
        for chunk in sources.chunks(250) {
            let condition = chunk.iter().fold(Condition::any(), |condition, source| {
                condition.add(
                    Condition::all()
                        .add(audio_volume::Column::AudioKind.eq(source.kind()))
                        .add(audio_volume::Column::SourceLocation.eq(source.location())),
                )
            });
            for row in audio_volume::Entity::find()
                .filter(condition)
                .all(&self.connection)
                .await?
            {
                values.insert(
                    (row.audio_kind, row.source_location),
                    u8::try_from(row.volume_percent)?,
                );
            }
        }
        Ok(values)
    }

    pub async fn for_audio_sources(&self, ids: &[i32]) -> Result<HashMap<i32, u8>> {
        let mut volumes = HashMap::new();
        for chunk in ids.chunks(500) {
            let placeholders = (1..=chunk.len())
                .map(|index| {
                    #[cfg(feature = "postgres")]
                    {
                        format!("${index}")
                    }
                    #[cfg(feature = "sqlite")]
                    {
                        let _ = index;
                        "?".to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            let rows = self.connection.query_all_raw(sea_orm::Statement::from_sql_and_values(
                self.connection.get_database_backend(),
                format!("SELECT a.id, v.volume_percent FROM audio_sources a \
                    JOIN audio_volume v ON v.audio_kind = a.kind AND v.source_location = a.location \
                    WHERE a.id IN ({placeholders})"),
                chunk.iter().copied().map(sea_orm::Value::from),
            )).await?;
            for row in rows {
                let percent: i32 = row.try_get("", "volume_percent")?;
                volumes.insert(row.try_get("", "id")?, u8::try_from(percent)?);
            }
        }
        Ok(volumes)
    }
}
