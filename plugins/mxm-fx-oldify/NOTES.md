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
