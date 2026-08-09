use std::sync::Arc;

use anyhow::{Context, Result};
use radio_db::Database;
use tokio::sync::{Mutex, MutexGuard};

#[derive(Clone)]
pub(crate) struct AppState {
    database: Arc<Mutex<Database>>,
}

impl AppState {
    pub(crate) async fn connect(database_url: &str) -> Result<Self> {
        let mut database = Database::connect(database_url)
            .await
            .context("Failed to connect to the configured SQLite database")?;

        database
            .check_connection()
            .await
            .context("The configured SQLite database did not respond to a connectivity check")?;
        database
            .apply_schema()
            .await
            .context("Failed to apply the beatmap schema to the configured SQLite database")?;

        Ok(Self {
            database: Arc::new(Mutex::new(database)),
        })
    }

    pub(crate) async fn database(&self) -> MutexGuard<'_, Database> {
        self.database.lock().await
    }
}
