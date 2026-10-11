# mxm-fx-oldify

An MXM effect that makes sound old: cassette tape and old records (vinyl and shellac), from a new
deck to a battered 78 on a radiogram.

**Pre-alpha.** Both engines work. The record: its era and the player it is played on (from a
wind-up gramophone to a modern deck), the acoustic horns, the cutting and replay curves, the groove's wear (worst on
the loud passages, if you like) and distortion, an off-centre hole, a warp, crackle, clicks, scratches,
cracks, surface noise, rumble, hum and the groove's echo of the music a turn later.
The cassette: tape type and grade, saturation, generations of copies, wow and flutter
(tempo-synced if you like, with flange), hiss and noise reduction, dropouts, a misaligned deck and
age. Drive, width and mix sit around them, and twenty-one factory sounds start from
both media. Still to come: skips, print-through and tape stop. The effect has a fixed latency of about 87 ms,
which it reports to the host.

Part of the MXM collection: every instrument, effect and tool lives in its own repository under
[github.com/mxm-audio](https://github.com/mxm-audio), built on
[mxm-kit](https://github.com/mxm-audio/mxm-kit).

## Building

Rust 1.95 or newer. On Linux, install ALSA, JACK, X11, xkbcommon and a GL loader first.

```bash
cargo xtask bundle mxm-fx-oldify --release   # -> target/bundled/mxm-fx-oldify.clap
cargo test
```

Copy `target/bundled/mxm-fx-oldify.clap` into your CLAP folder. This is pre-alpha: nothing is
released, so there are no official builds yet.

## Licence

GPL-3.0-or-later — see [`LICENSE`](LICENSE) and [`NOTICE.md`](NOTICE.md). The MXM
name and logo are trademarks; [`TRADEMARKS.md`](TRADEMARKS.md) says how they may be used.

Contributions are welcome: see [`CONTRIBUTING.md`](CONTRIBUTING.md). How the repository is
organised, and the rules each part keeps, are in [`AGENTS.md`](AGENTS.md).
