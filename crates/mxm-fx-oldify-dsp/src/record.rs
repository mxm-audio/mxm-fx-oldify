//! The record engine: a record side, a disc, a groove and a player.
//!
//! ```text
//! the recording horn → era (mono, the recording's band, the cutting limiter, the cutter, a brick
//!   wall) → cutting curve → groove (dynamic wear, tracing distortion, wear) → + the disc's own
//!   sound → pickup → replay curve → player (scratch filter, valves, speaker, horn or radio)
//!   → the gramophone's horn
//! ```
//!
//! The two horns are [`crate::horn`]'s, set by Horn and Horn size on any record and any player.
//!
//! The record's read-out (eccentricity, the turntable's wobble) is the chain's one moving read,
//! shared with the cassette ([`Record::wobbles`]): a time-warp passes through the curves and the
//! player almost unchanged and through a memoryless curve exactly, so one delay line serves both
//! media, and the latency stays what the cassette alone needs plus the record's reach. Hum is added
//! after the read ([`Hum`]), so it does not wobble.
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
use crate::groove::{Groove, GrooveControls};
use crate::horn::{Horn, HornControls};
use crate::player::{Player, PlayerChain};
use crate::transport::{Shape, Wobble, random_rms_share};
use std::f64::consts::{FRAC_1_SQRT_2, SQRT_2, TAU};

/// How far the record may move the read, in seconds either way: eccentricity's (the measured
/// reference's full eccentricity at its inner groove) and the turntable's share.
pub const ECCENTRIC_REACH_S: f64 = 0.012;
pub const TABLE_REACH_S: f64 = 0.004;
/// The most eccentricity, millimetres: offsets above 1 mm are common on old shellac, the measured
/// reference's full travel is about 2.5 mm on an LP's outer groove, and twice that is for effect.
pub const MAX_ECCENTRICITY_MM: f32 = 5.0;
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
    pub mains: Mains,
    /// RMS re full scale, [`NOISE_OFF_DB`] (off) ..= [`MAX_HUM_DB`].
    pub hum_db: f32,
    /// The acoustic horns the record was made and played through.
    pub horn: HornControls,
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
        mains: Mains::Off,
        hum_db: NOISE_OFF_DB,
        horn: HornControls::OFF,
    };

    /// The init patch's record: a played LP on a modern deck.
    pub const INIT: Self = Self {
        disc: DiscControls {
            speed: Speed::Lp33,
            surface_db: -58.0,
            crackle: 60.0,
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
            hum_db: if self.hum_db.is_finite() {
                self.hum_db.clamp(NOISE_OFF_DB, MAX_HUM_DB)
            } else {
                NOISE_OFF_DB
            },
            horn: self.horn.bounded(),
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

/// A wobble, its depth lowered to fit `reach_s`.
fn fit(mut wobble: Wobble, reach_s: f64) -> Wobble {
    let reach = wobble.reach_seconds();
    if reach > reach_s {
        wobble.depth *= reach_s / reach;
    }
    wobble
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

    #[inline]
    pub fn next(&mut self, mains: Mains, hum_db: f32, rate: f64) -> f64 {
        let hz = match mains {
            Mains::Off => return 0.0,
            Mains::Fifty => 50.0,
            Mains::Sixty => 60.0,
        };
        if hum_db <= NOISE_OFF_DB {
            return 0.0;
        }
        self.phase = (self.phase + hz / rate).fract();
        let p = TAU * self.phase;
        // The fundamental, a strong second, a weaker third; RMS of the sum about the level.
        let shape = p.sin() + 0.6 * (2.0 * p).sin() + 0.3 * (3.0 * p).sin();
        f64::from(crate::db_to_gain(hum_db)) * shape * SQRT_2 / (1.0f64 + 0.36 + 0.09).sqrt()
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
    targets: RecordControls,
}

impl Record {
    /// Allocates the disc's tables: call outside the audio thread.
    pub fn new(sample_rate: f64, seed: u32) -> Self {
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
            targets: RecordControls::CLEAN,
        }
    }

    pub fn reseed(&mut self, seed: u32) {
        self.disc.reseed(seed);
        self.groove.reseed(seed);
    }

    /// Whether the record makes any sound of its own: the disc's, or hum.
    pub fn sounds(&self) -> bool {
        self.disc.sounds()
            || (self.targets.mains != Mains::Off && self.targets.hum_db > NOISE_OFF_DB)
    }

    /// The hum's mains and level, as set.
    pub fn mains(&self) -> (Mains, f32) {
        (self.targets.mains, self.targets.hum_db)
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
    }

    pub fn set_controls(&mut self, c: RecordControls) {
        let c = c.bounded();
        self.targets = c;
        let rate = self.rate;
        if self.era != Some(c.era) {
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
        self.horn.set(c.horn);
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

    /// The record's moving read this sample: eccentricity locked to the turn, and the turntable's
    /// own wobble. Each is held within its reach.
    pub fn wobbles(&self) -> [Wobble; 2] {
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
        [fit(eccentric, ECCENTRIC_REACH_S), fit(table, TABLE_REACH_S)]
    }

    /// One frame through the record. `noise` (0–1) gates the disc's own sound.
    #[inline]
    pub fn process(&mut self, frame: [f32; 2], noise: f32) -> [f32; 2] {
        let c = self.targets;
        let era = c.era.model();
        let horned = self.horn.sounds();
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
        let groove = GrooveControls {
            tip_um: c.player.model().tip_um,
            ..c.groove
        };
        let grooved = self.groove.process([x[0] as f32, x[1] as f32], &groove);
        let own = if noise > 0.0 && self.disc.sounds() {
            self.disc.sample() * f64::from(noise)
        } else {
            0.0
        };
        let picked = self
            .player
            .pickup([f64::from(grooved[0]) + own, f64::from(grooved[1]) + own]);
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
        let mut record = Record::new(RATE, 1);
        record.set_controls(c);
        let n = 48_000;
        let (mut input, mut output) = (0.0, 0.0);
        for i in 0..n {
            let x = (0.05 * (TAU * hz * i as f64 / RATE).sin()) as f32;
            let y = record.process([x, x], 0.0)[0];
            if i > n / 2 {
                input += f64::from(x).powi(2);
                output += f64::from(y).powi(2);
            }
        }
        10.0 * (output / input).log10()
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
    fn eccentricity_reaches_twice_the_reference_and_its_full_travel_fits_the_reach() {
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
        // Half our travel is the reference's: on the inner groove it is not lowered to fit.
        let half = 0.5 * MAX_ECCENTRICITY_MM;
        let inner = at(&mut record, half, 1.0);
        let unlowered = f64::from(half) * 1e-3 / radius_m(Speed::Lp33, 1.0);
        assert!(
            inner > 0.0348 && (inner - unlowered).abs() < 1e-12,
            "{inner}"
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
            .map(|_| record.process([0.0, 0.0], 0.0)[0].abs())
            .sum();
        let open: f32 = (0..9_600)
            .map(|_| record.process([0.0, 0.0], 1.0)[0].abs())
            .sum();
        assert_eq!(closed, 0.0);
        assert!(open > 0.0);
    }

    /// **A crack pops; it never thumps**: through the replay curve's bass lift, from an LP on a
    /// modern deck to a 78 on a radiogram, nearly all of a crack's energy is above 100 Hz, where a
    /// bass drum's is below it (the owner, 2026-10-10: "It is the 'cracks' that sounds like bass
    /// drums").
    #[test]
    fn a_crack_pops_through_the_replay_curve_and_never_thumps() {
        for (speed, era, player) in [
            (Speed::Lp33, Era::Modern, Player::ModernDeck),
            (Speed::Shellac78, Era::Uk78, Player::ModernDeck),
            (Speed::Shellac78, Era::Us78, Player::Radiogram),
        ] {
            let mut record = Record::new(RATE, 3);
            record.set_controls(RecordControls {
                disc: DiscControls {
                    speed,
                    cracks: 1,
                    ..DiscControls::SILENT
                },
                era,
                player,
                ..RecordControls::INIT
            });
            let out: Vec<f64> = (0..(2.0 * RATE) as usize)
                .map(|_| f64::from(record.process([0.0, 0.0], 1.0)[0]))
                .collect();
            let peak = (0..out.len())
                .max_by(|&a, &b| out[a].abs().total_cmp(&out[b].abs()))
                .unwrap();
            let pop = &out[peak.saturating_sub(240)..(peak + (0.1 * RATE) as usize).min(out.len())];
            let low = Section::new(Kind::LowPass, 100.0, 0.0, FRAC_1_SQRT_2).coefficients(RATE);
            let mut low = [Biquad::new(low), Biquad::new(low)];
            let below: f64 = pop
                .iter()
                .map(|v| low.iter_mut().fold(*v, |s, b| b.process(s)).powi(2))
                .sum();
            let all: f64 = pop.iter().map(|v| v * v).sum();
            assert!(
                below < 0.1 * all,
                "{player:?}: {:.2} of the crack below 100 Hz",
                below / all
            );
        }
    }

    #[test]
    fn hum_is_its_mains_at_about_its_level() {
        let mut hum = Hum::default();
        let n = 48_000;
        let samples: Vec<f64> = (0..n)
            .map(|_| hum.next(Mains::Fifty, -40.0, RATE))
            .collect();
        let rms = (samples.iter().map(|v| v * v).sum::<f64>() / n as f64).sqrt();
        assert!((20.0 * rms.log10() + 40.0).abs() < 1.0);
        assert_eq!(hum.next(Mains::Off, -40.0, RATE), 0.0);
    }
}
