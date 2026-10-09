//! The factory bank, and the Init every bank starts from.
//!
//! **The bank is empty until its sounds are designed by ear** (`plugins/mxm-fx-oldify/AGENTS.md`,
//! *Presets*). The recipe check below runs over whatever ships, so the first sound added is held by
//! it at once.

use mxm_fx_oldify::params::MxmFxOldifyParams;
use mxm_fx_oldify::{MxmFxOldify, preset};
use nice_plug::params::InternalParamMut;
use nice_plug::prelude::*;

const FS: f32 = 48_000.0;

/// A host that applies each write at once, as a GUI context's would after its process flush.
struct ApplyingHost;

impl nice_plug::context::gui::GuiContextInner for ApplyingHost {
    // A test double has no host to ask for a restart (nice-plug 0.4).
    fn request_restart(&self) {}
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Clap
    }
    unsafe fn raw_begin_set_parameter(&self, _param: nice_plug::params::internals::ParamPtr) {}
    unsafe fn raw_set_parameter_normalized(
        &self,
        param: nice_plug::params::internals::ParamPtr,
        value: f32,
    ) {
        unsafe {
            let _ = param._internal_set_normalized_value(value);
        }
    }
    unsafe fn raw_end_set_parameter(&self, _param: nice_plug::params::internals::ParamPtr) {}
    fn get_state(&self) -> PluginState {
        PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        }
    }
    fn set_state(&self, _: PluginState) {}
}

fn apply(plugin: &MxmFxOldify, parsed: &preset::Preset, origin: preset::Origin) {
    let host = ApplyingHost;
    let setter = ParamSetter::new(&host);
    let (writes, problems) = parsed.resolve_from(&*plugin.params, Some(&origin));
    assert!(problems.is_empty(), "{}: {problems:?}", parsed.name);
    for (_, parameter, value) in writes {
        parameter.set(&setter, value);
    }
}

/// A harmonic note at −6 dBFS, the collection's declared programme for a clip audit.
fn programme() -> Vec<f32> {
    let frames = FS as usize;
    let normaliser: f32 = (1..=10).map(|harmonic| 1.0 / harmonic as f32).sum();
    (0..frames)
        .map(|sample| {
            let time = sample as f32 / FS;
            let wave: f32 = (1..=10)
                .map(|harmonic| {
                    (core::f32::consts::TAU * 180.0 * harmonic as f32 * time).sin()
                        / harmonic as f32
                })
                .sum();
            0.5 * wave / normaliser
        })
        .collect()
}

fn render(plugin: &mut MxmFxOldify, input: &[f32]) -> Vec<f32> {
    let mut output = Vec::with_capacity(input.len() * 2);
    for block in input.chunks(256) {
        let mut left = block.to_vec();
        let mut right = block.to_vec();
        {
            let mut channels: Vec<&mut [f32]> = vec![&mut left, &mut right];
            let _ = plugin.process_block_for_test(&mut channels);
        }
        for (&left, &right) in left.iter().zip(&right) {
            output.extend_from_slice(&[left, right]);
        }
    }
    output
}

#[test]
fn the_bank_is_empty_until_its_sounds_are_designed() {
    assert!(
        preset::FACTORY_FILES.is_empty(),
        "a factory sound shipped: record the bank in plugins/mxm-fx-oldify/AGENTS.md and give it \
         a design table and a render audit, as mxm-fx-delay's has"
    );
}

#[test]
fn every_shipped_recipe_loads_completely_and_renders_finite_without_clipping() {
    let input = programme();
    for (slug, text) in preset::FACTORY_FILES {
        let parsed = preset::Preset::parse(text, mxm_fx_oldify::CLAP_ID)
            .unwrap_or_else(|refused| panic!("{slug}: {refused:?}"));
        let mut plugin = MxmFxOldify::default();
        apply(&plugin, &parsed, preset::Origin::Factory);
        assert!(plugin.prepare_for_test(FS, 2));
        let wet = render(&mut plugin, &input);
        assert!(wet.iter().all(|sample| sample.is_finite()), "{slug}");
        let peak = wet
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        assert!(peak < 1.0, "{slug} clips the declared programme at {peak}");
    }
}

/// **Init, applied as the browser applies it, returns every parameter to its default and sounds
/// as a fresh instance does**: the init patch, the engaged cassette.
#[test]
fn init_applied_over_an_edited_patch_restores_the_init_patch() {
    let plugin = MxmFxOldify::default();
    let edited = MxmFxOldifyParams::default();
    unsafe {
        let _ = edited.drive._internal_set_normalized_value(0.9);
        let _ = edited.saturation._internal_set_normalized_value(1.0);
        let _ = edited.tape._internal_set_normalized_value(1.0);
        let _ = edited.mix._internal_set_normalized_value(0.2);
    }
    let edited_patch = preset::Preset::capture("Edited", preset::Category::Fx, &edited);
    apply(&plugin, &edited_patch, preset::Origin::User);
    assert_ne!(plugin.params.mix.value(), 1.0);

    let init = preset::Preset::init(&*plugin.params);
    apply(&plugin, &init, preset::Origin::User);
    for (id, param, _) in plugin.params.param_map() {
        // SAFETY: the parameters are this test's own.
        let (now, default) = unsafe {
            (
                param.unmodulated_normalized_value(),
                param.default_normalized_value(),
            )
        };
        assert!(
            (now - default).abs() < 1e-6,
            "{id} is not its default after Init"
        );
    }

    let input = programme();
    let mut plugin = plugin;
    assert!(plugin.prepare_for_test(FS, 2));
    let mut reference = MxmFxOldify::default();
    assert!(reference.prepare_for_test(FS, 2));
    let rendered = render(&mut plugin, &input);
    let expected = render(&mut reference, &input);
    let difference = rendered
        .iter()
        .zip(&expected)
        .fold(0.0f32, |peak, (a, b)| peak.max((a - b).abs()));
    // Init writes normalised values, so a control may land a rounding step off its default.
    assert!(
        difference < 1.0e-4,
        "Init does not sound as a fresh instance: {difference}"
    );
}
