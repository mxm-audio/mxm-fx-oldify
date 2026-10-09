//! Spacing loss: the treble a gap between the tape and the head costs, Wallace's
//! 54.6 · d / λ dB, with λ the recorded wavelength (speed / frequency) and d the spacing. At
//! cassette speed half a micrometre costs 5.7 dB at 10 kHz and 0.6 dB at 1 kHz. Being linear in
//! frequency in decibels, it is a tilt, not a resonance: one filter, driven by one spacing, models
//! Age (dirt, wear, a magnetised head) and will model a dropout's lift.
//!
//! **Two fitted tables, rows every [`STEP_UM`].** Each row is a high shelf and a low-pass
//! (`biquad`'s cookbook sections) fitted to the loss from 30 Hz to 18 kHz down to −45 dB, below
//! which a fit need only stay too (`ops: oldify/measure/fit_spacing.py`). `BASE` is fitted as
//! digital filters at 44.1 and 48 kHz together, within 2.9 dB; `HIGH` as analogue prototypes for
//! 88.2 kHz and up, within 2.3 dB. Rows need not move smoothly from one to the next, because a
//! spacing between two rows **crossfades the two rows' outputs**, never their coefficients: a slot
//! takes a new row only while its weight is zero, so a moving spacing never clicks. Row 0 is no
//! loss at all, the input itself.

use crate::biquad::{Biquad, Kind, Section};

/// The spacing between two rows, in micrometres.
pub const STEP_UM: f64 = 0.05;
/// Rows past row 0 in each table.
const ROWS: usize = 60;
/// The largest spacing modelled, in micrometres; more is held there.
pub const MAX_UM: f64 = STEP_UM * ROWS as f64;
/// From this rate up the analogue table is used.
const HIGH_FROM_HZ: f64 = 66_150.0;

/// High shelf (Hz, dB, Q), then low-pass (Hz, Q), by row.
type Row = [f64; 5];

const BASE: [Row; ROWS] = [
    [9521.0, -1.291, 0.4181, 19500.0, 0.8030],
    [8671.8, -2.175, 0.4102, 19500.0, 0.7713],
    [8360.1, -3.071, 0.4076, 19500.0, 0.7425],
    [8191.4, -3.964, 0.4063, 19500.0, 0.7150],
    [8082.9, -4.852, 0.4056, 19500.0, 0.6885],
    [8006.1, -5.736, 0.4051, 19500.0, 0.6630],
    [7949.1, -6.615, 0.4047, 19500.0, 0.6385],
    [7905.7, -7.490, 0.4043, 19500.0, 0.6149],
    [7872.9, -8.363, 0.4040, 19500.0, 0.5922],
    [7848.5, -9.233, 0.4036, 19500.0, 0.5704],
    [7744.3, -10.001, 0.4046, 19399.5, 0.5529],
    [7622.9, -10.715, 0.4057, 19288.4, 0.5351],
    [7479.0, -11.359, 0.4070, 19167.7, 0.5160],
    [7290.2, -11.887, 0.4088, 19025.9, 0.4948],
    [7007.2, -12.191, 0.4113, 18838.2, 0.4688],
    [6430.0, -11.837, 0.4161, 18507.1, 0.4279],
    [3950.7, -7.255, 0.4273, 17114.3, 0.3000],
    [3801.7, -7.253, 0.4235, 16521.7, 0.3000],
    [3668.9, -7.227, 0.4187, 15900.3, 0.3000],
    [3558.9, -7.192, 0.4127, 15263.6, 0.3000],
    [3475.5, -7.163, 0.4057, 14623.7, 0.3000],
    [3420.1, -7.147, 0.3978, 13989.4, 0.3000],
    [3392.6, -7.145, 0.3893, 13366.0, 0.3000],
    [3390.0, -7.153, 0.3802, 12752.6, 0.3000],
    [3407.0, -7.158, 0.3709, 12145.7, 0.3000],
    [3437.6, -7.145, 0.3615, 11539.7, 0.3000],
    [3473.4, -7.093, 0.3523, 10928.2, 0.3000],
    [3488.8, -6.927, 0.3427, 10275.8, 0.3000],
    [4326.8, -9.645, 0.3598, 11289.0, 0.3000],
    [4933.2, -12.116, 0.3685, 12126.9, 0.3000],
    [5406.4, -14.362, 0.3733, 12799.2, 0.3000],
    [5820.9, -16.555, 0.3763, 13403.8, 0.3000],
    [6206.7, -18.766, 0.3783, 13973.9, 0.3000],
    [6582.6, -21.035, 0.3795, 14525.4, 0.3000],
    [6963.1, -23.401, 0.3800, 15065.5, 0.3000],
    [7362.0, -25.901, 0.3798, 15598.9, 0.3000],
    [7788.0, -28.552, 0.3789, 16116.4, 0.3000],
    [8232.0, -31.294, 0.3772, 16585.2, 0.3000],
    [8643.8, -33.894, 0.3754, 16936.4, 0.3000],
    [8971.3, -36.113, 0.3740, 17126.0, 0.3000],
    [9214.6, -37.953, 0.3730, 17183.2, 0.3000],
    [9937.6, -40.474, 0.3574, 16305.6, 0.3221],
    [10097.3, -41.905, 0.3570, 16260.0, 0.3189],
    [10402.6, -43.947, 0.3556, 16166.0, 0.3212],
    [10745.0, -45.529, 0.3487, 14726.3, 0.3697],
    [3213.5, -7.424, 0.2220, 3581.4, 0.5314],
    [3300.0, -8.250, 0.2312, 3573.5, 0.5465],
    [3764.4, -9.974, 0.2332, 3617.3, 0.5753],
    [3916.3, -11.141, 0.2402, 3642.8, 0.5933],
    [3923.0, -11.946, 0.2483, 3647.9, 0.6054],
    [4278.2, -13.874, 0.2523, 3734.5, 0.6316],
    [4370.3, -15.010, 0.2581, 3766.6, 0.6464],
    [4362.1, -15.801, 0.2643, 3774.3, 0.6569],
    [4615.9, -17.640, 0.2687, 3867.1, 0.6774],
    [4692.2, -18.768, 0.2734, 3902.0, 0.6898],
    [4687.3, -19.554, 0.2782, 3909.9, 0.6987],
    [4783.8, -20.806, 0.2826, 3958.6, 0.7108],
    [4950.6, -22.353, 0.2861, 4028.7, 0.7239],
    [4959.3, -23.152, 0.2897, 4035.5, 0.7315],
    [5027.6, -24.246, 0.2931, 4068.1, 0.7403],
];

const HIGH: [Row; ROWS] = [
    [14538.9, -7.741, 0.5340, 19500.0, 1.4040],
    [9588.1, -4.119, 0.4948, 19500.0, 1.0190],
    [6720.6, -2.972, 0.4728, 19500.0, 0.8334],
    [5095.1, -2.534, 0.4566, 19500.0, 0.7162],
    [4052.0, -2.302, 0.4436, 19500.0, 0.6267],
    [3301.4, -2.130, 0.4325, 19500.0, 0.5525],
    [2715.0, -1.970, 0.4228, 19500.0, 0.4888],
    [2226.9, -1.804, 0.4146, 19500.0, 0.4333],
    [1789.4, -1.614, 0.4098, 19499.9, 0.3843],
    [1666.7, -1.671, 0.4098, 16758.8, 0.4015],
    [1609.8, -1.775, 0.4100, 14576.5, 0.4216],
    [5523.3, -2.891, 0.2000, 14025.3, 0.3996],
    [5372.4, -3.045, 0.2000, 12481.5, 0.4163],
    [5255.1, -3.208, 0.2000, 11287.6, 0.4303],
    [5171.4, -3.383, 0.2000, 10322.9, 0.4429],
    [5121.5, -3.578, 0.2000, 9521.6, 0.4549],
    [5106.1, -3.798, 0.2000, 8843.4, 0.4668],
    [4995.6, -4.078, 0.2064, 8306.2, 0.4787],
    [4911.4, -4.422, 0.2143, 7855.6, 0.4913],
    [4873.6, -4.844, 0.2229, 7476.2, 0.5051],
    [4882.6, -5.371, 0.2324, 7163.5, 0.5204],
    [4939.0, -6.033, 0.2428, 6915.9, 0.5378],
    [5042.5, -6.865, 0.2541, 6733.5, 0.5575],
    [5190.6, -7.891, 0.2659, 6615.3, 0.5798],
    [5376.6, -9.126, 0.2778, 6558.1, 0.6046],
    [5593.1, -10.563, 0.2893, 6556.1, 0.6315],
    [5834.9, -12.186, 0.3001, 6600.2, 0.6602],
    [6102.6, -13.981, 0.3099, 6682.5, 0.6900],
    [6402.3, -15.943, 0.3186, 6796.7, 0.7207],
    [6748.9, -18.087, 0.3260, 6938.0, 0.7516],
    [7177.7, -20.468, 0.3318, 7104.3, 0.7825],
    [7756.0, -23.219, 0.3354, 7300.5, 0.8126],
    [8706.2, -26.776, 0.3348, 7549.6, 0.8410],
    [19500.0, -44.435, 0.2720, 8391.4, 0.8535],
    [19500.0, -46.502, 0.2810, 8372.5, 0.8518],
    [19500.0, -48.513, 0.2897, 8344.9, 0.8510],
    [19500.0, -50.477, 0.2983, 8310.8, 0.8514],
    [19500.0, -52.401, 0.3067, 8271.5, 0.8532],
    [19500.0, -54.284, 0.3151, 8227.3, 0.8567],
    [19500.0, -56.124, 0.3233, 8177.7, 0.8619],
    [19500.0, -57.916, 0.3314, 8122.1, 0.8691],
    [19500.0, -59.656, 0.3392, 8060.0, 0.8781],
    [19113.4, -60.000, 0.3407, 7869.6, 0.8856],
    [18253.9, -60.000, 0.3475, 7772.2, 0.8959],
    [17985.3, -60.000, 0.3452, 7574.1, 0.8922],
    [17538.2, -60.000, 0.3462, 7418.6, 0.8939],
    [17160.8, -60.000, 0.3463, 7263.1, 0.8938],
    [16902.6, -60.000, 0.3445, 7092.2, 0.8911],
    [16380.2, -60.000, 0.3477, 6981.0, 0.8963],
    [16168.4, -60.000, 0.3456, 6820.3, 0.8927],
    [15927.4, -60.000, 0.3441, 6670.7, 0.8907],
    [15464.6, -60.000, 0.3471, 6573.1, 0.8953],
    [15269.2, -60.000, 0.3453, 6431.1, 0.8922],
    [15016.7, -60.000, 0.3446, 6304.9, 0.8915],
    [14629.2, -60.000, 0.3470, 6213.1, 0.8950],
    [14452.9, -60.000, 0.3452, 6086.2, 0.8922],
    [14238.1, -60.000, 0.3444, 5970.8, 0.8911],
    [13864.3, -60.000, 0.3472, 5893.2, 0.8953],
    [13708.5, -60.000, 0.3454, 5778.7, 0.8925],
    [13534.6, -60.000, 0.3442, 5671.0, 0.8908],
];

/// One row's two sections.
fn sections(row: &Row) -> [Section; 2] {
    [
        Section::new(Kind::HighShelf, row[0], row[1], row[2]),
        Section::new(Kind::LowPass, row[3], 0.0, row[4]),
    ]
}

/// A row's filter: none for row 0, else its two sections.
#[derive(Clone, Debug)]
struct Slot {
    row: usize,
    stages: [Biquad; 2],
}

impl Slot {
    fn none() -> Self {
        Self {
            row: 0,
            stages: [Biquad::default(), Biquad::default()],
        }
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        if self.row == 0 {
            x
        } else {
            let y = self.stages[0].process(x);
            self.stages[1].process(y)
        }
    }
}

/// One channel's spacing loss, for a spacing that may move every sample.
#[derive(Clone, Debug)]
pub struct Spacing {
    rate: f64,
    table: &'static [Row; ROWS],
    slots: [Slot; 2],
}

impl Spacing {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            rate: sample_rate,
            table: if sample_rate >= HIGH_FROM_HZ {
                &HIGH
            } else {
                &BASE
            },
            slots: [Slot::none(), Slot::none()],
        }
    }

    /// Back to no loss, with no filter state.
    pub fn reset(&mut self) {
        self.slots = [Slot::none(), Slot::none()];
    }

    /// Gives `slot` row `row`, keeping its state if it already has it.
    fn hold(&mut self, slot: usize, row: usize) {
        let s = &mut self.slots[slot];
        if s.row == row {
            return;
        }
        if s.row == 0 {
            s.stages.iter_mut().for_each(Biquad::reset);
        }
        s.row = row;
        if row > 0 {
            for (stage, section) in s.stages.iter_mut().zip(sections(&self.table[row - 1])) {
                stage.set(section.coefficients(self.rate));
            }
        }
    }

    /// One sample through the loss of `spacing_um` micrometres.
    #[inline]
    pub fn process(&mut self, x: f64, spacing_um: f64) -> f64 {
        let position = (spacing_um / STEP_UM).clamp(0.0, ROWS as f64);
        let lower = (position.floor() as usize).min(ROWS - 1);
        let weight = position - lower as f64;
        // The slot already holding one of the two rows keeps it; the other takes the other row.
        let (a, b) = if self.slots[1].row == lower || self.slots[0].row == lower + 1 {
            (1, 0)
        } else {
            (0, 1)
        };
        self.hold(a, lower);
        self.hold(b, lower + 1);
        let low = self.slots[a].process(x);
        let high = self.slots[b].process(x);
        if weight == 0.0 {
            low
        } else {
            low + (high - low) * weight
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The steady-state gain of a tone through a fixed spacing, in dB.
    fn gain_db(spacing_um: f64, hz: f64, rate: f64) -> f64 {
        let mut s = Spacing::new(rate);
        let n = (rate * 0.5) as usize;
        let mut sum = (0.0, 0.0);
        for i in 0..n {
            let x = (std::f64::consts::TAU * hz * i as f64 / rate).sin();
            let y = s.process(x, spacing_um);
            if i > n / 2 {
                sum.0 += x * x;
                sum.1 += y * y;
            }
        }
        10.0 * (sum.1 / sum.0).log10()
    }

    fn wallace_db(spacing_um: f64, hz: f64) -> f64 {
        -54.6 * spacing_um * 1e-6 * hz / 0.0476
    }

    #[test]
    fn no_spacing_is_the_input_itself() {
        let mut s = Spacing::new(48_000.0);
        for i in 0..1000 {
            let x = (i as f64 * 0.37).sin();
            assert_eq!(s.process(x, 0.0), x);
        }
    }

    #[test]
    fn the_loss_follows_wallace_at_every_common_rate() {
        for rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            for spacing in [0.25, 0.5, 1.0, 1.7, 2.5] {
                for hz in [1_000.0, 4_000.0, 10_000.0] {
                    let (got, want) = (gain_db(spacing, hz, rate), wallace_db(spacing, hz));
                    if want > -40.0 {
                        assert!(
                            (got - want).abs() < 3.0,
                            "{spacing} µm at {hz} Hz, {rate} Hz: {got:.1} dB, Wallace {want:.1} dB"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn between_two_rows_the_loss_is_between_theirs() {
        let (a, b) = (
            gain_db(1.0, 8_000.0, 48_000.0),
            gain_db(1.05, 8_000.0, 48_000.0),
        );
        let mid = gain_db(1.025, 8_000.0, 48_000.0);
        assert!(
            mid <= a.max(b) + 0.2 && mid >= a.min(b) - 0.2,
            "{a} {mid} {b}"
        );
    }

    #[test]
    fn a_sweeping_spacing_never_jumps() {
        // A 1 kHz tone, barely touched by any spacing, through a spacing swept 0 to 3 µm and back:
        // the output moves no faster than the tone itself can.
        let mut s = Spacing::new(48_000.0);
        let n = 96_000;
        let mut previous = 0.0;
        for i in 0..n {
            let t = i as f64 / n as f64;
            let spacing = MAX_UM * (1.0 - (2.0 * t - 1.0).abs());
            let y = s.process(
                (std::f64::consts::TAU * 1_000.0 * i as f64 / 48_000.0).sin(),
                spacing,
            );
            if i > 0 {
                assert!(
                    (y - previous).abs() < 0.2,
                    "jump of {} at sample {i}",
                    y - previous
                );
            }
            previous = y;
        }
    }
}
