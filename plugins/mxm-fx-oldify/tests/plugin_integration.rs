//! The plugin shell over its DSP, through the same block entry a host's `process` calls.

use mxm_fx_oldify::MxmFxOldify;
use mxm_fx_oldify::params::MediumChoice;
use mxm_fx_oldify_dsp::{MAX_GAIN_DB, MAX_WIDTH, db_to_gain};
use nice_plug::params::{InternalParamMut, Param};
use nice_plug::prelude::{EnumParam, FloatParam, Params};

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

/// Every parameter at one end of its travel.
fn every_parameter_at(plugin: &MxmFxOldify, normalized: f32) {
    for (_, param, _) in plugin.params.param_map() {
        unsafe {
            let _ = param._internal_set_normalized_value(normalized);
        }
    }
}

fn programme(frames: usize, step: f32) -> Vec<f32> {
    (0..frames).map(|n| (n as f32 * step).sin() * 0.5).collect()
}

fn render(plugin: &mut MxmFxOldify, source: &[f32]) -> [Vec<f32>; 2] {
    let (mut left, mut right) = (source.to_vec(), source.to_vec());
    let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
    let _ = plugin.process_block_for_test(&mut channels);
    [left, right]
}

#[test]
fn every_rate_and_block_size_stays_finite_at_the_extremes() {
    // Full scale through the loudest gain, the widest image and the tape's own lift (the DSP's
    // own bound for its extremes).
    let ceiling = 4.0 * db_to_gain(MAX_GAIN_DB) * MAX_WIDTH + 1.0;
    for rate in [1_000.0, 44_100.0, 48_000.0, 192_000.0, 768_000.0] {
        for frames in [1, 63, 1024, 8192] {
            for end in [0.0, 1.0] {
                for mix in [0.0, 0.5, 1.0] {
                    let mut plugin = MxmFxOldify::default();
                    every_parameter_at(&plugin, end);
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
                        "{rate} Hz, {frames} frames, every parameter at {end}, mix {mix}"
                    );
                }
            }
        }
    }
}

/// **The Record medium passes the input, delayed by the latency, to the bit**: the record engine
/// is not built yet. When it lands this is the test that changes, on purpose.
#[test]
fn the_record_medium_passes_the_input_delayed_until_its_engine_exists() {
    let source = programme(8_192, 0.031);
    let mut plugin = MxmFxOldify::default();
    set_enum(&plugin.params.medium, MediumChoice::Record);
    assert!(plugin.prepare_for_test(48_000.0, 2));
    let latency = plugin.latency() as usize;
    let [left, right] = render(&mut plugin, &source);
    assert_eq!(left[latency..], source[..8_192 - latency]);
    assert_eq!(right[latency..], source[..8_192 - latency]);
}

/// A named change to a fresh instance's parameters.
type Change = fn(&MxmFxOldify);

/// **The cassette's controls are heard**: Age, Saturation, Wow and Hiss each change what comes out
/// of the init patch.
#[test]
fn the_cassette_controls_change_the_sound() {
    let source = programme(16_384, 0.057);
    let mut init = MxmFxOldify::default();
    assert!(init.prepare_for_test(48_000.0, 2));
    let reference = render(&mut init, &source);
    let moved: [(&str, Change); 4] = [
        ("Age", |p| set_float(&p.params.age, 1.0)),
        ("Saturation", |p| set_float(&p.params.saturation, 1.0)),
        ("Wow", |p| set_float(&p.params.wow, 1.0)),
        ("Hiss", |p| set_float(&p.params.hiss, db_to_gain(-30.0))),
    ];
    for (name, change) in moved {
        let mut plugin = MxmFxOldify::default();
        change(&plugin);
        assert!(plugin.prepare_for_test(48_000.0, 2));
        let rendered = render(&mut plugin, &source);
        let difference = rendered[0]
            .iter()
            .zip(&reference[0])
            .fold(0.0f32, |peak, (a, b)| peak.max((a - b).abs()));
        assert!(difference > 1e-3, "{name} changed nothing: {difference}");
    }
}
