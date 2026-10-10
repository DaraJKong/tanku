//! The life of a run: its start, the lives of the player and its end.

use bevy::prelude::*;
use tanku_core::{
    rules::{RESPAWN_DELAY, SOLO_LIVES, SPAWN_FIRE_DELAY, TICK},
    seed,
    sim::{
        Gun, Motion, Player, Position, PreviousPosition, Run, RunPhase, RunRequest, SimEvent, Tank,
        Team,
    },
    stats::Genome,
};

use crate::{
    Casualties,
    field::{self, Field, InSector},
};

/// Removes everything the simulation owns.
fn clear(
    commands: &mut Commands,
    entities: &Query<Entity, Or<(With<InSector>, With<Player>)>>,
    casualties: &mut Casualties,
) {
    for entity in entities {
        commands.entity(entity).try_despawn();
    }
    *casualties = Casualties::default();
}

/// Loads the corridor around a sector and puts the player in it.
fn enter(commands: &mut Commands, field: &mut Field, sector: u32) -> Vec2 {
    field.loaded.clear();
    field.load_entry_barrier(commands, sector);
    // The player comes back in the sector of the last flag captured.
    field.load(commands, sector, true);
    field.load(commands, sector + 1, false);

    let plan = field.corridor.plan(sector);
    // The tank appears between that flag and the barrier.
    let position = if sector == 0 {
        field::entry_position(plan)
    } else {
        plan.gate_point.as_vec2()
    };
    let facing = plan.heading.forward().as_vec2().to_angle();
    commands.spawn((
        Tank {
            genome: Genome::PLAYER,
            team: Team::Players,
            sector,
        },
        Player,
        Position(position),
        PreviousPosition(position),
        Motion {
            hull: facing,
            turret: facing,
            ..default()
        },
        // The click or the key that started the run must not fire a shot.
        Gun {
            cooldown: SPAWN_FIRE_DELAY,
            ..Gun::loaded(&Genome::PLAYER)
        },
    ));
    position
}

pub fn handle_requests(
    mut commands: Commands,
    mut requests: MessageReader<RunRequest>,
    entities: Query<Entity, Or<(With<InSector>, With<Player>)>>,
    mut casualties: ResMut<Casualties>,
    mut events: MessageWriter<SimEvent>,
) {
    for request in requests.read() {
        clear(&mut commands, &entities, &mut casualties);
        match request {
            RunRequest::Start { seed: seed_text } => {
                let seed = seed::seed_value(seed_text);
                let mut field = Field::new(seed);
                enter(&mut commands, &mut field, 0);
                commands.insert_resource(field);
                commands.insert_resource(Run {
                    seed_text: seed_text.trim().to_uppercase(),
                    seed,
                    sector: 0,
                    cleared: 0,
                    checkpoint: 0,
                    lives: SOLO_LIVES,
                    kills: 0,
                    ticks: 0,
                    phase: RunPhase::Playing,
                });
                events.write(SimEvent::RunStarted);
            }
            RunRequest::Abandon => {
                commands.remove_resource::<Run>();
                commands.remove_resource::<Field>();
            }
        }
    }
}

/// Removes the tanks that were destroyed this tick. Losing the player's tank costs a life.
pub fn casualties(
    mut commands: Commands,
    mut casualties: ResMut<Casualties>,
    mut run: ResMut<Run>,
    tanks: Query<(&Tank, &Position, Has<Player>)>,
    mut events: MessageWriter<SimEvent>,
) {
    casualties.tanks.sort();
    casualties.tanks.dedup();
    for entity in casualties.tanks.drain(..) {
        let Ok((tank, position, is_player)) = tanks.get(entity) else {
            continue;
        };
        commands.entity(entity).try_despawn();
        events.write(SimEvent::TankDestroyed {
            at: position.0,
            team: tank.team,
        });
        if !is_player {
            run.kills += 1;
        } else if run.phase == RunPhase::Playing {
            run.lives -= 1;
            run.phase = if run.lives == 0 {
                events.write(SimEvent::RunOver);
                RunPhase::Over
            } else {
                RunPhase::Respawning(RESPAWN_DELAY)
            };
        }
    }
}

/// Counts the time played and brings a destroyed player back at the last checkpoint.
pub fn clock(
    mut commands: Commands,
    mut run: ResMut<Run>,
    mut field: ResMut<Field>,
    entities: Query<Entity, Or<(With<InSector>, With<Player>)>>,
    mut casualties: ResMut<Casualties>,
    mut events: MessageWriter<SimEvent>,
) {
    run.ticks += 1;
    let RunPhase::Respawning(left) = run.phase else {
        return;
    };
    if left > TICK {
        run.phase = RunPhase::Respawning(left - TICK);
        return;
    }
    // Every sector beyond the checkpoint is regenerated from the seed.
    clear(&mut commands, &entities, &mut casualties);
    let checkpoint = run.checkpoint;
    field.cleared.retain(|&sector| sector <= checkpoint);
    let at = enter(&mut commands, &mut field, checkpoint);
    run.sector = checkpoint;
    run.phase = RunPhase::Playing;
    events.write(SimEvent::PlayerRespawned { at });
}
