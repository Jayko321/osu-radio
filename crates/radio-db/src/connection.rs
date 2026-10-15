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

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::connect;
    use sea_orm::{ConnectionTrait, Statement};

    #[tokio::test]
    async fn memory_url_variants_keep_data_and_do_not_create_files() {
        let directory = tempfile::tempdir().unwrap();
        let named = format!(
            "sqlite://{}?mode=memory&cache=shared",
            directory.path().join("memory.sqlite").display()
        );
        for url in [":memory:", "sqlite::memory:", &named] {
            let connection = connect(url).await.unwrap();
            connection
                .execute_unprepared("CREATE TABLE memory_probe (id INTEGER PRIMARY KEY)")
                .await
                .unwrap();
            connection
                .execute_unprepared("INSERT INTO memory_probe VALUES (42)")
                .await
                .unwrap();
            let row = connection
                .clone()
                .query_one_raw(Statement::from_string(
                    connection.get_database_backend(),
                    "SELECT id FROM memory_probe",
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<i32>("", "id").unwrap(), 42);
        }
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn file_paths_and_urls_enable_wal_foreign_keys_and_the_busy_timeout() {
        let directory = tempfile::tempdir().unwrap();
        for (name, uri) in [("plain path.sqlite", false), ("url.sqlite", true)] {
            let path = directory.path().join(name);
            let url = if uri {
                format!("sqlite://{}", path.display())
            } else {
                path.to_str().unwrap().to_owned()
            };
            let connection = connect(&url).await.unwrap();
            let row = connection
                .query_one_raw(Statement::from_string(
                    connection.get_database_backend(),
                    "PRAGMA journal_mode",
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<String>("", "journal_mode").unwrap(), "wal");
            let row = connection
                .query_one_raw(Statement::from_string(
                    connection.get_database_backend(),
                    "PRAGMA busy_timeout",
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<i32>("", "timeout").unwrap(), 5000);
            connection
                .execute_unprepared("CREATE TABLE parent (id INTEGER PRIMARY KEY)")
                .await
                .unwrap();
            connection
                .execute_unprepared("CREATE TABLE child (parent_id INTEGER REFERENCES parent(id))")
                .await
                .unwrap();
            assert!(
                connection
                    .execute_unprepared("INSERT INTO child VALUES (42)")
                    .await
                    .is_err()
            );
            connection
                .execute_unprepared("INSERT INTO parent VALUES (42)")
                .await
                .unwrap();
            connection
                .execute_unprepared("INSERT INTO child VALUES (42)")
                .await
                .unwrap();
            drop(connection);
            let reopened = connect(&url).await.unwrap();
            let row = reopened
                .query_one_raw(Statement::from_string(
                    reopened.get_database_backend(),
                    "SELECT parent_id FROM child",
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<i32>("", "parent_id").unwrap(), 42);
        }
    }
}
