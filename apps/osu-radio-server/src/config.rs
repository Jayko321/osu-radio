use std::env;

use anyhow::{Context, Result};

#[cfg(feature = "sqlite")]
const DATABASE_URL_ENV: &str = "SQLITE_DATABASE_URL";
#[cfg(all(feature = "postgres", not(feature = "sqlite")))]
const DATABASE_URL_ENV: &str = "POSTGRES_DATABASE_URL";
const ADDRESS_ENV: &str = "OSU_RADIO_SERVER_ADDRESS";
const DEFAULT_ADDRESS: &str = "127.0.0.1:3000";

#[cfg(test)]
mod tests;

pub(crate) struct ServerConfig {
    pub(crate) address: String,
    pub(crate) database_url: String,
}

impl ServerConfig {
    pub(crate) fn from_env() -> Result<Self> {
        match dotenvy::dotenv() {
            Ok(_) => {}
            Err(error) if error.not_found() => {}
            Err(error) => return Err(error).context("Failed to load .env"),
        }

        let database_url = env::var(DATABASE_URL_ENV).with_context(|| {
            format!("{DATABASE_URL_ENV} must be set in the environment or .env")
        })?;
        let address = env::var(ADDRESS_ENV).unwrap_or_else(|_| DEFAULT_ADDRESS.to_owned());

        Ok(Self {
            address,
            database_url,
        })
    }
}
