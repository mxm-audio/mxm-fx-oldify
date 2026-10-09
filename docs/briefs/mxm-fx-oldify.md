# mxm-fx-oldify — interface brief

**Status:** brief, written before the editor beyond its placeholder cards. The product plan and the
measurements behind it are private (`ops`: the oldify plan and `measurements/` notes, cited as plain
text); this answers design-system §14 and does not reopen their DSP decisions.

## 1. Primary task

Make a sound old in one of two ways, or both: as **cassette tape** (from a good deck to a worn
pocket player, dubbed and redubbed) and as a **record** (from a new LP to a battered 78 on a
radiogram), and as a record taped to cassette. The everyday path is Medium → Age → the medium's few
character controls → Mix; the deep path opens one medium's whole chain, block by block, in signal
order.

## 2. Controls reached for first

**Medium, Age, Mix**, then per medium: for cassette **Tape** (type and grade), **Saturation**,
**Wow**, **Hiss**; for a record **Disc**, **Era** (the cut and the player), **Wear**, **Crackle**.
Age moves the whole medium older at once (`Age owns only what has no control of its own`, the
plan); the others are the first adjustments people make in every product the research surveyed.

## 3. Signal flow that must be visible

Input → Drive → *the record* (cut, groove, surface, pickup, player) → *the cassette* (noise
reduction, tape, transport, playback) → Width → Output → Mix. With Medium on one engine the other is
absent from the flow, not greyed. A compact flow strip across the top names the stages in order and
marks the active ones; it is a diagram of the chain, never a picture of a deck, a reel or a
turntable.

## 4. Cards and category

All primary category **Effects**, stable keys, in signal order. A card belongs to an engine and is
shown while that engine is in the chain:

| Key | Card | Engine | Controls (first-reach in bold) |
|---:|---|---|---|
| 0 | Medium | both | **Medium**, **Age**, Variation |
| 1 | Era | record | **Disc**, **Era** (cut), Player, Cut curve, Replay curve |
| 2 | Groove | record | **Wear**, Side position, Stylus, Groove distortion, Eccentricity, Warp |
| 3 | Surface | record | **Crackle**, Clicks, Surface noise, Scratches, Cracks, Rumble, Hum, Groove echo |
| 4 | Tape | cassette | **Tape**, Grade, Tape tone, **Saturation**, Character, Generations |
| 5 | Transport | cassette | **Wow** depth, rate (sync), shape; Flutter depth, rate (sync), shape; follows wow; Flange; Tape stop |
| 6 | Noise | cassette | **Hiss**, Noise reduction, Decode, its Amount and Brightness, Dropouts, Print-through |
| 7 | Deck | cassette | Azimuth, Crosstalk, Wrong EQ, Level error |
| 8 | Output | both | Drive, Width, Noise follows, Noise stops in silence, Output, **Mix**, Quality |

Cards keep their identities and order whatever Medium is; only which are present changes. Each
card's floor comes from its tree; category-first paging splits only where space requires.

## 5. Advanced controls

Everything here is software-added: there is no source panel. The first-reach controls (bold) are
always visible on their cards. The deck's alignment faults (card 7), the curves on the Era card and
the rarer surface faults are the disclosed depth: each card's disclosure opens its own detail, and
Age, the eras and the factory presets set them sensibly so most people never open them.

## 6. Identity accent

The collection default accent (dark `#4CC9D8`, light `#247F91`, measured in mxm-fx-delay's brief:
8.84:1 and 4.65:1 against `surface-1`) until a product hue is chosen; a warm, faded hue is the
proposal, to be measured against both themes before it replaces the default.

## 7. Live visualisations

- **Flow strip:** the active stages, and the medium's input and output levels.
- **Transport:** the speed deviation now (a moving trace of wow and flutter in percent), so a depth
  reads as what it does.
- **Surface:** the record's revolution as a ring with this revolution's scratches and cracks marked
  where they sit, and the playhead going round; a click lights where it happens. This is the
  revolution the noise is locked to, made visible.
- **Noise:** hiss and surface-noise level against the signal, so "too much at 30 %" can be seen.

All reserve geometry at rest, use shared tokens, carry semantic labels, and stop when the editor
closes (telemetry is lossy and write-only).

## 8. Source layout removed

No source panel exists. No tape deck, cassette shell, reels, turntable, tonearm or wood: the
collection is explicitly non-hardware (§2). The record ring is a diagram of where defects sit, not
a picture of a record.

## 9. Responsive and quarter-4K answer

With one medium, five cards (Medium, that medium's three or four, Output); with both, nine. The
opening size is derived with `mxm_plugin_test::opening_size::derive` from the measured layout of the
default medium (Cassette); the other media re-page within it. Every derived page fits 1920 × 1080
physical pixels at 100 % zoom; at 150 % and 200 % the window stays fixed, cards re-page, and
indivisible overflow scrolls. No first-reach control is ever behind a disclosure or clipped. Native
QA records Windows DPI 100 %, 150 % and 200 %; Linux and macOS are checked later, together.

## 10. Open for the owner

- Whether a card that belongs to an absent engine disappears (this brief) or stays and reads
  *Not in the chain*.
- The product hue (§6).
- Reporting latency: the transport's delay line needs a base delay, which the plugin would report to
  the host as latency (a first in the collection).
