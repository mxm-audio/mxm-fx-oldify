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
the groove's cited distortion and treble loss, Dynamic wear leaving a quiet groove exactly as it
was, wearing a loud one, a loud bass note where a middle one as fast is not, and the two walls
apart, the players' bands, crystal, radio and mono, and the record side's
brick wall, acoustic band, wrong-curve dullness, eccentricity and gated noise).

## Chosen constants

The smoothing, gain and width bounds, rate range, the transport's leaks (`LEAK`, `RANDOM_LEAK`),
reach and `FOLLOW_DEPTH`, the Age law, `EMPHASIS_DB`, `FADE_IN`, `MAX_DRIVE`, the curves' shapes,
the hiss's top and its 70 µs offset, the silence thresholds and times, the noise follower's,
`GENERATION_UM`, the dropout laws, `WRONG_EQ_DB`, the noise reduction's thresholds, corners, skew
and detector, the record's era Qs, limiter times, players, `TURNOVER`, the crackle, click, scratch
and crack sizes, the bursts' colours (`BRIGHTEST_HZ`, `DARKEST_HZ`), `GRIT`, `SCRATCH_WIDTH_S`,
`CRACK_WIDTH_S`, `BLOCK_BELOW`, the groove's corners and Dynamic wear's `STYLUS_HZ` (an estimate
from period compliance and tip mass), `STYLUS_Q`, `GIVE`, `KNEE`, `WALL_SPREAD_DB`,
`WORN_DULL_HZ`, `ROUGH`, `ROUGH_HZ` and `WORN_RELEASE_S` are chosen here, not measured.
`WORN_FROM_DB` is where our measurement of a vinyl emulation's worn stylus starts to compress.
`CRACK_SIZE` puts a crack's pop near −16 dBFS peak on an LP played on a modern deck and near
−12 dBFS on a 78 played on a radiogram (2026-10-10). The tape table, the spacing law, the dropout
statistics and lift, azimuth's geometry, the wow and flutter laws (linear, as the reference's) and
the wow, flutter and saturation extremes come from measurement or physics and say so.

## The random walk's leak

The random shape's walk leaks at the rate itself (`RANDOM_LEAK`): its speed wanders about the
rate, and it needs a third of the reach a slower leak did, so its full depth fits the bounded reach
at everyday rates.

## Noise reduction

Single-band: one stage, about 10 dB above its corner for quiet treble. Two-band: a second stage
20 dB lower and an octave lower, inside a top-octave skew. On white hiss the whole falls about
6–7 dB single-band and 8–9 dB two-band (its energy below the corners stays).

## The disc's sounds

**Why a click is noise.** A ring of one pitch is heard as a drop of water (the owner, 2026-10-09:
"the scratches… sound like rain drops"). **Why its DC is blocked.** The groove holds velocity; a
net push through the replay curve's bass lift thumps.

**Why a crack has no ring.** Esquef's thump, the tonearm's swing after a crack, is a damped ring
falling to the arm's resonance at 8–15 Hz. It is real, but it lives below hearing, and the
replay curve lifts it by 20 dB: measured after the curve, the ring sat 35–40 dB above the crack's
own pop and peaked at −8 dBFS, and the owner heard it as a drum machine's bass drum, whether it
started at hundreds of hertz or at tens (2026-10-09, 2026-10-10).

## Dynamic wear

The owner, 2026-10-10: "When a record has been played a lot, there is usually more wear in the
dynamic areas. Like a loud bass drum will be more worn down by the needle." Each wall's force is
the velocity through the stylus's impedance (`STYLUS_HZ`, so loud bass and sharp treble push
hardest); it gave way above a limit falling to `WORN_FROM_DB`; what it took comes back through the
impedance's exact discrete inverse, with dulling and roughness following it. Below the limit's knee
the groove is untouched, exactly (`dynamic_wear_takes_the_loud_passages_and_leaves_the_quiet_ones`).
The physics, with Copeland's and Hunt's words, heads `groove.rs`.

**Why the stylus resonates at 2 kHz.** The resonance decides which passages wear
first: below it the compliance's pull grows as the frequency falls, above it the tip mass's push
as it rises. Through Init's LP on a modern deck (2026-10-10; level change and distortion with
roughness, against the unworn):

| Dynamic wear | 60 Hz at −12 dBFS | 1 kHz at −12 dBFS | 6 kHz at −24 dBFS |
|---|---|---|---|
| 25 % | −3.6 dB, −14 dB | −0.2 dB, −39 dB | −1.3 dB, −29 dB |
| 50 % | −7.0 dB, −12 dB | −2.3 dB, −23 dB | −5.9 dB, −12 dB |
| 100 % | −11.6 dB, −16 dB | −9.1 dB, −14 dB | −10.5 dB, −2 dB |

Loud bass wears first, as the owner asked, then the treble, the middle last. At 1 kHz the treble
wore before the bass (a quiet 6 kHz tone lost 5 dB at 25 %, a loud 60 Hz one 0.8 dB). Roughness
and dulling follow what the wear took, not how far the force passed the limit: in the treble the
latter is huge, and turned a tone into noise at 25 %.
