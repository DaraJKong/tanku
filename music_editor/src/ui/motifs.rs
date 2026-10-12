//! The tab of the motifs: the few bars of melody that every tune of the piece is made of.
//!
//! A motif is drawn on a grid of nine degrees. What is written by hand is solid, and the
//! lines that the rules derive from it, its skeleton, its ornaments and its answer, are in
//! dashes under it. Its turn, which is written too, is solid.

use bevy::prelude::*;
use tanku_audio::{
    notation::{Form, Line, Motif, Piece, STEPS},
    score::{FIVE, answer, ornaments, skeleton},
};

use super::{
    Over, Playhead, checks_panel,
    widgets::{
        Dashed, Hatched, Kind, Look, button, choice, grow, head, note, panel, pressed, row,
        stepper, told,
    },
};
use crate::{
    checks::Checks,
    doc::Doc,
    lines, names,
    state::{Press, Tab, Tool, View, stating},
    theme,
};

/// The left of the grid, where the degrees are named, and the size of its cells.
const GUTTER: f32 = 94.0;
const WIDTH: f32 = 928.0;
const ROW: f32 = 36.0;
const TOP: f32 = 26.0;

fn bars_of(motif: &Motif) -> String {
    let count = motif.bars().len();
    let bars = if count == 1 { "bar" } else { "bars" };
    let turn = if motif.turn.is_some() {
        " and a turn"
    } else {
        ""
    };
    format!("{count} {bars}{turn}")
}

pub fn list(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View) {
    let piece = doc.piece();
    let shown = view.shown_motif(piece);
    head(parent, "Motifs", |_| {});
    for motif in piece.motifs() {
        let picked = shown == Some(motif.letter);
        let kind = if picked { Kind::Picked } else { Kind::Row };
        parent
            .spawn((
                pressed(Press::Motif(motif.letter), kind),
                Node {
                    height: px(48),
                    flex_shrink: 0.0,
                    padding: UiRect::horizontal(px(12)),
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
            ))
            .with_children(|row| {
                let (fill, edge, ink) = if picked {
                    (theme::ACCENT, theme::ACCENT, theme::TEXT_ON_ACCENT)
                } else {
                    (Color::NONE, theme::RULE, theme::TEXT)
                };
                row.spawn((
                    Node {
                        width: px(24),
                        height: px(24),
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    BackgroundColor(fill),
                    BorderColor::all(edge),
                    children![theme::strong((motif.letter as char).to_string(), 15.0, ink)],
                ));
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..grow()
                })
                .with_children(|words| {
                    words.spawn(theme::text(bars_of(motif), 16.0, theme::TEXT));
                    let stated = stating(piece, motif.letter).join(", ");
                    let stated = if stated.is_empty() {
                        "in no section".to_string()
                    } else {
                        stated
                    };
                    words.spawn(theme::text(stated, 13.0, theme::TEXT_SOFT));
                });
            });
    }
    parent
        .spawn(Node {
            padding: UiRect::axes(px(12), px(10)),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|new| {
            button(
                new,
                "New motif",
                Press::NewMotif,
                Kind::Hollow,
                (percent(100), 40.0),
            );
        });
    // What the two ways of drawing stand for.
    parent
        .spawn((
            Node {
                margin: UiRect::top(Val::Auto),
                padding: UiRect::axes(px(12), px(10)),
                flex_direction: FlexDirection::Column,
                row_gap: px(8),
                border: UiRect::top(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(|legend| {
            let sample = Node {
                width: px(32),
                height: px(8),
                flex_shrink: 0.0,
                ..default()
            };
            let pair = Node {
                align_items: AlignItems::Center,
                column_gap: px(10),
                ..default()
            };
            legend.spawn(pair.clone()).with_children(|pair| {
                pair.spawn((sample.clone(), BackgroundColor(theme::LINE)));
                pair.spawn(theme::text("Written by hand", 14.0, theme::TEXT_SOFT));
            });
            legend.spawn(pair).with_children(|pair| {
                pair.spawn(sample).with_children(|sample| {
                    sample.spawn(Dashed {
                        color: theme::LINE,
                        left: 0.0,
                        top: 0.0,
                        size: None,
                    });
                });
                pair.spawn(theme::text("Derived by rule", 14.0, theme::TEXT_SOFT));
            });
        });
}

pub fn center(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View) {
    let piece = doc.piece();
    let Some(motif) = view
        .shown_motif(piece)
        .and_then(|letter| piece.motif(letter))
    else {
        note(parent, "The piece has no motif yet.");
        return;
    };
    let letter = motif.letter as char;
    let turn = view.turn.then_some(motif.turn).flatten();
    // The bars that the grid shows: those of the motif, or its turn.
    let shown: Vec<Line> = match turn {
        Some(turn) => vec![turn],
        None => motif.bars().to_vec(),
    };
    let steps = STEPS * shown.len();
    let step = (WIDTH / steps as f32).floor();

    parent
        .spawn((
            Node {
                height: px(44),
                flex_shrink: 0.0,
                padding: UiRect::horizontal(px(14)),
                align_items: AlignItems::Center,
                column_gap: px(14),
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE),
        ))
        .with_children(|header| {
            let name = if turn.is_some() {
                format!("Turn of motif {letter}")
            } else {
                format!("Motif {letter}")
            };
            header.spawn(theme::title(name, 17.0, theme::TEXT));
            let what = if turn.is_some() {
                "The bar that ends a section in place of the last bar of the motif.".to_string()
            } else {
                let stated = stating(piece, motif.letter);
                match stated.len() {
                    0 => "No section states it yet.".to_string(),
                    _ => format!(
                        "{}, stated by the {}.",
                        bars_of(&motif),
                        stated.join(" and the ")
                    ),
                }
            };
            header.spawn((
                theme::text(capitalized(&what), 16.0, theme::TEXT_SOFT),
                grow(),
            ));
            if turn.is_none() {
                header.spawn(theme::label("Bars", 11.0, theme::TEXT_DIM));
                let count = motif.bars().len().to_string();
                stepper(
                    header,
                    count,
                    Press::MotifBars(-1),
                    Press::MotifBars(1),
                    24.0,
                );
            }
            let tools = [
                ("Draw", Press::Tool(Tool::Draw), view.tool == Tool::Draw),
                (
                    "Select",
                    Press::Tool(Tool::Select),
                    view.tool == Tool::Select,
                ),
                ("Erase", Press::Tool(Tool::Erase), view.tool == Tool::Erase),
            ];
            choice(header, tools, false, 32.0);
        });

    grid(parent, piece, view, &shown, step);
    as_written(parent, &shown, step, turn.is_some(), letter);
    derived(parent, piece, &motif, view, step);
    parent
        .spawn(Node {
            padding: UiRect::horizontal(px(14)),
            align_items: AlignItems::Center,
            column_gap: px(6),
            ..grow()
        })
        .with_children(|foot| {
            let words = "A digit starts a note, a dot holds it and a dash is silence. Derived \
                         lines follow every change and are not saved.";
            foot.spawn((theme::text(words, 14.0, theme::TEXT_SOFT), grow()));
            // The turn is written like a bar: the grid shows it when asked.
            match (motif.turn.is_some(), view.turn) {
                (false, _) => {
                    button(
                        foot,
                        "Give it a turn",
                        Press::HasTurn(true),
                        Kind::Hollow,
                        (Val::Auto, 32.0),
                    );
                }
                (true, shown) => {
                    let (name, press) = if shown {
                        ("Back to the bars", Press::Turn(false))
                    } else {
                        ("Draw the turn", Press::Turn(true))
                    };
                    button(foot, name, press, Kind::Plain, (Val::Auto, 32.0));
                    button(
                        foot,
                        "Remove the turn",
                        Press::HasTurn(false),
                        Kind::Plain,
                        (Val::Auto, 32.0),
                    );
                }
            }
        });
}

fn capitalized(words: &str) -> String {
    super::capital(words)
}

/// The color of the line at the left of a step: that of a bar, of a beat, or of a step.
fn rule(step: usize) -> Color {
    if step.is_multiple_of(STEPS) && step > 0 {
        theme::LINE.with_alpha(0.7)
    } else if step.is_multiple_of(4) {
        theme::GRID.with_alpha(0.3)
    } else {
        theme::GRID.with_alpha(0.14)
    }
}

/// The grid of nine degrees, with the notes of the bars on it.
fn grid(parent: &mut ChildSpawnerCommands, piece: &Piece, view: &View, bars: &[Line], step: f32) {
    let steps = STEPS * bars.len();
    parent
        .spawn((
            Node {
                height: px(TOP + 9.0 * ROW),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(theme::FLOOR),
        ))
        .with_children(|grid| {
            // The beats, by bar.
            grid.spawn((
                Node {
                    height: px(TOP),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BackgroundColor(theme::SURFACE_RAISED),
                BorderColor::all(theme::RULE_DIM),
            ))
            .with_children(|top| {
                top.spawn((
                    Node {
                        width: px(GUTTER),
                        padding: UiRect::left(px(12)),
                        ..default()
                    },
                    children![theme::label("Degree", 11.0, theme::TEXT_DIM)],
                ));
                for beat in 0..steps / 4 {
                    let name = format!("{}.{}", beat / 4 + 1, beat % 4 + 1);
                    top.spawn((
                        Node {
                            width: px(4.0 * step),
                            padding: UiRect::left(px(6)),
                            ..default()
                        },
                        children![theme::figure(name, 12.0, theme::TEXT_SOFT)],
                    ));
                }
            });
            for degree in (1..=9u8).rev() {
                let used = FIVE.contains(&((degree - 1) % 7 + 1));
                let ink = if used { theme::TEXT } else { theme::TEXT_DIM };
                grid.spawn(Node {
                    height: px(ROW),
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Node {
                            width: px(GUTTER),
                            flex_shrink: 0.0,
                            padding: UiRect::left(px(14)),
                            align_items: AlignItems::Center,
                            border: UiRect {
                                right: px(1),
                                bottom: px(1),
                                ..default()
                            },
                            ..default()
                        },
                        BackgroundColor(theme::SURFACE_RAISED),
                        BorderColor {
                            right: theme::RULE,
                            bottom: theme::RULE_DIM,
                            ..BorderColor::DEFAULT
                        },
                    ))
                    .with_children(|gutter| {
                        gutter.spawn((
                            Node {
                                width: px(30),
                                ..default()
                            },
                            theme::figure(degree.to_string(), 14.0, ink),
                        ));
                        let name = names::note(piece, piece.key.home, degree);
                        gutter.spawn(theme::strong(name, 15.0, ink));
                    });
                    // The cells, over strokes where the motifs don't go.
                    row.spawn(Node {
                        width: px(step * steps as f32),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|cells| {
                        if !used {
                            cells.spawn(Hatched);
                        }
                        for at in 0..steps {
                            let press = Press::Cell(at / STEPS, at % STEPS, degree);
                            cells.spawn((
                                Button,
                                press,
                                Look::cell(),
                                BackgroundColor(Color::NONE),
                                Node {
                                    width: px(step),
                                    height: px(ROW),
                                    border: UiRect {
                                        left: px(1),
                                        top: px(1),
                                        ..default()
                                    },
                                    ..default()
                                },
                                BorderColor {
                                    left: rule(at),
                                    top: theme::GRID.with_alpha(0.14),
                                    ..BorderColor::DEFAULT
                                },
                            ));
                        }
                    });
                });
            }
            // The notes, over the cells: a click goes through them to the cell under it.
            for (bar, line) in bars.iter().enumerate() {
                for (start, cell) in line.iter().enumerate() {
                    if cell.degree == 0 {
                        continue;
                    }
                    let picked = view.note == Some((bar, start));
                    let (fill, edge) = if picked {
                        (theme::ACCENT, Color::WHITE)
                    } else {
                        (theme::LINE, theme::LINE)
                    };
                    let left = GUTTER + (bar * STEPS + start) as f32 * step;
                    grid.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(left + 2.0),
                            top: px(TOP + (9 - cell.degree) as f32 * ROW + 6.0),
                            width: px(cell.steps as f32 * step - 3.0),
                            height: px(ROW - 12.0),
                            padding: UiRect::left(px(6)),
                            align_items: AlignItems::Center,
                            border: UiRect::all(px(1)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(fill),
                        BorderColor::all(edge),
                        children![theme::figure(
                            cell.degree.to_string(),
                            13.0,
                            theme::TEXT_ON_ACCENT
                        )],
                    ));
                }
            }
            // The line that goes over the grid while the motif plays alone.
            if !view.turn {
                grid.spawn((
                    Playhead {
                        origin: GUTTER,
                        step,
                        over: Over::Motif(bars.len()),
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(GUTTER),
                        top: px(0),
                        width: px(1),
                        height: percent(100),
                        ..default()
                    },
                    BackgroundColor(Color::WHITE),
                    Visibility::Hidden,
                ));
            }
        });
}

/// The bars as the file writes them, each symbol under its step.
fn as_written(
    parent: &mut ChildSpawnerCommands,
    bars: &[Line],
    step: f32,
    turn: bool,
    letter: char,
) {
    parent
        .spawn((
            Node {
                height: px(52),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                border: UiRect::vertical(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE),
        ))
        .with_children(|row| {
            row.spawn(Node {
                width: px(GUTTER),
                flex_shrink: 0.0,
                padding: UiRect::left(px(12)),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                ..default()
            })
            .with_children(|name| {
                name.spawn(theme::label("As written", 11.0, theme::TEXT_DIM));
                let head = if turn {
                    format!("turn {letter}")
                } else {
                    format!("motif {letter}")
                };
                name.spawn(theme::strong(head, 13.0, theme::TEXT_SOFT));
            });
            let size = (step - 4.0).clamp(11.0, 22.0);
            for (bar, line) in bars.iter().enumerate() {
                for (at, symbol) in lines::symbols(line).into_iter().enumerate() {
                    let border = if at == 0 { px(1) } else { px(0) };
                    row.spawn((
                        Node {
                            width: px(step),
                            height: percent(100),
                            flex_shrink: 0.0,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::left(border),
                            ..default()
                        },
                        BorderColor::all(if bar == 0 {
                            theme::RULE
                        } else {
                            theme::RULE_DIM
                        }),
                        children![theme::figure(
                            (symbol as char).to_string(),
                            size,
                            theme::TEXT
                        )],
                    ));
                }
            }
        });
}

/// The lines that the rules derive from the motif, in dashes, and its turn, which is
/// written.
fn derived(
    parent: &mut ChildSpawnerCommands,
    piece: &Piece,
    motif: &Motif,
    view: &View,
    step: f32,
) {
    const HEIGHT: f32 = 48.0;
    let bars = motif.bars();
    // While the grid shows the turn, a step is as wide as for a motif of one bar: the lines
    // under it keep the width of the motif.
    let step = if view.turn {
        (WIDTH / (STEPS * bars.len()) as f32).floor()
    } else {
        step
    };
    // Each line that a rule derives, with the rule and with where the arrangement plays it.
    let row = |name: &'static str, form: Form, derive: fn(Line) -> Line| {
        let lines: Vec<Line> = bars.iter().map(|line| derive(*line)).collect();
        (name, names::form_played(piece, form), lines)
    };
    let rows = [
        row("Skeleton", Form::Skeleton, skeleton),
        row("Ornaments", Form::Ornate, ornaments),
        row("Answer", Form::Answer, answer),
    ];
    // A thin bar for each note, as high in its row as its degree.
    let mark = |start: usize, cell: &tanku_audio::notation::Cell| {
        let left = GUTTER + start as f32 * step + 1.0;
        let top = 36.0 - (cell.degree.min(9) - 1) as f32 * 4.0;
        (left, top, cell.steps as f32 * step - 2.0)
    };
    parent
        .spawn((
            Node {
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BackgroundColor(theme::SURFACE_RAISED),
            BorderColor::all(theme::RULE),
        ))
        .with_children(|area| {
            for (name, when, lines) in rows {
                area.spawn((
                    Node {
                        height: px(HEIGHT),
                        border: UiRect::bottom(px(1)),
                        ..default()
                    },
                    BorderColor::all(theme::RULE_DIM),
                ))
                .with_children(|row| {
                    labels(row, name, &when);
                    for (bar, line) in lines.iter().enumerate() {
                        for (start, cell) in
                            line.iter().enumerate().filter(|(_, cell)| cell.degree != 0)
                        {
                            let (left, top, width) = mark(bar * STEPS + start, cell);
                            row.spawn(Dashed {
                                color: theme::LINE,
                                left,
                                top,
                                size: Some(Vec2::new(width, 6.0)),
                            });
                        }
                    }
                });
            }
            area.spawn(Node {
                height: px(HEIGHT),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                labels(row, "Turn", "ends a section");
                match &motif.turn {
                    Some(turn) => {
                        // Where the last bar of the motif is: the turn takes its place.
                        let from = (bars.len() - 1) * STEPS;
                        for (start, cell) in
                            turn.iter().enumerate().filter(|(_, cell)| cell.degree != 0)
                        {
                            let (left, top, width) = mark(from + start, cell);
                            row.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(left),
                                    top: px(top),
                                    width: px(width),
                                    height: px(6),
                                    ..default()
                                },
                                BackgroundColor(theme::LINE),
                            ));
                        }
                    }
                    None => {
                        row.spawn((
                            Node {
                                padding: UiRect::left(px(12)),
                                ..default()
                            },
                            children![theme::text(
                                "This motif has no turn.",
                                14.0,
                                theme::TEXT_DIM
                            )],
                        ));
                    }
                }
            });
        });
}

/// The name of a row of the lines under the grid, and when it is heard.
fn labels(parent: &mut ChildSpawnerCommands, name: &str, when: &str) {
    parent
        .spawn((
            Node {
                width: px(GUTTER),
                flex_shrink: 0.0,
                padding: UiRect::left(px(12)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                row_gap: px(3),
                border: UiRect::right(px(1)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(theme::RULE),
        ))
        .with_children(|labels| {
            labels.spawn(theme::title(name, 12.0, theme::TEXT));
            labels.spawn(theme::text(when, 12.0, theme::TEXT_DIM));
        });
}

pub fn side(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View, checks: &Checks) {
    let piece = doc.piece();
    let motif = view
        .shown_motif(piece)
        .and_then(|letter| piece.motif(letter));
    // The note that is selected, if the motif still has it.
    let picked = motif.and_then(|motif| {
        let (bar, start) = view.note?;
        let line = if view.turn {
            motif.turn?
        } else {
            *motif.bars().get(bar)?
        };
        (line[start].degree != 0).then_some((bar, start, line[start]))
    });
    parent.spawn(widgets_panel()).with_children(|panel| {
        head(panel, "Selected note", |_| {});
        match picked {
            Some((bar, start, cell)) => {
                widgets_row(panel, "Degree", |row| {
                    stepper(
                        row,
                        cell.degree.to_string(),
                        Press::NoteDegree(-1),
                        Press::NoteDegree(1),
                        34.0,
                    );
                });
                widgets_row(panel, "Length in steps", |row| {
                    stepper(
                        row,
                        cell.steps.to_string(),
                        Press::NoteLength(-1),
                        Press::NoteLength(1),
                        34.0,
                    );
                });
                told(
                    panel,
                    "Starts",
                    format!("bar {}, step {}", bar + 1, start + 1),
                    40.0,
                );
                // The note that it is in each mode of the piece.
                panel
                    .spawn(Node {
                        padding: UiRect::axes(px(12), px(10)),
                        column_gap: px(8),
                        ..default()
                    })
                    .with_children(|modes| {
                        for mode in [piece.key.home, piece.key.arena] {
                            modes.spawn((
                                Node {
                                    padding: UiRect::axes(px(10), px(6)),
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(2),
                                    border: UiRect::all(px(1)),
                                    ..grow()
                                },
                                BorderColor::all(theme::RULE_DIM),
                                children![
                                    theme::label(mode.name(), 12.0, theme::TEXT_DIM),
                                    theme::strong(
                                        names::note(piece, mode, cell.degree),
                                        22.0,
                                        theme::TEXT
                                    ),
                                ],
                            ));
                        }
                    });
            }
            None => note(
                panel,
                "Click a note of the grid to select it, or an empty step to draw one.",
            ),
        }
    });

    parent.spawn(widgets_panel()).with_children(|panel| {
        head(panel, "Played by", |_| {});
        let stated = motif
            .map(|motif| stating(piece, motif.letter))
            .unwrap_or_default();
        for name in &stated {
            let passage = piece.passage(name);
            let first = motif.map(|motif| motif.bars()[0]);
            let times = passage.map_or(0, |passage| {
                passage
                    .melody
                    .iter()
                    .filter(|bar| Some(**bar) == first)
                    .count()
            });
            let times = if times == 1 {
                "once".to_string()
            } else {
                format!("{times} times")
            };
            told(panel, &super::capital(name), times, 40.0);
        }
        if stated.is_empty() {
            note(panel, "No section states this motif.");
        }
        panel
            .spawn(Node {
                padding: UiRect::axes(px(12), px(8)),
                ..default()
            })
            .with_children(|remove| {
                button(
                    remove,
                    "Remove this motif",
                    Press::RemoveMotif,
                    Kind::Plain,
                    (percent(100), 32.0),
                );
            });
    });

    checks_panel(
        parent,
        checks,
        Tab::Motifs,
        Node {
            min_height: px(0),
            ..grow()
        },
    );
}

/// A panel of the right column that is as high as what it holds.
fn widgets_panel() -> impl Bundle {
    panel(Node {
        flex_shrink: 0.0,
        ..default()
    })
}

fn widgets_row(
    parent: &mut ChildSpawnerCommands,
    name: &str,
    right: impl FnOnce(&mut ChildSpawnerCommands),
) {
    row(parent, name, 46.0, right);
}
