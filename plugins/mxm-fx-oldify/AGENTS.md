# AGENTS.md — plugins/mxm-fx-oldify

Parent: [`../AGENTS.md`](../AGENTS.md)

# Purpose

The `mxm-fx-oldify` CLAP effect: permanent parameters and identity, the preset seam, control map,
telemetry and two-card editor around [`crates/mxm-fx-oldify-dsp`](../../crates/mxm-fx-oldify-dsp/AGENTS.md).
**The cassette and record engines are not built yet**; this is the shell they will arrive in.

UI brief: [`../../docs/briefs/mxm-fx-oldify.md`](../../docs/briefs/mxm-fx-oldify.md). The design and
measurements are private: `ops:notes/oldify/` (root *The design and its measurements are private*).

# Ownership

Owns `Cargo.toml`, `README.md`, `control-map.json`, `tests/`, `host-tests/` and `src/`; there is
no `presets/` until a factory sound ships. Its licence is the repository's root `LICENSE`. DSP
remains in the framework-free product crate; visual geometry remains in `mxm-ui`.

# Local Contracts

## Permanent identity and parameters

- Product and bundle name: `mxm-fx-oldify`; CLAP id: `dk.mxm.mxm-fx-oldify`.
- The sole crate-local name literal is `plugin_name!` in `src/lib.rs`; `bundler.toml` is the only
  external duplicate.
- Permanent parameter ids: `medium`, `age`, `drive`, `width`, `output`, `mix`. Medium's option ids
  `cassette`, `record` and `record-then-cassette` are permanent too: a stored enum is its id.
  Never rename or recycle one.
- CLAP features: audio-effect, distortion, stereo. Layouts: mono → stereo and stereo → stereo; a
  mono input feeds both sides. There is no note port and no developer MIDI channel.

| Id | Name | Range and law | Does today |
|---|---|---|---|
| `medium` | Medium | Cassette / Record / Record then cassette | Nothing yet |
| `age` | Age | 0–100 %, linear | Nothing yet |
| `drive` | Drive | `MIN_GAIN_DB`…`MAX_GAIN_DB`, stored as linear gain, `gain_skew_factor` so 0 dB is mid-travel, read in dB | The level into the chain |
| `width` | Width | 0…`MAX_WIDTH`, linear, read in percent; 100 % is the image as it came | Mid/side width |
| `output` | Output | As Drive | The level out of the chain |
| `mix` | Mix | 0–100 %, linear | Dry/wet crossfade; zero is the dry input to the bit |

The bounds are the DSP crate's constants, so the plugin's ranges and the DSP's clamps are one set
of numbers (`mxm_fx_oldify_dsp::db_to_gain`).

## Medium and Age do nothing yet

No code reads them: they exist now so their ids are fixed before anything is saved with them. Three
tests say so and **change on purpose when the engines land**:
`medium_and_age_are_inaudible_until_the_engines_exist` (`tests/plugin_integration.rs`),
`the_defaults_pass_the_input_to_the_bit_in_both_layouts` (`src/lib.rs`) and the host test
`the_default_passes_the_source_through_to_the_bit`. Their tooltips already say what they are for,
written for the player; the panel carries no note that they are inert, because the panel carries no
help text (design system §7.6).

## Init, and an effect that does not yet start engaged

Init is the CLAP defaults, generated: the identity chain (both gains at 0 dB, Width at the image as
it came, Mix fully wet) on a Cassette at `params::DEFAULT_AGE`
(`init_is_the_identity_chain_on_a_cassette_at_thirty_percent_age`,
`the_init_preset_is_the_parameter_defaults`). **Deviation from *an effect starts engaged*, until the
engines exist:** Medium and Age are the engaged setting, but nothing hears them, so an inserted
Oldify passes its input through to the bit. The deviation ends when the engines land; the brief
then sets how modest the engaged default is.

## Smoothing lives in the DSP

The parameters carry **no nice-plug smoother**: the DSP ramps each block's targets over its
`SMOOTHING_S`, one ramp per control, which `a_moved_control_ramps_and_is_heard` holds through the
shell. Sample-accurate automation splits a block at each event, so a target still lands on its
sample. A second smoother here would stack two ramps on one control. Age's smoothing is decided
with the engines, by what it moves.

## Presets

Through `mxm-preset` (`impl mxm_preset::Instrument` in `src/preset.rs`): the browser, the generated
Init, user presets, banks and favourites all work. **The factory bank is empty until the engines
exist** (the conventions let an effect ship what its brief justifies): a factory sound is a Medium
and an Age heard on purpose, and neither moves anything yet. `the_bank_is_empty_until_the_engines_exist`
pins that, and `every_shipped_recipe_loads_completely_and_renders_finite_without_clipping` already
runs over whatever ships. The first sounds bring what mxm-fx-delay's bank has: a design table, the
generator example and the test that the committed files are what it emits today. Whether a
factory sound preserves the live Mix is decided with the bank; nothing is factory-preserved now.

## Control map

`control-map.json` **claims no role**, and its comment says why: no role the standard declares is
a quantity this effect has. Mix is how much of the aged sound is heard, not how much delay
(`fx.delay`), reverb (`fx.reverb`), curve stack (`fx.dynamics`) or chorus (`fx.chorus_mix`);
Output is an effect's level, not an instrument's (`voice.output`, on the Voice page); Drive feeds
the medium, not a filter (`filter.drive`). Roles of its own are a collection-wide change to mxm-kit's
`docs/MXM_CONTROL_MAP.md` — a new page — and the owner's to rule on. Held by
`control_map_names_only_live_parameters_and_claims_no_role_yet` and the host test
`the_map_loads_and_claims_no_role`.

## Realtime and activity

Nothing allocates, locks or logs in `process`. No state outlives the input, so the status is always
`Normal` and there is never a tail. Doing nothing costs nothing: silence, Mix settled at zero and
the identity settled cost the DSP's input scan only (its contract).

## Editor and telemetry

Two stable Effects cards in signal order: **Medium** (Medium's switch, Age) and **Output** (the In
and Out levels, then Drive, Width, Output, Mix). They use shared dynamic paging, controls, the
preset app bar, keyboard cursor, themes and zoom. Every card body is a `mxm_ui::tree`
(`editor/sections.rs`), its floor computed by `tree::card_floor`; there is no declared usability
minimum. The window opens at the quarter-4K budget hugged (`REFERENCE`,
`the_opening_size_is_the_budget_hugged`); its minimum (`MINIMUM`) is one card wide — Output's, the
widest floor — and one row tall (`the_window_contract_fits_the_quarter_4k_budget_and_widest_card`,
`every_dynamic_page_fits_and_every_card_is_reachable`, `the_app_bar_holds_in_the_minimum_window`).
Drive, Width and Output are bipolar knobs, their rest at the centre. Each of Medium's cells carries
its own sentence (`details_of`). The cursor reaches and operates exactly the six parameters
(`the_keyboard_cursor_reaches_and_operates_every_parameter`), and every card passes the tree checks
at Init and at the longest readings with full levels.

`src/telemetry.rs` is the only audio-to-editor channel: the input and output peaks, max-combined
until read, and the output's clip, latched until acknowledged. The app bar's meter shows the output;
the Output card's display shows both. Telemetry may lose frames and never affects sound.

# Work Guidance

- The engines go in the DSP crate, between Drive and Width; this shell converts host state and units.
- A compile or a render does not approve a sound. Owner listening, native design-system §15 in
  both themes and one real DAW at a small buffer remain manual, and are reported as such.

# Verification

```bash
cargo test -p mxm-fx-oldify
cargo clippy -p mxm-fx-oldify --all-targets -- -D warnings
MXM_PICTURES=after cargo test -p mxm-fx-oldify --lib tree_pictures -- --ignored   # target/layout-tree/mxm-fx-oldify/after/
cargo xtask bundle mxm-fx-oldify --release
clap-validator validate target/bundled/mxm-fx-oldify.clap
cargo test -p mxm-fx-oldify-host-tests --test effect_chain
```

The host test's `control_map` module holds this plugin's map as the player reads it; run it after
any change to `control-map.json` or the standard.

# Child DOX Index

No child `AGENTS.md` files.
