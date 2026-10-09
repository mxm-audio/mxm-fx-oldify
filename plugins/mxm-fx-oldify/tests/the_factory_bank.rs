//! The factory bank, and the Init every bank starts from.
//!
//! **The bank is empty until the engines exist** (`plugins/mxm-fx-oldify/AGENTS.md`, *Presets*):
//! a factory sound is a Medium and an Age heard on purpose, and neither moves anything yet. The
//! recipe check below runs over whatever ships, so the first sound added is held by it at once.

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
fn the_bank_is_empty_until_the_engines_exist() {
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

/// **Init, applied as the browser applies it, is the defaults and leaves the sound as it came.**
#[test]
fn init_applied_over_an_edited_patch_restores_the_identity_chain() {
    let plugin = MxmFxOldify::default();
    let edited = MxmFxOldifyParams::default();
    unsafe {
        let _ = edited.drive._internal_set_normalized_value(0.9);
        let _ = edited.mix._internal_set_normalized_value(0.2);
    }
    let edited_patch = preset::Preset::capture("Edited", preset::Category::Fx, &edited);
    apply(&plugin, &edited_patch, preset::Origin::User);
    assert_ne!(plugin.params.mix.value(), 1.0);

    let init = preset::Preset::init(&*plugin.params);
    apply(&plugin, &init, preset::Origin::User);
    let mut plugin = plugin;
    assert!(plugin.prepare_for_test(FS, 2));
    let input = programme();
    let rendered = render(&mut plugin, &input);
    let left: Vec<f32> = rendered.iter().step_by(2).copied().collect();
    let difference = left
        .iter()
        .zip(&input)
        .fold(0.0f32, |peak, (out, dry)| peak.max((out - dry).abs()));
    // Init writes normalised values, so a gain may land a rounding step off 0 dB.
    assert!(
        difference < 1.0e-5,
        "Init is not the identity: {difference}"
    );
}
