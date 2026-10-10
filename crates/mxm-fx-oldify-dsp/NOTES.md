# NOTES.md — crates/mxm-fx-oldify-dsp

The reasoning and detail behind [`AGENTS.md`](AGENTS.md) that is still in force.

## What the tests hold

The chain's tests hold silence → exact zero at every setting, exact zero again after the tail, Mix
zero → the delayed input to the bit, a clean record → the delayed input at its level, the neutral
cassette → the delayed input to −50 dB, an impulse at the latency, blocks of 1, 37 and 512 bit-identical
through a quiet stretch and a retarget, a reset rendering the same again, finite bounded output at every extreme, Width
zero → mono, the hiss at its level and silent after its hold, the hiss playing on with Noise stops
off, following noise at three input levels, the ramps, the latency near 50 ms at 44.1, 48 and
96 kHz, and the refused rates. Each block's own tests hold its contract (the cookbook gains, the
tape table against the measurement, the curves' slope, bounds and asymmetry, treble saturating
first, the oversampler's round trip, the transport's depths, reach, seeds, unmoved read and
following flutter, the spacing loss against Wallace at every common rate and a sweep without a
jump, the dropouts' rate, lengths, depths, width and smoothness, azimuth's notch and loss, the
skew's exact split, and the noise reduction's lift, two-band's reach, exact decode, undecoded and
off paths, mistracking and hiss reduction, the curves' shapes, exact cut-and-replay and the UK 78
error, the disc's rate, heavy tail, turn-locked renewal, clicks that are noise and not a tone,
scratches the same tick every turn, clicks, scratches and cracks with no net push, a crack
popping through the replay curve with under a tenth of itself below 100 Hz, cracks and levels,
the groove's cited
distortion and treble loss, the players' bands, crystal, radio and mono, and the record side's
brick wall, acoustic band, wrong-curve dullness, eccentricity and gated noise).

## Chosen constants

The smoothing, gain and width bounds, rate range, the transport's leaks (`LEAK`, `RANDOM_LEAK`),
reach and `FOLLOW_DEPTH`, the Age law, `EMPHASIS_DB`, `FADE_IN`, `MAX_DRIVE`, the curves' shapes,
the hiss's top and its 70 µs offset, the silence thresholds and times, the noise follower's,
`GENERATION_UM`, the dropout laws, `WRONG_EQ_DB`, the noise reduction's thresholds, corners, skew
and detector, the record's era Qs, limiter times, players, `TURNOVER`, the crackle, click, scratch
and crack sizes, the bursts' colours (`BRIGHTEST_HZ`, `DARKEST_HZ`), `GRIT`, `SCRATCH_WIDTH_S`,
`CRACK_WIDTH_S`, `BLOCK_BELOW` and the groove's corners are chosen here, not measured.
`CRACK_SIZE` puts a crack's pop near −16 dBFS peak on an LP played on a modern deck and near
−12 dBFS on a 78 played on a radiogram (2026-10-10).

**Why a crack has no ring.** Esquef's thump, the tonearm's swing after a crack, is a damped ring
falling to the arm's resonance at 8–15 Hz. It is real, but it lives below hearing, and the
replay curve lifts it by 20 dB: measured after the curve, the ring sat 35–40 dB above the crack's
own pop and peaked at −8 dBFS, and the owner heard it as a drum machine's bass drum, whether it
started at hundreds of hertz or at tens (2026-10-09, 2026-10-10). The tape table, the spacing law, the dropout
statistics and lift, azimuth's geometry, the wow and flutter laws (linear, as the reference's) and
the wow, flutter and saturation extremes come from measurement or physics and say so.
