//! Bounded PCM lookahead lets a parameter change restart DSP at the audible
//! position without reopening the file or discarding prefetched source audio.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]

use super::{FileDecoder, PlaybackParameters};
use rodio::{Source, conversions::SampleRateConverter, source::Speed};
use signalsmith_stretch::Stretch;
use std::{collections::VecDeque, time::Duration};

const BLOCK: usize = 512;

pub(super) struct Stream {
    decoder: FileDecoder,
    channels: rodio::ChannelCount,
    rate: rodio::SampleRate,
    input: VecDeque<f32>,
    start: usize,
    decoded: usize,
    eof: bool,
    frame: f64,
    step: f64,
    channel: usize,
    block: VecDeque<f32>,
    dsp: Option<Dsp>,
    pitch: Option<Pitch>,
    active: PlaybackParameters,
    requested: PlaybackParameters,
}

struct Dsp {
    stretch: Stretch,
    feed: usize,
    fraction: f64,
    skip: usize,
    ready: VecDeque<f32>,
    flushed: bool,
}

struct Pitch {
    converter: SampleRateConverter<Speed<Pcm>>,
    feed: usize,
}

struct Pcm {
    samples: VecDeque<f32>,
    channels: rodio::ChannelCount,
    rate: rodio::SampleRate,
}

impl Iterator for Pcm {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        self.samples.pop_front()
    }
}

impl Source for Pcm {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> rodio::ChannelCount {
        self.channels
    }
    fn sample_rate(&self) -> rodio::SampleRate {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Stream {
    pub fn new(decoder: FileDecoder, parameters: PlaybackParameters) -> Self {
        let channels = decoder.channels();
        let rate = decoder.sample_rate();
        Self {
            decoder,
            channels,
            rate,
            input: VecDeque::new(),
            start: 0,
            decoded: 0,
            eof: false,
            frame: 0.0,
            step: 1.0,
            channel: 0,
            block: VecDeque::new(),
            dsp: None,
            pitch: None,
            active: parameters,
            requested: parameters,
        }
    }

    pub fn position(&self) -> Duration {
        Duration::from_secs_f64(self.frame / f64::from(self.rate.get()))
    }

    pub fn parameters(&self) -> PlaybackParameters {
        if self.frame == 0.0 {
            self.requested
        } else {
            self.active
        }
    }

    pub fn set_parameters(&mut self, parameters: PlaybackParameters, paused: bool) {
        self.requested = parameters;
        if paused && self.active != parameters {
            // Retain only an unfinished channel frame; regenerate the rest at this position.
            self.block
                .truncate((self.channels() - self.channel) % self.channels());
            self.dsp = None;
            self.pitch = None;
            self.active = parameters;
        }
    }

    fn channels(&self) -> usize {
        usize::from(self.channels.get())
    }

    fn frame_step(&self) -> f64 {
        self.pitch.as_ref().map_or_else(
            || f64::from(self.active.speed),
            |pitch| {
                f64::from(pitch.converter.inner().sample_rate().get()) / f64::from(self.rate.get())
            },
        )
    }

    fn ensure_input(&mut self, end: usize) {
        while !self.eof && self.decoded < end {
            for _ in 0..self.channels.get() {
                if let Some(sample) = self.decoder.next() {
                    self.input.push_back(sample);
                } else {
                    self.eof = true;
                    return;
                }
            }
            self.decoded += 1;
        }
    }

    /// Copy a bounded window, padding only beyond the decoder's real EOF.
    fn window(&mut self, start: usize, frames: usize) -> Vec<f32> {
        self.ensure_input(start + frames);
        let offset = (start - self.start) * self.channels();
        let count = frames * self.channels();
        let mut samples: Vec<_> = self
            .input
            .iter()
            .skip(offset)
            .take(count)
            .copied()
            .collect();
        samples.resize(count, 0.0);
        samples
    }

    fn start_dsp(&mut self) -> Dsp {
        let mut stretch = Stretch::preset_default(u32::from(self.channels.get()), self.rate.get());
        let start = self.frame as usize;
        let latency = stretch.input_latency();
        stretch.seek(self.window(start, latency), f64::from(self.active.speed));
        let skip = stretch.output_latency() * self.channels();
        Dsp {
            stretch,
            feed: start + latency,
            fraction: 0.0,
            skip,
            ready: VecDeque::new(),
            flushed: false,
        }
    }

    fn render_dsp(&mut self, dsp: &mut Dsp) -> Vec<f32> {
        let speed = f64::from(self.active.speed);
        while dsp.ready.len() < BLOCK * self.channels() && !dsp.flushed {
            let desired = (BLOCK as f64).mul_add(speed, dsp.fraction);
            let mut frames = desired.floor() as usize;
            self.ensure_input(dsp.feed + frames);
            let end = self.decoded + dsp.stretch.input_latency();
            let output_frames = if self.eof {
                frames = frames.min(end.saturating_sub(dsp.feed));
                ((frames as f64 - dsp.fraction) / speed).ceil().max(0.0) as usize
            } else {
                BLOCK
            };
            let mut output;
            if frames == 0 {
                output = vec![0.0; dsp.stretch.output_latency() * self.channels()];
                dsp.stretch.flush(&mut output);
                dsp.flushed = true;
            } else {
                output = vec![0.0; output_frames * self.channels()];
                dsp.stretch
                    .process(self.window(dsp.feed, frames), &mut output);
                dsp.feed += frames;
                dsp.fraction += (output_frames as f64).mul_add(speed, -(frames as f64));
            }
            let skip = dsp.skip.min(output.len());
            dsp.skip -= skip;
            dsp.ready.extend(output.into_iter().skip(skip));
        }
        let mut frames = BLOCK.min(dsp.ready.len() / self.channels());
        if self.eof {
            frames =
                frames.min(((self.decoded as f64 - self.frame).max(0.0) / speed).ceil() as usize);
        }
        dsp.ready.drain(..frames * self.channels()).collect()
    }

    fn render_pitch(&mut self) -> Vec<f32> {
        // Keep Rodio's interpolation phase across blocks to avoid fractional-frame drift.
        let end = (BLOCK as f64).mul_add(self.frame_step(), self.frame).ceil() as usize + 4;
        self.ensure_input(end);
        let feed = self
            .pitch
            .as_ref()
            .map_or(self.frame as usize, |pitch| pitch.feed);
        let count = end.min(self.decoded).saturating_sub(feed);
        let samples = self.window(feed, count);
        let mut pitch = if let Some(mut pitch) = self.pitch.take() {
            pitch
                .converter
                .inner_mut()
                .inner_mut()
                .samples
                .extend(samples);
            pitch.feed += count;
            pitch
        } else {
            let source = Pcm {
                samples: samples.into(),
                channels: self.channels,
                rate: self.rate,
            }
            .speed(self.active.speed);
            let from_rate = source.sample_rate();
            Pitch {
                converter: SampleRateConverter::new(source, from_rate, self.rate, self.channels),
                feed: feed + count,
            }
        };
        let output = pitch
            .converter
            .by_ref()
            .take(BLOCK * self.channels())
            .collect();
        self.pitch = Some(pitch);
        output
    }

    fn prepare_block(&mut self) {
        if !self.block.is_empty() {
            return;
        }
        let start = self.frame as usize;
        let discard = (start - self.start).min(self.decoded - self.start);
        self.input.drain(..discard * self.channels());
        self.start += discard;
        if self.active != self.requested {
            self.active = self.requested;
            self.dsp = None;
            self.pitch = None;
        }
        let preserving = !self.active.adjust_pitch && self.active.speed != 1.0;
        let samples = if preserving {
            let mut dsp = self.dsp.take().unwrap_or_else(|| self.start_dsp());
            let output = self.render_dsp(&mut dsp);
            self.dsp = Some(dsp);
            output
        } else if self.active.adjust_pitch && self.active.speed != 1.0 {
            self.render_pitch()
        } else {
            self.ensure_input(start + BLOCK);
            let frames = BLOCK.min(self.decoded.saturating_sub(start));
            self.window(start, frames)
        };
        self.block = samples.into();
        self.step = self.frame_step();
    }
}

impl Iterator for Stream {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.prepare_block();
        let sample = self.block.pop_front()?;
        self.channel += 1;
        if self.channel == self.channels() {
            self.channel = 0;
            self.frame += self.step;
        }
        Some(sample)
    }
}
