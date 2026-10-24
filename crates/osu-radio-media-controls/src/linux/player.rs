use super::{
    conversion, send,
    transform::{self, Published},
};
use crate::{Event, EventSender, Snapshot};
use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, RootInterface, Time,
    TrackId, Volume,
    zbus::{self, fdo},
};
use tokio::sync::watch;

pub(super) struct Player {
    pub instance: String,
    pub snapshots: watch::Receiver<Snapshot>,
    pub published: watch::Receiver<Published>,
    pub events: EventSender,
}

fn unsupported() -> fdo::Error {
    fdo::Error::NotSupported("osu! radio does not support this operation".into())
}

#[allow(clippy::unused_async_trait_impl)] // Match the upstream asynchronous interface signatures.
impl RootInterface for Player {
    async fn raise(&self) -> fdo::Result<()> {
        Ok(())
    }
    async fn quit(&self) -> fdo::Result<()> {
        Ok(())
    }
    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn fullscreen(&self) -> fdo::Result<bool> {
        Err(unsupported())
    }
    async fn set_fullscreen(&self, _value: bool) -> zbus::Result<()> {
        Err(unsupported().into())
    }
    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }
    async fn identity(&self) -> fdo::Result<String> {
        Ok("osu! radio".into())
    }
    async fn desktop_entry(&self) -> fdo::Result<String> {
        Err(unsupported())
    }
    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(Vec::new())
    }
    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(Vec::new())
    }
}

#[allow(clippy::unused_async_trait_impl)] // Callbacks only enqueue, and getters read a snapshot.
impl PlayerInterface for Player {
    async fn next(&self) -> fdo::Result<()> {
        send(
            &self.events,
            self.snapshots
                .borrow()
                .capabilities
                .next
                .then_some(Event::Next),
        );
        Ok(())
    }
    async fn previous(&self) -> fdo::Result<()> {
        send(
            &self.events,
            self.snapshots
                .borrow()
                .capabilities
                .previous
                .then_some(Event::Previous),
        );
        Ok(())
    }
    async fn pause(&self) -> fdo::Result<()> {
        send(
            &self.events,
            self.snapshots
                .borrow()
                .capabilities
                .pause
                .then_some(Event::Pause),
        );
        Ok(())
    }
    async fn play_pause(&self) -> fdo::Result<()> {
        let snapshot = self.snapshots.borrow();
        let allowed = snapshot.capabilities.play || snapshot.capabilities.pause;
        drop(snapshot);
        send(&self.events, allowed.then_some(Event::PlayPause));
        Ok(())
    }
    async fn stop(&self) -> fdo::Result<()> {
        send(
            &self.events,
            self.snapshots
                .borrow()
                .capabilities
                .stop
                .then_some(Event::Stop),
        );
        Ok(())
    }
    async fn play(&self) -> fdo::Result<()> {
        send(
            &self.events,
            self.snapshots
                .borrow()
                .capabilities
                .play
                .then_some(Event::Play),
        );
        Ok(())
    }
    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        let event = conversion::relative_seek(&self.snapshots.borrow(), offset.as_micros());
        send(&self.events, event);
        Ok(())
    }
    async fn set_position(&self, id: TrackId, position: Time) -> fdo::Result<()> {
        let event = conversion::absolute_seek(
            &self.snapshots.borrow(),
            &self.instance,
            id.as_str(),
            position.as_micros(),
        );
        send(&self.events, event);
        Ok(())
    }
    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Err(unsupported())
    }
    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(transform::status(self.published.borrow().snapshot.status))
    }
    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Err(unsupported())
    }
    async fn set_loop_status(&self, _value: LoopStatus) -> zbus::Result<()> {
        Err(unsupported().into())
    }
    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(self.published.borrow().snapshot.rate)
    }
    #[allow(clippy::float_cmp)] // Only an unchanged rate is a no-op; mods remain client-owned.
    async fn set_rate(&self, value: PlaybackRate) -> zbus::Result<()> {
        if value == self.published.borrow().snapshot.rate {
            Ok(())
        } else {
            Err(unsupported().into())
        }
    }
    async fn shuffle(&self) -> fdo::Result<bool> {
        Err(unsupported())
    }
    async fn set_shuffle(&self, _value: bool) -> zbus::Result<()> {
        Err(unsupported().into())
    }
    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(self.published.borrow().metadata.clone())
    }
    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(self.published.borrow().snapshot.volume)
    }
    async fn set_volume(&self, value: Volume) -> zbus::Result<()> {
        let value = conversion::volume(value)
            .ok_or_else(|| fdo::Error::InvalidArgs("volume must be finite".into()))?;
        send(
            &self.events,
            self.snapshots
                .borrow()
                .capabilities
                .set_volume
                .then_some(Event::SetVolume(value)),
        );
        Ok(())
    }
    async fn position(&self) -> fdo::Result<Time> {
        Ok(transform::time(conversion::position(
            &self.published.borrow().snapshot,
        )))
    }
    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(0.5)
    }
    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(2.0)
    }
    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(self.published.borrow().snapshot.capabilities.next)
    }
    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(self.published.borrow().snapshot.capabilities.previous)
    }
    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(self.published.borrow().snapshot.capabilities.play)
    }
    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(self.published.borrow().snapshot.capabilities.pause)
    }
    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(self.published.borrow().snapshot.capabilities.seek)
    }
    // MPRIS CanControl is constant; individual capabilities describe current availability.
    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
