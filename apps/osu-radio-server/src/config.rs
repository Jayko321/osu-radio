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
        if env::var_os("OSU_RADIO_SKIP_DOTENV").as_deref() != Some(std::ffi::OsStr::new("1")) {
            match dotenvy::dotenv() {
                Ok(_) => {}
                Err(error) if error.not_found() => {}
                Err(error) => return Err(error).context("Failed to load .env"),
            }
        }

        let database_url = match env::var(DATABASE_URL_ENV) {
            Ok(url) => url,
            #[cfg(all(target_os = "linux", feature = "sqlite"))]
            Err(env::VarError::NotPresent) => default_sqlite_path()?,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("{DATABASE_URL_ENV} must be set in the environment or .env")
                });
            }
        };
        let address = env::var(ADDRESS_ENV).unwrap_or_else(|_| DEFAULT_ADDRESS.to_owned());

        Ok(Self {
            address,
            database_url,
        })
    }
}

#[cfg(all(target_os = "linux", feature = "sqlite"))]
fn default_sqlite_path() -> Result<String> {
    use std::{fs, path::PathBuf};

    let base = env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|home| home.join(".local/share"))
        })
        .context("Cannot determine SQLite data directory: set an absolute XDG_DATA_HOME or HOME")?;
    let directory = base.join("osu-radio");
    let path = directory.join("osu-radio.db");
    let url = path
        .to_str()
        .context("SQLite data path is not valid UTF-8")?
        .to_owned();
    fs::create_dir_all(&directory).with_context(|| {
        format!(
            "Failed to create SQLite data directory `{}`",
            directory.display()
        )
    })?;
    Ok(url)
}
