//! Permanent parameter definitions, in signal order, and their mapping onto the DSP's controls.
//!
//! **Nothing here carries a nice-plug smoother.** Every continuous control is smoothed once, in
//! `mxm-fx-oldify-dsp`, which ramps each block's targets over its `SMOOTHING_S`; a second smoother
//! here would stack two ramps on one control. The switches are not smoothed anywhere: the DSP takes
//! a new tape, grade, character, shape or count as it comes.
//!
//! **Wow rate and Flutter rate sync to the host's tempo** (mxm-kit's plugin conventions, *Tempo sync
//! is one button changing one Rate or Time control*): each has one *sync* switch, and synced, the
//! rate's own position picks a division on its [`mxm_tempo::Ladder`]. [`MxmFxOldifyParams::controls`]
//! resolves it from the tempo it is handed, every block, so nothing resolved is kept to go stale
//! across an activation or a reset.

use std::sync::{Arc, RwLock};

use mxm_fx_oldify_dsp::cassette::{
    CROSSTALK_MAX_DB, CROSSTALK_OFF_DB, CassetteControls, FLUTTER_HZ, HISS_MAX_DB, HISS_OFF_DB,
    MAX_AZIMUTH_ARCMIN, MAX_GENERATIONS, MAX_VARIATION, Motion, WOW_HZ,
};
use mxm_fx_oldify_dsp::dropouts::DropoutControls;
use mxm_fx_oldify_dsp::saturate::Character;
use mxm_fx_oldify_dsp::tape::{Grade, Tape};
use mxm_fx_oldify_dsp::transport::Shape;
use mxm_fx_oldify_dsp::{Controls, MAX_GAIN_DB, MAX_WIDTH, MIN_GAIN_DB, Medium, db_to_gain};
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

#[derive(Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediumChoice {
    #[id = "cassette"]
    #[name = "Cassette"]
    Cassette,
    #[id = "record"]
    #[name = "Record"]
    Record,
    #[id = "record-then-cassette"]
    #[name = "Record then cassette"]
    RecordThenCassette,
}

impl MediumChoice {
    pub const fn dsp(self) -> Medium {
        match self {
            Self::Cassette => Medium::Cassette,
            Self::Record => Medium::Record,
            Self::RecordThenCassette => Medium::RecordThenCassette,
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

/// A rate in hertz. The unit's precision is chosen from the rounded reading, so the host's text
/// round trip is idempotent at the switch.
fn rate_to_string() -> ValueToString {
    Arc::new(|value| {
        if (value * 100.0).round() >= 100.0 {
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
    FloatParam::new(
        name,
        1.0,
        FloatRange::Skewed {
            min: db_to_gain(MIN_GAIN_DB),
            max: db_to_gain(MAX_GAIN_DB),
            factor: FloatRange::gain_skew_factor(MIN_GAIN_DB, MAX_GAIN_DB),
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

/// A tempo sync's switch: On/Off, as every sync in the collection reads.
fn sync(name: &'static str) -> BoolParam {
    BoolParam::new(name, false)
}

#[derive(Params)]
pub struct MxmFxOldifyParams {
    // ---- Medium ----
    #[id = "medium"]
    pub medium: EnumParam<MediumChoice>,
    #[id = "age"]
    pub age: FloatParam,
    #[id = "variation"]
    pub variation: IntParam,

    // ---- Tape ----
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

    // ---- Noise ----
    /// Stored as linear gain; the bottom of the travel is Off.
    #[id = "hiss"]
    pub hiss: FloatParam,
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
    #[id = "output"]
    pub output: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "noise_follows"]
    pub noise_follows: FloatParam,
    #[id = "noise_stops"]
    pub noise_stops: BoolParam,

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
        Self {
            medium: EnumParam::new("Medium", MediumChoice::Cassette),
            age: percent("Age", cassette.age, 1.0),
            variation: IntParam::new(
                "Variation",
                cassette.variation as i32,
                IntRange::Linear {
                    min: 0,
                    max: MAX_VARIATION as i32,
                },
            ),

            tape: EnumParam::new("Tape", TapeChoice::TypeI),
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

            wow: percent("Wow", cassette.wow.depth, 1.0),
            wow_rate: rate("Wow rate", cassette.wow.hz, WOW_HZ),
            wow_sync: sync("Wow sync"),
            wow_shape: EnumParam::new("Wow shape", WowShapeChoice::Random),
            flutter: percent("Flutter", cassette.flutter.depth, 1.0),
            flutter_rate: rate("Flutter rate", cassette.flutter.hz, FLUTTER_HZ),
            flutter_sync: sync("Flutter sync"),
            flutter_shape: EnumParam::new("Flutter shape", FlutterShapeChoice::Random),
            follow: BoolParam::new("Flutter follows wow", cassette.follow),
            flange: percent("Flange", cassette.flange, 1.0),

            hiss: off_level("Hiss", cassette.hiss_db, HISS_OFF_DB, HISS_MAX_DB),
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
            output: gain("Output"),
            mix: percent("Mix", init.mix, 1.0),
            noise_follows: percent("Noise follows", init.noise_follows, 1.0),
            noise_stops: BoolParam::new("Noise stops in silence", init.noise_stops),
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

    /// This block's targets for the DSP, as the host has them (host modulation included), the
    /// synced rates resolved at the host's `tempo`.
    pub fn controls(&self, tempo: Option<f64>) -> Controls {
        Controls {
            medium: self.medium.value().dsp(),
            drive: self.drive.value(),
            width: self.width.value(),
            output: self.output.value(),
            mix: self.mix.value(),
            noise_follows: self.noise_follows.value(),
            noise_stops: self.noise_stops.value(),
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
                hiss_db: hiss_db(self.hiss.value()),
                dropouts: DropoutControls {
                    amount: self.dropouts.value(),
                    intensity: self.dropout_intensity.value(),
                    width: self.dropout_width.value(),
                },
                azimuth: self.azimuth.value(),
                crosstalk_db: crosstalk_db(self.crosstalk.value()),
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

    /// **0 dB is the middle of a gain's travel**, so a knob's centre is the level it came in at.
    #[test]
    fn a_gain_reads_zero_decibels_at_the_middle_and_its_ends_at_plus_minus_twenty_four() {
        let params = MxmFxOldifyParams::default();
        for param in [&params.drive, &params.output] {
            assert_eq!(param.normalized_value_to_string(0.5, true), "0.0 dB");
            assert_eq!(param.normalized_value_to_string(0.0, true), "-24.0 dB");
            assert_eq!(param.normalized_value_to_string(1.0, true), "24.0 dB");
        }
        assert_eq!(params.width.normalized_value_to_string(0.5, true), "100 %");
        assert_eq!(params.width.normalized_value_to_string(1.0, true), "200 %");
    }

    /// **The bottom of Hiss and of Crosstalk is Off, and Off plays nothing**; wherever the knob
    /// reads Off, the DSP gets its off level, and nowhere else.
    #[test]
    fn a_level_reads_off_at_the_bottom_and_off_plays_nothing() {
        let params = MxmFxOldifyParams::default();
        let levels: [Level<'_>; 2] = [
            (&params.hiss, hiss_db, HISS_OFF_DB, "-30.0 dB"),
            (
                &params.crosstalk,
                crosstalk_db,
                CROSSTALK_OFF_DB,
                "-30.0 dB",
            ),
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

    #[test]
    fn parameter_text_is_idempotent_through_the_host_conversion() {
        let params = MxmFxOldifyParams::default();
        let continuous: [&FloatParam; 19] = [
            &params.age,
            &params.tape_tone,
            &params.saturation,
            &params.wow,
            &params.wow_rate,
            &params.flutter,
            &params.flutter_rate,
            &params.flange,
            &params.hiss,
            &params.dropouts,
            &params.dropout_intensity,
            &params.dropout_width,
            &params.azimuth,
            &params.crosstalk,
            &params.drive,
            &params.width,
            &params.output,
            &params.mix,
            &params.noise_follows,
        ];
        for param in continuous {
            for step in 0..=1000 {
                let normalized = step as f32 / 1000.0;
                let first = param.normalized_value_to_string(normalized, true);
                let reparsed = param
                    .string_to_normalized_value(&first)
                    .unwrap_or_else(|| panic!("{} rejected {first:?}", param.name()));
                let second = param.normalized_value_to_string(reparsed, true);
                assert_eq!(first, second, "{} at {normalized}", param.name());
            }
        }
        assert_eq!(
            params.azimuth.string_to_normalized_value("15'"),
            params.azimuth.string_to_normalized_value("15 arcmin")
        );
    }
}
