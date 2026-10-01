use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set, Value,
    sea_query::{Expr, ExprTrait, OnConflict},
};

use crate::{
    entities::{playlist, playlist_item},
    model::{Playlist, PlaylistBeatmap, PlaylistItem, PlaylistSummary},
};

pub struct PlaylistRepository<'a> {
    pub(crate) connection: sea_orm::DatabaseExecutor<'a>,
}

impl PlaylistRepository<'_> {
    pub async fn all(&self) -> Result<Vec<PlaylistSummary>> {
        self.summaries(None).await
    }

    pub async fn summary(&self, id: i32) -> Result<Option<PlaylistSummary>> {
        Ok(self.summaries(Some(id)).await?.into_iter().next())
    }

    // One statement covers every playlist; neither item labels nor PNG bytes are loaded.
    async fn summaries(&self, id: Option<i32>) -> Result<Vec<PlaylistSummary>> {
        let filter = if id.is_some() {
            #[cfg(feature = "postgres")]
            {
                "WHERE p.id = $1"
            }
            #[cfg(feature = "sqlite")]
            {
                "WHERE p.id = ?"
            }
        } else {
            ""
        };
        let statement = sea_orm::Statement::from_sql_and_values(
            self.connection.get_database_backend(),
            format!(
                "SELECT p.id, p.name, \
                (SELECT COUNT(*) FROM playlist_items item WHERE item.playlist_id = p.id) AS item_count, \
                (SELECT MIN(c.id) FROM beatmaps c WHERE c.audio_source_id = b.audio_source_id \
                AND c.background_path IS NOT NULL) AS cover_beatmap_id, \
                CASE WHEN p.custom_cover_png IS NULL THEN NULL ELSE p.custom_cover_revision END AS custom_cover_revision \
                FROM playlists p \
                LEFT JOIN playlist_items first_item ON first_item.id = \
                    (SELECT MIN(item.id) FROM playlist_items item WHERE item.playlist_id = p.id) \
                LEFT JOIN beatmaps b ON b.id = \
                    (SELECT MIN(candidate.id) FROM beatmaps candidate \
                    JOIN beatmap_sets s ON s.id = candidate.beatmap_set_id \
                    JOIN osu_installations i ON i.id = s.installation_id \
                    JOIN audio_sources a ON a.id = candidate.audio_source_id \
                    WHERE candidate.hash = first_item.beatmap_hash AND i.kind = first_item.source_kind \
                    AND a.kind IN ('local', 'copied')) \
                {filter} ORDER BY p.id"
            ),
            id.into_iter().map(Value::from),
        );
        self.connection
            .query_all_raw(statement)
            .await?
            .into_iter()
            .map(|row| {
                let count: i64 = row.try_get("", "item_count")?;
                Ok(PlaylistSummary {
                    id: row.try_get("", "id")?,
                    name: row.try_get("", "name")?,
                    item_count: u64::try_from(count)?,
                    cover_beatmap_id: row.try_get("", "cover_beatmap_id")?,
                    custom_cover_revision: row.try_get("", "custom_cover_revision")?,
                })
            })
            .collect()
    }

    pub async fn get(&self, id: i32) -> Result<Option<Playlist>> {
        let Some(row) = playlist::Entity::find_by_id(id)
            .select_only()
            .column(playlist::Column::Name)
            .into_tuple::<String>()
            .one(&self.connection)
            .await?
        else {
            return Ok(None);
        };
        let rows = playlist_item::Entity::find()
            .filter(playlist_item::Column::PlaylistId.eq(id))
            .order_by_asc(playlist_item::Column::Id)
            .all(&self.connection)
            .await?;
        Ok(Some(Playlist {
            id,
            name: row,
            items: self.resolve(rows).await?,
        }))
    }

    pub async fn item(&self, id: i32) -> Result<Option<PlaylistItem>> {
        let rows = playlist_item::Entity::find_by_id(id)
            .one(&self.connection)
            .await?
            .into_iter()
            .collect();
        Ok(self.resolve(rows).await?.into_iter().next())
    }

    async fn resolve(&self, rows: Vec<playlist_item::Model>) -> Result<Vec<PlaylistItem>> {
        let hashes: HashSet<_> = rows.iter().map(|item| item.beatmap_hash.clone()).collect();
        let mut candidates = self
            .library_rows("b.hash", hashes.into_iter().map(Value::from).collect())
            .await?;
        candidates.sort_by_key(|map| (map.audio_source_id.is_none(), map.beatmap_id));
        let mut resolved = HashMap::new();
        for map in candidates {
            if map.audio_source_id.is_some()
                && let Some(hash) = &map.beatmap_hash
            {
                resolved
                    .entry((map.source_kind.clone(), hash.clone()))
                    .or_insert(map);
            }
        }
        let items = rows
            .into_iter()
            .map(|item| {
                let map = resolved.get(&(item.source_kind.clone(), item.beatmap_hash.clone()));
                PlaylistItem {
                    last_played_at_ms: None,
                    volume_percent: None,
                    id: item.id,
                    playlist_id: item.playlist_id,
                    source_kind: item.source_kind,
                    beatmap_hash: item.beatmap_hash,
                    title: item.title,
                    artist: item.artist,
                    difficulty_name: item.difficulty_name,
                    beatmap_id: map.map(|map| map.beatmap_id),
                    beatmap_set_id: map.map(|map| map.beatmap_set_id),
                    audio_source_id: map.and_then(|map| map.audio_source_id),
                    cover_beatmap_id: map.and_then(|map| map.cover_beatmap_id),
                }
            })
            .collect();
        Ok(items)
    }

    pub async fn create(&self, name: &str) -> Result<PlaylistSummary> {
        let row = playlist::Entity::insert(playlist::ActiveModel {
            name: Set(name.to_owned()),
            ..Default::default()
        })
        .exec(&self.connection)
        .await?;
        Ok(PlaylistSummary {
            id: row.last_insert_id,
            name: name.to_owned(),
            item_count: 0,
            cover_beatmap_id: None,
            custom_cover_revision: None,
        })
    }

    pub async fn cover(&self, id: i32) -> Result<Option<Vec<u8>>> {
        Ok(playlist::Entity::find_by_id(id)
            .select_only()
            .column(playlist::Column::CustomCoverPng)
            .into_tuple::<Option<Vec<u8>>>()
            .one(&self.connection)
            .await?
            .flatten())
    }

    pub async fn set_cover(&self, id: i32, png: &[u8]) -> Result<bool> {
        Ok(playlist::Entity::update_many()
            .col_expr(playlist::Column::CustomCoverPng, Expr::value(png.to_vec()))
            .col_expr(
                playlist::Column::CustomCoverRevision,
                Expr::col(playlist::Column::CustomCoverRevision).add(1),
            )
            .filter(playlist::Column::Id.eq(id))
            .exec(&self.connection)
            .await?
            .rows_affected
            > 0)
    }

    pub async fn clear_cover(&self, id: i32) -> Result<bool> {
        Ok(playlist::Entity::update_many()
            .col_expr(
                playlist::Column::CustomCoverPng,
                Expr::value(Option::<Vec<u8>>::None),
            )
            .filter(playlist::Column::Id.eq(id))
            .exec(&self.connection)
            .await?
            .rows_affected
            > 0)
    }

    pub async fn rename(&self, id: i32, name: &str) -> Result<bool> {
        let result = playlist::Entity::update_many()
            .col_expr(
                playlist::Column::Name,
                sea_orm::sea_query::Expr::value(name),
            )
            .filter(playlist::Column::Id.eq(id))
            .exec(&self.connection)
            .await?;
        Ok(result.rows_affected > 0)
    }

    pub async fn delete(&self, id: i32) -> Result<bool> {
        Ok(playlist::Entity::delete_by_id(id)
            .exec(&self.connection)
            .await?
            .rows_affected
            > 0)
    }

    pub async fn remove_item(&self, id: i32, item_id: i32) -> Result<bool> {
        Ok(playlist_item::Entity::delete_many()
            .filter(playlist_item::Column::PlaylistId.eq(id))
            .filter(playlist_item::Column::Id.eq(item_id))
            .exec(&self.connection)
            .await?
            .rows_affected
            > 0)
    }

    pub async fn beatmaps(&self, ids: &[i32]) -> Result<Vec<PlaylistBeatmap>> {
        self.library_rows("b.id", ids.iter().copied().map(Value::from).collect())
            .await
    }

    /// Caller validates every input before inserting, under the singleton writer lock.
    pub async fn add(&self, id: i32, map: &PlaylistBeatmap) -> Result<()> {
        playlist_item::Entity::insert(playlist_item::ActiveModel {
            playlist_id: Set(id),
            source_kind: Set(map.source_kind.clone()),
            beatmap_hash: Set(map
                .beatmap_hash
                .clone()
                .context("Beatmap has no source hash")?),
            title: Set(map.title.clone()),
            artist: Set(map.artist.clone()),
            difficulty_name: Set(map.difficulty_name.clone()),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::columns([
                playlist_item::Column::PlaylistId,
                playlist_item::Column::SourceKind,
                playlist_item::Column::BeatmapHash,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&self.connection)
        .await?;
        Ok(())
    }

    async fn library_rows(&self, column: &str, values: Vec<Value>) -> Result<Vec<PlaylistBeatmap>> {
        let mut rows = Vec::new();
        for chunk in values.chunks(500) {
            let placeholders = (1..=chunk.len())
                .map(|index| {
                    #[cfg(feature = "postgres")]
                    {
                        format!("${index}")
                    }
                    #[cfg(feature = "sqlite")]
                    {
                        let _ = index;
                        "?".to_owned()
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            let statement = sea_orm::Statement::from_sql_and_values(
                self.connection.get_database_backend(),
                format!(
                    "SELECT b.id AS beatmap_id, b.beatmap_set_id, i.kind AS source_kind, b.hash AS beatmap_hash, \
                    COALESCE(NULLIF(m.title, ''), m.title_unicode) AS title, \
                    COALESCE(NULLIF(m.artist, ''), m.artist_unicode) AS artist, b.difficulty_name, \
                    CASE WHEN a.kind IN ('local', 'copied') THEN a.id ELSE NULL END AS audio_source_id, \
                    (SELECT MIN(c.id) FROM beatmaps c WHERE c.audio_source_id = a.id \
                    AND c.background_path IS NOT NULL) AS cover_beatmap_id \
                    FROM beatmaps b JOIN beatmap_sets s ON s.id = b.beatmap_set_id \
                    JOIN osu_installations i ON i.id = s.installation_id \
                    LEFT JOIN beatmap_metadata m ON m.hash = b.metadata_hash \
                    LEFT JOIN audio_sources a ON a.id = b.audio_source_id WHERE {column} IN ({placeholders})"
                ),
                chunk.to_vec(),
            );
            for row in self.connection.query_all_raw(statement).await? {
                rows.push(PlaylistBeatmap {
                    beatmap_id: row.try_get("", "beatmap_id")?,
                    beatmap_set_id: row.try_get("", "beatmap_set_id")?,
                    source_kind: row.try_get("", "source_kind")?,
                    beatmap_hash: row.try_get("", "beatmap_hash")?,
                    title: row.try_get("", "title")?,
                    artist: row.try_get("", "artist")?,
                    difficulty_name: row.try_get("", "difficulty_name")?,
                    audio_source_id: row.try_get("", "audio_source_id")?,
                    cover_beatmap_id: row.try_get("", "cover_beatmap_id")?,
                });
            }
        }
        Ok(rows)
    }
}
