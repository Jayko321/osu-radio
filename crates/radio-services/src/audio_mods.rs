use crate::{LibraryTrack, Playlist};
use anyhow::Result;
pub use radio_db::model::{AudioModsMode, AudioModsProfile, AudioModsSettings};
use radio_db::{Database, Transaction};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AudioModsPatch {
    pub individual_mods_enabled: Option<bool>,
    pub global_mode: Option<AudioModsMode>,
    pub global_speed_percent: Option<u16>,
    pub global_adjust_pitch: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioModsError {
    UnknownAudio(i32),
    InvalidProfile,
}
impl std::fmt::Display for AudioModsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownAudio(id) => write!(f, "Audio source {id} was not found."),
            Self::InvalidProfile => write!(
                f,
                "Invalid audio mods profile: none requires 100/false, DT 101–200, HT 50–99."
            ),
        }
    }
}
impl std::error::Error for AudioModsError {}

pub struct AudioModsService<'a> {
    pub(crate) database: &'a Database,
}
impl AudioModsService<'_> {
    pub async fn get(&self) -> Result<AudioModsSettings> {
        self.database.user_data().audio_mods().await
    }

    /// Omitted fields retain their values; mode changes never supply implicit speed/pitch defaults.
    pub async fn update(&self, patch: AudioModsPatch) -> Result<AudioModsSettings> {
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        let mut settings = transaction.user_data().audio_mods().await?;
        if let Some(enabled) = patch.individual_mods_enabled {
            settings.individual_mods_enabled = enabled;
        }
        if let Some(mode) = patch.global_mode {
            settings.global_profile.mode = mode;
        }
        if let Some(speed) = patch.global_speed_percent {
            settings.global_profile.speed_percent = speed;
        }
        if let Some(adjust_pitch) = patch.global_adjust_pitch {
            settings.global_profile.adjust_pitch = adjust_pitch;
        }
        if !settings.global_profile.is_valid() {
            return Err(AudioModsError::InvalidProfile.into());
        }
        transaction.user_data().update_audio_mods(settings).await?;
        transaction.commit().await?;
        Ok(settings)
    }

    /// None removes the override. Profiles survive library cleanup and are never global fallbacks.
    pub async fn set_profile(&self, id: i32, profile: Option<AudioModsProfile>) -> Result<()> {
        if profile.is_some_and(|profile| !profile.is_valid()) {
            return Err(AudioModsError::InvalidProfile.into());
        }
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        let source = transaction
            .audio_sources()
            .get(id)
            .await?
            .ok_or(AudioModsError::UnknownAudio(id))?;
        if let Some(profile) = profile {
            transaction
                .audio_mods()
                .set(&source.s_type, profile)
                .await?;
        } else {
            transaction.audio_mods().delete(&source.s_type).await?;
        }
        transaction.commit().await
    }
    pub(crate) async fn for_sources_in(
        transaction: &Transaction,
        sources: &[radio_db::model::SourceType],
    ) -> Result<std::collections::HashMap<(String, String), AudioModsProfile>> {
        transaction.audio_mods().for_sources(sources).await
    }

    pub(crate) async fn fill_tracks(
        transaction: &Transaction,
        tracks: &mut [LibraryTrack],
    ) -> Result<()> {
        let ids = tracks.iter().map(|t| t.audio_source_id).collect::<Vec<_>>();
        let profiles = transaction.audio_mods().for_audio_sources(&ids).await?;
        for track in tracks {
            track.mods = profiles.get(&track.audio_source_id).copied();
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
        let profiles = transaction.audio_mods().for_audio_sources(&ids).await?;
        for item in &mut playlist.items {
            item.mods = item
                .audio_source_id
                .and_then(|id| profiles.get(&id).copied());
        }
        Ok(())
    }
}
