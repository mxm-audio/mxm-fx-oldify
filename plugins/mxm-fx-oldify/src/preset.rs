//! The shared preset-system seam, and the factory bank.
//!
//! **Fifty sounds across both media, the chain of them and past what either really does**, and Init
//! beside them (the owner, 2026-10-12: "50 presets + the init"), designed as overrides on Init in
//! `preset_designs.rs`, the bank's one readable statement, and generated to `presets/` by
//! `examples/fx_oldify_build_presets.rs`. The files are compiled in; the tests below hold them to
//! what the generator emits today, so a design edited without regenerating fails here.
//!
//! **A recipe leaves out Mix and Quality, and only those** (`is_factory_preserved`): loading a
//! factory sound keeps the player's Mix, as mxm-fx-delay's do, and the Quality they chose, so
//! browsing changes neither.

use std::sync::RwLock;

pub use mxm_preset::{
    Category, Entry, INIT_NAME, Library, Loaded, Origin, Preset, PresetIdentity, Refused, Value,
    factory, loaded, mark_loaded, mark_none, read_favourites, snapshot, write_favourites,
};

use crate::params::MxmFxOldifyParams;

impl mxm_preset::Instrument for MxmFxOldifyParams {
    fn clap_id(&self) -> &'static str {
        crate::CLAP_ID
    }

    fn parameters(&self) -> Vec<(&'static str, &dyn mxm_params::ErasedParam)> {
        vec![
            ("record_on", &self.record_on),
            ("disc", &self.disc),
            ("era", &self.era),
            ("player", &self.player),
            ("cut_curve", &self.cut_curve),
            ("replay_curve", &self.replay_curve),
            ("horn", &self.horn),
            ("horn_size", &self.horn_size),
            ("wear", &self.wear),
            ("dynamic_wear", &self.dynamic_wear),
            ("side", &self.side),
            ("groove_distortion", &self.groove_distortion),
            ("eccentricity", &self.eccentricity),
            ("warp", &self.warp),
            ("skips", &self.skips),
            ("stuck_groove", &self.stuck_groove),
            ("crackle", &self.crackle),
            ("crackle_level", &self.crackle_level),
            ("clicks", &self.clicks),
            ("surface_noise", &self.surface_noise),
            ("scratches", &self.scratches),
            ("cracks", &self.cracks),
            ("rumble", &self.rumble),
            ("hum", &self.hum),
            ("hum_level", &self.hum_level),
            ("groove_echo", &self.groove_echo),
            ("tape_on", &self.tape_on),
            ("tape", &self.tape),
            ("grade", &self.grade),
            ("tape_tone", &self.tape_tone),
            ("saturation", &self.saturation),
            ("character", &self.character),
            ("generations", &self.generations),
            ("age", &self.age),
            ("wow", &self.wow),
            ("wow_rate", &self.wow_rate),
            ("wow_sync", &self.wow_sync),
            ("wow_shape", &self.wow_shape),
            ("flutter", &self.flutter),
            ("flutter_rate", &self.flutter_rate),
            ("flutter_sync", &self.flutter_sync),
            ("flutter_shape", &self.flutter_shape),
            ("follow", &self.follow),
            ("flange", &self.flange),
            ("tape_stop", &self.tape_stop),
            ("stop_time", &self.stop_time),
            ("stop_sync", &self.stop_sync),
            ("hiss", &self.hiss),
            ("print_through", &self.print_through),
            ("nr", &self.nr),
            ("nr_decode", &self.nr_decode),
            ("nr_amount", &self.nr_amount),
            ("nr_brightness", &self.nr_brightness),
            ("nr_level_error", &self.nr_level_error),
            ("dropouts", &self.dropouts),
            ("dropout_intensity", &self.dropout_intensity),
            ("dropout_width", &self.dropout_width),
            ("azimuth", &self.azimuth),
            ("crosstalk", &self.crosstalk),
            ("wrong_eq", &self.wrong_eq),
            ("drive", &self.drive),
            ("width", &self.width),
            ("mix", &self.mix),
            ("noise_follows", &self.noise_follows),
            ("noise_stops", &self.noise_stops),
            ("variation", &self.variation),
            ("quality", &self.quality),
        ]
    }

    fn identity(&self) -> &RwLock<PresetIdentity> {
        &self.preset
    }

    /// The installed set, but in a unit test, which is loaded from no bundle: the repository's.
    #[cfg(test)]
    fn factory_files(&self) -> Vec<(String, String)> {
        FACTORY_FILES
            .iter()
            .map(|(name, json)| ((*name).to_owned(), (*json).to_owned()))
            .collect()
    }

    fn default_missing_legacy_parameter(&self, id: &str) -> bool {
        TEMPO_SYNC_IDS.contains(&id)
    }

    /// The live Mix and Quality stay when a factory sound loads; the shared resolver accepts
    /// their absence from a trusted factory recipe only.
    fn is_factory_preserved(&self, id: &str) -> bool {
        PRESERVED.contains(&id)
    }
}

/// **The tempo syncs this plugin gained after its first presets**: Wow's and Flutter's on 2026-10-09,
/// Stop time's on 2026-10-11. A preset written before them was written unsynced, so each loads off
/// rather than keeping the instance's sync, and without reporting a missing control; a project
/// from before them restores them off (`Plugin::filter_state`).
pub(crate) const TEMPO_SYNC_IDS: &[&str] = &["wow_sync", "flutter_sync", "stop_sync"];

/// What a factory recipe leaves at the player's own: Mix, as the collection's effects do, and
/// Quality, how hard the effect works, which is the player's machine's to say, not a sound's.
pub const PRESERVED: [&str; 2] = ["mix", "quality"];

/// The factory set as it ships, `(slug, json)` in its order: the repository's `presets/`, which
/// is installed beside the plugin and read there (`mxm_preset::installed_factory_dir`). For the
/// tests, the integration tests and the audits, which are loaded from no bundle; the plugin itself
/// never reads it.
pub static FACTORY_FILES: std::sync::LazyLock<Vec<(&'static str, &'static str)>> =
    std::sync::LazyLock::new(|| {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("presets");
        mxm_preset::read_factory_dir(&folder)
            .into_iter()
            .map(|(file, json)| {
                let slug = file.strip_suffix(".json").unwrap_or(&file).to_owned();
                (&*String::leak(slug), &*String::leak(json))
            })
            .collect()
    });

#[cfg(test)]
mod tests {
    use super::*;
    use mxm_preset::Instrument;
    use nice_plug::prelude::Params;

    /// **Every stored `text` is this plugin's own formatting of the `v` beside it**
    /// (`mxm_preset::stale_factory_texts`).
    #[test]
    fn every_factory_text_is_this_plugins_formatting_of_its_value() {
        let stale = mxm_preset::stale_factory_texts(&MxmFxOldifyParams::default());
        assert!(
            stale.is_empty(),
            "{} stale texts — regenerate the files:\n{}",
            stale.len(),
            stale.join("\n")
        );
    }

    #[test]
    fn every_parameter_is_declared_once_in_the_preset_seam() {
        let params = MxmFxOldifyParams::default();
        let declared = params.parameters();
        assert_eq!(declared.len(), params.param_map().len());
        for (id, _, _) in params.param_map() {
            assert_eq!(
                declared.iter().filter(|(other, _)| *other == id).count(),
                1,
                "{id}"
            );
        }
    }

    /// **Init is generated from the parameter defaults**, and has no file of its own.
    #[test]
    fn the_init_preset_is_the_parameter_defaults() {
        let params = MxmFxOldifyParams::default();
        let init = Preset::init(&params);
        assert_eq!(init.params.len(), params.parameters().len());
        for (id, param) in params.parameters() {
            assert_eq!(
                init.params[id].v,
                param.default_normalised(),
                "{id} is not its default in Init"
            );
        }
    }

    /// **A project saved before the tempo syncs restores them Off** (`mxm_preset::add_switches_off`),
    /// whatever this instance had.
    #[test]
    fn an_older_state_restores_the_tempo_syncs_off() {
        use nice_plug::prelude::Plugin as _;
        let mut state = nice_plug::prelude::PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        };
        crate::MxmFxOldify::filter_state(&mut state);
        for id in TEMPO_SYNC_IDS {
            assert!(
                matches!(
                    state.params.get(*id),
                    Some(nice_plug::plugin::ParamValue::Bool(false))
                ),
                "{id} was not restored off"
            );
        }
    }

    /// **A preset saved before the tempo syncs loads them off, and cleanly** ([`TEMPO_SYNC_IDS`]).
    #[test]
    fn a_preset_from_before_the_tempo_syncs_loads_them_off() {
        let params = MxmFxOldifyParams::default();
        let mut old = Preset::init(&params);
        for id in TEMPO_SYNC_IDS {
            old.params.remove(*id);
        }
        let (writes, problems) = old.resolve(&params);
        assert!(problems.is_empty(), "{problems:?}");
        for id in TEMPO_SYNC_IDS {
            assert!(
                writes.iter().any(|(w, _, v)| w == id && *v == 0.0),
                "{id} was not written off"
            );
        }
    }

    #[test]
    fn every_shipped_recipe_is_what_the_generator_emits_today() {
        // Counts, uniqueness and completeness all pass on stale files, so nothing else in this
        // suite notices a design edited without rerunning the generator. Compare the committed
        // bytes against a fresh in-memory generation instead.
        let generated = crate::preset_designs::generate();
        assert_eq!(
            generated.len(),
            FACTORY_FILES.len(),
            "the bank and the design table disagree on how many sounds ship"
        );
        let shipped: std::collections::HashMap<_, _> = FACTORY_FILES.iter().copied().collect();
        for (slug, json) in &generated {
            let committed = shipped.get(slug.as_str()).unwrap_or_else(|| {
                panic!("{slug} is in the design table but no such file is compiled in")
            });
            if cfg!(target_os = "windows") {
                assert_eq!(
                    committed.replace("\r\n", "\n"),
                    *json,
                    "{slug}.json is stale: rerun `cargo run -p mxm-fx-oldify --release \
                     --example fx_oldify_build_presets`"
                );
            } else {
                let parse = |text: &str| Preset::parse(text, crate::CLAP_ID).unwrap();
                assert_same_within_rounding(&parse(committed), &parse(json), slug);
            }
        }
    }

    /// A shipped file against a fresh generation with each value compared within rounding,
    /// everything else exactly. The files hold Windows' bits, and each platform's maths library
    /// rounds in its own way (the owner, 2026-10-06: pin on Windows only).
    fn assert_same_within_rounding(shipped: &Preset, generated: &Preset, slug: &str) {
        for (id, value) in &shipped.params {
            let fresh = generated
                .params
                .get(id)
                .unwrap_or_else(|| panic!("{slug}.json is stale: {id} is no longer generated"));
            assert!(
                (value.v - fresh.v).abs() <= 1.0e-6,
                "{slug}.json is stale: {id} is {} in the file and {} when generated",
                value.v,
                fresh.v
            );
        }
        let (mut shipped, mut generated) = (shipped.clone(), generated.clone());
        for value in shipped
            .params
            .values_mut()
            .chain(generated.params.values_mut())
        {
            value.v = 0.0;
            value.text.clear();
        }
        assert_eq!(shipped, generated, "{slug}.json is stale");
    }

    /// **The bank is the design table, every sound complete, named once and different.**
    #[test]
    fn the_factory_bank_is_complete_and_every_sound_its_own() {
        use std::collections::BTreeSet;
        let params = MxmFxOldifyParams::default();
        assert_eq!(FACTORY_FILES.len(), crate::preset_designs::DESIGNS.len());
        let (mut names, mut fingerprints) = (BTreeSet::new(), BTreeSet::new());
        for (slug, json) in FACTORY_FILES.iter() {
            let preset = Preset::parse(json, crate::CLAP_ID).unwrap();
            assert!(
                names.insert(preset.name.clone()),
                "{slug}: a second {}",
                preset.name
            );
            assert_eq!(preset.category, Category::Fx, "{slug}");
            for id in PRESERVED {
                assert!(!preset.params.contains_key(id), "{slug} names {id}");
            }
            assert_eq!(
                preset.params.len(),
                params.parameters().len() - PRESERVED.len(),
                "{slug} is incomplete"
            );
            let fingerprint: Vec<_> = preset
                .params
                .iter()
                .map(|(id, value)| (id.clone(), value.v.to_bits()))
                .collect();
            assert!(
                fingerprints.insert(fingerprint),
                "{slug} repeats another sound"
            );
        }
    }

    #[test]
    fn mix_and_quality_are_the_only_factory_preserved_parameters() {
        let params = MxmFxOldifyParams::default();
        for (id, _) in params.parameters() {
            assert_eq!(
                params.is_factory_preserved(id),
                matches!(id, "mix" | "quality"),
                "{id}"
            );
        }
    }

    /// **Init is complete, but a trusted factory recipe may leave out Mix, and only Mix**: a user's
    /// or a bank's preset without it reports the missing control.
    #[test]
    fn init_is_complete_but_a_trusted_factory_recipe_may_omit_only_mix() {
        let params = MxmFxOldifyParams::default();
        let all = params.parameters().len();
        assert_eq!(Preset::init(&params).params.len(), all);
        let mut recipe = Preset::capture("Recipe", Category::Fx, &params);
        recipe.params.remove("mix");
        let (writes, problems) = recipe.resolve_from(&params, Some(&Origin::Factory));
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(writes.len(), all - 1);
        let (_, user_problems) = recipe.resolve_from(&params, Some(&Origin::User));
        assert_eq!(user_problems.len(), 1);
        assert!(user_problems[0].contains("mix"));
    }

    #[test]
    fn control_map_names_only_live_parameters_and_claims_no_role_yet() {
        let params = MxmFxOldifyParams::default();
        let live: std::collections::BTreeSet<_> =
            params.parameters().into_iter().map(|(id, _)| id).collect();
        let map: serde_json::Value =
            serde_json::from_str(include_str!("../control-map.json")).unwrap();
        let instrument = &map["instruments"][0];
        assert_eq!(instrument["clap_id"], crate::CLAP_ID);
        let mappings = instrument["params"].as_object().unwrap();
        for (role, id) in mappings {
            let id = id.as_str().unwrap();
            assert!(live.contains(id), "{role} points at missing `{id}`");
        }
        assert!(
            mappings.is_empty(),
            "a role was claimed: say why in `plugins/mxm-fx-oldify/AGENTS.md` and update this test"
        );
    }
}
