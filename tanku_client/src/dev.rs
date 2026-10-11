//! Development tools, left out of release builds.
//!
//! `TANKU_DEV_SCRIPT` holds commands separated by semicolons, run one after the other:
//! `start SEED PLAYERS`, `seat NUMBER`, `repair SECONDS` and `shield SECONDS`, which hold the
//! key of that name for so long, `wait SECONDS`, `warp SECTOR`,
//! `host PORT`, which opens a game for others to join, and takes browsers on the port after
//! that one, `join ADDRESS`, which joins a game by its address or by what its host hands out
//! for browsers,
//! `type TEXT`, which presses the keys that write a text, `turn QUARTERS`, `peek ENEMY`,
//! `flag SECTOR`, `look X Y`, `zoom METERS`, `clear`, `reach`, `bubble`, `die`, `pause`,
//! `settings`, `resume`,
//! `lose ENEMY`, `blast X Y`, which sets off a mine at a point of the map, `warn X Y`, which
//! sounds the warning of a mine there without any mine, `shot PATH` and
//! `quit`, plus `play DX DY AX AY SECONDS`, which drives the tank in a direction while it fires
//! at a point relative to itself, `aim AX AY`, which turns the turret to such a point without
//! firing, `press LABEL`, which presses the button of a menu that bears
//! this label, in capitals or not, `lineup`, which lays out one bullet of each kind next to the
//! tank, `pierce`, which sends a piercing bullet at a bullet of the players there,
//! `showcase STAT`, which lines up in the start bay one tank for each level of a stat,
//! `palette`, which lines up there three tanks of each hue, from a weak one to a strong one,
//! `doze`, which puts the tanks that are lined up to sleep, and `tape PATH`, which keeps the
//! music from there on. It is how the game is looked at, and listened to, without anyone at
//! the keyboard.
//!
//! A scripted run opens no window: it draws into an image, which `shot` saves. It makes no
//! sound, starts from the default settings and leaves the player's files alone. The music
//! that it would play goes to a sound file when the script quits, if `tape` asked for it,
//! next to a text file that lists what the music was told and when.

use std::{collections::VecDeque, fmt::Write, path::PathBuf, time::Duration};

use bevy::{
    app::ScheduleRunnerPlugin,
    camera::RenderTarget,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput, NativeKeyCode},
    },
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
    sim::{
        Blow, Bullet, Enemy, Flag, Mine, MineState, Motion, Position, PreviousPosition, Run,
        RunPhase, RunRequest, Shield, ShieldState, SimEvent, Tank, TankCommand, Team, Weapon,
    },
    stats::{Genome, Stat},
};
use tanku_server::{
    Casualties, Maker,
    field::{Field, InSector},
};

use crate::{
    Overlay, Screen,
    audio::Muted,
    camera::{MainCamera, View},
    crew::{LocalSeat, Own},
    host::{HostPort, WebPort},
    input,
    music::Baton,
    session::SessionRequest,
    storage,
};

/// Size of the image that a scripted run draws into.
const CANVAS: (u32, u32) = (1920, 1080);

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
}

/// The image that stands in for the window.
#[derive(Resource)]
struct Canvas(Handle<Image>);

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
const STATS: [(&str, Stat); 12] = [
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

/// What `palette` gives its tanks so that each hue shows: most of the points go to mobility,
/// to firepower, to mines and shield, then to software.
const HUES: [&[(Stat, u8)]; 4] = [
    &[(Stat::Engine, 6), (Stat::Tracks, 4)],
    &[
        (Stat::Engine, 2),
        (Stat::Muzzle, 5),
        (Stat::Magazine, 3),
        (Stat::Loader, 4),
    ],
    &[(Stat::Engine, 2), (Stat::Mines, 4), (Stat::Shield, 5)],
    &[(Stat::Engine, 1), (Stat::Brain, 4)],
];

/// The budgets of the tanks of `palette`, from a pale tank to a saturated one.
const STRENGTHS: [u16; 3] = [15, 55, 100];

/// Three tanks of each hue, each with a sight and a pointed tip.
fn palette() -> impl Iterator<Item = Genome> {
    HUES.into_iter().flat_map(|levels| {
        STRENGTHS.map(|budget| {
            let mut genome = Genome::empty(budget);
            for &(stat, level) in levels {
                genome.set(stat, level);
            }
            genome.set(Stat::Piercing, 1);
            genome.set(Stat::Ballistics, 3);
            genome
        })
    })
}

/// Whether a script runs the game.
pub fn scripted() -> bool {
    std::env::var_os("TANKU_DEV_SCRIPT").is_some()
}

/// Adds the engine for a scripted run: no window, and frames at a steady pace.
pub fn add_engine(app: &mut App) {
    storage::keep_nothing();
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<WinitPlugin>(),
        ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / 60.0)),
    ))
    .insert_resource(Muted);
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
    })
    .add_systems(PostStartup, draw_into_an_image)
    .add_systems(Update, run_script)
    .add_systems(Last, roll_the_tape)
    .add_systems(
        PreUpdate,
        (
            play.after(input::command_player),
            press.after(UiSystems::Focus),
        ),
    );
}

/// Points the camera at an image, and the interface with it.
fn draw_into_an_image(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    camera: Single<Entity, With<MainCamera>>,
) {
    let canvas = images.add(Image::new_target_texture(
        CANVAS.0,
        CANVAS.1,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    commands
        .entity(*camera)
        .insert((RenderTarget::from(canvas.clone()), IsDefaultUiCamera));
    commands.insert_resource(Canvas(canvas));
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
/// that were there. The tanks do nothing.
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
        let at = Vec2::new(column as f32 * 2.2 - 5.5, 12.5 - row as f32 * 2.8);
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
    mut buttons: Query<(&mut Interaction, &Children), With<Button>>,
    labels: Query<&Text>,
) {
    let Some(label) = script.press.take() else {
        return;
    };
    let bears = |children: &Children| {
        let mut texts = children.iter().filter_map(|child| labels.get(child).ok());
        texts.any(|text| text.0.eq_ignore_ascii_case(&label))
    };
    match buttons.iter_mut().find(|(_, children)| bears(children)) {
        Some((mut interaction, _)) => *interaction = Interaction::Pressed,
        None => warn!("no button bears the label: {label}"),
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
    (enemies, mut player, flags, exhibits, mut shields): (
        Query<(Entity, &Tank, &Position), With<Enemy>>,
        Query<(Entity, &mut Position), (With<Own>, Without<Enemy>)>,
        Query<(&Flag, &Position), Without<Own>>,
        Query<Entity, With<Exhibit>>,
        Query<(&Tank, &mut Shield)>,
    ),
    canvas: Res<Canvas>,
    (baton, tape, mut seat): (Res<Baton>, Option<Res<Tape>>, ResMut<LocalSeat>),
    (mut session_requests, mut keys, mut port, mut web): (
        MessageWriter<SessionRequest>,
        MessageWriter<KeyboardInput>,
        ResMut<HostPort>,
        ResMut<WebPort>,
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
        "start" => {
            requests.write(RunRequest::Start {
                seed: word(1).to_string(),
                players: (number(2) as u32).max(1),
            });
            screen.set(Screen::Playing);
        }
        "wait" => script.wait = number(1),
        // Opens a game for others to join, on a port of its own if one is given. Browsers
        // are then taken on the port after it.
        "host" => {
            if let Ok(wanted) = word(1).parse::<u16>() {
                port.0 = wanted;
                web.0 = wanted.checked_add(1);
            }
            session_requests.write(SessionRequest::Host);
        }
        // Joins the game at an address, from the screen where a player types it.
        "join" => {
            screen.set(Screen::Join);
            session_requests.write(SessionRequest::Join(word(1).to_string()));
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
        // Looks at the enemy with this number for a few seconds.
        "peek" => {
            if let Some((_, _, position)) = enemies.iter().nth(number(1) as usize) {
                view.look_at(position.0, 3.0);
            }
        }
        // Looks at the flag of a loaded sector for a few seconds.
        "flag" => {
            let sector = number(1) as u32;
            if let Some((_, position)) = flags.iter().find(|(flag, _)| flag.sector == sector) {
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
        // Raises the shield of every tank that has one, for ten seconds.
        "bubble" => {
            for (tank, mut shield) in &mut shields {
                if tank.genome.shield_cooldown().is_some() {
                    shield.state = ShieldState::Active(10.0);
                }
            }
        }
        "press" => script.press = Some(words[1..].join(" ")),
        // Holds the repair key for this long, or the shield key.
        "repair" => script.repair = number(1),
        "shield" => script.shield = number(1),
        // Destroys the player's tank.
        "die" => casualties
            .tanks
            .extend(player.iter().map(|(entity, _)| entity)),
        // Destroys the enemies of the sector the player is in.
        "clear" => {
            let sector = run.as_ref().map_or(0, |run| run.sector);
            let doomed = enemies.iter().filter(|(_, tank, _)| tank.sector == sector);
            casualties.tanks.extend(doomed.map(|(entity, ..)| entity));
        }
        // Puts the tank next to the flag of its sector.
        "reach" => {
            let sector = run.as_ref().map_or(0, |run| run.sector);
            let flag = flags.iter().find(|(flag, _)| flag.sector == sector);
            if let (Some((_, flag)), Ok((_, mut position))) = (flag, player.single_mut()) {
                position.0 = flag.0 + Vec2::X;
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
        // no bounce left on the west side and the most on the east side.
        "lineup" => {
            let (Ok((owner, position)), Some(run)) = (player.single(), run.as_ref()) else {
                return;
            };
            let position = *position;
            let kinds = [
                (Team::Players, 0),
                (Team::Players, 1),
                (Team::Enemies, 0),
                (Team::Enemies, 1),
            ];
            for (row, (team, pierces_left)) in kinds.into_iter().enumerate() {
                for bounces_left in 0..=Stat::Ricochet.max_level() {
                    let offset = Vec2::new(bounces_left as f32 * 1.6 - 2.4, 2.5 + row as f32 * 1.2);
                    commands.spawn((
                        Bullet {
                            owner,
                            team,
                            sector: run.sector,
                            velocity: Vec2::X * 0.01,
                            bounces_left,
                            pierces_left,
                            bounced: false,
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
        "shot" => {
            commands
                .spawn(Screenshot::image(canvas.0.clone()))
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
