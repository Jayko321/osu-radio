#![allow(clippy::expect_used, clippy::panic)]
// Ensure the generated native registration can resolve its Rust bridge.
use osu_radio_qt as _;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn run(args: &[&str], platform: &str) -> (std::process::ExitStatus, String) {
    let directory = tempfile::tempdir().expect("empty working directory");
    let mut output = tempfile::tempfile().expect("captured output");
    let mut child = Command::new(env!("CARGO_BIN_EXE_osu-radio-qt"))
        .args(args)
        .current_dir(directory.path())
        .env("QT_QPA_PLATFORM", platform)
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("QT_LOGGING_RULES", "qml.info=true;qml.warning=true")
        .env("QT_QUICK_BACKEND", "software")
        .env("QSG_RHI_BACKEND", "software")
        .env("QML_DISABLE_DISK_CACHE", "1")
        .env("OSU_RADIO_QT_SMOKE_TEST", "1")
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
        if start.elapsed() >= Duration::from_secs(15) {
            child.kill().expect("terminate timed out smoke test");
            child.wait().expect("reap timed out smoke test");
            panic!("Qt smoke test timed out: {}", read_output(&mut output));
        }
        thread::sleep(Duration::from_millis(20));
    };
    (status, read_output(&mut output))
}

fn read_output(file: &mut File) -> String {
    file.seek(SeekFrom::Start(0)).expect("rewind output");
    let mut output = String::new();
    file.read_to_string(&mut output).expect("read output");
    output
}

#[test]
fn both_qml_roots_and_adapter_load_from_an_empty_directory() {
    for args in [vec![], vec!["--component-gallery"]] {
        let (status, output) = run(&args, "offscreen");
        assert!(status.success(), "{args:?}: {status}\n{output}");
        assert!(output.contains("Adapter probe passed"), "{output}");
        for error in [
            "Error",
            "failed",
            "is not",
            "Cannot",
            "Unable",
            "Binding loop",
            "No such",
            "not found",
        ] {
            assert!(
                !output.contains(error),
                "QML diagnostic in {args:?}: {output}"
            );
        }
    }
}

#[test]
fn help_and_invalid_arguments_do_not_initialize_a_gui() {
    let (status, output) = run(&["--help"], "intentionally-nonexistent-platform");
    assert!(status.success(), "{output}");
    assert!(output.contains("Usage: osu-radio-qt"));
    assert!(!output.contains("Adapter probe"));
    let (status, output) = run(&["--unknown"], "intentionally-nonexistent-platform");
    assert_eq!(status.code(), Some(2), "{output}");
    assert!(output.contains("Unknown or repeated argument"));
}
