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
- **One fixed latency**, `Oldify::latency`: the oversampler's, the transport's base delay and the
  centre of azimuth's delay between the tracks, the same for every control and Medium; the dry path is delayed by it, so Mix never combs. The plugin
  reports it.
- Width works about the mid; Mix is a linear crossfade with the delayed dry.
- **The noise** is gated by the chain: it holds `NOISE_HOLD_S` after the input falls silent and
  fades over `NOISE_FADE_S`. **Noise follows** scales it by the input's level against
  `FOLLOW_REFERENCE` (up to `FOLLOW_MOST` times), from a fast-up, slow-down follower. **Noise stops
  in silence** off keeps it playing: the chain is then `endless`, never quiet, its tail `u32::MAX`.

## The cassette engine (`cassette.rs`)

```text
NR encode → saturator (pre-emphasis → 2× curve → de-emphasis) → tape response × Tape tone
  → transport (wow, flutter; Flange against the unmoved read) → crosstalk → + hiss
  → spacing loss (Age, generations, azimuth, a dropout's lift) → a dropout's dip
  → wrong playback EQ → azimuth's delay between the tracks → NR decode
```

- The physics' order, not the reference's (which filters before saturating): a known difference.
- **Laws** (chosen, named in `cassette.rs`): wow and flutter depth as the control squared, up to
  `WOW_MAX` and `FLUTTER_MAX` (the reference's extremes); Age as a spacing of `AGE_MAX_UM` times the
  control to the 1.5. **The transport's reach is bounded** (`WOW_REACH_S`, `FLUTTER_REACH_S`), so
  the latency stays near 50 ms; only slow, deep wow is lowered to fit.
- Hiss is seeded, independent per channel, 4 dB lower on the 70 µs tapes, and gated by the chain.
- **Generations** (1–4): each dub adds its hiss power, its own uncorrelated wobble (depth × √n)
  and `GENERATION_UM` of spacing.
- **Flange** mixes the line read at exactly the base delay against the moving read: through-zero
  tape flanging. **Flutter follows wow** moves the flutter's rate by up to `FOLLOW_DEPTH` with the
  wow's shape.
- **Azimuth**: across a track, the sinc's main lobe as the spacing losing the same at half its
  first null; between the tracks, `TRACK_PITCH_M` · tan θ / speed of delay, split either side of a
  fixed centre so the latency never moves. 15′ gives the mono notch at 6.1 kHz. **Crosstalk** bleeds
  each channel into the other; **Wrong EQ** is the other time constant's ±`WRONG_EQ_DB` shelf.
- **Variation** picks the seed (`seed_for`) of every random source: transport, hiss, dropouts.
  A change reseeds from the next draw; a clear goes back to the seed's start.

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
- **`nr`**: noise reduction, a sliding-band compander: `y = x + k · H(x)`, `H` a high-pass whose
  corner slides up and `k` falls with the side chain's own level (previous sample). **The decoder
  is the encoder solved for its input, exactly** (`x = (y − k · pending) / (1 + k · b₀)`), from its
  own side chain, so encode then decode is the identity and a tape between mistracks where the
  physics does; both sides move their corners on the same `UPDATE` schedule. Single-band: one
  stage, about 10 dB above its corner for quiet treble. Two-band: a second stage 20 dB lower and an
  octave lower, inside a top-octave skew. Decode off is the encoded sound; the level error scales
  what reaches the decoder and puts the level back, so only the tracking changes. On white hiss
  the whole falls about 6–7 dB single-band and 8–9 dB two-band (its energy below the corners stays).
- **`dropouts`**: Poisson events of raised-cosine dips, log-uniform lengths from 3 ms and uniform
  depths, each lifting the tape `LIFT_UM_PER_DB` per decibel (the measured treble excess). Amount
  sets the rate (up to `MAX_RATE_HZ`) and depth; Intensity makes them fewer, longer, deeper; Width
  blends a shared stream into two independent ones. Nothing is drawn while the amount is zero.
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
- **Seeded**: the transport's, the two hiss generators' and the three dropout streams' draws, at
  most one each per sample; how a host cuts
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

The smoothing, gain and width bounds, rate range, the transport's leak, reach and `FOLLOW_DEPTH`,
the depth and Age laws, `EMPHASIS_DB`, `FADE_IN`, `MAX_DRIVE`, the curves' shapes, the hiss's top
and its 70 µs offset, the silence thresholds and times, the noise follower's, `GENERATION_UM`, the
dropout laws, `WRONG_EQ_DB` and the noise reduction's thresholds, corners, skew and detector are chosen here, not measured. The tape table, the spacing law, the
dropout statistics and lift, azimuth's geometry and the wow, flutter and saturation extremes come
from measurement or physics and say so.

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
zero → mono, the hiss at its level and silent after its hold, the hiss playing on with Noise stops
off, following noise at three input levels, the ramps, the latency near 50 ms at 44.1, 48 and
96 kHz, and the refused rates. Each block's own tests hold its contract (the cookbook gains, the
tape table against the measurement, the curves' slope, bounds and asymmetry, treble saturating
first, the oversampler's round trip, the transport's depths, reach, seeds, unmoved read and
following flutter, the spacing loss against Wallace at every common rate and a sweep without a
jump, the dropouts' rate, lengths, depths, width and smoothness, azimuth's notch and loss, the
skew's exact split, and the noise reduction's lift, two-band's reach, exact decode, undecoded and
off paths, mistracking and hiss reduction).

# Child DOX Index

No child `AGENTS.md` files.
