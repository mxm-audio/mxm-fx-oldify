# AGENTS.md — crates/mxm-fx-oldify-dsp

Parent: [`../../AGENTS.md`](../../AGENTS.md)

# Purpose

Framework-free DSP for `mxm-fx-oldify`: the chain, the **cassette engine**, the **record
engine**, and the blocks they are built from. Still to come: the cassette's print-through, tape
stop and High quality; the record's warp, groove echo and skips. The design and the measurements
behind it are private, `ops:notes/oldify/` (root *The design and its measurements are private*).

# Ownership

Owns `src/` and `Cargo.toml`. Host parameters, presets, telemetry and editor behaviour belong to
`plugins/mxm-fx-oldify`. No plugin-framework type belongs here.

# Local Contracts

## The chain (`lib.rs`)

```text
input → Drive → [the record] → the cassette engine → ÷ Drive (a latency ago) → [+ hum]
  → Width → Output → Mix with the dry, delayed
```

- **Two switches, `record_on` and `tape_on`** (the owner, 2026-10-09: which controls belong to
  the record and which to the tape must be plain). Record on plays the record engine; tape on the
  cassette engine; both, a record taped to cassette; with the tape off the cassette engine still
  runs **neutral**, for its moving read alone; both off leaves Drive, Width, Output and Mix.
  Switching the record on clears it first.
- **One moving read for both media.** The record's eccentricity and turntable wobble ride the
  cassette transport's line (`Cassette::set_record_wobbles`, `Transport::set_extras`): a time-warp
  passes through the curves and the player almost unchanged and through a memoryless curve exactly,
  so the record needs no delay line of its own and the latency is one, whichever are on. Hum is
  added after the read and does not wobble.
- **Drive is compensated** by the drive the sound leaving the media went in with, one latency
  earlier: dividing by today's drive swung the level by Drive's whole range during a move.
- **One fixed latency**, `Oldify::latency`: the oversampler's, the transport's base delay (twice
  the wow's, flutter's, eccentricity's and turntable's reach) and the centre of azimuth's delay
  between the tracks, about 71 ms, the same for every control and switch; the dry path is
  delayed by it, so Mix never combs. The plugin reports it.
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

## The record engine (`record.rs`)

```text
era (mono, the acoustic band, the cutting limiter, the cutter, a brick wall) → cutting curve
  → groove (tracing distortion, wear) → + the disc's own sound → pickup → replay curve
  → player (scratch filter, valves, speaker, horn or radio)
```

- **The record side and the player side are apart** (`Era`, `Player`): any record meets any
  player, and a wrong replay curve errs by itself. `cut: None` follows the era, `replay: None` the
  player.
- **Era facts** are Copeland's and IASA's (cutter resonances, the 8.5 kHz wall on 1940s US 78s,
  wider cutters from 1943, limiters, mono before 1957–58); Q values and limiter times are chosen.
  **The players' speakers, horns and crystal resonance are not sourced**: chosen, the weakest
  numbers here.
- Wear raises the surface noise by up to `WEAR_NOISE_DB`; a player's own rumble adds to the
  disc's. Eccentricity is `d / R` at the turn, R from the disc's size and the side position, held
  within `ECCENTRIC_REACH_S`; the turntable's wobble within `TABLE_REACH_S`.

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
- **`curves`**: the period's recording curves by their time constants; replay `(1 + sT₂) /
  ((1 + sT₁)(1 + sT₃))` normalised at 1 kHz, cut its exact inverse with the cutting amplifier's
  `CEILING_S` on its lift; a missing low shelf stops at `MISSING_SHELF_S`. Bilinear, so the top
  octave is squeezed alike in cut and replay.
- **`disc`**: a disc that turns at its `Speed` and holds its defects at angles: per revolution
  tables of crackle (Pareto sizes) and clicks (each its own damped ring), `TURNOVER` of them renewed
  every turn; scratches and cracks at fixed angles, varied a little each time; surface noise
  swishing with the turn; rumble. Tables allocated in `new` for the busiest revolution.
- **`groove`**: tracing distortion `x / √(1 + c·x²)` on the lateral motion, `c` from the cited
  0.8 % (outer) and 4.5 % (inner) for an 18 µm tip, with the pinch effect's second harmonic in the
  vertical; a treble limit falling with the groove speed and with Wear (`FRESH_TOP_HZ`,
  `WORN_TOP_HZ`).
- **`player`**: crystal (amplitude-sensing) or magnetic pickup; scratch filter; single-ended or
  push-pull valves; speaker, horn or the radio's limiter and 4.5 kHz top. Period players are mono.
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
- **Mix zero is the dry input delayed by the latency, to the bit.** A clean modern record on a
  modern deck is the input delayed, at its level.
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
dropout laws, `WRONG_EQ_DB`, the noise reduction's thresholds, corners, skew and detector, the record's era Qs, limiter times, players, `TURNOVER`, crackle and click sizes and the groove's corners are chosen here, not measured. The tape table, the spacing law, the
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
zero → the delayed input to the bit, a clean record → the delayed input at its level, the neutral
cassette → the delayed input to −50 dB, an impulse at the latency, blocks of 1, 37 and 512 bit-identical through a quiet stretch
and a retarget, a reset rendering the same again, finite bounded output at every extreme, Width
zero → mono, the hiss at its level and silent after its hold, the hiss playing on with Noise stops
off, following noise at three input levels, the ramps, the latency near 50 ms at 44.1, 48 and
96 kHz, and the refused rates. Each block's own tests hold its contract (the cookbook gains, the
tape table against the measurement, the curves' slope, bounds and asymmetry, treble saturating
first, the oversampler's round trip, the transport's depths, reach, seeds, unmoved read and
following flutter, the spacing loss against Wallace at every common rate and a sweep without a
jump, the dropouts' rate, lengths, depths, width and smoothness, azimuth's notch and loss, the
skew's exact split, and the noise reduction's lift, two-band's reach, exact decode, undecoded and
off paths, mistracking and hiss reduction, the curves' shapes, exact cut-and-replay and the UK 78
error, the disc's rate, heavy tail, turn-locked renewal, cracks and levels, the groove's cited
distortion and treble loss, the players' bands, crystal, radio and mono, and the record side's
brick wall, acoustic band, wrong-curve dullness, eccentricity and gated noise).

# Child DOX Index

No child `AGENTS.md` files.
