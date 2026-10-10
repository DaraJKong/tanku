//! Development tools, left out of release builds.
//!
//! `TANKU_DEV_SCRIPT` holds commands separated by semicolons, run one after the other:
//! `start SEED`, `wait SECONDS`, `warp SECTOR`, `turn QUARTERS`, `peek ENEMY`, `pause`,
//! `settings`, `resume`, `lose`, `shot PATH` and `quit`, plus `play DX DY AX AY SECONDS`, which
//! drives the tank in a direction while it fires at a point relative to itself, and `lineup`,
//! which lays out one bullet of each kind next to the tank. It is how the game is looked at
//! without anyone at the keyboard.

use std::collections::VecDeque;

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, save_to_disk},
};
use tanku_core::{
    sim::{
        Bullet, Enemy, Player, Position, PreviousPosition, Run, RunPhase, RunRequest, SimEvent,
        TankCommand, Team,
    },
    stats::Stat,
};
use tanku_server::field::{Field, InSector};

use crate::{Overlay, Screen, camera::View, input};

#[derive(Resource)]
struct Script {
    commands: VecDeque<String>,
    wait: f32,
    /// What the script makes the player's tank do, and for how long.
    play: Option<(Vec2, Vec2, f32)>,
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
    .add_systems(Update, run_script)
    .add_systems(PreUpdate, play.after(input::command_player));
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
    enemies: Query<&Position, With<Enemy>>,
    player: Query<(Entity, &Position), With<Player>>,
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
            if let Some(position) = enemies.iter().nth(number(1) as usize) {
                view.look_at(position.0, 3.0);
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
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(std::path::PathBuf::from(word(1))));
            script.wait = 0.5;
        }
        "quit" => {
            exit.write(AppExit::Success);
        }
        other => warn!("unknown development command: {other}"),
    }
}
