use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("playback_queue"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("audio_source_ids"))
                            .json()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("current_index"))
                            .big_integer()
                            .null(),
                    )
                    .col(ColumnDef::new(Alias::new("mode")).text().not_null())
                    .col(
                        ColumnDef::new(Alias::new("revision"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("playback_token"))
                            .big_integer()
                            .not_null(),
                    )
                    .check(Expr::cust("id = 1"))
                    .check(Expr::cust("current_index IS NULL OR current_index >= 0"))
                    .check(Expr::cust("mode IN ('stopped', 'paused', 'playing')"))
                    .check(Expr::cust("revision >= 0 AND playback_token >= 0"))
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO playback_queue (id, audio_source_ids, current_index, mode, revision, playback_token) VALUES (1, '[]', NULL, 'stopped', 0, 0)",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("playback_queue")).to_owned())
            .await
    }
}
