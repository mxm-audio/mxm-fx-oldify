# AGENTS.md — plugins/mxm-fx-oldify

Parent: [`../AGENTS.md`](../AGENTS.md)

# Purpose

The `mxm-fx-oldify` CLAP effect: permanent parameters and identity, latency and tail reporting,
the preset seam, control map, telemetry and five-card editor around
[`crates/mxm-fx-oldify-dsp`](../../crates/mxm-fx-oldify-dsp/AGENTS.md). **The cassette engine is
in; the record engine is to come**, and so are the brief's later controls (tempo sync, noise
reduction, dropouts, Flange, Variation and the rest).

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
- The parameter ids and every option id below are permanent: a stored enum is its id. Never rename
  or recycle one. Declaration order is signal order, the cards'.
- CLAP features: audio-effect, distortion, stereo. Layouts: mono → stereo and stereo → stereo; a
  mono input feeds both sides. There is no note port and no developer MIDI channel.

| Id | Name | Range, law and option ids | Does |
|---|---|---|---|
| `medium` | Medium | Cassette / Record / Record then cassette (`cassette`, `record`, `record-then-cassette`) | Which medium. Record has no engine yet and passes the input delayed by the latency; Record then cassette is the cassette alone for now |
| `age` | Age | 0–100 %, linear | The cassette's spacing loss: the top falls away |
| `tape` | Tape | Type I / Type II / Type IV (`type-1`, `type-2`, `type-4`) | The tape's response and emphasis; Types II and IV hiss less |
| `grade` | Grade | Economy / Standard / High / Reference (`economy`, `standard`, `high`, `reference`) | With Tape, one of the twelve measured responses |
| `tape_tone` | Tape tone | 0–100 %, linear | How much of that response is heard |
| `saturation` | Saturation | 0–100 %, linear | The tape's saturation; zero is linear |
| `character` | Character | Soft / Hard (`soft`, `hard`) | The saturation's curve |
| `wow` | Wow | 0–100 %, linear | Slow speed wobble's depth |
| `wow_rate` | Wow rate | `cassette::WOW_HZ`, skewed so its geometric mean is mid-travel, read in Hz | Its rate |
| `wow_shape` | Wow shape | Sine / Random / Saw / Reverse saw (`sine`, `random`, `saw`, `reverse-saw`) | Its shape |
| `flutter` | Flutter | 0–100 %, linear | Fast speed wobble's depth |
| `flutter_rate` | Flutter rate | `cassette::FLUTTER_HZ`, skewed as Wow rate | Its rate |
| `flutter_shape` | Flutter shape | Sine / Random (`sine`, `random`) | Its shape |
| `hiss` | Hiss | `HISS_OFF_DB`…`HISS_MAX_DB`, stored as linear gain, `gain_skew_factor`; reads Off at the bottom | The tape's hiss, gated by the input |
| `drive` | Drive | `MIN_GAIN_DB`…`MAX_GAIN_DB`, stored as linear gain, 0 dB mid-travel, read in dB | How hard the tape is driven, compensated after it |
| `width` | Width | 0…`MAX_WIDTH`, linear, read in percent; 100 % is the image as it came | Mid/side width |
| `output` | Output | As Drive | The level out |
| `mix` | Mix | 0–100 %, linear | Dry/wet crossfade against the dry input delayed by the latency |

The bounds are the DSP crate's constants, so the plugin's ranges and the DSP's clamps are one set
of numbers. **Hiss is a level, so it stores linear gain** (the collection's rule) and
`params::hiss_db` converts it, returning `HISS_OFF_DB` — off in the DSP — wherever the reading
rounds to it, so the knob never reads Off while noise plays (`hiss_reads_off_at_the_bottom_and_off_plays_nothing`).

## Init: the engaged cassette

**The defaults are the DSP's init patch, `Controls::INIT`**, read from it in
`MxmFxOldifyParams::default`; the switches' defaults are written as choices, and
`the_defaults_are_the_dsp_init_patch` holds the whole set equal. Init is generated from them. The
init patch is an audible cassette, so *an effect starts engaged* is met
(`the_defaults_are_audible_and_a_mono_input_feeds_both_sides`).

## Latency and tail

- **The chain has one fixed latency**, `Oldify::latency()`, set by the sample rate alone: the same
  for every control and Medium. `activate` builds the engine (it allocates its delay lines there,
  never on the audio thread) and reports it with `ActivateContext::set_latency_samples`, once; it
  never changes while processing (`activation_reports_the_chains_latency_at_the_hosts_rate`). Mix
  zero is the dry input delayed by it (`mix_zero_is_the_dry_input_delayed_by_the_latency`).
- **The tail is the DSP's**, `Oldify::tail()`: the shell counts the samples since the input was
  last above the DSP's `SILENCE`, as the DSP counts them, and reports `Tail(n)` with what is left
  until the output is exact zero, then `Normal` once it is, so a host may sleep the effect
  (`the_tail_counts_down_to_exact_silence_and_then_the_effect_may_sleep`). A quiet chain fed
  silence costs the DSP's input scan only.

## Smoothing lives in the DSP

The parameters carry **no nice-plug smoother**: the DSP ramps every continuous control's block
target over its `SMOOTHING_S` (`a_moved_control_ramps_and_is_heard` holds it through the shell),
and takes a switch as it comes. Sample-accurate automation splits a block at each event, so a target
still lands on its sample. A second smoother here would stack two ramps on one control.

## Not yet, and the deviations that come with it

- **Wow rate and Flutter rate have no tempo sync yet.** The collection gives every rate a *<Control>
  sync*; these get theirs with the brief's sync controls, later. A recorded deviation until then.
- **The Record medium passes the input** until the record engine exists:
  `the_record_medium_passes_the_input_delayed_until_its_engine_exists` changes on purpose then.
- The brief's other controls (phase, Flange, Dropouts, noise reduction, Variation, Generations, the
  Deck card) are not parameters yet; each id is fixed when its control arrives.

## Presets

Through `mxm-preset` (`impl mxm_preset::Instrument` in `src/preset.rs`): the browser, the generated
Init, user presets, banks and favourites all work. **The factory bank is empty until its sounds are
designed by ear** (the conventions let an effect ship what its brief justifies):
`the_bank_is_empty_until_its_sounds_are_designed` pins that, and
`every_shipped_recipe_loads_completely_and_renders_finite_without_clipping` already runs over
whatever ships. The first sounds bring what mxm-fx-delay's bank has: a design table, the generator
example and the test that the committed files are what it emits today. Whether a factory sound
preserves the live Mix is decided with the bank; nothing is factory-preserved now.

## Control map

`control-map.json` **claims no role**, and its comment says why: no role the standard declares is
a quantity this effect has. Mix is how much of the aged sound is heard, not how much delay
(`fx.delay`), reverb (`fx.reverb`), curve stack (`fx.dynamics`) or chorus (`fx.chorus_mix`);
Output is an effect's level, not an instrument's (`voice.output`, on the Voice page); Drive feeds
the tape, not a filter (`filter.drive`); Wow and Flutter are a tape's speed, not a delay line's
wobble (`fx.delay_wobble`, on the Delay page). Roles of its own are a collection-wide change to
mxm-kit's `docs/MXM_CONTROL_MAP.md` — a new page — and the owner's to rule on. Held by
`control_map_names_only_live_parameters_and_claims_no_role_yet` and the host test
`the_map_loads_and_claims_no_role`.

## Editor and telemetry

Five Effects cards in signal order, each **under the brief's stable key** (`sections::KEYS`), so
the record engine's cards take their places without moving these: **Medium** (Medium, Age),
**Tape** (Tape, Grade, Tape tone and Saturation, Character), **Transport** (Wow and its rate, its
shape; Flutter and its rate, its shape), **Noise** (Hiss) and **Output** (the In and Out levels,
then Drive, Width, Output, Mix). All five show whatever Medium is: whether an absent engine's cards
disappear is the brief's open question, settled with the record engine's cards. The shapes are
drawn, not named; every switch cell carries its own sentence (`details_of`); Drive, Width and
Output are bipolar knobs.

Every card body is a `mxm_ui::tree` (`editor/sections.rs`), its floor computed by
`tree::card_floor`; there is no declared usability minimum. The window opens at the quarter-4K
budget hugged (`REFERENCE`, `the_opening_size_is_the_budget_hugged`), every card on the opening
page (`opening_page_shows_every_card`, which looks each card up by its key:
`paging_checks::all_rects` counts keys from zero). Its minimum (`MINIMUM`) is one card wide and one
row tall (`the_window_contract_fits_the_quarter_4k_budget_and_widest_card`,
`every_dynamic_page_fits_and_every_card_is_reachable`, `the_app_bar_holds_in_the_minimum_window`).
The cursor reaches and operates exactly every parameter
(`the_keyboard_cursor_reaches_and_operates_every_parameter`), and every card passes the tree checks
at Init and with every parameter at each end of its travel and full levels.

`src/telemetry.rs` is the only audio-to-editor channel: the input and output peaks, max-combined
until read, and the output's clip, latched until acknowledged. The app bar's meter shows the output;
the Output card's display shows both. Telemetry may lose frames and never affects sound.

# Work Guidance

- Engines and their behaviour stay in the DSP crate; this shell converts host state and units.
- A new control fixes its id here, in signal order, in `preset.rs`'s list and in its card, and
  `the_defaults_are_the_dsp_init_patch` keeps its default the DSP's.
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
any change to `control-map.json` or the standard. Its default-sound test judges the source's end by
the DSP's `SILENCE`, as the effect does.

# Child DOX Index

No child `AGENTS.md` files.
