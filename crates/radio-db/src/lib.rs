#[cfg(all(feature = "sqlite", feature = "postgres"))]
compile_error!("choose one db backend only");

#[cfg(not(any(feature = "sqlite", feature = "postgres")))]
compile_error!("radio-db requires at least one database backend feature: sqlite or postgres");

use anyhow::Result;

use diesel_async::{AsyncConnection, SimpleAsyncConnection};
use radio_core::import_types::ImportedBeatmapSet;

mod connection;
pub mod model;
pub mod repositories;
pub mod schema;
use connection::DatabaseConnection;
pub use repositories::beatmap::ImportSummary;

const CREATE_SCHEMA: &str =
    include_str!("migrations/2026-07-09-194951-0000_create_initial_schema/up.sql");
const DROP_SCHEMA: &str =
    include_str!("migrations/2026-07-09-194951-0000_create_initial_schema/down.sql");

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

    pub async fn apply_schema(&mut self) -> Result<()> {
        self.connection.batch_execute(CREATE_SCHEMA).await?;
        Ok(())
    }

    pub async fn reset_schema(&mut self) -> Result<()> {
        self.connection.batch_execute(DROP_SCHEMA).await?;
        self.apply_schema().await
    }

    pub async fn import_beatmap_sets(
        &mut self,
        beatmap_sets: &[ImportedBeatmapSet],
        beatmap_set_limit: Option<usize>,
    ) -> Result<ImportSummary> {
        repositories::beatmap::insert_beatmap_sets(
            &mut self.connection,
            beatmap_sets,
            beatmap_set_limit,
        )
        .await
    }
}

#[cfg(feature = "sqlite")]
async fn initialize_connection(connection: &mut DatabaseConnection) -> Result<()> {
    connection
        .batch_execute("PRAGMA foreign_keys = ON;")
        .await?;
    Ok(())
}

#[cfg(feature = "postgres")]
async fn initialize_connection(connection: &mut DatabaseConnection) -> Result<()> {
    connection
        .batch_execute("SET application_name = 'osu-radio';")
        .await?;
    Ok(())
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

    #[cfg(feature = "sqlite")]
    #[tokio::test]
    async fn loads_beatmap_metadata_with_its_joined_audio_source() {
        use crate::connection::DatabaseConnection;
        use crate::model::{BeatmapMetadata, NewAudioSource, SourceType};
        use crate::schema::{audio_sources, beatmap_metadata};
        use diesel::prelude::*;
        use diesel_async::{AsyncConnection, RunQueryDsl, SimpleAsyncConnection};

        let mut connection = DatabaseConnection::establish(":memory:")
            .await
            .expect("database connection should open");
        connection
            .batch_execute(super::CREATE_SCHEMA)
            .await
            .expect("initial schema migration should apply");

        let source = SourceType::Local("/osu/files/a/ab/abc".to_owned());
        diesel::insert_into(audio_sources::table)
            .values((audio_sources::id.eq(1), NewAudioSource::from(&source)))
            .execute(&mut connection)
            .await
            .expect("audio source should insert");
        diesel::insert_into(beatmap_metadata::table)
            .values((
                beatmap_metadata::id.eq(1),
                beatmap_metadata::title.eq("Song"),
                beatmap_metadata::audio_source_id.eq(1),
            ))
            .execute(&mut connection)
            .await
            .expect("metadata should insert");

        let loaded: Vec<BeatmapMetadata> = beatmap_metadata::table
            .left_join(audio_sources::table)
            .select(BeatmapMetadata::as_select())
            .load(&mut connection)
            .await
            .expect("metadata should load with its audio source");

        let audio_source = loaded[0]
            .audio_source
            .as_ref()
            .expect("joined audio source should be present");
        assert_eq!(audio_source.s_type, source);
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
