use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    Value, sea_query::OnConflict,
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
        Ok(playlist::Entity::find()
            .order_by_asc(playlist::Column::Id)
            .all(&self.connection)
            .await?
            .into_iter()
            .map(summary)
            .collect())
    }

    pub async fn get(&self, id: i32) -> Result<Option<Playlist>> {
        let Some(row) = playlist::Entity::find_by_id(id)
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
            name: row.name,
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
        Ok(summary(
            playlist::ActiveModel {
                name: Set(name.to_owned()),
                ..Default::default()
            }
            .insert(&self.connection)
            .await?,
        ))
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

fn summary(row: playlist::Model) -> PlaylistSummary {
    PlaylistSummary {
        id: row.id,
        name: row.name,
    }
}
