//! The heads-up display: rounds, mines, shield, sector and lives.
//!
//! It is laid out as the margin of a drawing: a table of the tank's weapons in one corner, the
//! title block of the sheet in the other, with a scale and a north arrow. What changes fastest
//! is also drawn on the ground under the player's tank, where the player is looking.

use bevy::prelude::*;
use tanku_core::{
    rules::{MAX_PLAYERS, SOLO_LIVES},
    seed::VERSION,
    sim::{
        Enemy, Gun, Player, Playtest, Position, Run, Shield, ShieldState, SimEvent, Tank,
        TankCommand, Team, Wreck,
    },
    stats::Stat,
};

use crate::{
    Screen,
    camera::{MainCamera, View},
    crew::{LocalSeat, Own},
    draft::{Draft, LIFT},
    repair, scene,
    settings::{Action, Settings},
    theme,
};

/// Seconds a message stays on screen.
const TOAST_TIME: f32 = 2.5;
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
    Sector,
    /// What stands for the lives when a playtest doesn't count them.
    Lives,
    Seed,
    Shield,
    Hint,
    /// What to do about a wreck, among several players.
    Repair,
}

#[derive(Component)]
struct Toast(f32);

#[derive(Component)]
struct EnemyMarker(usize);

/// The number of the player of a seat, next to the square of their tank.
#[derive(Component)]
struct MateNumber(u8);

/// The tag that follows the tank of the player of a seat, among several players: their tanks
/// look alike, and the tag tells whose each one is.
#[derive(Component)]
struct PlayerTag(u8);

/// How far over the middle of a tank its tag stands, in meters of ground, and how wide the
/// tag is on screen.
const TAG_RISE: f32 = 1.9;
const TAG_WIDTH: f32 = 30.0;

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
                weapons,
                toasts,
                mark_enemies,
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
        children![(
            MateNumber(seat),
            theme::label((seat + 1).to_string(), 12.0, theme::TEXT_DIM),
        )],
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
                            caption("SECTOR"),
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
                        ],
                    ),
                    (
                        cell(2.0, true),
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
                children![(Toast(0.0), theme::title("", 20.0, theme::TEXT))],
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
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(MARGIN),
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(10),
                    ..default()
                },
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
    for index in 0..MARKED_ENEMIES {
        commands.spawn((
            DespawnOnExit(Screen::Playing),
            EnemyMarker(index),
            Node {
                position_type: PositionType::Absolute,
                width: px(16),
                height: px(16),
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

fn shown(visible: bool) -> Display {
    if visible {
        Display::Flex
    } else {
        Display::None
    }
}

fn status(
    run: Option<Res<Run>>,
    playtest: Option<Res<Playtest>>,
    settings: Res<Settings>,
    seat: Res<LocalSeat>,
    crew: Query<(&Player, Has<Wreck>)>,
    tanks: Query<(&Position, Option<&Wreck>, &TankCommand, Has<Own>), With<Player>>,
    mut readouts: Query<(&Readout, &mut Text)>,
    mut pips: Query<(&Pip, &mut BackgroundColor, &mut BorderColor, &mut Node)>,
    mut numbers: Query<(&MateNumber, &mut TextColor)>,
) {
    let Some(run) = run else {
        return;
    };
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
        let own = (place.0, wreck.map(|wreck| wreck.repair), command.repair);
        repair::word(own, &wrecks)
    });
    for (readout, mut text) in &mut readouts {
        let wanted = match readout {
            Readout::Player => format!("PLAYER {}", seat.0 + 1),
            // A playtest says so, because it sets no record.
            Readout::Mode if run.playtest => "PLAYTEST".to_string(),
            Readout::Mode if run.solo => "SOLO".to_string(),
            Readout::Mode => "CO-OP".to_string(),
            Readout::Crew if run.solo => "LIVES".to_string(),
            Readout::Crew => "TEAM".to_string(),
            Readout::Sector if run.sector == 0 => "BAY".to_string(),
            Readout::Sector => run.sector.to_string(),
            Readout::Lives if infinite_lives && run.solo => "INFINITE".to_string(),
            Readout::Lives => String::new(),
            Readout::Seed => run.seed_text.clone(),
            // The controls are shown where a run starts. Several players have one more.
            Readout::Hint if run.sector == 0 => {
                let repair = if run.solo {
                    String::new()
                } else {
                    format!("hold {} to repair, ", key(Action::Repair))
                };
                format!(
                    "{}{}{}{} move, mouse aim, {} shoot, {} mine\n{} shield, {repair}{}/{} turn the view, Esc menu",
                    key(Action::Up),
                    key(Action::Left),
                    key(Action::Down),
                    key(Action::Right),
                    key(Action::Shoot),
                    key(Action::Mine),
                    key(Action::Shield),
                    key(Action::RotateLeft),
                    key(Action::RotateRight),
                )
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
    for (pip, mut color, mut border, mut node) in &mut pips {
        match *pip {
            // A single player has lives.
            Pip::Life(life) => {
                node.display = shown(run.solo && !infinite_lives);
                light(life <= run.lives, theme::ACCENT, &mut color, &mut border);
            }
            // Several players have each other: a square for each tank, with the number of
            // its player, lit while the tank runs and empty while it is a wreck.
            Pip::Mate(seat) => {
                let mate = crew.iter().find(|(player, _)| player.seat == seat);
                node.display = shown(!run.solo && mate.is_some());
                let runs = mate.is_some_and(|(_, wreck)| !wreck);
                light(runs, theme::ACCENT, &mut color, &mut border);
            }
            _ => {}
        }
    }
    for (number, mut ink) in &mut numbers {
        let mut mates = crew.iter().filter(|(player, _)| player.seat == number.0);
        let runs = mates.any(|(_, wreck)| !wreck);
        ink.set_if_neq(TextColor(if runs {
            theme::TEXT_ON_ACCENT
        } else {
            theme::TEXT_DIM
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
        node.display = shown(fitted);
        light(on, theme::TEXT, &mut color, &mut border);
    }
    let has_shield = genome.level(Stat::Shield) > 0;
    let cooldown = genome.shield_cooldown().unwrap_or(1.0);
    let (fill, shield_color, state) = shield_gauge(shield, cooldown, theme::ACCENT);
    for (bar, mut color, mut node) in &mut bars {
        match bar {
            Bar::Reload => node.width = percent(100.0 * gun.reload / genome.reload_time()),
            Bar::Shield => {
                node.width = percent(if has_shield { 100.0 * fill } else { 0.0 });
                color.0 = shield_color;
            }
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
    playtest: Option<Res<Playtest>>,
    mut events: MessageReader<SimEvent>,
    mut toast: Single<(&mut Toast, &mut Text, &mut TextColor)>,
) {
    let (toast, text, color) = &mut *toast;
    let infinite_lives = playtest.is_some_and(|playtest| playtest.infinite_lives);
    // Several things can happen at once, such as clearing a sector and unlocking its barrier.
    // The message that matters most is shown.
    let mut news: Option<(u8, String)> = None;
    for event in events.read() {
        let (weight, message) = match *event {
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
            SimEvent::TankDestroyed {
                team: Team::Players,
                ..
            } => {
                // A playtest that doesn't count its lives has none to announce, and several
                // players have wrecks to repair instead.
                let solo = run.as_ref().is_none_or(|run| run.solo);
                let lives = if infinite_lives || !solo {
                    0
                } else {
                    run.as_ref().map_or(0, |run| run.lives)
                };
                let message = match lives {
                    _ if !solo => "Tank destroyed - a teammate can repair it".to_string(),
                    0 => "Tank destroyed".to_string(),
                    1 => "Tank destroyed - last life".to_string(),
                    lives => format!("Tank destroyed - {lives} lives left"),
                };
                (3, message)
            }
            SimEvent::TankRepaired { .. } => (2, "Tank repaired".to_string()),
            // The waves of an arena are counted, so that the players know when it ends.
            SimEvent::WaveIncoming { wave, of } => (2, format!("Wave {wave} of {of}")),
            _ => continue,
        };
        if news.as_ref().is_none_or(|(shown, _)| weight >= *shown) {
            news = Some((weight, message));
        }
    }
    if let Some((_, message)) = news {
        text.0 = message.to_uppercase();
        toast.0 = TOAST_TIME;
    }
    toast.0 = (toast.0 - time.delta_secs()).max(0.0);
    color.0 = theme::TEXT.with_alpha((toast.0 / 0.5).min(1.0));
}

/// Points to the last enemies of the sector when they are out of view.
fn mark_enemies(
    run: Option<Res<Run>>,
    ui_scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    enemies: Query<(&Tank, &Position), With<Enemy>>,
    mut markers: Query<(&EnemyMarker, &mut Node)>,
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
    let margin = Vec2::splat(28.0);
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
        let Some(at) = on_screen.filter(|at| at.cmplt(Vec2::ZERO).any() || at.cmpgt(size).any())
        else {
            node.display = Display::None;
            continue;
        };
        let edge = at.clamp(margin, size - margin) - Vec2::splat(8.0);
        node.display = Display::Flex;
        node.left = px(edge.x / ui_scale.0);
        node.top = px(edge.y / ui_scale.0);
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
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    tanks: Query<(&Player, &GlobalTransform)>,
    mut tags: Query<(&PlayerTag, &mut Node)>,
) {
    let (camera, camera_transform) = *camera;
    let several = run.is_some_and(|run| !run.solo);
    for (tag, mut node) in &mut tags {
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
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        node.left = px(place.x / ui_scale.0);
        node.top = px(place.y / ui_scale.0);
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
    for (mut transform, mut visibility, tank) in &mut dials {
        transform.rotation = Quat::from_rotation_y(view.yaw());
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
    fn the_shield_bar_tells_its_state() {
        let gauge = |state, cooldown| shield_gauge(&Shield { state, cooldown }, 9.0, theme::ACCENT);
        assert_eq!(gauge(ShieldState::Ready, 0.0).2, "READY");
        assert_eq!(gauge(ShieldState::Active(0.3), 0.0).2, "UP");
        // While it recharges, it counts the seconds down.
        let (fill, _, word) = gauge(ShieldState::Ready, 4.5);
        assert_eq!((fill, word.as_str()), (0.5, "5 s"));
    }
}
