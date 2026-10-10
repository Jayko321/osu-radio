use std::{env, fs, io::ErrorKind, path::Path, process::Command};

use tempfile::TempDir;

use super::{ADDRESS_ENV, DATABASE_URL_ENV, ServerConfig};

const PROBE_ENV: &str = "OSU_RADIO_CONFIG_TEST_CASE";
const DATABASE_URL: &str = "database-from-environment";

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
    // Windows needs SystemRoot; PATH may contain runtime DLLs. Neither supplies app config.
    for name in ["SystemRoot", "PATH"] {
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
