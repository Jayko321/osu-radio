use anyhow::{Context, Result};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QuerySelect, Set};

use crate::{
    entities::user_data,
    model::{AudioSettings, USER_DATA_ID, UserData},
};

pub struct UserDataRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}

impl UserDataRepository<'_> {
    pub async fn audio_settings(&self) -> Result<AudioSettings> {
        let row = user_data::Entity::find_by_id(USER_DATA_ID)
            .one(&self.connection)
            .await?
            .context("Settings row is missing")?;
        Ok(AudioSettings {
            individual_volume_enabled: row.individual_volume_enabled,
            global_volume_percent: u8::try_from(row.global_volume_percent)?,
        })
    }
    pub async fn update_audio_settings(
        &self,
        enabled: Option<bool>,
        percent: Option<u8>,
    ) -> Result<AudioSettings> {
        anyhow::ensure!(
            percent.is_none_or(|p| p <= 100),
            "Volume must be between 0 and 100"
        );
        if enabled.is_some() || percent.is_some() {
            let mut changes = user_data::ActiveModel::default();
            if let Some(enabled) = enabled {
                changes.individual_volume_enabled = Set(enabled);
            }
            if let Some(percent) = percent {
                changes.global_volume_percent = Set(i32::from(percent));
            }
            user_data::Entity::update_many()
                .set(changes)
                .filter(user_data::Column::Id.eq(USER_DATA_ID))
                .exec(&self.connection)
                .await?;
        }
        self.audio_settings().await
    }
    /// Acquires a SQLite writer lock before reads and a PostgreSQL row lock.
    pub async fn lock(&self) -> Result<()> {
        // ponytail: one global writer serializes snapshot replacement and cleanup across processes;
        // use finer locks and coordinated garbage collection if write throughput becomes a bottleneck.
        let result = self
            .connection
            .execute_unprepared("UPDATE user_data SET id = id WHERE id = 1")
            .await?;
        anyhow::ensure!(
            result.rows_affected() == 1,
            "Settings row is missing; apply database migrations first"
        );
        Ok(())
    }

    pub async fn get(&self) -> Result<UserData> {
        let row = user_data::Entity::find_by_id(USER_DATA_ID)
            .select_only()
            .column(user_data::Column::Id)
            .into_tuple::<i32>()
            .one(&self.connection)
            .await?
            .context("Settings row is missing; apply database migrations first")?;
        Ok(UserData { id: row })
    }
}
