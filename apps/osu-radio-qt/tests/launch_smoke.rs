#![allow(clippy::expect_used, clippy::panic)]
// Ensure generated native registration links its Rust bridge into integration tests.
use osu_radio_qt as _;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn run(args: &[&str], platform: &str, directory: &Path, server: &Path, case: &str) -> String {
    let mut output = tempfile::tempfile().expect("captured output");
    let mut child = Command::new(env!("CARGO_BIN_EXE_osu-radio-qt"))
        .args(args)
        .current_dir(directory)
        .env("QT_QPA_PLATFORM", platform)
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("QT_LOGGING_RULES", "qml.info=true;qml.warning=true")
        .env("QT_QUICK_BACKEND", "software")
        .env("QSG_RHI_BACKEND", "software")
        .env("QML_DISABLE_DISK_CACHE", "1")
        .env("QT_QUICK_CONTROLS_STYLE", "org.kde.breeze")
        .env("OSU_RADIO_QT_SMOKE_TEST", "1")
        .env("OSU_RADIO_QT_PROBE_CASE", case)
        .env("OSU_RADIO_QT_PROBE_COVER", directory.join("source.png"))
        .env("OSU_RADIO_SERVER_BIN", server)
        .env_remove("SQLITE_DATABASE_URL")
        .env_remove("POSTGRES_DATABASE_URL")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .stdout(Stdio::from(output.try_clone().expect("stdout capture")))
        .stderr(Stdio::from(output.try_clone().expect("stderr capture")))
        .spawn()
        .expect("launch Qt binary");
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll Qt binary") {
            break status;
        }
        if start.elapsed() >= Duration::from_secs(20) {
            child.kill().expect("terminate timed out smoke test");
            child.wait().expect("reap timed out smoke test");
            panic!("Qt smoke test timed out: {}", read_output(&mut output));
        }
        thread::sleep(Duration::from_millis(20));
    };
    let output = read_output(&mut output);
    let expected_code = if args == ["--unknown"] { 2 } else { 0 };
    assert_eq!(
        status.code(),
        Some(expected_code),
        "{args:?}: {status}\n{output}"
    );
    output
}

fn read_output(file: &mut File) -> String {
    file.seek(SeekFrom::Start(0)).expect("rewind output");
    let mut output = String::new();
    file.read_to_string(&mut output).expect("read output");
    output
}

fn assert_probe(output: &str) {
    assert!(output.contains("Adapter probe passed"), "{output}");
    for error in [
        "Error:",
        "ReferenceError",
        "TypeError",
        "Cannot",
        "Unable",
        "Binding loop",
        "No such",
        "not found",
        "failed to load",
    ] {
        assert!(!output.contains(error), "QML diagnostic: {output}");
    }
}

#[test]
fn gallery_is_offline_and_its_adapter_notifies_only_on_changes() {
    let directory = tempfile::tempdir().expect("empty working directory");
    let output = run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "gallery",
    );
    assert_probe(&output);
    assert!(!directory.path().join("starts").exists());
}

#[cfg(unix)]
#[test]
fn cover_picker_uses_basic_controls_even_with_a_desktop_style_override() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated picker fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    let output = run(&[], "offscreen", directory.path(), &server, "native-picker");
    assert!(output.contains("Cover picker visible"), "{output}");
    assert!(
        output.contains("Visual probe finished: 0 issues"),
        "{output}"
    );
    for error in [
        "Binding loop",
        "TypeError",
        "ReferenceError",
        "VISUAL ISSUE",
    ] {
        assert!(!output.contains(error), "{output}");
    }
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn live_session_projects_independent_loads_targeted_media_and_id_selections() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated fixture directory");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    let output = run(&[], "offscreen", directory.path(), &server, "fixture");
    assert_probe(&output);
    assert_eq!(
        std::fs::read_to_string(directory.path().join("starts")).expect("startup count"),
        "2"
    );
    let requests =
        std::fs::read_to_string(directory.path().join("requests")).expect("HTTP requests");
    assert_eq!(
        requests
            .lines()
            .filter(|p| p.starts_with("/api/tracks"))
            .count(),
        5
    );
    assert_eq!(
        requests
            .lines()
            .filter(|p| *p == "/api/user-data/osu-folders")
            .count(),
        3
    );
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn songs_search_debounces_retries_and_clears_through_the_live_adapter() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated search fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    assert_probe(&run(&[], "offscreen", directory.path(), &server, "search"));
    let requests = std::fs::read_to_string(directory.path().join("requests")).expect("requests");
    assert_eq!(
        requests
            .lines()
            .filter(|line| line.starts_with("/api/tracks"))
            .collect::<Vec<_>>(),
        [
            "/api/tracks?q=",
            "/api/tracks?q=roc+hard",
            "/api/tracks?q=missing",
            "/api/tracks?q=retry",
            "/api/tracks?q=retry",
            "/api/tracks?q=",
        ]
    );
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn track_sort_menu_reorders_without_refetching_media_or_changing_selection() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated sorting fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    assert_probe(&run(&[], "offscreen", directory.path(), &server, "sorting"));
    let requests = std::fs::read_to_string(directory.path().join("requests")).expect("requests");
    assert_eq!(
        requests
            .lines()
            .filter(|line| line.starts_with("/api/tracks"))
            .count(),
        1
    );
    for id in [7, 42, 103] {
        assert_eq!(
            requests
                .lines()
                .filter(|line| *line == format!("/api/audio-sources/{id}/duration"))
                .count(),
            1
        );
        assert_eq!(
            requests
                .lines()
                .filter(|line| *line == format!("/api/beatmaps/{id}/cover"))
                .count(),
            1
        );
    }
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn queue_controls_preserve_pause_and_current_metadata_outside_search() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated queue fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    let output = run(&[], "offscreen", directory.path(), &server, "queue");
    assert_probe(&output);
    assert_child_reaped(directory.path());
    let commands = std::fs::read_to_string(directory.path().join("playback_commands"))
        .expect("queue commands");
    assert_eq!(commands.lines().count(), 4);
    assert!(commands.contains("next") && commands.contains("previous"));
}

#[cfg(unix)]
#[test]
fn playback_controls_show_loading_errors_and_global_volume_without_a_device() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated playback fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        &server,
        "playback",
    ));
    let requests = std::fs::read_to_string(directory.path().join("requests")).expect("requests");
    assert_eq!(
        requests
            .lines()
            .filter(|line| line.ends_with("/audio"))
            .collect::<Vec<_>>(),
        ["/api/audio-sources/42/audio"]
    );
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn closing_with_a_pending_http_request_awaits_child_cleanup() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated fixture directory");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    let output = run(&[], "offscreen", directory.path(), &server, "requests");
    assert_probe(&output);
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn folder_modal_stages_counts_applies_partial_success_and_retries_only_failures() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated folder fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    assert_probe(&run(&[], "offscreen", directory.path(), &server, "folders"));
    let requests = std::fs::read_to_string(directory.path().join("requests")).expect("requests");
    let actions: Vec<_> = requests
        .lines()
        .filter(|line| line.contains("/import ") || line.starts_with("DELETE"))
        .collect();
    assert_eq!(
        actions,
        [
            "POST /api/user-data/osu-folders/import /fixtures/101/osu!.db",
            "POST /api/user-data/osu-folders/import /fixtures/102/client.realm",
            "DELETE /api/user-data/osu-folders/31",
            "POST /api/user-data/osu-folders/import /fixtures/102/client.realm",
        ]
    );
    assert_eq!(
        std::fs::read_to_string(directory.path().join("metadata_max"))
            .expect("metadata concurrency"),
        "2"
    );
    assert_child_reaped(directory.path());
}

#[cfg(target_os = "linux")]
fn assert_child_reaped(directory: &Path) {
    let pid = std::fs::read_to_string(directory.join("child.pid")).expect("fixture child pid");
    assert!(
        !Path::new("/proc").join(pid).exists(),
        "launcher returned before reaping its child"
    );
}
#[cfg(all(unix, not(target_os = "linux")))]
fn assert_child_reaped(_directory: &Path) {}

#[test]
#[ignore = "needs cargo build -p osu-radio-server --locked (SQLite)"]
fn real_backend_uses_a_disposable_database() {
    let directory = tempfile::tempdir().expect("isolated backend directory");
    std::fs::write(
        directory.path().join(".env"),
        "SQLITE_DATABASE_URL=library.sqlite\n",
    )
    .expect("isolated env");
    let server = Path::new(env!("CARGO_BIN_EXE_osu-radio-qt"))
        .with_file_name(format!("osu-radio-server{}", std::env::consts::EXE_SUFFIX));
    assert!(
        server.is_file(),
        "build the server before running this test"
    );
    let output = run(&[], "offscreen", directory.path(), &server, "empty");
    assert_probe(&output);
    assert!(directory.path().join("library.sqlite").is_file());
}

#[cfg(unix)]
#[test]
#[ignore = "needs cargo build -p osu-radio-server --locked (SQLite)"]
fn real_backend_playlists_use_a_disposable_database() {
    let directory = tempfile::tempdir().expect("isolated playlist backend directory");
    std::fs::write(
        directory.path().join(".env"),
        "SQLITE_DATABASE_URL=library.sqlite\n",
    )
    .expect("isolated env");
    let server = Path::new(env!("CARGO_BIN_EXE_osu-radio-qt")).with_file_name("osu-radio-server");
    assert!(
        server.is_file(),
        "build the server before running this test"
    );
    assert_probe(&run(&[], "offscreen", directory.path(), &server, "empty"));
    // Test-only SQL after the real server creates its versioned schema. No application DB or files.
    let seeded = Command::new("python3").args(["-c", r"
import hashlib, json, sqlite3
with sqlite3.connect('library.sqlite') as db:
    db.execute('PRAGMA foreign_keys=ON')
    db.execute('INSERT INTO osu_installations (id,user_data_id,kind,root_path,marker_path) VALUES (1,1,?,?,?)', ('stable',json.dumps('/test/osu'),json.dumps('/test/osu/osu!.db')))
    db.execute('INSERT INTO beatmap_sets (id,installation_id) VALUES (1,1)')
    metadata = ['radio-db:metadata:v2','Test song',None,'Test artist',None,None,None,None,'audio.mp3',None]
    key = hashlib.sha256(json.dumps(metadata,separators=(',',':')).encode()).hexdigest()
    db.execute('INSERT INTO beatmap_metadata (hash,title,artist,audio_file) VALUES (?,?,?,?)',(key,'Test song','Test artist','audio.mp3'))
    db.execute('INSERT INTO audio_sources (id,kind,location) VALUES (1,?,?)',('local','/test/missing-audio'))
    for identifier,name in [(1,'Easy'),(2,'Hard')]:
        db.execute('INSERT INTO beatmaps (id,beatmap_set_id,hash,difficulty_name,metadata_hash,audio_source_id) VALUES (?,1,?,?,?,1)',(identifier,'test-'+name,name,key))
    db.execute('INSERT INTO playlists (id,name) VALUES (1,?)',('Unavailable',))
    db.execute('INSERT INTO playlist_items (playlist_id,source_kind,beatmap_hash,title,artist,difficulty_name) VALUES (1,?,?,?,?,?)',('stable','missing','Ghost','Artist','Missing'))
"]).current_dir(directory.path()).output().expect("seed disposable playlist fixture");
    assert!(
        seeded.status.success(),
        "{}",
        String::from_utf8_lossy(&seeded.stderr)
    );
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        &server,
        "playlists",
    ));
}

#[test]
fn help_and_invalid_arguments_do_not_initialize_a_gui() {
    let directory = tempfile::tempdir().expect("empty directory");
    let server = Path::new("/no/server/allowed");
    let output = run(
        &["--help"],
        "intentionally-nonexistent-platform",
        directory.path(),
        server,
        "none",
    );
    assert!(output.contains("Usage: osu-radio-qt"));
    assert!(!output.contains("Adapter probe"));
    let output = run(
        &["--unknown"],
        "intentionally-nonexistent-platform",
        directory.path(),
        server,
        "none",
    );
    assert!(output.contains("Unknown or repeated argument"));
}

#[cfg(unix)]
#[test]
fn individual_volume_controls_retry_preserve_selection_and_flush_at_close() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated volume fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    assert_probe(&run(&[], "offscreen", directory.path(), &server, "volume"));
    let settings: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.path().join("audio-settings.json"))
            .expect("saved settings"),
    )
    .expect("settings JSON");
    assert_eq!(
        settings
            .get("global_volume_percent")
            .and_then(serde_json::Value::as_u64),
        Some(55)
    );
    assert_eq!(
        settings
            .get("individual_volume_enabled")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    let volumes: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.path().join("audio-volumes.json"))
            .expect("saved volumes"),
    )
    .expect("volume JSON");
    assert_eq!(
        volumes.get("42").and_then(serde_json::Value::as_u64),
        Some(10)
    );
    assert!(volumes.get("7").is_none());
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn playlist_cover_failure_retries_created_id_and_reset_keeps_auto_cover() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated playlist cover fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        &server,
        "playlist-covers",
    ));
    let requests = std::fs::read_to_string(directory.path().join("requests")).expect("requests");
    assert_eq!(
        requests
            .lines()
            .filter(|line| *line == "POST /api/playlists ")
            .count(),
        1
    );
    assert_eq!(
        requests
            .lines()
            .filter(|line| line.starts_with("PUT /api/playlists/1/cover"))
            .count(),
        2
    );
    assert_eq!(
        requests
            .lines()
            .filter(|line| *line == "DELETE /api/playlists/1/cover")
            .count(),
        1
    );
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn playlist_editor_preserves_cached_covers_and_previews_automatic_cover_before_save() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated playlist preview fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        &server,
        "playlist-cover-preview",
    ));
    let requests = std::fs::read_to_string(directory.path().join("requests")).expect("requests");
    assert_eq!(
        requests
            .lines()
            .filter(|line| *line == "DELETE /api/playlists/1/cover")
            .count(),
        1,
        "cancelled automatic cover preview never writes to the server"
    );
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn warmed_navigation_fast_scrolling_and_cache_pressure_keep_displayed_artwork_ready() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated flicker fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    let output = run(
        &[],
        "offscreen",
        directory.path(),
        &server,
        "visual-flicker",
    );
    for marker in [
        "Visual probe finished: 0 issues",
        "flicker early artwork ready before duration",
        "flicker stopped viewport ready",
        "flicker stationary pressure settled without empty selected source",
    ] {
        assert!(output.contains(marker), "missing {marker}: {output}");
    }
    for error in [
        "Binding loop",
        "TypeError",
        "ReferenceError",
        "VISUAL ISSUE",
    ] {
        assert!(!output.contains(error), "{output}");
    }
    let requests = std::fs::read_to_string(directory.path().join("requests")).expect("requests");
    for request in [
        "/api/tracks?q=",
        "/api/beatmaps/1/cover",
        "/api/audio-sources/1/duration",
        "/api/playlists/1/cover",
    ] {
        assert_eq!(
            requests.lines().filter(|line| *line == request).count(),
            1,
            "warmed navigation and selected pin reuse {request}: {requests}"
        );
    }
    assert!(
        !requests.lines().any(|line| line.ends_with("/audio")),
        "paused fixture must not start audio: {requests}"
    );
    let checkpoint = output
        .lines()
        .find_map(|line| {
            line.split_once("flicker pressure checkpoint ")
                .and_then(|(_, tail)| tail.split_whitespace().next())
                .and_then(|timestamp| timestamp.parse::<u64>().ok())
        })
        .expect("pressure checkpoint timestamp");
    let settled = output
        .lines()
        .find_map(|line| {
            line.split_once("flicker stationary pressure settled without empty selected source ")
                .and_then(|(_, timestamp)| timestamp.trim().parse::<u64>().ok())
        })
        .expect("pressure settlement timestamp");
    assert!(settled.saturating_sub(checkpoint) >= 500, "{output}");
    let events =
        std::fs::read_to_string(directory.path().join("media-events")).expect("media events");
    for event in events.lines() {
        let mut fields = event.split_whitespace();
        let timestamp = fields
            .next()
            .expect("event time")
            .parse::<u64>()
            .expect("numeric event time");
        assert!(
            timestamp < checkpoint || timestamp >= settled,
            "stationary cache pressure refetches media: {event}\n{output}"
        );
    }
    assert_child_reaped(directory.path());
}
