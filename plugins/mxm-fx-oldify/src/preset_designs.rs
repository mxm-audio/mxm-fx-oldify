//! The factory bank as readable overrides on Init, in panel units: percentages as fractions,
//! levels in decibels, rates in hertz, arc-minutes, millimetres and events a second as the panel
//! reads them, switches and lists as their 0-based positions. A control a design does not name stays
//! at Init; Mix is never named. [`generate`] writes every parameter but Mix, which a factory sound
//! keeps from the player (`is_factory_preserved`); `examples/fx_oldify_build_presets.rs` writes them
//! to `presets/`, and the bank's tests hold the committed files to what it emits today.
//!
//! The values are first estimates, to be tuned by ear (the design's own notes are private,
//! `ops:notes/oldify/`).

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
const US_78: f32 = 2.0;
const UK_78: f32 = 3.0;
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
    ("wow", 0.048),
    ("wow_rate", 0.6),
    ("wow_shape", RANDOM),
    ("flutter", 0.014),
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
    ("wow", 0.09),
    ("wow_rate", 3.0),
    ("wow_shape", RANDOM),
    ("flutter", 0.032),
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
    ("surface_noise", -38.0),
    ("crackle", 500.0),
    ("clicks", 3.0),
    ("wear", 0.3),
];

const VINYL_FOR_A_MIXTAPE: Part = &[
    ("disc", LP),
    ("era", MODERN),
    ("player", MODERN_DECK),
    ("surface_noise", -60.0),
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
                ("wow", 0.01),
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
                ("wow", 0.203),
                ("wow_rate", 1.5),
                ("wow_shape", RANDOM),
                ("flutter", 0.062),
                ("flutter_rate", 9.0),
                ("flutter_shape", RANDOM),
                ("hiss", -50.0),
                ("dropouts", 0.35),
                ("dropout_intensity", 0.5),
                ("dropout_width", 0.4),
                ("age", 0.8),
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
                ("wow", 0.062),
                ("wow_rate", 0.6),
                ("wow_shape", RANDOM),
                ("flutter", 0.014),
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
                ("wow", 0.022),
                ("flutter", 0.006),
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
                ("wow", 0.04),
                ("flutter", 0.01),
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
                ("wow", 0.49),
                ("wow_rate", 0.8),
                ("wow_shape", SINE),
                ("flutter", 0.062),
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
                ("wow", 0.16),
                ("wow_rate", 0.3),
                ("wow_shape", SINE),
                ("flutter", 0.0),
                ("hiss", -70.0),
                ("age", 0.1),
                ("saturation", 0.15),
            ],
        ],
    ),
    // ---- Record ----
    (
        "Acoustic 78 on a wind-up",
        Category::Fx,
        "A horn-recorded 78 from before 1925 on a wind-up gramophone: narrow, loud and crackling.",
        &[
            ON_RECORD,
            &[
                ("disc", SHELLAC),
                ("era", ACOUSTIC),
                ("player", WIND_UP),
                ("surface_noise", -28.0),
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
                ("surface_noise", -35.0),
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
                ("surface_noise", -33.0),
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
                ("surface_noise", -30.0),
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
                ("surface_noise", -45.0),
                ("crackle", 150.0),
                ("clicks", 2.0),
                ("wear", 0.4),
                ("side", 0.5),
                ("eccentricity", 0.3),
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
                ("surface_noise", -56.0),
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
                ("surface_noise", -58.0),
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
                ("surface_noise", -36.0),
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
                ("surface_noise", -55.0),
                ("crackle", 300.0),
                ("clicks", 6.0),
                ("wear", 0.5),
                ("side", 0.8),
                ("eccentricity", 0.3),
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
];

/// The parameters whose panel value is a level in decibels, stored as linear gain.
const IN_DECIBELS: &[&str] = &[
    "hiss",
    "crosstalk",
    "surface_noise",
    "rumble",
    "hum_level",
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
        // A factory recipe leaves out Mix, which the shared resolver keeps from the live value.
        preset.params.remove("mix");
        let mut named = BTreeSet::new();
        for &(id, panel) in parts.iter().flat_map(|part| part.iter()) {
            assert!(named.insert(id), "{name}: {id} is set twice");
            assert_ne!(id, "mix", "{name}: a factory sound keeps the player's Mix");
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
            erased.len() - 1,
            "{name}: a factory recipe is complete but for Mix"
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
