//! The cassette engine, per stereo frame:
//!
//! ```text
//! saturator (pre-emphasis → 2× curve → de-emphasis) → tape response, by Tape tone
//!   → transport (wow and flutter) → + hiss → spacing loss (Age)
//! ```
//!
//! The order is the physics' (`ops: notes/oldify/reports`, the cassette signal chain): the record
//! head saturates the tape, playback losses act on what was recorded and on the tape's own noise
//! alike. The reference emulator we measured puts its tape response before its saturator; here the
//! physics stays and that is a known difference: a dull tape still saturates, and its losses take
//! the harmonics too.
//!
//! **Laws chosen here** ([`WOW_MAX`], [`FLUTTER_MAX`], [`AGE_MAX_UM`] and the curves on them):
//! wow and flutter depth go as the square of their controls, so a realistic deck (0.05–0.2 %)
//! sits in the first quarter of the travel and the far end reaches the reference's extremes;
//! Age is a spacing of up to [`AGE_MAX_UM`] (the reference's full Age reads as about 1.7 µm), as
//! the 1.5 power of the control.
//!
//! **The transport's reach is bounded**, so its delay, which the plugin reports as latency, stays
//! near 50 ms: a wobble whose read would go further than [`WOW_REACH_S`] or [`FLUTTER_REACH_S`]
//! has its depth lowered to fit. Only slow, deep wow meets it (1 % of random wow at 0.6 Hz, say).
//!
//! Hiss is Gaussian-like and seeded, independent per channel, a little darker on the 70 µs tapes
//! (4 dB less, as the time constant moving from 120 µs to 70 µs gives), and it stops in silence:
//! the caller passes the gate.

use crate::Ramp;
use crate::saturate::{Character, Saturator};
use crate::spacing::Spacing;
use crate::tape::{Grade, Tape, TapeResponse};
use crate::transport::{Rng, Shape, Transport, Wobble};

/// Wow's peak speed error at full depth: the measured reference's full travel, ±2.8 %.
pub const WOW_MAX: f32 = 0.028;
/// Flutter's at full depth: the reference's, ±5.6 %.
pub const FLUTTER_MAX: f32 = 0.056;
/// Wow's rate range, in hertz.
pub const WOW_HZ: (f32, f32) = (0.1, 5.0);
/// Flutter's rate range, in hertz: a capstan line sits near 7.6 Hz.
pub const FLUTTER_HZ: (f32, f32) = (4.0, 30.0);
/// How far wow may move the read, in seconds either way.
pub const WOW_REACH_S: f64 = 0.020;
/// How far flutter may.
pub const FLUTTER_REACH_S: f64 = 0.005;
/// Hiss at or below this is off.
pub const HISS_OFF_DB: f32 = -96.0;
/// The loudest hiss, RMS re full scale.
pub const HISS_MAX_DB: f32 = -30.0;
/// The spacing at full Age, in micrometres.
pub const AGE_MAX_UM: f64 = 2.0;
/// The 70 µs tapes' hiss, re ferric's.
const HISS_70US_DB: f32 = -4.0;
/// The hiss's top: tape noise is not flat to the Nyquist of a 96 kHz session.
const HISS_TOP_HZ: f64 = 16_000.0;

/// One wobble's controls: depth 0–1, rate in hertz, shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub depth: f32,
    pub hz: f32,
    pub shape: Shape,
}

/// The cassette's controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CassetteControls {
    pub tape: Tape,
    pub grade: Grade,
    /// How much of the tape's response is heard, 0–1.
    pub tone: f32,
    /// 0–1; zero is linear.
    pub saturation: f32,
    pub character: Character,
    pub wow: Motion,
    pub flutter: Motion,
    /// RMS re full scale, [`HISS_OFF_DB`]..=[`HISS_MAX_DB`].
    pub hiss_db: f32,
    /// 0–1.
    pub age: f32,
}

impl CassetteControls {
    /// Nothing but the tape's own delay: no response, saturation, wobble, hiss or age.
    pub const NEUTRAL: Self = Self {
        tape: Tape::I,
        grade: Grade::Standard,
        tone: 0.0,
        saturation: 0.0,
        character: Character::Soft,
        wow: Motion {
            depth: 0.0,
            hz: 0.5,
            shape: Shape::Random,
        },
        flutter: Motion {
            depth: 0.0,
            hz: 7.6,
            shape: Shape::Random,
        },
        hiss_db: HISS_OFF_DB,
        age: 0.0,
    };

    /// The init patch's cassette: ferric, standard grade, gently saturated, light random wow and
    /// flutter, hiss at −60 dB (`ops: notes/oldify/reports`, factory presets).
    pub const INIT: Self = Self {
        tape: Tape::I,
        grade: Grade::Standard,
        tone: 1.0,
        saturation: 0.25,
        character: Character::Soft,
        wow: Motion {
            depth: 0.25,
            hz: 0.5,
            shape: Shape::Random,
        },
        flutter: Motion {
            depth: 0.1,
            hz: 7.6,
            shape: Shape::Random,
        },
        hiss_db: -60.0,
        age: 0.3,
    };

    /// Inside the public ranges; a non-finite value is the neutral one.
    pub fn bounded(self) -> Self {
        let within = |v: f32, (lo, hi): (f32, f32), otherwise: f32| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                otherwise
            }
        };
        let unit = |v: f32| within(v, (0.0, 1.0), 0.0);
        Self {
            tone: unit(self.tone),
            saturation: unit(self.saturation),
            wow: Motion {
                depth: unit(self.wow.depth),
                hz: within(self.wow.hz, WOW_HZ, 0.5),
                shape: self.wow.shape,
            },
            flutter: Motion {
                depth: unit(self.flutter.depth),
                hz: within(self.flutter.hz, FLUTTER_HZ, 7.6),
                shape: self.flutter.shape,
            },
            hiss_db: within(self.hiss_db, (HISS_OFF_DB, HISS_MAX_DB), HISS_OFF_DB),
            age: unit(self.age),
            ..self
        }
    }
}

/// Hiss's linear RMS for a control in dB; zero when off.
pub fn hiss_gain(hiss_db: f32) -> f32 {
    if hiss_db <= HISS_OFF_DB {
        0.0
    } else {
        crate::db_to_gain(hiss_db)
    }
}

/// A wobble from its control values, its depth lowered if its read would go past `reach_s`.
fn wobble(depth: f32, hz: f32, shape: Shape, max: f32, reach_s: f64) -> Wobble {
    let mut wobble = Wobble {
        depth: f64::from(max * depth * depth),
        hz: f64::from(hz),
        shape,
    };
    let reach = wobble.reach_seconds();
    if reach > reach_s {
        wobble.depth *= reach_s / reach;
    }
    wobble
}

/// One channel's tape noise: Gaussian-like white noise under a gentle top.
#[derive(Clone, Debug)]
struct Hiss {
    rng: Rng,
    seed: u32,
    state: f64,
    pole: f64,
    /// Scales the low-passed noise back to unit RMS.
    norm: f64,
}

impl Hiss {
    fn new(rate: f64, seed: u32) -> Self {
        let pole = (-std::f64::consts::TAU * HISS_TOP_HZ.min(0.45 * rate) / rate).exp();
        Self {
            rng: Rng::new(seed),
            seed,
            state: 0.0,
            pole,
            // A one-pole's noise power gain is (1 − a)/(1 + a).
            norm: ((1.0 + pole) / (1.0 - pole)).sqrt(),
        }
    }

    fn reset(&mut self) {
        self.rng = Rng::new(self.seed);
        self.state = 0.0;
    }

    /// The next sample, unit RMS.
    #[inline]
    fn next(&mut self) -> f64 {
        // Four uniforms in −1..1 summed: variance 4/3, near-Gaussian (crest about 11 dB, as tape
        // hiss measures).
        let sum: f64 = (0..4).map(|_| self.rng.bipolar()).sum();
        let white = sum * (3.0f64 / 4.0).sqrt();
        self.state = self.pole * self.state + (1.0 - self.pole) * white;
        self.state * self.norm
    }
}

/// One channel's stages.
#[derive(Clone, Debug)]
struct Channel {
    saturator: Saturator,
    response: TapeResponse,
    hiss: Hiss,
    spacing: Spacing,
}

/// The cassette engine. Allocates in [`Cassette::new`] (the transport's line), never after.
#[derive(Clone, Debug)]
pub struct Cassette {
    rate: f64,
    channels: [Channel; 2],
    transport: Transport,
    tape: Option<(Tape, Grade)>,
    character: Character,
    wow_shape: Shape,
    flutter_shape: Shape,
    tone: Ramp,
    saturation: Ramp,
    wow_depth: Ramp,
    wow_hz: Ramp,
    flutter_depth: Ramp,
    flutter_hz: Ramp,
    hiss: Ramp,
    age: Ramp,
    targets: CassetteControls,
}

impl Cassette {
    /// At `sample_rate`, with ramps of `steps` samples, seeded from `seed`.
    pub fn new(sample_rate: f64, steps: u32, seed: u32) -> Self {
        let channel = |n: u32| Channel {
            saturator: Saturator::new(sample_rate),
            response: TapeResponse::default(),
            hiss: Hiss::new(sample_rate, seed.wrapping_add(n)),
            spacing: Spacing::new(sample_rate),
        };
        let init = CassetteControls::NEUTRAL;
        let ramp = |v: f32| Ramp::settled_at(v, steps);
        Self {
            rate: sample_rate,
            channels: [channel(1), channel(2)],
            transport: Transport::new(sample_rate, WOW_REACH_S + FLUTTER_REACH_S, seed),
            tape: None,
            character: init.character,
            wow_shape: init.wow.shape,
            flutter_shape: init.flutter.shape,
            tone: ramp(init.tone),
            saturation: ramp(init.saturation),
            wow_depth: ramp(init.wow.depth),
            wow_hz: ramp(init.wow.hz),
            flutter_depth: ramp(init.flutter.depth),
            flutter_hz: ramp(init.flutter.hz),
            hiss: ramp(hiss_gain(init.hiss_db)),
            age: ramp(init.age),
            targets: init,
        }
    }

    /// The delay the engine adds, in samples, whatever its controls: the saturator's oversampler
    /// and the transport's base delay.
    pub fn latency(&self) -> usize {
        Saturator::LATENCY + self.transport.latency() as usize
    }

    pub fn set_controls(&mut self, c: CassetteControls) {
        let c = c.bounded();
        self.targets = c;
        self.character = c.character;
        self.wow_shape = c.wow.shape;
        self.flutter_shape = c.flutter.shape;
        if self.tape != Some((c.tape, c.grade)) {
            self.tape = Some((c.tape, c.grade));
            for channel in &mut self.channels {
                channel.response.set(c.tape, c.grade, self.rate);
            }
        }
        self.tone.retarget(c.tone);
        self.saturation.retarget(c.saturation);
        self.wow_depth.retarget(c.wow.depth);
        self.wow_hz.retarget(c.wow.hz);
        self.flutter_depth.retarget(c.flutter.depth);
        self.flutter_hz.retarget(c.flutter.hz);
        self.hiss.retarget(hiss_gain(c.hiss_db));
        self.age.retarget(c.age);
    }

    fn ramps(&mut self) -> [&mut Ramp; 8] {
        [
            &mut self.tone,
            &mut self.saturation,
            &mut self.wow_depth,
            &mut self.wow_hz,
            &mut self.flutter_depth,
            &mut self.flutter_hz,
            &mut self.hiss,
            &mut self.age,
        ]
    }

    /// Ends every ramp at its target.
    pub fn snap(&mut self) {
        self.ramps().into_iter().for_each(Ramp::snap);
    }

    /// Moves every ramp `samples` on, processing nothing.
    pub fn advance(&mut self, samples: u32) {
        self.ramps().into_iter().for_each(|r| r.advance(samples));
    }

    pub fn is_settled(&self) -> bool {
        [
            &self.tone,
            &self.saturation,
            &self.wow_depth,
            &self.wow_hz,
            &self.flutter_depth,
            &self.flutter_hz,
            &self.hiss,
            &self.age,
        ]
        .iter()
        .all(|r| r.is_settled())
    }

    /// Clears every stage's memory and restarts every seed; the controls stay.
    pub fn clear(&mut self) {
        for channel in &mut self.channels {
            channel.saturator.reset();
            channel.response.reset();
            channel.hiss.reset();
            channel.spacing.reset();
        }
        self.transport.reset();
    }

    /// One stereo frame through the tape. `noise` (0–1) gates the hiss.
    #[inline]
    pub fn process(&mut self, input: [f32; 2], noise: f32) -> [f32; 2] {
        let (tape, _) = self.tape.unwrap_or((Tape::I, Grade::Standard));
        let tone = self.tone.next();
        let saturation = self.saturation.next();
        let wow = wobble(
            self.wow_depth.next(),
            self.wow_hz.next(),
            self.wow_shape,
            WOW_MAX,
            WOW_REACH_S,
        );
        let flutter = wobble(
            self.flutter_depth.next(),
            self.flutter_hz.next(),
            self.flutter_shape,
            FLUTTER_MAX,
            FLUTTER_REACH_S,
        );
        let hiss = f64::from(self.hiss.next() * noise)
            * if tape == Tape::I {
                1.0
            } else {
                f64::from(crate::db_to_gain(HISS_70US_DB))
            };
        let spacing = AGE_MAX_UM * f64::from(self.age.next()).powf(1.5);

        let mut recorded = [0.0f32; 2];
        for (y, (channel, &x)) in recorded
            .iter_mut()
            .zip(self.channels.iter_mut().zip(&input))
        {
            let saturated = channel
                .saturator
                .process(x, tape, saturation, self.character);
            let shaped = channel.response.process(f64::from(saturated)) as f32;
            *y = saturated + tone * (shaped - saturated);
        }
        let played = self.transport.process(recorded, &wow, &flutter);
        let mut out = [0.0f32; 2];
        for (y, (channel, &x)) in out.iter_mut().zip(self.channels.iter_mut().zip(&played)) {
            let noisy = if hiss == 0.0 {
                f64::from(x)
            } else {
                f64::from(x) + hiss * channel.hiss.next()
            };
            *y = channel.spacing.process(noisy, spacing) as f32;
        }
        out
    }
}
