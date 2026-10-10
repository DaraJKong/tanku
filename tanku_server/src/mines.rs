//! Mines, explosive crates and their blasts.

use std::collections::VecDeque;

use bevy::prelude::*;
use tanku_core::{
    rules::{BLAST_RADIUS, BLOCK_SIZE, MINE_FUSE, MINE_TRIGGER_RADIUS, MINE_WARNING, TICK},
    sim::{
        Block, Blow, Bullet, Enemy, ExplosiveCrate, Mine, MineState, Position, Protected, Shield,
        SimEvent, Tank, Team, Weapon, Wreck,
    },
    stats::Genome,
};

use crate::{Casualties, Maker, field::Field};

/// Arms the mines, burns their fuses and triggers them when an opposing tank comes close.
///
/// A mine warns before it blows, whether a tank set it off or its fuse is about to run out:
/// the last moments of a fuse are the same warning. Only a bullet or a blast sets a mine off
/// without one.
pub fn tick(
    mut casualties: ResMut<Casualties>,
    mut mines: Query<(Entity, &mut Mine, &Position)>,
    tanks: Query<(&Tank, &Position, Option<&Enemy>), Without<Wreck>>,
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
                if approached || fuse <= MINE_WARNING {
                    events.write(SimEvent::MineTriggered { at: position.0 });
                    // A fuse keeps the time it has left, so that it lasts as long as it says.
                    MineState::Triggered(fuse.min(MINE_WARNING))
                } else {
                    MineState::Armed(fuse - TICK)
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
///
/// A blast is the doing of a tank, which a tank it destroys is told. The blast of a mine is
/// the doing of the tank that laid it, whatever set the mine off. A crate belongs to nobody:
/// its blast is the doing of the tank whose bullet hit it, or of the tank behind the blast
/// that reached it.
pub fn blast(
    mut commands: Commands,
    mut field: ResMut<Field>,
    mut casualties: ResMut<Casualties>,
    mines: Query<(Entity, &Position, &Mine, Option<&Maker>)>,
    crates: Query<(Entity, &Position), With<ExplosiveCrate>>,
    blocks: Query<(Entity, &Position), With<Block>>,
    bullets: Query<(Entity, &Position), With<Bullet>>,
    tanks: Query<(Entity, &Position, &Shield), (With<Tank>, Without<Wreck>, Without<Protected>)>,
    mut events: MessageWriter<SimEvent>,
) {
    let culprits = std::mem::take(&mut casualties.culprits);
    let hand_behind = |explosive: Entity| {
        let mut hands = culprits.iter().filter(|(set_off, _)| *set_off == explosive);
        hands.next().map(|(_, culprit)| *culprit)
    };
    let mut pending: VecDeque<(Entity, Option<(Genome, Team)>)> = casualties
        .detonations
        .drain(..)
        .map(|explosive| (explosive, hand_behind(explosive)))
        .collect();
    let mut blown: Vec<Entity> = Vec::new();
    let mut stale = false;
    while let Some((explosive, hand)) = pending.pop_front() {
        if blown.contains(&explosive) {
            continue;
        }
        let (center, weapon, culprit) = if let Ok((_, position, mine, maker)) = mines.get(explosive)
        {
            let layer = maker.map(|maker| (maker.0, mine.team));
            (position.0, Weapon::Mine, layer)
        } else if let Ok((_, position)) = crates.get(explosive) {
            (position.0, Weapon::Crate, hand)
        } else {
            continue;
        };
        blown.push(explosive);
        commands.entity(explosive).try_despawn();
        events.write(SimEvent::Blast { at: center });
        stale |= crates.contains(explosive);

        // Walls and barriers stop a blast. Blocks and crates don't: they are destroyed.
        let reaches = |target: Vec2, size: f32| {
            target.distance(center) <= BLAST_RADIUS + size
                && !field.blocked(center, target, |solid| solid.stops_blasts())
        };
        for (entity, position, ..) in &mines {
            if reaches(position.0, 0.0) {
                pending.push_back((entity, culprit));
            }
        }
        for (entity, position) in &crates {
            if reaches(position.0, BLOCK_SIZE / 2.0) {
                pending.push_back((entity, culprit));
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
                if let Some((genome, team)) = culprit {
                    let blow = Blow {
                        weapon,
                        genome,
                        team,
                    };
                    casualties.blows.push((entity, blow));
                }
            }
        }
    }
    field.stale |= stale;
}
