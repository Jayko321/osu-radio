//! Optional, toolkit-free Discord RPC. The library owns the wire protocol; a dedicated
//! worker supplies bounded Tokio socket I/O and waits for either input or new state.
use crate::{
    Track,
    playback::{Playback, PlayerState},
};
use discord_rich_presence::{DiscordIpc, activity};
use std::{
    future::Future,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{sync::watch, task::JoinHandle, time::Instant};
mod transport;
use transport::Transport;

pub const APPLICATION_ID: &str = "1558624797142028359";
const LOGO_ASSET: &str = "osu_radio";
const IO_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    token: u64,
    audio: i32,
    state: PlayerState,
    seek: u64,
    title: String,
    artist: String,
    duration: Option<Duration>,
}
#[derive(Clone, Debug)]
struct Presence {
    key: Key,
    position: Duration,
    observed: Instant,
    paused_label: String,
}
impl Presence {
    fn from_playback(track: Option<&Track>, playback: &Playback) -> Option<Self> {
        let track = track?;
        if !playback.has_source
            || playback.error.is_some()
            || playback.current_audio_id != Some(track.audio_source_id)
            || playback.loading_audio_id.is_some()
            || !matches!(
                playback.snapshot.state,
                PlayerState::Playing | PlayerState::Paused
            )
        {
            return None;
        }
        let artist = field(&track.artist, "Unknown artist");
        let paused_label = field(
            &format!(
                "Paused • {artist} • {}:{:02}",
                playback.snapshot.position.as_secs() / 60,
                playback.snapshot.position.as_secs() % 60
            ),
            "Paused",
        );
        Some(Self {
            key: Key {
                token: playback.playback_token,
                audio: track.audio_source_id,
                state: playback.snapshot.state,
                seek: playback.seek_serial,
                title: field(&track.title, "Unknown title"),
                artist,
                // Decoder duration is authoritative, not a library estimate.
                duration: playback
                    .snapshot
                    .duration
                    .filter(|duration| !duration.is_zero()),
            },
            paused_label,
            position: playback.snapshot.position,
            observed: Instant::now(),
        })
    }
    fn activity(&self) -> activity::Activity<'_> {
        let mut result = activity::Activity::new()
            .activity_type(activity::ActivityType::Listening)
            .details(&self.key.title)
            .state(&self.key.artist);
        if self.key.state == PlayerState::Paused {
            // No timestamps: Discord has no frozen progress bar.
            return result.state(&self.paused_label);
        }
        if let Some(duration) = self.key.duration {
            let position = self
                .position
                .saturating_add(self.observed.elapsed())
                .min(duration);
            if position < duration
                && let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH)
                && let Some(start) = now.checked_sub(position)
                && let Some(end) = start.checked_add(duration)
                && let (Ok(start), Ok(end)) =
                    (i64::try_from(start.as_secs()), i64::try_from(end.as_secs()))
            {
                result = result.timestamps(activity::Timestamps::new().start(start).end(end));
            }
        }
        result
    }
}
fn field(value: &str, fallback: &str) -> String {
    let value = value.trim();
    let value = if value.is_empty() { fallback } else { value };
    // Discord's text limit is 128; preserve UTF-8 and keep within bytes too.
    let mut result = String::new();
    for ch in value.chars() {
        if result.len().saturating_add(ch.len_utf8()) > 128 {
            break;
        }
        result.push(ch);
    }
    result
}
#[derive(Clone, Default)]
struct Desired {
    enabled: bool,
    shutdown: bool,
    presence: Option<Presence>,
}

/// Lazily starts one managed worker. No Discord connection or worker is created by default.
#[derive(Default)]
pub(crate) struct Service {
    desired: Desired,
    sender: Option<watch::Sender<Desired>>,
    worker: Option<JoinHandle<()>>,
}
impl Service {
    pub(crate) fn enabled(&mut self, enabled: bool) {
        if self.desired.enabled == enabled {
            return;
        }
        self.desired.enabled = enabled;
        if enabled && self.worker.is_none() {
            let (sender, receiver) = watch::channel(self.desired.clone());
            let runtime = tokio::runtime::Handle::current();
            self.worker = Some(tokio::task::spawn_blocking(move || {
                run(
                    Transport::new(runtime.clone(), receiver.clone()),
                    receiver,
                    &runtime,
                );
            }));
            self.sender = Some(sender);
        }
        self.publish();
    }
    pub(crate) fn update(&mut self, track: Option<&Track>, playback: &Playback) {
        let next = Presence::from_playback(track, playback);
        let changed = self.desired.presence.as_ref().map(|value| &value.key)
            != next.as_ref().map(|value| &value.key);
        self.desired.presence = next;
        if changed {
            self.publish();
        } else if let Some(sender) = &self.sender {
            // Keep retry's position current without waking the worker or sending IPC.
            sender.send_if_modified(|value| {
                value.presence.clone_from(&self.desired.presence);
                false
            });
        }
    }
    #[cfg(test)]
    pub(crate) fn current_token(&self) -> Option<u64> {
        self.desired.presence.as_ref().map(|value| value.key.token)
    }
    fn publish(&self) {
        if let Some(sender) = &self.sender {
            sender.send_replace(self.desired.clone());
        }
    }
    pub(crate) fn begin_shutdown(&mut self) {
        self.desired.shutdown = true;
        self.publish();
    }
    pub(crate) async fn shutdown(&mut self) {
        self.begin_shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.await;
        }
        self.sender = None;
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.desired.shutdown = true;
        self.publish();
    }
}

// Small transport seam: deterministic lifecycle tests need neither Discord nor a device.
trait Rpc {
    fn connect(&mut self) -> bool;
    fn set(&mut self, presence: &Presence) -> bool;
    fn clear(&mut self) -> bool;
    fn close(&mut self);
    fn incoming(&mut self) -> impl Future<Output = ()> + Send;
    fn receive(&mut self) -> bool;
}
impl Rpc for Transport {
    fn connect(&mut self) -> bool {
        DiscordIpc::connect(self).is_ok()
    }
    fn set(&mut self, presence: &Presence) -> bool {
        let mut activity = presence.activity();
        if self.use_assets {
            activity = activity.assets(
                activity::Assets::new()
                    .large_image(LOGO_ASSET)
                    .large_text("osu! radio"),
            );
        }
        self.set_activity(activity).is_ok()
    }
    fn clear(&mut self) -> bool {
        self.cancellable = false;
        let result = self.clear_activity().is_ok();
        self.cancellable = true;
        result
    }
    fn close(&mut self) {
        let _ = DiscordIpc::close(self);
    }
    async fn incoming(&mut self) {
        self.ready().await;
    }
    fn receive(&mut self) -> bool {
        match self.recv() {
            Ok((1, payload)) => {
                let rejected =
                    payload.get("evt").and_then(serde_json::Value::as_str) == Some("ERROR");
                if rejected {
                    self.use_assets = false;
                } // Retry without optional artwork.
                !rejected
            }
            Ok((3, payload)) => self.send(payload, 4).is_ok(), // PING -> PONG, library framing.
            _ => false,
        }
    }
}
enum Wake {
    Changed,
    Input,
    Retry,
}
fn run(mut rpc: impl Rpc, mut desired: watch::Receiver<Desired>, runtime: &tokio::runtime::Handle) {
    let mut connected = false;
    let mut sent: Option<Key> = None;
    let mut retry = Duration::from_secs(1);
    let mut deadline = Instant::now();
    loop {
        let mut state = desired.borrow_and_update().clone();
        if state.shutdown || !state.enabled {
            if connected {
                let _ = rpc.clear();
            }
            rpc.close();
            connected = false;
            sent = None;
            retry = Duration::from_secs(1);
            deadline = Instant::now();
            if state.shutdown {
                break;
            }
        } else {
            if !connected && Instant::now() >= deadline {
                connected = rpc.connect();
                if connected {
                    retry = Duration::from_secs(1);
                    sent = None;
                } else {
                    rpc.close();
                    deadline = Instant::now()
                        .checked_add(retry)
                        .unwrap_or_else(Instant::now);
                    retry = retry.saturating_mul(2).min(Duration::from_secs(30));
                }
                // Connection can take time. Always re-read the newest coalesced snapshot.
                if desired.has_changed().unwrap_or(true) {
                    continue;
                }
                state = desired.borrow().clone();
            }
            if connected {
                let key = state.presence.as_ref().map(|presence| presence.key.clone());
                if key != sent {
                    let success = if let Some(presence) = &state.presence {
                        rpc.set(presence)
                    } else {
                        rpc.clear()
                    };
                    if success {
                        sent = key;
                    } else {
                        rpc.close();
                        connected = false;
                        sent = None;
                        deadline = Instant::now()
                            .checked_add(retry)
                            .unwrap_or_else(Instant::now);
                    }
                }
            }
        }
        let wake = runtime.block_on(async {
            tokio::select! {
                biased;
                _ = desired.changed() => Wake::Changed,
                () = rpc.incoming(), if connected => Wake::Input,
                () = tokio::time::sleep_until(deadline), if state.enabled && !connected => Wake::Retry,
            }
        });
        if matches!(wake, Wake::Changed) && desired.has_changed().is_err() {
            break;
        }
        if matches!(wake, Wake::Input) && !rpc.receive() {
            rpc.close();
            connected = false;
            sent = None;
            deadline = Instant::now()
                .checked_add(retry)
                .unwrap_or_else(Instant::now);
        }
    }
    if connected {
        let _ = rpc.clear();
    }
    rpc.close();
}

#[cfg(test)]
mod tests;
