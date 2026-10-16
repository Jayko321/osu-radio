// Frozen v13 inference; a launch must retain its source when its queue entry is rebound.
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INFER_SOURCE: &str = "q.mode IN ('playing','paused') AND NOT q.waiting_for_check AND e.id = q.current_entry_id AND e.audio_source_id IS NOT NULL AND e.audio_kind IS NOT NULL AND e.audio_location IS NOT NULL";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        connection
            .execute_unprepared(
                "ALTER TABLE playback_queue ADD COLUMN launch_audio_source_id INTEGER",
            )
            .await?;
        connection
            .execute_unprepared("ALTER TABLE playback_queue ADD COLUMN launch_audio_kind TEXT")
            .await?;
        connection
            .execute_unprepared("ALTER TABLE playback_queue ADD COLUMN launch_audio_location TEXT")
            .await?;
        connection.execute_unprepared("ALTER TABLE playback_queue ADD COLUMN launch_check_generation BIGINT CHECK (\
            (launch_audio_source_id IS NULL AND launch_audio_kind IS NULL AND launch_audio_location IS NULL AND launch_check_generation IS NULL) OR \
            (launch_audio_source_id IS NOT NULL AND launch_audio_kind IS NOT NULL AND launch_audio_kind IN ('local','copied','online') \
             AND launch_audio_location IS NOT NULL AND launch_check_generation IS NOT NULL AND launch_check_generation >= 0 \
             AND current_entry_id IS NOT NULL AND mode IN ('playing','paused') AND NOT waiting_for_check))").await?;
        connection.execute_unprepared(&format!("UPDATE playback_queue AS q SET \
            (launch_audio_source_id, launch_audio_kind, launch_audio_location, launch_check_generation) = \
            (SELECT e.audio_source_id, e.audio_kind, e.audio_location, e.check_generation FROM queue_entries e WHERE {INFER_SOURCE})")).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let mismatch = connection.query_one_raw(sea_orm::Statement::from_string(manager.get_database_backend(), format!(
            "SELECT q.id FROM playback_queue q LEFT JOIN queue_entries e ON {INFER_SOURCE} WHERE \
            q.launch_audio_source_id IS DISTINCT FROM e.audio_source_id OR \
            q.launch_audio_kind IS DISTINCT FROM e.audio_kind OR \
            q.launch_audio_location IS DISTINCT FROM e.audio_location OR \
            q.launch_check_generation IS DISTINCT FROM e.check_generation"
        ))).await?;
        if mismatch.is_some() {
            return Err(DbErr::Custom(
                "Cannot downgrade queue: launch source cannot be reconstructed from v13".into(),
            ));
        }
        for column in [
            "launch_check_generation",
            "launch_audio_location",
            "launch_audio_kind",
            "launch_audio_source_id",
        ] {
            connection
                .execute_unprepared(&format!("ALTER TABLE playback_queue DROP COLUMN {column}"))
                .await?;
        }
        Ok(())
    }
}
