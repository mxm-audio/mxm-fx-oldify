# AGENTS.md — crates/mxm-fx-oldify-dsp

Parent: [`../../AGENTS.md`](../../AGENTS.md)

# Purpose

Framework-free DSP for `mxm-fx-oldify`. **Today it is a placeholder chain**: the frame the cassette
and record engines will fill, and nothing of either engine yet. The design and measurements the
engines will be built from are private, `ops:notes/oldify/` (root *The design and its measurements
are private*).

# Ownership

Owns `src/` and `Cargo.toml`. Host parameters, presets, telemetry and editor behaviour belong to
`plugins/mxm-fx-oldify`. No plugin-framework type belongs here.

# Local Contracts

## The chain

Per stereo frame, in this order (`frame`):

```text
input → Drive gain → (the cassette and record engines, later) → Width (mid/side) → Output gain → Mix with the dry input
```

- **The dry input is taken as it arrived, not delayed**: the chain has no latency yet. An engine
  that adds latency brings its reporting and a delayed dry path with it.
- Width works about the mid: zero is the mid in both channels, 1 the image as it came, `MAX_WIDTH`
  doubles the side.
- Mix is a linear crossfade, `dry · (1 − mix) + wet · mix`.

## Smoothing

Every control multiplies the audio, so every control is smoothed: a linear ramp over
`SMOOTHING_S`, inside the collection's 5–20 ms band. A ramp's position is an **integer step
count** and its value is computed from it, never accumulated, so advancing a silent block whole
lands where the same block processed sample by sample does, and how a host cuts its blocks never
changes a sample. The same target asked for again does not restart a ramp. `reset` ends every ramp
at its target; `new` starts settled at `Controls::IDENTITY`.

## Exact paths

- **Silence in is exact zero out**, at every setting and through every ramp; a silent block costs
  the input scan only, and the ramps advance across it.
- **Mix settled at zero is the dry input to the bit**, and costs the scan only; a Mix ramping to
  zero lands on the dry input exactly once the ramp ends.
- **The identity settled is the input to the bit**, and costs the scan only; `frame` at the identity
  is exact too (Width skipped at exactly 1, both ends of the crossfade taken exactly).

## Realtime and numeric rules

- Rust MSRV is 1.87; there are no dependencies, normal, build or dev.
- Nothing allocates, ever; every method is allocation-, lock-, log- and I/O-free.
- Accepted sample rates are `MIN_SAMPLE_RATE`–`MAX_SAMPLE_RATE`, mxm-fx-delay's range; others,
  and non-numbers, are refused with `UnsupportedRate`.
- Non-finite input samples are taken as silence. Controls are bounded at the public seam
  (`Controls::bounded`): gains to `MIN_GAIN_DB`…`MAX_GAIN_DB`, Width to 0…`MAX_WIDTH`, Mix to 0…1,
  and a non-finite control is its identity value. Ordinary audio at the extremes stays finite and
  under `max gain × MAX_WIDTH × max gain`.
- `db_to_gain` is nice-plug's law above its −100 dB floor, so the plugin's ranges are these bounds
  to the bit.

## Chosen constants

`SMOOTHING_S`, the gain and width bounds and the accepted-rate range are chosen for this
implementation, not measured. The engines' constants will come from the measurements and say so.

# Work Guidance

- An engine goes into `frame` between Drive and Width, keeping the exact paths above: silence in
  stays exact zero out (noise that stops in silence), Mix zero stays the dry input, and block size
  stays invisible. Seed every random source, so Windows renders can be pinned to the bit.
- Measure an engine's behaviour; a compiling engine proves nothing about how it sounds.

# Verification

```bash
cargo test -p mxm-fx-oldify-dsp
cargo clippy -p mxm-fx-oldify-dsp --all-targets -- -D warnings
cargo tree -e normal,build -p mxm-fx-oldify-dsp
cargo +1.87.0 test -p mxm-fx-oldify-dsp
```

The tests hold silence → exact zero at every setting, Mix zero → the dry input to the bit (settled
and after a ramp), the identity → the input to the bit (both paths, and after a ramp back), blocks
of 1, 37 and 512 bit-identical across a silent stretch mid-ramp and a retarget, finite bounded
output at every extreme and for non-finite controls and input, Width's mid and mono, the ramp's
length and exact landing, and the refused rates.

# Child DOX Index

No child `AGENTS.md` files.
