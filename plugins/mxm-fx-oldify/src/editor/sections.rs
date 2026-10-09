//! Six Effects cards in signal order: Medium, Tape, Transport, Noise, Deck, Output.
//!
//! Each card keeps the key the brief gives it (`docs/briefs/mxm-fx-oldify.md` §4), so the record
//! engine's cards can take their places between Medium and Tape without moving these. Each card
//! body is a `mxm_ui::tree`: [`card`] describes it once, the paging renderer measures that one
//! description for the card's floor and height, and [`paint`] draws it leaf by leaf through the
//! bindings, so the controls, their gestures and their names are the bindings' own. Nothing is typed
//! and nothing is drawn to learn a size.

use std::collections::HashMap;

use egui::{Ui, Vec2, vec2};
use mxm_ui::control::{Size, Wave};
use mxm_ui::theme::Tokens;
use mxm_ui::tree::{Height, Kind, Node, leaf, row_gap, stack, switch_beside_knob};
use mxm_ui::visual::{AXIS_STROKE, CANVAS_RADIUS, EMPHASIS_STROKE};
use nice_plug::prelude::ParamSetter;

use super::binding::{Bound, segmented, segmented_waves, toggle_labelled};
use crate::params::MxmFxOldifyParams;

const KNOB: Size = Size::Standard;
/// The cards' titles, in paging order.
pub const TITLES: [&str; 6] = ["Medium", "Tape", "Transport", "Noise", "Deck", "Output"];
/// The cards' stable keys, the brief's: the record engine's cards take 1–3.
pub const KEYS: [u64; 6] = [0, 4, 5, 6, 7, 8];

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
            "medium",
            &params.medium,
            "What the sound has been through: a cassette, a record, or a record taped onto a cassette.",
        ),
        Bound::new(
            "age",
            &params.age,
            "How worn the medium is, from new to battered: older loses more of the top.",
        ),
        Bound::new(
            "variation",
            &params.variation,
            "Picks another take of the wobble, hiss and dropouts; the same number always sounds the same.",
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
            "wow",
            &params.wow,
            "How much the pitch drifts slowly up and down, as an uneven tape speed does.",
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
            "How much the pitch trembles quickly, roughening the sound.",
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
            "Makes the trembling speed up and slow down with the slow drift, as a slipping belt does.",
        ),
        Bound::new(
            "flange",
            &params.flange,
            "Blends in a steady copy of the tape, for the sweeping sound of two tapes side by side.",
        ),
        Bound::new("hiss", &params.hiss, "The tape's own hiss, from off to loud."),
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
        Bound::new("output", &params.output, "The level coming out.").bipolar(),
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
            "Stops the noise a moment after the sound does; off, it plays on in silence.",
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
        &mut |ui, _, leaf, rect| paint(ui, tokens, leaf, rect, &mut live),
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
        "medium" => &[
            "The sound of a cassette tape.",
            "The sound of an old record.",
            "An old record, taped onto a cassette.",
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
            "Wanders unpredictably, as a real tape does.",
            "Drifts up, then drops back.",
            "Drops, then jumps back up.",
        ],
        "flutter_shape" => &[
            "An even, steady tremble.",
            "An irregular, wandering tremble.",
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
        0 => stack(vec![
            segmented_leaf(params, "medium"),
            knobs(ui, params, &["age", "variation"]),
        ]),
        1 => stack(vec![
            segmented_leaf(params, "tape"),
            segmented_leaf(params, "grade"),
            knobs(ui, params, &["tape_tone", "saturation"]),
            segmented_leaf(params, "character"),
            segmented_leaf(params, "generations"),
        ]),
        // Each rate with its tempo sync straight after it, then its shape.
        2 => stack(vec![
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
        3 => stack(vec![
            knobs(ui, params, &["hiss"]),
            knobs(
                ui,
                params,
                &["dropouts", "dropout_intensity", "dropout_width"],
            ),
        ]),
        4 => stack(vec![
            knobs(ui, params, &["azimuth", "crosstalk"]),
            toggle_leaf(params, "wrong_eq"),
        ]),
        _ => stack(vec![
            display(Leaf::Levels, LEVELS_MIN),
            knobs(ui, params, &["drive", "width", "output", "mix"]),
            knobs(ui, params, &["noise_follows"]),
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

/// Draws one leaf, in the `Ui` the tree bounded to its rectangle, through the bindings.
pub fn paint(ui: &mut Ui, tokens: &Tokens, leaf: &Leaf, rect: egui::Rect, live: &mut Live<'_, '_>) {
    let params = live.params;
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
            "medium",
            "tape",
            "grade",
            "character",
            "generations",
            "wow_shape",
            "flutter_shape",
        ] {
            assert_eq!(options(&params, id).len(), details_of(id).len(), "{id}");
        }
        for id in ["wow_shape", "flutter_shape"] {
            assert_eq!(options(&params, id).len(), waves_of(id).len(), "{id}");
        }
    }

    #[test]
    fn the_keys_are_the_briefs_and_unique() {
        let mut keys = KEYS.to_vec();
        keys.dedup();
        assert_eq!(keys.len(), TITLES.len());
        assert!(KEYS.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
