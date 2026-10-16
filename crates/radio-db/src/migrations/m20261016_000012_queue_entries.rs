// Frozen v11/v12 definitions. Do not use runtime entities or queue models here.
use sea_orm::{ConnectionTrait, FromQueryResult, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;
#[derive(Debug, Clone, PartialEq, Eq, FromQueryResult)]
struct Entry {
    id: i64,
    position: i64,
    audio_kind: Option<String>,
    audio_location: Option<String>,
    installation_kind: Option<String>,
    hash_kind: Option<String>,
    beatmap_hash: Option<String>,
    audio_source_id: Option<i32>,
    playlist_item_id: Option<i32>,
    migration_audio_source_id: Option<i32>,
    migration_playlist_item_id: Option<i32>,
    title: Option<String>,
    title_unicode: Option<String>,
    artist: Option<String>,
    artist_unicode: Option<String>,
    difficulty_name: Option<String>,
    beatmap_id: Option<i32>,
    beatmap_set_id: Option<i32>,
    cover_beatmap_id: Option<i32>,
    availability: String,
    check_generation: i64,
}
#[derive(FromQueryResult)]
struct Legacy {
    audio_source_ids: serde_json::Value,
    playlist_item_ids: Option<serde_json::Value>,
    current_index: Option<i64>,
    mode: String,
    revision: i64,
    playback_token: i64,
}
#[derive(FromQueryResult)]
struct State {
    current_entry_id: Option<i64>,
    history_len: i64,
    last_entry_id: i64,
    mode: String,
    revision: i64,
    playback_token: i64,
}
#[derive(FromQueryResult)]
struct Audio {
    id: i32,
    kind: String,
    location: String,
}
#[derive(FromQueryResult)]
struct Caption {
    title: Option<String>,
    title_unicode: Option<String>,
    artist: Option<String>,
    artist_unicode: Option<String>,
    difficulty_name: Option<String>,
}
#[derive(FromQueryResult)]
struct Item {
    id: i32,
    source_kind: String,
    hash_kind: String,
    beatmap_hash: String,
}
#[derive(FromQueryResult)]
struct Map {
    id: i32,
    beatmap_set_id: i32,
}
const STATE_DDL: &str = "CREATE TABLE playback_queue_v12 (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    current_entry_id BIGINT CHECK (current_entry_id > 0),
    history_len BIGINT NOT NULL CHECK (history_len >= 0),
    last_entry_id BIGINT NOT NULL CHECK (last_entry_id >= 0),
    mode TEXT NOT NULL CHECK (mode IN ('stopped','paused','playing')),
    revision BIGINT NOT NULL CHECK (revision >= 0),
    playback_token BIGINT NOT NULL CHECK (playback_token >= 0),
    CHECK (mode = 'stopped' OR current_entry_id IS NOT NULL)
)";
const ENTRY_DDL: &str = "CREATE TABLE queue_entries (
    id BIGINT NOT NULL PRIMARY KEY CHECK (id > 0),
    position BIGINT NOT NULL UNIQUE CHECK (position >= 0),
    audio_kind TEXT,
    audio_location TEXT,
    installation_kind TEXT,
    hash_kind TEXT,
    beatmap_hash TEXT,
    audio_source_id INTEGER,
    playlist_item_id INTEGER,
    migration_audio_source_id INTEGER,
    migration_playlist_item_id INTEGER,
    title TEXT,
    title_unicode TEXT,
    artist TEXT,
    artist_unicode TEXT,
    difficulty_name TEXT,
    beatmap_id INTEGER,
    beatmap_set_id INTEGER,
    cover_beatmap_id INTEGER,
    availability TEXT NOT NULL CHECK (availability IN ('checking','available','unavailable')),
    check_generation BIGINT NOT NULL CHECK (check_generation >= 0),
    CHECK ((audio_kind IS NULL AND audio_location IS NULL) OR
           (audio_kind IS NOT NULL AND audio_kind IN ('local','copied','online') AND audio_location IS NOT NULL)),
    CHECK ((installation_kind IS NULL AND hash_kind IS NULL AND beatmap_hash IS NULL) OR
           (installation_kind IS NOT NULL AND hash_kind IS NOT NULL AND installation_kind IN ('stable','lazer') AND hash_kind IN ('source','md5') AND beatmap_hash IS NOT NULL))
)";
fn invalid(message: impl std::fmt::Display) -> DbErr {
    DbErr::Custom(message.to_string())
}
fn guard(valid: bool) -> Result<(), DbErr> {
    if valid {
        Ok(())
    } else {
        Err(invalid("Queue v12 cannot be downgraded without data loss"))
    }
}
async fn one<T: FromQueryResult>(
    db: &SchemaManagerConnection<'_>,
    sql: &str,
    values: Vec<Value>,
) -> Result<Option<T>, DbErr> {
    db.query_one_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        sql,
        values,
    ))
    .await?
    .map(|row| T::from_query_result(&row, ""))
    .transpose()
}
async fn snapshot(
    db: &SchemaManagerConnection<'_>,
    audio_id: i32,
    item_id: Option<i32>,
    position: i64,
) -> Result<Entry, DbErr> {
    let audio = one::<Audio>(
        db,
        "SELECT id,kind,location FROM audio_sources WHERE id = $1",
        vec![audio_id.into()],
    )
    .await?;
    let item = if let Some(id) = item_id {
        one::<Item>(
            db,
            "SELECT id,source_kind,hash_kind,beatmap_hash FROM playlist_items WHERE id = $1",
            vec![id.into()],
        )
        .await?
    } else {
        None
    };
    let library_sql = "SELECT m.title,m.title_unicode,m.artist,m.artist_unicode,b.difficulty_name FROM beatmaps b LEFT JOIN beatmap_metadata m ON m.hash = b.metadata_hash WHERE b.audio_source_id = $1 ORDER BY CASE WHEN b.metadata_hash IS NULL THEN 1 ELSE 0 END, b.id LIMIT 1";
    let library = one::<Caption>(db, library_sql, vec![audio_id.into()]).await?;
    let caption = if let Some(item) = &item {
        one::<Caption>(db, "SELECT title,title_unicode,artist,artist_unicode,difficulty_name FROM playlist_items WHERE id = $1", vec![item.id.into()]).await?
    } else {
        None
    };
    let map = if let Some(item) = &item {
        one::<Map>(db, "SELECT b.id,b.beatmap_set_id FROM beatmaps b JOIN beatmap_sets s ON s.id = b.beatmap_set_id JOIN osu_installations i ON i.id = s.installation_id WHERE i.kind = $1 AND (($2 = 'source' AND b.hash = $3) OR ($2 = 'md5' AND b.md5_hash = $3)) AND b.audio_source_id = $4 ORDER BY b.id LIMIT 1", vec![item.source_kind.clone().into(), item.hash_kind.clone().into(), item.beatmap_hash.clone().into(), audio_id.into()]).await?
    } else {
        one::<Map>(
            db,
            "SELECT id,beatmap_set_id FROM beatmaps WHERE audio_source_id = $1 ORDER BY id LIMIT 1",
            vec![audio_id.into()],
        )
        .await?
    };
    let cover = one::<Map>(db, "SELECT id,beatmap_set_id FROM beatmaps WHERE audio_source_id = $1 AND background_path IS NOT NULL ORDER BY id LIMIT 1", vec![audio_id.into()]).await?;
    Ok(Entry {
        id: position
            .checked_add(1)
            .ok_or_else(|| invalid("Queue IDs exhausted"))?,
        position,
        audio_kind: audio.as_ref().map(|a| a.kind.clone()),
        audio_location: audio.as_ref().map(|a| a.location.clone()),
        installation_kind: item.as_ref().map(|i| i.source_kind.clone()),
        hash_kind: item.as_ref().map(|i| i.hash_kind.clone()),
        beatmap_hash: item.as_ref().map(|i| i.beatmap_hash.clone()),
        audio_source_id: audio.as_ref().map(|a| a.id),
        playlist_item_id: item.as_ref().map(|i| i.id),
        migration_audio_source_id: audio.is_none().then_some(audio_id),
        migration_playlist_item_id: item.is_none().then_some(item_id).flatten(),
        title: caption
            .as_ref()
            .and_then(|c| c.title.clone())
            .or_else(|| library.as_ref().and_then(|c| c.title.clone()))
            .or_else(|| Some("Unavailable track".into())),
        title_unicode: caption
            .as_ref()
            .and_then(|c| c.title_unicode.clone())
            .or_else(|| library.as_ref().and_then(|c| c.title_unicode.clone())),
        artist: caption
            .as_ref()
            .and_then(|c| c.artist.clone())
            .or_else(|| library.as_ref().and_then(|c| c.artist.clone())),
        artist_unicode: caption
            .as_ref()
            .and_then(|c| c.artist_unicode.clone())
            .or_else(|| library.as_ref().and_then(|c| c.artist_unicode.clone())),
        difficulty_name: caption
            .as_ref()
            .and_then(|c| c.difficulty_name.clone())
            .or_else(|| library.as_ref().and_then(|c| c.difficulty_name.clone())),
        beatmap_id: map.as_ref().map(|m| m.id),
        beatmap_set_id: map.as_ref().map(|m| m.beatmap_set_id),
        cover_beatmap_id: cover.map(|m| m.id),
        availability: if audio
            .as_ref()
            .is_some_and(|a| a.kind == "local" || a.kind == "copied")
        {
            "checking"
        } else {
            "unavailable"
        }
        .into(),
        check_generation: 0,
    })
}
async fn insert(db: &SchemaManagerConnection<'_>, entry: Entry) -> Result<(), DbErr> {
    let statement = Query::insert()
        .into_table(Alias::new("queue_entries"))
        .columns(
            [
                "id",
                "position",
                "audio_kind",
                "audio_location",
                "installation_kind",
                "hash_kind",
                "beatmap_hash",
                "audio_source_id",
                "playlist_item_id",
                "migration_audio_source_id",
                "migration_playlist_item_id",
                "title",
                "title_unicode",
                "artist",
                "artist_unicode",
                "difficulty_name",
                "beatmap_id",
                "beatmap_set_id",
                "cover_beatmap_id",
                "availability",
                "check_generation",
            ]
            .into_iter()
            .map(Alias::new),
        )
        .values_panic([
            entry.id.into(),
            entry.position.into(),
            entry.audio_kind.into(),
            entry.audio_location.into(),
            entry.installation_kind.into(),
            entry.hash_kind.into(),
            entry.beatmap_hash.into(),
            entry.audio_source_id.into(),
            entry.playlist_item_id.into(),
            entry.migration_audio_source_id.into(),
            entry.migration_playlist_item_id.into(),
            entry.title.into(),
            entry.title_unicode.into(),
            entry.artist.into(),
            entry.artist_unicode.into(),
            entry.difficulty_name.into(),
            entry.beatmap_id.into(),
            entry.beatmap_set_id.into(),
            entry.cover_beatmap_id.into(),
            entry.availability.into(),
            entry.check_generation.into(),
        ])
        .to_owned();
    db.execute(&statement).await?;
    Ok(())
}
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let old = one::<Legacy>(db, "SELECT audio_source_ids,playlist_item_ids,current_index,mode,revision,playback_token FROM playback_queue WHERE id = 1", vec![]).await?.ok_or_else(|| invalid("Queue singleton missing"))?;
        let ids: Vec<i32> = serde_json::from_value(old.audio_source_ids).map_err(invalid)?;
        let items: Vec<Option<i32>> = old
            .playlist_item_ids
            .map(serde_json::from_value)
            .transpose()
            .map_err(invalid)?
            .unwrap_or_else(|| vec![None; ids.len()]);
        let len = <i64 as TryFrom<usize>>::try_from(ids.len()).map_err(invalid)?;
        let index = old.current_index;
        if ids.len() != items.len()
            || old.revision < 0
            || old.playback_token < 0
            || !matches!(old.mode.as_str(), "stopped" | "paused" | "playing")
            || !(if len == 0 {
                index.is_none() && old.mode == "stopped"
            } else {
                index.is_some_and(|i| i >= 0 && i <= len)
            })
            || (old.mode != "stopped" && index.is_none_or(|i| i >= len))
        {
            return Err(invalid("Invalid v11 playback queue"));
        }
        db.execute_unprepared(ENTRY_DDL).await?;
        for (position, (id, item)) in ids.into_iter().zip(items).enumerate() {
            insert(
                db,
                snapshot(
                    db,
                    id,
                    item,
                    <i64 as TryFrom<usize>>::try_from(position).map_err(invalid)?,
                )
                .await?,
            )
            .await?;
        }
        db.execute_unprepared(STATE_DDL).await?;
        let current = index
            .filter(|i| *i < len)
            .map(|i| {
                i.checked_add(1)
                    .ok_or_else(|| invalid("Queue IDs exhausted"))
            })
            .transpose()?;
        db.execute_raw(Statement::from_sql_and_values(db.get_database_backend(),
            "INSERT INTO playback_queue_v12 (id,current_entry_id,history_len,last_entry_id,mode,revision,playback_token) VALUES (1,$1,$2,$3,$4,$5,$6)",
            [current.into(), index.unwrap_or(0).into(), len.into(), old.mode.into(), old.revision.into(), old.playback_token.into()])).await?;
        db.execute_unprepared("DROP TABLE playback_queue").await?;
        db.execute_unprepared("ALTER TABLE playback_queue_v12 RENAME TO playback_queue")
            .await?;
        Ok(())
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let state = one::<State>(db, "SELECT current_entry_id,history_len,last_entry_id,mode,revision,playback_token FROM playback_queue WHERE id = 1", vec![]).await?.ok_or_else(|| invalid("Queue singleton missing"))?;
        let rows = db
            .query_all_raw(Statement::from_string(
                db.get_database_backend(),
                "SELECT * FROM queue_entries ORDER BY position",
            ))
            .await?;
        let entries = rows
            .iter()
            .map(|r| Entry::from_query_result(r, ""))
            .collect::<Result<Vec<_>, _>>()?;
        let len = <i64 as TryFrom<usize>>::try_from(entries.len()).map_err(invalid)?;
        guard(state.last_entry_id == len && state.revision >= 0 && state.playback_token >= 0)?;
        let index = if let Some(id) = state.current_entry_id {
            let position = id
                .checked_sub(1)
                .ok_or_else(|| invalid("Invalid queue ID"))?;
            guard(id > 0 && id <= len && state.history_len == position)?;
            Some(position)
        } else {
            guard(state.mode == "stopped" && state.history_len == len)?;
            (len > 0).then_some(len)
        };
        let mut ids = Vec::new();
        let mut items = Vec::new();
        for (position, entry) in entries.iter().enumerate() {
            let id = entry
                .audio_source_id
                .or(entry.migration_audio_source_id)
                .ok_or_else(|| invalid("Queue source cannot be restored to v11"))?;
            let item = entry.playlist_item_id.or(entry.migration_playlist_item_id);
            guard(
                *entry
                    == snapshot(
                        db,
                        id,
                        item,
                        <i64 as TryFrom<usize>>::try_from(position).map_err(invalid)?,
                    )
                    .await?,
            )?;
            ids.push(id);
            items.push(item);
        }
        // All losslessness checks precede the first schema mutation.
        db.execute_unprepared("CREATE TABLE playback_queue_v11 (
            id INTEGER PRIMARY KEY CHECK (id = 1), audio_source_ids JSON NOT NULL,
            playlist_item_ids JSON, current_index BIGINT CHECK (current_index >= 0),
            mode TEXT NOT NULL CHECK (mode IN ('stopped','paused','playing')),
            revision BIGINT NOT NULL CHECK (revision >= 0), playback_token BIGINT NOT NULL CHECK (playback_token >= 0))").await?;
        let statement = Query::insert()
            .into_table(Alias::new("playback_queue_v11"))
            .columns(
                [
                    "id",
                    "audio_source_ids",
                    "playlist_item_ids",
                    "current_index",
                    "mode",
                    "revision",
                    "playback_token",
                ]
                .map(Alias::new),
            )
            .values_panic([
                1.into(),
                serde_json::to_value(ids).map_err(invalid)?.into(),
                serde_json::to_value(items).map_err(invalid)?.into(),
                index.into(),
                state.mode.into(),
                state.revision.into(),
                state.playback_token.into(),
            ])
            .to_owned();
        db.execute(&statement).await?;
        db.execute_unprepared("DROP TABLE queue_entries").await?;
        db.execute_unprepared("DROP TABLE playback_queue").await?;
        db.execute_unprepared("ALTER TABLE playback_queue_v11 RENAME TO playback_queue")
            .await?;
        Ok(())
    }
}
