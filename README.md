# mxm-fx-oldify

An MXM effect that makes sound old: cassette tape and old records (vinyl and shellac), from a new
deck to a battered 78 on a radiogram.

**Pre-alpha, and the engines are not built yet.** Today the plugin is the shell every MXM plugin
has — parameters, presets, editor, control map — around a placeholder chain: Drive, stereo Width,
Output and a dry/wet Mix. Medium and Age are there, permanent, and do nothing until the cassette
and record engines arrive. Inserted at its defaults it passes the sound through untouched.

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
