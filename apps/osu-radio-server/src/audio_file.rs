use std::{fs::File, io, path::Path};

/// Reject nonregular paths before open (a FIFO can block), then check the descriptor too.
pub(crate) fn open_audio_file(path: &Path) -> io::Result<(File, u64)> {
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(io::Error::from(io::ErrorKind::NotFound));
    }
    Ok((file, metadata.len()))
}
