//! Optional, toolkit-free Discord RPC with bounded frames and correlated Activity replies.
use crate::{
    Track,
    playback::{Playback, PlayerState},
};
use discord_rich_presence::{DiscordIpc, activity, error::Error};
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
    /// Rate changes republish presence even when nothing else (e.g. paused) does.
    speed_bits: u32,
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
        // Snapshot times are source coordinates; Discord shows real time, scaled once here.
        let speed = playback.snapshot.parameters.speed;
        let speed = if (0.5..=2.0).contains(&speed) {
            speed
        } else {
            1.0
        };
        let real_time = |source: Duration| {
            Duration::try_from_secs_f64(source.as_secs_f64() / f64::from(speed))
                .unwrap_or(Duration::MAX)
        };
        let position = real_time(playback.snapshot.position);
        let paused_label = field(
            &format!(
                "Paused • {}:{:02} • {artist}",
                position.as_secs() / 60,
                position.as_secs() % 60
            ),
            "Paused",
        );
        Some(Self {
            key: Key {
                token: playback.playback_token,
                audio: track.audio_source_id,
                state: playback.snapshot.state,
                seek: playback.seek_serial,
                speed_bits: speed.to_bits(),
                title: field(&track.title, "Unknown title"),
                artist,
                // Decoder duration is authoritative, not a library estimate.
                duration: playback
                    .snapshot
                    .duration
                    .filter(|duration| !duration.is_zero())
                    .map(real_time),
            },
            paused_label,
            position,
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

// Tests exercise this same worker with either a mock or real local IPC transport.
trait Rpc {
    fn connect(&mut self) -> Result<(), Error>;
    fn set(&mut self, presence: &Presence, artwork: bool) -> Result<String, Error>;
    fn clear(&mut self) -> Result<String, Error>;
    fn close(&mut self);
    fn incoming(&mut self) -> impl Future<Output = Result<(), Error>> + Send;
    fn receive(&mut self) -> Result<Reply, Error>;
}
#[derive(Debug)]
enum Reply {
    Other,
    Ack(String),
    Rejected {
        nonce: Option<String>,
        code: Option<i64>,
        message: String,
    },
}
impl Rpc for Transport {
    fn connect(&mut self) -> Result<(), Error> {
        DiscordIpc::connect(self)
    }
    fn set(&mut self, presence: &Presence, artwork: bool) -> Result<String, Error> {
        let mut activity = presence.activity();
        if artwork {
            activity = activity.assets(
                activity::Assets::new()
                    .large_image(LOGO_ASSET)
                    .large_text("osu! radio"),
            );
        }
        let nonce = self.next_nonce();
        self.send(
            serde_json::json!({
                "cmd": "SET_ACTIVITY",
                "args": { "pid": std::process::id(), "activity": activity },
                "nonce": nonce,
            }),
            1,
        )?;
        Ok(nonce)
    }
    fn clear(&mut self) -> Result<String, Error> {
        let nonce = self.next_nonce();
        self.cancellable = false;
        let result = self.send(
            serde_json::json!({
                "cmd": "SET_ACTIVITY",
                "args": { "pid": std::process::id(), "activity": null },
                "nonce": nonce,
            }),
            1,
        );
        self.cancellable = true;
        result?;
        Ok(nonce)
    }
    fn close(&mut self) {
        let _ = DiscordIpc::close(self);
    }
    async fn incoming(&mut self) -> Result<(), Error> {
        self.ready().await
    }
    fn receive(&mut self) -> Result<Reply, Error> {
        let (opcode, bytes) = self.recv_raw()?;
        match opcode {
            1 => {
                let payload: serde_json::Value =
                    serde_json::from_slice(&bytes).map_err(|_| Error::JsonParseResponse)?;
                let nonce = payload.get("nonce").and_then(serde_json::Value::as_str);
                if payload.get("evt").and_then(serde_json::Value::as_str) == Some("ERROR") {
                    return Ok(Reply::Rejected {
                        nonce: nonce
                            .filter(|_| {
                                payload.get("cmd").and_then(serde_json::Value::as_str)
                                    == Some("SET_ACTIVITY")
                            })
                            .map(str::to_owned),
                        code: payload
                            .get("data")
                            .and_then(|data| data.get("code"))
                            .and_then(serde_json::Value::as_i64),
                        message: field(
                            payload
                                .get("data")
                                .and_then(|data| data.get("message"))
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("Unknown RPC error"),
                            "Unknown RPC error",
                        ),
                    });
                }
                if payload.get("cmd").and_then(serde_json::Value::as_str) == Some("SET_ACTIVITY")
                    && payload.get("evt").is_none_or(serde_json::Value::is_null)
                    && let Some(nonce) = nonce
                {
                    return Ok(Reply::Ack(nonce.to_owned()));
                }
                Ok(Reply::Other)
            }
            3 => {
                self.pong(&bytes)?;
                Ok(Reply::Other)
            }
            4 => Ok(Reply::Other),
            _ => Err(Error::ReadError(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                format!("Discord IPC closed or invalid opcode: {opcode}"),
            ))),
        }
    }
}
struct Request {
    nonce: String,
    key: Option<Key>,
    artwork: bool,
    fallback: bool,
}
struct Worker<R> {
    rpc: R,
    connected: bool,
    sent: Option<Key>,
    pending: Option<Request>,
    use_assets: bool,
    retry: Duration,
    deadline: Instant,
}
impl<R: Rpc> Worker<R> {
    fn failed(&mut self, stage: &str, error: &Error) {
        eprintln!("[discord] {stage}: {error:?}");
        self.rpc.close();
        self.connected = false;
        self.sent = None;
        self.pending = None;
        self.deadline = Instant::now()
            .checked_add(self.retry)
            .unwrap_or_else(Instant::now);
        self.retry = self.retry.saturating_mul(2).min(Duration::from_secs(30));
    }
    fn publish(
        &mut self,
        presence: Option<&Presence>,
        artwork: bool,
        fallback: bool,
    ) -> Result<(), Error> {
        let nonce = if let Some(presence) = presence {
            self.rpc.set(presence, artwork)?
        } else {
            self.rpc.clear()?
        };
        self.sent = presence.map(|presence| presence.key.clone());
        self.pending = Some(Request {
            nonce,
            key: self.sent.clone(),
            artwork,
            fallback,
        });
        Ok(())
    }
    fn receive(&mut self, desired: &watch::Receiver<Desired>) -> Result<(), Error> {
        let reply = self.rpc.receive()?;
        let state = desired.borrow().clone();
        match reply {
            Reply::Ack(nonce) => {
                if self.pending.as_ref().is_some_and(|request| {
                    request.nonce == nonce
                        && request.key.as_ref()
                            == state.presence.as_ref().map(|presence| &presence.key)
                }) {
                    if self
                        .pending
                        .as_ref()
                        .is_some_and(|request| request.fallback)
                    {
                        self.use_assets = false;
                    }
                    self.pending = None;
                    self.retry = Duration::from_secs(1);
                }
            }
            Reply::Rejected {
                nonce,
                code,
                message,
            } => {
                eprintln!("[discord] RPC ERROR code={code:?}: {message:?}");
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|request| Some(&request.nonce) == nonce.as_ref())
                {
                    let request = self.pending.take();
                    if let Some(request) = request
                        && request.artwork
                        && !request.fallback
                        && request.key.is_some()
                        && request.key.as_ref()
                            == state.presence.as_ref().map(|presence| &presence.key)
                    {
                        self.publish(state.presence.as_ref(), false, true)?;
                    }
                    // Keep sent: a rejected key is retried only after change or reconnect.
                }
            }
            Reply::Other => {}
        }
        Ok(())
    }
    fn park(&mut self) {
        if self.connected
            && let Err(error) = self.rpc.clear()
        {
            eprintln!("[discord] clear: {error:?}");
        }
        self.rpc.close();
        self.connected = false;
        self.sent = None;
        self.pending = None;
        self.retry = Duration::from_secs(1);
        self.deadline = Instant::now();
    }
}
enum Wake {
    Changed,
    Input(Result<(), Error>),
    Retry,
}
fn run(rpc: impl Rpc, mut desired: watch::Receiver<Desired>, runtime: &tokio::runtime::Handle) {
    let mut worker = Worker {
        rpc,
        connected: false,
        sent: None,
        pending: None,
        use_assets: true,
        retry: Duration::from_secs(1),
        deadline: Instant::now(),
    };
    loop {
        let mut state = desired.borrow_and_update().clone();
        if state.shutdown || !state.enabled {
            worker.park();
            if state.shutdown {
                break;
            }
        } else {
            if !worker.connected && Instant::now() >= worker.deadline {
                match worker.rpc.connect() {
                    Ok(()) => {
                        worker.connected = true;
                        worker.sent = None;
                    }
                    Err(error) => worker.failed("connect/handshake", &error),
                }
                // Connection can take time. Always re-read the newest coalesced snapshot.
                if desired.has_changed().unwrap_or(true) {
                    continue;
                }
                state = desired.borrow().clone();
            }
            if worker.connected
                && state.presence.as_ref().map(|presence| &presence.key) != worker.sent.as_ref()
                && let Err(error) =
                    worker.publish(state.presence.as_ref(), worker.use_assets, false)
            {
                worker.failed("write Activity", &error);
            }
        }
        let wake = runtime.block_on(async {
            tokio::select! {
                biased;
                _ = desired.changed() => Wake::Changed,
                result = worker.rpc.incoming(), if worker.connected => Wake::Input(result),
                () = tokio::time::sleep_until(worker.deadline), if state.enabled && !worker.connected => Wake::Retry,
            }
        });
        match wake {
            Wake::Changed if desired.has_changed().is_err() => break,
            Wake::Input(result) => {
                if let Err(error) = result.and_then(|()| worker.receive(&desired)) {
                    worker.failed("read/reply", &error);
                }
            }
            _ => {}
        }
    }
    worker.park();
}

#[cfg(test)]
mod tests;
