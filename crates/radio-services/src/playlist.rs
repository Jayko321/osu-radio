use anyhow::Result;
use image::{ImageDecoder, codecs::png::PngDecoder};
use radio_db::Database;
pub use radio_db::model::{Playlist, PlaylistItem, PlaylistSummary};
use std::{collections::HashMap, error::Error, fmt};

use crate::{ListeningHistoryService, PlaybackAssignment, QueueService};

pub const MAX_PLAYLIST_COVER_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistError {
    EmptyName,
    NotFound(i32),
    ItemNotFound(i32),
    InvalidBeatmap(i32),
    MissingHash(i32),
    NoAvailableItems,
    UnavailableItem(i32),
    InvalidCover,
    CoverTooLarge,
}
impl fmt::Display for PlaylistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "Playlist name cannot be empty."),
            Self::NotFound(id) => write!(f, "Playlist {id} was not found."),
            Self::ItemNotFound(id) => write!(f, "Playlist item {id} was not found."),
            Self::InvalidBeatmap(id) => write!(
                f,
                "Beatmap {id} was not found. Refresh the library and try again."
            ),
            Self::MissingHash(id) => write!(
                f,
                "Beatmap {id} has no source hash and cannot be added to a playlist."
            ),
            Self::NoAvailableItems => write!(f, "This playlist has no available entries."),
            Self::UnavailableItem(id) => write!(f, "Playlist item {id} is unavailable."),
            Self::InvalidCover => write!(f, "Playlist cover must be a valid 512×512 PNG image."),
            Self::CoverTooLarge => write!(f, "Playlist cover must not exceed 2 MiB."),
        }
    }
}
impl Error for PlaylistError {}

pub struct PlaylistService<'a> {
    pub(crate) database: &'a Database,
}
impl PlaylistService<'_> {
    pub async fn all(&self) -> Result<Vec<PlaylistSummary>> {
        let transaction = self.database.begin_read().await?;
        let result = transaction.playlists().all().await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub async fn get(&self, id: i32) -> Result<Option<Playlist>> {
        let transaction = self.database.begin_read().await?;
        let mut result = transaction.playlists().get(id).await?;
        if let Some(playlist) = &mut result {
            ListeningHistoryService::fill_playlist(&transaction, playlist).await?;
            crate::AudioSettingsService::fill_playlist(&transaction, playlist).await?;
        }
        transaction.commit().await?;
        Ok(result)
    }

    pub async fn create(&self, name: &str) -> Result<PlaylistSummary> {
        let name = checked_name(name)?;
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        let result = transaction.playlists().create(name).await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub async fn rename(&self, id: i32, name: &str) -> Result<PlaylistSummary> {
        let name = checked_name(name)?;
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        if !transaction.playlists().rename(id, name).await? {
            return Err(PlaylistError::NotFound(id).into());
        }
        let result = transaction
            .playlists()
            .summary(id)
            .await?
            .ok_or(PlaylistError::NotFound(id))?;
        transaction.commit().await?;
        Ok(result)
    }

    pub async fn cover(&self, id: i32) -> Result<Option<Vec<u8>>> {
        self.database.playlists().cover(id).await
    }

    pub async fn set_cover(&self, id: i32, png: &[u8]) -> Result<PlaylistSummary> {
        validate_cover(png)?;
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        if !transaction.playlists().set_cover(id, png).await? {
            return Err(PlaylistError::NotFound(id).into());
        }
        let result = transaction
            .playlists()
            .summary(id)
            .await?
            .ok_or(PlaylistError::NotFound(id))?;
        transaction.commit().await?;
        Ok(result)
    }

    pub async fn clear_cover(&self, id: i32) -> Result<()> {
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        if !transaction.playlists().clear_cover(id).await? {
            return Err(PlaylistError::NotFound(id).into());
        }
        transaction.commit().await
    }

    pub async fn delete(&self, id: i32) -> Result<()> {
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        if !transaction.playlists().delete(id).await? {
            return Err(PlaylistError::NotFound(id).into());
        }
        transaction.commit().await
    }

    pub async fn add_items(&self, id: i32, beatmap_ids: &[i32]) -> Result<Playlist> {
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        if transaction.playlists().get(id).await?.is_none() {
            return Err(PlaylistError::NotFound(id).into());
        }
        let maps: HashMap<_, _> = transaction
            .playlists()
            .beatmaps(beatmap_ids)
            .await?
            .into_iter()
            .map(|map| (map.beatmap_id, map))
            .collect();
        for id in beatmap_ids {
            let map = maps.get(id).ok_or(PlaylistError::InvalidBeatmap(*id))?;
            if map
                .beatmap_hash
                .as_deref()
                .is_none_or(|hash| hash.trim().is_empty())
            {
                return Err(PlaylistError::MissingHash(*id).into());
            }
        }
        for beatmap_id in beatmap_ids {
            if let Some(map) = maps.get(beatmap_id) {
                transaction.playlists().add(id, map).await?;
            }
        }
        let mut result = transaction
            .playlists()
            .get(id)
            .await?
            .ok_or(PlaylistError::NotFound(id))?;
        ListeningHistoryService::fill_playlist(&transaction, &mut result).await?;
        crate::AudioSettingsService::fill_playlist(&transaction, &mut result).await?;
        transaction.commit().await?;
        Ok(result)
    }

    pub async fn remove_item(&self, id: i32, item_id: i32) -> Result<()> {
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        if transaction.playlists().get(id).await?.is_none() {
            return Err(PlaylistError::NotFound(id).into());
        }
        if !transaction.playlists().remove_item(id, item_id).await? {
            return Err(PlaylistError::ItemNotFound(item_id).into());
        }
        transaction.commit().await
    }

    pub async fn play(&self, id: i32, start_item_id: Option<i32>) -> Result<PlaybackAssignment> {
        let transaction = self.database.begin().await?;
        transaction.user_data().lock().await?;
        let playlist = transaction
            .playlists()
            .get(id)
            .await?
            .ok_or(PlaylistError::NotFound(id))?;
        if let Some(item_id) = start_item_id {
            let item = playlist
                .items
                .iter()
                .find(|item| item.id == item_id)
                .ok_or(PlaylistError::ItemNotFound(item_id))?;
            if item.audio_source_id.is_none() {
                return Err(PlaylistError::UnavailableItem(item_id).into());
            }
        }
        let available: Vec<_> = playlist
            .items
            .iter()
            .filter(|item| item.audio_source_id.is_some())
            .collect();
        if available.is_empty() {
            return Err(PlaylistError::NoAvailableItems.into());
        }
        let start = start_item_id
            .and_then(|id| available.iter().position(|item| item.id == id))
            .unwrap_or(0);
        let ids = available
            .iter()
            .filter_map(|item| item.audio_source_id)
            .collect();
        let item_ids = available.iter().map(|item| Some(item.id)).collect();
        let result = QueueService::replace_in(&transaction, ids, item_ids, start).await?;
        transaction.commit().await?;
        Ok(result)
    }
}

fn checked_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() {
        return Err(PlaylistError::EmptyName.into());
    }
    Ok(name)
}

fn validate_cover(png: &[u8]) -> Result<()> {
    if png.len() > MAX_PLAYLIST_COVER_BYTES {
        return Err(PlaylistError::CoverTooLarge.into());
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(512);
    limits.max_image_height = Some(512);
    limits.max_alloc = Some(8 * 1024 * 1024);
    let decoder = PngDecoder::with_limits(std::io::Cursor::new(png), limits)
        .map_err(|_| PlaylistError::InvalidCover)?;
    if decoder.dimensions() != (512, 512) {
        return Err(PlaylistError::InvalidCover.into());
    }
    image::DynamicImage::from_decoder(decoder).map_err(|_| PlaylistError::InvalidCover)?;
    Ok(())
}
