//! Dropouts: moments the tape lifts off the head, from a crease, a speck of oxide or a worn spot.
//!
//! Our measurement of the reference emulator (2026-10-09, a 1 kHz tone for 29 s) found 9 to 24 per
//! second at its settings, mostly 1 to 16 dB deep and 3 to 140 ms long, mostly broadband: where
//! 250 Hz dipped 3.5 dB, 8 kHz dipped 5.0 dB, and the treble never dipped alone. Depth deepens
//! them; Intensity trades many shallow ones for fewer, longer, deeper ones; they are the same in
//! both channels until Width is raised, and independent at full Width
//! (`ops: notes/oldify/measurements/`).
//!
//! Each event is a raised-cosine dip of a random length (log-uniform) and depth (uniform),
//! starting at random (Poisson, one draw a sample). A dip lowers the level by its depth and lifts
//! the tape off the head by [`LIFT_UM_PER_DB`] for each decibel, which the spacing loss turns into
//! the extra treble loss measured. The laws on the controls are chosen ([`MAX_RATE_HZ`] and those in
//! [`Dropouts::next`]).

use crate::transport::Rng;

/// Events a second at full amount and no intensity: past the reference's busiest.
pub const MAX_RATE_HZ: f64 = 30.0;
/// The tape's lift for each decibel of dip, in micrometres: 1.5 dB more at 8 kHz than at 250 Hz
/// for a 3.5 dB dip, as measured, is about 0.16 µm (spacing loss is 9.2 dB/µm at 8 kHz).
pub const LIFT_UM_PER_DB: f64 = 0.047;
/// The shortest dropout, in seconds.
const SHORTEST_S: f64 = 0.003;
/// The longest at no intensity; intensity stretches it up to four times.
const LONGEST_S: f64 = 0.040;

/// The dropout controls, each 0–1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropoutControls {
    pub amount: f32,
    pub intensity: f32,
    /// 0 is the same dropouts in both channels; 1 independent ones.
    pub width: f32,
}

impl DropoutControls {
    pub const NONE: Self = Self {
        amount: 0.0,
        intensity: 0.25,
        width: 0.25,
    };
}

/// One stream of events.
#[derive(Clone, Debug)]
struct Events {
    rng: Rng,
    /// Samples into the current event, and its length; no event when `length` is zero.
    at: u32,
    length: u32,
    depth_db: f64,
}

impl Events {
    fn new(seed: u32) -> Self {
        Self {
            rng: Rng::new(seed),
            at: 0,
            length: 0,
            depth_db: 0.0,
        }
    }

    /// This sample's dip, in dB (0 or more).
    #[inline]
    fn next(&mut self, chance: f64, longest: f64, deepest_db: f64, rate: f64) -> f64 {
        if self.length == 0 {
            let draw = 0.5 * (self.rng.bipolar() + 1.0);
            if draw >= chance {
                return 0.0;
            }
            let u = 0.5 * (self.rng.bipolar() + 1.0);
            let seconds = SHORTEST_S * (longest / SHORTEST_S).powf(u);
            self.length = ((seconds * rate) as u32).max(2);
            self.depth_db = 1.0 + (deepest_db - 1.0).max(0.0) * 0.5 * (self.rng.bipolar() + 1.0);
            self.at = 0;
        }
        let shape = (std::f64::consts::PI * f64::from(self.at) / f64::from(self.length)).sin();
        self.at += 1;
        if self.at >= self.length {
            self.length = 0;
        }
        self.depth_db * shape * shape
    }
}

/// The dropouts of both channels.
#[derive(Clone, Debug)]
pub struct Dropouts {
    rate: f64,
    shared: Events,
    own: [Events; 2],
}

impl Dropouts {
    pub fn new(sample_rate: f64, seed: u32) -> Self {
        let mut dropouts = Self {
            rate: sample_rate,
            shared: Events::new(0),
            own: [Events::new(0), Events::new(0)],
        };
        dropouts.reseed(seed);
        dropouts
    }

    /// New seeds from the next draw on, or back to the seed's start after a [`Dropouts::clear`].
    pub fn reseed(&mut self, seed: u32) {
        self.shared.rng = Rng::new(seed.wrapping_add(0x5EED_0001));
        self.own[0].rng = Rng::new(seed.wrapping_add(0x5EED_0002));
        self.own[1].rng = Rng::new(seed.wrapping_add(0x5EED_0003));
    }

    /// No event in progress, every seed back at its start.
    pub fn clear(&mut self, seed: u32) {
        self.shared.length = 0;
        self.own.iter_mut().for_each(|events| events.length = 0);
        self.reseed(seed);
    }

    /// This sample's dip per channel, in dB. Nothing is drawn while the amount is zero.
    #[inline]
    pub fn next(&mut self, c: &DropoutControls) -> [f64; 2] {
        if c.amount <= 0.0 && self.shared.length == 0 && self.own.iter().all(|e| e.length == 0) {
            return [0.0, 0.0];
        }
        let (amount, intensity) = (f64::from(c.amount), f64::from(c.intensity));
        let per_second = MAX_RATE_HZ * amount.powf(1.5) * (1.0 - 0.6 * intensity);
        let chance = per_second / self.rate;
        let longest = LONGEST_S * (1.0 + 3.0 * intensity);
        let deepest = (4.0 + 12.0 * intensity) * (0.5 + 0.5 * amount);
        let width = f64::from(c.width);
        let shared = if width < 1.0 {
            self.shared.next(chance, longest, deepest, self.rate)
        } else {
            0.0
        };
        let mut dips = [shared; 2];
        if width > 0.0 {
            for (dip, events) in dips.iter_mut().zip(&mut self.own) {
                let own = events.next(chance, longest, deepest, self.rate);
                *dip = shared * (1.0 - width) + own * width;
            }
        }
        dips
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(c: DropoutControls, seconds: f64) -> Vec<[f64; 2]> {
        let mut d = Dropouts::new(48_000.0, 7);
        (0..(48_000.0 * seconds) as usize)
            .map(|_| d.next(&c))
            .collect()
    }

    /// Events: runs of nonzero dip.
    fn events(dips: &[f64]) -> Vec<(usize, f64)> {
        let mut out = Vec::new();
        let (mut length, mut deepest) = (0usize, 0.0f64);
        for &dip in dips {
            if dip > 0.0 {
                length += 1;
                deepest = deepest.max(dip);
            } else if length > 0 {
                out.push((length, deepest));
                length = 0;
                deepest = 0.0;
            }
        }
        out
    }

    #[test]
    fn no_amount_is_no_dropouts() {
        assert!(
            run(DropoutControls::NONE, 2.0)
                .iter()
                .all(|d| *d == [0.0, 0.0])
        );
    }

    #[test]
    fn full_amount_is_busy_and_intensity_makes_them_fewer_longer_and_deeper() {
        let mild = DropoutControls {
            amount: 1.0,
            intensity: 0.0,
            width: 0.0,
        };
        let intense = DropoutControls {
            intensity: 1.0,
            ..mild
        };
        let a = events(&run(mild, 20.0).iter().map(|d| d[0]).collect::<Vec<_>>());
        let b = events(&run(intense, 20.0).iter().map(|d| d[0]).collect::<Vec<_>>());
        let rate = |e: &[(usize, f64)]| e.len() as f64 / 20.0;
        let mean = |e: &[(usize, f64)], f: fn(&(usize, f64)) -> f64| {
            e.iter().map(f).sum::<f64>() / e.len() as f64
        };
        assert!((15.0..35.0).contains(&rate(&a)), "{} a second", rate(&a));
        assert!(rate(&b) < rate(&a));
        assert!(mean(&b, |e| e.0 as f64) > 2.0 * mean(&a, |e| e.0 as f64));
        assert!(mean(&b, |e| e.1) > mean(&a, |e| e.1) + 3.0);
        // Within the measured spans: 3 ms up, 16 dB down at most.
        assert!(a.iter().chain(&b).all(|e| e.0 >= 100 && e.1 <= 16.0 + 1e-9));
    }

    #[test]
    fn width_zero_is_the_same_in_both_channels_and_full_width_is_not() {
        let c = DropoutControls {
            amount: 0.6,
            intensity: 0.3,
            width: 0.0,
        };
        assert!(run(c, 5.0).iter().all(|d| d[0] == d[1]));
        let wide = run(DropoutControls { width: 1.0, ..c }, 5.0);
        assert!(wide.iter().any(|d| d[0] != d[1]));
    }

    #[test]
    fn every_dip_rises_and_falls_without_a_jump() {
        let c = DropoutControls {
            amount: 1.0,
            intensity: 1.0,
            width: 0.5,
        };
        let dips = run(c, 5.0);
        for pair in dips.windows(2) {
            for (now, before) in pair[1].iter().zip(&pair[0]) {
                assert!((now - before).abs() < 1.0, "a step of {}", now - before);
            }
        }
    }
}
