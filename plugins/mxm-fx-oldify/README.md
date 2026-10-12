# mxm-fx-oldify

A stereo effect that makes sound old: a cassette, an old record, or an old record taped onto a
cassette.

Two groups, each with its own switch heading its first card, and the output. Vinyl on alone plays
an old record; Tape on alone, a cassette; both on, a record taped to cassette. A group switched
off is dimmed but can still be set up.

- **Vinyl** — the Vinyl switch; the disc (LP, single or shellac 78), when it was cut (nine eras,
  from acoustic horns to modern), what it is played on (seven players, from a wind-up gramophone to
  a modern deck), and the cutting and replay curves, which follow the era and the player unless you
  pick others; Horn and Horn size, the acoustic horns a record was made and played through, for
  the thin, honky, tinny sound of a real old 78 on any record and player.
- **Vinyl groove** — wear, and dynamic wear, which wears the loud passages more (loud bass and sharp
  treble go gritty, distorted and dull); where on the side the needle is, groove distortion, an
  off-centre hole, and a warp that lurches the pitch and dips the level once a turn, going far past
  any real record; skips, the needle jumping back a groove to play the last turn again, and Stuck
  groove, which locks it there until let go.
- **Vinyl surface** — crackle, as dense and as loud as you like, clicks, surface noise, scratches
  and cracks that come round once a turn and move on after half a minute or so, rumble, mains
  hum, and the groove's echo of the music a turn later.
- **Tape** — the Tape switch; Type I, II or IV at four grades (twelve measured responses) and how
  much of it is heard; saturation, Soft or Hard; up to four generations of copies, each adding its
  own hiss, wobble and dullness; Age, worn from new to battered.
- **Tape transport** — wow and flutter (depth, rate, shape), each rate syncable to the host's tempo
  and each depth going past any real tape; flutter that follows the wow, its rate swung by
  it; Flange, a steady copy of the tape mixed against the moving one; and Tape stop, which slows
  the tape to a halt over the stop time (syncable too) and starts it again when let go.
- **Tape noise** — hiss; print-through, a faint muffled copy of the music a wrap of the reel later;
  noise reduction, single- or two-band, with its amount, brightness, a level
  error that makes it mistrack, and Decode off for the tape played without its decoder; and
  dropouts (how many, how deep and long, how differently left and right).
- **Tape deck** — azimuth (a tilted head), crosstalk between the tracks, and the wrong playback EQ.
- **Output** — Drive (±24 dB onto the tape, divided back out after it: more saturates more and
  buries the hiss without getting louder), Width, Mix against the dry input delayed to
  match, how far the noise follows the input, whether it stops in silence, Variation, which
  picks another take of everything random (the same number always sounds the same), and Quality:
  High saturates the tape at four times the rate and reads the moving tape and record more finely,
  for cleaner treble at more CPU. Loading a factory sound keeps your Mix and Quality.

The chain has one fixed latency (about 41 ms), reported to the host,
and a tail after which silence in is exact silence out, unless the noise is set to play on.
Fifty factory sounds cover cassettes, records and records taped to cassette, from a home deck
to an acoustic 78 on a wind-up gramophone, and go past them: melting tape, a saw-shaped wow, a
record warped twice as far as any real one.

```bash
cargo xtask bundle mxm-fx-oldify --release
```

The bundle is written to `target/bundled/mxm-fx-oldify.clap` with its control map beside it.

## Licence

GPL-3.0-or-later — see the repository's [`LICENSE`](../../LICENSE) at its root.
