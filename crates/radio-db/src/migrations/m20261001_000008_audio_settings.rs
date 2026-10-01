use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "ALTER TABLE user_data ADD COLUMN individual_volume_enabled BOOLEAN NOT NULL DEFAULT FALSE"
        ).await?;
        manager.get_connection().execute_unprepared(
            "ALTER TABLE user_data ADD COLUMN global_volume_percent INTEGER NOT NULL DEFAULT 100 CHECK (global_volume_percent BETWEEN 0 AND 100)"
        ).await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("audio_volume"))
                    .col(ColumnDef::new(Alias::new("audio_kind")).text().not_null())
                    .col(
                        ColumnDef::new(Alias::new("source_location"))
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("volume_percent"))
                            .integer()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(Alias::new("audio_kind"))
                            .col(Alias::new("source_location")),
                    )
                    .check(Expr::cust("audio_kind IN ('local', 'copied', 'online')"))
                    .check(Expr::cust("volume_percent BETWEEN 0 AND 100"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("audio_volume")).to_owned())
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE user_data DROP COLUMN global_volume_percent")
            .await?;
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE user_data DROP COLUMN individual_volume_enabled")
            .await?;
        Ok(())
    }
}
