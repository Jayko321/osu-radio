use crate::{LibraryTrack, Playlist};
use anyhow::Result;
pub use radio_db::model::AudioSettings;
use radio_db::{Database, Transaction};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSettingsError {
    UnknownAudio(i32),
    InvalidPercent,
}
impl std::fmt::Display for AudioSettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownAudio(id) => write!(f, "Audio source {id} was not found."),
            Self::InvalidPercent => write!(f, "Volume must be an integer between 0 and 100."),
        }
    }
}
impl std::error::Error for AudioSettingsError {}

pub struct AudioSettingsService<'a> {
    pub(crate) database: &'a Database,
}
impl AudioSettingsService<'_> {
    pub async fn get(&self) -> Result<AudioSettings> {
        self.database.user_data().audio_settings().await
    }
    pub async fn update(
        &self,
        enabled: Option<bool>,
        percent: Option<u8>,
    ) -> Result<AudioSettings> {
        if percent.is_some_and(|p| p > 100) {
            return Err(AudioSettingsError::InvalidPercent.into());
        }
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        let settings = transaction
            .user_data()
            .update_audio_settings(enabled, percent)
            .await?;
        transaction.commit().await?;
        Ok(settings)
    }
    /// None removes the override. The source key survives library cleanup and reimport.
    pub async fn set_volume(&self, id: i32, percent: Option<u8>) -> Result<()> {
        if percent.is_some_and(|p| p > 100) {
            return Err(AudioSettingsError::InvalidPercent.into());
        }
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        let source = transaction
            .audio_sources()
            .get(id)
            .await?
            .ok_or(AudioSettingsError::UnknownAudio(id))?;
        if let Some(percent) = percent {
            transaction
                .audio_volume()
                .set(&source.s_type, percent)
                .await?;
        } else {
            transaction.audio_volume().delete(&source.s_type).await?;
        }
        transaction.commit().await
    }
    pub(crate) async fn fill_tracks(
        transaction: &Transaction,
        tracks: &mut [LibraryTrack],
    ) -> Result<()> {
        let ids = tracks.iter().map(|t| t.audio_source_id).collect::<Vec<_>>();
        let volumes = transaction.audio_volume().for_audio_sources(&ids).await?;
        for track in tracks {
            track.volume_percent = volumes.get(&track.audio_source_id).copied();
        }
        Ok(())
    }
    pub(crate) async fn fill_playlist(
        transaction: &Transaction,
        playlist: &mut Playlist,
    ) -> Result<()> {
        let ids = playlist
            .items
            .iter()
            .filter_map(|i| i.audio_source_id)
            .collect::<Vec<_>>();
        let volumes = transaction.audio_volume().for_audio_sources(&ids).await?;
        for item in &mut playlist.items {
            item.volume_percent = item
                .audio_source_id
                .and_then(|id| volumes.get(&id).copied());
        }
        Ok(())
    }
}
