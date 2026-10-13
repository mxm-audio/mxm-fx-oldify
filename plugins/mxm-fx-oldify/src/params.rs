//! Permanent parameter definitions, in signal order, and their mapping onto the DSP's controls.
//!
//! **Nothing here carries a nice-plug smoother.** Every continuous control is smoothed once, in
//! `mxm-fx-oldify-dsp`, which ramps each block's targets over its `SMOOTHING_S`; a second smoother
//! here would stack two ramps on one control. The switches are not smoothed anywhere: the DSP takes
//! a new tape, grade, character, shape or count as it comes.
//!
//! **Wow rate, Flutter rate and Stop time sync to the host's tempo** (mxm-kit's plugin conventions,
//! *Tempo sync is one button changing one Rate or Time control*): each has one *sync* switch, and
//! synced, the control's own position picks a division on its [`mxm_tempo::Ladder`]. [`MxmFxOldifyParams::controls`]
//! resolves it from the tempo it is handed, every block, so nothing resolved is kept to go stale
//! across an activation or a reset.

use std::sync::{Arc, RwLock};

use mxm_fx_oldify_dsp::cassette::{
    CROSSTALK_MAX_DB, CROSSTALK_OFF_DB, CassetteControls, FLUTTER_HZ, HISS_MAX_DB, HISS_OFF_DB,
    MAX_AZIMUTH_ARCMIN, MAX_GENERATIONS, MAX_STOP_S, MAX_VARIATION, MIN_STOP_S, MOTION_MAX, Motion,
    WOW_HZ,
};
use mxm_fx_oldify_dsp::curves::Curve;
use mxm_fx_oldify_dsp::disc::{
    CRACKLE_LEVEL_DB, DiscControls, MAX_CLICKS_HZ, MAX_CRACKLE_HZ, MAX_CRACKS, MAX_RUMBLE_DB,
    MAX_SCRATCHES, MAX_SURFACE_DB, NOISE_OFF_DB, Speed,
};
use mxm_fx_oldify_dsp::dropouts::DropoutControls;
use mxm_fx_oldify_dsp::echo::MAX_ECHO_DB;
use mxm_fx_oldify_dsp::groove::GrooveControls;
use mxm_fx_oldify_dsp::horn::{HORN_LONGEST_M, HORN_SHORTEST_M, HornControls};
use mxm_fx_oldify_dsp::nr::{MAX_LEVEL_ERROR_DB, NoiseReduction, NrControls};
use mxm_fx_oldify_dsp::player::Player;
use mxm_fx_oldify_dsp::print::{MAX_PRINT_DB, PRINT_OFF_DB};
use mxm_fx_oldify_dsp::record::{Era, MAX_ECCENTRICITY_MM, MAX_WARP, Mains, RecordControls};
use mxm_fx_oldify_dsp::saturate::Character;
use mxm_fx_oldify_dsp::skip::MAX_SKIPS;
use mxm_fx_oldify_dsp::tape::{Grade, Tape};
use mxm_fx_oldify_dsp::transport::Shape;
use mxm_fx_oldify_dsp::{Controls, MAX_GAIN_DB, MAX_WIDTH, MIN_GAIN_DB, Quality, db_to_gain};
use mxm_preset::PresetIdentity;
use mxm_tempo::{Direction, Division, Ladder, Span};
use nice_plug::prelude::*;

/// **Wow rate's tempo sync**: the divisions its own range holds at 120 bpm, 1/8 (4 Hz) to four bars
/// (0.125 Hz), the top the fastest.
pub const WOW_SYNC: Ladder = Ladder::new(
    Span::new(Division::Eighth, Division::FourBars),
    Direction::Rate,
);

/// **Flutter rate's tempo sync**: the divisions its own range holds at 120 bpm, 1/32T (24 Hz) to
/// 1/8 (4 Hz), the top the fastest.
pub const FLUTTER_SYNC: Ladder = Ladder::new(
    Span::new(Division::ThirtySecondTriplet, Division::Eighth),
    Direction::Rate,
);

/// **Stop time's tempo sync**: the divisions its own range holds at 120 bpm, 1/32 (62.5 ms) to four
/// bars (8 s), the top the longest.
pub const STOP_SYNC: Ladder = Ladder::new(
    Span::new(Division::ThirtySecond, Division::FourBars),
    Direction::Time,
);

/// Every permanent id this plugin has retired. **None may ever be re-used**: a saved project or
/// preset naming one is skipped for it, and a new parameter under an old name would be handed a value
/// that meant something else.
///
/// `medium`, the Cassette / Record / Record then cassette selector, gave way to the Record and Tape
/// switches (the owner, 2026-10-09: "There is not way to know what cards control the tape, and what
/// controls the vinyl. So the tape/vinyl/tape+vinyl makes no sense."). `output`, an output level,
/// went because the collection's effects end in Mix alone (the owner, 2026-10-09: "it still has
/// both output and mix").
pub const RETIRED_IDS: [&str; 2] = ["medium", "output"];

/// The disc's speed, and so its size and its noise's pace.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscChoice {
    #[id = "lp-33"]
    #[name = "LP 33⅓"]
    Lp33,
    #[id = "single-45"]
    #[name = "Single 45"]
    Single45,
    #[id = "shellac-78"]
    #[name = "Shellac 78"]
    Shellac78,
}

impl DiscChoice {
    pub const fn dsp(self) -> Speed {
        match self {
            Self::Lp33 => Speed::Lp33,
            Self::Single45 => Speed::Single45,
            Self::Shellac78 => Speed::Shellac78,
        }
    }
}

/// When the record was cut.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EraChoice {
    #[id = "acoustic"]
    #[name = "Acoustic"]
    Acoustic,
    #[id = "early-electrical"]
    #[name = "Early electrical"]
    EarlyElectrical,
    #[id = "us-78"]
    #[name = "US 78, 1940s"]
    Us78,
    #[id = "uk-78"]
    #[name = "UK 78, 1940s"]
    Uk78,
    #[id = "late-78"]
    #[name = "Late 78"]
    Late78,
    #[id = "early-single"]
    #[name = "Early single"]
    EarlySingle,
    #[id = "early-lp"]
    #[name = "Early LP"]
    EarlyLp,
    #[id = "mono-lp"]
    #[name = "Mono LP"]
    MonoLp,
    #[id = "modern"]
    #[name = "Modern"]
    Modern,
}

impl EraChoice {
    pub const fn dsp(self) -> Era {
        match self {
            Self::Acoustic => Era::Acoustic,
            Self::EarlyElectrical => Era::EarlyElectrical,
            Self::Us78 => Era::Us78,
            Self::Uk78 => Era::Uk78,
            Self::Late78 => Era::Late78,
            Self::EarlySingle => Era::EarlySingle,
            Self::EarlyLp => Era::EarlyLp,
            Self::MonoLp => Era::MonoLp,
            Self::Modern => Era::Modern,
        }
    }
}

/// What the record is played on.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerChoice {
    #[id = "wind-up"]
    #[name = "Wind-up gramophone"]
    WindUp,
    #[id = "radiogram"]
    #[name = "Radiogram"]
    Radiogram,
    #[id = "jukebox"]
    #[name = "Jukebox"]
    Jukebox,
    #[id = "portable"]
    #[name = "Portable"]
    Portable,
    #[id = "mono-hi-fi"]
    #[name = "Mono hi-fi"]
    MonoHiFi,
    #[id = "on-the-radio"]
    #[name = "On the radio"]
    OnTheRadio,
    #[id = "modern-deck"]
    #[name = "Modern deck"]
    ModernDeck,
}

impl PlayerChoice {
    pub const fn dsp(self) -> Player {
        match self {
            Self::WindUp => Player::WindUp,
            Self::Radiogram => Player::Radiogram,
            Self::Jukebox => Player::Jukebox,
            Self::Portable => Player::Portable,
            Self::MonoHiFi => Player::MonoHiFi,
            Self::OnTheRadio => Player::OnTheRadio,
            Self::ModernDeck => Player::ModernDeck,
        }
    }
}

/// The curve a record was cut with: the era's, or one chosen.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutCurveChoice {
    #[id = "follow-era"]
    #[name = "Follow era"]
    FollowEra,
    #[id = "flat"]
    #[name = "Flat"]
    Flat,
    #[id = "uk-78"]
    #[name = "UK 78"]
    Uk78,
    #[id = "uk-78-with-treble-lift"]
    #[name = "UK 78 with treble lift"]
    Uk78Treble,
    #[id = "us-78"]
    #[name = "US 78"]
    Us78,
    #[id = "early-lp"]
    #[name = "Early LP"]
    EarlyLp,
    #[id = "early-45"]
    #[name = "Early 45"]
    Early45,
    #[id = "transcription"]
    #[name = "Transcription"]
    Transcription,
    #[id = "west-coast-lp"]
    #[name = "West Coast LP"]
    WestCoastLp,
    #[id = "standard"]
    #[name = "Standard"]
    Standard,
}

impl CutCurveChoice {
    /// `None` follows the era.
    pub const fn dsp(self) -> Option<Curve> {
        match self {
            Self::FollowEra => None,
            Self::Flat => Some(Curve::Flat),
            Self::Uk78 => Some(Curve::Uk78),
            Self::Uk78Treble => Some(Curve::Uk78Treble),
            Self::Us78 => Some(Curve::Us78),
            Self::EarlyLp => Some(Curve::EarlyLp),
            Self::Early45 => Some(Curve::Early45),
            Self::Transcription => Some(Curve::Transcription),
            Self::WestCoastLp => Some(Curve::WestCoastLp),
            Self::Standard => Some(Curve::Standard),
        }
    }
}

/// The curve a record is played back with: the player's, or one chosen.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayCurveChoice {
    #[id = "follow-player"]
    #[name = "Follow player"]
    FollowPlayer,
    #[id = "flat"]
    #[name = "Flat"]
    Flat,
    #[id = "uk-78"]
    #[name = "UK 78"]
    Uk78,
    #[id = "uk-78-with-treble-lift"]
    #[name = "UK 78 with treble lift"]
    Uk78Treble,
    #[id = "us-78"]
    #[name = "US 78"]
    Us78,
    #[id = "early-lp"]
    #[name = "Early LP"]
    EarlyLp,
    #[id = "early-45"]
    #[name = "Early 45"]
    Early45,
    #[id = "transcription"]
    #[name = "Transcription"]
    Transcription,
    #[id = "west-coast-lp"]
    #[name = "West Coast LP"]
    WestCoastLp,
    #[id = "standard"]
    #[name = "Standard"]
    Standard,
}

impl ReplayCurveChoice {
    /// `None` follows the player.
    pub const fn dsp(self) -> Option<Curve> {
        match self {
            Self::FollowPlayer => None,
            Self::Flat => Some(Curve::Flat),
            Self::Uk78 => Some(Curve::Uk78),
            Self::Uk78Treble => Some(Curve::Uk78Treble),
            Self::Us78 => Some(Curve::Us78),
            Self::EarlyLp => Some(Curve::EarlyLp),
            Self::Early45 => Some(Curve::Early45),
            Self::Transcription => Some(Curve::Transcription),
            Self::WestCoastLp => Some(Curve::WestCoastLp),
            Self::Standard => Some(Curve::Standard),
        }
    }
}

/// Mains hum's frequency.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumChoice {
    #[id = "off"]
    #[name = "Off"]
    Off,
    #[id = "50-hz"]
    #[name = "50 Hz"]
    Fifty,
    #[id = "60-hz"]
    #[name = "60 Hz"]
    Sixty,
}

impl HumChoice {
    pub const fn dsp(self) -> Mains {
        match self {
            Self::Off => Mains::Off,
            Self::Fifty => Mains::Fifty,
            Self::Sixty => Mains::Sixty,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapeChoice {
    #[id = "type-1"]
    #[name = "Type I"]
    TypeI,
    #[id = "type-2"]
    #[name = "Type II"]
    TypeII,
    #[id = "type-4"]
    #[name = "Type IV"]
    TypeIV,
}

impl TapeChoice {
    pub const fn dsp(self) -> Tape {
        match self {
            Self::TypeI => Tape::I,
            Self::TypeII => Tape::II,
            Self::TypeIV => Tape::IV,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradeChoice {
    #[id = "economy"]
    #[name = "Economy"]
    Economy,
    #[id = "standard"]
    #[name = "Standard"]
    Standard,
    #[id = "high"]
    #[name = "High"]
    High,
    #[id = "reference"]
    #[name = "Reference"]
    Reference,
}

impl GradeChoice {
    pub const fn dsp(self) -> Grade {
        match self {
            Self::Economy => Grade::Economy,
            Self::Standard => Grade::Standard,
            Self::High => Grade::High,
            Self::Reference => Grade::Reference,
        }
    }
}

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharacterChoice {
    #[id = "soft"]
    #[name = "Soft"]
    Soft,
    #[id = "hard"]
    #[name = "Hard"]
    Hard,
}

/// How hard the effect works.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityChoice {
    #[id = "normal"]
    #[name = "Normal"]
    Normal,
    #[id = "high"]
    #[name = "High"]
    High,
}

impl QualityChoice {
    pub const fn dsp(self) -> Quality {
        match self {
            Self::Normal => Quality::Normal,
            Self::High => Quality::High,
        }
    }
}

/// The noise reduction's system.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseReductionChoice {
    #[id = "off"]
    #[name = "Off"]
    Off,
    #[id = "single-band"]
    #[name = "Single-band"]
    SingleBand,
    #[id = "two-band"]
    #[name = "Two-band"]
    TwoBand,
}

impl NoiseReductionChoice {
    pub const fn dsp(self) -> NoiseReduction {
        match self {
            Self::Off => NoiseReduction::Off,
            Self::SingleBand => NoiseReduction::SingleBand,
            Self::TwoBand => NoiseReduction::TwoBand,
        }
    }
}

impl CharacterChoice {
    pub const fn dsp(self) -> Character {
        match self {
            Self::Soft => Character::Soft,
            Self::Hard => Character::Hard,
        }
    }
}

/// Wow's shapes.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WowShapeChoice {
    #[id = "sine"]
    #[name = "Sine"]
    Sine,
    #[id = "random"]
    #[name = "Random"]
    Random,
    #[id = "saw"]
    #[name = "Saw"]
    Saw,
    #[id = "reverse-saw"]
    #[name = "Reverse saw"]
    ReverseSaw,
}

impl WowShapeChoice {
    pub const fn dsp(self) -> Shape {
        match self {
            Self::Sine => Shape::Sine,
            Self::Random => Shape::Random,
            Self::Saw => Shape::Saw,
            Self::ReverseSaw => Shape::ReverseSaw,
        }
    }
}

/// Flutter's shapes: a capstan's tremble is steady or irregular, never a ramp.
#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlutterShapeChoice {
    #[id = "sine"]
    #[name = "Sine"]
    Sine,
    #[id = "random"]
    #[name = "Random"]
    Random,
}

impl FlutterShapeChoice {
    pub const fn dsp(self) -> Shape {
        match self {
            Self::Sine => Shape::Sine,
            Self::Random => Shape::Random,
        }
    }
}

type ValueToString = Arc<dyn Fn(f32) -> String + Send + Sync>;
type StringToValue = Arc<dyn Fn(&str) -> Option<f32> + Send + Sync>;

fn percent_to_string() -> ValueToString {
    Arc::new(|value| format!("{:.0} %", value * 100.0))
}

fn string_to_percent() -> StringToValue {
    Arc::new(|text| {
        text.trim()
            .trim_end_matches('%')
            .trim()
            .parse::<f32>()
            .ok()
            .map(|value| value / 100.0)
    })
}

/// A rate in hertz: to a thousandth under 1 Hz, a hundredth from it. The precision is chosen from
/// the reading rounded to the finer of the two, so the host's text round trip is idempotent at the
/// switch: 0.9946 reads 0.995 Hz and reads back the same.
fn rate_to_string() -> ValueToString {
    Arc::new(|value| {
        if (value * 1_000.0).round() >= 1_000.0 {
            format!("{value:.2} Hz")
        } else {
            format!("{value:.3} Hz")
        }
    })
}

fn string_to_rate() -> StringToValue {
    Arc::new(|text| {
        text.trim()
            .to_ascii_lowercase()
            .trim_end_matches("hz")
            .trim()
            .parse::<f32>()
            .ok()
    })
}

/// A time: milliseconds under a second, seconds from it. The unit is chosen from the rounded
/// reading, so the host's text round trip is idempotent at the switch (`time_text_checks`).
fn time_to_string() -> ValueToString {
    Arc::new(|seconds| {
        let milliseconds = seconds * 1_000.0;
        if milliseconds.round() >= 1_000.0 {
            format!("{seconds:.2} s")
        } else {
            format!("{milliseconds:.0} ms")
        }
    })
}

fn string_to_time() -> StringToValue {
    Arc::new(|text| {
        let lower = text.trim().to_ascii_lowercase();
        if let Some(value) = lower.strip_suffix("ms") {
            value.trim().parse::<f32>().ok().map(|v| v / 1_000.0)
        } else {
            lower.trim_end_matches('s').trim().parse::<f32>().ok()
        }
    })
}

/// A chance in percent: to a hundredth under one, a tenth under ten, whole from it, so a chance
/// that is not nothing never reads as nothing. The precision is chosen from the rounded reading, so
/// the host's text round trip is idempotent at each switch.
fn chance_to_string() -> ValueToString {
    Arc::new(|value| {
        let percent = value * 100.0;
        if (percent * 100.0).round() < 100.0 {
            format!("{percent:.2} %")
        } else if (percent * 10.0).round() < 100.0 {
            format!("{percent:.1} %")
        } else {
            format!("{percent:.0} %")
        }
    })
}

/// An angle in arc-minutes, read with the minute's own sign.
fn arcmin_to_string() -> ValueToString {
    Arc::new(|value| format!("{value:.1}′"))
}

/// Takes `15′`, `15'`, `15 arcmin` or a bare `15`.
fn string_to_arcmin() -> StringToValue {
    Arc::new(|text| {
        text.trim()
            .to_ascii_lowercase()
            .trim_end_matches("arcmin")
            .trim_end_matches(['′', '\''])
            .trim()
            .parse::<f32>()
            .ok()
    })
}

/// **A level with an Off at the bottom of its travel, as the DSP takes it, from the stored gain**:
/// `off_db` — off — wherever the reading rounds to it, so the knob never reads Off while the level
/// plays; otherwise its decibels to a hundredth, so a default in whole decibels reaches the DSP as
/// that number exactly.
fn level_db(gain: f32, off_db: f32, max_db: f32) -> f32 {
    let db = 20.0 * gain.max(f32::MIN_POSITIVE).log10();
    if (db * 10.0).round() / 10.0 <= off_db {
        off_db
    } else {
        ((db * 100.0).round() / 100.0).min(max_db)
    }
}

/// Hiss's level for the DSP ([`level_db`]).
pub fn hiss_db(gain: f32) -> f32 {
    level_db(gain, HISS_OFF_DB, HISS_MAX_DB)
}

/// Crosstalk's level for the DSP ([`level_db`]).
pub fn crosstalk_db(gain: f32) -> f32 {
    level_db(gain, CROSSTALK_OFF_DB, CROSSTALK_MAX_DB)
}

/// Print-through's level for the DSP ([`level_db`]).
pub fn print_db(gain: f32) -> f32 {
    level_db(gain, PRINT_OFF_DB, MAX_PRINT_DB)
}

/// A level from `off_db` (read "Off") to `max_db`, **stored as linear gain** (the collection's
/// rule for a level), its decibels spread evenly across the travel.
fn off_level(name: &'static str, default_db: f32, off_db: f32, max_db: f32) -> FloatParam {
    FloatParam::new(
        name,
        db_to_gain(default_db),
        FloatRange::Skewed {
            min: db_to_gain(off_db),
            max: db_to_gain(max_db),
            factor: FloatRange::gain_skew_factor(off_db, max_db),
        },
    )
    .with_value_to_string(Arc::new(move |gain| {
        let db = level_db(gain, off_db, max_db);
        if db <= off_db {
            "Off".to_owned()
        } else {
            format!("{db:.1} dB")
        }
    }))
    .with_string_to_value(Arc::new(move |text| {
        let text = text.trim();
        if text.eq_ignore_ascii_case("off") {
            return Some(db_to_gain(off_db));
        }
        text.trim_end_matches([' ', 'd', 'D', 'b', 'B'])
            .parse::<f32>()
            .ok()
            .map(|db| db_to_gain(db.clamp(off_db, max_db)))
    }))
}

/// A gain across ±24 dB, **stored as linear gain** (the collection's rule for gain parameters),
/// skewed so 0 dB sits at the middle of the travel and read in decibels.
fn gain(name: &'static str) -> FloatParam {
    gain_within(name, 1.0, MIN_GAIN_DB, MAX_GAIN_DB)
}

/// **NR level error as the DSP takes it, from the stored gain**: its decibels to a hundredth, so
/// 0 dB reaches the DSP as exactly zero.
pub fn level_error_db(gain: f32) -> f32 {
    let db = 20.0 * gain.max(f32::MIN_POSITIVE).log10();
    ((db * 100.0).round() / 100.0).clamp(-MAX_LEVEL_ERROR_DB, MAX_LEVEL_ERROR_DB)
}

/// A gain from `min_db` to `max_db`, stored as linear gain, its decibels spread evenly across the
/// travel and read in dB.
fn gain_within(name: &'static str, default: f32, min_db: f32, max_db: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: db_to_gain(min_db),
            max: db_to_gain(max_db),
            factor: FloatRange::gain_skew_factor(min_db, max_db),
        },
    )
    .with_unit(" dB")
    .with_value_to_string(formatters::v2s_f32_gain_to_db(1))
    .with_string_to_value(formatters::s2v_f32_gain_to_db())
}

fn percent(name: &'static str, default: f32, max: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max })
        .with_value_to_string(percent_to_string())
        .with_string_to_value(string_to_percent())
}

/// The skew that puts a range's geometric mean at the middle of the travel: a rate's octaves are
/// spread evenly enough that neither end crowds the knob.
fn geometric_skew((min, max): (f32, f32)) -> f32 {
    let middle = (min * max).sqrt();
    0.5f32.ln() / ((middle - min) / (max - min)).ln()
}

fn rate(name: &'static str, default: f32, range: (f32, f32)) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: range.0,
            max: range.1,
            factor: geometric_skew(range),
        },
    )
    .with_value_to_string(rate_to_string())
    .with_string_to_value(string_to_rate())
}

/// The loudest hum, RMS re full scale: the DSP's own bound.
pub use mxm_fx_oldify_dsp::record::MAX_HUM_DB;

/// Events a second. The precision is chosen from the rounded reading, so the host's text round
/// trip is idempotent at the switch.
fn per_second_to_string() -> ValueToString {
    Arc::new(|value| {
        if (value * 10.0).round() >= 100.0 {
            format!("{value:.0}/s")
        } else {
            format!("{value:.1}/s")
        }
    })
}

fn string_to_per_second() -> StringToValue {
    Arc::new(|text| {
        text.trim()
            .to_ascii_lowercase()
            .trim_end_matches("a second")
            .trim_end_matches("/s")
            .trim()
            .parse::<f32>()
            .ok()
    })
}

/// A count of events a second from zero to `max`, skewed by `skew` so the low counts, where the
/// ear hears each one, get the travel.
fn per_second(name: &'static str, default: f32, max: f32, skew: f32) -> FloatParam {
    FloatParam::new(
        name,
        default,
        FloatRange::Skewed {
            min: 0.0,
            max,
            factor: FloatRange::skew_factor(skew),
        },
    )
    .with_value_to_string(per_second_to_string())
    .with_string_to_value(string_to_per_second())
}

/// Surface noise's level for the DSP ([`level_db`]).
pub fn surface_db(gain: f32) -> f32 {
    level_db(gain, NOISE_OFF_DB, MAX_SURFACE_DB)
}

/// Rumble's level for the DSP ([`level_db`]).
pub fn rumble_db(gain: f32) -> f32 {
    level_db(gain, NOISE_OFF_DB, MAX_RUMBLE_DB)
}

/// Hum's level for the DSP ([`level_db`]).
pub fn hum_db(gain: f32) -> f32 {
    level_db(gain, NOISE_OFF_DB, MAX_HUM_DB)
}

/// The groove echo's level for the DSP ([`level_db`]).
pub fn echo_db(gain: f32) -> f32 {
    level_db(gain, NOISE_OFF_DB, MAX_ECHO_DB)
}

/// A tempo sync's switch: On/Off, as every sync in the collection reads.
fn sync(name: &'static str) -> BoolParam {
    BoolParam::new(name, false)
}

#[derive(Params)]
pub struct MxmFxOldifyParams {
    // ---- Record ----
    /// The record's switch: off, its cards are dimmed and it is out of the chain.
    #[id = "record_on"]
    pub record_on: BoolParam,
    #[id = "disc"]
    pub disc: EnumParam<DiscChoice>,
    #[id = "era"]
    pub era: EnumParam<EraChoice>,
    #[id = "player"]
    pub player: EnumParam<PlayerChoice>,
    #[id = "cut_curve"]
    pub cut_curve: EnumParam<CutCurveChoice>,
    #[id = "replay_curve"]
    pub replay_curve: EnumParam<ReplayCurveChoice>,
    #[id = "horn"]
    pub horn: FloatParam,
    /// The horn's length, metres.
    #[id = "horn_size"]
    pub horn_size: FloatParam,

    // ---- Groove ----
    #[id = "wear"]
    pub wear: FloatParam,
    #[id = "dynamic_wear"]
    pub dynamic_wear: FloatParam,
    #[id = "side"]
    pub side: FloatParam,
    #[id = "groove_distortion"]
    pub groove_distortion: FloatParam,
    #[id = "eccentricity"]
    pub eccentricity: FloatParam,
    #[id = "warp"]
    pub warp: FloatParam,
    /// The chance a turn that the stylus jumps back a groove.
    #[id = "skips"]
    pub skips: FloatParam,
    /// Held, the stylus goes round one groove.
    #[id = "stuck_groove"]
    pub stuck_groove: BoolParam,

    // ---- Surface ----
    #[id = "crackle"]
    pub crackle: FloatParam,
    /// Stored as linear gain, 0 dB mid-travel: the crackle's size against a typical record's.
    #[id = "crackle_level"]
    pub crackle_level: FloatParam,
    #[id = "clicks"]
    pub clicks: FloatParam,
    /// Stored as linear gain; the bottom of the travel is Off.
    #[id = "surface_noise"]
    pub surface_noise: FloatParam,
    #[id = "scratches"]
    pub scratches: IntParam,
    #[id = "cracks"]
    pub cracks: IntParam,
    /// Stored as linear gain; the bottom of the travel is Off.
    #[id = "rumble"]
    pub rumble: FloatParam,
    #[id = "hum"]
    pub hum: EnumParam<HumChoice>,
    /// Stored as linear gain; the bottom of the travel is Off.
    #[id = "hum_level"]
    pub hum_level: FloatParam,
    /// Stored as linear gain; the bottom of the travel is Off.
    #[id = "groove_echo"]
    pub groove_echo: FloatParam,

    // ---- Tape ----
    /// The tape's switch: off, its cards are dimmed and it is out of the chain.
    #[id = "tape_on"]
    pub tape_on: BoolParam,
    #[id = "tape"]
    pub tape: EnumParam<TapeChoice>,
    #[id = "grade"]
    pub grade: EnumParam<GradeChoice>,
    #[id = "tape_tone"]
    pub tape_tone: FloatParam,
    #[id = "saturation"]
    pub saturation: FloatParam,
    #[id = "character"]
    pub character: EnumParam<CharacterChoice>,
    #[id = "generations"]
    pub generations: IntParam,
    #[id = "age"]
    pub age: FloatParam,

    // ---- Transport ----
    #[id = "wow"]
    pub wow: FloatParam,
    #[id = "wow_rate"]
    pub wow_rate: FloatParam,
    /// Wow rate's tempo sync: its position picks a division of the host's tempo.
    #[id = "wow_sync"]
    pub wow_sync: BoolParam,
    #[id = "wow_shape"]
    pub wow_shape: EnumParam<WowShapeChoice>,
    #[id = "flutter"]
    pub flutter: FloatParam,
    #[id = "flutter_rate"]
    pub flutter_rate: FloatParam,
    /// Flutter rate's tempo sync.
    #[id = "flutter_sync"]
    pub flutter_sync: BoolParam,
    #[id = "flutter_shape"]
    pub flutter_shape: EnumParam<FlutterShapeChoice>,
    #[id = "follow"]
    pub follow: BoolParam,
    #[id = "flange"]
    pub flange: FloatParam,
    /// Held, the tape slows to a halt and stays stopped; let go, it starts again.
    #[id = "tape_stop"]
    pub tape_stop: BoolParam,
    /// Seconds from full speed to a halt.
    #[id = "stop_time"]
    pub stop_time: FloatParam,
    /// Stop time's tempo sync.
    #[id = "stop_sync"]
    pub stop_sync: BoolParam,

    // ---- Noise ----
    /// Stored as linear gain; the bottom of the travel is Off.
    #[id = "hiss"]
    pub hiss: FloatParam,
    /// Stored as linear gain; the bottom of the travel is Off.
    #[id = "print_through"]
    pub print_through: FloatParam,
    #[id = "nr"]
    pub nr: EnumParam<NoiseReductionChoice>,
    #[id = "nr_decode"]
    pub nr_decode: BoolParam,
    #[id = "nr_amount"]
    pub nr_amount: FloatParam,
    #[id = "nr_brightness"]
    pub nr_brightness: FloatParam,
    /// Stored as linear gain, read in dB.
    #[id = "nr_level_error"]
    pub nr_level_error: FloatParam,
    #[id = "dropouts"]
    pub dropouts: FloatParam,
    #[id = "dropout_intensity"]
    pub dropout_intensity: FloatParam,
    #[id = "dropout_width"]
    pub dropout_width: FloatParam,

    // ---- Deck ----
    #[id = "azimuth"]
    pub azimuth: FloatParam,
    /// Stored as linear gain; the bottom of the travel is Off.
    #[id = "crosstalk"]
    pub crosstalk: FloatParam,
    #[id = "wrong_eq"]
    pub wrong_eq: BoolParam,

    // ---- Output ----
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "width"]
    pub width: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "noise_follows"]
    pub noise_follows: FloatParam,
    #[id = "noise_stops"]
    pub noise_stops: BoolParam,
    #[id = "variation"]
    pub variation: IntParam,
    /// High: the tape's saturation at four times the rate, the moving read by a windowed sinc.
    #[id = "quality"]
    pub quality: EnumParam<QualityChoice>,

    #[persist = "preset"]
    pub preset: RwLock<PresetIdentity>,
}

impl Default for MxmFxOldifyParams {
    /// **The defaults are the DSP's init patch** (`Controls::INIT`), read from it rather than
    /// copied, so the two cannot drift apart; the switches' defaults are written as choices, and
    /// `the_defaults_are_the_dsp_init_patch` holds them.
    fn default() -> Self {
        let init = Controls::INIT;
        let cassette = init.cassette;
        let record = init.record;
        Self {
            record_on: BoolParam::new("Vinyl", init.record_on),
            disc: EnumParam::new("Disc", DiscChoice::Lp33),
            era: EnumParam::new("Era", EraChoice::Modern),
            player: EnumParam::new("Player", PlayerChoice::ModernDeck),
            cut_curve: EnumParam::new("Cut curve", CutCurveChoice::FollowEra),
            replay_curve: EnumParam::new("Replay curve", ReplayCurveChoice::FollowPlayer),

            horn: percent("Horn", record.horn.amount, 1.0),
            horn_size: FloatParam::new(
                "Horn size",
                record.horn.length_m,
                FloatRange::Linear {
                    min: HORN_SHORTEST_M,
                    max: HORN_LONGEST_M,
                },
            )
            .with_unit(" m")
            .with_value_to_string(formatters::v2s_f32_rounded(2))
            .with_string_to_value(Arc::new(|text| {
                text.trim()
                    .to_ascii_lowercase()
                    .trim_end_matches('m')
                    .trim()
                    .parse::<f32>()
                    .ok()
            })),

            wear: percent("Wear", record.groove.wear, 1.0),
            dynamic_wear: percent("Dynamic wear", record.groove.dynamic, 1.0),
            side: percent("Side position", record.groove.side, 1.0),
            groove_distortion: percent("Groove distortion", record.groove.distortion, 2.0),
            // Skewed so the everyday offsets, a few tenths of a millimetre, get the first half.
            eccentricity: FloatParam::new(
                "Eccentricity",
                record.eccentricity_mm,
                FloatRange::Skewed {
                    min: 0.0,
                    max: MAX_ECCENTRICITY_MM,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" mm")
            .with_value_to_string(formatters::v2s_f32_rounded(2))
            .with_string_to_value(Arc::new(|text| {
                text.trim()
                    .to_ascii_lowercase()
                    .trim_end_matches("mm")
                    .trim()
                    .parse::<f32>()
                    .ok()
            })),
            // Skewed as Eccentricity, so the warps real records have, a fraction of the
            // reference's, get the first half of the travel.
            warp: FloatParam::new(
                "Warp",
                record.warp,
                FloatRange::Skewed {
                    min: 0.0,
                    max: MAX_WARP,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_value_to_string(percent_to_string())
            .with_string_to_value(string_to_percent()),
            // Skewed so a real record's few in a hundred turns get the first half of the travel.
            skips: FloatParam::new(
                "Skips",
                record.skips,
                FloatRange::Skewed {
                    min: 0.0,
                    max: MAX_SKIPS,
                    factor: 0.5f32.ln() / (0.05 / MAX_SKIPS).ln(),
                },
            )
            .with_value_to_string(chance_to_string())
            .with_string_to_value(string_to_percent()),
            stuck_groove: BoolParam::new("Stuck groove", record.stuck),

            crackle: per_second("Crackle", record.disc.crackle, MAX_CRACKLE_HZ, -2.0),
            crackle_level: gain_within(
                "Crackle level",
                record.disc.crackle_gain,
                -CRACKLE_LEVEL_DB,
                CRACKLE_LEVEL_DB,
            ),
            clicks: per_second("Clicks", record.disc.clicks, MAX_CLICKS_HZ, -1.5),
            surface_noise: off_level(
                "Surface noise",
                record.disc.surface_db,
                NOISE_OFF_DB,
                MAX_SURFACE_DB,
            ),
            scratches: IntParam::new(
                "Scratches",
                i32::from(record.disc.scratches),
                IntRange::Linear {
                    min: 0,
                    max: MAX_SCRATCHES as i32,
                },
            ),
            cracks: IntParam::new(
                "Cracks",
                i32::from(record.disc.cracks),
                IntRange::Linear {
                    min: 0,
                    max: MAX_CRACKS as i32,
                },
            ),
            rumble: off_level("Rumble", record.disc.rumble_db, NOISE_OFF_DB, MAX_RUMBLE_DB),
            hum: EnumParam::new("Hum", HumChoice::Off),
            hum_level: off_level("Hum level", record.hum_db, NOISE_OFF_DB, MAX_HUM_DB),
            groove_echo: off_level("Groove echo", record.echo_db, NOISE_OFF_DB, MAX_ECHO_DB),

            tape_on: BoolParam::new("Tape", init.tape_on),
            tape: EnumParam::new("Tape type", TapeChoice::TypeI),
            grade: EnumParam::new("Grade", GradeChoice::Standard),
            tape_tone: percent("Tape tone", cassette.tone, 1.0),
            saturation: percent("Saturation", cassette.saturation, 1.0),
            character: EnumParam::new("Character", CharacterChoice::Soft),
            generations: IntParam::new(
                "Generations",
                i32::from(cassette.generations),
                IntRange::Linear {
                    min: 1,
                    max: i32::from(MAX_GENERATIONS),
                },
            ),
            age: percent("Age", cassette.age, 1.0),

            wow: percent("Wow", cassette.wow.depth, MOTION_MAX),
            wow_rate: rate("Wow rate", cassette.wow.hz, WOW_HZ),
            wow_sync: sync("Wow sync"),
            wow_shape: EnumParam::new("Wow shape", WowShapeChoice::Random),
            flutter: percent("Flutter", cassette.flutter.depth, MOTION_MAX),
            flutter_rate: rate("Flutter rate", cassette.flutter.hz, FLUTTER_HZ),
            flutter_sync: sync("Flutter sync"),
            flutter_shape: EnumParam::new("Flutter shape", FlutterShapeChoice::Random),
            follow: BoolParam::new("Flutter follows wow", cassette.follow),
            flange: percent("Flange", cassette.flange, 1.0),
            tape_stop: BoolParam::new("Tape stop", cassette.stop),
            stop_time: FloatParam::new(
                "Stop time",
                cassette.stop_s,
                FloatRange::Skewed {
                    min: MIN_STOP_S,
                    max: MAX_STOP_S,
                    factor: geometric_skew((MIN_STOP_S, MAX_STOP_S)),
                },
            )
            .with_value_to_string(time_to_string())
            .with_string_to_value(string_to_time()),
            stop_sync: sync("Stop sync"),

            hiss: off_level("Hiss", cassette.hiss_db, HISS_OFF_DB, HISS_MAX_DB),
            print_through: off_level(
                "Print-through",
                cassette.print_db,
                PRINT_OFF_DB,
                MAX_PRINT_DB,
            ),
            nr: EnumParam::new("Noise reduction", NoiseReductionChoice::Off),
            nr_decode: BoolParam::new("Decode", cassette.nr.decode),
            nr_amount: percent("NR amount", cassette.nr.amount, 1.0),
            nr_brightness: percent("NR brightness", cassette.nr.brightness, 1.0),
            nr_level_error: gain_within(
                "NR level error",
                db_to_gain(cassette.nr.level_error_db),
                -MAX_LEVEL_ERROR_DB,
                MAX_LEVEL_ERROR_DB,
            ),
            dropouts: percent("Dropouts", cassette.dropouts.amount, 1.0),
            dropout_intensity: percent("Dropout intensity", cassette.dropouts.intensity, 1.0),
            dropout_width: percent("Dropout width", cassette.dropouts.width, 1.0),

            // Skewed so the deck faults that matter, a few to twenty arc-minutes, take the first
            // half of the travel: 15′ is its middle.
            azimuth: FloatParam::new(
                "Azimuth",
                cassette.azimuth,
                FloatRange::Skewed {
                    min: 0.0,
                    max: MAX_AZIMUTH_ARCMIN,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_value_to_string(arcmin_to_string())
            .with_string_to_value(string_to_arcmin()),
            crosstalk: off_level(
                "Crosstalk",
                cassette.crosstalk_db,
                CROSSTALK_OFF_DB,
                CROSSTALK_MAX_DB,
            ),
            wrong_eq: BoolParam::new("Wrong EQ", cassette.wrong_eq),

            drive: gain("Drive"),
            width: percent("Width", init.width, MAX_WIDTH),
            mix: percent("Mix", init.mix, 1.0),
            noise_follows: percent("Noise follows", init.noise_follows, 1.0),
            noise_stops: BoolParam::new("Noise stops in silence", init.noise_stops),
            variation: IntParam::new(
                "Variation",
                cassette.variation as i32,
                IntRange::Linear {
                    min: 0,
                    max: MAX_VARIATION as i32,
                },
            ),
            quality: EnumParam::new("Quality", QualityChoice::Normal),
            preset: RwLock::new(PresetIdentity::none()),
        }
    }
}

impl MxmFxOldifyParams {
    /// A synced rate in hertz at `tempo`, or `None` for the free knob: sync off, or no usable
    /// tempo. The rate's **modulated** position picks the division, as a moved knob would.
    fn synced(ladder: Ladder, on: bool, rate: &FloatParam, tempo: Option<f64>) -> Option<f32> {
        ladder
            .resolve(
                on,
                tempo,
                rate.modulated_normalized_value(),
                f64::from(rate.preview_plain(0.0)),
                f64::from(rate.preview_plain(1.0)),
            )
            .map(|hz| hz as f32)
    }

    /// Wow's rate in force at `tempo`: its division while synced to a tempo, the knob otherwise.
    pub fn wow_hz(&self, tempo: Option<f64>) -> f32 {
        Self::synced(WOW_SYNC, self.wow_sync.value(), &self.wow_rate, tempo)
            .unwrap_or_else(|| self.wow_rate.value())
    }

    /// Flutter's rate in force at `tempo`.
    pub fn flutter_hz(&self, tempo: Option<f64>) -> f32 {
        Self::synced(
            FLUTTER_SYNC,
            self.flutter_sync.value(),
            &self.flutter_rate,
            tempo,
        )
        .unwrap_or_else(|| self.flutter_rate.value())
    }

    /// The stop's time in force at `tempo`: its division while synced to a tempo, the knob
    /// otherwise.
    pub fn stop_s(&self, tempo: Option<f64>) -> f32 {
        Self::synced(STOP_SYNC, self.stop_sync.value(), &self.stop_time, tempo)
            .unwrap_or_else(|| self.stop_time.value())
    }

    /// This block's targets for the DSP, as the host has them (host modulation included), the
    /// synced rates resolved at the host's `tempo`.
    pub fn controls(&self, tempo: Option<f64>) -> Controls {
        Controls {
            record_on: self.record_on.value(),
            tape_on: self.tape_on.value(),
            drive: self.drive.value(),
            width: self.width.value(),
            mix: self.mix.value(),
            noise_follows: self.noise_follows.value(),
            noise_stops: self.noise_stops.value(),
            quality: self.quality.value().dsp(),
            record: RecordControls {
                disc: DiscControls {
                    speed: self.disc.value().dsp(),
                    surface_db: surface_db(self.surface_noise.value()),
                    crackle: self.crackle.value(),
                    crackle_gain: self.crackle_level.value(),
                    clicks: self.clicks.value(),
                    scratches: u8::try_from(self.scratches.value()).unwrap_or(0),
                    cracks: u8::try_from(self.cracks.value()).unwrap_or(0),
                    rumble_db: rumble_db(self.rumble.value()),
                },
                era: self.era.value().dsp(),
                player: self.player.value().dsp(),
                cut: self.cut_curve.value().dsp(),
                replay: self.replay_curve.value().dsp(),
                groove: GrooveControls {
                    side: self.side.value(),
                    wear: self.wear.value(),
                    dynamic: self.dynamic_wear.value(),
                    distortion: self.groove_distortion.value(),
                    // The player sets the tip; the DSP ignores this one.
                    tip_um: Controls::INIT.record.groove.tip_um,
                },
                eccentricity_mm: self.eccentricity.value(),
                warp: self.warp.value(),
                skips: self.skips.value(),
                stuck: self.stuck_groove.value(),
                mains: self.hum.value().dsp(),
                hum_db: hum_db(self.hum_level.value()),
                horn: HornControls {
                    amount: self.horn.value(),
                    length_m: self.horn_size.value(),
                },
                echo_db: echo_db(self.groove_echo.value()),
            },
            cassette: CassetteControls {
                tape: self.tape.value().dsp(),
                grade: self.grade.value().dsp(),
                tone: self.tape_tone.value(),
                saturation: self.saturation.value(),
                character: self.character.value().dsp(),
                generations: u8::try_from(self.generations.value()).unwrap_or(1),
                wow: Motion {
                    depth: self.wow.value(),
                    hz: self.wow_hz(tempo),
                    shape: self.wow_shape.value().dsp(),
                },
                flutter: Motion {
                    depth: self.flutter.value(),
                    hz: self.flutter_hz(tempo),
                    shape: self.flutter_shape.value().dsp(),
                },
                follow: self.follow.value(),
                flange: self.flange.value(),
                stop: self.tape_stop.value(),
                stop_s: self.stop_s(tempo),
                hiss_db: hiss_db(self.hiss.value()),
                nr: NrControls {
                    kind: self.nr.value().dsp(),
                    decode: self.nr_decode.value(),
                    amount: self.nr_amount.value(),
                    brightness: self.nr_brightness.value(),
                    level_error_db: level_error_db(self.nr_level_error.value()),
                },
                dropouts: DropoutControls {
                    amount: self.dropouts.value(),
                    intensity: self.dropout_intensity.value(),
                    width: self.dropout_width.value(),
                },
                azimuth: self.azimuth.value(),
                crosstalk_db: crosstalk_db(self.crosstalk.value()),
                print_db: print_db(self.print_through.value()),
                wrong_eq: self.wrong_eq.value(),
                age: self.age.value(),
                variation: u32::try_from(self.variation.value()).unwrap_or(0),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::params::InternalParamMut;

    /// A level, its conversion for the DSP, its off level and its top reading.
    type Level<'a> = (&'a FloatParam, fn(f32) -> f32, f32, &'a str);
    /// A rate, its sync, its ladder and the rate in force.
    type Rate<'a> = (
        &'a FloatParam,
        &'a BoolParam,
        Ladder,
        fn(&MxmFxOldifyParams, Option<f64>) -> f32,
    );

    fn set<P: InternalParamMut>(param: &P, normalized: f32) {
        unsafe {
            let _ = param._internal_set_normalized_value(normalized);
        }
    }

    /// **The parameters' defaults are the init patch**, exactly: the CLAP defaults, Init and the
    /// DSP's `Controls::INIT` are one set of values.
    #[test]
    fn the_defaults_are_the_dsp_init_patch() {
        assert_eq!(MxmFxOldifyParams::default().controls(None), Controls::INIT);
    }

    /// **No retired id is back**, under any spelling the derive produces.
    #[test]
    fn no_retired_id_is_back() {
        use nice_plug::prelude::Params;
        let params = MxmFxOldifyParams::default();
        let ids: Vec<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        for retired in RETIRED_IDS {
            assert!(!ids.iter().any(|id| id == retired), "`{retired}` is back");
        }
    }

    /// **0 dB is the middle of a gain's travel**, so a knob's centre is the level it came in at.
    #[test]
    fn a_gain_reads_zero_decibels_at_the_middle_and_its_ends_at_plus_minus_twenty_four() {
        let params = MxmFxOldifyParams::default();
        let drive = &params.drive;
        assert_eq!(drive.normalized_value_to_string(0.5, true), "0.0 dB");
        assert_eq!(drive.normalized_value_to_string(0.0, true), "-24.0 dB");
        assert_eq!(drive.normalized_value_to_string(1.0, true), "24.0 dB");
        let error = &params.nr_level_error;
        assert_eq!(error.normalized_value_to_string(0.5, true), "0.0 dB");
        assert_eq!(error.normalized_value_to_string(0.0, true), "-6.0 dB");
        assert_eq!(error.normalized_value_to_string(1.0, true), "6.0 dB");
        assert_eq!(level_error_db(error.preview_plain(0.5)), 0.0);
        assert_eq!(params.width.normalized_value_to_string(0.5, true), "100 %");
        assert_eq!(params.width.normalized_value_to_string(1.0, true), "200 %");
    }

    /// **The bottom of every level with an Off is Off, and Off plays nothing**; wherever the knob
    /// reads Off, the DSP gets its off level, and nowhere else.
    #[test]
    fn a_level_reads_off_at_the_bottom_and_off_plays_nothing() {
        let params = MxmFxOldifyParams::default();
        let levels: [Level<'_>; 7] = [
            (&params.hiss, hiss_db, HISS_OFF_DB, "-30.0 dB"),
            (
                &params.crosstalk,
                crosstalk_db,
                CROSSTALK_OFF_DB,
                "-30.0 dB",
            ),
            (&params.surface_noise, surface_db, NOISE_OFF_DB, "-15.0 dB"),
            (&params.rumble, rumble_db, NOISE_OFF_DB, "-35.0 dB"),
            (&params.hum_level, hum_db, NOISE_OFF_DB, "-20.0 dB"),
            (&params.groove_echo, echo_db, NOISE_OFF_DB, "-12.0 dB"),
            (&params.print_through, print_db, PRINT_OFF_DB, "-12.0 dB"),
        ];
        for (param, to_db, off_db, top) in levels {
            assert_eq!(param.normalized_value_to_string(0.0, true), "Off");
            assert_eq!(param.normalized_value_to_string(1.0, true), top);
            assert_eq!(param.string_to_normalized_value("Off"), Some(0.0));
            assert_eq!(to_db(param.preview_plain(0.0)), off_db);
            for step in 0..=1000 {
                let normalized = step as f32 / 1000.0;
                let reading = param.normalized_value_to_string(normalized, true);
                let db = to_db(param.preview_plain(normalized));
                assert_eq!(
                    reading == "Off",
                    db <= off_db,
                    "{} reads {reading} at {normalized}",
                    param.name()
                );
            }
        }
        assert_eq!(
            mxm_fx_oldify_dsp::cassette::hiss_gain(hiss_db(params.hiss.preview_plain(0.0))),
            0.0
        );
    }

    /// **A rate's geometric mean sits mid-travel**, so its octaves are spread across the knob.
    #[test]
    fn a_rate_is_log_skewed_with_its_geometric_mean_mid_travel() {
        let params = MxmFxOldifyParams::default();
        for (param, (min, max)) in [
            (&params.wow_rate, WOW_HZ),
            (&params.flutter_rate, FLUTTER_HZ),
        ] {
            let middle = param.preview_plain(0.5);
            assert!(
                (middle - (min * max).sqrt()).abs() < 1e-3 * max,
                "{} reads {middle} mid-travel",
                param.name()
            );
            assert_eq!(param.preview_plain(0.0), min);
            assert!((param.preview_plain(1.0) - max).abs() < 1e-4);
        }
    }

    /// **Each ladder is its rate's own range at 120 bpm**: every division it offers is inside the
    /// range there, and the divisions on either side of it are not.
    #[test]
    fn each_sync_ladder_is_its_rates_own_range_at_120_bpm() {
        for (ladder, (min, max)) in [(WOW_SYNC, WOW_HZ), (FLUTTER_SYNC, FLUTTER_HZ)] {
            let inside = |division: Division| {
                let hz = division.hz(120.0);
                hz >= f64::from(min) && hz <= f64::from(max)
            };
            let divisions = ladder.span.divisions();
            assert!(divisions.iter().all(|d| inside(*d)), "{divisions:?}");
            let first = divisions[0].index();
            let last = divisions[divisions.len() - 1].index();
            if first > 0 {
                assert!(!inside(Division::ALL[first - 1]));
            }
            if last + 1 < Division::ALL.len() {
                assert!(!inside(Division::ALL[last + 1]));
            }
        }
    }

    /// **A sync picks a division and is inert without a tempo**: off, or with no tempo, the knob's
    /// own hertz stand; on at 120 bpm the ends of the travel are the ladder's ends, the top the
    /// fastest, and the DSP is handed that rate.
    #[test]
    fn each_sync_picks_a_division_and_is_inert_without_a_tempo() {
        let params = MxmFxOldifyParams::default();
        let rates: [Rate<'_>; 2] = [
            (
                &params.wow_rate,
                &params.wow_sync,
                WOW_SYNC,
                MxmFxOldifyParams::wow_hz,
            ),
            (
                &params.flutter_rate,
                &params.flutter_sync,
                FLUTTER_SYNC,
                MxmFxOldifyParams::flutter_hz,
            ),
        ];
        for (rate, switch, ladder, hz) in rates {
            set(rate, 1.0);
            set(switch, 0.0);
            assert_eq!(
                hz(&params, Some(120.0)),
                rate.value(),
                "off is the free rate"
            );
            set(switch, 1.0);
            assert_eq!(hz(&params, None), rate.value(), "no tempo is the free rate");
            let top = hz(&params, Some(120.0));
            set(rate, 0.0);
            let bottom = hz(&params, Some(120.0));
            let divisions = ladder.span.divisions();
            let fastest = divisions[0].hz(120.0) as f32;
            let slowest = divisions[divisions.len() - 1].hz(120.0) as f32;
            assert!((top - fastest).abs() < 1e-4, "{top} against {fastest}");
            assert!(
                (bottom - slowest).abs() < 1e-4,
                "{bottom} against {slowest}"
            );
            set(switch, 0.0);
        }
        set(&params.wow_sync, 1.0);
        assert_eq!(
            params.controls(Some(120.0)).cassette.wow.hz,
            params.wow_hz(Some(120.0))
        );
    }

    /// **Host modulation picks the division**, as a moved knob would.
    #[test]
    fn host_modulation_of_a_synced_rate_picks_the_division() {
        let params = MxmFxOldifyParams::default();
        set(&params.wow_sync, 1.0);
        set(&params.wow_rate, 0.2);
        let set_at = params.wow_hz(Some(120.0));
        unsafe {
            let _ = params.wow_rate._internal_modulate_value(0.6);
        }
        let modulated = WOW_SYNC.division(0.8, 120.0, 0.1, 5.0).hz(120.0) as f32;
        assert!((params.wow_hz(Some(120.0)) - modulated).abs() < 1e-5);
        assert!(modulated > set_at, "modulation up picked no faster rate");
    }

    /// **Stop time's ladder is its own range at 120 bpm**, the top the longest, and synced it
    /// hands the DSP the division in force; off, or with no tempo, the knob's own time.
    #[test]
    fn the_stop_sync_ladder_is_stop_times_own_range_and_picks_a_division() {
        let inside = |division: Division| {
            let s = division.seconds(120.0);
            s >= f64::from(MIN_STOP_S) && s <= f64::from(MAX_STOP_S)
        };
        let divisions = STOP_SYNC.span.divisions();
        assert!(divisions.iter().all(|d| inside(*d)), "{divisions:?}");
        let (first, last) = (divisions[0].index(), divisions[divisions.len() - 1].index());
        assert!(first == 0 || !inside(Division::ALL[first - 1]));
        assert!(last + 1 == Division::ALL.len() || !inside(Division::ALL[last + 1]));

        let params = MxmFxOldifyParams::default();
        set(&params.stop_time, 1.0);
        assert_eq!(params.stop_s(Some(120.0)), params.stop_time.value());
        set(&params.stop_sync, 1.0);
        assert_eq!(params.stop_s(None), params.stop_time.value());
        let longest = divisions[divisions.len() - 1].seconds(120.0) as f32;
        assert!((params.stop_s(Some(120.0)) - longest).abs() < 1e-4);
        set(&params.stop_time, 0.0);
        let shortest = divisions[0].seconds(120.0) as f32;
        assert!((params.stop_s(Some(120.0)) - shortest).abs() < 1e-4);
        assert_eq!(params.controls(Some(120.0)).cassette.stop_s, shortest);
    }

    /// **Stop time reads the same through the host's text round trip** either side of a second.
    #[test]
    fn stop_time_survives_the_hosts_text_round_trip() {
        mxm_plugin_test::time_text_checks::time_readings_round_trip(
            &MxmFxOldifyParams::default(),
            &["stop_time"],
        );
    }

    /// **Every parameter's text survives the host's round trip**, with the unit on, across the
    /// collection's grids and switch points (`mxm_plugin_test::text_checks`).
    #[test]
    fn every_parameter_text_survives_the_hosts_round_trip() {
        mxm_plugin_test::text_checks::every_text_round_trips(&MxmFxOldifyParams::default(), &[]);
    }

    /// Azimuth reads a typed arc-minute either way, `15'` or `15 arcmin`.
    #[test]
    fn azimuth_reads_either_spelling_of_an_arc_minute() {
        let params = MxmFxOldifyParams::default();
        assert_eq!(
            params.azimuth.string_to_normalized_value("15'"),
            params.azimuth.string_to_normalized_value("15 arcmin")
        );
    }

    /// **A rate reads the same through the host's text round trip either side of 1 Hz**, where its
    /// precision changes: the grid above can step over the sliver where it went wrong.
    #[test]
    fn a_rate_reads_the_same_either_side_of_one_hertz() {
        let params = MxmFxOldifyParams::default();
        for text in [
            "0.9944 Hz",
            "0.9945 Hz",
            "0.9946 Hz",
            "0.9949 Hz",
            "0.9994 Hz",
            "0.9995 Hz",
            "0.9996 Hz",
            "1 Hz",
            "1.004 Hz",
            "1.006 Hz",
        ] {
            let normalized = params
                .wow_rate
                .string_to_normalized_value(text)
                .unwrap_or_else(|| panic!("{text} rejected"));
            let first = params.wow_rate.normalized_value_to_string(normalized, true);
            let again = params
                .wow_rate
                .string_to_normalized_value(&first)
                .unwrap_or_else(|| panic!("{first} rejected"));
            let second = params.wow_rate.normalized_value_to_string(again, true);
            assert_eq!(first, second, "{text}");
        }
    }
}
