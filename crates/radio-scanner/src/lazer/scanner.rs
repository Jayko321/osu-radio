use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::{Context, Result, bail};
use radio_core::import_types::ImportedBeatmapSet;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

use crate::{BeatmapSetScanner, lazer::types::parse_lazer_beatmap_set_line};

const HELPER_PATH_ENV: &str = "OSU_LAZER_REALM_PARSER_PATH";
const BUILT_HELPER_PATH: &str = env!("OSU_LAZER_REALM_PARSER_BUILT_PATH");

/// Runs the bundled osu!lazer Realm extractor and maps its NDJSON output to core types.
///
/// Set `OSU_LAZER_REALM_PARSER_PATH` at runtime to override the helper built by Cargo.
pub async fn import_from_lazer_realm(realm_path: &Path) -> Result<Vec<ImportedBeatmapSet>> {
    let helper_path = env::var_os(HELPER_PATH_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(BUILT_HELPER_PATH));

    import_from_lazer_realm_with_helper(realm_path, helper_path).await
}

/// Runs an explicitly selected osu!lazer Realm extractor and maps its NDJSON output.
pub async fn import_from_lazer_realm_with_helper(
    realm_path: &Path,
    helper_path: impl AsRef<OsStr>,
) -> Result<Vec<ImportedBeatmapSet>> {
    let helper_path = helper_path.as_ref();
    let mut child = Command::new(helper_path)
        .arg(realm_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| {
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

    // Drain stderr concurrently so a verbose failure cannot block the helper while stdout is read.
    // If helper stderr ever becomes noisy, cap this buffer and return a truncated diagnostic.
    let mut stderr_reader = tokio::spawn(async move {
        let mut message = String::new();
        BufReader::new(stderr)
            .read_to_string(&mut message)
            .await
            .context("failed to read Realm helper stderr")?;
        Ok::<_, anyhow::Error>(message)
    });

    let mut lines = BufReader::new(stdout).lines();
    // This importer currently materializes the full lazer library for its callers. If large
    // libraries make this too expensive, change this boundary to stream records to a callback or
    // async stream instead of returning one Vec.
    let mut beatmap_sets = Vec::new();

    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                terminate_child(&mut child, &mut stderr_reader).await;
                return Err(error).context("failed to read Realm helper stdout");
            }
        };

        let beatmap_set = match parse_lazer_beatmap_set_line(&line) {
            Ok(beatmap_set) => beatmap_set,
            Err(error) => {
                terminate_child(&mut child, &mut stderr_reader).await;
                return Err(error);
            }
        };

        beatmap_sets.push(beatmap_set);
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
        } else {
            bail!("osu!lazer Realm helper exited with {status}: {detail}");
        }
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

    Ok(beatmap_sets)
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

pub struct LazerBeatmapScanner {
    db_path: PathBuf, //client.realm
}

impl LazerBeatmapScanner {
    pub fn new(path: &Path) -> Self {
        LazerBeatmapScanner {
            db_path: path.to_path_buf(),
        }
    }
}

#[async_trait::async_trait]
impl BeatmapSetScanner for LazerBeatmapScanner {
    async fn get_beatmap_sets(&self) -> Result<Vec<ImportedBeatmapSet>> {
        import_from_lazer_realm(&self.db_path)
            .await
            .with_context(|| format!("failed to import lazer marker `{}`", self.db_path.display()))
    }
}
