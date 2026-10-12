//! The record engine: a record side, a disc, a groove and a player.
//!
//! ```text
//! the recording horn → era (mono, the recording's band, the cutting limiter, the cutter, a brick
//!   wall) → cutting curve → + the groove echo → groove (dynamic wear, tracing distortion, wear)
//!   → + the disc's own sound → the stylus's skips → pickup → replay curve → player (scratch
//!   filter, valves, speaker, horn or radio) → the gramophone's horn
//! ```
//!
//! The two horns are [`crate::horn`]'s, set by Horn and Horn size on any record and any player.
//! The groove echo, the music a turn before, is [`crate::echo`]'s: cut into the groove, it is worn
//! and read as the music is. The skips are [`crate::skip`]'s: the stylus jumping back a groove,
//! the groove and its own sound with it, before the pickup.
//!
//! The record's read-out (eccentricity, the turntable's wobble, the warp) is the chain's one moving
//! read, shared with the cassette ([`Record::wobbles`]): a time-warp passes through the curves and
//! the player almost unchanged and through a memoryless curve exactly, so one delay line serves
//! both media, and every wobble shares its one room ([`crate::cassette::WOBBLE_ROOM_S`]). Hum is
//! added after the read ([`Hum`]), so it does not wobble. The warp's level dips at the stylus,
//! before any tape, by where in its turn the warp will be when the sample is read, so the dip and
//! the speed line up ([`crate::warp`]).
//!
//! **The era layer** (`ops: notes/oldify/reports`, from Copeland's *Manual of Analogue Sound
//! Restoration* §6, §11, §13): cutter resonances near 4.5–5.5 kHz in the first electrical years;
//! a deliberate 8.5 kHz brick wall on US 78s from about 1938 to 1947; cutters reaching 12 kHz
//! from late 1943 and 14 kHz with full-range microphones; limiters taking 4–8 dB off the peaks,
//! common in the US by 1939 and almost mandatory in popular music from 1954; mono before 1957–58.
//! The acoustic era's band, about 250 Hz to 2.5 kHz, is the cited 1920s figure. The resonances'
//! Q and the limiter's times are chosen.

use crate::biquad::{Biquad, Kind, Section};
use crate::curves::{Curve, CurveFilter};
use crate::disc::{Disc, DiscControls, NOISE_OFF_DB, Speed};
use crate::echo::{GrooveEcho, MAX_ECHO_DB};
use crate::groove::{Groove, GrooveControls};
use crate::horn::{Horn, HornControls};
use crate::player::{Player, PlayerChain};
use crate::skip::{MAX_SKIPS, Skips};
use crate::transport::{Shape, Wobble, random_rms_share};
use crate::{Ramp, SMOOTHING_S, warp};
use std::f64::consts::{FRAC_1_SQRT_2, SQRT_2, TAU};

/// The most eccentricity, millimetres: offsets above 1 mm are common on old shellac, the measured
/// reference's full travel is about 2.5 mm on an LP's outer groove, and twice that is for effect.
pub const MAX_ECCENTRICITY_MM: f32 = 5.0;
/// The most warp: twice the measured reference's full warp (a Warp of 1), for effect.
pub const MAX_WARP: f32 = 2.0;
/// Which of [`Record::wobbles`] is the warp.
pub const WARP_WOBBLE: usize = 2;
/// The loudest hum, RMS re full scale.
pub const MAX_HUM_DB: f32 = -20.0;
/// Wear raises the surface noise by up to this much.
const WEAR_NOISE_DB: f32 = 9.0;
/// The cutting limiter's times.
const LIMIT_ATTACK_S: f64 = 0.0025;
const LIMIT_RELEASE_S: f64 = 0.5;

/// When the record was cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Era {
    /// Before 1925: horn and diaphragm, no microphone.
    Acoustic,
    /// 1925–35: the first electrical cutters, resonant near 4.5 kHz.
    EarlyElectrical,
    /// US 78s of the 1940s: the 8.5 kHz brick wall, a limiter.
    Us78,
    /// UK 78s of the 1940s: a wider cutter, no limiter.
    Uk78,
    /// Late 78s, the 1950s: full-range cutters.
    Late78,
    /// Early 45s, 1949–52.
    EarlySingle,
    /// Early LPs, 1948–54.
    EarlyLp,
    /// Mono LPs, 1955–58.
    MonoLp,
    /// From stereo on.
    Modern,
}

/// An era's record side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EraModel {
    pub cut: Curve,
    /// The acoustic horn's band, hertz.
    band: Option<(f64, f64)>,
    /// The cutter's resonant top: hertz and Q.
    cutter: Option<(f64, f64)>,
    brick_hz: Option<f64>,
    limit_db: f64,
    pub mono: bool,
}

impl Era {
    pub const ALL: [Era; 9] = [
        Era::Acoustic,
        Era::EarlyElectrical,
        Era::Us78,
        Era::Uk78,
        Era::Late78,
        Era::EarlySingle,
        Era::EarlyLp,
        Era::MonoLp,
        Era::Modern,
    ];

    pub fn model(self) -> EraModel {
        let m = |cut, band, cutter, brick_hz, limit_db, mono| EraModel {
            cut,
            band,
            cutter,
            brick_hz,
            limit_db,
            mono,
        };
        match self {
            Era::Acoustic => m(Curve::Flat, Some((250.0, 2_500.0)), None, None, 0.0, true),
            Era::EarlyElectrical => m(Curve::Us78, None, Some((4_500.0, 2.5)), None, 0.0, true),
            Era::Us78 => m(
                Curve::Us78,
                None,
                Some((9_000.0, 1.2)),
                Some(8_500.0),
                6.0,
                true,
            ),
            Era::Uk78 => m(Curve::Uk78, None, Some((12_000.0, 1.0)), None, 0.0, true),
            Era::Late78 => m(
                Curve::Uk78Treble,
                None,
                Some((14_000.0, 0.9)),
                None,
                4.0,
                true,
            ),
            Era::EarlySingle => m(Curve::Early45, None, Some((12_000.0, 1.0)), None, 6.0, true),
            Era::EarlyLp => m(Curve::EarlyLp, None, Some((14_000.0, 0.9)), None, 4.0, true),
            Era::MonoLp => m(
                Curve::Standard,
                None,
                Some((15_000.0, 0.8)),
                None,
                4.0,
                true,
            ),
            Era::Modern => m(Curve::Standard, None, None, None, 0.0, false),
        }
    }
}

/// Mains hum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mains {
    Off,
    Fifty,
    Sixty,
}

/// The record's controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecordControls {
    pub disc: DiscControls,
    pub era: Era,
    pub player: Player,
    /// `None` follows the era.
    pub cut: Option<Curve>,
    /// `None` follows the player.
    pub replay: Option<Curve>,
    /// Side, wear and distortion; the tip comes from the player.
    pub groove: GrooveControls,
    pub eccentricity_mm: f32,
    /// How warped the disc is: 1 is the measured reference's full warp, up to [`MAX_WARP`].
    pub warp: f32,
    pub mains: Mains,
    /// RMS re full scale, [`NOISE_OFF_DB`] (off) ..= [`MAX_HUM_DB`].
    pub hum_db: f32,
    /// The acoustic horns the record was made and played through.
    pub horn: HornControls,
    /// The groove echo's level under the music, [`NOISE_OFF_DB`] (off) ..= [`MAX_ECHO_DB`].
    pub echo_db: f32,
    /// The chance a turn that the stylus jumps back a groove, 0 ..= [`MAX_SKIPS`].
    pub skips: f32,
    /// Stuck groove held: the stylus goes round the last turn until it is let go.
    pub stuck: bool,
}

impl RecordControls {
    /// A new LP of today on a modern deck: nearly clean.
    pub const CLEAN: Self = Self {
        disc: DiscControls::SILENT,
        era: Era::Modern,
        player: Player::ModernDeck,
        cut: None,
        replay: None,
        groove: GrooveControls::CLEAN,
        eccentricity_mm: 0.0,
        warp: 0.0,
        mains: Mains::Off,
        hum_db: NOISE_OFF_DB,
        horn: HornControls::OFF,
        echo_db: NOISE_OFF_DB,
        skips: 0.0,
        stuck: false,
    };

    /// The init patch's record: a played LP on a modern deck.
    pub const INIT: Self = Self {
        disc: DiscControls {
            speed: Speed::Lp33,
            surface_db: -70.0,
            crackle: 60.0,
            crackle_gain: 1.0,
            clicks: 1.0,
            scratches: 0,
            cracks: 0,
            rumble_db: NOISE_OFF_DB,
        },
        groove: GrooveControls {
            side: 0.25,
            wear: 0.2,
            dynamic: 0.0,
            distortion: 1.0,
            tip_um: 8.0,
        },
        eccentricity_mm: 0.2,
        // Off, but ready at a level that is heard when it is switched on.
        hum_db: -50.0,
        ..Self::CLEAN
    };

    pub fn bounded(self) -> Self {
        Self {
            disc: self.disc.bounded(),
            groove: self.groove.bounded(),
            eccentricity_mm: if self.eccentricity_mm.is_finite() {
                self.eccentricity_mm.clamp(0.0, MAX_ECCENTRICITY_MM)
            } else {
                0.0
            },
            warp: if self.warp.is_finite() {
                self.warp.clamp(0.0, MAX_WARP)
            } else {
                0.0
            },
            hum_db: if self.hum_db.is_finite() {
                self.hum_db.clamp(NOISE_OFF_DB, MAX_HUM_DB)
            } else {
                NOISE_OFF_DB
            },
            horn: self.horn.bounded(),
            echo_db: if self.echo_db.is_finite() {
                self.echo_db.clamp(NOISE_OFF_DB, MAX_ECHO_DB)
            } else {
                NOISE_OFF_DB
            },
            skips: if self.skips.is_finite() {
                self.skips.clamp(0.0, MAX_SKIPS)
            } else {
                0.0
            },
            ..self
        }
    }
}

/// The groove's radius at a side position, metres: a 12" LP, a 10" 78, a 7" single.
fn radius_m(speed: Speed, side: f64) -> f64 {
    let (outer, inner) = match speed {
        Speed::Lp33 => (0.146, 0.060),
        Speed::Shellac78 => (0.120, 0.050),
        Speed::Single45 => (0.085, 0.054),
    };
    outer - (outer - inner) * side
}

/// Mains hum: the mains and its next two harmonics, added after the read.
#[derive(Clone, Debug, Default)]
pub struct Hum {
    phase: f64,
}

impl Hum {
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// The hum's next sample at `gain`, its RMS.
    #[inline]
    pub fn next(&mut self, mains: Mains, gain: f64, rate: f64) -> f64 {
        let hz = match mains {
            Mains::Off => return 0.0,
            Mains::Fifty => 50.0,
            Mains::Sixty => 60.0,
        };
        if gain <= 0.0 {
            return 0.0;
        }
        self.phase = (self.phase + hz / rate).fract();
        let p = TAU * self.phase;
        // The fundamental, a strong second, a weaker third; RMS of the sum about the level.
        let shape = p.sin() + 0.6 * (2.0 * p).sin() + 0.3 * (3.0 * p).sin();
        gain * shape * SQRT_2 / (1.0f64 + 0.36 + 0.09).sqrt()
    }
}

/// The record side's filters, one channel.
#[derive(Clone, Debug, Default)]
struct Side {
    band: [Biquad; 2],
    cutter: Biquad,
    brick: [Biquad; 2],
}

/// The record engine.
#[derive(Clone, Debug)]
pub struct Record {
    rate: f64,
    era: Option<Era>,
    sides: [Side; 2],
    limiter: f64,
    attack: f64,
    release: f64,
    cut: [CurveFilter; 2],
    cut_curve: Option<Curve>,
    replay: [CurveFilter; 2],
    replay_curve: Option<Curve>,
    groove: Groove,
    disc: Disc,
    player: PlayerChain,
    horn: Horn,
    echo: GrooveEcho,
    /// The echo's gain, ramped.
    echo_gain: Ramp,
    skips: Skips,
    /// The warp, ramped: the level's dip follows it.
    warp: Ramp,
    /// The groove's side, wear, dynamic wear and distortion, the horn's amount and the hum's gain,
    /// ramped: each moves what is heard.
    side: Ramp,
    wear: Ramp,
    dynamic: Ramp,
    distortion: Ramp,
    horn_amount: Ramp,
    horn_length: Ramp,
    hum_gain: Ramp,
    /// Whether the horn sounded last sample: one coming in starts from an empty horn.
    horned: bool,
    /// The mains the hum last played at: switched off, the hum fades out at it.
    hum_mains: Mains,
    targets: RecordControls,
}

impl Record {
    /// Allocates the disc's tables and the echo's turn: call outside the audio thread.
    pub fn new(sample_rate: f64, seed: u32) -> Self {
        let steps = ((sample_rate * f64::from(SMOOTHING_S)).round() as u32).max(1);
        let curve = |cut| {
            [
                CurveFilter::new(Curve::Flat, cut, sample_rate),
                CurveFilter::new(Curve::Flat, cut, sample_rate),
            ]
        };
        Self {
            rate: sample_rate,
            era: None,
            sides: Default::default(),
            limiter: 0.0,
            attack: 1.0 - (-1.0 / (sample_rate * LIMIT_ATTACK_S)).exp(),
            release: 1.0 - (-1.0 / (sample_rate * LIMIT_RELEASE_S)).exp(),
            cut: curve(true),
            cut_curve: None,
            replay: curve(false),
            replay_curve: None,
            groove: Groove::new(sample_rate, seed),
            disc: Disc::new(sample_rate, seed),
            player: PlayerChain::new(sample_rate),
            horn: Horn::new(sample_rate),
            echo: GrooveEcho::new(sample_rate),
            echo_gain: Ramp::settled_at(0.0, steps),
            skips: Skips::new(sample_rate, seed),
            warp: Ramp::settled_at(0.0, steps),
            side: Ramp::settled_at(RecordControls::CLEAN.groove.side, steps),
            wear: Ramp::settled_at(RecordControls::CLEAN.groove.wear, steps),
            dynamic: Ramp::settled_at(RecordControls::CLEAN.groove.dynamic, steps),
            distortion: Ramp::settled_at(RecordControls::CLEAN.groove.distortion, steps),
            horn_amount: Ramp::settled_at(RecordControls::CLEAN.horn.amount, steps),
            horn_length: Ramp::settled_at(RecordControls::CLEAN.horn.length_m, steps),
            hum_gain: Ramp::settled_at(0.0, steps),
            horned: false,
            hum_mains: Mains::Off,
            targets: RecordControls::CLEAN,
        }
    }

    fn ramps(&mut self) -> [&mut Ramp; 9] {
        [
            &mut self.warp,
            &mut self.echo_gain,
            &mut self.side,
            &mut self.wear,
            &mut self.dynamic,
            &mut self.distortion,
            &mut self.horn_amount,
            &mut self.horn_length,
            &mut self.hum_gain,
        ]
    }

    pub fn reseed(&mut self, seed: u32) {
        self.disc.reseed(seed);
        self.groove.reseed(seed);
        self.skips.reseed(seed);
    }

    /// True while the groove is locked: Stuck groove held, the stylus going round.
    pub fn locked(&self) -> bool {
        self.skips.locked()
    }

    /// How much longer a skip under way sounds, samples ([`Skips::left`]).
    pub fn skip_left(&self) -> u64 {
        self.skips.left()
    }

    /// Whether the record makes any sound of its own: the disc's, or hum.
    pub fn sounds(&self) -> bool {
        self.disc.sounds()
            || (self.targets.mains != Mains::Off && self.targets.hum_db > NOISE_OFF_DB)
    }

    /// The hum's mains and level, as set.
    pub fn mains(&self) -> (Mains, f64) {
        (self.hum_mains, f64::from(self.hum_gain.value()))
    }

    /// Moves every ramp on by `samples`, the disc's too, playing nothing.
    pub fn advance(&mut self, samples: u32) {
        self.ramps().into_iter().for_each(|r| r.advance(samples));
        self.disc.advance(samples);
    }

    /// Ends every ramp at its target, the disc's too.
    pub fn snap(&mut self) {
        self.ramps().into_iter().for_each(Ramp::snap);
        self.disc.snap();
    }

    pub fn is_settled(&self) -> bool {
        [
            &self.warp,
            &self.echo_gain,
            &self.side,
            &self.wear,
            &self.dynamic,
            &self.distortion,
            &self.horn_amount,
            &self.horn_length,
            &self.hum_gain,
        ]
        .iter()
        .all(|r| r.is_settled())
            && self.disc.is_settled()
    }

    /// Clears every memory; the disc goes back to its seed's first turn.
    pub fn clear(&mut self) {
        for side in &mut self.sides {
            for b in side.band.iter_mut().chain(side.brick.iter_mut()) {
                b.reset();
            }
            side.cutter.reset();
        }
        self.limiter = 0.0;
        self.cut.iter_mut().for_each(CurveFilter::reset);
        self.replay.iter_mut().for_each(CurveFilter::reset);
        self.groove.reset();
        self.disc.clear();
        self.player.reset();
        self.horn.clear();
        self.echo.clear();
        self.skips.clear();
    }

    pub fn set_controls(&mut self, c: RecordControls) {
        let c = c.bounded();
        self.targets = c;
        let rate = self.rate;
        if self.era != Some(c.era) {
            // A new era starts its stages clean: a band or a wall it did not use holds nothing
            // from the last time it was used.
            for side in &mut self.sides {
                for b in side.band.iter_mut().chain(side.brick.iter_mut()) {
                    b.reset();
                }
                side.cutter.reset();
            }
            self.limiter = 0.0;
            self.era = Some(c.era);
            let m = c.era.model();
            let at = |kind, hz: f64, q: f64| Section::new(kind, hz, 0.0, q).coefficients(rate);
            for side in &mut self.sides {
                if let Some((lo, hi)) = m.band {
                    side.band[0].set(at(Kind::HighPass, lo, FRAC_1_SQRT_2));
                    side.band[1].set(at(Kind::LowPass, hi, FRAC_1_SQRT_2));
                }
                if let Some((hz, q)) = m.cutter {
                    side.cutter.set(at(Kind::LowPass, hz, q));
                }
                if let Some(hz) = m.brick_hz {
                    side.brick[0].set(at(Kind::LowPass, hz, 0.54));
                    side.brick[1].set(at(Kind::LowPass, hz, 1.31));
                }
            }
        }
        self.player.set_player(c.player);
        self.horn_amount.retarget(c.horn.amount);
        self.horn_length.retarget(c.horn.length_m);
        self.side.retarget(c.groove.side);
        self.wear.retarget(c.groove.wear);
        self.dynamic.retarget(c.groove.dynamic);
        self.distortion.retarget(c.groove.distortion);
        if c.mains != Mains::Off {
            self.hum_mains = c.mains;
        }
        self.hum_gain
            .retarget(if c.mains != Mains::Off && c.hum_db > NOISE_OFF_DB {
                crate::db_to_gain(c.hum_db)
            } else {
                0.0
            });
        self.warp.retarget(c.warp);
        self.echo_gain.retarget(if c.echo_db > NOISE_OFF_DB {
            crate::db_to_gain(c.echo_db)
        } else {
            0.0
        });
        let cut = c.cut.unwrap_or(c.era.model().cut);
        if self.cut_curve != Some(cut) {
            self.cut_curve = Some(cut);
            self.cut = [
                CurveFilter::new(cut, true, rate),
                CurveFilter::new(cut, true, rate),
            ];
        }
        let replay = c.replay.unwrap_or(c.player.model().replay);
        if self.replay_curve != Some(replay) {
            self.replay_curve = Some(replay);
            self.replay = [
                CurveFilter::new(replay, false, rate),
                CurveFilter::new(replay, false, rate),
            ];
        }
        // The disc hears the player's own rumble and the groove's wear in its noise.
        let model = c.player.model();
        let mut disc = c.disc;
        let power = |db: f32| {
            if db <= NOISE_OFF_DB {
                0.0
            } else {
                10f64.powf(f64::from(db) / 10.0)
            }
        };
        let rumble = power(disc.rumble_db) + power(model.rumble_db);
        disc.rumble_db = (10.0 * rumble.log10()) as f32;
        if disc.surface_db > NOISE_OFF_DB {
            disc.surface_db += WEAR_NOISE_DB * c.groove.wear;
        }
        self.disc.set_controls(disc);
    }

    /// The record's moving read this sample: eccentricity locked to the turn, the turntable's own
    /// wobble, and the warp ([`WARP_WOBBLE`]), at the turn too. The transport fits them into the
    /// room every wobble shares.
    pub fn wobbles(&self) -> [Wobble; 3] {
        let c = &self.targets;
        let speed = c.disc.speed;
        let eccentric = Wobble {
            depth: f64::from(c.eccentricity_mm) * 1e-3 / radius_m(speed, f64::from(c.groove.side)),
            hz: speed.hz(),
            shape: Shape::Sine,
        };
        let (wrms, hz) = c.player.model().wobble;
        let table = Wobble {
            depth: wrms / random_rms_share(),
            hz,
            shape: Shape::Random,
        };
        let warp = Wobble {
            depth: warp::depth(f64::from(c.warp)),
            hz: speed.hz(),
            shape: Shape::Warp,
        };
        [eccentric, table, warp]
    }

    /// One frame through the record. `noise` (0–1) gates the disc's own sound; `turn` is where the
    /// warp will be in its turn when this frame is read
    /// ([`crate::cassette::Cassette::record_phase_when_read`]).
    #[inline]
    pub fn process(&mut self, frame: [f32; 2], noise: f32, turn: f64) -> [f32; 2] {
        let c = self.targets;
        let warped = f64::from(self.warp.next());
        let echo = f64::from(self.echo_gain.next());
        let groove = GrooveControls {
            side: self.side.next(),
            wear: self.wear.next(),
            dynamic: self.dynamic.next(),
            distortion: self.distortion.next(),
            tip_um: c.player.model().tip_um,
        };
        self.hum_gain.next();
        self.echo.set_groove_speed(groove.speed());
        self.horn.set(HornControls {
            amount: self.horn_amount.next(),
            length_m: self.horn_length.next(),
        });
        let era = c.era.model();
        let horned = self.horn.sounds();
        if horned && !self.horned {
            // A horn coming in starts empty, not with whatever it held when it went out.
            self.horn.clear();
        }
        self.horned = horned;
        let mut x = [f64::from(frame[0]), f64::from(frame[1])];
        if horned {
            x = self.horn.record(x);
        }
        if era.mono {
            let mid = 0.5 * (x[0] + x[1]);
            x = [mid, mid];
        }
        if era.limit_db > 0.0 {
            let threshold = 10f64.powf(-era.limit_db / 20.0);
            let peak = x[0].abs().max(x[1].abs());
            let coefficient = if peak > self.limiter {
                self.attack
            } else {
                self.release
            };
            self.limiter += coefficient * (peak - self.limiter);
            if self.limiter > threshold {
                let gain = threshold / self.limiter;
                x = [x[0] * gain, x[1] * gain];
            }
        }
        for (v, (side, cut)) in x.iter_mut().zip(self.sides.iter_mut().zip(&mut self.cut)) {
            if era.band.is_some() {
                let low_cut = side.band[0].process(*v);
                *v = side.band[1].process(low_cut);
            }
            if era.cutter.is_some() {
                *v = side.cutter.process(*v);
            }
            if era.brick_hz.is_some() {
                let first = side.brick[0].process(*v);
                *v = side.brick[1].process(first);
            }
            *v = cut.process(*v);
        }
        x = self.echo.process(x, echo, c.disc.speed);
        let grooved = self.groove.process([x[0] as f32, x[1] as f32], &groove);
        // The disc turns while the record plays, gated or silent, so its defects stay where the
        // turn is and a sound in progress plays out.
        let own = self.disc.sample() * f64::from(noise);
        // The warp's dip, at the stylus: the music and the disc's own sound alike.
        let lift = if warped > 0.0 {
            10f64.powf(-warp::DIP_DB * warped * warp::dip(turn) / 20.0)
        } else {
            1.0
        };
        // The stylus finds the groove, or jumps it, the disc's own sound with it; only music
        // playing starts a skip.
        let playing = frame[0].abs() > crate::SILENCE || frame[1].abs() > crate::SILENCE;
        let found = self.skips.process(
            [f64::from(grooved[0]) + own, f64::from(grooved[1]) + own],
            c.skips,
            c.stuck,
            c.disc.speed,
            playing,
        );
        let picked = self.player.pickup([found[0] * lift, found[1] * lift]);
        let replayed = [
            self.replay[0].process(picked[0]),
            self.replay[1].process(picked[1]),
        ];
        let mut out = self.player.colour(replayed);
        if horned {
            out = self.horn.play(out);
        }
        [out[0] as f32, out[1] as f32]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn tone_db(c: RecordControls, hz: f64) -> f64 {
        tone_db_at(c, hz, 0.0)
    }

    /// A tone's gain through the record, the warp held at `turn`.
    fn tone_db_at(c: RecordControls, hz: f64, turn: f64) -> f64 {
        let mut record = Record::new(RATE, 1);
        record.set_controls(c);
        let n = 48_000;
        let (mut input, mut output) = (0.0, 0.0);
        for i in 0..n {
            let x = (0.05 * (TAU * hz * i as f64 / RATE).sin()) as f32;
            let y = record.process([x, x], 0.0, turn)[0];
            if i > n / 2 {
                input += f64::from(x).powi(2);
                output += f64::from(y).powi(2);
            }
        }
        10.0 * (output / input).log10()
    }

    /// **The disc turns while its sound is gated**: a record whose noise was gated off for a while
    /// plays on exactly as one that was never gated, its defects where the turn is.
    #[test]
    fn the_disc_turns_while_gated() {
        let c = RecordControls {
            disc: DiscControls {
                scratches: 3,
                crackle: 300.0,
                clicks: 5.0,
                ..RecordControls::INIT.disc
            },
            ..RecordControls::INIT
        };
        let run = |gated: bool| {
            let mut record = Record::new(RATE, 1);
            record.set_controls(c);
            record.snap();
            let out: Vec<f32> = (0..3 * RATE as usize)
                .map(|n| {
                    let noise = if gated && (RATE as usize..2 * RATE as usize).contains(&n) {
                        0.0
                    } else {
                        1.0
                    };
                    record.process([0.0, 0.0], noise, 0.0)[0]
                })
                .collect();
            (record.disc.angle(), out)
        };
        let ((angle, always), (gated_angle, gated)) = (run(false), run(true));
        assert_eq!(angle, gated_angle);
        // The filters after the disc remember the gated stretch for a moment; past it the two are
        // the same sound.
        let apart = (5 * RATE as usize / 2..3 * RATE as usize)
            .map(|n| (always[n] - gated[n]).abs())
            .fold(0.0f32, f32::max);
        assert!(apart < 1e-6, "{apart}");
    }

    /// **Hum switched off fades out**: its level ramps to nothing at the mains it played, not cut
    /// at once. Switched a sixth of a cycle in, where the hum is near its peak.
    #[test]
    fn hum_switched_off_fades_out() {
        let on = RecordControls {
            mains: Mains::Fifty,
            hum_db: MAX_HUM_DB,
            ..RecordControls::CLEAN
        };
        let mut record = Record::new(RATE, 1);
        record.set_controls(on);
        record.snap();
        let mut hum = Hum::default();
        let mut previous = 0.0f64;
        let mut largest = 0.0f64;
        let switch = 4_800 + 160;
        let mut at_switch = 0.0f64;
        for n in 0..9_600 {
            if n == switch {
                at_switch = previous;
                record.set_controls(RecordControls {
                    mains: Mains::Off,
                    ..on
                });
            }
            record.process([0.0, 0.0], 1.0, 0.0);
            let (mains, gain) = record.mains();
            if n == switch {
                assert_eq!(mains, Mains::Fifty, "the hum went off before its level");
            }
            let y = hum.next(mains, gain, RATE);
            if n > 0 {
                largest = largest.max((y - previous).abs());
            }
            previous = y;
        }
        // The hum moves at most about 0.003 a sample at 50 Hz and its loudest; a cut would jump by
        // all of it.
        assert!(at_switch > 0.1, "{at_switch}");
        assert!(largest < 0.02 && previous == 0.0, "{largest} {previous}");
    }

    /// **A stage switched out and back starts clean**: an era's wall and limiter and a player's
    /// stages, left holding a loud tone when the record moved to another era and player, give
    /// nothing back when it returns in silence; nor does Dynamic wear switched off and on.
    #[test]
    fn a_stage_switched_out_and_back_starts_clean() {
        let loud = RecordControls {
            era: Era::Us78,
            player: Player::Radiogram,
            groove: GrooveControls {
                dynamic: 1.0,
                distortion: 1.0,
                ..GrooveControls::CLEAN
            },
            ..RecordControls::CLEAN
        };
        let other = RecordControls {
            era: Era::Modern,
            player: Player::ModernDeck,
            groove: GrooveControls::CLEAN,
            ..RecordControls::CLEAN
        };
        let mut record = Record::new(RATE, 1);
        record.set_controls(loud);
        record.snap();
        for i in 0..24_000 {
            let x = (0.9 * (TAU * 220.0 * i as f64 / RATE).sin()) as f32;
            record.process([x, x], 0.0, 0.0);
        }
        record.set_controls(other);
        record.snap();
        for _ in 0..24_000 {
            record.process([0.0, 0.0], 0.0, 0.0);
        }
        record.set_controls(loud);
        record.snap();
        let back = (0..4_800)
            .map(|_| record.process([0.0, 0.0], 0.0, 0.0)[0].abs())
            .fold(0.0f32, f32::max);
        assert!(back < 1e-6, "{back}");
    }

    /// **A moved level ramps**: hum and surface noise turned up go there over the smoothing time,
    /// not in one sample.
    #[test]
    fn a_moved_record_level_ramps() {
        let quiet = RecordControls {
            mains: Mains::Fifty,
            hum_db: -60.0,
            ..RecordControls::CLEAN
        };
        let loud = RecordControls {
            hum_db: MAX_HUM_DB,
            ..quiet
        };
        let mut record = Record::new(RATE, 1);
        record.set_controls(quiet);
        record.snap();
        record.set_controls(loud);
        record.process([0.0, 0.0], 1.0, 0.0);
        let (_, first) = record.mains();
        assert!(
            first < 0.1 * f64::from(crate::db_to_gain(MAX_HUM_DB)),
            "{first}"
        );
        record.advance(RATE as u32);
        let (_, settled) = record.mains();
        assert_eq!(settled, f64::from(crate::db_to_gain(MAX_HUM_DB)));
    }

    #[test]
    fn a_clean_modern_record_on_a_modern_deck_is_nearly_flat() {
        for hz in [100.0, 1_000.0, 8_000.0] {
            let db = tone_db(RecordControls::CLEAN, hz);
            assert!(db.abs() < 1.0, "{hz} Hz: {db:.2} dB");
        }
    }

    #[test]
    fn the_us_78s_brick_wall_and_the_acoustic_band_take_their_toll() {
        let us = RecordControls {
            era: Era::Us78,
            ..RecordControls::CLEAN
        };
        assert!(tone_db(us, 14_000.0) < tone_db(us, 1_000.0) - 20.0);
        // Played flat, as a gramophone would, so only the recording's own band shows.
        let acoustic = RecordControls {
            era: Era::Acoustic,
            replay: Some(Curve::Flat),
            ..RecordControls::CLEAN
        };
        assert!(tone_db(acoustic, 80.0) < tone_db(acoustic, 1_000.0) - 10.0);
        assert!(tone_db(acoustic, 8_000.0) < tone_db(acoustic, 1_000.0) - 10.0);
    }

    #[test]
    fn a_uk_78_on_a_modern_deck_is_dull_and_on_its_own_curve_is_not() {
        let wrong = RecordControls {
            era: Era::Uk78,
            ..RecordControls::CLEAN
        };
        let right = RecordControls {
            replay: Some(Curve::Uk78),
            ..wrong
        };
        let tilt = |c| tone_db(c, 8_000.0) - tone_db(c, 1_000.0);
        assert!(
            tilt(wrong) < tilt(right) - 6.0,
            "{} against {}",
            tilt(wrong),
            tilt(right)
        );
    }

    #[test]
    fn eccentricity_wobbles_at_the_turn_and_grows_toward_the_middle() {
        let mut record = Record::new(RATE, 1);
        let outer = RecordControls {
            eccentricity_mm: 0.2,
            ..RecordControls::CLEAN
        };
        record.set_controls(outer);
        let a = record.wobbles()[0];
        assert!((a.hz - Speed::Lp33.hz()).abs() < 1e-9);
        record.set_controls(RecordControls {
            groove: GrooveControls {
                side: 1.0,
                ..GrooveControls::CLEAN
            },
            ..outer
        });
        let b = record.wobbles()[0];
        // 0.2 mm at the inner groove of an LP: the cited 0.66 % peak to peak.
        assert!(
            (2.0 * b.depth - 0.0066).abs() < 0.0005,
            "{} % p-p",
            200.0 * b.depth
        );
        assert!(b.depth > 2.0 * a.depth);
    }

    #[test]
    fn eccentricity_reaches_twice_the_reference_and_its_full_travel_fits_the_room() {
        // The reference's full eccentricity, measured: ±1.74 % on an LP's outer groove, ±3.48 %
        // on its inner.
        let mut record = Record::new(RATE, 1);
        let at = |record: &mut Record, mm: f32, side: f32| {
            record.set_controls(RecordControls {
                eccentricity_mm: mm,
                groove: GrooveControls {
                    side,
                    ..GrooveControls::CLEAN
                },
                ..RecordControls::CLEAN
            });
            record.wobbles()[0].depth
        };
        let full = at(&mut record, MAX_ECCENTRICITY_MM, 0.0);
        assert!(full > 2.0 * 0.0174 * 0.95, "{} % at full", 100.0 * full);
        // Half our travel is the reference's: on the inner groove it fits the room unlowered.
        let half = 0.5 * MAX_ECCENTRICITY_MM;
        let inner = at(&mut record, half, 1.0);
        let unlowered = f64::from(half) * 1e-3 / radius_m(Speed::Lp33, 1.0);
        assert!(
            inner > 0.0348 && (inner - unlowered).abs() < 1e-12,
            "{inner}"
        );
        let reach = record.wobbles()[0].reach_seconds();
        assert!(reach < crate::cassette::WOBBLE_ROOM_S, "{reach} s");
    }

    /// **A warp errs alike on every disc and groove** (the disc bent from its label), its whole
    /// travel inside the room every wobble shares.
    #[test]
    fn a_warp_errs_alike_everywhere_and_its_full_travel_fits_the_room() {
        let mut record = Record::new(RATE, 1);
        for speed in [Speed::Lp33, Speed::Single45, Speed::Shellac78] {
            for side in [0.0, 1.0] {
                record.set_controls(RecordControls {
                    disc: DiscControls {
                        speed,
                        ..DiscControls::SILENT
                    },
                    groove: GrooveControls {
                        side,
                        ..GrooveControls::CLEAN
                    },
                    warp: MAX_WARP,
                    ..RecordControls::CLEAN
                });
                let warp = record.wobbles()[WARP_WOBBLE];
                assert_eq!(warp.shape, Shape::Warp);
                assert_eq!(warp.hz, speed.hz());
                assert_eq!(
                    warp.depth,
                    warp::depth(f64::from(MAX_WARP)),
                    "{speed:?}, {side}"
                );
                assert!(warp.reach_seconds() < crate::cassette::WOBBLE_ROOM_S);
            }
        }
    }

    /// **The level dips where the warp's speed is highest**, by [`warp::DIP_DB`] at a Warp of 1,
    /// and not at all where it is lowest; no warp, no dip.
    #[test]
    fn a_warp_dips_the_level_where_its_speed_is_highest() {
        let (fastest, slowest) = (0.161_36, 0.785_13);
        let warped = RecordControls {
            warp: 1.0,
            ..RecordControls::CLEAN
        };
        let dip = tone_db_at(warped, 1_000.0, slowest) - tone_db_at(warped, 1_000.0, fastest);
        assert!((dip - warp::DIP_DB).abs() < 0.01, "{dip} dB");
        let plain = RecordControls::CLEAN;
        let undipped = tone_db_at(warped, 1_000.0, slowest) - tone_db_at(plain, 1_000.0, slowest);
        assert!(undipped.abs() < 1e-4, "{undipped} dB");
        assert_eq!(
            tone_db_at(plain, 1_000.0, fastest),
            tone_db_at(plain, 1_000.0, slowest)
        );
    }

    #[test]
    fn the_disc_sounds_only_through_the_gate() {
        let mut record = Record::new(RATE, 1);
        record.set_controls(RecordControls {
            disc: DiscControls {
                crackle: 500.0,
                surface_db: -40.0,
                ..DiscControls::SILENT
            },
            ..RecordControls::CLEAN
        });
        let closed: f32 = (0..9_600)
            .map(|_| record.process([0.0, 0.0], 0.0, 0.0)[0].abs())
            .sum();
        let open: f32 = (0..9_600)
            .map(|_| record.process([0.0, 0.0], 1.0, 0.0)[0].abs())
            .sum();
        assert_eq!(closed, 0.0);
        assert!(open > 0.0);
    }

    /// **A crack pops; it never thumps**: through the replay curve's bass lift, from an LP on a
    /// modern deck to a 78 on a radiogram, nearly all of a crack's energy is above 100 Hz, where a
    /// bass drum's is below it (the owner, 2026-10-10: "It is the 'cracks' that sounds like bass
    /// drums"). The crack alone, against the same disc without it (the player's own rumble goes on
    /// under it), whatever crack the seed draws.
    #[test]
    fn a_crack_pops_through_the_replay_curve_and_never_thumps() {
        for (speed, era, player) in [
            (Speed::Lp33, Era::Modern, Player::ModernDeck),
            (Speed::Shellac78, Era::Uk78, Player::ModernDeck),
            (Speed::Shellac78, Era::Us78, Player::Radiogram),
        ] {
            for seed in 1..=3 {
                let render = |cracks: u8| -> Vec<f64> {
                    let mut record = Record::new(RATE, seed);
                    record.set_controls(RecordControls {
                        disc: DiscControls {
                            speed,
                            cracks,
                            ..DiscControls::SILENT
                        },
                        era,
                        player,
                        ..RecordControls::INIT
                    });
                    (0..(2.0 * RATE) as usize)
                        .map(|_| f64::from(record.process([0.0, 0.0], 1.0, 0.0)[0]))
                        .collect()
                };
                let without = render(0);
                let out: Vec<f64> = render(1).iter().zip(&without).map(|(a, b)| a - b).collect();
                let peak = (0..out.len())
                    .max_by(|&a, &b| out[a].abs().total_cmp(&out[b].abs()))
                    .unwrap();
                let pop =
                    &out[peak.saturating_sub(240)..(peak + (0.1 * RATE) as usize).min(out.len())];
                let low = Section::new(Kind::LowPass, 100.0, 0.0, FRAC_1_SQRT_2).coefficients(RATE);
                let mut low = [Biquad::new(low), Biquad::new(low)];
                let below: f64 = pop
                    .iter()
                    .map(|v| low.iter_mut().fold(*v, |s, b| b.process(s)).powi(2))
                    .sum();
                let all: f64 = pop.iter().map(|v| v * v).sum();
                assert!(
                    below < 0.1 * all,
                    "{player:?}, seed {seed}: {:.2} of the crack below 100 Hz",
                    below / all
                );
            }
        }
    }

    #[test]
    fn hum_is_its_mains_at_about_its_level() {
        let mut hum = Hum::default();
        let n = 48_000;
        let samples: Vec<f64> = (0..n).map(|_| hum.next(Mains::Fifty, 0.01, RATE)).collect();
        let rms = (samples.iter().map(|v| v * v).sum::<f64>() / n as f64).sqrt();
        assert!((20.0 * rms.log10() + 40.0).abs() < 1.0);
        assert_eq!(hum.next(Mains::Off, 0.01, RATE), 0.0);
    }
}
