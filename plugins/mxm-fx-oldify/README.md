# mxm-fx-oldify

A stereo effect that will make sound old: a cassette, an old record, or an old record taped onto a
cassette, worn from new to battered by one Age control.

**The cassette and record engines are not built yet.** What runs today is the chain they will sit
in:

- **Drive** — the level into the medium, ±24 dB.
- **Width** — mid/side, from mono (0 %) through the image as it came (100 %) to twice as wide.
- **Output** — the level out, ±24 dB.
- **Mix** — a dry/wet crossfade; zero is the dry input to the bit.

**Medium** (Cassette, Record, Record then cassette) and **Age** are permanent parameters that move
nothing yet. At its defaults the plugin passes the sound through untouched, and there are no
factory presets until there is something to hear.

```bash
cargo xtask bundle mxm-fx-oldify --release
```

The bundle is written to `target/bundled/mxm-fx-oldify.clap` with its control map beside it.

## Licence

GPL-3.0-or-later — see the repository's [`LICENSE`](../../LICENSE) at its root.
