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
                    .table(Alias::new("listening_history"))
                    .col(ColumnDef::new(Alias::new("audio_kind")).text().not_null())
                    .col(
                        ColumnDef::new(Alias::new("source_location"))
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("last_played_at_ms"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("playback_token"))
                            .big_integer()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(Alias::new("audio_kind"))
                            .col(Alias::new("source_location")),
                    )
                    .check(Expr::cust("audio_kind IN ('local', 'copied', 'online')"))
                    .check(Expr::cust("playback_token >= 0"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("listening_history"))
                    .to_owned(),
            )
            .await
    }
}
