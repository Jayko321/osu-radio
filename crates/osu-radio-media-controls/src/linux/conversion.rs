use crate::{Event, LaunchKey, Snapshot};
use std::time::Duration;

pub(super) fn track_path(instance: &str, launch: LaunchKey) -> String {
    format!(
        "/io/osuradio/{instance}/entry_{:016x}/token_{:016x}",
        launch.entry_id, launch.playback_token
    )
}

pub(super) fn micros(duration: Duration) -> i64 {
    i64::try_from(duration.as_micros()).unwrap_or(i64::MAX)
}

pub(super) fn position(snapshot: &Snapshot) -> Duration {
    snapshot
        .duration
        .map_or(snapshot.position, |length| snapshot.position.min(length))
}

pub(super) fn absolute_seek(
    snapshot: &Snapshot,
    instance: &str,
    id: &str,
    micros: i64,
) -> Option<Event> {
    let launch = snapshot.launch?;
    if !snapshot.capabilities.seek || track_path(instance, launch) != id {
        return None;
    }
    let position = Duration::from_micros(u64::try_from(micros).ok()?);
    if snapshot.duration.is_some_and(|length| position > length) {
        return None;
    }
    Some(Event::SeekAbsolute { launch, position })
}

pub(super) fn relative_seek(snapshot: &Snapshot, offset_micros: i64) -> Option<Event> {
    let launch = snapshot.launch?;
    if !snapshot.capabilities.seek {
        return None;
    }
    // The client checks this launch before applying bounds or queuing a resulting Next.
    Some(Event::SeekRelative {
        launch,
        offset_micros,
    })
}

pub(super) fn volume(value: f64) -> Option<f64> {
    value.is_finite().then(|| value.clamp(0.0, 1.0))
}

pub(super) fn confirmed_seek(previous: &Snapshot, current: &Snapshot) -> Option<Duration> {
    (current.launch.is_some()
        && current.launch == previous.launch
        && current.seek_serial != previous.seek_serial)
        .then(|| position(current))
}
