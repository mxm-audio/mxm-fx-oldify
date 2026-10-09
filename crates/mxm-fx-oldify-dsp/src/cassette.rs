//! The cassette engine, per stereo frame:
//!
//! ```text
//! saturator (pre-emphasis → 2× curve → de-emphasis) → tape response, by Tape tone
//!   → transport (wow and flutter; Flange against the unmoved read) → crosstalk → + hiss
//!   → spacing loss (Age, generations, azimuth, a dropout's lift) → a dropout's dip
//!   → wrong playback EQ → azimuth's delay between the tracks
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
//! **Generations** is a dub of a dub: each copy adds its own noise (the hiss's power times the
//! count), its own wobble (the depth times the count's square root, the copies being
//! uncorrelated) and its own treble loss ([`GENERATION_UM`] of spacing each).
//!
//! **Azimuth** is the head tilted against the tracks. Across one track it is a sinc loss
//! (15 arc-minutes costs 4.9 dB at 10 kHz); here its main lobe is taken as the spacing that loses
//! the same 3.9 dB at half the sinc's first null, so it rides the spacing filter. Between the two
//! tracks it is a delay, [`TRACK_PITCH_M`] · tan θ / speed: summed to mono, 15′ puts a notch at
//! 6.1 kHz (`ops: notes/oldify/reports`, the physics).
//!
//! Hiss is Gaussian-like and seeded, independent per channel, a little darker on the 70 µs tapes
//! (4 dB less, as the time constant moving from 120 µs to 70 µs gives), and it stops in silence:
//! the caller passes the gate.

use crate::Ramp;
use crate::biquad::{Biquad, Kind, Section};
use crate::dropouts::{DropoutControls, Dropouts, LIFT_UM_PER_DB};
use crate::saturate::{Character, Saturator};
use crate::spacing::Spacing;
use crate::tape::{Grade, Tape, TapeResponse};
use crate::transport::{Rng, Shape, Transport, Wobble, hermite};

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
/// The most generations: the original and three dubs.
pub const MAX_GENERATIONS: u8 = 4;
/// Each dub's treble loss, as spacing. Chosen.
pub const GENERATION_UM: f64 = 0.25;
/// The largest azimuth error, in arc-minutes.
pub const MAX_AZIMUTH_ARCMIN: f32 = 60.0;
/// Crosstalk at or below this is off; a good deck separates its tracks by 34–60 dB.
pub const CROSSTALK_OFF_DB: f32 = -80.0;
/// The most crosstalk.
pub const CROSSTALK_MAX_DB: f32 = -30.0;
/// The largest Variation: the seed's range.
pub const MAX_VARIATION: u32 = 9_999;
/// One track's width on a stereo cassette, in metres.
const TRACK_WIDTH_M: f64 = 0.6e-3;
/// Between the two tracks' centres, in metres: what puts 15′'s mono notch at 6.1 kHz.
pub const TRACK_PITCH_M: f64 = 0.9e-3;
/// Cassette speed, metres a second.
const SPEED_M_S: f64 = 0.0476;
/// The 70 µs tapes' hiss, re ferric's.
const HISS_70US_DB: f32 = -4.0;
/// The hiss's top: tape noise is not flat to the Nyquist of a 96 kHz session.
const HISS_TOP_HZ: f64 = 16_000.0;
/// Wrong playback EQ's shelf: playing a tape with the other time constant. Chosen from the two
/// curves' difference, about ±4.7 dB above their corners.
const WRONG_EQ_DB: f64 = 4.7;
const WRONG_EQ_HZ: f64 = 1_800.0;

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
    /// 1 (the original) to [`MAX_GENERATIONS`].
    pub generations: u8,
    pub wow: Motion,
    pub flutter: Motion,
    /// Flutter's rate moves with the wow.
    pub follow: bool,
    /// The unmoved tape mixed in, 0–1: through-zero tape flanging.
    pub flange: f32,
    /// RMS re full scale, [`HISS_OFF_DB`]..=[`HISS_MAX_DB`].
    pub hiss_db: f32,
    pub dropouts: DropoutControls,
    /// Arc-minutes, 0..=[`MAX_AZIMUTH_ARCMIN`].
    pub azimuth: f32,
    /// [`CROSSTALK_OFF_DB`]..=[`CROSSTALK_MAX_DB`].
    pub crosstalk_db: f32,
    /// The tape played back with the other time constant.
    pub wrong_eq: bool,
    /// 0–1.
    pub age: f32,
    /// Which seed every random source starts from, 0..=[`MAX_VARIATION`].
    pub variation: u32,
}

impl CassetteControls {
    /// Nothing but the tape's own delay: no response, saturation, wobble, noise, fault or age.
    pub const NEUTRAL: Self = Self {
        tape: Tape::I,
        grade: Grade::Standard,
        tone: 0.0,
        saturation: 0.0,
        character: Character::Soft,
        generations: 1,
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
        follow: false,
        flange: 0.0,
        hiss_db: HISS_OFF_DB,
        dropouts: DropoutControls::NONE,
        azimuth: 0.0,
        crosstalk_db: CROSSTALK_OFF_DB,
        wrong_eq: false,
        age: 0.0,
        variation: 0,
    };

    /// The init patch's cassette: ferric, standard grade, gently saturated, light random wow and
    /// flutter, hiss at −60 dB, a few light dropouts, a good deck's crosstalk
    /// (`ops: notes/oldify/reports`, factory presets).
    pub const INIT: Self = Self {
        tone: 1.0,
        saturation: 0.25,
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
        dropouts: DropoutControls {
            amount: 0.1,
            intensity: 0.25,
            width: 0.25,
        },
        crosstalk_db: -45.0,
        age: 0.3,
        ..Self::NEUTRAL
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
            generations: self.generations.clamp(1, MAX_GENERATIONS),
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
            flange: unit(self.flange),
            hiss_db: within(self.hiss_db, (HISS_OFF_DB, HISS_MAX_DB), HISS_OFF_DB),
            dropouts: DropoutControls {
                amount: unit(self.dropouts.amount),
                intensity: unit(self.dropouts.intensity),
                width: unit(self.dropouts.width),
            },
            azimuth: within(self.azimuth, (0.0, MAX_AZIMUTH_ARCMIN), 0.0),
            crosstalk_db: within(
                self.crosstalk_db,
                (CROSSTALK_OFF_DB, CROSSTALK_MAX_DB),
                CROSSTALK_OFF_DB,
            ),
            age: unit(self.age),
            variation: self.variation.min(MAX_VARIATION),
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

/// Crosstalk's linear gain; zero when off.
fn crosstalk_gain(crosstalk_db: f32) -> f32 {
    if crosstalk_db <= CROSSTALK_OFF_DB {
        0.0
    } else {
        crate::db_to_gain(crosstalk_db)
    }
}

/// The seed a Variation starts every random source from.
pub fn seed_for(variation: u32) -> u32 {
    1u32.wrapping_add(variation.wrapping_mul(0x9E37_79B9))
}

/// The spacing, in micrometres, whose loss matches an azimuth error's main lobe: the same loss as
/// the sinc's at half its first null.
fn azimuth_spacing_um(arcmin: f64) -> f64 {
    let tan = (arcmin / 60.0).to_radians().tan();
    // Half the first null is f = v / (2 w tan θ), where the sinc has lost 3.92 dB; Wallace's loss
    // there is 54.6 d f / v.
    3.92 * 2.0 * TRACK_WIDTH_M * tan / 54.6 * 1e6
}

/// The delay between the tracks for an azimuth error, in seconds.
fn azimuth_delay_s(arcmin: f64) -> f64 {
    TRACK_PITCH_M * (arcmin / 60.0).to_radians().tan() / SPEED_M_S
}

/// A wobble from its control values, its depth lowered if its read would go past `reach_s`.
fn wobble(depth: f32, hz: f32, shape: Shape, max: f32, scale: f64, reach_s: f64) -> Wobble {
    let mut wobble = Wobble {
        depth: f64::from(max * depth * depth) * scale,
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

    fn reseed(&mut self, seed: u32) {
        self.seed = seed;
        self.rng = Rng::new(seed);
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

/// The delay between the two tracks: each channel read half a span either side of a fixed centre,
/// so the latency stays the same whatever the azimuth.
#[derive(Clone, Debug)]
struct Skew {
    lines: [Vec<f32>; 2],
    write: usize,
    /// The centre, in samples: half the largest delay, and room for the interpolator.
    centre: f64,
}

impl Skew {
    fn new(rate: f64) -> Self {
        let most = azimuth_delay_s(f64::from(MAX_AZIMUTH_ARCMIN)) * rate;
        let centre = (most / 2.0).ceil() + 2.0;
        let length = (2.0 * centre) as usize + 4;
        Self {
            lines: [vec![0.0; length], vec![0.0; length]],
            write: 0,
            centre,
        }
    }

    fn clear(&mut self) {
        self.lines.iter_mut().for_each(|l| l.fill(0.0));
        self.write = 0;
    }

    /// Writes this frame; reads the left `delay / 2` sooner and the right as much later than the
    /// centre. A delay of zero reads both at the centre, an exact whole-sample delay.
    #[inline]
    fn process(&mut self, input: [f32; 2], delay: f64) -> [f32; 2] {
        let length = self.lines[0].len();
        for (line, &x) in self.lines.iter_mut().zip(&input) {
            line[self.write] = x;
        }
        let mut out = [0.0f32; 2];
        for (ch, (y, line)) in out.iter_mut().zip(&self.lines).enumerate() {
            let side = if ch == 0 { -0.5 } else { 0.5 };
            let position = self.write as f64 - (self.centre + side * delay) + length as f64;
            let whole = position.floor();
            let fraction = position - whole;
            let index = whole as usize;
            let at = |k: usize| f64::from(line[(index + k + length - 1) % length]);
            *y = if fraction == 0.0 {
                line[index % length]
            } else {
                hermite(at(0), at(1), at(2), at(3), fraction) as f32
            };
        }
        self.write = (self.write + 1) % length;
        out
    }
}

/// One channel's stages.
#[derive(Clone, Debug)]
struct Channel {
    saturator: Saturator,
    response: TapeResponse,
    hiss: Hiss,
    spacing: Spacing,
    wrong_eq: Biquad,
}

/// The cassette engine. Allocates in [`Cassette::new`] (its delay lines), never after.
#[derive(Clone, Debug)]
pub struct Cassette {
    rate: f64,
    channels: [Channel; 2],
    transport: Transport,
    dropouts: Dropouts,
    skew: Skew,
    tape: Option<(Tape, Grade)>,
    targets: CassetteControls,
    seed: u32,
    tone: Ramp,
    saturation: Ramp,
    wow_depth: Ramp,
    wow_hz: Ramp,
    flutter_depth: Ramp,
    flutter_hz: Ramp,
    flange: Ramp,
    hiss: Ramp,
    azimuth: Ramp,
    crosstalk: Ramp,
    age: Ramp,
}

impl Cassette {
    /// At `sample_rate`, with ramps of `steps` samples.
    pub fn new(sample_rate: f64, steps: u32) -> Self {
        let init = CassetteControls::NEUTRAL;
        let seed = seed_for(init.variation);
        let channel = |n: u32| Channel {
            saturator: Saturator::new(sample_rate),
            response: TapeResponse::default(),
            hiss: Hiss::new(sample_rate, seed.wrapping_add(n)),
            spacing: Spacing::new(sample_rate),
            wrong_eq: Biquad::default(),
        };
        let ramp = |v: f32| Ramp::settled_at(v, steps);
        Self {
            rate: sample_rate,
            channels: [channel(1), channel(2)],
            transport: Transport::new(sample_rate, WOW_REACH_S + FLUTTER_REACH_S, seed),
            dropouts: Dropouts::new(sample_rate, seed),
            skew: Skew::new(sample_rate),
            tape: None,
            targets: init,
            seed,
            tone: ramp(init.tone),
            saturation: ramp(init.saturation),
            wow_depth: ramp(init.wow.depth),
            wow_hz: ramp(init.wow.hz),
            flutter_depth: ramp(init.flutter.depth),
            flutter_hz: ramp(init.flutter.hz),
            flange: ramp(init.flange),
            hiss: ramp(hiss_gain(init.hiss_db)),
            azimuth: ramp(init.azimuth),
            crosstalk: ramp(crosstalk_gain(init.crosstalk_db)),
            age: ramp(init.age),
        }
    }

    /// The delay the engine adds, in samples, whatever its controls: the saturator's oversampler,
    /// the transport's base delay and the centre of the delay between the tracks.
    pub fn latency(&self) -> usize {
        Saturator::LATENCY + self.transport.latency() as usize + self.skew.centre as usize
    }

    /// Whether any hiss is asked for.
    pub fn hisses(&self) -> bool {
        self.targets.hiss_db > HISS_OFF_DB
    }

    pub fn set_controls(&mut self, c: CassetteControls) {
        let c = c.bounded();
        if self.tape != Some((c.tape, c.grade)) {
            self.tape = Some((c.tape, c.grade));
            let shelf = Section::new(
                Kind::HighShelf,
                WRONG_EQ_HZ,
                if c.tape == Tape::I {
                    -WRONG_EQ_DB
                } else {
                    WRONG_EQ_DB
                },
                std::f64::consts::FRAC_1_SQRT_2,
            )
            .coefficients(self.rate);
            for channel in &mut self.channels {
                channel.response.set(c.tape, c.grade, self.rate);
                channel.wrong_eq.set(shelf);
            }
        }
        let seed = seed_for(c.variation);
        if seed != self.seed {
            self.seed = seed;
            self.transport.reseed(seed);
            self.dropouts.reseed(seed);
            for (n, channel) in (1u32..).zip(&mut self.channels) {
                channel.hiss.reseed(seed.wrapping_add(n));
            }
        }
        self.targets = c;
        self.tone.retarget(c.tone);
        self.saturation.retarget(c.saturation);
        self.wow_depth.retarget(c.wow.depth);
        self.wow_hz.retarget(c.wow.hz);
        self.flutter_depth.retarget(c.flutter.depth);
        self.flutter_hz.retarget(c.flutter.hz);
        self.flange.retarget(c.flange);
        self.hiss.retarget(hiss_gain(c.hiss_db));
        self.azimuth.retarget(c.azimuth);
        self.crosstalk.retarget(crosstalk_gain(c.crosstalk_db));
        self.age.retarget(c.age);
    }

    fn ramps(&mut self) -> [&mut Ramp; 11] {
        [
            &mut self.tone,
            &mut self.saturation,
            &mut self.wow_depth,
            &mut self.wow_hz,
            &mut self.flutter_depth,
            &mut self.flutter_hz,
            &mut self.flange,
            &mut self.hiss,
            &mut self.azimuth,
            &mut self.crosstalk,
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
            &self.flange,
            &self.hiss,
            &self.azimuth,
            &self.crosstalk,
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
            channel.wrong_eq.reset();
        }
        self.transport.reset();
        self.dropouts.clear(self.seed);
        self.skew.clear();
    }

    /// One stereo frame through the tape. `noise` (0–1, or more when it follows a loud input)
    /// scales the hiss.
    #[inline]
    pub fn process(&mut self, input: [f32; 2], noise: f32) -> [f32; 2] {
        let c = self.targets;
        let generations = f64::from(c.generations);
        let tone = self.tone.next();
        let saturation = self.saturation.next();
        let wow = wobble(
            self.wow_depth.next(),
            self.wow_hz.next(),
            c.wow.shape,
            WOW_MAX,
            generations.sqrt(),
            WOW_REACH_S,
        );
        let flutter = wobble(
            self.flutter_depth.next(),
            self.flutter_hz.next(),
            c.flutter.shape,
            FLUTTER_MAX,
            generations.sqrt(),
            FLUTTER_REACH_S,
        );
        let flange = self.flange.next();
        let hiss = f64::from(self.hiss.next() * noise)
            * generations.sqrt()
            * if c.tape == Tape::I {
                1.0
            } else {
                f64::from(crate::db_to_gain(HISS_70US_DB))
            };
        let azimuth = f64::from(self.azimuth.next());
        let crosstalk = self.crosstalk.next();
        let spacing = AGE_MAX_UM * f64::from(self.age.next()).powf(1.5)
            + GENERATION_UM * (generations - 1.0)
            + if azimuth > 0.0 {
                azimuth_spacing_um(azimuth)
            } else {
                0.0
            };
        let dips = self.dropouts.next(&c.dropouts);

        let mut recorded = [0.0f32; 2];
        for (y, (channel, &x)) in recorded
            .iter_mut()
            .zip(self.channels.iter_mut().zip(&input))
        {
            let saturated = channel
                .saturator
                .process(x, c.tape, saturation, c.character);
            let shaped = channel.response.process(f64::from(saturated)) as f32;
            *y = saturated + tone * (shaped - saturated);
        }
        let (moving, still) =
            self.transport
                .process_full(recorded, &wow, &flutter, if c.follow { 1.0 } else { 0.0 });
        let mut played = if flange > 0.0 {
            [
                moving[0] + 0.5 * flange * (still[0] - moving[0]),
                moving[1] + 0.5 * flange * (still[1] - moving[1]),
            ]
        } else {
            moving
        };
        if crosstalk > 0.0 {
            played = [
                played[0] + crosstalk * played[1],
                played[1] + crosstalk * played[0],
            ];
        }
        let mut out = [0.0f32; 2];
        for (ch, (y, channel)) in out.iter_mut().zip(&mut self.channels).enumerate() {
            let mut z = f64::from(played[ch]);
            if hiss != 0.0 {
                z += hiss * channel.hiss.next();
            }
            let dip = dips[ch];
            z = channel.spacing.process(z, spacing + LIFT_UM_PER_DB * dip);
            if dip > 0.0 {
                z *= f64::from(crate::db_to_gain(-dip as f32));
            }
            if c.wrong_eq {
                z = channel.wrong_eq.process(z);
            }
            *y = z as f32;
        }
        let delay = if azimuth > 0.0 {
            azimuth_delay_s(azimuth) * self.rate
        } else {
            0.0
        };
        self.skew.process(out, delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azimuth_matches_the_physics_at_fifteen_arc_minutes() {
        // The mono notch: half a period of delay at 6.1 kHz.
        let notch = 1.0 / (2.0 * azimuth_delay_s(15.0));
        assert!((notch - 6_100.0).abs() < 150.0, "notch at {notch} Hz");
        // The equivalent spacing loses about what the sinc does at 10 kHz (4.9 dB), give or take
        // the shapes' difference.
        let loss_10k = 54.6 * azimuth_spacing_um(15.0) * 1e-6 * 10_000.0 / SPEED_M_S;
        assert!((loss_10k - 4.9).abs() < 2.0, "{loss_10k} dB at 10 kHz");
    }

    #[test]
    fn the_skew_is_an_exact_delay_at_zero_and_splits_a_delay_either_side() {
        let mut skew = Skew::new(48_000.0);
        let centre = skew.centre as usize;
        let out: Vec<[f32; 2]> = (0..64)
            .map(|n| skew.process(if n == 3 { [1.0, 1.0] } else { [0.0, 0.0] }, 0.0))
            .collect();
        assert_eq!(out.iter().position(|f| f[0] == 1.0), Some(3 + centre));
        assert_eq!(out.iter().position(|f| f[1] == 1.0), Some(3 + centre));

        let mut skew = Skew::new(48_000.0);
        let out: Vec<[f32; 2]> = (0..64)
            .map(|n| skew.process(if n == 3 { [1.0, 1.0] } else { [0.0, 0.0] }, 4.0))
            .collect();
        let peak = |ch: usize| {
            (0..64)
                .max_by(|&a, &b| out[a][ch].abs().total_cmp(&out[b][ch].abs()))
                .unwrap()
        };
        assert_eq!(peak(0), 3 + centre - 2);
        assert_eq!(peak(1), 3 + centre + 2);
    }

    #[test]
    fn the_seed_follows_variation() {
        assert_ne!(seed_for(0), seed_for(1));
        assert_eq!(seed_for(42), seed_for(42));
    }
}
