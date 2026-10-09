//! The tape's saturation: the record head driving the tape's magnetisation into its limit.
//!
//! **Treble saturates first.** A deck records through a treble pre-emphasis and plays back through
//! the matching de-emphasis, so the tape's limit arrives 10–20 dB earlier in the treble than in
//! the midrange (`ops: notes/oldify/research_notes`, cassette physics). Putting a pre-emphasis
//! shelf before the curve and its exact inverse after reproduces that headroom gap with no
//! separate model: [`Saturator`] is pre-emphasis → 2× oversampled curve → de-emphasis. The shelf
//! sits at the tape's playback time constant, 120 µs for ferric, 70 µs for chrome and metal, and
//! lifts [`EMPHASIS_DB`].
//!
//! **Saturation 0 is linear.** The curve is crossfaded with the straight line over the first
//! [`FADE_IN`] of the travel, so zero passes the signal through, delayed by the oversampler and
//! within its passband ripple, where the measured reference's lowest setting still saturates (our
//! measurement): an improvement, labelled as one. Drive past that lowers the clip point, from
//! full scale to [`MAX_DRIVE`] below it, and leaves small signals at unity gain, as the reference
//! does.
//!
//! **Two characters.** Soft is an asymmetric hyperbolic tangent (even harmonics, a smooth knee:
//! ferric's early, gentle onset). Hard is `v / (1 + v⁴)^¼`, symmetric with a sharper knee
//! (chrome's "sharp and dissonant" overload). Both have unit slope at zero and are bounded and
//! monotonic.

use crate::biquad::{Biquad, Kind, Section};
use crate::oversample::Oversampler;
use crate::tape::Tape;

/// The pre-emphasis lift, and the de-emphasis cut, in dB. Chosen.
pub const EMPHASIS_DB: f64 = 10.0;
/// Saturation's share of the travel over which the curve fades in from the straight line.
pub const FADE_IN: f32 = 0.05;
/// The drive at full Saturation: the clip point falls 20 dB from full scale.
pub const MAX_DRIVE: f32 = 10.0;
/// The asymmetric curve's offset: its even harmonics. Chosen.
const ASYMMETRY: f32 = 0.15;

/// The curve's shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Character {
    Soft,
    Hard,
}

/// The curve, unit slope at zero.
#[inline]
pub fn curve(character: Character, v: f32) -> f32 {
    match character {
        Character::Soft => {
            let offset = ASYMMETRY.tanh();
            ((v + ASYMMETRY).tanh() - offset) / (1.0 - offset * offset)
        }
        Character::Hard => {
            // Past ±1e6 the curve is ±1 to the last bit; held there, v⁴ never overflows.
            let v = v.clamp(-1e6, 1e6);
            v / (1.0 + v * v * v * v).sqrt().sqrt()
        }
    }
}

/// The emphasis shelf's corner for a tape: its playback time constant, as a frequency.
fn corner_hz(tape: Tape) -> f64 {
    let micros = match tape {
        Tape::I => 120.0,
        Tape::II | Tape::IV => 70.0,
    };
    1.0 / (std::f64::consts::TAU * micros * 1e-6)
}

/// One channel's saturation stage.
#[derive(Clone, Debug)]
pub struct Saturator {
    rate: f64,
    tape: Option<Tape>,
    emphasis: Biquad,
    de_emphasis: Biquad,
    oversampler: Oversampler,
}

impl Saturator {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            rate: sample_rate,
            tape: None,
            emphasis: Biquad::default(),
            de_emphasis: Biquad::default(),
            oversampler: Oversampler::new(),
        }
    }

    /// The delay the stage adds, in samples: the oversampler's.
    pub const LATENCY: usize = crate::oversample::LATENCY_SAMPLES;

    pub fn reset(&mut self) {
        self.emphasis.reset();
        self.de_emphasis.reset();
        self.oversampler.reset();
    }

    /// Sets the emphasis for `tape`, keeping the filters' state.
    fn set_tape(&mut self, tape: Tape) {
        if self.tape == Some(tape) {
            return;
        }
        self.tape = Some(tape);
        let hz = corner_hz(tape);
        let shelf = |db| Section::new(Kind::HighShelf, hz, db, std::f64::consts::FRAC_1_SQRT_2);
        self.emphasis
            .set(shelf(EMPHASIS_DB).coefficients(self.rate));
        self.de_emphasis
            .set(shelf(-EMPHASIS_DB).coefficients(self.rate));
    }

    /// One sample through the stage at `saturation` (0–1).
    #[inline]
    pub fn process(&mut self, x: f32, tape: Tape, saturation: f32, character: Character) -> f32 {
        self.set_tape(tape);
        let drive = 1.0 + (MAX_DRIVE - 1.0) * saturation * saturation;
        let amount = (saturation / FADE_IN).min(1.0);
        let shape = |u: f32| u + amount * (curve(character, drive * u) / drive - u);
        let lifted = self.emphasis.process(f64::from(x)) as f32;
        let (even, odd) = self.oversampler.up(lifted);
        let y = self.oversampler.down(shape(even), shape(odd));
        self.de_emphasis.process(f64::from(y)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_curves_have_unit_slope_at_zero_are_bounded_and_rise() {
        for character in [Character::Soft, Character::Hard] {
            let slope = (curve(character, 1e-4) - curve(character, -1e-4)) / 2e-4;
            assert!((slope - 1.0).abs() < 1e-3, "{character:?}: slope {slope}");
            let mut previous = curve(character, -100.0);
            for i in -1000..=1000 {
                let y = curve(character, i as f32 * 0.1);
                // Rising, give or take the last bit where the curve is flat.
                assert!(
                    y >= previous - 1e-6 && y.abs() < 1.5,
                    "{character:?} at {i}"
                );
                previous = y;
            }
        }
        assert_eq!(curve(Character::Soft, 0.0), 0.0);
        assert_eq!(curve(Character::Hard, 0.0), 0.0);
    }

    #[test]
    fn soft_is_asymmetric_and_hard_is_not() {
        assert!(curve(Character::Soft, 3.0) + curve(Character::Soft, -3.0) < -0.05);
        assert_eq!(curve(Character::Hard, 3.0), -curve(Character::Hard, -3.0));
    }

    /// RMS of a tone through the stage, after it settles.
    fn through(level: f32, hz: f32, saturation: f32) -> (f32, f32) {
        let mut stage = Saturator::new(48_000.0);
        let n = 24_000;
        let (mut input, mut output) = (0.0f32, 0.0f32);
        for i in 0..n {
            let x = level * (std::f32::consts::TAU * hz * i as f32 / 48_000.0).sin();
            let y = stage.process(x, Tape::I, saturation, Character::Soft);
            if i > n / 2 {
                input += x * x;
                output += y * y;
            }
        }
        (input.sqrt(), output.sqrt())
    }

    #[test]
    fn saturation_zero_passes_the_signal_at_unity() {
        for hz in [100.0, 1_000.0, 10_000.0] {
            let (x, y) = through(0.9, hz, 0.0);
            assert!((20.0 * (y / x).log10()).abs() < 0.05, "{hz} Hz");
        }
    }

    #[test]
    fn small_signals_pass_at_unity_and_full_saturation_compresses_a_loud_one() {
        let (x, y) = through(0.01, 1_000.0, 1.0);
        assert!((20.0 * (y / x).log10()).abs() < 0.1);
        let (x, y) = through(0.9, 1_000.0, 1.0);
        assert!(20.0 * (y / x).log10() < -12.0);
    }

    #[test]
    fn the_treble_saturates_before_the_midrange() {
        // Same level, half saturation: the 8 kHz tone loses more than the 500 Hz one.
        let loss = |hz| {
            let (x, y) = through(0.3, hz, 0.5);
            20.0 * (y / x).log10()
        };
        assert!(
            loss(8_000.0) < loss(500.0) - 1.0,
            "{} {}",
            loss(8_000.0),
            loss(500.0)
        );
    }

    #[test]
    fn silence_stays_exact_zero() {
        let mut stage = Saturator::new(48_000.0);
        for _ in 0..1000 {
            assert_eq!(stage.process(0.0, Tape::II, 0.7, Character::Hard), 0.0);
        }
    }
}
