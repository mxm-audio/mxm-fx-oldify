//! The shared preset-system seam, with **no factory sounds yet**.
//!
//! A factory sound is a medium heard on purpose and designed by ear; the cassette engine is new and
//! the record engine is still to come, so the bank is empty until its sounds are designed
//! (`plugins/mxm-fx-oldify/AGENTS.md`). Init, user presets, banks and favourites all work now:
//! they are the shared system, and need no factory set.

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

    fn parameters(&self) -> Vec<(&'static str, &dyn mxm_preset::ErasedParam)> {
        vec![
            ("medium", &self.medium),
            ("age", &self.age),
            ("variation", &self.variation),
            ("tape", &self.tape),
            ("grade", &self.grade),
            ("tape_tone", &self.tape_tone),
            ("saturation", &self.saturation),
            ("character", &self.character),
            ("generations", &self.generations),
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
            ("hiss", &self.hiss),
            ("dropouts", &self.dropouts),
            ("dropout_intensity", &self.dropout_intensity),
            ("dropout_width", &self.dropout_width),
            ("azimuth", &self.azimuth),
            ("crosstalk", &self.crosstalk),
            ("wrong_eq", &self.wrong_eq),
            ("drive", &self.drive),
            ("width", &self.width),
            ("output", &self.output),
            ("mix", &self.mix),
            ("noise_follows", &self.noise_follows),
            ("noise_stops", &self.noise_stops),
        ]
    }

    fn identity(&self) -> &RwLock<PresetIdentity> {
        &self.preset
    }

    fn factory_files(&self) -> &'static [(&'static str, &'static str)] {
        FACTORY_FILES
    }

    fn default_missing_legacy_parameter(&self, id: &str) -> bool {
        TEMPO_SYNC_IDS.contains(&id)
    }
}

/// **The tempo syncs this plugin gained on 2026-10-09**. A preset written before them was written
/// unsynced, so each loads off rather than keeping the instance's sync, and without reporting a
/// missing control; a project from before them restores them off (`Plugin::filter_state`).
pub(crate) const TEMPO_SYNC_IDS: &[&str] = &["wow_sync", "flutter_sync"];

/// The compiled-in factory sounds: none until they are designed by ear.
pub const FACTORY_FILES: &[(&str, &str)] = &[];

#[cfg(test)]
mod tests {
    use super::*;
    use mxm_preset::Instrument;
    use nice_plug::prelude::Params;

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
