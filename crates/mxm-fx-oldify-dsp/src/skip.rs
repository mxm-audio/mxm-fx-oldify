//! Skips: the stylus jumping out of its groove.
//!
//! **A knock or a scratch throws the stylus into the next groove.** Back a groove, it plays the
//! last turn again; stuck there, a locked groove, it plays the same turn over and over until the
//! arm is nudged on (`ops: notes/oldify/reports`, the record half: skips and stuck grooves through
//! a one-revolution buffer). In real time the stylus can only go back, since the music a turn ahead
//! has not arrived, so a skip here is the stylus jumping back a groove and, when its repeats are
//! done or it is let go, jumping on to where the record has got to, losing the turns it repeated.
//! The music never falls behind, and the latency never moves.
//!
//! **What goes round is what was heard**: the line is written with the groove as played, skips and
//! all, so a turn repeated is the turn just heard, and a locked groove goes round on itself. Each
//! jump crosses over in [`JUMP_S`] and lands with a tick, one of the disc's own clicks
//! ([`TICK_SIZE`]); in a locked groove the tick is part of the turn and comes round with it.
//!
//! **Skips** is the chance a turn that the stylus jumps back: it repeats the turn once, and again
//! with [`RELOCK`] each time, up to [`MAX_REPEATS`]. It jumps only while the music plays, so the
//! silence after it never starts one, and only once a whole turn has played, so there is a turn to
//! go back to. **Stuck groove**, held, locks the groove: the last turn goes round for as long as it
//! is held. A turn going round keeps its length if the disc's speed changes under it.

use crate::disc::{Burst, Speed, Voice};
use crate::echo::turn_samples;
use crate::transport::Rng;

/// The most Skips: one a turn, on average, far past a real record's few in a hundred, for effect
/// (the owner, 2026-10-11: "our plugin should scale into the 'non physical' for creative use").
pub const MAX_SKIPS: f32 = 1.0;
/// A jump's crossover, seconds. Chosen: quick enough to be a jump, slow enough not to click.
pub const JUMP_S: f64 = 0.003;
/// The landing's tick, as a click's size. Chosen, about a loud click's.
pub const TICK_SIZE: f64 = 0.06;
/// The chance a skip repeats its turn once more. Chosen.
pub const RELOCK: f64 = 0.25;
/// The most turns a skip repeats.
pub const MAX_REPEATS: u32 = 3;
/// How long a tick may sound, seconds: the longest click and its filters' ring. Chosen, generous.
const TICK_S: f64 = 0.02;

/// Where the stylus is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// In the music.
    Playing,
    /// Going round a turn: the samples it has gone round, and how many it will (`None`: locked,
    /// for as long as Stuck groove is held).
    Looping { since: usize, until: Option<usize> },
}

/// A uniform draw in 0..1.
#[inline]
fn unit(rng: &mut Rng) -> f64 {
    0.5 * (rng.bipolar() + 1.0)
}

/// The stylus's jumps, from a line one turn of the slowest disc long. Allocates in
/// [`Skips::new`], never after.
#[derive(Clone, Debug)]
pub struct Skips {
    rate: f64,
    line: [Vec<f32>; 2],
    head: usize,
    /// Samples written since the last clear, up to the line's length: anything older reads as
    /// silence, so a clear costs nothing however long the line.
    filled: usize,
    rng: Rng,
    seed: u32,
    state: State,
    /// The loop's share of what is heard, 0 in the music, 1 going round, and its step a sample.
    share: f64,
    step: f64,
    /// The turn the stylus goes back by, in samples: the disc's, held while a turn goes round and
    /// until its jump on is done.
    reading: usize,
    /// The landing's tick.
    tick: Voice,
}

impl Skips {
    /// Allocates a turn of the slowest disc: call outside the audio thread.
    pub fn new(sample_rate: f64, seed: u32) -> Self {
        let length = turn_samples(Speed::Lp33, sample_rate);
        let mut skips = Self {
            rate: sample_rate,
            line: [vec![0.0; length], vec![0.0; length]],
            head: 0,
            filled: 0,
            rng: Rng::new(seed),
            seed,
            state: State::Playing,
            share: 0.0,
            step: 1.0 / (JUMP_S * sample_rate).max(1.0),
            reading: 1,
            tick: Voice::default(),
        };
        skips.clear();
        skips
    }

    /// A new seed, from the next draw on; whatever is going round goes on.
    pub fn reseed(&mut self, seed: u32) {
        if seed != self.seed {
            self.seed = seed;
            self.rng = Rng::new(seed ^ 0x5C1F_0001);
        }
    }

    /// Back in the music with an empty line and the seed's first draw.
    pub fn clear(&mut self) {
        self.filled = 0;
        self.rng = Rng::new(self.seed ^ 0x5C1F_0001);
        self.state = State::Playing;
        self.share = 0.0;
        self.tick = Voice::default();
    }

    /// True while the groove is locked.
    pub fn locked(&self) -> bool {
        matches!(self.state, State::Looping { until: None, .. })
    }

    /// How much longer a skip under way sounds, samples: its turns still to go round, its jump on
    /// and its tick. Zero in the music, the jump and the tick done, and while locked, which has
    /// no end.
    pub fn left(&self) -> u64 {
        let landing = ((JUMP_S + TICK_S) * self.rate).ceil() as u64;
        match self.state {
            State::Looping {
                since,
                until: Some(until),
            } => until.saturating_sub(since) as u64 + landing,
            State::Playing if self.share > 0.0 || self.tick.live => landing,
            _ => 0,
        }
    }

    /// The stylus lands: a tick.
    fn land(&mut self) {
        let sign = if self.rng.bipolar() < 0.0 { -1.0 } else { 1.0 };
        let burst = Burst::click(sign * TICK_SIZE, &mut self.rng);
        self.tick = Voice::new(self.rate, &burst, 1.0, 0.0);
    }

    /// One frame of the groove as the stylus finds it: `x` played, or a turn of `speed` before it
    /// gone round again. `chance` is Skips, a turn; `held` is Stuck groove; `playing`, whether the
    /// music is playing, without which no skip starts.
    #[inline]
    pub fn process(
        &mut self,
        x: [f64; 2],
        chance: f32,
        held: bool,
        speed: Speed,
        playing: bool,
    ) -> [f64; 2] {
        let length = self.line[0].len();
        let turn = turn_samples(speed, self.rate).min(length);
        if self.state == State::Playing && self.share == 0.0 {
            self.reading = turn;
        }
        let turn = self.reading;
        match self.state {
            // Only a whole turn played can be gone back to.
            // Only a whole turn played can be gone back to, and only once the last jump on is done.
            State::Playing if self.filled >= turn && self.share == 0.0 => {
                let until = if held {
                    Some(None)
                } else if chance > 0.0
                    && playing
                    && unit(&mut self.rng) < f64::from(chance) / turn as f64
                {
                    let mut repeats = 1;
                    while repeats < MAX_REPEATS && unit(&mut self.rng) < RELOCK {
                        repeats += 1;
                    }
                    Some(Some(repeats as usize * turn))
                } else {
                    None
                };
                if let Some(until) = until {
                    self.state = State::Looping { since: 0, until };
                    self.land();
                }
            }
            State::Playing => {}
            State::Looping { since, until } => {
                let done = match until {
                    Some(until) => since >= until && !held,
                    None => !held,
                };
                if done {
                    self.state = State::Playing;
                    self.land();
                } else if held && until.is_some() {
                    // Held while it goes round: it locks.
                    self.state = State::Looping { since, until: None };
                }
            }
        }
        let target = if self.state == State::Playing {
            0.0
        } else {
            1.0
        };
        if target > self.share {
            self.share = (self.share + self.step).min(1.0);
        } else if target < self.share {
            self.share = (self.share - self.step).max(0.0);
        }
        let tick = if self.tick.live {
            self.tick.play()
        } else {
            0.0
        };
        let at = (self.head + length - turn) % length;
        let mut out = x;
        for ((out, line), x) in out.iter_mut().zip(&self.line).zip(x) {
            let looped = if self.filled >= turn {
                f64::from(line[at])
            } else {
                0.0
            };
            *out = if self.share == 0.0 {
                x
            } else if self.share == 1.0 {
                looped
            } else {
                x + self.share * (looped - x)
            };
            if tick != 0.0 {
                *out += tick;
            }
        }
        for (line, out) in self.line.iter_mut().zip(out) {
            line[self.head] = out as f32;
        }
        self.head = (self.head + 1) % length;
        self.filled = (self.filled + 1).min(length);
        if let State::Looping { since, until } = self.state {
            self.state = State::Looping {
                since: since + 1,
                until,
            };
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    /// A programme that never repeats: noise, a little different each channel, in what the line
    /// holds exactly.
    fn programme(n: usize) -> Vec<[f64; 2]> {
        let mut rng = Rng::new(3);
        let mut next = || f64::from((0.5 * rng.bipolar()) as f32);
        (0..n).map(|_| [next(), next()]).collect()
    }

    /// What the line gives back of `v`.
    fn held_in_line(v: [f64; 2]) -> [f64; 2] {
        v.map(|v| f64::from(v as f32))
    }

    /// **Stuck, the last turn goes round as heard; let go, the stylus is back in the music**: once
    /// the jump and its tick are over, the output is the input a turn before, then the output a
    /// turn before, then, let go, the input again, to the bit.
    #[test]
    fn a_stuck_groove_goes_round_and_let_go_jumps_on_into_the_music() {
        let speed = Speed::Shellac78;
        let turn = turn_samples(speed, RATE);
        let settle = ((JUMP_S + TICK_S) * RATE) as usize;
        let (stuck_at, let_go) = (2 * turn, 5 * turn + 100);
        let x = programme(7 * turn);
        let mut skips = Skips::new(RATE, 1);
        let out: Vec<[f64; 2]> = (0..x.len())
            .map(|n| {
                let held = (stuck_at..let_go).contains(&n);
                skips.process(x[n], 0.0, held, speed, true)
            })
            .collect();
        assert!(
            (0..stuck_at).all(|n| out[n] == x[n]),
            "touched before the jump"
        );
        assert!((stuck_at + settle..stuck_at + turn).all(|n| out[n] == x[n - turn]));
        assert!((stuck_at + turn..let_go).all(|n| out[n] == held_in_line(out[n - turn])));
        assert!((let_go + settle..x.len()).all(|n| out[n] == x[n]));
        assert!(skips.left() == 0 && !skips.locked());
    }

    /// **Skips come at about their chance a turn, only while the music plays, and each repeats a
    /// turn or a few**.
    #[test]
    fn skips_come_at_their_chance_only_while_the_music_plays() {
        let speed = Speed::Shellac78;
        let turn = turn_samples(speed, RATE);
        let turns = 200;
        let count = |playing: bool| {
            let mut skips = Skips::new(RATE, 7);
            let (mut jumps, mut longest) = (0, 0);
            for _ in 0..turns * turn {
                let before = skips.state;
                skips.process([0.1, 0.1], 0.5, false, speed, playing);
                if let (
                    State::Playing,
                    State::Looping {
                        until: Some(until), ..
                    },
                ) = (before, skips.state)
                {
                    jumps += 1;
                    longest = longest.max(until / turn);
                }
            }
            (jumps, longest)
        };
        // Half a chance a turn, each skip taking about 1⅓ turns of going round: about 200 / 2.67.
        let (jumps, longest) = count(true);
        assert!((55..100).contains(&jumps), "{jumps} skips");
        assert!((2..=MAX_REPEATS as usize).contains(&longest), "{longest}");
        assert_eq!(count(false).0, 0);
    }

    /// **No skip before a whole turn has played**: held from the start, or as likely as can be,
    /// the stylus plays the first turn as it comes, then goes back.
    #[test]
    fn nothing_is_gone_back_to_before_a_whole_turn() {
        let speed = Speed::Shellac78;
        let turn = turn_samples(speed, RATE);
        let x = programme(2 * turn);
        for (chance, held) in [(0.0, true), (MAX_SKIPS, false)] {
            let mut skips = Skips::new(RATE, 1);
            let out: Vec<[f64; 2]> = (0..turn)
                .map(|n| skips.process(x[n], chance, held, speed, true))
                .collect();
            assert!((0..turn).all(|n| out[n] == x[n]), "{chance} {held}");
        }
        let mut skips = Skips::new(RATE, 1);
        for frame in x.iter().take(turn + 1) {
            skips.process(*frame, 0.0, true, speed, true);
        }
        assert!(skips.locked());
    }

    /// **A new skip waits for the last one's jump on**: held again while the stylus is still
    /// crossing back into the music, it goes round only once that crossing is done.
    #[test]
    fn a_new_skip_waits_for_the_last_jump_on() {
        let speed = Speed::Shellac78;
        let turn = turn_samples(speed, RATE);
        let x = programme(4 * turn);
        let mut skips = Skips::new(RATE, 1);
        let let_go = 2 * turn;
        for (n, frame) in x.iter().enumerate().take(let_go + 1) {
            skips.process(*frame, 0.0, (turn + 1..let_go).contains(&n), speed, true);
        }
        assert_eq!(skips.state, State::Playing);
        assert!(skips.share > 0.0 && skips.share < 1.0);
        let mut n = let_go + 1;
        while skips.share > 0.0 {
            skips.process(x[n], 0.0, true, speed, true);
            n += 1;
            assert!(
                skips.share == 0.0 || skips.state == State::Playing,
                "turned back mid-way"
            );
        }
        skips.process(x[n], 0.0, true, speed, true);
        assert!(skips.locked());
    }

    /// **A turn going round keeps its length** when the disc's speed changes under it.
    #[test]
    fn a_turn_going_round_keeps_its_length_when_the_speed_changes() {
        let turn = turn_samples(Speed::Shellac78, RATE);
        let x = programme(6 * turn);
        let mut skips = Skips::new(RATE, 1);
        let out: Vec<[f64; 2]> = (0..x.len())
            .map(|n| {
                let speed = if n < 3 * turn {
                    Speed::Shellac78
                } else {
                    Speed::Lp33
                };
                skips.process(x[n], 0.0, n >= turn + 10, speed, true)
            })
            .collect();
        assert!((4 * turn..6 * turn).all(|n| out[n] == held_in_line(out[n - turn])));
    }

    /// **Off, the groove passes untouched, to the bit; a clear forgets the turn**.
    #[test]
    fn off_is_exact_and_a_clear_forgets() {
        let speed = Speed::Single45;
        let turn = turn_samples(speed, RATE);
        let x = programme(2 * turn);
        let mut skips = Skips::new(RATE, 1);
        assert!(
            x.iter()
                .all(|x| skips.process(*x, 0.0, false, speed, true) == *x)
        );
        skips.clear();
        // Held after a clear, the stylus plays the next turn as it comes, here silence; what then
        // goes round is that silence, not the turn before the clear: only the landing's tick.
        let settle = ((JUMP_S + TICK_S) * RATE) as usize;
        let out: Vec<[f64; 2]> = (0..2 * turn + 1)
            .map(|_| skips.process([0.0, 0.0], 0.0, true, speed, false))
            .collect();
        assert!(skips.locked());
        assert!(out[..turn].iter().all(|v| *v == [0.0, 0.0]));
        assert!(
            out[turn..turn + settle].iter().any(|v| v[0] != 0.0),
            "no tick"
        );
        assert!(
            out[turn + settle..2 * turn]
                .iter()
                .all(|v| *v == [0.0, 0.0])
        );
    }
}
