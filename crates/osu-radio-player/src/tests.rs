#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
use super::*;

const AUDIO_FIXTURES: [&str; 4] = ["cbr.mp3", "vbr.mp3", "vorbis.ogg", "pcm.wav"];

struct Headless {
    player: Player,
    output: rodio::mixer::MixerSource,
}

impl Headless {
    fn new() -> Self {
        let (mixer, output) =
            rodio::mixer::mixer(2.try_into().unwrap(), 44_100.try_into().unwrap());
        Self {
            player: Player::with_output(mixer, None),
            output,
        }
    }

    fn command<T>(&mut self, command: impl FnOnce(&mut Player) -> T) -> T {
        command(&mut self.player)
    }

    fn consume(&mut self, samples: usize) -> Vec<f32> {
        self.output.by_ref().take(samples).collect()
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn osu_formats_and_extensionless_files_decode_real_audio() {
    for name in AUDIO_FIXTURES {
        let mut decoder = Player::decode(&fixture(name)).unwrap();
        assert!(decoder.total_duration().unwrap() >= Duration::from_millis(1400));
        let samples: Vec<_> = decoder.by_ref().collect();
        assert!(samples.len() > 100_000);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::copy(fixture(name), file.path()).unwrap();
        assert!(Player::decode(file.path()).unwrap().count() > 100_000);
    }
}

#[test]
fn pause_resume_stop_eof_and_replay() {
    let mut test = Headless::new();
    assert_eq!(test.player.snapshot(), Snapshot::default());
    assert!(matches!(test.player.play(), Err(PlayerError::Empty)));
    test.command(|player| player.load(fixture("cbr.mp3")))
        .unwrap();
    assert_eq!(test.player.snapshot().state, PlayerState::Paused);
    assert!(test.consume(1000).iter().all(|sample| *sample == 0.0));
    test.player.play().unwrap();
    assert!(
        test.consume(10_000)
            .iter()
            .any(|sample| sample.abs() > 0.01)
    );
    assert!(test.player.snapshot().position > Duration::ZERO);
    test.player.pause().unwrap();
    test.consume(1000); // the callback observes pause within 5 ms
    let paused = test.player.snapshot();
    assert_eq!(paused.state, PlayerState::Paused);
    test.consume(5000);
    assert_eq!(test.player.snapshot(), paused);
    test.player.play().unwrap();
    test.consume(10_000);
    assert!(test.player.snapshot().position > paused.position);
    test.command(Player::stop).unwrap();
    assert_eq!(test.player.snapshot().state, PlayerState::Stopped);
    assert_eq!(test.player.snapshot().position, Duration::ZERO);
    test.command(Player::play).unwrap();
    test.consume(200_000);
    assert_eq!(test.player.snapshot().state, PlayerState::Ended);
    assert_eq!(
        test.player.snapshot().position,
        test.player.snapshot().duration.unwrap()
    );
    test.command(Player::play).unwrap();
    assert_eq!(test.player.snapshot().state, PlayerState::Playing);
    assert!(
        test.consume(10_000)
            .iter()
            .any(|sample| sample.abs() > 0.01)
    );
}

#[test]
fn seeks_forward_backward_paused_and_after_eof() {
    for name in AUDIO_FIXTURES {
        let mut test = Headless::new();
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::copy(fixture(name), file.path()).unwrap();
        test.command(|player| player.load(file.path())).unwrap();
        test.command(|player| player.seek(Duration::from_millis(900)))
            .unwrap();
        assert_eq!(test.player.snapshot().state, PlayerState::Paused);
        assert_eq!(test.player.snapshot().position, Duration::from_millis(900));
        test.player.play().unwrap();
        test.consume(3000);
        test.command(|player| player.seek(Duration::from_millis(200)))
            .unwrap();
        assert_eq!(test.player.snapshot().state, PlayerState::Playing);
        assert!(test.player.snapshot().position < Duration::from_millis(300));
        test.consume(200_000);
        assert_eq!(test.player.snapshot().state, PlayerState::Ended);
        test.command(|player| player.seek(Duration::from_millis(500)))
            .unwrap();
        assert_eq!(test.player.snapshot().state, PlayerState::Paused);
        assert_eq!(test.player.snapshot().position, Duration::from_millis(500));
        test.command(|player| player.seek(Duration::from_secs(100)))
            .unwrap();
        assert_eq!(
            test.player.snapshot().position,
            test.player.snapshot().duration.unwrap()
        );
        test.command(Player::play).unwrap();
        assert_eq!(test.player.snapshot().state, PlayerState::Playing);
        assert!(test.player.snapshot().position < Duration::from_millis(100));
    }
}

#[test]
fn failed_open_decode_and_seek_preserve_current_track() {
    let mut test = Headless::new();
    let dir = tempfile::tempdir().unwrap();
    let current = dir.path().join("current");
    std::fs::copy(fixture("cbr.mp3"), &current).unwrap();
    test.command(|player| player.load(&current)).unwrap();
    test.player.play().unwrap();
    test.consume(10_000);
    let before = test.player.snapshot();
    assert!(matches!(
        test.player.load(dir.path().join("missing")),
        Err(PlayerError::Open(_))
    ));
    let broken = dir.path().join("broken");
    std::fs::write(&broken, b"not audio").unwrap();
    assert!(matches!(
        test.player.load(&broken),
        Err(PlayerError::Decode(_))
    ));
    std::fs::remove_file(&current).unwrap();
    assert!(matches!(
        test.player.seek(Duration::from_millis(400)),
        Err(PlayerError::Open(_))
    ));
    assert_eq!(test.player.snapshot(), before);
    test.consume(2000);
    assert!(test.player.snapshot().position > before.position);
}

#[test]
fn volume_is_validated_applied_and_retained_on_replacement() {
    let mut test = Headless::new();
    for volume in [-0.1, 1.1, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(matches!(
            test.player.set_volume(volume),
            Err(PlayerError::InvalidVolume)
        ));
        assert_eq!(test.player.snapshot().volume, 1.0);
    }
    test.player.set_volume(0.0).unwrap();
    test.command(|player| player.load(fixture("cbr.mp3")))
        .unwrap();
    test.player.play().unwrap();
    assert!(test.consume(10_000).iter().all(|sample| *sample == 0.0));
    test.player.set_volume(0.5).unwrap();
    assert!(
        test.consume(10_000)
            .iter()
            .any(|sample| sample.abs() > 0.01)
    );
    for name in AUDIO_FIXTURES {
        test.command(|player| player.load(fixture(name))).unwrap();
        assert_eq!(test.player.snapshot().volume, 0.5);
        assert_eq!(test.player.snapshot().state, PlayerState::Paused);
        assert_eq!(test.player.snapshot().position, Duration::ZERO);
        test.player.play().unwrap();
        assert!(
            test.consume(10_000)
                .iter()
                .any(|sample| sample.abs() > 0.01)
        );
    }
}

#[test]
fn source_release_and_shutdown_do_not_require_a_running_audio_callback() {
    let (sent, received) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let mut test = Headless::new();
        test.player.load(fixture("cbr.mp3")).unwrap();
        let first = test.player.source.clone().unwrap();
        test.player.play().unwrap();
        // No mixer sample is consumed: emulate callbacks stopping after device loss.
        test.player.seek(Duration::from_millis(400)).unwrap();
        assert!(lock(&first).is_none());
        let second = test.player.source.clone().unwrap();
        test.player.load(fixture("vbr.mp3")).unwrap();
        assert!(lock(&second).is_none());
        let third = test.player.source.clone().unwrap();
        *lock(&test.player.output_error) = Some("device disconnected".into());
        assert!(matches!(test.player.play(), Err(PlayerError::Output(_))));
        assert_eq!(test.player.snapshot().state, PlayerState::Stopped);
        assert_eq!(
            test.player.output_error().as_deref(),
            Some("device disconnected")
        );
        assert!(matches!(test.player.stop(), Err(PlayerError::Output(_))));
        assert!(lock(&third).is_none());
        drop(test);
        let mut playing = Headless::new();
        playing.player.load(fixture("cbr.mp3")).unwrap();
        playing.player.play().unwrap();
        let active = playing.player.source.clone().unwrap();
        drop(playing);
        assert!(lock(&active).is_none());
        sent.send(()).unwrap();
    });
    received
        .recv_timeout(Duration::from_secs(3))
        .expect("source cleanup must not wait for audio callbacks");
    thread.join().unwrap();
}
