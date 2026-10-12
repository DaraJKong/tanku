//! The tab of the transitions: the roll that announces a fight and the connection that leads
//! out of one. The file writes a few numbers of each, and the rules derive the rest, which is
//! drawn in dashes. What else the music does between two states is in the code, and is only
//! told here.

use bevy::prelude::*;
use tanku_audio::{
    notation::{BEAT, Instrument, Line, Piece, Scene},
    score::connection,
};

use super::{
    capital, checks_panel,
    widgets::{
        Dashed, Kind, caption, grow, head, heading, key, note, panel, pressed, row, stepper, told,
    },
};
use crate::{
    checks::Checks,
    doc::Doc,
    names,
    state::{Press, Tab, View, Way},
    theme,
};

/// The left of the drawing, where its rows are named, and the widths of its three parts: what
/// plays before, the transition, and what follows.
const GUTTER: f32 = 110.0;
const BEFORE: f32 = 210.0;
const DURING: f32 = 454.0;
const AFTER: f32 = 248.0;
/// The heights of its rows.
const TOP: f32 = 28.0;
const ROW: f32 = 34.0;

/// What else the music does between two states. None of it is in the file.
const IN_THE_CODE: [(&str, &str); 3] = [
    ("Tank destroyed", "Hush, to the bar line"),
    ("Pause menu", "Muffle, in 0.2 s"),
    ("Warning of a mine", "Duck, in 0.02 s"),
];

fn beats(count: u8) -> String {
    if count == 1 {
        "1 beat".to_string()
    } else {
        format!("{count} beats")
    }
}

pub fn list(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View) {
    let piece = doc.piece();
    head(parent, "Transitions", |_| {});
    caption(parent, "In the file");
    let ways = [
        (
            Way::Roll,
            "Into a fight",
            format!("Roll, {}", beats(piece.roll.beats)),
        ),
        (
            Way::Link,
            "Out of a fight",
            format!("Connection, {}", beats(piece.link.beats)),
        ),
    ];
    for (way, name, word) in ways {
        let picked = view.way == way;
        let kind = if picked { Kind::Picked } else { Kind::Row };
        parent
            .spawn((
                pressed(Press::Way(way), kind),
                Node {
                    height: px(46),
                    flex_shrink: 0.0,
                    padding: UiRect::horizontal(px(12)),
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
            ))
            .with_children(|row| {
                if picked {
                    row.spawn((
                        Node {
                            width: px(10),
                            height: px(10),
                            flex_shrink: 0.0,
                            ..default()
                        },
                        BackgroundColor(theme::ACCENT),
                    ));
                }
                two_lines(row, name, &word, theme::TEXT);
            });
    }
    caption(parent, "In the code");
    for (name, word) in IN_THE_CODE {
        parent
            .spawn((
                Node {
                    height: px(46),
                    flex_shrink: 0.0,
                    padding: UiRect::horizontal(px(12)),
                    align_items: AlignItems::Center,
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(theme::RULE_DIM),
            ))
            .with_children(|row| two_lines(row, name, word, theme::TEXT_SOFT));
    }
}

/// A name over a word that says more.
fn two_lines(parent: &mut ChildSpawnerCommands, name: &str, word: &str, ink: Color) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(1),
            ..default()
        })
        .with_children(|words| {
            words.spawn(theme::text(name, 16.0, ink));
            words.spawn(theme::text(word, 12.0, theme::TEXT_DIM));
        });
}

pub fn center(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View) {
    let piece = doc.piece();
    let (name, words) = match view.way {
        Way::Roll => (
            "Roll",
            "Into a fight: a drum rolls, louder on every step, and the fight starts when it ends.",
        ),
        Way::Link => (
            "Connection",
            "Out of a fight: the tune walks to the degree on which what follows starts.",
        ),
    };
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
            header.spawn(theme::title(name, 17.0, theme::TEXT));
            header.spawn(theme::text(words, 16.0, theme::TEXT_SOFT));
        });
    match view.way {
        Way::Roll => roll(parent, piece),
        Way::Link => link(parent, piece),
    }
}

/// The first section that a scene plays.
fn opens(piece: &Piece, scene: Scene) -> (String, u16) {
    match piece.play(scene).sections().first() {
        Some(&index) => {
            let passage = piece.section(index);
            (passage.name.as_str().to_string(), passage.tempo)
        }
        None => ("nothing".to_string(), piece.tempo),
    }
}

/// The drawing of a transition: a row of names at its left, then what plays before, the
/// transition and what follows, each under its name.
fn drawing(
    parent: &mut ChildSpawnerCommands,
    parts: [String; 3],
    rows: &[&str],
    foot: f32,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    let height = TOP + rows.len() as f32 * ROW + foot;
    parent
        .spawn((
            Node {
                height: px(height),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(theme::FLOOR),
        ))
        .with_children(|drawing| {
            // The names of the rows.
            drawing
                .spawn((
                    Node {
                        width: px(GUTTER),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::top(px(TOP)),
                        border: UiRect::right(px(1)),
                        ..default()
                    },
                    BackgroundColor(theme::SURFACE_RAISED),
                    BorderColor::all(theme::RULE),
                ))
                .with_children(|gutter| {
                    for name in rows.iter().chain(&["Tempo"]) {
                        gutter.spawn((
                            Node {
                                height: px(ROW),
                                flex_shrink: 0.0,
                                padding: UiRect::left(px(12)),
                                align_items: AlignItems::Center,
                                border: UiRect::top(px(1)),
                                ..default()
                            },
                            BorderColor::all(theme::RULE_DIM),
                            children![theme::title(*name, 12.0, theme::TEXT)],
                        ));
                    }
                });
            // The three parts, the one in the middle a little lighter.
            let widths = [BEFORE, DURING, AFTER];
            let mut left = GUTTER;
            for (place, (name, width)) in parts.into_iter().zip(widths).enumerate() {
                let fill = if place == 1 {
                    theme::ACCENT.with_alpha(0.08)
                } else {
                    Color::NONE
                };
                drawing
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(left),
                            top: px(0),
                            width: px(width),
                            height: percent(100),
                            border: UiRect::left(px(if place == 0 { 0.0 } else { 1.0 })),
                            ..default()
                        },
                        BackgroundColor(fill),
                        BorderColor::all(theme::RULE),
                    ))
                    .with_children(|part| {
                        part.spawn((
                            Node {
                                height: px(TOP),
                                width: percent(100),
                                padding: UiRect::left(px(8)),
                                align_items: AlignItems::Center,
                                border: UiRect::bottom(px(1)),
                                ..default()
                            },
                            BackgroundColor(theme::SURFACE_RAISED),
                            BorderColor::all(theme::RULE_DIM),
                            children![theme::label(name, 11.0, theme::TEXT_SOFT)],
                        ));
                    });
                left += width;
            }
            // The lines between the rows.
            for line in 1..=rows.len() {
                drawing.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(GUTTER),
                        right: px(0),
                        top: px(TOP + line as f32 * ROW),
                        height: px(1),
                        ..default()
                    },
                    BackgroundColor(theme::RULE_DIM),
                ));
            }
            content(drawing);
        });
}

/// Something that is written or played as it is: a full bar in a row of the drawing.
fn solid(parent: &mut ChildSpawnerCommands, line: usize, left: f32, width: f32, words: &str) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(left + 1.0),
            top: px(TOP + line as f32 * ROW + 8.0),
            width: px(width - 3.0),
            height: px(ROW - 15.0),
            padding: UiRect::left(px(6)),
            align_items: AlignItems::Center,
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(theme::LINE),
        children![theme::strong(words, 13.0, theme::TEXT_ON_ACCENT)],
    ));
}

/// Something that a rule derives: the same bar, in dashes.
fn derived(parent: &mut ChildSpawnerCommands, line: usize, left: f32, width: f32, words: String) {
    let (left, top) = (left + 1.0, TOP + line as f32 * ROW + 8.0);
    let size = Vec2::new(width - 3.0, ROW - 15.0);
    parent.spawn(Dashed {
        color: theme::LINE,
        left,
        top,
        size: Some(size),
    });
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(left + 6.0),
            top: px(top),
            height: px(size.y),
            align_items: AlignItems::Center,
            ..default()
        },
        children![theme::written(words, 13.0, theme::TEXT)],
    ));
}

/// A hit of a drum: a square in the middle of its step, as large as it is loud.
fn hit(parent: &mut ChildSpawnerCommands, line: usize, left: f32, level: u8) {
    let size = (6.0 + 16.0 * level as f32 / 255.0).round();
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(left + 2.0),
            top: px(TOP + line as f32 * ROW + ((ROW - size) / 2.0).round()),
            width: px(size),
            height: px(size),
            ..default()
        },
        BackgroundColor(theme::LINE),
    ));
}

/// A sentence in the last row of the drawing, under a part of it.
fn tempo(parent: &mut ChildSpawnerCommands, rows: usize, left: f32, words: String, ink: Color) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(left + 8.0),
            top: px(TOP + rows as f32 * ROW),
            height: px(ROW),
            align_items: AlignItems::Center,
            ..default()
        },
        children![theme::text(words, 14.0, ink)],
    ));
}

/// The notes of a connection, each with the step where it starts.
fn notes(line: &Line) -> Vec<(usize, u8, u8)> {
    let cells = line.iter().enumerate();
    cells
        .filter(|(_, cell)| cell.degree != 0)
        .map(|(step, cell)| (step, cell.degree, cell.steps))
        .collect()
}

/// The connection: what it does from the upper tonic, then every way that it can go.
fn link(parent: &mut ChildSpawnerCommands, piece: &Piece) {
    let link = piece.link;
    let (fight, from) = opens(piece, Scene::Corridor);
    let (calm, to) = opens(piece, Scene::Calm);
    let steps = link.beats as usize * BEAT;
    let step = DURING / steps as f32;
    let (before, during, after) = (GUTTER, GUTTER + BEFORE, GUTTER + BEFORE + DURING);
    let parts = [
        format!("{fight}, to its next beat"),
        format!("Connection, {}", beats(link.beats)),
        format!("{calm}, bar 1"),
    ];
    drawing(
        parent,
        parts,
        &["Kick", "Bass", "Pluck", "Lead"],
        ROW,
        |drawing| {
            // The kick keeps the beat, and the others walk together.
            hit(drawing, 0, before, 255);
            for beat in 0..link.beats as usize {
                hit(drawing, 0, during + (beat * BEAT) as f32 * step, 255);
            }
            hit(drawing, 0, after, 255);
            for line in 1..=3 {
                let words = if line == 3 { "8" } else { "" };
                solid(drawing, line, before, BEFORE, words);
            }
            for (start, degree, length) in notes(&connection(Some(8), link)) {
                let left = during + start as f32 * step;
                let width = length as f32 * step;
                for line in 1..=3 {
                    let name = names::note(piece, piece.key.home, degree);
                    let words = if line == 3 {
                        format!("{degree} {name}")
                    } else {
                        degree.to_string()
                    };
                    derived(drawing, line, left, width, words);
                }
            }
            tempo(drawing, 4, before, from.to_string(), theme::TEXT);
            let words = if from == to {
                "The tempo stays as it is.".to_string()
            } else {
                let longer = if to < from { "longer" } else { "shorter" };
                format!("Each step is a little {longer} than the one before.")
            };
            tempo(drawing, 4, during, words, theme::TEXT_SOFT);
            tempo(drawing, 4, after, to.to_string(), theme::TEXT);
        },
    );

    heading(
        parent,
        "The way",
        "Derived for each note the lead can be on, by steps of the scale and without turning back.",
        true,
    );
    // A step of the table is as wide as the room lets it be.
    const FROM: f32 = 198.0;
    let cell = (520.0 / steps as f32).floor().min(44.0);
    parent
        .spawn((
            Node {
                height: px(28),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                border: UiRect::vertical(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(|header| {
            let name = |name: String, width: Val, left: f32| {
                (
                    Node {
                        width,
                        flex_shrink: 0.0,
                        padding: UiRect::left(px(left)),
                        ..default()
                    },
                    children![theme::label(name, 11.0, theme::TEXT_DIM)],
                )
            };
            header.spawn(name("The lead is on".to_string(), px(FROM), 14.0));
            for beat in 1..=link.beats {
                header.spawn(name(format!("Beat {beat}"), px(BEAT as f32 * cell), 6.0));
            }
            header.spawn(name("What it is".to_string(), Val::Auto, 14.0));
        });
    // The lead is on one of the five degrees of the motifs, in any octave, or it is silent.
    // The ways that are the same are told together.
    let mut ways: Vec<(Vec<String>, Line)> = Vec::new();
    for from in [9, 8, 6, 5, 3, 2, 1] {
        let line = connection(Some(from), link);
        match ways.iter_mut().find(|(_, other)| *other == line) {
            Some((degrees, _)) => degrees.insert(0, from.to_string()),
            None => ways.push((vec![from.to_string()], line)),
        }
    }
    let silent = connection(None, link);
    for (degrees, line) in ways
        .iter()
        .map(|(degrees, line)| (degrees.join(" or "), line))
    {
        way(parent, &degrees, line, (FROM, cell, steps), link.degree);
    }
    way(
        parent,
        "nothing: it is silent",
        &silent,
        (FROM, cell, steps),
        link.degree,
    );
    parent
        .spawn(Node {
            padding: UiRect::axes(px(14), px(12)),
            min_height: px(0),
            overflow: Overflow::clip(),
            ..grow()
        })
        .with_children(|foot| {
            let words = "The ways follow from the settings and are not saved. The bass and the pluck go \
                         with the lead, each in its own octave. To hear one, play and turn the fight off.";
            foot.spawn(theme::wrapped(words, 14.0, theme::TEXT_SOFT));
        });
}

/// One way of the connection: where the lead is, the notes that lead from there, and what
/// they are in a word.
fn way(
    parent: &mut ChildSpawnerCommands,
    from: &str,
    line: &Line,
    (gutter, cell, steps): (f32, f32, usize),
    goal: u8,
) {
    let notes = notes(line);
    parent
        .spawn((
            Node {
                height: px(34),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: px(gutter),
                    flex_shrink: 0.0,
                    padding: UiRect::left(px(14)),
                    ..default()
                },
                children![theme::text(from, 16.0, theme::TEXT)],
            ));
            row.spawn((
                Node {
                    width: px(cell * steps as f32),
                    height: percent(100),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(theme::FLOOR),
            ))
            .with_children(|cells| {
                for &(start, degree, length) in &notes {
                    let left = start as f32 * cell;
                    cells.spawn(Dashed {
                        color: theme::LINE,
                        left: left + 2.0,
                        top: 6.0,
                        size: Some(Vec2::new(length as f32 * cell - 4.0, 21.0)),
                    });
                    cells.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(left + 9.0),
                            top: px(6),
                            height: px(21),
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        children![theme::figure(degree.to_string(), 14.0, theme::TEXT)],
                    ));
                }
            });
            let first = notes.first().map_or(goal, |note| note.1);
            let what = match (notes.len(), first > goal) {
                (0 | 1, _) => format!("It stays on {goal}."),
                (count, true) => format!("A fall of {count} notes."),
                (count, false) => format!("A rise of {count} notes."),
            };
            row.spawn((
                Node {
                    padding: UiRect::left(px(14)),
                    ..default()
                },
                children![theme::text(what, 15.0, theme::TEXT_SOFT)],
            ));
        });
}

/// The roll: the drum that announces a fight, in a corridor and in an arena.
fn roll(parent: &mut ChildSpawnerCommands, piece: &Piece) {
    let roll = piece.roll;
    let (fight, tempo_of) = opens(piece, Scene::Corridor);
    let hits = roll.beats as usize * BEAT;
    let step = DURING / hits as f32;
    let (before, during, after) = (GUTTER, GUTTER + BEFORE, GUTTER + BEFORE + DURING);
    // The level of each hit, as the game counts it.
    let (from, to) = (roll.from as i32, roll.to as i32);
    let level = |hit: usize| (from + (to - from) * (hit as i32 + 1) / hits as i32) as u8;
    let parts = [
        "Before the fight".to_string(),
        format!("Roll, {}", beats(roll.beats)),
        format!("{fight}, bar 1"),
    ];
    drawing(
        parent,
        parts,
        &["Corridor", "Arena", "The others"],
        ROW,
        |drawing| {
            for (line, drum) in [roll.drum, roll.arena].into_iter().enumerate() {
                for hit_at in 0..hits {
                    hit(drawing, line, during + hit_at as f32 * step, level(hit_at));
                }
                let name = format!("the {} rolls", names::instrument(drum).to_lowercase());
                tempo_at(drawing, line, before, name);
            }
            solid(drawing, 2, before, BEFORE, "");
            tempo_at(
                drawing,
                2,
                during,
                "Nothing else starts under the roll.".to_string(),
            );
            solid(drawing, 2, after, AFTER, "");
            tempo(
                drawing,
                3,
                during,
                format!("At the tempo of the fight: {tempo_of}."),
                theme::TEXT_SOFT,
            );
            tempo(drawing, 3, after, tempo_of.to_string(), theme::TEXT);
        },
    );

    heading(
        parent,
        "The hits",
        "Derived: one on every step, from the first level to the last.",
        true,
    );
    parent
        .spawn((
            Node {
                height: px(40),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                border: UiRect::vertical(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: px(GUTTER + BEFORE),
                    flex_shrink: 0.0,
                    padding: UiRect::left(px(14)),
                    ..default()
                },
                children![theme::text("Level of each hit", 16.0, theme::TEXT)],
            ));
            for hit_at in 0..hits {
                row.spawn((
                    Node {
                        width: px(step),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    children![theme::figure(
                        level(hit_at).to_string(),
                        13.0,
                        theme::TEXT_SOFT
                    )],
                ));
            }
        });
    parent
        .spawn(Node {
            padding: UiRect::axes(px(14), px(12)),
            min_height: px(0),
            overflow: Overflow::clip(),
            ..grow()
        })
        .with_children(|foot| {
            let words = "The roll is heard on the step where the fight starts, and the fight plays its \
                         first bar when the roll ends. To hear it, play and turn the fight on.";
            foot.spawn(theme::wrapped(words, 14.0, theme::TEXT_SOFT));
        });
}

/// A few words in a row of the drawing, at the left of a part of it.
fn tempo_at(parent: &mut ChildSpawnerCommands, line: usize, left: f32, words: String) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(left + 8.0),
            top: px(TOP + line as f32 * ROW),
            height: px(ROW),
            align_items: AlignItems::Center,
            ..default()
        },
        children![theme::text(words, 14.0, theme::TEXT_SOFT)],
    ));
}

pub fn side(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View, checks: &Checks) {
    let piece = doc.piece();
    let head_of = if view.way == Way::Roll {
        "roll"
    } else {
        "link"
    };
    let written = doc
        .record(&[head_of])
        .map_or(String::new(), |words| words.join(" "));
    parent
        .spawn(panel(Node {
            flex_shrink: 0.0,
            ..default()
        }))
        .with_children(|panel| {
            head(panel, "Settings", |head| {
                head.spawn(theme::written(written, 13.0, theme::TEXT_SOFT));
            });
            let number = |panel: &mut ChildSpawnerCommands, name: &str, word: usize, value: u8| {
                row(panel, name, 42.0, |row| {
                    let (less, more) = (Press::WayNumber(word, -1), Press::WayNumber(word, 1));
                    stepper(row, value.to_string(), less, more, 38.0);
                });
            };
            match view.way {
                Way::Link => {
                    let link = piece.link;
                    number(panel, "Goes to degree", 2, link.degree);
                    number(panel, "Lasts, in beats", 1, link.beats);
                    number(panel, "Notes at most", 3, link.notes);
                    caption(panel, "In the code");
                    told(panel, "Starts", "On the next beat", 36.0);
                    told(panel, "Mode", "Of the piece", 36.0);
                    told(panel, "Tempo", "Goes to that of what follows", 36.0);
                    let mode = capital(piece.key.home.name());
                    note(
                        panel,
                        &format!(
                            "In {mode} even after an arena: the first sign that the fight is over."
                        ),
                    );
                }
                Way::Roll => {
                    let roll = piece.roll;
                    number(panel, "Lasts, in beats", 1, roll.beats);
                    let drum = |panel: &mut ChildSpawnerCommands,
                                name: &str,
                                word: usize,
                                drum: Instrument| {
                        row(panel, name, 42.0, |row| {
                            let words = theme::text(names::instrument(drum), 15.0, theme::TEXT);
                            key(
                                row,
                                words,
                                Press::WayDrum(word),
                                Kind::Plain,
                                (px(110), 30.0),
                            );
                        });
                    };
                    drum(panel, "Drum", 2, roll.drum);
                    drum(panel, "Drum in an arena", 3, roll.arena);
                    number(panel, "First level", 4, roll.from);
                    number(panel, "Last level", 5, roll.to);
                    caption(panel, "In the code");
                    told(panel, "Starts", "At once", 36.0);
                    told(panel, "Tempo", "That of the fight", 36.0);
                }
            }
        });
    checks_panel(
        parent,
        checks,
        Tab::Transitions,
        Node {
            min_height: px(0),
            overflow: Overflow::clip(),
            ..grow()
        },
    );
}
