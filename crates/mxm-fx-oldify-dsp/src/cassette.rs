//! The cassette engine, per stereo frame:
//!
//! ```text
//! noise-reduction encode → saturator (pre-emphasis → 2× curve → de-emphasis)
//!   → tape response, by Tape tone → + print-through
//!   → transport (wow and flutter; Flange against the unmoved read)
//!   → crosstalk → + hiss → spacing loss (Age, generations, azimuth, a dropout's lift)
//!   → a dropout's dip → the head at a stopping tape's speed → wrong playback EQ
//!   → azimuth's delay between the tracks → noise-reduction decode
//! ```
//!
//! The order is the physics' (`ops: notes/oldify/reports`, the cassette signal chain): the record
//! head saturates the tape, playback losses act on what was recorded and on the tape's own noise
//! alike. The reference emulator we measured puts its tape response before its saturator; here the
//! physics stays and that is a known difference: a dull tape still saturates, and its losses take
//! the harmonics too.
//!
//! **Laws** ([`full_depth`], [`MOTION_MAX`], [`AGE_MAX_UM`] and the curves on them): wow and
//! flutter depth go as their controls, as the reference's do, and a control of 1 is the
//! reference's full travel at its rate, measured from 0.1 to 12 Hz: a sine's peak speed error
//! grows with the rate as the reach it moves the read falls (`ops: notes/oldify/measurements`, the
//! modulations). The controls go on to [`MOTION_MAX`], past the reference, for effect. Age is a
//! spacing of up to [`AGE_MAX_UM`] (the reference's full Age reads as about 1.7 µm), as the 1.5
//! power of the control, a law chosen here.
//!
//! **The transport's reach is bounded**, so its delay, which the plugin reports as latency, stays
//! put: a wobble whose read would go further than [`WOW_REACH_S`] or [`FLUTTER_REACH_S`] has its
//! depth lowered to fit. Only wow past the reference's travel, with copies, meets it.
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
//! **Tape stop**: held, the transport slows the tape to a halt over the stop time and holds it
//! there; let go, it starts again ([`crate::transport`]). A head reads a slower tape more quietly,
//! and the deck's playback EQ, made for full speed, no longer matches it: what plays below its
//! bass corner ([`STOP_LOW_S`]) or above its treble corner (the tape's time constant) falls with
//! the speed ([`Slowed`]), so the tape's sound dulls and thins as it slows, and is gone when it
//! stops.
//!
//! **Print-through** ([`crate::print`]) is the recorded tape a wrap of the reel later, printed
//! onto the layer wound against it on the shelf: added before the transport, it plays back as the
//! music does.
//!
//! Hiss is Gaussian-like and seeded, independent per channel, a little darker on the 70 µs tapes
//! (4 dB less, as the time constant moving from 120 µs to 70 µs gives), and it stops in silence:
//! the caller passes the gate.

use crate::Ramp;
use crate::biquad::{Biquad, Kind, Section};
use crate::dropouts::{DropoutControls, Dropouts, LIFT_UM_PER_DB};
use crate::nr::{Nr, NrControls};
use crate::print::{MAX_PRINT_DB, PRINT_OFF_DB, PrintThrough};
use crate::record::{ECCENTRIC_REACH_S, TABLE_REACH_S, WARP_REACH_S};
use crate::saturate::{Character, Saturator};
use crate::spacing::Spacing;
use crate::tape::{Grade, Tape, TapeResponse};
use crate::transport::{FOLLOW_FLOOR, FOLLOW_HZ, Rng, Shape, Transport, Wobble, hermite};

/// Wow's and Flutter's most: twice the measured reference's full travel, for effect.
pub const MOTION_MAX: f32 = 2.0;
/// The reference's wobble at full travel moves the read `FULL_REACH_S · e^(−FULL_FALL·f +
/// FULL_BEND·f²)` either way at `f` hertz: fitted to it within 0.5 % from 0.1 to 12 Hz.
const FULL_REACH_S: f64 = 3.513e-3;
const FULL_FALL: f64 = 0.1585;
const FULL_BEND: f64 = 0.00213;
/// Where the fit would turn back up, far past the rates asked of it: it holds its value above.
const FULL_TOP_HZ: f64 = FULL_FALL / (2.0 * FULL_BEND);
/// Wow's rate range, in hertz.
pub const WOW_HZ: (f32, f32) = (0.1, 5.0);
/// Flutter's rate range, in hertz: a capstan line sits near 7.6 Hz.
pub const FLUTTER_HZ: (f32, f32) = (4.0, 30.0);
/// How far wow may move the read, in seconds either way: [`MOTION_MAX`] of the reference's full
/// reach and three copies' worth.
pub const WOW_REACH_S: f64 = 0.013;
/// How far flutter may: the same at the slowest flutter.
pub const FLUTTER_REACH_S: f64 = 0.006;
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
pub const SPEED_M_S: f64 = 0.0476;
/// The 70 µs tapes' hiss, re ferric's.
const HISS_70US_DB: f32 = -4.0;
/// The hiss's top: tape noise is not flat to the Nyquist of a 96 kHz session.
const HISS_TOP_HZ: f64 = 16_000.0;
/// The tape stop's time: the shortest, the longest, and the plan's own.
pub const MIN_STOP_S: f32 = 0.05;
pub const MAX_STOP_S: f32 = 10.0;
pub const STOP_S: f32 = 1.5;
/// The playback EQ's bass time constant, seconds: 3180 µs, 50 Hz.
const STOP_LOW_S: f64 = 3180e-6;
/// Wrong playback EQ's shelf: playing a tape with the other time constant. Chosen from the two
/// curves' difference, about ±4.7 dB above their corners.
const WRONG_EQ_DB: f64 = 4.7;
const WRONG_EQ_HZ: f64 = 1_800.0;

/// One wobble's controls: depth 0–[`MOTION_MAX`] (1 the reference's full travel), rate in
/// hertz, shape.
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
    /// The flutter follows the wow: the wow moves only the flutter's rate.
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
    /// The copy a wrap later, dB under the music at its band's peak: [`PRINT_OFF_DB`] (off)
    /// ..= [`MAX_PRINT_DB`].
    pub print_db: f32,
    /// The tape played back with the other time constant.
    pub wrong_eq: bool,
    /// The tape stop held: the tape slows to a halt over `stop_s` and stays stopped.
    pub stop: bool,
    /// How long the tape takes to stop from full speed, seconds, [`MIN_STOP_S`]..=[`MAX_STOP_S`].
    pub stop_s: f32,
    /// 0–1.
    pub age: f32,
    /// Which seed every random source starts from, 0..=[`MAX_VARIATION`].
    pub variation: u32,
    /// Noise reduction: encoded before the tape, decoded after the deck.
    pub nr: NrControls,
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
        print_db: PRINT_OFF_DB,
        wrong_eq: false,
        stop: false,
        stop_s: STOP_S,
        age: 0.0,
        variation: 0,
        nr: NrControls::OFF,
    };

    /// The init patch's cassette: ferric, standard grade, gently saturated, light random wow and
    /// flutter, hiss at −60 dB, a few light dropouts, a good deck's crosstalk
    /// (`ops: notes/oldify/reports`, factory presets).
    pub const INIT: Self = Self {
        tone: 1.0,
        saturation: 0.25,
        wow: Motion {
            depth: 0.319,
            hz: 0.5,
            shape: Shape::Random,
        },
        flutter: Motion {
            depth: 0.018,
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
                depth: within(self.wow.depth, (0.0, MOTION_MAX), 0.0),
                hz: within(self.wow.hz, WOW_HZ, 0.5),
                shape: self.wow.shape,
            },
            flutter: Motion {
                depth: within(self.flutter.depth, (0.0, MOTION_MAX), 0.0),
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
            print_db: within(self.print_db, (PRINT_OFF_DB, MAX_PRINT_DB), PRINT_OFF_DB),
            stop_s: within(self.stop_s, (MIN_STOP_S, MAX_STOP_S), STOP_S),
            age: unit(self.age),
            variation: self.variation.min(MAX_VARIATION),
            nr: self.nr.bounded(),
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

/// Print-through's linear gain; zero when off.
fn print_gain(print_db: f32) -> f32 {
    if print_db <= PRINT_OFF_DB {
        0.0
    } else {
        crate::db_to_gain(print_db)
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

/// A wobble's peak speed error at `hz` at a control of 1: the reference's full travel, a sine's.
pub fn full_depth(hz: f64) -> f64 {
    let hz = hz.clamp(0.0, FULL_TOP_HZ);
    std::f64::consts::TAU * hz * FULL_REACH_S * (-FULL_FALL * hz + FULL_BEND * hz * hz).exp()
}

/// A wobble from its control values, its depth lowered if its read would go past `reach_s` at
/// `slowest` of its rate (a following flutter's floor).
fn wobble(depth: f32, hz: f32, shape: Shape, scale: f64, reach_s: f64, slowest: f64) -> Wobble {
    let hz = f64::from(hz);
    let mut wobble = Wobble {
        depth: full_depth(hz) * f64::from(depth) * scale,
        hz,
        shape,
    };
    let reach = Wobble {
        hz: hz * slowest,
        ..wobble
    }
    .reach_seconds();
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

/// One first-order section, `g·(p + z) / (p + q)` in the analogue, by the bilinear transform.
#[derive(Clone, Copy, Debug, Default)]
struct First {
    b0: f64,
    b1: f64,
    a1: f64,
    state: f64,
}

impl First {
    fn set(&mut self, g: f64, z: f64, q: f64, c: f64) {
        self.b0 = g * (c + z) / (c + q);
        self.b1 = g * (z - c) / (c + q);
        self.a1 = (q - c) / (c + q);
    }

    #[inline]
    fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.state;
        self.state = self.b1 * x - self.a1 * y;
        y
    }
}

/// **What the head and the playback EQ make of a tape at `σ` of its speed.** The head's output
/// for a recorded wavelength goes as the speed, and the EQ, which turns the head's rise with
/// frequency flat between its bass corner ω_lo and its treble corner ω_hi, now meets each sound at
/// σ of its frequency. Against full speed that is `(σ + p/ω_lo)/(1 + p/ω_lo)`, the bass below the
/// corner falling to σ, times `(1 + p/ω_hi)/(1 + p/(σ·ω_hi))`, the treble above it falling to σ:
/// at σ = 1 nothing, at σ = 0 nothing passes.
#[derive(Clone, Copy, Debug, Default)]
struct Slowed {
    low: First,
    high: First,
    speed: f64,
}

impl Slowed {
    #[inline]
    fn process(&mut self, x: f64, speed: f64, treble_s: f64, rate: f64) -> f64 {
        if speed != self.speed {
            self.speed = speed;
            let c = 2.0 * rate;
            let (low, high) = (1.0 / STOP_LOW_S, 1.0 / treble_s);
            self.low.set(1.0, speed * low, low, c);
            self.high.set(speed, high, speed * high, c);
        }
        self.high.process(self.low.process(x))
    }
}

/// One channel's stages.
#[derive(Clone, Debug)]
struct Channel {
    saturator: Saturator,
    response: TapeResponse,
    hiss: Hiss,
    spacing: Spacing,
    slowed: Slowed,
    wrong_eq: Biquad,
}

/// The cassette engine. Allocates in [`Cassette::new`] (its delay lines), never after.
#[derive(Clone, Debug)]
pub struct Cassette {
    rate: f64,
    channels: [Channel; 2],
    transport: Transport,
    dropouts: Dropouts,
    print: PrintThrough,
    skew: Skew,
    nr: Nr,
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
    print_gain: Ramp,
    age: Ramp,
    nr_amount: Ramp,
    nr_brightness: Ramp,
    nr_error: Ramp,
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
            slowed: Slowed::default(),
            wrong_eq: Biquad::default(),
        };
        let ramp = |v: f32| Ramp::settled_at(v, steps);
        Self {
            rate: sample_rate,
            channels: [channel(1), channel(2)],
            transport: Transport::with_stop(
                sample_rate,
                WOW_REACH_S + FLUTTER_REACH_S + ECCENTRIC_REACH_S + TABLE_REACH_S + WARP_REACH_S,
                seed,
                f64::from(MAX_STOP_S),
            ),
            dropouts: Dropouts::new(sample_rate, seed),
            print: PrintThrough::new(sample_rate),
            skew: Skew::new(sample_rate),
            nr: Nr::new(sample_rate),
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
            print_gain: ramp(print_gain(init.print_db)),
            age: ramp(init.age),
            nr_amount: ramp(init.nr.amount),
            nr_brightness: ramp(init.nr.brightness),
            nr_error: ramp(init.nr.level_error_db),
        }
    }

    /// The delay the engine adds, in samples, whatever its controls: the saturator's oversampler,
    /// the transport's base delay and the centre of the delay between the tracks.
    pub fn latency(&self) -> usize {
        Saturator::LATENCY + self.transport.latency() as usize + self.skew.centre as usize
    }

    /// A record's wobbles, which the tape's read follows too: one moving read for both media.
    pub fn set_record_wobbles(&mut self, wobbles: [Wobble; 3]) {
        self.transport.set_extras(wobbles);
    }

    /// Where the record's wobble `index` will be in its turn (0..1) when the sample the record
    /// plays next is read ([`Transport::extra_phase_when_read`]).
    pub fn record_phase_when_read(&self, index: usize) -> f64 {
        self.transport.extra_phase_when_read(index)
    }

    /// Whether any hiss is asked for.
    pub fn hisses(&self) -> bool {
        self.targets.hiss_db > HISS_OFF_DB
    }

    /// The tape's speed this sample, as a share of its own: 1 running, 0 stopped.
    pub fn speed(&self) -> f64 {
        self.transport.speed()
    }

    /// How far behind its base delay the tape's read will be when a stop under way comes to rest,
    /// in samples: how much longer the sound in it plays out ([`Transport::lag_to_come`]).
    pub fn stop_lag_to_come(&self) -> u64 {
        self.transport.lag_to_come().ceil() as u64
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
        self.transport.set_stop(c.stop, f64::from(c.stop_s));
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
        self.print_gain.retarget(print_gain(c.print_db));
        self.age.retarget(c.age);
        self.nr_amount.retarget(c.nr.amount);
        self.nr_brightness.retarget(c.nr.brightness);
        self.nr_error.retarget(c.nr.level_error_db);
    }

    fn ramps(&mut self) -> [&mut Ramp; 15] {
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
            &mut self.print_gain,
            &mut self.age,
            &mut self.nr_amount,
            &mut self.nr_brightness,
            &mut self.nr_error,
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
            &self.print_gain,
            &self.age,
            &self.nr_amount,
            &self.nr_brightness,
            &self.nr_error,
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
            channel.slowed = Slowed::default();
            channel.wrong_eq.reset();
        }
        self.transport.reset();
        self.dropouts.clear(self.seed);
        self.print.clear();
        self.skew.clear();
        self.nr.reset();
    }

    /// One stereo frame through the tape. `noise` (0–1, or more when it follows a loud input)
    /// scales the hiss.
    #[inline]
    pub fn process(&mut self, input: [f32; 2], noise: f32) -> [f32; 2] {
        let c = self.targets;
        let generations = f64::from(c.generations);
        let tone = self.tone.next();
        let saturation = self.saturation.next();
        let wow_depth = self.wow_depth.next();
        let wow = wobble(
            wow_depth,
            self.wow_hz.next(),
            c.wow.shape,
            generations.sqrt(),
            WOW_REACH_S,
            1.0,
        );
        // Following, the wow's control swings the flutter's rate, down to its floor.
        let follow = c.follow.then(|| FOLLOW_HZ * f64::from(wow_depth));
        let flutter = wobble(
            self.flutter_depth.next(),
            self.flutter_hz.next(),
            c.flutter.shape,
            generations.sqrt(),
            FLUTTER_REACH_S,
            if c.follow { FOLLOW_FLOOR } else { 1.0 },
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
        let printed = self.print_gain.next();
        let spacing = AGE_MAX_UM * f64::from(self.age.next()).powf(1.5)
            + GENERATION_UM * (generations - 1.0)
            + if azimuth > 0.0 {
                azimuth_spacing_um(azimuth)
            } else {
                0.0
            };
        let dips = self.dropouts.next(&c.dropouts);
        let nr = NrControls {
            amount: self.nr_amount.next(),
            brightness: self.nr_brightness.next(),
            level_error_db: self.nr_error.next(),
            ..c.nr
        };
        let input = self.nr.encode(input, &nr);

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
        let recorded = self.print.process(recorded, printed);
        let (moving, still) = self
            .transport
            .process_full(recorded, &wow, &flutter, follow);
        let speed = self.transport.speed();
        let treble_s = if c.tape == Tape::I { 120e-6 } else { 70e-6 };
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
            if speed == 0.0 {
                z = 0.0;
                channel.slowed = Slowed::default();
            } else if speed < 1.0 {
                z = channel.slowed.process(z, speed, treble_s, self.rate);
            } else if channel.slowed.speed != 0.0 {
                // Back to speed: the section is through, and starts clean at the next stop.
                channel.slowed = Slowed::default();
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
        let played = self.skew.process(out, delay);
        self.nr.decode(played, &nr)
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
