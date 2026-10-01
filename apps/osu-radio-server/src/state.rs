use anyhow::{Context, Result};
use radio_services::Services;
use tokio::sync::watch;

#[derive(Clone)]
pub(crate) struct AppState {
    services: Services,
    playback_revision: watch::Sender<u64>,
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
        database
            .queue()
            .recover()
            .await
            .context("Failed to restore the playback queue")?;
        let revision = database.queue().get().await?.revision;

        Ok(Self {
            services: database,
            playback_revision: watch::channel(revision).0,
        })
    }

    pub(crate) const fn services(&self) -> &Services {
        &self.services
    }

    pub(crate) fn subscribe_playback(&self) -> watch::Receiver<u64> {
        self.playback_revision.subscribe()
    }

    pub(crate) fn publish_playback(&self, revision: u64) {
        self.playback_revision.send_if_modified(|current| {
            if revision <= *current {
                return false;
            }
            *current = revision;
            true
        });
    }

    #[cfg(all(test, feature = "sqlite"))]
    pub(crate) fn playback_subscriber_count(&self) -> usize {
        self.playback_revision.receiver_count()
    }
}
