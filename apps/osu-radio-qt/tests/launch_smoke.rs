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
        .env("OSU_RADIO_QT_SMOKE_TEST", "1")
        .env("OSU_RADIO_QT_PROBE_CASE", case)
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
