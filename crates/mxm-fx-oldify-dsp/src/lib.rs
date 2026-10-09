//! Framework-free DSP for `mxm-fx-oldify`, per stereo frame:
//!
//! ```text
//! input → Drive → the medium (the cassette engine; the record engine to come) → ÷ Drive
//!   → Width (mid/side) → Output → Mix with the dry input, delayed to match
//! ```
//!
//! **Drive is compensated**: it sets how hard the medium is driven, and the level after it is
//! divided back out, by the drive the sound leaving the medium went in with a latency earlier, so driving harder saturates more and lowers the hiss against the music
//! without getting louder (`ops: notes/oldify/reports`, the cassette signal chain).
//!
//! **The chain has a fixed latency**, [`Oldify::latency`]: the cassette's oversampler and the
//! transport's base delay, the same for every control and every Medium. The dry input is delayed
//! by it, so Mix never combs, and the plugin reports it to the host.
//!
//! **Silence in is exact zero out** once the tails have run out: the noise stops after
//! [`NOISE_HOLD_S`] of silence, and after [`Oldify::tail`] every memory is cleared and the chain
//! is quiet, costing the input scan only, until the input returns.
//!
//! Every continuous control is smoothed by a linear ramp over [`SMOOTHING_S`], held as an integer
//! step count, so how a host cuts its blocks never changes a sample.

use core::fmt;

pub mod biquad;
pub mod cassette;
pub mod oversample;
pub mod saturate;
pub mod spacing;
pub mod tape;
pub mod transport;

use cassette::{Cassette, CassetteControls};

/// The lowest sample rate [`Oldify::new`] accepts.
pub const MIN_SAMPLE_RATE: f32 = 1_000.0;
/// The highest sample rate [`Oldify::new`] accepts.
pub const MAX_SAMPLE_RATE: f32 = 768_000.0;
/// The bottom of Drive's and Output's travel, in decibels.
pub const MIN_GAIN_DB: f32 = -24.0;
/// The top of Drive's and Output's travel, in decibels.
pub const MAX_GAIN_DB: f32 = 24.0;
/// The widest Width: 200 %. Zero is mono and 1 leaves the stereo image as it came.
pub const MAX_WIDTH: f32 = 2.0;
/// How long a control takes to reach a new value: inside the collection's 5–20 ms band for
/// anything that multiplies the audio (mxm-kit's plugin conventions, *Smooth signals, not
/// coefficients*).
pub const SMOOTHING_S: f32 = 0.010;
/// An input sample at or under this counts as silence: −120 dBFS.
pub const SILENCE: f32 = 1e-6;
/// How long the noise keeps going after the input falls silent, then fades over
/// [`NOISE_FADE_S`]. Chosen: long enough that a gap between notes keeps its hiss.
pub const NOISE_HOLD_S: f32 = 0.25;
/// The noise's fade in and out.
pub const NOISE_FADE_S: f32 = 0.05;
/// What the filters' tails need after the transport's line has emptied. Chosen, generous.
const FILTER_TAIL_S: f32 = 0.2;
/// The engine's seed. A Variation control will move it.
const SEED: u32 = 1;

/// Decibels to a linear gain, the same law as nice-plug's `util::db_to_gain` above its −100 dB
/// floor, so a plugin's range and this crate's bounds are the same numbers.
pub fn db_to_gain(db: f32) -> f32 {
    10.0f32.powf(db * 0.05)
}

/// Which medium the sound goes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Medium {
    Cassette,
    /// The record engine is not built yet: the sound passes, delayed by the latency.
    Record,
    /// A record taped to cassette; for now, the cassette alone.
    RecordThenCassette,
}

/// What one block asks for. Gains are linear, Width is a factor (1 is the image as it came), Mix is
/// the wet share of a linear crossfade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Controls {
    pub medium: Medium,
    /// The level into the medium, linear, divided back out after it.
    pub drive: f32,
    /// Stereo width: 0 mono, 1 unchanged, [`MAX_WIDTH`] twice as wide.
    pub width: f32,
    /// The chain's output level, linear.
    pub output: f32,
    /// The wet share: 0 is the dry input, 1 the chain alone.
    pub mix: f32,
    pub cassette: CassetteControls,
}

impl Controls {
    /// The chain as near transparent as it goes: a neutral cassette, every gain at unity. The
    /// output is the input delayed by the latency, through the oversampler's passband.
    pub const IDENTITY: Self = Self {
        medium: Medium::Cassette,
        drive: 1.0,
        width: 1.0,
        output: 1.0,
        mix: 1.0,
        cassette: CassetteControls::NEUTRAL,
    };

    /// The init patch: what the plugin's parameters default to.
    pub const INIT: Self = Self {
        cassette: CassetteControls::INIT,
        ..Self::IDENTITY
    };

    /// The controls inside their public ranges. A non-finite control is its identity value,
    /// never a NaN carried into the audio.
    pub fn bounded(self) -> Self {
        let gain = |value: f32| {
            if value.is_finite() {
                value.clamp(db_to_gain(MIN_GAIN_DB), db_to_gain(MAX_GAIN_DB))
            } else {
                1.0
            }
        };
        let within = |value: f32, max: f32, otherwise: f32| {
            if value.is_finite() {
                value.clamp(0.0, max)
            } else {
                otherwise
            }
        };
        Self {
            medium: self.medium,
            drive: gain(self.drive),
            width: within(self.width, MAX_WIDTH, 1.0),
            output: gain(self.output),
            mix: within(self.mix, 1.0, 1.0),
            cassette: self.cassette.bounded(),
        }
    }
}

impl Default for Controls {
    fn default() -> Self {
        Self::INIT
    }
}

/// A sample rate outside [`MIN_SAMPLE_RATE`]..=[`MAX_SAMPLE_RATE`], or not a number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnsupportedRate(pub f32);

impl fmt::Display for UnsupportedRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} Hz is outside the supported {MIN_SAMPLE_RATE}–{MAX_SAMPLE_RATE} Hz",
            self.0
        )
    }
}

impl std::error::Error for UnsupportedRate {}

/// One smoothed control: a linear ramp from `start` to `target` over `steps` samples.
///
/// The position is an integer, and the value is computed from it rather than accumulated, so
/// advancing by `n` at once lands on exactly the value `n` single steps would: a silent block
/// skipped whole and the same block processed sample by sample agree to the bit.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ramp {
    start: f32,
    target: f32,
    step: u32,
    steps: u32,
}

impl Ramp {
    pub(crate) fn settled_at(value: f32, steps: u32) -> Self {
        Self {
            start: value,
            target: value,
            step: steps,
            steps,
        }
    }

    /// A new target starts a new ramp from where this one is. The same target again changes
    /// nothing, so a host repeating a value every block does not restart the ramp.
    pub(crate) fn retarget(&mut self, target: f32) {
        if target != self.target {
            self.start = self.value();
            self.target = target;
            self.step = 0;
        }
    }

    pub(crate) fn value(&self) -> f32 {
        if self.step >= self.steps {
            self.target
        } else {
            self.start + (self.target - self.start) * (self.step as f32 / self.steps as f32)
        }
    }

    pub(crate) fn advance(&mut self, samples: u32) {
        self.step = self.step.saturating_add(samples).min(self.steps);
    }

    pub(crate) fn next(&mut self) -> f32 {
        self.advance(1);
        self.value()
    }

    pub(crate) fn is_settled(&self) -> bool {
        self.step >= self.steps
    }

    pub(crate) fn snap(&mut self) {
        self.step = self.steps;
    }
}

/// The dry input, delayed by the chain's latency.
#[derive(Debug, Clone)]
struct DryLine {
    line: [Vec<f32>; 2],
    head: usize,
}

impl DryLine {
    fn new(length: usize) -> Self {
        Self {
            line: [vec![0.0; length.max(1)], vec![0.0; length.max(1)]],
            head: 0,
        }
    }

    fn clear(&mut self) {
        self.line.iter_mut().for_each(|l| l.fill(0.0));
    }

    /// Writes this frame and returns the one from the line's length ago.
    #[inline]
    fn exchange(&mut self, frame: [f32; 2]) -> [f32; 2] {
        let out = [self.line[0][self.head], self.line[1][self.head]];
        self.line[0][self.head] = frame[0];
        self.line[1][self.head] = frame[1];
        self.head = (self.head + 1) % self.line[0].len();
        out
    }
}

/// The stereo processor. Allocates in [`Oldify::new`] only (the delay lines), never after: every
/// other method is realtime-safe.
#[derive(Debug, Clone)]
pub struct Oldify {
    medium: Medium,
    drive: Ramp,
    width: Ramp,
    output: Ramp,
    mix: Ramp,
    cassette: Cassette,
    dry: DryLine,
    /// Drive as it was one latency ago: what the sound leaving the medium was driven by.
    driven: Vec<f32>,
    driven_head: usize,
    latency: usize,
    /// Samples since the input was last above [`SILENCE`], saturating.
    silent_for: u64,
    noise_hold: u64,
    noise: Ramp,
    /// After this much silence every memory is cleared: [`Oldify::tail`].
    quiet_after: u64,
    /// True from a clear until the input returns: the output is exact zero.
    quiet: bool,
}

impl Oldify {
    /// A processor at `sample_rate`, every control at [`Controls::IDENTITY`] and settled.
    pub fn new(sample_rate: f32) -> Result<Self, UnsupportedRate> {
        // `contains` is false for NaN, so a non-number is refused with the rest.
        if !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&sample_rate) {
            return Err(UnsupportedRate(sample_rate));
        }
        let steps = ((sample_rate * SMOOTHING_S).round() as u32).max(1);
        let identity = Controls::IDENTITY;
        let cassette = Cassette::new(f64::from(sample_rate), steps, SEED);
        let latency = cassette.latency();
        let seconds = |s: f32| (sample_rate * s).round() as u64;
        Ok(Self {
            medium: identity.medium,
            drive: Ramp::settled_at(identity.drive, steps),
            width: Ramp::settled_at(identity.width, steps),
            output: Ramp::settled_at(identity.output, steps),
            mix: Ramp::settled_at(identity.mix, steps),
            cassette,
            dry: DryLine::new(latency),
            driven: vec![identity.drive; latency.max(1)],
            driven_head: 0,
            latency,
            silent_for: u64::MAX,
            noise_hold: seconds(NOISE_HOLD_S),
            noise: Ramp::settled_at(0.0, seconds(NOISE_FADE_S).max(1) as u32),
            quiet_after: 2 * latency as u64 + seconds(NOISE_HOLD_S + NOISE_FADE_S + FILTER_TAIL_S),
            quiet: true,
        })
    }

    /// The chain's delay, in samples, the same for every control: report it to the host.
    pub fn latency(&self) -> u32 {
        self.latency as u32
    }

    /// How long after the input falls silent the output is exact zero, in samples.
    pub fn tail(&self) -> u32 {
        u32::try_from(self.quiet_after).unwrap_or(u32::MAX)
    }

    /// Asks for new controls; each continuous one that changed ramps there over [`SMOOTHING_S`].
    pub fn set_controls(&mut self, controls: Controls) {
        let controls = controls.bounded();
        let cassette_was_on = self.medium != Medium::Record;
        self.medium = controls.medium;
        if self.medium != Medium::Record && !cassette_was_on {
            // Back into the cassette: it starts from silence, not from where it was left.
            self.cassette.clear();
            self.driven.fill(self.drive.value());
        }
        self.drive.retarget(controls.drive);
        self.width.retarget(controls.width);
        self.output.retarget(controls.output);
        self.mix.retarget(controls.mix);
        self.cassette.set_controls(controls.cassette);
    }

    /// Ends every ramp and clears every memory: the chain is quiet, as if long silent.
    pub fn reset(&mut self) {
        self.drive.snap();
        self.width.snap();
        self.output.snap();
        self.mix.snap();
        self.cassette.snap();
        self.noise = Ramp::settled_at(0.0, self.noise.steps);
        self.clear();
        self.silent_for = u64::MAX;
    }

    fn clear(&mut self) {
        self.cassette.clear();
        self.dry.clear();
        self.quiet = true;
    }

    /// The outer controls as they stand this sample; the cassette's are its own.
    pub fn current(&self) -> (f32, f32, f32, f32) {
        (
            self.drive.value(),
            self.width.value(),
            self.output.value(),
            self.mix.value(),
        )
    }

    /// True when no control is ramping.
    pub fn is_settled(&self) -> bool {
        self.drive.is_settled()
            && self.width.is_settled()
            && self.output.is_settled()
            && self.mix.is_settled()
            && self.cassette.is_settled()
    }

    /// Processes one block in place. `controls` are this block's targets.
    ///
    /// Both channels are processed up to the shorter one's length. A non-finite input sample is
    /// taken as silence. A quiet chain fed silence costs the input scan only and gives exact zero.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], controls: Controls) {
        self.set_controls(controls);
        let frames = left.len().min(right.len());
        let (left, right) = (&mut left[..frames], &mut right[..frames]);

        let mut silent = true;
        for sample in left.iter_mut().chain(right.iter_mut()) {
            if !sample.is_finite() {
                *sample = 0.0;
            } else if sample.abs() > SILENCE {
                silent = false;
            }
        }
        if silent && self.quiet {
            left.fill(0.0);
            right.fill(0.0);
            self.advance(frames);
            return;
        }
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            [*l, *r] = self.frame([*l, *r]);
        }
    }

    /// Moves every ramp and the silence count on by `frames`, processing nothing: what the
    /// per-sample path does to a quiet chain fed silence.
    fn advance(&mut self, frames: usize) {
        let samples = u32::try_from(frames).unwrap_or(u32::MAX);
        self.drive.advance(samples);
        self.width.advance(samples);
        self.output.advance(samples);
        self.mix.advance(samples);
        self.cassette.advance(samples);
        self.noise.advance(samples);
        self.silent_for = self.silent_for.saturating_add(frames as u64);
    }

    /// One stereo frame through the chain.
    #[inline]
    fn frame(&mut self, input: [f32; 2]) -> [f32; 2] {
        let active = input[0].abs() > SILENCE || input[1].abs() > SILENCE;
        self.silent_for = if active {
            0
        } else {
            self.silent_for.saturating_add(1)
        };
        self.noise.retarget(if self.silent_for < self.noise_hold {
            1.0
        } else {
            0.0
        });
        let noise = self.noise.next();
        let drive = self.drive.next();
        let width = self.width.next();
        let output = self.output.next();
        let mix = self.mix.next();

        if self.quiet {
            if !active {
                self.cassette.advance(1);
                return [0.0, 0.0];
            }
            self.quiet = false;
            // Nothing is in the medium: whatever drive is now is what all of it was driven by.
            let drive_now = drive;
            self.driven.fill(drive_now);
        }

        let dry = self.dry.exchange(input);
        let mut wet = match self.medium {
            Medium::Record => dry,
            Medium::Cassette | Medium::RecordThenCassette => {
                let driven = [input[0] * drive, input[1] * drive];
                let taped = self.cassette.process(driven, noise);
                // Divided by the drive the sound now leaving was driven by, a latency ago: by
                // today's, a Drive move would swing the level by up to its whole range.
                let then = self.driven[self.driven_head];
                self.driven[self.driven_head] = drive;
                self.driven_head = (self.driven_head + 1) % self.driven.len();
                [taped[0] / then, taped[1] / then]
            }
        };
        if self.medium == Medium::Record {
            self.cassette.advance(1);
        }

        // Width, about the mid: zero is the mid alone in both channels, 2 doubles the side.
        if width != 1.0 {
            let mid = (wet[0] + wet[1]) * 0.5;
            let side = (wet[0] - wet[1]) * 0.5 * width;
            wet = [mid + side, mid - side];
        }
        wet = [wet[0] * output, wet[1] * output];

        let out = if mix == 1.0 {
            wet
        } else if mix == 0.0 {
            dry
        } else {
            [
                dry[0] * (1.0 - mix) + wet[0] * mix,
                dry[1] * (1.0 - mix) + wet[1] * mix,
            ]
        };
        if self.silent_for >= self.quiet_after {
            self.clear();
        }
        out.map(flush)
    }
}

/// A subnormal sample as exact zero. **No output sample is subnormal**: nothing upstream flushes
/// them (nice-plug's flush-to-zero is behind a feature the collection leaves off), a decaying
/// filter or a subnormal input passes them straight through, and a host's validator rejects them.
#[inline]
fn flush(sample: f32) -> f32 {
    if sample.abs() < f32::MIN_POSITIVE {
        0.0
    } else {
        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::{HISS_MAX_DB, Motion};
    use crate::saturate::Character;
    use crate::tape::{Grade, Tape};
    use crate::transport::Shape;

    const RATE: f32 = 48_000.0;

    /// Deterministic noise in −1..1 (a 32-bit LCG), so every run hears the same programme.
    fn noise(frames: usize, seed: u32) -> Vec<f32> {
        let mut state = seed;
        (0..frames)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 8) as f32 / (1u32 << 23) as f32 - 1.0
            })
            .collect()
    }

    fn bits(samples: &[f32]) -> Vec<u32> {
        samples.iter().map(|sample| sample.to_bits()).collect()
    }

    fn settled_at(controls: Controls) -> Oldify {
        let mut oldify = Oldify::new(RATE).unwrap();
        oldify.set_controls(controls);
        oldify.reset();
        oldify
    }

    fn render(
        oldify: &mut Oldify,
        left: &[f32],
        right: &[f32],
        controls: Controls,
    ) -> [Vec<f32>; 2] {
        let (mut l, mut r) = (left.to_vec(), right.to_vec());
        oldify.process(&mut l, &mut r, controls);
        [l, r]
    }

    /// Everything at its far end: what a test of extremes runs.
    fn extremes() -> Vec<Controls> {
        let gains = [db_to_gain(MIN_GAIN_DB), 1.0, db_to_gain(MAX_GAIN_DB)];
        let loud = CassetteControls {
            tape: Tape::II,
            grade: Grade::Economy,
            tone: 1.0,
            saturation: 1.0,
            character: Character::Hard,
            wow: Motion {
                depth: 1.0,
                hz: 0.1,
                shape: Shape::Random,
            },
            flutter: Motion {
                depth: 1.0,
                hz: 30.0,
                shape: Shape::Sine,
            },
            hiss_db: HISS_MAX_DB,
            age: 1.0,
        };
        let mut all = Vec::new();
        for medium in [Medium::Cassette, Medium::Record] {
            for cassette in [CassetteControls::NEUTRAL, CassetteControls::INIT, loud] {
                for drive in gains {
                    for output in gains {
                        for (width, mix) in [(0.0, 0.0), (1.0, 0.5), (MAX_WIDTH, 1.0)] {
                            all.push(Controls {
                                medium,
                                drive,
                                width,
                                output,
                                mix,
                                cassette,
                            });
                        }
                    }
                }
            }
        }
        all
    }

    #[test]
    fn silence_in_is_exact_zero_out_at_every_setting_and_through_every_ramp() {
        for controls in extremes() {
            let mut oldify = Oldify::new(RATE).unwrap();
            for block in 0..2 {
                let [left, right] = render(&mut oldify, &[0.0; 512], &[0.0; 512], controls);
                assert!(
                    left.iter().chain(&right).all(|sample| *sample == 0.0),
                    "{controls:?} block {block} put something into silence"
                );
            }
        }
    }

    #[test]
    fn after_the_tail_silence_is_exact_zero_again() {
        let mut oldify = settled_at(Controls::INIT);
        let source = noise(9_600, 1);
        render(&mut oldify, &source, &source, Controls::INIT);
        let tail = oldify.tail() as usize;
        let [left, right] = render(
            &mut oldify,
            &vec![0.0; tail + 512],
            &vec![0.0; tail + 512],
            Controls::INIT,
        );
        assert!(left[..tail].iter().any(|s| *s != 0.0), "no tail at all");
        assert!(left[tail..].iter().chain(&right[tail..]).all(|s| *s == 0.0));
    }

    #[test]
    fn mix_zero_is_the_dry_input_delayed_by_the_latency_to_the_bit() {
        let (source_l, source_r) = (noise(8_192, 2), noise(8_192, 3));
        for controls in extremes().into_iter().filter(|c| c.mix == 0.0) {
            let mut oldify = settled_at(controls);
            let latency = oldify.latency() as usize;
            let [left, right] = render(&mut oldify, &source_l, &source_r, controls);
            assert!(left[..latency].iter().all(|s| *s == 0.0));
            assert_eq!(
                bits(&left[latency..]),
                bits(&source_l[..8_192 - latency]),
                "{controls:?}"
            );
            assert_eq!(bits(&right[latency..]), bits(&source_r[..8_192 - latency]));
        }
    }

    #[test]
    fn the_record_medium_passes_the_input_delayed_to_the_bit() {
        let controls = Controls {
            medium: Medium::Record,
            ..Controls::INIT
        };
        let source = noise(8_192, 4);
        let mut oldify = settled_at(controls);
        let latency = oldify.latency() as usize;
        let [left, _] = render(&mut oldify, &source, &source, controls);
        assert_eq!(bits(&left[latency..]), bits(&source[..8_192 - latency]));
    }

    #[test]
    fn the_neutral_cassette_is_the_input_delayed_through_the_passband() {
        // A band-limited programme (the oversampler's passband ends near 20 kHz at 48 kHz).
        let n = 16_384;
        let source: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / RATE;
                0.4 * (std::f32::consts::TAU * 220.0 * t).sin()
                    + 0.3 * (std::f32::consts::TAU * 3_100.0 * t).sin()
                    + 0.2 * (std::f32::consts::TAU * 12_000.0 * t).sin()
            })
            .collect();
        let mut oldify = settled_at(Controls::IDENTITY);
        let latency = oldify.latency() as usize;
        let [left, _] = render(&mut oldify, &source, &source, Controls::IDENTITY);
        let error: f32 = (latency + 2_000..n)
            .map(|i| (left[i] - source[i - latency]).powi(2))
            .sum();
        let signal: f32 = (latency + 2_000..n)
            .map(|i| source[i - latency].powi(2))
            .sum();
        let db = 10.0 * (error / signal).log10();
        assert!(
            db < -50.0,
            "neutral differs from the delayed input by {db:.1} dB"
        );
    }

    #[test]
    fn an_impulse_at_neutral_peaks_at_the_reported_latency() {
        let mut oldify = settled_at(Controls::IDENTITY);
        let latency = oldify.latency() as usize;
        let mut impulse = vec![0.0f32; 4 * latency];
        impulse[0] = 0.5;
        let [left, _] = render(&mut oldify, &impulse, &impulse, Controls::IDENTITY);
        let peak = (0..left.len())
            .max_by(|&a, &b| left[a].abs().total_cmp(&left[b].abs()))
            .unwrap();
        assert_eq!(peak, latency);
    }

    /// **How a host cuts its blocks never changes a sample**, with every random source running:
    /// the programme falls silent long enough for the chain to go quiet, returns, and is retargeted
    /// on a sample every block size here starts a block on.
    #[test]
    fn blocks_of_one_thirty_seven_and_five_hundred_twelve_are_identical() {
        let quiet = Oldify::new(RATE).unwrap().tail() as usize + 4_000;
        let length = 30_000 + quiet;
        let retarget = 37 * 512;
        let mut source_l = noise(length, 5);
        let mut source_r = noise(length, 6);
        source_l[200..900].fill(0.0);
        source_r[200..900].fill(0.0);
        source_l[25_000..25_000 + quiet].fill(0.0);
        source_r[25_000..25_000 + quiet].fill(0.0);
        let first = Controls {
            drive: db_to_gain(9.0),
            width: 1.6,
            output: db_to_gain(-7.0),
            mix: 0.7,
            ..Controls::INIT
        };
        let second = Controls {
            drive: db_to_gain(-6.0),
            width: 0.3,
            mix: 0.4,
            cassette: CassetteControls {
                tape: Tape::IV,
                saturation: 0.9,
                age: 0.8,
                ..CassetteControls::INIT
            },
            ..first
        };
        let run = |block: usize| {
            let mut oldify = Oldify::new(RATE).unwrap();
            let (mut left, mut right) = (source_l.clone(), source_r.clone());
            let mut start = 0;
            while start < length {
                let end = (start + block).min(length);
                let controls = if start < retarget { first } else { second };
                oldify.process(&mut left[start..end], &mut right[start..end], controls);
                start = end;
            }
            (bits(&left), bits(&right))
        };
        let reference = run(1);
        assert_ne!(reference.0, bits(&source_l), "the chain did nothing");
        for block in [37, 512] {
            assert!(
                run(block) == reference,
                "blocks of {block} differ from blocks of one"
            );
        }
    }

    #[test]
    fn a_reset_renders_the_same_again() {
        let source = noise(12_000, 7);
        let mut oldify = settled_at(Controls::INIT);
        let first = render(&mut oldify, &source, &source, Controls::INIT);
        oldify.reset();
        assert_eq!(render(&mut oldify, &source, &source, Controls::INIT), first);
    }

    #[test]
    fn extreme_and_non_finite_settings_and_input_stay_finite() {
        let mut source_l = noise(4_096, 8);
        let mut source_r = noise(4_096, 9);
        for (index, sample) in source_l.iter_mut().enumerate().take(1_024) {
            *sample = if index % 64 < 32 { 1.0 } else { -1.0 };
        }
        source_r[1_024..2_048].fill(1.0);
        let mut settings = extremes();
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            settings.push(Controls {
                drive: bad,
                width: bad,
                output: bad,
                mix: bad,
                cassette: CassetteControls {
                    tone: bad,
                    saturation: bad,
                    wow: Motion {
                        depth: bad,
                        hz: bad,
                        shape: Shape::Saw,
                    },
                    flutter: Motion {
                        depth: bad,
                        hz: bad,
                        shape: Shape::Random,
                    },
                    hiss_db: bad,
                    age: bad,
                    ..CassetteControls::INIT
                },
                ..Controls::INIT
            });
        }
        // Full scale through the loudest gains, widest image and the tape's own lift.
        let ceiling = 4.0 * db_to_gain(MAX_GAIN_DB) * MAX_WIDTH + 1.0;
        for controls in settings {
            let mut oldify = Oldify::new(RATE).unwrap();
            for _ in 0..2 {
                let [left, right] = render(&mut oldify, &source_l, &source_r, controls);
                for sample in left.iter().chain(&right) {
                    assert!(
                        sample.is_finite() && sample.abs() <= ceiling,
                        "{controls:?} gave {sample}"
                    );
                }
            }
        }

        // Non-finite audio is taken as silence.
        let mut oldify = settled_at(Controls::INIT);
        let mut left = vec![f32::NAN, f32::INFINITY, 0.25, f32::NEG_INFINITY];
        let mut right = vec![0.5, f32::NAN, f32::NAN, 0.0];
        oldify.process(&mut left, &mut right, Controls::INIT);
        assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
    }

    #[test]
    fn width_zero_is_mono() {
        let controls = Controls {
            width: 0.0,
            ..Controls::INIT
        };
        let mut oldify = settled_at(controls);
        let [left, right] = render(&mut oldify, &noise(8_192, 10), &noise(8_192, 11), controls);
        assert_eq!(bits(&left), bits(&right));
        assert!(left.iter().any(|s| *s != 0.0));
    }

    #[test]
    fn hiss_plays_at_its_level_while_there_is_input_and_stops_after() {
        // A tone 70 dB under the hiss keeps the chain awake; what comes out is the hiss.
        let controls = Controls {
            cassette: CassetteControls {
                hiss_db: -40.0,
                ..CassetteControls::NEUTRAL
            },
            ..Controls::IDENTITY
        };
        let n = 48_000;
        let quiet: Vec<f32> = (0..n)
            .map(|i| 1e-5 * (std::f32::consts::TAU * 1_000.0 * i as f32 / RATE).sin())
            .collect();
        let mut oldify = settled_at(controls);
        let [left, _] = render(&mut oldify, &quiet, &quiet, controls);
        let rms = (left[n / 2..].iter().map(|s| s * s).sum::<f32>() / (n / 2) as f32).sqrt();
        let db = 20.0 * rms.log10();
        assert!((db + 40.0).abs() < 1.0, "hiss at {db:.1} dB, asked for -40");

        let hold = (RATE * (NOISE_HOLD_S + NOISE_FADE_S)) as usize + oldify.latency() as usize;
        let [after, _] = render(
            &mut oldify,
            &vec![0.0; hold + 4_800],
            &vec![0.0; hold + 4_800],
            controls,
        );
        assert!(
            after[hold..].iter().all(|s| s.abs() < 1e-6),
            "the hiss kept going in silence"
        );
    }

    #[test]
    fn a_changed_control_ramps_over_the_smoothing_time_and_lands_exactly() {
        let mut oldify = Oldify::new(RATE).unwrap();
        let target = db_to_gain(MAX_GAIN_DB);
        oldify.set_controls(Controls {
            output: target,
            ..Controls::IDENTITY
        });
        let steps = (RATE * SMOOTHING_S).round() as usize;
        let largest = (target - 1.0) / steps as f32 * 1.001;
        let mut previous = oldify.current().2;
        assert_eq!(previous, 1.0, "the ramp jumped before the first sample");
        for sample in 1..=steps {
            let output = oldify.output.next();
            assert!(
                output - previous <= largest,
                "a step of {}",
                output - previous
            );
            assert!(output > previous, "the ramp stalled at sample {sample}");
            previous = output;
        }
        assert_eq!(previous, target);

        // Asking again for the target being ramped to does not restart the ramp.
        let mut oldify = Oldify::new(RATE).unwrap();
        let wanted = Controls {
            mix: 0.0,
            ..Controls::IDENTITY
        };
        oldify.set_controls(wanted);
        oldify.advance(steps / 2);
        let halfway = oldify.current();
        oldify.set_controls(wanted);
        assert_eq!(oldify.current(), halfway);

        // A reset ends the ramp at its target.
        oldify.reset();
        assert_eq!(oldify.current().3, 0.0);
        assert!(oldify.is_settled());
    }

    #[test]
    fn the_latency_is_the_same_for_every_control_and_near_fifty_milliseconds() {
        for rate in [44_100.0, RATE, 96_000.0] {
            let oldify = Oldify::new(rate).unwrap();
            let ms = oldify.latency() as f32 / rate * 1_000.0;
            assert!((45.0..60.0).contains(&ms), "{rate} Hz: {ms} ms");
        }
    }

    /// **No output sample is subnormal**, at any setting, even when the input carries subnormal
    /// samples between ordinary ones: the dry path and the Record medium would pass them through.
    #[test]
    fn no_output_sample_is_subnormal() {
        let frames = Oldify::new(RATE).unwrap().latency() as usize + 1_024;
        let mut source = noise(frames, 12);
        for (index, sample) in source.iter_mut().enumerate() {
            if index % 2 == 1 {
                *sample = if index % 4 == 1 { 1e-40 } else { -3e-42 };
            }
        }
        assert!(source.iter().any(|s| s.is_subnormal()));
        for controls in extremes() {
            let mut oldify = settled_at(controls);
            let [left, right] = render(&mut oldify, &source, &source, controls);
            assert!(
                !left.iter().chain(&right).any(|s| s.is_subnormal()),
                "{controls:?} put out a subnormal sample"
            );
        }
    }

    #[test]
    fn rates_outside_the_public_range_are_refused() {
        for rate in [MIN_SAMPLE_RATE, 44_100.0, RATE, 192_000.0, MAX_SAMPLE_RATE] {
            assert!(Oldify::new(rate).is_ok(), "{rate}");
        }
        for rate in [
            0.0,
            -48_000.0,
            MIN_SAMPLE_RATE - 1.0,
            MAX_SAMPLE_RATE + 1.0,
            f32::NAN,
            f32::INFINITY,
        ] {
            assert!(Oldify::new(rate).is_err(), "{rate}");
        }
    }
}
