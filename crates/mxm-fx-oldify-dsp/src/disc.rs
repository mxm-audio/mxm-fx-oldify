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
//!   (a smooth level change fixed to the angle, up to [`SWISH`] in each of three harmonics).
//! - **Crackle**: a dense bed of single-sample impulses with heavy-tailed (Pareto) sizes, up to
//!   2,000 a second on a 78, where they fuse into the familiar fry.
//! - **Clicks**: sparser, larger, each a burst of noise behind a sharp edge, 20 µs to 4 ms long,
//!   its colour its own and darker the longer it lasts. A click is "localized impulsive
//!   disturbances… durations range from less than 20 µs to 4 ms" whose initial part "behaves as
//!   high-variance noise" (Moliner & Välimäki, after Rayner & Godsill; Esquef): noise, not a
//!   tone. A damped ring of one pitch, which a click is not, is heard as a drop of water.
//! - **Scratches**: at a fixed angle every turn, louder: the stylus drops into the scratch and
//!   climbs out, two edges of opposite sign a scratch's width apart, the same burst each turn
//!   with its size varied a little.
//! - **Cracks**: at a fixed angle every turn, the loudest: the stylus falls across the crack and
//!   strikes its far side, two edges like a scratch's but larger, longer and further apart, which
//!   the replay curve turns into a short pop with body. **No ring follows**: the tonearm's own
//!   swing (Esquef's thump) is at 8–15 Hz, below hearing, and through the replay curve's bass
//!   lift any ring after a crack is heard as a drum machine's bass drum (the owner, 2026-10-09:
//!   "a bass drum with a rather long pitch decay"; 2026-10-10: "It is the 'cracks' that sounds
//!   like bass drums").
//!
//! A burst is free of DC: the groove holds velocity, and the velocity across a bump in the groove
//! sums to nothing; a burst with a net push, through a replay curve that lifts the bass by 20 dB,
//! thumps.
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
/// The swish's depth: each of its three harmonics of the turn moves the surface noise's level by
/// up to this share, so the swing never passes 5.4 dB peak to peak, under the 6 dB estimated at
/// most. Chosen.
pub const SWISH: f64 = 0.1;
/// The longest revolution, at 33⅓ rpm, in seconds: what the event tables are sized for.
const LONGEST_TURN_S: f64 = 1.8;
/// Crackle's size: the smallest impulse, and the tail's shape (Pareto α).
const CRACKLE_SIZE: f64 = 0.004;
const CRACKLE_ALPHA: f64 = 1.6;
/// The largest crackle impulse, as a multiple of the smallest.
const CRACKLE_LARGEST: f64 = 30.0;
/// A click's size.
const CLICK_SIZE: f64 = 0.03;
/// A scratch's and a crack's near edge.
const SCRATCH_SIZE: f64 = 0.12;
const CRACK_SIZE: f64 = 0.5;
/// A burst's DC block: a one-pole high-pass this far below its colour's corner. Chosen.
const BLOCK_BELOW: f64 = 4.0;
/// A click's shortest and longest (Rayner & Godsill, as cited).
const CLICK_SHORTEST_S: f64 = 0.000_02;
const CLICK_LONGEST_S: f64 = 0.004;
/// A burst's colour, a one-pole low-pass: the shortest click's corner, and the longest's. Chosen.
const BRIGHTEST_HZ: f64 = 16_000.0;
const DARKEST_HZ: f64 = 2_500.0;
/// A burst's noise against its edge. Chosen.
const GRIT: f64 = 0.35;
/// A scratch's width, as the time the stylus takes to cross it: 30–300 µm at 0.2–0.5 m/s on an
/// LP, faster on a 78 (est.).
const SCRATCH_WIDTH_S: (f64, f64) = (0.000_1, 0.001_5);
/// A crack's width, the same way: a gap of 0.1–0.5 mm, its far side often higher (est.).
const CRACK_WIDTH_S: (f64, f64) = (0.000_3, 0.002);
/// Voices sounding at once: clicks and the edges of scratches and cracks.
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

/// A burst of coloured noise behind a sharp edge: a click, a scratch's or a crack's edge.
#[derive(Clone, Copy, Debug, Default)]
struct Burst {
    /// The edge's height, signed: the click's polarity.
    size: f64,
    seconds: f64,
    /// The colour's corner.
    hz: f64,
    /// The burst's own generator's seed: the same seed is the same burst.
    seed: u32,
}

impl Burst {
    /// A click: its length log-uniform across Rayner & Godsill's range, darker the longer it is.
    fn click(size: f64, rng: &mut Rng) -> Self {
        let seconds = CLICK_SHORTEST_S * (CLICK_LONGEST_S / CLICK_SHORTEST_S).powf(unit(rng));
        Self {
            size,
            seconds,
            hz: Self::colour(seconds),
            seed: seed(rng),
        }
    }

    /// The corner for a burst this long: from [`BRIGHTEST_HZ`] at the shortest click to
    /// [`DARKEST_HZ`] at the longest, geometrically.
    fn colour(seconds: f64) -> f64 {
        let share = (seconds / CLICK_SHORTEST_S).ln() / (CLICK_LONGEST_S / CLICK_SHORTEST_S).ln();
        BRIGHTEST_HZ * (DARKEST_HZ / BRIGHTEST_HZ).powf(share.clamp(0.0, 1.0))
    }
}

/// One sounding burst.
#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    live: bool,
    /// Samples before it starts.
    wait: u32,
    /// The edge still to come, the noise's level and its per-sample decay, the noise's generator,
    /// the colour's coefficient and state, the DC block's coefficient and state, and the gain that
    /// gives the coloured noise unit RMS.
    edge: f64,
    grit: f64,
    grit_decay: f64,
    noise: Rng,
    colour: f64,
    tone: f64,
    block: f64,
    low: f64,
    gain: f64,
}

/// A gap the stylus crosses, a scratch or a crack: where it sits, its near edge's burst, and the
/// far edge's size against the near one's and its delay.
#[derive(Clone, Copy, Debug, Default)]
struct Gap {
    angle: f64,
    near: Burst,
    far: f64,
    width_s: f64,
}

impl Gap {
    /// A gap at a random angle: its near edge's size within `sizes`, its bursts' lengths within
    /// `lengths` of the clicks' range (as shares of its logarithm), its far edge within `far` of
    /// the near one, and its width within `widths`.
    fn new(
        sizes: (f64, f64),
        lengths: (f64, f64),
        far: (f64, f64),
        widths: (f64, f64),
        rng: &mut Rng,
    ) -> Self {
        let within = |(lo, hi): (f64, f64), rng: &mut Rng| lo + (hi - lo) * unit(rng);
        let angle = unit(rng);
        let sign = if rng.bipolar() < 0.0 { -1.0 } else { 1.0 };
        let size = sign * within(sizes, rng);
        let seconds =
            CLICK_SHORTEST_S * (CLICK_LONGEST_S / CLICK_SHORTEST_S).powf(within(lengths, rng));
        let (narrowest, widest) = widths;
        Self {
            angle,
            near: Burst {
                size,
                seconds,
                hz: Burst::colour(seconds),
                seed: seed(rng),
            },
            far: within(far, rng),
            width_s: narrowest * (widest / narrowest).powf(unit(rng)),
        }
    }
}

/// A fresh seed for a burst.
#[inline]
fn seed(rng: &mut Rng) -> u32 {
    (unit(rng) * f64::from(u32::MAX)) as u32
}

/// The disc.
#[derive(Clone, Debug)]
pub struct Disc {
    rate: f64,
    rng: Rng,
    seed: u32,
    angle: f64,
    /// This revolution's crackle (angle, size) and clicks (angle, burst), sorted by angle, and how
    /// far into each this revolution has come.
    crackle: Vec<(f64, f64)>,
    clicks: Vec<(f64, Burst)>,
    next_crackle: usize,
    next_click: usize,
    scratches: [Gap; MAX_SCRATCHES],
    cracks: [Gap; MAX_CRACKS],
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
            scratches: [Gap::default(); MAX_SCRATCHES],
            cracks: [Gap::default(); MAX_CRACKS],
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
            *amp = SWISH * unit(&mut self.rng);
            *phase = TAU * unit(&mut self.rng);
        }
        let rng = &mut self.rng;
        // A scratch's edges are short, the clicks' middle third, and its far edge lower; a
        // crack's are longer, its far side as high or nearly.
        for scratch in self.scratches.iter_mut() {
            *scratch = Gap::new(
                (0.5 * SCRATCH_SIZE, 1.5 * SCRATCH_SIZE),
                (0.33, 0.66),
                (0.3, 0.9),
                SCRATCH_WIDTH_S,
                rng,
            );
        }
        for crack in self.cracks.iter_mut() {
            *crack = Gap::new(
                (0.6 * CRACK_SIZE, 1.4 * CRACK_SIZE),
                (0.5, 0.9),
                (0.7, 1.0),
                CRACK_WIDTH_S,
                rng,
            );
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
            let angle = unit(rng);
            let sign = if rng.bipolar() < 0.0 { -1.0 } else { 1.0 };
            let size = sign * CLICK_SIZE * (0.3 + 1.7 * unit(rng).powi(2));
            (angle, Burst::click(size, rng))
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

    /// A voice playing `burst` at `scale` of its size, after `wait_s`.
    fn voice(&self, burst: &Burst, scale: f64, wait_s: f64) -> Voice {
        let corner = burst.hz.min(0.45 * self.rate);
        let colour = (-TAU * corner / self.rate).exp();
        let size = burst.size * scale;
        Voice {
            live: true,
            wait: (wait_s * self.rate) as u32,
            edge: size,
            grit: GRIT * size.abs(),
            // Down by e three times over its length.
            grit_decay: (-3.0 / (burst.seconds * self.rate).max(1.0)).exp(),
            noise: Rng::new(burst.seed),
            colour,
            tone: 0.0,
            block: (-TAU * corner / BLOCK_BELOW / self.rate).exp(),
            low: 0.0,
            // A one-pole's output variance is (1 − a) / (1 + a) of white's.
            gain: ((1.0 + colour) / (1.0 - colour)).sqrt(),
        }
    }

    /// Starts a voice, stealing the quietest when all are busy.
    fn start(&mut self, voice: Voice) {
        let loudness = |v: &Voice| v.grit.abs().max(v.edge.abs());
        let slot = self.voices.iter().position(|v| !v.live).unwrap_or_else(|| {
            (0..VOICES)
                .min_by(|&a, &b| loudness(&self.voices[a]).total_cmp(&loudness(&self.voices[b])))
                .unwrap_or(0)
        });
        self.voices[slot] = voice;
    }

    /// The stylus crosses `gap`: into it, then out of it, the far edge the other way a width
    /// later, both varied alike by `vary`.
    fn cross(&mut self, gap: Gap, vary: f64) {
        let near = self.voice(&gap.near, vary, 0.0);
        let far_burst = Burst {
            seed: gap.near.seed.wrapping_add(1),
            ..gap.near
        };
        let far = self.voice(&far_burst, -gap.far * vary, gap.width_s);
        self.start(near);
        self.start(far);
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
            let (_, burst) = self.clicks[self.next_click];
            let voice = self.voice(&burst, 1.0, 0.0);
            self.start(voice);
            self.next_click += 1;
        }
        for i in 0..usize::from(c.scratches) {
            let s = self.scratches[i];
            if s.angle >= before && s.angle < now {
                let vary = 0.85 + 0.3 * unit(&mut self.rng);
                self.cross(s, vary);
            }
        }
        for i in 0..usize::from(c.cracks) {
            let k = self.cracks[i];
            if k.angle >= before && k.angle < now {
                let vary = 0.9 + 0.2 * unit(&mut self.rng);
                self.cross(k, vary);
            }
        }
        if wrapped {
            self.angle -= 1.0;
            self.refill(&c, TURNOVER);
            // Events at the very start of the new turn wait for the next sample.
        }

        // The voices.
        for voice in self.voices.iter_mut().filter(|v| v.live) {
            if voice.wait > 0 {
                voice.wait -= 1;
                continue;
            }
            // The edge and the noise behind it, through the burst's colour, its DC taken out; it
            // plays on until its filters have emptied.
            let white = if voice.grit != 0.0 {
                voice.noise.bipolar() * 3f64.sqrt()
            } else {
                0.0
            };
            let drive = voice.edge + voice.grit * white;
            voice.edge = 0.0;
            voice.tone = voice.colour * voice.tone + (1.0 - voice.colour) * drive;
            voice.low = voice.block * voice.low + (1.0 - voice.block) * voice.tone;
            out += voice.gain * (voice.tone - voice.low);
            voice.grit *= voice.grit_decay;
            if voice.grit.abs() < 1e-7 {
                voice.grit = 0.0;
            }
            voice.live = voice.grit != 0.0 || voice.tone.abs() + voice.low.abs() > 1e-9;
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
    fn a_crack_pops_once_every_turn() {
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

    /// The largest third-octave band's share of `x`'s energy from 200 Hz to 16 kHz.
    fn largest_band_share(x: &[f64]) -> f64 {
        let n = x.len();
        let power = |hz: f64| {
            let (mut re, mut im) = (0.0, 0.0);
            for (i, v) in x.iter().enumerate() {
                let p = TAU * hz * i as f64 / RATE;
                re += v * p.cos();
                im += v * p.sin();
            }
            re * re + im * im
        };
        let step = RATE / n as f64;
        let bands: Vec<f64> = (0..22)
            .map(|b| 200.0 * 2f64.powf(b as f64 / 3.0))
            .map(|centre| {
                let (lo, hi) = (
                    centre * 2f64.powf(-1.0 / 6.0),
                    centre * 2f64.powf(1.0 / 6.0),
                );
                let mut sum = 0.0;
                let mut hz = (lo / step).ceil() * step;
                while hz < hi {
                    sum += power(hz);
                    hz += step;
                }
                sum
            })
            .collect();
        let total: f64 = bands.iter().sum();
        bands.iter().cloned().fold(0.0, f64::max) / total
    }

    /// **A click is noise, not a tone**: a click's energy spreads across the bands, where a ring of
    /// one pitch (heard as a drop of water) puts most of it in one or two.
    #[test]
    fn a_click_is_a_burst_of_noise_not_a_tone() {
        let c = DiscControls {
            clicks: 2.0,
            ..DiscControls::SILENT
        };
        let out = render(c, 6.0, 9);
        let mut shares = Vec::new();
        let mut i = 0;
        while i + 2048 < out.len() {
            if out[i].abs() > 1e-3 {
                shares.push(largest_band_share(&out[i..i + 2048]));
                i += 2048;
            } else {
                i += 1;
            }
        }
        assert!(shares.len() >= 6, "{} clicks", shares.len());
        let worst = shares.iter().cloned().fold(0.0, f64::max);
        assert!(worst < 0.35, "a click put {worst:.2} of itself in one band");
    }

    /// **A scratch is the same tick every turn**, in the same place, only its size varied.
    #[test]
    fn a_scratch_ticks_once_every_turn_the_same_tick_each_time() {
        let c = DiscControls {
            speed: Speed::Shellac78,
            scratches: 1,
            ..DiscControls::SILENT
        };
        let out = render(c, 4.0, 10);
        let turn = (RATE / Speed::Shellac78.hz()).round() as usize;
        let ticks: Vec<&[f64]> = (1..4)
            .map(|r| {
                let first = (r * turn..(r + 1) * turn)
                    .find(|&i| out[i] != 0.0)
                    .expect("a tick every turn");
                &out[first..first + 2048]
            })
            .collect();
        let correlation = |a: &[f64], b: &[f64]| {
            let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
            let norm = |x: &[f64]| x.iter().map(|v| v * v).sum::<f64>().sqrt();
            dot / (norm(a) * norm(b))
        };
        assert!(correlation(ticks[0], ticks[1]) > 0.999);
        assert!(correlation(ticks[1], ticks[2]) > 0.999);
        let share = largest_band_share(ticks[0]);
        assert!(
            share < 0.35,
            "the scratch put {share:.2} of itself in one band"
        );
    }

    /// **A click has no net push**: the groove holds velocity, which across a bump sums to
    /// nothing, so the replay curve's bass lift has nothing to make a thump of.
    #[test]
    fn a_click_a_scratch_and_a_crack_have_no_net_push() {
        for c in [
            DiscControls {
                clicks: 5.0,
                ..DiscControls::SILENT
            },
            DiscControls {
                scratches: 3,
                ..DiscControls::SILENT
            },
            DiscControls {
                cracks: 3,
                ..DiscControls::SILENT
            },
        ] {
            let out = render(c, 4.0, 11);
            let sum: f64 = out.iter().sum();
            let size: f64 = out.iter().map(|v| v.abs()).sum();
            assert!(size > 0.0);
            assert!(sum.abs() < 1e-3 * size, "{c:?}: net {sum:.4} of {size:.2}");
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

    /// The surface noise's level by where the turn is, over many turns, swings by less than the
    /// 6 dB estimated at most, whatever the seed.
    #[test]
    fn surface_noise_swishes_with_the_turn_by_a_few_decibels() {
        let c = DiscControls {
            speed: Speed::Shellac78,
            surface_db: -40.0,
            ..DiscControls::SILENT
        };
        let turn = RATE / Speed::Shellac78.hz();
        for seed in 1..=4 {
            let out = render(c, 16.0 * turn / RATE, seed);
            let mut power = [(0.0, 0usize); 32];
            for (i, v) in out.iter().enumerate() {
                let bin = &mut power[((i as f64 / turn).fract() * 32.0) as usize];
                bin.0 += v * v;
                bin.1 += 1;
            }
            let mean: Vec<f64> = power.iter().map(|(p, n)| p / *n as f64).collect();
            let high = mean.iter().copied().fold(0.0, f64::max);
            let low = mean.iter().copied().fold(f64::MAX, f64::min);
            let swing = 10.0 * (high / low).log10();
            assert!((0.5..6.0).contains(&swing), "seed {seed}: {swing:.1} dB");
        }
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
