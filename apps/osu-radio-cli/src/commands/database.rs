use std::env;

use anyhow::{Context, Result};
use radio_db::Database;

const SQLITE_DATABASE_URL_ENV: &str = "SQLITE_DATABASE_URL";

pub(crate) async fn database() -> Result<()> {
    open_database().await?;
    println!("Connected to the SQLite database configured in .env.");

    Ok(())
}

pub(crate) async fn open_database() -> Result<Database> {
    dotenvy::dotenv().context("Failed to load .env")?;

    let database_url = env::var(SQLITE_DATABASE_URL_ENV)
        .with_context(|| format!("{SQLITE_DATABASE_URL_ENV} must be set in .env"))?;

    connect(&database_url).await
}

async fn connect(database_url: &str) -> Result<Database> {
    let mut database = Database::connect(database_url)
        .await
        .context("Failed to connect to the configured SQLite database")?;

    database
        .check_connection()
        .await
        .context("The configured SQLite database did not respond to a connectivity check")?;

    Ok(database)
}

#[cfg(test)]
mod tests {
    use super::connect;

    #[tokio::test]
    async fn connects_to_an_in_memory_sqlite_database() {
        connect(":memory:")
            .await
            .expect("in-memory SQLite database should connect");
    }
}
