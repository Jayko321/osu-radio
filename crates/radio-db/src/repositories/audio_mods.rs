use crate::{
    entities::audio_mods,
    model::{AudioModsProfile, SourceType},
};
use anyhow::Result;
use sea_orm::{ConnectionTrait, EntityTrait, Set, sea_query::OnConflict};
use std::collections::HashMap;

pub struct AudioModsRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}
impl AudioModsRepository<'_> {
    pub async fn set(&self, source: &SourceType, profile: AudioModsProfile) -> Result<()> {
        anyhow::ensure!(profile.is_valid(), "Invalid audio mods profile");
        audio_mods::Entity::insert(audio_mods::ActiveModel {
            audio_kind: Set(source.kind().to_owned()),
            source_location: Set(source.location().to_owned()),
            mode: Set(profile.mode.as_str().to_owned()),
            speed_percent: Set(i32::from(profile.speed_percent)),
            adjust_pitch: Set(profile.adjust_pitch),
        })
        .on_conflict(
            OnConflict::columns([
                audio_mods::Column::AudioKind,
                audio_mods::Column::SourceLocation,
            ])
            .update_columns([
                audio_mods::Column::Mode,
                audio_mods::Column::SpeedPercent,
                audio_mods::Column::AdjustPitch,
            ])
            .to_owned(),
        )
        .exec_without_returning(&self.connection)
        .await?;
        Ok(())
    }
    pub async fn delete(&self, source: &SourceType) -> Result<()> {
        audio_mods::Entity::delete_by_id((source.kind().to_owned(), source.location().to_owned()))
            .exec(&self.connection)
            .await?;
        Ok(())
    }
    pub async fn for_sources(
        &self,
        sources: &[SourceType],
    ) -> Result<HashMap<(String, String), AudioModsProfile>> {
        use sea_orm::{ColumnTrait, Condition, QueryFilter};
        let mut values = HashMap::new();
        for chunk in sources.chunks(250) {
            let condition = chunk.iter().fold(Condition::any(), |condition, source| {
                condition.add(
                    Condition::all()
                        .add(audio_mods::Column::AudioKind.eq(source.kind()))
                        .add(audio_mods::Column::SourceLocation.eq(source.location())),
                )
            });
            for row in audio_mods::Entity::find()
                .filter(condition)
                .all(&self.connection)
                .await?
            {
                values.insert(
                    (row.audio_kind, row.source_location),
                    decode_profile(&row.mode, row.speed_percent, row.adjust_pitch)?,
                );
            }
        }
        Ok(values)
    }

    pub async fn for_audio_sources(&self, ids: &[i32]) -> Result<HashMap<i32, AudioModsProfile>> {
        let mut profiles = HashMap::new();
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
            let rows = self
                .connection
                .query_all_raw(sea_orm::Statement::from_sql_and_values(
                    self.connection.get_database_backend(),
                    format!(
                        "SELECT a.id, v.mode, v.speed_percent, v.adjust_pitch FROM audio_sources a \
                    JOIN audio_mods v ON v.audio_kind = a.kind AND v.source_location = a.location \
                    WHERE a.id IN ({placeholders})"
                    ),
                    chunk.iter().copied().map(sea_orm::Value::from),
                ))
                .await?;
            for row in rows {
                let profile = decode_profile(
                    &row.try_get::<String>("", "mode")?,
                    row.try_get("", "speed_percent")?,
                    row.try_get("", "adjust_pitch")?,
                )?;
                profiles.insert(row.try_get("", "id")?, profile);
            }
        }
        Ok(profiles)
    }
}

fn decode_profile(mode: &str, speed_percent: i32, adjust_pitch: bool) -> Result<AudioModsProfile> {
    let profile = AudioModsProfile {
        mode: mode.parse()?,
        speed_percent: u16::try_from(speed_percent)?,
        adjust_pitch,
    };
    anyhow::ensure!(profile.is_valid(), "Invalid stored audio mods profile");
    Ok(profile)
}
