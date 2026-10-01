use anyhow::{Context, Result, bail, ensure};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::{
    entities::queue,
    model::{PlaybackMode, QueueState},
};

pub struct QueueRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}

impl QueueRepository<'_> {
    pub async fn get(&self) -> Result<QueueState> {
        let row = queue::Entity::find_by_id(1)
            .one(&self.connection)
            .await?
            .context("Playback queue row is missing; apply database migrations first")?;
        let audio_source_ids: Vec<i32> = serde_json::from_value(row.audio_source_ids)?;
        let playlist_item_ids = row
            .playlist_item_ids
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_else(|| vec![None; audio_source_ids.len()]);
        let state = QueueState {
            audio_source_ids,
            playlist_item_ids,
            current_index: row.current_index.map(usize::try_from).transpose()?,
            mode: match row.mode.as_str() {
                "stopped" => PlaybackMode::Stopped,
                "paused" => PlaybackMode::Paused,
                "playing" => PlaybackMode::Playing,
                _ => bail!("Invalid persisted playback mode"),
            },
            revision: u64::try_from(row.revision)?,
            playback_token: u64::try_from(row.playback_token)?,
        };
        validate(&state)?;
        Ok(state)
    }

    /// Caller must hold the singleton writer lock throughout read/change/save.
    pub async fn save(&self, state: &QueueState) -> Result<()> {
        validate(state)?;
        queue::ActiveModel {
            id: Set(1),
            audio_source_ids: Set(serde_json::to_value(&state.audio_source_ids)?),
            playlist_item_ids: Set(Some(serde_json::to_value(&state.playlist_item_ids)?)),
            current_index: Set(state.current_index.map(i64::try_from).transpose()?),
            mode: Set(match state.mode {
                PlaybackMode::Stopped => "stopped",
                PlaybackMode::Paused => "paused",
                PlaybackMode::Playing => "playing",
            }
            .to_owned()),
            revision: Set(i64::try_from(state.revision)?),
            playback_token: Set(i64::try_from(state.playback_token)?),
        }
        .update(&self.connection)
        .await?;
        Ok(())
    }
}

fn validate(state: &QueueState) -> Result<()> {
    ensure!(
        state.audio_source_ids.len() == state.playlist_item_ids.len(),
        "Invalid playlist queue positions"
    );
    ensure!(
        if state.audio_source_ids.is_empty() {
            state.current_index.is_none() && state.mode == PlaybackMode::Stopped
        } else {
            state
                .current_index
                .is_some_and(|index| index <= state.audio_source_ids.len())
        },
        "Invalid persisted playback queue position"
    );
    ensure!(
        state.mode == PlaybackMode::Stopped
            || state
                .current_index
                .is_some_and(|index| index < state.audio_source_ids.len()),
        "Active playback requires a current queue item"
    );
    Ok(())
}
