//! The life of a sector: dormant, fought, cleared, left behind.

use bevy::prelude::*;
use tanku_core::{
    geom,
    rules::{ARENA_DELAY, BARRIER_GATHER_DISTANCE, FLAG_CAPTURE_RADIUS, TANK_RADIUS},
    sim::{Barrier, Enemy, Flag, Player, Position, PreviousPosition, Run, SimEvent, Tank, Wreck},
};

use crate::{
    field::{self, Arena, Field, Fuse},
    run::formation,
};

pub fn advance(
    mut commands: Commands,
    mut field: ResMut<Field>,
    mut run: ResMut<Run>,
    mut players: Query<(
        &Player,
        &mut Tank,
        &mut Position,
        &mut PreviousPosition,
        Has<Wreck>,
    )>,
    mut enemies: Query<(&Tank, &mut Enemy), Without<Player>>,
    mut barriers: Query<&mut Barrier>,
    mut flags: Query<(&mut Flag, &Position), Without<Player>>,
    mut events: MessageWriter<SimEvent>,
) {
    let front = run.sector;
    // An arena is fought for as long as it holds enemies back.
    let fought = enemies.iter().any(|(tank, _)| tank.sector == front)
        || field.arenas.get(&front).is_some_and(Arena::holds_more);
    if !fought && field.cleared.insert(front) {
        run.cleared = run.cleared.max(front);
        events.write(SimEvent::SectorCleared { sector: front });
    }

    // What follows is the doing of the tanks that still run.
    let living: Vec<Vec2> = players
        .iter()
        .filter(|(.., wreck)| !wreck)
        .map(|(_, _, position, ..)| position.0)
        .collect();
    if living.is_empty() {
        return;
    }
    // A flag is captured by driving up to it, once the enemies of its sector are destroyed.
    for (mut flag, at) in &mut flags {
        let reached = |position: &Vec2| position.distance(at.0) <= FLAG_CAPTURE_RADIUS;
        if !flag.captured && living.iter().any(reached) && field.cleared.contains(&flag.sector) {
            flag.captured = true;
            run.checkpoint = run.checkpoint.max(flag.sector);
            events.write(SimEvent::FlagCaptured { at: at.0 });
        }
    }
    // The barrier of the sector that the players have reached unlocks once its enemies are
    // destroyed and every player whose tank still runs is close to it.
    if let Some(mut barrier) = barriers.iter_mut().find(|barrier| barrier.sector == front)
        && !barrier.unlocked
    {
        let close = |position: &Vec2| {
            let distance = geom::distance_to_segment(*position, barrier.from, barrier.to);
            distance <= BARRIER_GATHER_DISTANCE
        };
        if field.cleared.contains(&front) && living.iter().all(close) {
            barrier.unlocked = true;
            field.stale = true;
            events.write(SimEvent::BarrierUnlocked { sector: front });
        }
    }

    // A player is through an unlocked barrier once the whole tank is past it. The first one
    // through wakes the enemies of the sector behind it.
    let open: Vec<Barrier> = barriers
        .iter()
        .filter(|barrier| barrier.unlocked)
        .copied()
        .collect();
    for (_, mut tank, position, _, wreck) in &mut players {
        let through = |barrier: &&Barrier| {
            barrier.sector == tank.sector
                && (position.0 - barrier.from).dot(barrier.outward) > TANK_RADIUS
        };
        if wreck || open.iter().find(through).is_none() {
            continue;
        }
        tank.sector += 1;
        let entered = tank.sector;
        if entered > run.sector {
            run.sector = entered;
            for (enemy_tank, mut enemy) in &mut enemies {
                if enemy_tank.sector == entered {
                    enemy.awake = true;
                }
            }
            // An arena lets the players in before its first wave is announced.
            if let Some(arena) = field.arenas.get_mut(&entered) {
                arena.fuse = Fuse::Burning(ARENA_DELAY);
            }
            // The next sector waits behind its barrier.
            field.load(&mut commands, entered + 1, false);
            events.write(SimEvent::SectorEntered { sector: entered });
        }
    }

    // When the last player whose tank still runs has crossed, the wrecks left behind are towed
    // across and the sectors further back are unloaded. The sector just left stays in view.
    let running = players.iter().filter(|(.., wreck)| !wreck);
    let Some(rear) = running.map(|(_, tank, ..)| tank.sector).min() else {
        return;
    };
    for (player, mut tank, mut position, mut previous, wreck) in &mut players {
        if wreck && tank.sector < rear {
            let plan = field.corridor.plan(rear);
            let forward = plan.heading.forward().as_vec2();
            let at = formation(field::entry_position(plan), forward, player.seat, false);
            (position.0, previous.0, tank.sector) = (at, at, rear);
        }
    }
    field.loaded.retain(|&loaded| loaded + 1 >= rear);
}
