//! Second-order filter sections from Robert Bristow-Johnson's "Cookbook formulae for audio EQ biquad
//! filter coefficients" (the bilinear transform of the analogue prototypes, prewarped at the
//! section's own frequency), run in transposed direct form II with `f64` state.
//!
//! A section is stored as what it means (kind, frequency, gain, Q), not as coefficients, so the
//! same table builds the same response at any sample rate; a frequency is held below
//! [`NYQUIST_FRACTION`] of the rate, where the cookbook's prewarp stays well behaved.

use std::f64::consts::PI;

/// The highest share of the sample rate a section's frequency may sit at.
pub const NYQUIST_FRACTION: f64 = 0.45;

/// The cookbook's section shapes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    LowPass,
    HighPass,
    Peak,
    LowShelf,
    HighShelf,
}

/// One section, as what it means.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Section {
    pub kind: Kind,
    pub hz: f64,
    pub gain_db: f64,
    pub q: f64,
}

impl Section {
    pub const fn new(kind: Kind, hz: f64, gain_db: f64, q: f64) -> Self {
        Self {
            kind,
            hz,
            gain_db,
            q,
        }
    }

    /// The section's coefficients at `sample_rate`, normalised so `a0` is one.
    pub fn coefficients(&self, sample_rate: f64) -> Coefficients {
        let hz = self.hz.clamp(1.0, NYQUIST_FRACTION * sample_rate);
        let a = 10f64.powf(self.gain_db / 40.0);
        let w = 2.0 * PI * hz / sample_rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * self.q.max(0.05));
        let (b, den) = match self.kind {
            Kind::LowPass => (
                [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0],
                [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
            ),
            Kind::HighPass => (
                [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
                [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
            ),
            Kind::Peak => (
                [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a],
                [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a],
            ),
            Kind::LowShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    [
                        a * ((a + 1.0) - (a - 1.0) * cos + s),
                        2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                        a * ((a + 1.0) - (a - 1.0) * cos - s),
                    ],
                    [
                        (a + 1.0) + (a - 1.0) * cos + s,
                        -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                        (a + 1.0) + (a - 1.0) * cos - s,
                    ],
                )
            }
            Kind::HighShelf => {
                let s = 2.0 * a.sqrt() * alpha;
                (
                    [
                        a * ((a + 1.0) + (a - 1.0) * cos + s),
                        -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                        a * ((a + 1.0) + (a - 1.0) * cos - s),
                    ],
                    [
                        (a + 1.0) - (a - 1.0) * cos + s,
                        2.0 * ((a - 1.0) - (a + 1.0) * cos),
                        (a + 1.0) - (a - 1.0) * cos - s,
                    ],
                )
            }
        };
        let a0 = den[0];
        Coefficients {
            b0: b[0] / a0,
            b1: b[1] / a0,
            b2: b[2] / a0,
            a1: den[1] / a0,
            a2: den[2] / a0,
        }
    }
}

/// A section's normalised coefficients.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coefficients {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl Coefficients {
    /// The pass-through section.
    pub const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// The magnitude at `hz`, for tests and plots.
    pub fn magnitude(&self, hz: f64, sample_rate: f64) -> f64 {
        // H(z) at z = e^{jw}: numerator and denominator in z^-1 and z^-2.
        let w = 2.0 * PI * hz / sample_rate;
        let (s1, c1) = (-w).sin_cos();
        let (s2, c2) = (-2.0 * w).sin_cos();
        let num = (
            self.b0 + self.b1 * c1 + self.b2 * c2,
            self.b1 * s1 + self.b2 * s2,
        );
        let den = (
            1.0 + self.a1 * c1 + self.a2 * c2,
            self.a1 * s1 + self.a2 * s2,
        );
        num.0.hypot(num.1) / den.0.hypot(den.1)
    }
}

/// A running section: coefficients and their transposed-direct-form-II state.
#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    c: Coefficients,
    s1: f64,
    s2: f64,
}

impl Default for Biquad {
    fn default() -> Self {
        Self::new(Coefficients::IDENTITY)
    }
}

impl Biquad {
    pub const fn new(c: Coefficients) -> Self {
        Self {
            c,
            s1: 0.0,
            s2: 0.0,
        }
    }

    /// New coefficients, keeping the state, so a slow change does not click.
    pub fn set(&mut self, c: Coefficients) {
        self.c = c;
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    /// The coefficients in use.
    pub fn coefficients(&self) -> Coefficients {
        self.c
    }

    /// What the next output holds before the next input's own term: the next output is
    /// `b0 · x + pending()`. A decoder that must solve for `x` reads it.
    pub fn pending(&self) -> f64 {
        self.s1
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.c.b0 * x + self.s1;
        self.s1 = self.c.b1 * x - self.c.a1 * y + self.s2;
        self.s2 = self.c.b2 * x - self.c.a2 * y;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(x: f64) -> f64 {
        20.0 * x.log10()
    }

    #[test]
    fn a_peak_has_its_gain_at_its_frequency_and_none_far_away() {
        let c = Section::new(Kind::Peak, 1000.0, 6.0, 1.0).coefficients(48_000.0);
        assert!((db(c.magnitude(1000.0, 48_000.0)) - 6.0).abs() < 0.01);
        assert!(db(c.magnitude(20.0, 48_000.0)).abs() < 0.05);
    }

    #[test]
    fn shelves_reach_their_gain_on_their_own_side() {
        let low = Section::new(Kind::LowShelf, 300.0, -4.0, 0.7).coefficients(48_000.0);
        let high = Section::new(Kind::HighShelf, 3000.0, -12.0, 0.7).coefficients(48_000.0);
        assert!((db(low.magnitude(20.0, 48_000.0)) + 4.0).abs() < 0.1);
        assert!(db(low.magnitude(15_000.0, 48_000.0)).abs() < 0.1);
        assert!((db(high.magnitude(20_000.0, 48_000.0)) + 12.0).abs() < 0.3);
        assert!(db(high.magnitude(50.0, 48_000.0)).abs() < 0.1);
    }

    #[test]
    fn a_running_section_matches_its_magnitude() {
        let c = Section::new(Kind::LowPass, 2000.0, 0.0, 0.707).coefficients(48_000.0);
        let mut f = Biquad::new(c);
        let hz = 7919.0;
        let settled: Vec<f64> = (0..48_000)
            .map(|n| f.process((2.0 * PI * hz * n as f64 / 48_000.0).sin()))
            .skip(24_000)
            .collect();
        // A sine's amplitude from its RMS: sampled peaks miss a tone with few samples a cycle.
        let amplitude =
            (2.0 * settled.iter().map(|y| y * y).sum::<f64>() / settled.len() as f64).sqrt();
        assert!((db(amplitude) - db(c.magnitude(hz, 48_000.0))).abs() < 0.05);
    }

    #[test]
    fn a_high_pass_passes_the_treble_and_stops_the_bass() {
        let c = Section::new(Kind::HighPass, 2000.0, 0.0, 0.707).coefficients(48_000.0);
        assert!(db(c.magnitude(16_000.0, 48_000.0)).abs() < 0.1);
        assert!(db(c.magnitude(100.0, 48_000.0)) < -40.0);
    }

    #[test]
    fn a_frequency_above_the_limit_is_held_below_nyquist() {
        let c = Section::new(Kind::LowPass, 30_000.0, 0.0, 0.707).coefficients(44_100.0);
        assert!(c.magnitude(1000.0, 44_100.0).is_finite());
        assert!(db(c.magnitude(1000.0, 44_100.0)).abs() < 0.1);
    }
}
