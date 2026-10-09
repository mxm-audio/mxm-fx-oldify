//! The groove: what the stylus does to the cut signal, before the pickup.
//!
//! **Inner grooves get worse by themselves.** An LP's groove passes the stylus at about 0.51 m/s
//! on the outside and 0.21 m/s on the inside (`ops: notes/oldify/reports`, the record half), so
//! one Side position moves three things together:
//!
//! - **Tracing distortion** grows as the groove speed's inverse square: the tracing index
//!   r·ω·v/V² puts it near 0.8 % outside and 4.5 % inside for an 18 µm tip at 1 kHz at full
//!   level, and with the tip's radius. Here the lateral (mid) motion is bent by a bounded, rising
//!   odd curve, `x / √(1 + c·x²)`, whose third harmonic is `c·A²/8` of a tone of amplitude `A`,
//!   with `c` set to give those figures; on lateral cuts the even harmonics cancel (Pierce and Hunt,
//!   1938), and the pinch effect puts the second harmonic into the vertical (side) channel.
//! - **Treble loss**: the stylus can trace a wavelength no shorter than it allows, so the top
//!   falls as the groove slows. **Wear** pulls it lower still: one hard anchor is 50 plays at 85 g
//!   wiping out what lay above 4 kHz. Here a low-pass whose corner is [`FRESH_TOP_HZ`] times the
//!   groove speed's ratio, lowered toward [`WORN_TOP_HZ`] by Wear.
//!
//! Distortion's strength is chosen to meet the cited figures; the corners are chosen around the
//! wear anchor.

use crate::biquad::{Biquad, Kind, Section};

/// The groove speed at the inner groove, as a share of the outer's (0.21 / 0.51 m/s on an LP).
pub const INNER_SPEED: f64 = 0.41;
/// The tracing curve's strength at the outer groove for an 18 µm tip and Groove distortion 1:
/// a full-level tone's third harmonic at 0.8 %.
const OUTER_STRENGTH: f64 = 0.064;
/// The tip the figures are for, micrometres.
pub const REFERENCE_TIP_UM: f64 = 18.0;
/// The top a fresh groove keeps at the outer groove, and a worn one, in hertz. Chosen around
/// the cited anchor.
pub const FRESH_TOP_HZ: f64 = 24_000.0;
pub const WORN_TOP_HZ: f64 = 4_000.0;
/// The pinch effect's second harmonic, as a share of the tracing curve's strength. Chosen.
const PINCH: f64 = 0.5;

/// The groove's controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrooveControls {
    /// 0 is the outer groove, 1 the inner.
    pub side: f32,
    /// 0–1.
    pub wear: f32,
    /// 0–2 of the physical amount; 1 is the physics.
    pub distortion: f32,
    /// The stylus tip's radius, micrometres: 4 (line contact) to 100 (a 78's steel needle).
    pub tip_um: f32,
}

impl GrooveControls {
    pub const CLEAN: Self = Self {
        side: 0.0,
        wear: 0.0,
        distortion: 0.0,
        tip_um: 18.0,
    };

    pub fn bounded(self) -> Self {
        let within = |v: f32, lo: f32, hi: f32, otherwise: f32| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                otherwise
            }
        };
        Self {
            side: within(self.side, 0.0, 1.0, 0.0),
            wear: within(self.wear, 0.0, 1.0, 0.0),
            distortion: within(self.distortion, 0.0, 2.0, 0.0),
            tip_um: within(self.tip_um, 4.0, 100.0, 18.0),
        }
    }

    /// The groove speed here, as a share of the outer groove's.
    pub fn speed(&self) -> f64 {
        1.0 - (1.0 - INNER_SPEED) * f64::from(self.side)
    }

    /// The tracing curve's strength `c`.
    pub fn strength(&self) -> f64 {
        OUTER_STRENGTH * f64::from(self.distortion) * f64::from(self.tip_um)
            / REFERENCE_TIP_UM
            / self.speed().powi(2)
    }

    /// The low-pass's corner, in hertz.
    pub fn top_hz(&self) -> f64 {
        FRESH_TOP_HZ * self.speed() * (WORN_TOP_HZ / FRESH_TOP_HZ).powf(f64::from(self.wear))
    }
}

/// The groove, both channels.
#[derive(Clone, Debug)]
pub struct Groove {
    rate: f64,
    tops: [Biquad; 2],
    top_hz: Option<f64>,
    /// The pinch effect's second harmonic, with its slow average taken out.
    pinch_mean: f64,
}

impl Groove {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            rate: sample_rate,
            tops: [Biquad::default(), Biquad::default()],
            top_hz: None,
            pinch_mean: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.tops.iter_mut().for_each(Biquad::reset);
        self.pinch_mean = 0.0;
    }

    /// One frame through the groove.
    #[inline]
    pub fn process(&mut self, frame: [f32; 2], c: &GrooveControls) -> [f32; 2] {
        let mut mid = 0.5 * (f64::from(frame[0]) + f64::from(frame[1]));
        let mut side = 0.5 * (f64::from(frame[0]) - f64::from(frame[1]));
        let strength = c.strength();
        if strength > 0.0 {
            let square = mid * mid;
            mid /= (1.0 + strength * square).sqrt();
            // The second harmonic of the lateral motion, in the vertical, without its DC.
            self.pinch_mean += 0.001 * (square - self.pinch_mean);
            side += PINCH * strength * 0.25 * (square - self.pinch_mean);
        }
        let mut out = [(mid + side) as f32, (mid - side) as f32];

        let top = c.top_hz();
        if top < 0.4 * self.rate {
            if self.top_hz != Some(top) {
                self.top_hz = Some(top);
                let section =
                    Section::new(Kind::LowPass, top, 0.0, std::f64::consts::FRAC_1_SQRT_2);
                let coefficients = section.coefficients(self.rate);
                self.tops.iter_mut().for_each(|b| b.set(coefficients));
            }
            for (y, top) in out.iter_mut().zip(&mut self.tops) {
                *y = top.process(f64::from(*y)) as f32;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    const RATE: f64 = 48_000.0;

    /// The third harmonic of a full-level 1 kHz tone, as a share of the fundamental.
    fn third_harmonic(c: &GrooveControls) -> f64 {
        let mut groove = Groove::new(RATE);
        let n = 48_000;
        let y: Vec<f64> = (0..n)
            .map(|i| {
                let x = (TAU * 1_000.0 * i as f64 / RATE).sin() as f32;
                f64::from(groove.process([x, x], c)[0])
            })
            .collect();
        let bin = |hz: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, v) in y[n / 2..].iter().enumerate() {
                let p = TAU * hz * i as f64 / RATE;
                re += v * p.cos();
                im += v * p.sin();
            }
            (re * re + im * im).sqrt()
        };
        bin(3_000.0) / bin(1_000.0)
    }

    #[test]
    fn tracing_distortion_meets_the_cited_figures_outside_and_in() {
        let outer = GrooveControls {
            distortion: 1.0,
            ..GrooveControls::CLEAN
        };
        let inner = GrooveControls { side: 1.0, ..outer };
        let (a, b) = (third_harmonic(&outer), third_harmonic(&inner));
        assert!((a - 0.008).abs() < 0.002, "outer {:.2} %", 100.0 * a);
        assert!((b - 0.045).abs() < 0.012, "inner {:.2} %", 100.0 * b);
    }

    #[test]
    fn a_clean_outer_groove_passes_the_signal_as_it_came() {
        let mut groove = Groove::new(RATE);
        for i in 0..1000 {
            let x = [
                ((i as f64) * 0.3).sin() as f32,
                ((i as f64) * 0.7).cos() as f32,
            ];
            let y = groove.process(x, &GrooveControls::CLEAN);
            assert!((y[0] - x[0]).abs() < 1e-6 && (y[1] - x[1]).abs() < 1e-6);
        }
    }

    #[test]
    fn wear_and_the_inner_groove_take_the_treble() {
        let fresh = GrooveControls::CLEAN;
        let worn = GrooveControls { wear: 1.0, ..fresh };
        let inner = GrooveControls { side: 1.0, ..fresh };
        assert!(fresh.top_hz() > 20_000.0);
        assert!((worn.top_hz() - WORN_TOP_HZ).abs() < 1.0);
        assert!(inner.top_hz() < 0.5 * fresh.top_hz());
    }

    #[test]
    fn the_curve_stays_bounded_and_finite_at_the_extremes() {
        let c = GrooveControls {
            side: 1.0,
            wear: 1.0,
            distortion: 2.0,
            tip_um: 100.0,
        };
        let mut groove = Groove::new(RATE);
        for i in 0..10_000 {
            let x = if i % 50 < 25 { 4.0 } else { -4.0 };
            let y = groove.process([x, -x], &c);
            assert!(y.iter().all(|v| v.is_finite() && v.abs() < 10.0));
        }
    }
}
