// Frozen v12/v13 inference: only an active current checking entry implies waiting.
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INFER_WAITING: &str = "(mode IN ('playing', 'paused') AND EXISTS (SELECT 1 FROM queue_entries e WHERE e.id = playback_queue.current_entry_id AND e.availability = 'checking'))";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "ALTER TABLE playback_queue ADD COLUMN waiting_for_check BOOLEAN NOT NULL DEFAULT FALSE CHECK (NOT waiting_for_check OR (current_entry_id IS NOT NULL AND mode IN ('playing','paused')))"
        ).await?;
        manager
            .get_connection()
            .execute_unprepared(&format!(
                "UPDATE playback_queue SET waiting_for_check = {INFER_WAITING}"
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mismatch = manager
            .get_connection()
            .query_one_raw(sea_orm::Statement::from_string(
                manager.get_database_backend(),
                format!("SELECT id FROM playback_queue WHERE waiting_for_check <> {INFER_WAITING}"),
            ))
            .await?;
        if mismatch.is_some() {
            return Err(DbErr::Custom(
                "Cannot downgrade queue: waiting intent cannot be reconstructed from v12".into(),
            ));
        }
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE playback_queue DROP COLUMN waiting_for_check")
            .await?;
        Ok(())
    }
}
