//! The life of a run: its start, the lives of a single player, the wrecks of several players
//! and its end.

use bevy::prelude::*;
use tanku_core::{
    rules::{MAX_PLAYERS, PLAYTEST_LAST_SECTOR, RESPAWN_DELAY, SOLO_LIVES, SPAWN_FIRE_DELAY, TICK},
    seed,
    sim::{
        Gun, Motion, Player, Playtest, Position, PreviousPosition, Protected, Run, RunPhase,
        RunRequest, Shield, SimEvent, Tank, Team, Wreck,
    },
    stats::Genome,
};

use crate::{
    Casualties,
    field::{self, Field, InSector},
};

/// How far apart the tanks of several players stand, side by side and row behind row.
const FORMATION_SPACING: f32 = 2.5;

/// Where the tank of a seat stands around a point: a single player on the point itself, several
/// players in rows of two, the later seats behind the first ones.
pub fn formation(middle: Vec2, forward: Vec2, seat: u8, solo: bool) -> Vec2 {
    if solo {
        return middle;
    }
    let across = Vec2::new(forward.y, -forward.x);
    let (side, row) = ((seat % 2) as f32 - 0.5, (seat / 2) as f32);
    middle + (across * side - forward * row) * FORMATION_SPACING
}

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

/// Loads the corridor around a sector and puts the players in it. Returns where they stand.
fn enter(commands: &mut Commands, field: &mut Field, sector: u32) -> Vec2 {
    field.loaded.clear();
    field.load_entry_barrier(commands, sector);
    // A player comes back in the sector of the last flag captured.
    field.load(commands, sector, true);
    field.load(commands, sector + 1, false);

    let plan = field.corridor.plan(sector);
    // The tanks appear between that flag and the barrier.
    let middle = if sector == 0 {
        field::entry_position(plan)
    } else {
        plan.gate_point.as_vec2()
    };
    let forward = plan.heading.forward().as_vec2();
    let facing = forward.to_angle();
    for &seat in &field.seats {
        let position = formation(middle, forward, seat, field.solo);
        commands.spawn((
            Tank {
                genome: Genome::PLAYER,
                team: Team::Players,
                sector,
            },
            Player { seat },
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
    }
    middle
}

/// Brings the players back at the checkpoint. Every sector beyond it is regenerated from the
/// seed.
fn respawn(
    commands: &mut Commands,
    run: &mut Run,
    field: &mut Field,
    entities: &Query<Entity, Or<(With<InSector>, With<Player>)>>,
    casualties: &mut Casualties,
    events: &mut MessageWriter<SimEvent>,
) {
    clear(commands, entities, casualties);
    let checkpoint = run.checkpoint;
    field.cleared.retain(|&sector| sector <= checkpoint);
    let at = enter(commands, field, checkpoint);
    run.sector = checkpoint;
    run.phase = RunPhase::Playing;
    events.write(SimEvent::PlayerRespawned { at });
}

pub fn handle_requests(
    mut commands: Commands,
    mut requests: MessageReader<RunRequest>,
    entities: Query<Entity, Or<(With<InSector>, With<Player>)>>,
    mut casualties: ResMut<Casualties>,
    mut events: MessageWriter<SimEvent>,
) {
    for request in requests.read() {
        match request {
            RunRequest::Start {
                seed: seed_text,
                players,
            } => {
                clear(&mut commands, &entities, &mut casualties);
                let seed = seed::seed_value(seed_text);
                let players = (*players).clamp(1, MAX_PLAYERS);
                let mut field = Field::new(seed, players);
                enter(&mut commands, &mut field, 0);
                commands.insert_resource(field);
                commands.insert_resource(Run {
                    seed_text: seed_text.trim().to_uppercase(),
                    seed,
                    players,
                    solo: players == 1,
                    sector: 0,
                    cleared: 0,
                    checkpoint: 0,
                    lives: SOLO_LIVES,
                    kills: 0,
                    ticks: 0,
                    phase: RunPhase::Playing,
                    lost_to: None,
                });
                events.write(SimEvent::RunStarted);
            }
            RunRequest::Abandon => {
                clear(&mut commands, &entities, &mut casualties);
                commands.remove_resource::<Run>();
                commands.remove_resource::<Field>();
            }
            // Left to `leave`, and to `skip`, which only a playtest runs.
            RunRequest::Leave { .. } | RunRequest::SkipTo { .. } => {}
        }
    }
}

/// In a playtest, puts the players in front of the sector that was asked for.
pub fn skip(
    mut commands: Commands,
    mut requests: MessageReader<RunRequest>,
    mut run: ResMut<Run>,
    mut field: ResMut<Field>,
    entities: Query<Entity, Or<(With<InSector>, With<Player>)>>,
    mut casualties: ResMut<Casualties>,
    mut events: MessageWriter<SimEvent>,
) {
    let asked = requests.read().filter_map(|request| match request {
        RunRequest::SkipTo { sector } => Some(*sector),
        _ => None,
    });
    // The last request wins.
    let Some(sector) = asked.last() else {
        return;
    };
    // The players come back at the end of the sector before it, as after capturing a flag
    // there.
    let checkpoint = sector.clamp(1, PLAYTEST_LAST_SECTOR) - 1;
    run.checkpoint = checkpoint;
    run.cleared = run.cleared.max(checkpoint);
    field.cleared.extend(0..=checkpoint);
    respawn(
        &mut commands,
        &mut run,
        &mut field,
        &entities,
        &mut casualties,
        &mut events,
    );
}

/// Takes the tanks of the players who left out of a run of several players. Later sectors are
/// loaded for those who stay. The run is over when none of them has a tank that still runs.
pub fn leave(
    mut commands: Commands,
    mut requests: MessageReader<RunRequest>,
    mut run: ResMut<Run>,
    mut field: ResMut<Field>,
    players: Query<(Entity, &Player, Has<Wreck>)>,
    mut events: MessageWriter<SimEvent>,
) {
    let mut gone: Vec<Entity> = Vec::new();
    for request in requests.read() {
        let RunRequest::Leave { seat } = request else {
            continue;
        };
        let leaver = players.iter().find(|(_, player, _)| player.seat == *seat);
        let Some((entity, ..)) = leaver.filter(|(entity, ..)| !gone.contains(entity)) else {
            continue;
        };
        if run.solo {
            continue;
        }
        gone.push(entity);
        commands.entity(entity).try_despawn();
        field.seats.retain(|taken| taken != seat);
        run.players = field.players();
    }
    let stays = |(entity, _, wreck): (Entity, &Player, bool)| !wreck && !gone.contains(&entity);
    if !gone.is_empty() && !players.iter().any(stays) && run.phase != RunPhase::Over {
        run.phase = RunPhase::Over;
        events.write(SimEvent::RunOver);
    }
}

/// Removes the tanks that were destroyed this tick. A single player loses a life with their
/// tank. Among several players, the tank becomes a wreck, and the run is over when all the
/// tanks of the players are wrecks.
pub fn casualties(
    mut commands: Commands,
    mut casualties: ResMut<Casualties>,
    mut run: ResMut<Run>,
    playtest: Option<Res<Playtest>>,
    mut tanks: Query<(
        &Tank,
        &Position,
        &mut PreviousPosition,
        &mut Motion,
        &mut Shield,
        Has<Player>,
        Has<Wreck>,
        Has<Protected>,
    )>,
    players: Query<(Entity, Has<Wreck>), With<Player>>,
    mut events: MessageWriter<SimEvent>,
) {
    let infinite_lives = playtest.is_some_and(|playtest| playtest.infinite_lives);
    casualties.tanks.sort();
    casualties.tanks.dedup();
    let blows = std::mem::take(&mut casualties.blows);
    let mut wrecked: Vec<Entity> = Vec::new();
    for entity in casualties.tanks.drain(..) {
        let Ok((tank, position, mut previous, mut motion, mut shield, is_player, wreck, safe)) =
            tanks.get_mut(entity)
        else {
            continue;
        };
        // Nothing destroys a wreck again, or a tank that was just repaired.
        if wreck || safe {
            continue;
        }
        events.write(SimEvent::TankDestroyed {
            at: position.0,
            team: tank.team,
        });
        if !is_player {
            commands.entity(entity).try_despawn();
            run.kills += 1;
            continue;
        }
        // The first blow that reached the tank this tick is the one that destroyed it.
        let mut reached = blows.iter().filter(|(victim, _)| *victim == entity);
        let blow = reached.next().map(|(_, blow)| *blow);
        if !run.solo {
            // The wreck stays where the tank was.
            previous.0 = position.0;
            motion.velocity = Vec2::ZERO;
            *shield = Shield::default();
            commands.entity(entity).insert(Wreck::default());
            wrecked.push(entity);
            run.lost_to = blow;
            continue;
        }
        commands.entity(entity).try_despawn();
        if run.phase == RunPhase::Playing {
            run.lost_to = blow;
            if !infinite_lives {
                run.lives -= 1;
            }
            run.phase = if run.lives == 0 {
                events.write(SimEvent::RunOver);
                RunPhase::Over
            } else {
                RunPhase::Respawning(RESPAWN_DELAY)
            };
        }
    }
    let running = |(entity, wreck): (Entity, bool)| !wreck && !wrecked.contains(&entity);
    if !wrecked.is_empty() && !players.iter().any(running) {
        run.phase = RunPhase::Over;
        events.write(SimEvent::RunOver);
    }
}

/// Counts the time played and brings a destroyed single player back at the last checkpoint.
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
    respawn(
        &mut commands,
        &mut run,
        &mut field,
        &entities,
        &mut casualties,
        &mut events,
    );
}
