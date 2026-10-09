//! `mxm-fx-oldify` through the player's ordinary discovery and serial effect chain.
//!
//! Product DSP remains in its own crate. These checks hold the shipping boundary: the bundle is
//! classified as an effect, its sidecar map loads, the default sound passes the source through to
//! the bit while the engines are not built, and player bypass restores dry audio to the bit.

use mxm_player_harness::app_harness;
use mxm_player_harness::harness;

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use harness::{CHANNELS, Harness, mxm_mono_01};
use mxm_player::events::input::Payload;

const SOURCE_ID: &str = "dk.mxm.mxm-mono-01";
const EFFECT_ID: &str = "dk.mxm.mxm-fx-oldify";
const BLOCK: usize = 256;
// A held note and its release, with room after it: nothing here has a tail.
const BLOCKS: usize = 200;

fn oldify() -> Option<PathBuf> {
    let path = app_harness::workspace_root().join("target/bundled/mxm-fx-oldify.clap");
    if path.exists() {
        Some(path)
    } else {
        eprintln!(
            "skipping: {} is missing — run `cargo xtask bundle mxm-fx-oldify --release`",
            path.display()
        );
        None
    }
}

#[test]
fn ordinary_discovery_classifies_the_bundle_and_loads_its_map() {
    let Some(bundle) = oldify() else {
        return;
    };
    let directory = bundle.parent().unwrap().to_owned();
    let mut app = app_harness::AppHarness::new("fx-oldify-discovery", vec![directory]);
    app.harness.state_mut().rescan();
    app.run();
    let state = app.state();
    let found = state
        .found
        .iter()
        .find(|found| found.id == EFFECT_ID)
        .unwrap_or_else(|| panic!("ordinary discovery did not find {EFFECT_ID}"));
    assert!(
        found.effect,
        "the one-input bundle was not classified as an effect"
    );
    assert!(
        app.app().control_map().knows_instrument(EFFECT_ID),
        "the control-map sidecar was not loaded"
    );
}

fn render(with_effect: bool, bypassed: bool) -> Option<Vec<f32>> {
    let source = mxm_mono_01()?;
    let effect = oldify()?;
    let chain: Vec<(&Path, &str)> = if with_effect {
        vec![(effect.as_path(), EFFECT_ID)]
    } else {
        vec![]
    };
    let mut harness = Harness::with_fx(&source, SOURCE_ID, 1, &chain).expect("the chain builds");
    if let Some(effect) = harness.fx.first() {
        effect.bypassed.store(bypassed, Ordering::Relaxed);
    }

    let mut output = Vec::with_capacity(BLOCKS * BLOCK * CHANNELS);
    for block in 0..BLOCKS {
        if block == 2 {
            assert!(harness.push_at(
                0,
                0,
                Payload::NoteOn {
                    channel: 0,
                    key: 60,
                    velocity: 100.0 / 127.0,
                },
            ));
        }
        if block == 28 {
            assert!(harness.push_at(
                0,
                0,
                Payload::NoteOff {
                    channel: 0,
                    key: 60,
                    velocity: 0.0,
                },
            ));
        }
        output.extend_from_slice(harness.render(BLOCK));
    }
    harness.shutdown();
    Some(output)
}

/// **Until the engines exist, the default Oldify is out of the way**: the chain hears the source
/// to the bit. When the engines land, this is the test that changes, on purpose.
#[test]
fn the_default_passes_the_source_through_to_the_bit() {
    let Some(dry) = render(false, false) else {
        return;
    };
    let wet = render(true, false).expect("the bundle existed above");
    assert!(dry.iter().any(|sample| *sample != 0.0));
    assert!(wet.iter().all(|sample| sample.is_finite()));
    assert_eq!(wet, dry);
}

#[test]
fn player_bypass_restores_dry_audio_to_the_bit() {
    let Some(dry) = render(false, false) else {
        return;
    };
    let bypassed = render(true, true).expect("the bundle existed above");
    assert!(dry.iter().any(|sample| *sample != 0.0));
    assert_eq!(bypassed, dry);
}

/// The effect's own control map, read through the player's `ControlMap` as a host reads it.
mod control_map {
    use mxm_player::control_map::ControlMap;
    use std::path::PathBuf;

    /// **The map loads and claims no role**: no role in the standard is a quantity this effect
    /// has (`plugins/mxm-fx-oldify/AGENTS.md`, *Control map*).
    #[test]
    fn the_map_loads_and_claims_no_role() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../control-map.json");
        let mut map = ControlMap::shipped();
        map.load_instrument_map(&path)
            .expect("mxm-fx-oldify's map loads");

        let id = "dk.mxm.mxm-fx-oldify";
        assert!(map.knows_instrument(id));
        for role in [
            "fx.delay",
            "fx.reverb",
            "fx.dynamics",
            "fx.chorus_mix",
            "voice.output",
            "filter.drive",
        ] {
            assert!(map.param_for(id, role).is_none(), "{role} was claimed");
        }
    }
}
