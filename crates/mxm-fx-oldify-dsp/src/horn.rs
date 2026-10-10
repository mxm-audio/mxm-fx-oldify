//! The acoustic horn: a record made into a horn and played back through one, the tinny, honky
//! sound of a real old 78.
//!
//! Before microphones the performers played into a horn whose narrow end held a diaphragm and the
//! cutting stylus; a wind-up gramophone played the record back through a soundbox and another
//! horn. The figures are Copeland's (*Manual of Analogue Sound Restoration Techniques*, ch. 12) and
//! our analysis of Vollmer and Bolles's 2024 impulse responses of twenty wind-up gramophones
//! (`ops: notes/oldify`, research notes, the acoustic horn). "Tinny" is not extra treble: it is
//! the bass gone and a narrow honk left. A 1911 record on an old horn machine has its 125–250 Hz
//! band 22–25 dB under its 1–2 kHz band, and only about 630 Hz–1.25 kHz within 10 dB of its peak.
//!
//! ```text
//! the recording half (the music alone): mono → the horn's air → its mouth's bass cut
//!   → the diaphragm's piston peak and its break-up notch → the top above them
//! the playback half (the music and the disc's own sound): the soundbox's distortion
//!   → the gramophone's horn → its band, its honk near 1 kHz and its second peak
//! ```
//!
//! - **The horn's air is a digital waveguide** (Smith, *Physical Audio Signal Processing*): the
//!   round trip, `2L / c`, fed back through a low-pass, the mouth letting the upper frequencies
//!   out: a cone's resonances at every multiple of `c / 2L` (Copeland: a fundamental of 150–300 Hz,
//!   114 Hz for a very long horn, and seven to eleven harmonics), strongest low down.
//! - **The mouth cuts the bass** with two first-order slopes an octave apart, about −4.5 dB at
//!   730 Hz for a 600 mm mouth; a longer horn's larger mouth cuts lower.
//! - **The diaphragm** has a symmetrical piston peak (+10 to +15 dB near 1.6 kHz) and, above it,
//!   a narrow, very deep break-up notch: the notes in it were simply not recorded, at 2,343 Hz in
//!   1911 rising to 2,953 Hz in 1924. Little survives above about 4 kHz. Recorded laterally, the
//!   chain added almost no distortion.
//! - **The gramophone** is the measured pre-1917 horn machine: the bass gone below about 225 Hz
//!   and shelved under 700 Hz, a dominant peak at 0.97–1.3 kHz, a second near 2.5 kHz, a cliff
//!   from about 3.2 kHz; its soundbox distorts 4.2–6.3 % at full level, mostly as a soft clip.
//! - **Horn** (0–1) moves every part together from nothing to the full acoustic sound: the
//!   feedbacks, the peaks' and notch's depths, the corners (geometrically, from outside hearing),
//!   the image toward mono and the soundbox's drive. At 0 the record passes untouched, exactly.
//!   **Horn size** is the horn's length, [`HORN_SHORTEST_M`] to [`HORN_LONGEST_M`]: its
//!   fundamental, its mouth's cut and the gramophone's bass limit.
//!
//! The waveguide's feedbacks and mouths, the notch's depth and Q, the shelf and the drive's
//! curve are estimates within the cited ranges.

use crate::biquad::{Biquad, Coefficients, Kind, Section};
use std::f64::consts::{FRAC_1_SQRT_2, PI, TAU};

/// The horn's length, metres: the cited fundamentals' 300 Hz and 114 Hz.
pub const HORN_SHORTEST_M: f32 = 0.57;
pub const HORN_LONGEST_M: f32 = 1.5;
/// The init patch's horn length: a fundamental near 190 Hz.
pub const HORN_INIT_M: f32 = 0.9;
const SPEED_OF_SOUND: f64 = 343.0;

/// The horn's air at full Horn: the waveguide's feedback, and its mouth's low-pass as a multiple
/// of the fundamental, so the first three resonances stand +5 to +7 dB and the sixth +3 dB, as the
/// cited +4 to +10 dB falling to +1 to +3 dB.
const AIR_FEEDBACK: f64 = 0.6;
const AIR_MOUTH: f64 = 3.0;
/// The mouth's bass cut at the init length: two first-order high-passes, here and an octave below,
/// near the cited −15 dB at 200 Hz (the low end of the estimated 500–900 Hz).
const MOUTH_CUT_HZ: f64 = 500.0;
/// The diaphragm's piston peak (Wilson's 1.6 kHz and Q 1.8) and break-up notch (between the cited
/// 2,343 and 2,953 Hz; "great depth", est. −20 to −40 dB, Q 8–20). The peak stands +6 dB, under
/// the cited +10 to +15: higher, the whole horn lost more bass against its middle than the
/// measured 22–25 dB and honked above the measured 630 Hz–1.25 kHz.
const PISTON_HZ: f64 = 1_600.0;
const PISTON_Q: f64 = 1.8;
const PISTON_DB: f64 = 6.0;
const NOTCH_HZ: f64 = 2_650.0;
const NOTCH_Q: f64 = 10.0;
const NOTCH_DB: f64 = -30.0;
/// The recording's top: two sections, 24 dB an octave (the cited 18–24), from the cited 3–3.5 kHz.
const RECORD_TOP_HZ: f64 = 3_300.0;
/// The gramophone at the init length: its bass limit (the cited 200–250 Hz), the shelf under its
/// middle, its honk and second peak, and its cliff (two sections from 3 kHz, giving the measured
/// −19 to −25 dB at 4 kHz under the honk).
const PLAY_BOTTOM_HZ: f64 = 225.0;
const PLAY_SHELF_HZ: f64 = 700.0;
const PLAY_SHELF_DB: f64 = -6.0;
const HONK_HZ: f64 = 1_150.0;
const HONK_Q: f64 = 2.5;
const HONK_DB: f64 = 7.0;
const SECOND_HZ: f64 = 2_500.0;
const SECOND_Q: f64 = 4.0;
const SECOND_DB: f64 = 3.0;
const PLAY_TOP_HZ: f64 = 3_000.0;
/// The gramophone's horn: its length against the recording horn's, its feedback and mouth.
const PLAY_LENGTH: f64 = 0.8;
const PLAY_FEEDBACK: f64 = 0.3;
const PLAY_MOUTH_HZ: f64 = 2_000.0;
/// The soundbox's drive at full Horn and its even part: about 5 % THD on a full-level tone.
const SOUNDBOX_DRIVE: f64 = 0.8;
const SOUNDBOX_EVEN: f64 = 0.1;
/// Where the corners start at Horn 0: outside hearing.
const OPEN_BOTTOM_HZ: f64 = 10.0;
const OPEN_TOP_HZ: f64 = 22_000.0;
/// How fast a new horn length is reached, seconds.
const LENGTH_GLIDE_S: f64 = 0.05;

/// The horn's controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HornControls {
    /// 0–1: how much of the sound went through horns.
    pub amount: f32,
    /// The horn's length, metres.
    pub length_m: f32,
}

impl HornControls {
    pub const OFF: Self = Self {
        amount: 0.0,
        length_m: HORN_INIT_M,
    };

    pub fn bounded(self) -> Self {
        Self {
            amount: if self.amount.is_finite() {
                self.amount.clamp(0.0, 1.0)
            } else {
                0.0
            },
            length_m: if self.length_m.is_finite() {
                self.length_m.clamp(HORN_SHORTEST_M, HORN_LONGEST_M)
            } else {
                HORN_INIT_M
            },
        }
    }

    /// The recording horn's fundamental, hertz.
    pub fn fundamental_hz(&self) -> f64 {
        SPEED_OF_SOUND / (2.0 * f64::from(self.length_m))
    }

    /// How a corner set by the horn's size moves: a longer horn, a larger mouth, a lower corner.
    fn size_scale(&self) -> f64 {
        (f64::from(HORN_INIT_M) / f64::from(self.length_m)).sqrt()
    }
}

/// A first-order high-pass, bilinear and prewarped.
fn first_order_high_pass(hz: f64, rate: f64) -> Coefficients {
    let k = (PI * hz.min(0.45 * rate) / rate).tan();
    Coefficients {
        b0: 1.0 / (1.0 + k),
        b1: -1.0 / (1.0 + k),
        b2: 0.0,
        a1: (k - 1.0) / (k + 1.0),
        a2: 0.0,
    }
}

/// One horn's air: a delay of its round trip fed back through its mouth.
#[derive(Clone, Debug)]
struct Waveguide {
    line: Vec<f64>,
    write: usize,
    /// The round trip in samples, gliding toward its target.
    period: f64,
    mouth: f64,
}

impl Waveguide {
    fn new(longest: usize) -> Self {
        Self {
            line: vec![0.0; longest + 4],
            write: 0,
            period: 2.0,
            mouth: 0.0,
        }
    }

    fn clear(&mut self) {
        self.line.iter_mut().for_each(|v| *v = 0.0);
        self.mouth = 0.0;
    }

    /// One sample: `feedback` of the delayed sound, through the mouth's one-pole `pole`, back in.
    #[inline]
    fn process(&mut self, x: f64, feedback: f64, pole: f64) -> f64 {
        let len = self.line.len();
        let read = self.write as f64 + len as f64 - self.period;
        let at = read.floor();
        let frac = read - at;
        let i = at as usize % len;
        let delayed = self.line[i] + frac * (self.line[(i + 1) % len] - self.line[i]);
        self.mouth = pole * self.mouth + (1.0 - pole) * delayed;
        let y = x + feedback * self.mouth;
        self.line[self.write] = y;
        self.write = (self.write + 1) % len;
        y
    }
}

/// One channel's horns and sections.
#[derive(Clone, Debug)]
struct Channel {
    air: Waveguide,
    /// The mouth's two cuts, the piston peak, the notch, the top's two sections.
    recording: [Biquad; 6],
    playback_air: Waveguide,
    /// The bass limit, the shelf, the honk, the second peak, the cliff's two sections.
    playback: [Biquad; 6],
}

/// The horn, both channels.
#[derive(Clone, Debug)]
pub struct Horn {
    rate: f64,
    channels: [Channel; 2],
    controls: Option<HornControls>,
    /// Per sample: the length's glide, the mouths' poles, the feedbacks, the soundbox's drive, the
    /// side's share; and the round trips, in samples, the periods glide to.
    glide: f64,
    air_pole: f64,
    play_pole: f64,
    air_feedback: f64,
    play_feedback: f64,
    drive: f64,
    side: f64,
    air_period: f64,
    play_period: f64,
}

impl Horn {
    /// Allocates the longest horns' lines: call outside the audio thread.
    pub fn new(sample_rate: f64) -> Self {
        let longest = |length: f64| (2.0 * length / SPEED_OF_SOUND * sample_rate).ceil() as usize;
        let channel = || Channel {
            air: Waveguide::new(longest(f64::from(HORN_LONGEST_M))),
            recording: [Biquad::default(); 6],
            playback_air: Waveguide::new(longest(f64::from(HORN_LONGEST_M) * PLAY_LENGTH)),
            playback: [Biquad::default(); 6],
        };
        let pole = |hz: f64| (-TAU * hz.min(0.45 * sample_rate) / sample_rate).exp();
        Self {
            rate: sample_rate,
            channels: [channel(), channel()],
            controls: None,
            glide: (-1.0 / (LENGTH_GLIDE_S * sample_rate)).exp(),
            air_pole: 0.0,
            play_pole: pole(PLAY_MOUTH_HZ),
            air_feedback: 0.0,
            play_feedback: 0.0,
            drive: 0.0,
            side: 1.0,
            air_period: 2.0,
            play_period: 2.0,
        }
    }

    pub fn clear(&mut self) {
        for channel in &mut self.channels {
            channel.air.clear();
            channel.playback_air.clear();
            channel
                .recording
                .iter_mut()
                .chain(channel.playback.iter_mut())
                .for_each(Biquad::reset);
            channel.air.period = self.air_period;
            channel.playback_air.period = self.play_period;
        }
    }

    /// Whether the horn changes the sound at all.
    pub fn sounds(&self) -> bool {
        self.controls.is_some_and(|c| c.amount > 0.0)
    }

    /// Builds the horn for `c`; nothing to do if it already is.
    pub fn set(&mut self, c: HornControls) {
        let c = c.bounded();
        if self.controls == Some(c) {
            return;
        }
        let first = self.controls.is_none();
        self.controls = Some(c);
        let rate = self.rate;
        let amount = f64::from(c.amount);
        let f0 = c.fundamental_hz();
        let size = c.size_scale();
        // A corner closing in from outside hearing to its full-Horn place, geometrically.
        let toward = |open: f64, full: f64| open * (full / open).powf(amount);
        let at = |kind, hz: f64, db: f64, q: f64| Section::new(kind, hz, db, q).coefficients(rate);
        self.air_feedback = AIR_FEEDBACK * amount;
        self.play_feedback = PLAY_FEEDBACK * amount;
        self.drive = SOUNDBOX_DRIVE * amount;
        self.side = 1.0 - amount;
        self.air_pole = (-TAU * (AIR_MOUTH * f0).min(0.45 * rate) / rate).exp();
        self.air_period = (rate / f0).max(2.0);
        self.play_period = (rate * PLAY_LENGTH / f0).max(2.0);
        let mouth = toward(OPEN_BOTTOM_HZ, MOUTH_CUT_HZ * size);
        let record_top = at(
            Kind::LowPass,
            toward(OPEN_TOP_HZ, RECORD_TOP_HZ),
            0.0,
            FRAC_1_SQRT_2,
        );
        let recording = [
            first_order_high_pass(mouth, rate),
            first_order_high_pass(0.5 * mouth, rate),
            at(Kind::Peak, PISTON_HZ, PISTON_DB * amount, PISTON_Q),
            at(Kind::Peak, NOTCH_HZ, NOTCH_DB * amount, NOTCH_Q),
            record_top,
            record_top,
        ];
        let play_top = at(
            Kind::LowPass,
            toward(OPEN_TOP_HZ, PLAY_TOP_HZ),
            0.0,
            FRAC_1_SQRT_2,
        );
        let playback = [
            at(
                Kind::HighPass,
                toward(OPEN_BOTTOM_HZ, PLAY_BOTTOM_HZ * size),
                0.0,
                FRAC_1_SQRT_2,
            ),
            at(
                Kind::LowShelf,
                PLAY_SHELF_HZ,
                PLAY_SHELF_DB * amount,
                FRAC_1_SQRT_2,
            ),
            at(Kind::Peak, HONK_HZ, HONK_DB * amount, HONK_Q),
            at(Kind::Peak, SECOND_HZ, SECOND_DB * amount, SECOND_Q),
            play_top,
            play_top,
        ];
        for channel in &mut self.channels {
            for (b, coefficients) in channel.recording.iter_mut().zip(recording) {
                b.set(coefficients);
            }
            for (b, coefficients) in channel.playback.iter_mut().zip(playback) {
                b.set(coefficients);
            }
            if first {
                channel.air.period = self.air_period;
                channel.playback_air.period = self.play_period;
            }
        }
    }

    /// The recording half, on the music alone.
    #[inline]
    pub fn record(&mut self, frame: [f64; 2]) -> [f64; 2] {
        let mid = 0.5 * (frame[0] + frame[1]);
        let side = 0.5 * (frame[0] - frame[1]) * self.side;
        let mut out = [mid + side, mid - side];
        for (v, channel) in out.iter_mut().zip(&mut self.channels) {
            let air = &mut channel.air;
            air.period = self.glide * air.period + (1.0 - self.glide) * self.air_period;
            let mut y = air.process(*v, self.air_feedback, self.air_pole);
            for b in &mut channel.recording {
                y = b.process(y);
            }
            *v = y;
        }
        out
    }

    /// The playback half, on the music and the disc's own sound.
    #[inline]
    pub fn play(&mut self, frame: [f64; 2]) -> [f64; 2] {
        let mut out = frame;
        for (v, channel) in out.iter_mut().zip(&mut self.channels) {
            // The soundbox, mostly a soft clip with a little even bend; its offset leaves through
            // the bass limit below.
            let mut y = *v;
            if self.drive > 0.0 {
                y = (self.drive * (y + SOUNDBOX_EVEN * y * y)).tanh() / self.drive;
            }
            let air = &mut channel.playback_air;
            air.period = self.glide * air.period + (1.0 - self.glide) * self.play_period;
            y = air.process(y, self.play_feedback, self.play_pole);
            for b in &mut channel.playback {
                y = b.process(y);
            }
            *v = y;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    /// The horn's gain at `hz` for a tone of `amplitude`: the recording half, then the playback
    /// half if `both`.
    fn gain_at(c: HornControls, hz: f64, amplitude: f64, both: bool) -> f64 {
        let mut horn = Horn::new(RATE);
        horn.set(c);
        horn.clear();
        let n = 24_000;
        let (mut input, mut output) = (0.0, 0.0);
        for i in 0..n {
            let x = amplitude * (TAU * hz * i as f64 / RATE).sin();
            let mut y = horn.record([x, x]);
            if both {
                y = horn.play(y);
            }
            if i > n / 2 {
                input += x * x;
                output += y[0] * y[0];
            }
        }
        10.0 * (output / input).log10()
    }

    fn gain(c: HornControls, hz: f64, both: bool) -> f64 {
        gain_at(c, hz, 0.01, both)
    }

    /// The mean gain over a band, both halves, in decibels of the mean power.
    fn band_db(c: HornControls, from: f64, to: f64) -> f64 {
        let n = 12;
        let power: f64 = (0..n)
            .map(|k| {
                let hz = from * (to / from).powf((f64::from(k) + 0.5) / f64::from(n));
                10f64.powf(gain(c, hz, true) / 10.0)
            })
            .sum();
        10.0 * (power / f64::from(n)).log10()
    }

    #[test]
    fn no_horn_is_no_horn_at_all() {
        let mut horn = Horn::new(RATE);
        horn.set(HornControls::OFF);
        assert!(!horn.sounds());
    }

    /// **A full horn is tinny as measured**: a 1911 record on an old horn machine has its
    /// 125–250 Hz band 22–25 dB under its 1–2 kHz band, and loses everything far above 3 kHz.
    #[test]
    fn a_full_horn_loses_the_bass_and_keeps_a_narrow_middle() {
        let c = HornControls {
            amount: 1.0,
            ..HornControls::OFF
        };
        let middle = band_db(c, 1_000.0, 2_000.0);
        let deficit = middle - band_db(c, 125.0, 250.0);
        assert!(
            (18.0..30.0).contains(&deficit),
            "125–250 Hz {deficit:.1} dB under 1–2 kHz"
        );
        assert!(gain(c, 5_000.0, true) < middle - 20.0, "5 kHz");
        assert!(gain(c, 60.0, true) < middle - 30.0, "60 Hz");
    }

    /// **The diaphragm's break-up notch**: the notes in it were not recorded.
    #[test]
    fn the_diaphragm_notches_out_its_band() {
        let c = HornControls {
            amount: 1.0,
            ..HornControls::OFF
        };
        let notch = gain(c, NOTCH_HZ, false);
        let piston = gain(c, PISTON_HZ, false);
        assert!(notch < piston - 25.0, "{notch:.1} against {piston:.1}");
    }

    /// **The recording horn honks at its fundamental, `c / 2L`**: the loudest point near it is
    /// there, well above the trough between it and the next resonance; a longer horn, lower.
    #[test]
    fn the_horn_resonates_at_its_fundamental_and_a_longer_one_lower() {
        for length_m in [0.6, 1.2] {
            let c = HornControls {
                amount: 1.0,
                length_m,
            };
            let f0 = c.fundamental_hz();
            let scan = |from: f64, to: f64| {
                (0..=40)
                    .map(|k| {
                        let hz = f0 * (from + (to - from) * f64::from(k) / 40.0);
                        (hz, gain(c, hz, false))
                    })
                    .collect::<Vec<_>>()
            };
            let (peak_hz, peak) = scan(0.8, 1.2)
                .into_iter()
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .unwrap();
            let trough = scan(1.3, 1.7)
                .into_iter()
                .map(|(_, db)| db)
                .fold(f64::INFINITY, f64::min);
            assert!(
                (peak_hz / f0 - 1.0).abs() < 0.08,
                "{length_m} m: the peak at {peak_hz:.0} Hz, not {f0:.0}"
            );
            assert!(
                peak > trough + 6.0,
                "{length_m} m: {peak:.1} dB at {peak_hz:.0} Hz, the trough {trough:.1}"
            );
        }
    }

    /// **The soundbox distorts on loud notes only**: a full-level tone comes out a few dB
    /// squashed, a quiet one as it went in.
    #[test]
    fn the_soundbox_squashes_loud_notes_and_leaves_quiet_ones() {
        let c = HornControls {
            amount: 1.0,
            ..HornControls::OFF
        };
        let quiet = gain_at(c, HONK_HZ, 0.01, true);
        let loud = gain_at(c, HONK_HZ, 1.0, true);
        assert!(quiet - loud > 0.5, "quiet {quiet:.1}, loud {loud:.1}");
        assert!(quiet - loud < 6.0, "quiet {quiet:.1}, loud {loud:.1}");
    }

    #[test]
    fn the_horn_stays_finite_and_bounded_at_full_scale() {
        let mut horn = Horn::new(RATE);
        horn.set(HornControls {
            amount: 1.0,
            length_m: HORN_SHORTEST_M,
        });
        for i in 0..48_000 {
            let x = if i % 40 < 20 { 2.0 } else { -2.0 };
            let recorded = horn.record([x, -x]);
            let y = horn.play(recorded);
            assert!(y.iter().all(|v| v.is_finite() && v.abs() < 20.0), "{y:?}");
        }
    }
}
