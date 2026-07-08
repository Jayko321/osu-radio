use std::{error::Error, fmt};

use radio_core::{OsuKind, OsuMarker, import_types::ImportedBeatmap};

pub mod helpers;
pub mod lazer;

pub use lazer::{import_from_lazer_realm, import_from_lazer_realm_with_helper};

#[async_trait::async_trait]
pub(crate) trait BeatmapScanner {
    async fn get_beatmaps(&self) -> anyhow::Result<Vec<ImportedBeatmap>>;
}

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

pub async fn get_beatmaps(marker: OsuMarker) -> anyhow::Result<Vec<ImportedBeatmap>> {
    let scanner: Box<dyn BeatmapScanner> = match marker.kind {
        OsuKind::Stable => {
            return Err(UnsupportedSourceError {
                kind: OsuKind::Stable,
            }
            .into());
        }
        OsuKind::Lazer => Box::new(lazer::scanner::LazerBeatmapScanner::new(
            &marker.marker_path,
        )),
    };

    scanner.get_beatmaps().await
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use radio_core::{OsuKind, OsuMarker};

    use super::{UnsupportedSourceError, get_beatmaps};

    #[tokio::test]
    async fn reports_unsupported_scanner_sources() {
        let error = get_beatmaps(OsuMarker {
            kind: OsuKind::Stable,
            marker_path: PathBuf::from("osu!.db"),
            root_path: PathBuf::from("."),
        })
        .await
        .unwrap_err();

        assert!(matches!(
            error.downcast_ref::<UnsupportedSourceError>(),
            Some(UnsupportedSourceError {
                kind: OsuKind::Stable
            })
        ));
    }
}
