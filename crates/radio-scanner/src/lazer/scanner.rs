use std::{
    env,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    process::Stdio,
};

use radio_core::import_types::ImportedBeatmap;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
};

use crate::{BeatmapScanner, ScannerError, lazer::types::parse_lazer_beatmap_line};

const HELPER_PATH_ENV: &str = "OSU_LAZER_REALM_PARSER_PATH";
const BUILT_HELPER_PATH: &str = env!("OSU_LAZER_REALM_PARSER_BUILT_PATH");

/// Runs the bundled osu!lazer Realm extractor and maps its NDJSON output to core types.
///
/// Set `OSU_LAZER_REALM_PARSER_PATH` at runtime to override the helper built by Cargo.
pub async fn import_from_lazer_realm(realm_path: &Path) -> io::Result<Vec<ImportedBeatmap>> {
    let helper_path = env::var_os(HELPER_PATH_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(BUILT_HELPER_PATH));

    import_from_lazer_realm_with_helper(realm_path, helper_path).await
}

/// Runs an explicitly selected osu!lazer Realm extractor and maps its NDJSON output.
pub async fn import_from_lazer_realm_with_helper(
    realm_path: &Path,
    helper_path: impl AsRef<OsStr>,
) -> io::Result<Vec<ImportedBeatmap>> {
    let mut child = Command::new(helper_path)
        .arg(realm_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("failed to capture Realm helper stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("failed to capture Realm helper stderr"))?;

    // Drain stderr concurrently so a verbose failure cannot block the helper while stdout is read.
    // If helper stderr ever becomes noisy, cap this buffer and return a truncated diagnostic.
    let mut stderr_reader = tokio::spawn(async move {
        let mut message = String::new();
        BufReader::new(stderr).read_to_string(&mut message).await?;
        Ok::<_, io::Error>(message)
    });

    let mut lines = BufReader::new(stdout).lines();
    // This importer currently materializes the full lazer library for its callers. If large
    // libraries make this too expensive, change this boundary to stream records to a callback or
    // async stream instead of returning one Vec.
    let mut beatmaps = Vec::new();

    loop {
        let line = match lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                terminate_child(&mut child, &mut stderr_reader).await;
                return Err(error);
            }
        };

        let beatmap = match parse_lazer_beatmap_line(&line) {
            Ok(beatmap) => beatmap,
            Err(error) => {
                terminate_child(&mut child, &mut stderr_reader).await;
                return Err(error);
            }
        };

        beatmaps.push(beatmap);
    }

    let status = match child.wait().await {
        Ok(status) => status,
        Err(error) => {
            stderr_reader.abort();
            let _ = stderr_reader.await;
            return Err(error);
        }
    };
    let stderr = stderr_reader
        .await
        .map_err(|error| io::Error::other(format!("Realm helper stderr task failed: {error}")))??;

    if !status.success() {
        let detail = stderr.trim();
        return Err(io::Error::other(if detail.is_empty() {
            format!("osu!lazer Realm helper exited with {status}")
        } else {
            format!("osu!lazer Realm helper exited with {status}: {detail}")
        }));
    }

    if !stderr.is_empty() {
        let mut error_output = tokio::io::stderr();
        error_output.write_all(stderr.as_bytes()).await?;
        error_output.flush().await?;
    }

    Ok(beatmaps)
}

async fn terminate_child(
    child: &mut tokio::process::Child,
    stderr_reader: &mut tokio::task::JoinHandle<io::Result<String>>,
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
impl BeatmapScanner for LazerBeatmapScanner {
    async fn get_beatmaps(&self) -> Result<Vec<ImportedBeatmap>, ScannerError> {
        Ok(import_from_lazer_realm(&self.db_path).await?)
    }
}
