use sea_orm::{FromQueryResult, Order};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        for (table, column) in COLUMNS {
            connection
                .execute_unprepared(&format!(
                    "ALTER TABLE {table} ADD COLUMN {column} TEXT NOT NULL DEFAULT ''"
                ))
                .await?;
        }
        // Rust normalization preserves Unicode expansion and context, unlike SQLite lower().
        let mut after = None::<String>;
        loop {
            let mut query = Query::select()
                .columns(
                    ["hash", "title", "title_unicode", "artist", "artist_unicode"].map(Alias::new),
                )
                .from(Alias::new("beatmap_metadata"))
                .order_by(Alias::new("hash"), Order::Asc)
                .limit(128)
                .to_owned();
            if let Some(hash) = &after {
                query.and_where(Expr::col(Alias::new("hash")).gt(hash.clone()));
            }
            let rows = Metadata::find_by_statement(manager.get_database_backend().build(&query))
                .all(connection)
                .await?;
            if rows.is_empty() {
                break;
            }
            for row in rows {
                let update = Query::update()
                    .table(Alias::new("beatmap_metadata"))
                    .values([
                        (
                            Alias::new("search_title"),
                            row.title.unwrap_or_default().to_lowercase().into(),
                        ),
                        (
                            Alias::new("search_title_unicode"),
                            row.title_unicode.unwrap_or_default().to_lowercase().into(),
                        ),
                        (
                            Alias::new("search_artist"),
                            row.artist.unwrap_or_default().to_lowercase().into(),
                        ),
                        (
                            Alias::new("search_artist_unicode"),
                            row.artist_unicode.unwrap_or_default().to_lowercase().into(),
                        ),
                    ])
                    .and_where(Expr::col(Alias::new("hash")).eq(row.hash.clone()))
                    .to_owned();
                connection.execute(&update).await?;
                after = Some(row.hash);
            }
        }
        let mut after = None::<i32>;
        loop {
            let mut query = Query::select()
                .columns([Alias::new("id"), Alias::new("difficulty_name")])
                .from(Alias::new("beatmaps"))
                .order_by(Alias::new("id"), Order::Asc)
                .limit(128)
                .to_owned();
            if let Some(id) = after {
                query.and_where(Expr::col(Alias::new("id")).gt(id));
            }
            let rows = Difficulty::find_by_statement(manager.get_database_backend().build(&query))
                .all(connection)
                .await?;
            if rows.is_empty() {
                break;
            }
            for row in rows {
                connection
                    .execute(
                        Query::update()
                            .table(Alias::new("beatmaps"))
                            .value(
                                Alias::new("search_difficulty_name"),
                                row.difficulty_name.unwrap_or_default().to_lowercase(),
                            )
                            .and_where(Expr::col(Alias::new("id")).eq(row.id)),
                    )
                    .await?;
                after = Some(row.id);
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (table, column) in COLUMNS.into_iter().rev() {
            manager
                .get_connection()
                .execute_unprepared(&format!("ALTER TABLE {table} DROP COLUMN {column}"))
                .await?;
        }
        Ok(())
    }
}

const COLUMNS: [(&str, &str); 5] = [
    ("beatmap_metadata", "search_title"),
    ("beatmap_metadata", "search_title_unicode"),
    ("beatmap_metadata", "search_artist"),
    ("beatmap_metadata", "search_artist_unicode"),
    ("beatmaps", "search_difficulty_name"),
];

#[derive(FromQueryResult)]
struct Metadata {
    hash: String,
    title: Option<String>,
    title_unicode: Option<String>,
    artist: Option<String>,
    artist_unicode: Option<String>,
}

#[derive(FromQueryResult)]
struct Difficulty {
    id: i32,
    difficulty_name: Option<String>,
}
