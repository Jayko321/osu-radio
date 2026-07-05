pub mod import_types;

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsuKind {
    Stable,
    Lazer,
}

#[derive(Debug, Clone)]
pub struct OsuMarker {
    pub kind: OsuKind,
    pub marker_path: PathBuf,
    pub root_path: PathBuf,
}
