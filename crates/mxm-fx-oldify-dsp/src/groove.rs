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
//!
//! **Dynamic wear: the loud passages wear most.** A worn hard stylus "can (and does) cut into
//! record grooves, particularly where the groove contains sharp deviations… Thus we cause
//! distortion and noise on loud notes. The damage is irreversible" (Copeland, *Manual of
//! Analogue Sound Restoration Techniques*, §4.9); below its elastic limit the groove is not worn
//! at all (Hunt, as cited there). What a wall feels is the stylus's force, the groove's velocity
//! times the stylus's mechanical impedance `Z = r + jωm + k/(jω)`: the compliance's pull grows
//! with the displacement (a loud bass note shoves a stiff old stylus a long way) and the tip
//! mass's push with the acceleration (a sharp treble transient), and the two cancel at the
//! stylus's own resonance, [`STYLUS_HZ`]. Where the force passed the wall's limit on earlier
//! plays, the wall gave way: the part above the limit, [`GIVE`] of it, is gone, taken back to
//! velocity through `1/Z` (a resonant band-pass, the exact inverse of the discrete `Z` the force
//! is computed with, so the groove below the limit is untouched and above it stays bounded),
//! which flattens the loud peaks and adds their harmonics; the worn stretch lost its fine
//! detail (its treble dulled toward [`WORN_DULL_HZ`] by the share of the sound the wear took)
//! and is rough (noise following what the wear took). The limit falls with Dynamic
//! wear, linearly in decibels, to [`WORN_FROM_DB`] at full; the two walls' limits differ by
//! [`WALL_SPREAD_DB`], so they wear apart. Below the limit's knee, nothing changes, exactly.

use crate::biquad::{Biquad, Coefficients, Kind, Section};
use crate::transport::Rng;
use std::f64::consts::TAU;

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
/// The stylus's own resonance, where its tip mass and compliance cancel, and its Q: a period
/// pickup's few milligrams on a stiff armature (about 1,000 N/m) put it near 2 kHz (est.).
pub const STYLUS_HZ: f64 = 2_000.0;
const STYLUS_Q: f64 = 1.0;
/// The wall's limit at full Dynamic wear, as the peak of a 1 kHz groove velocity, dBFS: where
/// our measurement of a vinyl emulation's worn stylus starts to compress
/// (`ops: notes/oldify/measurements`).
pub const WORN_FROM_DB: f64 = -24.0;
/// The share of the force above the limit the wall gave way to, and the knee's half-width as a
/// share of the limit. Chosen.
pub const GIVE: f64 = 0.8;
const KNEE: f64 = 0.5;
/// How far apart the two walls' limits are, dB. Chosen; the measured worn stylus differed
/// between the channels.
pub const WALL_SPREAD_DB: f64 = 3.0;
/// The worn stretch's dulled top, its roughness against what the wear took, the roughness's
/// high-pass corner and the envelopes' release. Chosen.
pub const WORN_DULL_HZ: f64 = 3_000.0;
const ROUGH: f64 = 0.3;
const ROUGH_HZ: f64 = 1_000.0;
const WORN_RELEASE_S: f64 = 0.03;
/// The displacement's leak, below the music.
const TRAVEL_LEAK_HZ: f64 = 5.0;

/// The groove's controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrooveControls {
    /// 0 is the outer groove, 1 the inner.
    pub side: f32,
    /// 0–1.
    pub wear: f32,
    /// 0–1: how far the loud passages are worn beyond the rest.
    pub dynamic: f32,
    /// 0–2 of the physical amount; 1 is the physics.
    pub distortion: f32,
    /// The stylus tip's radius, micrometres: 4 (line contact) to 100 (a 78's steel needle).
    pub tip_um: f32,
}

impl GrooveControls {
    pub const CLEAN: Self = Self {
        side: 0.0,
        wear: 0.0,
        dynamic: 0.0,
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
            dynamic: within(self.dynamic, 0.0, 1.0, 0.0),
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

    /// The walls' limit under Dynamic wear, as a 1 kHz groove velocity's peak; `None` when the
    /// loud passages are worn no more than the rest.
    pub fn limit(&self) -> Option<f64> {
        (self.dynamic > 0.0).then(|| 10f64.powf(WORN_FROM_DB * f64::from(self.dynamic) / 20.0))
    }
}

/// One wall's dynamic wear: the last velocity, the leaky displacement, the band-pass taking the
/// excess back to velocity, the envelopes of what it took and of the groove, and the dulling and
/// roughness filters' states.
#[derive(Clone, Copy, Debug, Default)]
struct Wall {
    last: f64,
    travel: f64,
    give: Biquad,
    taken: f64,
    level: f64,
    dull: f64,
    rough: f64,
}

/// The groove, both channels.
#[derive(Clone, Debug)]
pub struct Groove {
    rate: f64,
    tops: [Biquad; 2],
    top_hz: Option<f64>,
    /// The pinch effect's second harmonic, with its slow average taken out.
    pinch_mean: f64,
    walls: [Wall; 2],
    /// Per sample: the displacement's leak, the excess's release, the dulling and roughness poles.
    leak: f64,
    release: f64,
    dull_pole: f64,
    rough_pole: f64,
    rng: Rng,
    seed: u32,
}

impl Groove {
    pub fn new(sample_rate: f64, seed: u32) -> Self {
        let pole = |hz: f64| (-TAU * hz / sample_rate).exp();
        let leak = pole(TRAVEL_LEAK_HZ);
        // The force is Z(z)·v with Z(z) = 1 + Q·D·(1 − z⁻¹) + Q·E / (1 − ℓz⁻¹), D = rate / ω₀,
        // E = ω₀ / rate, ℓ the leak (`wear`'s difference and leaky sum): its inverse, exactly.
        let omega = TAU * STYLUS_HZ;
        let (d, e) = (
            STYLUS_Q * sample_rate / omega,
            STYLUS_Q * omega / sample_rate,
        );
        let n0 = 1.0 + d + e;
        let give = Coefficients {
            b0: 1.0 / n0,
            b1: -leak / n0,
            b2: 0.0,
            a1: -(d + leak * (1.0 + d)) / n0,
            a2: leak * d / n0,
        };
        Self {
            rate: sample_rate,
            tops: [Biquad::default(), Biquad::default()],
            top_hz: None,
            pinch_mean: 0.0,
            walls: [Wall {
                give: Biquad::new(give),
                ..Wall::default()
            }; 2],
            leak,
            release: (-1.0 / (WORN_RELEASE_S * sample_rate)).exp(),
            dull_pole: pole(WORN_DULL_HZ),
            rough_pole: pole(ROUGH_HZ),
            rng: Rng::new(seed ^ 0x6400_0E0F),
            seed,
        }
    }

    /// A new seed for the worn walls' roughness, from the next draw on; a reset starts it from the
    /// top.
    pub fn reseed(&mut self, seed: u32) {
        if seed != self.seed {
            self.seed = seed;
            self.rng = Rng::new(seed ^ 0x6400_0E0F);
        }
    }

    pub fn reset(&mut self) {
        self.tops.iter_mut().for_each(Biquad::reset);
        self.pinch_mean = 0.0;
        for wall in &mut self.walls {
            wall.give.reset();
            *wall = Wall {
                give: wall.give,
                ..Wall::default()
            };
        }
        self.rng = Rng::new(self.seed ^ 0x6400_0E0F);
    }

    /// One sample of wall `i` under dynamic wear at `limit`.
    #[inline]
    fn wear(&mut self, i: usize, v: f64, limit: f64) -> f64 {
        let rate = self.rate;
        let omega = TAU * STYLUS_HZ;
        let wall = &mut self.walls[i];
        wall.travel = self.leak * wall.travel + v / rate;
        let push = (v - wall.last) * rate;
        wall.last = v;
        // The force over the stylus's damping: the velocity, and the tip mass's push and the
        // compliance's pull, which cancel at the resonance.
        let force = v + STYLUS_Q * (push / omega + omega * wall.travel);
        // The left wall a little harder than the right.
        let limit = limit * 10f64.powf(if i == 0 { 0.5 } else { -0.5 } * WALL_SPREAD_DB / 20.0);
        let knee = KNEE * limit;
        let over = force.abs() - limit;
        let excess = if over <= -knee {
            0.0
        } else if over < knee {
            (over + knee).powi(2) / (4.0 * knee)
        } else {
            over
        };
        let given = wall.give.process(GIVE * excess.copysign(force));
        let mut y = v - given;
        wall.taken = given.abs().max(self.release * wall.taken);
        if wall.taken < 1e-9 {
            wall.taken = 0.0;
        }
        wall.level = v.abs().max(self.release * wall.level);
        // The dulling filter follows always, so a worn stretch starts from where the sound is.
        wall.dull = self.dull_pole * wall.dull + (1.0 - self.dull_pole) * y;
        if wall.taken > 0.0 {
            let share = (wall.taken / wall.level.max(1e-12)).min(1.0);
            y += share * (wall.dull - y);
            let white = self.rng.bipolar() * 3f64.sqrt();
            wall.rough = self.rough_pole * wall.rough + (1.0 - self.rough_pole) * white;
            y += ROUGH * wall.taken * (white - wall.rough);
        }
        y
    }

    /// One frame through the groove.
    #[inline]
    pub fn process(&mut self, frame: [f32; 2], c: &GrooveControls) -> [f32; 2] {
        let mut walls = [f64::from(frame[0]), f64::from(frame[1])];
        // The groove as earlier plays left it, then the stylus tracing it today.
        if let Some(limit) = c.limit() {
            for (i, v) in walls.iter_mut().enumerate() {
                *v = self.wear(i, *v, limit);
            }
        } else {
            // Followed while off, so switching it on starts from where the groove is, with nothing
            // worn from before.
            for (wall, v) in self.walls.iter_mut().zip(walls) {
                wall.travel = self.leak * wall.travel + v / self.rate;
                wall.last = v;
                wall.dull = self.dull_pole * wall.dull + (1.0 - self.dull_pole) * v;
                wall.level = 0.0;
                if wall.taken != 0.0 || wall.rough != 0.0 {
                    wall.give.reset();
                    wall.taken = 0.0;
                    wall.rough = 0.0;
                }
            }
        }
        let mut mid = 0.5 * (walls[0] + walls[1]);
        let mut side = 0.5 * (walls[0] - walls[1]);
        let strength = c.strength();
        let square = mid * mid;
        // The pinch's DC follows always, so distortion switched on starts from the sound's own.
        self.pinch_mean += 0.001 * (square - self.pinch_mean);
        if strength > 0.0 {
            mid /= (1.0 + strength * square).sqrt();
            // The second harmonic of the lateral motion, in the vertical, without its DC.
            side += PINCH * strength * 0.25 * (square - self.pinch_mean);
        }
        let mut out = [(mid + side) as f32, (mid - side) as f32];

        // Above `0.4` of the rate the limit passes everything and the groove is left as it is,
        // exactly; the filter still follows, at that corner, so a limit coming down starts from
        // where the sound is.
        let open = 0.4 * self.rate;
        let top = c.top_hz();
        let corner = top.min(open);
        if self.top_hz != Some(corner) {
            self.top_hz = Some(corner);
            let section = Section::new(Kind::LowPass, corner, 0.0, std::f64::consts::FRAC_1_SQRT_2);
            let coefficients = section.coefficients(self.rate);
            self.tops.iter_mut().for_each(|b| b.set(coefficients));
        }
        for (y, top) in out.iter_mut().zip(&mut self.tops) {
            let filtered = top.process(f64::from(*y)) as f32;
            if corner < open {
                *y = filtered;
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
        let mut groove = Groove::new(RATE, 1);
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
        let mut groove = Groove::new(RATE, 1);
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
            dynamic: 1.0,
            distortion: 2.0,
            tip_um: 100.0,
        };
        let mut groove = Groove::new(RATE, 1);
        for i in 0..10_000 {
            let x = if i % 50 < 25 { 4.0 } else { -4.0 };
            let y = groove.process([x, -x], &c);
            assert!(y.iter().all(|v| v.is_finite() && v.abs() < 10.0), "{y:?}");
        }
    }

    /// The left wall's last half second of a one-second tone of `amplitude` at `hz`.
    fn tone(c: &GrooveControls, hz: f64, amplitude: f64) -> Vec<f64> {
        let mut groove = Groove::new(RATE, 1);
        let n = 48_000;
        (0..n)
            .map(|i| {
                let x = (amplitude * (TAU * hz * i as f64 / RATE).sin()) as f32;
                f64::from(groove.process([x, x], c)[0])
            })
            .skip(n / 2)
            .collect()
    }

    /// What is left of `y` once its component at `hz` is taken out, against that component: the
    /// harmonics and the roughness.
    fn residual(y: &[f64], hz: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in y.iter().enumerate() {
            let p = TAU * hz * i as f64 / RATE;
            re += v * p.cos();
            im += v * p.sin();
        }
        let (a, b) = (2.0 * re / y.len() as f64, 2.0 * im / y.len() as f64);
        let rest: f64 = y
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let p = TAU * hz * i as f64 / RATE;
                (v - a * p.cos() - b * p.sin()).powi(2)
            })
            .sum();
        (rest / (0.5 * (a * a + b * b) * y.len() as f64)).sqrt()
    }

    /// **Dynamic wear leaves the quiet passages exactly as they were**, and wears the loud ones
    /// into distortion and a lower level.
    #[test]
    fn dynamic_wear_takes_the_loud_passages_and_leaves_the_quiet_ones() {
        let worn = GrooveControls {
            dynamic: 0.5,
            ..GrooveControls::CLEAN
        };
        assert_eq!(
            tone(&worn, 1_000.0, 0.03),
            tone(&GrooveControls::CLEAN, 1_000.0, 0.03)
        );
        let loud = tone(&worn, 1_000.0, 0.7);
        let clean = tone(&GrooveControls::CLEAN, 1_000.0, 0.7);
        let rms = |x: &[f64]| (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
        assert!(residual(&clean, 1_000.0) < 1e-6);
        assert!(
            residual(&loud, 1_000.0) > 0.05,
            "{:.3}",
            residual(&loud, 1_000.0)
        );
        assert!(rms(&loud) < 0.7 * rms(&clean));
    }

    /// **A loud bass note wears the groove**: at the same velocity a 60 Hz note shoves the stiff
    /// stylus far further than a 1 kHz one, so it is worn where the middle is not.
    #[test]
    fn a_loud_bass_note_wears_where_a_middle_one_as_fast_does_not() {
        let worn = GrooveControls {
            dynamic: 0.5,
            ..GrooveControls::CLEAN
        };
        let bass = residual(&tone(&worn, 60.0, 0.05), 60.0);
        let middle = residual(&tone(&worn, 1_000.0, 0.05), 1_000.0);
        assert!(bass > 0.05, "bass {bass:.3}");
        assert!(middle < 1e-6, "middle {middle:.3}");
    }

    /// **The two walls wear apart**: the same loud groove plays a little differently on each.
    #[test]
    fn the_two_walls_wear_apart() {
        let worn = GrooveControls {
            dynamic: 0.8,
            ..GrooveControls::CLEAN
        };
        let mut groove = Groove::new(RATE, 1);
        let mut apart = 0.0f64;
        for i in 0..4_800 {
            let x = (0.5 * (TAU * 200.0 * i as f64 / RATE).sin()) as f32;
            let y = groove.process([x, x], &worn);
            apart = apart.max((y[0] - y[1]).abs().into());
        }
        assert!(apart > 0.01, "{apart}");
    }

    /// **Distortion switched on in the silence is silent**: the pinch's average, taken while it was
    /// off, is of the silence, not of the loud groove before it.
    #[test]
    fn distortion_switched_on_in_the_silence_is_silent() {
        let distorted = GrooveControls {
            distortion: 1.0,
            ..GrooveControls::CLEAN
        };
        let mut groove = Groove::new(RATE, 1);
        for i in 0..24_000 {
            let x = (0.5 * (TAU * 200.0 * i as f64 / RATE).sin()) as f32;
            groove.process([x, x], &distorted);
        }
        for _ in 0..48_000 {
            groove.process([0.0, 0.0], &GrooveControls::CLEAN);
        }
        let largest = (0..4_800)
            .map(|_| groove.process([0.0, 0.0], &distorted)[0].abs())
            .fold(0.0f32, f32::max);
        assert!(largest < 1e-9, "{largest}");
    }

    /// **Dynamic wear switched back on starts clean**: a groove it followed under its limit, then
    /// switched off through a silence, wears the next loud stretch as a fresh one does.
    #[test]
    fn dynamic_wear_switched_back_on_starts_clean() {
        let worn = GrooveControls {
            dynamic: 1.0,
            ..GrooveControls::CLEAN
        };
        let limit = worn.limit().unwrap();
        // At the stylus's resonance its force is the velocity, up to twice it as the tone starts:
        // this stays under the softer wall's knee, and is louder than the burst after it.
        let under: Vec<f32> = (0..24_000)
            .map(|i| (0.15 * limit * (TAU * STYLUS_HZ * i as f64 / RATE).sin()) as f32)
            .collect();
        // Low, where the force is many times the velocity: over the limit.
        let burst: Vec<f32> = (0..9_600)
            .map(|i| (0.05 * limit * (TAU * 60.0 * i as f64 / RATE).sin()) as f32)
            .collect();
        let play = |history: bool| {
            let mut groove = Groove::new(RATE, 1);
            if history {
                for x in &under {
                    groove.process([*x, *x], &worn);
                }
                // Nothing worn, nothing drawn: only the level it followed is left.
                assert!(
                    groove
                        .walls
                        .iter()
                        .all(|w| w.taken == 0.0 && w.rough == 0.0 && w.level > 0.0)
                );
            }
            for _ in 0..96_000 {
                groove.process([0.0, 0.0], &GrooveControls::CLEAN);
            }
            let out: Vec<f32> = burst
                .iter()
                .map(|x| groove.process([*x, *x], &worn)[0])
                .collect();
            assert!(groove.walls[0].taken > 0.0);
            out
        };
        let (after, fresh) = (play(true), play(false));
        let apart = after
            .iter()
            .zip(&fresh)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(apart < 1e-6, "{apart}");
    }
}
