#![allow(clippy::panic_in_result_fn, clippy::float_cmp)] // Protocol expectations use exact values.

use mpris_server::{
    Metadata, TrackId,
    zbus::{
        self, Connection, Proxy,
        export::futures_core::Stream,
        proxy::{Builder, CacheProperties},
    },
};
use osu_radio_media_controls::{
    Artwork, ArtworkKey, Capabilities, Event, EventReceiver, LaunchKey, MediaControls,
    Metadata as Text, NativeContext, PlaybackStatus, Snapshot, Status,
};
use std::{
    error::Error,
    future::{Future, poll_fn},
    io::{BufRead, BufReader, Write},
    pin::Pin,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};
use tokio::time::{sleep, timeout};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const LIMIT: Duration = Duration::from_secs(5);
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// Every probe spawns its own daemon and a child test process with a replaced bus address.
// The desktop bus is never used, even when the suite is invoked without dbus-run-session.
fn child(case: &str) -> Result<bool> {
    if std::env::var("OSU_RADIO_MPRIS_PROBE").ok().as_deref() == Some(case) {
        let root = std::env::var("OSU_RADIO_MPRIS_PROBE_ROOT")?;
        let address = std::env::var("DBUS_SESSION_BUS_ADDRESS")?;
        if !address.starts_with(&format!("unix:path={root}/")) {
            return Err("probe bus is not isolated".into());
        }
        return Ok(true);
    }
    let fixture = tempfile::Builder::new()
        .prefix("osu-mpris-")
        .tempdir_in("/tmp")?;
    let root = fixture.path().to_str().ok_or("non-UTF8 fixture path")?;
    let address = format!("unix:path={root}/bus");
    let mut daemon = Process(
        Command::new("dbus-daemon")
            .args([
                "--session",
                "--nofork",
                "--print-address=1",
                &format!("--address={address}"),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    let mut printed = String::new();
    BufReader::new(daemon.0.stdout.take().ok_or("daemon stdout")?).read_line(&mut printed)?;
    if !printed.trim().starts_with(&address) {
        return Err("unexpected daemon address".into());
    }
    let bus_address = if case == "unavailable_bus" {
        format!("unix:path={root}/missing")
    } else {
        printed.trim().into()
    };
    let mut process = Process(
        Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                &format!("protocol::{case}"),
                "--ignored",
                "--nocapture",
            ])
            .env("OSU_RADIO_MPRIS_PROBE", case)
            .env("OSU_RADIO_MPRIS_PROBE_ROOT", root)
            .env("OSU_RADIO_MPRIS_PROBE_PID", daemon.0.id().to_string())
            .env("DBUS_SESSION_BUS_ADDRESS", bus_address)
            .env("TMPDIR", root)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    let output = process.0.stdout.take().ok_or("probe stdout")?;
    for line in BufReader::new(output).lines() {
        let line = line?;
        if line.contains("OSU_RADIO_TEST_DISCONNECT_BUS") {
            daemon.0.kill()?;
        }
        println!("{line}");
    }
    if !process.0.wait()?.success() {
        return Err("isolated protocol child failed".into());
    }
    Ok(false)
}

async fn bounded<T>(future: impl Future<Output = T>) -> Result<T> {
    Ok(timeout(LIMIT, future).await?)
}

async fn connected(controls: &MediaControls) -> Result {
    let mut status = controls.status();
    bounded(async {
        loop {
            let current = status.borrow_and_update().clone();
            match current {
                Status::Connected => return Ok(()),
                Status::Failed(error) => return Err(error.into()),
                _ => status.changed().await?,
            }
        }
    })
    .await?
}

async fn failed(controls: &MediaControls) -> Result {
    let mut status = controls.status();
    bounded(async {
        loop {
            if matches!(*status.borrow_and_update(), Status::Failed(_)) {
                return Ok(());
            }
            status.changed().await?;
        }
    })
    .await?
}

async fn observer() -> Result<Connection> {
    let connection = bounded(Connection::session()).await??;
    let dbus = zbus::fdo::DBusProxy::new(&connection).await?;
    let actual = dbus
        .get_connection_unix_process_id("org.freedesktop.DBus".try_into()?)
        .await?;
    let expected: u32 = std::env::var("OSU_RADIO_MPRIS_PROBE_PID")?.parse()?;
    assert_eq!(actual, expected);
    Ok(connection)
}

async fn names(connection: &Connection) -> Result<Vec<String>> {
    Ok(zbus::fdo::DBusProxy::new(connection)
        .await?
        .list_names()
        .await?
        .into_iter()
        .map(|name| name.to_string())
        .filter(|name| name.starts_with("org.mpris.MediaPlayer2.osu_radio."))
        .collect())
}

async fn proxy<'a>(
    connection: &'a Connection,
    name: &'a str,
    interface: &'a str,
) -> Result<Proxy<'a>> {
    Ok(Builder::new(connection)
        .destination(name)?
        .path(PATH)?
        .interface(interface)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?)
}

async fn event(events: &mut EventReceiver, expected: Event) -> Result {
    assert_eq!(bounded(events.recv()).await?, Some(expected));
    Ok(())
}

async fn metadata(player: &Proxy<'_>) -> Result<Metadata> {
    let values: std::collections::HashMap<String, zbus::zvariant::OwnedValue> =
        player.get_property("Metadata").await?;
    let mut metadata = Metadata::new();
    for (key, value) in values {
        metadata.set_value(&key, Some(value.into()));
    }
    Ok(metadata)
}

fn initial() -> Snapshot {
    let launch = LaunchKey {
        entry_id: 7,
        playback_token: 1,
    };
    Snapshot {
        launch: Some(launch),
        metadata: Some(Text {
            title: Some("曲 A".into()),
            artist: Some("Artist A".into()),
        }),
        duration: Some(Duration::from_secs(10)),
        status: PlaybackStatus::Playing,
        position: Duration::from_secs(3),
        volume: 0.4,
        capabilities: Capabilities {
            play: true,
            pause: true,
            stop: true,
            next: true,
            previous: true,
            seek: true,
            set_volume: true,
        },
        artwork: Some(Artwork {
            key: ArtworkKey {
                launch,
                cover_id: 1,
                generation: 1,
            },
            bytes: Some(Arc::from(&b"cover"[..])),
        }),
        ..Snapshot::default()
    }
}

async fn next_signal<S: Stream + Unpin>(stream: &mut S) -> Result<S::Item> {
    bounded(poll_fn(|context| Pin::new(&mut *stream).poll_next(context)))
        .await?
        .ok_or_else(|| "signal stream closed".into())
}

async fn silent<S: Stream + Unpin>(stream: &mut S) {
    assert!(
        timeout(
            Duration::from_millis(120),
            poll_fn(|context| Pin::new(&mut *stream).poll_next(context))
        )
        .await
        .is_err()
    );
}

#[tokio::test]
#[ignore = "requires Linux and a private dbus-daemon; never uses the desktop bus"]
#[allow(clippy::too_many_lines)] // One ordered trace checks protocol changes and signal absence.
async fn protocol_exchange() -> Result {
    if !child("protocol_exchange")? {
        return Ok(());
    }
    let connection = observer().await?;
    let a = initial();
    let launch = a.launch.ok_or("fixture launch")?;
    let (mut controls, mut events) = MediaControls::connect(NativeContext::Linux, a.clone())?;
    connected(&controls).await?;
    let services = names(&connection).await?;
    assert_eq!(services.len(), 1);
    let name = services.first().ok_or("MPRIS name")?;
    let player = proxy(&connection, name, PLAYER).await?;
    let root = proxy(&connection, name, "org.mpris.MediaPlayer2").await?;
    assert_eq!(root.get_property::<String>("Identity").await?, "osu! radio");
    assert!(!root.get_property::<bool>("HasTrackList").await?);
    let introspection = proxy(&connection, name, "org.freedesktop.DBus.Introspectable").await?;
    let xml: String = introspection.call("Introspect", &()).await?;
    assert!(xml.contains(PLAYER));
    assert!(!xml.contains("interface name=\"org.mpris.MediaPlayer2.TrackList\""));
    assert!(!xml.contains("interface name=\"org.mpris.MediaPlayer2.Playlists\""));
    let metadata = metadata(&player).await?;
    assert_eq!(metadata.title(), Some("曲 A"));
    assert_eq!(metadata.artist(), Some(vec!["Artist A".into()]));
    assert_eq!(
        metadata.length().map(|length| length.as_micros()),
        Some(10_000_000)
    );
    let id = metadata.trackid().ok_or("track id")?;
    let cover = metadata.art_url().ok_or("cover URI")?;
    let cover_path = cover.strip_prefix("file://").ok_or("file cover")?;
    assert!(std::path::Path::new(cover_path).exists());
    assert_eq!(
        player.get_property::<String>("PlaybackStatus").await?,
        "Playing"
    );
    assert_eq!(player.get_property::<i64>("Position").await?, 3_000_000);
    assert_eq!(player.get_property::<f64>("Volume").await?, 0.4);
    assert!(player.get_property::<bool>("CanSeek").await?);
    for (method, expected) in [
        ("Play", Event::Play),
        ("Pause", Event::Pause),
        ("PlayPause", Event::PlayPause),
        ("Stop", Event::Stop),
        ("Next", Event::Next),
        ("Previous", Event::Previous),
    ] {
        player.call::<_, _, ()>(method, &()).await?;
        event(&mut events, expected).await?;
    }
    for offset_micros in [-1_500_000_i64, i64::MIN, 7_000_001, i64::MAX] {
        player.call::<_, _, ()>("Seek", &(offset_micros,)).await?;
        event(
            &mut events,
            Event::SeekRelative {
                launch,
                offset_micros,
            },
        )
        .await?;
    }
    player
        .call::<_, _, ()>("SetPosition", &(id.clone(), 2_000_000_i64))
        .await?;
    event(
        &mut events,
        Event::SeekAbsolute {
            launch,
            position: Duration::from_secs(2),
        },
    )
    .await?;
    player.set_property("Volume", 0.7_f64).await?;
    event(&mut events, Event::SetVolume(0.7)).await?;
    for (id, position) in [
        (TrackId::NO_TRACK, 0_i64),
        (id.clone(), -1),
        (id.clone(), 10_000_001),
    ] {
        player
            .call::<_, _, ()>("SetPosition", &(id, position))
            .await?;
    }
    player.call::<_, _, ()>("Play", &()).await?;
    event(&mut events, Event::Play).await?;

    let properties = proxy(&connection, name, "org.freedesktop.DBus.Properties").await?;
    let mut changed = properties.receive_signal("PropertiesChanged").await?;
    let mut seeked = player.receive_signal("Seeked").await?;
    let mut tick = a.clone();
    tick.position = Duration::from_secs(4);
    assert!(controls.publish(tick.clone()));
    bounded(async {
        while player.get_property::<i64>("Position").await? != 4_000_000 {
            sleep(Duration::from_millis(5)).await;
        }
        Ok::<_, zbus::Error>(())
    })
    .await??;
    silent(&mut changed).await;
    silent(&mut seeked).await;
    tick.position = Duration::from_secs(2);
    tick.seek_serial = 1;
    assert!(controls.publish(tick.clone()));
    let signal = next_signal(&mut seeked).await?;
    assert_eq!(signal.body().deserialize::<(i64,)>()?, (2_000_000,));
    silent(&mut changed).await;
    assert!(controls.publish(tick.clone()));
    silent(&mut seeked).await;

    let mut b = tick;
    b.launch = Some(LaunchKey {
        playback_token: 2,
        ..launch
    });
    b.metadata = None;
    b.duration = None;
    b.artwork = None;
    b.status = PlaybackStatus::Paused;
    b.volume = 0.8;
    b.capabilities.next = false;
    b.capabilities.seek = false;
    assert!(controls.publish(b.clone()));
    let signal = next_signal(&mut changed).await?;
    let (interface, values, invalidated): (
        String,
        std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        Vec<String>,
    ) = signal.body().deserialize()?;
    assert_eq!(interface, PLAYER);
    assert!(invalidated.is_empty());
    for key in [
        "Metadata",
        "PlaybackStatus",
        "Volume",
        "CanGoNext",
        "CanSeek",
    ] {
        assert!(values.contains_key(key));
    }
    assert!(!values.contains_key("Position"));
    assert_eq!(
        <&str>::try_from(values.get("PlaybackStatus").ok_or("status signal")?)?,
        "Paused"
    );
    assert_eq!(
        f64::try_from(values.get("Volume").ok_or("volume signal")?)?,
        0.8
    );
    assert!(!bool::try_from(
        values.get("CanGoNext").ok_or("next signal")?
    )?);
    assert!(!bool::try_from(
        values.get("CanSeek").ok_or("seek signal")?
    )?);
    let metadata_b = self::metadata(&player).await?;
    assert_ne!(metadata_b.trackid(), Some(id.clone()));
    assert!(metadata_b.title().is_none());
    assert!(metadata_b.artist().is_none());
    assert!(metadata_b.length().is_none());
    assert!(metadata_b.art_url().is_none());
    assert!(!std::path::Path::new(cover_path).exists());
    silent(&mut seeked).await;
    b.capabilities.seek = true;
    assert!(controls.publish(b.clone()));
    let _ = next_signal(&mut changed).await?;
    player
        .call::<_, _, ()>("SetPosition", &(id, 1_000_000_i64))
        .await?;
    player.call::<_, _, ()>("Play", &()).await?;
    event(&mut events, Event::Play).await?;

    // A failed cover leaves the current launch, metadata and transport intact.
    b.artwork = Some(Artwork {
        key: ArtworkKey {
            launch: b.launch.ok_or("launch B")?,
            cover_id: 2,
            generation: 1,
        },
        bytes: Some(Arc::from(&b""[..])),
    });
    b.metadata = Some(Text {
        title: Some("B after failed cover".into()),
        artist: None,
    });
    assert!(controls.publish(b));
    let _ = next_signal(&mut changed).await?;
    let after_failure = self::metadata(&player).await?;
    assert_eq!(after_failure.title(), Some("B after failed cover"));
    assert!(after_failure.art_url().is_none());
    player.call::<_, _, ()>("Pause", &()).await?;
    event(&mut events, Event::Pause).await?;
    assert!(matches!(*controls.status().borrow(), Status::Connected));

    let (mut second, _) = MediaControls::connect(NativeContext::Linux, Snapshot::default())?;
    connected(&second).await?;
    assert_eq!(names(&connection).await?.len(), 2);
    bounded(second.shutdown()).await?;
    bounded(controls.shutdown()).await?;
    bounded(controls.shutdown()).await?;
    assert!(names(&connection).await?.is_empty());
    assert!(player.call::<_, _, ()>("Play", &()).await.is_err());
    assert_eq!(events.recv().await, None);
    Ok(())
}

#[tokio::test]
#[ignore = "requires Linux and a private dbus-daemon; never uses the desktop bus"]
async fn rate_changes_keep_source_time_and_launch_bound_commands() -> Result {
    if !child("rate_changes_keep_source_time_and_launch_bound_commands")? {
        return Ok(());
    }
    let connection = observer().await?;
    let mut snapshot = initial();
    let launch = snapshot.launch.ok_or("fixture launch")?;
    let (mut controls, mut events) =
        MediaControls::connect(NativeContext::Linux, snapshot.clone())?;
    connected(&controls).await?;
    let services = names(&connection).await?;
    let name = services.first().ok_or("MPRIS name")?;
    let player = proxy(&connection, name, PLAYER).await?;
    assert_eq!(player.get_property::<f64>("Rate").await?, 1.0);
    assert_eq!(player.get_property::<f64>("MinimumRate").await?, 0.5);
    assert_eq!(player.get_property::<f64>("MaximumRate").await?, 2.0);
    let original_metadata = metadata(&player).await?;
    let id = original_metadata.trackid().ok_or("track id")?;
    let properties = proxy(&connection, name, "org.freedesktop.DBus.Properties").await?;
    let mut changed = properties.receive_signal("PropertiesChanged").await?;
    let mut seeked = player.receive_signal("Seeked").await?;
    for status in [PlaybackStatus::Playing, PlaybackStatus::Paused] {
        for rate in [0.75, 1.2, 1.5] {
            snapshot.status = status;
            snapshot.rate = rate;
            assert!(controls.publish(snapshot.clone()));
            let signal = next_signal(&mut changed).await?;
            let (interface, values, invalidated): (
                String,
                std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
                Vec<String>,
            ) = signal.body().deserialize()?;
            assert_eq!(interface, PLAYER);
            assert!(invalidated.is_empty());
            assert_eq!(
                f64::try_from(values.get("Rate").ok_or("missing Rate change")?)?,
                rate
            );
            assert!(!values.contains_key("Metadata"));
            assert!(!values.contains_key("Position"));
            assert_eq!(player.get_property::<f64>("Rate").await?, rate);
            assert_eq!(metadata(&player).await?, original_metadata);
            assert_eq!(player.get_property::<i64>("Position").await?, 3_000_000);
            assert_eq!(
                player.get_property::<String>("PlaybackStatus").await?,
                if status == PlaybackStatus::Playing {
                    "Playing"
                } else {
                    "Paused"
                }
            );
            player
                .call::<_, _, ()>("SetPosition", &(id.clone(), 2_000_000_i64))
                .await?;
            event(
                &mut events,
                Event::SeekAbsolute {
                    launch,
                    position: Duration::from_secs(2),
                },
            )
            .await?;
            player.call::<_, _, ()>("Seek", &(-1_500_000_i64,)).await?;
            event(
                &mut events,
                Event::SeekRelative {
                    launch,
                    offset_micros: -1_500_000,
                },
            )
            .await?;
            assert!(player.set_property("Rate", 1.0_f64).await.is_err());
            // A sentinel proves that OS rate writes added no command before it.
            player.call::<_, _, ()>("Play", &()).await?;
            event(&mut events, Event::Play).await?;
            assert_eq!(player.get_property::<f64>("Rate").await?, rate);
        }
    }
    for rate in [0.0, 0.49, 2.01, f64::NAN, f64::INFINITY] {
        assert!(player.set_property("Rate", rate).await.is_err());
    }
    let rate = snapshot.rate;
    assert!(controls.publish(snapshot));
    silent(&mut changed).await;
    silent(&mut seeked).await;
    // zbus emits a property notification after a successful no-op setter too.
    player.set_property("Rate", rate).await?;
    player.call::<_, _, ()>("Play", &()).await?;
    event(&mut events, Event::Play).await?;
    bounded(controls.shutdown()).await?;
    assert!(names(&connection).await?.is_empty());
    Ok(())
}

#[tokio::test]
#[ignore = "requires Linux and a private dbus-daemon; never uses the desktop bus"]
async fn invalid_rate_fails_publication_and_acknowledges_cleanup() -> Result {
    if !child("invalid_rate_fails_publication_and_acknowledges_cleanup")? {
        return Ok(());
    }
    let connection = observer().await?;
    for initial_invalid in [true, false] {
        let mut snapshot = initial();
        if initial_invalid {
            snapshot.rate = 0.0;
        }
        let (mut controls, mut events) =
            MediaControls::connect(NativeContext::Linux, snapshot.clone())?;
        if !initial_invalid {
            connected(&controls).await?;
            snapshot.rate = f64::NAN;
            assert!(controls.publish(snapshot));
        }
        failed(&controls).await?;
        assert!(
            matches!(&*controls.status().borrow(), Status::Failed(error) if error.to_string().contains("rate"))
        );
        assert!(!controls.publish(initial()));
        assert_eq!(bounded(events.recv()).await?, None);
        bounded(controls.shutdown()).await?;
        assert!(names(&connection).await?.is_empty());
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires Linux and a private dbus-daemon; kills only its own daemon"]
async fn fatal_disconnect() -> Result {
    if !child("fatal_disconnect")? {
        return Ok(());
    }
    let _connection = observer().await?;
    let (mut controls, mut events) = MediaControls::connect(NativeContext::Linux, initial())?;
    connected(&controls).await?;
    println!("OSU_RADIO_TEST_DISCONNECT_BUS");
    std::io::stdout().flush()?;
    failed(&controls).await?;
    assert!(!controls.publish(initial()));
    assert_eq!(bounded(events.recv()).await?, None);
    bounded(controls.shutdown()).await?;
    sleep(Duration::from_millis(150)).await;
    assert!(matches!(*controls.status().borrow(), Status::Failed(_)));
    Ok(())
}

#[tokio::test]
#[ignore = "isolated child process with an unavailable session bus"]
async fn unavailable_bus() -> Result {
    if !child("unavailable_bus")? {
        return Ok(());
    }
    let (mut controls, mut events) = MediaControls::connect(NativeContext::Linux, initial())?;
    failed(&controls).await?;
    assert_eq!(bounded(events.recv()).await?, None);
    assert!(!controls.publish(initial()));
    bounded(controls.shutdown()).await?;
    bounded(controls.shutdown()).await?;
    sleep(Duration::from_millis(150)).await;
    assert!(matches!(*controls.status().borrow(), Status::Failed(_)));
    Ok(())
}
