//! Mines, explosive crates and their blasts.

use std::collections::VecDeque;

use bevy::prelude::*;
use tanku_core::{
    rules::{BLAST_RADIUS, BLOCK_SIZE, MINE_FUSE, MINE_TRIGGER_RADIUS, MINE_WARNING, TICK},
    sim::{
        Block, Bullet, Enemy, ExplosiveCrate, Mine, MineState, Position, Shield, SimEvent, Tank,
    },
};

use crate::{Casualties, field::Field};

/// Arms the mines, burns their fuses and triggers them when an opposing tank comes close.
pub fn tick(
    mut casualties: ResMut<Casualties>,
    mut mines: Query<(Entity, &mut Mine, &Position)>,
    tanks: Query<(&Tank, &Position, Option<&Enemy>)>,
    mut events: MessageWriter<SimEvent>,
) {
    for (entity, mut mine, position) in &mut mines {
        mine.state = match mine.state {
            MineState::Arming(left) if left > TICK => MineState::Arming(left - TICK),
            MineState::Arming(_) => MineState::Armed(MINE_FUSE),
            MineState::Armed(fuse) => {
                let approached = tanks.iter().any(|(tank, tank_position, enemy)| {
                    tank.team != mine.team
                        && enemy.is_none_or(|enemy| enemy.awake)
                        && tank_position.0.distance(position.0) <= MINE_TRIGGER_RADIUS
                });
                if approached {
                    events.write(SimEvent::MineTriggered { at: position.0 });
                    MineState::Triggered(MINE_WARNING)
                } else if fuse > TICK {
                    MineState::Armed(fuse - TICK)
                } else {
                    MineState::Triggered(0.0)
                }
            }
            MineState::Triggered(left) if left > TICK => MineState::Triggered(left - TICK),
            MineState::Triggered(_) => {
                casualties.detonations.push(entity);
                continue;
            }
        };
    }
}

/// Blows up every mine and crate that was set off this tick, and whatever their blasts set off
/// in turn.
pub fn blast(
    mut commands: Commands,
    mut field: ResMut<Field>,
    mut casualties: ResMut<Casualties>,
    mines: Query<(Entity, &Position), With<Mine>>,
    crates: Query<(Entity, &Position), With<ExplosiveCrate>>,
    blocks: Query<(Entity, &Position), With<Block>>,
    bullets: Query<(Entity, &Position), With<Bullet>>,
    tanks: Query<(Entity, &Position, &Shield), With<Tank>>,
    mut events: MessageWriter<SimEvent>,
) {
    let mut pending: VecDeque<Entity> = casualties.detonations.drain(..).collect();
    let mut blown: Vec<Entity> = Vec::new();
    let mut stale = false;
    while let Some(explosive) = pending.pop_front() {
        if blown.contains(&explosive) {
            continue;
        }
        let Ok((_, center)) = mines.get(explosive).or_else(|_| crates.get(explosive)) else {
            continue;
        };
        let center = center.0;
        blown.push(explosive);
        commands.entity(explosive).try_despawn();
        events.write(SimEvent::Blast { at: center });
        stale |= crates.contains(explosive);

        // Walls and barriers stop a blast. Blocks and crates don't: they are destroyed.
        let reaches = |target: Vec2, size: f32| {
            target.distance(center) <= BLAST_RADIUS + size
                && !field.blocked(center, target, |solid| solid.stops_blasts())
        };
        for (entity, position) in mines.iter().chain(&crates) {
            let size = if crates.contains(entity) {
                BLOCK_SIZE / 2.0
            } else {
                0.0
            };
            if reaches(position.0, size) {
                pending.push_back(entity);
            }
        }
        for (entity, position) in &blocks {
            if !blown.contains(&entity) && reaches(position.0, BLOCK_SIZE / 2.0) {
                blown.push(entity);
                commands.entity(entity).try_despawn();
                events.write(SimEvent::BlockDestroyed { at: position.0 });
                stale = true;
            }
        }
        for (entity, position) in &bullets {
            if reaches(position.0, 0.0) {
                commands.entity(entity).try_despawn();
            }
        }
        for (entity, position, shield) in &tanks {
            if !shield.is_active() && reaches(position.0, 0.0) {
                casualties.tanks.push(entity);
            }
        }
    }
    field.stale |= stale;
}
