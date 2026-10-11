//! Eight Effects cards in signal order, in two groups and the output: Vinyl, Vinyl groove,
//! Vinyl surface; Tape, Tape transport, Tape noise, Tape deck; Output.
//!
//! **A group is switched on its first card**: the Vinyl switch heads Vinyl, the Tape switch heads
//! Tape, and a group switched off is drawn dimmed but stays editable and reachable (the owner,
//! 2026-10-09). Each card keeps the key the brief gives it (`docs/briefs/mxm-fx-oldify.md` §4).
//! Each card body is a `mxm_ui::tree`: [`card`] describes it once, the paging renderer measures that
//! one description for the card's floor and height, and [`paint`] draws it leaf by leaf through the
//! bindings, so the controls, their gestures and their names are the bindings' own. Nothing is typed
//! and nothing is drawn to learn a size.

use std::collections::HashMap;

use egui::{Ui, Vec2, vec2};
use mxm_ui::control::{Size, Wave};
use mxm_ui::theme::Tokens;
use mxm_ui::tree::{Height, Kind, Node, leaf, row_gap, stack, switch_beside_knob};
use mxm_ui::visual::{AXIS_STROKE, CANVAS_RADIUS, EMPHASIS_STROKE};
use nice_plug::prelude::ParamSetter;

use super::binding::{Bound, segmented, segmented_waves, selector, toggle_labelled};
use crate::params::MxmFxOldifyParams;

const KNOB: Size = Size::Standard;
/// The cards' titles, in paging order.
pub const TITLES: [&str; 8] = [
    "Vinyl",
    "Vinyl groove",
    "Vinyl surface",
    "Tape",
    "Tape transport",
    "Tape noise",
    "Tape deck",
    "Output",
];
/// The cards' stable keys, the brief's; its Medium card's key 0 is retired with the card.
pub const KEYS: [u64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

/// How much a switched-off group's controls keep of their emphasis: the shared disabled module's
/// (`mxm_ui::shell`, *a disabled module reads at reduced emphasis but stays operable-looking and
/// legible*).
pub const DIMMED: f32 = 0.55;

/// The group a card belongs to, by its index: its switch's id, or `None` for Output.
fn group_of(index: usize) -> Option<&'static str> {
    match index {
        0..=2 => Some("record_on"),
        3..=6 => Some("tape_on"),
        _ => None,
    }
}

/// Whether card `index` belongs to a group switched off.
pub fn is_dimmed(params: &MxmFxOldifyParams, index: usize) -> bool {
    match group_of(index) {
        Some("record_on") => !params.record_on.value(),
        Some(_) => !params.tape_on.value(),
        None => false,
    }
}

/// The level display's stated size; it fills its card's width.
pub const LEVELS_MIN: Vec2 = vec2(160.0, 53.0);

/// Drawn, not named (design system §7.3), each announcing its option's own name. *Random* wanders
/// smoothly, so it is the smooth-random picture rather than sample-and-hold steps.
const WOW_SHAPES: [Wave; 4] = [Wave::Sine, Wave::SmoothRandom, Wave::RampUp, Wave::RampDown];
const FLUTTER_SHAPES: [Wave; 2] = [Wave::Sine, Wave::SmoothRandom];

/// The telemetry a frame paints: the input and output peaks.
#[derive(Clone, Copy, Default)]
pub struct Display {
    pub levels: [f32; 2],
}

/// The ladder of the rate `id`, if it has a tempo sync.
fn ladder_of(id: &str) -> Option<mxm_tempo::Ladder> {
    match id {
        "wow_rate" => Some(crate::params::WOW_SYNC),
        "flutter_rate" => Some(crate::params::FLUTTER_SYNC),
        _ => None,
    }
}

/// **The division a synced rate reads**, or `None` for its free value: sync off, or no tempo.
fn shown_division(
    params: &MxmFxOldifyParams,
    id: &str,
    tempo: Option<f64>,
) -> Option<mxm_tempo::Division> {
    use nice_plug::prelude::Param as _;
    let (on, param): (bool, &nice_plug::prelude::FloatParam) = match id {
        "wow_rate" => (params.wow_sync.value(), &params.wow_rate),
        "flutter_rate" => (params.flutter_sync.value(), &params.flutter_rate),
        _ => return None,
    };
    if !on {
        return None;
    }
    let ladder = ladder_of(id)?;
    let (lo, hi) = (
        f64::from(param.preview_plain(0.0)),
        f64::from(param.preview_plain(1.0)),
    );
    ladder.shown(param.unmodulated_normalized_value(), tempo, lo, hi)
}

/// A rate's tempo sync, the quarter note on its knob's grid.
fn sync_beside(id: &'static str) -> Node<Leaf> {
    switch_beside_knob(KNOB, leaf(Leaf::Picture(id), Kind::SyncToggle))
}

pub fn all_parameters(params: &MxmFxOldifyParams) -> Vec<Bound<'_>> {
    vec![
        Bound::new(
            "record_on",
            &params.record_on,
            "Plays the sound as an old record, vinyl or shellac; with Tape on too, the record is taped onto a cassette.",
        ),
        Bound::new(
            "disc",
            &params.disc,
            "The record's speed and size, from a long-player to a shellac 78.",
        ),
        Bound::new(
            "era",
            &params.era,
            "When the record was cut, and so how it was recorded and cut.",
        ),
        Bound::new(
            "player",
            &params.player,
            "What the record is played on, from a wind-up gramophone to a modern deck.",
        ),
        Bound::new(
            "cut_curve",
            &params.cut_curve,
            "The treble and bass curve the record was cut with.",
        ),
        Bound::new(
            "replay_curve",
            &params.replay_curve,
            "The curve it is played back with; a different one from the cut tilts the sound.",
        ),
        Bound::new(
            "horn",
            &params.horn,
            "How much of the sound went through acoustic horns, into the record and out of the gramophone: thin, honky and tinny, as a real old 78.",
        ),
        Bound::new(
            "horn_size",
            &params.horn_size,
            "How long the horn is: a short one honks higher and sounds more tinny, a long one fuller.",
        ),
        Bound::new(
            "wear",
            &params.wear,
            "How worn the groove is: a dulled top, more distortion and more surface noise.",
        ),
        Bound::new(
            "dynamic_wear",
            &params.dynamic_wear,
            "How much worse the loud passages are worn: loud bass and sharp treble go gritty, distorted and dull.",
        ),
        Bound::new(
            "side",
            &params.side,
            "Where on the side the needle is, from the outer groove to the inner, which is duller and more distorted.",
        ),
        Bound::new(
            "groove_distortion",
            &params.groove_distortion,
            "How much the needle distorts tracing the groove; 100 % is what the groove itself gives.",
        ),
        Bound::new(
            "eccentricity",
            &params.eccentricity,
            "How far off centre the hole is, so the pitch sways once a turn. Past about 2.5 mm it sways further than any real record.",
        ),
        Bound::new(
            "warp",
            &params.warp,
            "How warped the disc is, so the pitch lurches once a turn and the level dips with it. Real warps sit low on the knob; higher is for effect.",
        ),
        Bound::new(
            "crackle",
            &params.crackle,
            "How many tiny crackles a second, from a clean disc to a worn 78.",
        ),
        Bound::new(
            "crackle_level",
            &params.crackle_level,
            "How loud each crackle is: 0 dB is a typical record's; at the top a battered 78's crackle rivals the music.",
        ),
        Bound::new(
            "clicks",
            &params.clicks,
            "How many louder clicks a second, from dust and specks in the groove.",
        ),
        Bound::new(
            "surface_noise",
            &params.surface_noise,
            "The disc's own hiss and roar, from off to a worn 78's.",
        ),
        Bound::new(
            "scratches",
            &params.scratches,
            "How many scratches cross the record, each ticking once a turn for half a minute to a minute before the next turns up elsewhere.",
        ),
        Bound::new(
            "cracks",
            &params.cracks,
            "How many cracks run across the record, each popping once a turn for half a minute to a minute before the next turns up elsewhere.",
        ),
        Bound::new(
            "rumble",
            &params.rumble,
            "The turntable's low rumble, from off to loud.",
        ),
        Bound::new(
            "hum",
            &params.hum,
            "Mains hum, at the frequency of the mains it is played from.",
        ),
        Bound::new(
            "hum_level",
            &params.hum_level,
            "How loud the hum is, from off to loud.",
        ),
        Bound::new(
            "tape_on",
            &params.tape_on,
            "Plays the sound through a cassette tape; with Vinyl on too, the record is taped onto it.",
        ),
        Bound::new(
            "tape",
            &params.tape,
            "The kind of tape, from warm ferric to bright metal.",
        ),
        Bound::new(
            "grade",
            &params.grade,
            "How good the tape and the deck are, from cheap to the best there was.",
        ),
        Bound::new(
            "tape_tone",
            &params.tape_tone,
            "How much of the tape's own tone is heard.",
        ),
        Bound::new(
            "saturation",
            &params.saturation,
            "How hard the tape is pushed into distortion; zero is clean.",
        ),
        Bound::new(
            "character",
            &params.character,
            "How the tape distorts when pushed.",
        ),
        Bound::new(
            "generations",
            &params.generations,
            "How many times the tape has been copied: each copy adds its own hiss, wobble and dullness.",
        ),
        Bound::new(
            "age",
            &params.age,
            "How worn the tape is, from new to battered: older loses more of the top.",
        ),
        Bound::new(
            "wow",
            &params.wow,
            "How much the pitch drifts slowly up and down, as an uneven tape speed does. Past 100 % it drifts further than any real tape.",
        ),
        Bound::new("wow_rate", &params.wow_rate, "How fast the slow drift goes."),
        Bound::new(
            "wow_sync",
            &params.wow_sync,
            super::binding::SYNC_DESCRIPTION,
        ),
        Bound::new(
            "wow_shape",
            &params.wow_shape,
            "The shape of the slow drift.",
        ),
        Bound::new(
            "flutter",
            &params.flutter,
            "How much the pitch trembles quickly, roughening the sound. Past 100 % it trembles further than any real tape.",
        ),
        Bound::new(
            "flutter_rate",
            &params.flutter_rate,
            "How fast the trembling goes.",
        ),
        Bound::new(
            "flutter_sync",
            &params.flutter_sync,
            super::binding::SYNC_DESCRIPTION,
        ),
        Bound::new(
            "flutter_shape",
            &params.flutter_shape,
            "The shape of the trembling.",
        ),
        Bound::new(
            "follow",
            &params.follow,
            "The slow drift stops moving the pitch itself, and speeds the trembling up and slows it down instead, as a slipping belt does.",
        ),
        Bound::new(
            "flange",
            &params.flange,
            "Blends in a steady copy of the tape, for the sweeping sound of two tapes side by side.",
        ),
        Bound::new("hiss", &params.hiss, "The tape's own hiss, from off to loud."),
        Bound::new(
            "nr",
            &params.nr,
            "Lowers the hiss by lifting quiet treble on the way onto the tape and taking it back off on the way out.",
        ),
        Bound::new(
            "nr_decode",
            &params.nr_decode,
            "Off plays the tape without its decoder: brighter, compressed, and breathing on quiet passages.",
        ),
        Bound::new(
            "nr_amount",
            &params.nr_amount,
            "How much quiet treble is lifted and taken back: more lowers the hiss further.",
        ),
        Bound::new(
            "nr_brightness",
            &params.nr_brightness,
            "Where the noise reduction's band begins, from the upper middle up into the treble.",
        ),
        Bound::new(
            "nr_level_error",
            &params.nr_level_error,
            "How far the playback level is off from the recording's, so the treble pumps and the hiss breathes with the music.",
        )
        .bipolar(),
        Bound::new(
            "dropouts",
            &params.dropouts,
            "How often the tape briefly loses touch with the head, dipping the sound.",
        ),
        Bound::new(
            "dropout_intensity",
            &params.dropout_intensity,
            "Trades many shallow dropouts for fewer, longer and deeper ones.",
        ),
        Bound::new(
            "dropout_width",
            &params.dropout_width,
            "How differently the dropouts hit left and right, from both at once to each on its own.",
        ),
        Bound::new(
            "azimuth",
            &params.azimuth,
            "How far the playback head is tilted: it dulls the top, and more so in mono.",
        ),
        Bound::new(
            "crosstalk",
            &params.crosstalk,
            "How much each side leaks into the other, from off to a worn deck's.",
        ),
        Bound::new(
            "wrong_eq",
            &params.wrong_eq,
            "Plays the tape back with the wrong treble setting, too bright or too dull.",
        ),
        Bound::new(
            "drive",
            &params.drive,
            "How hard the sound goes onto the tape: more distorts more and buries the hiss, without getting louder.",
        )
        .bipolar(),
        Bound::new(
            "width",
            &params.width,
            "How wide the stereo image is, from mono to twice as wide.",
        )
        .bipolar(),
        Bound::new(
            "mix",
            &params.mix,
            "The balance of the dry sound and the aged one; at zero the effect is off.",
        ),
        Bound::new(
            "noise_follows",
            &params.noise_follows,
            "How far the noise rises with loud passages and falls with quiet ones.",
        ),
        Bound::new(
            "noise_stops",
            &params.noise_stops,
            "Stops the noise a couple of seconds after the sound does, never in the gaps between hits; off, it plays on in silence.",
        ),
        Bound::new(
            "variation",
            &params.variation,
            "Picks another take of the wobble, hiss, dropouts and crackle; the same number always sounds the same.",
        ),
    ]
}

fn bound<'a>(id: &str, params: &'a MxmFxOldifyParams) -> Bound<'a> {
    all_parameters(params)
        .into_iter()
        .find(|bound| bound.id == id)
        .expect("every drawn parameter is bound")
}

pub fn cards(
    ui: &mut Ui,
    tokens: &Tokens,
    params: &MxmFxOldifyParams,
    setter: &ParamSetter<'_>,
    text: &mut HashMap<&'static str, Option<String>>,
    tempo: Option<f64>,
    display: Display,
) -> f32 {
    let items = page_items(ui, params);
    let text_editing = text.values().any(Option::is_some);
    let mut live = Live {
        params,
        setter,
        text,
        tempo,
        display,
    };
    let report = mxm_ui::paging::editor::show(
        ui,
        tokens,
        &items,
        &[],
        text_editing,
        &mut |ui, index| card(ui, index, params),
        &mut |ui, index, leaf, rect| paint(ui, tokens, index, leaf, rect, &mut live),
    );
    report
        .visible
        .iter()
        .map(|(_, rect)| rect.bottom())
        .fold(ui.min_rect().bottom(), f32::max)
}

/// Every paging item, each floor computed from its card's tree in `ui`'s fonts: the tree's
/// narrowest and the card's chrome. There is no usability minimum to add, and each card is as wide
/// as its floor.
pub fn page_items(ui: &Ui, params: &MxmFxOldifyParams) -> Vec<mxm_ui::paging::Item<'static>> {
    use mxm_ui::paging::{Category, Item, Key};
    TITLES
        .iter()
        .zip(KEYS)
        .enumerate()
        .map(|(index, (title, key))| Item {
            key: Key(key),
            card: {
                let floor = mxm_ui::tree::card_floor(ui, title, &card(ui, index, params));
                mxm_ui::flow::Card::new(title, floor).capped(floor)
            },
            category: Category::Effects,
            kind: title,
        })
        .collect()
}

/// The paging items as the editor computes them, from a context set up as an editor's is — three
/// passes in, so the weighted font cuts are bound — for tests, which have no editor `Ui` to hand.
#[cfg(test)]
pub(crate) fn test_items() -> Vec<mxm_ui::paging::Item<'static>> {
    let ctx = egui::Context::default();
    mxm_ui::typography::apply(&ctx);
    mxm_ui::theme::apply(&ctx);
    let params = MxmFxOldifyParams::default();
    let mut items = Vec::new();
    for _ in 0..3 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            items = page_items(ui, &params);
        });
        output.textures_delta.clear();
    }
    items
}

/// The cards' floors in paging order, as [`test_items`] computes them.
#[cfg(test)]
pub(crate) fn test_floors() -> Vec<f32> {
    test_items().iter().map(|item| item.card.floor).collect()
}

/// What a leaf of this editor's cards draws. Hashed by what it names, which keeps its widget ids
/// stable wherever the tree places it.
#[derive(Clone, Copy, Debug, Hash)]
pub enum Leaf {
    Knob(&'static str),
    /// A list too long for a row of buttons.
    Selector(&'static str),
    Segmented(&'static str),
    Waves(&'static str),
    /// An on/off parameter's switch, labelled with its name.
    Toggle(&'static str),
    /// A rate's tempo sync, the quarter note beside it.
    Picture(&'static str),
    Levels,
}

/// A stepped parameter's painted cells: every option, as its own formatted text, so a host's list
/// and the switch cannot disagree.
fn options(params: &MxmFxOldifyParams, id: &str) -> Vec<String> {
    let param = bound(id, params).param;
    let last = param
        .steps()
        .unwrap_or_else(|| panic!("{id} is not a segmented control here"));
    (0..=last)
        .map(|option| param.format(option as f32 / last as f32))
        .collect()
}

/// The pictures of a shape switch's cells.
fn waves_of(id: &str) -> &'static [Wave] {
    match id {
        "wow_shape" => &WOW_SHAPES,
        "flutter_shape" => &FLUTTER_SHAPES,
        _ => &[],
    }
}

/// The collection's knob row (`mxm_ui::tree::knob_row`): equal columns at the one knob column.
fn knobs(ui: &Ui, params: &MxmFxOldifyParams, ids: &[&'static str]) -> Node<Leaf> {
    mxm_ui::tree::knob_row(
        ui,
        ids.iter()
            .map(|id| {
                let bound = bound(id, params);
                let param = bound.param;
                // A synced rate's column holds its free readings and every division.
                let widest = match ladder_of(id) {
                    Some(ladder) => super::binding::synced_widest(param, ladder.span),
                    None => mxm_ui::control::widest_value(|n| param.format(n as f32)),
                };
                let knob = leaf(
                    Leaf::Knob(id),
                    Kind::Knob {
                        name: bound.painted().to_owned(),
                        widest,
                        size: KNOB,
                        // In the collection's knob row, which sizes the columns.
                        column: 0.0,
                    },
                );
                (KNOB, knob)
            })
            .collect(),
    )
}

/// What each option of a stepped control does, one sentence per cell in the parameter's own order
/// (design system §7.3: the cells of a row do not share one sentence).
fn details_of(id: &str) -> &'static [&'static str] {
    match id {
        "disc" => &[
            "A long-player turning at 33⅓.",
            "A seven-inch single turning at 45.",
            "A shellac disc turning at 78.",
        ],
        "cracks" => &[
            "No cracks across the record.",
            "One crack, popping once a turn.",
            "Two cracks, two pops a turn.",
            "Three cracks, three pops a turn.",
        ],
        "hum" => &[
            "No hum from the mains.",
            "The hum of 50 Hz mains, as in Europe.",
            "The hum of 60 Hz mains, as in America.",
        ],
        "tape" => &[
            "Ferric tape: warm, with a soft top.",
            "Chrome tape: brighter, with less hiss.",
            "Metal tape: the brightest and cleanest.",
        ],
        "grade" => &[
            "A cheap tape on a cheap deck: dull and muffled.",
            "An everyday tape on an everyday deck.",
            "A good tape on a good deck.",
            "The best tape on the best deck: the most open sound.",
        ],
        "character" => &[
            "Rounds the peaks off gently and warmly.",
            "Clips the peaks with a sharper, harder edge.",
        ],
        "wow_shape" => &[
            "An even, smooth drift up and down.",
            "Wanders unpredictably, holding a pitch and then stepping to another.",
            "Sags slowly flat, then catches up quickly.",
            "Creeps slowly sharp, then drops back quickly.",
        ],
        "flutter_shape" => &[
            "An even, steady tremble.",
            "An irregular tremble, stepping unpredictably.",
        ],
        "nr" => &[
            "No noise reduction: the hiss as the tape made it.",
            "One band on the treble, as on most cassettes of the time.",
            "Two bands reaching lower, for about twice the hiss reduction.",
        ],
        "generations" => &[
            "The first recording, as it was made.",
            "A copy of the first recording.",
            "A copy of a copy.",
            "Three copies down from the first recording.",
        ],
        _ => &[],
    }
}

/// What each option of a list says, for the selector's hover: one list offers one sentence for the
/// whole control, so the option chosen adds its own to it (design system §7.3's rule, kept for a
/// list).
fn option_sentences(id: &str) -> &'static [&'static str] {
    match id {
        "era" => &[
            "Before 1925: cut through a horn, no microphone, only about 250 Hz to 2.5 kHz.",
            "1925 to 1935: the first microphones and electrical cutters, with a ringing top near 4.5 kHz.",
            "American 78s of the 1940s: the top cut off sharply at 8.5 kHz and the peaks held down.",
            "British 78s of the 1940s: a wider cut reaching about 12 kHz, with the peaks left alone.",
            "78s of the 1950s, cut with full-range microphones to about 14 kHz.",
            "The first 45s, 1949 to 1952: bright, with the peaks held down.",
            "The first long-players, 1948 to 1954, before one curve was agreed.",
            "Mono long-players of 1955 to 1958 on the standard curve, nearly full range.",
            "From stereo on: full range, and nothing added by the cut.",
        ],
        "player" => &[
            "A wind-up gramophone: a steel needle, a soundbox and a horn, nothing electrical.",
            "A 1940s radiogram: a crystal pickup, a scratch filter, valves and a cabinet speaker.",
            "A 1949 jukebox: a heavy pickup, push-pull valves and a big speaker.",
            "A 1950s portable: a crystal pickup, valves and a small speaker.",
            "A 1955 mono hi-fi: a magnetic pickup and a good speaker.",
            "A record on the 1940s radio: the station's turntable, the broadcast and a table radio.",
            "A modern deck with a fine cartridge, adding nothing of its own.",
        ],
        "cut_curve" => &[
            "The curve the record's era was cut with.",
            "No curve at all: the groove as the sound made it.",
            "British 78s from 1943 to 1953.",
            "British 78s from 1944 to 1956, with the top lifted.",
            "American 78s from 1932 to 1949.",
            "The first American long-players, 1948 to 1954.",
            "The first 45s, 1949 to 1952.",
            "Discs cut for broadcast.",
            "Long-players from the West Coast, from 1950.",
            "The curve every record has used since the mid-1950s.",
        ],
        "replay_curve" => &[
            "The curve the player plays back with.",
            "No curve at all: the groove played as it is.",
            "British 78s from 1943 to 1953.",
            "British 78s from 1944 to 1956, with the top lifted.",
            "American 78s from 1932 to 1949.",
            "The first American long-players, 1948 to 1954.",
            "The first 45s, 1949 to 1952.",
            "Discs cut for broadcast.",
            "Long-players from the West Coast, from 1950.",
            "The curve every record has used since the mid-1950s.",
        ],
        _ => &[],
    }
}

/// A list's leaf: the collection's caret selector (§7.4), for a choice too long for a row of
/// buttons.
fn selector_leaf(params: &MxmFxOldifyParams, id: &'static str) -> Node<Leaf> {
    leaf(
        Leaf::Selector(id),
        Kind::Selector {
            label: bound(id, params).painted().to_owned(),
            options: options(params, id),
            caption: false,
            width: None,
        },
    )
}

fn segmented_leaf(params: &MxmFxOldifyParams, id: &'static str) -> Node<Leaf> {
    leaf(
        Leaf::Segmented(id),
        Kind::Segmented {
            label: bound(id, params).painted().to_owned(),
            options: options(params, id),
            beside: None,
        },
    )
}

fn waves_leaf(params: &MxmFxOldifyParams, id: &'static str) -> Node<Leaf> {
    leaf(
        Leaf::Waves(id),
        Kind::Waves {
            label: Some(bound(id, params).painted().to_owned()),
            count: waves_of(id).len(),
            marks: Vec::new(),
            beside: None,
        },
    )
}

/// An on/off parameter's switch, labelled with its name (design system §7.2).
fn toggle_leaf(params: &MxmFxOldifyParams, id: &'static str) -> Node<Leaf> {
    leaf(
        Leaf::Toggle(id),
        Kind::Toggle {
            label: bound(id, params).painted().to_owned(),
        },
    )
}

/// A display that fills its card's width, at its stated size.
fn display(key: Leaf, min: Vec2) -> Node<Leaf> {
    leaf(
        key,
        Kind::Custom {
            min_width: min.x,
            height: Height::Fixed(min.y),
            fills: true,
        },
    )
}

/// Card `index`'s body, as a tree. Nothing in it follows telemetry: the level display states its
/// size, and only its painting reads the live levels.
pub fn card(ui: &Ui, index: usize, params: &MxmFxOldifyParams) -> Node<Leaf> {
    let gap = ui.spacing().item_spacing.x;
    match index {
        // Each group's switch heads its first card.
        0 => stack(vec![
            toggle_leaf(params, "record_on"),
            segmented_leaf(params, "disc"),
            selector_leaf(params, "era"),
            selector_leaf(params, "player"),
            selector_leaf(params, "cut_curve"),
            selector_leaf(params, "replay_curve"),
            knobs(ui, params, &["horn", "horn_size"]),
        ]),
        1 => stack(vec![
            knobs(ui, params, &["wear", "dynamic_wear", "side"]),
            knobs(ui, params, &["groove_distortion", "eccentricity", "warp"]),
        ]),
        2 => stack(vec![
            knobs(ui, params, &["crackle", "crackle_level", "clicks"]),
            knobs(ui, params, &["surface_noise", "scratches", "rumble"]),
            segmented_leaf(params, "cracks"),
            segmented_leaf(params, "hum"),
            knobs(ui, params, &["hum_level"]),
        ]),
        3 => stack(vec![
            toggle_leaf(params, "tape_on"),
            segmented_leaf(params, "tape"),
            segmented_leaf(params, "grade"),
            knobs(ui, params, &["tape_tone", "saturation"]),
            segmented_leaf(params, "character"),
            segmented_leaf(params, "generations"),
            knobs(ui, params, &["age"]),
        ]),
        // Each rate with its tempo sync straight after it, then its shape.
        4 => stack(vec![
            row_gap(
                gap,
                vec![
                    knobs(ui, params, &["wow", "wow_rate"]),
                    sync_beside("wow_sync"),
                ],
            ),
            waves_leaf(params, "wow_shape"),
            row_gap(
                gap,
                vec![
                    knobs(ui, params, &["flutter", "flutter_rate"]),
                    sync_beside("flutter_sync"),
                ],
            ),
            waves_leaf(params, "flutter_shape"),
            toggle_leaf(params, "follow"),
            knobs(ui, params, &["flange"]),
        ]),
        5 => stack(vec![
            knobs(ui, params, &["hiss"]),
            segmented_leaf(params, "nr"),
            toggle_leaf(params, "nr_decode"),
            knobs(
                ui,
                params,
                &["nr_amount", "nr_brightness", "nr_level_error"],
            ),
            knobs(
                ui,
                params,
                &["dropouts", "dropout_intensity", "dropout_width"],
            ),
        ]),
        6 => stack(vec![
            knobs(ui, params, &["azimuth", "crosstalk"]),
            toggle_leaf(params, "wrong_eq"),
        ]),
        _ => stack(vec![
            display(Leaf::Levels, LEVELS_MIN),
            knobs(ui, params, &["drive", "width", "mix"]),
            knobs(ui, params, &["noise_follows", "variation"]),
            toggle_leaf(params, "noise_stops"),
        ]),
    }
}

/// Everything a leaf draws with: the parameters and their host, the text-entry buffers, and the
/// telemetry snapshot taken once before the frame.
pub struct Live<'a, 'b> {
    pub params: &'a MxmFxOldifyParams,
    pub setter: &'a ParamSetter<'b>,
    pub text: &'a mut HashMap<&'static str, Option<String>>,
    /// The host tempo in force, which a synced rate reads its division against.
    pub tempo: Option<f64>,
    pub display: Display,
}

/// Draws one leaf of card `index`, in the `Ui` the tree bounded to its rectangle, through the
/// bindings. A switched-off group's leaves are drawn at [`DIMMED`], all but its own switch, which
/// stays at full strength so the way back on is plain; dimmed, every control still edits.
pub fn paint(
    ui: &mut Ui,
    tokens: &Tokens,
    index: usize,
    leaf: &Leaf,
    rect: egui::Rect,
    live: &mut Live<'_, '_>,
) {
    let params = live.params;
    let switch = matches!(*leaf, Leaf::Toggle(id) if Some(id) == group_of(index));
    if is_dimmed(params, index) && !switch {
        ui.multiply_opacity(DIMMED);
    }
    match *leaf {
        // At the knob row's column (`tree::knob_row`). Synced, a rate reads the division in
        // force; the host still reads its hertz.
        Leaf::Knob(id) => {
            let bound = bound(id, params);
            match shown_division(params, id, live.tempo) {
                Some(division) => bound.knob_with_reading(
                    ui,
                    tokens,
                    live.setter,
                    KNOB,
                    rect.width(),
                    live.text,
                    division.label(),
                ),
                None => bound.knob(ui, tokens, live.setter, KNOB, rect.width(), live.text),
            }
        }
        Leaf::Picture(id) => {
            super::binding::sync_picture(ui, tokens, id, bound(id, params).param, live.setter);
        }
        Leaf::Toggle(id) => {
            let bound = bound(id, params);
            toggle_labelled(
                ui,
                tokens,
                id,
                bound.param,
                bound.painted(),
                bound.description,
                live.setter,
                0.0,
            );
        }
        Leaf::Selector(id) => {
            let bound = bound(id, params);
            let labels = options(params, id);
            let options: Vec<&str> = labels.iter().map(String::as_str).collect();
            let chosen = (bound.param.normalised() * (options.len() - 1) as f32).round() as usize;
            let hover = match option_sentences(id).get(chosen) {
                Some(sentence) => format!("{} {sentence}", bound.description),
                None => bound.description.to_owned(),
            };
            selector(
                ui,
                tokens,
                id,
                bound.param,
                &options,
                None,
                &hover,
                live.setter,
            );
        }
        Leaf::Segmented(id) => {
            let bound = bound(id, params);
            let labels = options(params, id);
            let options: Vec<&str> = labels.iter().map(String::as_str).collect();
            segmented(
                ui,
                tokens,
                id,
                bound.param,
                &options,
                None,
                details_of(id),
                live.setter,
            );
        }
        Leaf::Waves(id) => {
            let bound = bound(id, params);
            let labels = options(params, id);
            let shapes: Vec<(Wave, &str)> = waves_of(id)
                .iter()
                .copied()
                .zip(labels.iter().map(String::as_str))
                .collect();
            segmented_waves(
                ui,
                tokens,
                id,
                bound.param,
                &shapes,
                None,
                details_of(id),
                live.setter,
            );
        }
        Leaf::Levels => level_display(ui, tokens, live.display),
    }
}

fn level_display(ui: &mut Ui, tokens: &Tokens, display: Display) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(LEVELS_MIN.x), LEVELS_MIN.y),
        egui::Sense::hover(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Input and output levels")
    });
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CANVAS_RADIUS, tokens.surface_2);
    for (index, (label, level)) in ["In", "Out"].into_iter().zip(display.levels).enumerate() {
        let y = rect.top() + 14.0 + index as f32 * 25.0;
        painter.text(
            egui::pos2(rect.left() + 8.0, y),
            egui::Align2::LEFT_CENTER,
            label,
            mxm_ui::typography::caption_style(ui.style()).resolve(ui.style()),
            tokens.text_secondary,
        );
        let left = rect.left() + 40.0;
        let right = egui::lerp(left..=(rect.right() - 8.0), level.clamp(0.0, 1.0));
        painter.line_segment(
            [egui::pos2(left, y), egui::pos2(rect.right() - 8.0, y)],
            egui::Stroke::new(AXIS_STROKE, tokens.border),
        );
        painter.line_segment(
            [egui::pos2(left, y), egui::pos2(right, y)],
            egui::Stroke::new(EMPHASIS_STROKE, tokens.accent),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::prelude::Params;

    #[test]
    fn every_parameter_is_bound_once() {
        let params = MxmFxOldifyParams::default();
        let ids: Vec<_> = all_parameters(&params)
            .iter()
            .map(|bound| bound.id)
            .collect();
        assert_eq!(ids.len(), params.param_map().len());
        for (id, _, _) in params.param_map() {
            assert_eq!(ids.iter().filter(|bound| **bound == id).count(), 1, "{id}");
        }
    }

    /// **Each button of a row says what it does**, and each shape cell has its picture.
    #[test]
    fn every_switch_cell_has_its_own_sentence_and_every_shape_its_picture() {
        let params = MxmFxOldifyParams::default();
        for id in [
            "tape",
            "grade",
            "character",
            "generations",
            "nr",
            "disc",
            "cracks",
            "hum",
            "wow_shape",
            "flutter_shape",
        ] {
            assert_eq!(options(&params, id).len(), details_of(id).len(), "{id}");
        }
        for id in ["wow_shape", "flutter_shape"] {
            assert_eq!(options(&params, id).len(), waves_of(id).len(), "{id}");
        }
        for id in ["era", "player", "cut_curve", "replay_curve"] {
            assert_eq!(
                options(&params, id).len(),
                option_sentences(id).len(),
                "{id}"
            );
        }
    }

    /// **A group switched off is dimmed, its switch never**, and the output never is.
    #[test]
    fn a_group_switched_off_is_dimmed_and_output_never() {
        use nice_plug::params::InternalParamMut;
        let params = MxmFxOldifyParams::default();
        // Init: the record off, the tape on.
        let dimmed: Vec<_> = (0..TITLES.len()).map(|i| is_dimmed(&params, i)).collect();
        assert_eq!(
            dimmed,
            [true, true, true, false, false, false, false, false]
        );
        unsafe {
            let _ = params.record_on._internal_set_normalized_value(1.0);
            let _ = params.tape_on._internal_set_normalized_value(0.0);
        }
        let dimmed: Vec<_> = (0..TITLES.len()).map(|i| is_dimmed(&params, i)).collect();
        assert_eq!(dimmed, [false, false, false, true, true, true, true, false]);
        assert_eq!(group_of(0), Some("record_on"));
        assert_eq!(group_of(3), Some("tape_on"));
        assert_eq!(group_of(TITLES.len() - 1), None);
    }

    #[test]
    fn the_keys_are_the_briefs_and_unique() {
        let mut keys = KEYS.to_vec();
        keys.dedup();
        assert_eq!(keys.len(), TITLES.len());
        assert!(KEYS.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
