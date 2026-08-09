use std::env;

use anyhow::{Context, Result};

const SQLITE_DATABASE_URL_ENV: &str = "SQLITE_DATABASE_URL";
const ADDRESS_ENV: &str = "OSU_RADIO_SERVER_ADDRESS";
const DEFAULT_ADDRESS: &str = "127.0.0.1:3000";

pub(crate) struct ServerConfig {
    pub(crate) address: String,
    pub(crate) database_url: String,
}

impl ServerConfig {
    pub(crate) fn from_env() -> Result<Self> {
        dotenvy::dotenv().context("Failed to load .env")?;

        let database_url = env::var(SQLITE_DATABASE_URL_ENV)
            .with_context(|| format!("{SQLITE_DATABASE_URL_ENV} must be set in .env"))?;
        let address = env::var(ADDRESS_ENV).unwrap_or_else(|_| DEFAULT_ADDRESS.to_owned());

        Ok(Self {
            address,
            database_url,
        })
    }
}
