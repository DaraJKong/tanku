//! The tab of the patterns: the bars of sixteen steps that the drums, the bass and the pluck
//! repeat under the melody.
//!
//! Every pattern of the piece is a row of one grid, so that they can be read against each
//! other. A click writes the symbol of the brush into a step, and the same click again
//! clears it.

use bevy::prelude::*;
use tanku_audio::{
    Place, checks,
    notation::{Instrument, Piece, Rhythm, STEPS},
};

use super::{
    Over, Playhead, checks_panel,
    widgets::{
        Kind, button, choice, field, grow, head, key, list_row, note, panel, pressed, row, stepper,
        told,
    },
};
use crate::{
    checks::Checks,
    doc::Doc,
    lines, names,
    state::{self, Field, Press, Tab, View},
    theme,
};

/// The left of the grid, where a pattern is named, the width of a step, and that of the
/// column of the levels.
const GUTTER: f32 = 230.0;
const STEP: f32 = 44.0;
const LEVEL: f32 = 88.0;
/// The height that the rows have together at most, and that of one row.
const ROWS: f32 = 480.0;
const ROW: f32 = 40.0;

/// The instruments that have patterns: the lead plays the motifs.
const PLAYERS: [Instrument; 5] = [
    Instrument::Kick,
    Instrument::Hat,
    Instrument::Snare,
    Instrument::Bass,
    Instrument::Pluck,
];

pub fn list(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View) {
    let piece = doc.piece();
    head(parent, "Patterns of", |_| {});
    for instrument in PLAYERS {
        let rhythms = piece.rhythms().iter();
        let count = rhythms
            .filter(|rhythm| rhythm.instrument == instrument)
            .count();
        let picked = view.pattern_of == instrument;
        let press = Press::PatternOf(instrument);
        list_row(
            parent,
            names::instrument(instrument),
            &count.to_string(),
            press,
            picked,
            38.0,
        );
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
                "New pattern",
                Press::NewPattern,
                Kind::Hollow,
                (percent(100), 40.0),
            );
        });
    parent
        .spawn((
            Node {
                margin: UiRect::top(Val::Auto),
                border: UiRect::top(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(|foot| note(foot, "The lead plays the motifs."));
}

/// The level of a pattern as its line ends: that of a hit, that of a soft hit if it has
/// some, and its echo.
fn levels(rhythm: &Rhythm) -> String {
    let mut words = rhythm.level.to_string();
    if rhythm.grid.contains(&b'o') {
        words.push_str(&format!(" {}", rhythm.soft));
    }
    if rhythm.echo {
        words.push_str(" echo");
    }
    words
}

pub fn center(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View) {
    let piece = doc.piece();
    let shown = view.shown_pattern(piece);
    let rhythm = shown
        .as_ref()
        .and_then(|(instrument, name)| piece.rhythm(*instrument, name));

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
            header.spawn(theme::title("Patterns", 17.0, theme::TEXT));
            let words = "One bar each: 16 steps in four beats.";
            header.spawn(theme::text(words, 16.0, theme::TEXT_SOFT));
        });

    // The beats.
    parent
        .spawn((
            Node {
                height: px(30),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BackgroundColor(theme::SURFACE_RAISED),
            BorderColor::all(theme::RULE),
        ))
        .with_children(|beats| {
            beats.spawn(Node {
                width: px(GUTTER),
                flex_shrink: 0.0,
                ..default()
            });
            for beat in 1..=4 {
                beats.spawn((
                    Node {
                        width: px(4.0 * STEP),
                        flex_shrink: 0.0,
                        padding: UiRect::left(px(6)),
                        ..default()
                    },
                    children![theme::figure(beat.to_string(), 12.0, theme::TEXT_SOFT)],
                ));
            }
            beats.spawn((
                Node {
                    width: px(LEVEL),
                    flex_shrink: 0.0,
                    padding: UiRect::right(px(14)),
                    justify_content: JustifyContent::FlexEnd,
                    ..default()
                },
                children![theme::label("Level", 11.0, theme::TEXT_DIM)],
            ));
        });

    let count = piece.rhythms().len();
    let height = (ROWS / count.max(1) as f32).floor().clamp(28.0, ROW);
    parent
        .spawn(Node {
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|grid| {
            // The floor of the grid, under the steps of every row.
            grid.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(GUTTER),
                    top: px(0),
                    bottom: px(0),
                    width: px(STEPS as f32 * STEP),
                    ..default()
                },
                BackgroundColor(theme::FLOOR),
            ));
            let mut last = None;
            for rhythm in piece.rhythms() {
                let first = last.is_some_and(|last| last != rhythm.instrument);
                let name = rhythm.name.as_str();
                let picked = shown.as_ref().is_some_and(|(instrument, shown)| {
                    *instrument == rhythm.instrument && shown == name
                });
                line(
                    grid,
                    piece,
                    rhythm,
                    (first, last != Some(rhythm.instrument)),
                    picked,
                    height,
                );
                last = Some(rhythm.instrument);
            }
            if count == 0 {
                note(grid, "The piece has no pattern yet.");
            }
            // The line that goes over the grid while the music plays.
            grid.spawn((
                Playhead {
                    origin: GUTTER,
                    step: STEP,
                    over: Over::Bar,
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
        });

    let Some(rhythm) = rhythm else {
        return;
    };
    as_written(parent, &rhythm);
    heard_over(parent, doc, view, &rhythm);
    let bar = lines::bar(&rhythm.grid);
    let words = if rhythm.instrument.is_drum() {
        format!("A drum is hit or it is not. In the file, a bar is four words of four steps: {bar}")
    } else {
        format!(
            "A note of the {} lasts until the next one, or until a dash. In the file, a bar is \
             four words of four steps: {bar}",
            rhythm.instrument.name()
        )
    };
    parent
        .spawn(Node {
            padding: UiRect::horizontal(px(14)),
            align_items: AlignItems::Center,
            min_height: px(0),
            ..grow()
        })
        .with_children(|foot| {
            foot.spawn(theme::text(words, 14.0, theme::TEXT_SOFT));
        });
}

/// One pattern: its instrument if it is the first of it, its name, where it is played, its
/// sixteen steps and its level.
fn line(
    parent: &mut ChildSpawnerCommands,
    piece: &Piece,
    rhythm: &Rhythm,
    (ruled, named): (bool, bool),
    picked: bool,
    height: f32,
) {
    let name = rhythm.name.as_str();
    let instrument = rhythm.instrument;
    let (fill, edge) = if picked {
        (theme::ACCENT.with_alpha(0.16), theme::ACCENT)
    } else {
        (Color::NONE, Color::NONE)
    };
    parent
        .spawn((
            Node {
                height: px(height),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                border: UiRect {
                    top: px(if ruled { 1.0 } else { 0.0 }),
                    bottom: px(1),
                    ..default()
                },
                ..default()
            },
            BorderColor {
                top: theme::RULE,
                bottom: theme::RULE_DIM,
                ..BorderColor::DEFAULT
            },
        ))
        .with_children(|row| {
            // The row that is selected is lit and edged with the accent, over its floor.
            row.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    right: px(0),
                    top: px(0),
                    bottom: px(0),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(fill),
                BorderColor::all(edge),
            ));
            row.spawn((
                pressed(Press::Pattern(instrument, name.to_string()), Kind::Row),
                Node {
                    width: px(GUTTER),
                    height: percent(100),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
            ))
            .with_children(|gutter| {
                let label = if named {
                    names::instrument(instrument)
                } else {
                    ""
                };
                gutter.spawn((
                    Node {
                        width: px(84),
                        flex_shrink: 0.0,
                        padding: UiRect::left(px(14)),
                        ..default()
                    },
                    children![theme::label(label, 12.0, theme::TEXT_DIM)],
                ));
                gutter
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|words| {
                        words.spawn(theme::text(name, 16.0, theme::TEXT));
                        if height >= 36.0 {
                            let played = names::played_in(piece, instrument, name);
                            words.spawn(theme::text(played, 12.0, theme::TEXT_DIM));
                        }
                    });
            });
            let pitched = !instrument.is_drum();
            // Whether a note sounds on the step before: a dot then holds it.
            let mut held = false;
            for (step, &symbol) in rhythm.grid.iter().enumerate() {
                let holds = symbol == b'.' && pitched && held;
                held = match symbol {
                    b'.' => held,
                    b'-' => false,
                    _ => true,
                };
                let rule = if step % 4 == 0 { 0.3 } else { 0.12 };
                row.spawn((
                    Button,
                    Press::Step(instrument, name.to_string(), step),
                    super::widgets::Look::cell(),
                    BackgroundColor(Color::NONE),
                    BorderColor::all(theme::GRID.with_alpha(rule)),
                    Node {
                        width: px(STEP),
                        height: percent(100),
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::left(px(1)),
                        ..default()
                    },
                ))
                .with_children(|cell| mark(cell, symbol, holds, height));
            }
            row.spawn((
                Node {
                    width: px(LEVEL),
                    flex_shrink: 0.0,
                    padding: UiRect::right(px(14)),
                    justify_content: JustifyContent::FlexEnd,
                    border: UiRect::left(px(1)),
                    ..default()
                },
                BorderColor::all(theme::GRID.with_alpha(0.3)),
                children![theme::written(levels(rhythm), 13.0, theme::TEXT_SOFT)],
            ));
        });
}

/// What a step of a pattern is drawn as: a square for a hit, a small one for a soft hit, a
/// letter or a digit on a square for a note, and a faint bar while a note is held.
fn mark(cell: &mut ChildSpawnerCommands, symbol: u8, holds: bool, height: f32) {
    let size = (height - 14.0).min(26.0);
    let square = |size: f32| Node {
        width: px(size),
        height: px(size),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };
    match symbol {
        b'x' => {
            cell.spawn((square(size), BackgroundColor(theme::LINE)));
        }
        b'o' => {
            cell.spawn((square(12.0), BackgroundColor(theme::LINE)));
        }
        b'.' if holds => {
            cell.spawn((
                Node {
                    width: percent(100),
                    height: px(6),
                    ..default()
                },
                BackgroundColor(theme::LINE.with_alpha(0.4)),
            ));
        }
        b'.' | b'-' => {}
        letter => {
            let letter = (letter as char).to_string();
            cell.spawn((
                square(size),
                BackgroundColor(theme::LINE),
                children![theme::strong(letter, 17.0, theme::TEXT_ON_ACCENT)],
            ));
        }
    }
}

/// A row under the grid: a name at its left, then something under each step.
fn under(
    parent: &mut ChildSpawnerCommands,
    edge: Color,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            Node {
                height: px(44),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BorderColor::all(edge),
        ))
        .with_children(content);
}

/// The pattern that is selected as the file writes it, each symbol under its step.
fn as_written(parent: &mut ChildSpawnerCommands, rhythm: &Rhythm) {
    parent.spawn((
        Node {
            height: px(1),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(theme::RULE),
    ));
    under(parent, theme::RULE_DIM, |row| {
        row.spawn(Node {
            width: px(GUTTER),
            flex_shrink: 0.0,
            padding: UiRect::axes(px(14), px(0)),
            align_items: AlignItems::Center,
            column_gap: px(8),
            overflow: Overflow::clip(),
            ..default()
        })
        .with_children(|name| {
            name.spawn((theme::label("As written", 12.0, theme::TEXT_DIM), grow()));
            let head = format!("pat {} {}", rhythm.instrument.name(), rhythm.name.as_str());
            name.spawn(theme::written(head, 13.0, theme::TEXT_SOFT));
        });
        for symbol in rhythm.grid {
            row.spawn((
                Node {
                    width: px(STEP),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                children![theme::strong(
                    (symbol as char).to_string(),
                    22.0,
                    theme::TEXT
                )],
            ));
        }
        row.spawn((
            Node {
                width: px(LEVEL),
                flex_shrink: 0.0,
                padding: UiRect::right(px(14)),
                justify_content: JustifyContent::FlexEnd,
                ..default()
            },
            children![theme::written(levels(rhythm), 15.0, theme::TEXT)],
        ));
    });
}

/// The notes that the pattern gives over a chord of the section that plays it.
fn heard_over(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View, rhythm: &Rhythm) {
    let piece = doc.piece();
    let name = rhythm.name.as_str();
    // The section that plays the pattern, or the one that is shown if none does.
    let section =
        state::playing(piece, rhythm.instrument, name).or_else(|| view.shown_section(piece));
    let passage = section.and_then(|section| piece.passage(&section));
    let arena = view.place == Place::Arena && view.fight;
    let mode = if arena {
        piece.key.arena
    } else {
        piece.key.home
    };
    under(parent, theme::RULE, |row| {
        let Some(passage) = passage.filter(|_| !rhythm.instrument.is_drum()) else {
            row.spawn((
                Node {
                    padding: UiRect::left(px(14)),
                    ..default()
                },
                children![theme::text(
                    "A drum has no pitch: it sounds the same over every chord.",
                    14.0,
                    theme::TEXT_DIM
                )],
            ));
            return;
        };
        let over = passage.chords[view.over.min(7)];
        row.spawn(Node {
            width: px(GUTTER),
            flex_shrink: 0.0,
            padding: UiRect::left(px(14)),
            align_items: AlignItems::Center,
            column_gap: px(8),
            ..default()
        })
        .with_children(|name| {
            name.spawn(theme::label("Heard over", 12.0, theme::TEXT_DIM));
            // The chords of the section, each once.
            let mut chords: Vec<(u8, usize)> = Vec::new();
            for (bar, chord) in passage.chords.into_iter().enumerate() {
                if chords.iter().all(|(other, _)| *other != chord) {
                    chords.push((chord, bar));
                }
            }
            name.spawn(Node::default()).with_children(|buttons| {
                for (chord, bar) in chords {
                    let kind = Kind::lit(chord == over);
                    let words = theme::strong(names::chord(piece, mode, chord), 14.0, kind.text());
                    key(buttons, words, Press::Over(bar), kind, (Val::Auto, 28.0));
                }
            });
        });
        for pitch in names::heard(doc.score(), rhythm, mode, over) {
            let words = pitch.map_or(String::new(), checks::note);
            row.spawn((
                Node {
                    width: px(STEP),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                children![theme::strong(words, 15.0, theme::TEXT_SOFT)],
            ));
        }
    });
}

/// The symbols that a click writes, with what each is: those of a drum, then those that only
/// an instrument with a pitch has.
const BRUSHES: [(u8, &str); 8] = [
    (b'x', "Hit"),
    (b'o', "Soft hit"),
    (b'R', "Root"),
    (b'O', "Root, higher"),
    (b'V', "Fifth"),
    (b'5', "A degree"),
    (b'-', "Silence"),
    (b'.', "Hold"),
];

pub fn side(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View, checks: &Checks) {
    let piece = doc.piece();
    let shown = view.shown_pattern(piece);
    let rhythm = shown
        .as_ref()
        .and_then(|(instrument, name)| piece.rhythm(*instrument, name));
    let compact = Node {
        flex_shrink: 0.0,
        ..default()
    };
    parent.spawn(panel(compact.clone())).with_children(|panel| {
        head(panel, "Selected pattern", |head| {
            if let Some(rhythm) = &rhythm {
                head.spawn(theme::written(
                    rhythm.instrument.name(),
                    13.0,
                    theme::TEXT_SOFT,
                ));
            }
        });
        let Some(rhythm) = &rhythm else {
            note(panel, "The piece has no pattern yet.");
            return;
        };
        row(panel, "Name", 44.0, |row| {
            field(row, view, Field::PatternName, rhythm.name.as_str(), 150.0);
        });
        row(panel, "Level of a hit", 44.0, |row| {
            stepper(
                row,
                rhythm.level.to_string(),
                Press::Level(-1),
                Press::Level(1),
                40.0,
            );
        });
        if rhythm.grid.contains(&b'o') {
            row(panel, "Level of a soft hit", 44.0, |row| {
                stepper(
                    row,
                    rhythm.soft.to_string(),
                    Press::Soft(-1),
                    Press::Soft(1),
                    40.0,
                );
            });
        }
        if rhythm.instrument == Instrument::Pluck {
            row(panel, "Echo", 44.0, |row| {
                // The same press takes the echo and gives it back.
                let options = [
                    ("Off", Press::Echo, !rhythm.echo),
                    ("On", Press::Echo, rhythm.echo),
                ];
                choice(row, options, false, 30.0);
            });
        }
        let played = names::played_in(piece, rhythm.instrument, rhythm.name.as_str());
        told(panel, "Played in", played, 40.0);
        panel
            .spawn(Node {
                padding: UiRect::axes(px(12), px(8)),
                ..default()
            })
            .with_children(|remove| {
                button(
                    remove,
                    "Remove this pattern",
                    Press::RemovePattern,
                    Kind::Plain,
                    (percent(100), 32.0),
                );
            });
    });

    let drum = rhythm
        .as_ref()
        .is_some_and(|rhythm| rhythm.instrument.is_drum());
    parent.spawn(panel(compact)).with_children(|panel| {
        head(panel, "Brush", |_| {});
        panel
            .spawn(Node {
                padding: UiRect::axes(px(12), px(10)),
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(6),
                row_gap: px(6),
                ..default()
            })
            .with_children(|brushes| {
                let degree = view.brush.is_ascii_digit();
                for (symbol, name) in BRUSHES {
                    // A drum is hit, softly or not, or left alone.
                    let name = match (drum, symbol) {
                        (true, b'.') => "Nothing",
                        (true, b'x' | b'o') => name,
                        (true, _) => continue,
                        (false, _) => name,
                    };
                    let digit = symbol.is_ascii_digit();
                    let symbol = if digit && degree { view.brush } else { symbol };
                    let kind = Kind::chosen(view.brush == symbol);
                    let letter = (symbol as char).to_string();
                    brushes
                        .spawn((
                            pressed(Press::Brush(symbol), kind),
                            Node {
                                width: px(131),
                                height: px(36),
                                padding: UiRect::horizontal(px(8)),
                                align_items: AlignItems::Center,
                                column_gap: px(10),
                                border: UiRect::all(px(1)),
                                ..default()
                            },
                        ))
                        .with_children(|brush| {
                            brush.spawn((
                                Node {
                                    width: px(22),
                                    justify_content: JustifyContent::Center,
                                    ..default()
                                },
                                children![theme::strong(letter, 18.0, kind.text())],
                            ));
                            brush.spawn(theme::text(name, 15.0, kind.text()));
                        });
                }
            });
        if view.brush.is_ascii_digit() && !drum {
            // The degree that the brush writes, from 1 to 9.
            let digit = view.brush - b'0';
            let lower = b'0' + (digit - 1).max(1);
            let higher = b'0' + (digit + 1).min(9);
            row(panel, "Degree of the scale", 40.0, |row| {
                stepper(
                    row,
                    digit.to_string(),
                    Press::Brush(lower),
                    Press::Brush(higher),
                    30.0,
                );
            });
        }
        let words = if drum {
            "A click writes the brush into a step, and the same click again clears it."
        } else {
            "x, o, R, O and V follow the chord of the bar, so one pattern serves every chord."
        };
        note(panel, words);
    });

    checks_panel(
        parent,
        checks,
        Tab::Patterns,
        Node {
            min_height: px(0),
            overflow: Overflow::clip(),
            ..grow()
        },
    );
}
