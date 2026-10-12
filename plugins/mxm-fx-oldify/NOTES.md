# NOTES.md — plugins/mxm-fx-oldify

The reasoning behind rules in [`AGENTS.md`](AGENTS.md) that are still in force.

## Why the control map claims no role

No role the standard declares is
a quantity this effect has. Mix is how much of the aged sound is heard, not how much delay
(`fx.delay`), reverb (`fx.reverb`), curve stack (`fx.dynamics`) or chorus (`fx.chorus_mix`); Drive feeds
the tape, not a filter (`filter.drive`); Wow and Flutter are a tape's speed, not a delay line's
wobble (`fx.delay_wobble`, on the Delay page). Roles of its own are a collection-wide change to
mxm-kit's `docs/MXM_CONTROL_MAP.md` — a new page — and the owner's to rule on.

## Why the ladders are the rates' own

Wow rate's and Flutter rate's tempo syncs each run over the divisions their own range holds at
120 bpm, not over `Span::LFO`: these are a tape's speed wobbles, not an LFO, and Flutter's range
runs past `Span::LFO`'s. Stop time's runs the same way over its own range, 1/32 to four bars.

## Levels

A level with an Off stores linear gain (the collection's rule for a level): Hiss, Crosstalk,
Print-through, Surface noise, Rumble, Hum level and Groove echo alike. `params::level_db` converts it, giving the DSP its off
level wherever the reading rounds to Off — so the knob never reads Off while the level plays — and
its decibels to a hundredth otherwise, so a whole-decibel default reaches the DSP exactly. NR level
error is a gain too, stored the same way (`params::level_error_db`).

## The factory bank

Fifty sounds and Init (the owner, 2026-10-12: "please make so there is 50 presets + the init"):
sixteen cassettes, twenty records, four records taped to cassette, and ten past the real thing,
the controls taken where no deck or record goes (melting tape, a saw-shaped or vibrato wow, flutter
at 30 Hz swung by the wow, a 200 % warp, a crackle that plays on through silence, a groove echo
far louder than a groove's): the owner, 2026-10-11, "our plugin should scale into the 'non
physical' for creative use". The design table
(`src/preset_designs.rs`) is the bank's one readable statement, each sound as overrides on Init in
panel units with a sentence saying what it is; a control a design does not name stays at Init. Its
`generate` writes every parameter but Mix and Quality: Mix is factory-preserved, as in
mxm-fx-delay, so browsing keeps the player's Mix (the coordinator, 2026-10-09: the collection's
convention wins; "unlisted controls stay at Init" meant the sound controls, not Mix), and Quality
too (2026-10-11), being how hard the player's machine works rather than part of a sound. The example only writes what `generate`
returns, so the stale-file test can hold the committed files to a fresh generation. The values are
first estimates, to be tuned by ear (2026-10-09). The shared browser shows no text per preset, so a
sound's sentence lives in the design table.

**Surface noise sits 12 dB under the research's levels** (the owner, 2026-10-11: "The surface noise
was far to loud in the presets"). The research gives a record's noise against full modulation, and
the bank sat there: everything the disc and player make, A-weighted, 29–40 dB under a full-scale
tone on the 78s and 54–65 dB on the LPs. Music sits 15–20 dB under full modulation, so on the 78s
the noise came out only 6–18 dB under a −18 dBFS pink programme (A-weighted, Noise follows off).
Every record design and Init's record came down 12 dB: the 78s now sit 18–30 dB under that
programme, the single and the LPs 35–51. A preset's surface noise is its render less the same
render with Surface noise off: the noise has its own generator, where the disc's crackle, clicks,
scratches and cracks share one, so turning one of those off reshuffles the rest.

## The cards

Eight Effects cards in signal order: **Vinyl** (the Vinyl switch; Disc; Era, Player, Cut curve
and Replay curve as lists; Horn, Horn size), **Vinyl groove** (Wear, Dynamic wear, Side position; Groove
distortion, Eccentricity, Warp; Skips; Stuck groove), **Vinyl surface** (Crackle, Crackle level, Clicks; Surface noise, Scratches, Rumble; Cracks; Hum;
Hum level, Groove echo), **Tape** (the Tape switch; Tape type; Grade; Tape tone, Saturation; Character;
Generations; Age), **Tape transport** (Wow, Wow rate and its sync; Wow shape; Flutter, Flutter rate
and its sync; Flutter shape; Flutter follows wow; Flange, Stop time and its sync; Tape stop), **Tape noise** (Hiss, Print-through; Noise reduction;
Decode; NR amount, NR brightness, NR level error; Dropouts, Dropout intensity, Dropout width),
**Tape deck** (Azimuth, Crosstalk; Wrong EQ) and **Output** (the In and Out levels; Drive, Width,
Mix; Noise follows, Variation; Noise stops in silence; Quality). Noise follows and Variation share a knob row
so Output fits beside the seven group cards on one page at the budget.

**Vinyl, not Record** (the owner, 2026-10-09: "'Record' should be 'Vinyl'"): the switch and
its three cards read Vinyl, shellac 78s included, the word people reach for; the permanent id
stays `record_on`, and the DSP keeps calling it the record.

**No output level** (the owner, 2026-10-09: "it still has both output and mix"; earlier, "our
effects only have 'mix'"): the collection's effects end in Mix, so the `output` id is retired.
Drive is divided back out after the media, so the level leaving is the level that came in.

**Why a switch on each group** (the owner, 2026-10-09): each group's switch heads its first card and
each card is named for its group, so which cards shape the record and which the tape is plain; a
switched-off group is dimmed, not hidden, so it can be set up before it is switched on. Age belongs
to the tape (the record has Wear) and Variation to Output (it seeds both groups).

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

## Dimming

Only the card's controls dim: mxm-ui draws the title row.

## The tree checks' states

every card passes the tree checks at Init, with every parameter at the bottom of its travel, and at
the top with the syncs on at no tempo, a slow tempo and 120 bpm.

## The meters

The app bar's meter shows the output; the Output card's display shows both.
