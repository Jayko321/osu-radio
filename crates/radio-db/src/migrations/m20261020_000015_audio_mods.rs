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
            "ALTER TABLE user_data ADD COLUMN individual_mods_enabled BOOLEAN NOT NULL DEFAULT FALSE CHECK (individual_mods_enabled IN (FALSE, TRUE))",
            "ALTER TABLE user_data ADD COLUMN global_mods_mode TEXT NOT NULL DEFAULT 'none'",
            "ALTER TABLE user_data ADD COLUMN global_mods_speed_percent INTEGER NOT NULL DEFAULT 100",
            "ALTER TABLE user_data ADD COLUMN global_mods_adjust_pitch BOOLEAN NOT NULL DEFAULT FALSE CHECK (global_mods_speed_percent = CAST(global_mods_speed_percent AS INTEGER) AND global_mods_adjust_pitch IN (FALSE, TRUE) AND (\
             (global_mods_mode = 'none' AND global_mods_speed_percent = 100 AND NOT global_mods_adjust_pitch) OR \
             (global_mods_mode = 'dt' AND global_mods_speed_percent BETWEEN 101 AND 200) OR \
             (global_mods_mode = 'ht' AND global_mods_speed_percent BETWEEN 50 AND 99)))",
            "CREATE TABLE audio_mods (audio_kind TEXT NOT NULL CHECK (audio_kind IN ('local','copied','online')), \
             source_location TEXT NOT NULL, mode TEXT NOT NULL, speed_percent INTEGER NOT NULL, adjust_pitch BOOLEAN NOT NULL, \
             PRIMARY KEY (audio_kind, source_location), \
             CHECK (speed_percent = CAST(speed_percent AS INTEGER) AND adjust_pitch IN (FALSE, TRUE) AND (\
             (mode = 'none' AND speed_percent = 100 AND NOT adjust_pitch) OR \
             (mode = 'dt' AND speed_percent BETWEEN 101 AND 200) OR \
             (mode = 'ht' AND speed_percent BETWEEN 50 AND 99))))",
        ] {
            connection.execute_unprepared(statement).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE audio_mods")
            .await?;
        // Drop the constraint-bearing column before the columns it references.
        for column in [
            "global_mods_adjust_pitch",
            "global_mods_speed_percent",
            "global_mods_mode",
            "individual_mods_enabled",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!("ALTER TABLE user_data DROP COLUMN {column}"))
                .await?;
        }
        Ok(())
    }
}
