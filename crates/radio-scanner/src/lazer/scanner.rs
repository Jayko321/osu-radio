use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{Context, Result, bail};
use radio_core::import_types::{ImportedBeatmapSet, ImportedSnapshot};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

use crate::lazer::types::{LazerRecord, parse_lazer_line};

const HELPER_PATH_ENV: &str = "OSU_LAZER_REALM_PARSER_PATH";
const BUILT_HELPER_PATH: &str = env!("OSU_LAZER_REALM_PARSER_BUILT_PATH");
// Package layout, relative to the directory of the running backend/CLI executable.
#[cfg(windows)]
pub(crate) const INSTALLED_HELPER_PATH: &str = "helper/osu-lazer-realm-parser.exe";
#[cfg(not(windows))]
pub(crate) const INSTALLED_HELPER_PATH: &str = "../lib/osu-radio/osu-lazer-realm-parser";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub async fn import_from_lazer_realm(realm_path: &Path) -> Result<Vec<ImportedBeatmapSet>> {
    Ok(import_snapshot_from_lazer_realm(realm_path)
        .await?
        .beatmap_sets)
}

pub async fn import_snapshot_from_lazer_realm(realm_path: &Path) -> Result<ImportedSnapshot> {
    let helper_path = select_helper(
        env::var_os(HELPER_PATH_ENV),
        env::current_exe().ok().as_deref(),
        BUILT_HELPER_PATH,
    )?;

    import_snapshot_from_lazer_realm_with_helper(realm_path, helper_path).await
}

/// Runtime override, then the installed helper next to `current_exe`, then the build-time path.
/// An explicit override is never replaced by another helper.
pub(crate) fn select_helper(
    runtime_override: Option<OsString>,
    current_exe: Option<&Path>,
    built: &str,
) -> Result<PathBuf> {
    if let Some(helper) = runtime_override {
        if helper.is_empty() {
            bail!("{HELPER_PATH_ENV} is set but empty");
        }
        return Ok(PathBuf::from(helper));
    }

    let installed = current_exe
        .and_then(Path::parent)
        .map(|directory| directory.join(INSTALLED_HELPER_PATH));
    Ok(match installed {
        Some(installed) if installed.is_file() => installed,
        _ => PathBuf::from(built),
    })
}

pub async fn import_from_lazer_realm_with_helper(
    realm_path: &Path,
    helper_path: impl AsRef<OsStr>,
) -> Result<Vec<ImportedBeatmapSet>> {
    Ok(
        import_snapshot_from_lazer_realm_with_helper(realm_path, helper_path)
            .await?
            .beatmap_sets,
    )
}

pub async fn import_snapshot_from_lazer_realm_with_helper(
    realm_path: &Path,
    helper_path: impl AsRef<OsStr>,
) -> Result<ImportedSnapshot> {
    let mut snapshot = ImportedSnapshot::default();
    consume_records(realm_path, helper_path, |record| match record {
        LazerRecord::BeatmapSet(set) => snapshot.beatmap_sets.push(set),
        LazerRecord::Collection(collection) => snapshot.collections.push(collection),
    })
    .await?;
    Ok(snapshot)
}

pub(crate) async fn count_beatmaps(realm_path: &Path) -> Result<usize> {
    let helper_path = select_helper(
        env::var_os(HELPER_PATH_ENV),
        env::current_exe().ok().as_deref(),
        BUILT_HELPER_PATH,
    )?;
    count_with_helper(realm_path, helper_path).await
}

pub(crate) async fn count_with_helper(
    realm_path: &Path,
    helper_path: impl AsRef<OsStr>,
) -> Result<usize> {
    let mut count = Some(0_usize);
    consume_records(realm_path, helper_path, |record| {
        if let LazerRecord::BeatmapSet(set) = record {
            count = count.and_then(|count| count.checked_add(set.beatmaps.len()));
        }
    })
    .await?;
    count.context("beatmap count overflow")
}

async fn consume_records(
    realm_path: &Path,
    helper_path: impl AsRef<OsStr>,
    mut consume: impl FnMut(LazerRecord),
) -> Result<()> {
    let helper_path = helper_path.as_ref();
    let mut command = Command::new(helper_path);
    command
        .arg(realm_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command.spawn().with_context(|| {
        format!(
            "failed to start osu!lazer Realm helper `{}` for `{}`",
            Path::new(helper_path).display(),
            realm_path.display()
        )
    })?;

    let stdout = child
        .stdout
        .take()
        .context("failed to capture Realm helper stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("failed to capture Realm helper stderr")?;

    let mut stderr_reader = tokio::spawn(async move {
        let mut message = String::new();
        BufReader::new(stderr)
            .read_to_string(&mut message)
            .await
            .context("failed to read Realm helper stderr")?;
        Ok::<_, anyhow::Error>(message)
    });

    let mut lines = BufReader::new(stdout).lines();
    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                terminate_child(&mut child, &mut stderr_reader).await;
                return Err(error).context("failed to read Realm helper stdout");
            }
        };

        let record = match parse_lazer_line(&line) {
            Ok(record) => record,
            Err(error) => {
                terminate_child(&mut child, &mut stderr_reader).await;
                return Err(error);
            }
        };

        consume(record);
    }

    let status = match child.wait().await {
        Ok(status) => status,
        Err(error) => {
            stderr_reader.abort();
            let _ = stderr_reader.await;
            return Err(error).context("failed to wait for Realm helper process");
        }
    };
    let stderr = stderr_reader
        .await
        .context("Realm helper stderr task failed")??;

    if !status.success() {
        let detail = stderr.trim();
        if detail.is_empty() {
            bail!("osu!lazer Realm helper exited with {status}");
        }

        bail!("osu!lazer Realm helper exited with {status}: {detail}");
    }

    if !stderr.is_empty() {
        let mut error_output = tokio::io::stderr();
        error_output
            .write_all(stderr.as_bytes())
            .await
            .context("failed to forward Realm helper stderr")?;
        error_output
            .flush()
            .await
            .context("failed to flush forwarded Realm helper stderr")?;
    }

    Ok(())
}

async fn terminate_child(
    child: &mut tokio::process::Child,
    stderr_reader: &mut tokio::task::JoinHandle<Result<String>>,
) {
    let _ = child.start_kill();
    let _ = child.wait().await;
    stderr_reader.abort();
    let _ = stderr_reader.await;
}
