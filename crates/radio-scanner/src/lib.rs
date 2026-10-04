#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unwrap_used
    )
)]

use std::{error::Error, ffi::OsStr, fmt};

use radio_core::{
    OsuKind, OsuMarker,
    import_types::{ImportedBeatmapSet, ImportedSnapshot},
};

pub mod discovery;
pub mod lazer;
pub mod stable;

pub use lazer::{import_from_lazer_realm, import_from_lazer_realm_with_helper};
pub use stable::import_from_stable_db;

#[derive(Debug)]
pub struct UnsupportedSourceError {
    pub kind: OsuKind,
}

impl fmt::Display for UnsupportedSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported osu! source: {:?}", self.kind)
    }
}

impl Error for UnsupportedSourceError {}

pub async fn get_beatmap_sets(marker: OsuMarker) -> anyhow::Result<Vec<ImportedBeatmapSet>> {
    Ok(import_snapshot(marker).await?.beatmap_sets)
}

pub async fn import_snapshot(marker: OsuMarker) -> anyhow::Result<ImportedSnapshot> {
    match marker.kind {
        OsuKind::Stable => stable::import_snapshot_from_stable_db(&marker.marker_path).await,
        OsuKind::Lazer => lazer::import_snapshot_from_lazer_realm(&marker.marker_path).await,
    }
}

pub async fn import_snapshot_with_helper(
    marker: OsuMarker,
    helper_path: impl AsRef<OsStr>,
) -> anyhow::Result<ImportedSnapshot> {
    match marker.kind {
        OsuKind::Stable => stable::import_snapshot_from_stable_db(&marker.marker_path).await,
        OsuKind::Lazer => {
            lazer::import_snapshot_from_lazer_realm_with_helper(&marker.marker_path, helper_path)
                .await
        }
    }
}

pub(crate) fn normalize_md5(value: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid MD5 hash: expected 32 ASCII hexadecimal characters"
    );
    Ok(value.to_ascii_lowercase())
}
