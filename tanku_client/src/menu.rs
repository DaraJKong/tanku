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
    lock::PASSWORD_LENGTH,
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
    session::{Asked, Drawn, Found, Game, Invitation, Notice, Session, SessionRequest, Shut, Sign},
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
/// What the solo screen and the lobby say of arena mode when it is chosen.
const ARENA_MODE: &str = "In arena mode, the run stays in one room. The groups of the sectors \
                          come there one after another, and the next one waits outside.";

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
    /// On that screen: turns arena mode on or off.
    Arena(bool),
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
    /// On that screen: gives the keys to a field.
    Field(Field),
    /// Shows the password that is being typed, or hides it again.
    Reveal,
    /// Opens the screen where the address of a game is typed.
    OpenJoin,
    /// Joins the game at the address that was typed.
    Join,
    /// Joins a game that was found on the local network, by the number of that game.
    JoinFound(u64),
    /// On the join screen: leaves the game that asks for its password for the list.
    Forget,
    /// In a lobby: the player is ready for the run, or takes that back.
    Ready,
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
    /// In a playtest of arena mode: sends a group all at once, or in waves again.
    WholeGroups,
    /// In a playtest of arena mode: brings blocks and crates back after a while, or lays
    /// the room out anew between two groups again.
    BlocksReturn,
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
struct HostRoot;

#[derive(Component)]
struct JoinRoot;

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

/// The text that shows a password as it is typed: by the host of a game, or by a player who
/// joins one.
#[derive(Component)]
struct PasswordField;

/// A password as it is typed: its letters, and whether the screen shows them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Secret {
    text: String,
    shown: bool,
}

/// A field of the host screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Field {
    #[default]
    Name,
    Password,
}

/// What a host types for the game that they are about to open.
#[derive(Resource, Default)]
struct Opening {
    name: String,
    password: Secret,
    /// The field that the keys write into.
    field: Field,
}

/// The password that a player types to join a game that asks for one.
#[derive(Resource, Default)]
struct Entry(Secret);

/// What a single player has set for their next run.
#[derive(Resource, Default)]
struct Setup {
    seed: String,
    /// Whether the player typed the seed. The game drew it otherwise.
    typed: bool,
    difficulty: Difficulty,
    /// Whether the run is in free mode, in the tank of the player's garage.
    free: bool,
    /// Whether the run is in arena mode: in one room, against one group after another.
    arena: bool,
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
        .init_resource::<Entry>()
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
        .add_systems(OnEnter(Screen::Host), set_up_to_host)
        .add_systems(OnEnter(Screen::Join), set_up_to_join)
        .add_systems(OnExit(Screen::Join), forget_the_game)
        .add_systems(OnEnter(Screen::Lobby), draw_a_seed.run_if(gathering))
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
                solo_screen.run_if(in_state(Screen::Solo)),
                garage_screen.run_if(in_state(Screen::Garage)),
                refresh_pause
                    .run_if(in_state(Overlay::Pause).and_then(resource_exists::<Playtest>)),
                refresh_settings.run_if(in_state(Overlay::Settings)),
                (
                    lobby_screen.run_if(in_state(Screen::Lobby)),
                    show_copy,
                    show_seed.after(solo_screen),
                )
                    .chain(),
                (
                    host_screen.run_if(in_state(Screen::Host)),
                    (forget_the_password, join_screen, list_games)
                        .chain()
                        .run_if(in_state(Screen::Join)),
                    (show_address, show_what_is_typed, tell_how_it_goes),
                )
                    .chain(),
                list_records.run_if(in_state(Screen::Menu)),
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
            "{best}: {} {}, seed {}, v{}",
            league.unit(),
            record.sector,
            record.seed,
            record.version
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
            "{whose}: {} {}, seed {}, v{}",
            league.unit(),
            record.sector,
            record.seed,
            record.version
        ))
    });
    teams.collect()
}

/// The seed of a run with the version of the game and, in hard mode, the difficulty: what
/// tells one level from another. The seed of a run in arena mode is that of a room.
fn level_line(run: &Run) -> String {
    let mut level = format!("Seed {}, v{VERSION}", run.seed_text);
    if run.difficulty == Difficulty::Hard {
        level.push_str(", hard mode");
    }
    if run.arena {
        level.push_str(", arena mode");
    }
    level
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
    setup.arena = last_arena(&settings, &unlocks, &options);
    listed.0 = League {
        difficulty: setup.difficulty,
        free: setup.free,
        arena: setup.arena,
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
/// the player asks for those of another league. Hard mode, arena mode and free mode each
/// have a switch, once the player has a run in them.
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
        let offers_arena = league.arena || played(|league| league.arena);
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
                    if offers_arena {
                        let to = League {
                            arena: !league.arena,
                            ..league
                        };
                        league_switch(header, "Arena mode", to, league.arena);
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
    let tank = shown_tank(Screen::Menu, (&setup, None), &options, &garage);
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
    lobbies: Query<&Lobby>,
    mut shown: Local<Option<(Screen, Genome)>>,
) {
    // The drawing follows the tank: every change in the garage shows at once.
    let shared = lobbies.single().ok().map(|lobby| lobby.free);
    let genome = shown_tank(*screen.get(), (&setup, shared), &options, &garage);
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
/// there. `shared` tells whether the game of several players that the player is in, if they
/// are in one, is set to free mode.
fn shown_tank(
    screen: Screen,
    (setup, shared): (&Setup, Option<bool>),
    options: &Options,
    garage: &Garage,
) -> Genome {
    let alone = matches!(screen, Screen::Menu | Screen::Solo | Screen::Achievements);
    match (screen, shared) {
        // The garage draws the tank that is being built.
        (Screen::Garage, _) => garage.tank,
        // A single player in free mode has the tank that they built or brought, and so has
        // each of several players.
        _ if alone && is_free(setup, options) => own_tank(options, garage),
        (Screen::Lobby, Some(true)) => own_tank(options, garage),
        // Outside free mode, everyone has the standard tank, and the host of a playtest the
        // one that it brought.
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

/// Asks to join a game: the one that asks for its password, with the password that was typed
/// for it, or else the one of the link in a browser, or the one at the address that was
/// typed, which is kept for next time.
fn ask_to_join(
    address: &Address,
    (asked, entry): (&Asked, &Entry),
    settings: &mut Settings,
    requests: &mut MessageWriter<SessionRequest>,
) {
    if let Some(shut) = &asked.0 {
        let password = entry.0.text.clone();
        requests.write(SessionRequest::JoinLocked(shut.at.clone(), password));
        return;
    }
    if let Some(link) = net::link() {
        requests.write(SessionRequest::Join(link));
        return;
    }
    let typed = address.0.trim();
    if settings.address != typed {
        settings.address = typed.to_string();
    }
    requests.write(SessionRequest::Join(typed.to_string()));
}

/// Picks a game that the join screen lists. Its address is the one that the screen shows from
/// then on, and is kept for next time. A game that has a password asks for it first, and one
/// that has none is joined at once.
fn pick(
    game: &Game,
    (address, asked): (&mut Address, &mut Asked),
    settings: &mut Settings,
    requests: &mut MessageWriter<SessionRequest>,
) {
    address.0.clone_from(&game.address);
    if settings.address != game.address {
        settings.address.clone_from(&game.address);
    }
    if game.locked {
        asked.0 = Some(Shut {
            at: game.address.clone(),
            name: game.name.clone(),
            address: game.address.clone(),
        });
    } else {
        requests.write(SessionRequest::Join(game.address.clone()));
    }
}

/// Takes what the player types on the join screen: the address of a game, or the password of
/// the one that asks for it. Enter joins.
fn type_address(
    mut keys: MessageReader<KeyboardInput>,
    screen: Res<State<Screen>>,
    session: Res<Session>,
    mut address: ResMut<Address>,
    (asked, mut entry): (Res<Asked>, ResMut<Entry>),
    mut settings: ResMut<Settings>,
    mut requests: MessageWriter<SessionRequest>,
) {
    // What is being joined stays as it is.
    let typing = *screen.get() == Screen::Join && *session == Session::Solo;
    for key in keys.read() {
        if !typing || key.state != ButtonState::Pressed {
            continue;
        }
        let (pressed, text) = (&key.logical_key, key.text.as_deref());
        if *pressed == Key::Enter {
            ask_to_join(&address, (&asked, &entry), &mut settings, &mut requests);
        } else if asked.0.is_some() {
            let mut password = entry.0.text.clone();
            typed_words(&mut password, pressed, text, PASSWORD_LENGTH);
            if password != entry.0.text {
                entry.0.text = password;
            }
        } else {
            typed(&mut address.0, pressed, text);
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
        arena: last_arena(&settings, &unlocks, &options),
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

/// Whether the player chose arena mode for the last run of their own, if it is still theirs
/// to choose.
fn last_arena(settings: &Settings, unlocks: &Unlocks, options: &Options) -> bool {
    settings.solo_arena && open(unlocks, options, Unlock::ArenaMode)
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
        arena: league.arena,
        tanks,
    }
}

/// The league of the run that a single player set up.
fn league_of(setup: &Setup, options: &Options) -> League {
    League {
        difficulty: setup.difficulty,
        free: is_free(setup, options),
        arena: setup.arena,
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

/// What a key makes of a seed that a player may type, if it makes anything of it. The first
/// key takes the place of a seed that the game drew. A key that writes nothing of a seed,
/// such as Shift or an arrow, leaves the seed as it is.
fn retyped(seed: &str, typed: bool, key: &Key, text: Option<&str>) -> Option<String> {
    let letters = text.unwrap_or_default().chars();
    let writes = letters.filter(char::is_ascii_alphanumeric).count() > 0;
    if !writes && *key != Key::Backspace {
        return None;
    }
    let mut wanted = String::new();
    if typed {
        wanted.push_str(seed);
    }
    typed_seed(&mut wanted, key, text);
    Some(wanted).filter(|wanted| wanted != seed)
}

/// Takes what the player types on the solo screen: the seed of the run, once typing one is
/// unlocked. Enter starts the run. In the lobby of a game, its host types the seed of the
/// next run, which nothing locks.
fn type_seed(
    mut keys: MessageReader<KeyboardInput>,
    screen: Res<State<Screen>>,
    (session, unlocks, seat): (Res<Session>, Res<Unlocks>, Res<LocalSeat>),
    (options, garage): (Res<Options>, Res<Garage>),
    (mut setup, mut drawn): (ResMut<Setup>, ResMut<Drawn>),
    mut lobbies: Query<&mut Lobby>,
    (mut requests, mut session_requests): (
        MessageWriter<RunRequest>,
        MessageWriter<SessionRequest>,
    ),
    mut next_screen: ResMut<NextState<Screen>>,
) {
    let setting_up = *screen.get() == Screen::Solo;
    let typing = setting_up && open(&unlocks, &options, Unlock::ChosenSeed);
    let gathered = *screen.get() == Screen::Lobby;
    let hosting = gathered && *session == Session::Host;
    let mut lobby = lobbies.single_mut().ok().filter(|_| gathered);
    for key in keys.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        let (pressed, text) = (&key.logical_key, key.text.as_deref());
        // In a lobby, Enter says that the player is ready, or takes it back.
        if let (Some(lobby), Key::Enter) = (&lobby, pressed) {
            let mine = lobby.ready.get(seat.0 as usize).copied().unwrap_or(false);
            session_requests.write(SessionRequest::Ready(!mine));
            continue;
        }
        // A playtest that was given a seed keeps it.
        let types = hosting && options.seed.is_none();
        if let Some(lobby) = lobby.as_mut().filter(|_| types) {
            if let Some(seed) = retyped(&lobby.seed, lobby.typed, pressed, text) {
                (lobby.seed, lobby.typed) = (seed, true);
                // A seed that the host typed is none that the game drew.
                drawn.0 = false;
            }
        } else if setting_up && *pressed == Key::Enter {
            begin(&setup, (&options, &garage), &mut requests, &mut next_screen);
        } else if typing && let Some(seed) = retyped(&setup.seed, setup.typed, pressed, text) {
            (setup.seed, setup.typed) = (seed, true);
        }
    }
}

/// Shows the seed of the solo screen, or the one of the lobby to its host, with a mark that
/// blinks where the next letter goes when the player may type one.
fn show_seed(
    time: Res<Time<Real>>,
    screen: Res<State<Screen>>,
    (session, unlocks): (Res<Session>, Res<Unlocks>),
    options: Res<Options>,
    setup: Res<Setup>,
    lobbies: Query<&Lobby>,
    mut fields: Query<&mut Text, With<SeedField>>,
) {
    let (seed, types) = match (*screen.get(), lobbies.single()) {
        (Screen::Lobby, Ok(lobby)) => {
            let types = *session == Session::Host && options.seed.is_none();
            (&lobby.seed, types)
        }
        _ => (&setup.seed, open(&unlocks, &options, Unlock::ChosenSeed)),
    };
    // A seed that is as long as it gets takes no further letter.
    let blinks = types && seed.len() < SEED_LENGTH && time.elapsed_secs() % 1.0 < 0.5;
    let wanted = format!("{seed}{}", if blinks { "_" } else { "" });
    for mut text in &mut fields {
        if text.0 != wanted {
            text.0.clone_from(&wanted);
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
    let rooms = open(&unlocks, &options, Unlock::ArenaMode);
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
                row(parent, "Arena mode", |row| {
                    if rooms {
                        for (name, on) in [("Off", false), ("On", true)] {
                            choice(row, name, 90.0, Press::Arena(on), setup.arena == on);
                        }
                    } else {
                        let unlock = unlocks::condition(Unlock::ArenaMode);
                        row.spawn(theme::text(unlock, 16.0, theme::TEXT_DIM));
                    }
                });
                if setup.arena {
                    line(parent, ARENA_MODE, theme::TEXT_DIM);
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
                let unearned = match (setup.arena, free, setup.typed) {
                    (true, ..) => Some("A run in arena mode earns no achievement."),
                    (false, true, _) => Some("A run in free mode earns no achievement."),
                    (false, false, true) => Some("A run on a typed seed earns no achievement."),
                    (false, false, false) => None,
                };
                if let Some(why) = unearned.filter(|_| unlocks.wanting() && !options.playtest) {
                    line(parent, why, theme::TEXT);
                }
                primary(parent, "Start", Press::Begin);
                button(parent, "Back", Press::Abandon);
            });
        });
}

/// The screen that the garage was opened from, and goes back to: the one where a single
/// player sets up a run, or the lobby of a game of several players.
fn garage_opened_from(session: Session) -> Screen {
    match session {
        Session::Solo => Screen::Solo,
        Session::Host | Session::Guest => Screen::Lobby,
    }
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

/// A password as the screen shows it while it is typed: its letters, or a dot for each.
fn masked(secret: &Secret) -> String {
    if secret.shown {
        secret.text.clone()
    } else {
        "\u{2022}".repeat(secret.text.chars().count())
    }
}

/// Asks to open the game that a host set up. Its name is kept for the next one, and its
/// password is not.
fn ask_to_host(
    opening: &Opening,
    settings: &mut Settings,
    requests: &mut MessageWriter<SessionRequest>,
) {
    let name = named(&opening.name);
    if settings.host_name != name {
        settings.host_name.clone_from(&name);
    }
    // The lobby opens on the run that the host set up last, which they can change there.
    requests.write(SessionRequest::Host(Sign {
        name,
        password: opening.password.text.clone(),
        difficulty: settings.host_difficulty,
        free: settings.host_free,
        arena: settings.host_arena,
    }));
}

/// Takes what the player types on the host screen: the name of the game or its password,
/// whichever field has the keys. Tab gives them to the other field, and Enter opens the game.
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
        let (pressed, text) = (&key.logical_key, key.text.as_deref());
        match (pressed, opening.field) {
            (Key::Enter, _) => ask_to_host(&opening, &mut settings, &mut requests),
            (Key::Tab, Field::Name) => opening.field = Field::Password,
            (Key::Tab, Field::Password) => opening.field = Field::Name,
            (_, Field::Name) => {
                let mut name = opening.name.clone();
                typed_words(&mut name, pressed, text, NAME_LENGTH);
                if name != opening.name {
                    opening.name = name;
                }
            }
            (_, Field::Password) => {
                let mut password = opening.password.text.clone();
                typed_words(&mut password, pressed, text, PASSWORD_LENGTH);
                if password != opening.password.text {
                    opening.password.text = password;
                }
            }
        }
    }
}

/// Shows what is typed into the fields of the host screen and into the password of the join
/// screen, with a mark that blinks where the next letter goes: in the field that has the
/// keys, while it has room.
fn show_what_is_typed(
    time: Res<Time<Real>>,
    screen: Res<State<Screen>>,
    session: Res<Session>,
    (opening, entry): (Res<Opening>, Res<Entry>),
    mut names: Query<&mut Text, (With<NameField>, Without<PasswordField>)>,
    mut passwords: Query<&mut Text, With<PasswordField>>,
) {
    let hosting = *screen.get() == Screen::Host;
    let blinks = *session == Session::Solo && time.elapsed_secs() % 1.0 < 0.5;
    let mark = |typed: bool, letters: &str, room: usize| {
        let marked = typed && blinks && letters.chars().count() < room;
        if marked { "_" } else { "" }
    };
    let typed = hosting && opening.field == Field::Name;
    let name = format!(
        "{}{}",
        opening.name,
        mark(typed, &opening.name, NAME_LENGTH)
    );
    // The password of the host screen, or the one that the join screen asks for.
    let (secret, typed) = if hosting {
        (&opening.password, opening.field == Field::Password)
    } else {
        (&entry.0, true)
    };
    let password = format!(
        "{}{}",
        masked(secret),
        mark(typed, &secret.text, PASSWORD_LENGTH)
    );
    for mut text in &mut names {
        if text.0 != name {
            text.0.clone_from(&name);
        }
    }
    for mut text in &mut passwords {
        if text.0 != password {
            text.0.clone_from(&password);
        }
    }
}

/// The box of a field that takes what is typed, around the text that shows it. Where a
/// screen has several, each is a button that gives the field the keys, and the one that has
/// them is edged with the accent.
fn field_box(parent: &mut ChildSpawnerCommands, text: impl Bundle, among: Option<(Press, bool)>) {
    let node = Node {
        flex_grow: 1.0,
        padding: UiRect::axes(px(12), px(10)),
        min_height: px(48),
        align_items: AlignItems::Center,
        border: UiRect::all(px(1)),
        ..default()
    };
    match among {
        None => {
            parent.spawn((ruled(node), children![text]));
        }
        Some((press, false)) => {
            let ground = BackgroundColor(theme::SURFACE_RAISED);
            let edge = BorderColor::all(theme::RULE);
            parent.spawn((Button, press, node, ground, edge, children![text]));
        }
        Some((press, true)) => {
            let ground = BackgroundColor(theme::SURFACE_RAISED);
            let edge = BorderColor::all(theme::ACCENT);
            parent.spawn((Button, Lit, press, node, ground, edge, children![text]));
        }
    }
}

/// The field of a password, next to the button that shows what is typed or hides it again.
fn password_row(parent: &mut ChildSpawnerCommands, shown: bool, among: Option<(Press, bool)>) {
    let row = Node {
        column_gap: px(6),
        align_items: AlignItems::Stretch,
        ..default()
    };
    parent.spawn(row).with_children(|row| {
        let text = (PasswordField, theme::text("", 22.0, theme::TEXT));
        field_box(row, text, among);
        // As tall as the field, with its word in the middle.
        let reveal = if shown { "Hide" } else { "Show" };
        row.spawn((
            Button,
            Press::Reveal,
            Node {
                padding: UiRect::axes(px(6), px(5)),
                align_items: AlignItems::Center,
                ..button_node(px(76))
            },
            BackgroundColor(theme::SURFACE_RAISED),
            BorderColor::all(theme::RULE),
            children![theme::text(reveal, 16.0, theme::TEXT)],
        ));
    });
}

/// A host who opens the host screen finds the name of the game that they hosted last, and no
/// password: the game keeps none.
fn set_up_to_host(settings: Res<Settings>, mut opening: ResMut<Opening>) {
    *opening = Opening {
        name: settings.host_name.clone(),
        ..default()
    };
}

/// The screen where a player sets up the game that they host for others: its name and its
/// password, both of which a game does without.
fn host_screen(
    mut commands: Commands,
    opening: Res<Opening>,
    roots: Query<Entity, With<HostRoot>>,
    mut drawn: Local<Option<(Field, bool)>>,
) {
    // What is typed shows in the fields as they are. The screen is drawn again when the
    // keys go to the other field, and when the password shows or hides.
    let wanted = (opening.field, opening.password.shown);
    if !roots.is_empty() && *drawn == Some(wanted) {
        return;
    }
    *drawn = Some(wanted);
    for root in &roots {
        commands.entity(root).despawn();
    }
    commands
        .spawn((
            HostRoot,
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
                let named = opening.field == Field::Name;
                let text = (NameField, theme::text("", 22.0, theme::TEXT));
                parent.spawn(Node::default()).with_children(|row| {
                    field_box(row, text, Some((Press::Field(Field::Name), named)))
                });
                section(parent, "Password");
                let among = (Press::Field(Field::Password), !named);
                password_row(parent, opening.password.shown, Some(among));
                line(
                    parent,
                    "The others find the game under its name on their join screen. With a \
                     password, only those who type it get in. A game needs neither. Tab changes \
                     the field, and Enter opens the game.",
                    theme::TEXT_DIM,
                );
                primary(parent, "Open the game", Press::Host);
                button(parent, "Back", Press::Abandon);
                status_line(parent);
            });
        });
}

/// A player who opens the join screen finds the address of the game that they joined last.
fn set_up_to_join(settings: Res<Settings>, mut address: ResMut<Address>) {
    if address.0.is_empty() {
        address.0.clone_from(&settings.address);
    }
}

/// A player who leaves the join screen leaves the game that asked for its password.
fn forget_the_game(mut asked: ResMut<Asked>, mut entry: ResMut<Entry>) {
    asked.0 = None;
    *entry = Entry::default();
}

/// A password is typed anew for each game that asks for one, and after one that was refused.
fn forget_the_password(asked: Res<Asked>, mut entry: ResMut<Entry>) {
    if asked.is_changed() && !entry.0.text.is_empty() {
        entry.0.text.clear();
    }
}

/// How the join screen names a game that asks for its password.
fn shut_words(shut: &Shut) -> String {
    match shut.name.as_str() {
        "" => format!("The game at {}", shut.address),
        name => format!("{name}, at {}", shut.address),
    }
}

/// The screen where a player joins a game: one that the local network lists, or one at an
/// address that they type. A game that has a password asks for it there.
fn join_screen(
    mut commands: Commands,
    (asked, entry): (Res<Asked>, Res<Entry>),
    roots: Query<Entity, With<JoinRoot>>,
    mut drawn: Local<Option<(Option<Shut>, bool)>>,
) {
    let wanted = (asked.0.clone(), entry.0.shown);
    if !roots.is_empty() && drawn.as_ref() == Some(&wanted) {
        return;
    }
    *drawn = Some(wanted);
    for root in &roots {
        commands.entity(root).despawn();
    }
    commands
        .spawn((
            JoinRoot,
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
                // A game that has a password asks for it before anything else.
                if let Some(shut) = &asked.0 {
                    section(parent, "Password of the game");
                    line(parent, shut_words(shut), theme::TEXT);
                    password_row(parent, entry.0.shown, None);
                    let hint = "Its host gives it to those who may join. Enter joins.";
                    line(parent, hint, theme::TEXT_DIM);
                    primary(parent, "Join", Press::Join);
                    button(parent, "Back", Press::Forget);
                    status_line(parent);
                    return;
                }
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
            let mut words = format!("{} of {MAX_PLAYERS} players", game.players);
            if game.standing == Standing::Full {
                words.push_str(" \u{b7} full");
            }
            // A game that has a password asks whoever joins for it.
            if game.locked {
                words.push_str(" \u{b7} locked");
            }
            words
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

/// How others join the game of this machine, in the lobby of its host: the players at a
/// desktop game, or those in a browser.
fn ways_to_join(
    parent: &mut ChildSpawnerCommands,
    invitation: &Invitation,
    way: Way,
    copied: &Copied,
) {
    // The buttons of a way stand in a row under what it tells.
    let buttons = Node {
        column_gap: px(6),
        ..default()
    };
    let code = invitation.web.as_ref().filter(|_| way == Way::Browser);
    if let (Some(code), Some(port)) = (code, invitation.web_port) {
        let [place, first, second] = link_ending(code);
        section(parent, "Link for a browser");
        parent.spawn(theme::figure(place, 20.0, theme::TEXT));
        // Nobody types the print: a button copies the lot.
        parent.spawn(theme::figure(first, 15.0, theme::TEXT));
        parent.spawn(theme::figure(second, 15.0, theme::TEXT));
        parent.spawn(buttons).with_children(|row| {
            let words = (
                CopyLabel,
                theme::text(copy_label(copied), 16.0, theme::TEXT),
            );
            small_with(row, words, 120.0, Press::Copy);
            small(row, "For a desktop", 140.0, Press::Show(Way::Desktop));
        });
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
    match &invitation.address {
        Some(address) => {
            parent.spawn(theme::figure(address.clone(), 24.0, theme::TEXT));
        }
        None => line(
            parent,
            "This machine has no address on a network.",
            theme::TEXT_DIM,
        ),
    }
    if invitation.web.is_some() {
        parent.spawn(buttons).with_children(|row| {
            small(row, "For a browser", 140.0, Press::Show(Way::Browser));
        });
    }
    if invitation.address.is_some() {
        let hint = "Players on this network find the game on their join screen, or type this \
                    address there.";
        line(parent, hint, theme::TEXT_DIM);
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

/// Whether the lobby opens for the players to gather: for a new game, or after a run. A
/// player who comes back from their garage finds the lobby as it was.
fn gathering(mut transitions: MessageReader<StateTransitionEvent<Screen>>) -> bool {
    let from_the_garage = |transition: &StateTransitionEvent<Screen>| {
        transition.exited == Some(Screen::Garage) && transition.entered == Some(Screen::Lobby)
    };
    !transitions.read().any(from_the_garage)
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
        lobby.typed = false;
        drawn.0 = true;
    }
}

/// What a single player has still to unlock before they can play the run that a lobby is set
/// to. A host who starts alone plays a solo run, with its locks.
fn locked_alone(lobby: &Lobby, unlocks: &Unlocks, options: &Options) -> Option<Unlock> {
    let needed = [
        lobby.difficulty.unlock(),
        lobby.arena.then_some(Unlock::ArenaMode),
        lobby.typed.then_some(Unlock::ChosenSeed),
        lobby.free.then_some(Unlock::FreeMode),
    ];
    let mut needed = needed.into_iter().flatten();
    needed.find(|unlock| !open(unlocks, options, *unlock))
}

/// What the lobby tells a host who is alone and has not unlocked what the run is set to.
fn locked_words(unlock: Unlock) -> String {
    let (name, condition) = (unlocks::name(unlock), unlocks::condition(unlock));
    format!(
        "Locked for a single player: {} ({}).",
        name.to_lowercase(),
        condition.to_lowercase()
    )
}

/// Why the host of a lobby can't be ready for its run, if they can't: the run has no seed, or
/// the host is alone and has not unlocked what the run is set to.
pub(crate) fn held_back(lobby: &Lobby, unlocks: &Unlocks, options: &Options) -> Option<String> {
    if lobby.seed.is_empty() {
        return Some("A run needs a seed: type one, or take a new one.".to_string());
    }
    let alone = lobby.players() == 1;
    let locked = locked_alone(lobby, unlocks, options).filter(|_| alone);
    locked.map(locked_words)
}

/// Why a host can't start the next run from the report of the last one, if they can't: on
/// the same seed, or on one that the game draws for it. A host whom the others have left
/// would play alone, under the locks of solo.
fn held_back_after(
    lobby: &Lobby,
    fresh: bool,
    unlocks: &Unlocks,
    options: &Options,
) -> Option<String> {
    let next = Lobby {
        // A seed that the game draws is none that was typed.
        typed: lobby.typed && !fresh,
        ..lobby.clone()
    };
    held_back(&next, unlocks, options)
}

/// Why the next run of a lobby will earn no achievement, if it won't.
fn unearned(lobby: &Lobby) -> Option<&'static str> {
    if lobby.playtest {
        Some("A playtest of the host: its runs set no record and earn no achievement.")
    } else if lobby.arena {
        Some("A run in arena mode earns no achievement.")
    } else if lobby.free {
        Some("A run in free mode earns no achievement.")
    } else if lobby.typed {
        Some("A run on a typed seed earns no achievement.")
    } else {
        None
    }
}

/// A seat of the lobby: its number, who sits there, and whether they are ready for the run.
fn seat_row(
    parent: &mut ChildSpawnerCommands,
    seat: usize,
    (member, ready): (Option<Member>, bool),
    own: bool,
) {
    let taken = member.is_some();
    let word = match (taken, ready) {
        (false, _) => theme::label("", 13.0, theme::TEXT_DIM),
        (true, false) => theme::label("Not ready", 13.0, theme::TEXT_DIM),
        (true, true) => theme::label("Ready", 13.0, theme::GOOD),
    };
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
            // The word of the player stands at the end of the row.
            (
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                children![theme::text(who, 16.0, ink)],
            ),
            word,
        ],
    ));
}

/// How the lobby stands, under the button that says a player is ready: the countdown to the
/// run, whom the run waits for, or what starts it.
fn lobby_words(lobby: &Lobby, own: usize) -> (String, Color) {
    let seated = lobby.seats.iter().zip(lobby.ready);
    let waited_for = seated.filter(|(seat, ready)| seat.is_some() && !ready);
    let mine = lobby.ready.get(own).copied().unwrap_or(false);
    match (lobby.countdown, waited_for.count()) {
        (Some(seconds), _) => (format!("The run starts in {seconds}"), theme::ACCENT),
        (None, 1) if mine => ("Waiting for 1 player".to_string(), theme::TEXT_DIM),
        (None, others) if mine && others > 0 => {
            (format!("Waiting for {others} players"), theme::TEXT_DIM)
        }
        (None, _) => {
            let told = "The run starts once everyone is ready";
            (told.to_string(), theme::TEXT_DIM)
        }
    }
}

/// A column of a menu to the right of its margin, level with what stands under its title. It
/// ends short of what the sheet draws.
fn aside(root: &mut ChildSpawnerCommands, content: impl FnOnce(&mut ChildSpawnerCommands)) {
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: px(620),
        top: px(227),
        width: px(380),
        flex_direction: FlexDirection::Column,
        row_gap: px(10),
        ..default()
    })
    .with_children(content);
}

/// The lobby: who is there, what they are about to play, and how others join. The host sets
/// the run, and the others see it. It is drawn again whenever something it shows changes.
fn lobby_screen(
    mut commands: Commands,
    lobbies: Query<Ref<Lobby>>,
    session: Res<Session>,
    seat: Res<LocalSeat>,
    (invitation, way, copied): (Res<Invitation>, Res<Way>, Res<Copied>),
    (options, unlocks, garage): (Res<Options>, Res<Unlocks>, Res<Garage>),
    roots: Query<Entity, With<LobbyRoot>>,
) {
    let Ok(lobby) = lobbies.single() else {
        return;
    };
    let stale = lobby.is_changed()
        || seat.is_changed()
        || invitation.is_changed()
        || way.is_changed()
        || unlocks.is_changed()
        || garage.is_changed();
    if !roots.is_empty() && !stale {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    let host = *session == Session::Host;
    // A playtest that was given a seed keeps it, and one that brought a tank plays in it.
    let (reseeds, brought) = (host && options.seed.is_none(), options.tank.is_some());
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
                // A game that has a password says so: whoever is to join needs it.
                let lock = if lobby.locked { " \u{b7} locked" } else { "" };
                parent.spawn(theme::label(
                    format!("{game} \u{b7} {role}{lock}"),
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
                        let seated = lobby.seats.iter().zip(lobby.ready);
                        for (number, (member, ready)) in seated.enumerate() {
                            let own = number == seat.0 as usize;
                            seat_row(seats, number, (*member, ready), own);
                        }
                    });
                section(parent, "Run");
                row(parent, "Difficulty", |row| {
                    if !host {
                        let name = unlocks::difficulty_name(lobby.difficulty);
                        row.spawn(theme::text(name, 16.0, theme::TEXT));
                        return;
                    }
                    for difficulty in Difficulty::ALL {
                        let name = unlocks::difficulty_name(difficulty);
                        let chosen = lobby.difficulty == difficulty;
                        choice(row, name, 90.0, Press::Difficulty(difficulty), chosen);
                    }
                });
                row(parent, "Arena mode", |row| {
                    if !host {
                        row.spawn(theme::text(on_off(lobby.arena), 16.0, theme::TEXT));
                        return;
                    }
                    for (name, on) in [("Off", false), ("On", true)] {
                        choice(row, name, 90.0, Press::Arena(on), lobby.arena == on);
                    }
                });
                row(parent, "Free mode", |row| {
                    if !host {
                        row.spawn(theme::text(on_off(lobby.free), 16.0, theme::TEXT));
                        return;
                    }
                    for (name, on) in [("Off", false), ("On", true)] {
                        choice(row, name, 90.0, Press::Free(on), lobby.free == on);
                    }
                });
                // In free mode, each player has the tank of their own garage.
                if lobby.free {
                    row(parent, "Your tank", |row| {
                        if host && brought {
                            let told = "The tank of this playtest";
                            row.spawn(theme::text(told, 16.0, theme::TEXT));
                            return;
                        }
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
                            theme::code(lobby.seed.clone(), 16.0, theme::TEXT)
                        )],
                    ));
                    if reseeds {
                        small(row, "New seed", 110.0, Press::Reseed);
                    }
                });
                if reseeds {
                    let hint = "Type a seed of your own, in letters and digits.";
                    line(parent, hint, theme::TEXT_DIM);
                }
                // A player who has everything is not told what the run doesn't earn, but
                // everyone is told of a playtest: it sets no record either.
                let told = unlocks.wanting() || lobby.playtest;
                if let Some(why) = unearned(&lobby).filter(|_| told) {
                    line(parent, why, theme::TEXT);
                }
                // The run starts once everyone is ready. A host who is alone plays a solo
                // run, with its locks: what a single player has not unlocked holds them back.
                let own = seat.0 as usize;
                let mine = lobby.ready.get(own).copied().unwrap_or(false);
                let held = held_back(&lobby, &unlocks, &options).filter(|_| host);
                match (held, mine) {
                    (Some(why), _) => line(parent, why, theme::TEXT),
                    (None, false) => primary(parent, "Ready", Press::Ready),
                    (None, true) => button(parent, "Not ready", Press::Ready),
                }
                let (words, ink) = lobby_words(&lobby, own);
                line(parent, words, ink);
                let leave = if host {
                    "Close the game"
                } else {
                    "Leave the game"
                };
                button(parent, leave, Press::Abandon);
            });
            // The host tells the others how to join, next to the lobby.
            if host {
                aside(root, |parent| {
                    ways_to_join(parent, &invitation, *way, &copied);
                });
            }
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

/// What a run counts, with a capital: the sectors of the corridor, or the groups of arena
/// mode.
fn unit_name(in_a_room: bool) -> &'static str {
    if in_a_room { "Group" } else { "Sector" }
}

/// What a group of arena mode does in a playtest, and what its blocks do: as the switches of
/// the menu say them.
fn group_words(whole: bool) -> &'static str {
    if whole { "All at once" } else { "In two waves" }
}

fn block_words(back: bool) -> &'static str {
    if back {
        "Back after 10 s"
    } else {
        "New between groups"
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
                // A run in arena mode skips to the lull before a group.
                let in_a_room = run.as_ref().is_some_and(|run| run.arena);
                let unit = if in_a_room { "group" } else { "sector" };
                row(
                    parent,
                    &format!("{} to skip to", unit_name(in_a_room)),
                    |row| {
                        for change in [-10, -1, 1, 10] {
                            small(row, format!("{change:+}"), 48.0, Press::Destination(change));
                        }
                    },
                );
                let skip = format!("Skip to {unit} {}", destination.0);
                button(parent, skip, Press::Skip);
                // What a playtest tries in arena mode, from the next group on.
                if in_a_room {
                    row(parent, "A group comes", |row| {
                        let state = group_words(playtest.whole_groups);
                        small(row, state, 170.0, Press::WholeGroups);
                    });
                    row(parent, "Blocks and crates", |row| {
                        let state = block_words(playtest.blocks_return);
                        small(row, state, 170.0, Press::BlocksReturn);
                    });
                }
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

/// Where a run ended, as the stamp of the game over screen tells it: in a sector of the
/// corridor, or at a group of arena mode.
fn ending(run: &Run) -> String {
    match (run.arena, run.sector, run.lull) {
        (false, 0, _) => "DESTROYED IN THE START BAY".to_string(),
        (false, sector, _) => format!("DESTROYED IN SECTOR {sector}"),
        // Between two groups, a player has only their own shots and mines to fear.
        (true, cleared, Some(_)) => format!("DESTROYED BEFORE GROUP {}", cleared + 1),
        (true, group, None) => format!("DESTROYED BY GROUP {group}"),
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
    (unlocks, options): (Res<Unlocks>, Res<Options>),
    lobbies: Query<&Lobby>,
) {
    // What holds a host back from the same seed again, and from a new run.
    let held = |fresh: bool| {
        let lobby = lobbies.single().ok();
        lobby.and_then(|lobby| held_back_after(lobby, fresh, &unlocks, &options))
    };
    let (playtest, team) = run
        .as_ref()
        .map_or((false, 1), |run| (run.playtest, run.team));
    let mode = match team {
        _ if playtest => "PLAYTEST".to_string(),
        1 => "SOLO".to_string(),
        team => format!("CO-OP \u{b7} {team} PLAYERS"),
    };
    let league = run.as_ref().map(|run| League::of(run)).unwrap_or_default();
    let mode = match league.arena {
        true => format!("{mode} \u{b7} ARENA MODE"),
        false => mode,
    };
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
                        children![theme::title(ending(run), 24.0, theme::DANGER)],
                    ));
                    let seconds = run.ticks / TICK_RATE as u64;
                    parent
                        .spawn(ruled(Node {
                            align_self: AlignSelf::Stretch,
                            ..default()
                        }))
                        .with_children(|figures| {
                            let scored = if run.arena {
                                "GROUPS CLEARED"
                            } else {
                                "SECTORS CLEARED"
                            };
                            figure(figures, scored, run.cleared.to_string(), false);
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
                    // In arena mode and in free mode, the next run earns nothing either,
                    // retry or not.
                    let why = if league.arena {
                        "Runs in arena mode earn no achievement."
                    } else if league.free {
                        "Runs in free mode earn no achievement."
                    } else {
                        "This run earned no achievement: its seed was typed or played again."
                    };
                    line(parent, why, theme::TEXT_DIM);
                }
                let retry = |buttons: &mut ChildSpawnerCommands| {
                    primary(buttons, "Retry this seed", Press::Retry);
                    if wanting && !league.free && !league.arena {
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
                        // A host whom the others have left would go on alone: a run
                        // that a single player has not unlocked is not offered.
                        Session::Host => {
                            let (again, anew) = (held(false), held(true));
                            if again.is_none() {
                                retry(buttons);
                            }
                            if anew.is_none() {
                                button(buttons, "New run", Press::NewRun);
                            }
                            if let Some(why) = anew.or(again) {
                                line(buttons, why, theme::TEXT_DIM);
                            }
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
    (options, run, time, seat, unlocks): (
        Res<Options>,
        Option<Res<Run>>,
        Res<Time<Real>>,
        Res<LocalSeat>,
        Res<Unlocks>,
    ),
    (session, mut address, mut notice, mut opening, mut asked, mut entry): (
        Res<Session>,
        ResMut<Address>,
        ResMut<Notice>,
        ResMut<Opening>,
        ResMut<Asked>,
        ResMut<Entry>,
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
            // The choice is kept for the next time: the one of a host for their next game,
            // and the one of a single player for their next run.
            Press::Difficulty(difficulty) => match (*session, lobbies.single_mut()) {
                (Session::Host, Ok(mut lobby)) => {
                    lobby.difficulty = difficulty;
                    if settings.host_difficulty != difficulty {
                        settings.host_difficulty = difficulty;
                    }
                }
                (Session::Solo, _) => {
                    setup.difficulty = difficulty;
                    if settings.solo_difficulty != difficulty {
                        settings.solo_difficulty = difficulty;
                    }
                }
                _ => {}
            },
            Press::Free(on) => match (*session, lobbies.single_mut()) {
                (Session::Host, Ok(mut lobby)) => {
                    lobby.free = on;
                    if settings.host_free != on {
                        settings.host_free = on;
                    }
                }
                (Session::Solo, _) => {
                    setup.free = on;
                    if settings.solo_free != on {
                        settings.solo_free = on;
                    }
                }
                _ => {}
            },
            Press::Arena(on) => match (*session, lobbies.single_mut()) {
                (Session::Host, Ok(mut lobby)) => {
                    lobby.arena = on;
                    if settings.host_arena != on {
                        settings.host_arena = on;
                    }
                }
                (Session::Solo, _) => {
                    setup.arena = on;
                    if settings.solo_arena != on {
                        settings.solo_arena = on;
                    }
                }
                _ => {}
            },
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
            Press::CloseGarage => next_screen.set(garage_opened_from(*session)),
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
                    // A host starts the run for everyone, on the seed of the lobby, as
                    // the lobby is set and in the tanks that the players have.
                    Session::Host => {
                        if let Ok(mut lobby) = lobbies.single_mut() {
                            // A host whom the others have left starts no run that a
                            // single player has not unlocked.
                            if held_back_after(&lobby, fresh, &unlocks, &options).is_some() {
                                continue;
                            }
                            lobby.seed = seed;
                            // A seed that the game has just drawn is none that was typed.
                            lobby.typed &= !fresh;
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
            Press::Field(field) => opening.field = field,
            // The password of the screen that is open.
            Press::Reveal => match *screen.get() {
                Screen::Host => opening.password.shown = !opening.password.shown,
                _ => entry.0.shown = !entry.0.shown,
            },
            Press::OpenJoin => {
                notice.0 = None;
                next_screen.set(Screen::Join);
            }
            // A browser joins the game of its link, and a desktop game the one at the
            // address that was typed. A game that asked for its password is joined with
            // the one that was typed.
            Press::Join => {
                let typed = (&*asked, &*entry);
                ask_to_join(&address, typed, &mut settings, &mut session_requests);
            }
            // A game that was found is joined as if its address had been typed. While a
            // game is being joined, the address stays the one of that game. One that has a
            // password asks for it first: nothing is sent before the player has typed it.
            Press::JoinFound(id) => {
                let found = found.0.iter().find(|game| game.id == id);
                if let Some(game) = found.filter(|_| *session == Session::Solo) {
                    notice.0 = None;
                    let typed = (&mut *address, &mut *asked);
                    pick(game, typed, &mut settings, &mut session_requests);
                }
            }
            Press::Forget => {
                notice.0 = None;
                asked.0 = None;
            }
            // The player says that they are ready, or takes it back.
            Press::Ready => {
                if let Ok(lobby) = lobbies.single() {
                    let mine = lobby.ready.get(seat.0 as usize).copied().unwrap_or(false);
                    session_requests.write(SessionRequest::Ready(!mine));
                }
            }
            Press::Reseed => {
                if let Ok(mut lobby) = lobbies.single_mut() {
                    lobby.seed = fresh_seed();
                    lobby.typed = false;
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
            Press::WholeGroups => {
                if let Some(playtest) = &mut playtest {
                    playtest.whole_groups = !playtest.whole_groups;
                }
            }
            Press::BlocksReturn => {
                if let Some(playtest) = &mut playtest {
                    playtest.blocks_return = !playtest.blocks_return;
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
    (session, mut asked): (Res<Session>, ResMut<Asked>),
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
        next_screen.set(garage_opened_from(*session));
        return;
    }
    if *screen.get() == Screen::Join && *overlay.get() == Overlay::None {
        if *session == Session::Guest {
            session_requests.write(SessionRequest::Leave);
        }
        // Esc leaves a game that asks for its password for the list, as "Back" does.
        if *session == Session::Solo && asked.0.is_some() {
            asked.0 = None;
        } else {
            next_screen.set(Screen::Menu);
        }
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
            arena: false,
            lull: None,
        }
    }

    fn league(difficulty: Difficulty, free: bool) -> League {
        League {
            difficulty,
            free,
            arena: false,
        }
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

    /// Sits this many players in the lobby of a game, the host first. The game is set to
    /// what takes the most room to show: hard mode and free mode, on a seed that was typed
    /// and is as long as one gets.
    fn gather(app: &mut App, players: usize) {
        let mut lobby = Lobby {
            name: LONG_GAME.into(),
            locked: true,
            seed: LONG_SEED.into(),
            typed: true,
            difficulty: Difficulty::Hard,
            free: true,
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

    /// Sets the lobby of the tests to arena mode too.
    fn in_arena_mode(app: &mut App) {
        let world = app.world_mut();
        let mut lobby = world.query::<&mut Lobby>().single_mut(world).unwrap();
        lobby.arena = true;
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
            // Every switch of the list is on, and its lines count groups.
            ("the main menu with the records of arena mode", |app| {
                let every = League {
                    arena: true,
                    ..league(Difficulty::Hard, true)
                };
                app.insert_resource(Records::of_every_team(128, LONG_SEED))
                    .insert_resource(Listed(every))
                    .insert_resource(Notice(Some(LONG_NOTICE.into())));
                build(app, main_menu);
            }),
            // A notice on top of this one would cross the fine line around the sheet by a
            // few pixels in the smallest windows. A playtest is a tool of the game's makers.
            ("the main menu of a playtest", |app| {
                app.insert_resource(Records::of_every_team(128, "85GVTHQA"))
                    .insert_resource(Playtest {
                        infinite_lives: true,
                        ..default()
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
                        ..default()
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
                    arena: false,
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
                        arena: false,
                    });
                build(app, solo_screen);
            }),
            // Every mode is unlocked and chosen, on a seed that was typed: the screen then
            // holds every row and every note that it has.
            ("the solo screen in arena mode", |app| {
                let unlocks = [
                    Unlock::ArenaMode,
                    Unlock::ChosenSeed,
                    Unlock::FreeMode,
                    Unlock::HardMode,
                ];
                app.insert_resource(Unlocks::of(&unlocks))
                    .insert_resource(Garage { tank: racer() })
                    .insert_resource(Setup {
                        seed: LONG_SEED.into(),
                        typed: true,
                        difficulty: Difficulty::Hard,
                        free: true,
                        arena: true,
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
                        arena: false,
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
                        arena: false,
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
            // The longest name and the longest password that a host types, which shows, with
            // the longest notice: the port may be taken.
            ("the host screen", |app| {
                assert_eq!(LONG_GAME.chars().count(), NAME_LENGTH);
                assert_eq!(LONG_GAME.chars().count(), PASSWORD_LENGTH);
                app.insert_resource(Opening {
                    name: LONG_GAME.into(),
                    password: Secret {
                        text: LONG_GAME.into(),
                        shown: true,
                    },
                    field: Field::Password,
                })
                .insert_resource(Notice(Some(LONG_NOTICE.into())));
                build(app, host_screen);
            }),
            // As many games as the screen lists, each under a name as long as one gets but
            // the last, which has none.
            ("the join screen", |app| {
                let game = |id: u64, standing: Standing| Game {
                    id,
                    name: if id == 4 { "" } else { LONG_GAME }.into(),
                    locked: true,
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
            // A game under the longest name at the longest address, whose password was typed
            // wrong: the longest password shows.
            (
                "the join screen of a game that asks for its password",
                |app| {
                    app.insert_resource(Asked(Some(Shut {
                        at: format!("{LONG_ADDRESS}:28405"),
                        name: LONG_GAME.into(),
                        address: format!("{LONG_ADDRESS}:28405"),
                    })))
                    .insert_resource(Entry(Secret {
                        text: LONG_GAME.into(),
                        shown: true,
                    }))
                    .insert_resource(Notice(Some(LONG_NOTICE.into())));
                    build(app, join_screen);
                },
            ),
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
            // A host who is alone and has unlocked nothing is told what locks the run.
            ("the lobby of a host who is alone", |app| {
                gather(app, 1);
                app.insert_resource(Session::Host)
                    .insert_resource(invitation());
                build(app, lobby_screen);
            }),
            ("the lobby of a guest", |app| {
                gather(app, 4);
                app.insert_resource(Session::Guest)
                    .insert_resource(crew::LocalSeat(3));
                build(app, lobby_screen);
            }),
            // Arena mode is on too: the lobby then holds every row that it has.
            ("the lobby of a host in arena mode", |app| {
                gather(app, 4);
                in_arena_mode(app);
                app.insert_resource(Session::Host)
                    .insert_resource(invitation());
                build(app, lobby_screen);
            }),
            ("the lobby of a host who is alone in arena mode", |app| {
                gather(app, 1);
                in_arena_mode(app);
                app.insert_resource(Session::Host)
                    .insert_resource(invitation());
                build(app, lobby_screen);
            }),
            ("the lobby of a guest in arena mode", |app| {
                gather(app, 4);
                in_arena_mode(app);
                app.insert_resource(Session::Guest)
                    .insert_resource(crew::LocalSeat(3));
                build(app, lobby_screen);
            }),
            // The game of a playtest says so to its guests, which takes the most words.
            ("the lobby of a guest of a playtest", |app| {
                gather(app, 4);
                let world = app.world_mut();
                let mut lobby = world.query::<&mut Lobby>().single_mut(world).unwrap();
                lobby.playtest = true;
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
            // A run of four players in every mode, which was lost between two groups.
            ("the report of a run in arena mode", |app| {
                let run = Run {
                    seed_text: LONG_SEED.into(),
                    difficulty: Difficulty::Hard,
                    earns: false,
                    free: true,
                    arena: true,
                    lull: Some(3),
                    ..lost_run(4)
                };
                app.insert_resource(run)
                    .insert_resource(Session::Host)
                    .insert_resource(Records::of_every_team(128, LONG_SEED));
                build(app, game_over);
            }),
            ("the report of a run that unlocked four things", |app| {
                let mut unlocks = Unlocks::default();
                unlocks.new = vec![
                    Unlock::ArenaMode,
                    Unlock::ChosenSeed,
                    Unlock::FreeMode,
                    Unlock::HardMode,
                ];
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
                        ..default()
                    });
                build(app, pause_menu);
            }),
            // In arena mode, a playtest has two more switches, and its level a longer line.
            (
                "the menu of a host during a playtest of arena mode",
                |app| {
                    let run = Run {
                        seed_text: LONG_SEED.into(),
                        difficulty: Difficulty::Hard,
                        arena: true,
                        ..lost_run(4)
                    };
                    app.insert_resource(run)
                        .insert_resource(Session::Host)
                        .insert_resource(Playtest {
                            infinite_lives: true,
                            blocks_return: true,
                            ..default()
                        });
                    build(app, pause_menu);
                },
            ),
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
            .init_resource::<Entry>()
            .init_resource::<Settings>()
            .init_resource::<Setup>()
            .init_resource::<Listed>()
            .init_resource::<Garage>()
            .init_resource::<Unlocks>()
            .init_resource::<Way>()
            .init_resource::<Copied>()
            .init_resource::<Capture>()
            .init_resource::<Outcome>()
            .insert_resource(Destination(1))
            .add_message::<RunRequest>();
        // The fonts are there from the second frame on.
        app.update();
        screen(&mut app);
        // What the menus fill in afterward: the games that were found, the best runs, the
        // address that was typed and the notice.
        build(&mut app, list_games);
        build(&mut app, list_records);
        build(&mut app, show_address);
        build(&mut app, show_what_is_typed);
        build(&mut app, show_seed);
        build(&mut app, tell_how_it_goes);
        app.update();
        app.update();
        app
    }

    /// Presses the button of a screen that does this, as a click does, and lets the menus
    /// do what follows. The button is let go afterward, so that the next click is one press.
    fn click(app: &mut App, wanted: Press) {
        let world = app.world_mut();
        let mut buttons = world.query::<(&Press, &mut Interaction)>();
        let pressed = buttons.iter_mut(world).find(|(does, _)| **does == wanted);
        let (_, mut interaction) = pressed.expect("a button that does it");
        *interaction = Interaction::Pressed;
        build(app, press);
        let world = app.world_mut();
        for (_, mut interaction) in buttons.iter_mut(world) {
            *interaction = Interaction::None;
        }
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
            locked: false,
            machine: std::net::Ipv4Addr::new(192, 168, 0, 21),
            address: "192.168.0.21".to_string(),
            players,
            standing,
        };
        assert_eq!(found_words(&game(2, Standing::Open)), "2 of 4 players");
        // A game that has a password is listed as locked.
        let locked = Game {
            locked: true,
            ..game(2, Standing::Open)
        };
        assert_eq!(found_words(&locked), "2 of 4 players \u{b7} locked");
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
    fn the_host_screen_opens_the_game_that_was_typed() {
        let mut world = World::new();
        let mut settings = Settings::default();
        settings.host_name = "Friday".into();
        (settings.host_difficulty, settings.host_free) = (Difficulty::Hard, true);
        world.insert_resource(settings);
        world.insert_resource(State::new(Screen::Host));
        world.insert_resource(Session::Solo);
        // What was typed for another game is gone, its password too.
        world.insert_resource(Opening {
            name: "Another".into(),
            password: Secret {
                text: "old".into(),
                shown: true,
            },
            field: Field::Password,
        });
        world.init_resource::<Messages<KeyboardInput>>();
        world.init_resource::<Messages<SessionRequest>>();
        // The screen opens on the name of the game that was hosted last, without a password.
        world.run_system_once(set_up_to_host).unwrap();
        let opening = world.resource::<Opening>();
        assert_eq!(opening.name, "Friday");
        assert_eq!((opening.password.clone(), opening.field), default());
        // The host types more of the name, goes to the password with Tab, types it, comes
        // back to the name with Tab for a last letter, then presses Enter.
        let mut keys = world.resource_mut::<Messages<KeyboardInput>>();
        let typed = [
            strokes(" night"),
            vec![stroke(Key::Tab, Some("\t"))],
            strokes("sesame "),
            vec![stroke(Key::Backspace, None), stroke(Key::Tab, None)],
            strokes("s "),
            vec![stroke(Key::Enter, None)],
        ];
        for key in typed.into_iter().flatten() {
            keys.write(key);
        }
        world.run_system_once(type_name).unwrap();
        let mut asked = world.resource_mut::<Messages<SessionRequest>>();
        let asked: Vec<SessionRequest> = asked.drain().collect();
        // The game opens under the name without the space at its end, and the name is kept
        // for the next game. The password goes with it as it was typed, and the lobby
        // opens on the run that the host set up for their last game.
        let sign = Sign {
            name: "Friday nights".into(),
            password: "sesame".into(),
            difficulty: Difficulty::Hard,
            free: true,
            arena: false,
        };
        assert_eq!(asked, [SessionRequest::Host(sign)]);
        assert_eq!(world.resource::<Settings>().host_name, "Friday nights");
        // Nothing is typed into a game that is open already.
        world.insert_resource(Session::Host);
        world
            .resource_mut::<Messages<KeyboardInput>>()
            .write(stroke(Key::Enter, None));
        world.run_system_once(type_name).unwrap();
        assert!(world.resource::<Messages<SessionRequest>>().is_empty());
    }

    #[test]
    fn a_password_shows_as_dots_until_the_player_asks_to_see_it() {
        let mut secret = Secret {
            text: "s\u{e9}same".into(),
            shown: false,
        };
        // A dot for each letter, not for each byte.
        assert_eq!(masked(&secret), "\u{2022}".repeat(6));
        secret.shown = true;
        assert_eq!(masked(&secret), "s\u{e9}same");
        assert_eq!(masked(&Secret::default()), "");
    }

    #[test]
    fn a_game_that_asks_for_its_password_is_joined_with_the_one_that_was_typed() {
        let mut world = World::new();
        world.insert_resource(Settings::default());
        world.insert_resource(State::new(Screen::Join));
        world.insert_resource(Session::Solo);
        world.insert_resource(Address("192.168.0.21".into()));
        world.init_resource::<Asked>();
        world.init_resource::<Entry>();
        world.init_resource::<Messages<KeyboardInput>>();
        world.init_resource::<Messages<SessionRequest>>();
        let asked = |world: &mut World, keys: Vec<KeyboardInput>| -> Vec<SessionRequest> {
            let mut typed = world.resource_mut::<Messages<KeyboardInput>>();
            for key in keys {
                typed.write(key);
            }
            world.run_system_once(type_address).unwrap();
            // A system that is run once reads every key that the world still holds.
            world.resource_mut::<Messages<KeyboardInput>>().clear();
            let mut asked = world.resource_mut::<Messages<SessionRequest>>();
            asked.drain().collect()
        };
        // Without a game that asks for one, the keys write the address.
        let enter = || stroke(Key::Enter, None);
        let keys = [strokes(":4000"), vec![enter()]].concat();
        let at = "192.168.0.21:4000".to_string();
        assert_eq!(asked(&mut world, keys), [SessionRequest::Join(at.clone())]);
        assert_eq!(world.resource::<Settings>().address, at);
        // The game there has a password: the keys now write it, and Enter joins with it.
        world.insert_resource(Asked(Some(Shut {
            at: at.clone(),
            name: String::new(),
            address: at.clone(),
        })));
        let keys = [strokes("s\u{e9}same!"), vec![enter()]].concat();
        let locked = SessionRequest::JoinLocked(at.clone(), "s\u{e9}same!".into());
        assert_eq!(asked(&mut world, keys), [locked]);
        // The address is as it was.
        assert_eq!(world.resource::<Address>().0, at);
        // Another game that asks, or the same one again after a wrong password, starts
        // from nothing: the password is typed anew.
        world.run_system_once(forget_the_password).unwrap();
        assert_eq!(world.resource::<Entry>().0.text, "");
        // A player who goes back to the list is asked nothing more.
        world.resource_mut::<Entry>().0.text = "half".into();
        world.run_system_once(forget_the_game).unwrap();
        assert_eq!(world.resource::<Asked>().0, None);
        assert_eq!(world.resource::<Entry>().0, Secret::default());
        assert_eq!(asked(&mut world, vec![enter()]), [SessionRequest::Join(at)]);
    }

    #[test]
    fn a_game_of_the_list_that_has_a_password_asks_for_it_before_anything_is_sent() {
        fn click(
            In(game): In<Game>,
            (mut address, mut asked): (ResMut<Address>, ResMut<Asked>),
            mut settings: ResMut<Settings>,
            mut requests: MessageWriter<SessionRequest>,
        ) {
            let typed = (&mut *address, &mut *asked);
            pick(&game, typed, &mut settings, &mut requests);
        }
        let mut world = World::new();
        world.insert_resource(Settings::default());
        world.insert_resource(Address("10.0.0.7".into()));
        world.init_resource::<Asked>();
        world.init_resource::<Messages<SessionRequest>>();
        let mut clicked = |game: &Game| -> (Vec<SessionRequest>, Option<Shut>) {
            world.run_system_once_with(click, game.clone()).unwrap();
            let mut requests = world.resource_mut::<Messages<SessionRequest>>();
            let requests: Vec<SessionRequest> = requests.drain().collect();
            assert_eq!(world.resource::<Address>().0, game.address);
            assert_eq!(world.resource::<Settings>().address, game.address);
            (requests, world.resource::<Asked>().0.clone())
        };
        let open = Game {
            id: 1,
            name: "Friday".into(),
            locked: false,
            machine: std::net::Ipv4Addr::new(192, 168, 0, 21),
            address: "192.168.0.21".into(),
            players: 1,
            standing: Standing::Open,
        };
        // A game without a password is joined at once.
        let join = SessionRequest::Join("192.168.0.21".into());
        assert_eq!(clicked(&open), (vec![join], None));
        // One that has a password is not: the player is asked for it, by the name and the
        // address that the list gave.
        let locked = Game {
            locked: true,
            address: "192.168.0.22:4000".into(),
            ..open
        };
        let shut = Shut {
            at: "192.168.0.22:4000".into(),
            name: "Friday".into(),
            address: "192.168.0.22:4000".into(),
        };
        assert_eq!(clicked(&locked), (Vec::new(), Some(shut)));
    }

    #[test]
    fn a_game_that_asks_for_its_password_is_named_as_the_player_found_it() {
        let shut = |name: &str| Shut {
            at: "192.168.0.21".into(),
            name: name.into(),
            address: "192.168.0.21".into(),
        };
        assert_eq!(shut_words(&shut("")), "The game at 192.168.0.21");
        assert_eq!(shut_words(&shut("Friday")), "Friday, at 192.168.0.21");
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
        // The seed of a run in arena mode is that of a room.
        let room = Run {
            arena: true,
            ..hard
        };
        assert_eq!(
            level_line(&room),
            format!("Seed 85GVTHQA, v{VERSION}, hard mode, arena mode")
        );
    }

    #[test]
    fn a_run_in_arena_mode_has_records_of_its_own_which_count_groups() {
        let normal = League::default();
        let arena = League {
            arena: true,
            ..normal
        };
        let run = |arena: bool, team: u32, cleared: u32| Run {
            arena,
            earns: !arena,
            cleared,
            ..lost_run(team)
        };
        let mut records = Records::default();
        assert!(keep_record(&mut records, &run(true, 1, 9)));
        // It is no record of the corridor, however far it got, and the other way around.
        assert!(records.best(normal, 1).is_none());
        assert!(keep_record(&mut records, &run(false, 1, 4)));
        assert!(!keep_record(&mut records, &run(true, 1, 9)));
        assert!(keep_record(&mut records, &run(true, 3, 2)));
        assert_eq!(records.best(arena, 1).unwrap().sector, 9);
        assert_eq!(records.best(normal, 1).unwrap().sector, 4);
        assert_eq!(
            record_line(&records, arena, 1),
            format!("Best run in arena mode: group 9, seed 85GVTHQA, v{VERSION}")
        );
        assert_eq!(
            record_line(&records, normal, 1),
            format!("Best run: sector 4, seed 85GVTHQA, v{VERSION}")
        );
        assert_eq!(record_line(&records, arena, 2), "No run in arena mode yet");
        let listed = records_of(&records, arena);
        assert_eq!(listed.len(), 2);
        assert!(listed[0].starts_with("Best run: group 9"));
        assert!(listed[1].starts_with("3 players: group 2"));
    }

    #[test]
    fn the_report_of_a_run_in_arena_mode_counts_groups() {
        let room = |sector: u32, lull: Option<u8>| Run {
            arena: true,
            earns: false,
            sector,
            cleared: sector.saturating_sub(lull.is_none() as u32),
            lull,
            ..lost_run(1)
        };
        // The stamp says which group ended the run, or which one was about to come.
        assert_eq!(ending(&room(7, None)), "DESTROYED BY GROUP 7");
        assert_eq!(ending(&room(7, Some(2))), "DESTROYED BEFORE GROUP 8");
        assert_eq!(ending(&room(0, Some(5))), "DESTROYED BEFORE GROUP 1");
        assert_eq!(ending(&lost_run(1)), "DESTROYED IN SECTOR 128");
        let bay = Run {
            sector: 0,
            ..lost_run(1)
        };
        assert_eq!(ending(&bay), "DESTROYED IN THE START BAY");

        const ARENA: &str = "Runs in arena mode earn no achievement.";
        const RETRY: &str = "A retry earns no achievement.";
        let report = texts(|app| {
            app.insert_resource(Run {
                arena: true,
                earns: false,
                sector: 7,
                cleared: 6,
                ..lost_run(1)
            });
            build(app, game_over);
        });
        assert!(says(&report, "RUN REPORT \u{b7} SOLO \u{b7} ARENA MODE"));
        assert!(says(&report, "DESTROYED BY GROUP 7"));
        assert!(says(&report, "GROUPS CLEARED") && !says(&report, "SECTORS CLEARED"));
        assert!(says(&report, "No run in arena mode yet"));
        // A new run in arena mode earns nothing either: the retry is not singled out.
        assert!(says(&report, ARENA) && !says(&report, RETRY));
        let corridor = texts(|app| {
            app.insert_resource(lost_run(1));
            build(app, game_over);
        });
        assert!(says(&corridor, "SECTORS CLEARED") && !says(&corridor, "GROUPS CLEARED"));
    }

    #[test]
    fn the_solo_screen_offers_arena_mode_once_it_is_unlocked() {
        let offers = |screen: fn(&mut App)| -> Vec<bool> {
            let arena = presses(screen).into_iter().filter_map(|press| match press {
                Press::Arena(on) => Some(on),
                _ => None,
            });
            arena.collect()
        };
        // Locked: nothing to choose, and the screen says what unlocks it.
        let locked = |app: &mut App| build(app, solo_screen);
        assert_eq!(offers(locked), [] as [bool; 0]);
        assert!(says(&texts(locked), "Clear sector 5"));
        // Unlocked and off: it can be turned on.
        let off = |app: &mut App| {
            app.insert_resource(Unlocks::of(&[Unlock::ArenaMode]));
            build(app, solo_screen);
        };
        assert_eq!(offers(off), [true]);
        assert!(!says(&texts(off), ARENA_MODE));
        // On: the screen says what it is, and that its runs earn nothing.
        let on = |app: &mut App| {
            app.insert_resource(Unlocks::of(&[Unlock::ArenaMode]))
                .insert_resource(Setup {
                    seed: "85GVTHQA".into(),
                    arena: true,
                    ..default()
                });
            build(app, solo_screen);
        };
        assert_eq!(offers(on), [false]);
        let told = texts(on);
        assert!(says(&told, ARENA_MODE));
        assert!(says(&told, "A run in arena mode earns no achievement."));
        // A playtest has it without unlocking it.
        let playtest = |app: &mut App| {
            app.insert_resource(Options {
                playtest: true,
                ..default()
            });
            build(app, solo_screen);
        };
        assert_eq!(offers(playtest), [true]);

        // The choice is kept for the next run, and the run that starts is in arena mode.
        let mut app = stage(off, WINDOWS[0]);
        click(&mut app, Press::Arena(true));
        assert!(app.world().resource::<Setup>().arena);
        assert!(app.world().resource::<Settings>().solo_arena);
        app.world_mut().resource_mut::<Setup>().seed = "85GVTHQA".into();
        build(&mut app, solo_screen);
        click(&mut app, Press::Begin);
        let mut requests = app.world_mut().resource_mut::<Messages<RunRequest>>();
        let started: Vec<bool> = requests
            .drain()
            .map(|request| match request {
                RunRequest::Start { arena, .. } => arena,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(started, [true]);
        // The next visit opens on arena mode only while it is still unlocked.
        let mut settings = Settings::default();
        settings.solo_arena = true;
        let options = Options::default();
        assert!(last_arena(
            &settings,
            &Unlocks::of(&[Unlock::ArenaMode]),
            &options
        ));
        assert!(!last_arena(&settings, &Unlocks::default(), &options));
        assert!(!last_arena(
            &Settings::default(),
            &Unlocks::of(&Unlock::ALL),
            &options
        ));
    }

    #[test]
    fn the_host_sets_arena_mode_in_the_lobby_and_the_guests_see_it() {
        let mut host = in_the_lobby(Session::Host, 2);
        assert!(buttons(&mut host).contains(&Press::Arena(true)));
        assert!(!buttons(&mut host).contains(&Press::Arena(false)));
        click(&mut host, Press::Arena(true));
        assert!(lobby_of(&mut host).arena);
        assert!(host.world().resource::<Settings>().host_arena);
        // What a single player chose last for a run of their own is as it was.
        assert!(!host.world().resource::<Settings>().solo_arena);
        build(&mut host, lobby_screen);
        assert!(buttons(&mut host).contains(&Press::Arena(false)));
        // A guest has nothing to press about it.
        let mut guest = in_the_lobby(Session::Guest, 2);
        guest
            .world_mut()
            .spawn((Button, Press::Arena(true), Interaction::None));
        click(&mut guest, Press::Arena(true));
        assert!(!lobby_of(&mut guest).arena);

        // A run in arena mode earns nothing, and the lobby says so before anything else
        // but a playtest.
        let lobby = Lobby {
            arena: true,
            free: true,
            typed: true,
            ..default()
        };
        assert_eq!(
            unearned(&lobby),
            Some("A run in arena mode earns no achievement.")
        );
        // A host who is alone plays under the locks of solo: arena mode is one of them.
        let alone = Lobby {
            seats: [Some(Member::Host), None, None, None],
            seed: "85GVTHQA".into(),
            arena: true,
            ..default()
        };
        let options = Options::default();
        assert_eq!(
            held_back(&alone, &Unlocks::default(), &options),
            Some("Locked for a single player: arena mode (clear sector 5).".to_string())
        );
        assert_eq!(
            held_back(&alone, &Unlocks::of(&[Unlock::ArenaMode]), &options),
            None
        );
        let together = Lobby {
            seats: [Some(Member::Host), Some(Member::Guest(7)), None, None],
            ..alone
        };
        assert_eq!(held_back(&together, &Unlocks::default(), &options), None);
    }

    #[test]
    fn a_playtest_of_arena_mode_has_two_switches_and_skips_to_a_group() {
        let menu = |arena: bool| {
            move |app: &mut App| {
                app.insert_resource(Run {
                    arena,
                    sector: 7,
                    phase: RunPhase::Playing,
                    ..lost_run(1)
                })
                .insert_resource(Playtest::default())
                .insert_resource(Destination(8));
                build(app, pause_menu);
            }
        };
        let said = |app: &mut App, wanted: &str| {
            let world = app.world_mut();
            let mut texts = world.query::<&Text>();
            texts
                .iter(world)
                .any(|text| text.0.to_lowercase() == wanted.to_lowercase())
        };
        // In the corridor, a playtest skips to a sector and has nothing to say of groups.
        let mut corridor = stage(|_| {}, WINDOWS[0]);
        menu(false)(&mut corridor);
        let offered = buttons(&mut corridor);
        assert!(!offered.contains(&Press::WholeGroups) && !offered.contains(&Press::BlocksReturn));
        assert!(said(&mut corridor, "Skip to sector 8"));

        let mut room = stage(|_| {}, WINDOWS[0]);
        menu(true)(&mut room);
        let offered = buttons(&mut room);
        assert!(offered.contains(&Press::WholeGroups) && offered.contains(&Press::BlocksReturn));
        assert!(said(&mut room, "Skip to group 8"));
        assert!(said(&mut room, "In two waves") && said(&mut room, "New between groups"));
        // Each switch turns its rule over, and the menu says what holds now.
        click(&mut room, Press::WholeGroups);
        let playtest = *room.world().resource::<Playtest>();
        assert!(playtest.whole_groups && !playtest.blocks_return);
        click(&mut room, Press::BlocksReturn);
        let playtest = *room.world().resource::<Playtest>();
        assert!(playtest.whole_groups && playtest.blocks_return);
        build(&mut room, refresh_pause);
        assert!(said(&mut room, "All at once") && said(&mut room, "Back after 10 s"));
        click(&mut room, Press::WholeGroups);
        assert!(!room.world().resource::<Playtest>().whole_groups);
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
                    ..
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
            arena: false,
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
        listed.sort_by_key(|league| (league.difficulty, league.arena, league.free));
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
        // Hard mode, arena mode and free mode each have a switch once there is a run of
        // theirs.
        let arena = League {
            arena: true,
            ..normal
        };
        let every = |app: &mut App| {
            app.insert_resource(Records::of_every_team(128, "85GVTHQA"));
            build(app, main_menu);
        };
        assert_eq!(listed(every), [free, arena, hard]);
        let arena_alone = |app: &mut App| {
            let mut records = Records::default();
            let arena = League {
                arena: true,
                ..default()
            };
            records.submit(arena, 1, 9, "85GVTHQA");
            app.insert_resource(records);
            build(app, main_menu);
        };
        assert_eq!(listed(arena_alone), [arena]);
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
        let shown = |screen: Screen, setup: &Setup, shared: Option<bool>, options: &Options| {
            shown_tank(screen, (setup, shared), options, &garage)
        };
        for screen in alone {
            assert_eq!(shown(screen, &plain, None, &none), Genome::PLAYER);
            assert_eq!(shown(screen, &free, None, &none), racer());
        }
        // The garage draws what is being built, alone or in a game of several players.
        assert_eq!(shown(Screen::Garage, &plain, None, &none), racer());
        assert_eq!(shown(Screen::Garage, &plain, Some(false), &none), racer());
        // A game of several players is in the standard tank, whatever a player set up for
        // a run of their own, unless its host set it to free mode.
        for screen in [Screen::Join, Screen::Lobby] {
            assert_eq!(shown(screen, &free, None, &none), Genome::PLAYER);
            assert_eq!(shown(screen, &free, Some(false), &none), Genome::PLAYER);
        }
        assert_eq!(shown(Screen::Lobby, &plain, Some(true), &none), racer());
        // The tank that a playtest brought is drawn everywhere but in the garage.
        let parts = [(Stat::Engine, 2), (Stat::Piercing, 1)];
        let brought = Options {
            playtest: true,
            tank: Some(Genome::of_the_garage(&parts).unwrap()),
            ..default()
        };
        for screen in alone.into_iter().chain([Screen::Join, Screen::Lobby]) {
            for shared in [None, Some(false), Some(true)] {
                assert_eq!(
                    shown(screen, &plain, shared, &brought),
                    brought.tank.unwrap()
                );
            }
        }
    }

    /// The lobby of a game that the test set up, as its host sees it or as a guest does.
    fn in_the_lobby(session: Session, players: usize) -> App {
        let mut app = stage(|_| {}, WINDOWS[0]);
        gather(&mut app, players);
        app.insert_resource(session)
            .insert_resource(crew::LocalSeat(if session == Session::Host {
                0
            } else {
                1
            }));
        build(&mut app, lobby_screen);
        app
    }

    /// The lobby that a game holds.
    fn lobby_of(app: &mut App) -> Lobby {
        let world = app.world_mut();
        world.query::<&Lobby>().single(world).unwrap().clone()
    }

    fn buttons(app: &mut App) -> Vec<Press> {
        let world = app.world_mut();
        let mut buttons = world.query_filtered::<&Press, With<Button>>();
        buttons.iter(world).copied().collect()
    }

    #[test]
    fn the_host_sets_the_run_of_a_lobby_and_the_guests_see_it() {
        // The lobby of the tests is in hard mode and in free mode, on a seed that was typed.
        let mut host = in_the_lobby(Session::Host, 2);
        let offered = buttons(&mut host);
        for press in [
            Press::Difficulty(Difficulty::Normal),
            Press::Free(false),
            Press::OpenGarage,
            Press::Reseed,
            Press::Ready,
        ] {
            assert!(offered.contains(&press));
        }
        // What is chosen stands in the accent, and is no button.
        assert!(!offered.contains(&Press::Difficulty(Difficulty::Hard)));
        assert!(!offered.contains(&Press::Free(true)));

        // A guest sees what the run is set to and can change none of it. In free mode,
        // their garage is theirs to open.
        let mut guest = in_the_lobby(Session::Guest, 2);
        let offered = buttons(&mut guest);
        assert_eq!(offered.len(), 3);
        for press in [Press::OpenGarage, Press::Ready, Press::Abandon] {
            assert!(offered.contains(&press));
        }
        let world = guest.world_mut();
        let told: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        for wanted in ["Hard", "On", LONG_SEED, "59 of 59 points"] {
            assert!(says(&told, wanted), "{wanted}");
        }
        // Nor does a press that reaches a guest's game by another way.
        for press in [Press::Difficulty(Difficulty::Normal), Press::Free(false)] {
            guest.world_mut().spawn((Button, press, Interaction::None));
            click(&mut guest, press);
        }
        let lobby = lobby_of(&mut guest);
        assert_eq!((lobby.difficulty, lobby.free), (Difficulty::Hard, true));

        // The host changes the difficulty and the mode, and both are kept for the next game.
        // Until then, the settings hold what the host chose for their last game, and what
        // they chose for their last run alone.
        let mut settings = host.world_mut().resource_mut::<Settings>();
        (settings.host_difficulty, settings.host_free) = (Difficulty::Hard, true);
        (settings.solo_difficulty, settings.solo_free) = (Difficulty::Hard, true);
        host.insert_resource(Setup {
            difficulty: Difficulty::Hard,
            free: true,
            ..default()
        });
        click(&mut host, Press::Difficulty(Difficulty::Normal));
        click(&mut host, Press::Free(false));
        let lobby = lobby_of(&mut host);
        assert_eq!((lobby.difficulty, lobby.free), (Difficulty::Normal, false));
        let settings = host.world().resource::<Settings>();
        assert_eq!(
            (settings.host_difficulty, settings.host_free),
            (Difficulty::Normal, false)
        );
        // What a single player chose last for a run of their own is as it was.
        assert_eq!(
            (settings.solo_difficulty, settings.solo_free),
            (Difficulty::Hard, true)
        );
        let setup = host.world().resource::<Setup>();
        assert_eq!((setup.difficulty, setup.free), (Difficulty::Hard, true));
        // A new seed is one that the game drew.
        click(&mut host, Press::Reseed);
        let lobby = lobby_of(&mut host);
        assert!(lobby.seed != LONG_SEED && !lobby.seed.is_empty() && !lobby.typed);
        assert!(host.world().resource::<Drawn>().0);
        // Outside free mode, nobody has a garage to open.
        build(&mut host, lobby_screen);
        assert!(!buttons(&mut host).contains(&Press::OpenGarage));
    }

    #[test]
    fn a_key_that_writes_nothing_leaves_the_seed_as_it_is() {
        let letter = |letter: &str| (Key::Character(letter.into()), Some(letter.to_string()));
        let made = |seed: &str, typed: bool, (key, text): (Key, Option<String>)| {
            retyped(seed, typed, &key, text.as_deref())
        };
        // The first letter takes the place of a seed that the game drew, and the next ones
        // follow it.
        assert_eq!(made("85GVTHQA", false, letter("f")), Some("F".to_string()));
        assert_eq!(made("F", true, letter("r")), Some("FR".to_string()));
        // Backspace takes a letter away, and all of a seed that the game drew.
        assert_eq!(
            made("FR", true, (Key::Backspace, None)),
            Some("F".to_string())
        );
        assert_eq!(
            made("85GVTHQA", false, (Key::Backspace, None)),
            Some(String::new())
        );
        // Keys that write nothing, or nothing that a seed is made of, change no seed:
        // neither one that the game drew nor one that was typed.
        let idle = [
            (Key::Shift, None),
            (Key::ArrowLeft, None),
            (Key::Escape, None),
            (Key::Tab, Some("\t".to_string())),
            (Key::Enter, None),
            letter(" "),
            letter("!"),
        ];
        for key in idle {
            assert_eq!(made("85GVTHQA", false, key.clone()), None, "{key:?}");
            assert_eq!(made("FR", true, key.clone()), None, "{key:?}");
        }
        // A seed that is as long as it gets takes no further letter.
        assert_eq!(made(LONG_SEED, true, letter("w")), None);
        // Nor does one that has nothing left to lose.
        assert_eq!(made("", true, (Key::Backspace, None)), None);
    }

    #[test]
    fn the_host_of_a_lobby_types_the_seed_of_the_next_run() {
        let typed = |session: Session, options: Options, keys: &str| -> (Lobby, bool) {
            let mut world = World::new();
            world.insert_resource(State::new(Screen::Lobby));
            world.insert_resource(session);
            world.insert_resource(options);
            world.insert_resource(Drawn(true));
            world.init_resource::<Unlocks>();
            world.init_resource::<Garage>();
            world.init_resource::<Setup>();
            world.init_resource::<NextState<Screen>>();
            world.init_resource::<Messages<KeyboardInput>>();
            world.init_resource::<Messages<RunRequest>>();
            world.init_resource::<Messages<SessionRequest>>();
            world.insert_resource(crew::LocalSeat(1));
            world.spawn(Lobby {
                seed: "85GVTHQA".into(),
                ready: [true, false, false, false],
                ..default()
            });
            let mut pressed = world.resource_mut::<Messages<KeyboardInput>>();
            for key in strokes(keys) {
                pressed.write(key);
            }
            pressed.write(stroke(Key::Enter, None));
            world.run_system_once(type_seed).unwrap();
            // Enter starts nothing in a lobby: it says that the player at this screen, who
            // has the second seat here, is ready. A single player's run is not set up there.
            assert!(world.resource::<Messages<RunRequest>>().is_empty());
            let mut asked = world.resource_mut::<Messages<SessionRequest>>();
            let asked: Vec<SessionRequest> = asked.drain().collect();
            assert_eq!(asked, [SessionRequest::Ready(true)]);
            assert_eq!(world.resource::<Setup>().seed, "");
            let lobby = world.query::<&Lobby>().single(&world).unwrap().clone();
            (lobby, world.resource::<Drawn>().0)
        };
        let seed = |(lobby, drawn): (Lobby, bool)| (lobby.seed, lobby.typed, drawn);
        // The first key takes the place of the seed that the game drew. A seed that the
        // host typed is a chosen one, which nothing locks for a host, and none that the
        // game drew.
        assert_eq!(
            seed(typed(Session::Host, default(), "friday 13!")),
            ("FRIDAY13".to_string(), true, false)
        );
        // Nothing was typed: the seed is the one that the game drew.
        assert_eq!(
            seed(typed(Session::Host, default(), "")),
            ("85GVTHQA".to_string(), false, true)
        );
        // A guest types nothing into the lobby of their host.
        assert_eq!(
            seed(typed(Session::Guest, default(), "mine")),
            ("85GVTHQA".to_string(), false, true)
        );
        // A playtest that was given a seed keeps it.
        let given = Options {
            playtest: true,
            seed: Some("85GVTHQA".into()),
            tank: None,
        };
        assert_eq!(
            seed(typed(Session::Host, given, "mine")),
            ("85GVTHQA".to_string(), false, true)
        );
    }

    #[test]
    fn a_host_who_is_alone_plays_under_the_locks_of_solo() {
        let none = Options::default();
        let lobby = |players: usize, set: fn(&mut Lobby)| {
            let mut lobby = Lobby {
                seed: "85GVTHQA".into(),
                ..default()
            };
            for seat in lobby.seats.iter_mut().take(players) {
                *seat = Some(Member::Host);
            }
            set(&mut lobby);
            lobby
        };
        let nothing = Unlocks::default();
        // A run in normal mode on a seed that the game drew is everyone's.
        assert_eq!(held_back(&lobby(1, |_| {}), &nothing, &none), None);
        // What a single player has not unlocked holds a host back while they are alone.
        let hard = |lobby: &mut Lobby| lobby.difficulty = Difficulty::Hard;
        let typed = |lobby: &mut Lobby| lobby.typed = true;
        let free = |lobby: &mut Lobby| lobby.free = true;
        let locks = [
            (
                hard as fn(&mut Lobby),
                Unlock::HardMode,
                "hard mode (clear sector 30)",
            ),
            (typed, Unlock::ChosenSeed, "chosen seed (clear sector 10)"),
            (free, Unlock::FreeMode, "free mode (clear sector 20)"),
        ];
        for (set, unlock, words) in locks {
            let told = held_back(&lobby(1, set), &nothing, &none);
            assert_eq!(told, Some(format!("Locked for a single player: {words}.")));
            // Not once they have unlocked it, nor in a playtest, which has everything.
            let earned = Unlocks::of(&[unlock]);
            assert_eq!(held_back(&lobby(1, set), &earned, &none), None);
            let playtest = Options {
                playtest: true,
                ..default()
            };
            assert_eq!(held_back(&lobby(1, set), &nothing, &playtest), None);
            // And nothing is locked for a host who has company.
            assert_eq!(held_back(&lobby(2, set), &nothing, &none), None);
        }
        // Hard mode is also theirs who unlocked the difficulty above it.
        let above = Unlocks::of(&[Unlock::ImpossibleMode]);
        assert_eq!(held_back(&lobby(1, hard), &above, &none), None);
        // No run starts without a seed, alone or not.
        let seedless = |lobby: &mut Lobby| lobby.seed.clear();
        for players in [1, 4] {
            let told = held_back(&lobby(players, seedless), &nothing, &none);
            assert!(told.is_some_and(|told| told.starts_with("A run needs a seed")));
        }

        // The lobby of the tests is in hard mode and in free mode on a typed seed: a host
        // who is alone there and has unlocked nothing is not offered to be ready, and is
        // told why.
        let mut alone = in_the_lobby(Session::Host, 1);
        assert!(!buttons(&mut alone).contains(&Press::Ready));
        let world = alone.world_mut();
        let mut texts = world.query::<&Text>();
        let told: Vec<String> = texts.iter(world).map(|text| text.0.clone()).collect();
        assert!(says(
            &told,
            "Locked for a single player: hard mode (clear sector 30)."
        ));
        // With a guest, they are.
        let mut together = in_the_lobby(Session::Host, 2);
        assert!(buttons(&mut together).contains(&Press::Ready));
    }

    /// What a game asked about its game of several players since the last time.
    fn session_requests(app: &mut App) -> Vec<SessionRequest> {
        let mut asked = app.world_mut().resource_mut::<Messages<SessionRequest>>();
        asked.drain().collect()
    }

    #[test]
    fn a_host_whom_the_others_left_starts_no_run_that_solo_locks() {
        // The report of a run that a host played in hard mode, on a seed that they typed.
        let report = |players: usize, unlocks: &[Unlock]| {
            let mut app = stage(|_| {}, WINDOWS[0]);
            gather(&mut app, players);
            let world = app.world_mut();
            let mut lobby = world.query::<&mut Lobby>().single_mut(world).unwrap();
            lobby.free = false;
            app.insert_resource(Session::Host)
                .insert_resource(Unlocks::of(unlocks))
                .insert_resource(lost_run(4));
            build(&mut app, game_over);
            app
        };
        let offered = |app: &mut App| {
            let offered = buttons(app);
            let has = |press: Press| offered.contains(&press);
            (has(Press::Retry), has(Press::NewRun), has(Press::Regroup))
        };
        // With company, the host picks the same seed again or a new run.
        let mut together = report(2, &[]);
        assert_eq!(offered(&mut together), (true, true, true));
        // Alone, with nothing unlocked, neither: hard mode is locked for a single player.
        let mut alone = report(1, &[]);
        assert_eq!(offered(&mut alone), (false, false, true));
        let world = alone.world_mut();
        let mut texts = world.query::<&Text>();
        let told: Vec<String> = texts.iter(world).map(|text| text.0.clone()).collect();
        assert!(says(
            &told,
            "Locked for a single player: hard mode (clear sector 30)."
        ));
        // A press that reaches the game all the same starts nothing.
        for press in [Press::Retry, Press::NewRun] {
            alone.world_mut().spawn((Button, press, Interaction::None));
            click(&mut alone, press);
        }
        assert!(session_requests(&mut alone).is_empty());
        // With hard mode unlocked, a new run is theirs, on a seed that the game draws. The
        // same seed again is not: it was typed, which solo locks too.
        let mut hard = report(1, &[Unlock::HardMode]);
        assert_eq!(offered(&mut hard), (false, true, true));
        click(&mut hard, Press::NewRun);
        assert_eq!(session_requests(&mut hard), [SessionRequest::Start]);
        let lobby = lobby_of(&mut hard);
        assert!(!lobby.typed && lobby.seed != LONG_SEED);
        // With both unlocked, so is the same seed again.
        let mut both = report(1, &[Unlock::HardMode, Unlock::ChosenSeed]);
        assert_eq!(offered(&mut both), (true, true, true));
        click(&mut both, Press::Retry);
        assert_eq!(session_requests(&mut both), [SessionRequest::Start]);
    }

    #[test]
    fn a_player_says_that_they_are_ready_and_takes_it_back() {
        // The button says the word of the player at this screen, host or guest, and takes
        // it back once it is said.
        for (session, own) in [(Session::Host, 0), (Session::Guest, 1)] {
            let mut app = in_the_lobby(session, 2);
            click(&mut app, Press::Ready);
            assert_eq!(session_requests(&mut app), [SessionRequest::Ready(true)]);
            let world = app.world_mut();
            let mut lobby = world.query::<&mut Lobby>().single_mut(world).unwrap();
            lobby.ready[own] = true;
            // The other player being ready changes nothing to it.
            lobby.ready[1 - own] = false;
            build(&mut app, lobby_screen);
            click(&mut app, Press::Ready);
            assert_eq!(session_requests(&mut app), [SessionRequest::Ready(false)]);
        }
    }

    #[test]
    fn the_lobby_says_who_is_ready_and_how_long_until_the_run() {
        let (host, guest) = (Some(Member::Host), Some(Member::Guest(7)));
        let lobby = |ready: [bool; 4], countdown: Option<u8>| Lobby {
            seats: [host, guest, guest, None],
            ready,
            countdown,
            ..default()
        };
        let words = |lobby: &Lobby, own: usize| lobby_words(lobby, own).0;
        // Whoever is not ready is told what starts the run.
        let nobody = lobby([false; 4], None);
        assert_eq!(words(&nobody, 0), "The run starts once everyone is ready");
        // Whoever is ready is told how many players the run waits for.
        let one = lobby([true, false, false, false], None);
        assert_eq!(words(&one, 0), "Waiting for 2 players");
        assert_eq!(words(&one, 1), "The run starts once everyone is ready");
        let two = lobby([true, true, false, false], None);
        assert_eq!(words(&two, 1), "Waiting for 1 player");
        // An open seat is waited for by nobody, whatever was left on it.
        let stale = lobby([true, true, false, true], None);
        assert_eq!(words(&stale, 0), "Waiting for 1 player");
        // Once everyone is ready, the lobby counts down, in the accent.
        let all = lobby([true, true, true, false], Some(3));
        assert_eq!(
            lobby_words(&all, 2),
            ("The run starts in 3".to_string(), theme::ACCENT)
        );

        // Each seat that is taken says whether its player is ready.
        let mut app = stage(|_| {}, WINDOWS[0]);
        app.world_mut()
            .spawn(lobby([true, false, true, false], None));
        app.insert_resource(Session::Guest)
            .insert_resource(crew::LocalSeat(1));
        build(&mut app, lobby_screen);
        let world = app.world_mut();
        let mut texts = world.query::<&Text>();
        let told: Vec<String> = texts.iter(world).map(|text| text.0.clone()).collect();
        let count = |word: &str| told.iter().filter(|text| text.as_str() == word).count();
        // The words of a label are in capitals. The third word "READY" is the button.
        assert_eq!((count("READY"), count("NOT READY")), (3, 1));
        assert!(says(&told, "The run starts once everyone is ready"));
    }

    #[test]
    fn the_lobby_says_when_its_run_will_earn_nothing() {
        let lobby = |set: fn(&mut Lobby)| {
            let mut lobby = Lobby::default();
            set(&mut lobby);
            lobby
        };
        assert_eq!(unearned(&lobby(|_| {})), None);
        let free = unearned(&lobby(|lobby| lobby.free = true)).unwrap();
        assert!(free.contains("free mode"));
        let typed = unearned(&lobby(|lobby| lobby.typed = true)).unwrap();
        assert!(typed.contains("typed seed"));
        // A playtest earns nothing whatever it is set to, and sets no record either.
        let playtest = unearned(&lobby(|lobby| {
            lobby.playtest = true;
            lobby.free = true;
        }));
        assert!(playtest.unwrap().contains("playtest"));

        // The guests are told too: the lobby of the tests is in free mode.
        let told = |app: &mut App| -> Vec<String> {
            let world = app.world_mut();
            let mut texts = world.query::<&Text>();
            texts.iter(world).map(|text| text.0.clone()).collect()
        };
        let mut guest = in_the_lobby(Session::Guest, 2);
        assert!(says(&told(&mut guest), free));
        // A player who has everything is not told what a run doesn't earn.
        let mut guest = stage(|_| {}, WINDOWS[0]);
        gather(&mut guest, 2);
        guest
            .insert_resource(Session::Guest)
            .insert_resource(Unlocks::of(&Unlock::ALL));
        build(&mut guest, lobby_screen);
        assert!(!says(&told(&mut guest), free));
    }

    #[test]
    fn a_player_who_leaves_the_garage_is_back_where_they_opened_it() {
        assert_eq!(garage_opened_from(Session::Solo), Screen::Solo);
        assert_eq!(garage_opened_from(Session::Host), Screen::Lobby);
        assert_eq!(garage_opened_from(Session::Guest), Screen::Lobby);
    }

    #[test]
    fn a_seed_is_drawn_when_the_players_gather_and_not_after_a_visit_to_the_garage() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<Screen>()
            .init_resource::<Time<Real>>()
            .init_resource::<Options>()
            .init_resource::<Drawn>()
            .insert_resource(Session::Host)
            .add_systems(OnEnter(Screen::Lobby), draw_a_seed.run_if(gathering));
        app.world_mut().spawn(Lobby::default());
        let go = |app: &mut App, to: Screen| {
            app.world_mut().resource_mut::<NextState<Screen>>().set(to);
            app.update();
        };
        // A game that opens has a seed that the game drew.
        go(&mut app, Screen::Lobby);
        let first = lobby_of(&mut app);
        assert!(!first.seed.is_empty() && !first.typed);
        assert!(app.world().resource::<Drawn>().0);
        // The host types one, and visits their garage: the lobby is as they left it.
        let world = app.world_mut();
        let mut lobby = world.query::<&mut Lobby>().single_mut(world).unwrap();
        (lobby.seed, lobby.typed) = ("MINE".into(), true);
        go(&mut app, Screen::Garage);
        go(&mut app, Screen::Lobby);
        let kept = lobby_of(&mut app);
        assert_eq!((kept.seed.as_str(), kept.typed), ("MINE", true));
        // After a run, the players gather for another, on a new seed.
        go(&mut app, Screen::Playing);
        go(&mut app, Screen::Lobby);
        let next = lobby_of(&mut app);
        assert!(next.seed != "MINE" && !next.seed.is_empty() && !next.typed);
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
            arena: false,
            lull: None,
        };
        let mut records = Records::default();
        assert!(!keep_record(&mut records, &run));
        assert_eq!(records, Records::default());
    }
}
