# AGENTS.md — plugins/mxm-fx-oldify

Parent: [`../AGENTS.md`](../AGENTS.md)

# Purpose

The `mxm-fx-oldify` CLAP effect: permanent parameters and identity, tempo sync, latency and tail
reporting, the preset seam, control map, telemetry and nine-card editor around
[`crates/mxm-fx-oldify-dsp`](../../crates/mxm-fx-oldify-dsp/AGENTS.md). **Both engines are in**: the
record (era, player, curves, groove, surface) and the cassette (tape, transport, noise reduction,
noise, dropouts, deck faults). What is still to come is under *Still to come*.

UI brief: [`../../docs/briefs/mxm-fx-oldify.md`](../../docs/briefs/mxm-fx-oldify.md). The design and
measurements are private: `ops:notes/oldify/` (root *The design and its measurements are private*).

# Ownership

Owns `Cargo.toml`, `README.md`, `NOTES.md`, `control-map.json`, `presets/`, `examples/`, `tests/`,
`host-tests/` and `src/`. Its licence is the repository's root `LICENSE`. DSP remains in the
framework-free product crate; visual geometry remains in `mxm-ui`.

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
| Medium | `medium` | Medium | Cassette / Record / Record then cassette (`cassette`, `record`, `record-then-cassette`) |
| | `age` | Age | 0–100 %, linear |
| | `variation` | Variation | Integer 0…`MAX_VARIATION`: the seed every random source starts from |
| Era | `disc` | Disc | LP 33⅓ / Single 45 / Shellac 78 (`lp-33`, `single-45`, `shellac-78`) |
| | `era` | Era | Acoustic / Early electrical / US 78, 1940s / UK 78, 1940s / Late 78 / Early single / Early LP / Mono LP / Modern (`acoustic`, `early-electrical`, `us-78`, `uk-78`, `late-78`, `early-single`, `early-lp`, `mono-lp`, `modern`) |
| | `player` | Player | Wind-up gramophone / Radiogram / Jukebox / Portable / Mono hi-fi / On the radio / Modern deck (`wind-up`, `radiogram`, `jukebox`, `portable`, `mono-hi-fi`, `on-the-radio`, `modern-deck`) |
| | `cut_curve`, `replay_curve` | Cut curve, Replay curve | Follow era (`follow-era`) or Follow player (`follow-player`), then Flat / UK 78 / UK 78 with treble lift / US 78 / Early LP / Early 45 / Transcription / West Coast LP / Standard (`flat`, `uk-78`, `uk-78-with-treble-lift`, `us-78`, `early-lp`, `early-45`, `transcription`, `west-coast-lp`, `standard`) |
| Groove | `wear`, `side` | Wear, Side position | 0–100 %, linear; side runs from the outer groove to the inner |
| | `groove_distortion` | Groove distortion | 0–200 %, linear; 100 % is the groove's own |
| | `eccentricity` | Eccentricity | 0…`record::MAX_ECCENTRICITY_MM` mm, skewed so the everyday tenths get the first half |
| Surface | `crackle`, `clicks` | Crackle, Clicks | 0…`disc::MAX_CRACKLE_HZ` / `MAX_CLICKS_HZ` a second, read `60/s`, skewed so the low counts get the travel |
| | `surface_noise`, `rumble` | Surface noise, Rumble | `disc::NOISE_OFF_DB` (Off)…`MAX_SURFACE_DB` / `MAX_RUMBLE_DB`, as Hiss |
| | `scratches`, `cracks` | Scratches, Cracks | Integers 0…`disc::MAX_SCRATCHES` / `MAX_CRACKS`; Cracks drawn as buttons |
| | `hum`, `hum_level` | Hum, Hum level | Off / 50 Hz / 60 Hz (`off`, `50-hz`, `60-hz`); the level `NOISE_OFF_DB` (Off)…`params::MAX_HUM_DB`, as Hiss |
| Tape | `tape` | Tape | Type I / Type II / Type IV (`type-1`, `type-2`, `type-4`) |
| | `grade` | Grade | Economy / Standard / High / Reference (`economy`, `standard`, `high`, `reference`) |
| | `tape_tone`, `saturation` | Tape tone, Saturation | 0–100 %, linear; zero Saturation is linear |
| | `character` | Character | Soft / Hard (`soft`, `hard`) |
| | `generations` | Generations | Integer 1…`MAX_GENERATIONS`, drawn as buttons |
| Transport | `wow`, `flutter` | Wow, Flutter | 0–100 %, linear |
| | `wow_rate`, `flutter_rate` | Wow rate, Flutter rate | `cassette::WOW_HZ`, `FLUTTER_HZ`, skewed so the geometric mean is mid-travel, read in Hz |
| | `wow_sync`, `flutter_sync` | Wow sync, Flutter sync | Off / On: the rate beside it on `params::WOW_SYNC` / `FLUTTER_SYNC` |
| | `wow_shape`, `flutter_shape` | Wow shape, Flutter shape | Sine / Random / Saw / Reverse saw (`sine`, `random`, `saw`, `reverse-saw`); Flutter's the first two |
| | `follow` | Flutter follows wow | Off / On |
| | `flange` | Flange | 0–100 %, linear |
| Noise | `hiss` | Hiss | `HISS_OFF_DB`…`HISS_MAX_DB`, stored as linear gain, `gain_skew_factor`; Off at the bottom |
| | `nr` | Noise reduction | Off / Single-band / Two-band (`off`, `single-band`, `two-band`) |
| | `nr_decode` | Decode | On / Off: Off plays the tape without its decoder |
| | `nr_amount`, `nr_brightness` | NR amount, NR brightness | 0–100 %, linear: half is the period's 10 dB a stage; the band's corner across `nr::CORNER_HZ` |
| | `nr_level_error` | NR level error | ±`nr::MAX_LEVEL_ERROR_DB`, stored as linear gain, 0 dB mid-travel, read in dB; `params::level_error_db` gives the DSP its decibels to a hundredth, so 0 dB arrives as zero |
| | `dropouts`, `dropout_intensity`, `dropout_width` | Dropouts, Dropout intensity, Dropout width | 0–100 %, linear |
| Deck | `azimuth` | Azimuth | 0…`MAX_AZIMUTH_ARCMIN` arc-minutes, read `15.0′` (`'` and `arcmin` parse too), skewed so 15′ is mid-travel |
| | `crosstalk` | Crosstalk | `CROSSTALK_OFF_DB`…`CROSSTALK_MAX_DB`, as Hiss |
| | `wrong_eq` | Wrong EQ | Off / On |
| Output | `drive`, `output` | Drive, Output | `MIN_GAIN_DB`…`MAX_GAIN_DB`, stored as linear gain, 0 dB mid-travel, read in dB; Drive compensated after the tape |
| | `width` | Width | 0…`MAX_WIDTH`, linear, read in percent; 100 % is the image as it came |
| | `mix` | Mix | 0–100 %, linear, against the dry input delayed by the latency |
| | `noise_follows` | Noise follows | 0–100 %, linear |
| | `noise_stops` | Noise stops in silence | Off / On |

The bounds are the DSP crate's constants, so the plugin's ranges and the DSP's clamps are one set of
numbers. **A level stores linear gain**; one with an Off reads Off exactly where the DSP gets its
off level (`params::level_db`, `a_level_reads_off_at_the_bottom_and_off_plays_nothing`; [NOTES.md §
Levels](NOTES.md#levels)).

## Init: the engaged cassette

**The defaults are the DSP's init patch, `Controls::INIT`**, read from it in
`MxmFxOldifyParams::default`; the switches' defaults are written as choices, and
`the_defaults_are_the_dsp_init_patch` holds the whole set equal. Init is generated from them, and it
is engaged ([NOTES.md § Engaged](NOTES.md#engaged)).

## Tempo sync

Wow rate and Flutter rate keep the collection's one sync interface, as mxm-fx-delay's Rate: one
*sync* switch, drawn as the quarter note beside its rate, default Off. **Each ladder is its rate's
own range at 120 bpm** (`each_sync_ladder_is_its_rates_own_range_at_120_bpm`), the top the fastest
([NOTES.md § Why the ladders are the rates' own](NOTES.md#why-the-ladders-are-the-rates-own); how a
rate resolves: [NOTES.md § Resolving a synced rate](NOTES.md#resolving-a-synced-rate)). **Nothing
resolved is kept**, so nothing goes stale: an activation forgets the tempo, and a reset resolves at
the last tempo seen (`the_tempo_reaches_the_editor_and_an_activation_forgets_it`,
`a_reset_resolves_the_syncs_at_the_last_tempo_seen`; [NOTES.md § A synced
knob](NOTES.md#a-synced-knob)). An older preset loads both syncs Off and an older project restores
them Off (`preset::TEMPO_SYNC_IDS`, `mxm_preset::add_switches_off` in `filter_state`).

## Latency and tail

- **The chain has one fixed latency**, `Oldify::latency()`, set by the sample rate alone: the
  record's reach added to the cassette's, whatever the Medium. `activate` builds the engine (it
  allocates its delay lines there, never on the audio thread) and reports it with
  `ActivateContext::set_latency_samples`, once; it never changes while processing
  (`activation_reports_the_chains_latency_at_the_hosts_rate`). Mix zero is the dry input delayed by
  it (`mix_zero_is_the_dry_input_delayed_by_the_latency`).
- **The tail is the DSP's.** While `Oldify::endless()` — Noise stops in silence off, with the
  medium's own noise on — the status is **`KeepAlive`** ([NOTES.md §
  KeepAlive](NOTES.md#keepalive)). Otherwise the shell counts the samples since the input was last
  above the DSP's `SILENCE`, as the DSP counts them, and reports `Tail(n)` with what is left of
  `Oldify::tail()` until the output is exact zero, then `Normal`, so a host may sleep the effect
  (`the_tail_counts_down_to_exact_silence_and_then_the_effect_may_sleep`,
  `an_endless_hiss_keeps_the_effect_alive_until_the_noise_stops`).

## Smoothing lives in the DSP

The parameters carry **no nice-plug smoother**: the DSP ramps every continuous control's block
target over its `SMOOTHING_S` (`a_moved_control_ramps_and_is_heard` holds it through the shell), and
takes a switch or a count as it comes ([NOTES.md § Smoothing](NOTES.md#smoothing)).

## Still to come

- The record's warp and groove echo, and skips; the cassette's print-through, tape stop and High
  quality. None is a parameter yet: each id is fixed when its control arrives.

## Presets

Through `mxm-preset` (`src/preset.rs`). **Twenty-one factory sounds**, category Fx, designed in
`src/preset_designs.rs` as overrides on Init and written to `presets/` by
`examples/fx_oldify_build_presets.rs`. **A recipe leaves out Mix** (`is_factory_preserved`), as
mxm-fx-delay's do, so loading a sound keeps the player's dry/wet balance
(`loading_a_factory_sound_keeps_the_players_mix`). Held by
`every_shipped_recipe_is_what_the_generator_emits_today`,
`the_factory_bank_is_complete_and_every_sound_its_own` and
`every_shipped_recipe_loads_renders_finite_without_clipping_and_is_heard` ([NOTES.md § The factory
bank](NOTES.md#the-factory-bank)).

## Control map

`control-map.json` **claims no role**: no role the standard declares is a quantity this effect has,
and roles of its own are a new page, the owner's to rule on (its comment, and [NOTES.md § Why the
control map claims no role](NOTES.md#why-the-control-map-claims-no-role)). Held by
`control_map_names_only_live_parameters_and_claims_no_role_yet` and the host test
`the_map_loads_and_claims_no_role`.

## Editor and telemetry

Nine Effects cards in signal order, each **under the brief's stable key** (`sections::KEYS`), what
each holds in `sections::card` ([NOTES.md § The cards](NOTES.md#the-cards)). All nine show whatever
Medium is: whether an absent engine's cards disappear is the brief's open question. Era, Player and
the curves are caret selectors whose hover adds the chosen option's sentence (`option_sentences`).
The shapes are drawn, not named; every switch cell carries its own sentence (`details_of`); Drive,
Width, Output and NR level error are bipolar knobs; the on/off parameters are the collection's
labelled toggle.

Every card body is a `mxm_ui::tree` (`editor/sections.rs`), its floor computed by
`tree::card_floor`; there is no declared usability minimum. The window opens at the quarter-4K
budget hugged (`REFERENCE`, `the_opening_size_is_the_budget_hugged`), every card on the opening page
(`opening_page_shows_every_card`). Its minimum (`MINIMUM`) is one card wide and as tall as the
tallest card's row (`the_window_contract_fits_the_quarter_4k_budget_and_widest_card`,
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
- A compile or a render does not approve a sound. Owner listening, native design-system §15 in both
  themes and one real DAW at a small buffer remain manual, and are reported as such.

# Verification

```bash
cargo test -p mxm-fx-oldify
cargo clippy -p mxm-fx-oldify --all-targets -- -D warnings
MXM_PICTURES=after cargo test -p mxm-fx-oldify --lib tree_pictures -- --ignored   # target/layout-tree/mxm-fx-oldify/after/
cargo run -p mxm-fx-oldify --release --example fx_oldify_build_presets   # after editing a design
cargo xtask bundle mxm-fx-oldify --release
clap-validator validate target/bundled/mxm-fx-oldify.clap
cargo test -p mxm-fx-oldify-host-tests --test effect_chain
```

The host test's `control_map` module holds this plugin's map as the player reads it; run it after
any change to `control-map.json` or the standard ([NOTES.md § The host
test](NOTES.md#the-host-test)).

# Child DOX Index

No child `AGENTS.md` files.
