//! The menus: main menu, pause, settings and game over.

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use tanku_core::{
    discovery::Standing,
    modes::{Difficulty, Unlock},
    protocol::{Lobby, Member, NAME_LENGTH, lettered, named},
    rules::{MAX_PLAYERS, PLAYTEST_LAST_SECTOR, TICK_RATE},
    seed::{self, VERSION},
    sim::{Playtest, Run, RunPhase, RunRequest, SimEvent},
    stats::{Genome, PLAYER_BUDGET, Stat},
};

use crate::{
    Overlay, Screen,
    camera::View,
    cli::Options,
    crew::LocalSeat,
    exhibit::{self, Exhibit, Show},
    garage::Garage,
    net, report,
    scene::Paints,
    session::{Drawn, Found, Game, Invitation, Notice, Session, SessionRequest, Sign},
    settings::{Action, Binding, League, Records, Settings},
    theme,
    unlocks::{self, Unlocks},
};

/// Seconds between the end of a run and the game over screen.
const GAME_OVER_DELAY: f32 = 1.4;
/// Meters of ground that the main menu shows from the bottom to the top of the screen.
const EXHIBIT_SPAN: f32 = 7.5;
/// Where the tank stands on the screen of the main menu, in meters from its middle.
const EXHIBIT_PLACE: Vec2 = Vec2::new(3.2, 0.2);
/// The longest address that the join screen takes.
const ADDRESS_LENGTH: usize = 64;
/// The longest seed that a player types: as many letters as the screens have room for.
const SEED_LENGTH: usize = 10;
/// How wide the columns of the table of the garage are: the stat, its level between the two
/// buttons that change it, what the level gives, its price and the price of one more.
const GARAGE_COLUMNS: [f32; 5] = [104.0, 106.0, 134.0, 52.0, 44.0];
/// What the solo screen says of hard mode when it is chosen.
const HARD_MODE: &str = "In hard mode, enemies have more points, every brain from the first \
                         sector, and the guns and the experience of twenty sectors further.";

/// What a button does.
#[derive(Component, Clone, Copy, PartialEq)]
enum Press {
    /// Opens the screen where a single player sets up a run.
    Solo,
    /// On that screen: starts the run.
    Begin,
    /// On that screen: draws another seed.
    Redraw,
    /// On that screen: sets how hard the run is.
    Difficulty(Difficulty),
    /// On that screen: turns free mode on or off.
    Free(bool),
    /// On that screen: opens the garage.
    OpenGarage,
    /// In the garage: gives the tank a level more or less of a stat.
    Level(Stat, i8),
    /// In the garage: brings the standard tank back.
    Standard,
    /// Leaves the garage for the screen that opened it.
    CloseGarage,
    /// On the main menu: lists the best runs of a league.
    List(League),
    OpenAchievements,
    /// Opens the screen where a host sets up a game.
    OpenHost,
    /// On that screen: opens the game on this machine, for others to join.
    Host,
    /// Opens the screen where the address of a game is typed.
    OpenJoin,
    /// Joins the game at the address that was typed.
    Join,
    /// Joins a game that was found on the local network, by the number of that game.
    JoinFound(u64),
    /// In a lobby: the host starts a run with everyone who is there.
    Start,
    /// In a lobby: the host draws another seed.
    Reseed,
    /// In a lobby: the host is shown how players of that kind join.
    Show(Way),
    /// In a lobby: the host copies what the link for browsers ends with.
    Copy,
    /// The host ends the run and brings everyone back to the lobby.
    Regroup,
    OpenSettings,
    Quit,
    Resume,
    /// Leaves the run or the game of several players, for the main menu.
    Abandon,
    Retry,
    NewRun,
    Back,
    Fullscreen,
    Vsync,
    Shake,
    Volume(f32),
    Music(f32),
    Scale(f32),
    Rebind(Action, usize),
    ResetBindings,
    /// In a playtest: turns the infinite lives on or off.
    InfiniteLives,
    /// In a playtest: moves the sector to skip to by this many.
    Destination(i32),
    /// In a playtest: skips to that sector.
    Skip,
}

/// The button that the player most likely wants. It is filled with the accent.
#[derive(Component)]
struct Primary;

/// A button that is on: it is edged with the accent, and a press turns it off.
#[derive(Component)]
struct Lit;

#[derive(Component)]
struct PauseRoot;

#[derive(Component)]
struct SettingsRoot;

#[derive(Component)]
struct LobbyRoot;

#[derive(Component)]
struct SoloRoot;

#[derive(Component)]
struct GarageRoot;

/// Where the main menu lists the best runs of a difficulty.
#[derive(Component)]
struct RecordList;

/// The league whose best runs the main menu lists.
#[derive(Resource, Default)]
struct Listed(League);

/// The text that shows the address being typed.
#[derive(Component)]
struct AddressField;

/// The text that shows the seed of the solo screen, which the player may type.
#[derive(Component)]
struct SeedField;

/// The line that says how a game of several players goes: that joining is under way, or why
/// it failed.
#[derive(Component)]
struct StatusLine;

/// Where the join screen lists the games that were found on the local network.
#[derive(Component)]
struct FoundList;

/// The address that the player types to join a game.
#[derive(Resource, Default)]
struct Address(String);

/// The text that shows the name of a game as its host types it.
#[derive(Component)]
struct NameField;

/// What a host types for the game that they are about to open.
#[derive(Resource, Default)]
struct Opening {
    name: String,
}

/// What a single player has set for their next run.
#[derive(Resource, Default)]
struct Setup {
    seed: String,
    /// Whether the player typed the seed. The game drew it otherwise.
    typed: bool,
    difficulty: Difficulty,
    /// Whether the run is in free mode, in the tank of the player's garage.
    free: bool,
}

/// The players whom the lobby of a host tells how to join: those at a desktop game, or
/// those in a browser.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Way {
    #[default]
    Desktop,
    Browser,
}

/// Whether the last copy of the link for browsers reached the clipboard, and the seconds for
/// which the lobby still says so.
#[derive(Resource, Default)]
struct Copied(Option<(bool, f32)>);

/// The words on the button that copies the link for browsers.
#[derive(Component)]
struct CopyLabel;

/// Seconds for which the lobby says how a copy went.
const COPY_SHOWN: f32 = 2.0;

/// The sector a playtest offers to skip to.
#[derive(Resource)]
struct Destination(u32);

/// The binding the settings are waiting a key for.
#[derive(Resource, Default)]
struct Capture {
    waiting: Option<(Action, usize)>,
    /// The click that asked for a key isn't the key.
    armed: bool,
    /// A click that became a binding isn't a click on a button.
    swallowed: bool,
}

/// How the last run ended.
#[derive(Resource, Default)]
struct Outcome {
    countdown: Option<f32>,
    new_record: bool,
}

pub fn plugin(app: &mut App) {
    // The player's tank stands behind the menus that come before a run.
    let before_a_run = in_state(Screen::Menu)
        .or_else(in_state(Screen::Solo))
        .or_else(in_state(Screen::Garage))
        .or_else(in_state(Screen::Achievements))
        .or_else(in_state(Screen::Host))
        .or_else(in_state(Screen::Join))
        .or_else(in_state(Screen::Lobby));
    app.init_resource::<Capture>()
        .init_resource::<Outcome>()
        .init_resource::<Address>()
        .init_resource::<Opening>()
        .init_resource::<Setup>()
        .init_resource::<Listed>()
        .init_resource::<Way>()
        .init_resource::<Copied>()
        .insert_resource(Destination(1))
        .add_systems(
            OnEnter(Screen::Menu),
            (remember_the_last_run, main_menu).chain(),
        )
        // A player who comes back from the garage finds the run as they had set it up.
        .add_systems(
            OnTransition {
                exited: Screen::Menu,
                entered: Screen::Solo,
            },
            draw_a_seed_for_one,
        )
        .add_systems(OnEnter(Screen::Achievements), achievements_screen)
        .add_systems(OnEnter(Screen::Host), host_screen)
        .add_systems(OnEnter(Screen::Join), join_screen)
        .add_systems(OnEnter(Screen::Lobby), draw_a_seed)
        .add_systems(OnExit(Screen::Menu), put_the_exhibit_away)
        .add_systems(OnExit(Screen::Solo), put_the_exhibit_away)
        .add_systems(OnExit(Screen::Garage), put_the_exhibit_away)
        .add_systems(OnExit(Screen::Achievements), put_the_exhibit_away)
        .add_systems(OnExit(Screen::Host), put_the_exhibit_away)
        .add_systems(OnExit(Screen::Join), put_the_exhibit_away)
        .add_systems(OnExit(Screen::Lobby), put_the_exhibit_away)
        .add_systems(OnEnter(Screen::GameOver), game_over)
        .add_systems(
            OnTransition {
                exited: Overlay::None,
                entered: Overlay::Pause,
            },
            offer_the_next_sector.run_if(resource_exists::<Playtest>),
        )
        .add_systems(OnEnter(Overlay::Pause), pause_menu)
        .add_systems(OnEnter(Overlay::Settings), settings_menu)
        .add_systems(
            Update,
            (
                (
                    capture_binding,
                    type_address,
                    type_name,
                    type_seed,
                    press,
                    escape,
                )
                    .chain(),
                (solo_screen.run_if(in_state(Screen::Solo)), show_seed).chain(),
                garage_screen.run_if(in_state(Screen::Garage)),
                refresh_pause
                    .run_if(in_state(Overlay::Pause).and_then(resource_exists::<Playtest>)),
                refresh_settings.run_if(in_state(Overlay::Settings)),
                (lobby_screen.run_if(in_state(Screen::Lobby)), show_copy).chain(),
                list_games.run_if(in_state(Screen::Join)),
                list_records.run_if(in_state(Screen::Menu)),
                (show_address, show_name, tell_how_it_goes),
                hover,
                pause_time,
                (watch_run, keep_records).chain(),
                exhibit.run_if(before_a_run),
            ),
        );
}

/// The fine line around the screen, as around a drawing.
fn sheet_edge() -> impl Bundle {
    (
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
    )
}

/// The left margin of the sheet, where the menus that fill the screen stand. The right of the
/// sheet is left to what is drawn on it.
fn margin(
    root: &mut ChildSpawnerCommands,
    width: f32,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(120),
        top: px(90),
        width: px(width),
        flex_direction: FlexDirection::Column,
        row_gap: px(10),
        ..default()
    })
    .with_children(content);
}

/// A box with a ruled edge.
pub fn ruled(node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(px(1)),
            ..node
        },
        BackgroundColor(theme::SURFACE),
        BorderColor::all(theme::RULE),
    )
}

/// A screen-wide container with a sheet in its middle, which holds a column of content.
fn panel(
    commands: &mut Commands,
    scope: impl Bundle,
    scrim: Color,
    content: impl FnOnce(&mut ChildSpawnerCommands),
) {
    commands
        .spawn((
            scope,
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(scrim),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn(ruled(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                row_gap: px(10),
                padding: UiRect::all(px(28)),
                min_width: px(380),
                ..default()
            }))
            .with_children(content);
        });
}

fn heading(parent: &mut ChildSpawnerCommands, content: &str) {
    parent.spawn((
        theme::title(content, 40.0, theme::TEXT),
        Node {
            margin: UiRect::bottom(px(8)),
            ..default()
        },
    ));
}

fn line(parent: &mut ChildSpawnerCommands, content: impl Into<String>, color: Color) {
    parent.spawn(theme::text(content, 17.0, color));
}

/// The name of a group of rows, a little apart from what comes before it.
fn section(parent: &mut ChildSpawnerCommands, content: &str) {
    parent.spawn((
        theme::label(content, 13.0, theme::TEXT_DIM),
        Node {
            margin: UiRect::top(px(8)),
            ..default()
        },
    ));
}

fn button_node(width: Val) -> Node {
    Node {
        width,
        padding: UiRect::axes(px(16), px(10)),
        justify_content: JustifyContent::Center,
        border: UiRect::all(px(1)),
        ..default()
    }
}

fn button(parent: &mut ChildSpawnerCommands, content: impl Into<String>, press: Press) {
    parent.spawn((
        Button,
        press,
        button_node(Val::Auto),
        BackgroundColor(theme::SURFACE_RAISED),
        BorderColor::all(theme::RULE),
        children![theme::title(content, 20.0, theme::TEXT)],
    ));
}

/// The button that the player most likely wants.
fn primary(parent: &mut ChildSpawnerCommands, content: impl Into<String>, press: Press) {
    parent.spawn((
        Button,
        Primary,
        press,
        button_node(Val::Auto),
        BackgroundColor(theme::ACCENT),
        BorderColor::all(theme::ACCENT),
        children![theme::title(content, 20.0, theme::TEXT_ON_ACCENT)],
    ));
}

/// A label followed by its controls.
fn row(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    controls: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn(Node {
            column_gap: px(6),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            // The label sits in a box of fixed width, so that the controls line up.
            row.spawn((
                Node {
                    width: px(200),
                    ..default()
                },
                children![theme::text(label, 16.0, theme::TEXT)],
            ));
            controls(row);
        });
}

/// A small button, for the controls of a row.
fn small(parent: &mut ChildSpawnerCommands, content: impl Into<String>, width: f32, press: Press) {
    small_with(
        parent,
        theme::text(content, 16.0, theme::TEXT),
        width,
        press,
    );
}

/// A small button around words of its own, which may change.
fn small_with(parent: &mut ChildSpawnerCommands, words: impl Bundle, width: f32, press: Press) {
    parent.spawn((
        Button,
        press,
        Node {
            padding: UiRect::axes(px(6), px(5)),
            ..button_node(px(width))
        },
        BackgroundColor(theme::SURFACE_RAISED),
        BorderColor::all(theme::RULE),
        children![words],
    ));
}

/// One of the choices of a row. The one that is chosen stands in the accent and takes no
/// press, and each of the others is a small button.
fn choice(
    parent: &mut ChildSpawnerCommands,
    content: &str,
    width: f32,
    press: Press,
    chosen: bool,
) {
    if !chosen {
        return small(parent, content, width, press);
    }
    parent.spawn((
        Node {
            padding: UiRect::axes(px(6), px(5)),
            ..button_node(px(width))
        },
        BorderColor::all(theme::ACCENT),
        children![theme::text(content, 16.0, theme::ACCENT)],
    ));
}

/// A figure of a run with its name above it, in a row of figures.
fn figure(parent: &mut ChildSpawnerCommands, name: &str, value: String, last: bool) {
    parent.spawn((
        Node {
            flex_grow: 1.0,
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
        children![
            theme::label(name, 12.0, theme::TEXT_DIM),
            theme::figure(value, 36.0, theme::TEXT),
        ],
    ));
}

fn on_off(on: bool) -> &'static str {
    if on { "On" } else { "Off" }
}

/// A level of sound that the settings can show: a whole number of tenths, from 0 to 1.
fn level(wanted: f32) -> f32 {
    (wanted * 10.0).round().clamp(0.0, 10.0) / 10.0
}

/// The best run of a league and of a team of some size, or of a single player.
fn record_line(records: &Records, league: League, players: u32) -> String {
    let (best, none) = match league.words() {
        Some(words) => (
            format!("Best run in {words}"),
            format!("No run in {words} yet"),
        ),
        None => ("Best run".to_string(), "No run yet".to_string()),
    };
    match records.best(league, players) {
        Some(record) => format!(
            "{best}: sector {}, seed {}, v{}",
            record.sector, record.seed, record.version
        ),
        None => none,
    }
}

/// The best runs of a league, a line for each size of team that has one: a single player
/// first.
fn records_of(records: &Records, league: League) -> Vec<String> {
    let teams = (1..=MAX_PLAYERS).filter_map(|players| {
        let record = records.best(league, players)?;
        let whose = match players {
            1 => "Best run".to_string(),
            players => format!("{players} players"),
        };
        Some(format!(
            "{whose}: sector {}, seed {}, v{}",
            record.sector, record.seed, record.version
        ))
    });
    teams.collect()
}

/// The seed of a run with the version of the game and, in hard mode, the difficulty: what
/// tells one level from another.
fn level_line(run: &Run) -> String {
    let level = format!("Seed {}, v{VERSION}", run.seed_text);
    match run.difficulty {
        Difficulty::Normal => level,
        Difficulty::Hard => format!("{level}, hard mode"),
    }
}

/// A line that tells the player how a game of several players goes.
fn status_line(parent: &mut ChildSpawnerCommands) {
    parent.spawn((StatusLine, theme::text("", 17.0, theme::DANGER)));
}

/// The main menu opens on what the player chose last for a run of their own: it draws the
/// tank of that run, and first lists the best runs of its league.
fn remember_the_last_run(
    settings: Res<Settings>,
    unlocks: Res<Unlocks>,
    options: Res<Options>,
    mut setup: ResMut<Setup>,
    mut listed: ResMut<Listed>,
) {
    setup.difficulty = last_difficulty(&settings, &unlocks, &options);
    setup.free = last_free(&settings, &unlocks, &options);
    listed.0 = League {
        difficulty: setup.difficulty,
        free: setup.free,
    };
}

/// A small button of the record list that turns a league on or off. It is lit while its
/// league is the one that is listed.
fn league_switch(parent: &mut ChildSpawnerCommands, name: &str, to: League, lit: bool) {
    let node = Node {
        padding: UiRect::axes(px(8), px(2)),
        border: UiRect::all(px(1)),
        ..default()
    };
    let (edge, ink) = if lit {
        (theme::ACCENT, theme::ACCENT)
    } else {
        (theme::RULE, theme::TEXT)
    };
    let button = (
        Button,
        Press::List(to),
        node,
        BackgroundColor(theme::SURFACE_RAISED),
        BorderColor::all(edge),
        children![theme::label(name, 12.0, ink)],
    );
    if lit {
        parent.spawn((button, Lit));
    } else {
        parent.spawn(button);
    }
}

/// Lists the best runs of a league on the main menu, again whenever there is a new one or
/// the player asks for those of another league. Hard mode and free mode each have a switch,
/// once the player has a run in them.
fn list_records(
    mut commands: Commands,
    records: Res<Records>,
    listed: Res<Listed>,
    lists: Query<(Entity, Ref<RecordList>)>,
) {
    for (list, fresh) in &lists {
        if !records.is_changed() && !listed.is_changed() && !fresh.is_added() {
            continue;
        }
        let league = listed.0;
        let hard = league.difficulty == Difficulty::Hard;
        // A switch is there while it is on, and once some run was of its kind.
        let played = |of_kind: fn(League) -> bool| {
            League::all()
                .filter(|league| of_kind(*league))
                .any(|league| !records_of(&records, league).is_empty())
        };
        let offers_hard = hard || played(|league| league.difficulty == Difficulty::Hard);
        let offers_free = league.free || played(|league| league.free);
        let mut lines = records_of(&records, league);
        if lines.is_empty() {
            lines.push(match league.words() {
                Some(words) => format!("No run in {words} yet"),
                None => "No run yet".to_string(),
            });
        }
        commands
            .entity(list)
            .despawn_related::<Children>()
            .with_children(|record| {
                let header = Node {
                    align_items: AlignItems::Center,
                    column_gap: px(8),
                    ..default()
                };
                record.spawn(header).with_children(|header| {
                    header.spawn((
                        Node {
                            flex_grow: 1.0,
                            ..default()
                        },
                        children![theme::label("RECORD", 12.0, theme::TEXT_DIM)],
                    ));
                    if offers_hard {
                        let difficulty = if hard {
                            Difficulty::Normal
                        } else {
                            Difficulty::Hard
                        };
                        let to = League {
                            difficulty,
                            ..league
                        };
                        league_switch(header, "Hard mode", to, hard);
                    }
                    if offers_free {
                        let to = League {
                            free: !league.free,
                            ..league
                        };
                        league_switch(header, "Free mode", to, league.free);
                    }
                });
                for line in lines {
                    record.spawn(theme::text(line, 16.0, theme::TEXT));
                }
            });
    }
}

fn main_menu(
    mut commands: Commands,
    playtest: Option<Res<Playtest>>,
    (setup, options, garage): (Res<Setup>, Res<Options>, Res<Garage>),
) {
    // What the drawing shows: the standard tank, the one that the player built for free
    // mode or the one that a playtest brought. Those two may have points left over.
    let tank = shown_tank(Screen::Menu, &setup, &options, &garage);
    let spent = format!("{} OF {} POINTS", tank.cost(), tank.budget);
    let (drawn, budget) = match (options.tank.is_some(), is_free(&setup, &options)) {
        (true, _) => ("PLAYTEST TANK", spent),
        (false, true) => ("GARAGE TANK", spent),
        (false, false) => ("PLAYER TANK", format!("{} POINTS", tank.budget)),
    };
    commands
        .spawn((
            DespawnOnExit(Screen::Menu),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(10),
            children![sheet_edge()],
        ))
        .with_children(|root| {
            // The menu is the left margin of the sheet. The tank is drawn on its right.
            margin(root, 400.0, |parent| {
                parent.spawn(theme::figure(
                    format!("VERSION {VERSION}"),
                    15.0,
                    theme::TEXT_DIM,
                ));
                parent.spawn((
                    theme::title("TANKU", 112.0, theme::TEXT),
                    Node {
                        margin: UiRect::bottom(px(14)),
                        ..default()
                    },
                ));
                primary(parent, "Solo", Press::Solo);
                // A browser can't accept connections, so it hosts no game.
                if cfg!(not(target_arch = "wasm32")) {
                    button(parent, "Host a game", Press::OpenHost);
                }
                button(parent, "Join a game", Press::OpenJoin);
                button(parent, "Achievements", Press::OpenAchievements);
                button(parent, "Settings", Press::OpenSettings);
                // A browser tab is closed by the browser.
                if cfg!(not(target_arch = "wasm32")) {
                    button(parent, "Quit", Press::Quit);
                }
                // `list_records` fills it in.
                parent.spawn((
                    RecordList,
                    ruled(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        padding: UiRect::axes(px(12), px(8)),
                        margin: UiRect::top(px(8)),
                        ..default()
                    }),
                ));
                status_line(parent);
                if playtest.is_some() {
                    line(
                        parent,
                        "Playtest: cheats are in the pause menu",
                        theme::ACCENT,
                    );
                    line(parent, "A playtest sets no record", theme::TEXT_DIM);
                    if let Some(seed) = &options.seed {
                        line(
                            parent,
                            format!("Every run is on seed {seed}"),
                            theme::TEXT_DIM,
                        );
                    }
                }
            });
            // What the drawing shows, as in the corner of a sheet.
            root.spawn((
                ruled(Node {
                    position_type: PositionType::Absolute,
                    right: px(48),
                    bottom: px(48),
                    ..default()
                }),
                children![
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(4),
                            padding: UiRect::axes(px(12), px(8)),
                            border: UiRect::right(px(1)),
                            ..default()
                        },
                        BorderColor::all(theme::RULE),
                        children![
                            theme::label("DRAWING", 12.0, theme::TEXT_DIM),
                            theme::title(drawn, 18.0, theme::TEXT),
                        ],
                    ),
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: px(4),
                            padding: UiRect::axes(px(12), px(8)),
                            ..default()
                        },
                        children![
                            theme::label("BUDGET", 12.0, theme::TEXT_DIM),
                            theme::figure(budget, 18.0, theme::TEXT),
                        ],
                    ),
                ],
            ));
        });
}

/// Draws the player's tank large on the sheet behind the menus that come before a run, inside
/// the circle that stands for it when something hits it and the one that its shield reaches.
///
/// It waits for the menu to be on screen: the first one comes before the paints are mixed.
fn exhibit(
    mut commands: Commands,
    exhibits: Query<Entity, With<Exhibit>>,
    paints: Res<Paints>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut view: ResMut<View>,
    screen: Res<State<Screen>>,
    (setup, options, garage): (Res<Setup>, Res<Options>, Res<Garage>),
    mut shown: Local<Option<(Screen, Genome)>>,
) {
    // The drawing follows the tank: every change in the garage shows at once.
    let genome = shown_tank(*screen.get(), &setup, &options, &garage);
    let wanted = (*screen.get(), genome);
    if !exhibits.is_empty() && *shown == Some(wanted) {
        return;
    }
    for stale in &exhibits {
        commands.entity(stale).despawn();
    }
    *shown = Some(wanted);
    let show = Show {
        genome,
        color: theme::PLAYER,
        span: EXHIBIT_SPAN,
        place: EXHIBIT_PLACE,
        turret: 0.35,
        sways: true,
        shield: genome.shield_cooldown().is_some(),
    };
    exhibit::put(
        &mut commands,
        &mut meshes,
        &paints,
        &mut view,
        wanted.0,
        &show,
    );
}

/// The tank that the menus draw on a screen: the one that the player takes into a run from
/// there.
fn shown_tank(screen: Screen, setup: &Setup, options: &Options, garage: &Garage) -> Genome {
    let alone = matches!(screen, Screen::Menu | Screen::Solo | Screen::Achievements);
    match screen {
        // The garage draws the tank that is being built.
        Screen::Garage => garage.tank,
        // A single player in free mode has the tank that they built or brought.
        _ if alone && is_free(setup, options) => own_tank(options, garage),
        // Several players have the standard tank, and the host of a playtest the one that
        // it brought.
        _ => options.tank.unwrap_or(Genome::PLAYER),
    }
}

/// What a key does to the address being typed.
fn typed(address: &mut String, key: &Key, text: Option<&str>) {
    if *key == Key::Backspace {
        address.pop();
        return;
    }
    let fits = |letter: &char| letter.is_ascii_alphanumeric() || matches!(letter, '.' | ':' | '-');
    for letter in text.unwrap_or_default().chars().filter(fits) {
        if address.len() < ADDRESS_LENGTH {
            address.push(letter);
        }
    }
}

/// Asks to join the game at the address that was typed, and keeps the address for next time.
fn ask_to_join(
    address: &Address,
    settings: &mut Settings,
    requests: &mut MessageWriter<SessionRequest>,
) {
    let typed = address.0.trim();
    if settings.address != typed {
        settings.address = typed.to_string();
    }
    requests.write(SessionRequest::Join(typed.to_string()));
}

/// Takes what the player types on the join screen. Enter joins.
fn type_address(
    mut keys: MessageReader<KeyboardInput>,
    screen: Res<State<Screen>>,
    session: Res<Session>,
    mut address: ResMut<Address>,
    mut settings: ResMut<Settings>,
    mut requests: MessageWriter<SessionRequest>,
) {
    // An address that is being joined stays as it is.
    let typing = *screen.get() == Screen::Join && *session == Session::Solo;
    for key in keys.read() {
        if !typing || key.state != ButtonState::Pressed {
            continue;
        }
        if key.logical_key == Key::Enter {
            ask_to_join(&address, &mut settings, &mut requests);
        } else {
            typed(&mut address.0, &key.logical_key, key.text.as_deref());
        }
    }
}

/// Shows the address as it is typed, with a mark that blinks where the next letter goes.
fn show_address(
    time: Res<Time<Real>>,
    session: Res<Session>,
    address: Res<Address>,
    mut fields: Query<&mut Text, With<AddressField>>,
) {
    let blinks = *session == Session::Solo && time.elapsed_secs() % 1.0 < 0.5;
    let wanted = format!("{}{}", address.0, if blinks { "_" } else { "" });
    for mut text in &mut fields {
        if text.0 != wanted {
            text.0 = wanted.clone();
        }
    }
}

/// Says how a game of several players goes: that the game is being looked for, or why it
/// could not be joined, or how it ended.
fn tell_how_it_goes(
    screen: Res<State<Screen>>,
    session: Res<Session>,
    notice: Res<Notice>,
    address: Res<Address>,
    mut lines: Query<(&mut Text, &mut TextColor), With<StatusLine>>,
) {
    let (wanted, color) = match (&notice.0, *session, *screen.get()) {
        // A browser was given a link, and no address was typed.
        (_, Session::Guest, Screen::Join) if cfg!(target_arch = "wasm32") => (
            "Looking for the game of the link...".to_string(),
            theme::TEXT_DIM,
        ),
        (_, Session::Guest, Screen::Join) => (
            format!("Looking for a game at {}...", address.0.trim()),
            theme::TEXT_DIM,
        ),
        (Some(notice), ..) => (notice.clone(), theme::DANGER),
        (None, ..) => (String::new(), theme::TEXT_DIM),
    };
    for (mut text, mut shade) in &mut lines {
        if text.0 != wanted {
            text.0 = wanted.clone();
        }
        shade.set_if_neq(TextColor(color));
    }
}

/// Draws a seed for the run that a single player sets up. Its difficulty and its mode are
/// those that the player chose last, if they are still theirs to choose.
fn draw_a_seed_for_one(
    options: Res<Options>,
    time: Res<Time<Real>>,
    settings: Res<Settings>,
    unlocks: Res<Unlocks>,
    mut setup: ResMut<Setup>,
) {
    *setup = Setup {
        seed: new_seed(&options, time.elapsed().as_nanos() as u64),
        typed: false,
        difficulty: last_difficulty(&settings, &unlocks, &options),
        free: last_free(&settings, &unlocks, &options),
    };
}

/// Whether a mode or an option is open to a single player. A playtest has them all.
fn open(unlocks: &Unlocks, options: &Options, unlock: Unlock) -> bool {
    options.playtest || unlocks.has(unlock)
}

/// Whether a single player may play a difficulty.
fn plays(unlocks: &Unlocks, options: &Options, difficulty: Difficulty) -> bool {
    let unlock = difficulty.unlock();
    unlock.is_none_or(|unlock| open(unlocks, options, unlock))
}

/// The difficulty that the player chose last for a run of their own, if it is still theirs
/// to choose: a choice that was made in a playtest is none for a run that counts.
fn last_difficulty(settings: &Settings, unlocks: &Unlocks, options: &Options) -> Difficulty {
    Some(settings.solo_difficulty)
        .filter(|&difficulty| plays(unlocks, options, difficulty))
        .unwrap_or_default()
}

/// Whether the player chose free mode for the last run of their own, if it is still theirs
/// to choose.
fn last_free(settings: &Settings, unlocks: &Unlocks, options: &Options) -> bool {
    settings.solo_free && open(unlocks, options, Unlock::FreeMode)
}

/// Whether the next run of a single player is in free mode: they chose it, or a playtest
/// brought a tank of its own, which every one of its runs is in.
fn is_free(setup: &Setup, options: &Options) -> bool {
    setup.free || options.tank.is_some()
}

/// The tank that a single player has in free mode: the one that a playtest brought, or else
/// the one of their garage.
fn own_tank(options: &Options, garage: &Garage) -> Genome {
    options.tank.unwrap_or(garage.tank)
}

/// What a key does to the seed being typed. A seed is made of letters and digits, and reads
/// the same in small letters and in capitals.
fn typed_seed(seed: &mut String, key: &Key, text: Option<&str>) {
    if *key == Key::Backspace {
        seed.pop();
        return;
    }
    let letters = text.unwrap_or_default().chars();
    for letter in letters.filter(char::is_ascii_alphanumeric) {
        if seed.len() < SEED_LENGTH {
            seed.push(letter.to_ascii_uppercase());
        }
    }
}

/// What starts the run of a single player: on a seed that the game drew for it or not, in a
/// league, and in free mode in the tank that the player built or brought.
fn solo_run(
    seed: String,
    drawn: bool,
    league: League,
    options: &Options,
    garage: &Garage,
) -> RunRequest {
    let tanks = if league.free {
        vec![own_tank(options, garage)]
    } else {
        Vec::new()
    };
    RunRequest::Start {
        seed,
        players: 1,
        drawn,
        difficulty: league.difficulty,
        free: league.free,
        tanks,
    }
}

/// The league of the run that a single player set up.
fn league_of(setup: &Setup, options: &Options) -> League {
    League {
        difficulty: setup.difficulty,
        free: is_free(setup, options),
    }
}

/// Starts the run that a single player set up. A run needs a seed.
fn begin(
    setup: &Setup,
    (options, garage): (&Options, &Garage),
    requests: &mut MessageWriter<RunRequest>,
    screen: &mut NextState<Screen>,
) {
    if setup.seed.is_empty() {
        return;
    }
    let (seed, league) = (setup.seed.clone(), league_of(setup, options));
    requests.write(solo_run(seed, !setup.typed, league, options, garage));
    screen.set(Screen::Playing);
}

/// Takes what the player types on the solo screen: the seed of the run, once typing one is
/// unlocked. Enter starts the run.
fn type_seed(
    mut keys: MessageReader<KeyboardInput>,
    screen: Res<State<Screen>>,
    unlocks: Res<Unlocks>,
    (options, garage): (Res<Options>, Res<Garage>),
    mut setup: ResMut<Setup>,
    mut requests: MessageWriter<RunRequest>,
    mut next_screen: ResMut<NextState<Screen>>,
) {
    let setting_up = *screen.get() == Screen::Solo;
    let typing = setting_up && open(&unlocks, &options, Unlock::ChosenSeed);
    for key in keys.read() {
        if !setting_up || key.state != ButtonState::Pressed {
            continue;
        }
        if key.logical_key == Key::Enter {
            begin(&setup, (&options, &garage), &mut requests, &mut next_screen);
        } else if typing {
            // The first key takes the place of a seed that the game drew.
            let mut seed = String::new();
            if setup.typed {
                seed.clone_from(&setup.seed);
            }
            typed_seed(&mut seed, &key.logical_key, key.text.as_deref());
            if seed != setup.seed {
                (setup.seed, setup.typed) = (seed, true);
            }
        }
    }
}

/// Shows the seed of the solo screen, with a mark that blinks where the next letter goes
/// when the player may type one.
fn show_seed(
    time: Res<Time<Real>>,
    unlocks: Res<Unlocks>,
    options: Res<Options>,
    setup: Res<Setup>,
    mut fields: Query<&mut Text, With<SeedField>>,
) {
    // A seed that is as long as it gets takes no further letter.
    let typing = open(&unlocks, &options, Unlock::ChosenSeed) && setup.seed.len() < SEED_LENGTH;
    let blinks = typing && time.elapsed_secs() % 1.0 < 0.5;
    let wanted = format!("{}{}", setup.seed, if blinks { "_" } else { "" });
    for mut text in &mut fields {
        if text.0 != wanted {
            text.0 = wanted.clone();
        }
    }
}

/// The screen where a single player sets up a run: its difficulty, its seed and its options.
/// What is locked says what unlocks it.
fn solo_screen(
    mut commands: Commands,
    setup: Res<Setup>,
    unlocks: Res<Unlocks>,
    (options, garage): (Res<Options>, Res<Garage>),
    roots: Query<Entity, With<SoloRoot>>,
) {
    let stale = setup.is_changed() || unlocks.is_changed() || garage.is_changed();
    if !roots.is_empty() && !stale {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    let hard = plays(&unlocks, &options, Difficulty::Hard);
    let types = open(&unlocks, &options, Unlock::ChosenSeed);
    // A playtest that brought a tank plays every run in it: it has no garage to open.
    let (free, brought) = (is_free(&setup, &options), options.tank.is_some());
    let frees = open(&unlocks, &options, Unlock::FreeMode);
    commands
        .spawn((
            SoloRoot,
            DespawnOnExit(Screen::Solo),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(10),
            children![sheet_edge()],
        ))
        .with_children(|root| {
            margin(root, 460.0, |parent| {
                parent.spawn(theme::label(
                    "Run of a single player",
                    13.0,
                    theme::TEXT_DIM,
                ));
                parent.spawn(theme::title("Solo", 84.0, theme::TEXT));
                section(parent, "Run");
                row(parent, "Difficulty", |row| {
                    // A player who has a single difficulty has nothing to choose.
                    if !hard {
                        row.spawn(theme::text("Normal", 16.0, theme::TEXT));
                        return;
                    }
                    for difficulty in Difficulty::ALL {
                        let name = unlocks::difficulty_name(difficulty);
                        let chosen = setup.difficulty == difficulty;
                        choice(row, name, 90.0, Press::Difficulty(difficulty), chosen);
                    }
                });
                if !hard {
                    row(parent, "Hard mode", |row| {
                        let unlock = unlocks::condition(Unlock::HardMode);
                        row.spawn(theme::text(unlock, 16.0, theme::TEXT_DIM));
                    });
                } else if setup.difficulty == Difficulty::Hard {
                    line(parent, HARD_MODE, theme::TEXT_DIM);
                }
                row(parent, "Free mode", |row| {
                    if brought {
                        let told = "On, in the tank of this playtest";
                        row.spawn(theme::text(told, 16.0, theme::TEXT));
                    } else if frees {
                        for (name, on) in [("Off", false), ("On", true)] {
                            choice(row, name, 90.0, Press::Free(on), setup.free == on);
                        }
                    } else {
                        let unlock = unlocks::condition(Unlock::FreeMode);
                        row.spawn(theme::text(unlock, 16.0, theme::TEXT_DIM));
                    }
                });
                // In free mode, the player's tank is the one of their garage.
                if free && !brought {
                    row(parent, "Tank", |row| {
                        let spent = format!("{} of {PLAYER_BUDGET} points", garage.tank.cost());
                        row.spawn((
                            Node {
                                min_width: px(130),
                                ..default()
                            },
                            children![theme::text(spent, 16.0, theme::TEXT)],
                        ));
                        small(row, "Garage", 110.0, Press::OpenGarage);
                    });
                }
                row(parent, "Seed", |row| {
                    row.spawn((
                        Node {
                            min_width: px(130),
                            ..default()
                        },
                        children![(
                            SeedField,
                            theme::code(setup.seed.clone(), 16.0, theme::TEXT)
                        )],
                    ));
                    small(row, "New seed", 110.0, Press::Redraw);
                });
                let hint = if types {
                    "Type a seed of your own, in letters and digits.".to_string()
                } else {
                    let unlock = unlocks::condition(Unlock::ChosenSeed).to_lowercase();
                    format!("To type a seed of your own: {unlock}.")
                };
                line(parent, hint, theme::TEXT_DIM);
                // A playtest earns nothing either way, and says what it is elsewhere.
                let unearned = match (free, setup.typed) {
                    (true, _) => Some("A run in free mode earns no achievement."),
                    (false, true) => Some("A run on a typed seed earns no achievement."),
                    (false, false) => None,
                };
                if let Some(why) = unearned.filter(|_| unlocks.wanting() && !options.playtest) {
                    line(parent, why, theme::TEXT);
                }
                primary(parent, "Start", Press::Begin);
                button(parent, "Back", Press::Abandon);
            });
        });
}

/// A cell of the table of the garage: a box of a width that holds what a column tells of a
/// stat.
fn cell(parent: &mut ChildSpawnerCommands, width: f32, content: impl Bundle) {
    parent.spawn((
        Node {
            width: px(width),
            ..default()
        },
        children![content],
    ));
}

/// The button that gives the tank of the garage a level more or less of a stat. Where the
/// garage lets no such level be, a dim box of the same size stands in its place.
fn step(parent: &mut ChildSpawnerCommands, garage: &Garage, stat: Stat, by: i8) {
    let sign = if by > 0 { "+" } else { "-" };
    if garage.changed(stat, by).is_some() {
        return small(parent, sign, 34.0, Press::Level(stat, by));
    }
    parent.spawn((
        Node {
            padding: UiRect::axes(px(6), px(5)),
            ..button_node(px(34.0))
        },
        BorderColor::all(theme::RULE_DIM),
        children![theme::text(sign, 16.0, theme::RULE)],
    ));
}

/// What the level of a stat gives the tank of the garage. A stat that a tank does without
/// says so: the report of a run only lists the parts that a tank has.
fn gives(tank: &Genome, stat: Stat) -> String {
    let without = matches!(stat, Stat::Ricochet | Stat::Mines | Stat::Piercing);
    if without && tank.level(stat) == 0 {
        return "None".to_string();
    }
    report::gives(tank, stat)
}

/// The garage: where a player spends the points of their tank for free mode. Every stat has
/// its level, what that level gives, what it costs and what one more level would cost. The
/// tank is drawn next to the list as it will look, and follows every change.
fn garage_screen(
    mut commands: Commands,
    garage: Res<Garage>,
    roots: Query<Entity, With<GarageRoot>>,
) {
    if !roots.is_empty() && !garage.is_changed() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    let tank = garage.tank;
    commands
        .spawn((
            GarageRoot,
            DespawnOnExit(Screen::Garage),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(10),
            children![sheet_edge()],
        ))
        .with_children(|root| {
            margin(root, 500.0, |parent| {
                parent.spawn(theme::label("Tank of free mode", 13.0, theme::TEXT_DIM));
                parent.spawn(theme::title("Garage", 84.0, theme::TEXT));
                let table = Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    padding: UiRect::axes(px(12), px(8)),
                    ..default()
                };
                parent.spawn(ruled(table)).with_children(|table| {
                    let line = Node {
                        column_gap: px(6),
                        align_items: AlignItems::Center,
                        ..default()
                    };
                    let head = |name: &'static str| theme::label(name, 12.0, theme::TEXT_DIM);
                    table.spawn(line.clone()).with_children(|names| {
                        cell(names, GARAGE_COLUMNS[0], head("Stat"));
                        cell(names, GARAGE_COLUMNS[1], head("Level"));
                        cell(names, GARAGE_COLUMNS[2], head("Gives"));
                        cell(names, GARAGE_COLUMNS[3], head("Price"));
                        cell(names, GARAGE_COLUMNS[4], head("Next"));
                    });
                    for stat in Stat::GARAGE {
                        let level = tank.level(stat);
                        // What one more level would cost, where there is one.
                        let next = (level < stat.max_level())
                            .then(|| stat.price(level + 1) - stat.price(level));
                        let next = next.map_or(String::new(), |price| format!("+{price}"));
                        let name = report::stat_name(stat);
                        table.spawn(line.clone()).with_children(|row| {
                            cell(row, GARAGE_COLUMNS[0], theme::text(name, 16.0, theme::TEXT));
                            let steps = Node {
                                width: px(GARAGE_COLUMNS[1]),
                                column_gap: px(6),
                                align_items: AlignItems::Center,
                                ..default()
                            };
                            row.spawn(steps).with_children(|steps| {
                                step(steps, &garage, stat, -1);
                                let figure = theme::figure(level.to_string(), 16.0, theme::TEXT);
                                steps.spawn((
                                    Node {
                                        width: px(20),
                                        justify_content: JustifyContent::Center,
                                        ..default()
                                    },
                                    children![figure],
                                ));
                                step(steps, &garage, stat, 1);
                            });
                            let gives = gives(&tank, stat);
                            cell(
                                row,
                                GARAGE_COLUMNS[2],
                                theme::text(gives, 16.0, theme::TEXT),
                            );
                            let price = stat.price(level).to_string();
                            cell(
                                row,
                                GARAGE_COLUMNS[3],
                                theme::figure(price, 16.0, theme::TEXT),
                            );
                            cell(
                                row,
                                GARAGE_COLUMNS[4],
                                theme::figure(next, 16.0, theme::TEXT_DIM),
                            );
                        });
                    }
                });
                parent
                    .spawn(ruled(Node {
                        align_self: AlignSelf::Stretch,
                        ..default()
                    }))
                    .with_children(|figures| {
                        let (spent, left) = (tank.cost(), garage.left());
                        figure(figures, "POINTS SPENT", spent.to_string(), false);
                        figure(figures, "POINTS LEFT", left.to_string(), true);
                    });
                primary(parent, "Done", Press::CloseGarage);
                button(parent, "Standard tank", Press::Standard);
            });
        });
}

/// The screen that lists what runs unlock for solo, and what those of the player have.
fn achievements_screen(mut commands: Commands, unlocks: Res<Unlocks>) {
    commands
        .spawn((
            DespawnOnExit(Screen::Achievements),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(10),
            children![sheet_edge()],
        ))
        .with_children(|root| {
            margin(root, 460.0, |parent| {
                parent.spawn(theme::label(
                    "What a run unlocks for solo",
                    13.0,
                    theme::TEXT_DIM,
                ));
                parent.spawn(theme::title("Achievements", 64.0, theme::TEXT));
                for unlock in Unlock::ALL {
                    let (state, color) = if unlocks.has(unlock) {
                        ("Unlocked", theme::GOOD)
                    } else {
                        ("Locked", theme::TEXT_DIM)
                    };
                    // What isn't built yet says so, before a player sets out to earn it.
                    let state = if unlocks::built(unlock) {
                        state.to_string()
                    } else {
                        format!("{state} \u{b7} not in the game yet")
                    };
                    parent
                        .spawn(ruled(Node {
                            align_self: AlignSelf::Stretch,
                            flex_direction: FlexDirection::Column,
                            row_gap: px(2),
                            padding: UiRect::axes(px(12), px(8)),
                            ..default()
                        }))
                        .with_children(|entry| {
                            entry.spawn(theme::title(unlocks::name(unlock), 20.0, theme::TEXT));
                            let condition = unlocks::condition(unlock);
                            entry.spawn(theme::text(condition, 16.0, theme::TEXT_DIM));
                            entry.spawn(theme::label(state, 13.0, color));
                        });
                }
                line(
                    parent,
                    "A run earns them on a seed that the game drew for it, alone or with others.",
                    theme::TEXT_DIM,
                );
                button(parent, "Back", Press::Abandon);
            });
        });
}

/// What a key does to words that a player types for others to read, such as the name of a
/// game: it adds the letters that the interface draws, as long as there is room.
fn typed_words(words: &mut String, key: &Key, text: Option<&str>, length: usize) {
    if *key == Key::Backspace {
        words.pop();
        return;
    }
    let letters = text.unwrap_or_default().chars();
    for letter in letters.filter(|letter| lettered(*letter)) {
        if words.chars().count() < length {
            words.push(letter);
        }
    }
}

/// Asks to open the game that a host set up, and keeps its name for the next one.
fn ask_to_host(
    opening: &Opening,
    settings: &mut Settings,
    requests: &mut MessageWriter<SessionRequest>,
) {
    let name = named(&opening.name);
    if settings.host_name != name {
        settings.host_name.clone_from(&name);
    }
    requests.write(SessionRequest::Host(Sign { name }));
}

/// Takes what the player types on the host screen: the name of the game. Enter opens it.
fn type_name(
    mut keys: MessageReader<KeyboardInput>,
    screen: Res<State<Screen>>,
    session: Res<Session>,
    mut opening: ResMut<Opening>,
    mut settings: ResMut<Settings>,
    mut requests: MessageWriter<SessionRequest>,
) {
    let typing = *screen.get() == Screen::Host && *session == Session::Solo;
    for key in keys.read() {
        if !typing || key.state != ButtonState::Pressed {
            continue;
        }
        if key.logical_key == Key::Enter {
            ask_to_host(&opening, &mut settings, &mut requests);
        } else {
            let mut name = opening.name.clone();
            typed_words(
                &mut name,
                &key.logical_key,
                key.text.as_deref(),
                NAME_LENGTH,
            );
            if name != opening.name {
                opening.name = name;
            }
        }
    }
}

/// Shows the name of a game as its host types it, with a mark that blinks where the next
/// letter goes while there is room for one.
fn show_name(
    time: Res<Time<Real>>,
    opening: Res<Opening>,
    mut fields: Query<&mut Text, With<NameField>>,
) {
    let room = opening.name.chars().count() < NAME_LENGTH;
    let blinks = room && time.elapsed_secs() % 1.0 < 0.5;
    let wanted = format!("{}{}", opening.name, if blinks { "_" } else { "" });
    for mut text in &mut fields {
        if text.0 != wanted {
            text.0.clone_from(&wanted);
        }
    }
}

/// The screen where a player sets up the game that they host for others.
fn host_screen(mut commands: Commands, settings: Res<Settings>, mut opening: ResMut<Opening>) {
    // The name of the game that was hosted last is the likeliest one.
    opening.name.clone_from(&settings.host_name);
    commands
        .spawn((
            DespawnOnExit(Screen::Host),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(10),
            children![sheet_edge()],
        ))
        .with_children(|root| {
            margin(root, 440.0, |parent| {
                parent.spawn(theme::label(
                    "Game of several players",
                    13.0,
                    theme::TEXT_DIM,
                ));
                parent.spawn((
                    theme::title("Host a game", 64.0, theme::TEXT),
                    Node {
                        margin: UiRect::bottom(px(8)),
                        ..default()
                    },
                ));
                section(parent, "Name of the game");
                parent.spawn((
                    ruled(Node {
                        padding: UiRect::axes(px(12), px(10)),
                        min_height: px(48),
                        align_items: AlignItems::Center,
                        ..default()
                    }),
                    children![(NameField, theme::text("", 22.0, theme::TEXT))],
                ));
                line(
                    parent,
                    "The others find the game under this name on their join screen. A game \
                     needs none. Enter opens it.",
                    theme::TEXT_DIM,
                );
                primary(parent, "Open the game", Press::Host);
                button(parent, "Back", Press::Abandon);
                status_line(parent);
            });
        });
}

/// The screen where the player types the address of a game to join.
fn join_screen(mut commands: Commands, settings: Res<Settings>, mut address: ResMut<Address>) {
    // The game that was joined last is the likeliest one.
    if address.0.is_empty() {
        address.0 = settings.address.clone();
    }
    commands
        .spawn((
            DespawnOnExit(Screen::Join),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(10),
            children![sheet_edge()],
        ))
        .with_children(|root| {
            margin(root, 440.0, |parent| {
                parent.spawn(theme::label(
                    "Game of several players",
                    13.0,
                    theme::TEXT_DIM,
                ));
                parent.spawn((
                    theme::title("Join a game", 64.0, theme::TEXT),
                    Node {
                        margin: UiRect::bottom(px(8)),
                        ..default()
                    },
                ));
                // A browser neither looks around a network nor joins by an address alone.
                if cfg!(target_arch = "wasm32") {
                    section(parent, "Link of the host");
                    for told in [
                        "A browser joins a game by the link that its host hands out.",
                        "Open that link in this browser, or paste it into this page.",
                    ] {
                        line(parent, told, theme::TEXT_DIM);
                    }
                    // A link that was followed before can be followed again.
                    if net::link().is_some() {
                        primary(parent, "Join the game of the link", Press::Join);
                    }
                    button(parent, "Back", Press::Abandon);
                    status_line(parent);
                    return;
                }
                section(parent, "Games on this network");
                parent.spawn((
                    FoundList,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(6),
                        ..default()
                    },
                ));
                section(parent, "Address of the host");
                parent.spawn((
                    ruled(Node {
                        padding: UiRect::axes(px(12), px(10)),
                        min_height: px(48),
                        align_items: AlignItems::Center,
                        ..default()
                    }),
                    children![(AddressField, theme::figure("", 22.0, theme::TEXT))],
                ));
                line(
                    parent,
                    "For a game that isn't listed: the lobby of its host shows it. Enter joins.",
                    theme::TEXT_DIM,
                );
                primary(parent, "Join", Press::Join);
                button(parent, "Back", Press::Abandon);
                status_line(parent);
            });
        });
}

/// What a row of the join screen says about a game that was found, besides its name and its
/// address.
fn found_words(game: &Game) -> String {
    match game.standing {
        Standing::Open | Standing::Full => {
            let seats = format!("{} of {MAX_PLAYERS} players", game.players);
            if game.standing == Standing::Full {
                format!("{seats} \u{b7} full")
            } else {
                seats
            }
        }
        Standing::Running => "Run in progress".to_string(),
        Standing::Other => "Another version of the game".to_string(),
    }
}

/// The row of a game that was found: its name over its address, or its address alone, and
/// what a player can do about it. One that can be joined is a button.
fn found_row(parent: &mut ChildSpawnerCommands, game: &Game) {
    let open = game.standing == Standing::Open;
    let ink = if open { theme::TEXT } else { theme::TEXT_DIM };
    let node = Node {
        flex_direction: FlexDirection::Column,
        row_gap: px(2),
        padding: UiRect::axes(px(16), px(8)),
        border: UiRect::all(px(1)),
        ..default()
    };
    let (address, words) = (game.address.clone(), found_words(game));
    let content = |row: &mut ChildSpawnerCommands| {
        let below = Node {
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: px(16),
            ..default()
        };
        if game.name.is_empty() {
            row.spawn(theme::figure(address, 20.0, ink));
            row.spawn(theme::text(words, 15.0, theme::TEXT_DIM));
        } else {
            row.spawn(theme::text(game.name.clone(), 18.0, ink));
            row.spawn((
                below,
                children![
                    theme::figure(address, 15.0, theme::TEXT_DIM),
                    theme::text(words, 15.0, theme::TEXT_DIM),
                ],
            ));
        }
    };
    if open {
        let button = (
            Button,
            Press::JoinFound(game.id),
            node,
            BackgroundColor(theme::SURFACE_RAISED),
            BorderColor::all(theme::RULE),
        );
        parent.spawn(button).with_children(content);
    } else {
        parent
            .spawn((node, BorderColor::all(theme::RULE_DIM)))
            .with_children(content);
    }
}

/// Lists the games that were found on the local network, again whenever one comes, goes or
/// changes.
fn list_games(mut commands: Commands, found: Res<Found>, lists: Query<(Entity, Ref<FoundList>)>) {
    for (list, fresh) in &lists {
        if !found.is_changed() && !fresh.is_added() {
            continue;
        }
        commands
            .entity(list)
            .despawn_related::<Children>()
            .with_children(|rows| {
                for game in &found.0 {
                    found_row(rows, game);
                }
                if found.0.is_empty() {
                    let none = "None so far. A game that is hosted on this network shows here.";
                    line(rows, none, theme::TEXT_DIM);
                }
            });
    }
}

/// What a host hands out for browsers, as the end of a link, in lines that fit the lobby:
/// where the game takes browsers, then the print of its certificate in two halves.
fn link_ending(code: &str) -> [String; 3] {
    let (place, print) = code.split_once('/').unwrap_or((code, ""));
    let (first, second) = print.split_at(print.len() / 2);
    [format!("#{place}/"), first.to_string(), second.to_string()]
}

/// What the button of the lobby copies: what the link for browsers ends with, mark and all.
/// It goes after the address of the game's page, or is pasted into that page.
fn link_end(code: &str) -> String {
    format!("#{code}")
}

/// What the button that copies the link says: what it does, or how it just went.
fn copy_label(copied: &Copied) -> &'static str {
    match copied.0 {
        None => "Copy",
        Some((true, _)) => "Copied",
        Some((false, _)) => "Not copied",
    }
}

/// Puts a text on the clipboard of the system. Says whether it got there: a machine may have
/// no clipboard that the game can reach.
fn copy(clipboard: Option<&mut Clipboard>, text: String) -> bool {
    clipboard.is_some_and(|clipboard| clipboard.set_text(text).is_ok())
}

/// Says on the button that copies the link how the last copy went, for a moment.
fn show_copy(
    time: Res<Time<Real>>,
    mut copied: ResMut<Copied>,
    mut labels: Query<&mut Text, With<CopyLabel>>,
) {
    if let Some((_, left)) = &mut copied.0 {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            copied.0 = None;
        }
    }
    let wanted = copy_label(&copied);
    for mut label in &mut labels {
        if label.0 != wanted {
            label.0 = wanted.to_string();
        }
    }
}

/// A line of the lobby that tells how to join, with a small button at its end.
fn told_row(
    parent: &mut ChildSpawnerCommands,
    told: impl Bundle,
    control: impl FnOnce(&mut ChildSpawnerCommands),
) {
    let row = Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        column_gap: px(12),
        ..default()
    };
    parent.spawn(row).with_children(|row| {
        row.spawn(told);
        control(row);
    });
}

/// How others join the game of this machine, in the lobby of its host: the players at a
/// desktop game, or those in a browser.
fn ways_to_join(
    parent: &mut ChildSpawnerCommands,
    invitation: &Invitation,
    way: Way,
    copied: &Copied,
) {
    let code = invitation.web.as_ref().filter(|_| way == Way::Browser);
    if let (Some(code), Some(port)) = (code, invitation.web_port) {
        let [place, first, second] = link_ending(code);
        section(parent, "Link for a browser");
        told_row(parent, theme::figure(place, 20.0, theme::TEXT), |row| {
            small(row, "For a desktop", 150.0, Press::Show(Way::Desktop));
        });
        // Nobody types the print: a button copies the lot.
        told_row(parent, theme::figure(first, 15.0, theme::TEXT), |row| {
            let words = (
                CopyLabel,
                theme::text(copy_label(copied), 16.0, theme::TEXT),
            );
            small_with(row, words, 150.0, Press::Copy);
        });
        parent.spawn(theme::figure(second, 15.0, theme::TEXT));
        let hint = "In a browser, it goes at the end of the address of the game's page, or is \
                    pasted into that page. The log has it too.";
        line(parent, hint, theme::TEXT_DIM);
        let hint = format!(
            "From the internet, your public address goes in place of this one, and UDP port \
             {port} must reach this machine."
        );
        line(parent, hint, theme::TEXT_DIM);
        return;
    }
    section(parent, "Address");
    let other = |row: &mut ChildSpawnerCommands| {
        if invitation.web.is_some() {
            small(row, "For a browser", 150.0, Press::Show(Way::Browser));
        }
    };
    match &invitation.address {
        Some(address) => {
            let told = theme::figure(address.clone(), 24.0, theme::TEXT);
            told_row(parent, told, other);
            let hint = "Players on this network find the game on their join screen, or type \
                        this address there.";
            line(parent, hint, theme::TEXT_DIM);
        }
        None => {
            let hint = "This machine has no address on a network.";
            told_row(parent, theme::text(hint, 17.0, theme::TEXT_DIM), other);
        }
    }
    line(
        parent,
        format!(
            "From the internet, they need your public address, and UDP port {} must reach \
             this machine.",
            invitation.port
        ),
        theme::TEXT_DIM,
    );
    // A game that can't listen for browsers says why none can join.
    if let (None, Some(port)) = (&invitation.web, invitation.web_port) {
        let hint = format!("No browser can join: UDP port {port} is taken on this machine.");
        line(parent, hint, theme::TEXT_DIM);
    }
}

/// The host draws a seed whenever the players gather in the lobby.
fn draw_a_seed(
    session: Res<Session>,
    options: Res<Options>,
    time: Res<Time<Real>>,
    mut lobbies: Query<&mut Lobby>,
    mut drawn: ResMut<Drawn>,
) {
    if let (Session::Host, Ok(mut lobby)) = (*session, lobbies.single_mut()) {
        lobby.seed = new_seed(&options, time.elapsed().as_nanos() as u64);
        drawn.0 = true;
    }
}

/// A seat of the lobby: its number and who sits there.
fn seat_row(parent: &mut ChildSpawnerCommands, seat: usize, member: Option<Member>, own: bool) {
    let taken = member.is_some();
    let color = theme::PLAYER;
    let who = match (member, own) {
        (None, _) => "Open",
        (Some(Member::Host), true) => "You, the host",
        (Some(Member::Host), false) => "The host",
        (Some(Member::Guest(_)), true) => "You",
        (Some(Member::Guest(_)), false) => "A guest",
    };
    let ink = if taken { theme::TEXT } else { theme::TEXT_DIM };
    parent.spawn((
        Node {
            column_gap: px(12),
            align_items: AlignItems::Center,
            padding: UiRect::axes(px(12), px(8)),
            border: if seat == 0 {
                UiRect::ZERO
            } else {
                UiRect::top(px(1))
            },
            ..default()
        },
        BorderColor::all(theme::RULE_DIM),
        children![
            (
                Node {
                    width: px(14),
                    height: px(14),
                    border: UiRect::all(px(1)),
                    ..default()
                },
                BackgroundColor(if taken { color } else { Color::NONE }),
                BorderColor::all(if taken { color } else { theme::RULE_DIM }),
            ),
            (
                Node {
                    width: px(110),
                    ..default()
                },
                children![theme::label(format!("Player {}", seat + 1), 13.0, ink)],
            ),
            theme::text(who, 16.0, ink),
        ],
    ));
}

/// The lobby: who is there, what they are about to play, and how others join. It is drawn
/// again whenever something it shows changes.
fn lobby_screen(
    mut commands: Commands,
    lobbies: Query<Ref<Lobby>>,
    session: Res<Session>,
    seat: Res<LocalSeat>,
    (invitation, way, copied): (Res<Invitation>, Res<Way>, Res<Copied>),
    options: Res<Options>,
    roots: Query<Entity, With<LobbyRoot>>,
) {
    let Ok(lobby) = lobbies.single() else {
        return;
    };
    let stale =
        lobby.is_changed() || seat.is_changed() || invitation.is_changed() || way.is_changed();
    if !roots.is_empty() && !stale {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    let host = *session == Session::Host;
    commands
        .spawn((
            LobbyRoot,
            DespawnOnExit(Screen::Lobby),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(10),
            children![sheet_edge()],
        ))
        .with_children(|root| {
            margin(root, 460.0, |parent| {
                // The game goes by the name that its host gave it.
                let role = if host { "host" } else { "guest" };
                let game = match lobby.name.as_str() {
                    "" => "Game of several players",
                    name => name,
                };
                parent.spawn(theme::label(
                    format!("{game} \u{b7} {role}"),
                    13.0,
                    theme::TEXT_DIM,
                ));
                parent.spawn(theme::title("Lobby", 84.0, theme::TEXT));
                section(parent, "Players");
                parent
                    .spawn(ruled(Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    }))
                    .with_children(|seats| {
                        for (number, member) in lobby.seats.iter().enumerate() {
                            seat_row(seats, number, *member, number == seat.0 as usize);
                        }
                    });
                section(parent, "Run");
                row(parent, "Difficulty", |row| {
                    row.spawn(theme::text("Normal", 16.0, theme::TEXT));
                });
                row(parent, "Seed", |row| {
                    row.spawn((
                        Node {
                            min_width: px(130),
                            ..default()
                        },
                        children![theme::code(lobby.seed.clone(), 16.0, theme::TEXT)],
                    ));
                    // A playtest that was given a seed keeps it.
                    if host && options.seed.is_none() {
                        small(row, "New seed", 110.0, Press::Reseed);
                    }
                });
                if host {
                    ways_to_join(parent, &invitation, *way, &copied);
                    let start = if lobby.players() > 1 {
                        "Start"
                    } else {
                        "Start alone"
                    };
                    primary(parent, start, Press::Start);
                    button(parent, "Close the game", Press::Abandon);
                } else {
                    line(parent, "The host starts the run.", theme::TEXT);
                    button(parent, "Leave the game", Press::Abandon);
                }
            });
        });
}

/// The game takes the view back.
fn put_the_exhibit_away(mut view: ResMut<View>) {
    view.release();
}

/// A playtest offers to skip the sector the player is in.
fn offer_the_next_sector(run: Option<Res<Run>>, mut destination: ResMut<Destination>) {
    if let Some(run) = run {
        destination.0 = (run.sector + 1).min(PLAYTEST_LAST_SECTOR);
    }
}

fn pause_menu(
    mut commands: Commands,
    run: Option<Res<Run>>,
    playtest: Option<Res<Playtest>>,
    destination: Res<Destination>,
    session: Res<Session>,
) {
    // Only a single player's run stands still while the menu is open.
    let paused = run.as_ref().is_none_or(|run| run.solo);
    panel(
        &mut commands,
        (PauseRoot, DespawnOnExit(Overlay::Pause)),
        theme::SCRIM,
        |parent| {
            heading(parent, if paused { "Paused" } else { "Menu" });
            if !paused {
                line(parent, "The run goes on meanwhile", theme::TEXT_DIM);
            }
            primary(parent, "Resume", Press::Resume);
            button(parent, "Settings", Press::OpenSettings);
            match *session {
                Session::Solo => button(parent, "Quit to the menu", Press::Abandon),
                Session::Host => {
                    button(parent, "Back to the lobby", Press::Regroup);
                    button(parent, "Close the game", Press::Abandon);
                }
                Session::Guest => button(parent, "Leave the game", Press::Abandon),
            }
            // The cheats are the host's: a guest's simulation is not here.
            if let Some(playtest) = playtest.filter(|_| *session != Session::Guest) {
                section(parent, "Playtest");
                row(parent, "Infinite lives", |row| {
                    let state = on_off(playtest.infinite_lives);
                    small(row, state, 70.0, Press::InfiniteLives);
                });
                row(parent, "Sector to skip to", |row| {
                    for change in [-10, -1, 1, 10] {
                        small(row, format!("{change:+}"), 48.0, Press::Destination(change));
                    }
                });
                let skip = format!("Skip to sector {}", destination.0);
                button(parent, skip, Press::Skip);
            }
            if let Some(run) = run {
                line(parent, level_line(&run), theme::TEXT_DIM);
            }
        },
    );
}

/// Draws the pause menu again when a playtest changed one of its options.
fn refresh_pause(
    mut commands: Commands,
    run: Option<Res<Run>>,
    playtest: Res<Playtest>,
    destination: Res<Destination>,
    session: Res<Session>,
    root: Query<Entity, With<PauseRoot>>,
) {
    if !playtest.is_changed() && !destination.is_changed() {
        return;
    }
    for entity in &root {
        commands.entity(entity).despawn();
    }
    pause_menu(commands, run, Some(playtest), destination, session);
}

/// The seed of a new run: the one that the command line gave to a playtest, or else a random
/// one.
fn new_seed(options: &Options, entropy: u64) -> String {
    let chosen = options.seed.clone();
    chosen.unwrap_or_else(|| seed::random_seed_text(entropy))
}

/// Keeps the record of a run for a team of the size it started with, unless it was a playtest.
/// Returns whether the record is new.
fn keep_record(records: &mut Records, run: &Run) -> bool {
    !run.playtest && records.submit(League::of(run), run.team, run.cleared, &run.seed_text)
}

/// Where a run ended, as the stamp of the game over screen tells it.
fn ending(sector: u32) -> String {
    match sector {
        0 => "DESTROYED IN THE START BAY".to_string(),
        sector => format!("DESTROYED IN SECTOR {sector}"),
    }
}

/// The report of a run that is over. It is the left margin of a sheet on which the tank that
/// ended the run is drawn, with the list of its parts.
fn game_over(
    mut commands: Commands,
    run: Option<Res<Run>>,
    records: Res<Records>,
    outcome: Res<Outcome>,
    session: Res<Session>,
    unlocks: Res<Unlocks>,
) {
    let (playtest, team) = run
        .as_ref()
        .map_or((false, 1), |run| (run.playtest, run.team));
    let mode = match team {
        _ if playtest => "PLAYTEST".to_string(),
        1 => "SOLO".to_string(),
        team => format!("CO-OP \u{b7} {team} PLAYERS"),
    };
    let league = run.as_ref().map(|run| League::of(run)).unwrap_or_default();
    let mode = match league.free {
        true if !playtest => format!("{mode} \u{b7} FREE MODE"),
        _ => mode,
    };
    let mode = match league.difficulty {
        Difficulty::Normal => mode,
        Difficulty::Hard => format!("{mode} \u{b7} HARD MODE"),
    };
    let blow = run.as_ref().and_then(|run| run.lost_to);
    commands
        .spawn((
            DespawnOnExit(Screen::GameOver),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            GlobalZIndex(10),
            children![sheet_edge()],
        ))
        .with_children(|root| {
            root.spawn(Node {
                position_type: PositionType::Absolute,
                left: px(120),
                top: px(90),
                width: px(560),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Start,
                row_gap: px(10),
                ..default()
            })
            .with_children(|parent| {
                parent.spawn(theme::label(
                    format!("Run report \u{b7} {mode}"),
                    13.0,
                    theme::TEXT_DIM,
                ));
                parent.spawn(theme::title("Game over", 84.0, theme::TEXT));
                if let Some(run) = &run {
                    // A stamp across the sheet, as on a drawing that was turned down.
                    parent.spawn((
                        Node {
                            padding: UiRect::axes(px(14), px(6)),
                            margin: UiRect::bottom(px(18)),
                            border: UiRect::all(px(2)),
                            ..default()
                        },
                        BorderColor::all(theme::DANGER),
                        BackgroundColor(theme::SURFACE),
                        UiTransform::from_rotation(Rot2::degrees(-3.0)),
                        children![theme::title(ending(run.sector), 24.0, theme::DANGER)],
                    ));
                    let seconds = run.ticks / TICK_RATE as u64;
                    parent
                        .spawn(ruled(Node {
                            align_self: AlignSelf::Stretch,
                            ..default()
                        }))
                        .with_children(|figures| {
                            figure(figures, "SECTORS CLEARED", run.cleared.to_string(), false);
                            let time = format!("{}:{:02}", seconds / 60, seconds % 60);
                            figure(figures, "TIME", time, false);
                            figure(figures, "KILLS", run.kills.to_string(), true);
                        });
                    line(parent, level_line(run), theme::TEXT_DIM);
                }
                if playtest {
                    line(parent, "Playtest: no record kept", theme::TEXT_DIM);
                } else if outcome.new_record {
                    line(parent, "New record", theme::GOOD);
                } else {
                    let best = record_line(&records, league, team);
                    line(parent, best, theme::TEXT_DIM);
                }
                for &unlock in &unlocks.new {
                    let name = unlocks::name(unlock);
                    line(parent, format!("Unlocked: {name}"), theme::GOOD);
                }
                // A player who has everything is not told what they miss.
                let wanting = unlocks.wanting() && !playtest;
                if wanting && run.as_ref().is_some_and(|run| !run.earns) {
                    // In free mode, the next run earns nothing either, retry or not.
                    let why = if league.free {
                        "Runs in free mode earn no achievement."
                    } else {
                        "This run earned no achievement: its seed was typed or played again."
                    };
                    line(parent, why, theme::TEXT_DIM);
                }
                let retry = |buttons: &mut ChildSpawnerCommands| {
                    primary(buttons, "Retry this seed", Press::Retry);
                    if wanting && !league.free {
                        line(buttons, "A retry earns no achievement.", theme::TEXT_DIM);
                    }
                };
                parent
                    .spawn(Node {
                        width: px(400),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(10),
                        margin: UiRect::top(px(14)),
                        ..default()
                    })
                    .with_children(|buttons| match *session {
                        Session::Solo => {
                            retry(buttons);
                            button(buttons, "New run", Press::NewRun);
                            button(buttons, "Menu", Press::Abandon);
                        }
                        Session::Host => {
                            retry(buttons);
                            button(buttons, "New run", Press::NewRun);
                            button(buttons, "Back to the lobby", Press::Regroup);
                        }
                        Session::Guest => {
                            line(buttons, "The host decides what comes next", theme::TEXT);
                            button(buttons, "Leave the game", Press::Abandon);
                        }
                    });
            });
            if let Some(blow) = &blow {
                report::panels(root, blow, team == 1);
            }
        });
}

fn settings_menu(mut commands: Commands, settings: Res<Settings>, capture: Res<Capture>) {
    panel(
        &mut commands,
        (SettingsRoot, DespawnOnExit(Overlay::Settings)),
        theme::SCRIM,
        |parent| {
            heading(parent, "Settings");
            parent
                .spawn(Node {
                    column_gap: px(36),
                    ..default()
                })
                .with_children(|columns| {
                    let column = Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(7),
                        ..default()
                    };
                    columns.spawn(column.clone()).with_children(|controls| {
                        section(controls, "Controls");
                        for action in Action::ALL {
                            row(controls, action.label(), |row| {
                                for (slot, binding) in
                                    settings.bindings(action).into_iter().enumerate()
                                {
                                    let content = if capture.waiting == Some((action, slot)) {
                                        "press a key".to_string()
                                    } else {
                                        binding.map_or("-".to_string(), Binding::name)
                                    };
                                    small(row, content, 126.0, Press::Rebind(action, slot));
                                }
                            });
                        }
                        row(controls, "Menu", |row| {
                            row.spawn(theme::text("Esc", 16.0, theme::TEXT_DIM));
                        });
                        button(controls, "Reset the controls", Press::ResetBindings);
                    });

                    columns.spawn(column).with_children(|options| {
                        section(options, "Video");
                        row(options, "Fullscreen", |row| {
                            small(row, on_off(settings.fullscreen), 70.0, Press::Fullscreen);
                        });
                        row(options, "Vertical sync", |row| {
                            small(row, on_off(settings.vsync), 70.0, Press::Vsync);
                        });
                        section(options, "Audio");
                        row(options, "Volume", |row| {
                            small(row, "-", 34.0, Press::Volume(-0.1));
                            row.spawn(theme::figure(
                                format!("{:.0}%", settings.volume * 100.0),
                                15.0,
                                theme::TEXT,
                            ));
                            small(row, "+", 34.0, Press::Volume(0.1));
                        });
                        row(options, "Music", |row| {
                            small(row, "-", 34.0, Press::Music(-0.1));
                            row.spawn(theme::figure(
                                format!("{:.0}%", settings.music * 100.0),
                                15.0,
                                theme::TEXT,
                            ));
                            small(row, "+", 34.0, Press::Music(0.1));
                        });
                        section(options, "Accessibility");
                        row(options, "Camera shake", |row| {
                            small(row, on_off(settings.camera_shake), 70.0, Press::Shake);
                        });
                        row(options, "Interface size", |row| {
                            small(row, "-", 34.0, Press::Scale(-0.25));
                            row.spawn(theme::figure(
                                format!("{:.0}%", settings.ui_scale * 100.0),
                                15.0,
                                theme::TEXT,
                            ));
                            small(row, "+", 34.0, Press::Scale(0.25));
                        });
                    });
                });
            button(parent, "Back", Press::Back);
        },
    );
}

/// Draws the settings again when one of them changed.
fn refresh_settings(
    mut commands: Commands,
    settings: Res<Settings>,
    capture: Res<Capture>,
    root: Query<Entity, With<SettingsRoot>>,
) {
    if !settings.is_changed() && !capture.is_changed() {
        return;
    }
    for entity in &root {
        commands.entity(entity).despawn();
    }
    settings_menu(commands, settings, capture);
}

/// Gives the next key or button pressed to the binding that waits for one.
fn capture_binding(
    mut capture: ResMut<Capture>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut settings: ResMut<Settings>,
) {
    let Some((action, slot)) = capture.waiting else {
        return;
    };
    if !capture.armed {
        capture.armed = true;
    } else if keys.just_pressed(KeyCode::Escape) {
        capture.waiting = None;
    } else if let Some(binding) = Binding::pressed_now(&keys, &mouse) {
        settings.bind(action, slot, binding);
        capture.waiting = None;
        capture.swallowed = matches!(binding, Binding::Mouse(_));
    }
}

fn press(
    buttons: Query<(&Interaction, &Press), Changed<Interaction>>,
    screen: Res<State<Screen>>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut next_overlay: ResMut<NextState<Overlay>>,
    mut settings: ResMut<Settings>,
    mut capture: ResMut<Capture>,
    mut playtest: Option<ResMut<Playtest>>,
    mut destination: ResMut<Destination>,
    (options, run, time): (Res<Options>, Option<Res<Run>>, Res<Time<Real>>),
    (session, mut address, mut notice, opening): (
        Res<Session>,
        ResMut<Address>,
        ResMut<Notice>,
        Res<Opening>,
    ),
    (mut setup, mut drawn, mut listed, mut garage): (
        ResMut<Setup>,
        ResMut<Drawn>,
        ResMut<Listed>,
        ResMut<Garage>,
    ),
    (found, mut way, invitation, mut clipboard, mut copied): (
        Res<Found>,
        ResMut<Way>,
        Res<Invitation>,
        Option<ResMut<Clipboard>>,
        ResMut<Copied>,
    ),
    mut lobbies: Query<&mut Lobby>,
    mut requests: MessageWriter<RunRequest>,
    mut session_requests: MessageWriter<SessionRequest>,
    mut exit: MessageWriter<AppExit>,
) {
    if capture.swallowed {
        capture.swallowed = false;
        return;
    }
    // The moment of the click is all the entropy a seed needs.
    let fresh_seed = || new_seed(&options, time.elapsed().as_nanos() as u64);
    for (interaction, press) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *press {
            Press::Solo => {
                notice.0 = None;
                next_screen.set(Screen::Solo);
            }
            Press::Begin => begin(&setup, (&options, &garage), &mut requests, &mut next_screen),
            Press::Redraw => (setup.seed, setup.typed) = (fresh_seed(), false),
            // The choice is kept for the next time.
            Press::Difficulty(difficulty) => {
                setup.difficulty = difficulty;
                if settings.solo_difficulty != difficulty {
                    settings.solo_difficulty = difficulty;
                }
            }
            // The choice is kept for the next time.
            Press::Free(on) => {
                setup.free = on;
                if settings.solo_free != on {
                    settings.solo_free = on;
                }
            }
            Press::OpenGarage => next_screen.set(Screen::Garage),
            Press::Level(stat, by) => {
                if let Some(tank) = garage.changed(stat, by) {
                    garage.tank = tank;
                }
            }
            Press::Standard => {
                if garage.tank != Genome::PLAYER {
                    garage.tank = Genome::PLAYER;
                }
            }
            Press::CloseGarage => next_screen.set(Screen::Solo),
            Press::List(league) => listed.0 = league,
            Press::OpenAchievements => next_screen.set(Screen::Achievements),
            Press::NewRun | Press::Retry => {
                // A seed that is played again was not drawn for the run: it can be practiced.
                let (seed, fresh) = match (*press, &run) {
                    (Press::Retry, Some(run)) => (run.seed_text.clone(), false),
                    _ => (fresh_seed(), true),
                };
                notice.0 = None;
                match *session {
                    Session::Solo => {
                        // The next run is as hard as the one that is over, and in the
                        // same tank.
                        let league = run.as_ref().map(|run| League::of(run));
                        let league = league.unwrap_or(league_of(&setup, &options));
                        requests.write(solo_run(seed, fresh, league, &options, &garage));
                        next_screen.set(Screen::Playing);
                        next_overlay.set(Overlay::None);
                    }
                    // A host starts the run for everyone, on the seed of the lobby.
                    Session::Host => {
                        if let Ok(mut lobby) = lobbies.single_mut() {
                            lobby.seed = seed;
                        }
                        drawn.0 = fresh;
                        session_requests.write(SessionRequest::Start);
                    }
                    Session::Guest => {}
                }
            }
            Press::OpenHost => {
                notice.0 = None;
                next_screen.set(Screen::Host);
            }
            Press::Host => {
                notice.0 = None;
                ask_to_host(&opening, &mut settings, &mut session_requests);
            }
            Press::OpenJoin => {
                notice.0 = None;
                next_screen.set(Screen::Join);
            }
            // A browser joins the game of its link, and a desktop game the one at the
            // address that was typed.
            Press::Join => match net::link() {
                Some(link) => {
                    session_requests.write(SessionRequest::Join(link));
                }
                None => ask_to_join(&address, &mut settings, &mut session_requests),
            },
            // A game that was found is joined as if its address had been typed. While a
            // game is being joined, the address stays the one of that game.
            Press::JoinFound(id) => {
                let found = found.0.iter().find(|game| game.id == id);
                if let Some(game) = found.filter(|_| *session == Session::Solo) {
                    address.0 = game.address.clone();
                    ask_to_join(&address, &mut settings, &mut session_requests);
                }
            }
            Press::Start => {
                session_requests.write(SessionRequest::Start);
            }
            Press::Reseed => {
                if let Ok(mut lobby) = lobbies.single_mut() {
                    lobby.seed = fresh_seed();
                    drawn.0 = true;
                }
            }
            Press::Show(wanted) => *way = wanted,
            Press::Copy => {
                if let Some(code) = &invitation.web {
                    let done = copy(clipboard.as_deref_mut(), link_end(code));
                    copied.0 = Some((done, COPY_SHOWN));
                }
            }
            // The guests follow their host to the lobby.
            Press::Regroup => {
                requests.write(RunRequest::Abandon);
                next_screen.set(Screen::Lobby);
                next_overlay.set(Overlay::None);
            }
            Press::OpenSettings => next_overlay.set(Overlay::Settings),
            Press::Resume => next_overlay.set(Overlay::None),
            Press::Back => {
                capture.waiting = None;
                next_overlay.set(if *screen.get() == Screen::Playing {
                    Overlay::Pause
                } else {
                    Overlay::None
                });
            }
            Press::Abandon => {
                match *session {
                    Session::Solo => {
                        requests.write(RunRequest::Abandon);
                    }
                    // A host closes the game for everyone, and a guest leaves it.
                    Session::Host | Session::Guest => {
                        session_requests.write(SessionRequest::Leave);
                    }
                }
                next_screen.set(Screen::Menu);
                next_overlay.set(Overlay::None);
            }
            Press::Quit => {
                exit.write(AppExit::Success);
            }
            Press::Fullscreen => settings.fullscreen = !settings.fullscreen,
            Press::Vsync => settings.vsync = !settings.vsync,
            Press::Shake => settings.camera_shake = !settings.camera_shake,
            Press::Volume(change) => settings.volume = level(settings.volume + change),
            Press::Music(change) => settings.music = level(settings.music + change),
            Press::Scale(change) => {
                settings.ui_scale = (settings.ui_scale + change).clamp(0.75, 2.0)
            }
            Press::Rebind(action, slot) => {
                capture.waiting = Some((action, slot));
                capture.armed = false;
            }
            Press::ResetBindings => settings.reset_bindings(),
            Press::InfiniteLives => {
                if let Some(playtest) = &mut playtest {
                    playtest.infinite_lives = !playtest.infinite_lives;
                }
            }
            Press::Destination(change) => {
                let sector = destination.0.saturating_add_signed(change);
                destination.0 = sector.clamp(1, PLAYTEST_LAST_SECTOR);
            }
            // The game goes on at once, in front of that sector.
            Press::Skip => {
                requests.write(RunRequest::SkipTo {
                    sector: destination.0,
                });
                next_overlay.set(Overlay::None);
            }
        }
    }
}

/// Esc opens and closes the menu during a run, and leaves the settings.
fn escape(
    keys: Res<ButtonInput<KeyCode>>,
    capture: Res<Capture>,
    screen: Res<State<Screen>>,
    overlay: Res<State<Overlay>>,
    session: Res<Session>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut next_overlay: ResMut<NextState<Overlay>>,
    mut session_requests: MessageWriter<SessionRequest>,
) {
    if !keys.just_pressed(KeyCode::Escape) || capture.waiting.is_some() {
        return;
    }
    // Esc leaves the screens that come before a run as their button "Back" does.
    let before = matches!(
        *screen.get(),
        Screen::Solo | Screen::Achievements | Screen::Host
    );
    if before && *overlay.get() == Overlay::None {
        next_screen.set(Screen::Menu);
        return;
    }
    if *screen.get() == Screen::Garage && *overlay.get() == Overlay::None {
        next_screen.set(Screen::Solo);
        return;
    }
    if *screen.get() == Screen::Join && *overlay.get() == Overlay::None {
        if *session == Session::Guest {
            session_requests.write(SessionRequest::Leave);
        }
        next_screen.set(Screen::Menu);
        return;
    }
    let playing = *screen.get() == Screen::Playing;
    match *overlay.get() {
        Overlay::None if playing => next_overlay.set(Overlay::Pause),
        Overlay::Pause => next_overlay.set(Overlay::None),
        Overlay::Settings if playing => next_overlay.set(Overlay::Pause),
        Overlay::Settings => next_overlay.set(Overlay::None),
        Overlay::None => {}
    }
}

fn hover(
    mut buttons: Query<
        (
            &Interaction,
            Has<Primary>,
            Has<Lit>,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (Changed<Interaction>, With<Press>),
    >,
) {
    for (interaction, primary, lit, mut color, mut border) in &mut buttons {
        let (fill, edge) = match (interaction, primary) {
            // A button that is on keeps the edge that says so.
            (Interaction::Hovered, false) if lit => (theme::SURFACE_HOVERED, theme::ACCENT),
            (Interaction::None, false) if lit => (theme::SURFACE_RAISED, theme::ACCENT),
            (Interaction::Pressed, true) => (theme::TEXT, theme::TEXT),
            (Interaction::Pressed, false) => (theme::ACCENT, theme::ACCENT),
            (Interaction::Hovered, true) => (theme::ACCENT_HOVERED, theme::ACCENT_HOVERED),
            (Interaction::Hovered, false) => (theme::SURFACE_HOVERED, theme::TEXT),
            (Interaction::None, true) => (theme::ACCENT, theme::ACCENT),
            (Interaction::None, false) => (theme::SURFACE_RAISED, theme::RULE),
        };
        color.0 = fill;
        *border = BorderColor::all(edge);
    }
}

/// In solo, an open menu pauses the game. A run of several players goes on.
fn pause_time(
    screen: Res<State<Screen>>,
    overlay: Res<State<Overlay>>,
    run: Option<Res<Run>>,
    mut time: ResMut<Time<Virtual>>,
) {
    let solo = run.is_none_or(|run| run.solo);
    let paused = solo && *screen.get() == Screen::Playing && *overlay.get() != Overlay::None;
    if paused && !time.is_paused() {
        time.pause();
    } else if !paused && time.is_paused() {
        time.unpause();
    }
}

/// Shows the game over screen a moment after a run is lost, and keeps the record.
fn watch_run(
    time: Res<Time<Real>>,
    mut events: MessageReader<SimEvent>,
    mut outcome: ResMut<Outcome>,
    mut records: ResMut<Records>,
    run: Option<Res<Run>>,
    screen: Res<State<Screen>>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut next_overlay: ResMut<NextState<Overlay>>,
) {
    if events.read().any(|event| *event == SimEvent::RunOver) {
        outcome.countdown = Some(GAME_OVER_DELAY);
    }
    let Some(left) = outcome.countdown else {
        return;
    };
    if left > time.delta_secs() {
        outcome.countdown = Some(left - time.delta_secs());
        return;
    }
    outcome.countdown = None;
    // A host may have started the next run already: then there is nothing to report.
    let lost = run.filter(|run| run.phase == RunPhase::Over);
    if let (Some(run), Screen::Playing) = (lost, *screen.get()) {
        outcome.new_record = keep_record(&mut records, &run);
        next_screen.set(Screen::GameOver);
        next_overlay.set(Overlay::None);
    }
}

/// Keeps the record of a run that goes away before it is lost: its player left it, or its
/// host ended it or closed the game.
fn keep_records(run: Option<Res<Run>>, mut records: ResMut<Records>, mut last: Local<Option<Run>>) {
    let gone = match (&run, &*last) {
        (Some(run), Some(last)) => run.seed != last.seed || run.ticks < last.ticks,
        (None, Some(_)) => true,
        (_, None) => false,
    };
    if let (true, Some(ended)) = (gone, last.take()) {
        keep_record(&mut records, &ended);
    }
    match (run, &mut *last) {
        (Some(run), Some(last)) if *last == *run => {}
        (run, last) => *last = run.map(|run| run.clone()),
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        ecs::{message::Messages, system::RunSystemOnce},
        platform::collections::HashMap,
    };
    use tanku_core::{
        sim::{Blow, Team, Weapon},
        stats::Stat,
    };

    use super::*;
    use crate::{crew, hud::SHEET_EDGE, session, sheet};

    /// A notice as long as they get: three lines of a menu.
    const LONG_NOTICE: &str = "No game answers at 192.168.100.200:28405. Its host must run \
                               version 0.4.0 too, and the link must be the one that its lobby \
                               shows now.";
    /// The longest address that a machine has on a network.
    const LONG_ADDRESS: &str = "192.168.100.200";
    /// The longest seed that a player types, in the widest letter.
    const LONG_SEED: &str = "WWWWWWWWWW";
    /// A name as long as the join screen takes, which fills two lines of its field.
    const LONG_NAME: &str = "a-machine-with-the-longest-name-of-its-network.example.org:28405";
    /// The name of a game, as long as one gets and in the widest letter.
    const LONG_GAME: &str = "WWWWWWWWWWWWWWWWWWWWWWWW";

    /// A run that was lost in a far sector by a team of this size, to the tank that has
    /// the most parts to list.
    fn lost_run(team: u32) -> Run {
        let mut genome = Genome::empty(135);
        for stat in Stat::HARDWARE
            .into_iter()
            .chain([Stat::Brain, Stat::Ballistics])
        {
            genome.set(stat, stat.max_level());
        }
        Run {
            seed_text: "85GVTHQA".into(),
            seed: 1,
            difficulty: default(),
            players: team,
            team,
            solo: team == 1,
            sector: 128,
            cleared: 127,
            checkpoint: 124,
            lives: 0,
            kills: 1234,
            ticks: 3 * 3600 * TICK_RATE as u64,
            phase: RunPhase::Over,
            lost_to: Some(Blow {
                weapon: Weapon::Crate,
                genome,
                team: Team::Enemies,
            }),
            playtest: false,
            earns: true,
            free: false,
        }
    }

    fn league(difficulty: Difficulty, free: bool) -> League {
        League { difficulty, free }
    }

    /// A tank that a playtest brings: the fastest engine, the fastest gun and the fastest
    /// loader, with two points left over.
    fn racer() -> Genome {
        let parts = [
            (Stat::Engine, 7),
            (Stat::Muzzle, 7),
            (Stat::Loader, 7),
            (Stat::Magazine, 1),
        ];
        Genome::of_the_garage(&parts).unwrap()
    }

    /// Sits this many players in the lobby of a game, the host first.
    fn gather(app: &mut App, players: usize) {
        let mut lobby = Lobby {
            name: LONG_GAME.into(),
            seed: "85GVTHQA".into(),
            ..default()
        };
        for (seat, place) in lobby.seats.iter_mut().take(players).enumerate() {
            *place = Some(match seat {
                0 => Member::Host,
                seat => Member::Guest(seat as u64),
            });
        }
        app.world_mut().spawn(lobby);
    }

    /// What a host hands out, on a machine with the longest address, to players at a
    /// desktop and to those in a browser.
    fn invitation() -> Invitation {
        Invitation {
            address: Some(LONG_ADDRESS.into()),
            port: 28405,
            web: Some(format!(
                "{LONG_ADDRESS}:28406/{}",
                "0123456789abcdef".repeat(4)
            )),
            web_port: Some(28406),
        }
    }

    fn build<Marker>(app: &mut App, menu: impl IntoSystem<(), (), Marker>) {
        app.world_mut().run_system_once(menu).unwrap();
    }

    /// Every screen of the menus, each with as much on it as it ever holds.
    fn screens() -> Vec<(&'static str, fn(&mut App))> {
        vec![
            // A record for every difficulty and every size of team, each on a seed as long
            // as one gets.
            ("the main menu", |app| {
                app.insert_resource(Records::of_every_team(128, LONG_SEED))
                    .insert_resource(Notice(Some(LONG_NOTICE.into())));
                build(app, main_menu);
            }),
            (
                "the main menu with the records of hard mode and free mode",
                |app| {
                    app.insert_resource(Records::of_every_team(128, LONG_SEED))
                        .insert_resource(Listed(league(Difficulty::Hard, true)))
                        .insert_resource(Notice(Some(LONG_NOTICE.into())));
                    build(app, main_menu);
                },
            ),
            // A notice on top of this one would cross the fine line around the sheet by a
            // few pixels in the smallest windows. A playtest is a tool of the game's makers.
            ("the main menu of a playtest", |app| {
                app.insert_resource(Records::of_every_team(128, "85GVTHQA"))
                    .insert_resource(Playtest {
                        infinite_lives: true,
                    })
                    .insert_resource(Options {
                        playtest: true,
                        seed: Some("85GVTHQA".into()),
                        tank: None,
                    });
                build(app, main_menu);
            }),
            // The corner of the sheet then says the most about the tank that is drawn.
            ("the main menu of a playtest in a tank of its own", |app| {
                app.insert_resource(Records::of_every_team(128, "85GVTHQA"))
                    .insert_resource(Playtest {
                        infinite_lives: true,
                    })
                    .insert_resource(Options {
                        playtest: true,
                        seed: Some("85GVTHQA".into()),
                        tank: Some(racer()),
                    });
                build(app, main_menu);
            }),
            ("the solo screen", |app| {
                app.insert_resource(Setup {
                    seed: "85GVTHQA".into(),
                    typed: false,
                    difficulty: Difficulty::Normal,
                    free: false,
                });
                build(app, solo_screen);
            }),
            // A seed as long as one gets, typed by a player who unlocked that and nothing
            // else: the screen then says the most.
            ("the solo screen with a seed that was typed", |app| {
                assert_eq!(LONG_SEED.len(), SEED_LENGTH);
                app.insert_resource(Unlocks::of(&[Unlock::ChosenSeed]))
                    .insert_resource(Setup {
                        seed: LONG_SEED.into(),
                        typed: true,
                        difficulty: Difficulty::Normal,
                        free: false,
                    });
                build(app, solo_screen);
            }),
            // Hard mode is unlocked and chosen, and something is left to unlock: the screen
            // then offers the difficulties, and says what hard mode is and what the run earns.
            ("the solo screen in hard mode", |app| {
                let unlocks = [Unlock::ChosenSeed, Unlock::HardMode];
                app.insert_resource(Unlocks::of(&unlocks))
                    .insert_resource(Setup {
                        seed: LONG_SEED.into(),
                        typed: true,
                        difficulty: Difficulty::Hard,
                        free: false,
                    });
                build(app, solo_screen);
            }),
            // Free mode and hard mode are both on, a seed was typed and something is left
            // to unlock: the screen then holds every row and every note that it has.
            ("the solo screen in free mode", |app| {
                let unlocks = [Unlock::ChosenSeed, Unlock::FreeMode, Unlock::HardMode];
                app.insert_resource(Unlocks::of(&unlocks))
                    .insert_resource(Garage { tank: racer() })
                    .insert_resource(Setup {
                        seed: LONG_SEED.into(),
                        typed: true,
                        difficulty: Difficulty::Hard,
                        free: true,
                    });
                build(app, solo_screen);
            }),
            (
                "the solo screen of a playtest in a tank of its own",
                |app| {
                    app.insert_resource(Options {
                        playtest: true,
                        seed: Some("85GVTHQA".into()),
                        tank: Some(racer()),
                    });
                    build(app, solo_screen);
                },
            ),
            // The standard tank spends every point: no level can be added to it.
            ("the garage", |app| build(app, garage_screen)),
            // A tank with points left, whose stats say the most about what they give.
            ("the garage with points left", |app| {
                let parts = [
                    (Stat::Engine, 6),
                    (Stat::Tracks, 4),
                    (Stat::Muzzle, 5),
                    (Stat::Magazine, 1),
                    (Stat::Loader, 5),
                    (Stat::Ricochet, 2),
                    (Stat::Shield, 1),
                ];
                let tank = Genome::of_the_garage(&parts).unwrap();
                assert_eq!(tank.cost(), 58);
                app.insert_resource(Garage { tank });
                build(app, garage_screen);
            }),
            ("the achievements", |app| {
                app.insert_resource(Unlocks::of(&[Unlock::ChosenSeed, Unlock::HardMode]));
                build(app, achievements_screen);
            }),
            // The longest name that a host types, on a machine that kept it from the last
            // game, with the longest notice: the port may be taken.
            ("the host screen", |app| {
                assert_eq!(LONG_GAME.chars().count(), NAME_LENGTH);
                let mut settings = Settings::default();
                settings.host_name = LONG_GAME.into();
                app.insert_resource(settings)
                    .insert_resource(Notice(Some(LONG_NOTICE.into())));
                build(app, host_screen);
            }),
            // As many games as the screen lists, each under a name as long as one gets but
            // the last, which has none.
            ("the join screen", |app| {
                let game = |id: u64, standing: Standing| Game {
                    id,
                    name: if id == 4 { "" } else { LONG_GAME }.into(),
                    machine: std::net::Ipv4Addr::new(192, 168, 100, 200),
                    address: format!("{LONG_ADDRESS}:2840{id}"),
                    players: 4,
                    standing,
                };
                let found = vec![
                    game(1, Standing::Open),
                    game(2, Standing::Full),
                    game(3, Standing::Running),
                    game(4, Standing::Other),
                ];
                assert_eq!(found.len(), crate::discovery::SHOWN);
                assert_eq!(LONG_NAME.len(), ADDRESS_LENGTH);
                app.insert_resource(Found(found))
                    .insert_resource(Address(LONG_NAME.into()))
                    .insert_resource(Notice(Some(LONG_NOTICE.into())));
                build(app, join_screen);
            }),
            ("the lobby of a host", |app| {
                gather(app, 4);
                // Nothing else on the machine has taken the port for browsers, or something
                // has: the lobby then says so.
                app.insert_resource(Session::Host)
                    .insert_resource(Invitation {
                        web: None,
                        ..invitation()
                    });
                build(app, lobby_screen);
            }),
            ("the lobby of a host, for browsers", |app| {
                gather(app, 1);
                // The button that copies the link has just failed, which takes the most
                // words to say.
                app.insert_resource(Session::Host)
                    .insert_resource(invitation())
                    .insert_resource(Way::Browser)
                    .insert_resource(Copied(Some((false, COPY_SHOWN))));
                build(app, lobby_screen);
            }),
            ("the lobby of a host without a network", |app| {
                gather(app, 1);
                app.insert_resource(Session::Host)
                    .insert_resource(Invitation {
                        address: None,
                        ..invitation()
                    });
                build(app, lobby_screen);
            }),
            ("the lobby of a guest", |app| {
                gather(app, 4);
                app.insert_resource(Session::Guest)
                    .insert_resource(crew::LocalSeat(3));
                build(app, lobby_screen);
            }),
            ("the report of a run", |app| {
                app.insert_resource(lost_run(1))
                    .insert_resource(Records::of_every_team(128, "85GVTHQA"));
                build(app, game_over);
            }),
            // A run on a seed that was played again, by a player who has things left to
            // unlock: the report then says the most.
            ("the report of a run that earned nothing", |app| {
                let run = Run {
                    earns: false,
                    ..lost_run(1)
                };
                app.insert_resource(run)
                    .insert_resource(Records::of_every_team(128, "85GVTHQA"));
                build(app, game_over);
            }),
            // A run of four players in hard mode and in free mode, on a seed as long as one
            // gets: the lines of the report are then as long as they get.
            ("the report of a run in hard mode and free mode", |app| {
                let run = Run {
                    seed_text: LONG_SEED.into(),
                    difficulty: Difficulty::Hard,
                    earns: false,
                    free: true,
                    ..lost_run(4)
                };
                app.insert_resource(run)
                    .insert_resource(Session::Host)
                    .insert_resource(Records::of_every_team(128, LONG_SEED));
                build(app, game_over);
            }),
            ("the report of a run that unlocked three things", |app| {
                let mut unlocks = Unlocks::default();
                unlocks.new = vec![Unlock::ChosenSeed, Unlock::FreeMode, Unlock::HardMode];
                app.insert_resource(lost_run(1))
                    .insert_resource(unlocks)
                    .insert_resource(Records::of_every_team(128, "85GVTHQA"));
                build(app, game_over);
            }),
            ("the report of a run, for its host", |app| {
                app.insert_resource(lost_run(4))
                    .insert_resource(Session::Host);
                build(app, game_over);
            }),
            ("the report of a run, for a guest", |app| {
                app.insert_resource(lost_run(4))
                    .insert_resource(Session::Guest);
                build(app, game_over);
            }),
            ("the settings", |app| {
                app.insert_resource(Capture {
                    waiting: Some((Action::Repair, 1)),
                    ..default()
                });
                build(app, settings_menu);
            }),
            ("the menu of a host during a playtest", |app| {
                let run = Run {
                    seed_text: LONG_SEED.into(),
                    difficulty: Difficulty::Hard,
                    ..lost_run(4)
                };
                app.insert_resource(run)
                    .insert_resource(Session::Host)
                    .insert_resource(Playtest {
                        infinite_lives: true,
                    });
                build(app, pause_menu);
            }),
        ]
    }

    /// A piece of the interface, as the engine laid it out.
    struct Piece {
        entity: Entity,
        /// What it holds, for whoever reads why a test failed.
        name: String,
        /// Where it lies, in the pixels of the window.
        lies: Rect,
        /// Whether it is a text or a button: something that must show whole, with nothing
        /// else on it.
        marks: bool,
        /// Whether it has an edge that the engine laid out as no line at all.
        lost_an_edge: bool,
        /// The pieces that it is inside of, from the nearest one to the screen itself.
        inside: Vec<Entity>,
    }

    /// A game with a window of this many pixels and a screen of the menus on it.
    fn stage(screen: fn(&mut App), pixels: UVec2) -> App {
        let mut app = sheet::tests::window(pixels, 1.0);
        app.init_state::<Screen>().init_state::<Overlay>();
        theme::plugin(&mut app);
        session::plugin(&mut app);
        crew::plugin(&mut app);
        app.init_resource::<Records>()
            .init_resource::<Options>()
            .init_resource::<Address>()
            .init_resource::<Opening>()
            .init_resource::<Settings>()
            .init_resource::<Setup>()
            .init_resource::<Listed>()
            .init_resource::<Garage>()
            .init_resource::<Unlocks>()
            .init_resource::<Way>()
            .init_resource::<Copied>()
            .init_resource::<Capture>()
            .init_resource::<Outcome>()
            .insert_resource(Destination(1));
        // The fonts are there from the second frame on.
        app.update();
        screen(&mut app);
        // What the menus fill in afterward: the games that were found, the best runs, the
        // address that was typed and the notice.
        build(&mut app, list_games);
        build(&mut app, list_records);
        build(&mut app, show_address);
        build(&mut app, show_name);
        build(&mut app, tell_how_it_goes);
        app.update();
        app.update();
        app
    }

    /// What the buttons of a screen of the menus do.
    fn presses(screen: fn(&mut App)) -> Vec<Press> {
        let mut app = stage(screen, WINDOWS[0]);
        let world = app.world_mut();
        let mut buttons = world.query_filtered::<&Press, With<Button>>();
        buttons.iter(world).copied().collect()
    }

    /// Lays out a screen of the menus as a window of this many pixels shows it.
    fn lay_out(screen: fn(&mut App), pixels: UVec2) -> Vec<Piece> {
        let mut app = stage(screen, pixels);

        let world = app.world_mut();
        let mut nodes = world.query::<(
            Entity,
            (&Node, &ComputedNode, &UiGlobalTransform),
            Option<&Text>,
            Has<Button>,
            Option<&ChildOf>,
        )>();
        let parents: HashMap<Entity, Entity> = nodes
            .iter(world)
            .filter_map(|(entity, .., parent)| Some((entity, parent?.parent())))
            .collect();
        let inside = |entity: Entity| -> Vec<Entity> {
            std::iter::successors(parents.get(&entity).copied(), |parent| {
                parents.get(parent).copied()
            })
            .collect()
        };
        let texts: Vec<(Entity, String)> = nodes
            .iter(world)
            .filter_map(|(entity, .., text, _, _)| Some((entity, text?.0.clone())))
            .filter(|(_, text)| !text.is_empty())
            .collect();
        nodes
            .iter(world)
            .map(|(entity, (node, computed, transform), text, button, _)| {
                // A piece goes by its own words, or by the first words inside it.
                let words = texts
                    .iter()
                    .find(|(holder, _)| *holder == entity || inside(*holder).contains(&entity));
                Piece {
                    entity,
                    name: words.map_or("an empty box".to_string(), |(_, text)| format!("{text:?}")),
                    lies: Rect::from_center_size(transform.translation, computed.size),
                    marks: button || text.is_some_and(|text| !text.0.is_empty()),
                    lost_an_edge: [
                        (node.border.left, computed.border.min_inset.x),
                        (node.border.right, computed.border.max_inset.x),
                        (node.border.top, computed.border.min_inset.y),
                        (node.border.bottom, computed.border.max_inset.y),
                    ]
                    .into_iter()
                    .any(|(drawn, laid_out)| drawn != Val::ZERO && laid_out < 1.0),
                    inside: inside(entity),
                }
            })
            .collect()
    }

    /// The windows that the menus are laid out in: the smallest that the game is meant
    /// for, then one for each size that a window gives the interface up to its full size,
    /// and a large one.
    const WINDOWS: [UVec2; 6] = [
        UVec2::new(1280, 720),
        UVec2::new(1366, 768),
        UVec2::new(1440, 900),
        UVec2::new(1536, 864),
        UVec2::new(1600, 900),
        UVec2::new(1920, 1080),
    ];

    /// The texts and the buttons of a screen, which must show whole with nothing on them,
    /// and the lowest point that any of them reaches.
    fn marks(pieces: &[Piece]) -> (Vec<&Piece>, f32) {
        let marks: Vec<&Piece> = pieces.iter().filter(|piece| piece.marks).collect();
        let lowest = marks.iter().map(|piece| piece.lies.max.y);
        let lowest = lowest.fold(0.0, f32::max);
        (marks, lowest)
    }

    /// Prints how far down each menu goes in each window, in the pixels of that window, under
    /// how far down the fine line around the sheet is there:
    /// `cargo test -p tanku_client room_the_menus_take -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn print_the_room_the_menus_take() {
        let row = |name: &str, cells: [f32; 6]| {
            let cells = cells.map(|cell| format!("{cell:5.0}"));
            println!("{name:>40}  {}", cells.join(" "));
        };
        row(
            "In a window this high",
            WINDOWS.map(|pixels| pixels.y as f32),
        );
        let line = |pixels: UVec2| pixels.y as f32 - SHEET_EDGE * sheet::fit(pixels.as_vec2());
        row("the fine line is at", WINDOWS.map(line));
        for (name, screen) in screens() {
            row(
                name,
                WINDOWS.map(|pixels| marks(&lay_out(screen, pixels)).1),
            );
        }
    }

    #[test]
    fn a_seed_is_typed_in_capitals_and_stops_at_its_length() {
        let mut seed = String::new();
        let press = |seed: &mut String, text: &str| {
            typed_seed(seed, &Key::Character(text.into()), Some(text));
        };
        press(&mut seed, "ta");
        press(&mut seed, "N-k u");
        assert_eq!(seed, "TANKU");
        press(&mut seed, "0123456789");
        assert_eq!(seed, "TANKU01234");
        typed_seed(&mut seed, &Key::Backspace, None);
        assert_eq!(seed, "TANKU0123");
    }

    #[test]
    fn every_menu_fits_the_window_whole() {
        let mut faults = Vec::new();
        for (name, screen) in screens() {
            for pixels in WINDOWS {
                let pieces = lay_out(screen, pixels);
                let (marks, _) = marks(&pieces);
                assert!(!marks.is_empty(), "{name} shows nothing");
                // Nothing of a menu crosses the fine line around the sheet, which is as far
                // from the edge of the window as the interface is large. A pixel of slack:
                // the engine lays every piece out on whole pixels.
                let edge = SHEET_EDGE * sheet::fit(pixels.as_vec2());
                let window = Rect::from_corners(Vec2::ZERO, pixels.as_vec2());
                let allowed = window.inflate(1.0 - edge);
                for piece in &marks {
                    let lies = piece.lies;
                    if lies.is_empty() {
                        faults.push(format!("{name}, {pixels}: {} takes no room", piece.name));
                    } else if !(allowed.contains(lies.min) && allowed.contains(lies.max)) {
                        faults.push(format!(
                            "{name}, {pixels}: {} lies from {} to {}, and must stay between \
                             {} and {}",
                            piece.name, lies.min, lies.max, allowed.min, allowed.max
                        ));
                    }
                }
                // No box has lost a side, and no table a rule.
                for piece in pieces.iter().filter(|piece| piece.lost_an_edge) {
                    faults.push(format!("{name}, {pixels}: {} lost an edge", piece.name));
                }
                // No text or button lies on another, unless one holds the other.
                for (index, piece) in marks.iter().enumerate() {
                    for other in &marks[index + 1..] {
                        let apart = piece.inside.contains(&other.entity)
                            || other.inside.contains(&piece.entity)
                            || piece.lies.inflate(-1.0).intersect(other.lies).is_empty();
                        if !apart {
                            faults.push(format!(
                                "{name}, {pixels}: {} lies on {}",
                                piece.name, other.name
                            ));
                        }
                    }
                }
            }
        }
        assert!(faults.is_empty(), "{}", faults.join("\n"));
    }

    #[test]
    fn a_seed_from_the_command_line_serves_every_run() {
        let chosen = Options {
            playtest: true,
            seed: Some("85GVTHQA".into()),
            tank: None,
        };
        assert_eq!(new_seed(&chosen, 1), "85GVTHQA");
        assert_eq!(new_seed(&chosen, 2), "85GVTHQA");
        let random = Options::default();
        assert_ne!(new_seed(&random, 1), new_seed(&random, 2));
    }

    #[test]
    fn what_a_host_hands_out_for_browsers_fits_the_lobby() {
        let print = "0123456789abcdef".repeat(4);
        let lines = link_ending(&format!("192.168.0.21:28406/{print}"));
        assert_eq!(lines[0], "#192.168.0.21:28406/");
        // Put end to end, the lines are the end of the link, which is what the lobby copies.
        assert_eq!(lines.concat(), format!("#192.168.0.21:28406/{print}"));
        assert_eq!(
            lines.concat(),
            link_end(&format!("192.168.0.21:28406/{print}"))
        );
        assert!(lines.iter().all(|line| line.len() <= 32));
    }

    #[test]
    fn the_button_that_copies_the_link_says_how_the_copy_went() {
        assert_eq!(copy_label(&Copied(None)), "Copy");
        assert_eq!(copy_label(&Copied(Some((true, COPY_SHOWN)))), "Copied");
        assert_eq!(copy_label(&Copied(Some((false, COPY_SHOWN)))), "Not copied");
        // A machine without a clipboard that the game can reach copies nothing, and the
        // game says so.
        assert!(!copy(None, "#192.168.0.21:28406/0123".to_string()));
    }

    #[test]
    fn a_game_that_was_found_says_whether_it_can_be_joined() {
        let game = |players: u8, standing: Standing| Game {
            id: 1,
            name: String::new(),
            machine: std::net::Ipv4Addr::new(192, 168, 0, 21),
            address: "192.168.0.21".to_string(),
            players,
            standing,
        };
        assert_eq!(found_words(&game(2, Standing::Open)), "2 of 4 players");
        assert_eq!(
            found_words(&game(4, Standing::Full)),
            "4 of 4 players \u{b7} full"
        );
        assert_eq!(found_words(&game(3, Standing::Running)), "Run in progress");
        assert_eq!(
            found_words(&game(1, Standing::Other)),
            "Another version of the game"
        );
    }

    /// A key as the keyboard sends it, with the text that it writes if it writes any.
    fn stroke(key: Key, text: Option<&str>) -> KeyboardInput {
        KeyboardInput {
            key_code: KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
            logical_key: key,
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    /// The keys that write a text, one after the other.
    fn strokes(text: &str) -> Vec<KeyboardInput> {
        let letters = text.chars().map(|letter| letter.to_string());
        letters
            .map(|letter| stroke(Key::Character(letter.as_str().into()), Some(&letter)))
            .collect()
    }

    #[test]
    fn a_name_is_typed_a_key_at_a_time() {
        let press = |name: &mut String, letter: &str| {
            let key = Key::Character(letter.into());
            typed_words(name, &key, Some(letter), NAME_LENGTH);
        };
        let mut name = String::new();
        for letter in ["S", "o", "i", "r", "\u{e9}", "e", " ", "4", "!"] {
            press(&mut name, letter);
        }
        assert_eq!(name, "Soir\u{e9}e 4!");
        // What the interface has no letter for is not taken: it would show as nothing.
        press(&mut name, "\u{1F600}");
        press(&mut name, "\t");
        assert_eq!(name, "Soir\u{e9}e 4!");
        typed_words(&mut name, &Key::Backspace, None, NAME_LENGTH);
        assert_eq!(name, "Soir\u{e9}e 4");
        // A name stops at its length, which is counted in letters and not in bytes.
        let mut long = String::new();
        for _ in 0..NAME_LENGTH + 5 {
            press(&mut long, "\u{e9}");
        }
        assert_eq!(long.chars().count(), NAME_LENGTH);
    }

    #[test]
    fn the_host_screen_opens_the_game_under_the_name_that_was_typed() {
        let mut world = World::new();
        let mut settings = Settings::default();
        settings.host_name = "Friday".into();
        world.insert_resource(settings);
        world.insert_resource(State::new(Screen::Host));
        world.insert_resource(Session::Solo);
        world.init_resource::<Opening>();
        world.init_resource::<Messages<KeyboardInput>>();
        world.init_resource::<Messages<SessionRequest>>();
        // The screen opens on the name of the game that was hosted last.
        world.run_system_once(host_screen).unwrap();
        assert_eq!(world.resource::<Opening>().name, "Friday");
        // The host types more of it, then Enter.
        let mut keys = world.resource_mut::<Messages<KeyboardInput>>();
        for key in strokes(" night ") {
            keys.write(key);
        }
        keys.write(stroke(Key::Enter, None));
        world.run_system_once(type_name).unwrap();
        let mut asked = world.resource_mut::<Messages<SessionRequest>>();
        let asked: Vec<SessionRequest> = asked.drain().collect();
        // The game opens under the name without the space at its end, and the name is kept
        // for the next game.
        let name = "Friday night".to_string();
        assert_eq!(asked, [SessionRequest::Host(Sign { name: name.clone() })]);
        assert_eq!(world.resource::<Settings>().host_name, name);
        // Nothing is typed into a game that is open already.
        world.insert_resource(Session::Host);
        world
            .resource_mut::<Messages<KeyboardInput>>()
            .write(stroke(Key::Enter, None));
        world.run_system_once(type_name).unwrap();
        assert!(world.resource::<Messages<SessionRequest>>().is_empty());
    }

    #[test]
    fn an_address_is_typed_a_key_at_a_time() {
        let press = |address: &mut String, letter: &str| {
            typed(address, &Key::Character(letter.into()), Some(letter));
        };
        let mut address = String::new();
        for letter in "192.168.1.20:4000".chars() {
            press(&mut address, &letter.to_string());
        }
        assert_eq!(address, "192.168.1.20:4000");
        typed(&mut address, &Key::Backspace, Some("\u{8}"));
        assert_eq!(address, "192.168.1.20:400");
        // What no address holds is left out.
        typed(&mut address, &Key::Space, Some(" "));
        press(&mut address, "/");
        typed(&mut address, &Key::ArrowLeft, None);
        assert_eq!(address, "192.168.1.20:400");
        // An address has an end.
        press(&mut address, &"a".repeat(2 * ADDRESS_LENGTH));
        assert_eq!(address.len(), ADDRESS_LENGTH);
    }

    #[test]
    fn a_level_of_sound_moves_by_tenths_and_stays_in_range() {
        assert_eq!(level(0.7 + 0.1), 0.8);
        assert_eq!(level(0.5 - 0.1), 0.4);
        assert_eq!(level(1.0 + 0.1), 1.0);
        assert_eq!(level(0.0 - 0.1), 0.0);
        // Ten steps down from the top reach silence exactly.
        let mut music = 1.0;
        for _ in 0..10 {
            music = level(music - 0.1);
        }
        assert_eq!(music, 0.0);
    }

    #[test]
    fn a_run_sets_the_record_of_its_league_and_of_a_team_of_its_size() {
        let (normal, hard) = (League::default(), league(Difficulty::Hard, false));
        let free = league(Difficulty::Normal, true);
        let run = |league: League, team: u32, cleared: u32| Run {
            difficulty: league.difficulty,
            free: league.free,
            cleared,
            ..lost_run(team)
        };
        let mut records = Records::default();
        assert!(keep_record(&mut records, &run(hard, 1, 3)));
        assert!(records.best(normal, 1).is_none());
        assert_eq!(records.best(hard, 1).unwrap().sector, 3);
        // A run in normal mode that got further is no record of hard mode.
        assert!(keep_record(&mut records, &run(normal, 1, 12)));
        assert_eq!(records.best(hard, 1).unwrap().sector, 3);
        assert!(!keep_record(&mut records, &run(hard, 1, 3)));
        assert!(keep_record(&mut records, &run(hard, 2, 1)));
        // Nor is a run in free mode a record of the standard tank, however far it got.
        assert!(keep_record(&mut records, &run(free, 1, 40)));
        assert_eq!(records.best(normal, 1).unwrap().sector, 12);
        assert_eq!(records.best(free, 1).unwrap().sector, 40);
        // The report and the main menu say which league a best run is of.
        assert_eq!(
            record_line(&records, normal, 1),
            format!("Best run: sector 12, seed 85GVTHQA, v{VERSION}")
        );
        assert_eq!(
            record_line(&records, hard, 1),
            format!("Best run in hard mode: sector 3, seed 85GVTHQA, v{VERSION}")
        );
        assert_eq!(
            record_line(&records, free, 1),
            format!("Best run in free mode: sector 40, seed 85GVTHQA, v{VERSION}")
        );
        assert_eq!(record_line(&records, normal, 3), "No run yet");
        assert_eq!(record_line(&records, hard, 3), "No run in hard mode yet");
        assert_eq!(
            record_line(&records, league(Difficulty::Hard, true), 1),
            "No run in hard mode and free mode yet"
        );
        let hard = records_of(&records, hard);
        assert_eq!(hard.len(), 2);
        assert!(hard[0].starts_with("Best run: sector 3"));
        assert!(hard[1].starts_with("2 players: sector 1"));
    }

    #[test]
    fn a_level_is_told_by_its_seed_its_version_and_its_difficulty() {
        let run = lost_run(1);
        assert_eq!(level_line(&run), format!("Seed 85GVTHQA, v{VERSION}"));
        let hard = Run {
            difficulty: Difficulty::Hard,
            ..run
        };
        assert_eq!(
            level_line(&hard),
            format!("Seed 85GVTHQA, v{VERSION}, hard mode")
        );
    }

    /// The difficulties that the solo screen lets the player switch to.
    fn offered(screen: fn(&mut App)) -> Vec<Difficulty> {
        let offered = presses(screen).into_iter().filter_map(|press| match press {
            Press::Difficulty(difficulty) => Some(difficulty),
            _ => None,
        });
        offered.collect()
    }

    #[test]
    fn the_solo_screen_offers_hard_mode_once_it_is_unlocked() {
        // A player who has not unlocked it has nothing to choose.
        assert_eq!(offered(|app| build(app, solo_screen)), []);
        // One who has is offered the difficulty that isn't chosen.
        let unlocked = |app: &mut App| {
            app.insert_resource(Unlocks::of(&[Unlock::HardMode]));
            build(app, solo_screen);
        };
        assert_eq!(offered(unlocked), [Difficulty::Hard]);
        let chosen = |app: &mut App| {
            app.insert_resource(Unlocks::of(&[Unlock::ImpossibleMode]))
                .insert_resource(Setup {
                    difficulty: Difficulty::Hard,
                    ..default()
                });
            build(app, solo_screen);
        };
        assert_eq!(offered(chosen), [Difficulty::Normal]);
        // A playtest has nothing locked.
        let playtest = |app: &mut App| {
            app.insert_resource(Options {
                playtest: true,
                seed: None,
                tank: None,
            });
            build(app, solo_screen);
        };
        assert_eq!(offered(playtest), [Difficulty::Hard]);
    }

    /// What the solo screen sets up for a player who chose a difficulty last, and free mode
    /// with it: the difficulty of the run, and whether it is in free mode.
    fn set_up_as(last: Difficulty, unlocks: &[Unlock], playtest: bool) -> (Difficulty, bool) {
        let mut world = World::new();
        world.insert_resource(Options {
            playtest,
            ..default()
        });
        world.insert_resource(Time::<Real>::default());
        let mut settings = Settings::default();
        (settings.solo_difficulty, settings.solo_free) = (last, true);
        world.insert_resource(settings);
        world.insert_resource(Unlocks::of(unlocks));
        world.init_resource::<Setup>();
        world.run_system_once(draw_a_seed_for_one).unwrap();
        let setup = world.resource::<Setup>();
        assert!(!setup.seed.is_empty() && !setup.typed);
        (setup.difficulty, setup.free)
    }

    fn set_up(last: Difficulty, unlocks: &[Unlock], playtest: bool) -> Difficulty {
        set_up_as(last, unlocks, playtest).0
    }

    #[test]
    fn the_solo_screen_opens_on_the_difficulty_that_was_chosen_last_if_it_is_unlocked() {
        use Difficulty::{Hard, Normal};
        assert_eq!(set_up(Normal, &[Unlock::HardMode], false), Normal);
        assert_eq!(set_up(Hard, &[Unlock::HardMode], false), Hard);
        // Whoever unlocked the impossible mode has hard mode too.
        assert_eq!(set_up(Hard, &[Unlock::ImpossibleMode], false), Hard);
        // A choice that was made in a playtest is not one for a run that counts.
        assert_eq!(set_up(Hard, &[Unlock::ChosenSeed], false), Normal);
        assert_eq!(set_up(Hard, &[], true), Hard);
        // Free mode is kept in the same way, each on its own.
        let free = Unlock::FreeMode;
        assert_eq!(set_up_as(Hard, &[free], false), (Normal, true));
        assert_eq!(set_up_as(Hard, &[Unlock::HardMode], false), (Hard, false));
        assert_eq!(
            set_up_as(Hard, &[free, Unlock::HardMode], false),
            (Hard, true)
        );
        assert_eq!(set_up_as(Normal, &[], true), (Normal, true));
    }

    #[test]
    fn the_solo_screen_starts_the_run_that_it_shows() {
        fn start(
            setup: Res<Setup>,
            (options, garage): (Res<Options>, Res<Garage>),
            mut requests: MessageWriter<RunRequest>,
            mut screen: ResMut<NextState<Screen>>,
        ) {
            begin(&setup, (&options, &garage), &mut requests, &mut screen);
        }
        // What the screen asks for: the seed, whether the game drew it, the difficulty,
        // whether the run is in free mode, and the tank of the player if it is.
        type Asked = (String, bool, Difficulty, bool, Vec<Genome>);
        let asked = |setup: Setup, options: Options, garage: Garage| -> Vec<Asked> {
            let mut world = World::new();
            world.insert_resource(setup);
            world.insert_resource(options);
            world.insert_resource(garage);
            world.init_resource::<Messages<RunRequest>>();
            world.init_resource::<NextState<Screen>>();
            world.run_system_once(start).unwrap();
            let mut requests = world.resource_mut::<Messages<RunRequest>>();
            let told = |request: RunRequest| match request {
                RunRequest::Start {
                    seed,
                    players: 1,
                    drawn,
                    difficulty,
                    free,
                    tanks,
                } => (seed, drawn, difficulty, free, tanks),
                other => panic!("{other:?}"),
            };
            requests.drain().map(told).collect()
        };
        let built = Garage { tank: racer() };
        let typed = Setup {
            seed: "TANKU".into(),
            typed: true,
            difficulty: Difficulty::Hard,
            free: false,
        };
        // A seed that was typed is not one that the game drew. Outside free mode, the
        // player has the standard tank, whatever stands in their garage.
        assert_eq!(
            asked(typed, Options::default(), built),
            [(
                "TANKU".to_string(),
                false,
                Difficulty::Hard,
                false,
                Vec::new()
            )]
        );
        // A run needs a seed.
        assert!(asked(Setup::default(), Options::default(), built).is_empty());
        // In free mode, the player has the tank of their garage, which is the standard one
        // until they build another.
        let free = || Setup {
            seed: "TANKU".into(),
            free: true,
            ..default()
        };
        assert_eq!(
            asked(free(), Options::default(), built),
            [(
                "TANKU".to_string(),
                true,
                Difficulty::Normal,
                true,
                vec![racer()]
            )]
        );
        assert_eq!(
            asked(free(), Options::default(), Garage::default())[0].4,
            [Genome::PLAYER]
        );
        // A playtest that brought a tank of its own plays every run in free mode, in it.
        let brought = Options {
            playtest: true,
            tank: Some(racer()),
            ..default()
        };
        let plain = Setup {
            seed: "TANKU".into(),
            ..default()
        };
        assert_eq!(
            asked(plain, brought, Garage::default()),
            [(
                "TANKU".to_string(),
                true,
                Difficulty::Normal,
                true,
                vec![racer()]
            )]
        );
    }

    /// What a screen of the menus says, a text at a time.
    fn texts(screen: fn(&mut App)) -> Vec<String> {
        let mut app = stage(screen, WINDOWS[0]);
        let world = app.world_mut();
        let mut texts = world.query::<&Text>();
        texts.iter(world).map(|text| text.0.clone()).collect()
    }

    fn says(texts: &[String], wanted: &str) -> bool {
        texts.iter().any(|text| text == wanted)
    }

    #[test]
    fn the_report_of_a_run_says_why_it_earned_nothing() {
        const TYPED: &str = "This run earned no achievement: its seed was typed or played again.";
        const FREE: &str = "Runs in free mode earn no achievement.";
        const RETRY: &str = "A retry earns no achievement.";
        // A run that earns says nothing of itself, and warns of a retry.
        let earned = texts(|app| {
            app.insert_resource(lost_run(1));
            build(app, game_over);
        });
        assert!(!says(&earned, TYPED) && !says(&earned, FREE) && says(&earned, RETRY));
        assert!(says(&earned, "RUN REPORT \u{b7} SOLO"));
        let retried = texts(|app| {
            app.insert_resource(Run {
                earns: false,
                ..lost_run(1)
            });
            build(app, game_over);
        });
        assert!(says(&retried, TYPED) && !says(&retried, FREE) && says(&retried, RETRY));
        // In free mode, a new run earns nothing either: the retry is not singled out.
        let free = texts(|app| {
            app.insert_resource(Run {
                earns: false,
                free: true,
                difficulty: Difficulty::Hard,
                ..lost_run(1)
            });
            build(app, game_over);
        });
        assert!(says(&free, FREE) && !says(&free, TYPED) && !says(&free, RETRY));
        assert!(says(
            &free,
            "RUN REPORT \u{b7} SOLO \u{b7} FREE MODE \u{b7} HARD MODE"
        ));
        // A player who has everything is told nothing of what a run earns.
        let complete = texts(|app| {
            app.insert_resource(Unlocks::of(&Unlock::ALL))
                .insert_resource(Run {
                    earns: false,
                    free: true,
                    ..lost_run(1)
                });
            build(app, game_over);
        });
        assert!(!says(&complete, FREE) && !says(&complete, RETRY));
    }

    #[test]
    fn the_main_menu_names_the_tank_that_a_playtest_brought() {
        let standard = texts(|app| build(app, main_menu));
        assert!(says(&standard, "PLAYER TANK") && says(&standard, "59 POINTS"));
        // The tank of a playtest may have points left over, and the menu says so.
        let brought = texts(|app| {
            app.insert_resource(Options {
                playtest: true,
                tank: Some(racer()),
                ..default()
            });
            build(app, main_menu);
        });
        assert!(says(&brought, "PLAYTEST TANK") && says(&brought, "57 OF 59 POINTS"));
        assert!(!says(&brought, "PLAYER TANK"));
        // In free mode, the tank is the one of the garage.
        let built = texts(|app| {
            app.insert_resource(Garage { tank: racer() })
                .insert_resource(Setup {
                    free: true,
                    ..default()
                });
            build(app, main_menu);
        });
        assert!(says(&built, "GARAGE TANK") && says(&built, "57 OF 59 POINTS"));
    }

    /// The leagues whose best runs the main menu offers to list instead: those of normal
    /// mode first, and the standard tank before free mode.
    fn listed(screen: fn(&mut App)) -> Vec<League> {
        let listed = presses(screen).into_iter().filter_map(|press| match press {
            Press::List(league) => Some(league),
            _ => None,
        });
        let mut listed: Vec<League> = listed.collect();
        listed.sort_by_key(|league| (league.difficulty, league.free));
        listed
    }

    #[test]
    fn the_main_menu_lists_the_best_runs_of_a_league_once_there_is_one() {
        let (normal, hard) = (League::default(), league(Difficulty::Hard, false));
        let (free, both) = (
            league(Difficulty::Normal, true),
            league(Difficulty::Hard, true),
        );
        // A player who only played in normal mode and in the standard tank is shown those
        // runs, and no way to others.
        let plain = |app: &mut App| {
            let mut records = Records::default();
            records.submit(League::default(), 1, 12, "85GVTHQA");
            app.insert_resource(records);
            build(app, main_menu);
        };
        assert_eq!(listed(plain), []);
        // Hard mode and free mode each have a switch once there is a run of theirs.
        let every = |app: &mut App| {
            app.insert_resource(Records::of_every_team(128, "85GVTHQA"));
            build(app, main_menu);
        };
        assert_eq!(listed(every), [free, hard]);
        let hard_alone = |app: &mut App| {
            let mut records = Records::default();
            records.submit(league(Difficulty::Hard, false), 1, 2, "85GVTHQA");
            app.insert_resource(records);
            build(app, main_menu);
        };
        assert_eq!(listed(hard_alone), [hard]);
        // A switch that is on leads back, runs or not.
        let on = |app: &mut App| {
            app.insert_resource(Listed(league(Difficulty::Hard, true)));
            build(app, main_menu);
        };
        assert_eq!(listed(on), [free, hard]);
        assert_ne!(both, normal);
    }

    #[test]
    fn the_solo_screen_offers_free_mode_and_its_garage_once_it_is_unlocked() {
        let offers = |screen: fn(&mut App)| -> (Vec<bool>, bool) {
            let presses = presses(screen);
            let free = presses.iter().filter_map(|press| match press {
                Press::Free(on) => Some(*on),
                _ => None,
            });
            (free.collect(), presses.contains(&Press::OpenGarage))
        };
        // Locked: nothing to choose.
        assert_eq!(offers(|app| build(app, solo_screen)), (vec![], false));
        // Unlocked and off: it can be turned on, and the garage waits for that.
        let off = |app: &mut App| {
            app.insert_resource(Unlocks::of(&[Unlock::FreeMode]));
            build(app, solo_screen);
        };
        assert_eq!(offers(off), (vec![true], false));
        let on = |app: &mut App| {
            app.insert_resource(Unlocks::of(&[Unlock::FreeMode]))
                .insert_resource(Setup {
                    free: true,
                    ..default()
                });
            build(app, solo_screen);
        };
        assert_eq!(offers(on), (vec![false], true));
        // A playtest that brought a tank plays in it: it has nothing to choose or build.
        let brought = |app: &mut App| {
            app.insert_resource(Options {
                playtest: true,
                tank: Some(racer()),
                ..default()
            });
            build(app, solo_screen);
        };
        assert_eq!(offers(brought), (vec![], false));
    }

    #[test]
    fn the_garage_offers_the_levels_that_the_points_pay_for() {
        let steps = |screen: fn(&mut App)| -> (Vec<Stat>, Vec<Stat>) {
            let presses = presses(screen);
            let of = |wanted: i8| -> Vec<Stat> {
                let steps = presses.iter().filter_map(|press| match press {
                    Press::Level(stat, by) if *by == wanted => Some(*stat),
                    _ => None,
                });
                // In the order of the table.
                let mut steps: Vec<Stat> = steps.collect();
                steps.sort_by_key(|stat| *stat as usize);
                steps
            };
            (of(-1), of(1))
        };
        // The standard tank spends every point: no level can be added, and every stat
        // that it has can come down. It has no piercing to give up.
        let (down, up) = steps(|app| build(app, garage_screen));
        assert_eq!(up, []);
        let has = |stat: &Stat| Genome::PLAYER.level(*stat) > 0;
        let owned: Vec<Stat> = Stat::GARAGE.into_iter().filter(has).collect();
        assert_eq!(down, owned);
        // A tank with points left takes the levels that they pay for. Its engine stays.
        let cheap = |app: &mut App| {
            let parts = [
                (Stat::Engine, 1),
                (Stat::Muzzle, 7),
                (Stat::Magazine, 5),
                (Stat::Loader, 7),
            ];
            let tank = Genome::of_the_garage(&parts).unwrap();
            assert_eq!(tank.cost(), 54);
            app.insert_resource(Garage { tank });
            build(app, garage_screen);
        };
        let (down, up) = steps(cheap);
        assert_eq!(down, [Stat::Muzzle, Stat::Magazine, Stat::Loader]);
        // Five points pay for a level of the engine, of the tracks needs an engine, which
        // it has, a first bounce, a first mine or piercing bullets, and not for a shield.
        let affordable = [
            Stat::Engine,
            Stat::Tracks,
            Stat::Ricochet,
            Stat::Mines,
            Stat::Piercing,
        ];
        assert_eq!(up, affordable);
    }

    #[test]
    fn the_garage_says_what_each_level_gives() {
        let parts = [(Stat::Engine, 1), (Stat::Shield, 1), (Stat::Piercing, 1)];
        let tank = Genome::of_the_garage(&parts).unwrap();
        let told: Vec<String> = Stat::GARAGE.map(|stat| gives(&tank, stat)).to_vec();
        let wanted = [
            "2 m/s",
            "90\u{b0}/s",
            "6 m/s",
            "1 round",
            "4 s",
            "None",
            "None",
            "10 s cooldown",
            "1 bullet",
        ];
        assert_eq!(told, wanted);
        let standard = Stat::GARAGE.map(|stat| gives(&Genome::PLAYER, stat));
        let wanted = [
            "5 m/s",
            "315\u{b0}/s",
            "9 m/s",
            "5 rounds",
            "1 s",
            "1 bounce",
            "2 mines",
            "9 s cooldown",
            "None",
        ];
        assert_eq!(standard.to_vec(), wanted);
    }

    #[test]
    fn the_menus_draw_the_tank_that_the_player_takes_into_a_run() {
        let garage = Garage { tank: racer() };
        let (plain, free) = (
            Setup::default(),
            Setup {
                free: true,
                ..default()
            },
        );
        let none = Options::default();
        let alone = [Screen::Menu, Screen::Solo, Screen::Achievements];
        for screen in alone {
            assert_eq!(shown_tank(screen, &plain, &none, &garage), Genome::PLAYER);
            assert_eq!(shown_tank(screen, &free, &none, &garage), racer());
        }
        // The garage draws what is being built, and a game of several players is in the
        // standard tank.
        assert_eq!(shown_tank(Screen::Garage, &plain, &none, &garage), racer());
        for screen in [Screen::Join, Screen::Lobby] {
            assert_eq!(shown_tank(screen, &free, &none, &garage), Genome::PLAYER);
        }
        // The tank that a playtest brought is drawn everywhere but in the garage.
        let parts = [(Stat::Engine, 2), (Stat::Piercing, 1)];
        let brought = Options {
            playtest: true,
            tank: Some(Genome::of_the_garage(&parts).unwrap()),
            ..default()
        };
        for screen in alone.into_iter().chain([Screen::Join, Screen::Lobby]) {
            assert_eq!(
                shown_tank(screen, &plain, &brought, &garage),
                brought.tank.unwrap()
            );
        }
    }

    #[test]
    fn a_playtest_sets_no_record() {
        let run = Run {
            seed_text: "TEST".into(),
            seed: 1,
            difficulty: default(),
            players: 1,
            team: 1,
            solo: true,
            sector: 41,
            cleared: 40,
            checkpoint: 40,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase: RunPhase::Over,
            lost_to: None,
            playtest: true,
            earns: false,
            free: false,
        };
        let mut records = Records::default();
        assert!(!keep_record(&mut records, &run));
        assert_eq!(records, Records::default());
    }
}
