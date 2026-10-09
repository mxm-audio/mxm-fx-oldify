//! How much each tape control changes the sound: renders test signals through the chain at Init
//! and with one control moved, and prints what moved — the level and octave bands of pink noise at
//! −18 dBFS, the THD+N of a 1 kHz tone at −12 dBFS, the speed's spread on a 3150 Hz tone and the
//! noise floor — then the same controls reached by moving them while playing, against the same
//! controls from the start. A control that moves nothing here is not working.
//!
//! `cargo run --release -p mxm-fx-oldify-dsp --example measure_controls`

use mxm_fx_oldify_dsp::cassette::{CassetteControls, Motion};
use mxm_fx_oldify_dsp::nr::{NoiseReduction, NrControls};
use mxm_fx_oldify_dsp::saturate::Character;
use mxm_fx_oldify_dsp::tape::{Grade, Tape};
use mxm_fx_oldify_dsp::transport::Shape;
use mxm_fx_oldify_dsp::{Controls, Oldify, db_to_gain};
use std::f64::consts::TAU;

const RATE: f32 = 48_000.0;
const SECONDS: f64 = 6.0;

fn render(c: Controls, input: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut chain = Oldify::new(RATE).unwrap();
    chain.set_controls(c);
    chain.reset();
    let latency = chain.latency() as usize;
    let mut out = Vec::with_capacity(input.len());
    for block in input.chunks(256) {
        let mut l: Vec<f32> = block.iter().map(|f| f[0]).collect();
        let mut r: Vec<f32> = block.iter().map(|f| f[1]).collect();
        chain.process(&mut l, &mut r, c);
        out.extend(l.iter().zip(&r).map(|(a, b)| [*a, *b]));
    }
    out.drain(..latency);
    out
}

fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let w = -TAU / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (s, c) = (w * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * c - im[b] * s, re[b] * s + im[b] * c);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}

/// Averaged power spectrum of the left channel, Hann, 8192 points.
fn spectrum(x: &[[f32; 2]]) -> Vec<f64> {
    let n = 8192;
    let mut sum = vec![0.0; n / 2];
    let mut count = 0;
    let mut start = 0;
    while start + n <= x.len() {
        let mut re: Vec<f64> = (0..n)
            .map(|i| {
                let w = 0.5 - 0.5 * (TAU * i as f64 / n as f64).cos();
                f64::from(x[start + i][0]) * w
            })
            .collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        for k in 0..n / 2 {
            sum[k] += re[k] * re[k] + im[k] * im[k];
        }
        count += 1;
        start += n / 2;
    }
    sum.iter().map(|p| p / count as f64).collect()
}

const BANDS: [f64; 9] = [
    63.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];

fn bands(spec: &[f64]) -> Vec<f64> {
    let hz = |k: usize| k as f64 * f64::from(RATE) / 8192.0;
    BANDS
        .iter()
        .map(|&f| {
            let (lo, hi) = (f / 2f64.sqrt(), f * 2f64.sqrt());
            let p: f64 = (1..spec.len())
                .filter(|&k| hz(k) >= lo && hz(k) < hi)
                .map(|k| spec[k])
                .sum();
            10.0 * p.max(1e-30).log10()
        })
        .collect()
}

/// Pink noise at `level_db` RMS, the same in both channels (Paul Kellet's filter).
fn noise(level_db: f32) -> Vec<[f32; 2]> {
    let mut seed = 0x1234_5678u32;
    let mut white = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f64 / u32::MAX as f64) * 2.0 - 1.0
    };
    let n = (SECONDS * f64::from(RATE)) as usize;
    let (mut b0, mut b1, mut b2) = (0.0, 0.0, 0.0);
    let gain = f64::from(db_to_gain(level_db)) * 3.0;
    (0..n)
        .map(|_| {
            let w = white();
            b0 = 0.99765 * b0 + w * 0.0990460;
            b1 = 0.96300 * b1 + w * 0.2965164;
            b2 = 0.57000 * b2 + w * 1.0526913;
            let p = (b0 + b1 + b2 + w * 0.1848) * 0.11 * gain;
            [p as f32, p as f32]
        })
        .collect()
}

fn tone(hz: f64, level_db: f32) -> Vec<[f32; 2]> {
    let n = (SECONDS * f64::from(RATE)) as usize;
    let a = f64::from(db_to_gain(level_db));
    (0..n)
        .map(|i| {
            let v = (a * (TAU * hz * i as f64 / f64::from(RATE)).sin()) as f32;
            [v, v]
        })
        .collect()
}

fn rms_db(x: &[[f32; 2]]) -> f64 {
    let p: f64 = x.iter().map(|f| f64::from(f[0]).powi(2)).sum::<f64>() / x.len() as f64;
    10.0 * p.max(1e-30).log10()
}

/// THD+N of a 1 kHz tone: everything but the fundamental's bins, against it, in dB.
fn thd_db(x: &[[f32; 2]]) -> f64 {
    let spec = spectrum(&x[x.len() / 3..]);
    let k0 = (1_000.0 / f64::from(RATE) * 8192.0).round() as usize;
    let fundamental: f64 = spec[k0 - 3..=k0 + 3].iter().sum();
    let rest: f64 = spec[10..].iter().sum::<f64>() - fundamental;
    10.0 * (rest / fundamental).max(1e-30).log10()
}

/// The speed's standard deviation on a 3150 Hz tone, percent, from zero crossings.
fn wow_percent(x: &[[f32; 2]]) -> f64 {
    let rate = f64::from(RATE);
    let mut crossings = Vec::new();
    for i in 1..x.len() {
        let (a, b) = (f64::from(x[i - 1][0]), f64::from(x[i][0]));
        if a <= 0.0 && b > 0.0 {
            crossings.push(i as f64 - 1.0 + a / (a - b));
        }
    }
    // Frequency over 16-cycle windows.
    let f: Vec<f64> = crossings
        .windows(17)
        .step_by(4)
        .map(|w| 16.0 * rate / (w[16] - w[0]))
        .collect();
    let f = &f[f.len() / 4..];
    let mean = f.iter().sum::<f64>() / f.len() as f64;
    let sd = (f.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / f.len() as f64).sqrt();
    100.0 * sd / mean
}

struct Base {
    pink: Vec<f64>,
    pink_rms: f64,
    thd: f64,
    wow: f64,
    floor: f64,
}

fn measure(c: Controls, inputs: &Inputs) -> Base {
    let pink = render(c, &inputs.pink);
    let hot = render(c, &inputs.hot);
    let wow = render(c, &inputs.wow);
    let quiet = render(c, &inputs.quiet);
    Base {
        pink: bands(&spectrum(&pink)),
        pink_rms: rms_db(&pink),
        thd: thd_db(&hot),
        wow: wow_percent(&wow),
        floor: rms_db(&quiet),
    }
}

struct Inputs {
    pink: Vec<[f32; 2]>,
    hot: Vec<[f32; 2]>,
    wow: Vec<[f32; 2]>,
    quiet: Vec<[f32; 2]>,
}

fn main() {
    let inputs = Inputs {
        pink: noise(-18.0),
        hot: tone(1_000.0, -12.0),
        wow: tone(3_150.0, -18.0),
        // A quiet tone keeps the noise gate open; its own level is far below the hiss.
        quiet: tone(100.0, -90.0),
    };
    let init = Controls::INIT;
    let base = measure(init, &inputs);
    println!(
        "{:<28} {:>6} {:>6} {:>6} {:>6} | pink bands re Init, dB: {}",
        "",
        "rms",
        "thd",
        "w&f%",
        "floor",
        BANDS
            .iter()
            .map(|b| format!("{:>6}", b))
            .collect::<String>()
    );
    let row = |name: &str, m: &Base, reference: Option<&Base>| {
        let bands: String = match reference {
            Some(r) => m
                .pink
                .iter()
                .zip(&r.pink)
                .map(|(a, b)| format!("{:>6.1}", a - b))
                .collect(),
            None => m.pink.iter().map(|a| format!("{:>6.1}", a)).collect(),
        };
        println!(
            "{:<28} {:>6.1} {:>6.1} {:>6.3} {:>6.1} | {}",
            name, m.pink_rms, m.thd, m.wow, m.floor, bands
        );
    };
    row("Init (absolute)", &base, None);
    let identity = measure(Controls::IDENTITY, &inputs);
    row("Identity", &identity, Some(&base));

    let tape = |f: &dyn Fn(&mut CassetteControls)| {
        let mut c = init;
        f(&mut c.cassette);
        c
    };
    let cases: Vec<(&str, Controls)> = vec![
        (
            "Tape off",
            Controls {
                tape_on: false,
                ..init
            },
        ),
        ("Tape type II", tape(&|c| c.tape = Tape::II)),
        ("Tape type IV", tape(&|c| c.tape = Tape::IV)),
        ("Grade economy", tape(&|c| c.grade = Grade::Economy)),
        ("Grade reference", tape(&|c| c.grade = Grade::Reference)),
        ("Tape tone 0", tape(&|c| c.tone = 0.0)),
        ("Saturation 0", tape(&|c| c.saturation = 0.0)),
        ("Saturation 1", tape(&|c| c.saturation = 1.0)),
        (
            "Saturation 1 hard",
            tape(&|c| {
                c.saturation = 1.0;
                c.character = Character::Hard;
            }),
        ),
        ("Generations 4", tape(&|c| c.generations = 4)),
        ("Age 0", tape(&|c| c.age = 0.0)),
        ("Age 1", tape(&|c| c.age = 1.0)),
        ("Wow 0", tape(&|c| c.wow.depth = 0.0)),
        ("Wow 1", tape(&|c| c.wow.depth = 1.0)),
        ("Flutter 1", tape(&|c| c.flutter.depth = 1.0)),
        ("Flange 1", tape(&|c| c.flange = 1.0)),
        ("Hiss -30", tape(&|c| c.hiss_db = -30.0)),
        ("Hiss off", tape(&|c| c.hiss_db = -96.0)),
        (
            "NR single",
            tape(&|c| {
                c.nr = NrControls {
                    kind: NoiseReduction::SingleBand,
                    ..NrControls::OFF
                }
            }),
        ),
        (
            "NR single, no decode",
            tape(&|c| {
                c.nr = NrControls {
                    kind: NoiseReduction::SingleBand,
                    decode: false,
                    ..NrControls::OFF
                }
            }),
        ),
        (
            "NR two, no decode",
            tape(&|c| {
                c.nr = NrControls {
                    kind: NoiseReduction::TwoBand,
                    decode: false,
                    ..NrControls::OFF
                }
            }),
        ),
        ("Dropouts 1", tape(&|c| c.dropouts.amount = 1.0)),
        ("Azimuth 30'", tape(&|c| c.azimuth = 30.0)),
        ("Azimuth 60'", tape(&|c| c.azimuth = 60.0)),
        ("Crosstalk -30", tape(&|c| c.crosstalk_db = -30.0)),
        ("Wrong EQ", tape(&|c| c.wrong_eq = true)),
        (
            "Wow 1 sine 2 Hz",
            tape(&|c| {
                c.wow = Motion {
                    depth: 1.0,
                    hz: 2.0,
                    shape: Shape::Sine,
                }
            }),
        ),
        (
            "Drive +12",
            Controls {
                drive: db_to_gain(12.0),
                ..init
            },
        ),
        (
            "Drive +12 sat 1",
            Controls {
                drive: db_to_gain(12.0),
                ..tape(&|c| c.saturation = 1.0)
            },
        ),
    ];
    for (name, c) in &cases {
        row(name, &measure(*c, &inputs), Some(&base));
    }

    // The same controls reached by moving them while playing: the last half of a render that
    // starts at Init and switches at a third, against the same controls from the start.
    println!(
        "\nMoved while playing (bands of the last half, re the same controls from the start):"
    );
    for (name, c) in &cases {
        let switched = {
            let mut chain = Oldify::new(RATE).unwrap();
            chain.set_controls(init);
            chain.reset();
            let mut out = Vec::new();
            for (i, block) in inputs.pink.chunks(256).enumerate() {
                let now = if i * 256 < inputs.pink.len() / 3 {
                    init
                } else {
                    *c
                };
                let mut l: Vec<f32> = block.iter().map(|f| f[0]).collect();
                let mut r: Vec<f32> = block.iter().map(|f| f[1]).collect();
                chain.process(&mut l, &mut r, now);
                out.extend(l.iter().zip(&r).map(|(a, b)| [*a, *b]));
            }
            out
        };
        let straight = render(*c, &inputs.pink);
        let half = |x: &[[f32; 2]]| bands(&spectrum(&x[x.len() / 2..]));
        let (a, b) = (half(&switched), half(&straight));
        println!(
            "{:<28} {}",
            name,
            a.iter()
                .zip(&b)
                .map(|(x, y)| format!("{:>6.1}", x - y))
                .collect::<String>()
        );
    }
}
