//! The plugin shell over its DSP, through the same block entry a host's `process` calls.

use mxm_fx_oldify::MxmFxOldify;
use mxm_fx_oldify::params::MediumChoice;
use mxm_fx_oldify_dsp::{MAX_GAIN_DB, MAX_WIDTH, MIN_GAIN_DB, db_to_gain};
use nice_plug::params::{InternalParamMut, Param};
use nice_plug::prelude::{EnumParam, FloatParam};

fn set_float(param: &FloatParam, value: f32) {
    unsafe {
        let _ = param._internal_set_normalized_value(param.preview_normalized(value));
    }
}

fn set_enum<E: nice_plug::params::enums::Enum + PartialEq>(param: &EnumParam<E>, value: E) {
    unsafe {
        let _ = param._internal_set_normalized_value(param.preview_normalized(value));
    }
}

#[test]
fn every_rate_and_block_size_stays_finite_at_the_extremes() {
    let gains = [db_to_gain(MIN_GAIN_DB), db_to_gain(MAX_GAIN_DB)];
    let ceiling = db_to_gain(MAX_GAIN_DB) * MAX_WIDTH * db_to_gain(MAX_GAIN_DB) + 1.0;
    for rate in [1_000.0, 44_100.0, 48_000.0, 96_000.0, 192_000.0, 768_000.0] {
        for frames in [1, 7, 63, 256, 1024, 8192] {
            for (drive, output) in [(gains[0], gains[1]), (gains[1], gains[1])] {
                for width in [0.0, MAX_WIDTH] {
                    for mix in [0.0, 0.5, 1.0] {
                        let mut plugin = MxmFxOldify::default();
                        set_float(&plugin.params.drive, drive);
                        set_float(&plugin.params.output, output);
                        set_float(&plugin.params.width, width);
                        set_float(&plugin.params.mix, mix);
                        assert!(plugin.prepare_for_test(rate, 1));
                        let mut left: Vec<f32> = (0..frames)
                            .map(|n| if n % 8 < 4 { 1.0 } else { -1.0 })
                            .collect();
                        let mut right = vec![0.0; frames];
                        left[0] = f32::NAN;
                        let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
                        let _ = plugin.process_block_for_test(&mut channels);
                        assert!(
                            left.iter()
                                .chain(&right)
                                .all(|sample| sample.is_finite() && sample.abs() <= ceiling),
                            "{rate} Hz, {frames} frames, drive {drive}, width {width}, mix {mix}"
                        );
                    }
                }
            }
        }
    }
}

/// **Medium and Age move nothing yet**: every Medium at every Age leaves the identity chain
/// passing the input to the bit. When the engines land this is the test that changes, on purpose.
#[test]
fn medium_and_age_are_inaudible_until_the_engines_exist() {
    let source: Vec<f32> = (0..2048).map(|n| (n as f32 * 0.031).sin() * 0.5).collect();
    for medium in [
        MediumChoice::Cassette,
        MediumChoice::Record,
        MediumChoice::RecordThenCassette,
    ] {
        for age in [0.0, 0.3, 1.0] {
            let mut plugin = MxmFxOldify::default();
            set_enum(&plugin.params.medium, medium);
            set_float(&plugin.params.age, age);
            assert!(plugin.prepare_for_test(48_000.0, 2));
            let mut left = source.clone();
            let mut right = source.clone();
            let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
            let _ = plugin.process_block_for_test(&mut channels);
            assert_eq!(left, source, "{medium:?} at {age}");
            assert_eq!(right, source, "{medium:?} at {age}");
        }
    }
}
