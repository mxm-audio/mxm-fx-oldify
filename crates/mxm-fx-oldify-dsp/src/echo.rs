//! Groove echo: a faint copy of the music one turn of the disc late.
//!
//! **A groove is cut beside the one cut a turn before, and the soft lacquer between them gives**:
//! "a groove on a disc is modulated by its neighboring groove", so faint programme comes "one
//! revolution before or after intended", most noticeable in the silences before and after a
//! passage, and "usually low in level compared to program material since it is a byproduct of
//! heavy modulation of the neighboring groove" (AV Artifact Atlas, *Cross-talk between spiral
//! grooves*; `ops: notes/oldify/research_notes`, the record half). A turn is 1.8 s on an LP,
//! 1.33 s on a single, 0.77 s on a 78. Its level is estimated at −60 to −35 dB under the music; past
//! that, to [`MAX_ECHO_DB`], it is for effect.
//!
//! **Only the echo after**: the one before needs a turn of look-ahead, 1.8 s of latency on an LP.
//!
//! **The bass and the lower middle**: the land between two grooves passes a long wavelength's
//! push better than a short one's, so the echo is the cut music through a low-pass at
//! [`ECHO_TOP_HZ`] at the outer groove, lower as the groove slows toward the middle, where the
//! same note is a shorter wavelength. Chosen.
//!
//! It is cut into the groove, so it goes through everything after the cut — the groove's wear,
//! the read's wobbles, the pickup, the replay curve — as the music does.

use crate::biquad::{Biquad, Kind, Section};
use crate::disc::Speed;

/// The loudest groove echo, dB under the music: about 23 dB past the loudest a lacquer gives, for
/// effect (the owner, 2026-10-11: "our plugin should scale into the 'non physical' for creative
/// use"). Chosen.
pub const MAX_ECHO_DB: f32 = -12.0;
/// The echo's low-pass corner at the outer groove, hertz, two coincident poles. Chosen.
pub const ECHO_TOP_HZ: f64 = 1_000.0;

/// One turn's line, written with the cut music and read a turn behind.
#[derive(Clone, Debug)]
pub struct GrooveEcho {
    rate: f64,
    line: [Vec<f32>; 2],
    head: usize,
    /// Samples written since the last clear, up to the line's length: anything older reads as
    /// silence, so a clear costs nothing however long the line.
    filled: usize,
    low: [Biquad; 2],
    top_hz: f64,
}

impl GrooveEcho {
    /// Allocates the slowest disc's turn: call outside the audio thread.
    pub fn new(sample_rate: f64) -> Self {
        let length = turn_samples(Speed::Lp33, sample_rate);
        Self {
            rate: sample_rate,
            line: [vec![0.0; length], vec![0.0; length]],
            head: 0,
            filled: 0,
            low: Default::default(),
            top_hz: 0.0,
        }
    }

    /// Empties the line and the low-pass.
    pub fn clear(&mut self) {
        self.filled = 0;
        self.low.iter_mut().for_each(Biquad::reset);
    }

    /// Sets the low-pass for the groove speed here, as a share of the outer groove's.
    pub fn set_groove_speed(&mut self, share: f64) {
        let top_hz = ECHO_TOP_HZ * share;
        if top_hz != self.top_hz {
            self.top_hz = top_hz;
            let c = Section::new(Kind::LowPass, top_hz, 0.0, 0.5).coefficients(self.rate);
            self.low.iter_mut().for_each(|low| low.set(c));
        }
    }

    /// The cut music `x` with the music a turn of `speed` ago under it at `gain`; writes `x` into
    /// the line whatever the gain, so the echo is there the moment it is turned up.
    #[inline]
    pub fn process(&mut self, x: [f64; 2], gain: f64, speed: Speed) -> [f64; 2] {
        let length = self.line[0].len();
        let turn = turn_samples(speed, self.rate).min(length);
        let mut out = x;
        if gain > 0.0 {
            // Read before writing, so a turn of the whole line reads the slot about to be written.
            let at = (self.head + length - turn) % length;
            for ((out, line), low) in out.iter_mut().zip(&self.line).zip(&mut self.low) {
                let then = if self.filled >= turn {
                    f64::from(line[at])
                } else {
                    0.0
                };
                *out += gain * low.process(then);
            }
        }
        for (line, x) in self.line.iter_mut().zip(x) {
            line[self.head] = x as f32;
        }
        self.head = (self.head + 1) % length;
        self.filled = (self.filled + 1).min(length);
        out
    }
}

/// A turn of `speed` at `sample_rate`, in samples.
pub fn turn_samples(speed: Speed, sample_rate: f64) -> usize {
    ((sample_rate / speed.hz()).round() as usize).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    const RATE: f64 = 48_000.0;

    /// A tone burst of `hz` through the echo, then silence: the output.
    fn burst(speed: Speed, hz: f64, gain: f64) -> Vec<f64> {
        let mut echo = GrooveEcho::new(RATE);
        echo.set_groove_speed(1.0);
        let length = turn_samples(speed, RATE) + 9_600;
        (0..length)
            .map(|i| {
                let x = if i < 9_600 {
                    (TAU * hz * i as f64 / RATE).sin()
                } else {
                    0.0
                };
                echo.process([x, x], gain, speed)[0]
            })
            .collect()
    }

    fn rms(x: &[f64]) -> f64 {
        (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt()
    }

    /// The echo comes exactly a turn late on every disc, at its level in the bass.
    #[test]
    fn the_echo_comes_a_turn_late_at_its_level() {
        for speed in [Speed::Lp33, Speed::Single45, Speed::Shellac78] {
            let gain = f64::from(crate::db_to_gain(-30.0));
            let out = burst(speed, 60.0, gain);
            let turn = turn_samples(speed, RATE);
            assert!(
                out[9_600..turn].iter().all(|v| *v == 0.0),
                "{speed:?}: heard before its turn"
            );
            assert!(out[turn] == 0.0 && out[turn + 1] != 0.0, "{speed:?}");
            // The burst's middle, past the low-pass's start.
            let db = 20.0 * (rms(&out[turn + 2_400..turn + 7_200]) / rms_of_sine()).log10();
            assert!((db + 30.0).abs() < 0.5, "{speed:?}: {db:.2} dB");
        }
    }

    fn rms_of_sine() -> f64 {
        std::f64::consts::FRAC_1_SQRT_2
    }

    /// The treble echoes far less than the bass, and less still toward the middle of the side.
    #[test]
    fn the_echo_is_the_bass() {
        let echoed = |hz: f64, share: f64| {
            let mut echo = GrooveEcho::new(RATE);
            echo.set_groove_speed(share);
            let turn = turn_samples(Speed::Shellac78, RATE);
            let out: Vec<f64> = (0..turn + 9_600)
                .map(|i| {
                    let x = if i < 9_600 {
                        (TAU * hz * i as f64 / RATE).sin()
                    } else {
                        0.0
                    };
                    echo.process([x, x], 1.0, Speed::Shellac78)[0]
                })
                .collect();
            20.0 * (rms(&out[turn + 2_400..turn + 7_200]) / rms_of_sine()).log10()
        };
        assert!(echoed(100.0, 1.0) > -0.5);
        assert!(echoed(5_000.0, 1.0) < -25.0);
        assert!(echoed(1_000.0, crate::groove::INNER_SPEED) < echoed(1_000.0, 1.0) - 6.0);
    }

    /// Off leaves the music as it is, to the bit, and a clear forgets what was written.
    #[test]
    fn off_is_exact_and_a_clear_forgets() {
        let mut echo = GrooveEcho::new(RATE);
        echo.set_groove_speed(1.0);
        let turn = turn_samples(Speed::Shellac78, RATE);
        for i in 0..turn {
            let x = (i as f64 * 0.37).sin();
            assert_eq!(echo.process([x, -x], 0.0, Speed::Shellac78), [x, -x]);
        }
        echo.clear();
        for _ in 0..2 * turn {
            assert_eq!(echo.process([0.0, 0.0], 1.0, Speed::Shellac78), [0.0, 0.0]);
        }
    }
}
