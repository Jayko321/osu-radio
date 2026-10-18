//! System I/O runs in the media-controls worker; this coordinator only routes client state.
use super::Controller;
use osu_radio_media_controls::{Error, Event, NativeContext, Snapshot, Status};

impl Controller {
    pub(crate) fn system_artwork_key(&self) -> Option<(i32, osu_radio_media_controls::ArtworkKey)> {
        if !self.system_media_active() {
            return None;
        }
        let launch = self.player.system_launch()?;
        let track = self.player.current_track.as_ref()?;
        if self.player.assignment.as_ref()?.current_audio_source_id != Some(track.audio_source_id) {
            return None;
        }
        Some((
            track.audio_source_id,
            osu_radio_media_controls::ArtworkKey {
                launch,
                cover_id: track.cover_beatmap_id?,
                generation: self.media.generation,
            },
        ))
    }

    pub(crate) fn system_media_snapshot(&self) -> Snapshot {
        let mut snapshot = self.player.system_snapshot(
            self.session.is_some(),
            self.settings.volume.settings.is_some(),
            f64::from(self.assignment_volume()),
            self.settings.preferences.name_preferences,
            self.settings
                .volume
                .settings
                .is_some_and(|settings| settings.individual_volume_enabled),
        );
        snapshot.artwork = self.media.system_artwork(self.system_artwork_key());
        snapshot
    }

    pub(crate) fn publish_system_media(&self) {
        if !self.system_media_active() {
            return;
        }
        if let Some(controls) = &self.system_media {
            let _ = controls.publish(self.system_media_snapshot());
        }
    }

    pub(crate) fn system_media_active(&self) -> bool {
        self.system_media.as_ref().is_some_and(|controls| {
            matches!(
                *controls.status().borrow(),
                Status::Connecting | Status::Connected
            )
        })
    }

    pub(crate) fn connect_system_media(&mut self, context: NativeContext) -> Result<(), Error> {
        self.check_system_media_failure();
        if self.system_media_failed {
            return Err(Error::new(
                "system media integration failed; restart the application to reconnect",
            ));
        }
        if self.system_media.is_some() {
            self.publish_system_media();
            return Ok(());
        }
        match osu_radio_media_controls::MediaControls::connect(
            context,
            self.system_media_snapshot(),
        ) {
            Ok((controls, events)) => {
                self.system_media = Some(controls);
                self.system_events = Some(events);
                self.reconcile_media();
                Ok(())
            }
            Err(error) => {
                self.system_media_failed = true;
                eprintln!("System media controls: {error}");
                Err(error)
            }
        }
    }

    fn check_system_media_failure(&mut self) {
        if !self.system_media_failed
            && let Some(controls) = &self.system_media
            && let Status::Failed(error) = &*controls.status().borrow()
        {
            eprintln!("System media controls: {error}");
            self.system_media_failed = true;
        }
    }

    pub(crate) fn system_media_ended(&mut self) {
        self.check_system_media_failure();
        self.system_events = None;
        self.reconcile_media();
    }

    pub(crate) async fn disconnect_system_media(&mut self) {
        self.check_system_media_failure();
        self.system_events = None;
        if let Some(mut controls) = self.system_media.take() {
            controls.shutdown().await;
            // A fatal error racing with early shutdown must remain terminal too.
            if let Status::Failed(error) = &*controls.status().borrow() {
                if !self.system_media_failed {
                    eprintln!("System media controls: {error}");
                }
                self.system_media_failed = true;
            }
        }
        self.reconcile_media();
    }

    pub(crate) fn system_media_event(&mut self, event: Event) {
        let snapshot = self.system_media_snapshot();
        match event {
            Event::SetVolume(volume) if snapshot.capabilities.set_volume => {
                self.set_current_volume(volume);
            }
            event => self.system_player_event(event, &snapshot),
        }
    }
}

#[cfg(test)]
mod tests;
