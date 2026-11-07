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
        for (table, column, kind) in COLUMNS {
            connection
                .execute_unprepared(&format!("ALTER TABLE {table} ADD COLUMN {column} {kind}"))
                .await?;
        }
        let mut after = None::<String>;
        loop {
            let mut query = Query::select()
                .columns(["hash", "author", "source"].map(Alias::new))
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
                let mapper = row
                    .author
                    .as_ref()
                    .and_then(|author| author.get("username"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_lowercase();
                connection
                    .execute(
                        &Query::update()
                            .table(Alias::new("beatmap_metadata"))
                            .values([
                                (Alias::new("search_mapper"), mapper.into()),
                                (
                                    Alias::new("search_source"),
                                    row.source.unwrap_or_default().to_lowercase().into(),
                                ),
                            ])
                            .and_where(Expr::col(Alias::new("hash")).eq(row.hash.clone()))
                            .to_owned(),
                    )
                    .await?;
                after = Some(row.hash);
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (table, column, _) in COLUMNS.into_iter().rev() {
            manager
                .get_connection()
                .execute_unprepared(&format!("ALTER TABLE {table} DROP COLUMN {column}"))
                .await?;
        }
        Ok(())
    }
}

const COLUMNS: [(&str, &str, &str); 7] = [
    ("beatmaps", "stars", "DOUBLE PRECISION"),
    ("beatmaps", "status", "TEXT"),
    ("beatmap_sets", "ranked_at_ms", "BIGINT"),
    ("beatmap_sets", "submitted_at_ms", "BIGINT"),
    ("audio_sources", "duration_ms", "BIGINT"),
    (
        "beatmap_metadata",
        "search_mapper",
        "TEXT NOT NULL DEFAULT ''",
    ),
    (
        "beatmap_metadata",
        "search_source",
        "TEXT NOT NULL DEFAULT ''",
    ),
];

#[derive(FromQueryResult)]
struct Metadata {
    hash: String,
    author: Option<serde_json::Value>,
    source: Option<String>,
}
