//! The tab of the instruments: the numbers of the sound of each, which its `sound` line
//! writes, a drawing of what they give, and the mix.
//!
//! What an instrument is made of is in the code, and is only told here. The drawing is not a
//! sketch: it is the first bar that the instrument plays alone, as the game computes it.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use tanku_audio::{
    Context, checks,
    notation::{Instrument, Part, Piece, Scene},
    synth,
};

use super::{
    checks_panel,
    widgets::{Kind, grow, head, heading, key, list_row, note, panel, pressed, row, stepper, told},
};
use crate::{
    checks::Checks,
    doc::Doc,
    names,
    parts::{self, Dial},
    plot,
    state::{Press, Tab, View},
    theme,
};

/// The instruments in the order of the window: those that have a pitch, then the drums.
const ORDER: [Instrument; 6] = [
    Instrument::Bass,
    Instrument::Pluck,
    Instrument::Lead,
    Instrument::Kick,
    Instrument::Hat,
    Instrument::Snare,
];

/// What the code makes an instrument of: in a word, in a sentence, and as its parts.
const fn about(instrument: Instrument) -> (&'static str, &'static str, &'static str) {
    match instrument {
        Instrument::Kick => ("Sine", "A sine wave whose pitch falls.", "A sine wave"),
        Instrument::Hat => (
            "Noise",
            "Noise, of which only the top is kept.",
            "Noise through a filter",
        ),
        Instrument::Snare => (
            "Noise, tone",
            "A band of noise over a short tone.",
            "Noise through a filter, and a sine wave",
        ),
        Instrument::Bass => (
            "Square",
            "A square wave through a filter that closes.",
            "A square wave through a filter",
        ),
        Instrument::Pluck => (
            "String",
            "A plucked string, with an echo.",
            "A string of noise that rings",
        ),
        Instrument::Lead => (
            "Pulse",
            "A pulse wave with a slow vibrato.",
            "A pulse wave through a filter",
        ),
    }
}

/// What the selected instrument sounds like, and the notes that the instruments reach: both
/// are computed from the piece when it changes, while the tab is shown.
#[derive(Resource, Default)]
pub struct Sound {
    key: Option<(u64, Instrument, Context)>,
    image: Handle<Image>,
    /// The scene that the instrument was heard in, if it plays in one.
    scene: Option<String>,
    /// The lowest and the highest note of the bass, of the pluck and of the lead.
    ranges: [Option<(u8, u8)>; 3],
}

/// Listens to the selected instrument again when the piece, the instrument or the transport
/// has changed.
pub fn listen(
    doc: Res<Doc>,
    view: Res<View>,
    mut sound: ResMut<Sound>,
    mut images: ResMut<Assets<Image>>,
) {
    if view.tab != Tab::Instruments {
        return;
    }
    let key = (doc.revision(), view.instrument, view.context());
    if sound.key == Some(key) {
        return;
    }
    let heard = plot::heard(doc.lines(), doc.piece(), view.instrument, key.2);
    let size = Extent3d {
        width: plot::WIDTH as u32,
        height: plot::HEIGHT as u32,
        depth_or_array_layers: 1,
    };
    let image = Image::new(
        size,
        TextureDimension::D2,
        plot::picture(heard.as_ref()),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    // The ranges are those of the piece: the same until it changes.
    let same = sound.key.is_some_and(|(revision, ..)| revision == key.0);
    let ranges = if same {
        sound.ranges
    } else {
        checks::heard(doc.score())
    };
    *sound = Sound {
        key: Some(key),
        image: images.add(image),
        scene: heard.as_ref().map(plot::told),
        ranges,
    };
}

pub fn list(parent: &mut ChildSpawnerCommands, _: &Doc, view: &View) {
    head(parent, "Instruments", |_| {});
    for instrument in ORDER {
        let picked = view.instrument == instrument;
        let press = Press::Instrument(instrument);
        list_row(
            parent,
            names::instrument(instrument),
            about(instrument).0,
            press,
            picked,
            38.0,
        );
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
        .with_children(|foot| note(foot, "Computed by the game: no sound file ships."));
}

/// The line of the document that starts with these words, with one space between two words.
fn written(doc: &Doc, head: &[&str]) -> String {
    doc.record(head)
        .map_or(String::new(), |words| words.join(" "))
}

pub fn center(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View, sound: &Sound) {
    let piece = doc.piece();
    let instrument = view.instrument;
    let name = names::instrument(instrument);
    parent
        .spawn((
            Node {
                height: px(44),
                flex_shrink: 0.0,
                padding: UiRect::horizontal(px(14)),
                align_items: AlignItems::Center,
                column_gap: px(14),
                border: UiRect::bottom(px(1)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(theme::RULE),
        ))
        .with_children(|header| {
            header.spawn(theme::title(name, 17.0, theme::TEXT));
            header.spawn((
                theme::text(about(instrument).1, 16.0, theme::TEXT_SOFT),
                grow(),
            ));
            let line = written(doc, &["sound", instrument.name()]);
            header.spawn(theme::written(line, 14.0, theme::TEXT_SOFT));
        });

    drawing(parent, sound, name);
    dials(parent, doc, instrument);

    let words = "Level of each instrument on the left and on the right, in percent. The sum goes \
                 through a soft limit, so nothing here can clip.";
    heading(parent, "Mix", words, true);
    mix(parent, doc, piece, view, sound);
}

/// A word over the drawing.
fn over(parent: &mut ChildSpawnerCommands, (left, top): (f32, f32), words: impl Bundle) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            ..default()
        },
        children![words],
    ));
}

/// The drawing of the instrument: the start of its first note, and its first bar.
fn drawing(parent: &mut ChildSpawnerCommands, sound: &Sound, name: &str) {
    parent
        .spawn((
            Node {
                height: px(plot::HEIGHT as f32),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(theme::FLOOR),
        ))
        .with_children(|drawing| {
            drawing.spawn((
                ImageNode::new(sound.image.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    width: px(plot::WIDTH as f32),
                    height: px(plot::HEIGHT as f32),
                    ..default()
                },
            ));
            let (wave, bar) = (plot::WAVE as f32, plot::BAR as f32);
            over(
                drawing,
                (14.0, 10.0),
                theme::label("Wave", 12.0, theme::TEXT_DIM),
            );
            over(
                drawing,
                (bar - 34.0, 10.0),
                theme::label("One bar", 12.0, theme::TEXT_DIM),
            );
            let Some(scene) = &sound.scene else {
                let words = format!(
                    "The {} plays in no scene, so there is nothing to draw.",
                    name.to_lowercase()
                );
                over(
                    drawing,
                    (wave, 60.0),
                    theme::text(words, 15.0, theme::TEXT_SOFT),
                );
                return;
            };
            let first = format!("the first {} ms of its first note", plot::WAVE_MS);
            over(
                drawing,
                (70.0, 9.0),
                theme::text(first, 13.0, theme::TEXT_SOFT),
            );
            let alone = format!("alone, as in {scene}");
            over(
                drawing,
                (bar + 44.0, 9.0),
                theme::text(alone, 13.0, theme::TEXT_SOFT),
            );
            for beat in 0..4 {
                let left = bar + (beat * plot::SPAN / 4) as f32 + 4.0;
                let number = theme::figure((beat + 1).to_string(), 12.0, theme::TEXT_SOFT);
                over(drawing, (left, 176.0), number);
            }
        });
}

/// A tag for what the file doesn't hold.
fn in_the_code(parent: &mut ChildSpawnerCommands) {
    parent.spawn(theme::label("In the code", 10.0, theme::TEXT_DIM));
}

/// The numbers of the sound, in two columns: each with a bar that shows how far it is between
/// its bounds.
fn dials(parent: &mut ChildSpawnerCommands, doc: &Doc, instrument: Instrument) {
    let line = doc
        .find(&["sound", instrument.name()])
        .map(|at| doc.lines()[at].as_str());
    let all = parts::dials(line.unwrap_or_default(), instrument);
    let settings = parts::settings(instrument);
    // The last setting has the column at the right, and the others the one at the left.
    let (last, rest) = settings
        .split_last()
        .expect("every instrument has settings");
    let split = all.len().saturating_sub(last.1.len());
    let (left, right) = all.split_at(split);
    let names = |settings: &[(&str, &[parts::Knob])]| {
        let keys: Vec<&str> = settings.iter().map(|(key, _)| *key).collect();
        keys.join(", ")
    };
    parent
        .spawn((
            Node {
                flex_shrink: 0.0,
                border: UiRect::top(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE),
        ))
        .with_children(|columns| {
            let column = |columns: &mut ChildSpawnerCommands,
                          name: String,
                          dials: &[Dial],
                          first: bool| {
                columns
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            border: UiRect::right(px(if first { 1.0 } else { 0.0 })),
                            ..grow()
                        },
                        BorderColor::all(theme::RULE),
                    ))
                    .with_children(|column| {
                        column.spawn((
                            Node {
                                height: px(28),
                                padding: UiRect::left(px(14)),
                                align_items: AlignItems::Center,
                                border: UiRect::bottom(px(1)),
                                ..default()
                            },
                            BackgroundColor(theme::SURFACE_RAISED),
                            BorderColor::all(theme::RULE_DIM),
                            children![theme::label(name, 11.0, theme::TEXT_DIM)],
                        ));
                        let fixed = |column: &mut ChildSpawnerCommands,
                                     name: &str,
                                     value: String| {
                            column
                                .spawn((
                                    Node {
                                        height: px(40),
                                        padding: UiRect::horizontal(px(14)),
                                        align_items: AlignItems::Center,
                                        border: UiRect::bottom(px(1)),
                                        ..default()
                                    },
                                    BorderColor::all(theme::RULE_DIM),
                                ))
                                .with_children(|row| {
                                    row.spawn((
                                        Node {
                                            width: px(150),
                                            flex_shrink: 0.0,
                                            ..default()
                                        },
                                        children![theme::text(name, 16.0, theme::TEXT_SOFT)],
                                    ));
                                    row.spawn((theme::text(value, 15.0, theme::TEXT_SOFT), grow()));
                                    in_the_code(row);
                                });
                        };
                        if first {
                            fixed(column, "Made of", about(instrument).2.to_string());
                        }
                        for dial in dials {
                            self::dial(column, dial);
                        }
                        if !first {
                            fixed(
                                column,
                                "Notes at once",
                                synth::voices(instrument).to_string(),
                            );
                        }
                    });
            };
            column(columns, names(rest), left, true);
            column(columns, names(std::slice::from_ref(last)), right, false);
        });
}

/// One number of the sound: its name, where it is between its bounds, and its value between
/// the buttons that move it.
fn dial(parent: &mut ChildSpawnerCommands, dial: &Dial) {
    parent
        .spawn((
            Node {
                height: px(40),
                padding: UiRect::horizontal(px(14)),
                align_items: AlignItems::Center,
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    width: px(150),
                    flex_shrink: 0.0,
                    overflow: Overflow::clip(),
                    ..default()
                },
                children![theme::text(dial.knob.label, 16.0, theme::TEXT)],
            ));
            bar(row, parts::share(dial), grow());
            let sign = |sign: &str| theme::text(sign.to_string(), 18.0, theme::TEXT);
            row.spawn(Node {
                width: px(8),
                ..default()
            });
            key(
                row,
                sign("-"),
                Press::Setting(dial.word, -1),
                Kind::Plain,
                (px(32), 30.0),
            );
            row.spawn((
                Node {
                    width: px(74),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                children![theme::written(
                    parts::shown(dial.value, dial.knob.unit),
                    15.0,
                    theme::TEXT
                )],
            ));
            key(
                row,
                sign("+"),
                Press::Setting(dial.word, 1),
                Kind::Plain,
                (px(32), 30.0),
            );
        });
}

/// A bar that is full for a share of its length.
fn bar(parent: &mut ChildSpawnerCommands, share: f32, node: Node) {
    parent.spawn((
        Node {
            height: px(8),
            border: UiRect::all(px(1)),
            ..node
        },
        BorderColor::all(theme::RULE_DIM),
        children![(
            Node {
                width: percent((share.clamp(0.0, 1.0) * 100.0).round()),
                height: percent(100),
                ..default()
            },
            BackgroundColor(theme::LINE),
        )],
    ));
}

/// The range of an instrument that has a pitch, as its lowest and its highest note.
fn range((lowest, highest): (u8, u8)) -> String {
    format!("{} to {}", checks::note(lowest), checks::note(highest))
}

/// The mix: the level of each instrument on each side, and what else places it.
fn mix(parent: &mut ChildSpawnerCommands, doc: &Doc, piece: &Piece, view: &View, sound: &Sound) {
    const NAME: f32 = 150.0;
    const SIDE: f32 = 222.0;
    const VOICES: f32 = 110.0;
    const RANGE: f32 = 140.0;
    parent
        .spawn((
            Node {
                height: px(26),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                border: UiRect::vertical(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE_DIM),
        ))
        .with_children(|header| {
            let columns = [
                ("Instrument", NAME),
                ("Left", SIDE),
                ("Right", SIDE),
                ("Notes at once", VOICES),
                ("Heard from", RANGE),
                ("Place in stereo", 150.0),
            ];
            for (place, (name, width)) in columns.into_iter().enumerate() {
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
    for instrument in ORDER {
        let picked = view.instrument == instrument;
        let kind = if picked { Kind::Picked } else { Kind::Row };
        let [left, right] = piece.mix(instrument);
        let words = doc.record(&["mix", instrument.name()]).unwrap_or_default();
        let level = |side: usize| {
            words
                .get(2 + side)
                .map_or(String::new(), |word| word.to_string())
        };
        parent
            .spawn((
                pressed(Press::Instrument(instrument), kind),
                Node {
                    height: px(31),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
            ))
            .with_children(|row| {
                let cell = |width: f32, left: f32| Node {
                    width: px(width),
                    flex_shrink: 0.0,
                    padding: UiRect::left(px(left)),
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    ..default()
                };
                row.spawn((
                    cell(NAME, 14.0),
                    children![theme::text(
                        names::instrument(instrument),
                        16.0,
                        theme::TEXT
                    )],
                ));
                for (side, share) in [left, right].into_iter().enumerate() {
                    row.spawn(cell(SIDE, 0.0)).with_children(|cell| {
                        let node = Node {
                            width: px(150),
                            flex_shrink: 0.0,
                            ..default()
                        };
                        bar(cell, share, node);
                        cell.spawn(theme::figure(level(side), 14.0, theme::TEXT));
                    });
                }
                let voices = synth::voices(instrument).to_string();
                row.spawn((
                    cell(VOICES, 0.0),
                    children![theme::figure(voices, 14.0, theme::TEXT_SOFT)],
                ));
                let pitched = checks::RANGES
                    .iter()
                    .position(|(other, ..)| *other == instrument);
                let heard = match pitched {
                    Some(place) => sound.ranges[place].map_or("Not heard".to_string(), range),
                    None => "No pitch".to_string(),
                };
                row.spawn((
                    cell(RANGE, 0.0),
                    children![theme::text(heard, 15.0, theme::TEXT_SOFT)],
                ));
                let place = if (left - right).abs() < 0.005 {
                    "Middle"
                } else if left > right {
                    "Left"
                } else {
                    "Right"
                };
                row.spawn((
                    cell(150.0, 0.0),
                    children![theme::text(place, 15.0, theme::TEXT_SOFT)],
                ));
            });
    }
}

/// The scenes in which an instrument plays, in a few words.
fn plays_in(piece: &Piece, instrument: Instrument) -> String {
    let plays = |scene: Scene| piece.play(scene).part(instrument) != Part::Silent;
    let mut scenes = Vec::new();
    for (scene, name) in [(Scene::Menu, "the menu"), (Scene::Calm, "the calm")] {
        if plays(scene) {
            scenes.push(name);
        }
    }
    match (plays(Scene::Corridor), plays(Scene::Arena)) {
        (true, true) => scenes.push("fights"),
        (true, false) => scenes.push("fights in a corridor"),
        (false, true) => scenes.push("fights in an arena"),
        (false, false) => {}
    }
    if scenes.is_empty() {
        return "Nowhere".to_string();
    }
    super::capital(&scenes.join(", "))
}

pub fn side(parent: &mut ChildSpawnerCommands, doc: &Doc, view: &View, checks: &Checks) {
    let piece = doc.piece();
    let instrument = view.instrument;
    parent
        .spawn(panel(Node {
            flex_shrink: 0.0,
            ..default()
        }))
        .with_children(|panel| {
            head(panel, names::instrument(instrument), |head| {
                head.spawn(theme::written(
                    written(doc, &["mix", instrument.name()]),
                    13.0,
                    theme::TEXT_SOFT,
                ));
            });
            let words = doc.record(&["mix", instrument.name()]).unwrap_or_default();
            for (side, name) in ["Level on the left", "Level on the right"]
                .into_iter()
                .enumerate()
            {
                let level = words
                    .get(2 + side)
                    .map_or(String::new(), |word| word.to_string());
                row(panel, name, 44.0, |row| {
                    stepper(row, level, Press::Mix(side, -1), Press::Mix(side, 1), 46.0);
                });
            }
            let allowed = checks::RANGES
                .iter()
                .find(|(other, ..)| *other == instrument);
            let allowed = match allowed {
                Some(&(_, lowest, highest)) => {
                    format!("{}, kept by a check", range((lowest, highest)))
                }
                None => "No pitch".to_string(),
            };
            told(panel, "Range", allowed, 40.0);
            told(panel, "Plays in", plays_in(piece, instrument), 40.0);
        });
    checks_panel(
        parent,
        checks,
        Tab::Instruments,
        Node {
            min_height: px(0),
            overflow: Overflow::clip(),
            ..grow()
        },
    );
}
