use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for column in ["title_unicode", "artist_unicode"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("playlist_items"))
                        .add_column(ColumnDef::new(Alias::new(column)).text().null())
                        .to_owned(),
                )
                .await?;
        }
        manager
            .get_connection()
            .execute_unprepared(
                "UPDATE playlist_items SET \
            title_unicode = (SELECT m.title_unicode FROM beatmaps b \
                LEFT JOIN beatmap_metadata m ON m.hash = b.metadata_hash WHERE b.id = \
                (SELECT MIN(candidate.id) FROM beatmaps candidate \
                    JOIN beatmap_sets s ON s.id = candidate.beatmap_set_id \
                    JOIN osu_installations i ON i.id = s.installation_id \
                    JOIN audio_sources a ON a.id = candidate.audio_source_id \
                    WHERE candidate.hash = playlist_items.beatmap_hash \
                    AND i.kind = playlist_items.source_kind AND a.kind IN ('local', 'copied'))), \
            artist_unicode = (SELECT m.artist_unicode FROM beatmaps b \
                LEFT JOIN beatmap_metadata m ON m.hash = b.metadata_hash WHERE b.id = \
                (SELECT MIN(candidate.id) FROM beatmaps candidate \
                    JOIN beatmap_sets s ON s.id = candidate.beatmap_set_id \
                    JOIN osu_installations i ON i.id = s.installation_id \
                    JOIN audio_sources a ON a.id = candidate.audio_source_id \
                    WHERE candidate.hash = playlist_items.beatmap_hash \
                    AND i.kind = playlist_items.source_kind AND a.kind IN ('local', 'copied'))) \
            WHERE EXISTS (SELECT 1 FROM beatmaps candidate \
                JOIN beatmap_sets s ON s.id = candidate.beatmap_set_id \
                JOIN osu_installations i ON i.id = s.installation_id \
                JOIN audio_sources a ON a.id = candidate.audio_source_id \
                WHERE candidate.hash = playlist_items.beatmap_hash \
                AND i.kind = playlist_items.source_kind AND a.kind IN ('local', 'copied'))",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for column in ["artist_unicode", "title_unicode"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("playlist_items"))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
