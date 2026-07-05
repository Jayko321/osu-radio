use std::{error::Error, fmt, io};

use radio_core::{OsuKind, OsuMarker, import_types::ImportedBeatmap};

pub mod helpers;
pub mod lazer;

pub use lazer::{import_from_lazer_realm, import_from_lazer_realm_with_helper};

#[async_trait::async_trait]
pub(crate) trait BeatmapScanner {
    async fn get_beatmaps(&self) -> Result<Vec<ImportedBeatmap>, ScannerError>;
}

#[derive(Debug)]
pub enum ScannerError {
    Import(io::Error),
    UnsupportedSource(OsuKind),
}

impl fmt::Display for ScannerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScannerError::Import(error) => write!(formatter, "{error}"),
            ScannerError::UnsupportedSource(kind) => {
                write!(formatter, "unsupported osu! source: {kind:?}")
            }
        }
    }
}

impl Error for ScannerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ScannerError::Import(error) => Some(error),
            ScannerError::UnsupportedSource(_) => None,
        }
    }
}

impl From<io::Error> for ScannerError {
    fn from(error: io::Error) -> Self {
        ScannerError::Import(error)
    }
}

pub async fn get_beatmaps(marker: OsuMarker) -> Result<Vec<ImportedBeatmap>, ScannerError> {
    let scanner: Box<dyn BeatmapScanner> = match marker.kind {
        OsuKind::Stable => return Err(ScannerError::UnsupportedSource(OsuKind::Stable)),
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

    use super::{ScannerError, get_beatmaps};

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
            error,
            ScannerError::UnsupportedSource(OsuKind::Stable)
        ));
    }
}
