//! The player: what turned the groove back into sound, from a wind-up gramophone to a modern deck.
//!
//! A record's sound is the record side times the player side; keeping them apart lets any record
//! meet any player (`ops: notes/oldify/reports`, the era layer). A player here is a chain:
//!
//! ```text
//! pickup (crystal or magnetic) → the replay curve (the caller's) → scratch filter
//!   → valve stage → speaker, horn, or the AM radio chain
//! ```
//!
//! - **Crystal pickups** sense the groove's amplitude, not its velocity: a built-in fall of 6 dB an
//!   octave against a magnetic pickup, so a crystal plays a turnover-only 78 roughly flat with no
//!   replay curve, and loads of 250 kΩ–1 MΩ took the bass off. Their treble resonance is not
//!   sourced: chosen here (a few dB near 3.5 kHz).
//! - **Valve stages**: single-ended is asymmetric and rich in the second harmonic (a single-ended
//!   output stage near 8 % at full power); push-pull cancels the even harmonics.
//! - **Speakers and horns** are band-passes with chosen corners and resonances: the best acoustic
//!   gramophone reached about 100 Hz–5 kHz. **The period's speaker and cabinet responses are not
//!   sourced**, the weakest numbers in the record engine.
//! - **The radio**: a station limiter taking 4–8 dB off the peaks, the broadcast's 4.5 kHz top,
//!   and a small receiver.
//!
//! Every period player is mono; the modern deck is stereo.

use crate::biquad::{Biquad, Kind, Section};
use crate::curves::Curve;
use std::f64::consts::FRAC_1_SQRT_2;

/// The players, as the player sees them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Player {
    /// A wind-up gramophone: soundbox and horn, a steel needle.
    WindUp,
    /// A 1940s radiogram: crystal pickup, scratch filter, single-ended valves, a cabinet speaker.
    Radiogram,
    /// A 1949 jukebox: a heavy magnetic pickup, push-pull valves, a big speaker.
    Jukebox,
    /// A 1950s portable: crystal pickup, a small speaker, single-ended valves.
    Portable,
    /// A 1955 mono hi-fi: magnetic pickup, a good speaker.
    MonoHiFi,
    /// A record on the 1940s radio: the station's turntable, the broadcast, a table radio.
    OnTheRadio,
    /// A modern deck: a fine magnetic cartridge, nothing added.
    ModernDeck,
}

/// What a player's valve stage does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Valve {
    None,
    /// Asymmetric, at this drive.
    SingleEnded(f64),
    /// Symmetric, at this drive.
    PushPull(f64),
}

/// A player's fixed character.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model {
    pub crystal: bool,
    /// The stylus tip it plays with, micrometres.
    pub tip_um: f32,
    /// The replay curve it applies when the replay curve follows the player.
    pub replay: Curve,
    scratch_hz: Option<f64>,
    valve: Valve,
    /// The speaker's (or horn's) band, hertz, and a resonance: (hertz, dB, Q), 0 dB for none.
    band: (f64, f64),
    peaks: [(f64, f64, f64); 2],
    radio: bool,
    pub mono: bool,
    /// The turntable's own wobble, weighted RMS as a fraction, and its rate, hertz.
    pub wobble: (f64, f64),
    /// The turntable's rumble, dB RMS re full scale.
    pub rumble_db: f32,
}

impl Player {
    pub const ALL: [Player; 7] = [
        Player::WindUp,
        Player::Radiogram,
        Player::Jukebox,
        Player::Portable,
        Player::MonoHiFi,
        Player::OnTheRadio,
        Player::ModernDeck,
    ];

    /// The player's character. Chosen from the period descriptions cited at the top; the
    /// corners and resonances are estimates.
    pub fn model(self) -> Model {
        let none = (0.0, 0.0, 1.0);
        match self {
            Player::WindUp => Model {
                crystal: false,
                tip_um: 64.0,
                replay: Curve::Flat,
                scratch_hz: None,
                valve: Valve::None,
                band: (120.0, 4_500.0),
                peaks: [(700.0, 6.0, 3.0), (2_200.0, 4.0, 4.0)],
                radio: false,
                mono: true,
                wobble: (0.003, 0.9),
                rumble_db: -50.0,
            },
            Player::Radiogram => Model {
                crystal: true,
                tip_um: 64.0,
                replay: Curve::Flat,
                scratch_hz: Some(5_000.0),
                valve: Valve::SingleEnded(2.0),
                band: (80.0, 6_000.0),
                peaks: [(1_500.0, 2.0, 1.5), none],
                radio: false,
                mono: true,
                wobble: (0.0015, 0.8),
                rumble_db: -52.0,
            },
            Player::Jukebox => Model {
                crystal: false,
                tip_um: 64.0,
                replay: Curve::Us78,
                scratch_hz: None,
                valve: Valve::PushPull(1.5),
                band: (50.0, 8_000.0),
                peaks: [(120.0, 3.0, 1.2), none],
                radio: false,
                mono: true,
                wobble: (0.001, 1.3),
                rumble_db: -48.0,
            },
            Player::Portable => Model {
                crystal: true,
                tip_um: 25.0,
                replay: Curve::Flat,
                scratch_hz: Some(4_500.0),
                valve: Valve::SingleEnded(3.0),
                band: (150.0, 5_000.0),
                peaks: [(1_800.0, 4.0, 2.0), none],
                radio: false,
                mono: true,
                wobble: (0.004, 0.75),
                rumble_db: -45.0,
            },
            Player::MonoHiFi => Model {
                crystal: false,
                tip_um: 25.0,
                replay: Curve::Standard,
                scratch_hz: None,
                valve: Valve::PushPull(1.0),
                band: (40.0, 12_000.0),
                peaks: [none, none],
                radio: false,
                mono: true,
                wobble: (0.0008, 0.556),
                rumble_db: -58.0,
            },
            Player::OnTheRadio => Model {
                crystal: false,
                tip_um: 25.0,
                replay: Curve::Us78,
                scratch_hz: None,
                valve: Valve::SingleEnded(1.5),
                band: (150.0, 4_500.0),
                peaks: [(1_200.0, 3.0, 1.5), none],
                radio: true,
                mono: true,
                wobble: (0.0005, 1.3),
                rumble_db: -62.0,
            },
            Player::ModernDeck => Model {
                crystal: false,
                tip_um: 8.0,
                replay: Curve::Standard,
                scratch_hz: None,
                valve: Valve::None,
                band: (5.0, 40_000.0),
                peaks: [none, none],
                radio: false,
                mono: false,
                wobble: (0.00025, 0.556),
                rumble_db: -72.0,
            },
        }
    }
}

/// The radio station's limiter: 4–8 dB off the peaks of music near full scale.
const LIMIT: f64 = 0.4;
const LIMIT_ATTACK_S: f64 = 0.001;
const LIMIT_RELEASE_S: f64 = 0.2;
/// The broadcast's top, hertz.
const BROADCAST_TOP_HZ: f64 = 4_500.0;

/// One channel of a player.
#[derive(Clone, Debug, Default)]
struct Channel {
    crystal: [Biquad; 3],
    scratch: Biquad,
    broadcast: [Biquad; 2],
    band: [Biquad; 2],
    peaks: [Biquad; 2],
}

/// A player, both channels.
#[derive(Clone, Debug)]
pub struct PlayerChain {
    rate: f64,
    player: Option<Player>,
    model: Model,
    channels: [Channel; 2],
    limiter: f64,
    attack: f64,
    release: f64,
}

impl PlayerChain {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            rate: sample_rate,
            player: None,
            model: Player::ModernDeck.model(),
            channels: Default::default(),
            limiter: 0.0,
            attack: 1.0 - (-1.0 / (sample_rate * LIMIT_ATTACK_S)).exp(),
            release: 1.0 - (-1.0 / (sample_rate * LIMIT_RELEASE_S)).exp(),
        }
    }

    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            for b in channel
                .crystal
                .iter_mut()
                .chain(channel.broadcast.iter_mut())
                .chain(channel.band.iter_mut())
                .chain(channel.peaks.iter_mut())
                .chain(std::iter::once(&mut channel.scratch))
            {
                b.reset();
            }
        }
        self.limiter = 0.0;
    }

    pub fn model(&self) -> Model {
        self.model
    }

    /// Builds `player`'s filters; nothing to do if it already is.
    pub fn set_player(&mut self, player: Player) {
        if self.player == Some(player) {
            return;
        }
        self.player = Some(player);
        self.model = player.model();
        let m = self.model;
        let rate = self.rate;
        let at = |kind, hz: f64, db: f64, q: f64| Section::new(kind, hz, db, q).coefficients(rate);
        // The crystal: amplitude from velocity (a shelf falling 20 dB from 500 Hz to 5 kHz), its
        // chosen resonance, and the load's bass cut.
        let crystal = [
            at(Kind::HighShelf, 1_580.0, -20.0, 0.5),
            at(Kind::Peak, 3_500.0, 5.0, 2.0),
            at(Kind::HighPass, 60.0, 0.0, FRAC_1_SQRT_2),
        ];
        for channel in &mut self.channels {
            for (b, c) in channel.crystal.iter_mut().zip(crystal) {
                b.set(c);
            }
            if let Some(hz) = m.scratch_hz {
                channel
                    .scratch
                    .set(at(Kind::LowPass, hz, 0.0, FRAC_1_SQRT_2));
            }
            for b in &mut channel.broadcast {
                b.set(at(Kind::LowPass, BROADCAST_TOP_HZ, 0.0, FRAC_1_SQRT_2));
            }
            channel.band[0].set(at(Kind::HighPass, m.band.0, 0.0, 0.8));
            channel.band[1].set(at(Kind::LowPass, m.band.1, 0.0, 0.8));
            for (b, (hz, db, q)) in channel.peaks.iter_mut().zip(m.peaks) {
                b.set(if db == 0.0 {
                    crate::biquad::Coefficients::IDENTITY
                } else {
                    at(Kind::Peak, hz, db, q)
                });
            }
        }
    }

    /// The pickup's own response: a crystal's, or nothing for a magnetic one.
    #[inline]
    pub fn pickup(&mut self, frame: [f64; 2]) -> [f64; 2] {
        if !self.model.crystal {
            return frame;
        }
        let mut out = frame;
        for (y, channel) in out.iter_mut().zip(&mut self.channels) {
            // The shelf is −20 dB above its corner; +10 dB brings 1 kHz back near unity.
            let mut v = *y * 3.16;
            for b in &mut channel.crystal {
                v = b.process(v);
            }
            *y = v;
        }
        out
    }

    /// Everything after the replay curve: scratch filter, valves, speaker or radio.
    #[inline]
    pub fn colour(&mut self, frame: [f64; 2]) -> [f64; 2] {
        let m = self.model;
        let mut out = if m.mono {
            let mid = 0.5 * (frame[0] + frame[1]);
            [mid, mid]
        } else {
            frame
        };
        if m.radio {
            // The station's limiter, on the louder channel, then the broadcast's top.
            let peak = out[0].abs().max(out[1].abs());
            let coefficient = if peak > self.limiter {
                self.attack
            } else {
                self.release
            };
            self.limiter += coefficient * (peak - self.limiter);
            let gain = if self.limiter > LIMIT {
                LIMIT / self.limiter
            } else {
                1.0
            };
            for (y, channel) in out.iter_mut().zip(&mut self.channels) {
                let mut v = *y * gain;
                for b in &mut channel.broadcast {
                    v = b.process(v);
                }
                *y = v;
            }
        }
        for (y, channel) in out.iter_mut().zip(&mut self.channels) {
            let mut v = *y;
            if m.scratch_hz.is_some() {
                v = channel.scratch.process(v);
            }
            v = match m.valve {
                Valve::None => v,
                Valve::SingleEnded(drive) => {
                    let offset = 0.2f64.tanh();
                    ((drive * v + 0.2).tanh() - offset) / ((1.0 - offset * offset) * drive)
                }
                Valve::PushPull(drive) => (drive * v).tanh() / drive,
            };
            for b in channel.band.iter_mut().chain(channel.peaks.iter_mut()) {
                v = b.process(v);
            }
            *y = v;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    const RATE: f64 = 48_000.0;

    /// A player's gain at `hz`, pickup and colour (no replay curve), for a quiet tone.
    fn gain_db(player: Player, hz: f64) -> f64 {
        let mut chain = PlayerChain::new(RATE);
        chain.set_player(player);
        let n = 48_000;
        let (mut input, mut output) = (0.0, 0.0);
        for i in 0..n {
            let x = 0.01 * (TAU * hz * i as f64 / RATE).sin();
            let picked = chain.pickup([x, x]);
            let y = chain.colour(picked)[0];
            if i > n / 2 {
                input += x * x;
                output += y * y;
            }
        }
        10.0 * (output / input).log10()
    }

    #[test]
    fn a_modern_deck_adds_nothing_to_the_band() {
        for hz in [50.0, 1_000.0, 15_000.0] {
            assert!(gain_db(Player::ModernDeck, hz).abs() < 0.5, "{hz} Hz");
        }
    }

    #[test]
    fn a_gramophone_horn_keeps_only_the_middle() {
        let mid = gain_db(Player::WindUp, 1_000.0);
        assert!(gain_db(Player::WindUp, 60.0) < mid - 12.0);
        assert!(gain_db(Player::WindUp, 10_000.0) < mid - 12.0);
    }

    #[test]
    fn a_crystal_pickup_falls_with_frequency_against_a_magnetic_one() {
        let mut chain = PlayerChain::new(RATE);
        chain.set_player(Player::Radiogram);
        let pickup_db = |chain: &mut PlayerChain, hz: f64| {
            chain.reset();
            let n = 48_000;
            let (mut input, mut output) = (0.0, 0.0);
            for i in 0..n {
                let x = (TAU * hz * i as f64 / RATE).sin();
                let y = chain.pickup([x, x])[0];
                if i > n / 2 {
                    input += x * x;
                    output += y * y;
                }
            }
            10.0 * (output / input).log10()
        };
        let low = pickup_db(&mut chain, 300.0);
        let high = pickup_db(&mut chain, 10_000.0);
        assert!(low - high > 12.0, "{low:.1} at 300 Hz, {high:.1} at 10 kHz");
    }

    #[test]
    fn the_radio_limits_loud_peaks_and_stops_at_its_top() {
        let mut chain = PlayerChain::new(RATE);
        chain.set_player(Player::OnTheRadio);
        let n = 48_000;
        let mut peak = 0.0f64;
        for i in 0..n {
            let x = 0.95 * (TAU * 400.0 * i as f64 / RATE).sin();
            let y = chain.colour([x, x])[0];
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        let reduction = 20.0 * (0.95 / peak).log10();
        assert!(reduction > 4.0, "only {reduction:.1} dB off the peaks");
        assert!(gain_db(Player::OnTheRadio, 9_000.0) < -20.0);
    }

    #[test]
    fn period_players_are_mono_and_the_modern_deck_is_not() {
        for player in Player::ALL {
            let mut chain = PlayerChain::new(RATE);
            chain.set_player(player);
            let mut last = [0.0; 2];
            for i in 0..2_000 {
                let x = (i as f64 * 0.05).sin() * 0.1;
                last = chain.colour([x, -x]);
            }
            if player == Player::ModernDeck {
                assert!(last[0] != last[1]);
            } else {
                assert_eq!(last[0], last[1], "{player:?}");
            }
        }
    }
}
