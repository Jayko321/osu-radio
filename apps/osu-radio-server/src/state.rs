use anyhow::{Context, Result};
use radio_db::Database;

#[derive(Clone)]
pub(crate) struct AppState {
    database: Database,
}

impl AppState {
    pub(crate) async fn connect(database_url: &str) -> Result<Self> {
        let database = Database::connect(database_url)
            .await
            .context("Failed to connect to the configured database")?;

        database
            .check_connection()
            .await
            .context("The configured database did not respond to a connectivity check")?;
        database
            .migrate()
            .await
            .context("Failed to apply the beatmap schema to the configured database")?;

        Ok(Self { database })
    }

    pub(crate) const fn database(&self) -> &Database {
        &self.database
    }
}
