use crate::{
    entities::{queue, queue_entry},
    model::{
        PlaybackMode, QueueAvailability, QueueEntry, QueueEntryId, QueueLaunchSource,
        QueuePlaylistSource, QueueState, RepeatMode, SourceType,
    },
};
use anyhow::{Context, Result, bail, ensure};
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, FromQueryResult, Set, Statement};

pub struct QueueRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}
impl QueueRepository<'_> {
    /// One statement keeps list and singleton on one snapshot on either backend/isolation level.
    pub async fn get(&self) -> Result<QueueState> {
        let rows = self.connection.query_all_raw(Statement::from_string(
            self.connection.get_database_backend(),
            "SELECT q.id AS q_id, q.current_entry_id AS q_current_entry_id, q.history_len AS q_history_len, q.last_entry_id AS q_last_entry_id, q.mode AS q_mode, q.waiting_for_check AS q_waiting_for_check, q.launch_audio_source_id AS q_launch_audio_source_id, q.launch_audio_kind AS q_launch_audio_kind, q.launch_audio_location AS q_launch_audio_location, q.launch_check_generation AS q_launch_check_generation, q.shuffle AS q_shuffle, q.repeat_mode AS q_repeat_mode, q.repeat_launch AS q_repeat_launch, q.revision AS q_revision, q.playback_token AS q_playback_token, e.* FROM playback_queue q LEFT JOIN queue_entries e ON TRUE WHERE q.id = 1 ORDER BY e.position",
        )).await?;
        let first = rows
            .first()
            .context("Playback queue row is missing; apply database migrations first")?;
        let row = queue::Model::from_query_result(first, "q_")?;
        let mut entries = Vec::new();
        for result in &rows {
            if result.try_get::<Option<i64>>("", "id")?.is_some() {
                entries.push(entry_from_row(queue_entry::Model::from_query_result(
                    result, "",
                )?)?);
            }
        }
        let state = QueueState {
            entries,
            current_entry_id: row.current_entry_id.map(QueueEntryId::new).transpose()?,
            history_len: usize::try_from(row.history_len)?,
            last_entry_id: row.last_entry_id,
            waiting_for_check: row.waiting_for_check,
            launch_source: match (
                row.launch_audio_source_id,
                row.launch_audio_kind,
                row.launch_audio_location,
                row.launch_check_generation,
            ) {
                (Some(audio_source_id), Some(kind), Some(location), Some(generation)) => {
                    Some(QueueLaunchSource {
                        audio_source_id,
                        source: SourceType::from_parts(&kind, location)?,
                        check_generation: u64::try_from(generation)?,
                    })
                }
                (None, None, None, None) => None,
                _ => bail!("Incomplete queue launch source"),
            },
            shuffle: row.shuffle,
            repeat: match row.repeat_mode.as_str() {
                "off" => RepeatMode::Off,
                "all" => RepeatMode::All,
                "one" => RepeatMode::One,
                _ => bail!("Invalid persisted repeat mode"),
            },
            repeat_launch: row.repeat_launch,
            mode: match row.mode.as_str() {
                "stopped" => PlaybackMode::Stopped,
                "paused" => PlaybackMode::Paused,
                "playing" => PlaybackMode::Playing,
                _ => bail!("Invalid persisted playback mode"),
            },
            revision: u64::try_from(row.revision)?,
            playback_token: u64::try_from(row.playback_token)?,
        };
        state.validate()?;
        Ok(state)
    }
    /// Transaction only. Caller holds the settings singleton writer lock until outer commit.
    pub async fn save(&self, state: &QueueState) -> Result<()> {
        ensure!(
            matches!(self.connection, sea_orm::DatabaseExecutor::Transaction(_)),
            "Queue save requires an outer transaction and singleton writer lock"
        );
        state.validate()?;
        let previous = self.get().await?;
        ensure!(
            state.last_entry_id >= previous.last_entry_id
                && state.revision >= previous.revision
                && state.playback_token >= previous.playback_token,
            "Queue counters cannot go backwards"
        );
        let existing: std::collections::HashSet<_> =
            previous.entries.iter().map(|e| e.id).collect();
        ensure!(
            state
                .entries
                .iter()
                .all(|e| e.id.get() > previous.last_entry_id || existing.contains(&e.id)),
            "Queue entry IDs cannot be reused"
        );
        let rows = state
            .entries
            .iter()
            .map(entry_to_row)
            .collect::<Result<Vec<_>>>()?;
        // shortcut: replaces the full list per save; use row diffs for frequent large-queue edits.
        queue_entry::Entity::delete_many()
            .exec(&self.connection)
            .await?;
        for chunk in rows.chunks(100) {
            queue_entry::Entity::insert_many(chunk.to_vec())
                .exec(&self.connection)
                .await?;
        }
        queue::ActiveModel {
            id: Set(1),
            waiting_for_check: Set(state.waiting_for_check),
            launch_audio_source_id: Set(state.launch_source.as_ref().map(|s| s.audio_source_id)),
            launch_audio_kind: Set(state
                .launch_source
                .as_ref()
                .map(|s| s.source.kind().to_owned())),
            launch_audio_location: Set(state
                .launch_source
                .as_ref()
                .map(|s| s.source.location().to_owned())),
            launch_check_generation: Set(state
                .launch_source
                .as_ref()
                .map(|s| i64::try_from(s.check_generation))
                .transpose()?),
            shuffle: Set(state.shuffle),
            repeat_mode: Set(match state.repeat {
                RepeatMode::Off => "off",
                RepeatMode::All => "all",
                RepeatMode::One => "one",
            }
            .into()),
            repeat_launch: Set(state.repeat_launch),
            current_entry_id: Set(state.current_entry_id.map(QueueEntryId::get)),
            history_len: Set(i64::try_from(state.history_len)?),
            last_entry_id: Set(state.last_entry_id),
            mode: Set(match state.mode {
                PlaybackMode::Stopped => "stopped",
                PlaybackMode::Paused => "paused",
                PlaybackMode::Playing => "playing",
            }
            .into()),
            revision: Set(i64::try_from(state.revision)?),
            playback_token: Set(i64::try_from(state.playback_token)?),
        }
        .update(&self.connection)
        .await?;
        Ok(())
    }
}
fn entry_from_row(row: queue_entry::Model) -> Result<QueueEntry> {
    let audio_source = match (row.audio_kind, row.audio_location) {
        (Some(kind), Some(location)) => Some(SourceType::from_parts(&kind, location)?),
        (None, None) => None,
        _ => bail!("Incomplete queue audio source key"),
    };
    let playlist_source = match (row.installation_kind, row.hash_kind, row.beatmap_hash) {
        (Some(installation_kind), Some(hash_kind), Some(beatmap_hash)) => {
            Some(QueuePlaylistSource {
                installation_kind,
                hash_kind,
                beatmap_hash,
            })
        }
        (None, None, None) => None,
        _ => bail!("Incomplete queue playlist source key"),
    };
    Ok(QueueEntry {
        id: QueueEntryId::new(row.id)?,
        position: usize::try_from(row.position)?,
        audio_source,
        playlist_source,
        audio_source_id: row.audio_source_id,
        playlist_item_id: row.playlist_item_id,
        migration_audio_source_id: row.migration_audio_source_id,
        migration_playlist_item_id: row.migration_playlist_item_id,
        title: row.title,
        title_unicode: row.title_unicode,
        artist: row.artist,
        artist_unicode: row.artist_unicode,
        difficulty_name: row.difficulty_name,
        beatmap_id: row.beatmap_id,
        beatmap_set_id: row.beatmap_set_id,
        cover_beatmap_id: row.cover_beatmap_id,
        availability: match row.availability.as_str() {
            "checking" => QueueAvailability::Checking,
            "available" => QueueAvailability::Available,
            "unavailable" => QueueAvailability::Unavailable,
            _ => bail!("Invalid queue availability"),
        },
        check_generation: u64::try_from(row.check_generation)?,
        shuffle_order: row.shuffle_order,
    })
}
fn entry_to_row(e: &QueueEntry) -> Result<queue_entry::ActiveModel> {
    Ok(queue_entry::ActiveModel {
        id: Set(e.id.get()),
        position: Set(i64::try_from(e.position)?),
        audio_kind: Set(e.audio_source.as_ref().map(|s| s.kind().to_owned())),
        audio_location: Set(e.audio_source.as_ref().map(|s| s.location().to_owned())),
        installation_kind: Set(e
            .playlist_source
            .as_ref()
            .map(|s| s.installation_kind.clone())),
        hash_kind: Set(e.playlist_source.as_ref().map(|s| s.hash_kind.clone())),
        beatmap_hash: Set(e.playlist_source.as_ref().map(|s| s.beatmap_hash.clone())),
        audio_source_id: Set(e.audio_source_id),
        playlist_item_id: Set(e.playlist_item_id),
        migration_audio_source_id: Set(e.migration_audio_source_id),
        migration_playlist_item_id: Set(e.migration_playlist_item_id),
        title: Set(e.title.clone()),
        title_unicode: Set(e.title_unicode.clone()),
        artist: Set(e.artist.clone()),
        artist_unicode: Set(e.artist_unicode.clone()),
        difficulty_name: Set(e.difficulty_name.clone()),
        beatmap_id: Set(e.beatmap_id),
        beatmap_set_id: Set(e.beatmap_set_id),
        cover_beatmap_id: Set(e.cover_beatmap_id),
        availability: Set(match e.availability {
            QueueAvailability::Checking => "checking",
            QueueAvailability::Available => "available",
            QueueAvailability::Unavailable => "unavailable",
        }
        .into()),
        check_generation: Set(i64::try_from(e.check_generation)?),
        shuffle_order: Set(e.shuffle_order),
    })
}
