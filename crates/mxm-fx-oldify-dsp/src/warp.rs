//! A warped record: once a turn the disc rises and falls under the stylus, and the speed and the
//! level swing with it.
//!
//! **The speed.** A warp lifts the arm, and the stylus, below the arm's pivot, swings along the
//! groove as it rises, so the read moves by the arm's geometry times the warp's height, over the
//! groove's speed; the speed error is the height's slope against the angle over the groove's
//! radius, whatever the disc's speed (`ops: notes/oldify/research_notes`, the vinyl mechanics,
//! after Rosenberg's US 4,182,517). A disc bent from its label, its height growing with the radius,
//! errs alike at every groove: ours does.
//!
//! **The shape and the size are the reference's**, measured at an LP's outer groove (`ops:
//! notes/oldify/measurements`, the modulations): the turn's fundamental with its second and third
//! harmonics ([`HARMONICS`]), the fundamental's peak [`FULL`] at its full travel. A Warp of 1 is
//! that, and the control goes on past it for effect.
//!
//! **The level dips once a turn** with it, deepest where the speed is highest, by [`DIP_DB`] at a
//! Warp of 1: the reference's, the same at every frequency we measured. No source gives its size.

use std::f64::consts::TAU;

/// The speed's harmonics of the turn, `sin(nθ + phase)`: amplitude against the fundamental's, and
/// phase, radians. The reference's, at its full travel.
pub const HARMONICS: [(f64, f64); 3] = [(1.0, 0.0), (0.514, 0.487), (0.133, -2.337)];
/// The fundamental's peak speed error at the reference's full warp: ±1.128 %.
pub const FULL: f64 = 0.011_28;
/// How far the level dips at a Warp of 1, decibels, from the speed's lowest to its highest.
pub const DIP_DB: f64 = 2.43;
/// The read's peak, `Σ (aₙ/n)·cos(nθ + φₙ)` at its largest: a sine of this depth reaches as far.
pub const READ_PEAK: f64 = 1.202_79;
/// The speed's lowest and highest, against the fundamental's peak.
pub const SPEED_LOW: f64 = -1.401_11;
pub const SPEED_HIGH: f64 = 1.236_55;

/// The read at `phase` (0..1 of a turn), ±1 at its furthest: a transport's [`Shape::Warp`]
/// (`crate::transport::Shape::Warp`), whose speed is [`speed`].
pub fn read(phase: f64) -> f64 {
    let theta = TAU * phase;
    let mut sum = 0.0;
    for (n, (amplitude, offset)) in HARMONICS.iter().enumerate() {
        let k = (n + 1) as f64;
        sum += amplitude / k * (k * theta + offset).cos();
    }
    sum / READ_PEAK
}

/// The speed error at `phase`, the fundamental's peak 1.
pub fn speed(phase: f64) -> f64 {
    let theta = TAU * phase;
    HARMONICS
        .iter()
        .enumerate()
        .map(|(n, (amplitude, offset))| {
            let k = (n + 1) as f64;
            amplitude * (k * theta + offset).sin()
        })
        .sum()
}

/// How much of the level's dip is taken at `phase`: 0 where the speed is lowest, 1 where it is
/// highest.
pub fn dip(phase: f64) -> f64 {
    ((speed(phase) - SPEED_LOW) / (SPEED_HIGH - SPEED_LOW)).clamp(0.0, 1.0)
}

/// The depth a transport's [`Shape::Warp`](crate::transport::Shape::Warp) wobble takes for a warp
/// of `amount` (1 the reference's): the sine's that reaches as far.
pub fn depth(amount: f64) -> f64 {
    amount * FULL * READ_PEAK
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn() -> impl Iterator<Item = f64> {
        (0..100_000).map(|i| f64::from(i) / 100_000.0)
    }

    #[test]
    fn the_shapes_constants_are_its_own() {
        let peak = turn()
            .map(|p| (read(p) * READ_PEAK).abs())
            .fold(0.0, f64::max);
        let low = turn().map(speed).fold(f64::MAX, f64::min);
        let high = turn().map(speed).fold(f64::MIN, f64::max);
        assert!((peak - READ_PEAK).abs() < 1e-5, "{peak}");
        assert!((low - SPEED_LOW).abs() < 1e-5, "{low}");
        assert!((high - SPEED_HIGH).abs() < 1e-5, "{high}");
    }

    /// The read's slope is the speed: `dD/dt = −δ`, so a sine of depth [`depth`] moving the read
    /// by [`read`] goes at [`FULL`] times [`speed`].
    #[test]
    fn the_reads_slope_is_the_speed() {
        let h = 1e-6;
        for p in turn().step_by(997) {
            let slope = -(read(p + h) - read(p - h)) / (2.0 * h) / TAU;
            assert!((slope * READ_PEAK - speed(p)).abs() < 1e-6, "{p}");
        }
    }

    /// The reference's full warp, peak to peak: 2.99 %.
    #[test]
    fn a_full_warp_swings_as_far_as_the_references() {
        let p2p = FULL * (SPEED_HIGH - SPEED_LOW) * 100.0;
        assert!((p2p - 2.99).abs() < 0.03, "{p2p} %");
    }
}
