//! The tab of the piece: its tempo, against the rules of the game, and its key, with the two
//! modes that every line of the piece is heard in.

use bevy::prelude::*;
use tanku_audio::{
    notation::{BEAT, Piece, STEPS},
    score::Mode,
};

use super::{
    capital, checks_panel,
    widgets::{Kind, grow, head, key, note, panel, square, stepper},
};
use crate::{
    actions::TONICS,
    checks::{Checks, RULES, on_a_step},
    doc::Doc,
    lines, names,
    state::{Press, Tab, View},
    theme,
};

/// Ticks of the game in a second.
const TICKS: f32 = 60.0;
/// Bars in a phrase of a section.
const PHRASE: usize = 4;

/// The sections that have a tempo of their own, each with it.
fn own_tempos(piece: &Piece) -> Vec<String> {
    let own = piece
        .passages()
        .iter()
        .filter(|passage| passage.tempo != piece.tempo);
    own.map(|passage| format!("{}: {}", passage.name.as_str(), passage.tempo))
        .collect()
}

pub fn list(parent: &mut ChildSpawnerCommands, doc: &Doc, _: &View) {
    let piece = doc.piece();
    head(parent, "The piece", |_| {});
    let own = piece
        .passages()
        .iter()
        .filter(|passage| passage.tempo != piece.tempo);
    let own: Vec<String> = own
        .map(|passage| format!("{} in the {}", passage.tempo, passage.name.as_str()))
        .collect();
    let tempo = match own.is_empty() {
        true => piece.tempo.to_string(),
        false => format!("{}, and {}", piece.tempo, own.join(", ")),
    };
    let tonic = TONICS[piece.key.tonic as usize % 12];
    let (home, arena) = (
        capital(piece.key.home.name()),
        capital(piece.key.arena.name()),
    );
    let modes = if home == arena {
        home
    } else {
        format!("{home}, {arena} in an arena")
    };
    let facts = [
        ("Tempo", tempo),
        ("Time", "4/4, fixed for now".to_string()),
        ("Key", format!("{tonic}, and it never changes")),
        ("Modes", modes),
    ];
    for (name, value) in facts {
        parent.spawn((
            Node {
                padding: UiRect::axes(px(12), px(8)),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                border: UiRect::bottom(px(1)),
                flex_shrink: 0.0,
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
            children![
                theme::label(name, 12.0, theme::TEXT_DIM),
                theme::wrapped(value, 15.0, theme::TEXT),
            ],
        ));
    }
    parent
        .spawn((
            Node {
                margin: UiRect::top(Val::Auto),
                border: UiRect::top(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(|foot| {
            note(
                foot,
                "One piece, the same for every seed and on every platform.",
            )
        });
}

/// The name of a part of the tab, and what it is about.
fn part(parent: &mut ChildSpawnerCommands, name: &str, words: &str, top: bool) {
    parent
        .spawn((
            Node {
                height: px(44),
                flex_shrink: 0.0,
                padding: UiRect::horizontal(px(14)),
                align_items: AlignItems::Center,
                column_gap: px(14),
                border: UiRect {
                    top: px(if top { 1.0 } else { 0.0 }),
                    bottom: px(1),
                    ..default()
                },
                ..default()
            },
            BorderColor::all(theme::RULE),
        ))
        .with_children(|header| {
            header.spawn(theme::title(name, 17.0, theme::TEXT));
            header.spawn(theme::text(words, 16.0, theme::TEXT_SOFT));
        });
}

/// A row with a name at its left.
fn line(height: f32) -> impl Bundle {
    (
        Node {
            height: px(height),
            flex_shrink: 0.0,
            padding: UiRect::horizontal(px(14)),
            align_items: AlignItems::Center,
            column_gap: px(8),
            border: UiRect::bottom(px(1)),
            ..default()
        },
        BorderColor::all(theme::RULE_DIM),
    )
}

/// A row that tells something the file doesn't hold yet.
fn fixed(parent: &mut ChildSpawnerCommands, name: &str, value: &str) {
    parent.spawn(line(44.0)).with_children(|row| {
        row.spawn((theme::text(name, 16.0, theme::TEXT_SOFT), grow()));
        row.spawn(theme::figure(value, 16.0, theme::TEXT_SOFT));
        row.spawn((
            Node {
                margin: UiRect::left(px(6)),
                ..default()
            },
            children![theme::label("Fixed for now", 10.0, theme::TEXT_DIM)],
        ));
    });
}

/// A number of seconds as the window writes it.
fn seconds(seconds: f32) -> String {
    format!("{} s", lines::number(seconds))
}

/// A few of something, in words up to five.
fn counted(count: f32, one: &str, many: &str) -> String {
    let words = ["One", "Two", "Three", "Four", "Five"];
    let whole = (count.round() - count).abs() < 1e-4;
    match (whole, count.round() as usize) {
        (true, 1) => format!("One {one}"),
        (true, count @ 2..=5) => format!("{} {many}", words[count - 1]),
        _ => format!("{} {many}", lines::number(count)),
    }
}

/// What a time of the game is in the music at a tempo: bars, beats, or a part of a beat.
fn in_the_music(lasts: f32, tempo: u16) -> String {
    let beats = lasts * tempo as f32 / 60.0;
    let bars = beats / BEAT as f32;
    if !on_a_step(lasts, tempo) {
        return format!("{} beats, between two steps", lines::number(beats));
    }
    if (bars.round() - bars).abs() < 1e-4 && bars >= 1.0 {
        return counted(bars, "bar", "bars");
    }
    match beats {
        beats if (beats - 0.25).abs() < 1e-4 => "A quarter of a beat".to_string(),
        beats if (beats - 0.5).abs() < 1e-4 => "Half a beat".to_string(),
        beats => counted(beats, "beat", "beats"),
    }
}

pub fn center(parent: &mut ChildSpawnerCommands, doc: &Doc, _: &View) {
    let piece = doc.piece();
    let tempo = piece.tempo;
    part(
        parent,
        "Time",
        "The tempo of a fight is the game's own: its rules last whole numbers of beats.",
        false,
    );
    parent
        .spawn(Node {
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|columns| {
            columns
                .spawn((
                    Node {
                        width: px(470),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        border: UiRect::right(px(1)),
                        ..default()
                    },
                    BorderColor::all(theme::RULE),
                ))
                .with_children(|left| {
                    left.spawn(line(52.0)).with_children(|row| {
                        row.spawn((
                            theme::text("Tempo, in beats per minute", 16.0, theme::TEXT),
                            grow(),
                        ));
                        stepper(
                            row,
                            tempo.to_string(),
                            Press::Tempo(-1),
                            Press::Tempo(1),
                            64.0,
                        );
                    });
                    left.spawn(line(44.0)).with_children(|row| {
                        row.spawn((
                            theme::text("A section can have its own", 16.0, theme::TEXT_SOFT),
                            grow(),
                        ));
                        let own = own_tempos(piece);
                        let words = match own.is_empty() {
                            true => "none has, set in Sections".to_string(),
                            false => format!("{}, set in Sections", own.join(", ")),
                        };
                        row.spawn(theme::text(words, 15.0, theme::TEXT_SOFT));
                    });
                    fixed(left, "Time signature", "4/4");
                    fixed(
                        left,
                        "Steps in a beat, bars in a phrase",
                        &format!("{BEAT}, {PHRASE}"),
                    );
                    left.spawn(line(44.0)).with_children(|row| {
                        row.spawn((
                            Node {
                                width: px(104),
                                flex_shrink: 0.0,
                                ..default()
                            },
                            children![theme::label("As written", 12.0, theme::TEXT_DIM)],
                        ));
                        for head in ["tempo", "key"] {
                            let written = doc
                                .record(&[head])
                                .map_or(String::new(), |words| words.join(" "));
                            row.spawn((
                                Node {
                                    margin: UiRect::right(px(14)),
                                    ..default()
                                },
                                children![theme::written(written, 16.0, theme::TEXT)],
                            ));
                        }
                    });
                    // What the tempo gives, in seconds and in ticks of the game.
                    let step = 60.0 / tempo as f32 / BEAT as f32;
                    let bar = step * STEPS as f32;
                    let facts = [
                        ("A step", seconds(step)),
                        ("A bar", seconds(bar)),
                        ("A phrase", seconds(bar * PHRASE as f32)),
                        ("Ticks in a bar", lines::number(bar * TICKS)),
                    ];
                    left.spawn(Node {
                        height: px(56),
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|row| {
                        for (place, (name, value)) in facts.into_iter().enumerate() {
                            row.spawn((
                                Node {
                                    padding: UiRect::left(px(14)),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::Center,
                                    row_gap: px(3),
                                    border: UiRect::left(px(if place == 0 { 0.0 } else { 1.0 })),
                                    ..grow()
                                },
                                BorderColor::all(theme::RULE_DIM),
                                children![
                                    theme::label(name, 11.0, theme::TEXT_DIM),
                                    theme::strong(value, 18.0, theme::TEXT),
                                ],
                            ));
                        }
                    });
                });
            columns
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    min_width: px(0),
                    ..grow()
                })
                .with_children(|right| {
                    right
                        .spawn((
                            Node {
                                height: px(28),
                                flex_shrink: 0.0,
                                align_items: AlignItems::Center,
                                border: UiRect::bottom(px(1)),
                                ..default()
                            },
                            BorderColor::all(theme::RULE_DIM),
                        ))
                        .with_children(|header| {
                            let music = format!("In the music, at {tempo}");
                            let columns = [
                                ("A rule of the game", 304.0, 14.0),
                                ("Lasts", 80.0, 0.0),
                                (music.as_str(), 160.0, 0.0),
                            ];
                            for (name, width, left) in columns {
                                header.spawn((
                                    Node {
                                        width: px(width),
                                        flex_shrink: 0.0,
                                        padding: UiRect::left(px(left)),
                                        ..default()
                                    },
                                    children![theme::label(name, 11.0, theme::TEXT_DIM)],
                                ));
                            }
                        });
                    for (rule, lasts) in RULES {
                        right.spawn(line(38.0)).with_children(|row| {
                            row.spawn((
                                Node {
                                    width: px(282),
                                    flex_shrink: 0.0,
                                    overflow: Overflow::clip(),
                                    ..default()
                                },
                                children![theme::text(rule, 15.0, theme::TEXT)],
                            ));
                            row.spawn((
                                Node {
                                    width: px(72),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                                children![theme::written(seconds(lasts), 14.0, theme::TEXT_SOFT)],
                            ));
                            let on = on_a_step(lasts, tempo);
                            row.spawn(square(Some(on)));
                            let ink = if on { theme::TEXT } else { theme::DANGER };
                            row.spawn(theme::text(in_the_music(lasts, tempo), 15.0, ink));
                        });
                    }
                    let shots = lines::number(60.0 / tempo as f32 / RULES[0].1);
                    let words =
                        format!("A player who holds the trigger fires {shots} shots per beat.");
                    right
                        .spawn(Node {
                            padding: UiRect::horizontal(px(14)),
                            align_items: AlignItems::Center,
                            ..grow()
                        })
                        .with_children(|foot| {
                            foot.spawn(theme::text(words, 14.0, theme::TEXT_SOFT));
                        });
                });
        });

    part(
        parent,
        "Pitch",
        "Every line is written in degrees, so it exists in both modes without being written twice.",
        true,
    );
    let choice = |parent: &mut ChildSpawnerCommands,
                  name: &str,
                  keys: &mut dyn FnMut(&mut ChildSpawnerCommands)| {
        parent.spawn(line(46.0)).with_children(|row| {
            row.spawn((
                Node {
                    width: px(168),
                    flex_shrink: 0.0,
                    ..default()
                },
                children![theme::text(name, 16.0, theme::TEXT)],
            ));
            keys(row);
        });
    };
    choice(parent, "Key", &mut |row| {
        row.spawn(Node::default()).with_children(|keys| {
            for (tonic, name) in TONICS.into_iter().enumerate() {
                let kind = Kind::chosen(piece.key.tonic as usize == tonic);
                let words = theme::strong(name, 15.0, kind.text());
                key(keys, words, Press::Tonic(tonic as u8), kind, (px(46), 32.0));
            }
        });
        row.spawn((
            Node {
                margin: UiRect::left(px(6)),
                ..default()
            },
            children![theme::text(
                "The cues stay in A and E.",
                14.0,
                theme::TEXT_SOFT
            )],
        ));
    });
    let modes =
        |parent: &mut ChildSpawnerCommands, name: &str, now: Mode, press: fn(Mode) -> Press| {
            choice(parent, name, &mut |row| {
                row.spawn(Node::default()).with_children(|keys| {
                    for mode in Mode::ALL {
                        let kind = Kind::chosen(mode == now);
                        let words = theme::text(capital(mode.name()), 15.0, kind.text());
                        key(keys, words, press(mode), kind, (Val::Auto, 32.0));
                    }
                });
            });
        };
    modes(parent, "Mode of the piece", piece.key.home, Press::Home);
    modes(
        parent,
        "Mode of a fight in an arena",
        piece.key.arena,
        Press::Arena,
    );
    scales(parent, piece);

    let (home, arena) = (piece.key.home, piece.key.arena);
    let differ = (1..=7)
        .filter(|&degree| home.semitones(degree) != arena.semitones(degree))
        .count();
    let minor = |mode: Mode| mode.semitones(3) == 3;
    let kinds = match (minor(home), minor(arena)) {
        (true, true) => "Both modes are minor",
        (false, false) => "Both modes are major",
        _ => "One mode is major and the other is minor",
    };
    let words = match differ {
        0 => "An arena is in the mode of the piece: its fights sound like the others.".to_string(),
        1 => format!(
            "{kinds} and differ by one note, so an arena does not change the piece: it colors it."
        ),
        count => format!("{kinds} and differ by {count} notes: every line is heard in both."),
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

/// The two modes on the twelve notes from the tonic to the tonic above it, and their chords.
fn scales(parent: &mut ChildSpawnerCommands, piece: &Piece) {
    const GUTTER: f32 = 190.0;
    const NOTE: f32 = 64.0;
    const TOP: f32 = 30.0;
    const ROW: f32 = 40.0;
    let key = piece.key;
    let rows = [(key.home, "the piece"), (key.arena, "fights in an arena")];
    parent
        .spawn((
            Node {
                height: px(TOP + 3.0 * ROW),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(theme::FLOOR),
        ))
        .with_children(|drawing| {
            let gutter = |height: f32| {
                (
                    Node {
                        width: px(GUTTER),
                        height: px(height),
                        flex_shrink: 0.0,
                        padding: UiRect::left(px(14)),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        row_gap: px(1),
                        border: UiRect::right(px(1)),
                        ..default()
                    },
                    BackgroundColor(theme::SURFACE_RAISED),
                    BorderColor::all(theme::RULE),
                )
            };
            let ruled = |height: f32| {
                (
                    Node {
                        height: px(height),
                        flex_shrink: 0.0,
                        border: UiRect::bottom(px(1)),
                        ..default()
                    },
                    BorderColor::all(theme::RULE_DIM),
                )
            };
            // The notes, from the tonic up.
            drawing.spawn(ruled(TOP)).with_children(|row| {
                row.spawn(gutter(TOP));
                for semitone in 0..=12 {
                    let ends = semitone % 12 == 0 || semitone == 7;
                    let ink = if ends { theme::TEXT } else { theme::TEXT_DIM };
                    let name = TONICS[(key.tonic as usize + semitone) % 12];
                    row.spawn((
                        Node {
                            width: px(NOTE),
                            height: percent(100),
                            flex_shrink: 0.0,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(theme::SURFACE_RAISED),
                        children![theme::strong(name, 14.0, ink)],
                    ));
                }
            });
            for (place, (mode, whose)) in rows.into_iter().enumerate() {
                let other = rows[1 - place].0;
                drawing.spawn(ruled(ROW)).with_children(|row| {
                    row.spawn(gutter(ROW)).with_children(|name| {
                        name.spawn(theme::title(mode.name(), 12.0, theme::TEXT));
                        name.spawn(theme::text(whose, 12.0, theme::TEXT_DIM));
                    });
                    for semitone in 0..=12u8 {
                        let degree = (1..=8).find(|&degree| mode.semitones(degree) == semitone);
                        // A degree that the other mode has on another note stands out.
                        let apart =
                            degree.is_some_and(|degree| other.semitones(degree) != semitone);
                        let edge = if apart { theme::ACCENT } else { Color::NONE };
                        row.spawn(Node {
                            width: px(NOTE),
                            flex_shrink: 0.0,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|cell| {
                            let Some(degree) = degree else {
                                return;
                            };
                            cell.spawn((
                                Node {
                                    width: px(28),
                                    height: px(28),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border: UiRect::all(px(1)),
                                    ..default()
                                },
                                BorderColor::all(edge),
                                children![theme::figure(degree.to_string(), 17.0, theme::TEXT)],
                            ));
                        });
                    }
                });
            }
            // The chords that a section can write, in each mode.
            drawing
                .spawn(Node {
                    height: px(ROW),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(gutter(ROW)).with_children(|name| {
                        name.spawn(theme::title("Chords", 12.0, theme::TEXT));
                    });
                    for (mode, _) in
                        rows.into_iter()
                            .take(if key.home == key.arena { 1 } else { 2 })
                    {
                        let chord = |written: u8| names::chord(piece, mode, written);
                        let words = format!(
                            "{}: {} on 1, {} on 4, {} on 5, {} on {}",
                            capital(mode.name()),
                            chord(1),
                            chord(4),
                            chord(5),
                            chord(0),
                            mode.color()
                        );
                        row.spawn((
                            Node {
                                width: px(416),
                                flex_shrink: 0.0,
                                padding: UiRect::left(px(14)),
                                ..default()
                            },
                            children![theme::text(words, 15.0, theme::TEXT_SOFT)],
                        ));
                    }
                });
        });
}

/// What a change of each thing of this tab does to the rest of the piece.
const TOUCHES: [(&str, &str); 4] = [
    (
        "Tempo",
        "Nothing is rewritten. The first check says whether shots and reloads still end on a step.",
    ),
    (
        "Time signature",
        "Fixed for now: the file has no line for it, and every rule counts on bars of 16 steps.",
    ),
    (
        "Key",
        "Nothing is rewritten. The ranges are checked again, and the cues stay in A.",
    ),
    (
        "Mode",
        "Nothing is rewritten. Every melody is checked again against its chords.",
    ),
];

pub fn side(parent: &mut ChildSpawnerCommands, _: &Doc, _: &View, checks: &Checks) {
    checks_panel(
        parent,
        checks,
        Tab::Piece,
        Node {
            flex_shrink: 0.0,
            ..default()
        },
    );
    parent
        .spawn(panel(Node {
            min_height: px(0),
            overflow: Overflow::clip(),
            ..grow()
        }))
        .with_children(|panel| {
            head(panel, "What a change here touches", |_| {});
            for (name, words) in TOUCHES {
                panel.spawn((
                    Node {
                        padding: UiRect::axes(px(12), px(8)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(2),
                        border: UiRect::bottom(px(1)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BorderColor::all(theme::RULE_DIM),
                    children![
                        theme::text(name, 16.0, theme::TEXT),
                        theme::wrapped(words, 14.0, theme::TEXT_SOFT),
                    ],
                ));
            }
        });
}
