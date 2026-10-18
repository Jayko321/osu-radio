use crate::{ArtworkStore, Error, Event, LaunchKey, NativeContext, Snapshot};
use tokio::sync::{mpsc, oneshot, watch};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Connected,
    Failed(Error),
    Disconnected,
}

/// A nonblocking publisher and an acknowledged lifetime, owned by the client controller.
pub struct MediaControls {
    snapshots: watch::Sender<Snapshot>,
    stop: watch::Sender<bool>,
    status: watch::Receiver<Status>,
    completed: Option<oneshot::Receiver<()>>,
}
impl MediaControls {
    /// Starts exactly one adapter on the existing Tokio runtime. Failure never retries.
    /// Native initialization is asynchronous; observe `status()` for its result.
    pub fn connect(
        context: NativeContext,
        initial: Snapshot,
    ) -> Result<(Self, EventReceiver), Error> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|error| Error::new(format!("media controls require Tokio: {error}")))?;
        let (controls, mut backend, events) = test_backend(initial);
        runtime.spawn(async move {
            if let Err(error) = crate::run_native(context, &mut backend).await {
                backend.fail(error);
            }
            // Backend drops after all native locals have been released.
        });
        Ok((controls, events))
    }

    /// Coalesces updates even if the native consumer is slow. False means disabled.
    #[must_use]
    pub fn publish(&self, snapshot: Snapshot) -> bool {
        if *self.stop.borrow() {
            return false;
        }
        self.snapshots.send_replace(snapshot);
        true
    }

    #[must_use]
    pub fn status(&self) -> watch::Receiver<Status> {
        self.status.clone()
    }

    /// Reject callbacks immediately, then wait for native cleanup and artwork release.
    /// Cancellation-safe and idempotent. Drop requests stop but cannot await cleanup.
    pub async fn shutdown(&mut self) {
        self.begin_shutdown();
        if let Some(completed) = &mut self.completed {
            let _ = completed.await;
            self.completed = None;
        }
    }

    fn begin_shutdown(&self) {
        self.stop.send_replace(true);
        self.snapshots.send_replace(Snapshot::default());
    }
}
impl Drop for MediaControls {
    fn drop(&mut self) {
        self.begin_shutdown();
    }
}

/// Callback-safe event sink; it owns no playback rules or native resources.
#[derive(Clone)]
pub struct EventSender {
    sender: mpsc::UnboundedSender<Event>,
    snapshots: watch::Receiver<Snapshot>,
    stop: watch::Receiver<bool>,
}
impl EventSender {
    /// Capture at callback receipt, before queueing a seek. MPRIS also validates `TrackId`.
    #[must_use]
    pub fn current_launch(&self) -> Option<LaunchKey> {
        self.snapshots.borrow().launch
    }

    #[must_use]
    pub fn send(&self, event: Event) -> bool {
        !*self.stop.borrow() && self.sender.send(event).is_ok()
    }
}

/// One ordered consumer for the controller event loop. Queued events are discarded on stop.
pub struct EventReceiver {
    receiver: mpsc::UnboundedReceiver<Event>,
    stop: watch::Receiver<bool>,
}
impl EventReceiver {
    pub async fn recv(&mut self) -> Option<Event> {
        if *self.stop.borrow() {
            return None;
        }
        tokio::select! {
            biased;
            _ = self.stop.changed() => None,
            event = self.receiver.recv() => {
                if *self.stop.borrow() { None } else { event }
            }
        }
    }
}

/// Adapter endpoint, also exposed for deterministic tests without OS/GUI dependencies.
///
/// Keep this owner alive until native handlers, session and child tasks are released.
/// Clone `events` into callbacks, never the lifetime owner itself.
/// Clone a snapshot and release its watch borrow before native I/O or an await.
pub struct Backend {
    pub snapshots: watch::Receiver<Snapshot>,
    pub stop: watch::Receiver<bool>,
    pub events: EventSender,
    pub artwork: ArtworkStore,
    stop_sender: watch::Sender<bool>,
    status: watch::Sender<Status>,
    completed: Option<oneshot::Sender<()>>,
}
impl Backend {
    /// Call only after native registration succeeds. False means shutdown already began.
    #[must_use]
    pub fn connected(&self) -> bool {
        if *self.stop.borrow() {
            return false;
        }
        self.status.send_replace(Status::Connected);
        true
    }

    /// Fatal integration error, never an individual artwork error. No automatic reconnect.
    pub fn fail(&self, error: Error) {
        self.status.send_replace(Status::Failed(error));
        self.stop_sender.send_replace(true);
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        if !*self.stop.borrow() {
            self.fail(Error::new("media adapter exited without shutdown"));
        }
        self.artwork.clear();
        if !matches!(*self.status.borrow(), Status::Failed(_)) {
            self.status.send_replace(Status::Disconnected);
        }
        if let Some(completed) = self.completed.take() {
            let _ = completed.send(());
        }
    }
}

/// Minimal test/native endpoint pair using the production channels and cleanup boundary.
/// Tests drive `Backend` directly; production callers use `MediaControls::connect`.
#[must_use]
pub fn test_backend(initial: Snapshot) -> (MediaControls, Backend, EventReceiver) {
    let (snapshots, snapshot_receiver) = watch::channel(initial);
    let (stop, stop_receiver) = watch::channel(false);
    let (status_sender, status) = watch::channel(Status::Connecting);
    let (completed_sender, completed) = oneshot::channel();
    let (event_sender, receiver) = mpsc::unbounded_channel();
    let events = EventSender {
        sender: event_sender,
        snapshots: snapshot_receiver.clone(),
        stop: stop_receiver.clone(),
    };
    (
        MediaControls {
            snapshots,
            stop: stop.clone(),
            status,
            completed: Some(completed),
        },
        Backend {
            snapshots: snapshot_receiver,
            stop: stop_receiver.clone(),
            events,
            artwork: ArtworkStore::default(),
            stop_sender: stop,
            status: status_sender,
            completed: Some(completed_sender),
        },
        EventReceiver {
            receiver,
            stop: stop_receiver,
        },
    )
}
