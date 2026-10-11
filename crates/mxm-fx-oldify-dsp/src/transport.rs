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
//! [`POINTS_PER_CYCLE`] a cycle: its speed holds, then steps, about the rate.
//!
//! **Centring.** Each shape's read is centred, so its speed has exactly its shape. A change of
//! depth, rate or shape would make the read jump; the jump goes into a drift term instead, so the
//! read never jumps, and the drift leaks away through a first-order high-pass at an eighth of the
//! wobble's rate, within a cycle or two. Until a change has leaked away the read can swing up to
//! twice its reach one way, so the base delay is twice the reach.
//!
//! **Reading between samples:** four-point, third-order Hermite interpolation (Catmull–Rom; Olli
//! Niemitalo, "Polynomial Interpolators for High-Quality Resampling of Oversampled Audio", 2001),
//! flat enough at these small speed changes.
//!
//! **A following flutter** is the wow's other use, the reference's: the wow moves no read of its
//! own, and swings the flutter's rate instead ([`FOLLOW_HZ`]), never below [`FOLLOW_FLOOR`] of it.
//!
//! Every random draw comes from the transport's own seeded generator, one at each of a random
//! wobble's points, so a render with the same seed is the same at any block size.

use std::f64::consts::TAU;

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
    /// Two more wobbles the read follows, set from outside: a record's eccentricity and its
    /// turntable's own wobble.
    extras: [Wobble; 2],
    extra: [Oscillator; 2],
    rng: Rng,
    seed: u32,
}

impl Transport {
    /// A transport at `rate` whose wobbles reach up to `reach_seconds` (the sum of their
    /// [`Wobble::reach_seconds`] at the most the parameters allow). Allocates; call it outside the
    /// audio thread.
    pub fn new(rate: f64, reach_seconds: f64, seed: u32) -> Self {
        let reach = (reach_seconds * rate).ceil();
        // Below the base: twice the reach for a change not yet re-centred, and four samples for the
        // interpolator to read ahead of. Above it: twice the reach, and room to read behind.
        let base = 2.0 * reach + 4.0;
        let length = (base + 2.0 * reach) as usize + 8;
        Self {
            lines: [vec![0.0; length], vec![0.0; length]],
            write: 0,
            rate,
            base,
            wow: Oscillator::default(),
            flutter: Oscillator::default(),
            extras: [Wobble::NONE; 2],
            extra: [Oscillator::default(); 2],
            rng: Rng::new(seed),
            seed,
        }
    }

    /// The base delay in whole samples: the latency to report.
    pub fn latency(&self) -> u32 {
        self.base as u32
    }

    /// Back to the start: silence in the line, the wobbles centred at phase zero, the seed's first
    /// draw.
    pub fn reset(&mut self) {
        self.lines.iter_mut().for_each(|line| line.fill(0.0));
        self.write = 0;
        self.wow = Oscillator::default();
        self.flutter = Oscillator::default();
        self.extra = [Oscillator::default(); 2];
        self.rng = Rng::new(self.seed);
    }

    /// The two further wobbles the read follows from the next sample on; their reach counts
    /// against the reach the transport was made with.
    pub fn set_extras(&mut self, extras: [Wobble; 2]) {
        self.extras = extras;
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
            offset += oscillator.next(wobble, self.rate, &mut self.rng);
        }
        let room = length as f64 - self.base - 6.0;
        let delay = self.base + offset.clamp(-(self.base - 2.0), room);

        let position = self.write as f64 - delay + length as f64;
        let whole = position.floor();
        let fraction = position - whole;
        let index = whole as usize;
        let mut out = [0.0f32; 2];
        for (y, line) in out.iter_mut().zip(&self.lines) {
            let at = |k: usize| f64::from(line[(index + k + length - 1) % length]);
            *y = hermite(at(0), at(1), at(2), at(3), fraction) as f32;
        }
        let still = (self.write + length - self.base as usize) % length;
        let fixed = [self.lines[0][still], self.lines[1][still]];
        self.write = (self.write + 1) % length;
        (out, fixed)
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
