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
    pub async fn revision(&self) -> Result<u64> {
        #[derive(FromQueryResult)]
        struct Revision {
            revision: i64,
        }
        let row = Revision::find_by_statement(Statement::from_string(
            self.connection.get_database_backend(),
            "SELECT revision FROM playback_queue WHERE id = 1",
        ))
        .one(&self.connection)
        .await?
        .context("Playback queue row is missing")?;
        Ok(u64::try_from(row.revision)?)
    }

    pub async fn pending_checks(
        &self,
        excluded: &[(QueueEntryId, QueueLaunchSource)],
        limit: usize,
    ) -> Result<Vec<(QueueEntryId, QueueLaunchSource)>> {
        #[derive(FromQueryResult)]
        struct Pending {
            id: i64,
            check_generation: i64,
            audio_source_id: i32,
            audio_kind: String,
            audio_location: String,
            position: i64,
            priority: i32,
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        let excluded = excluded
            .iter()
            .map(|(id, source)| {
                (
                    id.get(),
                    source.check_generation,
                    source.audio_source_id,
                    source.source.kind(),
                    source.source.location(),
                )
            })
            .collect::<std::collections::HashSet<_>>();
        let mut checks = Vec::new();
        let mut cursor = None;
        // Page candidates rather than binding the unbounded list of failed checks into SQL.
        loop {
            let page_size = 100;
            let after = cursor.map_or_else(String::new, |(priority, position)| format!(
                " AND (CASE WHEN q.waiting_for_check AND e.id = q.current_entry_id THEN 0 ELSE 1 END > {priority} OR (CASE WHEN q.waiting_for_check AND e.id = q.current_entry_id THEN 0 ELSE 1 END = {priority} AND e.position > {position}))"
            ));
            let sql = format!(
                "SELECT e.id, e.check_generation, e.audio_source_id, e.audio_kind, e.audio_location, e.position, CAST(CASE WHEN q.waiting_for_check AND e.id = q.current_entry_id THEN 0 ELSE 1 END AS INTEGER) AS priority FROM queue_entries e CROSS JOIN playback_queue q WHERE q.id = 1 AND e.availability = 'checking' AND e.audio_source_id IS NOT NULL AND e.audio_kind IS NOT NULL AND e.audio_location IS NOT NULL{after} ORDER BY priority, e.position LIMIT {page_size}"
            );
            let page = super::query_all::<Pending>(
                &self.connection,
                Statement::from_string(self.connection.get_database_backend(), sql),
            )
            .await?;
            let exhausted = page.len() < page_size;
            for row in page {
                cursor = Some((row.priority, row.position));
                let generation = u64::try_from(row.check_generation)?;
                if excluded.contains(&(
                    row.id,
                    generation,
                    row.audio_source_id,
                    row.audio_kind.as_str(),
                    row.audio_location.as_str(),
                )) {
                    continue;
                }
                checks.push((
                    QueueEntryId::new(row.id)?,
                    QueueLaunchSource {
                        audio_source_id: row.audio_source_id,
                        source: SourceType::from_parts(&row.audio_kind, row.audio_location)?,
                        check_generation: generation,
                    },
                ));
                if checks.len() == limit {
                    break;
                }
            }
            if exhausted || checks.len() == limit {
                break;
            }
        }
        Ok(checks)
    }
    /// One statement keeps list and singleton on one snapshot on either backend/isolation level.
    pub async fn get(&self) -> Result<QueueState> {
        let mut header = None;
        let mut entries = Vec::new();
        super::query_each::<QueueRow>(&self.connection, Statement::from_string(
            self.connection.get_database_backend(),
            "SELECT CASE WHEN e.position = 0 OR e.id IS NULL THEN q.id END AS q_id, q.current_entry_id AS q_current_entry_id, q.history_len AS q_history_len, q.last_entry_id AS q_last_entry_id, q.mode AS q_mode, q.waiting_for_check AS q_waiting_for_check, q.launch_audio_source_id AS q_launch_audio_source_id, q.launch_audio_kind AS q_launch_audio_kind, q.launch_audio_location AS q_launch_audio_location, q.launch_check_generation AS q_launch_check_generation, q.shuffle AS q_shuffle, q.repeat_mode AS q_repeat_mode, q.repeat_launch AS q_repeat_launch, q.revision AS q_revision, q.playback_token AS q_playback_token, e.* FROM playback_queue q LEFT JOIN queue_entries e ON TRUE WHERE q.id = 1 ORDER BY e.position",
        ), |result| {
            if let Some(row) = result.header { header = Some(row); }
            if let Some(entry) = result.entry { entries.push(entry_from_row(entry)?); }
            Ok(())
        }).await?;
        let row =
            header.context("Playback queue row is missing; apply database migrations first")?;
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
        // shortcut: replaces the full list per save; use row diffs for frequent large-queue edits.
        queue_entry::Entity::delete_many()
            .exec(&self.connection)
            .await?;
        for chunk in state.entries.chunks(100) {
            let rows = chunk.iter().map(entry_to_row).collect::<Result<Vec<_>>>()?;
            queue_entry::Entity::insert_many(rows)
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

struct QueueRow {
    header: Option<queue::Model>,
    entry: Option<queue_entry::Model>,
}
impl FromQueryResult for QueueRow {
    fn from_query_result(row: &sea_orm::QueryResult, _: &str) -> Result<Self, sea_orm::DbErr> {
        Ok(Self {
            header: if row.try_get::<Option<i32>>("q_", "id")?.is_some() {
                Some(queue::Model::from_query_result(row, "q_")?)
            } else {
                None
            },
            entry: if row.try_get::<Option<i64>>("", "id")?.is_some() {
                Some(queue_entry::Model::from_query_result(row, "")?)
            } else {
                None
            },
        })
    }
}
