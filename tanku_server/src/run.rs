//! The life of a run: its start, the lives of a single player, the wrecks of several players
//! and its end.

use bevy::prelude::*;
use std::collections::BTreeMap;

use tanku_core::{
    rules::{MAX_PLAYERS, PLAYTEST_LAST_SECTOR, RESPAWN_DELAY, SOLO_LIVES, SPAWN_FIRE_DELAY, TICK},
    seed,
    sim::{
        Bullet, Enemy, Gun, Mine, Motion, Player, Playtest, Position, PreviousPosition, Protected,
        Run, RunPhase, RunRequest, Shield, SimEvent, Tank, TankCommand, Team, Wreck,
    },
};

use crate::{
    Casualties,
    brain::Brain,
    field::{self, Field, Fuse, Home, InSector, Recruit},
    room,
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

/// Loads the corridor around a sector and puts the players in it. In arena mode, opens the
/// room for a run that has cleared this many groups. Returns where the players stand.
fn enter(commands: &mut Commands, field: &mut Field, sector: u32) -> Vec2 {
    field.loaded.clear();
    field.arenas.clear();
    if field.room.is_some() {
        room::open(commands, field, sector);
    } else {
        field.load_entry_barrier(commands, sector);
        field.load(commands, sector);
        field.load(commands, sector + 1);
    }
    place_players(commands, field, sector)
}

/// Puts the tanks of the players in a sector that is loaded: in front of its barrier, or
/// where a run starts in the start bay. In arena mode, they stand in the middle of the room,
/// and the sector is the number of the group that is on. Returns where they stand.
fn place_players(commands: &mut Commands, field: &mut Field, sector: u32) -> Vec2 {
    let (middle, forward) = match &field.room {
        Some(state) => (state.room.start().as_vec2(), Vec2::Y),
        None => {
            let plan = field.corridor.plan(sector);
            let middle = if sector == 0 {
                field::entry_position(plan)
            } else {
                plan.gate_point.as_vec2()
            };
            (middle, plan.heading.forward().as_vec2())
        }
    };
    let facing = forward.to_angle();
    for &seat in &field.seats {
        let position = formation(middle, forward, seat, field.solo);
        let genome = field.tanks[seat as usize];
        commands.spawn((
            Tank {
                genome,
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
                ..Gun::loaded(&genome)
            },
        ));
    }
    middle
}

/// Puts the players in front of the barrier of the checkpoint, with the corridor loaded
/// afresh around them: every sector beyond the checkpoint is as the seed makes it. This is
/// how a playtest skips to a sector.
fn restart(
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
    playtest: Option<Res<Playtest>>,
    mut events: MessageWriter<SimEvent>,
) {
    for request in requests.read() {
        match request {
            RunRequest::Start {
                seed: seed_text,
                players,
                drawn,
                difficulty,
                free,
                arena,
                tanks,
            } => {
                clear(&mut commands, &entities, &mut casualties);
                let seed = seed::seed_value(seed_text);
                let players = (*players).clamp(1, MAX_PLAYERS);
                let mut field = if *arena {
                    Field::in_a_room(seed, players, *difficulty)
                } else {
                    Field::new(seed, players, *difficulty)
                };
                // A seat keeps the standard tank unless it was given one that a player may
                // have built: whoever asks may be wrong about what a tank is.
                for (seat, tank) in field.tanks.iter_mut().zip(tanks) {
                    if tank.is_a_players() {
                        *seat = *tank;
                    }
                }
                enter(&mut commands, &mut field, 0);
                commands.insert_resource(field);
                commands.insert_resource(Run {
                    seed_text: seed_text.trim().to_uppercase(),
                    seed,
                    difficulty: *difficulty,
                    players,
                    team: players,
                    solo: players == 1,
                    sector: 0,
                    cleared: 0,
                    checkpoint: 0,
                    lives: SOLO_LIVES,
                    kills: 0,
                    ticks: 0,
                    phase: RunPhase::Playing,
                    lost_to: None,
                    playtest: playtest.is_some(),
                    // A seed can't be practiced to unlock anything, and no tank can be
                    // built for it. What a run unlocks is earned in the corridor.
                    earns: *drawn && !*free && !*arena && playtest.is_none(),
                    free: *free,
                    arena: *arena,
                    lull: None,
                    paused: false,
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

/// In a playtest, puts the players in front of the sector that was asked for. In arena mode,
/// that is the lull before the group of that number.
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
    // The players come back at the end of the sector before it, as after clearing it.
    let checkpoint = sector.clamp(1, PLAYTEST_LAST_SECTOR) - 1;
    run.checkpoint = checkpoint;
    run.cleared = run.cleared.max(checkpoint);
    field.cleared.extend(0..=checkpoint);
    restart(
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

/// An enemy as its sector takes it back when a single player who fell returns.
type Returning<'a> = (
    Entity,
    &'a Tank,
    &'a Home,
    &'a mut Position,
    &'a mut PreviousPosition,
    &'a mut Motion,
    &'a mut Gun,
    &'a mut Shield,
    &'a mut Enemy,
    &'a mut Brain,
    &'a mut TankCommand,
);

/// Counts the time played and brings a destroyed single player back in front of the barrier
/// of the last sector cleared, which is where the sector that they fell in begins.
///
/// A sector keeps what was done to it: the enemies that were destroyed stay destroyed, and so
/// do the blocks and the crates. The enemies that are left go back to where they started, as
/// they were, and sleep until the player crosses the barrier again. An arena holds the rest
/// of the wave that was on back, to send it again. Nothing is left flying or lying in wait.
pub fn clock(
    mut commands: Commands,
    mut run: ResMut<Run>,
    mut field: ResMut<Field>,
    entities: Query<Entity, Or<(With<InSector>, With<Player>)>>,
    (players, weapons, mut enemies): (
        Query<(), With<Player>>,
        Query<Entity, Or<(With<Bullet>, With<Mine>)>>,
        Query<Returning, Without<Player>>,
    ),
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
    let checkpoint = run.checkpoint;
    // A tank that still stands, or a checkpoint that isn't loaded, is the doing of a playtest
    // or of a test: the corridor is loaded afresh.
    if !players.is_empty() || !field.loaded.contains(&checkpoint) {
        restart(
            &mut commands,
            &mut run,
            &mut field,
            &entities,
            &mut casualties,
            &mut events,
        );
        return;
    }
    for weapon in &weapons {
        commands.entity(weapon).try_despawn();
    }
    *casualties = Casualties::default();
    let mut held: BTreeMap<u32, Vec<Recruit>> = BTreeMap::new();
    for returning in &mut enemies {
        let (entity, tank, home, mut position, mut previous, mut motion, mut gun, ..) = returning;
        let (.., mut shield, mut enemy, mut brain, mut command) = returning;
        // What is left of the wave of an arena is held back again, to appear as it did.
        if field.arenas.contains_key(&tank.sector) {
            let recruit = Recruit {
                number: home.number,
                genome: tank.genome,
                at: home.at,
            };
            held.entry(tank.sector).or_default().push(recruit);
            commands.entity(entity).try_despawn();
            continue;
        }
        (position.0, previous.0) = (home.at, home.at);
        *motion = Motion {
            hull: home.hull,
            turret: home.turret,
            ..default()
        };
        *gun = Gun::loaded(&tank.genome);
        *shield = Shield::default();
        enemy.awake = false;
        *brain = field.brain(tank.sector, home.number).0;
        *command = TankCommand::default();
    }
    for (sector, arena) in field.arenas.iter_mut() {
        let mut wave = held.remove(sector).unwrap_or_default();
        wave.extend(arena.incoming.drain(..).map(|(recruit, _)| recruit));
        wave.sort_by_key(|recruit| recruit.number);
        if !wave.is_empty() {
            arena.waiting.push_front(wave);
        }
        arena.fuse = Fuse::Unlit;
    }
    // In arena mode, the player comes back in the middle of the room, to the group that
    // is on: what is left of it went back into reserve.
    let back = if field.room.is_some() {
        run.sector
    } else {
        checkpoint
    };
    let at = place_players(&mut commands, &mut field, back);
    run.sector = back;
    run.phase = RunPhase::Playing;
    events.write(SimEvent::PlayerRespawned { at });
}
