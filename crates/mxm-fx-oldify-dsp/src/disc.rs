//! The disc: a record that turns, and holds its defects at angles on its surface.
//!
//! **Why a disc and not an overlay.** Listeners pick out added crackle as fake most of the time;
//! noise generated a revolution at a time, with small changes from one revolution to the next, was
//! judged real at chance. What gives fakes away is clicks that never change their sound and noise
//! that repeats too exactly (Moliner & Välimäki, DAFx 2022; `ops: notes/oldify/reports`, the
//! record half). So the disc turns at its speed (1.8 s a turn at 33⅓, 1.333 s at 45, 0.769 s at
//! 78), and keeps, for each revolution, where its crackle and clicks are; each new revolution
//! keeps most of them and replaces [`TURNOVER`] of them. Scratches and cracks sit at fixed angles
//! and come round every turn, each with its own sound, varied a little each time.
//!
//! What it makes, all in the groove, before the replay curve (which softens a vinyl click into a
//! tick and leaves a 78's crackle bright) and before the read-out (so it shares the music's wow):
//!
//! - **Surface noise**: noise rising about 3 dB an octave above 500 Hz, swishing with the turn
//!   (a smooth level change fixed to the angle, a little different each revolution).
//! - **Crackle**: a dense bed of single-sample impulses with heavy-tailed (Pareto) sizes, up to
//!   2,000 a second on a 78, where they fuse into the familiar fry.
//! - **Clicks**: sparser, larger, each a damped ring of its own pitch and length (20 µs to 4 ms).
//! - **Scratches**: a click at a fixed angle every turn, louder, its sound its own.
//! - **Cracks**: a thump at a fixed angle every turn: a short burst, then a damped ring falling in
//!   pitch, over 50 ms (Esquef's model, as cited there).
//! - **Rumble**: low noise under 30 Hz.
//!
//! Every draw comes from the disc's seeded generator, so a render is the same every time and at
//! any block size. The event tables are allocated in [`Disc::new`] and never grow.

use crate::transport::Rng;
use std::f64::consts::{PI, TAU};

/// The share of a revolution's crackle and clicks replaced each turn.
pub const TURNOVER: f64 = 0.12;
/// The most crackle a second, as on a worn 78.
pub const MAX_CRACKLE_HZ: f32 = 2_000.0;
/// The most clicks a second.
pub const MAX_CLICKS_HZ: f32 = 50.0;
pub const MAX_SCRATCHES: usize = 8;
pub const MAX_CRACKS: usize = 3;
/// Surface noise and rumble at or below this are off.
pub const NOISE_OFF_DB: f32 = -80.0;
/// The loudest surface noise, RMS re full scale: a worn 78.
pub const MAX_SURFACE_DB: f32 = -15.0;
/// The loudest rumble.
pub const MAX_RUMBLE_DB: f32 = -35.0;
/// The longest revolution, at 33⅓ rpm, in seconds: what the event tables are sized for.
const LONGEST_TURN_S: f64 = 1.8;
/// Crackle's size: the smallest impulse, and the tail's shape (Pareto α).
const CRACKLE_SIZE: f64 = 0.004;
const CRACKLE_ALPHA: f64 = 1.6;
/// The largest crackle impulse, as a multiple of the smallest.
const CRACKLE_LARGEST: f64 = 30.0;
/// A click's size.
const CLICK_SIZE: f64 = 0.03;
/// A scratch's and a crack's.
const SCRATCH_SIZE: f64 = 0.12;
const CRACK_SIZE: f64 = 0.25;
/// Voices sounding at once: clicks, scratches and thumps.
const VOICES: usize = 32;

/// The disc's speed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    Lp33,
    Single45,
    Shellac78,
}

impl Speed {
    pub fn rpm(self) -> f64 {
        match self {
            Speed::Lp33 => 100.0 / 3.0,
            Speed::Single45 => 45.0,
            Speed::Shellac78 => 78.0,
        }
    }

    /// Revolutions a second.
    pub fn hz(self) -> f64 {
        self.rpm() / 60.0
    }
}

/// The disc's controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiscControls {
    pub speed: Speed,
    /// RMS re full scale, [`NOISE_OFF_DB`]..=[`MAX_SURFACE_DB`].
    pub surface_db: f32,
    /// Impulses a second, 0..=[`MAX_CRACKLE_HZ`].
    pub crackle: f32,
    /// Clicks a second, 0..=[`MAX_CLICKS_HZ`].
    pub clicks: f32,
    pub scratches: u8,
    pub cracks: u8,
    /// RMS re full scale, [`NOISE_OFF_DB`]..=[`MAX_RUMBLE_DB`].
    pub rumble_db: f32,
}

impl DiscControls {
    pub const SILENT: Self = Self {
        speed: Speed::Lp33,
        surface_db: NOISE_OFF_DB,
        crackle: 0.0,
        clicks: 0.0,
        scratches: 0,
        cracks: 0,
        rumble_db: NOISE_OFF_DB,
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
            surface_db: within(self.surface_db, NOISE_OFF_DB, MAX_SURFACE_DB, NOISE_OFF_DB),
            crackle: within(self.crackle, 0.0, MAX_CRACKLE_HZ, 0.0),
            clicks: within(self.clicks, 0.0, MAX_CLICKS_HZ, 0.0),
            scratches: self.scratches.min(MAX_SCRATCHES as u8),
            cracks: self.cracks.min(MAX_CRACKS as u8),
            rumble_db: within(self.rumble_db, NOISE_OFF_DB, MAX_RUMBLE_DB, NOISE_OFF_DB),
            ..self
        }
    }

    /// Whether the disc makes any sound of its own.
    pub fn sounds(&self) -> bool {
        self.surface_db > NOISE_OFF_DB
            || self.crackle > 0.0
            || self.clicks > 0.0
            || self.scratches > 0
            || self.cracks > 0
            || self.rumble_db > NOISE_OFF_DB
    }
}

fn level(db: f32, off: f32) -> f64 {
    if db <= off {
        0.0
    } else {
        f64::from(crate::db_to_gain(db))
    }
}

/// A uniform draw in 0..1.
#[inline]
fn unit(rng: &mut Rng) -> f64 {
    0.5 * (rng.bipolar() + 1.0)
}

/// One sounding event: a damped ring, its pitch possibly falling.
#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    live: bool,
    amp: f64,
    /// Per-sample decay, and the pitch's per-sample fall (1 is none).
    decay: f64,
    fall: f64,
    /// The ring's phase step, and phase.
    step: f64,
    phase: f64,
    /// A burst at the start, in samples, of noise: a thump's initial crack.
    burst: u32,
}

/// A fixed defect: a scratch or a crack.
#[derive(Clone, Copy, Debug, Default)]
struct Fixed {
    angle: f64,
    amp: f64,
    hz: f64,
    decay_s: f64,
}

/// The disc.
#[derive(Clone, Debug)]
pub struct Disc {
    rate: f64,
    rng: Rng,
    seed: u32,
    angle: f64,
    /// This revolution's crackle (angle, size) and clicks (angle, size, pitch Hz, decay s), sorted
    /// by angle, and how far into each this revolution has come.
    crackle: Vec<(f64, f64)>,
    clicks: Vec<(f64, f64, f64, f64)>,
    next_crackle: usize,
    next_click: usize,
    scratches: [Fixed; MAX_SCRATCHES],
    cracks: [Fixed; MAX_CRACKS],
    voices: [Voice; VOICES],
    /// The swish: three harmonics of the turn, amplitude and phase each.
    swish: [(f64, f64); 3],
    /// Surface noise's tilt and rumble's two poles.
    tilt: f64,
    rumble: [f64; 2],
    rumble_pole: f64,
    targets: DiscControls,
}

impl Disc {
    /// Allocates the event tables, sized for the busiest revolution: call outside the audio
    /// thread.
    pub fn new(sample_rate: f64, seed: u32) -> Self {
        let most_crackle = (f64::from(MAX_CRACKLE_HZ) * LONGEST_TURN_S).ceil() as usize + 1;
        let most_clicks = (f64::from(MAX_CLICKS_HZ) * LONGEST_TURN_S).ceil() as usize + 1;
        let mut disc = Self {
            rate: sample_rate,
            rng: Rng::new(seed),
            seed,
            angle: 0.0,
            crackle: Vec::with_capacity(most_crackle),
            clicks: Vec::with_capacity(most_clicks),
            next_crackle: 0,
            next_click: 0,
            scratches: [Fixed::default(); MAX_SCRATCHES],
            cracks: [Fixed::default(); MAX_CRACKS],
            voices: [Voice::default(); VOICES],
            swish: [(0.0, 0.0); 3],
            tilt: 0.0,
            rumble: [0.0; 2],
            rumble_pole: (-TAU * 20.0 / sample_rate).exp(),
            targets: DiscControls::SILENT,
        };
        disc.clear();
        disc
    }

    /// A new seed: a different disc, from the next revolution on.
    pub fn reseed(&mut self, seed: u32) {
        if seed != self.seed {
            self.seed = seed;
            self.clear();
        }
    }

    /// Back to the seed's own disc at angle zero, nothing sounding.
    pub fn clear(&mut self) {
        self.rng = Rng::new(self.seed ^ 0xD15C_0001);
        self.angle = 0.0;
        self.voices = [Voice::default(); VOICES];
        self.tilt = 0.0;
        self.rumble = [0.0; 2];
        for (amp, phase) in &mut self.swish {
            *amp = 0.4 * unit(&mut self.rng);
            *phase = TAU * unit(&mut self.rng);
        }
        for fixed in self.scratches.iter_mut() {
            *fixed = Fixed {
                angle: unit(&mut self.rng),
                amp: SCRATCH_SIZE * (0.5 + unit(&mut self.rng)),
                hz: 1_500.0 + 4_000.0 * unit(&mut self.rng),
                decay_s: 0.0003 + 0.0015 * unit(&mut self.rng),
            };
        }
        for fixed in self.cracks.iter_mut() {
            *fixed = Fixed {
                angle: unit(&mut self.rng),
                amp: CRACK_SIZE * (0.6 + 0.8 * unit(&mut self.rng)),
                hz: 180.0 + 220.0 * unit(&mut self.rng),
                decay_s: 0.03 + 0.04 * unit(&mut self.rng),
            };
        }
        self.crackle.clear();
        self.clicks.clear();
        let targets = self.targets;
        self.refill(&targets, 1.0);
    }

    pub fn set_controls(&mut self, c: DiscControls) {
        self.targets = c.bounded();
    }

    /// Whether the disc, as last set, makes any sound of its own.
    pub fn sounds(&self) -> bool {
        self.targets.sounds()
    }

    /// Replaces `share` of this revolution's events with new ones, matches the tables' sizes to
    /// the asked-for rates, and sorts them by angle.
    fn refill(&mut self, c: &DiscControls, share: f64) {
        let turn_s = 1.0 / c.speed.hz();
        let want_crackle =
            ((f64::from(c.crackle) * turn_s).round() as usize).min(self.crackle.capacity());
        let want_clicks =
            ((f64::from(c.clicks) * turn_s).round() as usize).min(self.clicks.capacity());
        let rng = &mut self.rng;
        let crackle = |rng: &mut Rng| {
            let size = CRACKLE_SIZE * unit(rng).max(1e-9).powf(-1.0 / CRACKLE_ALPHA);
            let sign = if rng.bipolar() < 0.0 { -1.0 } else { 1.0 };
            (unit(rng), sign * size.min(CRACKLE_SIZE * CRACKLE_LARGEST))
        };
        let click = |rng: &mut Rng| {
            let sign = if rng.bipolar() < 0.0 { -1.0 } else { 1.0 };
            (
                unit(rng),
                sign * CLICK_SIZE * (0.3 + 1.7 * unit(rng).powi(2)),
                1_000.0 + 7_000.0 * unit(rng),
                0.00002 * (200.0f64).powf(unit(rng)),
            )
        };
        self.crackle.truncate(want_crackle);
        for event in &mut self.crackle {
            if unit(rng) < share {
                *event = crackle(rng);
            }
        }
        while self.crackle.len() < want_crackle {
            self.crackle.push(crackle(rng));
        }
        self.clicks.truncate(want_clicks);
        for event in &mut self.clicks {
            if unit(rng) < share {
                *event = click(rng);
            }
        }
        while self.clicks.len() < want_clicks {
            self.clicks.push(click(rng));
        }
        self.crackle.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        self.clicks.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        self.next_crackle = 0;
        self.next_click = 0;
    }

    /// Starts a ring, stealing the quietest voice when all are busy.
    fn ring(&mut self, amp: f64, hz: f64, decay_s: f64, fall_per_s: f64, burst_s: f64) {
        let slot = self.voices.iter().position(|v| !v.live).unwrap_or_else(|| {
            (0..VOICES)
                .min_by(|&a, &b| {
                    self.voices[a]
                        .amp
                        .abs()
                        .total_cmp(&self.voices[b].amp.abs())
                })
                .unwrap_or(0)
        });
        self.voices[slot] = Voice {
            live: true,
            amp,
            decay: (-1.0 / (decay_s * self.rate)).exp(),
            fall: (-fall_per_s / self.rate).exp(),
            step: TAU * hz.min(0.45 * self.rate) / self.rate,
            phase: 0.0,
            burst: (burst_s * self.rate) as u32,
        };
    }

    /// One sample of the disc's own sound, in the groove.
    #[inline]
    pub fn sample(&mut self) -> f64 {
        let c = self.targets;
        let step = c.speed.hz() / self.rate;
        let before = self.angle;
        self.angle += step;
        let wrapped = self.angle >= 1.0;
        let now = if wrapped { 1.0 } else { self.angle };
        let mut out = 0.0;

        // The events between the last angle and this one.
        while self.next_crackle < self.crackle.len() && self.crackle[self.next_crackle].0 < now {
            out += self.crackle[self.next_crackle].1;
            self.next_crackle += 1;
        }
        while self.next_click < self.clicks.len() && self.clicks[self.next_click].0 < now {
            let (_, amp, hz, decay) = self.clicks[self.next_click];
            self.ring(amp, hz, decay, 0.0, 0.0);
            self.next_click += 1;
        }
        for i in 0..usize::from(c.scratches) {
            let s = self.scratches[i];
            if s.angle >= before && s.angle < now {
                let vary = 0.85 + 0.3 * unit(&mut self.rng);
                self.ring(s.amp * vary, s.hz * vary, s.decay_s, 0.0, 0.0);
            }
        }
        for i in 0..usize::from(c.cracks) {
            let k = self.cracks[i];
            if k.angle >= before && k.angle < now {
                let vary = 0.9 + 0.2 * unit(&mut self.rng);
                // The pitch falls by e every 40 ms: from a few hundred hertz toward the bass.
                self.ring(k.amp * vary, k.hz, k.decay_s, 25.0, 0.0015);
            }
        }
        if wrapped {
            self.angle -= 1.0;
            self.refill(&c, TURNOVER);
            // Events at the very start of the new turn wait for the next sample.
        }

        // The voices.
        for voice in self.voices.iter_mut().filter(|v| v.live) {
            let mut v = voice.amp * voice.phase.sin();
            if voice.burst > 0 {
                v += voice.amp * 0.5 * self.rng.bipolar();
                voice.burst -= 1;
            }
            out += v;
            voice.phase += voice.step;
            voice.step *= voice.fall;
            voice.amp *= voice.decay;
            if voice.amp.abs() < 1e-7 {
                voice.live = false;
            }
        }

        // Surface noise, swishing with the turn.
        let surface = level(c.surface_db, NOISE_OFF_DB);
        if surface > 0.0 {
            let white = self.rng.bipolar() * 3f64.sqrt();
            // A first-order lift: the difference from a slow average leans the noise upward.
            self.tilt += 0.06 * (white - self.tilt);
            let tilted = white + 1.5 * (white - self.tilt);
            let swish: f64 = self
                .swish
                .iter()
                .enumerate()
                .map(|(k, (a, p))| a * (TAU * (k as f64 + 1.0) * self.angle + p).sin())
                .sum();
            out += surface * 0.5 * tilted * (1.0 + swish);
        }

        // Rumble: low-passed noise.
        let rumble = level(c.rumble_db, NOISE_OFF_DB);
        if rumble > 0.0 {
            let white = self.rng.bipolar() * 3f64.sqrt();
            self.rumble[0] = self.rumble_pole * self.rumble[0] + (1.0 - self.rumble_pole) * white;
            self.rumble[1] =
                self.rumble_pole * self.rumble[1] + (1.0 - self.rumble_pole) * self.rumble[0];
            // Two identical poles pass white noise's power times (1 − a)⁴ (1 + a²) / (1 − a²)³,
            // a = the pole: divided back out, the rumble has unit RMS before its level.
            let a = self.rumble_pole;
            let norm = ((1.0 - a * a).powi(3) / ((1.0 - a).powi(4) * (1.0 + a * a))).sqrt();
            out += rumble * self.rumble[1] * norm;
        }
        out
    }

    /// The disc's angle, 0..1: where a defect locked to it sits.
    pub fn angle(&self) -> f64 {
        self.angle
    }
}

/// The angle in radians, for a wobble locked to the turn.
pub fn radians(angle: f64) -> f64 {
    TAU * angle - PI
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn render(c: DiscControls, seconds: f64, seed: u32) -> Vec<f64> {
        let mut disc = Disc::new(RATE, seed);
        disc.set_controls(c);
        disc.clear();
        (0..(RATE * seconds) as usize)
            .map(|_| disc.sample())
            .collect()
    }

    #[test]
    fn a_silent_disc_is_exact_zero() {
        assert!(
            render(DiscControls::SILENT, 3.0, 1)
                .iter()
                .all(|s| *s == 0.0)
        );
    }

    #[test]
    fn crackle_comes_at_its_rate_and_its_sizes_are_heavy_tailed() {
        let c = DiscControls {
            crackle: 200.0,
            ..DiscControls::SILENT
        };
        let out = render(c, 9.0, 2);
        let impulses: Vec<f64> = out.iter().filter(|s| **s != 0.0).map(|s| s.abs()).collect();
        let per_second = impulses.len() as f64 / 9.0;
        assert!((per_second - 200.0).abs() < 25.0, "{per_second} a second");
        let mean = impulses.iter().sum::<f64>() / impulses.len() as f64;
        let largest = impulses.iter().cloned().fold(0.0, f64::max);
        assert!(
            largest > 6.0 * mean,
            "not heavy-tailed: largest {largest}, mean {mean}"
        );
    }

    #[test]
    fn crackle_is_locked_to_the_turn_but_never_exactly_repeats() {
        // Compare one revolution with the next: most impulses sit where they sat, some moved.
        let c = DiscControls {
            speed: Speed::Shellac78,
            crackle: 400.0,
            ..DiscControls::SILENT
        };
        let out = render(c, 3.0, 3);
        let turn = (RATE / Speed::Shellac78.hz()).round() as usize;
        let at = |from: usize| -> Vec<usize> {
            (from..from + turn - 2)
                .filter(|&i| out[i] != 0.0)
                .map(|i| i - from)
                .collect()
        };
        let (a, b) = (at(turn), at(2 * turn));
        let kept = a
            .iter()
            .filter(|i| b.iter().any(|j| j.abs_diff(**i) <= 1))
            .count();
        let share = kept as f64 / a.len() as f64;
        assert!(
            (0.7..0.97).contains(&share),
            "{share} of the crackle came round again"
        );
    }

    #[test]
    fn a_crack_thumps_once_every_turn() {
        let c = DiscControls {
            speed: Speed::Shellac78,
            cracks: 1,
            ..DiscControls::SILENT
        };
        let out = render(c, 4.0, 4);
        let turn = RATE / Speed::Shellac78.hz();
        // The loudest moment of each revolution sits at the same place.
        let peaks: Vec<usize> = (0..4)
            .map(|r| {
                let from = (r as f64 * turn) as usize;
                let to = (((r + 1) as f64) * turn) as usize;
                (from..to.min(out.len()))
                    .max_by(|&x, &y| out[x].abs().total_cmp(&out[y].abs()))
                    .unwrap()
                    - from
            })
            .collect();
        for p in &peaks[1..] {
            assert!(p.abs_diff(peaks[0]) < 400, "{peaks:?}");
        }
    }

    #[test]
    fn surface_noise_and_rumble_play_at_about_their_levels() {
        let rms = |x: &[f64]| (x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64).sqrt();
        let surface = render(
            DiscControls {
                surface_db: -40.0,
                ..DiscControls::SILENT
            },
            4.0,
            5,
        );
        let db = 20.0 * rms(&surface[RATE as usize..]).log10();
        assert!((db + 40.0).abs() < 3.0, "surface at {db:.1} dB");
        let rumble = render(
            DiscControls {
                rumble_db: -40.0,
                ..DiscControls::SILENT
            },
            4.0,
            6,
        );
        let db = 20.0 * rms(&rumble[RATE as usize..]).log10();
        assert!((db + 40.0).abs() < 3.0, "rumble at {db:.1} dB");
    }

    #[test]
    fn the_same_seed_is_the_same_disc_and_another_is_not() {
        let c = DiscControls {
            crackle: 300.0,
            clicks: 5.0,
            scratches: 2,
            surface_db: -50.0,
            ..DiscControls::SILENT
        };
        assert_eq!(render(c, 2.0, 7), render(c, 2.0, 7));
        assert_ne!(render(c, 2.0, 7), render(c, 2.0, 8));
    }
}
