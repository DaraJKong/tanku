//! Development tools, left out of release builds: they come with the `script` feature, which
//! a development build has.
//!
//! `TANKU_DEV_SCRIPT` holds commands separated by semicolons, run one after the other:
//! `start SEED PLAYERS DIFFICULTY`, which starts a run in normal mode unless `hard` is asked
//! for, in arena mode if `arena` comes after it, in the tank of the command line if `--tank`
//! gave one, and in a playtest with every seat but the first driven by the game if
//! `teammates` comes after it, by the reference player that is named (`novice`, `regular`
//! or `expert`, regular if none is), `seat NUMBER`, `repair SECONDS` and `shield SECONDS`, which hold the
//! key of that name for so long, `wait SECONDS`, `warp SECTOR`, `unlock`, which unlocks every
//! mode and option, or those that it names, as `unlock seed free`, `tank TANK`, which builds the tank of the garage as the command line
//! writes one,
//! `host PORT NAME`, which opens a game for others to join, under a name if one is given, and
//! takes browsers on the port after that one, `password WORDS`, which gives a password to the
//! game that `host` opens next, `port PORT`, which sets the port of a game that is hosted
//! from the menus, `join ADDRESS PASSWORD`, which joins a game by its address or by what its
//! host hands out for browsers, with a password if one is given,
//! `type TEXT`, which presses the keys that write a text, `key NAME`, which presses `enter`,
//! `tab` or `backspace`, `turn QUARTERS`, `peek ENEMY`,
//! `look X Y`, `zoom METERS`, `clear`, `reach DX DY`, which puts the tank that far from the life
//! that an arena left, `lives NUMBER`, `cross`, which carries the tank through the barrier of
//! its sector,
//! `bubble`, `die`, `wreck SEAT`, which destroys the tank of the player of a seat, `pause`,
//! `settings`, `resume`,
//! `lose ENEMY`, `blast X Y`, which sets off a mine at a point of the map, `rout`, which sets
//! one off under each enemy of the sector of the player, `warn X Y`, which
//! sounds the warning of a mine there without any mine, `shot PATH` and
//! `quit`, plus `play DX DY AX AY SECONDS`, which drives the tank in a direction while it fires
//! at a point relative to itself, `aim AX AY`, which turns the turret to such a point without
//! firing, `press LABEL`, which presses the button of a menu that bears
//! this label, in capitals or not, or `press ROW: LABEL` for the button of one row of a
//! menu, as `press Arena mode: On`, `hover LABEL`, which keeps the pointer from then on over
//! what bears this label and answers it, a button or a row, as `hover Muzzle` in the garage,
//! and `hover` alone, which takes it away, `lineup`, which lays out one bullet of each kind next to the
//! tank, `pierce`, which sends a piercing bullet at a bullet of the players there,
//! `showcase STAT`, which lines up in the start bay one tank for each level of a stat,
//! `palette`, which lines up there three tanks of each class, from a weak one to a strong one,
//! `doze`, which puts the tanks that are lined up to sleep, `rouse`, which wakes them again,
//! `canvas WIDTH HEIGHT SCALE`,
//! which draws from there on as a window of that many pixels shows the game, on a screen
//! that enlarges everything by that scale if one is given, `tape PATH`, which keeps the
//! music from there on, and `lift SECONDS`, which holds everything that wakes at one moment of
//! its rise, in seconds since it woke, to look at it, until `lift` alone lets it go on. It is
//! how the game is looked at, and listened to, without anyone at the keyboard.
//!
//! A scripted run opens no window: it draws into an image, which `shot` saves. The image is
//! 1920 by 1080 pixels until `canvas` gives it another size. The run makes no sound, starts
//! from the default settings and leaves the player's files alone. The music that it would
//! play goes to a sound file when the script quits, if `tape` asked for it, next to a text
//! file that lists what the music was told and when.

use std::{collections::VecDeque, fmt::Write, path::PathBuf, time::Duration};

use bevy::{
    app::ScheduleRunnerPlugin,
    camera::{ImageRenderTarget, RenderTarget},
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput, NativeKeyCode},
    },
    log::LogPlugin,
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, save_to_disk},
    },
    ui::UiSystems,
    window::ExitCondition,
    winit::WinitPlugin,
};
use tanku_core::{
    modes::{Difficulty, Unlock},
    reference::{Reference, Teammates},
    sim::{
        Barrier, Blow, Bullet, Enemy, Kind, Life, Mine, MineState, Motion, Position,
        PreviousPosition, Run, RunPhase, RunRequest, Shield, ShieldState, SimEvent, Tank,
        TankCommand, Team, Weapon,
    },
    stats::{Class, Genome, Stat},
};
use tanku_server::{
    Casualties, Maker,
    field::{Field, InSector},
};

use crate::{
    Overlay, Screen,
    audio::Muted,
    camera::{MainCamera, View},
    cli::{self, Options},
    crew::{LocalSeat, Own},
    garage::Garage,
    host::{HostPort, WebPort},
    input,
    margin::Margin,
    music::Baton,
    profile,
    session::{SessionRequest, Sign},
    storage,
    unlocks::Unlocks,
    wake::Hold,
};

/// Size of the image that a scripted run draws into, until the script gives it another.
const CANVAS: UVec2 = UVec2::new(1920, 1080);

#[derive(Resource)]
struct Script {
    commands: VecDeque<String>,
    wait: f32,
    /// What the script makes the player's tank do, and for how long.
    play: Option<(Vec2, Vec2, f32)>,
    /// The point that the script keeps the turret turned to, from the tank.
    aim: Option<Vec2>,
    /// Seconds during which the script still holds the repair key, and the shield key.
    repair: f32,
    shield: f32,
    /// The label of the button that the script presses.
    press: Option<String>,
    /// The label of what the script keeps the pointer over.
    hover: Option<String>,
    /// The password of the game that the script hosts next.
    password: String,
}

/// Whether the interface is hidden, to look at the game alone.
#[derive(Resource, Default)]
struct Bare(bool);

/// Hides every part of the interface while the script asks for the game alone, and gives
/// each back the visibility it had.
fn strip(
    bare: Res<Bare>,
    mut roots: Query<(Entity, &mut Visibility), (With<Node>, Without<ChildOf>)>,
    mut hidden: Local<Vec<(Entity, Visibility)>>,
) {
    if bare.0 {
        for (entity, mut visibility) in &mut roots {
            if *visibility != Visibility::Hidden {
                hidden.push((entity, *visibility));
                *visibility = Visibility::Hidden;
            }
        }
    } else {
        for (entity, was) in hidden.drain(..) {
            if let Ok((_, mut visibility)) = roots.get_mut(entity) {
                *visibility = was;
            }
        }
    }
}

/// The music that a scripted run would play, kept from the command `tape` on.
#[derive(Resource)]
struct Tape {
    path: PathBuf,
    music: tanku_audio::Music,
    samples: Vec<f32>,
    /// When the tape started, in seconds of the clock of the machine.
    started: f64,
    /// What the music was told, and how many seconds into the tape.
    told: Vec<(f64, tanku_audio::Context)>,
}

impl Tape {
    /// Writes the sound file, and the list of what the music was told next to it.
    fn save(&self) {
        let mut list = String::new();
        for (at, context) in &self.told {
            let _ = writeln!(
                list,
                "{:02}:{:04.1}  {context:?}",
                *at as u32 / 60,
                at % 60.0
            );
        }
        let written = tanku_audio::wav::write(&self.path, &self.samples)
            .and_then(|()| std::fs::write(self.path.with_extension("txt"), list));
        if let Err(error) = written {
            warn!("the tape was not saved: {error}");
        }
    }
}

/// A tank that `showcase` put on display.
#[derive(Component)]
struct Exhibit;

/// The stats by the names that `showcase` takes.
const STATS: [(&str, Stat); 15] = [
    ("engine", Stat::Engine),
    ("tracks", Stat::Tracks),
    ("turret", Stat::Turret),
    ("muzzle", Stat::Muzzle),
    ("magazine", Stat::Magazine),
    ("loader", Stat::Loader),
    ("ricochet", Stat::Ricochet),
    ("mines", Stat::Mines),
    ("shield", Stat::Shield),
    ("piercing", Stat::Piercing),
    ("seeking", Stat::Seeking),
    ("splitting", Stat::Splitting),
    ("exploding", Stat::Exploding),
    ("brain", Stat::Brain),
    ("ballistics", Stat::Ballistics),
];

/// A middling tank, on which the levels of one stat are shown.
const SHOWN_ON: [(Stat, u8); 9] = [
    (Stat::Engine, 3),
    (Stat::Tracks, 2),
    (Stat::Turret, 2),
    (Stat::Muzzle, 3),
    (Stat::Magazine, 2),
    (Stat::Loader, 3),
    (Stat::Ricochet, 1),
    (Stat::Mines, 2),
    (Stat::Brain, 2),
];

/// The budgets of the tanks of `palette`, from a pale tank to a saturated one.
const STRENGTHS: [u16; 3] = [15, 55, 100];

/// Three tanks of each class, from a weak one to a strong one, each with a few levels of
/// what its class favors.
fn palette() -> impl Iterator<Item = Genome> {
    Class::ALL.into_iter().flat_map(|class| {
        STRENGTHS.map(|budget| {
            let mut genome = Genome::empty(budget);
            genome.class = class;
            genome.set(Stat::Engine, 2);
            for &stat in class.favorites() {
                genome.set(stat, class.highest(stat).min(3));
            }
            genome
        })
    })
}

/// Whether a script runs the game.
pub fn scripted() -> bool {
    std::env::var_os("TANKU_DEV_SCRIPT").is_some()
}

/// Adds the engine for a scripted run: no window, and frames at a steady pace, unless a
/// profile lets them come as fast as they can (see `profile.rs`).
pub fn add_engine(app: &mut App) {
    storage::keep_nothing();
    let pace = if profile::uncapped() {
        Duration::ZERO
    } else {
        Duration::from_secs_f64(1.0 / 60.0)
    };
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(LogPlugin {
                custom_layer: profile::layer,
                ..default()
            })
            .set(crate::task_pools())
            .disable::<WinitPlugin>(),
        ScheduleRunnerPlugin::run_loop(pace),
    ))
    .insert_resource(Muted);
    profile::plugin(app);
}

pub fn plugin(app: &mut App) {
    let Ok(script) = std::env::var("TANKU_DEV_SCRIPT") else {
        return;
    };
    app.insert_resource(Script {
        commands: script
            .split(';')
            .map(|command| command.trim().to_string())
            .collect(),
        wait: 0.0,
        play: None,
        aim: None,
        repair: 0.0,
        shield: 0.0,
        press: None,
        hover: None,
        password: String::new(),
    })
    .init_resource::<Bare>()
    .add_systems(PostStartup, draw_into_an_image)
    .add_systems(Update, run_script)
    .add_systems(
        PostUpdate,
        strip.before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
    )
    .add_systems(Last, roll_the_tape)
    .add_systems(
        PreUpdate,
        (
            play.after(input::command_player),
            press.after(UiSystems::Focus),
            hover.after(UiSystems::Focus),
        ),
    );
}

/// An image of this size for the game to draw into.
fn blank(size: UVec2) -> Image {
    Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None)
}

/// Points the camera at an image, which stands in for the window, and the interface with it.
fn draw_into_an_image(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    camera: Single<Entity, With<MainCamera>>,
) {
    let canvas = images.add(blank(CANVAS));
    commands
        .entity(*camera)
        .insert((RenderTarget::from(canvas), IsDefaultUiCamera));
}

/// Computes the music that the frame would have played, since nothing plays it.
fn roll_the_tape(time: Res<Time<Real>>, baton: Res<Baton>, tape: Option<ResMut<Tape>>) {
    let Some(mut tape) = tape else {
        return;
    };
    let tape = &mut *tape;
    let now = time.elapsed_secs_f64() - tape.started;
    let context = baton.0.context();
    if tape.told.last().is_none_or(|(_, last)| *last != context) {
        tape.told.push((now, context));
    }
    let frames = (now * tanku_audio::SAMPLE_RATE as f64) as usize;
    let recorded = tape.samples.len();
    let due = frames * tanku_audio::CHANNELS as usize;
    if due > recorded {
        tape.samples.resize(due, 0.0);
        tape.music.render(&mut tape.samples[recorded..]);
    }
}

/// Takes the controls of the player's tank while the script plays.
fn play(
    time: Res<Time<Real>>,
    mut script: ResMut<Script>,
    mut player: Query<(&Position, &mut TankCommand), With<Own>>,
) {
    let (repair, shield) = (script.repair > 0.0, script.shield > 0.0);
    script.repair = (script.repair - time.delta_secs()).max(0.0);
    script.shield = (script.shield - time.delta_secs()).max(0.0);
    let Ok((position, mut command)) = player.single_mut() else {
        return;
    };
    command.repair |= repair;
    command.shield |= shield;
    if let Some(aim) = script.aim {
        command.aim = position.0 + aim;
    }
    let Some((drive, aim, left)) = script.play else {
        return;
    };
    script.play = (left > 0.0).then_some((drive, aim, left - time.delta_secs()));
    *command = TankCommand {
        drive,
        aim: position.0 + aim,
        fire: aim != Vec2::ZERO,
        mine: left % 2.0 < 0.1,
        shield,
        repair,
    };
}

/// Puts tanks on display in rows of six from the north of the start bay, in place of those
/// that were there: the bay has room for six rows, which is three tanks of each class. The
/// tanks do nothing.
fn display(
    commands: &mut Commands,
    exhibits: &Query<Entity, With<Exhibit>>,
    genomes: impl Iterator<Item = Genome>,
) {
    for exhibit in exhibits {
        commands.entity(exhibit).despawn();
    }
    for (place, genome) in genomes.enumerate() {
        let (row, column) = (place / 6, place % 6);
        let at = Vec2::new(column as f32 * 2.2 - 5.5, 14.0 - row as f32 * 2.5);
        let facing = std::f32::consts::FRAC_PI_2;
        commands.spawn((
            Exhibit,
            Tank {
                genome,
                team: Team::Enemies,
                sector: 0,
            },
            Position(at),
            PreviousPosition(at),
            Motion {
                hull: facing,
                turret: facing + 0.5,
                ..default()
            },
            // Aiming where the turret already points keeps it still.
            TankCommand {
                aim: at + Vec2::from_angle(facing + 0.5) * 10.0,
                ..default()
            },
            InSector(0),
        ));
    }
}

/// Presses the button that the script asked for, as a click does. It comes after the interface
/// has looked for the cursor, which would let go of the button, and before the menus react.
fn press(
    mut script: ResMut<Script>,
    mut buttons: Query<(Entity, &mut Interaction), With<Button>>,
    inside: Query<&Children>,
    holders: Query<&ChildOf>,
    labels: Query<&Text>,
) {
    let Some(asked) = script.press.take() else {
        return;
    };
    // Two rows of a menu may hold buttons that bear the same word: `ROW: LABEL` names the
    // button of one row.
    let (row, label) = match asked.split_once(": ") {
        Some((row, label)) => (Some(row), label),
        None => (None, asked.as_str()),
    };
    // A piece bears every text that it holds: the row of a game holds several.
    let bears = |piece: Entity, wanted: &str| {
        let mut texts = inside
            .iter_descendants(piece)
            .filter_map(|piece| labels.get(piece).ok());
        texts.any(|text| text.0.eq_ignore_ascii_case(wanted))
    };
    // The row of a button is what holds it: further up, the whole menu bears every label.
    let in_its_row = |button: Entity| {
        row.is_none_or(|row| {
            let held_by = holders.get(button).map(ChildOf::parent);
            held_by.is_ok_and(|holder| bears(holder, row))
        })
    };
    let mut found = buttons
        .iter_mut()
        .filter(|(button, _)| bears(*button, label) && in_its_row(*button));
    match found.next() {
        Some((_, mut interaction)) => *interaction = Interaction::Pressed,
        None => warn!("no button bears the label: {asked}"),
    }
}

/// Keeps the pointer over what the script asked for, as if it stood there: what bears its
/// label and answers the pointer, the nearest such thing to the label.
fn hover(
    script: Res<Script>,
    mut answering: Query<(Entity, &mut Interaction)>,
    inside: Query<&Children>,
    labels: Query<&Text>,
) {
    let Some(asked) = &script.hover else {
        return;
    };
    let bears = |piece: Entity| {
        let mut texts = inside
            .iter_descendants(piece)
            .filter_map(|piece| labels.get(piece).ok());
        texts.any(|text| text.0.eq_ignore_ascii_case(asked))
    };
    // What holds the label the closest has the fewest pieces under it.
    let pieces = |piece: Entity| inside.iter_descendants(piece).count();
    let nearest = answering
        .iter()
        .filter(|(piece, _)| bears(*piece))
        .min_by_key(|(piece, _)| pieces(*piece))
        .map(|(piece, _)| piece);
    if let Some(piece) = nearest
        && let Ok((_, mut interaction)) = answering.get_mut(piece)
    {
        interaction.set_if_neq(Interaction::Hovered);
    }
}

fn run_script(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut script: ResMut<Script>,
    mut requests: MessageWriter<RunRequest>,
    mut screen: ResMut<NextState<Screen>>,
    mut overlay: ResMut<NextState<Overlay>>,
    mut view: ResMut<View>,
    mut run: Option<ResMut<Run>>,
    mut field: Option<ResMut<Field>>,
    mut casualties: ResMut<Casualties>,
    (enemies, mut player, lives, exhibits, mut shields, barriers, crew): (
        Query<(Entity, &Tank, &Position), With<Enemy>>,
        Query<(Entity, &mut Position), (With<Own>, Without<Enemy>)>,
        Query<&Position, (With<Life>, Without<Own>)>,
        Query<Entity, With<Exhibit>>,
        Query<(&Tank, &mut Shield)>,
        Query<&Barrier>,
        Query<(Entity, &tanku_core::sim::Player)>,
    ),
    (mut images, mut camera): (
        ResMut<Assets<Image>>,
        Single<(Entity, &RenderTarget, &mut Projection), With<MainCamera>>,
    ),
    (baton, tape, mut seat, mut unlocks, options, mut garage): (
        Res<Baton>,
        Option<Res<Tape>>,
        ResMut<LocalSeat>,
        ResMut<Unlocks>,
        Res<Options>,
        ResMut<Garage>,
    ),
    (mut session_requests, mut keys, mut port, mut web, mut hold, mut margin, mut bare): (
        MessageWriter<SessionRequest>,
        MessageWriter<KeyboardInput>,
        ResMut<HostPort>,
        ResMut<WebPort>,
        ResMut<Hold>,
        ResMut<Margin>,
        ResMut<Bare>,
    ),
    mut events: MessageWriter<SimEvent>,
    mut exit: MessageWriter<AppExit>,
) {
    script.wait -= time.delta_secs();
    if script.wait > 0.0 {
        return;
    }
    let Some(command) = script.commands.pop_front() else {
        return;
    };
    let words: Vec<&str> = command.split_whitespace().collect();
    let word = |index: usize| words.get(index).copied().unwrap_or("");
    let number = |index: usize| word(index).parse::<f32>().unwrap_or(0.0);
    match word(0) {
        // With `teammates`, the game drives every seat but the first, in the hands of the
        // reference player that is named, or of the regular one: a playtest's alone.
        "start" => {
            let players = (number(2) as u32).max(1);
            let teammates = Teammates {
                count: if words.contains(&"teammates") {
                    players as u8 - 1
                } else {
                    0
                },
                player: words
                    .iter()
                    .find_map(|word| Reference::of_name(word))
                    .unwrap_or_default(),
            };
            requests.write(RunRequest::Start {
                seed: word(1).to_string(),
                players,
                drawn: true,
                difficulty: Difficulty::of_key(word(3)).unwrap_or_default(),
                // A tank from the command line makes it a run of free mode.
                free: options.tank.is_some(),
                arena: words[1..].contains(&"arena"),
                tanks: options.tanks(),
                teammates,
            });
            screen.set(Screen::Playing);
        }
        "wait" => script.wait = number(1),
        // Unlocks the modes and the options that are named, as `seed` or `free` or `hard`,
        // or all of them if none is: as a player has who earned them.
        "unlock" => {
            let named = |unlock: &Unlock| words.len() == 1 || words[1..].contains(&unlock.key());
            let earned: Vec<Unlock> = Unlock::ALL.into_iter().filter(named).collect();
            *unlocks = Unlocks::of(&earned);
        }
        // Builds the tank of the garage, as the command line writes a tank.
        "tank" => match cli::tank(word(1)) {
            Ok(tank) => garage.tank = tank,
            Err(why) => warn!("no tank can be built as asked: {why}"),
        },
        // Opens a game for others to join, on a port of its own if one is given. Browsers
        // are then taken on the port after it. The words after the port are its name.
        "host" => {
            if let Ok(wanted) = word(1).parse::<u16>() {
                port.0 = wanted;
                web.0 = wanted.checked_add(1);
            }
            let name = words.get(2..).unwrap_or_default().join(" ");
            let password = std::mem::take(&mut script.password);
            session_requests.write(SessionRequest::Host(Sign {
                name,
                password,
                ..default()
            }));
        }
        // Gives a password to the game that the script hosts next.
        "password" => script.password = words[1..].join(" "),
        // The port of the game that this machine hosts from its menus. Browsers are taken
        // on the port after it.
        "port" => {
            if let Ok(wanted) = word(1).parse::<u16>() {
                port.0 = wanted;
                web.0 = wanted.checked_add(1);
            }
        }
        // Joins the game at an address, from the screen where a player types it, with a
        // password if one follows the address.
        "join" => {
            screen.set(Screen::Join);
            let (at, password) = (word(1).to_string(), words.get(2..).unwrap_or_default());
            session_requests.write(match password {
                [] => SessionRequest::Join(at),
                password => SessionRequest::JoinLocked(at, password.join(" ")),
            });
        }
        // Presses a key that writes nothing: `enter`, `tab` or `backspace`.
        "key" => {
            let key = match word(1) {
                "enter" => Some(Key::Enter),
                "tab" => Some(Key::Tab),
                "backspace" => Some(Key::Backspace),
                _ => None,
            };
            match key {
                Some(key) => {
                    keys.write(KeyboardInput {
                        key_code: KeyCode::Unidentified(NativeKeyCode::Unidentified),
                        logical_key: key,
                        state: ButtonState::Pressed,
                        text: None,
                        repeat: false,
                        window: Entity::PLACEHOLDER,
                    });
                }
                None => warn!("no such key: {}", word(1)),
            }
        }
        // Presses the keys that write a text, one after the other.
        "type" => {
            for letter in words[1..].join(" ").chars() {
                let letter = letter.to_string();
                keys.write(KeyboardInput {
                    key_code: KeyCode::Unidentified(NativeKeyCode::Unidentified),
                    logical_key: Key::Character(letter.as_str().into()),
                    state: ButtonState::Pressed,
                    text: Some(letter.as_str().into()),
                    repeat: false,
                    window: Entity::PLACEHOLDER,
                });
            }
        }
        // Sits the script at the place of another player of the run: the view and the
        // commands that follow are those of that seat.
        "seat" => seat.0 = number(1) as u8,
        // Comes back at the barrier of a sector, as after a death at that checkpoint.
        "warp" => {
            let sector = number(1) as u32;
            if let (Some(run), Some(field)) = (run.as_mut(), field.as_mut()) {
                run.checkpoint = sector;
                run.cleared = run.cleared.max(sector);
                run.phase = RunPhase::Respawning(0.0);
                field.cleared.extend(0..=sector);
            }
        }
        "turn" => view.turns = number(1) as i32,
        // Takes the dimensions and the hatching out of the void with `margin off`, and gives
        // them back with `margin on`.
        "margin" => *margin = Margin(word(1) != "off"),
        // Hides the interface, or shows it again with `bare off`: the game alone.
        "bare" => bare.0 = word(1) != "off",
        // Holds everything that wakes at one moment of its rise, in seconds since it woke.
        // Without a number, what wakes takes its time again.
        "lift" => *hold = Hold(word(1).parse().ok()),
        // Looks at the enemy with this number for a few seconds.
        "peek" => {
            if let Some((_, _, position)) = enemies.iter().nth(number(1) as usize) {
                view.look_at(position.0, 3.0);
            }
        }
        // Looks at a point of the map until the script ends.
        "look" => view.look_at(Vec2::new(number(1), number(2)), f32::MAX),
        // Shows this many meters of ground from the bottom to the top of the screen. The
        // game shows 30.
        "zoom" => view.span = Some(number(1)).filter(|span| *span > 0.0),
        // Lines up one tank for each level of a stat.
        "showcase" => {
            let Some(&(_, stat)) = STATS.iter().find(|(name, _)| *name == word(1)) else {
                warn!("unknown stat: {}", word(1));
                return;
            };
            let levels = (0..=stat.max_level()).map(|level| {
                let mut genome = Genome::empty(70);
                for (other, shown) in SHOWN_ON {
                    genome.set(other, shown);
                }
                // A bullet that explodes never bounces, so no tank that the game builds
                // has a ring behind its bell.
                if stat == Stat::Exploding {
                    genome.set(Stat::Ricochet, 0);
                }
                genome.set(stat, level);
                genome
            });
            display(&mut commands, &exhibits, levels);
        }
        // Lines up three tanks of each hue, from a weak one to a strong one.
        "palette" => display(&mut commands, &exhibits, palette()),
        // Puts the tanks on display to sleep: they are drawn as dormant enemies are.
        "doze" => {
            for exhibit in &exhibits {
                commands.entity(exhibit).insert(Enemy { awake: false });
            }
        }
        // Wakes the tanks on display again, as the enemies of a sector that a player enters.
        "rouse" => {
            for exhibit in &exhibits {
                commands.entity(exhibit).insert(Enemy { awake: true });
            }
        }
        // Raises the shield of every tank that has one, for ten seconds.
        "bubble" => {
            for (tank, mut shield) in &mut shields {
                if tank.genome.shield_cooldown().is_some() {
                    shield.state = ShieldState::Active(10.0);
                }
            }
        }
        "press" => script.press = Some(words[1..].join(" ")),
        "hover" => script.hover = Some(words[1..].join(" ")).filter(|label| !label.is_empty()),
        // Holds the repair key for this long, or the shield key.
        "repair" => script.repair = number(1),
        "shield" => script.shield = number(1),
        // Destroys the player's tank.
        "die" => casualties
            .tanks
            .extend(player.iter().map(|(entity, _)| entity)),
        // Destroys the tank of the player of a seat, a teammate's among several players.
        "wreck" => {
            let seat = number(1) as u8;
            let tanks = crew.iter().filter(|(_, player)| player.seat == seat);
            casualties.tanks.extend(tanks.map(|(entity, _)| entity));
        }
        // Destroys the enemies of the sector the player is in.
        "clear" => {
            let sector = run.as_ref().map_or(0, |run| run.sector);
            let doomed = enemies.iter().filter(|(_, tank, _)| tank.sector == sector);
            casualties.tanks.extend(doomed.map(|(entity, ..)| entity));
        }
        // Puts the tank this far east and north of the life that lies on the floor.
        "reach" => {
            let aside = Vec2::new(number(1), number(2));
            if let (Some(life), Ok((_, mut position))) = (lives.iter().next(), player.single_mut())
            {
                position.0 = life.0 + aside;
            }
        }
        // Gives a single player this many lives.
        "lives" => {
            if let Some(run) = run.as_mut() {
                run.lives = number(1) as u32;
            }
        }
        // Carries the tank through the barrier of the sector that the run has reached, if it
        // is unlocked, as a player who drives through it.
        "cross" => {
            let sector = run.as_ref().map_or(0, |run| run.sector);
            let gate = barriers.iter().find(|gate| gate.sector == sector);
            if let (Some(gate), Ok((_, mut position))) = (gate, player.single_mut()) {
                position.0 = (gate.from + gate.to) / 2.0 + gate.outward * 2.5;
            }
        }
        "play" => {
            let (drive, aim) = (
                Vec2::new(number(1), number(2)),
                Vec2::new(number(3), number(4)),
            );
            script.play = Some((drive, aim, number(5)));
        }
        // Turns the turret to a point relative to the tank, and keeps it there.
        "aim" => script.aim = Some(Vec2::new(number(1), number(2))),
        // Sets off a mine of the player at a point of the map, in the sector the player is in.
        "blast" => {
            if let (Ok((owner, _)), Some(run)) = (player.single(), run.as_ref()) {
                commands.spawn((
                    Mine {
                        owner,
                        team: Team::Players,
                        sector: run.sector,
                        state: MineState::Triggered(0.0),
                    },
                    Maker(Genome::PLAYER),
                    Position(Vec2::new(number(1), number(2))),
                    InSector(run.sector),
                ));
            }
        }
        // Sets off a mine of the player under each enemy of the sector the player is in: the
        // tanks that are there are destroyed, as the player would destroy them. In arena mode,
        // a tank that waits in reserve comes for each of them, as in any fight, and needs a
        // `rout` of its own.
        "rout" => {
            if let (Ok((owner, _)), Some(run)) = (player.single(), run.as_ref()) {
                let foes = enemies
                    .iter()
                    .filter(|(_, tank, _)| tank.sector == run.sector);
                for (_, _, at) in foes {
                    commands.spawn((
                        Mine {
                            owner,
                            team: Team::Players,
                            sector: run.sector,
                            state: MineState::Triggered(0.0),
                        },
                        Maker(Genome::PLAYER),
                        Position(at.0),
                        InSector(run.sector),
                    ));
                }
            }
        }
        // Sounds the warning of a mine at a point of the map. Nothing blows up.
        "warn" => {
            let at = Vec2::new(number(1), number(2));
            events.write(SimEvent::MineTriggered { at });
        }
        // Ends the run as the loss of the last life does. With a number, it was lost to the
        // bullet of the enemy with this number, among those that are loaded or on display.
        "lose" => {
            if let Some(run) = run.as_mut() {
                let foes = shields.iter().map(|(tank, _)| tank);
                let mut foes = foes.filter(|tank| tank.team == Team::Enemies);
                let enemy = word(1).parse::<usize>().ok();
                let enemy = enemy.and_then(|number| foes.nth(number));
                run.lost_to = enemy.map(|tank| Blow {
                    weapon: Weapon::Bullet,
                    genome: tank.genome,
                    team: Team::Enemies,
                });
                run.lives = 0;
                run.phase = RunPhase::Over;
                events.write(SimEvent::RunOver);
            }
        }
        // Rows of bullets that hardly move: players' then enemies', normal then piercing, with
        // no bounce left on the west side and the most on the east side. Then a row of bullets
        // that seek, and a bullet that splits next to one that explodes.
        "lineup" => {
            let (Ok((owner, position)), Some(run)) = (player.single(), run.as_ref()) else {
                return;
            };
            let position = *position;
            // It splits long after the script is over.
            let splitting = Kind::Splitting {
                fuse: f32::MAX,
                bounces: 1,
            };
            let most = Stat::Ricochet.max_level();
            let kinds = [
                (Team::Players, 0, Kind::Plain, most),
                (Team::Players, 1, Kind::Plain, most),
                (Team::Enemies, 0, Kind::Plain, most),
                (Team::Enemies, 1, Kind::Plain, most),
                (Team::Enemies, 0, Kind::Seeking, most),
                (Team::Enemies, 0, splitting, 0),
                (Team::Enemies, 0, Kind::Exploding, 0),
            ];
            for (row, (team, pierces_left, kind, most)) in kinds.into_iter().enumerate() {
                // The two that never bounce share a row.
                let (row, first) = if row == 6 { (5, 2) } else { (row, 0) };
                for bounces_left in 0..=most {
                    let column = (first + bounces_left) as f32;
                    let offset = Vec2::new(column * 1.6 - 2.4, 2.5 + row as f32 * 1.2);
                    commands.spawn((
                        Bullet {
                            owner,
                            team,
                            sector: run.sector,
                            velocity: Vec2::X * 0.01,
                            bounces_left,
                            pierces_left,
                            bounced: false,
                            kind,
                        },
                        Position(position.0 + offset),
                        PreviousPosition(position.0 + offset),
                        InSector(run.sector),
                    ));
                }
            }
        }
        // Sends a piercing bullet of the enemies at a bullet of the players, north of the tank.
        "pierce" => {
            let (Ok((owner, position)), Some(run)) = (player.single(), run.as_ref()) else {
                return;
            };
            let position = *position;
            let shots = [
                (Team::Enemies, 1, Vec2::new(-3.25, 3.0), Vec2::X * 3.0),
                (Team::Players, 0, Vec2::new(0.5, 3.0), Vec2::X * 0.01),
            ];
            for (team, pierces_left, offset, velocity) in shots {
                commands.spawn((
                    Bullet {
                        owner,
                        team,
                        sector: run.sector,
                        velocity,
                        bounces_left: 1,
                        pierces_left,
                        bounced: false,
                        kind: default(),
                    },
                    Position(position.0 + offset),
                    PreviousPosition(position.0 + offset),
                    InSector(run.sector),
                ));
            }
        }
        "pause" => overlay.set(Overlay::Pause),
        "settings" => overlay.set(Overlay::Settings),
        "resume" => overlay.set(Overlay::None),
        // Draws as a window of this size would, from here on. With a third number, the
        // window is on a screen of that scale: `canvas 1920 1080 1.5` is a screen of 1920 by
        // 1080 pixels whose system shows everything half as large again.
        "canvas" => {
            let size = UVec2::new(number(1) as u32, number(2) as u32).max(UVec2::ONE);
            let scale_factor = Some(number(3)).filter(|scale| *scale > 0.0);
            let (camera, target, projection) = &mut *camera;
            let Some(canvas) = target.as_image() else {
                return;
            };
            if let Some(mut image) = images.get_mut(canvas) {
                *image = blank(size);
            }
            commands
                .entity(*camera)
                .insert(RenderTarget::Image(ImageRenderTarget {
                    handle: canvas.clone(),
                    scale_factor: scale_factor.unwrap_or(1.0),
                }));
            // The engine only looks at the size of an image a frame after it changed. A
            // projection that changed makes it look at once, so that no frame is drawn for
            // the old size into the new image.
            projection.set_changed();
        }
        // The image is the camera's with its scale: one that is named without it is another
        // image to the engine, which nothing draws into.
        "shot" => {
            commands
                .spawn(Screenshot(camera.1.clone()))
                .observe(save_to_disk(std::path::PathBuf::from(word(1))));
            script.wait = 0.5;
        }
        // Keeps the music from here on. The piece starts with the tape.
        "tape" => commands.insert_resource(Tape {
            path: PathBuf::from(word(1)),
            music: baton.0.music(),
            samples: Vec::new(),
            started: time.elapsed_secs_f64(),
            told: Vec::new(),
        }),
        "quit" => {
            if let Some(tape) = &tape {
                tape.save();
            }
            exit.write(AppExit::Success);
        }
        other => warn!("unknown development command: {other}"),
    }
}
