# NOTES.md — crates/mxm-fx-oldify-dsp

The reasoning and detail behind [`AGENTS.md`](AGENTS.md) that is still in force.

## What the tests hold

The chain's tests hold silence → exact zero at every setting, exact zero again after the tail, Mix
zero → the delayed input to the bit, a clean record → the delayed input at its level, the neutral
cassette → the delayed input to −50 dB, an impulse at the latency, blocks of 1, 37 and 512 bit-identical
through a quiet stretch and a retarget, a reset rendering the same again, finite bounded output at every extreme, Width
zero → mono, the hiss at its level and silent after its hold, the hiss playing on with Noise stops
off, following noise at three input levels, the ramps, the latency near 90 ms at 44.1, 48 and
96 kHz, a warp's dip in step with its speed at the read, and the refused rates. Each block's own tests hold its contract (the cookbook gains, the
tape table against the measurement, the curves' slope, bounds and asymmetry, treble saturating
first, the oversampler's round trip, the transport's depths (the saw's, the random's and the
warp's as the reference's), reach, the phase an extra wobble will have at the read, the random's straight reads between its points, seeds, unmoved read, a
following flutter swung by a wow that moves no read, and its floor, the spacing loss against Wallace at every common rate and a sweep without a
jump, the dropouts' rate, lengths, depths, width and smoothness, azimuth's notch and loss, the
skew's exact split, and the noise reduction's lift, two-band's reach, exact decode, undecoded and
off paths, mistracking and hiss reduction, the curves' shapes, exact cut-and-replay and the UK 78
error, the disc's rate, heavy tail, turn-locked renewal, clicks that are noise and not a tone,
scratches the same tick every turn, clicks, scratches and cracks with no net push, a crack
popping through the replay curve with under a tenth of itself below 100 Hz, cracks and levels,
the groove's cited distortion and treble loss, Dynamic wear leaving a quiet groove exactly as it
was, wearing a loud one, a loud bass note where a middle one as fast is not, and the two walls
apart, the horn keeping only the middle at full, honking at `c / 2L` and staying bounded at full
scale, the players' bands, crystal, radio and mono, and the record side's
brick wall, acoustic band, wrong-curve dullness, eccentricity reaching twice the reference's and
its full travel fitting the reach, a warp alike on every disc and groove within its reach and
dipping the level where its speed is highest, and gated noise; the warp's shape constants and the
slope of its read).

## Chosen constants

The smoothing, gain and width bounds, rate range, the transport's leak (`LEAK`) and reach, the
Age law, `EMPHASIS_DB`, `FADE_IN`, `MAX_DRIVE`, the curves' shapes,
the hiss's top and its 70 µs offset, the silence thresholds and times, the noise follower's,
`GENERATION_UM`, the dropout laws, `WRONG_EQ_DB`, the noise reduction's thresholds, corners, skew
and detector, the record's era Qs, limiter times, players, `TURNOVER`, the crackle, click, scratch
and crack sizes, the bursts' colours (`BRIGHTEST_HZ`, `DARKEST_HZ`), `GRIT`, `SCRATCH_WIDTH_S`,
`CRACK_WIDTH_S`, `BLOCK_BELOW`, the groove's corners and Dynamic wear's `STYLUS_HZ` (an estimate
from period compliance and tip mass), `STYLUS_Q`, `GIVE`, `KNEE`, `WALL_SPREAD_DB`,
`WORN_DULL_HZ`, `ROUGH`, `ROUGH_HZ` and `WORN_RELEASE_S`, and the horn's feedbacks, mouths,
heights, corners, `BLAST` and the gramophone's `PLAY_LENGTH` and soundbox are chosen here, not
measured; the horn's lengths and its diaphragm's place come from Copeland's figures.
`WORN_FROM_DB` is where our measurement of a vinyl emulation's worn stylus starts to compress.
`CRACK_SIZE` puts a crack's pop near −16 dBFS peak on an LP played on a modern deck and near
−12 dBFS on a 78 played on a radiogram (2026-10-10). The tape table, the spacing law, the dropout
statistics and lift, azimuth's geometry, the wobbles' law (`full_depth`), their shapes, the
random's `POINTS_PER_CYCLE`, `FOLLOW_HZ`, `FOLLOW_FLOOR`, the warp's `HARMONICS`, `FULL` and
`DIP_DB`, and the saturation extremes come from measurement or physics and say so; `MOTION_MAX`,
`MAX_ECCENTRICITY_MM` and `MAX_WARP` are twice the references' travel, chosen.

## The chain

**Two switches, `record_on` and `tape_on`** (the owner, 2026-10-09: which controls belong to the
record and which to the tape must be plain).

**One moving read for both media.** The record's eccentricity, turntable wobble and warp ride the
cassette transport's line (`Cassette::set_record_wobbles`, `Transport::set_extras`): a time-warp
passes through the curves and the player almost unchanged and through a memoryless curve exactly,
so the record needs no delay line of its own and the latency is one, whichever are on.

**Drive is compensated** by the drive the sound leaving the media went in with, one latency
earlier: dividing by today's drive swung the level by Drive's whole range during a move.

**Laws**: wow and flutter depth go as the control, as the reference's do, so the whole travel is
heard (the owner, 2026-10-09, of a squared law: "I dont think the paramaters do much"); how deep a
control of 1 goes at each rate is the reference's own law ([§ The modulations,
fitted](#the-modulations-fitted)).

## Azimuth

**Azimuth**: across a track, the sinc's main lobe as the spacing losing the same at half its first
null; between the tracks, `TRACK_PITCH_M` · tan θ / speed of delay, split either side of a fixed
centre so the latency never moves. 15′ gives the mono notch at 6.1 kHz.

## Subnormals

`Oldify::frame` flushes one to exact zero, so a subnormal input sample comes out of the dry path as
zero (`no_output_sample_is_subnormal`). Nothing upstream flushes them — nice-plug's flush-to-zero
is behind its `unsafe_flush_denormals` feature, which the collection leaves off — and
clap-validator rejects them (`process-sleep-constant-mask` and `param-fuzz-bounds` found them,
2026-10-09).

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

## The horn

The owner, 2026-10-10: "I cannot get this to sound really old. Like I would expect from an
'oldify' plugin… Like the tinny sound on a real old 78 record", and then "Make sure it is a
parameter or two. Not just baked in at set level". Measured before the horn, the Acoustic era on
the wind-up gramophone stayed within about ±4 dB from 250 Hz to 2.5 kHz, smooth: muffled, not
tinny.

**Fitted to what "tinny" measures.** No source puts a number on tinny, so the research computed it
from the measured responses (`ops: notes/oldify`, research notes, the acoustic horn): the
125–250 Hz band's deficit under the 1–2 kHz band, the spectral centroid and the band within 10 dB
of the peak. Against them, at full Horn (2026-10-10):

| | Deficit | Centroid | Within 10 dB |
|---|---|---|---|
| Measured: a 1911 recording alone | 10.7 dB | 1,001 Hz | 310–1,260 Hz |
| Measured: an old horn machine alone | 11.8 dB | 1,386 Hz | — |
| Measured: the two together | 22.5–25 dB | 968–1,196 Hz | 630–1,250 Hz |
| Ours, 0.9 m: the recording half | 13.4 dB | 1,195 Hz | 178–2,263 Hz |
| Ours, 0.9 m: the playback half | 13.2 dB | 1,282 Hz | 449–2,851 Hz |
| Ours, 0.9 m: both | 26.0 dB | 1,239 Hz | 713–1,796 Hz |
| Ours, 0.6 m and 1.4 m: both | 34.9 and 24.4 dB | 1,297 and 1,298 Hz | 800–1,796 and 713–2,016 Hz |

With the diaphragm's peak at the cited +10 dB and the mouth's cut at 800 Hz the whole lost 36 dB
and kept only 1,131–1,600 Hz: why both sit lower. A short horn is the tinniest; Horn size sweeps
the measured range. The owner, 2026-10-11, of the horn as it is: "it sounds good to my ears."

## The modulations, fitted

The owner, 2026-10-11: "measure that the modulations in the tape and vinyl parts actually fits the
ones in the two plugins that were used as reference... and of course change oldify to fit", and
"our plugin should scale into the 'non physical' for creative use". Measured alike, on a 3150 Hz
tone, demodulated (`ops: notes/oldify/measurements`, the modulations, with every number):

- **A wobble's depth is a reach.** The tape reference's sine moves the read about 3.5 ms either way
  at full travel at the slowest rates, falling as the rate rises, so its peak speed error grows
  with the rate and levels off near 8 Hz; wow and flutter keep the one law, and depth is linear.
  `full_depth` fits it within 0.5 % from 0.1 to 12 Hz, and ours now reads within 0.5 % of it at
  every rate measured. A depth fixed in speed, as ours was, was six times too deep at 0.1 Hz and
  half as deep at 5 Hz.
- **Every shape reaches as far.** Its saw is the read's saw cut to two harmonics (equal in speed);
  its random draws points uniform across the reach every 16 384 samples a hertz (at 48 kHz) and
  reads straight between them: steps in speed, 0.38 of the depth RMS, centred near the rate, with
  sinc nulls at 2.93 times it. Ours had a true saw in speed and a smooth wander as deep as a sine.
- **Its Flutter Mod. is a mode, not an extra:** the wow moves no read and swings the flutter's
  rate ±4.5 Hz at full, linear in the wow's depth, at every flutter and wow rate, stopping a 5 Hz
  flutter at 2.5 Hz. Ours swung half the flutter's rate whatever the wow and kept the wow. Under a
  4 Hz wow the two differ a little (ours swings a tenth wider by zero crossings), where a flutter
  cycle spans half a wow's and the measurement cannot say more.
- **Flange already matched:** the moving and the steady copy mixed alike at every setting.
- **The record reference's eccentricity** at full is ±1.74 % on an LP's outer groove, about 2.5 mm
  of ours, rising to ±3.48 % on the inner (ours, from the radius, ±4.2 %; its inner groove may sit
  further out). Its own law adds a second harmonic up to −13 dB and stays at 33⅓'s rate at 45: an
  off-centre hole gives neither, so ours keeps one sine at the turn.

**Past the references.** Wow and Flutter go to twice the reference's travel, Eccentricity to 5 mm.
The presets and Init were rescaled to keep each one's RMS speed error as it was.

## The warp, fitted

The owner, 2026-10-11, offered the record reference's warp as measured (a once-a-turn wobble up to
±1.1 % with strong harmonics, and a level swing up to 2.4 dB): "build the warp". Measured as the
modulations above (`ops: notes/oldify/measurements`, the modulations):

- **The speed** is the reference's at an LP's outer groove, a Warp of 1 its full travel: the
  turn's fundamental at ±1.128 %, the second harmonic −5.8 dB and the third −17.5 dB at fixed
  phases (`HARMONICS`), 2.99 % peak to peak; a fourth and fifth near −34 dB are left out. Ours
  measures ±1.125 %, −5.7 dB and −17.4 dB, at its phases within 0.03 rad.
- **Alike on every disc and groove.** A warp's speed error is its height's slope against the
  angle over the groove's radius, whatever the disc's speed (`ops: notes/oldify/research_notes`,
  the vinyl mechanics); a disc bent from its label, its height growing with the radius, errs alike
  everywhere. The reference's grows toward the middle (1.6 times at its inner groove), is a fifth
  deeper at 45 and turns its harmonics' phases across the side, which one bend does not give: ours
  keeps one shape. A real warp's wow, about 0.05–0.4 %, is up to a third of the reference's full.
- **The level dips once a turn**, deepest where the speed is highest. The reference's swing is a
  plain gain: 2.43 dB peak to peak at its full warp at 250 Hz, 1 kHz and 3150 Hz alike, always
  below the unwarped level, the same at 33 and 45. Ours dips from 0 to `DIP_DB` times Warp in step
  with the speed. No source gives its size: it is the reference's, as the owner asked for it.
- **The dip rides the record, before any tape**, so a tape's noise reduction encodes it as it would
  a played record's; the speed rides the shared read, a base delay later. The record takes its
  phase from where the warp will be when the sample is read (`Transport::extra_phase_when_read`),
  so the two line up: on a 78, where they would be furthest apart, the level follows the speed at a
  correlation of −0.996, and −0.60 without it (`a_warps_dip_lines_up_with_its_speed`).
- **The law and the knob.** The value is linear in the speed: 100 % is the reference's full warp,
  200 % twice it, for effect. The reference's own knob goes as about its 1.55th power, so ours is
  skewed as Eccentricity, giving real warps the first half of the travel. `WARP_REACH_S` holds
  200 % on an LP, the slowest disc: the latency grew by 16 ms for it.
- **Presets**: the early single on a portable (20 %) and the worn LP (15 %) carry a warp; Init has
  none.
