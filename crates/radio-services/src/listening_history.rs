use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use radio_db::{Transaction, model::SourceType, repositories::ListeningHistoryRepository};

use crate::{LibraryTrack, Playlist};

pub struct ListeningHistoryService<'a> {
    pub(crate) repository: ListeningHistoryRepository<'a>,
}

impl ListeningHistoryService<'_> {
    pub async fn for_audio_sources(&self, ids: &[i32]) -> Result<HashMap<i32, i64>> {
        self.repository.for_audio_sources(ids).await
    }

    pub(crate) async fn record_started(
        transaction: &Transaction,
        source: &SourceType,
        playback_token: u64,
    ) -> Result<bool> {
        let millis = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        let millis = i64::try_from(millis).context("Playback timestamp exceeds BIGINT")?;
        transaction
            .listening_history()
            .record_started(source, playback_token, millis)
            .await
    }

    pub(crate) async fn fill_tracks(
        transaction: &Transaction,
        tracks: &mut [LibraryTrack],
    ) -> Result<()> {
        let ids = tracks
            .iter()
            .map(|track| track.audio_source_id)
            .collect::<Vec<_>>();
        let dates = transaction
            .listening_history()
            .for_audio_sources(&ids)
            .await?;
        for track in tracks {
            track.last_played_at_ms = dates.get(&track.audio_source_id).copied();
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
            .filter_map(|item| item.audio_source_id)
            .collect::<Vec<_>>();
        let dates = transaction
            .listening_history()
            .for_audio_sources(&ids)
            .await?;
        for item in &mut playlist.items {
            item.last_played_at_ms = item.audio_source_id.and_then(|id| dates.get(&id).copied());
        }
        Ok(())
    }
}
