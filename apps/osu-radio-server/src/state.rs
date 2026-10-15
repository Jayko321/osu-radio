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

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use radio_core::{OsuKind, OsuMarker};
    use radio_services::PlaybackMode;

    #[tokio::test]
    async fn reconnect_recovers_persisted_queue_and_clones_share_monotone_notifications() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("state.sqlite");
        let state = AppState::connect(database.to_str().unwrap()).await.unwrap();
        let marker = directory.path().join("client.realm");
        std::fs::write(&marker, b"synthetic marker").unwrap();
        let installation = state
            .services()
            .osu_installations()
            .register(
                &OsuMarker {
                    kind: OsuKind::Lazer,
                    root_path: directory.path().into(),
                    marker_path: marker.clone(),
                },
                None,
            )
            .await
            .unwrap()
            .into_installation();
        state
            .services()
            .osu_installations()
            .replace_snapshot(
                installation.id,
                &[crate::test_support::beatmap_set(
                    1,
                    directory.path().join("absent-audio.mp3").to_str().unwrap(),
                )],
            )
            .await
            .unwrap();
        let id = state
            .services()
            .beatmap_sets()
            .search_tracks("")
            .await
            .unwrap()[0]
            .audio_source_id;
        state.services().queue().append(vec![id, id]).await.unwrap();
        let mut expected = state.services().queue().get().await.unwrap();
        assert_eq!(expected.mode, PlaybackMode::Playing);
        expected.mode = PlaybackMode::Paused;
        expected.revision = expected.revision.checked_add(1).unwrap();
        expected.playback_token = expected.playback_token.checked_add(1).unwrap();
        drop(state);
        let reopened = AppState::connect(database.to_str().unwrap()).await.unwrap();
        assert_eq!(reopened.services().queue().get().await.unwrap(), expected);
        let mut updates = reopened.subscribe_playback();
        assert_eq!(*updates.borrow(), expected.revision);
        let cloned = reopened.clone();
        cloned.publish_playback(expected.revision);
        cloned.publish_playback(expected.revision.checked_sub(1).unwrap());
        assert!(!updates.has_changed().unwrap());
        cloned.publish_playback(u64::MAX);
        assert!(updates.has_changed().unwrap());
        assert_eq!(*updates.borrow_and_update(), u64::MAX);
        reopened.publish_playback(expected.revision);
        assert!(!updates.has_changed().unwrap());
        assert_eq!(
            cloned.services().queue().get().await.unwrap(),
            expected,
            "notification does not mutate persisted state"
        );
        drop(updates);
        assert_eq!(reopened.playback_subscriber_count(), 0);
        assert_eq!(std::fs::read(marker).unwrap(), b"synthetic marker");
    }
}
