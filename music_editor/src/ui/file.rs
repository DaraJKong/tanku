//! The tab of the file: the lines as they will be saved, with what each part of them means,
//! what is not saved yet, and the checks that say whether it should be.

use bevy::prelude::*;

use super::{
    check,
    widgets::{Kind, Scrolls, button, grow, head, mark, note, panel, scrolling, told},
};
use crate::{
    checks::{Checks, Item, Standing},
    doc::Doc,
    lines::{self, PARTS},
    state::{Press, Tab, View},
    theme,
};

/// The height of a line of the listing, and the width of its part of the sheet.
const LINE: f32 = 13.0;
const LISTING: f32 = 716.0;
/// The size of its characters, and the width of one in the face of the figures.
const SIZE: f32 = 12.0;
const CHARACTER: f32 = 0.54 * SIZE;

/// What the lines of each part mean, in the order of `PARTS`.
const NOTES: [&str; 8] = [
    "The format and its version, the tempo, the key and its two modes. A line that starts with # is a comment.",
    "A digit starts a note on that degree, a dot holds it, a dash is silence. A turn is the last bar of a section.",
    "Instrument, name, one bar, then the level of a hit and of a soft hit. x is a hit, o a soft one. On the bass \
     and the pluck they take the next note of the chord of the bar, R is its root, O that root an octave higher, \
     V its fifth and a digit a degree of the scale. A dot holds a note and a dash ends it. echo gives a pattern \
     of the pluck its echo.",
    "A chord per bar, C for the color chord. The melody: A, a with its turn, C:7 ending on 7. Then its own tempo.",
    "A scene, the sections that it plays in turn, then each instrument and its line: a pattern or a form of the \
     melody, @ its octave, : its level. / puts a line under another, + plays two together, * leaves the \
     instrument to the threat lines.",
    "Roll: beats, drum, drum in an arena, levels. Link: beats, degree, notes.",
    "How loud each instrument is on the left and on the right, in percent.",
    "The settings of each instrument by their names: times in ms, frequencies in Hz, levels in percent. How an \
     instrument is made stays in the code.",
];

pub fn list(parent: &mut ChildSpawnerCommands, doc: &Doc, _: &View) {
    head(parent, "In this file", |head| {
        head.spawn(theme::label("Lines", 11.0, theme::TEXT_DIM));
    });
    let parts = lines::parts(doc.lines());
    for (place, (name, _)) in PARTS.into_iter().enumerate() {
        let count = parts.iter().filter(|part| **part == Some(place)).count();
        parent
            .spawn((
                Node {
                    height: px(28),
                    flex_shrink: 0.0,
                    padding: UiRect::horizontal(px(12)),
                    align_items: AlignItems::Center,
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(theme::RULE_DIM),
            ))
            .with_children(|row| {
                row.spawn((theme::text(name, 15.0, theme::TEXT), grow()));
                row.spawn(theme::figure(count.to_string(), 13.0, theme::TEXT_SOFT));
            });
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
                "One record per line, so a change reads well in a diff.",
            )
        });
}

/// The name of the file of the document, and the folder that it is in.
fn place(doc: &Doc) -> (String, String) {
    let Some(path) = &doc.path else {
        return (
            "no file".to_string(),
            "Nowhere: it can't be saved".to_string(),
        );
    };
    let name = |part: Option<&std::ffi::OsStr>| part.map(|part| part.to_string_lossy().to_string());
    let file = name(path.file_name()).unwrap_or_default();
    let folder = name(path.parent().and_then(|folder| folder.file_name()));
    let inside = match folder {
        Some(folder) => format!("{folder}/{file}"),
        None => file.clone(),
    };
    (file, inside)
}

pub fn center(parent: &mut ChildSpawnerCommands, doc: &Doc, _: &View) {
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
            header.spawn(theme::title(place(doc).0, 17.0, theme::TEXT));
            let words = "What the editor saves, and all that the game is compiled with.";
            header.spawn((theme::text(words, 16.0, theme::TEXT_SOFT), grow()));
            button(header, "Copy", Press::Copy, Kind::Plain, (Val::Auto, 32.0));
        });

    let parts = lines::parts(doc.lines());
    scrolling(parent, Scrolls::Listing, theme::FLOOR, |listing| {
        for (place, ((name, _), meaning)) in PARTS.into_iter().zip(NOTES).enumerate() {
            let of_part = |at: &usize| parts[*at] == Some(place);
            let numbers: Vec<usize> = (0..doc.lines().len()).filter(of_part).collect();
            if numbers.is_empty() {
                continue;
            }
            listing
                .spawn((
                    Node {
                        flex_shrink: 0.0,
                        padding: UiRect::vertical(px(1)),
                        border: UiRect::bottom(px(1)),
                        ..default()
                    },
                    BorderColor::all(theme::RULE_DIM),
                ))
                .with_children(|part| {
                    part.spawn(Node {
                        width: px(LISTING),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::clip_x(),
                        ..default()
                    })
                    .with_children(|rows| {
                        for at in numbers {
                            line(rows, doc, at);
                        }
                    });
                    part.spawn((
                        Node {
                            padding: UiRect::horizontal(px(12)),
                            flex_direction: FlexDirection::Column,
                            border: UiRect::left(px(1)),
                            min_width: px(0),
                            ..grow()
                        },
                        BorderColor::all(theme::RULE_DIM),
                        children![
                            theme::title(name, 11.0, theme::TEXT),
                            theme::wrapped(meaning, 12.0, theme::TEXT_SOFT),
                        ],
                    ));
                });
        }
    });
}

/// One line of the listing: its number in the file, a mark if it is not saved, and the line
/// as it is written.
fn line(parent: &mut ChildSpawnerCommands, doc: &Doc, at: usize) {
    let text = &doc.lines()[at];
    let ink = if lines::is_record(text) {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    parent
        .spawn(Node {
            height: px(LINE),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node {
                    width: px(34),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::FlexEnd,
                    ..default()
                },
                children![theme::figure((at + 1).to_string(), 11.0, theme::TEXT_DIM)],
            ));
            row.spawn(Node {
                width: px(18),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|mark| {
                if doc.is_changed(at) {
                    mark.spawn((
                        Node {
                            width: px(7),
                            height: px(7),
                            ..default()
                        },
                        BackgroundColor(theme::UNSAVED),
                    ));
                }
            });
            if !lines::is_record(text) {
                row.spawn(theme::written(text.as_str(), SIZE, ink));
                return;
            }
            // Each word in its column, as an editor with characters of one width shows the
            // file: a bar in the face of the figures, and a name in the lettering.
            row.spawn(Node {
                height: percent(100),
                ..grow()
            })
            .with_children(|words| {
                for (column, word) in lines::columns(text) {
                    let node = Node {
                        position_type: PositionType::Absolute,
                        left: px((column as f32 * CHARACTER).round()),
                        top: px(-1),
                        ..default()
                    };
                    if lines::is_figures(word) {
                        words.spawn((theme::figure(word, SIZE, ink), node));
                    } else {
                        words.spawn((theme::written(word, SIZE, ink), node));
                    }
                }
            });
        });
}

/// What is not saved, a record at a time: what it is, and what happened to it.
fn unsaved(doc: &Doc) -> Vec<(String, String)> {
    let names = |lines: &[String]| -> Vec<String> {
        let records = lines.iter().filter(|line| lines::is_record(line));
        records.map(|line| lines::name(line)).collect()
    };
    let (before, now) = (names(doc.saved()), names(doc.lines()));
    let mut entries = Vec::new();
    for at in doc.changes().0 {
        let name = lines::name(&doc.lines()[at]);
        let what = if before.contains(&name) {
            "Changed"
        } else {
            "New"
        };
        entries.push((name, format!("{what}, line {}.", at + 1)));
    }
    for name in before.into_iter().filter(|name| !now.contains(name)) {
        entries.push((name, "Removed.".to_string()));
    }
    entries
}

pub fn side(parent: &mut ChildSpawnerCommands, doc: &Doc, _: &View, checks: &Checks) {
    let compact = || {
        panel(Node {
            flex_shrink: 0.0,
            ..default()
        })
    };
    parent.spawn(compact()).with_children(|panel| {
        head(panel, "File", |_| {});
        told(panel, "Where", place(doc).1, 38.0);
        let version = doc.record(&["tanku-music"]).and_then(|words| words.get(1).map(|word| word.to_string()));
        told(panel, "Format", format!("Text, version {}", version.unwrap_or_default()), 38.0);
        let bytes = doc.text().len();
        let size = match bytes {
            0..1000 => format!("{bytes} bytes"),
            _ => format!("{} kB", lines::number((bytes as f32 / 100.0).round() / 10.0)),
        };
        told(panel, "Size", format!("{} lines, {size}", doc.lines().len()), 38.0);
        let game = doc.path.as_ref().is_some_and(|path| path.ends_with("tanku_audio/music.score"));
        let words = if game {
            "The game reads this file while it is compiled. A file that does not read stops the build, and \
             the error says on which line."
        } else {
            "This is not the file of the game, which is compiled with tanku_audio/music.score."
        };
        note(panel, words);
    });

    let entries = unsaved(doc);
    parent.spawn(compact()).with_children(|panel| {
        head(panel, "Not saved", |head| {
            let count = doc.unsaved();
            if count > 0 {
                head.spawn((
                    Node {
                        width: px(10),
                        height: px(10),
                        ..default()
                    },
                    BackgroundColor(theme::UNSAVED),
                ));
            }
            head.spawn(theme::figure(count.to_string(), 14.0, theme::TEXT));
        });
        for (name, what) in entries.iter().take(3) {
            panel.spawn((
                Node {
                    padding: UiRect::axes(px(12), px(6)),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(theme::RULE_DIM),
                children![
                    theme::written(name.as_str(), 15.0, theme::TEXT),
                    theme::text(what.as_str(), 13.0, theme::TEXT_SOFT),
                ],
            ));
        }
        match entries.len() {
            0 => note(
                panel,
                "The lines are those of the file as it was last saved.",
            ),
            1..=3 => {}
            more => note(panel, &format!("And {} more.", more - 3)),
        }
        panel
            .spawn(Node {
                padding: UiRect::axes(px(12), px(10)),
                column_gap: px(8),
                ..default()
            })
            .with_children(|buttons| {
                let changed = !entries.is_empty();
                let dim = if changed { Kind::Plain } else { Kind::Hollow };
                button(buttons, "Revert", Press::Revert, dim, (percent(50), 38.0));
                button(
                    buttons,
                    "Save",
                    Press::Save,
                    Kind::chosen(changed),
                    (percent(50), 38.0),
                );
            });
    });

    // Every check of the six tabs in one line, then the one that is about saving.
    let all = Item::of(Tab::File);
    let others: Vec<Item> = all
        .iter()
        .copied()
        .filter(|item| *item != Item::SameSound)
        .collect();
    let (hold, running) = checks.count(all);
    parent
        .spawn(panel(Node {
            min_height: px(0),
            overflow: Overflow::clip(),
            ..grow()
        }))
        .with_children(|panel| {
            let ink = if hold == all.len() {
                theme::GOOD_TEXT
            } else if running {
                theme::TEXT_SOFT
            } else {
                theme::DANGER
            };
            head(panel, "Before saving", |head| {
                head.spawn(theme::written(
                    format!("{hold} of {} hold", all.len()),
                    13.0,
                    ink,
                ));
            });
            let failing: Vec<&str> = others
                .iter()
                .filter(|item| matches!(checks.standing(**item), Standing::Fails(_)))
                .map(|item| item.title())
                .collect();
            let (held, waiting) = checks.count(&others);
            let holds = match (failing.is_empty(), waiting) {
                (false, _) => Some(false),
                (true, true) => None,
                (true, false) => Some(held == others.len()),
            };
            panel
                .spawn((
                    Node {
                        padding: UiRect::axes(px(12), px(8)),
                        column_gap: px(10),
                        border: UiRect::bottom(px(1)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BorderColor::all(theme::RULE_DIM),
                ))
                .with_children(|row| {
                    row.spawn(mark(holds));
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(2),
                        ..grow()
                    })
                    .with_children(|words| {
                        let (title, more) = match failing.len() {
                            0 if waiting => (
                                "The checks are running".to_string(),
                                "Those of all six tabs.".to_string(),
                            ),
                            0 => (
                                "Every check holds".to_string(),
                                "The checks of all six tabs.".to_string(),
                            ),
                            1 => ("One check fails".to_string(), format!("{}.", failing[0])),
                            count => (
                                format!("{count} checks fail"),
                                format!("{}.", failing.join(", ")),
                            ),
                        };
                        words.spawn(theme::text(title, 16.0, theme::TEXT));
                        words.spawn(theme::wrapped(more, 14.0, theme::TEXT_SOFT));
                    });
                });
            check(panel, checks, Item::SameSound, true);
        });
}
