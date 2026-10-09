//! Permanent parameter definitions.
//!
//! **Nothing here carries a nice-plug smoother.** Every control that moves the audio — Drive,
//! Width, Output and Mix — is smoothed once, in `mxm-fx-oldify-dsp`, which ramps each block's
//! targets over its `SMOOTHING_S`; a second smoother here would stack two ramps on one control.
//! Medium and Age move nothing yet.

use std::sync::{Arc, RwLock};

use mxm_fx_oldify_dsp::{MAX_GAIN_DB, MAX_WIDTH, MIN_GAIN_DB, db_to_gain};
use mxm_preset::PresetIdentity;
use nice_plug::prelude::*;

/// Age's default: a used but cared-for medium, once the engines exist to hear it.
pub const DEFAULT_AGE: f32 = 0.3;

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

#[derive(Params)]
pub struct MxmFxOldifyParams {
    #[id = "medium"]
    pub medium: EnumParam<MediumChoice>,
    #[id = "age"]
    pub age: FloatParam,
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
    fn default() -> Self {
        Self {
            medium: EnumParam::new("Medium", MediumChoice::Cassette),
            age: percent("Age", DEFAULT_AGE, 1.0),
            drive: gain("Drive"),
            width: percent("Width", 1.0, MAX_WIDTH),
            output: gain("Output"),
            mix: percent("Mix", 1.0, 1.0),
            preset: RwLock::new(PresetIdentity::none()),
        }
    }
}

impl MxmFxOldifyParams {
    /// This block's targets for the DSP, as the host has them (host modulation included).
    pub fn controls(&self) -> mxm_fx_oldify_dsp::Controls {
        mxm_fx_oldify_dsp::Controls {
            drive: self.drive.value(),
            width: self.width.value(),
            output: self.output.value(),
            mix: self.mix.value(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The init patch is the identity chain, and the controls that will colour it start where
    /// the brief puts them**: Cassette at 30 % Age. Until the engines exist that is inaudible, which
    /// `plugins/mxm-fx-oldify/AGENTS.md` records as the effect's one departure from *an effect
    /// starts engaged*.
    #[test]
    fn init_is_the_identity_chain_on_a_cassette_at_thirty_percent_age() {
        let params = MxmFxOldifyParams::default();
        assert_eq!(params.medium.value(), MediumChoice::Cassette);
        assert_eq!(params.age.value(), DEFAULT_AGE);
        assert_eq!(params.controls(), mxm_fx_oldify_dsp::Controls::IDENTITY);
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

    #[test]
    fn parameter_text_is_idempotent_through_the_host_conversion() {
        let params = MxmFxOldifyParams::default();
        let continuous: [&FloatParam; 5] = [
            &params.age,
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
