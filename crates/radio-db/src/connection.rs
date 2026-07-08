#[cfg(all(feature = "postgres", not(feature = "sqlite")))]
use diesel_async::AsyncPgConnection;

#[cfg(all(feature = "sqlite", not(feature = "postgres")))]
pub type DatabaseConnection =
    diesel_async::sync_connection_wrapper::SyncConnectionWrapper<diesel::sqlite::SqliteConnection>;
#[cfg(all(feature = "postgres", not(feature = "sqlite")))]
pub type DatabaseConnection = AsyncPgConnection;
