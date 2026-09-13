use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn column(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name))
}
fn id() -> ColumnDef {
    column("id")
        .integer()
        .not_null()
        .auto_increment()
        .primary_key()
        .to_owned()
}
fn foreign_key(
    from: &str,
    to: &str,
    key: &str,
    action: ForeignKeyAction,
) -> ForeignKeyCreateStatement {
    ForeignKey::create()
        .from_col(Alias::new(from))
        .to(Alias::new(to), Alias::new(key))
        .on_delete(action)
        .to_owned()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    #[allow(clippy::too_many_lines)] // Keep the frozen initial schema in one migration.
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("user_data"))
                    .col(column("id").integer().not_null().primary_key())
                    .check(Expr::col(Alias::new("id")).eq(1))
                    .to_owned(),
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared("INSERT INTO user_data (id) VALUES (1)")
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("osu_installations"))
                    .col(id())
                    .col(column("user_data_id").integer().not_null())
                    .col(column("kind").text().not_null())
                    .col(column("root_path").text().not_null())
                    .col(column("marker_path").text().not_null().unique_key())
                    .col(column("label").text())
                    .col(column("enabled").boolean().not_null().default(true))
                    .col(column("last_scanned_at").text())
                    .check(Expr::col(Alias::new("kind")).is_in(["stable", "lazer"]))
                    .foreign_key(&mut foreign_key(
                        "user_data_id",
                        "user_data",
                        "id",
                        ForeignKeyAction::Restrict,
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("beatmap_sets"))
                    .col(id())
                    .col(column("online_id").integer())
                    .col(column("hash").text())
                    .col(column("installation_id").integer().not_null())
                    .foreign_key(&mut foreign_key(
                        "installation_id",
                        "osu_installations",
                        "id",
                        ForeignKeyAction::Cascade,
                    ))
                    .to_owned(),
            )
            .await?;

        let mut metadata = Table::create();
        metadata
            .table(Alias::new("beatmap_metadata"))
            .col(column("hash").text().not_null().primary_key());
        for name in [
            "title",
            "title_unicode",
            "artist",
            "artist_unicode",
            "source",
            "tags",
            "audio_file",
            "background_file",
        ] {
            metadata.col(column(name).text());
        }
        metadata
            .col(column("author").json_binary())
            .col(column("user_tags").json_binary().not_null())
            .col(column("preview_time").integer());
        manager.create_table(metadata).await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("audio_sources"))
                    .col(id())
                    .col(column("kind").text().not_null())
                    .col(column("location").text().not_null())
                    .check(Expr::col(Alias::new("kind")).is_in(["local", "copied", "online"]))
                    .index(
                        Index::create()
                            .name("audio_sources_kind_location_key")
                            .unique()
                            .col(Alias::new("kind"))
                            .col(Alias::new("location")),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("beatmaps"))
                    .col(id())
                    .col(column("difficulty_name").text())
                    .col(column("bpm").double())
                    .col(column("hash").text())
                    .col(column("beatmap_set_id").integer().not_null())
                    .col(column("metadata_hash").text())
                    .col(column("audio_source_id").integer())
                    .foreign_key(&mut foreign_key(
                        "beatmap_set_id",
                        "beatmap_sets",
                        "id",
                        ForeignKeyAction::Cascade,
                    ))
                    .foreign_key(&mut foreign_key(
                        "metadata_hash",
                        "beatmap_metadata",
                        "hash",
                        ForeignKeyAction::Restrict,
                    ))
                    .foreign_key(&mut foreign_key(
                        "audio_source_id",
                        "audio_sources",
                        "id",
                        ForeignKeyAction::Restrict,
                    ))
                    .to_owned(),
            )
            .await?;

        for (table, field) in [
            ("osu_installations", "user_data_id"),
            ("beatmap_sets", "installation_id"),
            ("beatmaps", "beatmap_set_id"),
            ("beatmaps", "metadata_hash"),
            ("beatmaps", "audio_source_id"),
        ] {
            manager
                .create_index(
                    Index::create()
                        .name(format!("idx_{table}_{field}"))
                        .table(Alias::new(table))
                        .col(Alias::new(field))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
