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
    session::{Found, Game, Invitation, Notice, Session, SessionRequest},
    settings::{Action, Binding, Records, Settings},
    theme,
};

/// The only mode there is so far, as records name it.
const MODE: &str = "normal";
/// Seconds between the end of a run and the game over screen.
const GAME_OVER_DELAY: f32 = 1.4;
/// Meters of ground that the main menu shows from the bottom to the top of the screen.
const EXHIBIT_SPAN: f32 = 7.5;
/// Where the tank stands on the screen of the main menu, in meters from its middle.
const EXHIBIT_PLACE: Vec2 = Vec2::new(3.2, 0.2);
/// The longest address that the join screen takes.
const ADDRESS_LENGTH: usize = 64;

/// What a button does.
#[derive(Component, Clone, Copy, PartialEq)]
enum Press {
    Solo,
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

/// The text that shows the address being typed.
#[derive(Component)]
struct AddressField;

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
        .or_else(in_state(Screen::Join))
        .or_else(in_state(Screen::Lobby));
    app.init_resource::<Capture>()
        .init_resource::<Outcome>()
        .init_resource::<Address>()
        .init_resource::<Way>()
        .init_resource::<Copied>()
        .insert_resource(Destination(1))
        .add_systems(OnEnter(Screen::Menu), main_menu)
        .add_systems(OnEnter(Screen::Join), join_screen)
        .add_systems(OnEnter(Screen::Lobby), draw_a_seed)
        .add_systems(OnExit(Screen::Menu), put_the_exhibit_away)
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
                (capture_binding, type_address, press, escape).chain(),
                refresh_pause
                    .run_if(in_state(Overlay::Pause).and_then(resource_exists::<Playtest>)),
                refresh_settings.run_if(in_state(Overlay::Settings)),
                (lobby_screen.run_if(in_state(Screen::Lobby)), show_copy).chain(),
                list_games.run_if(in_state(Screen::Join)),
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

/// A button for something that isn't there yet.
fn placeholder(parent: &mut ChildSpawnerCommands, content: &str) {
    parent.spawn((
        button_node(Val::Auto),
        BorderColor::all(theme::RULE_DIM),
        children![theme::title(
            format!("{content} (later)"),
            20.0,
            theme::TEXT_DIM
        )],
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

/// The best run of a team of some size, or of a single player.
fn record_line(records: &Records, players: u32) -> String {
    match records.best(MODE, players) {
        Some(record) => format!(
            "Best run: sector {}, seed {}, v{}",
            record.sector, record.seed, record.version
        ),
        None => "No run yet".to_string(),
    }
}

/// The best runs of the teams that have one, a line for each size.
fn team_records(records: &Records) -> Vec<String> {
    let teams = (2..=MAX_PLAYERS).filter_map(|players| {
        let record = records.best(MODE, players)?;
        Some(format!(
            "{players} players: sector {}, seed {}, v{}",
            record.sector, record.seed, record.version
        ))
    });
    teams.collect()
}

/// A line that tells the player how a game of several players goes.
fn status_line(parent: &mut ChildSpawnerCommands) {
    parent.spawn((StatusLine, theme::text("", 17.0, theme::DANGER)));
}

fn main_menu(
    mut commands: Commands,
    records: Res<Records>,
    playtest: Option<Res<Playtest>>,
    options: Res<Options>,
) {
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
                placeholder(parent, "Achievements");
                button(parent, "Settings", Press::OpenSettings);
                // A browser tab is closed by the browser.
                if cfg!(not(target_arch = "wasm32")) {
                    button(parent, "Quit", Press::Quit);
                }
                parent
                    .spawn(ruled(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: px(4),
                        padding: UiRect::axes(px(12), px(8)),
                        margin: UiRect::top(px(14)),
                        ..default()
                    }))
                    .with_children(|record| {
                        record.spawn(theme::label("RECORD", 12.0, theme::TEXT_DIM));
                        record.spawn(theme::text(record_line(&records, 1), 16.0, theme::TEXT));
                        for team in team_records(&records) {
                            record.spawn(theme::text(team, 16.0, theme::TEXT));
                        }
                    });
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
) {
    if let (Session::Host, Ok(mut lobby)) = (*session, lobbies.single_mut()) {
        lobby.seed = new_seed(&options, time.elapsed().as_nanos() as u64);
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
                row(parent, "Mode", |row| {
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
                line(
                    parent,
                    format!("Seed {}, v{VERSION}", run.seed_text),
                    theme::TEXT_DIM,
                );
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
    !run.playtest && records.submit(MODE, run.team, run.cleared, &run.seed_text)
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
) {
    let (playtest, team) = run
        .as_ref()
        .map_or((false, 1), |run| (run.playtest, run.team));
    let mode = match team {
        _ if playtest => "PLAYTEST".to_string(),
        1 => "SOLO".to_string(),
        team => format!("CO-OP \u{b7} {team} PLAYERS"),
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
                    line(
                        parent,
                        format!("Seed {}, v{VERSION}", run.seed_text),
                        theme::TEXT_DIM,
                    );
                }
                if playtest {
                    line(parent, "Playtest: no record kept", theme::TEXT_DIM);
                } else if outcome.new_record {
                    line(parent, "New record", theme::GOOD);
                } else {
                    line(parent, record_line(&records, team), theme::TEXT_DIM);
                }
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
                            primary(buttons, "Retry this seed", Press::Retry);
                            button(buttons, "New run", Press::NewRun);
                            button(buttons, "Menu", Press::Abandon);
                        }
                        Session::Host => {
                            primary(buttons, "Retry this seed", Press::Retry);
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
            Press::Solo | Press::NewRun | Press::Retry => {
                let seed = match (*press, &run) {
                    (Press::Retry, Some(run)) => run.seed_text.clone(),
                    _ => fresh_seed(),
                };
                notice.0 = None;
                match *session {
                    Session::Solo => {
                        requests.write(RunRequest::Start { seed, players: 1 });
                        next_screen.set(Screen::Playing);
                        next_overlay.set(Overlay::None);
                    }
                    // A host starts the run for everyone, on the seed of the lobby.
                    Session::Host => {
                        if let Ok(mut lobby) = lobbies.single_mut() {
                            lobby.seed = seed;
                        }
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
    // Esc leaves the join screen as its button "Back" does.
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
    use super::*;

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
    fn a_playtest_sets_no_record() {
        let run = Run {
            seed_text: "TEST".into(),
            seed: 1,
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
        };
        let mut records = Records::default();
        assert!(!keep_record(&mut records, &run));
        assert_eq!(records, Records::default());
    }
}
