//! The heads-up display: rounds, mines, shield, sector and lives.
//!
//! It is laid out as the margin of a drawing: a table of the tank's weapons in one corner, the
//! title block of the sheet in the other, with a scale and a north arrow. What changes fastest
//! is also drawn on the ground under the player's tank, where the player is looking.

use bevy::prelude::*;
use tanku_core::{
    rules::{MAX_PLAYERS, REPAIR_TIME, SOLO_LIVES},
    seed::VERSION,
    sim::{
        Barrier, Enemy, Gun, Player, Playtest, Position, Run, Shield, ShieldState, SimEvent, Tank,
        TankCommand, Wreck,
    },
    stats::{Genome, Stat},
};

use crate::{
    Screen,
    camera::{MainCamera, View},
    crew::{LocalSeat, Own},
    draft::{Draft, LIFT},
    notices::Alarms,
    repair, scene,
    settings::{Action, Settings},
    tags, theme,
    unlocks::{self, Fair, Unlocks},
};

/// Seconds a message stays on screen.
const TOAST_TIME: f32 = 2.5;
/// The last seconds of a lull of arena mode that a message counts down. The first one is left
/// to the message that says that a group is cleared.
const LULL_COUNTED: u8 = 2;
/// Markers point to the last enemies of a sector when this many remain, or fewer.
const MARKED_ENEMIES: usize = 2;
/// Distance between the panels and the edge of the screen, and between that edge and the
/// fine line that frames the sheet.
const MARGIN: f32 = 48.0;
pub const SHEET_EDGE: f32 = 16.0;
/// Length that the scale shows, in meters.
const SCALE: f32 = 5.0;
/// The dial around the player's tank: its radius, and the width of its arc.
const DIAL_RADIUS: f32 = 1.75;
const DIAL_WIDTH: f32 = 0.16;
/// The arc of one round in degrees, the gap after it, and the steps in which it fills up
/// while the round is loaded.
const DIAL_ROUND: f32 = 11.0;
const DIAL_GAP: f32 = 3.0;
const DIAL_STEPS: u8 = 4;

/// A square that is lit or not: a round, a mine, a life, or the tank of the player of a seat.
#[derive(Component, Clone, Copy)]
enum Pip {
    Round(u8),
    Mine(u8),
    Life(u32),
    Mate(u8),
}

/// A bar that fills up.
#[derive(Component, Clone, Copy)]
enum Bar {
    Reload,
    Shield,
}

/// A text that follows the game.
#[derive(Component, Clone, Copy)]
enum Readout {
    /// Which player the weapons are those of.
    Player,
    /// How the game is played: alone, with others, or as a playtest.
    Mode,
    /// What the squares next to it count: the lives of a single player, or the team.
    Crew,
    /// What the number next to it counts: the sectors of the corridor, or the groups of
    /// arena mode.
    Unit,
    Sector,
    /// What stands for the lives when a playtest doesn't count them.
    Lives,
    Seed,
    /// Says that the run earns no achievement, when it earns none.
    Earning,
    /// How hard the run is.
    Difficulty,
    Shield,
    Hint,
    /// What to do about a wreck, among several players.
    Repair,
}

#[derive(Component)]
struct Toast(f32);

/// What the display tells the player in the middle of the screen: what to do about a wreck,
/// and the controls where a run starts.
#[derive(Component)]
struct Notes;

/// Where those notes stand: at the bottom of the screen, or under the messages at its top
/// in arena mode, where the tanks in reserve stand at the bottom.
fn notes(in_a_room: bool) -> Node {
    let (top, bottom) = if in_a_room {
        (px(MARGIN + 28.0), auto())
    } else {
        (auto(), px(MARGIN))
    };
    Node {
        position_type: PositionType::Absolute,
        top,
        bottom,
        width: percent(100),
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(10),
        ..default()
    }
}

/// Keeps the notes clear of the tanks in reserve.
fn seat_notes(run: Option<Res<Run>>, mut held: Single<&mut Node, With<Notes>>) {
    let wanted = notes(run.is_some_and(|run| run.arena));
    if (held.top, held.bottom) != (wanted.top, wanted.bottom) {
        (held.top, held.bottom) = (wanted.top, wanted.bottom);
    }
}

#[derive(Component)]
struct EnemyMarker(usize);

/// The number of the player of a seat, next to the square of their tank.
#[derive(Component)]
struct MateNumber(u8);

/// The fill of the square of the tank of a seat, which rises as its wreck is repaired, as its
/// circle is inked.
#[derive(Component)]
struct RepairFill(u8);

/// The line that strikes out the square of a player who left the game, for a moment before
/// it goes.
#[derive(Component)]
struct Strike(u8);

/// The bar under the name on the marker of the tank of a seat, which fills as its wreck is
/// repaired.
#[derive(Component)]
struct RepairBar(u8);

/// How far the repair of a wreck has got, from nothing to done.
fn repaired(wreck: Option<&Wreck>) -> f32 {
    wreck.map_or(0.0, |wreck| (wreck.repair / REPAIR_TIME).clamp(0.0, 1.0))
}

/// The tag that follows the tank of the player of a seat, among several players: their tanks
/// look alike, and the tag tells whose each one is.
#[derive(Component)]
struct PlayerTag(u8);

/// How far over the middle of a tank its tag stands, in meters of ground, and how wide the
/// tag is on screen.
const TAG_RISE: f32 = 1.9;
const TAG_WIDTH: f32 = 30.0;

/// The marker at the edge of the screen that points to the tank of the player of a seat while
/// it is out of view, among several players.
#[derive(Component)]
struct MateMarker(u8);

/// How far inside the edge of the screen the middle of a marker stands, in pixels: between the
/// line that frames the sheet and the panels, and clear of what is written at the bottom of
/// the screen.
const EDGE_MARGIN: f32 = 28.0;
/// The size of the marker of an enemy on screen: a square on its corner.
const ENEMY_MARKER: f32 = 16.0;
/// The size of the marker of a teammate on screen, and the least gap between two markers.
const MATE_MARKER: Vec2 = Vec2::new(TAG_WIDTH, 20.0);
/// The red ring that closes on the marker of a tank that was just destroyed: how far from the
/// marker it starts, and how long it takes to close.
const RING_REACH: f32 = 10.0;
const RING_TIME: f32 = 0.6;
const MARKER_GAP: f32 = 3.0;

/// How a player is named for short: by the number of their seat.
pub fn short_name(seat: u8) -> String {
    format!("P{}", seat + 1)
}

/// The bar of the scale, which is as long as [`SCALE`] meters of ground.
#[derive(Component)]
struct ScaleBar;

#[derive(Component)]
struct NorthArrow;

/// The dial drawn on the ground around the player's tank.
#[derive(Component)]
struct Dial;

/// A part of the dial: a step of the arc of a round, or the dot of a mine.
#[derive(Component, Clone, Copy)]
enum DialPart {
    Round { round: u8, step: u8 },
    Mine(u8),
}

#[derive(Resource)]
struct DialParts {
    step: Handle<Mesh>,
    dot: Handle<Mesh>,
    lit: Handle<StandardMaterial>,
    unlit: Handle<StandardMaterial>,
    loading: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(OnEnter(Screen::Playing), build)
        .add_systems(
            Update,
            (
                status,
                seat_notes,
                weapons,
                toasts,
                (mark_enemies, mark_mates).chain(),
                tag_players,
                instruments,
                (attach_dial, turn_dial, light_dial).chain(),
            )
                .run_if(in_state(Screen::Playing)),
        );
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let flat = |color: Color| StandardMaterial {
        base_color: color,
        alpha_mode: if color.alpha() < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        unlit: true,
        cull_mode: None,
        ..default()
    };
    let mut step = Draft::default();
    let share = DIAL_ROUND / DIAL_STEPS as f32 / 360.0;
    step.ring(
        Vec3::ZERO,
        (
            DIAL_RADIUS - DIAL_WIDTH / 2.0,
            DIAL_RADIUS + DIAL_WIDTH / 2.0,
        ),
        (1, share),
        Color::WHITE,
    );
    let mut dot = Draft::default();
    dot.disc(Vec3::X * DIAL_RADIUS, 0.1, Color::WHITE);
    commands.insert_resource(DialParts {
        step: meshes.add(step.build()),
        dot: meshes.add(dot.build()),
        lit: materials.add(flat(theme::TEXT)),
        unlit: materials.add(flat(theme::TEXT.with_alpha(0.2))),
        loading: materials.add(flat(theme::ACCENT)),
    });
}

/// A see-through sheet with a ruled edge, which holds its rows in a column.
fn panel(node: Node) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            border: UiRect::all(px(1)),
            ..node
        },
        BackgroundColor(theme::SURFACE),
        BorderColor::all(theme::RULE),
    )
}

/// A row of a panel, with a rule under it unless it is the last one.
fn row(last: bool) -> impl Bundle {
    (
        Node {
            column_gap: px(6),
            align_items: AlignItems::Center,
            padding: UiRect::axes(px(12), px(8)),
            border: if last {
                UiRect::ZERO
            } else {
                UiRect::bottom(px(1))
            },
            ..default()
        },
        BorderColor::all(theme::RULE_DIM),
    )
}

/// A cell of the title block, with a rule on its right unless it is the last one.
fn cell(grow: f32, last: bool) -> impl Bundle {
    (
        Node {
            flex_grow: grow,
            flex_basis: px(0),
            flex_direction: FlexDirection::Column,
            row_gap: px(4),
            padding: UiRect::axes(px(12), px(8)),
            border: if last {
                UiRect::ZERO
            } else {
                UiRect::right(px(1))
            },
            ..default()
        },
        BorderColor::all(theme::RULE),
    )
}

fn caption(content: &str) -> impl Bundle {
    theme::label(content, 12.0, theme::TEXT_DIM)
}

fn label(content: &str) -> impl Bundle {
    (
        Node {
            width: px(78),
            ..default()
        },
        children![theme::label(content, 13.0, theme::TEXT_DIM)],
    )
}

fn pip(pip: Pip) -> impl Bundle {
    (
        pip,
        Node {
            width: px(16),
            height: px(16),
            border: UiRect::all(px(1)),
            // Mines are round.
            border_radius: if matches!(pip, Pip::Mine(_)) {
                BorderRadius::MAX
            } else {
                BorderRadius::ZERO
            },
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor::all(theme::RULE),
    )
}

/// The square of the tank of the player of a seat, which bears their number: the tanks of
/// the players all have one color.
fn mate(seat: u8) -> impl Bundle {
    (
        Pip::Mate(seat),
        Node {
            width: px(20),
            height: px(18),
            border: UiRect::all(px(1)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor::all(theme::RULE),
        children![
            (
                RepairFill(seat),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    right: px(0),
                    bottom: px(0),
                    height: percent(0),
                    ..default()
                },
                BackgroundColor(theme::PLAYER.with_alpha(0.55)),
            ),
            (
                MateNumber(seat),
                theme::label((seat + 1).to_string(), 12.0, theme::TEXT_DIM),
            ),
            (
                Strike(seat),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(1),
                    right: px(1),
                    top: px(8),
                    height: px(1),
                    ..default()
                },
                BackgroundColor(theme::TEXT_DIM),
                Visibility::Hidden,
            ),
        ],
    )
}

fn bar(bar: Bar) -> impl Bundle {
    (
        Node {
            width: px(126),
            height: px(8),
            border: UiRect::all(px(1)),
            ..default()
        },
        BorderColor::all(theme::RULE),
        children![(
            bar,
            Node {
                width: percent(0),
                height: percent(100),
                ..default()
            },
            BackgroundColor(theme::ACCENT),
        )],
    )
}

/// The table of the player's weapons.
fn weapons_panel() -> impl Bundle {
    (
        panel(Node {
            position_type: PositionType::Absolute,
            left: px(MARGIN),
            bottom: px(MARGIN),
            ..default()
        }),
        children![
            (
                row(false),
                children![
                    (
                        Node {
                            width: px(12),
                            height: px(12),
                            margin: UiRect::right(px(4)),
                            ..default()
                        },
                        BackgroundColor(theme::ACCENT),
                    ),
                    (Readout::Player, theme::title("", 15.0, theme::TEXT)),
                ],
            ),
            (
                row(false),
                children![
                    label("ROUNDS"),
                    pip(Pip::Round(1)),
                    pip(Pip::Round(2)),
                    pip(Pip::Round(3)),
                    pip(Pip::Round(4)),
                    pip(Pip::Round(5)),
                    pip(Pip::Round(6)),
                ],
            ),
            (row(false), children![label("RELOAD"), bar(Bar::Reload)]),
            (
                row(false),
                children![
                    label("MINES"),
                    pip(Pip::Mine(1)),
                    pip(Pip::Mine(2)),
                    pip(Pip::Mine(3)),
                    pip(Pip::Mine(4)),
                ],
            ),
            (
                row(true),
                children![
                    label("SHIELD"),
                    bar(Bar::Shield),
                    (
                        Readout::Shield,
                        theme::label("", 13.0, theme::TEXT_DIM),
                        Node {
                            margin: UiRect::left(px(6)),
                            ..default()
                        },
                    ),
                ],
            ),
        ],
    )
}

/// The title block of the sheet: the game and its mode, the sector, the lives, and the seed
/// with its version.
fn title_block() -> impl Bundle {
    (
        panel(Node {
            width: px(430),
            ..default()
        }),
        children![
            (
                Node {
                    border: UiRect::bottom(px(1)),
                    ..default()
                },
                BorderColor::all(theme::RULE),
                children![
                    (
                        cell(1.3, false),
                        children![
                            caption("GAME"),
                            theme::title("TANKU", 26.0, theme::TEXT),
                            (Readout::Mode, theme::label("", 13.0, theme::ACCENT)),
                        ],
                    ),
                    (
                        cell(1.0, false),
                        children![
                            (Readout::Unit, caption("")),
                            (Readout::Sector, theme::figure("", 40.0, theme::TEXT)),
                        ],
                    ),
                    (
                        cell(1.0, true),
                        children![
                            (Readout::Crew, caption("")),
                            (
                                Node {
                                    column_gap: px(6),
                                    margin: UiRect::top(px(10)),
                                    ..default()
                                },
                                children![
                                    pip(Pip::Life(1)),
                                    pip(Pip::Life(2)),
                                    pip(Pip::Life(3)),
                                    mate(0),
                                    mate(1),
                                    mate(2),
                                    mate(3),
                                    (Readout::Lives, theme::label("", 13.0, theme::ACCENT)),
                                ],
                            ),
                        ],
                    ),
                ],
            ),
            (
                Node::default(),
                children![
                    (
                        cell(1.3, false),
                        children![
                            caption("SEED"),
                            (Readout::Seed, theme::code("", 16.0, theme::TEXT)),
                            (Readout::Earning, theme::label("", 11.0, theme::TEXT_DIM)),
                        ],
                    ),
                    (
                        cell(1.0, false),
                        children![
                            caption("DIFFICULTY"),
                            (Readout::Difficulty, theme::title("", 15.0, theme::TEXT)),
                        ],
                    ),
                    (
                        cell(1.0, true),
                        children![
                            caption("VERSION"),
                            theme::figure(VERSION, 15.0, theme::TEXT)
                        ],
                    ),
                ],
            ),
        ],
    )
}

/// The scale and the north arrow, as on the corner of a plan.
fn instruments_row() -> impl Bundle {
    let mark = |filled: bool| {
        (
            Node {
                flex_grow: 1.0,
                ..default()
            },
            BackgroundColor(if filled { theme::TEXT_DIM } else { Color::NONE }),
        )
    };
    (
        Node {
            column_gap: px(18),
            align_items: AlignItems::End,
            ..default()
        },
        children![
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    ..default()
                },
                children![
                    (
                        ScaleBar,
                        Node {
                            width: px(180),
                            height: px(8),
                            border: UiRect::all(px(1)),
                            ..default()
                        },
                        BorderColor::all(theme::TEXT_DIM),
                        children![mark(true), mark(false), mark(true), mark(false), mark(true)],
                    ),
                    (
                        Node {
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        },
                        children![
                            theme::figure("0", 12.0, theme::TEXT_DIM),
                            theme::figure("5 m", 12.0, theme::TEXT_DIM),
                        ],
                    ),
                ],
            ),
            theme::title("N", 13.0, theme::TEXT_DIM),
            // A needle from the middle of a ring to its edge, which points north.
            (
                NorthArrow,
                UiTransform::IDENTITY,
                Node {
                    width: px(44),
                    height: px(44),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::MAX,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BorderColor::all(theme::RULE),
                children![
                    (
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(4),
                            width: px(2),
                            height: px(18),
                            ..default()
                        },
                        BackgroundColor(theme::TEXT),
                    ),
                    (
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(1),
                            width: px(8),
                            height: px(8),
                            ..default()
                        },
                        BackgroundColor(theme::TEXT),
                        UiTransform::from_rotation(Rot2::degrees(45.0)),
                    ),
                ],
            ),
        ],
    )
}

fn build(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(Screen::Playing),
        Node {
            width: percent(100),
            height: percent(100),
            ..default()
        },
        // The display never takes a click away from the game.
        Pickable::IGNORE,
        children![
            // The fine line around the sheet.
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(SHEET_EDGE),
                    right: px(SHEET_EDGE),
                    top: px(SHEET_EDGE),
                    bottom: px(SHEET_EDGE),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BorderColor::all(theme::RULE.with_alpha(0.35)),
            ),
            (
                Node {
                    position_type: PositionType::Absolute,
                    top: px(MARGIN - 8.0),
                    width: percent(100),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                // Over the game, which may have a block or a crate there: it has a dark edge.
                children![theme::edged(
                    Toast(0.0),
                    theme::Face::Title,
                    "",
                    20.0,
                    theme::TEXT
                )],
            ),
            weapons_panel(),
            (
                Node {
                    position_type: PositionType::Absolute,
                    right: px(MARGIN),
                    bottom: px(MARGIN),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::End,
                    row_gap: px(16),
                    ..default()
                },
                children![instruments_row(), title_block()],
            ),
            (
                Notes,
                notes(false),
                children![
                    (Readout::Repair, theme::title("", 18.0, theme::TEXT)),
                    (Readout::Hint, theme::text("", 15.0, theme::TEXT_DIM)),
                ],
            ),
        ],
    ));
    for seat in 0..MAX_PLAYERS as u8 {
        commands.spawn((
            DespawnOnExit(Screen::Playing),
            PlayerTag(seat),
            Node {
                position_type: PositionType::Absolute,
                width: px(TAG_WIDTH),
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(0), px(1)),
                border: UiRect::all(px(1)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(theme::SURFACE),
            BorderColor::all(theme::PLAYER),
            Pickable::IGNORE,
            children![theme::label(short_name(seat), 12.0, theme::TEXT)],
        ));
    }
    for seat in 0..MAX_PLAYERS as u8 {
        let (fill, _) = mate_look(false);
        commands.spawn((
            DespawnOnExit(Screen::Playing),
            MateMarker(seat),
            Node {
                position_type: PositionType::Absolute,
                width: px(MATE_MARKER.x),
                height: px(MATE_MARKER.y),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(fill),
            BorderColor::all(theme::PLAYER),
            Outline::new(px(0), px(0), Color::NONE),
            Pickable::IGNORE,
            children![
                theme::label(short_name(seat), 12.0, theme::TEXT_ON_ACCENT),
                (
                    RepairBar(seat),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0),
                        bottom: px(0),
                        width: percent(0),
                        height: px(3),
                        ..default()
                    },
                    BackgroundColor(theme::PLAYER),
                ),
            ],
        ));
    }
    for index in 0..MARKED_ENEMIES {
        commands.spawn((
            DespawnOnExit(Screen::Playing),
            EnemyMarker(index),
            Node {
                position_type: PositionType::Absolute,
                width: px(ENEMY_MARKER),
                height: px(ENEMY_MARKER),
                border: UiRect::all(px(1)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(theme::DANGER),
            BorderColor::all(theme::TEXT),
            // A square on its corner: it points every way.
            UiTransform::from_rotation(Rot2::degrees(45.0)),
            Pickable::IGNORE,
        ));
    }
}

/// Lights a pip or puts it out.
fn light(on: bool, lit: Color, color: &mut BackgroundColor, border: &mut BorderColor) {
    (color.0, *border) = if on {
        (lit, BorderColor::all(lit))
    } else {
        (Color::NONE, BorderColor::all(theme::RULE))
    };
}

/// Lights a square in the colors of a loss, or puts it out, while it blinks for the loss.
fn alarm(on: bool, color: &mut BackgroundColor, border: &mut BorderColor) {
    (color.0, *border) = if on {
        (theme::DANGER_FILL, BorderColor::all(theme::DANGER))
    } else {
        (Color::NONE, BorderColor::all(theme::RULE))
    };
}

fn shown(visible: bool) -> Display {
    if visible {
        Display::Flex
    } else {
        Display::None
    }
}

/// Shows a node or hides it. A node that is touched is laid out again, with the whole
/// interface around it: one that doesn't change is left alone.
fn show(node: &mut Mut<Node>, visible: bool) {
    let display = shown(visible);
    if node.display != display {
        node.display = display;
    }
}

/// Shows a node at a place of the screen, in the pixels of the interface, touching it only if
/// it moves or comes back.
fn put(node: &mut Mut<Node>, at: Vec2) {
    let (left, top) = (px(at.x), px(at.y));
    if node.display != Display::Flex || node.left != left || node.top != top {
        (node.display, node.left, node.top) = (Display::Flex, left, top);
    }
}

/// How a run is played, as the display says it under the name of the game: alone or with
/// others, in arena mode, and in free mode. A playtest says that it is one, because it sets
/// no record.
fn mode(run: &Run) -> String {
    let mut told = match (run.playtest, run.solo) {
        (true, _) => "PLAYTEST",
        (false, true) => "SOLO",
        (false, false) => "CO-OP",
    }
    .to_string();
    if run.arena {
        told.push_str(" \u{b7} ARENA");
    }
    if run.free && !run.playtest {
        told.push_str(" \u{b7} FREE");
    }
    told
}

/// The group of arena mode that a run is at: the one that is on, or the next one during the
/// lull before it.
fn group(run: &Run) -> u32 {
    run.sector + run.lull.is_some() as u32
}

/// What the display counts, and the number that it shows: the sector that the players have
/// reached, or the start bay. In arena mode, the group that is on, or the next one during
/// the lull before it.
fn place(run: &Run) -> (&'static str, String) {
    match (run.arena, run.lull, run.sector) {
        (true, Some(_), _) => ("NEXT GROUP", group(run).to_string()),
        (true, None, _) => ("GROUP", group(run).to_string()),
        (false, _, 0) => ("SECTOR", "BAY".to_string()),
        (false, _, sector) => ("SECTOR", sector.to_string()),
    }
}

/// What a message says of a wave that is on its way: in an arena of the corridor, which
/// wave of how many. In arena mode, the group that comes: it has no waves to count.
fn wave_words(run: Option<&Run>, wave: u32, of: u32) -> String {
    match run.filter(|run| run.arena) {
        Some(run) => format!("Group {}", group(run)),
        None => format!("Wave {wave} of {of}"),
    }
}

/// What a message says of the group that a lull is about to end with, for its last seconds.
fn lull_words(run: &Run, shown: Option<u8>) -> Option<String> {
    let left = run.lull.filter(|left| *left <= LULL_COUNTED)?;
    (run.arena && Some(left) != shown).then(|| format!("Group {} in {left}", group(run)))
}

/// What the start bay tells of the controls, with the keys of the player: of mines and of a
/// shield only if their tank has them, and of the repair among several players.
fn controls(key: impl Fn(Action) -> String, tank: &Genome, solo: bool) -> String {
    let moves = Action::Up;
    let moves = [moves, Action::Left, Action::Down, Action::Right].map(&key);
    let mut first = format!(
        "{} move, mouse aim, {} shoot",
        moves.concat(),
        key(Action::Shoot)
    );
    if tank.mines() > 0 {
        first.push_str(&format!(", {} mine", key(Action::Mine)));
    }
    let mut second = String::new();
    if tank.shield_cooldown().is_some() {
        second.push_str(&format!("{} shield, ", key(Action::Shield)));
    }
    if !solo {
        second.push_str(&format!("hold {} to repair, ", key(Action::Repair)));
    }
    let turns = (key(Action::RotateLeft), key(Action::RotateRight));
    format!(
        "{first}\n{second}{}/{} turn the view, Esc menu",
        turns.0, turns.1
    )
}

fn status(
    run: Option<Res<Run>>,
    playtest: Option<Res<Playtest>>,
    settings: Res<Settings>,
    seat: Res<LocalSeat>,
    crew: Query<(&Player, Has<Wreck>)>,
    own: Query<&Tank, With<Own>>,
    tanks: Query<(&Position, Option<&Wreck>, &TankCommand, Has<Own>), With<Player>>,
    (alarms, unlocks, fair): (Res<Alarms>, Res<Unlocks>, Res<Fair>),
    mut readouts: Query<(&Readout, &mut Text, &mut TextColor), Without<MateNumber>>,
    mut pips: Query<(&Pip, &mut BackgroundColor, &mut BorderColor, &mut Node)>,
    mut numbers: Query<(&MateNumber, &mut TextColor), Without<Readout>>,
    (wrecks, mut fills, mut strikes): (
        Query<(&Player, &Wreck)>,
        Query<(&RepairFill, &mut Node), Without<Pip>>,
        Query<(&Strike, &mut Visibility)>,
    ),
) {
    let Some(run) = run else {
        return;
    };
    for (strike, mut visibility) in &mut strikes {
        let gone = !crew.iter().any(|(player, _)| player.seat == strike.0);
        let wanted = if gone && alarms.left(strike.0).is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        visibility.set_if_neq(wanted);
    }
    // The square of a wreck fills as it is repaired.
    for (fill, mut node) in &mut fills {
        let wreck = wrecks.iter().find(|(player, _)| player.seat == fill.0);
        let wanted = percent(100.0 * repaired(wreck.map(|(_, wreck)| wreck)));
        if node.height != wanted {
            node.height = wanted;
        }
    }
    let infinite_lives = playtest.is_some_and(|playtest| playtest.infinite_lives);
    let key = |action| {
        let bound = settings.bindings(action)[0];
        bound.map_or("?".to_string(), |binding| binding.name())
    };
    // What the player is told about the wrecks, where their own tank still shows.
    let repair = tanks.iter().find(|(.., own)| *own).and_then(|own| {
        let (place, wreck, command, _) = own;
        let wrecks = tanks.iter().filter(|(.., own)| !own);
        let wrecks: Vec<(Vec2, f32, bool)> = wrecks
            .filter_map(|(place, wreck, ..)| {
                let wreck = wreck?;
                Some((place.0, wreck.repair, wreck.repaired_by(seat.0)))
            })
            .collect();
        // The teammates whose tanks run can repair it, and those at it are named.
        let menders = crew
            .iter()
            .filter(|(player, wreck)| !wreck && player.seat != seat.0);
        let menders = menders.fold(0, |seats, (player, _)| seats | 1 << player.seat);
        let wreck = wreck.map(|wreck| (wreck.repair, wreck.repairers));
        let own = (place.0, wreck, command.repair);
        repair::word(own, menders, &wrecks)
    });
    // A single player on their last life is told so, in the color of danger, for as long as
    // it lasts.
    let last_life = run.solo && !infinite_lives && run.lives == 1;
    for (readout, mut text, mut ink) in &mut readouts {
        if matches!(readout, Readout::Crew) {
            let color = if last_life {
                theme::DANGER
            } else {
                theme::TEXT_DIM
            };
            ink.set_if_neq(TextColor(color));
        }
        let wanted = match readout {
            Readout::Player => format!("PLAYER {}", seat.0 + 1),
            Readout::Mode => mode(&run),
            Readout::Crew if last_life => "LAST LIFE".to_string(),
            Readout::Crew if run.solo => "LIVES".to_string(),
            Readout::Crew => "TEAM".to_string(),
            Readout::Unit => place(&run).0.to_string(),
            Readout::Sector => place(&run).1,
            Readout::Lives if infinite_lives && run.solo => "INFINITE".to_string(),
            Readout::Lives => String::new(),
            Readout::Seed => run.seed_text.clone(),
            // A playtest says what it is next to the name of the game, and whether the
            // achievements that it earns are fake ones.
            Readout::Earning if run.playtest && unlocks.fake => {
                match unlocks::earns(&run, &unlocks, *fair) {
                    true => "FAKE ACHIEVEMENTS",
                    false => "NO ACHIEVEMENTS",
                }
                .to_string()
            }
            Readout::Earning if !run.earns && !run.playtest => "NO ACHIEVEMENTS".to_string(),
            Readout::Earning => String::new(),
            Readout::Difficulty => unlocks::difficulty_name(run.difficulty).to_uppercase(),
            // The controls are shown where a run starts.
            Readout::Hint if run.sector == 0 => {
                let tank = own.single().map_or(Genome::PLAYER, |tank| tank.genome);
                controls(key, &tank, run.solo)
            }
            Readout::Hint => String::new(),
            Readout::Repair => {
                let word = repair.filter(|_| !run.solo);
                let sentence = word.map(|word| word.sentence(&key(Action::Repair)));
                sentence.unwrap_or_default().to_uppercase()
            }
            Readout::Shield => continue,
        };
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
    for (pip, mut color_of, mut border_of, mut node) in &mut pips {
        // The square is colored here, and its colors are only touched if they change: the
        // engine draws a square that is touched again.
        let (mut color, mut border) = (*color_of, *border_of);
        match *pip {
            // A single player has lives.
            Pip::Life(life) => {
                show(&mut node, run.solo && !infinite_lives);
                light(life <= run.lives, theme::ACCENT, &mut color, &mut border);
                // The last one is edged in the color of danger.
                if last_life && life == 1 {
                    border = BorderColor::all(theme::DANGER);
                }
                // The life that was just lost blinks red, and one that was taken back green.
                let lost = alarms.life.filter(|(lost, _)| *lost == life);
                if let Some(on) = Alarms::blink(lost.map(|(_, since)| since)) {
                    alarm(on, &mut color, &mut border);
                }
                let back = alarms.back.filter(|(back, _)| *back == life);
                if Alarms::blink(back.map(|(_, since)| since)) == Some(true) {
                    (color.0, border) = (theme::GOOD_FILL, BorderColor::all(theme::GOOD));
                }
            }
            // Several players have each other: a square for each tank, with the number of
            // its player, lit while the tank runs and empty while it is a wreck.
            Pip::Mate(seat) => {
                let mate = crew.iter().find(|(player, _)| player.seat == seat);
                // The square of a player who left the game stays a moment, struck out.
                let leaving = mate.is_none() && alarms.left(seat).is_some();
                show(&mut node, !run.solo && (mate.is_some() || leaving));
                let runs = mate.is_some_and(|(_, wreck)| !wreck);
                light(runs, theme::ACCENT, &mut color, &mut border);
                if leaving {
                    border = BorderColor::all(theme::RULE_DIM);
                }
                // The square of a tank that was just destroyed blinks.
                if let Some(on) = Alarms::blink(alarms.lost(seat)) {
                    alarm(on, &mut color, &mut border);
                }
            }
            _ => {}
        }
        color_of.set_if_neq(color);
        border_of.set_if_neq(border);
    }
    for (number, mut ink) in &mut numbers {
        let mut mates = crew.iter().filter(|(player, _)| player.seat == number.0);
        let runs = mates.any(|(_, wreck)| !wreck);
        let blinks = Alarms::blink(alarms.lost(number.0)) == Some(true);
        ink.set_if_neq(TextColor(match (blinks, runs) {
            (true, _) => theme::DANGER,
            (false, true) => theme::TEXT_ON_ACCENT,
            (false, false) => theme::TEXT_DIM,
        }));
    }
}

/// What the shield bar shows: how full it is, in which color, and a word about its state.
/// A shield that is ready shows the color of its player.
fn shield_gauge(shield: &Shield, cooldown: f32, accent: Color) -> (f32, Color, String) {
    match shield.state {
        ShieldState::Active(_) => (1.0, theme::TEXT, "UP".to_string()),
        ShieldState::Ready if shield.cooldown > 0.0 => (
            1.0 - shield.cooldown / cooldown,
            theme::TEXT_DIM,
            format!("{:.0} s", shield.cooldown.ceil()),
        ),
        ShieldState::Ready => (1.0, accent, "READY".to_string()),
    }
}

fn weapons(
    player: Query<(&Tank, &Gun, &Shield), With<Own>>,
    mut pips: Query<(&Pip, &mut BackgroundColor, &mut BorderColor, &mut Node)>,
    mut bars: Query<(&Bar, &mut BackgroundColor, &mut Node), Without<Pip>>,
    mut readouts: Query<(&Readout, &mut Text)>,
) {
    let Ok((tank, gun, shield)) = player.single() else {
        return;
    };
    let genome = &tank.genome;
    let armed = gun.mines;
    for (pip, mut color, mut border, mut node) in &mut pips {
        let (fitted, on) = match *pip {
            Pip::Round(round) => (round <= genome.magazine(), round <= gun.rounds),
            Pip::Mine(mine) => (mine <= genome.mines(), mine + armed <= genome.mines()),
            Pip::Life(_) | Pip::Mate(_) => continue,
        };
        show(&mut node, fitted);
        let (mut fill, mut edge) = (*color, *border);
        light(on, theme::TEXT, &mut fill, &mut edge);
        color.set_if_neq(fill);
        border.set_if_neq(edge);
    }
    let has_shield = genome.level(Stat::Shield) > 0;
    let cooldown = genome.shield_cooldown().unwrap_or(1.0);
    let (fill, shield_color, state) = shield_gauge(shield, cooldown, theme::ACCENT);
    for (bar, mut color, mut node) in &mut bars {
        let width = match bar {
            Bar::Reload => percent(100.0 * gun.reload / genome.reload_time()),
            Bar::Shield => {
                color.set_if_neq(BackgroundColor(shield_color));
                percent(if has_shield { 100.0 * fill } else { 0.0 })
            }
        };
        if node.width != width {
            node.width = width;
        }
    }
    for (readout, mut text) in &mut readouts {
        if matches!(readout, Readout::Shield) {
            let wanted = if has_shield { state.as_str() } else { "NONE" };
            if text.0 != wanted {
                text.0 = wanted.to_string();
            }
        }
    }
}

/// Short messages at the top of the screen.
fn toasts(
    time: Res<Time>,
    run: Option<Res<Run>>,
    mut events: MessageReader<SimEvent>,
    mut toast: Single<(&mut Toast, &mut Text, &mut TextColor)>,
    (barriers, running, seat): (
        Query<&Barrier>,
        Query<(&Player, &Position), Without<Wreck>>,
        Res<LocalSeat>,
    ),
    (mut shown, mut counted, mut waited): (Local<u8>, Local<Option<u8>>, Local<bool>),
) {
    let (toast, text, color) = &mut *toast;
    // Several things can happen at once, such as clearing a sector and unlocking its barrier.
    // The message that matters most is shown.
    let mut news: Option<(u8, String)> = None;
    // The last seconds of a lull of arena mode are counted down.
    if let Some(run) = &run {
        if let Some(message) = lull_words(run, *counted) {
            news = Some((1, message));
        }
        *counted = run.lull;
    }
    let in_a_room = run.as_ref().is_some_and(|run| run.arena);
    for event in events.read() {
        let (weight, message) = match *event {
            SimEvent::SectorCleared { sector } if sector > 0 && in_a_room => {
                (1, format!("Group {sector} cleared"))
            }
            SimEvent::SectorCleared { sector } if sector > 0 => {
                (1, format!("Sector {sector} cleared"))
            }
            // An arena leaves a life, which is of use only to a player who lost one.
            SimEvent::LifeDropped { .. } => {
                let short = run.as_ref().is_some_and(|run| run.lives < SOLO_LIVES);
                let message = if short {
                    "Arena cleared - take the life at the barrier"
                } else {
                    "Arena cleared - you have every life"
                };
                (2, message.to_string())
            }
            SimEvent::LifeTaken { .. } => (2, "Life gained".to_string()),
            SimEvent::BarrierUnlocked { sector } if sector > 0 => {
                (1, "Barrier unlocked".to_string())
            }
            // The waves of an arena are counted, so that the players know when it ends. A
            // group of arena mode is named when it comes.
            SimEvent::WaveIncoming { wave, of } => (2, wave_words(run.as_deref(), wave, of)),
            _ => continue,
        };
        if news.as_ref().is_none_or(|(shown, _)| weight >= *shown) {
            news = Some((weight, message));
        }
    }
    // A barrier that starts to wait for teammates who are far from it says so once, as soon
    // as the line is free: the sector that was cleared is told first.
    if let Some(run) = &run {
        let running: Vec<(u8, Vec2)> = running
            .iter()
            .map(|(player, position)| (player.seat, position.0))
            .collect();
        let waits = barriers
            .iter()
            .map(|barrier| tags::waiting(run, barrier, &running))
            .find(|waits| !waits.is_empty());
        match waits {
            Some(waits) if !*waited && news.is_none() && toast.0 == 0.0 => {
                let who = tags::names(&waits, seat.0);
                news = Some((1, format!("The barrier waits for {who}")));
                *waited = true;
            }
            Some(_) => {}
            None => *waited = false,
        }
    }
    // A message stays until its time is over, unless one that matters as much comes.
    if toast.0 == 0.0 {
        *shown = 0;
    }
    if let Some((weight, message)) = news.filter(|(weight, _)| *weight >= *shown) {
        text.0 = message.to_uppercase();
        toast.0 = TOAST_TIME;
        *shown = weight;
    }
    // Nothing moves while the host of a game holds it, and everyone is told why.
    if run.as_ref().is_some_and(|run| run.paused) {
        text.0 = "Paused by the host".to_uppercase();
        toast.0 = 0.5;
        *shown = u8::MAX;
    }
    toast.0 = (toast.0 - time.delta_secs()).max(0.0);
    color.set_if_neq(TextColor(theme::TEXT.with_alpha((toast.0 / 0.5).min(1.0))));
}

/// Points to the last enemies of the sector when they are out of view.
fn mark_enemies(
    run: Option<Res<Run>>,
    ui_scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    enemies: Query<(&Tank, &Position), With<Enemy>>,
    mut markers: Query<(&EnemyMarker, &mut Node), Without<MateMarker>>,
) {
    let (camera, camera_transform) = *camera;
    let sector = run.map_or(0, |run| run.sector);
    let left: Vec<Vec2> = enemies
        .iter()
        .filter(|(tank, _)| tank.sector == sector)
        .map(|(_, position)| position.0)
        .collect();
    let Some(size) = camera.logical_viewport_size() else {
        return;
    };
    for (marker, mut node) in &mut markers {
        let on_screen = left
            .get(marker.0)
            .filter(|_| left.len() <= MARKED_ENEMIES)
            .and_then(|&at| {
                camera
                    .world_to_viewport(camera_transform, scene::to_world(at, 0.5))
                    .ok()
            });
        // An enemy behind the camera has no place on the screen, but that never happens when
        // looking down from above.
        let Some(edge) = on_screen.and_then(|at| edge_place(at, size, EDGE_MARGIN)) else {
            show(&mut node, false);
            continue;
        };
        let edge = edge - Vec2::splat(ENEMY_MARKER / 2.0);
        put(&mut node, edge / ui_scale.0);
    }
}

/// Where the marker of something that is out of view goes on the screen: on its edge, a margin
/// inside, on the side where the thing is. What is in view has no marker.
fn edge_place(at: Vec2, size: Vec2, margin: f32) -> Option<Vec2> {
    let out_of_view = at.cmplt(Vec2::ZERO).any() || at.cmpgt(size).any();
    out_of_view.then(|| at.clamp(Vec2::splat(margin), size - margin))
}

/// The fill of the marker of a teammate and the color of its number: full for a tank that
/// runs and hollow for a wreck, as the squares of the team are in the corner of the screen.
fn mate_look(wreck: bool) -> (Color, Color) {
    if wreck {
        (theme::SURFACE, theme::TEXT)
    } else {
        (theme::PLAYER, theme::TEXT_ON_ACCENT)
    }
}

/// The edge of the marker of a tank, and the ring around it: a red ring closes on the marker
/// of a tank that was destroyed this long ago, and the marker is red until it has.
fn closing(since: Option<f32>) -> (Color, Outline) {
    match since.filter(|since| *since < RING_TIME) {
        Some(since) => {
            let reach = RING_REACH * (1.0 - since / RING_TIME);
            (theme::DANGER, Outline::new(px(2), px(reach), theme::DANGER))
        }
        None => (theme::PLAYER, Outline::new(px(0), px(0), Color::NONE)),
    }
}

/// Whether a tank gets a marker when it is out of view. The tanks of the other players do. The
/// tank of the player at this screen is in view for as long as it runs: it gets one as a
/// wreck, when the view follows a teammate.
fn is_marked(own: bool, wreck: bool) -> bool {
    !own || wreck
}

/// The way the edge of the screen runs where the marker of something out of view stands:
/// across for the top and the bottom edges, and down for the two sides.
fn edge_run(at: Vec2, size: Vec2) -> Vec2 {
    if at.y < 0.0 || at.y > size.y {
        Vec2::X
    } else {
        Vec2::Y
    }
}

/// A place for a marker of this size next to where it wants to be on the edge of the screen,
/// clear of the markers that stand there already. It steps along the edge, to one side and
/// to the other, until it is clear, and stays on the screen. Each marker that stands already
/// is given by its middle and its size.
fn clear_place(
    wanted: Vec2,
    marker: Vec2,
    along: Vec2,
    screen: Vec2,
    taken: &[(Vec2, Vec2)],
) -> Vec2 {
    let clear = |at: &Vec2| {
        taken.iter().all(|(other, size)| {
            let (apart, room) = ((*at - *other).abs(), (marker + *size) / 2.0 + MARKER_GAP);
            apart.x >= room.x || apart.y >= room.y
        })
    };
    let on_screen = |at: &Vec2| {
        let margin = Vec2::splat(EDGE_MARGIN);
        at.cmpge(margin).all() && at.cmple(screen - margin).all()
    };
    let step = (marker * along).length() + MARKER_GAP;
    let steps = (0..=8).flat_map(|far| [far as f32, -(far as f32)]);
    let mut places = steps.map(|far| wanted + along * step * far);
    places
        .find(|at| on_screen(at) && clear(at))
        .unwrap_or(wanted)
}

/// Among several players, points to the tanks of the players that are out of view, each by
/// the number of its player. Markers that would hide each other, or the marker of an enemy,
/// stand side by side instead.
fn mark_mates(
    run: Option<Res<Run>>,
    ui_scale: Res<UiScale>,
    alarms: Res<Alarms>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    tanks: Query<(&Player, &Position, Has<Own>, Option<&Wreck>)>,
    enemies: Query<&Node, (With<EnemyMarker>, Without<MateMarker>)>,
    mut bars: Query<(&RepairBar, &mut Node), (Without<MateMarker>, Without<EnemyMarker>)>,
    mut markers: Query<(
        &MateMarker,
        &mut Node,
        &mut BackgroundColor,
        &mut BorderColor,
        &mut Outline,
        &Children,
    )>,
    mut numbers: Query<&mut TextColor>,
) {
    let (camera, camera_transform) = *camera;
    let several = run.is_some_and(|run| !run.solo);
    // The screen, in the pixels of the interface.
    let screen = camera.logical_viewport_size().filter(|_| several);
    let screen = screen.map(|size| size / ui_scale.0);
    // The markers of the enemies stand where they are.
    let mut taken: Vec<(Vec2, Vec2)> = Vec::new();
    for node in enemies.iter().filter(|node| node.display != Display::None) {
        if let (Val::Px(left), Val::Px(top)) = (node.left, node.top) {
            // A square on its corner is as wide as it is long from corner to corner.
            let size = Vec2::splat(ENEMY_MARKER * std::f32::consts::SQRT_2);
            taken.push((Vec2::new(left, top) + ENEMY_MARKER / 2.0, size));
        }
    }
    // The tanks that are out of view, in the order of their seats, each with its place.
    let mut marked: Vec<(u8, Vec2, bool)> = Vec::new();
    let mut seats: Vec<_> = tanks.iter().collect();
    seats.sort_by_key(|(player, ..)| player.seat);
    // The marker of a wreck fills a bar under its name as it is repaired.
    for (bar, mut node) in &mut bars {
        let wreck = seats.iter().find(|(player, ..)| player.seat == bar.0);
        let wanted = percent(100.0 * repaired(wreck.and_then(|(.., wreck)| *wreck)));
        if node.width != wanted {
            node.width = wanted;
        }
    }
    for (player, position, own, wreck) in seats {
        let wreck = wreck.is_some();
        let Some(screen) = screen.filter(|_| is_marked(own, wreck)) else {
            continue;
        };
        let middle = scene::to_world(position.0, 0.5);
        let Ok(at) = camera.world_to_viewport(camera_transform, middle) else {
            continue;
        };
        let at = at / ui_scale.0;
        let Some(edge) = edge_place(at, screen, EDGE_MARGIN) else {
            continue;
        };
        let place = clear_place(edge, MATE_MARKER, edge_run(at, screen), screen, &taken);
        taken.push((place, MATE_MARKER));
        marked.push((player.seat, place, wreck));
    }
    for (marker, mut node, mut fill, mut border, mut outline, children) in &mut markers {
        // A red ring closes on the marker of a tank that was just destroyed.
        let (edge, ring) = closing(alarms.lost(marker.0));
        border.set_if_neq(BorderColor::all(edge));
        if *outline != ring {
            *outline = ring;
        }
        let Some(&(_, place, wreck)) = marked.iter().find(|(seat, ..)| *seat == marker.0) else {
            if node.display != Display::None {
                node.display = Display::None;
            }
            continue;
        };
        let corner = (place - MATE_MARKER / 2.0).round();
        put(&mut node, corner);
        let (wanted, ink) = mate_look(wreck);
        if fill.0 != wanted {
            fill.0 = wanted;
        }
        for child in children.iter() {
            if let Ok(mut color) = numbers.get_mut(child)
                && color.0 != ink
            {
                color.0 = ink;
            }
        }
    }
}

/// Where the tag of a tank goes on the screen: over the tank, by so many meters of ground,
/// and centered on it. `meter` is how long a meter of ground is on the screen there.
fn tag_place(tank: Vec2, meter: f32) -> Vec2 {
    tank - Vec2::new(TAG_WIDTH / 2.0, TAG_RISE * meter)
}

/// Among several players, puts over each of their tanks the number of its player: the tanks
/// all have one color, and a wreck keeps its tag.
fn tag_players(
    run: Option<Res<Run>>,
    ui_scale: Res<UiScale>,
    alarms: Res<Alarms>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    tanks: Query<(&Player, &GlobalTransform)>,
    mut tags: Query<(&PlayerTag, &mut Node, &mut BorderColor)>,
) {
    let (camera, camera_transform) = *camera;
    let several = run.is_some_and(|run| !run.solo);
    for (tag, mut node, mut border) in &mut tags {
        // The tag of a tank that was just destroyed is in the color of danger, for as long
        // as its alert shows.
        let color = if alarms.lost(tag.0).is_some() {
            theme::DANGER
        } else {
            theme::PLAYER
        };
        border.set_if_neq(BorderColor::all(color));
        let tank = tanks.iter().find(|(player, _)| player.seat == tag.0);
        let place = tank.filter(|_| several).and_then(|(_, tank)| {
            let middle = tank.translation();
            // A meter of ground across the screen, at the tank.
            let aside = middle + camera_transform.rotation() * Vec3::X;
            let at = camera.world_to_viewport(camera_transform, middle).ok()?;
            let beside = camera.world_to_viewport(camera_transform, aside).ok()?;
            Some(tag_place(at, at.distance(beside)))
        });
        let Some(place) = place else {
            show(&mut node, false);
            continue;
        };
        put(&mut node, place / ui_scale.0);
    }
}

/// Keeps the scale as long as its meters of ground, and the arrow pointing north.
fn instruments(
    view: Res<View>,
    ui_scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut bar: Single<&mut Node, With<ScaleBar>>,
    mut arrow: Single<&mut UiTransform, With<NorthArrow>>,
) {
    let (camera, transform) = *camera;
    // The ground under the middle of the screen, where a meter is as long as it gets drawn.
    let middle = scene::to_world(view.focus(), 0.0);
    let half = transform.rotation() * Vec3::X * SCALE / 2.0;
    let ends = [middle - half, middle + half].map(|end| camera.world_to_viewport(transform, end));
    if let [Ok(from), Ok(to)] = ends {
        let wanted = px((from.distance(to) / ui_scale.0).round());
        if bar.width != wanted {
            bar.width = wanted;
        }
    }
    // North is at the top of the screen until the view turns, and the arrow turns with it.
    let wanted = Rot2::radians(view.yaw());
    if arrow.rotation != wanted {
        arrow.rotation = wanted;
    }
}

/// Where a part of the dial lies, in degrees around the tank from the right of the screen
/// toward its bottom. The arc of the rounds is centered under the tank, the first round on the
/// left, and the mines follow it further left.
fn dial_angle(part: DialPart, rounds: u8) -> f32 {
    let pitch = DIAL_ROUND + DIAL_GAP;
    let left = 90.0 + (rounds as f32 * pitch - DIAL_GAP) / 2.0;
    match part {
        DialPart::Round { round, step } => {
            let step_arc = DIAL_ROUND / DIAL_STEPS as f32;
            left - round as f32 * pitch - (step + 1) as f32 * step_arc
        }
        DialPart::Mine(mine) => left + 8.0 + 8.0 * mine as f32,
    }
}

/// Gives its dial to the tank of the player at this screen, and takes it back from a tank
/// that isn't theirs anymore.
fn attach_dial(
    mut commands: Commands,
    players: Query<(Entity, &Tank), Added<Own>>,
    mut left: RemovedComponents<Own>,
    dials: Query<(Entity, &ChildOf), With<Dial>>,
    parts: Res<DialParts>,
) {
    for tank in left.read() {
        for (dial, parent) in &dials {
            if parent.parent() == tank {
                commands.entity(dial).despawn();
            }
        }
    }
    for (entity, tank) in &players {
        let rounds = tank.genome.magazine();
        let steps = (0..rounds)
            .flat_map(|round| (0..DIAL_STEPS).map(move |step| DialPart::Round { round, step }));
        let mines = (0..tank.genome.mines()).map(DialPart::Mine);
        let dial = commands
            .spawn((
                Dial,
                Transform::from_xyz(0.0, 6.0 * LIFT, 0.0),
                Visibility::default(),
                ChildOf(entity),
            ))
            .id();
        for part in steps.chain(mines) {
            let shape = match part {
                DialPart::Round { .. } => &parts.step,
                DialPart::Mine(_) => &parts.dot,
            };
            // A part is drawn at the right of the tank, and turned to its place.
            let turn = Quat::from_rotation_y(-dial_angle(part, rounds).to_radians());
            commands.spawn((
                part,
                Mesh3d(shape.clone()),
                MeshMaterial3d(parts.unlit.clone()),
                Transform::from_rotation(turn),
                ChildOf(dial),
            ));
        }
    }
}

/// Keeps the dial under the tank on the screen, whichever way the view is turned. A wreck
/// has none: its weapons are of no use until it is repaired.
fn turn_dial(
    view: Res<View>,
    mut dials: Query<(&mut Transform, &mut Visibility, &ChildOf), With<Dial>>,
    wrecks: Query<(), With<Wreck>>,
) {
    let turn = Quat::from_rotation_y(view.yaw());
    for (mut transform, mut visibility, tank) in &mut dials {
        if transform.rotation != turn {
            transform.rotation = turn;
        }
        let wanted = if wrecks.contains(tank.parent()) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// How many steps of the arc of a round are lit, and whether the round is being loaded.
fn round_steps(round: u8, loaded: u8, progress: f32) -> (u8, bool) {
    if round < loaded {
        (DIAL_STEPS, false)
    } else if round == loaded {
        ((progress * DIAL_STEPS as f32) as u8, true)
    } else {
        (0, false)
    }
}

fn light_dial(
    player: Query<(&Tank, &Gun), With<Own>>,
    parts: Res<DialParts>,
    mut drawn: Query<(&DialPart, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    let Ok((tank, gun)) = player.single() else {
        return;
    };
    let genome = &tank.genome;
    let progress = (gun.reload / genome.reload_time()).clamp(0.0, 1.0);
    let armed = gun.mines;
    for (part, mut material) in &mut drawn {
        let wanted = match *part {
            DialPart::Round { round, step } => match round_steps(round, gun.rounds, progress) {
                (lit, _) if step >= lit => &parts.unlit,
                (_, true) => &parts.loading,
                (_, false) => &parts.lit,
            },
            DialPart::Mine(mine) if mine + armed < genome.mines() => &parts.lit,
            DialPart::Mine(_) => &parts.unlit,
        };
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_stands_at_the_edge_of_the_screen_toward_what_is_out_of_view() {
        let (size, margin) = (Vec2::new(1920.0, 1080.0), EDGE_MARGIN);
        // What is in view has no marker, up to the very edge.
        for at in [
            Vec2::new(960.0, 540.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(1920.0, 1080.0),
        ] {
            assert_eq!(edge_place(at, size, margin), None, "{at}");
        }
        // What is past an edge has one a margin inside that edge, level with it.
        let below = edge_place(Vec2::new(700.0, 1500.0), size, margin);
        assert_eq!(below, Some(Vec2::new(700.0, 1080.0 - margin)));
        let left = edge_place(Vec2::new(-300.0, 400.0), size, margin);
        assert_eq!(left, Some(Vec2::new(margin, 400.0)));
        // Past a corner, it stands in the corner.
        let corner = edge_place(Vec2::new(2500.0, -90.0), size, margin);
        assert_eq!(corner, Some(Vec2::new(1920.0 - margin, margin)));
        // Just past an edge near a corner, it keeps clear of the other edge too.
        let near = edge_place(Vec2::new(10.0, 1100.0), size, margin);
        assert_eq!(near, Some(Vec2::new(margin, 1080.0 - margin)));
    }

    #[test]
    fn the_marker_of_a_teammate_tells_a_tank_that_runs_from_a_wreck() {
        // Full for a tank that runs, in the color of the players, and hollow for a wreck.
        let ((runs, on_full), (wreck, on_hollow)) = (mate_look(false), mate_look(true));
        assert_eq!(runs, theme::PLAYER);
        assert_ne!(wreck, runs);
        assert_ne!(on_full, on_hollow);
        // Every other player's tank is pointed to, and one's own once it is a wreck.
        assert!(is_marked(false, false) && is_marked(false, true));
        assert!(is_marked(true, true) && !is_marked(true, false));
        // A marker fits between the line that frames the sheet and the panels.
        let half = MATE_MARKER.y / 2.0;
        assert!(EDGE_MARGIN - half >= SHEET_EDGE && EDGE_MARGIN + half <= MARGIN);
    }

    #[test]
    fn markers_that_would_hide_each_other_stand_side_by_side() {
        let screen = Vec2::new(1920.0, 1080.0);
        let (marker, across) = (MATE_MARKER, Vec2::X);
        let wanted = Vec2::new(900.0, 1080.0 - EDGE_MARGIN);
        let place = |taken: &[(Vec2, Vec2)]| clear_place(wanted, marker, across, screen, taken);
        // Alone, a marker stands where its tank is.
        assert_eq!(place(&[]), wanted);
        // Three wrecks that lie together behind the player: three markers in a row along
        // the bottom edge, the first in its place and the others on either side of it.
        let step = Vec2::X * (marker.x + MARKER_GAP);
        let mut taken = vec![(wanted, marker)];
        assert_eq!(place(&taken), wanted + step);
        taken.push((wanted + step, marker));
        assert_eq!(place(&taken), wanted - step);
        // The marker of an enemy in the same direction is left to show too.
        let diamond = Vec2::splat(ENEMY_MARKER * std::f32::consts::SQRT_2);
        let beside = place(&[(wanted + Vec2::X * 4.0, diamond)]);
        assert!((beside.x - wanted.x - 4.0).abs() >= (marker.x + diamond.x) / 2.0);
        assert_eq!(beside.y, wanted.y);
        // One that is far enough is no matter.
        assert_eq!(place(&[(wanted + Vec2::X * 60.0, marker)]), wanted);

        // Along a side of the screen, markers step up and down, and stay on the screen.
        let corner = Vec2::new(EDGE_MARGIN, 1080.0 - EDGE_MARGIN);
        let down = clear_place(corner, marker, Vec2::Y, screen, &[(corner, marker)]);
        assert_eq!(down, corner - Vec2::Y * (marker.y + MARKER_GAP));
        // The top and the bottom edges run across, and the two sides run down.
        assert_eq!(edge_run(Vec2::new(500.0, 1300.0), screen), Vec2::X);
        assert_eq!(edge_run(Vec2::new(500.0, -20.0), screen), Vec2::X);
        assert_eq!(edge_run(Vec2::new(-20.0, 500.0), screen), Vec2::Y);
        assert_eq!(edge_run(Vec2::new(2000.0, 500.0), screen), Vec2::Y);
        assert_eq!(edge_run(Vec2::new(2000.0, 1300.0), screen), Vec2::X);
    }

    #[test]
    fn a_tag_stands_over_its_tank_and_names_its_player_by_number() {
        assert_eq!(short_name(0), "P1");
        assert_eq!(short_name(3), "P4");
        // With 36 pixels to the meter, the tag is centered on the tank and well over it.
        let place = tag_place(Vec2::new(400.0, 300.0), 36.0);
        assert_eq!(place.x + TAG_WIDTH / 2.0, 400.0);
        assert!(
            place.y < 300.0 - 1.3 * 36.0,
            "the tag would cover the barrel"
        );
    }

    #[test]
    fn the_arc_of_the_rounds_is_centered_under_the_tank() {
        for rounds in 1..=6 {
            // From the left end of the first round to the right end of the last.
            let left = dial_angle(DialPart::Round { round: 0, step: 0 }, rounds)
                + DIAL_ROUND / DIAL_STEPS as f32;
            let last = DialPart::Round {
                round: rounds - 1,
                step: DIAL_STEPS - 1,
            };
            let right = dial_angle(last, rounds);
            assert!((left + right - 180.0).abs() < 1e-3, "{rounds} rounds");
            // The arc stays under the tank: it never climbs to its sides.
            assert!(left < 180.0 && right > 0.0);
        }
    }

    #[test]
    fn the_steps_of_a_round_follow_each_other_without_a_gap() {
        let at = |round, step| dial_angle(DialPart::Round { round, step }, 5);
        let step_arc = DIAL_ROUND / DIAL_STEPS as f32;
        assert!((at(0, 0) - at(0, 1) - step_arc).abs() < 1e-4);
        // Between two rounds there is a gap.
        let gap = at(0, DIAL_STEPS - 1) - (at(1, 0) + step_arc);
        assert!((gap - DIAL_GAP).abs() < 1e-4);
    }

    #[test]
    fn a_round_fills_up_as_it_is_loaded() {
        assert_eq!(round_steps(0, 2, 0.5), (DIAL_STEPS, false));
        assert_eq!(round_steps(1, 2, 0.5), (DIAL_STEPS, false));
        assert_eq!(round_steps(2, 2, 0.0), (0, true));
        assert_eq!(round_steps(2, 2, 0.5), (DIAL_STEPS / 2, true));
        assert_eq!(round_steps(3, 2, 0.5), (0, false));
    }

    #[test]
    fn the_display_says_how_a_run_is_played() {
        let run = |solo: bool, free: bool, playtest: bool| Run {
            seed_text: "TEST".into(),
            seed: 1,
            difficulty: default(),
            players: if solo { 1 } else { 3 },
            team: if solo { 1 } else { 3 },
            solo,
            sector: 1,
            cleared: 0,
            checkpoint: 0,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase: tanku_core::sim::RunPhase::Playing,
            lost_to: None,
            playtest,
            earns: !free && !playtest,
            free,
            arena: false,
            lull: None,
            paused: false,
        };
        assert_eq!(mode(&run(true, false, false)), "SOLO");
        assert_eq!(mode(&run(false, false, false)), "CO-OP");
        assert_eq!(mode(&run(true, true, false)), "SOLO \u{b7} FREE");
        assert_eq!(mode(&run(false, true, false)), "CO-OP \u{b7} FREE");
        // A playtest says that it is one, in a tank of its own or not.
        assert_eq!(mode(&run(true, false, true)), "PLAYTEST");
        assert_eq!(mode(&run(true, true, true)), "PLAYTEST");
        // A run in arena mode says so, whoever plays it.
        let in_a_room = |solo: bool, free: bool, playtest: bool| Run {
            arena: true,
            ..run(solo, free, playtest)
        };
        assert_eq!(mode(&in_a_room(true, false, false)), "SOLO \u{b7} ARENA");
        assert_eq!(
            mode(&in_a_room(false, true, false)),
            "CO-OP \u{b7} ARENA \u{b7} FREE"
        );
        assert_eq!(mode(&in_a_room(true, true, true)), "PLAYTEST \u{b7} ARENA");
    }

    #[test]
    fn the_display_counts_sectors_in_the_corridor_and_groups_in_arena_mode() {
        let run = |arena: bool, sector: u32, lull: Option<u8>| Run {
            seed_text: "TEST".into(),
            seed: 1,
            difficulty: default(),
            players: 1,
            team: 1,
            solo: true,
            sector,
            cleared: sector.saturating_sub(1),
            checkpoint: sector.saturating_sub(1),
            lives: 3,
            kills: 0,
            ticks: 0,
            phase: tanku_core::sim::RunPhase::Playing,
            lost_to: None,
            playtest: false,
            earns: !arena,
            free: false,
            arena,
            lull,
            paused: false,
        };
        let told = |run: &Run| {
            let (unit, number) = place(run);
            format!("{unit} {number}")
        };
        assert_eq!(told(&run(false, 0, None)), "SECTOR BAY");
        assert_eq!(told(&run(false, 7, None)), "SECTOR 7");
        // In arena mode, the group that is on. During a lull, the one that comes next: the
        // first one when the run starts.
        assert_eq!(told(&run(true, 0, Some(3))), "NEXT GROUP 1");
        assert_eq!(told(&run(true, 1, None)), "GROUP 1");
        assert_eq!(told(&run(true, 7, Some(2))), "NEXT GROUP 8");
        assert_eq!(told(&run(true, 8, None)), "GROUP 8");

        // A wave of an arena of the corridor is counted. A group of arena mode is named: it
        // has no waves to count.
        let corridor = run(false, 7, None);
        assert_eq!(wave_words(Some(&corridor), 2, 3), "Wave 2 of 3");
        assert_eq!(wave_words(None, 1, 3), "Wave 1 of 3");
        let room = run(true, 8, None);
        assert_eq!(wave_words(Some(&room), 1, 1), "Group 8");
        // A guest may hear of the group before it hears that the lull is over.
        assert_eq!(wave_words(Some(&run(true, 7, Some(1))), 1, 1), "Group 8");

        // The last seconds of a lull are counted down, each of them once. Its first second
        // is left to the message that says that a group is cleared.
        let lull = |left: u8| run(true, 7, Some(left));
        assert_eq!(lull_words(&lull(3), None), None);
        assert_eq!(
            lull_words(&lull(2), Some(3)),
            Some("Group 8 in 2".to_string())
        );
        assert_eq!(lull_words(&lull(2), Some(2)), None);
        assert_eq!(
            lull_words(&lull(1), Some(2)),
            Some("Group 8 in 1".to_string())
        );
        assert_eq!(lull_words(&run(true, 8, None), Some(1)), None);
        assert_eq!(lull_words(&run(false, 7, None), None), None);

        // The notes stand at the bottom of the screen, and under the messages at its top in
        // arena mode, where the tanks in reserve stand at the bottom.
        assert_eq!(notes(false).bottom, px(MARGIN));
        assert_eq!(notes(false).top, auto());
        assert_eq!(notes(true).bottom, auto());
        assert_ne!(notes(true).top, auto());
    }

    #[test]
    fn the_start_bay_tells_of_the_controls_that_the_tank_has() {
        let key = |action: Action| match action {
            Action::Up => "W",
            Action::Left => "A",
            Action::Down => "S",
            Action::Right => "D",
            Action::Shoot => "Left click",
            Action::Mine => "Right click",
            Action::Shield => "Space",
            Action::Repair => "Left Shift",
            Action::RotateLeft => "Q",
            Action::RotateRight => "E",
        };
        let key = |action: Action| key(action).to_string();
        assert_eq!(
            controls(key, &Genome::PLAYER, true),
            "WASD move, mouse aim, Left click shoot, Right click mine\n\
             Space shield, Q/E turn the view, Esc menu"
        );
        assert_eq!(
            controls(key, &Genome::PLAYER, false),
            "WASD move, mouse aim, Left click shoot, Right click mine\n\
             Space shield, hold Left Shift to repair, Q/E turn the view, Esc menu"
        );
        // A tank that a player built without mines or without a shield is told of
        // neither.
        let parts = [(Stat::Engine, 4), (Stat::Muzzle, 7)];
        let bare = Genome::of_the_garage(&parts).unwrap();
        assert_eq!(
            controls(key, &bare, true),
            "WASD move, mouse aim, Left click shoot\nQ/E turn the view, Esc menu"
        );
        let parts = [(Stat::Engine, 4), (Stat::Shield, 1)];
        let shielded = Genome::of_the_garage(&parts).unwrap();
        assert_eq!(
            controls(key, &shielded, false),
            "WASD move, mouse aim, Left click shoot\n\
             Space shield, hold Left Shift to repair, Q/E turn the view, Esc menu"
        );
    }

    #[test]
    fn a_red_ring_closes_on_the_marker_of_a_tank_just_destroyed() {
        // Far from it at first, then on it, and gone once it has closed.
        let (edge, ring) = closing(Some(0.0));
        assert_eq!(
            (edge, ring.offset, ring.width),
            (theme::DANGER, px(RING_REACH), px(2))
        );
        assert_eq!(
            closing(Some(RING_TIME / 2.0)).1.offset,
            px(RING_REACH / 2.0)
        );
        let (edge, ring) = closing(Some(RING_TIME));
        assert_eq!((edge, ring.width), (theme::PLAYER, px(0)));
        assert_eq!(closing(None).0, theme::PLAYER);
    }

    #[test]
    fn the_square_and_the_marker_of_a_wreck_fill_as_it_is_repaired() {
        let wreck = |repair: f32| Wreck {
            repair,
            ..default()
        };
        assert_eq!(repaired(None), 0.0);
        assert_eq!(repaired(Some(&wreck(0.0))), 0.0);
        assert_eq!(repaired(Some(&wreck(REPAIR_TIME / 2.0))), 0.5);
        assert_eq!(repaired(Some(&wreck(REPAIR_TIME * 2.0))), 1.0);
    }

    #[test]
    fn the_shield_bar_tells_its_state() {
        let gauge = |state, cooldown| shield_gauge(&Shield { state, cooldown }, 9.0, theme::ACCENT);
        assert_eq!(gauge(ShieldState::Ready, 0.0).2, "READY");
        assert_eq!(gauge(ShieldState::Active(0.3), 0.0).2, "UP");
        // While it recharges, it counts the seconds down.
        let (fill, _, word) = gauge(ShieldState::Ready, 4.5);
        assert_eq!((fill, word.as_str()), (0.5, "5 s"));
    }
}
