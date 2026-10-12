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
    protocol::{Lobby, Member},
    rules::{MAX_PLAYERS, PLAYTEST_LAST_SECTOR, TICK_RATE},
    seed::{self, VERSION},
    sim::{Playtest, Run, RunPhase, RunRequest, SimEvent},
    stats::Genome,
};

use crate::{
    Overlay, Screen,
    camera::View,
    cli::Options,
    crew::LocalSeat,
    exhibit::{self, Exhibit, Show},
    net, report,
    scene::Paints,
    session::{Drawn, Found, Game, Invitation, Notice, Session, SessionRequest},
    settings::{Action, Binding, Records, Settings},
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
    /// On the main menu: lists the best runs of a difficulty.
    List(Difficulty),
    OpenAchievements,
    /// Opens a game on this machine for others to join.
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

#[derive(Component)]
struct PauseRoot;

#[derive(Component)]
struct SettingsRoot;

#[derive(Component)]
struct LobbyRoot;

#[derive(Component)]
struct SoloRoot;

/// Where the main menu lists the best runs of a difficulty.
#[derive(Component)]
struct RecordList;

/// The difficulty whose best runs the main menu lists.
#[derive(Resource, Default)]
struct Listed(Difficulty);

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

/// What a single player has set for their next run.
#[derive(Resource, Default)]
struct Setup {
    seed: String,
    /// Whether the player typed the seed. The game drew it otherwise.
    typed: bool,
    difficulty: Difficulty,
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
        .or_else(in_state(Screen::Achievements))
        .or_else(in_state(Screen::Join))
        .or_else(in_state(Screen::Lobby));
    app.init_resource::<Capture>()
        .init_resource::<Outcome>()
        .init_resource::<Address>()
        .init_resource::<Setup>()
        .init_resource::<Listed>()
        .init_resource::<Way>()
        .init_resource::<Copied>()
        .insert_resource(Destination(1))
        .add_systems(
            OnEnter(Screen::Menu),
            (list_what_was_played, main_menu).chain(),
        )
        .add_systems(OnEnter(Screen::Solo), draw_a_seed_for_one)
        .add_systems(OnEnter(Screen::Achievements), achievements_screen)
        .add_systems(OnEnter(Screen::Join), join_screen)
        .add_systems(OnEnter(Screen::Lobby), draw_a_seed)
        .add_systems(OnExit(Screen::Menu), put_the_exhibit_away)
        .add_systems(OnExit(Screen::Solo), put_the_exhibit_away)
        .add_systems(OnExit(Screen::Achievements), put_the_exhibit_away)
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
                (capture_binding, type_address, type_seed, press, escape).chain(),
                (solo_screen.run_if(in_state(Screen::Solo)), show_seed).chain(),
                refresh_pause
                    .run_if(in_state(Overlay::Pause).and_then(resource_exists::<Playtest>)),
                refresh_settings.run_if(in_state(Overlay::Settings)),
                (lobby_screen.run_if(in_state(Screen::Lobby)), show_copy).chain(),
                list_games.run_if(in_state(Screen::Join)),
                list_records.run_if(in_state(Screen::Menu)),
                (show_address, tell_how_it_goes),
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

/// The best run of a difficulty and of a team of some size, or of a single player.
fn record_line(records: &Records, difficulty: Difficulty, players: u32) -> String {
    let (best, none) = match difficulty {
        Difficulty::Normal => ("Best run", "No run yet"),
        Difficulty::Hard => ("Best run in hard mode", "No run in hard mode yet"),
    };
    match records.best(difficulty, players) {
        Some(record) => format!(
            "{best}: sector {}, seed {}, v{}",
            record.sector, record.seed, record.version
        ),
        None => none.to_string(),
    }
}

/// The best runs of a difficulty, a line for each size of team that has one: a single player
/// first.
fn records_of(records: &Records, difficulty: Difficulty) -> Vec<String> {
    let teams = (1..=MAX_PLAYERS).filter_map(|players| {
        let record = records.best(difficulty, players)?;
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

/// The main menu first lists the best runs of the difficulty that the player chose last
/// for a run of their own.
fn list_what_was_played(
    settings: Res<Settings>,
    unlocks: Res<Unlocks>,
    options: Res<Options>,
    mut listed: ResMut<Listed>,
) {
    listed.0 = last_difficulty(&settings, &unlocks, &options);
}

/// Lists the best runs of a difficulty on the main menu, again whenever there is a new one
/// or the player asks for those of another difficulty. A difficulty is only offered once the
/// player has a run in it.
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
        let (name, none) = match listed.0 {
            Difficulty::Normal => ("RECORD", "No run yet"),
            Difficulty::Hard => ("RECORD IN HARD MODE", "No run in hard mode yet"),
        };
        let other = match listed.0 {
            Difficulty::Normal => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Normal,
        };
        let offered = listed.0 != Difficulty::Normal || !records_of(&records, other).is_empty();
        let mut lines = records_of(&records, listed.0);
        if lines.is_empty() {
            lines.push(none.to_string());
        }
        commands
            .entity(list)
            .despawn_related::<Children>()
            .with_children(|record| {
                let header = Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: px(12),
                    ..default()
                };
                record.spawn(header).with_children(|header| {
                    header.spawn(theme::label(name, 12.0, theme::TEXT_DIM));
                    if offered {
                        let to = match other {
                            Difficulty::Normal => "Normal mode",
                            Difficulty::Hard => "Hard mode",
                        };
                        header.spawn((
                            Button,
                            Press::List(other),
                            Node {
                                padding: UiRect::axes(px(8), px(2)),
                                border: UiRect::all(px(1)),
                                ..default()
                            },
                            BackgroundColor(theme::SURFACE_RAISED),
                            BorderColor::all(theme::RULE),
                            children![theme::label(to, 12.0, theme::TEXT)],
                        ));
                    }
                });
                for line in lines {
                    record.spawn(theme::text(line, 16.0, theme::TEXT));
                }
            });
    }
}

fn main_menu(mut commands: Commands, playtest: Option<Res<Playtest>>, options: Res<Options>) {
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
                    button(parent, "Host a game", Press::Host);
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
                            theme::title("PLAYER TANK", 18.0, theme::TEXT),
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
                            theme::figure(
                                format!("{} POINTS", Genome::PLAYER.budget),
                                18.0,
                                theme::TEXT
                            ),
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
    mut shown: Local<Option<Screen>>,
) {
    let wanted = *screen.get();
    if !exhibits.is_empty() && *shown == Some(wanted) {
        return;
    }
    for stale in &exhibits {
        commands.entity(stale).despawn();
    }
    *shown = Some(wanted);
    let show = Show {
        genome: Genome::PLAYER,
        color: theme::PLAYER,
        span: EXHIBIT_SPAN,
        place: EXHIBIT_PLACE,
        turret: 0.35,
        sways: true,
        shield: true,
    };
    exhibit::put(
        &mut commands,
        &mut meshes,
        &paints,
        &mut view,
        wanted,
        &show,
    );
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

/// Draws a seed for the run that a single player sets up. Its difficulty is the one that the
/// player chose last, if it is still theirs to choose.
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

/// Starts the run that a single player set up. A run needs a seed.
fn begin(setup: &Setup, requests: &mut MessageWriter<RunRequest>, screen: &mut NextState<Screen>) {
    if setup.seed.is_empty() {
        return;
    }
    requests.write(RunRequest::Start {
        seed: setup.seed.clone(),
        players: 1,
        drawn: !setup.typed,
        difficulty: setup.difficulty,
    });
    screen.set(Screen::Playing);
}

/// Takes what the player types on the solo screen: the seed of the run, once typing one is
/// unlocked. Enter starts the run.
fn type_seed(
    mut keys: MessageReader<KeyboardInput>,
    screen: Res<State<Screen>>,
    unlocks: Res<Unlocks>,
    options: Res<Options>,
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
            begin(&setup, &mut requests, &mut next_screen);
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
    options: Res<Options>,
    roots: Query<Entity, With<SoloRoot>>,
) {
    if !roots.is_empty() && !setup.is_changed() && !unlocks.is_changed() {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
    // Free mode is not in the game yet: once it is unlocked, it says so.
    let waits = |unlock: Unlock| {
        if open(&unlocks, &options, unlock) {
            "Not in the game yet".to_string()
        } else {
            unlocks::condition(unlock)
        }
    };
    let hard = plays(&unlocks, &options, Difficulty::Hard);
    let types = open(&unlocks, &options, Unlock::ChosenSeed);
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
                    row.spawn(theme::text(waits(Unlock::FreeMode), 16.0, theme::TEXT_DIM));
                });
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
                if setup.typed && unlocks.wanting() && !options.playtest {
                    line(
                        parent,
                        "A run on a typed seed earns no achievement.",
                        theme::TEXT,
                    );
                }
                primary(parent, "Start", Press::Begin);
                button(parent, "Back", Press::Abandon);
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

/// What a row of the join screen says about a game that was found, next to its address.
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

/// The row of a game that was found. One that can be joined is a button.
fn found_row(parent: &mut ChildSpawnerCommands, game: &Game) {
    let open = game.standing == Standing::Open;
    let ink = if open { theme::TEXT } else { theme::TEXT_DIM };
    let node = Node {
        justify_content: JustifyContent::SpaceBetween,
        align_items: AlignItems::Center,
        column_gap: px(16),
        ..button_node(Val::Auto)
    };
    let words = children![
        theme::figure(game.address.clone(), 20.0, ink),
        theme::text(found_words(game), 16.0, ink),
    ];
    if open {
        parent.spawn((
            Button,
            Press::JoinFound(game.id),
            node,
            BackgroundColor(theme::SURFACE_RAISED),
            BorderColor::all(theme::RULE),
            words,
        ));
    } else {
        parent.spawn((node, BorderColor::all(theme::RULE_DIM), words));
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
                let role = if host { "host" } else { "guest" };
                parent.spawn(theme::label(
                    format!("Game of several players \u{b7} {role}"),
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
    !run.playtest && records.submit(run.difficulty, run.team, run.cleared, &run.seed_text)
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
    let difficulty = run
        .as_ref()
        .map_or(Difficulty::Normal, |run| run.difficulty);
    let mode = match difficulty {
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
                    let best = record_line(&records, difficulty, team);
                    line(parent, best, theme::TEXT_DIM);
                }
                for &unlock in &unlocks.new {
                    let name = unlocks::name(unlock);
                    line(parent, format!("Unlocked: {name}"), theme::GOOD);
                }
                // A player who has everything is not told what they miss.
                let wanting = unlocks.wanting() && !playtest;
                if wanting && run.as_ref().is_some_and(|run| !run.earns) {
                    let why = "This run earned no achievement: its seed was typed or played again.";
                    line(parent, why, theme::TEXT_DIM);
                }
                let retry = |buttons: &mut ChildSpawnerCommands| {
                    primary(buttons, "Retry this seed", Press::Retry);
                    if wanting {
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
    (session, mut address, mut notice): (Res<Session>, ResMut<Address>, ResMut<Notice>),
    (mut setup, mut drawn, mut listed): (ResMut<Setup>, ResMut<Drawn>, ResMut<Listed>),
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
            Press::Begin => begin(&setup, &mut requests, &mut next_screen),
            Press::Redraw => (setup.seed, setup.typed) = (fresh_seed(), false),
            // The choice is kept for the next time.
            Press::Difficulty(difficulty) => {
                setup.difficulty = difficulty;
                if settings.solo_difficulty != difficulty {
                    settings.solo_difficulty = difficulty;
                }
            }
            Press::List(difficulty) => listed.0 = difficulty,
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
                        // The next run is as hard as the one that is over.
                        let difficulty = run.as_ref().map(|run| run.difficulty);
                        requests.write(RunRequest::Start {
                            seed,
                            players: 1,
                            drawn: fresh,
                            difficulty: difficulty.unwrap_or(setup.difficulty),
                        });
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
            Press::Host => {
                notice.0 = None;
                session_requests.write(SessionRequest::Host);
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
    let before = matches!(*screen.get(), Screen::Solo | Screen::Achievements);
    if before && *overlay.get() == Overlay::None {
        next_screen.set(Screen::Menu);
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
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        (Changed<Interaction>, With<Press>),
    >,
) {
    for (interaction, primary, mut color, mut border) in &mut buttons {
        let (fill, edge) = match (interaction, primary) {
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
        }
    }

    /// Sits this many players in the lobby of a game, the host first.
    fn gather(app: &mut App, players: usize) {
        let mut lobby = Lobby {
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
            ("the main menu with the records of hard mode", |app| {
                app.insert_resource(Records::of_every_team(128, LONG_SEED))
                    .insert_resource(Listed(Difficulty::Hard))
                    .insert_resource(Notice(Some(LONG_NOTICE.into())));
                build(app, main_menu);
            }),
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
                    });
                build(app, main_menu);
            }),
            ("the solo screen", |app| {
                app.insert_resource(Setup {
                    seed: "85GVTHQA".into(),
                    typed: false,
                    difficulty: Difficulty::Normal,
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
                    });
                build(app, solo_screen);
            }),
            ("the achievements", |app| {
                app.insert_resource(Unlocks::of(&[Unlock::ChosenSeed, Unlock::HardMode]));
                build(app, achievements_screen);
            }),
            ("the join screen", |app| {
                let game = |id: u64, standing: Standing| Game {
                    id,
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
                    game(5, Standing::Open),
                ];
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
            // A run of four players in hard mode, on a seed as long as one gets: the lines
            // of the report are then as long as they get.
            ("the report of a run in hard mode", |app| {
                let run = Run {
                    seed_text: LONG_SEED.into(),
                    difficulty: Difficulty::Hard,
                    earns: false,
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
            .init_resource::<Setup>()
            .init_resource::<Listed>()
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
    fn a_run_sets_the_record_of_its_difficulty_and_of_a_team_of_its_size() {
        use Difficulty::{Hard, Normal};
        let run = |difficulty: Difficulty, team: u32, cleared: u32| Run {
            difficulty,
            cleared,
            ..lost_run(team)
        };
        let mut records = Records::default();
        assert!(keep_record(&mut records, &run(Hard, 1, 3)));
        assert!(records.best(Normal, 1).is_none());
        assert_eq!(records.best(Hard, 1).unwrap().sector, 3);
        // A run in normal mode that got further is no record of hard mode.
        assert!(keep_record(&mut records, &run(Normal, 1, 12)));
        assert_eq!(records.best(Hard, 1).unwrap().sector, 3);
        assert!(!keep_record(&mut records, &run(Hard, 1, 3)));
        assert!(keep_record(&mut records, &run(Hard, 2, 1)));
        // The report and the main menu say which difficulty a best run is of.
        assert_eq!(
            record_line(&records, Normal, 1),
            format!("Best run: sector 12, seed 85GVTHQA, v{VERSION}")
        );
        assert_eq!(
            record_line(&records, Hard, 1),
            format!("Best run in hard mode: sector 3, seed 85GVTHQA, v{VERSION}")
        );
        assert_eq!(record_line(&records, Normal, 3), "No run yet");
        assert_eq!(record_line(&records, Hard, 3), "No run in hard mode yet");
        let hard = records_of(&records, Hard);
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
            });
            build(app, solo_screen);
        };
        assert_eq!(offered(playtest), [Difficulty::Hard]);
    }

    /// What the solo screen sets up for a player who chose a difficulty last.
    fn set_up(last: Difficulty, unlocks: &[Unlock], playtest: bool) -> Difficulty {
        let mut world = World::new();
        world.insert_resource(Options {
            playtest,
            seed: None,
        });
        world.insert_resource(Time::<Real>::default());
        let mut settings = Settings::default();
        settings.solo_difficulty = last;
        world.insert_resource(settings);
        world.insert_resource(Unlocks::of(unlocks));
        world.init_resource::<Setup>();
        world.run_system_once(draw_a_seed_for_one).unwrap();
        let setup = world.resource::<Setup>();
        assert!(!setup.seed.is_empty() && !setup.typed);
        setup.difficulty
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
    }

    #[test]
    fn the_solo_screen_starts_a_run_of_the_difficulty_and_on_the_seed_that_it_shows() {
        fn start(
            setup: Res<Setup>,
            mut requests: MessageWriter<RunRequest>,
            mut screen: ResMut<NextState<Screen>>,
        ) {
            begin(&setup, &mut requests, &mut screen);
        }
        let asked = |setup: Setup| {
            let mut world = World::new();
            world.insert_resource(setup);
            world.init_resource::<Messages<RunRequest>>();
            world.init_resource::<NextState<Screen>>();
            world.run_system_once(start).unwrap();
            let mut requests = world.resource_mut::<Messages<RunRequest>>();
            let asked: Vec<RunRequest> = requests.drain().collect();
            asked
        };
        let started = asked(Setup {
            seed: "TANKU".into(),
            typed: true,
            difficulty: Difficulty::Hard,
        });
        let [
            RunRequest::Start {
                seed,
                players,
                drawn,
                difficulty,
            },
        ] = started.as_slice()
        else {
            panic!("{started:?}");
        };
        // A seed that was typed is not one that the game drew.
        assert_eq!((seed.as_str(), *players, *drawn), ("TANKU", 1, false));
        assert_eq!(*difficulty, Difficulty::Hard);
        // A run needs a seed.
        assert!(asked(Setup::default()).is_empty());
    }

    /// The difficulties whose best runs the main menu offers to list instead.
    fn listed(screen: fn(&mut App)) -> Vec<Difficulty> {
        let listed = presses(screen).into_iter().filter_map(|press| match press {
            Press::List(difficulty) => Some(difficulty),
            _ => None,
        });
        listed.collect()
    }

    #[test]
    fn the_main_menu_lists_the_best_runs_of_hard_mode_once_there_is_one() {
        // A player who never played in hard mode is shown the best runs of normal mode,
        // and no way to others.
        let normal = |app: &mut App| {
            let mut records = Records::default();
            records.submit(Difficulty::Normal, 1, 12, "85GVTHQA");
            app.insert_resource(records);
            build(app, main_menu);
        };
        assert_eq!(listed(normal), []);
        let both = |app: &mut App| {
            app.insert_resource(Records::of_every_team(128, "85GVTHQA"));
            build(app, main_menu);
        };
        assert_eq!(listed(both), [Difficulty::Hard]);
        // From the best runs of hard mode, there is always a way back.
        let hard = |app: &mut App| {
            app.insert_resource(Listed(Difficulty::Hard));
            build(app, main_menu);
        };
        assert_eq!(listed(hard), [Difficulty::Normal]);
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
        };
        let mut records = Records::default();
        assert!(!keep_record(&mut records, &run));
        assert_eq!(records, Records::default());
    }
}
