//! The tape and its grade: twelve playback responses, measured.
//!
//! Each is the measured reference emulator's corresponding profile (2026-10-09, relative to its
//! tape bypass, a -30 dBFS sweep at 48 kHz) and fitted with five cookbook sections: a low shelf,
//! two peaks, a high shelf and a low-pass. The fit is within 0.6 dB from 30 Hz to 18 kHz for ten of
//! them and within 4.6 dB for the two economy ferric grades, where they are already 20 to 37 dB
//! down. Our own measurement of what the software does, which the collection's research boundary
//! lets cross; the notes are private (`ops: notes/oldify/measurements/`).

use crate::biquad::{Biquad, Kind, Section};

/// The tape formulation: what the deck's EQ switch is set for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tape {
    /// Ferric (IEC Type I, 120 µs).
    I,
    /// Chrome or chrome-equivalent (Type II, 70 µs).
    II,
    /// Metal (Type IV, 70 µs).
    IV,
}

/// How good the tape and the deck are, cheapest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grade {
    Economy,
    Standard,
    High,
    Reference,
}

const SECTIONS: usize = 5;

/// The fitted sections, by tape and grade.
const PROFILES: [[[Section; SECTIONS]; 4]; 3] = [
    // Type I
    [
        // I Economy (measured as Cheap): worst 4.08 dB, RMS 0.67 dB
        [
            Section::new(Kind::LowShelf, 1497.1, 0.09, 3.000),
            Section::new(Kind::Peak, 3239.9, -5.79, 0.592),
            Section::new(Kind::Peak, 9213.5, -11.94, 4.684),
            Section::new(Kind::HighShelf, 6451.7, -29.85, 0.452),
            Section::new(Kind::LowPass, 18375.8, 0.00, 1.109),
        ],
        // I Standard (measured as Value): worst 4.58 dB, RMS 0.59 dB
        [
            Section::new(Kind::LowShelf, 809.4, -0.10, 1.515),
            Section::new(Kind::Peak, 1586.9, -4.83, 0.868),
            Section::new(Kind::Peak, 9881.0, -4.90, 3.568),
            Section::new(Kind::HighShelf, 12849.0, -40.00, 0.428),
            Section::new(Kind::LowPass, 15063.2, 0.00, 3.460),
        ],
        // I High (measured as Standard): worst 0.24 dB, RMS 0.05 dB
        [
            Section::new(Kind::LowShelf, 1500.0, 0.01, 3.000),
            Section::new(Kind::Peak, 4631.7, 4.31, 0.831),
            Section::new(Kind::Peak, 8392.5, -0.59, 3.247),
            Section::new(Kind::HighShelf, 5632.6, -19.42, 0.432),
            Section::new(Kind::LowPass, 14816.3, 0.00, 2.369),
        ],
        // I Reference (measured as Master): worst 0.58 dB, RMS 0.09 dB
        [
            Section::new(Kind::LowShelf, 1500.0, -0.00, 3.000),
            Section::new(Kind::Peak, 5647.4, 5.24, 0.914),
            Section::new(Kind::Peak, 9782.8, 1.84, 1.919),
            Section::new(Kind::HighShelf, 3959.7, -12.97, 0.608),
            Section::new(Kind::LowPass, 15173.1, 0.00, 1.804),
        ],
    ],
    // Type II
    [
        // II Economy (measured as Cheap): worst 0.28 dB, RMS 0.06 dB
        [
            Section::new(Kind::LowShelf, 1322.2, 0.00, 3.000),
            Section::new(Kind::Peak, 5082.3, 3.59, 0.946),
            Section::new(Kind::Peak, 9438.1, 8.20, 0.453),
            Section::new(Kind::HighShelf, 3717.4, -13.54, 0.630),
            Section::new(Kind::LowPass, 15789.4, 0.00, 1.878),
        ],
        // II Standard (measured as Value): worst 0.2 dB, RMS 0.04 dB
        [
            Section::new(Kind::LowShelf, 922.2, 0.00, 3.000),
            Section::new(Kind::Peak, 4907.0, 7.42, 0.505),
            Section::new(Kind::Peak, 10460.7, 3.79, 0.661),
            Section::new(Kind::HighShelf, 2803.7, -9.84, 0.611),
            Section::new(Kind::LowPass, 15499.1, 0.00, 1.979),
        ],
        // II High (measured as Standard): worst 0.11 dB, RMS 0.02 dB
        [
            Section::new(Kind::LowShelf, 1500.0, 0.00, 3.000),
            Section::new(Kind::Peak, 8000.0, 8.25, 0.329),
            Section::new(Kind::Peak, 11963.8, -0.96, 2.418),
            Section::new(Kind::HighShelf, 4063.1, -8.51, 0.561),
            Section::new(Kind::LowPass, 15314.4, 0.00, 1.441),
        ],
        // II Reference (measured as Master): worst 0.23 dB, RMS 0.04 dB
        [
            Section::new(Kind::LowShelf, 1150.3, 0.01, 3.000),
            Section::new(Kind::Peak, 4838.7, 5.62, 0.553),
            Section::new(Kind::Peak, 10521.9, 2.74, 1.029),
            Section::new(Kind::HighShelf, 2424.2, -4.27, 0.761),
            Section::new(Kind::LowPass, 15632.7, 0.00, 1.904),
        ],
    ],
    // Type IV
    [
        // IV Economy (measured as Cheap): worst 0.14 dB, RMS 0.03 dB
        [
            Section::new(Kind::LowShelf, 1191.2, -2.50, 0.508),
            Section::new(Kind::Peak, 5012.0, 5.46, 0.881),
            Section::new(Kind::Peak, 10272.2, 10.10, 0.464),
            Section::new(Kind::HighShelf, 3313.9, -11.77, 0.671),
            Section::new(Kind::LowPass, 15638.2, 0.00, 1.745),
        ],
        // IV Standard (measured as Value): worst 0.13 dB, RMS 0.02 dB
        [
            Section::new(Kind::LowShelf, 1124.7, 0.00, 3.000),
            Section::new(Kind::Peak, 4744.0, 3.85, 0.719),
            Section::new(Kind::Peak, 9806.6, 2.15, 1.192),
            Section::new(Kind::HighShelf, 2612.8, -4.43, 0.694),
            Section::new(Kind::LowPass, 17568.2, 0.00, 0.931),
        ],
        // IV High (measured as Standard): worst 0.13 dB, RMS 0.02 dB
        [
            Section::new(Kind::LowShelf, 1141.7, 0.00, 3.000),
            Section::new(Kind::Peak, 2442.2, 1.59, 0.553),
            Section::new(Kind::Peak, 8525.7, 1.74, 1.126),
            Section::new(Kind::HighShelf, 6841.7, -0.72, 1.508),
            Section::new(Kind::LowPass, 17693.3, 0.00, 0.804),
        ],
        // IV Reference (measured as Master): worst 0.26 dB, RMS 0.04 dB
        [
            Section::new(Kind::LowShelf, 885.6, 0.02, 3.000),
            Section::new(Kind::Peak, 4907.0, 8.28, 0.374),
            Section::new(Kind::Peak, 10886.9, 2.02, 1.202),
            Section::new(Kind::HighShelf, 1985.5, -5.32, 0.761),
            Section::new(Kind::LowPass, 17358.2, 0.00, 1.252),
        ],
    ],
];

/// The sections of one tape and grade.
pub fn sections(tape: Tape, grade: Grade) -> &'static [Section; SECTIONS] {
    let t = match tape {
        Tape::I => 0,
        Tape::II => 1,
        Tape::IV => 2,
    };
    let g = match grade {
        Grade::Economy => 0,
        Grade::Standard => 1,
        Grade::High => 2,
        Grade::Reference => 3,
    };
    &PROFILES[t][g]
}

/// One channel's tape response.
#[derive(Clone, Debug, Default)]
pub struct TapeResponse {
    stages: [Biquad; SECTIONS],
}

impl TapeResponse {
    /// Builds `tape` and `grade` at `sample_rate`, keeping the filters' state.
    pub fn set(&mut self, tape: Tape, grade: Grade, sample_rate: f64) {
        for (stage, section) in self.stages.iter_mut().zip(sections(tape, grade)) {
            stage.set(section.coefficients(sample_rate));
        }
    }

    pub fn reset(&mut self) {
        self.stages.iter_mut().for_each(Biquad::reset);
    }

    #[inline]
    pub fn process(&mut self, x: f64) -> f64 {
        self.stages.iter_mut().fold(x, |y, stage| stage.process(y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response_db(tape: Tape, grade: Grade, hz: f64) -> f64 {
        sections(tape, grade)
            .iter()
            .map(|s| 20.0 * s.coefficients(48_000.0).magnitude(hz, 48_000.0).log10())
            .sum()
    }

    #[test]
    fn the_fits_hold_what_was_measured() {
        // Spot values from the measurement (dB re tape bypass): a check that the table is the fit.
        assert!((response_db(Tape::I, Grade::High, 4000.0) + 2.9).abs() < 0.5);
        assert!((response_db(Tape::II, Grade::High, 4000.0) - 2.2).abs() < 0.5);
        assert!((response_db(Tape::IV, Grade::Reference, 4000.0) - 3.2).abs() < 0.5);
        assert!((response_db(Tape::I, Grade::Economy, 2000.0) + 8.6).abs() < 1.0);
    }

    #[test]
    fn every_profile_is_flat_in_the_bass_and_falls_at_the_top() {
        for tape in [Tape::I, Tape::II, Tape::IV] {
            for grade in [
                Grade::Economy,
                Grade::Standard,
                Grade::High,
                Grade::Reference,
            ] {
                let low = response_db(tape, grade, 100.0);
                let top = response_db(tape, grade, 18_000.0);
                assert!(low.abs() < 3.0, "{tape:?} {grade:?} at 100 Hz: {low}");
                assert!(top < -2.5, "{tape:?} {grade:?} at 18 kHz: {top}");
            }
        }
    }

    #[test]
    fn it_builds_at_every_common_rate() {
        for rate in [44_100.0, 48_000.0, 88_200.0, 96_000.0, 192_000.0] {
            let mut tape = TapeResponse::default();
            tape.set(Tape::I, Grade::Economy, rate);
            let y: f64 = (0..1000)
                .map(|n| tape.process(if n == 0 { 1.0 } else { 0.0 }))
                .sum();
            assert!(y.is_finite(), "{rate}");
        }
    }
}
