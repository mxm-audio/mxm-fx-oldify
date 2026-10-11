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
//! **High quality doubles the doubling**: the curve runs at four times the rate, through a second
//! half-band pair inside the first, so less of what the curve adds above the base Nyquist folds
//! back. Normal delays its curve's output by that pair's round trip, so both have one latency; the
//! last doubled-rate samples are kept, so a switch brings the stage it moves to up to date at once
//! and never starts it cold.
//!
//! **Two characters.** Soft is an asymmetric hyperbolic tangent (even harmonics, a smooth knee:
//! ferric's early, gentle onset). Hard is `v / (1 + v⁴)^¼`, symmetric with a sharper knee
//! (chrome's "sharp and dissonant" overload). Both have unit slope at zero and are bounded and
//! monotonic.

use crate::biquad::{Biquad, Kind, Section};
use crate::oversample::{HALF_TAPS, LATENCY_SAMPLES, Oversampler};
use crate::tape::Tape;

/// The pre-emphasis lift, and the de-emphasis cut, in dB. Chosen.
pub const EMPHASIS_DB: f64 = 10.0;
/// Saturation's share of the travel over which the curve fades in from the straight line.
pub const FADE_IN: f32 = 0.05;
/// The drive at full Saturation: the clip point falls 20 dB from full scale.
pub const MAX_DRIVE: f32 = 10.0;
/// The asymmetric curve's offset: its even harmonics. Chosen.
const ASYMMETRY: f32 = 0.15;
/// The inner pair's round trip, in samples at the doubled rate: what Normal delays its curve by.
const PAD: usize = LATENCY_SAMPLES;
/// The doubled-rate samples kept to bring the inner pair up to date when High comes on: enough to
/// refill both its histories.
const KEEP: usize = 4 * HALF_TAPS + 1;

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
    /// High's second doubling, and Normal's delay in its place.
    inner: Oversampler,
    pad: [f32; PAD],
    pad_head: usize,
    /// The last doubled-rate samples into the curve, oldest at the head.
    kept: [f32; KEEP],
    kept_head: usize,
    /// The quality running, and the one asked for.
    high: bool,
    wanted: bool,
}

impl Saturator {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            rate: sample_rate,
            tape: None,
            emphasis: Biquad::default(),
            de_emphasis: Biquad::default(),
            oversampler: Oversampler::new(),
            inner: Oversampler::new(),
            pad: [0.0; PAD],
            pad_head: 0,
            kept: [0.0; KEEP],
            kept_head: 0,
            high: false,
            wanted: false,
        }
    }

    /// The delay the stage adds, in samples, at either quality: the outer pair's round trip and the
    /// inner's at the doubled rate.
    pub const LATENCY: usize = LATENCY_SAMPLES + LATENCY_SAMPLES / 2;

    pub fn reset(&mut self) {
        self.emphasis.reset();
        self.de_emphasis.reset();
        self.oversampler.reset();
        self.inner.reset();
        self.pad = [0.0; PAD];
        self.pad_head = 0;
        self.kept = [0.0; KEEP];
        self.kept_head = 0;
        self.high = self.wanted;
    }

    /// High quality, the curve at four times the rate, from the next sample on.
    pub fn set_high(&mut self, high: bool) {
        self.wanted = high;
    }

    /// One doubled-rate sample through the inner pair, the curve at four times the rate.
    #[inline]
    fn twice(&mut self, v: f32, shape: &impl Fn(f32) -> f32) -> f32 {
        let (a, b) = self.inner.up(v);
        self.inner.down(shape(a), shape(b))
    }

    /// One doubled-rate sample through Normal's delay.
    #[inline]
    fn delay(&mut self, v: f32) -> f32 {
        let out = self.pad[self.pad_head];
        self.pad[self.pad_head] = v;
        self.pad_head = (self.pad_head + 1) % PAD;
        out
    }

    /// Moves to the quality asked for, its stage brought up to date from the samples kept: the
    /// inner pair run over them, or Normal's delay filled with their curve.
    fn switch(&mut self, shape: &impl Fn(f32) -> f32) {
        self.high = self.wanted;
        if self.high {
            self.inner.reset();
            for i in 0..KEEP {
                let v = self.kept[(self.kept_head + i) % KEEP];
                let _ = self.twice(v, shape);
            }
        } else {
            for i in KEEP - PAD..KEEP {
                let v = self.kept[(self.kept_head + i) % KEEP];
                let _ = self.delay(shape(v));
            }
        }
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
        if self.wanted != self.high {
            self.switch(&shape);
        }
        let (even_out, odd_out) = if self.high {
            (self.twice(even, &shape), self.twice(odd, &shape))
        } else {
            (self.delay(shape(even)), self.delay(shape(odd)))
        };
        for v in [even, odd] {
            self.kept[self.kept_head] = v;
            self.kept_head = (self.kept_head + 1) % KEEP;
        }
        let y = self.oversampler.down(even_out, odd_out);
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

    /// A 7 kHz tone of `level` through `saturation` of `character` at 48 kHz, quality `high`:
    /// what is not the tone or its harmonics between 100 Hz and 12 kHz, the aliases, against the
    /// tone, dB.
    fn aliases_db(high: bool, level: f32, saturation: f32, character: Character) -> f32 {
        let (rate, hz) = (48_000.0f32, 7_000.0f32);
        let mut stage = Saturator::new(f64::from(rate));
        stage.set_high(high);
        stage.reset();
        let n = 1 << 15;
        let out: Vec<f32> = (0..2 * n)
            .map(|i| {
                let x = level * (std::f32::consts::TAU * hz * i as f32 / rate).sin();
                stage.process(x, Tape::II, saturation, character)
            })
            .collect();
        // A Hann-windowed DFT of the settled half, bin by bin: the tone's harmonics and their
        // neighbours, against everything else.
        let y = &out[n..];
        let (mut tone, mut rest) = (0.0f64, 0.0f64);
        for bin in 1..n / 2 {
            let f = bin as f32 * rate / n as f32;
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in y.iter().enumerate() {
                let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
                let a = std::f64::consts::TAU * bin as f64 * i as f64 / n as f64;
                re += f64::from(*v) * w * a.cos();
                im -= f64::from(*v) * w * a.sin();
            }
            let power = re * re + im * im;
            let harmonic = (1..4).any(|k| (f - hz * k as f32).abs() < 30.0);
            if harmonic {
                tone += power;
            } else if f > 100.0 && f < 12_000.0 {
                // Above the soft curve's own offset, which the window spreads a few bins.
                rest += power;
            }
        }
        (10.0 * (rest / tone).log10()) as f32
    }

    /// **High folds back less**: a tape driven as tapes are, its aliases far under Normal's; at
    /// the hardest drive, whose harmonics fall as slowly as a square wave's, the one doubling's
    /// worth.
    #[test]
    #[ignore = "a slow DFT; run when the oversampling changes"]
    fn high_quality_aliases_less_than_normal() {
        for (level, saturation, character, better) in [
            (0.5, 0.5, Character::Soft, 10.0),
            (0.9, 1.0, Character::Hard, 5.0),
        ] {
            let normal = aliases_db(false, level, saturation, character);
            let high = aliases_db(true, level, saturation, character);
            println!("{character:?} at {saturation}: normal {normal:.1} dB, high {high:.1} dB");
            assert!(
                high < normal - better,
                "normal {normal:.1} dB, high {high:.1} dB"
            );
        }
    }

    /// **Both qualities have one latency, and a switch is seamless**: at saturation zero both
    /// pass the signal delayed by `LATENCY`, and switching back and forth mid-tone moves no sample
    /// further than the tone itself does.
    #[test]
    fn both_qualities_share_one_latency_and_switch_without_a_step() {
        for high in [false, true] {
            let mut stage = Saturator::new(48_000.0);
            stage.set_high(high);
            stage.reset();
            let out: Vec<f32> = (0..200)
                .map(|i| {
                    let x = if i == 10 { 1.0 } else { 0.0 };
                    stage.process(x, Tape::I, 0.0, Character::Soft)
                })
                .collect();
            let peak = (0..out.len())
                .max_by(|&a, &b| out[a].abs().total_cmp(&out[b].abs()))
                .unwrap();
            assert_eq!(peak, 10 + Saturator::LATENCY, "high {high}");
        }
        let mut stage = Saturator::new(48_000.0);
        let step = |i: usize| 0.8 * (std::f32::consts::TAU * 440.0 * i as f32 / 48_000.0).sin();
        let mut previous = 0.0f32;
        let mut largest = 0.0f32;
        for i in 0..9_600 {
            stage.set_high((i / 1_200) % 2 == 1);
            let y = stage.process(step(i), Tape::I, 0.6, Character::Soft);
            if i > 100 {
                largest = largest.max((y - previous).abs());
            }
            previous = y;
        }
        // A 440 Hz tone at 0.8 moves at most 0.046 a sample.
        assert!(largest < 0.05, "a switch stepped {largest}");
    }

    #[test]
    fn silence_stays_exact_zero() {
        let mut stage = Saturator::new(48_000.0);
        for _ in 0..1000 {
            assert_eq!(stage.process(0.0, Tape::II, 0.7, Character::Hard), 0.0);
        }
    }
}
