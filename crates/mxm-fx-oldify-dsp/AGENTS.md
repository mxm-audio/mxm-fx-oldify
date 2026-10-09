# AGENTS.md — crates/mxm-fx-oldify-dsp

Parent: [`../../AGENTS.md`](../../AGENTS.md)

# Purpose

Framework-free DSP for `mxm-fx-oldify`: the chain, the **cassette engine**, and the blocks it is
built from. The record engine, noise reduction, dropouts and the cassette's remaining faults are to
come. The design and the measurements behind it are private, `ops:notes/oldify/` (root *The design
and its measurements are private*).

# Ownership

Owns `src/` and `Cargo.toml`. Host parameters, presets, telemetry and editor behaviour belong to
`plugins/mxm-fx-oldify`. No plugin-framework type belongs here.

# Local Contracts

## The chain (`lib.rs`)

```text
input → Drive → the medium → ÷ Drive (a latency ago) → Width → Output → Mix with the dry, delayed
```

- **Medium**: `Cassette` and `RecordThenCassette` run the cassette engine; `Record` has no engine
  yet and passes the input delayed. Switching back into the cassette clears it first.
- **Drive is compensated** by the drive the sound leaving the medium went in with, one latency
  earlier: dividing by today's drive swung the level by Drive's whole range during a move.
- **One fixed latency**, `Oldify::latency`: the oversampler's and the transport's base delay, the
  same for every control and Medium; the dry path is delayed by it, so Mix never combs. The plugin
  reports it.
- Width works about the mid; Mix is a linear crossfade with the delayed dry.

## The cassette engine (`cassette.rs`)

```text
saturator (pre-emphasis → 2× curve → de-emphasis) → tape response × Tape tone
  → transport (wow, flutter) → + hiss → spacing loss (Age)
```

- The physics' order, not the reference's (which filters before saturating): a known difference.
- **Laws** (chosen, named in `cassette.rs`): wow and flutter depth as the control squared, up to
  `WOW_MAX` and `FLUTTER_MAX` (the reference's extremes); Age as a spacing of `AGE_MAX_UM` times the
  control to the 1.5. **The transport's reach is bounded** (`WOW_REACH_S`, `FLUTTER_REACH_S`), so
  the latency stays near 50 ms; only slow, deep wow is lowered to fit.
- Hiss is seeded, independent per channel, 4 dB lower on the 70 µs tapes, and gated by the chain.

## The blocks

- **`biquad`**: Bristow-Johnson's cookbook sections, kept as what they mean and built at any rate
  below `NYQUIST_FRACTION`; transposed direct form II, `f64` state; `set` keeps the state.
- **`tape`**: the twelve tape responses, fitted to our measurement of the reference
  (`ops:oldify/measure/fit_profiles.py`). Data from the fit: regenerate, never hand-edit.
- **`saturate`**: emphasis shelves at the tape's time constant (`EMPHASIS_DB`) around a 2×
  oversampled curve, Soft (asymmetric tanh) or Hard (`v / (1 + v⁴)^¼`). **Saturation 0 is linear**
  (the curve fades in over `FADE_IN`); drive lowers the clip point by up to `MAX_DRIVE`.
- **`oversample`**: an honest copy of mxm-mono-00's half-band pair; its latency is part of ours.
- **`transport`**: a delay line read by the integral of the speed error. Sine and saw are exact
  centred integrals; a change goes into a drift term that leaks away at an eighth of the rate, so
  the read never jumps; the random shape is Ornstein–Uhlenbeck. The base delay is twice the reach.
  Wow and flutter are the same in both channels, as measured.
- **`spacing`**: Wallace's spacing loss from two fitted tables (`BASE` at 44.1 and 48 kHz, `HIGH`
  from 88.2 kHz; `ops:oldify/measure/fit_spacing.py`). Between two rows it **crossfades the two
  rows' outputs**; a slot takes a new row only at weight zero, so a moving spacing never clicks.

## Smoothing

Every continuous control ramps linearly over `SMOOTHING_S`, as an **integer step count**, its value
computed, never accumulated; the same target again does not restart a ramp. Tape, Grade, Character
and the shapes switch at once. `reset` ends every ramp and clears every memory.

## Silence, seeds and exact paths

- **Silence in is exact zero out once the tails have run out**: the noise holds `NOISE_HOLD_S`
  after the input falls under `SILENCE`, fades over `NOISE_FADE_S`, and after `Oldify::tail` every
  memory is cleared and the chain is **quiet**. A quiet chain fed silence costs the input scan only
  and gives exact zero; the input returning wakes it, with every seed back at its start.
- **Mix zero is the dry input delayed by the latency, to the bit**; so is the Record medium.
- **A neutral cassette** (`Controls::IDENTITY`) is the input delayed, within the oversampler's
  passband; an impulse peaks at exactly the reported latency.
- **Seeded**: the transport's and the two hiss generators' draws, one per sample; how a host cuts
  its blocks never changes a sample, quiet stretches included.

## Realtime and numeric rules

- Rust MSRV is 1.87 (no let chains); there are no dependencies, normal, build or dev.
- `Oldify::new` allocates the delay lines; nothing allocates after it, and every other method is
  allocation-, lock-, log- and I/O-free.
- Accepted sample rates are `MIN_SAMPLE_RATE`–`MAX_SAMPLE_RATE`; others, and non-numbers, are
  refused with `UnsupportedRate`.
- Non-finite input is silence. Controls are bounded at the public seam (`Controls::bounded`,
  `CassetteControls::bounded`); a non-finite control is its neutral value. Output at every extreme
  stays finite and under the test's stated ceiling.
- `db_to_gain` is nice-plug's law above its −100 dB floor.
- **No output sample is subnormal**: `Oldify::frame` flushes one to exact zero, so a subnormal
  input sample comes out of the dry path as zero (`no_output_sample_is_subnormal`). Nothing upstream
  flushes them — nice-plug's flush-to-zero is behind its `unsafe_flush_denormals` feature, which the
  collection leaves off — and clap-validator rejects them (`process-sleep-constant-mask` and
  `param-fuzz-bounds` found them, 2026-10-09).

## Chosen constants

The smoothing, gain and width bounds, rate range, the transport's leak and reach, the depth and Age
laws, `EMPHASIS_DB`, `FADE_IN`, `MAX_DRIVE`, the curves' shapes, the hiss's top and its 70 µs
offset, and the silence thresholds and times are chosen here, not measured. The tape table, the
spacing law and the wow, flutter and saturation extremes come from measurement or physics and say so.

# Work Guidance

- Keep the exact paths above when a block is added: noise that stops in silence, Mix zero as the
  delayed dry, block size invisible, every random source seeded.
- Measure a block's behaviour; a compiling block proves nothing about how it sounds.

# Verification

```bash
cargo test -p mxm-fx-oldify-dsp
cargo clippy -p mxm-fx-oldify-dsp --all-targets -- -D warnings
cargo tree -e normal,build -p mxm-fx-oldify-dsp
cargo +1.87.0 test -p mxm-fx-oldify-dsp
```

The chain's tests hold silence → exact zero at every setting, exact zero again after the tail, Mix
zero and the Record medium → the delayed input to the bit, the neutral cassette → the delayed input
to −50 dB, an impulse at the latency, blocks of 1, 37 and 512 bit-identical through a quiet stretch
and a retarget, a reset rendering the same again, finite bounded output at every extreme, Width
zero → mono, the hiss at its level and silent after its hold, the ramps, the latency near 50 ms at
44.1, 48 and 96 kHz, and the refused rates. Each block's own tests hold its contract (the cookbook
gains, the tape table against the measurement, the curves' slope, bounds and asymmetry, treble
saturating first, the oversampler's round trip, the transport's depths, reach and seeds, the
spacing loss against Wallace at every common rate and a sweep without a jump).

# Child DOX Index

No child `AGENTS.md` files.
