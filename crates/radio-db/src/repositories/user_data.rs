use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait};

use crate::{
    entities::user_data,
    model::{USER_DATA_ID, UserData},
};

pub struct UserDataRepository<'a> {
    pub(crate) connection: &'a DatabaseConnection,
}

impl UserDataRepository<'_> {
    pub async fn get(&self) -> Result<UserData> {
        let row = user_data::Entity::find_by_id(USER_DATA_ID)
            .one(self.connection)
            .await?
            .context("Settings row is missing; apply database migrations first")?;
        Ok(UserData { id: row.id })
    }
}

/// A write statement acquires a SQLite writer lock before any reads and a PostgreSQL row lock.
pub(crate) async fn lock(connection: &impl ConnectionTrait) -> Result<()> {
    // ponytail: one global writer serializes snapshot replacement and cleanup across processes;
    // use finer locks and coordinated garbage collection if write throughput becomes a bottleneck.
    let result = connection
        .execute_unprepared("UPDATE user_data SET id = id WHERE id = 1")
        .await?;
    anyhow::ensure!(
        result.rows_affected() == 1,
        "Settings row is missing; apply database migrations first"
    );
    Ok(())
}
