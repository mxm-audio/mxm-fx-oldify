//! Permanent parameter definitions, in signal order, and their mapping onto the DSP's controls.
//!
//! **Nothing here carries a nice-plug smoother.** Every continuous control is smoothed once, in
//! `mxm-fx-oldify-dsp`, which ramps each block's targets over its `SMOOTHING_S`; a second smoother
//! here would stack two ramps on one control. The switches are not smoothed anywhere: the DSP takes
//! a new tape, grade, character or shape as it comes.

use std::sync::{Arc, RwLock};

use mxm_fx_oldify_dsp::cassette::{
    CassetteControls, FLUTTER_HZ, HISS_MAX_DB, HISS_OFF_DB, Motion, WOW_HZ,
};
use mxm_fx_oldify_dsp::saturate::Character;
use mxm_fx_oldify_dsp::tape::{Grade, Tape};
use mxm_fx_oldify_dsp::transport::Shape;
use mxm_fx_oldify_dsp::{Controls, MAX_GAIN_DB, MAX_WIDTH, MIN_GAIN_DB, Medium, db_to_gain};
use mxm_preset::PresetIdentity;
use nice_plug::prelude::*;

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

/// A linear gain in decibels.
fn gain_to_db(gain: f32) -> f32 {
    20.0 * gain.max(f32::MIN_POSITIVE).log10()
}

/// **Hiss's level as the DSP takes it, from the stored gain**: [`HISS_OFF_DB`] — off — wherever the
/// reading rounds to it, so what the knob reads and what plays are the same thing.
pub fn hiss_db(gain: f32) -> f32 {
    let db = gain_to_db(gain);
    if (db * 10.0).round() / 10.0 <= HISS_OFF_DB {
        HISS_OFF_DB
    } else {
        db.min(HISS_MAX_DB)
    }
}

fn hiss_to_string() -> ValueToString {
    Arc::new(|gain| {
        let db = hiss_db(gain);
        if db <= HISS_OFF_DB {
            "Off".to_owned()
        } else {
            format!("{db:.1} dB")
        }
    })
}

fn string_to_hiss() -> StringToValue {
    Arc::new(|text| {
        let text = text.trim();
        if text.eq_ignore_ascii_case("off") {
            return Some(db_to_gain(HISS_OFF_DB));
        }
        text.trim_end_matches([' ', 'd', 'D', 'b', 'B'])
            .parse::<f32>()
            .ok()
            .map(|db| db_to_gain(db.clamp(HISS_OFF_DB, HISS_MAX_DB)))
    })
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

#[derive(Params)]
pub struct MxmFxOldifyParams {
    #[id = "medium"]
    pub medium: EnumParam<MediumChoice>,
    #[id = "age"]
    pub age: FloatParam,

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

    #[id = "wow"]
    pub wow: FloatParam,
    #[id = "wow_rate"]
    pub wow_rate: FloatParam,
    #[id = "wow_shape"]
    pub wow_shape: EnumParam<WowShapeChoice>,
    #[id = "flutter"]
    pub flutter: FloatParam,
    #[id = "flutter_rate"]
    pub flutter_rate: FloatParam,
    #[id = "flutter_shape"]
    pub flutter_shape: EnumParam<FlutterShapeChoice>,

    /// Stored as linear gain; the bottom of the travel is Off.
    #[id = "hiss"]
    pub hiss: FloatParam,

    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "width"]
    pub width: FloatParam,
    #[id = "output"]
    pub output: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,

    #[persist = "preset"]
    pub preset: RwLock<PresetIdentity>,
}

impl Default for MxmFxOldifyParams {
    /// **The defaults are the DSP's init patch** (`Controls::INIT`), read from it rather than
    /// copied, so the two cannot drift apart.
    fn default() -> Self {
        let init = Controls::INIT;
        let cassette = init.cassette;
        Self {
            medium: EnumParam::new("Medium", MediumChoice::Cassette),
            age: percent("Age", cassette.age, 1.0),

            tape: EnumParam::new("Tape", TapeChoice::TypeI),
            grade: EnumParam::new("Grade", GradeChoice::Standard),
            tape_tone: percent("Tape tone", cassette.tone, 1.0),
            saturation: percent("Saturation", cassette.saturation, 1.0),
            character: EnumParam::new("Character", CharacterChoice::Soft),

            wow: percent("Wow", cassette.wow.depth, 1.0),
            wow_rate: rate("Wow rate", cassette.wow.hz, WOW_HZ),
            wow_shape: EnumParam::new("Wow shape", WowShapeChoice::Random),
            flutter: percent("Flutter", cassette.flutter.depth, 1.0),
            flutter_rate: rate("Flutter rate", cassette.flutter.hz, FLUTTER_HZ),
            flutter_shape: EnumParam::new("Flutter shape", FlutterShapeChoice::Random),

            hiss: FloatParam::new(
                "Hiss",
                db_to_gain(cassette.hiss_db),
                FloatRange::Skewed {
                    min: db_to_gain(HISS_OFF_DB),
                    max: db_to_gain(HISS_MAX_DB),
                    factor: FloatRange::gain_skew_factor(HISS_OFF_DB, HISS_MAX_DB),
                },
            )
            .with_value_to_string(hiss_to_string())
            .with_string_to_value(string_to_hiss()),

            drive: gain("Drive"),
            width: percent("Width", init.width, MAX_WIDTH),
            output: gain("Output"),
            mix: percent("Mix", init.mix, 1.0),
            preset: RwLock::new(PresetIdentity::none()),
        }
    }
}

impl MxmFxOldifyParams {
    /// This block's targets for the DSP, as the host has them (host modulation included).
    pub fn controls(&self) -> Controls {
        Controls {
            medium: self.medium.value().dsp(),
            drive: self.drive.value(),
            width: self.width.value(),
            output: self.output.value(),
            mix: self.mix.value(),
            cassette: CassetteControls {
                tape: self.tape.value().dsp(),
                grade: self.grade.value().dsp(),
                tone: self.tape_tone.value(),
                saturation: self.saturation.value(),
                character: self.character.value().dsp(),
                wow: Motion {
                    depth: self.wow.value(),
                    hz: self.wow_rate.value(),
                    shape: self.wow_shape.value().dsp(),
                },
                flutter: Motion {
                    depth: self.flutter.value(),
                    hz: self.flutter_rate.value(),
                    shape: self.flutter_shape.value().dsp(),
                },
                hiss_db: hiss_db(self.hiss.value()),
                age: self.age.value(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The parameters' defaults are the init patch**, exactly: the CLAP defaults, Init and the
    /// DSP's `Controls::INIT` are one set of values.
    #[test]
    fn the_defaults_are_the_dsp_init_patch() {
        assert_eq!(MxmFxOldifyParams::default().controls(), Controls::INIT);
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

    /// **The bottom of Hiss is Off, and Off is silence in the DSP**; the top is its loudest.
    #[test]
    fn hiss_reads_off_at_the_bottom_and_off_plays_nothing() {
        let params = MxmFxOldifyParams::default();
        assert_eq!(params.hiss.normalized_value_to_string(0.0, true), "Off");
        assert_eq!(
            params.hiss.normalized_value_to_string(1.0, true),
            "-30.0 dB"
        );
        assert_eq!(params.hiss.string_to_normalized_value("Off"), Some(0.0));
        assert_eq!(hiss_db(params.hiss.preview_plain(0.0)), HISS_OFF_DB);
        assert_eq!(
            mxm_fx_oldify_dsp::cassette::hiss_gain(hiss_db(params.hiss.preview_plain(0.0))),
            0.0
        );
        // Wherever the knob reads Off, nothing plays.
        for step in 0..=1000 {
            let normalized = step as f32 / 1000.0;
            let reading = params.hiss.normalized_value_to_string(normalized, true);
            let db = hiss_db(params.hiss.preview_plain(normalized));
            assert_eq!(
                reading == "Off",
                db <= HISS_OFF_DB,
                "{reading} at {normalized}"
            );
        }
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

    #[test]
    fn parameter_text_is_idempotent_through_the_host_conversion() {
        let params = MxmFxOldifyParams::default();
        let continuous: [&FloatParam; 12] = [
            &params.age,
            &params.tape_tone,
            &params.saturation,
            &params.wow,
            &params.wow_rate,
            &params.flutter,
            &params.flutter_rate,
            &params.hiss,
            &params.drive,
            &params.width,
            &params.output,
            &params.mix,
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
    }
}
