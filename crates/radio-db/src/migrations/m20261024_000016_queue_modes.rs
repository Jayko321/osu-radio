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
        for statement in [
            "ALTER TABLE playback_queue ADD COLUMN shuffle BOOLEAN NOT NULL DEFAULT FALSE CHECK (shuffle IN (FALSE, TRUE))",
            "ALTER TABLE playback_queue ADD COLUMN repeat_mode TEXT NOT NULL DEFAULT 'off' CHECK (repeat_mode IN ('off','all','one'))",
            "ALTER TABLE playback_queue ADD COLUMN repeat_launch BOOLEAN NOT NULL DEFAULT FALSE CHECK (repeat_launch IN (FALSE, TRUE))",
            "ALTER TABLE queue_entries ADD COLUMN shuffle_order BIGINT",
        ] {
            connection.execute_unprepared(statement).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let enabled = connection
            .query_one_raw(sea_orm::Statement::from_string(
                manager.get_database_backend(),
                "SELECT id FROM playback_queue WHERE shuffle OR repeat_mode <> 'off'",
            ))
            .await?;
        if enabled.is_some() {
            return Err(DbErr::Custom(
                "Cannot downgrade queue: shuffle or repeat mode cannot be represented in v15"
                    .into(),
            ));
        }
        for statement in [
            "ALTER TABLE queue_entries DROP COLUMN shuffle_order",
            "ALTER TABLE playback_queue DROP COLUMN repeat_launch",
            "ALTER TABLE playback_queue DROP COLUMN repeat_mode",
            "ALTER TABLE playback_queue DROP COLUMN shuffle",
        ] {
            connection.execute_unprepared(statement).await?;
        }
        Ok(())
    }
}
