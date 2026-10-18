use super::conversion;
use crate::{Error, LaunchKey, PlaybackStatus, Snapshot};
use mpris_server::{Metadata, Property, Time, TrackId};
use std::{fmt::Write, path::Path, time::Duration};

#[derive(Clone)]
pub(super) struct Published {
    pub snapshot: Snapshot,
    pub metadata: Metadata,
}

impl Published {
    pub fn new(snapshot: Snapshot, artwork: Option<&Path>, instance: &str) -> Result<Self, Error> {
        let mut metadata = Metadata::new();
        if let Some(launch) = snapshot.launch {
            metadata.set_trackid(Some(track_id(instance, launch)?));
            metadata.set_length(snapshot.duration.map(time));
            if let Some(text) = &snapshot.metadata {
                metadata.set_title(text.title.as_deref().filter(|value| !value.is_empty()));
                metadata.set_artist(
                    text.artist
                        .as_deref()
                        .filter(|value| !value.is_empty())
                        .map(|artist| [artist]),
                );
            }
            metadata.set_art_url(artwork.map(file_uri));
        }
        Ok(Self { snapshot, metadata })
    }
}

pub(super) fn track_id(instance: &str, launch: LaunchKey) -> Result<TrackId, Error> {
    TrackId::try_from(conversion::track_path(instance, launch))
        .map_err(|error| Error::new(format!("MPRIS TrackId: {error}")))
}

pub(super) fn time(duration: Duration) -> Time {
    Time::from_micros(conversion::micros(duration))
}

pub(super) const fn status(status: PlaybackStatus) -> mpris_server::PlaybackStatus {
    match status {
        PlaybackStatus::Stopped => mpris_server::PlaybackStatus::Stopped,
        PlaybackStatus::Playing => mpris_server::PlaybackStatus::Playing,
        PlaybackStatus::Paused => mpris_server::PlaybackStatus::Paused,
    }
}

#[allow(clippy::float_cmp)] // Publish any change to the client's normalized volume value.
pub(super) fn changes(previous: &Published, current: &Published) -> Vec<Property> {
    let old = &previous.snapshot;
    let new = &current.snapshot;
    let mut properties = Vec::new();
    if previous.metadata != current.metadata {
        properties.push(Property::Metadata(current.metadata.clone()));
    }
    if old.status != new.status {
        properties.push(Property::PlaybackStatus(status(new.status)));
    }
    if old.volume != new.volume {
        properties.push(Property::Volume(new.volume));
    }
    for (changed, property) in [
        (
            old.capabilities.play != new.capabilities.play,
            Property::CanPlay(new.capabilities.play),
        ),
        (
            old.capabilities.pause != new.capabilities.pause,
            Property::CanPause(new.capabilities.pause),
        ),
        (
            old.capabilities.next != new.capabilities.next,
            Property::CanGoNext(new.capabilities.next),
        ),
        (
            old.capabilities.previous != new.capabilities.previous,
            Property::CanGoPrevious(new.capabilities.previous),
        ),
        (
            old.capabilities.seek != new.capabilities.seek,
            Property::CanSeek(new.capabilities.seek),
        ),
    ] {
        if changed {
            properties.push(property);
        }
    }
    properties
}

fn file_uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for byte in path.as_os_str().as_encoded_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(char::from(*byte));
            }
            _ => {
                let _ = write!(uri, "%{byte:02X}");
            }
        }
    }
    uri
}

#[cfg(test)]
mod tests;
