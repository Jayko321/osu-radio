use std::collections::BTreeSet;

use sea_orm::{EntityTrait, IntoActiveModel};
use sea_orm_migration::prelude::*;
use sha2::{Digest, Sha256};

#[path = "m20260919_000004_tags_legacy.rs"]
mod legacy;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_tables(manager).await?;
        let connection = manager.get_connection();
        let rows = legacy::Entity::find().all(connection).await?;
        let mut new_hashes = BTreeSet::new();
        // Keep the old rows and references until all tags have been extracted.
        for row in &rows {
            transfer_tags(connection, row).await?;
        }
        for row in &rows {
            let mut expected = row.clone();
            expected.hash = metadata_hash_v2(row)?;
            expected.tags = None;
            expected.user_tags = serde_json::json!([]);
            legacy::Entity::insert(expected.clone().into_active_model())
                .on_conflict(
                    OnConflict::column(legacy::Column::Hash)
                        .do_nothing()
                        .to_owned(),
                )
                .exec_without_returning(connection)
                .await?;
            let mut actual = legacy::Entity::find_by_id(&expected.hash)
                .one(connection)
                .await?
                .ok_or_else(|| DbErr::Migration("metadata missing after insertion".into()))?;
            // A key can already exist. Compare all retained content, never overwrite it.
            actual.tags = None;
            actual.user_tags = serde_json::json!([]);
            if actual != expected {
                return Err(DbErr::Migration(format!(
                    "metadata hash collision or corrupted content for {}",
                    expected.hash
                )));
            }
            connection
                .execute(
                    Query::update()
                        .table(Alias::new("beatmaps"))
                        .value(Alias::new("metadata_hash"), expected.hash.clone())
                        .and_where(Expr::col(Alias::new("metadata_hash")).eq(&row.hash)),
                )
                .await?;
            new_hashes.insert(expected.hash);
        }
        for row in rows {
            if !new_hashes.contains(&row.hash) {
                legacy::Entity::delete_by_id(row.hash)
                    .exec(connection)
                    .await?;
            }
        }
        // Separate statements are supported by both backends, including SQLite.
        for name in ["tags", "user_tags"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("beatmap_metadata"))
                        .drop_column(Alias::new(name))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Normalized set tags cannot restore the original metadata tag distribution".into(),
        ))
    }
}

async fn create_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    // Explicit collations also make uniqueness independent of the database locale.
    #[cfg(feature = "sqlite")]
    let tags = "CREATE TABLE tags (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT COLLATE BINARY NOT NULL UNIQUE)";
    #[cfg(feature = "postgres")]
    let tags = "CREATE TABLE tags (id SERIAL PRIMARY KEY, name TEXT COLLATE \"C\" NOT NULL UNIQUE)";
    let connection = manager.get_connection();
    connection.execute_unprepared(tags).await?;
    connection.execute_unprepared(
        "CREATE TABLE beatmap_set_tags (beatmap_set_id INTEGER NOT NULL REFERENCES beatmap_sets(id) ON DELETE CASCADE, tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE RESTRICT, PRIMARY KEY (beatmap_set_id, tag_id))"
    ).await?;
    connection
        .execute_unprepared("CREATE INDEX idx_beatmap_set_tags_tag_id ON beatmap_set_tags(tag_id)")
        .await?;
    Ok(())
}

async fn transfer_tags(
    connection: &SchemaManagerConnection<'_>,
    row: &legacy::Model,
) -> Result<(), DbErr> {
    let user_tags: Vec<String> =
        serde_json::from_value(row.user_tags.clone()).map_err(|error| {
            DbErr::Migration(format!("invalid user tags for {}: {error}", row.hash))
        })?;
    let names: BTreeSet<_> = row
        .tags
        .as_deref()
        .unwrap_or_default()
        .split_whitespace()
        .chain(user_tags.iter().map(String::as_str))
        .map(|name| name.trim().to_lowercase())
        .filter(|name| !name.is_empty())
        .collect();
    let sets = Query::select()
        .distinct()
        .column(Alias::new("beatmap_set_id"))
        .from(Alias::new("beatmaps"))
        .and_where(Expr::col(Alias::new("metadata_hash")).eq(&row.hash))
        .to_owned();
    let set_rows = connection.query_all(&sets).await?;
    for set in set_rows {
        let set_id: i32 = set.try_get("", "beatmap_set_id")?;
        for name in &names {
            connection
                .execute(
                    Query::insert()
                        .into_table(Alias::new("tags"))
                        .columns([Alias::new("name")])
                        .values_panic([name.clone().into()])
                        .on_conflict(
                            OnConflict::column(Alias::new("name"))
                                .do_nothing()
                                .to_owned(),
                        ),
                )
                .await?;
            let tag = connection
                .query_one(
                    Query::select()
                        .column(Alias::new("id"))
                        .from(Alias::new("tags"))
                        .and_where(Expr::col(Alias::new("name")).eq(name)),
                )
                .await?
                .ok_or_else(|| DbErr::Migration("tag missing after insertion".into()))?;
            let tag_id: i32 = tag.try_get("", "id")?;
            connection
                .execute(
                    Query::insert()
                        .into_table(Alias::new("beatmap_set_tags"))
                        .columns([Alias::new("beatmap_set_id"), Alias::new("tag_id")])
                        .values_panic([set_id.into(), tag_id.into()])
                        .on_conflict(
                            OnConflict::columns([
                                Alias::new("beatmap_set_id"),
                                Alias::new("tag_id"),
                            ])
                            .do_nothing()
                            .to_owned(),
                        ),
                )
                .await?;
        }
    }
    Ok(())
}

// Frozen v2 identity: do not replace this with a future runtime hash implementation.
fn metadata_hash_v2(row: &legacy::Model) -> Result<String, DbErr> {
    let encode = || -> Result<Vec<u8>, serde_json::Error> {
        let author = row
            .author
            .as_ref()
            .map(|value| {
                Ok::<_, serde_json::Error>((
                    serde_json::from_value::<Option<i32>>(value["online_id"].clone())?,
                    serde_json::from_value::<Option<String>>(value["username"].clone())?,
                    serde_json::from_value::<Option<String>>(value["country_code"].clone())?,
                ))
            })
            .transpose()?;
        serde_json::to_vec(&(
            "radio-db:metadata:v2",
            &row.title,
            &row.title_unicode,
            &row.artist,
            &row.artist_unicode,
            author,
            &row.source,
            row.preview_time,
            &row.audio_file,
            &row.background_file,
        ))
    };
    let bytes = encode().map_err(|error| DbErr::Migration(error.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
