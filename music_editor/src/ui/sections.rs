//! The tab of the sections: the eight bars of each, with their chords and the motifs that
//! their melody states, and the arrangement, which says what every instrument plays in each
//! scene and at each threat.

use bevy::prelude::*;
use tanku_audio::notation::{Instrument, Part, Piece, STEPS, Scene, THREATS};

use super::{
    Over, Playhead, capital, checks_of,
    widgets::{
        Dashed, Kind, Look, button, field, grow, head, heading, key, list_row, note, panel, row,
        stepper, told, turner,
    },
};
use crate::{
    actions,
    checks::Checks,
    doc::Doc,
    names,
    parts::{PartText, SectionLine, Statement, Voice},
    state::{Ending, Field, Picked, Press, Row, Tab, View},
    theme,
};

/// The left of the strip of the bars, where its rows are named, and the width of a bar.
const GUTTER: f32 = 110.0;
const BAR: f32 = 114.0;

/// The scenes that play a section, in a word or two.
fn played_by(piece: &Piece, name: &str) -> String {
    let plays = |scene: Scene| {
        let sections = piece.play(scene).sections().iter();
        sections
            .map(|&index| piece.section(index))
            .any(|passage| passage.name.is(name))
    };
    let mut scenes = Vec::new();
    for scene in [Scene::Menu, Scene::Calm] {
        if plays(scene) {
            scenes.push(scene.name());
        }
    }
    match (plays(Scene::Corridor), plays(Scene::Arena)) {
        (true, true) => scenes.push("fights"),
        (true, false) => scenes.push("corridor"),
        (false, true) => scenes.push("arena"),
        (false, false) => {}
    }
    if scenes.is_empty() {
        return "unused".to_string();
    }
    scenes.join(", ")
}

pub fn list(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View) {
    let piece = doc.piece();
    let shown = view.shown_section(piece);
    head(parent, "Sections", |_| {});
    for passage in piece.passages() {
        let name = passage.name.as_str();
        let picked = shown.as_deref() == Some(name);
        let word = played_by(piece, name);
        list_row(
            parent,
            name,
            &word,
            Press::Section(name.to_string()),
            picked,
            44.0,
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
                "New section",
                Press::NewSection,
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
        .with_children(|foot| note(foot, "A scene plays its sections in turn."));
}

/// The line of the document that starts with these words.
fn line_of<'a>(doc: &'a Doc, head: &[&str]) -> Option<&'a str> {
    doc.find(head).map(|at| doc.lines()[at].as_str())
}

/// The section that is shown, as its line writes it.
fn shown(doc: &Doc, view: &View) -> Option<SectionLine> {
    let name = view.shown_section(doc.piece())?;
    SectionLine::parse(line_of(doc, &["sec", &name])?)
}

pub fn center(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View) {
    let piece = doc.piece();
    let Some(section) = shown(doc, view) else {
        note(parent, "The piece has no section yet.");
        return;
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
            header.spawn(theme::title(section.name.as_str(), 17.0, theme::TEXT));
            let words = "Eight bars, one chord each, in two phrases.";
            header.spawn((theme::text(words, 16.0, theme::TEXT_SOFT), grow()));
            let written = line_of(doc, &["sec", &section.name]).unwrap_or_default();
            let written = written.split_whitespace().collect::<Vec<_>>().join(" ");
            header.spawn(theme::written(written, 14.0, theme::TEXT_SOFT));
        });

    strip(parent, piece, view, &section);
    arrangement(parent, doc, view);
    by_threat(parent, doc, view);
}

/// The name of a row of the strip, and a word under it.
fn named(parent: &mut ChildSpawnerCommands, name: &str, word: &str, height: f32, bottom: bool) {
    parent
        .spawn((
            Node {
                height: px(height),
                flex_shrink: 0.0,
                padding: UiRect::left(px(12)),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                row_gap: px(3),
                border: UiRect::bottom(px(if bottom { 1.0 } else { 0.0 })),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(|words| {
            words.spawn(theme::title(name, 12.0, theme::TEXT));
            if !word.is_empty() {
                words.spawn(theme::text(word, 12.0, theme::TEXT_DIM));
            }
        });
}

/// What a statement is, in a few words, next to how the file writes it.
fn stated(statement: &Statement) -> String {
    let letter = statement.letter as char;
    match (statement.last, statement.turned) {
        (Some(degree), _) => format!("{}: {letter} ending on {degree}", statement.write()),
        (None, true) => format!("{}: {letter} with its turn", statement.write()),
        (None, false) => statement.write(),
    }
}

/// The eight bars of the section: the chord of each, and the motifs that its melody states.
fn strip(parent: &mut ChildSpawnerCommands, piece: &Piece, view: &View, section: &SectionLine) {
    const TOP: f32 = 26.0;
    const CHORD: f32 = 64.0;
    const MELODY: f32 = 80.0;
    let passage = piece.passage(&section.name);
    let place = piece
        .passages()
        .iter()
        .position(|passage| passage.name.is(&section.name));
    let (home, arena) = (piece.key.home, piece.key.arena);
    parent
        .spawn((
            Node {
                height: px(TOP + CHORD + MELODY),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(theme::FLOOR),
        ))
        .with_children(|strip| {
            strip
                .spawn((
                    Node {
                        width: px(GUTTER),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        border: UiRect::right(px(1)),
                        ..default()
                    },
                    BackgroundColor(theme::SURFACE_RAISED),
                    BorderColor::all(theme::RULE),
                ))
                .with_children(|gutter| {
                    gutter.spawn((
                        Node {
                            height: px(TOP),
                            flex_shrink: 0.0,
                            padding: UiRect::left(px(12)),
                            align_items: AlignItems::Center,
                            border: UiRect::bottom(px(1)),
                            ..default()
                        },
                        BorderColor::all(theme::RULE_DIM),
                        children![theme::label("Bar", 11.0, theme::TEXT_DIM)],
                    ));
                    let modes = if home == arena {
                        capital(home.name())
                    } else {
                        format!("{}, {}", capital(home.name()), capital(arena.name()))
                    };
                    named(gutter, "Chord", &modes, CHORD, true);
                    named(gutter, "Melody", "on the lead", MELODY, false);
                });
            for bar in 0..8 {
                let picked = view.bar == bar;
                let chord = section.chords[bar];
                // A phrase is four bars: the line between the two is the heaviest.
                let rule = if bar == 4 {
                    theme::LINE.with_alpha(0.7)
                } else {
                    theme::GRID.with_alpha(0.2)
                };
                strip
                    .spawn((
                        Button,
                        Press::Bar(bar),
                        Look::cell(),
                        BackgroundColor(Color::NONE),
                        BorderColor::all(rule),
                        Node {
                            width: px(BAR),
                            flex_shrink: 0.0,
                            flex_direction: FlexDirection::Column,
                            border: UiRect::left(px(if bar == 0 { 0.0 } else { 1.0 })),
                            ..default()
                        },
                    ))
                    .with_children(|column| {
                        column.spawn((
                            Node {
                                height: px(TOP),
                                flex_shrink: 0.0,
                                padding: UiRect::left(px(8)),
                                align_items: AlignItems::Center,
                                border: UiRect::bottom(px(1)),
                                ..default()
                            },
                            BackgroundColor(theme::SURFACE_RAISED),
                            BorderColor::all(theme::RULE_DIM),
                            children![theme::figure((bar + 1).to_string(), 12.0, theme::TEXT_SOFT)],
                        ));
                        column
                            .spawn((
                                Node {
                                    height: px(CHORD),
                                    flex_shrink: 0.0,
                                    padding: UiRect::left(px(10)),
                                    flex_direction: FlexDirection::Column,
                                    justify_content: JustifyContent::Center,
                                    row_gap: px(2),
                                    border: UiRect::bottom(px(1)),
                                    ..default()
                                },
                                BorderColor::all(theme::RULE_DIM),
                            ))
                            .with_children(|cell| {
                                let symbol = if chord == 0 {
                                    "C".to_string()
                                } else {
                                    chord.to_string()
                                };
                                cell.spawn(theme::strong(symbol, 22.0, theme::TEXT));
                                let (first, second) = (
                                    names::chord(piece, home, chord),
                                    names::chord(piece, arena, chord),
                                );
                                let name = if first == second {
                                    first
                                } else {
                                    format!("{first}, {second}")
                                };
                                cell.spawn(theme::strong(name, 14.0, theme::TEXT_SOFT));
                            });
                        // The statement that starts on this bar, over its notes.
                        let starts = section
                            .stating(piece, bar)
                            .filter(|(_, from, _)| *from == bar);
                        if let Some((at, _, bars)) = starts {
                            let words = stated(&section.statements[at]);
                            column.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(10),
                                    top: px(TOP + CHORD + 6.0),
                                    width: px(bars as f32 * BAR - 14.0),
                                    overflow: Overflow::clip(),
                                    ..default()
                                },
                                children![theme::text(words, 12.0, theme::TEXT_SOFT)],
                            ));
                        }
                        if let Some(passage) = &passage {
                            let step = (BAR - 10.0) / STEPS as f32;
                            let notes = passage.melody[bar].iter().enumerate();
                            for (start, cell) in notes.filter(|(_, cell)| cell.degree != 0) {
                                let low = TOP + CHORD + MELODY - 12.0;
                                column.spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        left: px((5.0 + start as f32 * step).round()),
                                        top: px(low - (cell.degree.min(9) - 1) as f32 * 5.5),
                                        width: px((cell.steps as f32 * step - 1.0).round()),
                                        height: px(4),
                                        ..default()
                                    },
                                    BackgroundColor(theme::LINE),
                                ));
                            }
                        }
                        if picked {
                            column.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: px(0),
                                    right: px(0),
                                    top: px(TOP),
                                    bottom: px(0),
                                    border: UiRect::all(px(1)),
                                    ..default()
                                },
                                BackgroundColor(theme::ACCENT.with_alpha(0.16)),
                                BorderColor::all(theme::ACCENT),
                            ));
                        }
                    });
            }
            // The line that goes over the bars while the section plays.
            if let Some(place) = place {
                strip.spawn((
                    Playhead {
                        origin: GUTTER,
                        step: BAR / STEPS as f32,
                        over: Over::Section(place as u8),
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

/// The names of the columns of a table.
fn columns(parent: &mut ChildSpawnerCommands, names: impl IntoIterator<Item = (String, f32)>) {
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
            for (place, (name, width)) in names.into_iter().enumerate() {
                header.spawn((
                    Node {
                        width: px(width),
                        flex_shrink: 0.0,
                        padding: UiRect::left(px(if place == 0 { 14.0 } else { 0.0 })),
                        ..default()
                    },
                    children![theme::label(name, 11.0, theme::TEXT_DIM)],
                ));
            }
        });
}

/// A cell of the arrangement: what the file writes there, in a button that selects it.
fn cell(
    parent: &mut ChildSpawnerCommands,
    words: Option<String>,
    press: Press,
    picked: bool,
    (width, height): (f32, f32),
) {
    parent
        .spawn(Node {
            width: px(width),
            flex_shrink: 0.0,
            padding: UiRect::right(px(8)),
            ..default()
        })
        .with_children(|cell| {
            let silent = words.is_none();
            let (ink, edge, fill) = match (picked, silent) {
                (true, _) => (theme::TEXT, theme::ACCENT, theme::ACCENT.with_alpha(0.16)),
                (false, true) => (theme::TEXT_DIM, Color::NONE, Color::NONE),
                (false, false) => (theme::TEXT, theme::RULE, theme::SURFACE_RAISED),
            };
            let mut button = cell.spawn((
                Button,
                press,
                Look::new(fill, theme::SURFACE_HOVERED),
                BackgroundColor(fill),
                BorderColor::all(edge),
                Node {
                    width: percent(100),
                    height: px(height),
                    padding: UiRect::horizontal(px(5)),
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    overflow: Overflow::clip(),
                    ..default()
                },
            ));
            button.with_children(|button| {
                let words = words.unwrap_or("Silent".to_string());
                button.spawn(theme::written(words, 13.0, ink));
                if silent && !picked {
                    button.spawn(Dashed {
                        color: theme::RULE,
                        left: 0.0,
                        top: 0.0,
                        size: None,
                    });
                }
            });
        });
}

/// What the file writes for an instrument in a line of the arrangement.
fn written(doc: &Doc, row: Row, instrument: Instrument) -> Option<String> {
    let head = row.head();
    let line = line_of(doc, &[head[0].as_str(), head[1].as_str()])?;
    match PartText::of(line, instrument) {
        PartText::Silent => None,
        PartText::ByThreat => Some("* by threat".to_string()),
        part => part.write(),
    }
}

/// What each instrument plays in each scene.
fn arrangement(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View) {
    const SCENE: f32 = 170.0;
    const SECTIONS: f32 = 126.0;
    const PART: f32 = 121.0;
    let piece = doc.piece();
    let words = "What each instrument plays in each scene: a pattern or a line of the melody, its \
                 octave after @ and its level after a colon.";
    heading(parent, "Arrangement", words, true);
    let instruments =
        Instrument::ALL.map(|instrument| (names::instrument(instrument).to_string(), PART));
    let first = [
        ("Scene".to_string(), SCENE),
        ("Sections".to_string(), SECTIONS),
    ];
    columns(parent, first.into_iter().chain(instruments));
    for scene in Scene::ALL {
        parent
            .spawn((
                Node {
                    height: px(40),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(theme::RULE_DIM),
            ))
            .with_children(|row| {
                let mode = if scene == Scene::Arena {
                    piece.key.arena
                } else {
                    piece.key.home
                };
                row.spawn(Node {
                    width: px(SCENE),
                    flex_shrink: 0.0,
                    padding: UiRect::left(px(14)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|name| {
                    name.spawn(theme::text(names::scene(scene), 16.0, theme::TEXT));
                    let word = format!("play {}, in {}", scene.name(), capital(mode.name()));
                    name.spawn(theme::text(word, 11.0, theme::TEXT_DIM));
                });
                let sections = piece.play(scene).sections().iter();
                let sections: Vec<&str> = sections
                    .map(|&index| piece.section(index).name.as_str())
                    .collect();
                let picked = view.picked == Some(Picked::Sections(scene));
                let press = Press::Pick(Picked::Sections(scene));
                cell(
                    row,
                    Some(sections.join(", ")),
                    press,
                    picked,
                    (SECTIONS, 28.0),
                );
                for instrument in Instrument::ALL {
                    let part = Picked::Part(Row::Scene(scene), instrument);
                    let words = written(doc, Row::Scene(scene), instrument);
                    cell(
                        row,
                        words,
                        Press::Pick(part),
                        view.picked == Some(part),
                        (PART, 28.0),
                    );
                }
            });
    }
}

/// The instruments that a scene leaves to the threat, or that a line of a threat names.
fn left_to_threat(piece: &Piece) -> Vec<Instrument> {
    let left = |instrument: &Instrument| {
        let scenes = Scene::ALL.into_iter();
        let threats = 0..THREATS as u8;
        scenes
            .clone()
            .any(|scene| piece.play(scene).part(*instrument) == Part::ByThreat)
            || threats
                .clone()
                .any(|threat| piece.threat(threat, *instrument).is_some())
    };
    Instrument::ALL.into_iter().filter(left).collect()
}

/// What the instruments that a scene leaves to the threat play at each threat.
fn by_threat(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View) {
    const THREAT: f32 = 272.0;
    let piece = doc.piece();
    let words = "The highest brain tier awake sets what a scene leaves to the threat. Without a \
                 fight, the line of threat 0 plays.";
    heading(parent, "By threat", words, false);
    let instruments = left_to_threat(piece);
    if instruments.is_empty() {
        note(
            parent,
            "No scene leaves an instrument to the threat. A part that plays * by threat does.",
        );
        return;
    }
    let width = (750.0 / instruments.len() as f32).floor().min(250.0);
    let heads = instruments
        .iter()
        .map(|instrument| (names::instrument(*instrument).to_string(), width));
    columns(
        parent,
        [("Threat".to_string(), THREAT)].into_iter().chain(heads),
    );
    for threat in 0..THREATS as u8 {
        parent
            .spawn((
                Node {
                    height: px(36),
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
                        width: px(60),
                        flex_shrink: 0.0,
                        padding: UiRect::left(px(14)),
                        ..default()
                    },
                    children![theme::figure(threat.to_string(), 18.0, theme::TEXT)],
                ));
                row.spawn((
                    Node {
                        width: px(THREAT - 60.0),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    children![theme::text(names::brain(threat), 16.0, theme::TEXT_SOFT)],
                ));
                for &instrument in &instruments {
                    let part = Picked::Part(Row::Threat(threat), instrument);
                    let words = written(doc, Row::Threat(threat), instrument);
                    cell(
                        row,
                        words,
                        Press::Pick(part),
                        view.picked == Some(part),
                        (width, 26.0),
                    );
                }
            });
    }
}

/// A panel of the right column that is as high as what it holds.
fn compact() -> impl Bundle {
    panel(Node {
        flex_shrink: 0.0,
        ..default()
    })
}

pub fn side(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View, checks: &Checks) {
    let piece = doc.piece();
    let section = shown(doc, view);

    parent.spawn(compact()).with_children(|panel| {
        head(panel, "Section", |head| {
            if section.is_some() {
                let words = theme::label("Remove", 11.0, theme::TEXT_SOFT);
                key(
                    head,
                    words,
                    Press::RemoveSection,
                    Kind::Plain,
                    (Val::Auto, 24.0),
                );
            }
        });
        let Some(section) = &section else {
            note(panel, "The piece has no section yet.");
            return;
        };
        row(panel, "Name", 40.0, |row| {
            field(row, view, Field::SectionName, &section.name, 150.0);
        });
        row(panel, "Tempo", 40.0, |row| {
            let own = section.tempo.is_some();
            let words = |words: &str, kind: Kind| theme::text(words.to_string(), 14.0, kind.text());
            let kind = Kind::lit(!own);
            key(
                row,
                words("Of the piece", kind),
                Press::OwnTempo(false),
                kind,
                (Val::Auto, 28.0),
            );
            match section.tempo {
                Some(tempo) => stepper(
                    row,
                    tempo.to_string(),
                    Press::SectionTempo(-1),
                    Press::SectionTempo(1),
                    34.0,
                ),
                None => {
                    let kind = Kind::lit(false);
                    key(
                        row,
                        words("Its own", kind),
                        Press::OwnTempo(true),
                        kind,
                        (Val::Auto, 28.0),
                    );
                }
            }
        });
    });

    parent.spawn(compact()).with_children(|panel| {
        head(panel, "Selected bar", |head| {
            head.spawn(theme::figure((view.bar + 1).to_string(), 15.0, theme::TEXT));
        });
        let Some(section) = &section else {
            return;
        };
        row(panel, "Chord on", 36.0, |row| {
            row.spawn(Node::default()).with_children(|keys| {
                for chord in [1, 2, 3, 4, 5, 6, 7, 0] {
                    let kind = Kind::chosen(section.chords[view.bar] == chord);
                    let symbol = if chord == 0 {
                        "C".to_string()
                    } else {
                        chord.to_string()
                    };
                    let words = theme::strong(symbol, 15.0, kind.text());
                    key(keys, words, Press::Chord(chord), kind, (px(25), 28.0));
                }
            });
        });
        let Some((at, from, bars)) = section.stating(piece, view.bar) else {
            note(panel, "The melody doesn't reach this bar.");
            return;
        };
        let statement = section.statements[at];
        let over = match bars {
            1 => format!("Bar {}", from + 1),
            2 => format!("Bars {} and {}", from + 1, from + 2),
            _ => format!("Bars {} to {}", from + 1, from + bars),
        };
        row(panel, &over, 36.0, |row| {
            row.spawn(Node::default()).with_children(|keys| {
                for motif in piece.motifs() {
                    let kind = Kind::chosen(statement.letter == motif.letter);
                    let words =
                        theme::strong((motif.letter as char).to_string(), 15.0, kind.text());
                    key(
                        keys,
                        words,
                        Press::States(motif.letter),
                        kind,
                        (px(28), 28.0),
                    );
                }
            });
        });
        let motif = piece.motif(statement.letter);
        row(panel, "Ends", 36.0, |row| {
            let words = |words: &str, kind: Kind| theme::text(words.to_string(), 14.0, kind.text());
            let plain = statement.last.is_none() && !statement.turned;
            let kind = Kind::chosen(plain);
            key(
                row,
                words("As written", kind),
                Press::Ends(Ending::Written),
                kind,
                (Val::Auto, 28.0),
            );
            if motif.is_some_and(|motif| motif.turn.is_some()) {
                let kind = Kind::chosen(statement.turned);
                key(
                    row,
                    words("With its turn", kind),
                    Press::Ends(Ending::Turned),
                    kind,
                    (Val::Auto, 28.0),
                );
            }
        });
        row(panel, "Last note on", 36.0, |row| {
            // The degree that the file gives it, or the one that the motif ends on.
            let own = actions::last_degree(piece, statement.letter);
            let degree = statement
                .last
                .or(own)
                .map_or("-".to_string(), |degree| degree.to_string());
            let (lower, higher) = (
                Press::Ends(Ending::Degree(-1)),
                Press::Ends(Ending::Degree(1)),
            );
            stepper(row, degree, lower, higher, 30.0);
        });
    });

    parent.spawn(compact()).with_children(|panel| match view.picked {
        Some(Picked::Part(row, instrument)) => part(panel, doc, view, row, instrument),
        Some(Picked::Sections(scene)) => in_turn(panel, piece, scene),
        None => {
            head(panel, "Selected part", |_| {});
            let words = "Click a cell of the arrangement to change what an instrument plays, or \
                         the sections that a scene plays in turn.";
            note(panel, words);
        }
    });

    checks_of(
        parent,
        checks,
        Tab::Sections,
        Node {
            min_height: px(0),
            overflow: Overflow::clip(),
            ..grow()
        },
        false,
    );
}

/// The part that is selected: what the instrument plays in that line, its octave, its level
/// and its second line.
fn part(
    panel: &mut ChildSpawnerCommands,
    doc: &Doc,
    view: &View,
    line: Row,
    instrument: Instrument,
) {
    let piece = doc.piece();
    let heading = line.head();
    let found = line_of(doc, &[heading[0].as_str(), heading[1].as_str()]);
    let part = found.map_or(PartText::Silent, |line| PartText::of(line, instrument));
    head(panel, "Selected part", |head| {
        let of = format!("{} {}, {}", heading[0], heading[1], instrument.name());
        head.spawn(theme::written(of, 13.0, theme::TEXT_SOFT));
    });
    let (first, second, together) = match &part {
        PartText::Plays {
            first,
            second,
            together,
        } => (first, second, *together),
        other => {
            let words = if *other == PartText::ByThreat {
                "* by threat"
            } else {
                "silent"
            };
            row(panel, "Plays", 36.0, |row| {
                turner(row, words, Press::PartLine(-1), Press::PartLine(1), 110.0);
            });
            let words = match (other, line) {
                (PartText::ByThreat, _) => "The line of the threat says what it plays.",
                (_, Row::Scene(_)) => {
                    "The arrows turn through silence, the threat, its patterns and the lines of the melody."
                }
                (_, Row::Threat(_)) => {
                    "The arrows turn through silence, its patterns and the lines of the melody."
                }
            };
            note(panel, words);
            return;
        }
    };
    // A drum plays one line. The others can have a second one.
    let pitched = !instrument.is_drum();
    let voice: Option<&Voice> = if view.second && pitched {
        second.as_ref()
    } else {
        Some(first)
    };
    if pitched {
        row(panel, "Line", 36.0, |row| {
            let words = |words: &str, kind: Kind| theme::text(words.to_string(), 14.0, kind.text());
            let kind = Kind::chosen(!view.second);
            key(
                row,
                words("First", kind),
                Press::Second(false),
                kind,
                (Val::Auto, 28.0),
            );
            let kind = Kind::chosen(view.second);
            let name = match second {
                Some(second) => format!("Second: {}", second.name),
                None => "Second: none".to_string(),
            };
            key(
                row,
                words(&name, kind),
                Press::Second(true),
                kind,
                (Val::Auto, 28.0),
            );
        });
    }
    row(panel, "Plays", 36.0, |row| {
        let name = voice.map_or("nothing", |voice| voice.name.as_str());
        turner(row, name, Press::PartLine(-1), Press::PartLine(1), 110.0);
    });
    let Some(voice) = voice else {
        note(
            panel,
            "A second line sounds where the first one starts nothing.",
        );
        return;
    };
    if let Some(octave) = voice.octave {
        row(panel, "Octave, 1 to 5", 36.0, |row| {
            stepper(
                row,
                octave.to_string(),
                Press::PartOctave(-1),
                Press::PartOctave(1),
                34.0,
            );
        });
    }
    if voice.is_tune() {
        row(panel, "Level", 36.0, |row| {
            stepper(
                row,
                voice.loud().to_string(),
                Press::PartLevel(-1),
                Press::PartLevel(1),
                34.0,
            );
        });
    } else if let Some(rhythm) = piece.rhythm(instrument, &voice.name) {
        told(
            panel,
            "Level",
            format!("{}, set by its pattern", rhythm.level),
            36.0,
        );
    }
    if view.second && instrument == Instrument::Lead {
        row(panel, "Sounds", 36.0, |row| {
            let words = |words: &str, kind: Kind| theme::text(words.to_string(), 14.0, kind.text());
            let kind = Kind::chosen(!together);
            key(
                row,
                words("In the gaps", kind),
                Press::Together(false),
                kind,
                (Val::Auto, 28.0),
            );
            let kind = Kind::chosen(together);
            key(
                row,
                words("Together", kind),
                Press::Together(true),
                kind,
                (Val::Auto, 28.0),
            );
        });
    }
}

/// The sections that a scene plays in turn: four places at most.
fn in_turn(panel: &mut ChildSpawnerCommands, piece: &Piece, scene: Scene) {
    head(panel, "Sections in turn", |head| {
        head.spawn(theme::written(
            format!("play {}", scene.name()),
            13.0,
            theme::TEXT_SOFT,
        ));
    });
    let sections = piece.play(scene).sections();
    for (place, name) in ["First", "Second", "Third", "Fourth"]
        .into_iter()
        .enumerate()
    {
        let played = sections
            .get(place)
            .map(|&index| piece.section(index).name.as_str());
        // A place can be filled once the one before it is.
        if place > sections.len() {
            told(panel, name, "none", 36.0);
            continue;
        }
        row(panel, name, 36.0, |row| {
            let (last, next) = (Press::Turns(place, -1), Press::Turns(place, 1));
            turner(row, played.unwrap_or("none"), last, next, 110.0);
        });
    }
}
