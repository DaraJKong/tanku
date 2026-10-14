//! The pieces that the screens of the menus are drawn with.
//!
//! Every screen is a sheet of one set, as a set of drawings is: the frame with its zones, the
//! name of the screen over its title in the left of the sheet, its controls as the rows of a
//! list, its information in ruled blocks with a label over each, and a title block flush with
//! the frame in the bottom right corner (ISO 7200). A tank that a screen shows is drawn as an
//! assembly, its parts named by balloons and listed above the title block (ISO 7573).
//!
//! Every control is at least 44 by 44 pixels at 1080p, and no letter has capitals under
//! 14.8 pixels: the sizes of drawing 06 of the canvas, which the settings scale.

use bevy::prelude::*;

use crate::{balloons, hud::SHEET_EDGE, sheet::FRAME_WIDTH, theme};

/// Where what a screen holds starts, from the left of the sheet.
pub const LEFT: f32 = 120.0;
/// A row of a list, and the cell of its number.
pub const ROW: f32 = 56.0;
/// The smallest control: a step, and the height of a choice.
pub const CONTROL: f32 = 44.0;
/// The room on each side of what a cell or a field holds.
pub const PAD: f32 = 10.0;
/// The rows of a parts list, and the height of the rows of the title block.
pub const PART_ROW: f32 = 32.0;
pub const TITLE_ROWS: [f32; 2] = [104.0, 70.0];
/// The balloons of a title block, which name who a sheet is drawn for.
const BALLOON: f32 = 36.0;

/// The ground of what a block holds, through which the game shows a little.
const GROUND: Color = theme::BLOCK_GROUND;

/// A label: the name of a field, a header, a note. The smallest lettering, dim.
pub fn key(content: impl Into<String>) -> impl Bundle {
    theme::label(content, theme::LABEL, theme::TEXT_DIM)
}

/// A value: what a field or a cell holds.
pub fn value(content: impl Into<String>, color: Color) -> impl Bundle {
    theme::label(content, theme::VALUE, color)
}

/// The name of what a screen is, in the smallest capitals, over its title.
#[expect(dead_code, reason = "the screens take it as they are drawn again")]
pub fn heading(
    parent: &mut ChildSpawnerCommands,
    what: impl Into<String>,
    title: impl Into<String>,
) {
    parent.spawn(key(what));
    parent.spawn((
        theme::label(title, theme::HEADING, theme::TEXT),
        Node {
            margin: UiRect::bottom(px(12)),
            ..default()
        },
    ));
}

/// A block: edged with the wide line and ruled inside with the narrow one.
pub fn block(node: Node) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            border: UiRect::all(px(FRAME_WIDTH)),
            ..node
        },
        BackgroundColor(GROUND),
        BorderColor::all(theme::RULE),
    )
}

/// The narrow rules of a cell: on its right and under it, where another follows.
pub fn rules(right: bool, below: bool) -> UiRect {
    let line = |on: bool| if on { px(1) } else { px(0) };
    UiRect {
        right: line(right),
        bottom: line(below),
        ..default()
    }
}

/// A row of a table or of a block, ruled under it unless it is the last.
pub fn row(
    parent: &mut ChildSpawnerCommands,
    height: f32,
    below: bool,
    cells: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            Node {
                height: px(height),
                flex_shrink: 0.0,
                border: rules(false, below),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(cells);
}

/// A cell of a row, this wide, ruled on its right unless it is the last, with what it holds in
/// the middle of its height.
pub fn cell(
    parent: &mut ChildSpawnerCommands,
    width: f32,
    right: bool,
    holds: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            Node {
                width: px(width),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                column_gap: px(8),
                padding: UiRect::horizontal(px(PAD)),
                border: rules(right, false),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(holds);
}

/// A header of a table: its labels in a row, ruled under it, or over it for a parts list,
/// whose header is at its bottom.
pub fn header(parent: &mut ChildSpawnerCommands, columns: &[(f32, &str)], at_bottom: bool) {
    let rule = if at_bottom {
        UiRect::top(px(1))
    } else {
        UiRect::bottom(px(1))
    };
    let height = if at_bottom { PART_ROW } else { CONTROL };
    parent
        .spawn((
            Node {
                height: px(height),
                flex_shrink: 0.0,
                border: rule,
                ..default()
            },
            BorderColor::all(theme::RULE),
        ))
        .with_children(|row| {
            let last = columns.len().saturating_sub(1);
            for (index, (width, word)) in columns.iter().enumerate() {
                let word = word.to_string();
                cell(row, *width, index < last, |cell| {
                    cell.spawn(key(word));
                });
            }
        });
}

/// A row of a list, which takes a click on its whole: its number in a cell of its own and its
/// name. `press` says what it does. The one that the player most likely wants is filled:
/// `filled` is then its marker.
pub fn list_row(
    parent: &mut ChildSpawnerCommands,
    number: usize,
    name: impl Into<String>,
    press: impl Bundle,
    filled: bool,
) {
    let (fill, edge, ink, number_ink) = if filled {
        (
            theme::ACCENT,
            theme::ACCENT,
            theme::TEXT_ON_ACCENT,
            theme::TEXT_ON_ACCENT,
        )
    } else {
        (
            theme::SURFACE_RAISED,
            theme::RULE,
            theme::TEXT,
            theme::TEXT_DIM,
        )
    };
    parent
        .spawn((
            Button,
            press,
            Node {
                height: px(ROW),
                flex_shrink: 0.0,
                border: UiRect::all(px(1)),
                ..default()
            },
            // The rows of a list share their rules: each one stands a pixel higher than the
            // one before it, which the layout doesn't see. A negative margin would do it, but
            // the engine then lays out the list shorter than its rows.
            UiTransform::from_translation(Val2::px(0.0, 1.0 - number as f32)),
            BackgroundColor(fill),
            BorderColor::all(edge),
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: px(ROW),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::right(px(1)),
                    ..default()
                },
                BorderColor::all(if filled { theme::VOID } else { theme::RULE_DIM }),
                children![theme::figure(
                    format!("{number:02}"),
                    theme::LABEL,
                    number_ink
                )],
            ));
            row.spawn((
                Node {
                    flex_grow: 1.0,
                    align_items: AlignItems::Center,
                    padding: UiRect::horizontal(px(18)),
                    ..default()
                },
                children![value(name, ink)],
            ));
        });
}

/// One of the choices of a row, 44 px high: the chosen one is edged in the accent and takes no
/// press, each of the others is a button.
#[expect(dead_code, reason = "the screens take it as they are drawn again")]
pub fn choice(
    parent: &mut ChildSpawnerCommands,
    word: impl Into<String>,
    width: f32,
    press: impl Bundle,
    chosen: bool,
) {
    let node = Node {
        width: px(width),
        height: px(CONTROL),
        flex_shrink: 0.0,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(px(1)),
        ..default()
    };
    if chosen {
        parent.spawn((
            node,
            BorderColor::all(theme::ACCENT),
            BackgroundColor(theme::ACCENT.with_alpha(0.12)),
            children![value(word, theme::ACCENT)],
        ));
    } else {
        parent.spawn((
            Button,
            press,
            node,
            BorderColor::all(theme::RULE),
            BackgroundColor(theme::SURFACE_RAISED),
            children![value(word, theme::TEXT)],
        ));
    }
}

/// A button of a form that isn't a row of a list, as wide as given and 56 px high, filled if
/// it is the one that the player most likely wants.
#[expect(dead_code, reason = "the screens take it as they are drawn again")]
pub fn key_button(
    parent: &mut ChildSpawnerCommands,
    word: impl Into<String>,
    width: Val,
    press: impl Bundle,
    filled: bool,
) {
    let (fill, edge, ink) = if filled {
        (theme::ACCENT, theme::ACCENT, theme::TEXT_ON_ACCENT)
    } else {
        (theme::SURFACE_RAISED, theme::RULE, theme::TEXT)
    };
    parent.spawn((
        Button,
        press,
        Node {
            width,
            height: px(ROW),
            flex_shrink: 0.0,
            padding: UiRect::horizontal(px(18)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            ..default()
        },
        BackgroundColor(fill),
        BorderColor::all(edge),
        children![value(word, ink)],
    ));
}

/// What a field of a title block holds, under its label.
pub enum Holds {
    /// The name of the game, as the legal owner of the sheet, with a line under it in the
    /// accent if one is given.
    Owner(Option<String>),
    /// A value, in a color.
    Value(String, Color),
    /// A title and the line that says what it is.
    Titled(String, String),
    /// The balloon of the player of a seat, who the sheet is drawn for.
    #[expect(dead_code, reason = "the screens take it as they are drawn again")]
    Balloon(u8),
}

/// A field of a title block: its width, its label, and what it holds.
pub struct Field {
    pub width: f32,
    pub label: &'static str,
    pub holds: Holds,
}

impl Field {
    pub fn new(width: f32, label: &'static str, holds: Holds) -> Self {
        Self {
            width,
            label,
            holds,
        }
    }
}

/// The width of a title block whose upper row has these fields, its edges included.
pub fn title_width(upper: &[Field]) -> f32 {
    upper.iter().map(|field| field.width).sum::<f32>() + 2.0 * FRAME_WIDTH
}

/// The height of a title block, its edges included.
pub fn title_height() -> f32 {
    TITLE_ROWS[0] + TITLE_ROWS[1] + 2.0 * FRAME_WIDTH
}

/// The title block of a screen, in the bottom right corner of the sheet, flush with the frame:
/// two rows of fields, each with its label over what it holds. The rows may part their fields
/// differently, as ISO 7200 lets them, but they are as wide.
pub fn title_block(parent: &mut ChildSpawnerCommands, upper: Vec<Field>, lower: Vec<Field>) {
    let width = title_width(&upper);
    parent
        .spawn(block(Node {
            position_type: PositionType::Absolute,
            right: px(SHEET_EDGE),
            bottom: px(SHEET_EDGE),
            width: px(width),
            ..default()
        }))
        .with_children(|block| {
            for (index, fields) in [upper, lower].into_iter().enumerate() {
                row(block, TITLE_ROWS[index], index == 0, |row| {
                    let last = fields.len().saturating_sub(1);
                    for (place, field) in fields.into_iter().enumerate() {
                        title_field(row, field, place < last);
                    }
                });
            }
        });
}

/// A field of a title block.
fn title_field(row: &mut ChildSpawnerCommands, field: Field, right: bool) {
    row.spawn((
        Node {
            width: px(field.width),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            padding: UiRect::axes(px(PAD), px(8)),
            border: rules(right, false),
            ..default()
        },
        BorderColor::all(theme::RULE_DIM),
    ))
    .with_children(|holds| {
        holds.spawn(key(field.label));
        match field.holds {
            Holds::Owner(line) => {
                holds.spawn(theme::label("Tanku", theme::ALERT, theme::TEXT));
                if let Some(line) = line {
                    holds.spawn(theme::label(line, theme::LABEL, theme::ACCENT));
                }
            }
            Holds::Value(said, color) => {
                holds.spawn(value(said, color));
            }
            Holds::Titled(title, line) => {
                holds.spawn(value(title, theme::TEXT));
                holds.spawn(theme::label(line, theme::LABEL, theme::LINE));
            }
            Holds::Balloon(seat) => {
                holds.spawn((
                    Node {
                        margin: UiRect::top(px(4)),
                        ..default()
                    },
                    children![balloons::balloon(seat, BALLOON, theme::LABEL)],
                ));
            }
        }
    });
}

/// A part of a tank, as a parts list tells it.
pub struct Part {
    pub part: String,
    pub stat: String,
    pub level: String,
    pub gives: String,
    pub points: String,
}

/// The parts list of a tank drawn as an assembly, over the title block of its screen and as
/// wide as it: its items numbered up from its header, which is at its bottom (ISO 7573). The
/// numbers are those of the balloons on the drawing.
pub fn parts_list(parent: &mut ChildSpawnerCommands, parts: &[Part], columns: [f32; 6]) {
    let width = columns.iter().sum::<f32>() + 2.0 * FRAME_WIDTH;
    parent
        .spawn(block(Node {
            position_type: PositionType::Absolute,
            right: px(SHEET_EDGE),
            bottom: px(SHEET_EDGE + title_height() - FRAME_WIDTH),
            width: px(width),
            ..default()
        }))
        .with_children(|list| {
            for (index, part) in parts.iter().enumerate().rev() {
                let said = [
                    (index + 1).to_string(),
                    part.part.clone(),
                    part.stat.clone(),
                    part.level.clone(),
                    part.gives.clone(),
                    part.points.clone(),
                ];
                row(list, PART_ROW, index > 0, |row| {
                    for (column, word) in said.into_iter().enumerate() {
                        cell(row, columns[column], column < 5, |cell| {
                            cell.spawn(theme::label(word, theme::LABEL, theme::TEXT));
                        });
                    }
                });
            }
            let names = ["Item", "Part", "Stat", "Level", "Gives", "Points"];
            let header: Vec<(f32, &str)> = columns.iter().copied().zip(names).collect();
            header_row(list, &header);
        });
}

/// The header of a parts list, at its bottom, over a rule.
fn header_row(parent: &mut ChildSpawnerCommands, columns: &[(f32, &str)]) {
    header(parent, columns, true);
}

/// The notes of a screen: what it has to say that is no control, at the bottom of the sheet
/// between these distances from its left and from its right, left of the title block, as the
/// notes of a drawing stand near its title block.
pub fn notes(left: f32, right: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: px(left),
        right: px(right),
        bottom: px(SHEET_EDGE + 12.0),
        flex_direction: FlexDirection::Column,
        row_gap: px(6),
        ..default()
    }
}

/// The width of a block whose columns are these, its edges included.
pub fn title_width_of(columns: &[f32]) -> f32 {
    columns.iter().sum::<f32>() + 2.0 * FRAME_WIDTH
}
