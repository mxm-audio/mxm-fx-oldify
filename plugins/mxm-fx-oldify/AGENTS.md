# AGENTS.md — plugins/mxm-fx-oldify

Parent: [`../AGENTS.md`](../AGENTS.md)

# Purpose

The `mxm-fx-oldify` CLAP effect: permanent parameters and identity, tempo sync, latency and tail
reporting, the preset seam, control map, telemetry and six-card editor around
[`crates/mxm-fx-oldify-dsp`](../../crates/mxm-fx-oldify-dsp/AGENTS.md). **The cassette engine is
in, with its noise, dropouts and deck faults; still to come are noise reduction, print-through,
tape stop, Quality and the record engine.**

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

| Card | Id | Name | Range, law and option ids |
|---|---|---|---|
| Medium | `medium` | Medium | Cassette / Record / Record then cassette (`cassette`, `record`, `record-then-cassette`). Record has no engine yet and passes the input delayed; Record then cassette is the cassette alone for now |
| | `age` | Age | 0–100 %, linear |
| | `variation` | Variation | Integer 0…`MAX_VARIATION`: the seed every random source starts from |
| Tape | `tape` | Tape | Type I / Type II / Type IV (`type-1`, `type-2`, `type-4`) |
| | `grade` | Grade | Economy / Standard / High / Reference (`economy`, `standard`, `high`, `reference`) |
| | `tape_tone` | Tape tone | 0–100 %, linear |
| | `saturation` | Saturation | 0–100 %, linear; zero is linear |
| | `character` | Character | Soft / Hard (`soft`, `hard`) |
| | `generations` | Generations | Integer 1…`MAX_GENERATIONS`, drawn as buttons |
| Transport | `wow`, `flutter` | Wow, Flutter | 0–100 %, linear |
| | `wow_rate`, `flutter_rate` | Wow rate, Flutter rate | `cassette::WOW_HZ`, `FLUTTER_HZ`, skewed so the geometric mean is mid-travel, read in Hz |
| | `wow_sync`, `flutter_sync` | Wow sync, Flutter sync | Off / On: the rate beside it on `params::WOW_SYNC` / `FLUTTER_SYNC` |
| | `wow_shape` | Wow shape | Sine / Random / Saw / Reverse saw (`sine`, `random`, `saw`, `reverse-saw`) |
| | `flutter_shape` | Flutter shape | Sine / Random (`sine`, `random`) |
| | `follow` | Flutter follows wow | Off / On |
| | `flange` | Flange | 0–100 %, linear |
| Noise | `hiss` | Hiss | `HISS_OFF_DB`…`HISS_MAX_DB`, stored as linear gain, `gain_skew_factor`; Off at the bottom |
| | `dropouts`, `dropout_intensity`, `dropout_width` | Dropouts, Dropout intensity, Dropout width | 0–100 %, linear |
| Deck | `azimuth` | Azimuth | 0…`MAX_AZIMUTH_ARCMIN` arc-minutes, read `15.0′` (`'` and `arcmin` parse too), skewed so 15′ is mid-travel |
| | `crosstalk` | Crosstalk | `CROSSTALK_OFF_DB`…`CROSSTALK_MAX_DB`, as Hiss |
| | `wrong_eq` | Wrong EQ | Off / On |
| Output | `drive` | Drive | `MIN_GAIN_DB`…`MAX_GAIN_DB`, stored as linear gain, 0 dB mid-travel, read in dB; compensated after the tape |
| | `width` | Width | 0…`MAX_WIDTH`, linear, read in percent; 100 % is the image as it came |
| | `output` | Output | As Drive |
| | `mix` | Mix | 0–100 %, linear, against the dry input delayed by the latency |
| | `noise_follows` | Noise follows | 0–100 %, linear |
| | `noise_stops` | Noise stops in silence | Off / On |

The bounds are the DSP crate's constants, so the plugin's ranges and the DSP's clamps are one set
of numbers. **A level with an Off stores linear gain** (the collection's rule for a level), Hiss and
Crosstalk alike: `params::hiss_db` and `crosstalk_db` convert it, giving the DSP its off level
wherever the reading rounds to Off — so the knob never reads Off while the level plays — and its
decibels to a hundredth otherwise, so a whole-decibel default reaches the DSP exactly
(`a_level_reads_off_at_the_bottom_and_off_plays_nothing`).

## Init: the engaged cassette

**The defaults are the DSP's init patch, `Controls::INIT`**, read from it in
`MxmFxOldifyParams::default`; the switches' defaults are written as choices, and
`the_defaults_are_the_dsp_init_patch` holds the whole set equal. Init is generated from them. The
init patch is an audible cassette, so *an effect starts engaged* is met
(`the_defaults_are_audible_and_a_mono_input_feeds_both_sides`).

## Tempo sync

Wow rate and Flutter rate keep the collection's one sync interface, as mxm-fx-delay's Rate: one
*sync* switch, drawn as the quarter note beside its rate, default Off. **Each ladder is its rate's
own range at 120 bpm** (`each_sync_ladder_is_its_rates_own_range_at_120_bpm`), the top the fastest:
these are a tape's speed wobbles, not an LFO, and Flutter's range runs past `Span::LFO`'s.
`MxmFxOldifyParams::controls(tempo)` resolves each synced rate every block from the host's tempo and
the rate's modulated position (`Ladder::resolve`, clamped into the rate's range), and the free knob
without a tempo; the DSP ramps the result like any turned knob. **Nothing resolved is kept**, so
nothing goes stale: an activation forgets the tempo, and a reset resolves at the last tempo seen
(`the_tempo_reaches_the_editor_and_an_activation_forgets_it`,
`a_reset_resolves_the_syncs_at_the_last_tempo_seen`). Synced, the knob reads the division in force
against the tempo published in the telemetry, its column holding every reading. An older preset
loads both syncs Off and an older project restores them Off (`preset::TEMPO_SYNC_IDS`,
`mxm_preset::add_switches_off` in `filter_state`).

## Latency and tail

- **The chain has one fixed latency**, `Oldify::latency()`, set by the sample rate alone. `activate`
  builds the engine (it allocates its delay lines there, never on the audio thread) and reports it
  with `ActivateContext::set_latency_samples`, once; it never changes while processing
  (`activation_reports_the_chains_latency_at_the_hosts_rate`). Mix zero is the dry input delayed by
  it (`mix_zero_is_the_dry_input_delayed_by_the_latency`).
- **The tail is the DSP's.** While `Oldify::endless()` — Noise stops in silence off, with hiss on —
  the status is **`KeepAlive`**, the one an effect that never falls silent reports, never `Normal`
  and never a finite tail. Otherwise the shell counts the samples since the input was last above
  the DSP's `SILENCE`, as the DSP counts them, and reports `Tail(n)` with what is left of
  `Oldify::tail()` until the output is exact zero, then `Normal`, so a host may sleep the effect
  (`the_tail_counts_down_to_exact_silence_and_then_the_effect_may_sleep`,
  `an_endless_hiss_keeps_the_effect_alive_until_the_noise_stops`).

## Smoothing lives in the DSP

The parameters carry **no nice-plug smoother**: the DSP ramps every continuous control's block
target over its `SMOOTHING_S` (`a_moved_control_ramps_and_is_heard` holds it through the shell),
and takes a switch or a count as it comes. Sample-accurate automation splits a block at each event,
so a target still lands on its sample.

## Still to come

- **The record engine.** The Record medium passes the input until then:
  `the_record_medium_passes_the_input_delayed_until_its_engine_exists` changes on purpose then.
- Noise reduction, print-through, tape stop and Quality are not parameters yet; each id is fixed
  when its control arrives.

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

Six Effects cards in signal order, each **under the brief's stable key** (`sections::KEYS`), so the
record engine's cards take keys 1–3 without moving these: **Medium** (Medium; Age, Variation),
**Tape** (Tape; Grade; Tape tone, Saturation; Character; Generations), **Transport** (Wow, Wow rate
and its sync; Wow shape; Flutter, Flutter rate and its sync; Flutter shape; Flutter follows wow;
Flange), **Noise** (Hiss; Dropouts, Dropout intensity, Dropout width), **Deck** (Azimuth,
Crosstalk; Wrong EQ) and **Output** (the In and Out levels; Drive, Width, Output, Mix; Noise
follows; Noise stops in silence). All six show whatever Medium is: whether an absent engine's cards
disappear is the brief's open question, settled with the record engine's cards. The shapes are
drawn, not named; every switch cell carries its own sentence (`details_of`); Drive, Width and
Output are bipolar knobs; the on/off parameters are the collection's labelled toggle.

Every card body is a `mxm_ui::tree` (`editor/sections.rs`), its floor computed by
`tree::card_floor`; there is no declared usability minimum. The window opens at the quarter-4K
budget hugged (`REFERENCE`, `the_opening_size_is_the_budget_hugged`), every card on the opening
page (`opening_page_shows_every_card`, which looks each card up by its key:
`paging_checks::all_rects` counts keys from zero). Its minimum (`MINIMUM`) is one card wide and one
row tall (`the_window_contract_fits_the_quarter_4k_budget_and_widest_card`,
`every_dynamic_page_fits_and_every_card_is_reachable`, `the_app_bar_holds_in_the_minimum_window`).
The cursor reaches and operates exactly every parameter, the sync switches included
(`the_keyboard_cursor_reaches_and_operates_every_parameter`), and every card passes the tree checks
at Init, with every parameter at the bottom of its travel, and at the top with the syncs on at no
tempo, a slow tempo and 120 bpm.

`src/telemetry.rs` is the only audio-to-editor channel: the input and output peaks, max-combined
until read, the output's clip, latched until acknowledged, and the host tempo in force. The app
bar's meter shows the output; the Output card's display shows both. Telemetry may lose frames and
never affects sound.

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
