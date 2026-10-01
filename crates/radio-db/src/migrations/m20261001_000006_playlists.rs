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
                    .table(Alias::new("playlists"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Alias::new("name")).text().not_null())
                    .check(Expr::cust("length(trim(name)) > 0"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("playlist_items"))
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("playlist_id"))
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("source_kind")).text().not_null())
                    .col(ColumnDef::new(Alias::new("beatmap_hash")).text().not_null())
                    .col(ColumnDef::new(Alias::new("title")).text().null())
                    .col(ColumnDef::new(Alias::new("artist")).text().null())
                    .col(ColumnDef::new(Alias::new("difficulty_name")).text().null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_playlist_items_playlist")
                            .from(Alias::new("playlist_items"), Alias::new("playlist_id"))
                            .to(Alias::new("playlists"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("uq_playlist_items_key")
                            .unique()
                            .col(Alias::new("playlist_id"))
                            .col(Alias::new("source_kind"))
                            .col(Alias::new("beatmap_hash")),
                    )
                    .check(Expr::cust("length(trim(beatmap_hash)) > 0"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("playback_queue"))
                    .add_column(
                        ColumnDef::new(Alias::new("playlist_item_ids"))
                            .json()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_beatmaps_source_hash")
                    .table(Alias::new("beatmaps"))
                    .col(Alias::new("hash"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_beatmaps_source_hash")
                    .table(Alias::new("beatmaps"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("playback_queue"))
                    .drop_column(Alias::new("playlist_item_ids"))
                    .to_owned(),
            )
            .await?;
        for table in ["playlist_items", "playlists"] {
            manager
                .drop_table(Table::drop().table(Alias::new(table)).to_owned())
                .await?;
        }
        Ok(())
    }
}
