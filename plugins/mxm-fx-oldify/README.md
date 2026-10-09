# mxm-fx-oldify

A stereo effect that makes sound old: a cassette, an old record, or an old record taped onto a
cassette, worn from new to battered by one Age control.

**The cassette engine is in; the record engine is not built yet.** Per frame:

- **Drive** — how hard the sound goes onto the tape, ±24 dB, divided back out after it: more
  saturates more and buries the hiss without getting louder.
- **The cassette** — saturation (Soft or Hard), the response of one of twelve tapes (Type I, II or
  IV, four grades) at its Tape tone, wow and flutter (depth, rate, shape), hiss that fades out when
  the sound stops, and Age's loss of the top.
- **Width** — mid/side, from mono (0 %) through the image as it came (100 %) to twice as wide.
- **Output** — the level out, ±24 dB.
- **Mix** — a dry/wet crossfade against the dry input delayed to match; zero is that delayed input
  to the bit.

**Record** passes the sound through, delayed, until its engine exists; **Record then cassette** is
the cassette alone for now. The chain has one fixed latency (about 50 ms), reported to the host,
and a tail after which silence in is exact silence out. There are no factory presets yet.

```bash
cargo xtask bundle mxm-fx-oldify --release
```

The bundle is written to `target/bundled/mxm-fx-oldify.clap` with its control map beside it.

## Licence

GPL-3.0-or-later — see the repository's [`LICENSE`](../../LICENSE) at its root.
