//! Record equalisation curves: how a disc was cut, and how it was played back.
//!
//! A record is cut with the treble lifted and the bass held down (the **cutting curve**), and
//! played back through the curve's inverse (the **replay curve**). Each period curve is stated by
//! its time constants, `T₁` (the low shelf, often absent), `T₂` (the turnover) and `T₃` (the treble
//! roll-off, often absent); its replay response is
//!
//! ```text
//! H(s) = (1 + s·T₂) / ((1 + s·T₁)(1 + s·T₃))
//! ```
//!
//! normalised to 0 dB at 1 kHz, and the cutting curve is its exact inverse, the cut's treble lift
//! capped at [`CEILING_S`] as a cutting amplifier caps it. The bilinear transform squeezes the top
//! octave (about 1.3 dB at 10 kHz at 48 kHz); a cut and a replay through the same transform still
//! cancel exactly, and a mismatched pair errs on both alike. A curve with no low
//! shelf keeps lifting the bass on replay down to [`MISSING_SHELF_S`], a stand-in for the replay
//! amplifier's own limit. Cut with one curve and played with another, the mismatch emerges by
//! itself: a UK 78 played through the standard curve loses about 13.5 dB at 10 kHz.
//!
//! The constants are the period's, as restoration practice states them
//! (`ops: notes/oldify/reports`, the era layer, from Copeland's *Manual of Analogue Sound
//! Restoration* §6 and IASA TC-04). The labels are descriptive: no maker's name.

use std::f64::consts::TAU;

/// A curve with no low shelf: its replay bass lift stops here, about 20 Hz.
pub const MISSING_SHELF_S: f64 = 7_958e-6;
/// The cutting amplifier's own ceiling on the treble lift, 3.18 µs (about 50 kHz), mirrored on
/// replay: without it a cutting curve's lift would rise without bound.
pub const CEILING_S: f64 = 3.18e-6;

/// The period curves, by the label the player sees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Curve {
    /// No equalisation: a flat cut, or a crystal pickup's roughly flat replay.
    Flat,
    /// UK 78s, coarse groove, 1943–53: 250 Hz turnover.
    Uk78,
    /// UK 78s with treble lift, 1944–56: 300 Hz turnover, 25 µs.
    Uk78Treble,
    /// US 78s, 1932–49: 318 µs turnover, 63.6 µs.
    Us78,
    /// Early US LPs, 1948–54: 1590 / 400 / 100 µs.
    EarlyLp,
    /// Early 45s, 1949–52: 200 µs turnover, 63.6 µs.
    Early45,
    /// Broadcast transcriptions: 2250 / 275 / 100 µs.
    Transcription,
    /// West-coast LPs from 1950: 398 µs turnover, 63.6 µs.
    WestCoastLp,
    /// The standard from 1952–55 on: 3180 / 318 / 75 µs.
    Standard,
}

impl Curve {
    pub const ALL: [Curve; 9] = [
        Curve::Flat,
        Curve::Uk78,
        Curve::Uk78Treble,
        Curve::Us78,
        Curve::EarlyLp,
        Curve::Early45,
        Curve::Transcription,
        Curve::WestCoastLp,
        Curve::Standard,
    ];

    /// `(T₁, T₂, T₃)` in seconds; `None` for a constant the curve does not have. Flat has none.
    pub fn time_constants(self) -> (Option<f64>, Option<f64>, Option<f64>) {
        let us = |v: f64| Some(v * 1e-6);
        match self {
            Curve::Flat => (None, None, None),
            Curve::Uk78 => (None, us(636.0), None),
            Curve::Uk78Treble => (None, us(531.0), us(25.0)),
            Curve::Us78 => (None, us(318.0), us(63.6)),
            Curve::EarlyLp => (us(1_590.0), us(400.0), us(100.0)),
            Curve::Early45 => (None, us(200.0), us(63.6)),
            Curve::Transcription => (us(2_250.0), us(275.0), us(100.0)),
            Curve::WestCoastLp => (None, us(398.0), us(63.6)),
            Curve::Standard => (us(3_180.0), us(318.0), us(75.0)),
        }
    }

    /// The replay response's magnitude at `hz`, normalised to 1 at 1 kHz: the analogue curve.
    pub fn replay_gain(self, hz: f64) -> f64 {
        let raw = |f: f64| {
            let (t1, t2, t3) = self.time_constants();
            let w = TAU * f;
            let mag = |t: Option<f64>| t.map_or(1.0, |t| (1.0 + (w * t).powi(2)).sqrt());
            let shelf = if t2.is_some() {
                Some(t1.unwrap_or(MISSING_SHELF_S))
            } else {
                t1
            };
            mag(t2) / (mag(shelf) * mag(t3))
        };
        raw(hz) / raw(1_000.0)
    }
}

/// A first-order section `(1 + s·zero) / (1 + s·pole)` by the bilinear transform, prewarped at
/// 1 kHz, where every curve is normalised. Either constant may be absent (`None` is 1).
#[derive(Clone, Copy, Debug, Default)]
struct FirstOrder {
    b0: f64,
    b1: f64,
    a1: f64,
    state: f64,
}

impl FirstOrder {
    fn new(zero: Option<f64>, pole: Option<f64>, rate: f64) -> Self {
        if zero.is_none() && pole.is_none() {
            return Self {
                b0: 1.0,
                ..Self::default()
            };
        }
        // s → k (1 − z⁻¹)/(1 + z⁻¹), k chosen so 1 kHz maps exactly.
        let w = TAU * 1_000.0;
        let k = w / (w / (2.0 * rate)).tan();
        let (z, p) = (zero.unwrap_or(0.0), pole.unwrap_or(0.0));
        let (n0, n1) = (1.0 + k * z, 1.0 - k * z);
        let (d0, d1) = (1.0 + k * p, 1.0 - k * p);
        Self {
            b0: n0 / d0,
            b1: n1 / d0,
            a1: d1 / d0,
            state: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.state;
        self.state = self.b1 * x - self.a1 * y;
        y
    }
}

/// One channel of a curve, either way round.
#[derive(Clone, Debug)]
pub struct CurveFilter {
    sections: [FirstOrder; 2],
    /// Brings the response to unity at 1 kHz.
    gain: f64,
}

impl CurveFilter {
    /// The replay curve of `curve` at `rate`, or, with `cut`, the cutting curve: its exact inverse.
    pub fn new(curve: Curve, cut: bool, rate: f64) -> Self {
        let (t1, t2, t3) = curve.time_constants();
        let shelf = if t2.is_some() {
            Some(t1.unwrap_or(MISSING_SHELF_S))
        } else {
            t1
        };
        // Replay: the turnover's zero over the shelf's pole, then the ceiling's zero over the
        // treble's pole. Cut swaps every zero and pole.
        let ceiling = t3.map(|_| CEILING_S);
        let (a, b) = if cut {
            (
                FirstOrder::new(shelf, t2, rate),
                FirstOrder::new(t3, ceiling, rate),
            )
        } else {
            (
                FirstOrder::new(t2, shelf, rate),
                FirstOrder::new(ceiling, t3, rate),
            )
        };
        let replay_at_1k = {
            let w = TAU * 1_000.0;
            let mag = |t: Option<f64>| t.map_or(1.0, |t| (1.0 + (w * t).powi(2)).sqrt());
            mag(t2) * mag(ceiling) / (mag(shelf) * mag(t3))
        };
        Self {
            sections: [a, b],
            gain: if cut {
                replay_at_1k
            } else {
                1.0 / replay_at_1k
            },
        }
    }

    pub fn reset(&mut self) {
        self.sections.iter_mut().for_each(|s| s.state = 0.0);
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.sections[0].process(x * self.gain);
        self.sections[1].process(y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn measured_db(filter: &mut CurveFilter, hz: f64) -> f64 {
        let n = 96_000;
        let (mut input, mut output) = (0.0, 0.0);
        for i in 0..n {
            let x = (TAU * hz * i as f64 / RATE).sin();
            let y = filter.process(x);
            if i > n / 2 {
                input += x * x;
                output += y * y;
            }
        }
        10.0 * (output / input).log10()
    }

    #[test]
    fn the_standard_curve_has_its_familiar_shape() {
        // About +19.3 dB at 20 Hz... here the shelf holds it near +19 dB; −13.7 dB at 10 kHz;
        // 0 dB at 1 kHz.
        let db = |hz| 20.0 * Curve::Standard.replay_gain(hz).log10();
        assert!(db(1_000.0).abs() < 1e-9);
        assert!((db(10_000.0) + 13.7).abs() < 0.2, "{}", db(10_000.0));
        assert!((db(50.0) - 16.9).abs() < 0.3, "{}", db(50.0));
    }

    #[test]
    fn the_digital_curves_follow_the_analogue_ones_below_the_top_octave() {
        for curve in Curve::ALL {
            let mut replay = CurveFilter::new(curve, false, RATE);
            // The top octave is squeezed by the bilinear transform: allowed for at 10 kHz.
            for (hz, within) in [
                (50.0, 0.5),
                (200.0, 0.5),
                (1_000.0, 0.5),
                (4_000.0, 0.6),
                (10_000.0, 2.0),
            ] {
                let want = 20.0 * curve.replay_gain(hz).log10();
                let got = measured_db(&mut replay, hz);
                assert!(
                    (got - want).abs() < within,
                    "{curve:?} at {hz} Hz: {got:.2} against {want:.2}"
                );
            }
        }
    }

    #[test]
    fn cutting_then_replaying_the_same_curve_is_flat() {
        for curve in Curve::ALL {
            let mut cut = CurveFilter::new(curve, true, RATE);
            let mut replay = CurveFilter::new(curve, false, RATE);
            let mut worst = 0.0f64;
            for i in 0..48_000 {
                let x = (i as f64 * 0.731).sin() * 0.5 + (i as f64 * 0.0123).sin() * 0.3;
                let y = replay.process(cut.process(x));
                if i > 24_000 {
                    worst = worst.max((y - x).abs());
                }
            }
            assert!(worst < 1e-6, "{curve:?}: off by {worst}");
        }
    }

    #[test]
    fn a_uk_78_through_the_standard_curve_loses_its_treble() {
        // The period's commonest error: about −13.5 dB at 10 kHz.
        let mut cut = CurveFilter::new(Curve::Uk78, true, RATE);
        let mut replay = CurveFilter::new(Curve::Standard, false, RATE);
        let n = 96_000;
        let (mut input, mut output) = (0.0, 0.0);
        for i in 0..n {
            let x = (TAU * 10_000.0 * i as f64 / RATE).sin();
            let y = replay.process(cut.process(x));
            if i > n / 2 {
                input += x * x;
                output += y * y;
            }
        }
        let db = 10.0 * (output / input).log10();
        assert!((db + 13.5).abs() < 1.5, "{db:.1} dB");
    }
}
