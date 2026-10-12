//! The window: one sheet of 1600 by 900, with the piece at its top, seven tabs at its left,
//! what the tab shows in its middle, and the transport at its bottom.
//!
//! The sheet is drawn again from the document and the view whenever one of them changes.
//! Nothing of it holds a state of its own, so what is on screen is what is in the file.

mod file;
mod instruments;
mod motifs;
mod patterns;
mod piece;
mod sections;
mod transitions;
pub mod widgets;

use bevy::{
    camera::CameraUpdateSystems,
    input::{
        ButtonState,
        keyboard::{Key as Pressed, KeyboardInput},
    },
    prelude::*,
    ui::{DefaultUiCamera, UiSystems},
};
use tanku_audio::{Place, notation::Piece};

use crate::{
    actions::{self, Key},
    audition::{self, Audition},
    checks::{Checks, Item, Standing},
    doc::Doc,
    sound::Engine,
    state::{Press, Tab, View},
    theme,
};
use widgets::{Kind, button, choice, grow, head, key, mark, panel, strip};

/// The sheet that the window is drawn for, in its own pixels.
pub const SHEET: Vec2 = Vec2::new(1600.0, 900.0);
/// A window of another size shows the sheet larger or smaller, a step at a time: the engine
/// keeps every size of text that it ever drew.
const STEPS: f32 = 20.0;

/// The camera that draws the window.
#[derive(Component)]
pub struct MainCamera;

/// The sheet, whose content is drawn again after every change.
#[derive(Component)]
struct Sheet;

/// A bar of the meter: the level of the left or of the right.
#[derive(Component)]
struct Level(usize);

/// Where the music is, as bar, beat and step.
#[derive(Component)]
struct Clock;

/// The line that goes over a grid as the music plays it.
#[derive(Component, Clone, Copy)]
pub struct Playhead {
    /// The left of the first step, and how wide a step is.
    pub origin: f32,
    pub step: f32,
    pub over: Over,
}

/// What a grid shows of the piece, which tells where its playhead is.
#[derive(Clone, Copy, PartialEq)]
pub enum Over {
    /// A motif of so many bars, played alone.
    Motif(usize),
    /// One bar, whatever bar plays.
    Bar,
    /// The eight bars of a section, by its place among those of the piece.
    Section(u8),
}

pub fn plugin(app: &mut App) {
    app.add_plugins(widgets::plugin)
        .init_resource::<Checks>()
        .init_resource::<instruments::Sound>()
        .init_resource::<file::Scrolled>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                (press, keys),
                run_checks,
                play,
                instruments::listen,
                draw,
                (widgets::draw_dashes, widgets::draw_hatches),
                (show_sound, file::scroll),
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            fit.after(CameraUpdateSystems).before(UiSystems::Prepare),
        );
}

fn setup(mut commands: Commands) {
    commands.spawn((MainCamera, Camera2d));
    commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(theme::VOID),
        ))
        .with_children(|window| {
            window.spawn((
                Sheet,
                Node {
                    width: px(SHEET.x),
                    height: px(SHEET.y),
                    flex_shrink: 0.0,
                    padding: UiRect::all(px(28)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12),
                    ..default()
                },
            ));
        });
}

/// How much of its size the sheet keeps in a window of this many pixels.
pub fn scale(window: Vec2) -> f32 {
    let steps = (window / (SHEET / STEPS)).min_element().floor();
    steps.clamp(STEPS / 2.0, 2.0 * STEPS) / STEPS
}

/// Shows the sheet as large as the window lets it.
fn fit(shown_by: DefaultUiCamera, cameras: Query<&Camera>, mut ui_scale: ResMut<UiScale>) {
    let camera = shown_by.get().and_then(|camera| cameras.get(camera).ok());
    let wanted = camera
        .and_then(Camera::logical_viewport_size)
        .map_or(1.0, scale);
    if ui_scale.0 != wanted {
        ui_scale.0 = wanted;
    }
}

/// Seconds before a button that is held presses again, and between two of its presses then.
const HELD_AFTER: f32 = 0.4;
const HELD_EVERY: f32 = 0.05;

/// Does what a button asks when it is pressed, and again while it is held if it moves a
/// number.
fn press(
    pressed: Query<(&Interaction, &Press), Changed<Interaction>>,
    buttons: Query<(&Interaction, &Press)>,
    held: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut again: Local<Option<(Press, f32)>>,
    mut doc: ResMut<Doc>,
    mut view: ResMut<View>,
    mut clipboard: Option<ResMut<Clipboard>>,
) {
    let big = held.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let now = time.elapsed_secs();
    for (interaction, press) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if *press == Press::Copy {
            let text = doc.text();
            let copied = clipboard
                .as_deref_mut()
                .is_some_and(|clipboard| clipboard.set_text(text).is_ok());
            doc.refusal = (!copied).then(|| "the file was not copied: no clipboard".to_string());
        } else {
            actions::apply(press, big, &mut doc, &mut view);
        }
        *again = press.repeats().then(|| (press.clone(), now + HELD_AFTER));
    }
    if !mouse.pressed(MouseButton::Left) {
        *again = None;
    }
    let Some((press, next)) = &mut *again else {
        return;
    };
    // The sheet is drawn again after every change, so the button that is held is another
    // one each time: it is known by what it asks.
    let over = |(interaction, other): (&Interaction, &Press)| {
        *interaction != Interaction::None && other == press
    };
    if now >= *next && buttons.iter().any(over) {
        *next = now + HELD_EVERY;
        doc.joined = true;
        actions::apply(press, big, &mut doc, &mut view);
        doc.joined = false;
    }
}

/// Takes what is typed: into the name that is being written, or as a shortcut.
fn keys(
    mut typed: MessageReader<KeyboardInput>,
    held: Res<ButtonInput<KeyCode>>,
    mut doc: ResMut<Doc>,
    mut view: ResMut<View>,
) {
    let control = held.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let shift = held.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    for event in typed.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        if view.typing.is_some() {
            let key = match &event.logical_key {
                Pressed::Enter => Some(Key::Enter),
                Pressed::Escape => Some(Key::Escape),
                Pressed::Backspace => Some(Key::Back),
                _ => None,
            };
            let letters = event.text.as_deref().unwrap_or_default().chars();
            let keys = key.into_iter().chain(letters.map(Key::Letter));
            for key in keys.collect::<Vec<_>>() {
                actions::typed(&mut doc, &mut view, key);
            }
            continue;
        }
        let shortcut = match (&event.logical_key, control) {
            (Pressed::Space, false) if view.playing => Some(Press::Stop),
            (Pressed::Space, false) => Some(Press::Play),
            (Pressed::Character(letter), true) => match (letter.to_lowercase().as_str(), shift) {
                ("z", false) => Some(Press::Undo),
                ("z", true) | ("y", _) => Some(Press::Redo),
                ("s", _) => Some(Press::Save),
                _ => None,
            },
            _ => None,
        };
        if let Some(press) = shortcut {
            actions::apply(&press, shift, &mut doc, &mut view);
        }
    }
}

/// Keeps the checks about the state of the document that the window shows.
fn run_checks(doc: Res<Doc>, mut checks: ResMut<Checks>) {
    let told = checks.bypass_change_detection();
    if told.follow(doc.revision(), doc.score(), doc.saved_score()) {
        checks.set_changed();
    }
}

/// Keeps the sound on what the play button plays: it starts it, hands it each new state of
/// the piece, and tells it where the game is.
fn play(
    mut doc: ResMut<Doc>,
    mut view: ResMut<View>,
    mut engine: ResMut<Engine>,
    mut last: Local<Option<(u64, Audition)>>,
) {
    let audition = view.audition(doc.piece());
    let context = audition.context().unwrap_or(view.context());
    if !view.playing {
        if engine.playing() {
            engine.stop();
        }
        *last = None;
        return;
    }
    engine.bypass_change_detection().tell(context);
    let now = (doc.revision(), audition);
    if engine.playing() && last.as_ref() == Some(&now) {
        return;
    }
    match audition::score(doc.lines(), doc.piece(), &now.1) {
        // The same thing in another state goes on from where it is. Another thing starts.
        Ok(score) => match &*last {
            Some((_, playing)) if engine.playing() && *playing == now.1 => engine.rescore(score),
            _ => engine.play(score, context),
        },
        Err(why) => {
            doc.refusal = Some(why);
            view.playing = false;
            engine.stop();
            *last = None;
            return;
        }
    }
    *last = Some(now);
}

/// Draws the sheet again when the document, the view or the checks have changed.
fn draw(
    mut commands: Commands,
    sheet: Single<Entity, With<Sheet>>,
    doc: Res<Doc>,
    view: Res<View>,
    checks: Res<Checks>,
    sound: Res<instruments::Sound>,
) {
    if !(doc.is_changed() || view.is_changed() || checks.is_changed() || sound.is_changed()) {
        return;
    }
    let (doc, view, checks, sound) = (&*doc, &*view, &*checks, &*sound);
    let piece = doc.piece();
    commands
        .entity(*sheet)
        .despawn_related::<Children>()
        .with_children(|sheet| {
            // The fine line around the sheet, as around a drawing.
            sheet.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(16),
                    right: px(16),
                    top: px(16),
                    bottom: px(16),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(theme::RULE.with_alpha(0.35)),
            ));
            top_bar(sheet, doc, piece);
            sheet
                .spawn(Node {
                    column_gap: px(12),
                    min_height: px(0),
                    ..grow()
                })
                .with_children(|body| {
                    body.spawn(Node {
                        width: px(196),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        row_gap: px(12),
                        ..default()
                    })
                    .with_children(|left| {
                        tabs(left, view.tab);
                        left.spawn(panel(Node {
                            min_height: px(0),
                            ..grow()
                        }))
                        .with_children(|list| match view.tab {
                            Tab::Motifs => motifs::list(list, doc, view),
                            Tab::Patterns => patterns::list(list, doc, view),
                            Tab::Sections => sections::list(list, doc, view),
                            Tab::Transitions => transitions::list(list, doc, view),
                            Tab::Instruments => instruments::list(list, doc, view),
                            Tab::Piece => piece::list(list, doc, view),
                            Tab::File => file::list(list, doc, view),
                        });
                    });
                    body.spawn(panel(Node {
                        min_width: px(0),
                        ..grow()
                    }))
                    .with_children(|center| match view.tab {
                        Tab::Motifs => motifs::center(center, doc, view),
                        Tab::Patterns => patterns::center(center, doc, view),
                        Tab::Sections => sections::center(center, doc, view),
                        Tab::Transitions => transitions::center(center, doc, view),
                        Tab::Instruments => instruments::center(center, doc, view, sound),
                        Tab::Piece => piece::center(center, doc, view),
                        Tab::File => file::center(center, doc, view),
                    });
                    body.spawn(Node {
                        width: px(300),
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        row_gap: px(12),
                        ..default()
                    })
                    .with_children(|right| match view.tab {
                        Tab::Motifs => motifs::side(right, doc, view, checks),
                        Tab::Patterns => patterns::side(right, doc, view, checks),
                        Tab::Sections => sections::side(right, doc, view, checks),
                        Tab::Transitions => transitions::side(right, doc, view, checks),
                        Tab::Instruments => instruments::side(right, doc, view, checks),
                        Tab::Piece => piece::side(right, doc, view, checks),
                        Tab::File => file::side(right, doc, view, checks),
                    });
                });
            bottom_bar(sheet, view, piece);
            if let Some(why) = &doc.refusal {
                refusal(sheet, why);
            }
        });
}

/// One piece of the top bar: a name over a value.
fn told(parent: &mut ChildSpawnerCommands, name: &str, value: impl Bundle) {
    parent.spawn((
        Node {
            padding: UiRect::horizontal(px(16)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            row_gap: px(3),
            border: UiRect::right(px(1)),
            ..default()
        },
        BorderColor::all(theme::RULE),
        children![theme::label(name, 12.0, theme::TEXT_DIM), value],
    ));
}

/// The first letter of a word in a capital: the name of a mode, as the window writes it.
pub fn capital(word: &str) -> String {
    let mut letters = word.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

/// The piece at a glance, what is not saved, and the buttons that undo and save.
fn top_bar(parent: &mut ChildSpawnerCommands, doc: &Doc, piece: &Piece) {
    let file = doc
        .path
        .as_ref()
        .and_then(|path| path.file_name())
        .map_or("no file".to_string(), |name| {
            name.to_string_lossy().to_string()
        });
    parent
        .spawn(strip(Node {
            height: px(60),
            flex_shrink: 0.0,
            ..default()
        }))
        .with_children(|bar| {
            bar.spawn((
                Node {
                    width: px(196),
                    padding: UiRect::horizontal(px(14)),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    row_gap: px(2),
                    border: UiRect::right(px(1)),
                    ..default()
                },
                BorderColor::all(theme::RULE),
                children![
                    theme::label("Tanku", 12.0, theme::TEXT_DIM),
                    theme::title("Music editor", 20.0, theme::TEXT),
                ],
            ));
            told(bar, "File", theme::strong(file, 17.0, theme::TEXT));
            told(
                bar,
                "Tempo",
                theme::figure(piece.tempo.to_string(), 20.0, theme::TEXT),
            );
            told(bar, "Time", theme::figure("4/4", 20.0, theme::TEXT));
            let tonic = actions::TONICS[piece.key.tonic as usize % 12];
            told(bar, "Key", theme::strong(tonic, 20.0, theme::TEXT));
            told(
                bar,
                "Mode",
                theme::strong(capital(piece.key.home.name()), 18.0, theme::TEXT),
            );
            let arena = capital(piece.key.arena.name());
            told(bar, "In an arena", theme::strong(arena, 18.0, theme::TEXT));

            bar.spawn(Node {
                padding: UiRect::horizontal(px(16)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                column_gap: px(10),
                ..grow()
            })
            .with_children(|right| {
                let unsaved = doc.unsaved();
                if unsaved > 0 {
                    right.spawn((
                        Node {
                            width: px(10),
                            height: px(10),
                            ..default()
                        },
                        BackgroundColor(theme::UNSAVED),
                    ));
                    let changes = if unsaved == 1 { "change" } else { "changes" };
                    let words = format!("{unsaved} {changes} not saved");
                    right.spawn((
                        theme::label(words, 14.0, theme::TEXT_SOFT),
                        Node {
                            margin: UiRect::right(px(8)),
                            ..default()
                        },
                    ));
                }
                let dim = |can: bool| if can { Kind::Plain } else { Kind::Hollow };
                button(
                    right,
                    "Undo",
                    Press::Undo,
                    dim(doc.can_undo()),
                    (Val::Auto, 40.0),
                );
                button(
                    right,
                    "Redo",
                    Press::Redo,
                    dim(doc.can_redo()),
                    (Val::Auto, 40.0),
                );
                let save = if unsaved > 0 {
                    Kind::Accent
                } else {
                    Kind::Plain
                };
                button(right, "Save", Press::Save, save, (Val::Auto, 40.0));
            });
        });
}

/// The seven tabs.
fn tabs(parent: &mut ChildSpawnerCommands, shown: Tab) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|tabs| {
            for (place, tab) in Tab::ALL.into_iter().enumerate() {
                let kind = Kind::chosen(tab == shown);
                let number = if tab == shown {
                    kind.text()
                } else {
                    theme::TEXT_DIM
                };
                tabs.spawn((
                    widgets::pressed(Press::Tab(tab), kind),
                    Node {
                        height: px(44),
                        padding: UiRect::horizontal(px(14)),
                        align_items: AlignItems::Center,
                        column_gap: px(12),
                        border: UiRect::all(px(1)),
                        ..default()
                    },
                    children![
                        theme::figure(format!("{:02}", place + 1), 12.0, number),
                        theme::title(tab.name(), 15.0, kind.text()),
                    ],
                ));
            }
        });
}

/// A group of the bottom bar: a name over some buttons.
fn group(
    parent: &mut ChildSpawnerCommands,
    name: &str,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            ..default()
        })
        .with_children(|group| {
            group.spawn(theme::label(name, 11.0, theme::TEXT_DIM));
            content(group);
        });
}

/// The transport: what plays, where the game is, and where the music has got to.
fn bottom_bar(parent: &mut ChildSpawnerCommands, view: &View, piece: &Piece) {
    parent
        .spawn(strip(Node {
            height: px(64),
            flex_shrink: 0.0,
            align_items: AlignItems::Center,
            column_gap: px(18),
            padding: UiRect::horizontal(px(12)),
            ..default()
        }))
        .with_children(|bar| {
            bar.spawn(Node {
                column_gap: px(6),
                ..default()
            })
            .with_children(|buttons| {
                let playing = Kind::chosen(!view.playing);
                button(buttons, "Play", Press::Play, playing, (Val::Auto, 40.0));
                button(
                    buttons,
                    "Stop",
                    Press::Stop,
                    Kind::chosen(view.playing),
                    (Val::Auto, 40.0),
                );
            });
            group(bar, "Plays", |group| {
                let now = view.plays[view.tab as usize];
                let choices = View::choices(view.tab).iter().enumerate();
                let options =
                    choices.map(|(place, name)| (*name, Press::Plays(place), place == now));
                choice(group, options, false, 32.0);
            });
            // What is played alone is played in one place: the game has no say in it.
            let free = view.audition(piece).context().is_none();
            let lit = |on: bool| free && on;
            group(bar, "Place", |group| {
                let places = [
                    ("Menu", Place::Menu),
                    ("Bay", Place::Bay),
                    ("Corridor", Place::Corridor),
                    ("Arena", Place::Arena),
                ];
                let options = places
                    .map(|(name, place)| (name, Press::Place(place), lit(view.place == place)));
                choice(group, options, true, 32.0);
            });
            group(bar, "Fight", |group| {
                let options = [
                    ("Off", Press::Fight(false), lit(!view.fight)),
                    ("On", Press::Fight(true), lit(view.fight)),
                ];
                choice(group, options, true, 32.0);
            });
            group(bar, "Threat", |group| {
                group.spawn(Node::default()).with_children(|row| {
                    for threat in 0..5 {
                        let kind = Kind::lit(lit(view.threat == threat));
                        let figure = theme::figure(threat.to_string(), 15.0, kind.text());
                        key(row, figure, Press::Threat(threat), kind, (px(34), 32.0));
                    }
                });
            });

            bar.spawn(Node {
                margin: UiRect::left(Val::Auto),
                align_items: AlignItems::Center,
                column_gap: px(16),
                ..default()
            })
            .with_children(|right| {
                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::FlexEnd,
                        row_gap: px(4),
                        ..default()
                    })
                    .with_children(|clock| {
                        clock.spawn(theme::label("Bar . beat . step", 11.0, theme::TEXT_DIM));
                        clock.spawn((Clock, theme::figure("-.-.-", 22.0, theme::TEXT)));
                    });
                right
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        ..default()
                    })
                    .with_children(|meter| {
                        for side in 0..2 {
                            meter.spawn((
                                Node {
                                    width: px(120),
                                    height: px(8),
                                    border: UiRect::all(px(1)),
                                    ..default()
                                },
                                BorderColor::all(theme::RULE),
                                children![(
                                    Level(side),
                                    Node {
                                        width: percent(0),
                                        height: percent(100),
                                        ..default()
                                    },
                                    BackgroundColor(theme::TEXT),
                                )],
                            ));
                        }
                    });
            });
        });
}

/// Why the last change was not made, over the foot of the sheet until the next press.
fn refusal(parent: &mut ChildSpawnerCommands, why: &str) {
    parent.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(248),
            right: px(352),
            bottom: px(116),
            padding: UiRect::axes(px(14), px(10)),
            border: UiRect::all(px(1)),
            column_gap: px(10),
            ..default()
        },
        BackgroundColor(theme::VOID),
        BorderColor::all(theme::DANGER),
        GlobalZIndex(10),
        children![
            (
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
                theme::title("Not done", 13.0, theme::DANGER),
            ),
            theme::wrapped(why, 15.0, theme::TEXT),
        ],
    ));
}

/// The panel of the checks of a tab: how many hold, then each with what it promises, or with
/// what it found.
pub fn checks_panel(parent: &mut ChildSpawnerCommands, checks: &Checks, tab: Tab, node: Node) {
    checks_of(parent, checks, tab, node, true);
}

/// The same, with or without what each check promises: a tab whose column is full only
/// names them.
pub fn checks_of(
    parent: &mut ChildSpawnerCommands,
    checks: &Checks,
    tab: Tab,
    node: Node,
    detailed: bool,
) {
    let items = Item::of(tab);
    parent.spawn(panel(node)).with_children(|panel| {
        let (hold, running) = checks.count(items);
        let all = hold == items.len();
        let color = if all {
            theme::GOOD_TEXT
        } else if running {
            theme::TEXT_SOFT
        } else {
            theme::DANGER
        };
        head(panel, "Checks", |head| {
            head.spawn(theme::written(
                format!("{hold} of {} hold", items.len()),
                13.0,
                color,
            ));
        });
        for item in items {
            check(panel, checks, *item, detailed);
        }
    });
}

/// One check: its mark, its name, and a sentence.
pub fn check(parent: &mut ChildSpawnerCommands, checks: &Checks, item: Item, detailed: bool) {
    let standing = checks.standing(item);
    let holds = match standing {
        Standing::Running => None,
        Standing::Holds => Some(true),
        Standing::Fails(_) => Some(false),
    };
    parent
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
                words.spawn(theme::text(item.title(), 16.0, theme::TEXT));
                match standing {
                    Standing::Fails(found) => {
                        for what in found.iter().take(2) {
                            words.spawn(theme::wrapped(what, 14.0, theme::TEXT));
                        }
                        if found.len() > 2 {
                            let more = format!("And {} more.", found.len() - 2);
                            words.spawn(theme::text(more, 14.0, theme::TEXT_SOFT));
                        }
                    }
                    _ if detailed => {
                        words.spawn(theme::wrapped(item.promise(), 14.0, theme::TEXT_SOFT));
                    }
                    _ => {}
                }
            });
        });
}

/// Shows where the music is and how loud: the meter, the clock, and the line over the grids.
fn show_sound(
    engine: Res<Engine>,
    mut levels: Query<(&Level, &mut Node), Without<Playhead>>,
    mut clocks: Query<&mut Text, With<Clock>>,
    mut playheads: Query<(&Playhead, &mut Node, &mut Visibility), Without<Level>>,
) {
    let heard = engine.heard();
    for (level, mut node) in &mut levels {
        // The meter is in decibels: its whole length is the last 48 of them.
        let peak = heard.level[level.0].max(1e-6);
        let shown = ((20.0 * peak.log10() + 48.0) / 48.0).clamp(0.0, 1.0);
        let width = percent((shown * 50.0).round() * 2.0);
        if node.width != width {
            node.width = width;
        }
    }
    let words = match heard.spot {
        Some(spot) => format!(
            "{}.{}.{}",
            spot.bar + 1,
            spot.step / 4 + 1,
            spot.step % 4 + 1
        ),
        None => "-.-.-".to_string(),
    };
    for mut text in &mut clocks {
        if text.0 != words {
            text.0 = words.clone();
        }
    }
    for (playhead, mut node, mut visibility) in &mut playheads {
        let step = heard
            .spot
            .filter(|spot| !spot.between)
            .and_then(|spot| match playhead.over {
                Over::Motif(bars) => Some((spot.bar as usize % bars) * 16 + spot.step as usize),
                Over::Bar => Some(spot.step as usize),
                Over::Section(section) => {
                    (spot.section == section).then_some(spot.bar as usize * 16 + spot.step as usize)
                }
            });
        let wanted = if step.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if let Some(step) = step {
            let left = px((playhead.origin + step as f32 * playhead.step).round());
            if node.left != left {
                node.left = left;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::time::TimeUpdateStrategy;

    use super::*;
    use crate::state::Field;

    const FILE: &str = include_str!("../../../tanku_audio/music.score");

    /// The document of the game with the two systems that take the mouse and the keys, in
    /// frames of a tenth of a second.
    fn app() -> App {
        let doc = Doc::of(FILE, None).unwrap();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                100,
            )))
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<KeyboardInput>()
            .insert_resource(View::of(doc.piece()))
            .insert_resource(doc)
            .add_systems(Update, (press, keys).chain());
        app
    }

    fn tempo(app: &App) -> u16 {
        app.world().resource::<Doc>().piece().tempo
    }

    /// The editor without a screen: it lays the sheet out as a window of this many pixels
    /// shows it, and draws nothing.
    fn window(pixels: UVec2) -> App {
        use bevy::{
            app::TerminalCtrlCHandlerPlugin,
            audio::AudioPlugin,
            camera::{ImageRenderTarget, RenderTarget},
            clipboard::ClipboardPlugin,
            log::LogPlugin,
            render::{RenderPlugin, render_resource::TextureFormat, settings::WgpuSettings},
            window::ExitCondition,
            winit::WinitPlugin,
        };

        let doc = Doc::of(FILE, None).unwrap();
        let mut app = App::new();
        let no_window = WindowPlugin {
            primary_window: None,
            exit_condition: ExitCondition::DontExit,
            ..default()
        };
        let no_renderer = RenderPlugin {
            render_creation: WgpuSettings {
                backends: None,
                ..default()
            }
            .into(),
            ..default()
        };
        app.add_plugins(
            DefaultPlugins
                .set(no_window)
                .set(no_renderer)
                .disable::<WinitPlugin>()
                // A test leaves the sound and the clipboard of the desktop alone.
                .disable::<AudioPlugin>()
                .disable::<ClipboardPlugin>()
                .disable::<LogPlugin>()
                .disable::<TerminalCtrlCHandlerPlugin>(),
        )
        .insert_resource(View::of(doc.piece()))
        .insert_resource(doc)
        .insert_resource(crate::sound::engine().0)
        .add_plugins((theme::plugin, plugin));
        // What `App::run` does before the first frame, then the frame that makes the camera.
        app.finish();
        app.cleanup();
        app.update();
        let image =
            Image::new_target_texture(pixels.x, pixels.y, TextureFormat::Rgba8UnormSrgb, None);
        let canvas = app.world_mut().resource_mut::<Assets<Image>>().add(image);
        let mut cameras = app.world_mut().query_filtered::<Entity, With<MainCamera>>();
        let camera = cameras.single(app.world()).unwrap();
        let target = RenderTarget::Image(ImageRenderTarget {
            handle: canvas,
            scale_factor: 1.0,
        });
        app.world_mut()
            .entity_mut(camera)
            .insert((target, IsDefaultUiCamera));
        frames(&mut app);
        app
    }

    /// The frames that it takes to draw the sheet again and to lay it out.
    fn frames(app: &mut App) {
        for _ in 0..3 {
            app.update();
        }
    }

    /// Does what a button of the window asks, and lays the sheet out again.
    fn pressing(app: &mut App, press: Press) {
        let world = app.world_mut();
        world.resource_scope(|world, mut doc: Mut<Doc>| {
            let mut view = world.resource_mut::<View>();
            actions::apply(&press, false, &mut doc, &mut view);
        });
        frames(app);
    }

    /// Where a node is, in the pixels of the window: its left, its top, its right and its
    /// bottom.
    fn place(node: &ComputedNode, at: &UiGlobalTransform) -> [f32; 4] {
        let (half, center) = (node.size / 2.0, at.translation);
        [
            center.x - half.x,
            center.y - half.y,
            center.x + half.x,
            center.y + half.y,
        ]
    }

    /// Fails if a button of the sheet crosses the fine line around it, or lies on another.
    fn check_the_sheet(app: &mut App, what: &str) {
        let world = app.world_mut();
        let scale = world.resource::<UiScale>().0;
        let mut sheets = world.query_filtered::<(&ComputedNode, &UiGlobalTransform), With<Sheet>>();
        let (node, at) = sheets.single(world).unwrap();
        let sheet = place(node, at);
        let line = 16.0 * scale - 0.5;
        let mut buttons =
            world.query_filtered::<(&ComputedNode, &UiGlobalTransform, &Press), With<Button>>();
        let buttons: Vec<([f32; 4], Press)> = buttons
            .iter(world)
            .map(|(node, at, press)| (place(node, at), press.clone()))
            .collect();
        assert!(buttons.len() > 20, "{what}: only {} buttons", buttons.len());
        // The sheet is as large as the window shows it, and the transport is at its foot.
        let size = Vec2::new(sheet[2] - sheet[0], sheet[3] - sheet[1]);
        assert!(
            (size - SHEET * scale).abs().max_element() < 1.0,
            "{what}: a sheet of {size}"
        );
        let play = buttons.iter().find(|(_, press)| *press == Press::Play);
        let play = play.unwrap_or_else(|| panic!("{what}: no button plays")).0;
        assert!(
            play[1] > sheet[1] + 0.88 * size.y && play[0] < sheet[0] + 0.05 * size.x,
            "{what}: {play:?}"
        );
        for (button, press) in &buttons {
            let [left, top, right, bottom] = *button;
            assert!(
                right - left > 4.0 && bottom - top > 4.0,
                "{what}: {press:?} has no size"
            );
            let inside = left >= sheet[0] + line
                && top >= sheet[1] + line
                && right <= sheet[2] - line
                && bottom <= sheet[3] - line;
            assert!(
                inside,
                "{what}: {press:?} at {button:?} crosses the line of the sheet {sheet:?}"
            );
        }
        for (place, (one, press)) in buttons.iter().enumerate() {
            for (other, pressed) in &buttons[place + 1..] {
                // Two buttons side by side share an edge, and no more.
                let apart = one[2] <= other[0] + 1.0
                    || other[2] <= one[0] + 1.0
                    || one[3] <= other[1] + 1.0
                    || other[3] <= one[1] + 1.0;
                assert!(
                    apart,
                    "{what}: {press:?} at {one:?} lies on {pressed:?} at {other:?}"
                );
            }
        }
    }

    #[test]
    fn every_tab_is_laid_out_inside_the_sheet() {
        use tanku_audio::notation::{Instrument, Scene};

        use crate::state::{Picked, Row, Way};

        for pixels in [
            UVec2::new(1600, 900),
            UVec2::new(1280, 720),
            UVec2::new(1920, 1080),
        ] {
            let mut app = window(pixels);
            let look = |app: &mut App, press: Press| {
                let what = format!("{pixels} after {press:?}");
                pressing(app, press);
                check_the_sheet(app, &what);
            };
            // The motifs, with a note selected, then the turn of one.
            look(&mut app, Press::Cell(0, 0, 1));
            look(&mut app, Press::Motif(b'C'));
            look(&mut app, Press::Motif(b'A'));
            look(&mut app, Press::Turn(true));
            // The patterns: a drum, the bass with a degree for a brush, and a name that is
            // typed. One more pattern makes the rows lower.
            look(&mut app, Press::Tab(Tab::Patterns));
            look(&mut app, Press::Pattern(Instrument::Bass, "leaping".into()));
            look(&mut app, Press::Brush(b'5'));
            look(&mut app, Press::Pattern(Instrument::Pluck, "tick".into()));
            look(&mut app, Press::Type(Field::PatternName));
            look(&mut app, Press::NewPattern);
            // The sections: a part with its second line, the lead at the highest threat, the
            // sections of a scene, and a tempo of its own.
            look(&mut app, Press::Tab(Tab::Sections));
            look(&mut app, Press::Section("bridge".into()));
            look(
                &mut app,
                Press::Pick(Picked::Part(Row::Scene(Scene::Calm), Instrument::Pluck)),
            );
            look(&mut app, Press::Second(true));
            look(
                &mut app,
                Press::Pick(Picked::Part(Row::Threat(4), Instrument::Lead)),
            );
            look(&mut app, Press::Second(true));
            look(
                &mut app,
                Press::Pick(Picked::Part(Row::Scene(Scene::Menu), Instrument::Kick)),
            );
            look(&mut app, Press::Pick(Picked::Sections(Scene::Corridor)));
            let bass = Picked::Part(Row::Scene(Scene::Corridor), Instrument::Bass);
            look(&mut app, Press::Pick(bass));
            look(&mut app, Press::Section("verse".into()));
            look(&mut app, Press::OwnTempo(true));
            look(&mut app, Press::NewSection);
            // The two transitions, every instrument, the piece and the file.
            look(&mut app, Press::Tab(Tab::Transitions));
            look(&mut app, Press::Way(Way::Roll));
            look(&mut app, Press::WayNumber(1, 1));
            look(&mut app, Press::Way(Way::Link));
            look(&mut app, Press::WayNumber(1, 1));
            look(&mut app, Press::Tab(Tab::Instruments));
            for instrument in Instrument::ALL {
                look(&mut app, Press::Instrument(instrument));
            }
            // An instrument that the mix silences plays in no scene: there is nothing to draw.
            look(&mut app, Press::Instrument(Instrument::Hat));
            look(&mut app, Press::Mix(0, -100));
            look(&mut app, Press::Mix(1, -100));
            look(&mut app, Press::Tab(Tab::Piece));
            look(&mut app, Press::Tempo(1));
            look(&mut app, Press::Tab(Tab::File));
            // Nothing was refused on the way.
            assert_eq!(app.world().resource::<Doc>().refusal, None);
            // A change that the reader refuses is told over the foot of the sheet.
            look(&mut app, Press::Tab(Tab::Motifs));
            look(&mut app, Press::RemoveMotif);
            assert!(app.world().resource::<Doc>().refusal.is_some());
        }
    }

    /// Presses a key, with the text that it types if it types some.
    fn key(app: &mut App, logical_key: Pressed, text: Option<&str>) {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::F24,
            logical_key,
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }

    fn letter(app: &mut App, letter: &str) {
        key(app, Pressed::Character(letter.into()), Some(letter));
    }

    #[test]
    fn a_button_that_is_held_presses_again() {
        let mut app = app();
        let button = app
            .world_mut()
            .spawn((Interaction::Pressed, Press::Tempo(1)))
            .id();
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.press(MouseButton::Left);
        app.update();
        assert_eq!(tempo(&app), 121);
        // Nothing more for the first four tenths of a second, then a step on every frame.
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(tempo(&app), 121);
        for _ in 0..4 {
            app.update();
        }
        let held = tempo(&app);
        assert!((123..=125).contains(&held), "{held}");
        // A cursor that left the button stops it, and so does a button that is let go.
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::None;
        app.update();
        assert_eq!(tempo(&app), held);
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Hovered;
        app.update();
        assert_eq!(tempo(&app), held + 1);
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.release(MouseButton::Left);
        app.update();
        app.update();
        assert_eq!(tempo(&app), held + 1);
        // All of it was one change.
        let mut doc = app.world_mut().resource_mut::<Doc>();
        assert!(doc.undo() && !doc.can_undo());
        assert_eq!(doc.text(), FILE);

        // A button that selects something is pressed once, however long it is held.
        let mut app = self::app();
        app.world_mut()
            .spawn((Interaction::Pressed, Press::Tab(Tab::Piece)));
        let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse.press(MouseButton::Left);
        for _ in 0..8 {
            app.update();
        }
        assert_eq!(app.world().resource::<View>().tab, Tab::Piece);
        assert_eq!(app.world().resource::<Doc>().revision(), 0);
    }

    #[test]
    fn the_keys_are_shortcuts_or_the_letters_of_a_name() {
        let mut app = app();
        key(&mut app, Pressed::Space, Some(" "));
        assert!(app.world().resource::<View>().playing);
        key(&mut app, Pressed::Space, Some(" "));
        assert!(!app.world().resource::<View>().playing);

        // Control with Z undoes, with Y or with Shift and Z does again, and a letter alone
        // does nothing.
        app.world_mut()
            .resource_mut::<Doc>()
            .set_word(&["tempo"], 1, "96");
        letter(&mut app, "z");
        assert_eq!(tempo(&app), 96);
        let mut held = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        held.press(KeyCode::ControlLeft);
        letter(&mut app, "z");
        assert_eq!(tempo(&app), 120);
        letter(&mut app, "y");
        assert_eq!(tempo(&app), 96);
        letter(&mut app, "z");
        let mut held = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        held.press(KeyCode::ShiftLeft);
        letter(&mut app, "Z");
        assert_eq!(tempo(&app), 96);
        // This document has no file: saving says so.
        letter(&mut app, "s");
        let refusal = app.world().resource::<Doc>().refusal.clone();
        assert_eq!(
            refusal.as_deref(),
            Some("this document has no file to be saved in")
        );
        let mut held = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        held.release_all();

        // While a name is typed, the keys write it: the space bar plays nothing, and Enter
        // gives the section its name wherever the file has it.
        app.world_mut()
            .spawn((Interaction::Pressed, Press::Type(Field::SectionName)));
        app.update();
        for _ in 0..5 {
            key(&mut app, Pressed::Backspace, None);
        }
        for typed in ["O", "p", " ", "e", "n"] {
            letter(&mut app, typed);
        }
        key(&mut app, Pressed::Space, Some(" "));
        let view = app.world().resource::<View>();
        assert_eq!(
            view.typing.as_ref().map(|typing| typing.text.as_str()),
            Some("open")
        );
        assert!(!view.playing);
        key(&mut app, Pressed::Enter, None);
        let (view, doc) = (
            app.world().resource::<View>(),
            app.world().resource::<Doc>(),
        );
        assert_eq!(
            (view.typing.as_ref(), view.section.as_str()),
            (None, "open")
        );
        assert!(doc.piece().passage("open").is_some() && doc.piece().passage("intro").is_none());
        assert!(doc.lines()[29].starts_with("play menu     open "));
    }

    #[test]
    fn the_sheet_follows_the_window_a_step_at_a_time() {
        assert_eq!(scale(SHEET), 1.0);
        assert_eq!(scale(Vec2::new(1920.0, 1080.0)), 1.2);
        assert_eq!(scale(Vec2::new(1908.0, 1000.0)), 1.1);
        assert_eq!(scale(Vec2::new(1280.0, 720.0)), 0.8);
        // Never so small that nothing can be read, and never larger than twice its size.
        assert_eq!(scale(Vec2::new(320.0, 200.0)), 0.5);
        assert_eq!(scale(Vec2::new(6000.0, 4000.0)), 2.0);
    }
}
