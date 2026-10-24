#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
#![allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::indexing_slicing
)]
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
        assert!(decoder.total_duration().unwrap() <= Duration::from_millis(1700));
        assert_eq!(decoder.channels(), 2.try_into().unwrap());
        assert_eq!(decoder.sample_rate(), 44_100.try_into().unwrap());
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
    test.player.set_parameters(parameters(1.2, false)).unwrap();
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
        test.player.set_parameters(parameters(0.75, false)).unwrap();
        test.player.load(fixture("cbr.mp3")).unwrap();
        let first = test.player.source.clone().unwrap();
        test.player.play().unwrap();
        test.consume(5000); // Allocate DSP, then emulate callbacks stopping after device loss.
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
        playing
            .player
            .set_parameters(parameters(1.5, false))
            .unwrap();
        playing.player.load(fixture("cbr.mp3")).unwrap();
        playing.player.play().unwrap();
        playing.consume(5000);
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

fn parameters(speed: f32, adjust_pitch: bool) -> PlaybackParameters {
    PlaybackParameters {
        speed,
        adjust_pitch,
    }
}

// A generated stereo tone also detects accidental mixing/interleaving of channels.
fn tone(frames: u32, channels: u16, rate: u32) -> tempfile::NamedTempFile {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new().unwrap();
    let size = frames * u32::from(channels) * 2;
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + size).to_le_bytes()).unwrap();
    file.write_all(b"WAVEfmt \x10\0\0\0\x01\0").unwrap();
    file.write_all(&channels.to_le_bytes()).unwrap();
    file.write_all(&rate.to_le_bytes()).unwrap();
    file.write_all(&(rate * u32::from(channels) * 2).to_le_bytes())
        .unwrap();
    file.write_all(&(channels * 2).to_le_bytes()).unwrap();
    file.write_all(&16_u16.to_le_bytes()).unwrap();
    file.write_all(b"data").unwrap();
    file.write_all(&size.to_le_bytes()).unwrap();
    for frame in 0..frames {
        for channel in 0..channels {
            let frequency = f64::from(channel).mul_add(220.0, 440.0);
            let phase = std::f64::consts::TAU * frequency * f64::from(frame) / f64::from(rate);
            file.write_all(&((phase.sin() * 16000.0) as i16).to_le_bytes())
                .unwrap();
        }
    }
    file
}

fn finish(test: &mut Headless) -> Vec<f32> {
    let mut output = Vec::new();
    for _ in 0..1000 {
        output.extend(test.consume(882));
        if test.player.snapshot().state == PlayerState::Ended {
            let ended = test.player.snapshot();
            assert!(test.consume(5000).iter().all(|sample| *sample == 0.0));
            assert_eq!(test.player.snapshot(), ended);
            return output;
        }
    }
    assert_eq!(
        test.player.snapshot().state,
        PlayerState::Ended,
        "audio did not end"
    );
    output
}

fn frequency(samples: &[f32], channel: usize) -> f64 {
    // Ignore edges affected by STFT windows and count upward zero crossings.
    let start = samples.len() / 5 / 2 * 2;
    let end = samples.len() * 4 / 5 / 2 * 2;
    let crossings = samples[start..end]
        .iter()
        .skip(channel)
        .step_by(2)
        .zip(samples[start..end].iter().skip(channel + 2).step_by(2))
        .filter(|(a, b)| **a <= 0.0 && **b > 0.0)
        .count();
    crossings as f64 * 44100.0 / ((end - start) / 2) as f64
}

#[test]
fn speed_and_pitch_match_signal_frequency_and_duration() {
    let file = tone(88_337, 2, 44_100); // non-integral final block
    for speed in [0.5, 0.75, 1.0, 1.2, 1.5, 2.0] {
        for adjust_pitch in [false, true] {
            let mut test = Headless::new();
            test.player
                .set_parameters(parameters(speed, adjust_pitch))
                .unwrap();
            test.player.load(file.path()).unwrap();
            test.player.play().unwrap();
            let output = finish(&mut test);
            let expected = 88_337.0 / 44100.0 / f64::from(speed);
            let elapsed = output.len() as f64 / 88200.0;
            // 10 ms observation chunks, Rodio queue polling and resampling spans.
            assert!(
                (elapsed - expected).abs() < 0.035,
                "{speed}/{adjust_pitch}: {elapsed} vs {expected}"
            );
            for channel in 0..2 {
                let expected = (channel as f64).mul_add(220.0, 440.0)
                    * if adjust_pitch { f64::from(speed) } else { 1.0 };
                let measured = frequency(&output, channel);
                // 1% allows crossing quantization and spectral window transients.
                assert!(
                    (measured / expected - 1.0).abs() < 0.01,
                    "{speed}/{adjust_pitch}, channel {channel}: {measured} vs {expected}"
                );
            }
            assert_eq!(
                test.player.snapshot().parameters,
                parameters(speed, adjust_pitch)
            );
        }
    }
}

#[test]
fn live_parameters_preserve_history_pause_and_source_seek_for_all_codecs() {
    for name in AUDIO_FIXTURES {
        let mut test = Headless::new();
        test.player.load(fixture(name)).unwrap();
        test.player.play().unwrap();
        test.consume(26_460); // 300 ms at 1x
        let before = test.player.snapshot();
        test.player.set_parameters(parameters(1.5, false)).unwrap();
        assert_eq!(test.player.snapshot().position, before.position);
        test.consume(17_640); // 200 ms at 1.5x, with one block of old audio
        let after = test.player.snapshot();
        let advance = after.position.saturating_sub(before.position).as_secs_f64();
        assert!((advance - 0.3).abs() < 0.02, "{name}: {advance}");
        assert_eq!(after.parameters, parameters(1.5, false));
        test.player.pause().unwrap();
        test.consume(1000);
        let paused = test.player.snapshot();
        test.player.set_parameters(parameters(0.75, true)).unwrap();
        test.consume(5000);
        assert_eq!(test.player.snapshot().state, PlayerState::Paused);
        assert_eq!(test.player.snapshot().position, paused.position);
        assert_eq!(test.player.snapshot().parameters, parameters(0.75, true));
        test.player.play().unwrap();
        test.consume(5000);
        assert_eq!(test.player.snapshot().parameters, parameters(0.75, true));
        test.player.pause().unwrap();
        test.consume(1000);
        test.player.seek(Duration::from_millis(400)).unwrap();
        assert_eq!(test.player.snapshot().position, Duration::from_millis(400));
        assert_eq!(test.player.snapshot().state, PlayerState::Paused);
        test.player.play().unwrap();
        test.consume(17_640);
        assert!((test.player.snapshot().position.as_secs_f64() - 0.55).abs() < 0.02);
        // Toggle pitch at speed != 1, then return to the ordinary path.
        test.player.set_parameters(parameters(0.75, false)).unwrap();
        test.consume(5000);
        test.player.set_parameters(parameters(1.0, false)).unwrap();
        finish(&mut test);
    }
}

#[test]
fn invalid_speed_is_atomic_and_short_dsp_tail_is_not_lost() {
    let mut test = Headless::new();
    test.player.set_parameters(parameters(1.2, false)).unwrap();
    test.player.load(fixture("pcm.wav")).unwrap();
    test.player.play().unwrap();
    test.consume(15_000);
    let before = test.player.snapshot();
    for speed in [
        0.0,
        -1.0,
        0.499,
        2.001,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        assert!(matches!(
            test.player.set_parameters(parameters(speed, true)),
            Err(PlayerError::InvalidSpeed)
        ));
        assert_eq!(test.player.snapshot(), before);
    }
    for frames in [137, 513, 10_137] {
        let file = tone(frames, 1, 48_000);
        for speed in [0.5, 1.2, 2.0] {
            let mut stream = Stream::new(
                Player::decode(file.path()).unwrap(),
                parameters(speed, false),
            );
            let samples: Vec<_> = stream.by_ref().collect();
            let expected = (f64::from(frames) / f64::from(speed)).ceil() as usize;
            assert_eq!(samples.len(), expected, "{frames} at {speed}");
            assert!(
                samples.iter().any(|s| s.abs() > 0.01),
                "short signal vanished"
            );
            assert!(
                samples[samples.len().saturating_sub(64)..]
                    .iter()
                    .any(|s| s.abs() > 0.001),
                "tail vanished"
            );
            assert_eq!(stream.next(), None);
            assert_eq!(stream.next(), None);
        }
    }
}

#[test]
fn live_pitch_changes_use_the_existing_source_and_preplay_parameters() {
    let mut test = Headless::new();
    let file = tone(48_000 * 4, 1, 48_000);
    test.player.load(file.path()).unwrap();
    // The caller may set parameters after load, even after paused callbacks.
    test.consume(5000);
    test.player.set_parameters(parameters(1.2, true)).unwrap();
    let source = test.player.source.clone().unwrap();
    std::fs::remove_file(file.path()).unwrap();
    test.player.play().unwrap();
    let output = test.consume(44_100);
    assert!((frequency(&output, 0) / 528.0 - 1.0).abs() < 0.01);
    for parameters in [
        parameters(1.2, false),
        parameters(0.5, false),
        parameters(2.0, true),
        parameters(1.0, false),
    ] {
        let before = test.player.snapshot().position;
        test.player.set_parameters(parameters).unwrap();
        assert_eq!(test.player.snapshot().position, before);
        assert!(Arc::ptr_eq(&source, test.player.source.as_ref().unwrap()));
        let output = test.consume(44_100);
        let expected = 440.0
            * if parameters.adjust_pitch {
                f64::from(parameters.speed)
            } else {
                1.0
            };
        assert!((frequency(&output, 0) / expected - 1.0).abs() < 0.015);
        assert_eq!(test.player.snapshot().parameters, parameters);
    }
}

#[test]
fn pitch_resampling_keeps_fractional_frames_across_blocks() {
    let frames = 100_137;
    let rate = 48_000;
    let file = tone(frames, 1, rate);
    for speed in [0.5, 0.75, 1.2, 1.234_567, 2.0] {
        let mut stream = Stream::new(
            Player::decode(file.path()).unwrap(),
            parameters(speed, true),
        );
        let count = stream.by_ref().count();
        // Rodio rounds the speed-adjusted sample rate to integer Hz; interpolation
        // may drain one endpoint frame. This bound cannot hide per-block drift.
        let from_rate = (rate as f32 * speed) as u32;
        let expected = (f64::from(frames) * f64::from(rate) / f64::from(from_rate)).ceil() as usize;
        assert!(
            count.abs_diff(expected) <= 2,
            "{speed}: {count} vs {expected}"
        );
        assert!(
            (stream.position().as_secs_f64() - f64::from(frames) / f64::from(rate)).abs() < 0.0001
        );
    }
}
