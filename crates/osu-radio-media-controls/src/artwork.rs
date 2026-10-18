use crate::{ArtworkKey, Snapshot};
use std::{
    io::{self, Write},
    path::Path,
};
use tempfile::NamedTempFile;

/// Owns only the current encoded cover. Adapters convert its path to native resources.
#[derive(Default)]
pub struct ArtworkStore {
    desired: Option<ArtworkKey>,
    file: Option<NamedTempFile>,
}
impl ArtworkStore {
    /// A changed launch, cover or generation immediately removes the previous file.
    pub fn set_current(&mut self, key: Option<ArtworkKey>) {
        if self.desired != key {
            self.clear();
            self.desired = key;
        }
    }

    /// Returns false for a stale result. An I/O failure clears only the cover.
    pub fn install(&mut self, key: ArtworkKey, bytes: &[u8]) -> io::Result<bool> {
        if self.desired != Some(key) {
            return Ok(false);
        }
        self.file = None;
        if bytes.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "empty media artwork",
            ));
        }
        let mut file = NamedTempFile::new()?;
        file.write_all(bytes)?;
        file.flush()?;
        self.file = Some(file);
        Ok(true)
    }

    /// Apply before native metadata publication. Report errors without failing the backend.
    /// Position-only updates reuse the file; pending data retains only a matching identity.
    pub fn update(&mut self, snapshot: &Snapshot) -> io::Result<()> {
        let artwork = snapshot
            .artwork
            .as_ref()
            .filter(|artwork| Some(artwork.key.launch) == snapshot.launch);
        self.set_current(artwork.map(|artwork| artwork.key));
        if self.file.is_none()
            && let Some(artwork) = artwork
            && let Some(bytes) = &artwork.bytes
        {
            self.install(artwork.key, bytes)?;
        }
        Ok(())
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.file.as_ref().map(NamedTempFile::path)
    }

    pub fn clear(&mut self) {
        self.file = None;
        self.desired = None;
    }
}
