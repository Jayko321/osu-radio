#[cfg(all(feature = "sqlite", feature = "postgres"))]
compile_error!("choose one db backend only");

#[cfg(not(any(feature = "sqlite", feature = "postgres")))]
compile_error!("radio-db requires at least one database backend feature: sqlite or postgres");

use anyhow::Result;

use diesel_async::{AsyncConnection, SimpleAsyncConnection};

mod connection;
pub mod model;
pub mod schema;
use connection::DatabaseConnection;

pub struct Database {
    connection: DatabaseConnection,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let mut connection = DatabaseConnection::establish(database_url).await?;
        initialize_connection(&mut connection).await?;

        Ok(Self { connection })
    }

    pub async fn check_connection(&mut self) -> Result<()> {
        check_connection(&mut self.connection).await
    }
}

#[cfg(feature = "sqlite")]
async fn initialize_connection(connection: &mut DatabaseConnection) -> Result<()> {
    connection
        .batch_execute("PRAGMA busy_timeout = 2000;")
        .await?;
    connection
        .batch_execute("PRAGMA journal_mode = WAL;")
        .await?;
    connection
        .batch_execute("PRAGMA synchronous = NORMAL;")
        .await?;
    connection
        .batch_execute("PRAGMA wal_autocheckpoint = 1000;")
        .await?;
    check_connection(connection).await
}

#[cfg(feature = "postgres")]
async fn initialize_connection(connection: &mut DatabaseConnection) -> Result<()> {
    connection
        .batch_execute("SET application_name = 'osu-radio';")
        .await?;
    check_connection(connection).await
}

async fn check_connection(connection: &mut impl AsyncConnection) -> Result<()> {
    connection.batch_execute("SELECT 1;").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Database;

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn connects_to_in_memory_sqlite_database_by_default() {
        let mut database = Database::connect(":memory:")
            .await
            .expect("database connection should open");

        database
            .check_connection()
            .await
            .expect("database connection should respond");
    }

    #[cfg(feature = "postgres")]
    #[tokio::test]
    #[ignore = "requires a running PostgreSQL database and POSTGRES_DATABASE_URL"]
    async fn connects_to_configured_postgres_database() {
        dotenvy::dotenv().ok();
        let database_url =
            std::env::var("POSTGRES_DATABASE_URL").expect("POSTGRES_DATABASE_URL must be set");

        let mut database = Database::connect(&database_url)
            .await
            .expect("database connection should open");

        database
            .check_connection()
            .await
            .expect("database connection should respond");
    }
}
