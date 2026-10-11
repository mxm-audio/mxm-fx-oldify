//! The transport: a moving read from a delay line, so the speed the music is played at varies as a
//! tape's or a record's does.
//!
//! **Speed in, delay out.** A playback speed `1 + δ(t)` heard through a delay line reading
//! `x(t − D(t))` is the same as `dD/dt = −δ`: the delay is the running integral of the speed error,
//! about a base delay `D₀`. A sine of peak `δ` at `f` hertz moves the read by `δ·rate / (2π·f)`
//! samples either way. Only speed changes that average to zero can be heard through a fixed delay,
//! which is every wobble here.
//!
//! **Every shape moves the read as far as a sine of its depth** (`depth / (2π f)` either way), as
//! in the reference we measured (`ops: notes/oldify/measurements`, the modulations): a wobble's
//! depth is its sine's peak speed error at its rate, and the shape decides only how the read gets
//! there. The sine's speed is a sine. The saw's read is a saw's first two harmonics (the second
//! half the first): the tape falls behind slowly and catches up quickly, the reverse saw the other
//! way about. The random shape's read is drawn straight between points uniform across its reach,
//! [`POINTS_PER_CYCLE`] a cycle: its speed holds, then steps, about the rate. The warp's read is
//! a warped record's turn ([`crate::warp`]).
//!
//! **Centring.** Each shape's read is centred, so its speed has exactly its shape. A change of
//! depth, rate or shape would make the read jump; the jump goes into a drift term instead, so the
//! read never jumps, and the drift leaks away through a first-order high-pass at an eighth of the
//! wobble's rate, within a cycle or two. Until a change has leaked away the read can swing up to
//! twice its reach one way, so the base delay is twice the reach.
//!
//! **One room for every wobble.** The wobbles share the reach the transport was made with: while
//! together they would move the read further, each is lowered by the same share, so any one alone
//! keeps its whole depth and only an extreme stack gives way. The tracks' delay, the two channels
//! read a little apart (a tilted head), is read here too, either side of the moving read, and
//! counts against the room first.
//!
//! **Reading between samples:** four-point, third-order Hermite interpolation (Catmull–Rom; Olli
//! Niemitalo, "Polynomial Interpolators for High-Quality Resampling of Oversampled Audio", 2001),
//! flat enough at these small speed changes, though it dulls the top octave by a few decibels where
//! the read falls halfway between samples, and so moves the treble as the read moves. **High
//! quality** reads with a Kaiser-windowed sinc instead ([`Sinc`]; Smith, "Digital Audio
//! Resampling", the bandlimited interpolation it describes): 32 taps, flat to 0.42 of the rate.
//! The base delay leaves room ahead for it at either quality, so the latency is one.
//!
//! **A following flutter** is the wow's other use, the reference's: the wow moves no read of its
//! own, and swings the flutter's rate instead ([`FOLLOW_HZ`]), never below [`FOLLOW_FLOOR`] of it.
//!
//! **A tape stop** is the same law with the speed taken all the way down: held, the speed falls
//! straight to nothing over the stop time (a constant brake), and the read falls behind by what the
//! slower tape did not play, `Σ(1 − speed)`; stopped, nothing is read. Let go, the tape starts
//! again: the speed rises straight to its own over as long as the base delay, from a read put ahead
//! of the base by exactly what the start will lose, so it lands on the base delay at full speed and
//! the latency never moves. Let go while still slowing, it brakes the rest of the way in
//! [`BRAKE_S`] first. The line holds the longest stop's lag beside the wobbles' reach.
//!
//! Every random draw comes from the transport's own seeded generator, one at each of a random
//! wobble's points, so a render with the same seed is the same at any block size.

use crate::oversample::bessel_i0;
use std::f64::consts::{PI, TAU};

/// How a wobble moves the read over one cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Sine,
    /// Straight between random points: the speed holds, then steps.
    Random,
    /// Falling behind slowly, then catching up.
    Saw,
    /// Getting ahead slowly, then dropping back.
    ReverseSaw,
    /// A warped record's turn: [`crate::warp::read`].
    Warp,
}

/// One wobble: a depth, a rate and a shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wobble {
    /// A sine's peak speed error at this rate, as a fraction (0.01 is 1 %); every shape moves the
    /// read as far as that sine.
    pub depth: f64,
    pub hz: f64,
    pub shape: Shape,
}

impl Wobble {
    pub const NONE: Self = Self {
        depth: 0.0,
        hz: 1.0,
        shape: Shape::Sine,
    };

    /// How far it moves the read, in seconds either way, once centred: a sine's reach, whatever
    /// the shape.
    pub fn reach_seconds(&self) -> f64 {
        self.depth.abs() / (TAU * self.hz.max(0.01))
    }
}

/// A random wobble's RMS speed error, as a share of its depth: points uniform in ±1 of its reach,
/// [`POINTS_PER_CYCLE`] a cycle, the read straight between them, so its speed is the difference
/// of two such points (variance 2/3) times the points' rate.
pub fn random_rms_share() -> f64 {
    POINTS_PER_CYCLE * (2.0f64 / 3.0).sqrt() / TAU
}

/// A xorshift32 generator (George Marsaglia, "Xorshift RNGs", 2003): small, fast, seeded.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u32);

impl Default for Rng {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        // Zero is xorshift's one fixed point; multiplying by an odd number keeps seeds distinct.
        Self(seed.wrapping_mul(2_654_435_761).max(1))
    }

    /// A uniform value in -1..1.
    #[inline]
    pub fn bipolar(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        f64::from(x) / f64::from(u32::MAX) * 2.0 - 1.0
    }
}

/// The leak's corner, as a share of the wobble's rate.
const LEAK: f64 = 1.0 / 8.0;
/// A random wobble's points a cycle: the reference draws one every 16 384 samples at 48 kHz for a
/// 1 Hz wobble, at every rate we measured.
pub const POINTS_PER_CYCLE: f64 = 48_000.0 / 16_384.0;
/// How far a following flutter's rate swings at the wow's peak, in hertz at full Wow (a Wow of
/// 1): the reference's, at every flutter and wow rate we measured.
pub const FOLLOW_HZ: f64 = 4.5;
/// The slowest a following flutter goes, as a share of its own rate: the reference stops a 5 Hz
/// flutter at 2.5 Hz.
pub const FOLLOW_FLOOR: f64 = 0.5;
/// The two-harmonic saw's peak, `sin θ − ½·sin 2θ` at θ = 2π/3 (3√3/4): what scales it to ±1.
const SAW_PEAK: f64 = 1.299_038_105_676_658;
/// A windowed-sinc read's taps either side of the point read: 32 in all.
pub const SINC_HALF: usize = 16;
const SINC_TAPS: usize = 2 * SINC_HALF;
/// The table's rows across one sample, read between in a straight line.
const SINC_PHASES: usize = 256;
/// The Kaiser window's shape: flat within a hundredth of a decibel to 0.42 of the rate (20 kHz
/// at 48 kHz), 0.4 dB down at 0.45, where Hermite is down 12 dB.
const SINC_BETA: f64 = 7.0;
/// The nearest any read comes to the sample just written: the windowed sinc's reach ahead.
pub const READ_AHEAD: f64 = SINC_HALF as f64;

/// A table of Kaiser-windowed sinc kernels, one row a step of the fraction, each summing to one,
/// the first and last exactly the sample itself. Allocated in [`Sinc::new`].
#[derive(Clone, Debug)]
pub struct Sinc {
    rows: Vec<[f32; SINC_TAPS]>,
}

impl Default for Sinc {
    fn default() -> Self {
        Self::new()
    }
}

impl Sinc {
    pub fn new() -> Self {
        let rows = (0..=SINC_PHASES)
            .map(|phase| {
                let mut row = [0.0f32; SINC_TAPS];
                if phase == 0 || phase == SINC_PHASES {
                    row[SINC_HALF - 1 + phase / SINC_PHASES] = 1.0;
                    return row;
                }
                let fraction = phase as f64 / SINC_PHASES as f64;
                let mut taps = [0.0f64; SINC_TAPS];
                for (k, tap) in taps.iter_mut().enumerate() {
                    let t = k as f64 - (SINC_HALF - 1) as f64 - fraction;
                    let x = t / SINC_HALF as f64;
                    let window =
                        bessel_i0(SINC_BETA * (1.0 - x * x).max(0.0).sqrt()) / bessel_i0(SINC_BETA);
                    *tap = (PI * t).sin() / (PI * t) * window;
                }
                let sum: f64 = taps.iter().sum();
                for (w, tap) in row.iter_mut().zip(taps) {
                    *w = (tap / sum) as f32;
                }
                row
            })
            .collect();
        Self { rows }
    }

    /// The kernel for a point `fraction` (0..1) past a sample: its taps from `SINC_HALF − 1`
    /// samples before that sample to `SINC_HALF` after.
    #[inline]
    pub fn weights(&self, fraction: f64) -> [f32; SINC_TAPS] {
        let scaled = fraction * SINC_PHASES as f64;
        let row = (scaled as usize).min(SINC_PHASES - 1);
        let between = (scaled - row as f64) as f32;
        let (a, b) = (&self.rows[row], &self.rows[row + 1]);
        let mut w = [0.0f32; SINC_TAPS];
        for ((w, a), b) in w.iter_mut().zip(a).zip(b) {
            *w = a + between * (b - a);
        }
        w
    }

    /// The point `fraction` past `line[index]`, the line circular.
    #[inline]
    pub fn read(&self, line: &[f32], index: usize, fraction: f64) -> f64 {
        let length = line.len();
        let w = self.weights(fraction);
        let first = (index + length - (SINC_HALF - 1)) % length;
        if first + SINC_TAPS <= length {
            line[first..first + SINC_TAPS]
                .iter()
                .zip(&w)
                .map(|(x, w)| f64::from(*x) * f64::from(*w))
                .sum()
        } else {
            (0..SINC_TAPS)
                .map(|k| f64::from(line[(first + k) % length]) * f64::from(w[k]))
                .sum()
        }
    }
}

/// A tape let go while it is still slowing stops in this long from full speed, then starts.
/// Chosen: a quick brake, so the tape starts soon after it is let go.
pub const BRAKE_S: f64 = 0.05;

/// Where a tape stop has got to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Running,
    /// Slowing at the stop time's pace, or, let go, braking.
    Stopping {
        braking: bool,
    },
    Stopped,
    /// Getting back to speed: the start's step.
    Starting(u32),
}

/// A tape stop: its phase, the speed as a share of the tape's own, and how far the read is behind
/// the base delay (ahead of it while starting), in samples.
#[derive(Clone, Copy, Debug)]
struct Stop {
    phase: Phase,
    speed: f64,
    lag: f64,
}

impl Stop {
    /// Running, or stopped if the stop is held.
    fn at_rest(held: bool) -> Self {
        Self {
            phase: if held { Phase::Stopped } else { Phase::Running },
            speed: if held { 0.0 } else { 1.0 },
            lag: 0.0,
        }
    }
}

/// A shape's read at `phase` (a random wobble's at `point`, where it is between its points), ±1:
/// a sine's read is a cosine, so its speed (`dD/dt = −δ`) is a sine.
fn wave(shape: Shape, phase: f64, point: f64) -> f64 {
    let theta = TAU * phase;
    let saw = || (theta.sin() - 0.5 * (2.0 * theta).sin()) / SAW_PEAK;
    match shape {
        Shape::Sine => theta.cos(),
        Shape::Saw => saw(),
        Shape::ReverseSaw => -saw(),
        Shape::Random => point,
        Shape::Warp => crate::warp::read(phase),
    }
}

/// Where a wobble puts the read, in samples: its wave times its reach.
fn centred(wobble: &Wobble, phase: f64, point: f64, rate: f64) -> f64 {
    wobble.depth * rate / (TAU * wobble.hz.max(1e-6)) * wave(wobble.shape, phase, point)
}

/// One wobble's running state.
#[derive(Clone, Copy, Debug, Default)]
struct Oscillator {
    phase: f64,
    /// The random shape's last point and its next, ±1, and how far it is from one to the other.
    points: [f64; 2],
    between: f64,
    /// What the read's offset carries besides the centred wave, in samples: the jumps a change
    /// made, leaking away.
    drift: f64,
    /// Last sample's wobble, to see a change; none before the first sample, so a reset starts
    /// centred.
    last: Option<Wobble>,
    /// The offset this wobble made last sample, in samples.
    offset: f64,
    /// The shape's value this sample, about −1..1: what a wobble that follows this one reads.
    level: f64,
}

impl Oscillator {
    /// Steps one sample and returns the read's offset, in samples.
    #[inline]
    fn next(&mut self, wobble: &Wobble, rate: f64, rng: &mut Rng) -> f64 {
        let step = wobble.hz / rate;
        let previous = self.phase;
        self.phase = (previous + step).fract();
        self.between += POINTS_PER_CYCLE * step;
        // The first sample after a reset draws too, so a random wobble sets out from the centre at
        // once rather than a point's time later.
        if self.between >= 1.0 || self.last.is_none() {
            self.between = self.between.fract();
            // Only a random wobble draws; any other heads for the centre, so that a change to the
            // random shape starts from there.
            let next = if wobble.shape == Shape::Random {
                rng.bipolar()
            } else {
                0.0
            };
            self.points = [self.points[1], next];
        }
        let point = self.points[0] + (self.points[1] - self.points[0]) * self.between;
        let now = centred(wobble, self.phase, point, rate);
        // Where last sample's wobble would have taken the read, at its own rate: the offset follows
        // it exactly, and the difference goes into the drift.
        let before = self.last.map_or(now, |last| {
            centred(&last, (previous + last.hz / rate).fract(), point, rate)
        });
        self.last = Some(*wobble);
        self.drift = self.drift * (1.0 - TAU * step * LEAK) + before - now;
        self.level = match wobble.shape {
            Shape::Sine => (TAU * self.phase).sin(),
            shape => wave(shape, self.phase, point),
        };
        self.offset = now + self.drift;
        self.offset
    }
}

/// A stereo delay line read at a moving position.
#[derive(Clone, Debug)]
pub struct Transport {
    lines: [Vec<f32>; 2],
    write: usize,
    rate: f64,
    /// The base delay, in samples: the latency the plugin reports.
    base: f64,
    wow: Oscillator,
    flutter: Oscillator,
    /// Three more wobbles the read follows, set from outside: a record's eccentricity, its
    /// turntable's own wobble and its warp.
    extras: [Wobble; 3],
    extra: [Oscillator; 3],
    rng: Rng,
    seed: u32,
    /// The reach the wobbles share, seconds either way.
    room_s: f64,
    /// The delay between the two tracks' reads, samples: the left half of it sooner, the right as
    /// much later.
    tracks: f64,
    /// How far the wobbles may take the read past the base, in samples: twice their reach.
    wobble_room: f64,
    /// How far a stop may take it on past that.
    stop_room: f64,
    stop: Stop,
    /// The stop held, and the stop's pace: the speed it loses a sample.
    held: bool,
    pace: f64,
    /// High quality's read, and whether it is on.
    sinc: Sinc,
    high: bool,
}

impl Transport {
    /// A transport at `rate` whose wobbles share `reach_seconds`: together they move the read no
    /// further either way, lowered alike if they would. No room for a stop. Allocates; call it
    /// outside the audio thread.
    pub fn new(rate: f64, reach_seconds: f64, seed: u32) -> Self {
        Self::with_stop(rate, reach_seconds, seed, 0.0)
    }

    /// A transport as [`Transport::new`] with room for a stop of up to `longest_stop_s` from full
    /// speed. Allocates; call it outside the audio thread.
    pub fn with_stop(rate: f64, reach_seconds: f64, seed: u32, longest_stop_s: f64) -> Self {
        let reach = (reach_seconds * rate).ceil();
        // Below the base: twice the reach for a change not yet re-centred, and the windowed sinc's
        // reach ahead and two. Above it: twice the reach, and room to read behind.
        let base = 2.0 * reach + READ_AHEAD + 2.0;
        let wobble_room = 2.0 * reach + 2.0;
        // A stop from full speed loses half its time; a brake after it at most its own.
        let stop_room = if longest_stop_s > 0.0 {
            ((0.5 * longest_stop_s + BRAKE_S) * rate).ceil() + 2.0
        } else {
            0.0
        };
        let length = (base + wobble_room + stop_room) as usize + SINC_HALF + 2;
        Self {
            lines: [vec![0.0; length], vec![0.0; length]],
            write: 0,
            rate,
            base,
            wow: Oscillator::default(),
            flutter: Oscillator::default(),
            extras: [Wobble::NONE; 3],
            extra: [Oscillator::default(); 3],
            rng: Rng::new(seed),
            seed,
            room_s: reach_seconds,
            tracks: 0.0,
            wobble_room,
            stop_room,
            stop: Stop::at_rest(false),
            held: false,
            pace: 1.0,
            sinc: Sinc::new(),
            high: false,
        }
    }

    /// High quality's read, a windowed sinc, from the next sample on.
    pub fn set_high(&mut self, high: bool) {
        self.high = high;
    }

    /// The base delay in whole samples: the latency to report.
    pub fn latency(&self) -> u32 {
        self.base as u32
    }

    /// Back to the start: silence in the line, the wobbles centred at phase zero, the seed's first
    /// draw, and the tape running, or stopped if the stop is held. Only the stretch a running read
    /// reaches is emptied: a stop's read falls behind no faster than the tape is written, so it
    /// never reaches past the start, and a long line costs no more to reset than a short one.
    pub fn reset(&mut self) {
        let length = self.lines[0].len();
        let near = ((self.base + self.wobble_room) as usize + SINC_HALF + 2).min(length);
        for line in &mut self.lines {
            line[length - near..].fill(0.0);
        }
        self.write = 0;
        self.stop = Stop::at_rest(self.held);
        self.wow = Oscillator::default();
        self.flutter = Oscillator::default();
        self.extra = [Oscillator::default(); 3];
        self.rng = Rng::new(self.seed);
    }

    /// The three further wobbles the read follows from the next sample on; their reach counts
    /// against the reach the transport was made with.
    pub fn set_extras(&mut self, extras: [Wobble; 3]) {
        self.extras = extras;
    }

    /// Where extra wobble `index` will be in its cycle (0..1) when a sample written next is read,
    /// a base delay on: what something riding that wobble before the line goes by, so the two
    /// line up at the read.
    pub fn extra_phase_when_read(&self, index: usize) -> f64 {
        let ahead = (self.base + 1.0) * self.extras[index].hz / self.rate;
        (self.extra[index].phase + ahead).fract()
    }

    /// The delay between the two tracks' reads, samples, from the next sample on: the left read
    /// half of it sooner than the moving read, the right as much later. It counts against the
    /// room the wobbles share.
    pub fn set_tracks(&mut self, samples: f64) {
        self.tracks = samples.max(0.0);
    }

    /// The tape stop: held, the tape slows to a halt over `seconds` from full speed and stays
    /// stopped; let go, it starts again. From the next sample on.
    pub fn set_stop(&mut self, held: bool, seconds: f64) {
        self.held = held;
        self.pace = 1.0 / (seconds * self.rate).max(1.0);
    }

    /// The tape's speed this sample, as a share of its own: 1 running, 0 stopped.
    pub fn speed(&self) -> f64 {
        self.stop.speed
    }

    /// How far behind the base delay the read will be when the stop under way comes to rest, in
    /// samples: what the sound still in the line has left to play out. Zero unless it is slowing.
    pub fn lag_to_come(&self) -> f64 {
        let Phase::Stopping { braking } = self.stop.phase else {
            return 0.0;
        };
        let pace = if braking {
            self.pace.max(1.0 / (BRAKE_S * self.rate))
        } else {
            self.pace
        };
        // n more steps of the speed falling by `pace`: Σ (1 − (s − k·pace)), k = 1..n.
        let n = (self.stop.speed / pace).ceil();
        (self.stop.lag + n * (1.0 - self.stop.speed) + pace * n * (n + 1.0) / 2.0).max(0.0)
    }

    /// One sample of the tape stop: the speed's and the read's lag's next step.
    fn step_stop(&mut self) {
        let start = self.base.max(1.0);
        let stop = &mut self.stop;
        stop.phase = match stop.phase {
            Phase::Running if self.held => Phase::Stopping { braking: false },
            Phase::Starting(_) if self.held => Phase::Stopping { braking: false },
            Phase::Stopping { .. } => Phase::Stopping {
                braking: !self.held,
            },
            phase => phase,
        };
        match stop.phase {
            Phase::Stopping { braking } => {
                let pace = if braking {
                    self.pace.max(1.0 / (BRAKE_S * self.rate))
                } else {
                    self.pace
                };
                stop.speed = (stop.speed - pace).max(0.0);
                stop.lag += 1.0 - stop.speed;
                if stop.speed == 0.0 {
                    stop.phase = if braking {
                        Phase::Starting(0)
                    } else {
                        Phase::Stopped
                    };
                }
            }
            Phase::Stopped if !self.held => stop.phase = Phase::Starting(0),
            Phase::Starting(step) => {
                // The k-th step of `start` puts the speed at k / start and the read
                // −(start − 1)/2 + k − k(k + 1)/(2·start) from the base: on it at the last.
                let k = f64::from(step + 1);
                if k >= start {
                    *stop = Stop::at_rest(false);
                } else {
                    stop.speed = k / start;
                    stop.lag = -(start - 1.0) / 2.0 + k - k * (k + 1.0) / (2.0 * start);
                    stop.phase = Phase::Starting(step + 1);
                }
            }
            _ => {}
        }
        if let Phase::Starting(0) = stop.phase {
            // Stopped, and silent: the read is put ahead by what the start will lose.
            stop.speed = 0.0;
            stop.lag = -(start - 1.0) / 2.0;
        }
    }

    /// A new seed for the random shapes, from the next draw on; the line and phases stay.
    pub fn reseed(&mut self, seed: u32) {
        if seed != self.seed {
            self.seed = seed;
            self.rng = Rng::new(seed);
        }
    }

    /// One stereo sample through the moving read, with this sample's wow and flutter.
    #[inline]
    pub fn process(&mut self, input: [f32; 2], wow: &Wobble, flutter: &Wobble) -> [f32; 2] {
        self.process_full(input, wow, flutter, None).0
    }

    /// One stereo sample: the moving read, and the line read at exactly the base delay (the tape
    /// as it would play with no wobble, for flanging against). With `follow`, the flutter follows
    /// the wow: the wow's own read eases home, and the flutter's rate swings by `follow` hertz at
    /// the wow's peak, never below [`FOLLOW_FLOOR`] of itself.
    #[inline]
    pub fn process_full(
        &mut self,
        input: [f32; 2],
        wow: &Wobble,
        flutter: &Wobble,
        follow: Option<f64>,
    ) -> ([f32; 2], [f32; 2]) {
        let length = self.lines[0].len();
        for (line, &x) in self.lines.iter_mut().zip(&input) {
            line[self.write] = x;
        }
        let wow = match follow {
            Some(_) => Wobble { depth: 0.0, ..*wow },
            None => *wow,
        };
        // The room: what the tracks' delay leaves, shared by every wobble, a following flutter at
        // its floor. Past it, each is lowered by the same share.
        let floor = if follow.is_some() { FOLLOW_FLOOR } else { 1.0 };
        let total = wow.reach_seconds()
            + Wobble {
                hz: flutter.hz * floor,
                ..*flutter
            }
            .reach_seconds()
            + self.extras.iter().map(Wobble::reach_seconds).sum::<f64>();
        let room = (self.room_s - 0.5 * self.tracks / self.rate).max(0.0);
        let share = if total > room { room / total } else { 1.0 };
        let fit = |wobble: Wobble| {
            if share < 1.0 {
                Wobble {
                    depth: wobble.depth * share,
                    ..wobble
                }
            } else {
                wobble
            }
        };
        let wow = fit(wow);
        let flutter = &fit(*flutter);
        let wow_offset = self.wow.next(&wow, self.rate, &mut self.rng);
        let flutter = match follow {
            Some(swing) => Wobble {
                hz: (flutter.hz + swing * self.wow.level).max(FOLLOW_FLOOR * flutter.hz),
                ..*flutter
            },
            None => *flutter,
        };
        let mut offset = wow_offset + self.flutter.next(&flutter, self.rate, &mut self.rng);
        for (oscillator, wobble) in self.extra.iter_mut().zip(&self.extras) {
            offset += oscillator.next(&fit(*wobble), self.rate, &mut self.rng);
        }
        self.step_stop();
        let (out, fixed) = if self.stop.speed == 0.0 {
            // Stopped: nothing passes the head.
            ([0.0; 2], [0.0; 2])
        } else {
            let wobbled = offset.clamp(-(self.base - READ_AHEAD), self.wobble_room);
            let lag = self.stop.lag;
            let most = self.base + self.wobble_room + self.stop_room;
            let half = 0.5 * self.tracks;
            let at = |centre: f64| {
                [
                    (centre - half).clamp(READ_AHEAD, most),
                    (centre + half).clamp(READ_AHEAD, most),
                ]
            };
            let moving = self.read(at(self.base + wobbled + lag));
            let fixed = if lag == 0.0 && half == 0.0 {
                let still = (self.write + length - self.base as usize) % length;
                [self.lines[0][still], self.lines[1][still]]
            } else {
                self.read(at(self.base + lag))
            };
            (moving, fixed)
        };
        self.write = (self.write + 1) % length;
        (out, fixed)
    }

    /// Each channel read its own `delays` samples behind the sample just written.
    #[inline]
    fn read(&self, delays: [f64; 2]) -> [f32; 2] {
        let length = self.lines[0].len();
        let mut out = [0.0f32; 2];
        for ((y, line), delay) in out.iter_mut().zip(&self.lines).zip(delays) {
            let position = self.write as f64 - delay + length as f64;
            let whole = position.floor();
            let fraction = position - whole;
            let index = whole as usize;
            *y = if self.high && fraction != 0.0 {
                self.sinc.read(line, index % length, fraction) as f32
            } else {
                let at = |k: usize| f64::from(line[(index + k + length - 1) % length]);
                hermite(at(0), at(1), at(2), at(3), fraction) as f32
            };
        }
        out
    }
}

/// Four-point, third-order Hermite: the value between `x0` and `x1` at `t` (0..1), from the
/// points before and after.
#[inline]
pub(crate) fn hermite(xm1: f64, x0: f64, x1: f64, x2: f64, t: f64) -> f64 {
    let c1 = 0.5 * (x1 - xm1);
    let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
    let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
    ((c3 * t + c2) * t + c1) * t + x0
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    /// The instantaneous frequency of a rendered tone, by zero crossings, as a speed error.
    fn speed_errors(samples: &[f32], hz: f64) -> Vec<f64> {
        let mut crossings = Vec::new();
        for n in 1..samples.len() {
            if samples[n - 1] <= 0.0 && samples[n] > 0.0 {
                let t = f64::from(samples[n - 1]) / f64::from(samples[n - 1] - samples[n]);
                crossings.push(n as f64 - 1.0 + t);
            }
        }
        crossings
            .windows(2)
            .map(|w| RATE / (w[1] - w[0]) / hz - 1.0)
            .collect()
    }

    /// A 3150 Hz tone through the transport, and how far the read went, as a share of the reach,
    /// after the first two cycles.
    fn render(wow: Wobble, flutter: Wobble, seconds: f64, seed: u32) -> (Vec<f32>, f64) {
        let reach = wow.reach_seconds() + flutter.reach_seconds();
        let mut transport = Transport::new(RATE, reach, seed);
        let settled = 2.0 * RATE / wow.hz.min(flutter.hz);
        let mut furthest = 0.0f64;
        let out = (0..(seconds * RATE) as usize)
            .map(|n| {
                let x = (TAU * 3150.0 * n as f64 / RATE).sin() as f32;
                let y = transport.process([x, x], &wow, &flutter)[0];
                if n as f64 > settled {
                    let offset = transport.wow.offset + transport.flutter.offset;
                    furthest = furthest.max(offset.abs());
                }
                y
            })
            .collect();
        (out, furthest / RATE / reach.max(1e-12))
    }

    fn rms(values: &[f64]) -> f64 {
        (values.iter().map(|e| e * e).sum::<f64>() / values.len() as f64).sqrt()
    }

    #[test]
    fn a_sine_wow_moves_the_speed_by_its_depth_within_its_reach() {
        let wow = Wobble {
            depth: 0.01,
            hz: 1.0,
            shape: Shape::Sine,
        };
        let (out, furthest) = render(wow, Wobble::NONE, 6.0, 1);
        // RMS rather than peak: the measurement's own jitter would raise a peak.
        let rms = rms(&speed_errors(&out[RATE as usize..], 3150.0));
        let expected = 0.01 * std::f64::consts::FRAC_1_SQRT_2;
        assert!(
            (rms / expected - 1.0).abs() < 0.02,
            "rms {rms}, expected {expected}"
        );
        assert!(furthest < 1.001, "the read went {furthest} times its reach");
    }

    #[test]
    fn a_saw_wow_moves_the_read_as_far_as_a_sine_within_its_reach() {
        let wow = Wobble {
            depth: 0.01,
            hz: 2.0,
            shape: Shape::Saw,
        };
        let (out, furthest) = render(wow, Wobble::NONE, 6.0, 1);
        // Its speed is d·(cos θ − cos 2θ) / SAW_PEAK: an RMS of d / SAW_PEAK, 0.77 d, as the
        // reference's measured saw.
        let rms = rms(&speed_errors(&out[RATE as usize..], 3150.0));
        let expected = 0.01 / SAW_PEAK;
        assert!(
            (rms / expected - 1.0).abs() < 0.03,
            "rms {rms}, expected {expected}"
        );
        assert!(furthest < 1.001, "the read went {furthest} times its reach");
    }

    /// A warp's speed is the reference's shape at [`crate::warp::FULL`] for a warp of 1, its read
    /// as far as a sine of its depth.
    #[test]
    fn a_warp_moves_the_speed_in_the_references_shape_within_its_reach() {
        let wow = Wobble {
            depth: crate::warp::depth(1.0),
            hz: 0.5,
            shape: Shape::Warp,
        };
        let (out, furthest) = render(wow, Wobble::NONE, 9.0, 1);
        let mut crossings = Vec::new();
        for n in RATE as usize..out.len() {
            if out[n - 1] <= 0.0 && out[n] > 0.0 {
                let t = f64::from(out[n - 1]) / f64::from(out[n - 1] - out[n]);
                crossings.push(n as f64 - 1.0 + t);
            }
        }
        // Against the shape, 32 periods at a time (10 ms), which averages the measurement's own
        // jitter away and keeps any error of the shape.
        let (mut off, mut shape) = (Vec::new(), Vec::new());
        for block in crossings.chunks(32).filter(|block| block.len() == 32) {
            let (first, last) = (block[0], block[31]);
            let measured = RATE * 31.0 / (last - first) / 3150.0 - 1.0;
            let samples = (first.ceil() as usize..=last.floor() as usize).step_by(8);
            let expected: Vec<f64> = samples
                .map(|n| crate::warp::FULL * crate::warp::speed((n as f64 / RATE * wow.hz).fract()))
                .collect();
            let expected = expected.iter().sum::<f64>() / expected.len() as f64;
            off.push(measured - expected);
            shape.push(expected);
        }
        let share = rms(&off) / rms(&shape);
        assert!(share < 0.005, "{share} of the shape off it");
        assert!(furthest < 1.001, "the read went {furthest} times its reach");
    }

    /// What rides an extra wobble before the line, by its phase when read, lines up with the
    /// read: a base delay and a sample on, the wobble is there.
    #[test]
    fn an_extra_wobbles_phase_when_read_is_where_it_is_at_the_read() {
        let warp = Wobble {
            depth: 1e-4,
            hz: 1.3,
            shape: Shape::Warp,
        };
        let mut transport = Transport::new(RATE, 0.01, 1);
        transport.set_extras([Wobble::NONE, Wobble::NONE, warp]);
        let step = |t: &mut Transport| t.process([0.0, 0.0], &Wobble::NONE, &Wobble::NONE);
        for _ in 0..1_000 {
            step(&mut transport);
        }
        let predicted = transport.extra_phase_when_read(2);
        for _ in 0..=transport.latency() {
            step(&mut transport);
        }
        let apart = (transport.extra[2].phase - predicted).abs();
        assert!(apart.min(1.0 - apart) < 1e-9, "{apart}");
    }

    #[test]
    fn a_random_wobble_has_about_its_depth_and_stays_within_its_reach() {
        let wow = Wobble {
            depth: 0.01,
            hz: 2.0,
            shape: Shape::Random,
        };
        let (out, furthest) = render(wow, Wobble::NONE, 60.0, 3);
        let errors = speed_errors(&out[RATE as usize..], 3150.0);
        // About 350 points: their RMS within a tenth of the expected, which the reference's
        // measured random holds too (0.37–0.42 of its depth).
        let expected = 0.01 * random_rms_share();
        let rms = rms(&errors);
        assert!(
            (rms / expected - 1.0).abs() < 0.1,
            "rms {rms}, expected about {expected}"
        );
        assert!(furthest <= 1.0, "the read went {furthest} times its reach");
    }

    #[test]
    fn a_random_wobble_reads_straight_between_its_points() {
        let wow = Wobble {
            depth: 0.01,
            hz: 3.0,
            shape: Shape::Random,
        };
        let seconds = 10.0;
        let mut transport = Transport::new(RATE, wow.reach_seconds(), 2);
        let offsets: Vec<f64> = (0..(seconds * RATE) as usize)
            .map(|_| {
                transport.process([0.0, 0.0], &wow, &Wobble::NONE);
                transport.wow.offset
            })
            .collect();
        // The read bends only at a point: its speed holds between them. A point between two
        // samples bends the read at both, so a run of bent samples is one bend.
        let bent: Vec<bool> = offsets
            .windows(3)
            .map(|w| (w[2] - 2.0 * w[1] + w[0]).abs() > 1e-9)
            .collect();
        let bends = bent.windows(2).filter(|pair| pair[1] && !pair[0]).count();
        let points = POINTS_PER_CYCLE * wow.hz * seconds;
        assert!(
            (bends as f64 - points).abs() <= 2.0,
            "{bends} bends, {points} points"
        );
    }

    #[test]
    fn the_random_rms_share_is_the_points_spread() {
        // Uniform points in ±1 (variance 1/3), the speed their difference over a point's time.
        let share = random_rms_share();
        assert!((share - 0.3807).abs() < 1e-4, "{share}");
    }

    #[test]
    fn a_change_of_rate_does_not_jump_the_read_and_settles_back_to_centre() {
        let slow = Wobble {
            depth: 0.01,
            hz: 1.0,
            shape: Shape::Sine,
        };
        let fast = Wobble { hz: 4.0, ..slow };
        let mut transport = Transport::new(RATE, slow.reach_seconds(), 1);
        let mut previous = 0.0f64;
        let mut largest_step = 0.0f64;
        for n in 0..(8.0 * RATE) as usize {
            let wow = if n < (1.25 * RATE) as usize {
                slow
            } else {
                fast
            };
            transport.process([0.0, 0.0], &wow, &Wobble::NONE);
            let offset = transport.wow.offset;
            if n > 0 {
                largest_step = largest_step.max((offset - previous).abs());
            }
            previous = offset;
        }
        // No step faster than the wobble's own speed allows, give or take the leak.
        assert!(
            largest_step < 0.0102,
            "the read stepped {largest_step} samples in one sample"
        );
        // Long after, the read is the fast wobble's own, centred.
        assert!(transport.wow.drift.abs() < 1e-3 * RATE * fast.reach_seconds());
    }

    #[test]
    fn the_same_seed_renders_the_same_and_another_does_not() {
        let wow = Wobble {
            depth: 0.005,
            hz: 0.7,
            shape: Shape::Random,
        };
        let flutter = Wobble {
            depth: 0.001,
            hz: 8.0,
            shape: Shape::Random,
        };
        let a = render(wow, flutter, 1.0, 7).0;
        assert_eq!(a, render(wow, flutter, 1.0, 7).0);
        assert_ne!(a, render(wow, flutter, 1.0, 8).0);
    }

    #[test]
    fn reset_starts_the_same_render_again() {
        let wow = Wobble {
            depth: 0.004,
            hz: 1.3,
            shape: Shape::Random,
        };
        let mut transport = Transport::new(RATE, wow.reach_seconds(), 5);
        let run = |t: &mut Transport| -> Vec<f32> {
            (0..24_000)
                .map(|n| {
                    let x = (TAU * 440.0 * n as f64 / RATE).sin() as f32;
                    t.process([x, -x], &wow, &Wobble::NONE)[1]
                })
                .collect()
        };
        let first = run(&mut transport);
        transport.reset();
        assert_eq!(first, run(&mut transport));
    }

    #[test]
    fn the_fixed_read_is_the_base_delay_whatever_the_wobble() {
        let wow = Wobble {
            depth: 0.01,
            hz: 1.0,
            shape: Shape::Random,
        };
        let mut transport = Transport::new(RATE, wow.reach_seconds(), 1);
        let latency = transport.latency() as usize;
        let fixed: Vec<f32> = (0..2_000)
            .map(|n| {
                let x = if n == 10 { 1.0 } else { 0.0 };
                transport.process_full([x, x], &wow, &Wobble::NONE, None).1[0]
            })
            .collect();
        assert_eq!(fixed.iter().position(|&y| y == 1.0), Some(10 + latency));
    }

    #[test]
    fn a_following_flutter_speeds_up_and_slows_down_with_the_wow() {
        // Flutter alone at 8 Hz, then following a slow sine wow: its cycles crowd and spread.
        let wow = Wobble {
            depth: 0.002,
            hz: 0.5,
            shape: Shape::Sine,
        };
        let flutter = Wobble {
            depth: 0.003,
            hz: 8.0,
            shape: Shape::Sine,
        };
        let count = |follow: Option<f64>| {
            let mut transport =
                Transport::new(RATE, wow.reach_seconds() + flutter.reach_seconds(), 1);
            // Cycles in the second the wow is above its centre, then in the second it is below.
            let mut halves = [0u32; 2];
            let mut previous = 0.0f64;
            for n in 0..(RATE as usize * 2) {
                transport.process_full([0.0, 0.0], &wow, &flutter, follow);
                let phase = transport.flutter.phase;
                if phase < previous {
                    halves[usize::from(n >= RATE as usize)] += 1;
                }
                previous = phase;
            }
            halves
        };
        let still = count(None);
        let moving = count(Some(FOLLOW_HZ));
        assert!(still[0].abs_diff(still[1]) <= 1, "{still:?}");
        assert!(moving[0] > moving[1] + 3, "{moving:?}");
    }

    #[test]
    fn a_wow_the_flutter_follows_moves_no_read_of_its_own() {
        let wow = Wobble {
            depth: 0.02,
            hz: 1.0,
            shape: Shape::Sine,
        };
        let mut transport = Transport::new(RATE, wow.reach_seconds(), 1);
        let latency = transport.latency() as usize;
        let out: Vec<f32> = (0..(2.0 * RATE) as usize)
            .map(|n| {
                let x = if n == 10 { 1.0 } else { 0.0 };
                transport
                    .process_full([x, x], &wow, &Wobble::NONE, Some(FOLLOW_HZ))
                    .0[0]
            })
            .collect();
        assert_eq!(out.iter().position(|&y| y == 1.0), Some(10 + latency));
        assert_eq!(out.iter().filter(|&&y| y != 0.0).count(), 1);
    }

    #[test]
    fn a_following_flutter_slows_no_further_than_its_floor() {
        // A deep swing about a slow flutter: its rate stops at FOLLOW_FLOOR of itself.
        let wow = Wobble {
            depth: 0.0,
            hz: 0.25,
            shape: Shape::Sine,
        };
        let flutter = Wobble {
            depth: 0.001,
            hz: 4.0,
            shape: Shape::Sine,
        };
        let mut transport = Transport::new(RATE, 0.01, 1);
        let mut previous = 0.0f64;
        let mut longest = 0usize;
        let mut since = 0usize;
        for _ in 0..(8.0 * RATE) as usize {
            transport.process_full([0.0, 0.0], &wow, &flutter, Some(2.0 * FOLLOW_HZ));
            since += 1;
            if transport.flutter.phase < previous {
                longest = longest.max(since);
                since = 0;
            }
            previous = transport.flutter.phase;
        }
        let slowest = RATE / longest as f64;
        assert!(
            (slowest / (FOLLOW_FLOOR * flutter.hz) - 1.0).abs() < 0.02,
            "slowest cycle {slowest} Hz"
        );
    }

    /// **The windowed sinc reads a tone between samples flat to 20 kHz at 48 kHz**, where Hermite
    /// dulls it, and reads a sample itself exactly.
    #[test]
    fn the_sinc_reads_between_samples_flat_to_the_top() {
        let sinc = Sinc::new();
        let n = 4_096;
        for hz in [1_000.0, 12_000.0, 20_000.0] {
            let line: Vec<f32> = (0..n)
                .map(|i| (TAU * hz * i as f64 / RATE).sin() as f32)
                .collect();
            for fraction in [0.25, 0.5, 0.8] {
                let worst = (100..n - 100)
                    .map(|i| {
                        let exact = (TAU * hz * (i as f64 + fraction) / RATE).sin();
                        (sinc.read(&line, i, fraction) - exact).abs()
                    })
                    .fold(0.0f64, f64::max);
                assert!(worst < 2e-3, "{hz} Hz at {fraction}: {worst}");
            }
        }
        let line: Vec<f32> = (0..64).map(|i| i as f32).collect();
        assert_eq!(sinc.read(&line, 20, 0.0), 20.0);
    }

    /// **High quality reads the tone a wobble moves without dulling its treble**: a 16 kHz tone
    /// through a wow keeps its level read by the sinc, and loses some of it read by Hermite.
    #[test]
    fn high_quality_keeps_the_treble_a_wobble_moves() {
        let wow = Wobble {
            depth: 0.003,
            hz: 0.7,
            shape: Shape::Sine,
        };
        let level = |high: bool| {
            let mut transport = Transport::new(RATE, wow.reach_seconds(), 1);
            transport.set_high(high);
            let out: Vec<f32> = (0..96_000)
                .map(|n| {
                    let x = (TAU * 16_000.0 * n as f64 / RATE).sin() as f32;
                    transport.process([x, x], &wow, &Wobble::NONE)[0]
                })
                .collect();
            let tail = &out[48_000..];
            (tail.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / tail.len() as f64).sqrt()
                / std::f64::consts::FRAC_1_SQRT_2
        };
        let (normal, high) = (20.0 * level(false).log10(), 20.0 * level(true).log10());
        assert!(high > -0.2, "high {high:.2} dB");
        assert!(
            normal < high - 1.0,
            "normal {normal:.2} dB, high {high:.2} dB"
        );
    }

    /// **The wobbles share one room**: together past it, each is lowered by the same share and the
    /// read stays inside it.
    #[test]
    fn wobbles_past_the_room_together_shrink_together() {
        let room = 0.002;
        let sine = |hz: f64| Wobble {
            depth: room * TAU * hz,
            hz,
            shape: Shape::Sine,
        };
        let mut transport = Transport::new(RATE, room, 1);
        transport.set_extras([sine(1.3), Wobble::NONE, Wobble::NONE]);
        let (mut together, mut wow) = (0.0f64, 0.0f64);
        for _ in 0..(8.0 * RATE) as usize {
            transport.process([0.0, 0.0], &sine(0.5), &Wobble::NONE);
            together = together.max((transport.wow.offset + transport.extra[0].offset).abs());
            wow = wow.max(transport.wow.offset.abs());
        }
        assert!(together <= room * RATE * 1.0001, "{together} samples");
        assert!(
            (wow / (0.5 * room * RATE) - 1.0).abs() < 0.01,
            "{wow} samples"
        );
    }

    /// **The tracks' delay reads the left half of it sooner and the right as much later**, about
    /// the read itself, so the latency stays.
    #[test]
    fn the_tracks_delay_splits_either_side_of_the_read() {
        let mut transport = Transport::new(RATE, 0.001, 1);
        transport.set_tracks(4.0);
        let latency = transport.latency() as usize;
        let out: Vec<[f32; 2]> = (0..2 * latency)
            .map(|n| {
                let x = if n == 3 { 1.0 } else { 0.0 };
                transport.process([x, x], &Wobble::NONE, &Wobble::NONE)
            })
            .collect();
        let peak = |ch: usize| {
            (0..out.len())
                .max_by(|&a, &b| out[a][ch].abs().total_cmp(&out[b][ch].abs()))
                .unwrap()
        };
        assert_eq!(peak(0), 3 + latency - 2);
        assert_eq!(peak(1), 3 + latency + 2);
    }

    #[test]
    fn no_wobble_is_a_plain_delay_of_its_latency() {
        let mut transport = Transport::new(RATE, 0.001, 1);
        let latency = transport.latency() as usize;
        let out: Vec<f32> = (0..1000)
            .map(|n| {
                let x = if n == 10 { 1.0 } else { 0.0 };
                transport.process([x, x], &Wobble::NONE, &Wobble::NONE)[0]
            })
            .collect();
        assert_eq!(out.iter().position(|&y| y == 1.0), Some(10 + latency));
        assert_eq!(out.iter().filter(|&&y| y != 0.0).count(), 1);
    }
}
