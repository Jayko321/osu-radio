use crate::{Backend, Error, Event, EventSender, Snapshot};
use mpris_server::{Server, Signal};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::watch;

mod conversion;
mod player;
mod transform;

use conversion::confirmed_seek;
use player::Player;
use transform::{Published, changes};

static INSTANCE: AtomicU64 = AtomicU64::new(0);

pub(crate) async fn run(backend: &mut Backend) -> Result<(), Error> {
    if *backend.stop.borrow() {
        return Ok(());
    }
    let instance = format!(
        "instance{}_{}",
        std::process::id(),
        INSTANCE.fetch_add(1, Ordering::Relaxed)
    );
    let initial = backend.snapshots.borrow_and_update().clone();
    let mut previous = publish(backend, initial, &instance)?;
    let (published, receiver) = watch::channel(previous.clone());
    let player = Player {
        instance: instance.clone(),
        snapshots: backend.snapshots.clone(),
        published: receiver,
        events: backend.events.clone(),
    };
    let suffix = format!("osu_radio.{instance}");
    let server = tokio::select! {
        biased;
        _ = backend.stop.changed() => return Ok(()),
        result = Server::new(&suffix, player) => {
            result.map_err(native_error)?
        }
    };
    let connection = server.connection().clone();
    let result = if backend.connected() {
        async {
            loop {
                tokio::select! {
                    biased;
                    _ = backend.stop.changed() => break Ok(()),
                    () = connection.closed() => break Err(Error::new("MPRIS session bus disconnected")),
                    update = backend.snapshots.changed() => {
                        update.map_err(|error| Error::new(error.to_string()))?;
                        let snapshot = backend.snapshots.borrow_and_update().clone();
                        let current = publish(backend, snapshot, &instance)?;
                        let properties = changes(&previous, &current);
                        let seek = confirmed_seek(&previous.snapshot, &current.snapshot);
                        published.send_replace(current.clone());
                        let notify = async {
                            server.properties_changed(properties).await.map_err(native_error)?;
                            if let Some(position) = seek {
                                server.emit(Signal::Seeked { position: transform::time(position) }).await.map_err(native_error)?;
                            }
                            Ok::<(), Error>(())
                        };
                        tokio::select! {
                            biased;
                            _ = backend.stop.changed() => break Ok(()),
                            () = connection.closed() => break Err(Error::new("MPRIS session bus disconnected")),
                            result = notify => result?,
                        }
                        previous = current;
                    }
                }
            }
        }
        .await
    } else {
        Ok(())
    };
    if let Err(error) = &result {
        backend.fail(error.clone());
    }
    // Explicit close stops the object server as well as releasing the name.
    let released = server.release_bus_name().await.map_err(native_error);
    drop(server);
    let closed = connection.clone().close().await.map_err(native_error);
    connection.graceful_shutdown().await;
    result?;
    released?;
    closed
}

#[allow(clippy::needless_pass_by_value)] // Used directly as a Result::map_err callback.
fn native_error(error: mpris_server::zbus::Error) -> Error {
    Error::new(format!("MPRIS: {error}"))
}

fn publish(backend: &mut Backend, snapshot: Snapshot, instance: &str) -> Result<Published, Error> {
    if let Err(error) = backend.artwork.update(&snapshot) {
        eprintln!("MPRIS artwork: {error}");
    }
    Published::new(snapshot, backend.artwork.path(), instance)
}

fn send(events: &EventSender, event: Option<Event>) {
    if let Some(event) = event {
        let _ = events.send(event);
    }
}
