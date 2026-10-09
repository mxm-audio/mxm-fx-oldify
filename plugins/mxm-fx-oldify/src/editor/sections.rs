//! Five Effects cards in signal order: Medium, Tape, Transport, Noise, Output.
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
use mxm_ui::tree::{Height, Kind, Node, leaf, stack};
use mxm_ui::visual::{AXIS_STROKE, CANVAS_RADIUS, EMPHASIS_STROKE};
use nice_plug::prelude::ParamSetter;

use super::binding::{Bound, segmented, segmented_waves};
use crate::params::MxmFxOldifyParams;

const KNOB: Size = Size::Standard;
/// The cards' titles, in paging order.
pub const TITLES: [&str; 5] = ["Medium", "Tape", "Transport", "Noise", "Output"];
/// The cards' stable keys, the brief's: the record engine's cards take 1–3 and the deck's 7.
pub const KEYS: [u64; 5] = [0, 4, 5, 6, 8];

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
            "wow",
            &params.wow,
            "How much the pitch drifts slowly up and down, as an uneven tape speed does.",
        ),
        Bound::new("wow_rate", &params.wow_rate, "How fast the slow drift goes."),
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
            "flutter_shape",
            &params.flutter_shape,
            "The shape of the trembling.",
        ),
        Bound::new(
            "hiss",
            &params.hiss,
            "The tape's own hiss, from off to loud; it fades out when the sound stops.",
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
    display: Display,
) -> f32 {
    let items = page_items(ui, params);
    let text_editing = text.values().any(Option::is_some);
    let mut live = Live {
        params,
        setter,
        text,
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
                let knob = leaf(
                    Leaf::Knob(id),
                    Kind::Knob {
                        name: bound.painted().to_owned(),
                        widest: mxm_ui::control::widest_value(|n| param.format(n as f32)),
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
    match index {
        0 => stack(vec![
            segmented_leaf(params, "medium"),
            knobs(ui, params, &["age"]),
        ]),
        1 => stack(vec![
            segmented_leaf(params, "tape"),
            segmented_leaf(params, "grade"),
            knobs(ui, params, &["tape_tone", "saturation"]),
            segmented_leaf(params, "character"),
        ]),
        2 => stack(vec![
            knobs(ui, params, &["wow", "wow_rate"]),
            waves_leaf(params, "wow_shape"),
            knobs(ui, params, &["flutter", "flutter_rate"]),
            waves_leaf(params, "flutter_shape"),
        ]),
        3 => stack(vec![knobs(ui, params, &["hiss"])]),
        _ => stack(vec![
            display(Leaf::Levels, LEVELS_MIN),
            knobs(ui, params, &["drive", "width", "output", "mix"]),
        ]),
    }
}

/// Everything a leaf draws with: the parameters and their host, the text-entry buffers, and the
/// telemetry snapshot taken once before the frame.
pub struct Live<'a, 'b> {
    pub params: &'a MxmFxOldifyParams,
    pub setter: &'a ParamSetter<'b>,
    pub text: &'a mut HashMap<&'static str, Option<String>>,
    pub display: Display,
}

/// Draws one leaf, in the `Ui` the tree bounded to its rectangle, through the bindings.
pub fn paint(ui: &mut Ui, tokens: &Tokens, leaf: &Leaf, rect: egui::Rect, live: &mut Live<'_, '_>) {
    let params = live.params;
    match *leaf {
        // At the knob row's column (`tree::knob_row`).
        Leaf::Knob(id) => {
            bound(id, params).knob(ui, tokens, live.setter, KNOB, rect.width(), live.text);
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
