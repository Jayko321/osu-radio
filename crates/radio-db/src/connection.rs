use anyhow::Result;
use sea_orm::{ConnectOptions, DatabaseConnection};

pub(crate) async fn connect(database_url: &str) -> Result<DatabaseConnection> {
    #[cfg(feature = "sqlite")]
    let mut options = sqlite_options(database_url);
    #[cfg(not(feature = "sqlite"))]
    let mut options = ConnectOptions::new(database_url);
    options.sqlx_logging(false);
    Ok(sea_orm::Database::connect(options).await?)
}

#[cfg(feature = "sqlite")]
fn sqlite_options(database_url: &str) -> ConnectOptions {
    use sea_orm::sqlx::sqlite::SqliteJournalMode;
    use std::time::Duration;

    let in_memory = database_url == ":memory:"
        || database_url.starts_with("sqlite::memory:")
        || database_url.contains("mode=memory");
    let url = if in_memory && database_url == ":memory:" {
        "sqlite::memory:".to_owned()
    } else if database_url.starts_with("sqlite:") {
        database_url.to_owned()
    } else {
        format!("sqlite://{database_url}")
    };
    let mut options = ConnectOptions::new(url);
    options.max_connections(if in_memory { 1 } else { 5 });
    options.map_sqlx_sqlite_opts(move |options| {
        let options = options
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        if in_memory {
            options
        } else {
            options.journal_mode(SqliteJournalMode::Wal)
        }
    });
    options
}
