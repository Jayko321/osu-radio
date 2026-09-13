#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unwrap_used
    )
)]

#[cfg(all(feature = "sqlite", feature = "postgres"))]
compile_error!("radio-db requires exactly one backend: enable sqlite or postgres, not both");
#[cfg(not(any(feature = "sqlite", feature = "postgres")))]
compile_error!("radio-db requires exactly one backend: enable sqlite or postgres");

mod connection;
mod entities;
mod migrations;
pub mod model;
pub mod repositories;

use anyhow::Result;
use repositories::{
    AudioSourceRepository, BeatmapMetadataRepository, BeatmapRepository, BeatmapSetRepository,
    OsuInstallationRepository, UserDataRepository,
};
pub use repositories::{ImportSummary, RegisteredInstallation, metadata_hash};

/// Cloneable pool handle. Domain operations are exposed by concrete repositories.
#[derive(Clone)]
pub struct Database {
    connection: sea_orm::DatabaseConnection,
}

impl Database {
    /// Opens the pool without modifying the schema, so legacy databases can be reset explicitly.
    pub async fn connect(database_url: &str) -> Result<Self> {
        Ok(Self {
            connection: connection::connect(database_url).await?,
        })
    }

    pub async fn check_connection(&self) -> Result<()> {
        self.connection.ping().await?;
        Ok(())
    }

    pub async fn migrate(&self) -> Result<()> {
        migrations::migrate(&self.connection).await
    }

    /// Discards application data only. This is never called by ordinary startup.
    pub async fn reset(&self) -> Result<()> {
        migrations::reset(&self.connection).await
    }

    #[must_use]
    pub const fn user_data(&self) -> UserDataRepository<'_> {
        UserDataRepository {
            connection: &self.connection,
        }
    }
    #[must_use]
    pub const fn osu_installations(&self) -> OsuInstallationRepository<'_> {
        OsuInstallationRepository {
            connection: &self.connection,
        }
    }
    #[must_use]
    pub const fn beatmap_sets(&self) -> BeatmapSetRepository<'_> {
        BeatmapSetRepository {
            connection: &self.connection,
        }
    }
    #[must_use]
    pub const fn beatmaps(&self) -> BeatmapRepository<'_> {
        BeatmapRepository {
            connection: &self.connection,
        }
    }
    #[must_use]
    pub const fn beatmap_metadata(&self) -> BeatmapMetadataRepository<'_> {
        BeatmapMetadataRepository {
            connection: &self.connection,
        }
    }
    #[must_use]
    pub const fn audio_sources(&self) -> AudioSourceRepository<'_> {
        AudioSourceRepository {
            connection: &self.connection,
        }
    }
}

#[cfg(test)]
mod tests;
