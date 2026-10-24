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
    run_with_server_url(args, platform, directory, server, case, None)
}

fn run_with_server_url(
    args: &[&str],
    platform: &str,
    directory: &Path,
    server: &Path,
    case: &str,
    server_url: Option<&str>,
) -> String {
    let mut output = tempfile::tempfile().expect("captured output");
    let mut command = Command::new(env!("CARGO_BIN_EXE_osu-radio-qt"));
    command
        .args(args)
        .current_dir(directory)
        .env("XDG_CONFIG_HOME", directory.join("config"))
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
        .env_remove("OSU_RADIO_SERVER_URL")
        .env("SQLITE_DATABASE_URL", directory.join("library.sqlite"))
        .env("XDG_DATA_HOME", directory.join("data"))
        .env_remove("POSTGRES_DATABASE_URL")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY");
    if let Some(url) = server_url {
        command.env("OSU_RADIO_SERVER_URL", url);
    }
    let mut child = command
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
    let expected_code = if args == ["--unknown"] {
        2
    } else {
        i32::from(case == "system-media-failed-launch")
    };
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
    // A missing D-Bus session disables integration, while the Qt probe still succeeds.
    let qml_output = output
        .lines()
        .filter(|line| !line.starts_with("System media controls:"))
        .collect::<Vec<_>>()
        .join("\n");
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
        assert!(!qml_output.contains(error), "QML diagnostic: {output}");
    }
}

#[cfg(unix)]
#[test]
fn external_server_connects_without_a_local_binary_and_survives_qt_shutdown() {
    use osu_radio_client::{ApiClient, EmbeddedServer, ServerOptions};
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir().expect("isolated external server");
    let fixture = directory.path().join("fixture-server");
    std::fs::write(&fixture, format!(
        "#!/usr/bin/env python3\nimport os\nos.environ['OSU_RADIO_QT_PROBE_CASE'] = 'search'\n{}",
        include_str!("fixtures/server.py"),
    )).expect("write fixture");
    std::fs::set_permissions(&fixture, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    let runtime = tokio::runtime::Runtime::new().expect("external server runtime");
    let server = runtime
        .block_on(EmbeddedServer::start(ServerOptions {
            binary: Some(fixture),
            working_directory: Some(directory.path().to_owned()),
            ..Default::default()
        }))
        .expect("start external fixture");
    assert_probe(&run_with_server_url(
        &[],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "search",
        Some(server.base_url()),
    ));
    assert_eq!(
        std::fs::read_to_string(directory.path().join("starts")).expect("startup count"),
        "1"
    );
    let api = ApiClient::new(server.base_url()).expect("external API");
    runtime
        .block_on(api.queue())
        .expect("external server still responds after Qt exits");
    runtime
        .block_on(server.shutdown())
        .expect("stop external fixture");
    assert_child_reaped(directory.path());
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
    assert!(!output.contains("System media Qt attach:"), "{output}");
    assert!(!directory.path().join("starts").exists());
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires a Windows desktop and the native windows Qt plugin"]
fn native_system_media_detaches_before_hwnd_destruction_on_all_exit_paths() {
    use tokio::io::AsyncBufReadExt;
    let runtime = tokio::runtime::Runtime::new().expect("fixture runtime");
    for case in [
        "system-media-close",
        "system-media-quit",
        "system-media-failed-launch",
    ] {
        let directory = tempfile::tempdir().expect("isolated native lifecycle probe");
        let (mut fixture, url) = runtime.block_on(async {
            let mut fixture = tokio::process::Command::new("python")
                .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/server.py"))
                .current_dir(directory.path())
                .env("OSU_RADIO_QT_PROBE_CASE", "search")
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .expect("start external Python fixture");
            let mut lines =
                tokio::io::BufReader::new(fixture.stdout.take().expect("readiness pipe")).lines();
            let ready = tokio::time::timeout(Duration::from_secs(5), lines.next_line())
                .await
                .expect("fixture readiness deadline")
                .expect("read readiness")
                .expect("ready line");
            let url = ready
                .strip_prefix("listening on ")
                .expect("fixture URL")
                .to_owned();
            (fixture, url)
        });
        let output = run_with_server_url(
            &[],
            "windows",
            directory.path(),
            &directory.path().join("no-local-server.exe"),
            case,
            Some(&url),
        );
        assert!(
            output.contains("System media Qt attach: \"windows\""),
            "{output}"
        );
        if case != "system-media-failed-launch" {
            assert!(
                output.contains("System media Qt native publication observed"),
                "{output}"
            );
        }
        let ack = output
            .find("System media Qt detach acknowledged; window alive")
            .expect(&output);
        let destroyed = output
            .find("System media Qt window destroyed after detach")
            .expect(&output);
        assert!(ack < destroyed, "{output}");
        assert!(!output.contains("System media controls:"), "{output}");
        assert_eq!(
            std::fs::read_to_string(directory.path().join("starts")).expect("fixture starts"),
            "1"
        );
        runtime
            .block_on(
                osu_radio_client::ApiClient::new(&url)
                    .expect("external API")
                    .queue(),
            )
            .expect("external fixture survives Qt shutdown");
        runtime
            .block_on(fixture.kill())
            .expect("stop and reap external fixture");
        eprintln!("{case}:\n{output}");
    }
}

#[test]
fn gallery_queue_uses_the_shared_virtualized_panel_without_a_server() {
    let directory = tempfile::tempdir().expect("isolated gallery queue");
    assert_probe(&run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "gallery-queue",
    ));
    assert!(!directory.path().join("starts").exists());
}

#[test]
fn gallery_insertion_menus_share_typed_actions_without_a_server() {
    let directory = tempfile::tempdir().expect("isolated gallery insertions");
    assert_probe(&run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "gallery-insert",
    ));
    assert!(!directory.path().join("starts").exists());
}

#[test]
fn offline_controls_handle_keyboard_disabled_states_and_presentation_bindings() {
    let directory = tempfile::tempdir().expect("isolated controls probe");
    let output = run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "controls",
    );
    assert_probe(&output);
    assert!(
        output.contains("Adapter probe passed: controls"),
        "{output}"
    );
    assert!(!directory.path().join("starts").exists());
}

#[test]
fn offline_menus_and_modals_preserve_value_roles_focus_and_dismissal_rules() {
    let directory = tempfile::tempdir().expect("isolated popup probe");
    let output = run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "controls-popups",
    );
    assert_probe(&output);
    assert!(
        output.contains("Adapter probe passed: controls-popups"),
        "{output}"
    );
    assert!(!directory.path().join("starts").exists());
}

#[test]
fn offline_mods_popup_renders_state_and_emits_mouse_and_keyboard_intents() {
    let directory = tempfile::tempdir().expect("isolated mods popup probe");
    let output = run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "controls-mods",
    );
    assert_probe(&output);
    assert!(
        output.contains("Adapter probe passed: controls-mods"),
        "{output}"
    );
    assert!(!directory.path().join("starts").exists());
    assert!(!directory.path().join("library.sqlite").exists());
}

#[test]
fn offline_progress_drag_is_cancelled_when_launch_or_speed_changes() {
    let directory = tempfile::tempdir().expect("isolated progress probe");
    let output = run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "controls-progress",
    );
    assert_probe(&output);
    assert!(!directory.path().join("starts").exists());
}

#[test]
fn gallery_mods_popup_uses_the_shared_mock_without_a_server() {
    let directory = tempfile::tempdir().expect("isolated gallery mods");
    assert_probe(&run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "gallery-mods",
    ));
    assert!(!directory.path().join("starts").exists());
    assert!(!directory.path().join("library.sqlite").exists());
}

#[cfg(unix)]
#[test]
fn live_mods_controls_use_f1_retry_individual_mode_and_flush_at_close() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated mods fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    assert_probe(&run(&[], "offscreen", directory.path(), &server, "mods"));
    let saved: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(directory.path().join("audio-mods.json")).expect("saved mods"),
    )
    .expect("mods JSON");
    // The last edit (HT 0.75x for B) was still inside its debounce when the window closed.
    assert_eq!(
        saved.pointer("/tracks/42"),
        Some(&serde_json::json!({"mode": "ht", "speed_percent": 75, "adjust_pitch": false}))
    );
    assert_eq!(
        saved.pointer("/settings"),
        Some(&serde_json::json!({
            "individual_mods_enabled": true,
            "global_profile": {"mode": "none", "speed_percent": 100, "adjust_pitch": false}
        }))
    );
    assert!(saved.pointer("/tracks/7").is_none() && saved.pointer("/tracks/103").is_none());
    assert_child_reaped(directory.path());
}

#[test]
fn offline_window_bar_routes_controls_and_resize_edges_without_a_window_manager() {
    let directory = tempfile::tempdir().expect("isolated window-controls probe");
    let output = run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "controls-window",
    );
    assert_probe(&output);
    assert!(
        output.contains("Adapter probe passed: controls-window"),
        "{output}"
    );
}

#[test]
fn offline_composed_panels_gate_mutations_and_preserve_ids_and_delegate_lifetimes() {
    let directory = tempfile::tempdir().expect("isolated composed-panel probe");
    let output = run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "controls-panels",
    );
    assert_probe(&output);
    assert!(
        output.contains("Adapter probe passed: controls-panels"),
        "{output}"
    );
}

#[test]
fn every_embedded_image_and_font_decodes_in_the_offline_gallery() {
    let directory = tempfile::tempdir().expect("isolated asset probe");
    let output = run(
        &["--component-gallery"],
        "offscreen",
        directory.path(),
        Path::new("/no/server/allowed"),
        "native-assets",
    );
    assert!(
        output.contains("Embedded asset probe checked 37 resources"),
        "{output}"
    );
    assert!(
        output.contains("Visual probe finished: 0 issues"),
        "{output}"
    );
    for error in [
        "VISUAL ISSUE",
        "ReferenceError",
        "TypeError",
        "Binding loop",
        "Cannot",
        "failed to load",
    ] {
        assert!(!output.contains(error), "{output}");
    }
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
fn settings_search_filters_visible_labels_and_preserves_layout_and_state() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated settings search fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        &server,
        "settings-search",
    ));
    let requests = std::fs::read_to_string(directory.path().join("requests")).expect("requests");
    assert!(
        requests
            .lines()
            .filter(|line| line.starts_with("/api/tracks"))
            .all(|line| line == "/api/tracks?q="),
        "Settings search must not search the library: {requests}"
    );
    assert!(
        !requests
            .lines()
            .any(|line| line.starts_with("POST ") || line.starts_with("DELETE ")),
        "Settings/About search must not create or delete backend data: {requests}"
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
fn insertion_menus_preserve_selection_and_send_typed_items_in_both_positions() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated insertion fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700)).expect("executable");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        &server,
        "queue-insert",
    ));
    let operations: Vec<serde_json::Value> =
        std::fs::read_to_string(directory.path().join("queue_operations"))
            .expect("insertion operations")
            .lines()
            .map(|line| serde_json::from_str(line).expect("JSON"))
            .collect();
    assert_eq!(operations.len(), 9);
    for (index, op) in operations.iter().take(7).enumerate() {
        assert_eq!(op.get("method").expect("method"), "POST");
        assert_eq!(op.get("path").expect("path"), "/api/queue/items");
        let item = match index {
            0 | 1 => serde_json::json!({"type":"audio", "audio_source_id":7}),
            2 | 3 => serde_json::json!({"type":"audio", "audio_source_id":103}),
            _ => serde_json::json!({"type":"playlist_item", "playlist_id":1, "item_id":index + 7}),
        };
        assert_eq!(
            *op.get("body").expect("body"),
            serde_json::json!({"items":[item], "placement":if index % 2 == 1 {"next"} else {"end"}})
        );
        assert_eq!(op.get("status").expect("status"), 200);
    }
    let whole = operations.get(7).expect("whole playlist");
    assert_eq!(whole.get("path").expect("path"), "/api/playlists/1/queue");
    assert_eq!(whole.get("method").expect("method"), "POST");
    assert_eq!(*whole.get("body").expect("body"), serde_json::json!({}));
    assert_eq!(
        *whole.get("after_ids").expect("after_ids"),
        serde_json::json!([
            "1", "8", "6", "4", "2", "3", "5", "7", "9", "10", "11", "12"
        ])
    );
    let failed = operations.last().expect("failed operation");
    assert_eq!(failed.get("status").expect("status"), 409);
    assert_eq!(
        failed.get("after_ids").expect("after_ids"),
        failed.get("before_ids").expect("before_ids")
    );
    assert!(
        !directory.path().join("playback_commands").exists(),
        "menus/selection must never Play"
    );
    let requests = std::fs::read_to_string(directory.path().join("requests")).expect("requests");
    assert_eq!(
        requests
            .lines()
            .filter(|line| *line == "/api/playlists/1")
            .count(),
        1,
        "whole playlist append never reads its items again"
    );
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn full_queue_visual_probe_preserves_cards_at_minimum_window_size() {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().expect("isolated queue visual fixture");
    let server = directory.path().join("fixture-server");
    std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
    std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
        .expect("executable fixture");
    let output = run(&[], "offscreen", directory.path(), &server, "visual-queue");
    assert!(
        output.contains("Visual probe finished: 0 issues"),
        "{output}"
    );
    for error in [
        "VISUAL ISSUE",
        "ReferenceError",
        "TypeError",
        "Cannot",
        "Binding loop",
    ] {
        assert!(!output.contains(error), "{output}");
    }
    assert_child_reaped(directory.path());
}

#[cfg(unix)]
#[test]
fn managed_queue_virtualizes_history_and_preserves_exact_entry_commands() {
    use std::os::unix::fs::PermissionsExt;
    let mut measurements = Vec::new();
    let mut media_counts = Vec::new();
    let mut media_paths = Vec::new();
    for case in ["queue-managed-small", "queue-managed"] {
        let directory = tempfile::tempdir().expect("isolated managed queue fixture");
        let server = directory.path().join("fixture-server");
        std::fs::write(&server, include_str!("fixtures/server.py")).expect("write fixture");
        std::fs::set_permissions(&server, std::fs::Permissions::from_mode(0o700))
            .expect("executable fixture");
        let output = run(&[], "offscreen", directory.path(), &server, case);
        assert_probe(&output);
        let metrics = output
            .lines()
            .find_map(|line| line.split_once("Queue metrics ").map(|(_, json)| json))
            .expect("delegate metrics");
        let metrics: serde_json::Value = serde_json::from_str(metrics).expect("metrics JSON");
        eprintln!("{case}: {metrics}");
        measurements.push(metrics);
        assert_queue_operations(directory.path());
        let requests =
            std::fs::read_to_string(directory.path().join("requests")).expect("requests");
        let media: Vec<_> = requests
            .lines()
            .filter(|line| line.ends_with("/duration") || line.ends_with("/cover"))
            .collect();
        assert!(media.len() < 60, "bounded media requests: {media:?}");
        for path in ["/api/audio-sources/42/duration", "/api/beatmaps/42/cover"] {
            assert_eq!(
                media.iter().filter(|line| **line == path).count(),
                1,
                "duplicates share {path}"
            );
        }
        eprintln!("{case}: {} media requests", media.len());
        let checkpoint = std::fs::read_to_string(directory.path().join("queue_viewport_requests"))
            .expect("viewport route checkpoint before drag/edge scrolling");
        let viewport_media = checkpoint
            .lines()
            .filter(|path| path.ends_with("/duration") || path.ends_with("/cover"))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        eprintln!(
            "{case}: {} viewport route media requests",
            viewport_media.len()
        );
        media_counts.push(viewport_media.len());
        media_paths.push(viewport_media);
        assert_child_reaped(directory.path());
    }
    let small = measurements.first().expect("small metrics");
    let large = measurements.get(1).expect("large metrics");
    assert_eq!(small.get("viewport"), large.get("viewport"));
    for key in ["initialCreated", "created", "live", "demand"] {
        let a = small
            .get(key)
            .and_then(serde_json::Value::as_i64)
            .expect("small measurement");
        let b = large
            .get(key)
            .and_then(serde_json::Value::as_i64)
            .expect("large measurement");
        // Qt may construct one extra viewport while polishing a jump.
        assert!(
            a.abs_diff(b) <= 12,
            "{key} depends on viewport, not history: {measurements:?}"
        );
    }
    assert!(
        media_counts
            .first()
            .expect("small media")
            .abs_diff(*media_counts.get(1).expect("large media"))
            <= 6,
        "media requests differ: {media_paths:?}"
    );
}

#[cfg(unix)]
fn assert_queue_operations(directory: &Path) {
    let operations: Vec<serde_json::Value> =
        std::fs::read_to_string(directory.join("queue_operations"))
            .expect("operations")
            .lines()
            .map(|line| serde_json::from_str(line).expect("operation JSON"))
            .collect();
    let routes: Vec<_> = operations
        .iter()
        .map(|op| {
            (
                op.get("method")
                    .and_then(serde_json::Value::as_str)
                    .expect("method"),
                op.get("path")
                    .and_then(serde_json::Value::as_str)
                    .expect("path"),
            )
        })
        .collect();
    assert_eq!(
        routes,
        [
            ("PATCH", "/api/queue/items/9007199254740997"),
            ("PATCH", "/api/queue/items/9223372036854775807"),
            ("PATCH", "/api/queue/items/9223372036854775807"),
            ("PATCH", "/api/queue/items/9223372036854775807"),
            ("DELETE", "/api/queue/items/9007199254740995"),
            ("PATCH", "/api/queue/items/9223372036854775807"),
            ("PATCH", "/api/queue/items/9223372036854775807"),
            ("POST", "/api/queue/items/9007199254740997/play"),
            ("POST", "/api/queue/items/100000/play"),
            ("DELETE", "/api/queue/items/9007199254740999"),
            ("DELETE", "/api/queue/upcoming"),
            ("DELETE", "/api/queue"),
        ],
        "one exact command per completed action; cancel/navigation/open send none"
    );
    let anchors: Vec<_> = operations
        .iter()
        .filter_map(|op| op.pointer("/body/before_entry_id").cloned())
        .collect();
    assert_eq!(
        anchors,
        [
            serde_json::json!("9223372036854775807"),
            serde_json::json!("9007199254740995"),
            serde_json::json!("9223372036854775807"),
            serde_json::json!("9007199254740997"),
            serde_json::Value::Null,
            serde_json::json!("9007199254740997")
        ]
    );
    let rejected = operations.first().expect("rejected move");
    assert_eq!(
        rejected.get("status").and_then(serde_json::Value::as_u64),
        Some(409)
    );
    assert_eq!(rejected.get("before_ids"), rejected.get("after_ids"));
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
            "POST /api/user-data/osu-folders/52/import ",
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

#[cfg(unix)]
#[test]
#[ignore = "needs cargo build -p osu-radio-server --locked (SQLite)"]
fn real_backend_queue_survives_restart_and_preserves_exact_entry_actions() {
    let directory = tempfile::tempdir().expect("isolated real queue backend");
    let server = Path::new(env!("CARGO_BIN_EXE_osu-radio-qt")).with_file_name("osu-radio-server");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/queue_backend.py");
    for mode in ["prepare", "verify"] {
        if mode == "verify" {
            // Both launches use the real embedded server and the same private database.
            for case in ["queue-real-recovery", "queue-real"] {
                assert_probe(&run(
                    &[],
                    "offscreen",
                    directory.path(),
                    &directory.path().join("fixture-server"),
                    case,
                ));
            }
        }
        let output = Command::new("python3")
            .arg(&fixture)
            .arg(mode)
            .arg(&server)
            .current_dir(directory.path())
            .output()
            .expect("real queue fixture");
        assert!(
            output.status.success(),
            "{mode}: {}\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
            std::fs::read_to_string(directory.path().join("server.log")).unwrap_or_default()
        );
    }
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

#[cfg(target_os = "linux")]
#[test]
fn local_track_name_preferences_persist_and_report_read_write_failures_offline() {
    let directory = tempfile::tempdir().expect("isolated name preferences");
    let server = Path::new("/no/server/allowed");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        server,
        "names-write",
    ));
    let path = directory.path().join("config/osu-radio/osu-radio-qt.conf");
    let saved = std::fs::read_to_string(&path).expect("native settings file");
    assert!(
        saved.contains("use_unicode_titles=true") && saved.contains("use_unicode_artists=true"),
        "{saved}"
    );
    // An unrelated key is preserved because each toggle writes only its own key.
    std::fs::write(&path, format!("{saved}\n[unrelated]\nkeep=value\n"))
        .expect("unrelated preference");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        server,
        "names-read",
    ));
    assert!(
        std::fs::read_to_string(&path)
            .expect("unchanged native settings")
            .contains("keep=value")
    );
    std::fs::write(&path, "[display\nuse_unicode_titles=true\n").expect("corrupt preferences");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        server,
        "names-read-error",
    ));
    let blocked = tempfile::tempdir().expect("isolated inaccessible preferences");
    std::fs::write(blocked.path().join("config"), "not a directory")
        .expect("block settings directory");
    assert_probe(&run(
        &[],
        "offscreen",
        blocked.path(),
        server,
        "names-write-error",
    ));
    assert!(
        !directory.path().join("starts").exists(),
        "offline preferences do not need a server"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn discord_preference_persists_searches_and_toggles_without_discord_or_server() {
    let directory = tempfile::tempdir().expect("isolated Discord preference");
    let server = Path::new("/no/server/allowed");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        server,
        "discord-write",
    ));
    let path = directory.path().join("config/osu-radio/osu-radio-qt.conf");
    let saved = std::fs::read_to_string(&path).expect("Discord settings");
    assert!(saved.contains("discord_rich_presence=true"), "{saved}");
    std::fs::write(
        &path,
        format!("{saved}\n[display]\nuse_unicode_titles=true\nuse_unicode_artists=true\n[unrelated]\nkeep=value\n"),
    )
        .expect("unrelated preference");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        server,
        "discord-read",
    ));
    assert!(
        std::fs::read_to_string(&path)
            .expect("preserved preference")
            .contains("keep=value")
    );
    std::fs::write(&path, "[integrations\ndiscord_rich_presence=true\n")
        .expect("corrupt preference");
    assert_probe(&run(
        &[],
        "offscreen",
        directory.path(),
        server,
        "discord-read-error",
    ));
    let blocked = tempfile::tempdir().expect("inaccessible preference");
    std::fs::write(blocked.path().join("config"), "not a directory").expect("blocked config");
    assert_probe(&run(
        &[],
        "offscreen",
        blocked.path(),
        server,
        "discord-write-error",
    ));
    assert!(!directory.path().join("starts").exists());
}
