use anyhow::{Context, Result};
use radio_services::Services;

#[derive(Clone)]
pub(crate) struct AppState {
    services: Services,
}

impl AppState {
    pub(crate) async fn connect(database_url: &str) -> Result<Self> {
        let database = Services::connect(database_url)
            .await
            .context("Failed to connect to the configured database")?;

        database
            .check_connection()
            .await
            .context("The configured database did not respond to a connectivity check")?;
        database
            .migrate()
            .await
            .context("Failed to apply the beatmap schema to the configured database")?;

        Ok(Self { services: database })
    }

    pub(crate) const fn services(&self) -> &Services {
        &self.services
    }
}
