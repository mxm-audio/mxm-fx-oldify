# mxm-fx-oldify

A stereo effect that makes sound old: a cassette, an old record, or an old record taped onto a
cassette, worn from new to battered by one Age control.

**The cassette engine is in; the record engine is not built yet.** Six cards, in signal order:

- **Medium** — Cassette, Record or Record then cassette; Age; Variation, which picks another take of
  everything random (the same number always sounds the same).
- **Tape** — Type I, II or IV at four grades (twelve measured responses) and how much of it is heard;
  saturation, Soft or Hard; up to four generations of copies, each adding its own hiss, wobble and
  dullness.
- **Transport** — wow and flutter (depth, rate, shape), each rate syncable to the host's tempo;
  flutter that follows the wow; Flange, a steady copy of the tape mixed against the moving one.
- **Noise** — hiss; noise reduction, single- or two-band, with its amount, brightness, a level
  error that makes it mistrack, and Decode off for the tape played without its decoder; and
  dropouts (how many, how deep and long, how differently left and right).
- **Deck** — azimuth (a tilted head), crosstalk between the tracks, and the wrong playback EQ.
- **Output** — Drive (±24 dB onto the tape, divided back out after it: more saturates more and
  buries the hiss without getting louder), Width, Output, a dry/wet Mix against the dry input
  delayed to match, how far the noise follows the input, and whether it stops in silence.

**Record** passes the sound through, delayed, until its engine exists; **Record then cassette** is
the cassette alone for now. The chain has one fixed latency (about 50 ms), reported to the host,
and a tail after which silence in is exact silence out, unless the noise is set to play on. There
are no factory presets yet.

```bash
cargo xtask bundle mxm-fx-oldify --release
```

The bundle is written to `target/bundled/mxm-fx-oldify.clap` with its control map beside it.

## Licence

GPL-3.0-or-later — see the repository's [`LICENSE`](../../LICENSE) at its root.
