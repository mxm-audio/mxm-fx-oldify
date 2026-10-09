//! Noise reduction: a sliding-band compander, encoded before the tape and decoded after it.
//!
//! **What it models** (`ops: notes/oldify/research_notes`, cassette physics; the published
//! descriptions of the period's systems, no maker's name): single-band noise reduction lifts quiet
//! treble by up to 10 dB above about 4 kHz on the way to the tape and takes it back on the way out,
//! taking the tape's hiss down with it; it acts on low and middle levels only, its band sliding up
//! out of the way of loud treble. Two-band cascades two such stages, staggered 20 dB apart and
//! reaching down to about 1 kHz, for 20 dB, and skews the top octave down before the tape and up
//! after it. Played back undecoded, the tape sounds brighter and hissier on quiet passages; decoded
//! with a level or treble error, the bands mistrack and the hiss breathes with the music.
//!
//! **A stage** adds a side chain to the signal: `y = x + k · H(x)`, where `H` is a high-pass at a
//! corner `f` and `k` its gain. Both follow the side chain's own level `L`, from the previous
//! sample: `k = k₀ / (1 + L/T)` and `f = f₀ · √(1 + L/T)`, `T` the stage's threshold. Quiet, the
//! stage lifts by `1 + k₀` above its corner; loud, `k` falls and the corner slides up and away.
//!
//! **The decoder is the encoder solved for its input**, exactly: the high-pass's next output is
//! `b₀ · x + pending`, so `x = (y − k · pending) / (1 + k · b₀)`, with `k` and the coefficients from
//! the decoder's own side chain, which matches the encoder's when what reaches it is what the
//! encoder sent. Encode then decode with nothing between gives the input back to rounding; a tape
//! between gives mistracking where the physics does. Both sides recompute their corners on the same
//! schedule, every [`UPDATE`] samples, so nothing depends on a host's blocks.

use crate::biquad::{Biquad, Kind, Section};

/// Samples between a stage's corner updates; its gain moves every sample.
pub const UPDATE: u32 = 8;
/// The largest lift Amount reaches, in dB (the default, half, is the 10 dB of the period).
pub const MAX_LIFT_DB: f64 = 20.0;
/// The lowest and highest corner Brightness reaches, in hertz; half is about 2.4 kHz.
pub const CORNER_HZ: (f64, f64) = (1_000.0, 6_000.0);
/// A single stage's threshold: the side-chain level that halves its gain, −40 dBFS, so that
/// quiet treble is lifted fully and treble near the tape's reference level (−10 dBFS) hardly.
const THRESHOLD: f64 = 0.01;
/// Two-band's second stage: 20 dB lower, an octave lower.
const SECOND_THRESHOLD: f64 = 0.001;
/// Two-band's top-octave skew, in dB, and where.
const SKEW_DB: f64 = 8.0;
const SKEW_HZ: f64 = 12_000.0;
/// The side chain's level detector, in seconds.
const ATTACK_S: f64 = 0.002;
const RELEASE_S: f64 = 0.060;
/// The level error's range, in dB.
pub const MAX_LEVEL_ERROR_DB: f32 = 6.0;

/// Which system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseReduction {
    Off,
    SingleBand,
    TwoBand,
}

/// The noise reduction's controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NrControls {
    pub kind: NoiseReduction,
    /// Off: encoded but never decoded, the brighter, compressed sound of a tape played without
    /// its decoder.
    pub decode: bool,
    /// The lift, 0–1 of [`MAX_LIFT_DB`].
    pub amount: f32,
    /// The band's corner, 0–1 across [`CORNER_HZ`].
    pub brightness: f32,
    /// What the tape gains or loses between encoder and decoder, in dB: the decoder mistracks.
    pub level_error_db: f32,
}

impl NrControls {
    pub const OFF: Self = Self {
        kind: NoiseReduction::Off,
        decode: true,
        amount: 0.5,
        brightness: 0.5,
        level_error_db: 0.0,
    };

    pub fn bounded(self) -> Self {
        let unit = |v: f32, otherwise: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1.0)
            } else {
                otherwise
            }
        };
        Self {
            amount: unit(self.amount, 0.5),
            brightness: unit(self.brightness, 0.5),
            level_error_db: if self.level_error_db.is_finite() {
                self.level_error_db
                    .clamp(-MAX_LEVEL_ERROR_DB, MAX_LEVEL_ERROR_DB)
            } else {
                0.0
            },
            ..self
        }
    }

    fn lift(&self) -> f64 {
        10f64.powf(MAX_LIFT_DB * f64::from(self.amount) / 20.0) - 1.0
    }

    fn corner(&self) -> f64 {
        let (lo, hi) = CORNER_HZ;
        lo * (hi / lo).powf(f64::from(self.brightness))
    }
}

/// One compander stage, one channel, one direction.
#[derive(Clone, Debug)]
struct Stage {
    rate: f64,
    high_pass: Biquad,
    level: f64,
    attack: f64,
    release: f64,
    /// Samples until the next corner update.
    countdown: u32,
}

impl Stage {
    fn new(rate: f64) -> Self {
        Self {
            rate,
            high_pass: Biquad::default(),
            level: 0.0,
            attack: 1.0 - (-1.0 / (rate * ATTACK_S)).exp(),
            release: 1.0 - (-1.0 / (rate * RELEASE_S)).exp(),
            countdown: 0,
        }
    }

    fn reset(&mut self) {
        self.high_pass.reset();
        self.level = 0.0;
        self.countdown = 0;
    }

    /// This sample's gain, after moving the corner if it is due: the same on either side for the
    /// same side-chain history.
    #[inline]
    fn gain(&mut self, lift: f64, corner: f64, threshold: f64) -> f64 {
        let r = self.level / threshold;
        if self.countdown == 0 {
            let hz = corner * (1.0 + r).sqrt();
            let section = Section::new(Kind::HighPass, hz, 0.0, std::f64::consts::FRAC_1_SQRT_2);
            self.high_pass.set(section.coefficients(self.rate));
            self.countdown = UPDATE;
        }
        self.countdown -= 1;
        lift / (1.0 + r)
    }

    #[inline]
    fn listen(&mut self, side: f64) {
        let side = side.abs();
        let coefficient = if side > self.level {
            self.attack
        } else {
            self.release
        };
        self.level += coefficient * (side - self.level);
    }

    #[inline]
    fn encode(&mut self, x: f64, lift: f64, corner: f64, threshold: f64) -> f64 {
        let k = self.gain(lift, corner, threshold);
        let side = self.high_pass.process(x);
        self.listen(side);
        x + k * side
    }

    #[inline]
    fn decode(&mut self, y: f64, lift: f64, corner: f64, threshold: f64) -> f64 {
        let k = self.gain(lift, corner, threshold);
        let b0 = self.high_pass.coefficients().b0;
        let x = (y - k * self.high_pass.pending()) / (1.0 + k * b0);
        let side = self.high_pass.process(x);
        self.listen(side);
        x
    }
}

/// One channel's encoder or decoder: two stages and the skew.
#[derive(Clone, Debug)]
struct Side {
    first: Stage,
    second: Stage,
    skew: Biquad,
}

impl Side {
    fn new(rate: f64, skew_db: f64) -> Self {
        let shelf = Section::new(
            Kind::HighShelf,
            SKEW_HZ,
            skew_db,
            std::f64::consts::FRAC_1_SQRT_2,
        );
        Self {
            first: Stage::new(rate),
            second: Stage::new(rate),
            skew: Biquad::new(shelf.coefficients(rate)),
        }
    }

    fn reset(&mut self) {
        self.first.reset();
        self.second.reset();
        self.skew.reset();
    }
}

/// The noise reduction of both channels: encoders before the tape, decoders after it.
#[derive(Clone, Debug)]
pub struct Nr {
    encoders: [Side; 2],
    decoders: [Side; 2],
}

impl Nr {
    pub fn new(sample_rate: f64) -> Self {
        let side = |db| Side::new(sample_rate, db);
        Self {
            encoders: [side(-SKEW_DB), side(-SKEW_DB)],
            decoders: [side(SKEW_DB), side(SKEW_DB)],
        }
    }

    pub fn reset(&mut self) {
        self.encoders.iter_mut().for_each(Side::reset);
        self.decoders.iter_mut().for_each(Side::reset);
    }

    /// Before the tape.
    #[inline]
    pub fn encode(&mut self, frame: [f32; 2], c: &NrControls) -> [f32; 2] {
        if c.kind == NoiseReduction::Off {
            return frame;
        }
        let (lift, corner) = (c.lift(), c.corner());
        let mut out = [0.0f32; 2];
        for (y, (side, &x)) in out.iter_mut().zip(self.encoders.iter_mut().zip(&frame)) {
            let mut v = f64::from(x);
            if c.kind == NoiseReduction::TwoBand {
                v = side.skew.process(v);
                v = side.first.encode(v, lift, corner, THRESHOLD);
                v = side.second.encode(v, lift, corner / 2.0, SECOND_THRESHOLD);
            } else {
                v = side.first.encode(v, lift, corner, THRESHOLD);
            }
            *y = v as f32;
        }
        out
    }

    /// After the tape: the decoder, or nothing when it is off.
    #[inline]
    pub fn decode(&mut self, frame: [f32; 2], c: &NrControls) -> [f32; 2] {
        if c.kind == NoiseReduction::Off || !c.decode {
            return frame;
        }
        let (lift, corner) = (c.lift(), c.corner());
        let error = 10f64.powf(f64::from(c.level_error_db) / 20.0);
        let mut out = [0.0f32; 2];
        for (y, (side, &x)) in out.iter_mut().zip(self.decoders.iter_mut().zip(&frame)) {
            // The tape's level error reaches the decoder, which then mistracks; the level after it
            // is put back, so only the tracking changes.
            let mut v = f64::from(x) * error;
            if c.kind == NoiseReduction::TwoBand {
                v = side.second.decode(v, lift, corner / 2.0, SECOND_THRESHOLD);
                v = side.first.decode(v, lift, corner, THRESHOLD);
                v = side.skew.process(v);
            } else {
                v = side.first.decode(v, lift, corner, THRESHOLD);
            }
            *y = (v / error) as f32;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn on(kind: NoiseReduction) -> NrControls {
        NrControls {
            kind,
            ..NrControls::OFF
        }
    }

    fn tone(hz: f64, level: f64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (level * (std::f64::consts::TAU * hz * i as f64 / RATE).sin()) as f32)
            .collect()
    }

    fn rms(x: &[f32]) -> f64 {
        (x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
    }

    /// The encoder's lift for a steady tone, in dB.
    fn lift_db(kind: NoiseReduction, hz: f64, level: f64) -> f64 {
        let mut nr = Nr::new(RATE);
        let x = tone(hz, level, 48_000);
        let y: Vec<f32> = x.iter().map(|&v| nr.encode([v, v], &on(kind))[0]).collect();
        20.0 * (rms(&y[24_000..]) / rms(&x[24_000..])).log10()
    }

    #[test]
    fn quiet_treble_is_lifted_about_ten_db_and_loud_treble_hardly() {
        let quiet = lift_db(NoiseReduction::SingleBand, 8_000.0, 0.001);
        let loud = lift_db(NoiseReduction::SingleBand, 8_000.0, 0.3);
        let bass = lift_db(NoiseReduction::SingleBand, 200.0, 0.001);
        assert!((quiet - 10.0).abs() < 1.5, "quiet treble {quiet:.1} dB");
        assert!(loud < 2.0, "loud treble {loud:.1} dB");
        assert!(bass < 1.0, "quiet bass {bass:.1} dB");
    }

    #[test]
    fn two_band_lifts_quiet_signals_further_and_lower() {
        let single = lift_db(NoiseReduction::SingleBand, 2_000.0, 0.0003);
        let double = lift_db(NoiseReduction::TwoBand, 2_000.0, 0.0003);
        assert!(
            double > single + 5.0,
            "single {single:.1}, two-band {double:.1}"
        );
    }

    #[test]
    fn decoding_what_was_encoded_gives_it_back() {
        for kind in [NoiseReduction::SingleBand, NoiseReduction::TwoBand] {
            let mut nr = Nr::new(RATE);
            let c = on(kind);
            // A programme that moves through every level: quiet treble, a loud burst, silence.
            let mut x = tone(6_000.0, 0.002, 12_000);
            x.extend(tone(3_000.0, 0.5, 12_000));
            x.extend(tone(9_000.0, 0.0005, 12_000));
            let mut worst = 0.0f64;
            for &v in &x {
                let y = nr.encode([v, -v], &c);
                let back = nr.decode(y, &c);
                worst = worst.max(f64::from((back[0] - v).abs()));
                worst = worst.max(f64::from((back[1] + v).abs()));
            }
            assert!(worst < 1e-5, "{kind:?}: off by {worst}");
        }
    }

    #[test]
    fn undecoded_is_the_encoded_and_off_is_nothing() {
        let mut nr = Nr::new(RATE);
        let c = NrControls {
            decode: false,
            ..on(NoiseReduction::SingleBand)
        };
        assert_eq!(nr.decode([0.25, -0.5], &c), [0.25, -0.5]);
        let off = NrControls::OFF;
        assert_eq!(nr.encode([0.25, -0.5], &off), [0.25, -0.5]);
        assert_eq!(nr.decode([0.25, -0.5], &off), [0.25, -0.5]);
    }

    #[test]
    fn a_level_error_makes_the_decoder_mistrack() {
        // Treble in the compander's sliding range, decoded by a deck 4 dB out: the level after
        // the decoder is put back, but the treble does not come back as it went in.
        let decoded_db = |error: f32| {
            let mut nr = Nr::new(RATE);
            let c = NrControls {
                level_error_db: error,
                ..on(NoiseReduction::SingleBand)
            };
            let x = tone(8_000.0, 0.01, 48_000);
            let y: Vec<f32> = x
                .iter()
                .map(|&v| {
                    let e = nr.encode([v, v], &c);
                    nr.decode(e, &c)[0]
                })
                .collect();
            20.0 * (rms(&y[24_000..]) / rms(&x[24_000..])).log10()
        };
        assert!(decoded_db(0.0).abs() < 0.01);
        assert!(decoded_db(-4.0).abs() > 0.5, "{:.2} dB", decoded_db(-4.0));
    }
}
