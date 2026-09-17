use sea_orm::{FromQueryResult, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(FromQueryResult)]
struct InstallationPaths {
    id: i32,
    root_path: String,
    marker_path: String,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let mut rows = InstallationPaths::find_by_statement(Statement::from_string(
            connection.get_database_backend(),
            "SELECT id, root_path, marker_path FROM osu_installations",
        ))
        .all(connection)
        .await?;
        // JSON quoting lengthens every key. Move longer keys first so even paths
        // literally containing another path's JSON cannot collide during conversion.
        rows.sort_by_key(|row| std::cmp::Reverse(row.marker_path.len()));
        for row in rows {
            let root = serde_json::to_string(&row.root_path)
                .map_err(|error| DbErr::Migration(error.to_string()))?;
            let marker = serde_json::to_string(&row.marker_path)
                .map_err(|error| DbErr::Migration(error.to_string()))?;
            connection
                .execute(
                    Query::update()
                        .table(Alias::new("osu_installations"))
                        .values([
                            (Alias::new("root_path"), root.into()),
                            (Alias::new("marker_path"), marker.into()),
                        ])
                        .and_where(Expr::col(Alias::new("id")).eq(row.id)),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Native installation paths cannot be downgraded to lossy text storage".into(),
        ))
    }
}
