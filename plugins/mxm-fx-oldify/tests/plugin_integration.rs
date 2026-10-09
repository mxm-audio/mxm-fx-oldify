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
    let _ = plugin.process_block_for_test(&mut channels, None);
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
                    let _ = plugin.process_block_for_test(&mut channels, None);
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

/// Deterministic noise in −`level`..`level` (a 32-bit LCG): broadband, so a change anywhere in
/// the spectrum shows.
fn noise(frames: usize, seed: u32, level: f32) -> Vec<f32> {
    let mut state = seed;
    (0..frames)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            level * ((state >> 8) as f32 / (1u32 << 23) as f32 - 1.0)
        })
        .collect()
}

/// A parameter moved by id, as a normalised value.
type Move<'a> = (&'a str, f32);

/// Renders a pair of `level` noise through a fresh instance with `moves` applied.
fn render_moved(moves: &[Move<'_>], level: f32) -> [Vec<f32>; 2] {
    let (left, right) = (noise(24_000, 1, level), noise(24_000, 2, level));
    let mut plugin = MxmFxOldify::default();
    for (id, value) in moves {
        let (_, param, _) = plugin
            .params
            .param_map()
            .into_iter()
            .find(|(other, _, _)| other == id)
            .unwrap_or_else(|| panic!("no parameter {id}"));
        unsafe {
            let _ = param._internal_set_normalized_value(*value);
        }
    }
    assert!(plugin.prepare_for_test(48_000.0, 2));
    let (mut l, mut r) = (left, right);
    let mut channels: Vec<&mut [f32]> = vec![&mut l, &mut r];
    let _ = plugin.process_block_for_test(&mut channels, None);
    [l, r]
}

/// The largest difference between two renders, over both channels.
fn difference(a: &[Vec<f32>; 2], b: &[Vec<f32>; 2]) -> f32 {
    a.iter()
        .zip(b)
        .flat_map(|(x, y)| x.iter().zip(y))
        .fold(0.0f32, |peak, (x, y)| peak.max((x - y).abs()))
}

/// **Every cassette control is heard**: each one moved from the init patch changes what comes
/// out, in one channel or the other. A control that shapes something off at Init is moved with
/// that something on: the dropouts' shapers with Dropouts up, the noise reduction's with it on,
/// heard on a quiet programme, where it works.
#[test]
fn every_cassette_control_changes_the_sound() {
    const LOUD: f32 = 0.3;
    const QUIET: f32 = 0.01;
    let alone: [Move<'_>; 16] = [
        ("age", 1.0),
        ("variation", 0.5),
        ("tape", 1.0),
        ("grade", 0.0),
        ("saturation", 1.0),
        ("character", 1.0),
        ("generations", 1.0),
        ("wow", 1.0),
        ("flange", 1.0),
        ("hiss", 1.0),
        ("nr", 1.0),
        ("dropouts", 1.0),
        ("azimuth", 1.0),
        ("crosstalk", 1.0),
        ("wrong_eq", 1.0),
        ("noise_follows", 1.0),
    ];
    let reference = render_moved(&[], LOUD);
    for moved in alone {
        let heard = difference(&render_moved(&[moved], LOUD), &reference);
        assert!(heard > 1e-3, "{} changed nothing: {heard}", moved.0);
    }
    // (what must be on, its programme's level, the threshold, the controls it shapes)
    let shaped: [(Move<'_>, f32, f32, [Move<'_>; 4]); 2] = [
        (
            ("dropouts", 1.0),
            LOUD,
            1e-3,
            [
                ("dropout_intensity", 1.0),
                ("dropout_width", 1.0),
                ("dropout_intensity", 0.0),
                ("dropout_width", 0.0),
            ],
        ),
        (
            ("nr", 0.5),
            QUIET,
            1e-4,
            [
                ("nr_decode", 0.0),
                ("nr_amount", 1.0),
                ("nr_brightness", 1.0),
                ("nr_level_error", 1.0),
            ],
        ),
    ];
    for (on, level, threshold, moves) in shaped {
        let base = render_moved(&[on], level);
        for moved in moves {
            let heard = difference(&render_moved(&[on, moved], level), &base);
            assert!(heard > threshold, "{} changed nothing: {heard}", moved.0);
        }
    }
}
