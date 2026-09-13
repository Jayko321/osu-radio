use anyhow::{Result, bail};
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use sea_orm_migration::{MigratorTrait, SchemaManager, prelude::*};

mod m20260913_000001_library;
mod m20260913_000002_background;

pub(crate) struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260913_000001_library::Migration),
            Box::new(m20260913_000002_background::Migration),
        ]
    }
}

// Child-first order is also valid for the legacy Diesel schema.
const APPLICATION_TABLES: &[&str] = &[
    "beatmaps",
    "beatmap_sets",
    "beatmap_metadata",
    "audio_sources",
    "osu_installations",
    "user_data",
];

pub(crate) async fn migrate(connection: &DatabaseConnection) -> Result<()> {
    let manager = SchemaManager::new(connection);
    let has_history = manager.has_table("seaql_migrations").await?
        && !Migrator::get_applied_migrations(connection)
            .await?
            .is_empty();
    if !has_history {
        for table in APPLICATION_TABLES {
            if manager.has_table(*table).await? {
                bail!(
                    "Legacy or unversioned osu-radio database detected. Use a new database or explicitly reset it with `store --clear` (discards the stored library and settings); legacy data migration is not supported."
                );
            }
        }
    }
    Migrator::up(connection, None).await?;
    Ok(())
}

pub(crate) async fn reset(connection: &DatabaseConnection) -> Result<()> {
    let transaction = connection.begin().await?;
    for table in APPLICATION_TABLES
        .iter()
        .copied()
        .chain(["__diesel_schema_migrations", "seaql_migrations"])
    {
        transaction
            .execute(
                &Table::drop()
                    .table(Alias::new(table))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
    }
    // Migrate within the same transaction: a failed reset cannot leave a half-created schema.
    Migrator::up(&transaction, None).await?;
    transaction.commit().await?;
    Ok(())
}
