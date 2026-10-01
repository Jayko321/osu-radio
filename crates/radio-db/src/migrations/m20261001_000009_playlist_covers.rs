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
            .alter_table(
                Table::alter()
                    .table(Alias::new("playlists"))
                    .add_column(
                        ColumnDef::new(Alias::new("custom_cover_png"))
                            .binary()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager.get_connection().execute_unprepared(
            "ALTER TABLE playlists ADD COLUMN custom_cover_revision BIGINT NOT NULL DEFAULT 0 CHECK (custom_cover_revision BETWEEN 0 AND 9223372036854775807)"
        ).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for column in ["custom_cover_revision", "custom_cover_png"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("playlists"))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
