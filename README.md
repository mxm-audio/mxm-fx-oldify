# mxm-fx-oldify

An MXM effect that makes sound old: cassette tape and old records (vinyl and shellac), from a new
deck to a battered 78 on a radiogram.

**Pre-alpha.** The cassette engine works: tape type and grade, saturation, generations of
copies, wow and flutter (tempo-synced if you like, with flange), hiss and noise reduction,
dropouts, a misaligned deck and age, with drive, width, output and mix around them. The record
engine is still to come (the Record medium passes the sound through for now), and so are
print-through and tape stop. The effect has a fixed latency of about 50 ms, which it reports to the host.

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
