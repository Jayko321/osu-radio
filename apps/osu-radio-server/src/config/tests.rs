use std::{env, fs, io::ErrorKind, path::Path, process::Command};

use tempfile::TempDir;

use super::{ADDRESS_ENV, DATABASE_URL_ENV, ServerConfig};

const PROBE_ENV: &str = "OSU_RADIO_CONFIG_TEST_CASE";
const DATABASE_URL: &str = "database-from-environment";

#[test]
fn repository_dotenv_parses_without_loading_values_or_connecting() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.env");
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return,
        Err(error) => panic!(
            "cannot read repository .env ({:?}); contents withheld",
            error.kind()
        ),
    };
    let before: std::collections::BTreeMap<_, _> = env::vars_os().collect();
    // Iteration validates syntax without installing values or opening a database.
    for entry in dotenvy::from_read_iter(file) {
        assert!(
            entry.is_ok(),
            "repository .env syntax error; contents withheld"
        );
    }
    assert!(
        env::vars_os().collect::<std::collections::BTreeMap<_, _>>() == before,
        "dotenv parsing must not modify the process environment"
    );
}

fn directory_without_dotenv() -> TempDir {
    let directory = TempDir::new().expect("temporary working directory");
    for ancestor in directory.path().ancestors() {
        assert!(
            !ancestor.join(".env").exists(),
            "fixture must not discover an ancestor .env: {}",
            ancestor.display()
        );
    }
    directory
}

fn check_in_subprocess(directory: &Path, scenario: &str, variables: &[(&str, &str)]) {
    let mut command = Command::new(env::current_exe().expect("test executable"));
    command
        .args(["--exact", "config::tests::configuration_probe", "--ignored"])
        .current_dir(directory)
        .env_clear()
        .env(PROBE_ENV, scenario)
        .envs(variables.iter().copied());
    // Preserve runtime lookup and per-process coverage without inheriting app configuration.
    for name in ["SystemRoot", "PATH", "LLVM_PROFILE_FILE"] {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    let output = command.output().expect("run isolated configuration probe");
    assert!(
        output.status.success(),
        "probe {scenario} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn environment_only_keeps_default_address() {
    let directory = directory_without_dotenv();
    check_in_subprocess(
        directory.path(),
        "default-address",
        &[(DATABASE_URL_ENV, DATABASE_URL)],
    );
}

#[test]
fn environment_only_uses_explicit_address() {
    let directory = directory_without_dotenv();
    check_in_subprocess(
        directory.path(),
        "explicit-address",
        &[
            (DATABASE_URL_ENV, DATABASE_URL),
            (ADDRESS_ENV, "127.0.0.1:0"),
        ],
    );
}

#[cfg(not(all(any(target_os = "linux", windows), feature = "sqlite")))]
#[test]
fn selected_database_url_is_still_required() {
    let directory = directory_without_dotenv();
    let other_database_env = if cfg!(feature = "sqlite") {
        "POSTGRES_DATABASE_URL"
    } else {
        "SQLITE_DATABASE_URL"
    };
    check_in_subprocess(
        directory.path(),
        "missing-database",
        &[(other_database_env, DATABASE_URL)],
    );
}

#[test]
fn valid_dotenv_supplies_configuration() {
    let directory = directory_without_dotenv();
    fs::write(
        directory.path().join(".env"),
        format!("{DATABASE_URL_ENV}={DATABASE_URL}\n{ADDRESS_ENV}=127.0.0.1:0\n"),
    )
    .expect("write .env");
    check_in_subprocess(directory.path(), "explicit-address", &[]);
}

#[test]
fn process_environment_takes_precedence_over_dotenv() {
    let directory = directory_without_dotenv();
    fs::write(
        directory.path().join(".env"),
        format!("{DATABASE_URL_ENV}=database-from-dotenv\n{ADDRESS_ENV}=127.0.0.1:3001\n"),
    )
    .expect("write .env");
    check_in_subprocess(
        directory.path(),
        "explicit-address",
        &[
            (DATABASE_URL_ENV, DATABASE_URL),
            (ADDRESS_ENV, "127.0.0.1:0"),
        ],
    );
}

#[test]
fn malformed_dotenv_is_fatal_even_with_database_in_environment() {
    let directory = directory_without_dotenv();
    fs::write(directory.path().join(".env"), "INVALID='unterminated\n")
        .expect("write malformed .env");
    check_in_subprocess(
        directory.path(),
        "parse-error",
        &[(DATABASE_URL_ENV, DATABASE_URL)],
    );
}

#[test]
fn unreadable_dotenv_is_fatal_even_with_database_in_environment() {
    let directory = directory_without_dotenv();
    // Invalid UTF-8 produces a deterministic read error, even when tests run as root.
    fs::write(directory.path().join(".env"), [0xff, b'\n']).expect("write invalid UTF-8 .env");
    check_in_subprocess(
        directory.path(),
        "read-error",
        &[(DATABASE_URL_ENV, DATABASE_URL)],
    );
}

#[test]
fn only_one_skips_dotenv_including_parent_search() {
    for contents in [
        format!("{DATABASE_URL_ENV}=database-from-dotenv\n").into_bytes(),
        b"INVALID='unterminated\n".to_vec(),
        vec![0xff],
    ] {
        let directory = directory_without_dotenv();
        fs::write(directory.path().join(".env"), &contents).unwrap();
        let child = directory.path().join("child");
        fs::create_dir(&child).unwrap();
        for cwd in [directory.path(), child.as_path()] {
            check_in_subprocess(
                cwd,
                "default-address",
                &[
                    (DATABASE_URL_ENV, DATABASE_URL),
                    ("OSU_RADIO_SKIP_DOTENV", "1"),
                ],
            );
        }
    }
    let directory = directory_without_dotenv();
    fs::write(directory.path().join(".env"), "INVALID='unterminated\n").unwrap();
    for value in ["", "0", "true", "01"] {
        check_in_subprocess(
            directory.path(),
            "parse-error",
            &[
                (DATABASE_URL_ENV, DATABASE_URL),
                ("OSU_RADIO_SKIP_DOTENV", value),
            ],
        );
    }
}

#[cfg(all(target_os = "linux", feature = "sqlite"))]
#[test]
fn linux_sqlite_defaults_are_private_absolute_and_only_used_without_a_url() {
    let directory = directory_without_dotenv();
    let home = directory.path().join("home with spaces");
    let xdg = directory.path().join("xdg with spaces");
    for value in [
        None,
        Some(""),
        Some("relative/data"),
        Some(xdg.to_str().unwrap()),
    ] {
        let mut variables = vec![("HOME", home.to_str().unwrap())];
        if let Some(value) = value {
            variables.push(("XDG_DATA_HOME", value));
        }
        let expected = if value == Some(xdg.to_str().unwrap()) {
            xdg.join("osu-radio/osu-radio.db")
        } else {
            home.join(".local/share/osu-radio/osu-radio.db")
        };
        variables.push(("EXPECTED_SQLITE_PATH", expected.to_str().unwrap()));
        check_in_subprocess(directory.path(), "sqlite-default", &variables);
    }
    let expected = xdg.join("osu-radio/osu-radio.db");
    check_in_subprocess(
        directory.path(),
        "sqlite-default",
        &[
            ("XDG_DATA_HOME", xdg.to_str().unwrap()),
            ("EXPECTED_SQLITE_PATH", expected.to_str().unwrap()),
        ],
    );
    for home in [None, Some(""), Some("relative/home")] {
        let variables = home.map_or_else(Vec::new, |home| vec![("HOME", home)]);
        check_in_subprocess(directory.path(), "missing-home", &variables);
    }
    let blocked = directory.path().join("blocked");
    fs::write(&blocked, b"file instead of directory").unwrap();
    check_in_subprocess(
        directory.path(),
        "mkdir-error",
        &[("XDG_DATA_HOME", blocked.to_str().unwrap())],
    );
    let unused = directory.path().join("unused");
    check_in_subprocess(
        directory.path(),
        "default-address",
        &[
            (DATABASE_URL_ENV, DATABASE_URL),
            ("XDG_DATA_HOME", unused.to_str().unwrap()),
        ],
    );
    assert!(!unused.exists());
}

#[cfg(all(windows, feature = "sqlite"))]
#[test]
fn windows_sqlite_defaults_use_an_absolute_local_app_data_and_only_without_a_url() {
    let directory = directory_without_dotenv();
    let local = directory.path().join("Local с пробелами");
    let expected = local.join("osu-radio").join("osu-radio.db");
    check_in_subprocess(
        directory.path(),
        "sqlite-default",
        &[
            ("LOCALAPPDATA", local.to_str().unwrap()),
            ("EXPECTED_SQLITE_PATH", expected.to_str().unwrap()),
        ],
    );
    for value in [None, Some(""), Some("relative\\data")] {
        let variables = value.map_or_else(Vec::new, |value| vec![("LOCALAPPDATA", value)]);
        check_in_subprocess(directory.path(), "missing-home", &variables);
    }
    let blocked = directory.path().join("blocked");
    fs::write(&blocked, b"file instead of directory").unwrap();
    check_in_subprocess(
        directory.path(),
        "mkdir-error",
        &[("LOCALAPPDATA", blocked.to_str().unwrap())],
    );
    for value in ["relative\\data", ""] {
        check_in_subprocess(
            directory.path(),
            "default-address",
            &[(DATABASE_URL_ENV, DATABASE_URL), ("LOCALAPPDATA", value)],
        );
    }
    let unused = directory.path().join("unused");
    check_in_subprocess(
        directory.path(),
        "default-address",
        &[
            (DATABASE_URL_ENV, DATABASE_URL),
            ("LOCALAPPDATA", unused.to_str().unwrap()),
        ],
    );
    assert!(!unused.exists());
}

#[test]
#[ignore = "invoked by configuration tests in an isolated child process"]
fn configuration_probe() {
    let scenario = env::var(PROBE_ENV).expect("configuration probe scenario");
    match scenario.as_str() {
        "default-address" | "explicit-address" => {
            let config = ServerConfig::from_env().expect("configuration should load");
            assert_eq!(config.database_url, DATABASE_URL);
            assert_eq!(
                config.address,
                if scenario == "default-address" {
                    "127.0.0.1:3000"
                } else {
                    "127.0.0.1:0"
                }
            );
        }
        "missing-database" => {
            let error = ServerConfig::from_env()
                .err()
                .expect("database URL required");
            assert!(error.to_string().contains(DATABASE_URL_ENV));
            assert!(matches!(
                error.downcast_ref::<env::VarError>(),
                Some(env::VarError::NotPresent)
            ));
        }
        #[cfg(all(any(target_os = "linux", windows), feature = "sqlite"))]
        "sqlite-default" => {
            let config = ServerConfig::from_env().unwrap();
            let expected = env::var("EXPECTED_SQLITE_PATH").unwrap();
            assert_eq!(config.database_url, expected);
            assert!(Path::new(&expected).parent().unwrap().is_dir());
            assert!(
                !Path::new(&expected).exists(),
                "configuration creates only the directory"
            );
        }
        #[cfg(all(any(target_os = "linux", windows), feature = "sqlite"))]
        "missing-home" | "mkdir-error" => {
            let error = ServerConfig::from_env()
                .err()
                .expect("invalid default directory");
            assert!(error.to_string().contains(if scenario == "missing-home" {
                "Cannot determine SQLite data directory"
            } else {
                "Failed to create SQLite data directory"
            }));
        }
        "parse-error" | "read-error" => {
            let error = ServerConfig::from_env().err().expect(".env must fail");
            assert_eq!(error.to_string(), "Failed to load .env");
            let source = error
                .downcast_ref::<dotenvy::Error>()
                .expect("dotenv cause");
            if scenario == "parse-error" {
                assert!(matches!(source, dotenvy::Error::LineParse(_, _)));
            } else {
                assert!(
                    matches!(source, dotenvy::Error::Io(error) if error.kind() == ErrorKind::InvalidData)
                );
            }
        }
        _ => panic!("unknown configuration probe scenario: {scenario}"),
    }
}
