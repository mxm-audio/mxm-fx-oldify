//! The factory bank as readable overrides on Init, in panel units: percentages as fractions,
//! levels in decibels, rates in hertz, arc-minutes, millimetres and events a second as the panel
//! reads them, switches and lists as their 0-based positions. A control a design does not name stays
//! at Init; Mix is never named. [`generate`] writes every parameter but Mix, which a factory sound
//! keeps from the player (`is_factory_preserved`); `examples/fx_oldify_build_presets.rs` writes them
//! to `presets/`, and the bank's tests hold the committed files to what it emits today.
//!
//! The owner listened to the bank and approved it (2026-10-12: "presets are good"); the design's
//! own notes are private, `ops:notes/oldify/`.

use mxm_preset::Category;

/// A part of a design: parameter ids and their panel values.
pub type Part = &'static [(&'static str, f32)];

/// A factory sound: its name, category, what it is (one sentence, for whoever reads or lists the
/// bank) and its parts, applied in order over Init.
pub type Design = (&'static str, Category, &'static str, &'static [Part]);

// Switches.
const OFF: f32 = 0.0;
const ON: f32 = 1.0;
// Tape and grade.
const TYPE_I: f32 = 0.0;
const TYPE_II: f32 = 1.0;
const TYPE_IV: f32 = 2.0;
const ECONOMY: f32 = 0.0;
const STANDARD: f32 = 1.0;
const HIGH: f32 = 2.0;
const REFERENCE: f32 = 3.0;
// Character.
const SOFT: f32 = 0.0;
const HARD: f32 = 1.0;
// Shapes: wow's four, flutter's first two.
const SINE: f32 = 0.0;
const RANDOM: f32 = 1.0;
const SAW: f32 = 2.0;
// Noise reduction.
const NR_OFF: f32 = 0.0;
const SINGLE_BAND: f32 = 1.0;
const TWO_BAND: f32 = 2.0;
// Disc.
const LP: f32 = 0.0;
const SINGLE: f32 = 1.0;
const SHELLAC: f32 = 2.0;
// Era.
const ACOUSTIC: f32 = 0.0;
const EARLY_ELECTRICAL: f32 = 1.0;
const US_78: f32 = 2.0;
const UK_78: f32 = 3.0;
const LATE_78: f32 = 4.0;
const EARLY_SINGLE: f32 = 5.0;
const EARLY_LP: f32 = 6.0;
const MONO_LP: f32 = 7.0;
const MODERN: f32 = 8.0;
// Player.
const WIND_UP: f32 = 0.0;
const RADIOGRAM: f32 = 1.0;
const JUKEBOX: f32 = 2.0;
const PORTABLE: f32 = 3.0;
const MONO_HI_FI: f32 = 4.0;
const ON_THE_RADIO: f32 = 5.0;
const MODERN_DECK: f32 = 6.0;
// Cutting and replay curves: a follow first, then Flat, the 78s', Early LP, Early 45, ...
const TRANSCRIPTION: f32 = 7.0;
// Hum.
const HUM_50: f32 = 1.0;

/// A cassette alone: the tape on, the record off.
const ON_CASSETTE: Part = &[("tape_on", ON), ("record_on", OFF)];
/// A record alone.
const ON_RECORD: Part = &[("record_on", ON), ("tape_on", OFF)];
/// Both on: a record taped to cassette.
const CHAINED: Part = &[("record_on", ON), ("tape_on", ON)];

const HOME_DECK: Part = &[
    ("tape", TYPE_I),
    ("grade", STANDARD),
    ("tape_tone", 1.0),
    ("saturation", 0.25),
    ("character", SOFT),
    ("wow", 0.207),
    ("wow_rate", 0.6),
    ("wow_shape", RANDOM),
    ("flutter", 0.026),
    ("flutter_rate", 7.6),
    ("flutter_shape", RANDOM),
    ("hiss", -62.0),
    ("nr", NR_OFF),
    ("dropouts", 0.05),
    ("dropout_intensity", 0.25),
    ("dropout_width", 0.25),
    ("age", 0.2),
    ("crosstalk", -45.0),
];

const POCKET_PLAYER: Part = &[
    ("tape", TYPE_I),
    ("grade", ECONOMY),
    ("saturation", 0.35),
    ("wow", 0.112),
    ("wow_rate", 3.0),
    ("wow_shape", RANDOM),
    ("flutter", 0.059),
    ("flutter_rate", 9.0),
    ("flutter_shape", RANDOM),
    ("hiss", -55.0),
    ("dropouts", 0.15),
    ("dropout_intensity", 0.3),
    ("dropout_width", 0.25),
    ("age", 0.4),
    ("azimuth", 15.0),
    ("crosstalk", -40.0),
];

const ON_1940S_RADIO: Part = &[
    ("disc", SHELLAC),
    ("era", US_78),
    ("player", ON_THE_RADIO),
    ("surface_noise", -50.0),
    ("crackle", 500.0),
    ("clicks", 3.0),
    ("wear", 0.3),
];

const VINYL_FOR_A_MIXTAPE: Part = &[
    ("disc", LP),
    ("era", MODERN),
    ("player", MODERN_DECK),
    ("surface_noise", -72.0),
    ("crackle", 80.0),
    ("clicks", 1.5),
    ("wear", 0.2),
];

pub const DESIGNS: &[Design] = &[
    // ---- Cassette ----
    (
        "Home deck",
        Category::Fx,
        "An everyday ferric tape on a good home deck: a little warmth, a slight waver and hiss.",
        &[ON_CASSETTE, HOME_DECK],
    ),
    (
        "Chrome studio deck",
        Category::Fx,
        "The best chrome tape on a studio deck with two-band noise reduction: nearly clean.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_II),
                ("grade", REFERENCE),
                ("saturation", 0.2),
                ("character", HARD),
                ("wow", 0.051),
                ("wow_rate", 0.5),
                ("wow_shape", RANDOM),
                ("flutter", 0.004),
                ("flutter_rate", 7.6),
                ("flutter_shape", SINE),
                ("hiss", -64.0),
                ("nr", TWO_BAND),
                ("nr_decode", 1.0),
                ("dropouts", 0.0),
                ("age", 0.05),
                ("azimuth", 2.0),
                ("crosstalk", -55.0),
            ],
        ],
    ),
    (
        "Pocket player",
        Category::Fx,
        "A cheap tape in a pocket player: dull, wobbly and hissy, its head a little crooked.",
        &[ON_CASSETTE, POCKET_PLAYER],
    ),
    (
        "Worn pocket tape",
        Category::Fx,
        "A cheap tape played until it wore out: seasick, dropping out and losing its top.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_I),
                ("grade", ECONOMY),
                ("saturation", 0.45),
                ("wow", 0.403),
                ("wow_rate", 1.5),
                ("wow_shape", RANDOM),
                ("flutter", 0.114),
                ("flutter_rate", 9.0),
                ("flutter_shape", RANDOM),
                ("hiss", -50.0),
                ("dropouts", 0.35),
                ("dropout_intensity", 0.5),
                ("dropout_width", 0.4),
                ("age", 0.8),
                ("print_through", -50.0),
                ("azimuth", 30.0),
                ("crosstalk", -38.0),
            ],
        ],
    ),
    (
        "Third-generation dub",
        Category::Fx,
        "A copy of a copy of a tape: each generation's hiss, waver and dullness stacked up.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_I),
                ("grade", STANDARD),
                ("generations", 3.0),
                ("saturation", 0.3),
                ("wow", 0.268),
                ("wow_rate", 0.6),
                ("wow_shape", RANDOM),
                ("flutter", 0.026),
                ("hiss", -60.0),
                ("dropouts", 0.08),
                ("age", 0.35),
            ],
        ],
    ),
    (
        "Encoded, never decoded",
        Category::Fx,
        "A noise-reduced tape played without its decoder: bright, squashed and breathing.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_II),
                ("grade", HIGH),
                ("nr", SINGLE_BAND),
                ("nr_decode", 0.0),
                ("hiss", -62.0),
                ("wow", 0.112),
                ("flutter", 0.011),
                ("age", 0.15),
            ],
        ],
    ),
    (
        "Breathing hiss",
        Category::Fx,
        "Noise reduction played back at the wrong level, so the hiss pumps with the music.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_I),
                ("grade", STANDARD),
                ("nr", SINGLE_BAND),
                ("nr_decode", 1.0),
                ("nr_level_error", -4.0),
                ("hiss", -56.0),
                ("wow", 0.204),
                ("flutter", 0.018),
                ("age", 0.25),
            ],
        ],
    ),
    (
        "Seasick tape",
        Category::Fx,
        "A deep, slow, even pitch sway, as from a stretched tape on a slipping belt.",
        &[
            ON_CASSETTE,
            &[
                ("wow", 0.881),
                ("wow_rate", 0.8),
                ("wow_shape", SINE),
                ("flutter", 0.117),
                ("flutter_rate", 6.0),
                ("flutter_shape", RANDOM),
                ("saturation", 0.3),
                ("hiss", -60.0),
                ("age", 0.3),
            ],
        ],
    ),
    (
        "Tape flanger",
        Category::Fx,
        "Two tapes side by side, one drifting slowly against the other: a deep, sweeping flange.",
        &[
            ON_CASSETTE,
            &[
                ("flange", 1.0),
                ("wow", 0.709),
                ("wow_rate", 0.3),
                ("wow_shape", SINE),
                ("flutter", 0.0),
                ("hiss", -70.0),
                ("age", 0.1),
                ("saturation", 0.15),
            ],
        ],
    ),
    (
        "Metal tape, recorded hot",
        Category::Fx,
        "A metal tape on a studio deck, recorded hot without noise reduction: dense, bright and clean.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_IV),
                ("grade", REFERENCE),
                ("saturation", 0.6),
                ("character", HARD),
                ("wow", 0.051),
                ("wow_rate", 0.5),
                ("wow_shape", RANDOM),
                ("flutter", 0.004),
                ("flutter_shape", SINE),
                ("hiss", -66.0),
                ("dropouts", 0.0),
                ("age", 0.0),
                ("azimuth", 1.0),
                ("crosstalk", -60.0),
            ],
        ],
    ),
    (
        "Car stereo",
        Category::Fx,
        "A ferric tape left on a hot dashboard, played on the car's deck: wobbly, dull and narrow.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_I),
                ("grade", ECONOMY),
                ("tape_tone", 0.6),
                ("saturation", 0.4),
                ("wow", 0.45),
                ("wow_rate", 1.2),
                ("wow_shape", RANDOM),
                ("flutter", 0.09),
                ("flutter_rate", 11.0),
                ("flutter_shape", RANDOM),
                ("hiss", -54.0),
                ("dropouts", 0.2),
                ("dropout_intensity", 0.35),
                ("dropout_width", 0.3),
                ("age", 0.6),
                ("azimuth", 22.0),
                ("crosstalk", -35.0),
                ("width", 0.7),
            ],
        ],
    ),
    (
        "Answering machine",
        Category::Fx,
        "A message on a worn little answering-machine tape: mono, muffled, hissy and wavering.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_I),
                ("grade", ECONOMY),
                ("tape_tone", 0.15),
                ("saturation", 0.6),
                ("character", HARD),
                ("wow", 0.5),
                ("wow_rate", 2.0),
                ("wow_shape", RANDOM),
                ("flutter", 0.2),
                ("flutter_rate", 14.0),
                ("flutter_shape", RANDOM),
                ("hiss", -44.0),
                ("dropouts", 0.3),
                ("dropout_intensity", 0.5),
                ("dropout_width", 0.3),
                ("age", 1.0),
                ("width", 0.0),
            ],
        ],
    ),
    (
        "Four-track bounce",
        Category::Fx,
        "Tracks from a home four-track bounced down onto one: hot, a little crushed, with a second pass of hiss.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_II),
                ("grade", HIGH),
                ("generations", 2.0),
                ("saturation", 0.55),
                ("character", HARD),
                ("nr", SINGLE_BAND),
                ("nr_decode", ON),
                ("wow", 0.15),
                ("wow_rate", 0.6),
                ("wow_shape", RANDOM),
                ("flutter", 0.02),
                ("hiss", -58.0),
                ("dropouts", 0.05),
                ("age", 0.2),
            ],
        ],
    ),
    (
        "Mix-bus tape",
        Category::Fx,
        "The tape's saturation and response alone, for a mix: no waver, no hiss and no dropouts.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_I),
                ("grade", HIGH),
                ("saturation", 0.5),
                ("character", SOFT),
                ("wow", 0.0),
                ("flutter", 0.0),
                ("hiss", -96.0),
                ("dropouts", 0.0),
                ("age", 0.0),
                ("crosstalk", -80.0),
            ],
        ],
    ),
    (
        "Stored for thirty years",
        Category::Fx,
        "A tape left in a drawer for decades: each layer printed onto the next, the top gone soft.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_I),
                ("grade", STANDARD),
                ("print_through", -38.0),
                ("age", 0.7),
                ("saturation", 0.25),
                ("wow", 0.25),
                ("wow_rate", 0.5),
                ("wow_shape", RANDOM),
                ("hiss", -58.0),
                ("dropouts", 0.25),
                ("dropout_intensity", 0.3),
                ("dropout_width", 0.2),
            ],
        ],
    ),
    (
        "Chrome on the ferric setting",
        Category::Fx,
        "A chrome tape played back with a ferric tape's EQ: brighter and hissier than it was made.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_II),
                ("grade", STANDARD),
                ("wrong_eq", ON),
                ("hiss", -60.0),
                ("wow", 0.15),
                ("flutter", 0.015),
                ("age", 0.1),
            ],
        ],
    ),
    // ---- Record ----
    (
        "Acoustic 78 on a wind-up",
        Category::Fx,
        "A horn-recorded 78 from before 1925 on a wind-up gramophone: tinny, honky, loud and crackling.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", ACOUSTIC),
                ("player", WIND_UP),
                ("horn", 0.85),
                ("dynamic_wear", 0.3),
                ("surface_noise", -46.0),
                ("crackle", 900.0),
                ("clicks", 8.0),
                ("scratches", 1.0),
                ("wear", 0.4),
                ("side", 0.5),
                ("eccentricity", 0.6),
            ],
        ],
    ),
    (
        "1942 dance band on a radiogram",
        Category::Fx,
        "An American 78 of the war years on a family radiogram: warm, boxy and softly crackling.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", US_78),
                ("player", RADIOGRAM),
                ("surface_noise", -47.0),
                ("crackle", 600.0),
                ("clicks", 4.0),
                ("wear", 0.3),
                ("side", 0.4),
                ("eccentricity", 0.4),
            ],
        ],
    ),
    (
        "Wartime UK 78",
        Category::Fx,
        "A British 78 of the 1940s on a radiogram: a little more top than its American cousin.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", UK_78),
                ("player", RADIOGRAM),
                ("surface_noise", -45.0),
                ("crackle", 700.0),
                ("clicks", 5.0),
                ("wear", 0.35),
                ("side", 0.4),
                ("eccentricity", 0.5),
            ],
        ],
    ),
    (
        "1949 jukebox",
        Category::Fx,
        "A hard-played 78 on a heavy jukebox: loud, worn, distorted and full in the bass.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", US_78),
                ("player", JUKEBOX),
                ("surface_noise", -42.0),
                ("crackle", 800.0),
                ("clicks", 8.0),
                ("wear", 0.6),
                ("groove_distortion", 1.3),
                ("side", 0.6),
                ("eccentricity", 0.5),
            ],
        ],
    ),
    (
        "1952 single on a portable",
        Category::Fx,
        "An early 45 on a small portable player: bright, thin and a little worn.",
        &[
            ON_RECORD,
            &[
                ("disc", SINGLE),
                ("era", EARLY_SINGLE),
                ("player", PORTABLE),
                ("surface_noise", -57.0),
                ("crackle", 150.0),
                ("clicks", 2.0),
                ("wear", 0.4),
                ("side", 0.5),
                ("eccentricity", 0.3),
                ("warp", 0.2),
            ],
        ],
    ),
    (
        "1955 LP on mono hi-fi",
        Category::Fx,
        "A mono long-player on a good hi-fi of its day: clean, warm and quiet.",
        &[
            ON_RECORD,
            &[
                ("disc", LP),
                ("era", MONO_LP),
                ("player", MONO_HI_FI),
                ("surface_noise", -68.0),
                ("crackle", 50.0),
                ("clicks", 0.8),
                ("wear", 0.15),
                ("side", 0.3),
                ("eccentricity", 0.2),
            ],
        ],
    ),
    (
        "Early LP, wrong curve",
        Category::Fx,
        "An early long-player on a modern deck, played back with today's curve: the balance tilted.",
        &[
            ON_RECORD,
            &[
                ("disc", LP),
                ("era", EARLY_LP),
                ("player", MODERN_DECK),
                ("surface_noise", -70.0),
                ("crackle", 40.0),
                ("clicks", 0.5),
                ("wear", 0.15),
            ],
        ],
    ),
    (
        "Record on 1940s radio",
        Category::Fx,
        "A 78 played on air in the 1940s and heard on a table radio: narrow and distant.",
        &[ON_RECORD, ON_1940S_RADIO],
    ),
    (
        "Cracked shellac",
        Category::Fx,
        "A British 78 with a crack across it, popping once every turn.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", UK_78),
                ("player", RADIOGRAM),
                ("cracks", 1.0),
                ("surface_noise", -48.0),
                ("crackle", 600.0),
                ("clicks", 4.0),
                ("wear", 0.3),
            ],
        ],
    ),
    (
        "Worn LP, modern deck",
        Category::Fx,
        "A well-loved modern LP near its end, on a good deck: crackle and clicks over a full sound.",
        &[
            ON_RECORD,
            &[
                ("disc", LP),
                ("era", MODERN),
                ("player", MODERN_DECK),
                ("surface_noise", -67.0),
                ("crackle", 300.0),
                ("clicks", 6.0),
                ("wear", 0.5),
                ("side", 0.8),
                ("eccentricity", 0.3),
                ("warp", 0.15),
            ],
        ],
    ),
    (
        "1928 electrical 78 on a gramophone",
        Category::Fx,
        "An early electrical 78 of the late 1920s on a wind-up gramophone: fuller than a horn recording, still crackling.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", EARLY_ELECTRICAL),
                ("player", WIND_UP),
                ("surface_noise", -46.0),
                ("crackle", 700.0),
                ("clicks", 6.0),
                ("wear", 0.35),
                ("side", 0.5),
                ("eccentricity", 0.5),
            ],
        ],
    ),
    (
        "Late 78 on a radiogram",
        Category::Fx,
        "One of the last 78s, from the mid-1950s, on a valve radiogram: quieter shellac and a wider band.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", LATE_78),
                ("player", RADIOGRAM),
                ("surface_noise", -52.0),
                ("crackle", 400.0),
                ("clicks", 3.0),
                ("wear", 0.2),
                ("side", 0.35),
                ("eccentricity", 0.3),
            ],
        ],
    ),
    (
        "Radio transcription disc",
        Category::Fx,
        "A 1940s broadcast transcription disc, cut and played on its own curve, heard on a table radio.",
        &[
            ON_RECORD,
            &[
                ("disc", LP),
                ("era", US_78),
                ("player", ON_THE_RADIO),
                ("cut_curve", TRANSCRIPTION),
                ("replay_curve", TRANSCRIPTION),
                ("surface_noise", -58.0),
                ("crackle", 150.0),
                ("clicks", 1.5),
                ("wear", 0.2),
            ],
        ],
    ),
    (
        "Skipping 45",
        Category::Fx,
        "A scratched 45 on a portable player that now and then jumps back a groove.",
        &[
            ON_RECORD,
            &[
                ("disc", SINGLE),
                ("era", EARLY_SINGLE),
                ("player", PORTABLE),
                ("scratches", 2.0),
                ("skips", 0.04),
                ("surface_noise", -55.0),
                ("crackle", 200.0),
                ("clicks", 3.0),
                ("wear", 0.45),
                ("eccentricity", 0.4),
            ],
        ],
    ),
    (
        "Stuck 78",
        Category::Fx,
        "A 78 that soon sticks in its groove and goes round one turn for as long as Stuck groove is on.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", US_78),
                ("player", JUKEBOX),
                ("skips", 0.5),
                ("stuck_groove", ON),
                ("surface_noise", -48.0),
                ("crackle", 600.0),
                ("clicks", 4.0),
                ("wear", 0.3),
            ],
        ],
    ),
    (
        "Thrift-store LP",
        Category::Fx,
        "A battered bargain-bin LP near its label, on a modern deck: dust, clicks, scratches and a slight warp.",
        &[
            ON_RECORD,
            &[
                ("disc", LP),
                ("era", MODERN),
                ("player", MODERN_DECK),
                ("surface_noise", -60.0),
                ("crackle", 450.0),
                ("clicks", 9.0),
                ("scratches", 3.0),
                ("wear", 0.55),
                ("side", 0.85),
                ("warp", 0.3),
                ("eccentricity", 0.6),
                ("rumble", -55.0),
            ],
        ],
    ),
    (
        "Ungrounded turntable",
        Category::Fx,
        "A clean LP on a deck whose ground wire has come loose: a steady mains hum under the music.",
        &[
            ON_RECORD,
            &[
                ("disc", LP),
                ("era", MODERN),
                ("player", MODERN_DECK),
                ("hum", HUM_50),
                ("hum_level", -42.0),
                ("rumble", -58.0),
            ],
        ],
    ),
    (
        "1958 jukebox 45",
        Category::Fx,
        "A rock-and-roll 45 worn hard on a diner jukebox: loud, bright and fraying on the peaks.",
        &[
            ON_RECORD,
            &[
                ("disc", SINGLE),
                ("era", EARLY_SINGLE),
                ("player", JUKEBOX),
                ("wear", 0.55),
                ("dynamic_wear", 0.6),
                ("groove_distortion", 1.3),
                ("surface_noise", -54.0),
                ("crackle", 250.0),
                ("clicks", 5.0),
                ("side", 0.5),
                ("eccentricity", 0.3),
            ],
        ],
    ),
    (
        "Small-horn gramophone",
        Category::Fx,
        "A horn recording on a portable gramophone with the shortest horn: nasal, small and boxy.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", ACOUSTIC),
                ("player", WIND_UP),
                ("horn", 1.0),
                ("horn_size", 0.57),
                ("surface_noise", -46.0),
                ("crackle", 800.0),
                ("clicks", 7.0),
                ("wear", 0.45),
                ("side", 0.5),
            ],
        ],
    ),
    (
        "Ghost in the groove",
        Category::Fx,
        "A loud mono LP cut with its grooves too close: each turn bleeds faintly into the next.",
        &[
            ON_RECORD,
            &[
                ("disc", LP),
                ("era", MONO_LP),
                ("player", MONO_HI_FI),
                ("groove_echo", -32.0),
                ("surface_noise", -66.0),
                ("crackle", 80.0),
                ("clicks", 1.0),
                ("wear", 0.2),
            ],
        ],
    ),
    // ---- Record then cassette ----
    (
        "Mixtape from vinyl",
        Category::Fx,
        "A modern LP taped onto a home cassette: the record's crackle under the tape's hiss.",
        &[CHAINED, VINYL_FOR_A_MIXTAPE, HOME_DECK],
    ),
    (
        "Taped off the radio",
        Category::Fx,
        "A 78 on the 1940s radio, taped on a pocket player: narrow, distant and wobbly.",
        &[CHAINED, ON_1940S_RADIO, POCKET_PLAYER],
    ),
    (
        "78s dubbed to cassette",
        Category::Fx,
        "A family's British 78s copied onto a home cassette: crackle under hiss, the tape's waver over the record's.",
        &[
            CHAINED,
            &[
                ("disc", SHELLAC),
                ("era", UK_78),
                ("player", RADIOGRAM),
                ("surface_noise", -47.0),
                ("crackle", 650.0),
                ("clicks", 4.0),
                ("wear", 0.35),
                ("side", 0.4),
                ("eccentricity", 0.5),
            ],
            HOME_DECK,
        ],
    ),
    (
        "Lo-fi beat tape",
        Category::Fx,
        "A dusty LP sampled onto a worn cassette: crackle, a lazy waver and a soft top, under a beat.",
        &[
            CHAINED,
            &[
                ("disc", LP),
                ("era", MODERN),
                ("player", MODERN_DECK),
                ("surface_noise", -62.0),
                ("crackle", 350.0),
                ("clicks", 2.0),
                ("wear", 0.35),
                ("tape", TYPE_I),
                ("grade", STANDARD),
                ("tape_tone", 0.55),
                ("saturation", 0.45),
                ("wow", 0.35),
                ("wow_rate", 0.6),
                ("wow_shape", RANDOM),
                ("flutter", 0.05),
                ("flutter_rate", 7.0),
                ("flutter_shape", RANDOM),
                ("hiss", -60.0),
                ("dropouts", 0.08),
                ("age", 0.5),
                ("width", 0.8),
            ],
        ],
    ),
    // ---- Past the real thing: the controls taken where no deck or record goes, for effect ----
    (
        "Melting tape",
        Category::Fx,
        "Tape stretched past any use: the pitch slumps and slides, the top washes out and the sound keeps falling away.",
        &[
            ON_CASSETTE,
            &[
                ("wow", 1.0),
                ("wow_rate", 0.2),
                ("wow_shape", RANDOM),
                ("flutter", 0.4),
                ("flutter_rate", 6.0),
                ("flutter_shape", RANDOM),
                ("tape_tone", 0.5),
                ("saturation", 0.4),
                ("age", 1.0),
                ("hiss", -55.0),
                ("dropouts", 0.5),
                ("dropout_intensity", 0.6),
                ("dropout_width", 0.6),
            ],
        ],
    ),
    (
        "Sawtooth wow",
        Category::Fx,
        "A saw-shaped wow three times a second: the pitch sags slowly and springs back fast, as no real transport moves.",
        &[
            ON_CASSETTE,
            &[
                ("wow", 0.5),
                ("wow_rate", 3.0),
                ("wow_shape", SAW),
                ("flutter", 0.0),
                ("hiss", -70.0),
                ("dropouts", 0.0),
                ("age", 0.1),
            ],
        ],
    ),
    (
        "Tape vibrato",
        Category::Fx,
        "An even sine wow five times a second: a steady vibrato, too regular for any real deck.",
        &[
            ON_CASSETTE,
            &[
                ("wow", 0.6),
                ("wow_rate", 5.0),
                ("wow_shape", SINE),
                ("flutter", 0.0),
                ("hiss", -72.0),
                ("dropouts", 0.0),
                ("age", 0.05),
            ],
        ],
    ),
    (
        "Flutter fuzz",
        Category::Fx,
        "Flutter at its fastest and deepest, its rate swung by the wow: a warbling, grainy buzz on every note.",
        &[
            ON_CASSETTE,
            &[
                ("flutter", 1.0),
                ("flutter_rate", 30.0),
                ("flutter_shape", SINE),
                ("follow", ON),
                ("wow", 0.6),
                ("wow_rate", 0.4),
                ("wow_shape", SINE),
                ("saturation", 0.4),
                ("hiss", -64.0),
                ("dropouts", 0.0),
            ],
        ],
    ),
    (
        "Disintegrating tape",
        Category::Fx,
        "A tape falling apart as it plays: crushed, buried in hiss and dropping out every moment.",
        &[
            ON_CASSETTE,
            &[
                ("tape", TYPE_I),
                ("grade", ECONOMY),
                ("generations", 4.0),
                ("saturation", 0.9),
                ("character", HARD),
                ("wow", 0.5),
                ("wow_rate", 1.0),
                ("wow_shape", RANDOM),
                ("flutter", 0.15),
                ("flutter_rate", 10.0),
                ("hiss", -42.0),
                ("dropouts", 0.85),
                ("dropout_intensity", 0.9),
                ("dropout_width", 0.7),
                ("age", 1.0),
                ("azimuth", 45.0),
                ("crosstalk", -32.0),
            ],
        ],
    ),
    (
        "Overdriven tape",
        Category::Fx,
        "Driven twelve decibels into the tape and brought back down after: thick, squared off and rich in harmonics.",
        &[
            ON_CASSETTE,
            &[
                ("drive", 12.0),
                ("saturation", 0.85),
                ("character", HARD),
                ("wow", 0.1),
                ("hiss", -62.0),
                ("dropouts", 0.0),
            ],
        ],
    ),
    (
        "Phase-smeared tape",
        Category::Fx,
        "A head a whole degree out of line and the image widened: the stereo smears, and in mono the top cancels.",
        &[
            ON_CASSETTE,
            &[
                ("azimuth", 60.0),
                ("crosstalk", -36.0),
                ("width", 1.6),
                ("wow", 0.2),
                ("hiss", -60.0),
            ],
        ],
    ),
    (
        "Funhouse warp",
        Category::Fx,
        "An LP warped twice as far as any real record, and off its centre too: the pitch heaves every turn.",
        &[
            ON_RECORD,
            &[
                ("disc", LP),
                ("era", MODERN),
                ("player", MODERN_DECK),
                ("warp", 2.0),
                ("eccentricity", 2.0),
                ("wear", 0.2),
            ],
        ],
    ),
    (
        "Crackle bed",
        Category::Fx,
        "Heavy crackle over an otherwise clean record, playing on through the silences between the music.",
        &[
            ON_RECORD,
            &[
                ("disc", LP),
                ("era", MODERN),
                ("player", MODERN_DECK),
                ("wear", 0.0),
                ("crackle", 1200.0),
                ("crackle_level", 4.0),
                ("clicks", 4.0),
                ("surface_noise", -58.0),
                ("noise_follows", 0.0),
                ("noise_stops", OFF),
            ],
        ],
    ),
    (
        "Haunted gramophone",
        Category::Fx,
        "A horn record through the longest horn, each turn echoing back far louder than any groove could.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", ACOUSTIC),
                ("player", WIND_UP),
                ("horn", 1.0),
                ("horn_size", 1.5),
                ("groove_echo", -14.0),
                ("surface_noise", -44.0),
                ("crackle", 1000.0),
                ("clicks", 6.0),
                ("wear", 0.5),
                ("side", 0.6),
            ],
        ],
    ),
];

/// The parameters whose panel value is a level in decibels, stored as linear gain.
const IN_DECIBELS: &[&str] = &[
    "hiss",
    "crosstalk",
    "crackle_level",
    "surface_noise",
    "rumble",
    "hum_level",
    "groove_echo",
    "print_through",
    "drive",
    "nr_level_error",
];

/// Every design as its file: the slug it is written to and its complete JSON. The generation law
/// lives here, not in the example, so the bank's tests can hold the committed files to it.
pub fn generate() -> Vec<(String, String)> {
    use crate::params::MxmFxOldifyParams;
    use crate::preset::{Preset, Value};
    use mxm_preset::Instrument;
    use nice_plug::prelude::Params;
    use std::collections::{BTreeSet, HashMap};

    let params = MxmFxOldifyParams::default();
    let erased: HashMap<_, _> = params.parameters().into_iter().collect();
    let pointers: HashMap<_, _> = params
        .param_map()
        .into_iter()
        .map(|(id, pointer, _)| (id, pointer))
        .collect();
    let mut slugs = BTreeSet::new();
    let mut generated = Vec::with_capacity(DESIGNS.len());
    for (name, category, description, parts) in DESIGNS {
        assert_ne!(
            *category,
            Category::Uncategorised,
            "{name}: a shipped sound states its category"
        );
        assert!(
            description.ends_with('.') && description.len() > 20,
            "{name}: a sound says what it is in a sentence"
        );
        let mut preset = Preset::capture(*name, *category, &params);
        // A factory recipe leaves out Mix and Quality, which the shared resolver keeps from the
        // live values.
        for id in crate::preset::PRESERVED {
            preset.params.remove(id);
        }
        let mut named = BTreeSet::new();
        for &(id, panel) in parts.iter().flat_map(|part| part.iter()) {
            assert!(named.insert(id), "{name}: {id} is set twice");
            assert!(
                !crate::preset::PRESERVED.contains(&id),
                "{name}: a factory sound keeps the player's {id}"
            );
            let pointer = pointers
                .get(id)
                .unwrap_or_else(|| panic!("{name}: unknown parameter {id}"));
            let plain = if IN_DECIBELS.contains(&id) {
                mxm_fx_oldify_dsp::db_to_gain(panel)
            } else {
                panel
            };
            // SAFETY: `params` outlives the pointer, which is only read here.
            let normalised = unsafe { pointer.preview_normalized(plain) };
            preset.params.insert(
                id.to_owned(),
                Value {
                    v: normalised,
                    text: erased[id].format(normalised),
                },
            );
        }
        assert_eq!(
            preset.params.len(),
            erased.len() - crate::preset::PRESERVED.len(),
            "{name}: a factory recipe is complete but for Mix and Quality"
        );
        let slug = slug(name);
        assert!(slugs.insert(slug.clone()), "duplicate slug {slug}");
        generated.push((slug, preset.to_json() + "\n"));
    }
    generated
}

/// The file stem a sound's name compiles to.
pub fn slug(name: &str) -> String {
    name.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
