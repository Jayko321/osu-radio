use anyhow::{Result, bail};
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use sea_orm_migration::{MigratorTrait, SchemaManager, prelude::*};

mod m20260913_000001_library;
mod m20260913_000002_background;
mod m20260914_000003_native_paths;

pub(crate) struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260913_000001_library::Migration),
            Box::new(m20260913_000002_background::Migration),
            Box::new(m20260914_000003_native_paths::Migration),
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
    let transaction = begin_schema_change(connection).await?;
    let manager = SchemaManager::new(&transaction);
    let has_history = manager.has_table("seaql_migrations").await?
        && !Migrator::get_applied_migrations(&transaction)
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
    Migrator::up(&transaction, None).await?;
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn reset(connection: &DatabaseConnection) -> Result<()> {
    let transaction = begin_schema_change(connection).await?;
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

// Lock before reading history, including on a database with no application tables yet.
async fn begin_schema_change(
    connection: &DatabaseConnection,
) -> Result<sea_orm::DatabaseTransaction> {
    #[cfg(feature = "sqlite")]
    let transaction = connection
        .begin_with_options(sea_orm::TransactionOptions {
            sqlite_transaction_mode: Some(sea_orm::SqliteTransactionMode::Immediate),
            ..Default::default()
        })
        .await?;
    #[cfg(feature = "postgres")]
    let transaction = {
        let transaction = connection
            .begin_with_config(Some(sea_orm::IsolationLevel::ReadCommitted), None)
            .await?;
        // Fixed application namespace and schema-lock ID; released on commit/rollback/drop.
        transaction
            .execute_unprepared("SELECT pg_advisory_xact_lock(1918985321, 1)")
            .await?;
        transaction
    };
    Ok(transaction)
}
