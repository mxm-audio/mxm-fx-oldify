# NOTES.md — plugins/mxm-fx-oldify

The reasoning behind rules in [`AGENTS.md`](AGENTS.md) that are still in force.

## Why the control map claims no role

No role the standard declares is
a quantity this effect has. Mix is how much of the aged sound is heard, not how much delay
(`fx.delay`), reverb (`fx.reverb`), curve stack (`fx.dynamics`) or chorus (`fx.chorus_mix`);
Output is an effect's level, not an instrument's (`voice.output`, on the Voice page); Drive feeds
the tape, not a filter (`filter.drive`); Wow and Flutter are a tape's speed, not a delay line's
wobble (`fx.delay_wobble`, on the Delay page). Roles of its own are a collection-wide change to
mxm-kit's `docs/MXM_CONTROL_MAP.md` — a new page — and the owner's to rule on.

## Why the ladders are the rates' own

Wow rate's and Flutter rate's tempo syncs each run over the divisions their own range holds at
120 bpm, not over `Span::LFO`: these are a tape's speed wobbles, not an LFO, and Flutter's range
runs past `Span::LFO`'s.

## Levels

A level with an Off stores linear gain (the collection's rule for a level): Hiss, Crosstalk,
Surface noise, Rumble and Hum level alike. `params::level_db` converts it, giving the DSP its off
level wherever the reading rounds to Off — so the knob never reads Off while the level plays — and
its decibels to a hundredth otherwise, so a whole-decibel default reaches the DSP exactly. NR level
error is a gain too, stored the same way (`params::level_error_db`).

## The factory bank

Twenty-one sounds: nine cassettes, ten records, two records taped to cassette. The design table
(`src/preset_designs.rs`) is the bank's one readable statement, each sound as overrides on Init in
panel units with a sentence saying what it is; a control a design does not name stays at Init. Its
`generate` writes every parameter but Mix: Mix is factory-preserved, as in mxm-fx-delay, so
browsing keeps the player's dry/wet balance (the coordinator, 2026-10-09: the collection's
convention wins; "unlisted controls stay at Init" meant the sound controls, not Mix). The example only writes what `generate`
returns, so the stale-file test can hold the committed files to a fresh generation. The values are
first estimates, to be tuned by ear (2026-10-09). The shared browser shows no text per preset, so a
sound's sentence lives in the design table.

## The cards

Nine Effects cards in signal order: **Medium** (Medium; Age, Variation), **Era** (Disc; Era, Player,
Cut curve and Replay curve as lists), **Groove** (Wear, Side position; Groove distortion,
Eccentricity), **Surface** (Crackle, Clicks, Surface noise; Scratches, Rumble; Cracks; Hum; Hum
level), **Tape** (Tape; Grade; Tape tone, Saturation; Character; Generations), **Transport** (Wow,
Wow rate and its sync; Wow shape; Flutter, Flutter rate and its sync; Flutter shape; Flutter follows
wow; Flange), **Noise** (Hiss; Noise reduction; Decode; NR amount, NR brightness, NR level error;
Dropouts, Dropout intensity, Dropout width), **Deck** (Azimuth, Crosstalk; Wrong EQ) and **Output**
(the In and Out levels; Drive, Width, Output, Mix; Noise follows; Noise stops in silence).

**A list too long for a row of buttons** (Era, Player, the curves) is the collection's caret
selector (§7.4), and since a list has one hover sentence, the option chosen appends its own
(`option_sentences`): what that era or player was.

## Why the opening-page test looks each card up by its key

`opening_page_shows_every_card`, which looks each card up by its key: `paging_checks::all_rects`
counts keys from zero

## Smoothing

Sample-accurate automation splits a block at each event, so a target still lands on its sample.

## KeepAlive

the status is **`KeepAlive`**, the one an effect that never falls silent reports, never `Normal` and
never a finite tail.

## A synced knob

Synced, the knob reads the division in force against the tempo published in the telemetry, its
column holding every reading.

## The host test

Its default-sound test judges the source's end by the DSP's `SILENCE`, as the effect does.

## Resolving a synced rate

`MxmFxOldifyParams::controls(tempo)` resolves each synced rate every block from the host's tempo and
the rate's modulated position (`Ladder::resolve`, clamped into the rate's range), and the free knob
without a tempo; the DSP ramps the result like any turned knob.

## Engaged

The init patch is an audible cassette, so *an effect starts engaged* is met
(`the_defaults_are_audible_and_a_mono_input_feeds_both_sides`).
