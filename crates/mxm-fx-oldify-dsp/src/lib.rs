//! Framework-free DSP for `mxm-fx-oldify`.
//!
//! **A placeholder chain.** The cassette and record engines are not built yet. What runs is the
//! frame they will sit in, per stereo frame:
//!
//! ```text
//! input → Drive gain → (the engines, later) → Width (mid/side) → Output gain → Mix with the dry input
//! ```
//!
//! The dry input is taken as it arrived, not delayed: there is no latency yet. Every control
//! multiplies the audio, so every control is smoothed — a linear ramp over [`SMOOTHING_S`], held
//! as an integer step count so that how a host cuts its blocks never changes a sample.

use core::fmt;

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

/// Decibels to a linear gain, the same law as nice-plug's `util::db_to_gain` above its −100 dB
/// floor, so a plugin's range and this crate's bounds are the same numbers.
pub fn db_to_gain(db: f32) -> f32 {
    10.0f32.powf(db * 0.05)
}

/// What one block asks for. Gains are linear, Width is a factor (1 is the image as it came), Mix is
/// the wet share of a linear crossfade.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Controls {
    /// The input level into the chain, linear.
    pub drive: f32,
    /// Stereo width: 0 mono, 1 unchanged, [`MAX_WIDTH`] twice as wide.
    pub width: f32,
    /// The chain's output level, linear.
    pub output: f32,
    /// The wet share: 0 is the dry input, 1 the chain alone.
    pub mix: f32,
}

impl Controls {
    /// Every control at the value that leaves the audio as it came.
    pub const IDENTITY: Self = Self {
        drive: 1.0,
        width: 1.0,
        output: 1.0,
        mix: 1.0,
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
            drive: gain(self.drive),
            width: within(self.width, MAX_WIDTH, 1.0),
            output: gain(self.output),
            mix: within(self.mix, 1.0, 1.0),
        }
    }
}

impl Default for Controls {
    fn default() -> Self {
        Self::IDENTITY
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
struct Ramp {
    start: f32,
    target: f32,
    step: u32,
    steps: u32,
}

impl Ramp {
    fn settled_at(value: f32, steps: u32) -> Self {
        Self {
            start: value,
            target: value,
            step: steps,
            steps,
        }
    }

    /// A new target starts a new ramp from where this one is. The same target again changes
    /// nothing, so a host repeating a value every block does not restart the ramp.
    fn retarget(&mut self, target: f32) {
        if target != self.target {
            self.start = self.value();
            self.target = target;
            self.step = 0;
        }
    }

    fn value(&self) -> f32 {
        if self.step >= self.steps {
            self.target
        } else {
            self.start + (self.target - self.start) * (self.step as f32 / self.steps as f32)
        }
    }

    fn advance(&mut self, samples: u32) {
        self.step = self.step.saturating_add(samples).min(self.steps);
    }

    fn next(&mut self) -> f32 {
        self.advance(1);
        self.value()
    }

    fn is_settled(&self) -> bool {
        self.step >= self.steps
    }

    fn snap(&mut self) {
        self.step = self.steps;
    }
}

/// The stereo processor. Allocates nothing, ever; every method is realtime-safe.
#[derive(Debug, Clone)]
pub struct Oldify {
    drive: Ramp,
    width: Ramp,
    output: Ramp,
    mix: Ramp,
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
        Ok(Self {
            drive: Ramp::settled_at(identity.drive, steps),
            width: Ramp::settled_at(identity.width, steps),
            output: Ramp::settled_at(identity.output, steps),
            mix: Ramp::settled_at(identity.mix, steps),
        })
    }

    /// Asks for new controls; each one that changed ramps there over [`SMOOTHING_S`].
    pub fn set_controls(&mut self, controls: Controls) {
        let controls = controls.bounded();
        self.drive.retarget(controls.drive);
        self.width.retarget(controls.width);
        self.output.retarget(controls.output);
        self.mix.retarget(controls.mix);
    }

    /// Ends every ramp in progress: each control is at its target from the next sample. The
    /// chain holds no audio yet, so this is all a reset has to clear.
    pub fn reset(&mut self) {
        self.drive.snap();
        self.width.snap();
        self.output.snap();
        self.mix.snap();
    }

    /// The controls the processor is heading for (and, once settled, at).
    pub fn targets(&self) -> Controls {
        Controls {
            drive: self.drive.target,
            width: self.width.target,
            output: self.output.target,
            mix: self.mix.target,
        }
    }

    /// The controls as they stand this sample.
    pub fn current(&self) -> Controls {
        Controls {
            drive: self.drive.value(),
            width: self.width.value(),
            output: self.output.value(),
            mix: self.mix.value(),
        }
    }

    /// True when no control is ramping.
    pub fn is_settled(&self) -> bool {
        self.drive.is_settled()
            && self.width.is_settled()
            && self.output.is_settled()
            && self.mix.is_settled()
    }

    /// Processes one block in place. `controls` are this block's targets.
    ///
    /// Both channels are processed up to the shorter one's length. A non-finite input sample is
    /// taken as silence. Three blocks cost no more than that scan, and each gives exactly what the
    /// per-sample path would:
    ///
    /// - **silence in** is exact zero out, whatever the controls; the ramps advance across it;
    /// - **Mix settled at zero** is the dry input, untouched;
    /// - **every control settled at its identity** is the input, untouched.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], controls: Controls) {
        self.set_controls(controls);
        let frames = left.len().min(right.len());
        let (left, right) = (&mut left[..frames], &mut right[..frames]);

        let mut silent = true;
        for sample in left.iter_mut().chain(right.iter_mut()) {
            if !sample.is_finite() {
                *sample = 0.0;
            } else if *sample != 0.0 {
                silent = false;
            }
        }
        if silent {
            self.advance(frames);
            return;
        }
        if self.is_settled() && (self.mix.target == 0.0 || self.targets() == Controls::IDENTITY) {
            return;
        }

        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let current = Controls {
                drive: self.drive.next(),
                width: self.width.next(),
                output: self.output.next(),
                mix: self.mix.next(),
            };
            [*l, *r] = frame([*l, *r], current);
        }
    }

    fn advance(&mut self, frames: usize) {
        let samples = u32::try_from(frames).unwrap_or(u32::MAX);
        self.drive.advance(samples);
        self.width.advance(samples);
        self.output.advance(samples);
        self.mix.advance(samples);
    }
}

/// One stereo frame through the chain at `controls`, which are taken as already bounded.
///
/// Width is skipped at exactly 1 and the crossfade's ends are taken exactly, so the identity
/// controls return the frame to the bit and Mix zero returns the dry frame to the bit.
pub fn frame(dry: [f32; 2], controls: Controls) -> [f32; 2] {
    // Drive: the level into the medium.
    let mut wet = [dry[0] * controls.drive, dry[1] * controls.drive];

    // The cassette and record engines will run here, between Drive and Width.

    // Width, about the mid: zero is the mid alone in both channels, 2 doubles the side.
    if controls.width != 1.0 {
        let mid = (wet[0] + wet[1]) * 0.5;
        let side = (wet[0] - wet[1]) * 0.5 * controls.width;
        wet = [mid + side, mid - side];
    }

    wet = [wet[0] * controls.output, wet[1] * controls.output];

    let mix = controls.mix;
    if mix == 1.0 {
        wet
    } else if mix == 0.0 {
        dry
    } else {
        [
            dry[0] * (1.0 - mix) + wet[0] * mix,
            dry[1] * (1.0 - mix) + wet[1] * mix,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Every combination of the controls' ends and middles, the ones a test of extremes needs.
    fn extremes() -> Vec<Controls> {
        let gains = [db_to_gain(MIN_GAIN_DB), 1.0, db_to_gain(MAX_GAIN_DB)];
        let mut all = Vec::new();
        for drive in gains {
            for width in [0.0, 1.0, MAX_WIDTH] {
                for output in gains {
                    for mix in [0.0, 0.5, 1.0] {
                        all.push(Controls {
                            drive,
                            width,
                            output,
                            mix,
                        });
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
            // Two blocks: the first ramps from the identity, the second settles.
            for block in 0..2 {
                let mut left = vec![0.0f32; 512];
                let mut right = vec![0.0f32; 512];
                oldify.process(&mut left, &mut right, controls);
                assert!(
                    left.iter().chain(&right).all(|sample| *sample == 0.0),
                    "{controls:?} block {block} put something into silence"
                );
            }
        }
        // And the per-sample path itself, which the silent block never reaches.
        for controls in extremes() {
            assert_eq!(frame([0.0, 0.0], controls), [0.0, 0.0], "{controls:?}");
        }
    }

    #[test]
    fn mix_zero_is_the_dry_input_to_the_bit() {
        let source_l = noise(4096, 1);
        let source_r = noise(4096, 2);
        for controls in extremes().into_iter().filter(|c| c.mix == 0.0) {
            let mut oldify = settled_at(controls);
            let (mut left, mut right) = (source_l.clone(), source_r.clone());
            oldify.process(&mut left, &mut right, controls);
            assert_eq!(bits(&left), bits(&source_l), "{controls:?}");
            assert_eq!(bits(&right), bits(&source_r), "{controls:?}");
            // The per-sample path ends the crossfade the same way.
            assert_eq!(
                frame([source_l[7], source_r[7]], controls).map(f32::to_bits),
                [source_l[7].to_bits(), source_r[7].to_bits()]
            );
        }

        // A Mix ramping down to zero lands on the dry input exactly, once the ramp is over.
        let wet = Controls {
            drive: db_to_gain(12.0),
            width: 1.7,
            ..Controls::IDENTITY
        };
        let mut oldify = settled_at(wet);
        let (mut left, mut right) = (source_l.clone(), source_r.clone());
        oldify.process(&mut left, &mut right, Controls { mix: 0.0, ..wet });
        let ramp = (RATE * SMOOTHING_S).round() as usize;
        assert_ne!(
            left[..ramp],
            source_l[..ramp],
            "Mix jumped instead of ramping"
        );
        assert_eq!(bits(&left[ramp..]), bits(&source_l[ramp..]));
        assert_eq!(bits(&right[ramp..]), bits(&source_r[ramp..]));
    }

    #[test]
    fn the_identity_settings_pass_the_input_to_the_bit() {
        let source_l = noise(4096, 3);
        let source_r = noise(4096, 4);
        let mut oldify = Oldify::new(RATE).unwrap();
        let (mut left, mut right) = (source_l.clone(), source_r.clone());
        oldify.process(&mut left, &mut right, Controls::IDENTITY);
        assert_eq!(bits(&left), bits(&source_l));
        assert_eq!(bits(&right), bits(&source_r));

        // The per-sample path, which a settled identity skips, is exact too.
        for (l, r) in source_l.iter().zip(&source_r) {
            assert_eq!(
                frame([*l, *r], Controls::IDENTITY).map(f32::to_bits),
                [l.to_bits(), r.to_bits()]
            );
        }

        // And a ramp that comes back to the identity ends exact.
        let mut oldify = settled_at(Controls {
            drive: 3.0,
            width: 0.2,
            output: 0.5,
            mix: 0.6,
        });
        let (mut left, mut right) = (source_l.clone(), source_r.clone());
        oldify.process(&mut left, &mut right, Controls::IDENTITY);
        let ramp = (RATE * SMOOTHING_S).round() as usize;
        assert_eq!(bits(&left[ramp..]), bits(&source_l[ramp..]));
        assert_eq!(bits(&right[ramp..]), bits(&source_r[ramp..]));
    }

    /// **How a host cuts its blocks never changes a sample.** The programme ramps from the
    /// identity, falls silent part-way through the ramp (where a silent block is skipped whole),
    /// returns, and is retargeted again on a sample every block size here starts a block on.
    #[test]
    fn blocks_of_one_thirty_seven_and_five_hundred_twelve_are_identical() {
        const LENGTH: usize = 24_000;
        const RETARGET: usize = 37 * 512;
        let mut source_l = noise(LENGTH, 5);
        let mut source_r = noise(LENGTH, 6);
        source_l[200..900].fill(0.0);
        source_r[200..900].fill(0.0);
        let first = Controls {
            drive: db_to_gain(9.0),
            width: 1.6,
            output: db_to_gain(-7.0),
            mix: 0.7,
        };
        let second = Controls {
            drive: db_to_gain(-15.0),
            width: 0.3,
            output: db_to_gain(20.0),
            mix: 0.4,
        };

        let render = |block: usize| {
            let mut oldify = Oldify::new(RATE).unwrap();
            let (mut left, mut right) = (source_l.clone(), source_r.clone());
            let mut start = 0;
            while start < LENGTH {
                let end = (start + block).min(LENGTH);
                let controls = if start < RETARGET { first } else { second };
                oldify.process(&mut left[start..end], &mut right[start..end], controls);
                start = end;
            }
            (bits(&left), bits(&right))
        };
        let reference = render(1);
        assert_ne!(reference.0, bits(&source_l), "the chain did nothing");
        for block in [37, 512] {
            assert!(
                render(block) == reference,
                "blocks of {block} differ from blocks of one"
            );
        }
    }

    #[test]
    fn extreme_and_non_finite_settings_and_input_stay_finite() {
        let mut source_l = noise(2048, 7);
        let mut source_r = noise(2048, 8);
        // Full scale, a square edge and a DC hold: the hardest ordinary audio for the gains.
        for (index, sample) in source_l.iter_mut().enumerate().take(512) {
            *sample = if index % 64 < 32 { 1.0 } else { -1.0 };
        }
        source_r[512..1024].fill(1.0);

        let mut settings = extremes();
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            settings.push(Controls {
                drive: bad,
                width: bad,
                output: bad,
                mix: bad,
            });
        }
        let ceiling = db_to_gain(MAX_GAIN_DB) * MAX_WIDTH * db_to_gain(MAX_GAIN_DB) + 1.0;
        for controls in settings {
            let mut oldify = Oldify::new(RATE).unwrap();
            // Two blocks: the first ramps from the identity, the second is settled.
            for _ in 0..2 {
                let (mut left, mut right) = (source_l.clone(), source_r.clone());
                oldify.process(&mut left, &mut right, controls);
                for sample in left.iter().chain(&right) {
                    assert!(
                        sample.is_finite() && sample.abs() <= ceiling,
                        "{controls:?} gave {sample}"
                    );
                }
            }
        }

        // Non-finite audio is taken as silence.
        let mut oldify = settled_at(Controls {
            drive: 2.0,
            mix: 0.5,
            ..Controls::IDENTITY
        });
        let mut left = vec![f32::NAN, f32::INFINITY, 0.25, f32::NEG_INFINITY];
        let mut right = vec![0.5, f32::NAN, f32::NAN, 0.0];
        oldify.process(&mut left, &mut right, oldify.targets());
        assert!(left.iter().chain(&right).all(|sample| sample.is_finite()));
        assert_eq!(left[0], 0.0);
    }

    #[test]
    fn width_zero_is_mono_and_the_mid_is_kept_at_every_width() {
        for width in [0.0, 0.5, 1.0, 1.5, MAX_WIDTH] {
            let controls = Controls {
                width,
                ..Controls::IDENTITY
            };
            let [l, r] = frame([0.75, -0.25], controls);
            assert!(
                ((l + r) * 0.5 - 0.25).abs() < 1e-6,
                "{width}: the mid moved"
            );
            assert!(
                ((l - r) * 0.5 - 0.5 * width).abs() < 1e-6,
                "{width}: the side"
            );
            if width == 0.0 {
                assert_eq!(l, r, "Width zero is not mono");
            }
        }
    }

    #[test]
    fn a_changed_control_ramps_over_the_smoothing_time_and_lands_exactly() {
        let mut oldify = Oldify::new(RATE).unwrap();
        let target = db_to_gain(MAX_GAIN_DB);
        oldify.set_controls(Controls {
            drive: target,
            ..Controls::IDENTITY
        });
        let steps = (RATE * SMOOTHING_S).round() as usize;
        let largest = (target - 1.0) / steps as f32 * 1.001;
        let mut previous = oldify.current().drive;
        assert_eq!(previous, 1.0, "the ramp jumped before the first sample");
        for sample in 1..=steps {
            let drive = oldify.drive.next();
            assert!(
                drive - previous <= largest,
                "a step of {}",
                drive - previous
            );
            assert!(drive > previous, "the ramp stalled at sample {sample}");
            previous = drive;
        }
        assert_eq!(previous, target);
        assert!(oldify.is_settled());

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
        assert_eq!(oldify.current(), wanted);
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
