use sea_orm::Statement;
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
                    .table(Alias::new("beatmaps"))
                    .add_column(ColumnDef::new(Alias::new("md5_hash")).string().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_beatmaps_md5_hash")
                    .table(Alias::new("beatmaps"))
                    .col(Alias::new("md5_hash"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("playlist_items"))
                    .add_column(
                        ColumnDef::new(Alias::new("hash_kind"))
                            .string()
                            .not_null()
                            .default("source")
                            .check(Expr::col(Alias::new("hash_kind")).is_in(["source", "md5"])),
                    )
                    .to_owned(),
            )
            .await?;
        for column in ["origin_source", "origin_collection"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("playlists"))
                        .add_column(ColumnDef::new(Alias::new(column)).text().null())
                        .to_owned(),
                )
                .await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("uq_playlists_origin")
                    .unique()
                    .table(Alias::new("playlists"))
                    .col(Alias::new("origin_source"))
                    .col(Alias::new("origin_collection"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let row = connection.query_one_raw(Statement::from_string(manager.get_database_backend(),
            "SELECT (SELECT COUNT(*) FROM playlists WHERE origin_source IS NOT NULL OR origin_collection IS NOT NULL) + (SELECT COUNT(*) FROM playlist_items WHERE hash_kind = 'md5') AS count".to_owned())).await?.ok_or_else(|| DbErr::Custom("Missing downgrade check".into()))?;
        if row.try_get::<i64>("", "count")? != 0 {
            return Err(DbErr::Custom(
                "Cannot downgrade imported collection playlists or MD5 membership".into(),
            ));
        }
        for (index, table) in [
            ("uq_playlists_origin", "playlists"),
            ("idx_beatmaps_md5_hash", "beatmaps"),
        ] {
            manager
                .drop_index(
                    Index::drop()
                        .name(index)
                        .table(Alias::new(table))
                        .to_owned(),
                )
                .await?;
        }
        for (table, column) in [
            ("playlists", "origin_collection"),
            ("playlists", "origin_source"),
            ("playlist_items", "hash_kind"),
            ("beatmaps", "md5_hash"),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new(table))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
