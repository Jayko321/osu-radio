use crate::model::SourceType;
use lofty::{config::ParseOptions, file::AudioFile, probe::Probe};
use std::{fs::File, io::BufReader, path::Path};

pub async fn probe_source_duration(source: SourceType) -> anyhow::Result<Option<u64>> {
    Ok(match source {
        SourceType::Local(path) | SourceType::Copied(path) => {
            tokio::task::spawn_blocking(move || probe_duration(Path::new(&path))).await?
        }
        SourceType::Online(_) => None,
    })
}

fn probe_duration(path: &Path) -> Option<u64> {
    if !std::fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let file = File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let audio = Probe::new(BufReader::new(file))
        .guess_file_type()
        .ok()?
        .options(ParseOptions::new().read_tags(false))
        .read()
        .ok()?;
    let duration = audio.properties().duration();
    if duration.is_zero() {
        return None;
    }
    u64::try_from(duration.as_millis()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn duration_rejects_fifo_before_a_blocking_open() {
        let directory = tempfile::tempdir().unwrap();
        let fifo = directory.path().join("audio-fifo");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let (sender, receiver) = std::sync::mpsc::channel();
        let probe_path = fifo.clone();
        let probe = std::thread::spawn(move || {
            sender.send(probe_duration(&probe_path)).unwrap();
        });
        let result = receiver.recv_timeout(std::time::Duration::from_secs(2));
        if result.is_err() {
            // Unblock a regressed read-only open so the failed test still joins its thread.
            drop(File::options().write(true).open(&fifo).unwrap());
        }
        probe.join().unwrap();
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn probes_extensionless_wav_mp3_and_ogg_and_rejects_missing_or_corrupt_media() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("hash-without-extension");
        for bytes in [
            include_bytes!("../../../apps/osu-radio-server/tests/fixtures/tone.wav").as_slice(),
            include_bytes!("../../../apps/osu-radio-server/tests/fixtures/tone.mp3").as_slice(),
            include_bytes!("../../../apps/osu-radio-server/tests/fixtures/tone.ogg").as_slice(),
        ] {
            std::fs::write(&path, bytes).unwrap();
            let duration = probe_duration(&path).unwrap();
            assert!((200..=350).contains(&duration), "duration: {duration}");
        }
        std::fs::write(&path, b"broken media").unwrap();
        assert_eq!(probe_duration(&path), None);
        std::fs::remove_file(&path).unwrap();
        assert_eq!(probe_duration(&path), None);
        assert_eq!(probe_duration(directory.path()), None);
    }
}
