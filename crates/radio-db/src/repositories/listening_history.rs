use std::collections::HashMap;

use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, EntityTrait, Set, sea_query::OnConflict};

use crate::{entities::listening_history, model::SourceType};

pub struct ListeningHistoryRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}

impl ListeningHistoryRepository<'_> {
    /// Caller holds the singleton writer lock for the complete queue transaction.
    pub async fn record_started(
        &self,
        source: &SourceType,
        playback_token: u64,
        last_played_at_ms: i64,
    ) -> Result<bool> {
        let playback_token =
            i64::try_from(playback_token).context("Playback token exceeds BIGINT")?;
        if listening_history::Entity::find_by_id((
            source.kind().to_owned(),
            source.location().to_owned(),
        ))
        .one(&self.connection)
        .await?
        .is_some_and(|row| row.playback_token == playback_token)
        {
            return Ok(false);
        }
        listening_history::Entity::insert(listening_history::ActiveModel {
            audio_kind: Set(source.kind().to_owned()),
            source_location: Set(source.location().to_owned()),
            last_played_at_ms: Set(last_played_at_ms),
            playback_token: Set(playback_token),
        })
        .on_conflict(
            OnConflict::columns([
                listening_history::Column::AudioKind,
                listening_history::Column::SourceLocation,
            ])
            .update_columns([
                listening_history::Column::LastPlayedAtMs,
                listening_history::Column::PlaybackToken,
            ])
            .to_owned(),
        )
        .exec_without_returning(&self.connection)
        .await?;
        Ok(true)
    }

    /// Resolve dates by the current source IDs without binding history to their lifetime.
    pub async fn for_audio_sources(&self, ids: &[i32]) -> Result<HashMap<i32, i64>> {
        let mut dates = HashMap::new();
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
                format!("SELECT a.id, h.last_played_at_ms FROM audio_sources a \
                    JOIN listening_history h ON h.audio_kind = a.kind AND h.source_location = a.location \
                    WHERE a.id IN ({placeholders})"),
                chunk.iter().copied().map(sea_orm::Value::from),
            )).await?;
            for row in rows {
                dates.insert(
                    row.try_get("", "id")?,
                    row.try_get("", "last_played_at_ms")?,
                );
            }
        }
        Ok(dates)
    }
}
