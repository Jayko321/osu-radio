#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unwrap_used
    )
)]

mod audio_source;
mod beatmap;
mod beatmap_metadata;
mod beatmap_set;
mod osu_installation;
mod tag;
mod user_data;
pub use tag::TagService;

use anyhow::Result;
pub use audio_source::AudioSourceService;
pub use beatmap::BeatmapService;
pub use beatmap_metadata::BeatmapMetadataService;
pub use beatmap_set::{BeatmapSetService, BeatmapSetWithAudio, LibraryTrack, TrackDifficulty};
pub use osu_installation::{FolderChanges, OsuInstallationService, RegisterFolderError};
use radio_db::Database;
pub use radio_db::{RegisteredInstallation, metadata_hash, model};
pub use user_data::{UserDataOverview, UserDataService};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    pub beatmap_sets: usize,
    pub beatmaps: usize,
    pub audio_sources: usize,
}

/// Shared application boundary for all persisted-model access.
#[derive(Clone)]
pub struct Services {
    database: Database,
}

impl Services {
    pub async fn connect(database_url: &str) -> Result<Self> {
        Ok(Self {
            database: Database::connect(database_url).await?,
        })
    }
    pub async fn check_connection(&self) -> Result<()> {
        self.database.check_connection().await
    }
    pub async fn migrate(&self) -> Result<()> {
        self.database.migrate().await
    }
    /// Explicitly discards application data; ordinary startup only migrates.
    pub async fn reset(&self) -> Result<()> {
        self.database.reset().await
    }
    #[must_use]
    pub const fn user_data(&self) -> UserDataService<'_> {
        UserDataService { services: self }
    }
    #[must_use]
    pub const fn osu_installations(&self) -> OsuInstallationService<'_> {
        OsuInstallationService {
            database: &self.database,
        }
    }
    #[must_use]
    pub const fn beatmap_sets(&self) -> BeatmapSetService<'_> {
        BeatmapSetService {
            database: &self.database,
        }
    }
    #[must_use]
    pub const fn beatmaps(&self) -> BeatmapService<'_> {
        BeatmapService {
            repository: self.database.beatmaps(),
        }
    }
    #[must_use]
    pub const fn beatmap_metadata(&self) -> BeatmapMetadataService<'_> {
        BeatmapMetadataService {
            repository: self.database.beatmap_metadata(),
        }
    }
    #[must_use]
    pub const fn tags(&self) -> TagService<'_> {
        TagService {
            repository: self.database.tags(),
        }
    }
    #[must_use]
    pub const fn audio_sources(&self) -> AudioSourceService<'_> {
        AudioSourceService {
            repository: self.database.audio_sources(),
        }
    }
}

#[cfg(test)]
mod tests;
