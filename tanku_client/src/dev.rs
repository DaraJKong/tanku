//! Development tools, left out of release builds.
//!
//! `TANKU_DEV_SCRIPT` holds commands separated by semicolons, run one after the other:
//! `start SEED`, `wait SECONDS`, `warp SECTOR`, `turn QUARTERS`, `peek ENEMY`, `flag SECTOR`,
//! `look X Y`, `zoom METERS`, `clear`, `reach`, `bubble`, `pause`, `settings`, `resume`, `lose`,
//! `shot PATH` and `quit`, plus `play DX DY AX AY SECONDS`, which drives the tank in a direction
//! while it fires at a point relative to itself, `lineup`, which lays out one bullet of each
//! kind next to the tank, and `showcase STAT`, which lines up in the start bay one tank for
//! each level of a stat. It is how the game is looked at without anyone at the keyboard.
//!
//! A scripted run opens no window: it draws into an image, which `shot` saves. It makes no
//! sound, starts from the default settings and leaves the player's files alone.

use std::{collections::VecDeque, time::Duration};

use bevy::{
    app::ScheduleRunnerPlugin,
    camera::RenderTarget,
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, save_to_disk},
    },
    window::ExitCondition,
    winit::WinitPlugin,
};
use tanku_core::{
    sim::{
        Bullet, Enemy, Flag, Motion, Player, Position, PreviousPosition, Run, RunPhase, RunRequest,
        Shield, ShieldState, SimEvent, Tank, TankCommand, Team,
    },
    stats::{Genome, Stat},
};
use tanku_server::{
    Casualties,
    field::{Field, InSector},
};

use crate::{
    Overlay, Screen,
    audio::Muted,
    camera::{MainCamera, View},
    input, storage,
};

/// Size of the image that a scripted run draws into.
const CANVAS: (u32, u32) = (1920, 1080);

#[derive(Resource)]
struct Script {
    commands: VecDeque<String>,
    wait: f32,
    /// What the script makes the player's tank do, and for how long.
    play: Option<(Vec2, Vec2, f32)>,
}

/// The image that stands in for the window.
#[derive(Resource)]
struct Canvas(Handle<Image>);

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
    })
    .add_systems(PostStartup, draw_into_an_image)
    .add_systems(Update, run_script)
    .add_systems(PreUpdate, play.after(input::command_player));
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

/// Takes the controls of the player's tank while the script plays.
fn play(
    time: Res<Time<Real>>,
    mut script: ResMut<Script>,
    mut player: Query<(&Position, &mut TankCommand), With<Player>>,
) {
    let Some((drive, aim, left)) = script.play else {
        return;
    };
    script.play = (left > 0.0).then_some((drive, aim, left - time.delta_secs()));
    if let Ok((position, mut command)) = player.single_mut() {
        *command = TankCommand {
            drive,
            aim: position.0 + aim,
            fire: aim != Vec2::ZERO,
            mine: left % 2.0 < 0.1,
            shield: false,
        };
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
        Query<(Entity, &mut Position), (With<Player>, Without<Enemy>)>,
        Query<(&Flag, &Position), Without<Player>>,
        Query<Entity, With<Exhibit>>,
        Query<(&Tank, &mut Shield)>,
    ),
    canvas: Res<Canvas>,
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
            });
            screen.set(Screen::Playing);
        }
        "wait" => script.wait = number(1),
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
        // Lines up one tank for each level of a stat, in rows of six from the north of the
        // start bay. The tanks do nothing.
        "showcase" => {
            for exhibit in &exhibits {
                commands.entity(exhibit).despawn();
            }
            let Some(&(_, stat)) = STATS.iter().find(|(name, _)| *name == word(1)) else {
                warn!("unknown stat: {}", word(1));
                return;
            };
            for level in 0..=stat.max_level() {
                let mut genome = Genome::empty(70);
                for (other, shown) in SHOWN_ON {
                    genome.set(other, shown);
                }
                genome.set(stat, level);
                let (row, column) = (level / 6, level % 6);
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
        // Raises the shield of every tank that has one, for ten seconds.
        "bubble" => {
            for (tank, mut shield) in &mut shields {
                if tank.genome.shield().is_some() {
                    shield.state = ShieldState::Active(10.0);
                }
            }
        }
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
        // Ends the run as the loss of the last life does.
        "lose" => {
            if let Some(run) = run.as_mut() {
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
        "pause" => overlay.set(Overlay::Pause),
        "settings" => overlay.set(Overlay::Settings),
        "resume" => overlay.set(Overlay::None),
        "shot" => {
            commands
                .spawn(Screenshot::image(canvas.0.clone()))
                .observe(save_to_disk(std::path::PathBuf::from(word(1))));
            script.wait = 0.5;
        }
        "quit" => {
            exit.write(AppExit::Success);
        }
        other => warn!("unknown development command: {other}"),
    }
}
