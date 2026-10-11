//! Print-through: a faint copy of the music a wrap of the tape later.
//!
//! **A stored tape magnetises the layers wound against it.** Each layer's recorded field reaches
//! the layers either side, and over months on the shelf they take a faint copy of it. A cassette is
//! wound coating out, so the copy the outer layer takes is the louder: the echo comes after the
//! music, a wrap of the reel later, about 1.45 s near the hub to 3.2 s at full pack (`ops:
//! notes/oldify/reports`, the cassette physics). Here the wrap is the one halfway through a C60's
//! tape ([`wrap_s`]). No measured cassette level exists; real ones are put at −60 to −45 dB, and the
//! travel goes on to [`MAX_PRINT_DB`] for effect.
//!
//! **Its band is the layer's thickness.** The field a recorded wavelength λ throws across a gap d
//! goes as k·e^(−kd), k = 2π/λ: a long wave's field is weak, a short one's dies across the gap,
//! and the strongest copy is at λ = 2πd. At cassette speed v the copy is the music through
//! (f/f₀)·e^(1 − f/f₀), f₀ = v / 2πd, about 470 Hz for a C60's tape ([`peak_hz`]): rising 6 dB an
//! octave below, falling fast above. Here a first-order high-pass and two low-pass sections,
//! fitted to that within a decibel down to 50 dB under its peak ([`SHAPE`]).
//!
//! **Only the echo after**: the one before needs a wrap of look-ahead, and on a cassette it is the
//! quieter. It is printed on the tape, so it plays back as the music does: through the wobbles,
//! the losses, a dropout and a stop.

use crate::biquad::{Biquad, Coefficients, Kind, Section};
use crate::cassette::SPEED_M_S;
use std::f64::consts::PI;

/// Print-through at or below this is off.
pub const PRINT_OFF_DB: f32 = -80.0;
/// The loudest print-through, dB under the music at its band's peak: far past a real tape's, for
/// effect (the owner, 2026-10-11: "our plugin should scale into the 'non physical' for creative
/// use"). Chosen, as the groove echo's.
pub const MAX_PRINT_DB: f32 = -12.0;
/// The cassette's hub radius, metres: the plan's assumption, which puts a wrap at the hub at
/// 1.45 s.
const HUB_RADIUS_M: f64 = 0.011;
/// A C60's tape: thirty minutes a side at cassette speed, metres, and its thickness, base and
/// coating, metres.
const TAPE_LENGTH_M: f64 = 30.0 * 60.0 * SPEED_M_S;
const TAPE_THICKNESS_M: f64 = 16e-6;
/// The fit of the print's band, as shares of [`peak_hz`] and Q: a first-order high-pass, then
/// two low-pass sections (`ops: notes/oldify`, fitted 2026-10-11 to 0.7 dB RMS over the band
/// above −50 dB).
const SHAPE: (f64, [(f64, f64); 2]) = (0.365, [(1.51, 0.88), (3.79, 1.65)]);

/// The wrap a stored cassette prints across, halfway through its tape, seconds: the pack's radius
/// where half the tape is wound on, `√(r₀² + L·d / 2π)`, once round at cassette speed.
pub fn wrap_s() -> f64 {
    let radius =
        (HUB_RADIUS_M * HUB_RADIUS_M + TAPE_LENGTH_M * TAPE_THICKNESS_M / (2.0 * PI)).sqrt();
    2.0 * PI * radius / SPEED_M_S
}

/// The wrap in samples at `sample_rate`.
pub fn wrap_samples(sample_rate: f64) -> usize {
    ((wrap_s() * sample_rate).round() as usize).max(1)
}

/// Where the copy is strongest, hertz: the wavelength 2π times the tape's thickness.
pub fn peak_hz() -> f64 {
    SPEED_M_S / (2.0 * PI * TAPE_THICKNESS_M)
}

/// A first-order high-pass at `hz`, by the bilinear transform.
fn high_pass(hz: f64, sample_rate: f64) -> Coefficients {
    let k = (PI * hz.min(0.45 * sample_rate) / sample_rate).tan();
    Coefficients {
        b0: 1.0 / (1.0 + k),
        b1: -1.0 / (1.0 + k),
        b2: 0.0,
        a1: (k - 1.0) / (k + 1.0),
        a2: 0.0,
    }
}

/// One channel's band.
#[derive(Clone, Debug, Default)]
struct Band([Biquad; 3]);

impl Band {
    fn new(sample_rate: f64) -> Self {
        let peak = peak_hz();
        let (high, lows) = SHAPE;
        let low = |(share, q): (f64, f64)| {
            let mut b = Biquad::default();
            b.set(Section::new(Kind::LowPass, share * peak, 0.0, q).coefficients(sample_rate));
            b
        };
        let mut first = Biquad::default();
        first.set(high_pass(high * peak, sample_rate));
        Self([first, low(lows[0]), low(lows[1])])
    }

    fn reset(&mut self) {
        self.0.iter_mut().for_each(Biquad::reset);
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        self.0.iter_mut().fold(x, |v, b| b.process(v))
    }
}

/// One wrap's line, written with the recorded tape and read a wrap behind.
#[derive(Clone, Debug)]
pub struct PrintThrough {
    line: [Vec<f32>; 2],
    head: usize,
    /// Samples written since the last clear, up to the line's length: anything older reads as
    /// silence, so a clear costs nothing however long the line.
    filled: usize,
    band: [Band; 2],
}

impl PrintThrough {
    /// Allocates the wrap: call outside the audio thread.
    pub fn new(sample_rate: f64) -> Self {
        let length = wrap_samples(sample_rate);
        Self {
            line: [vec![0.0; length], vec![0.0; length]],
            head: 0,
            filled: 0,
            band: [Band::new(sample_rate), Band::new(sample_rate)],
        }
    }

    /// Empties the line and the band.
    pub fn clear(&mut self) {
        self.filled = 0;
        self.band.iter_mut().for_each(Band::reset);
    }

    /// The recorded tape `x` with the copy printed from a wrap before at `gain`; writes `x` into
    /// the line whatever the gain, so the copy is there the moment it is turned up.
    #[inline]
    pub fn process(&mut self, x: [f32; 2], gain: f32) -> [f32; 2] {
        let length = self.line[0].len();
        let mut out = x;
        if gain > 0.0 {
            // The slot about to be written holds the sample a whole wrap ago.
            for ((out, line), band) in out.iter_mut().zip(&self.line).zip(&mut self.band) {
                let then = if self.filled >= length {
                    f64::from(line[self.head])
                } else {
                    0.0
                };
                *out += gain * band.process(then) as f32;
            }
        }
        for (line, x) in self.line.iter_mut().zip(x) {
            line[self.head] = x;
        }
        self.head = (self.head + 1) % length;
        self.filled = (self.filled + 1).min(length);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    const RATE: f64 = 48_000.0;

    /// The plan's geometry: a wrap at the hub is 1.45 s and at full pack about 3.2 s, so halfway
    /// through the tape it is between; the band peaks near the cited 470 Hz for a C60.
    #[test]
    fn the_wrap_and_the_band_are_a_c60s() {
        let hub = 2.0 * PI * HUB_RADIUS_M / SPEED_M_S;
        assert!((hub - 1.45).abs() < 0.01, "{hub}");
        let full =
            2.0 * PI * (HUB_RADIUS_M * HUB_RADIUS_M + TAPE_LENGTH_M * TAPE_THICKNESS_M / PI).sqrt()
                / SPEED_M_S;
        assert!((full - 3.2).abs() < 0.15, "{full}");
        assert!(wrap_s() > hub && wrap_s() < full, "{}", wrap_s());
        assert!((peak_hz() - 470.0).abs() < 10.0, "{}", peak_hz());
    }

    /// A tone through the print-through, then silence: the copy's level a wrap later, in dB.
    fn printed(hz: f64, db: f32) -> (Vec<f32>, f64) {
        let mut print = PrintThrough::new(RATE);
        let wrap = wrap_samples(RATE);
        let gain = crate::db_to_gain(db);
        let out: Vec<f32> = (0..wrap + 9_600)
            .map(|i| {
                let x = if i < 9_600 {
                    (TAU * hz * i as f64 / RATE).sin() as f32
                } else {
                    0.0
                };
                print.process([x, x], gain)[0]
            })
            .collect();
        let rms = |x: &[f32]| {
            (x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
        };
        let db = 20.0
            * (rms(&out[wrap + 2_400..wrap + 7_200]) / std::f64::consts::FRAC_1_SQRT_2).log10();
        (out, db)
    }

    /// The copy comes exactly a wrap late, at its level at the band's peak, and the treble and
    /// the deep bass far under it.
    #[test]
    fn the_copy_comes_a_wrap_late_at_its_level_in_its_band() {
        let wrap = wrap_samples(RATE);
        let (out, db) = printed(peak_hz(), -40.0);
        assert!(
            out[9_600..wrap].iter().all(|v| *v == 0.0),
            "heard before its wrap"
        );
        // The tone starts at zero, so its copy's first sound is a sample after the wrap.
        assert!(out[wrap] == 0.0 && out[wrap + 1] != 0.0);
        assert!((db + 40.0).abs() < 1.0, "{db:.2} dB at the peak");
        assert!(printed(4_000.0, -40.0).1 < -80.0);
        assert!(printed(50.0, -40.0).1 < -50.0);
    }

    /// Off leaves the tape as it is, to the bit, and a clear forgets what was written.
    #[test]
    fn off_is_exact_and_a_clear_forgets() {
        let mut print = PrintThrough::new(RATE);
        let wrap = wrap_samples(RATE);
        for i in 0..wrap {
            let x = (i as f32 * 0.37).sin();
            assert_eq!(print.process([x, -x], 0.0), [x, -x]);
        }
        print.clear();
        for _ in 0..2 * wrap {
            assert_eq!(print.process([0.0, 0.0], 1.0), [0.0, 0.0]);
        }
    }
}
