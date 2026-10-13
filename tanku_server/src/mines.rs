//! Mines, explosive crates and their blasts.

use std::collections::VecDeque;

use bevy::prelude::*;
use tanku_core::{
    rules::{BLAST_RADIUS, BLOCK_SIZE, MINE_FUSE, MINE_TRIGGER_RADIUS, MINE_WARNING, TICK},
    sim::{
        Block, Blow, Bullet, Enemy, ExplosiveCrate, Kind, Mine, MineState, Playtest, Position,
        Protected, Shield, SimEvent, Tank, Team, Weapon, Wreck,
    },
    stats::Genome,
};

use crate::{Casualties, Maker, field::Field, tanks};

/// Arms the mines, burns their fuses and triggers them when an opposing tank comes close.
///
/// A mine warns before it blows, whether a tank set it off or its fuse is about to run out:
/// the last moments of a fuse are the same warning. Only a bullet or a blast sets a mine off
/// without one.
pub fn tick(
    mut casualties: ResMut<Casualties>,
    mut mines: Query<(Entity, &mut Mine, &Position)>,
    tanks: Query<(Entity, &Tank, &Position, Option<&Enemy>), Without<Wreck>>,
    playtest: Option<Res<Playtest>>,
    mut events: MessageWriter<SimEvent>,
) {
    let indiscriminate = playtest.is_some_and(|playtest| playtest.indiscriminate);
    for (entity, mut mine, position) in &mut mines {
        mine.state = match mine.state {
            MineState::Arming(left) if left > TICK => MineState::Arming(left - TICK),
            MineState::Arming(_) => MineState::Armed(MINE_FUSE),
            MineState::Armed(fuse) => {
                let approached = tanks
                    .iter()
                    .any(|(tank_entity, tank, tank_position, enemy)| {
                        tanks::sets_off(&mine, tank_entity, tank.team, indiscriminate)
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

/// What goes off: a mine or a crate, or an exploding bullet at the place where it ended.
#[derive(Clone, Copy)]
enum Charge {
    Explosive(Entity),
    Bullet(Vec2),
}

/// Blows up every mine and crate that was set off this tick and every exploding bullet that
/// ended, and whatever their blasts set off in turn.
///
/// A blast is the doing of a tank, which a tank it destroys is told. The blast of a mine is
/// the doing of the tank that laid it, whatever set the mine off, and the blast of a bullet
/// is the doing of the tank that fired it. A crate belongs to nobody: its blast is the doing
/// of the tank whose bullet hit it, or of the tank behind the blast that reached it.
pub fn blast(
    mut commands: Commands,
    mut field: ResMut<Field>,
    mut casualties: ResMut<Casualties>,
    mines: Query<(Entity, &Position, &Mine, Option<&Maker>)>,
    crates: Query<(Entity, &Position), With<ExplosiveCrate>>,
    blocks: Query<(Entity, &Position), With<Block>>,
    bullets: Query<(Entity, &Position, &Bullet, Option<&Maker>)>,
    tanks: Query<(Entity, &Position, &Shield), (With<Tank>, Without<Wreck>, Without<Protected>)>,
    mut events: MessageWriter<SimEvent>,
) {
    let culprits = std::mem::take(&mut casualties.culprits);
    let hand_behind = |explosive: Entity| {
        let mut hands = culprits.iter().filter(|(set_off, _)| *set_off == explosive);
        hands.next().map(|(_, culprit)| *culprit)
    };
    let mut pending: VecDeque<(Charge, Option<(Genome, Team)>)> = casualties
        .detonations
        .drain(..)
        .map(|explosive| (Charge::Explosive(explosive), hand_behind(explosive)))
        .collect();
    let ended = casualties.blasts.drain(..);
    pending.extend(ended.map(|(at, shooter)| (Charge::Bullet(at), shooter)));
    let mut blown: Vec<Entity> = Vec::new();
    let mut stale = false;
    while let Some((charge, hand)) = pending.pop_front() {
        let (center, weapon, culprit) = match charge {
            Charge::Bullet(at) => (at, Weapon::Bullet, hand),
            Charge::Explosive(explosive) if blown.contains(&explosive) => continue,
            Charge::Explosive(explosive) => {
                let (center, weapon, culprit) =
                    if let Ok((_, position, mine, maker)) = mines.get(explosive) {
                        let layer = maker.map(|maker| (maker.0, mine.team));
                        (position.0, Weapon::Mine, layer)
                    } else if let Ok((_, position)) = crates.get(explosive) {
                        (position.0, Weapon::Crate, hand)
                    } else {
                        continue;
                    };
                blown.push(explosive);
                commands.entity(explosive).try_despawn();
                stale |= crates.contains(explosive);
                (center, weapon, culprit)
            }
        };
        events.write(SimEvent::Blast { at: center });

        // Walls and barriers stop a blast. Blocks and crates don't: they are destroyed.
        let reaches = |target: Vec2, size: f32| {
            target.distance(center) <= BLAST_RADIUS + size
                && !field.blocked(center, target, |solid| solid.stops_blasts())
        };
        for (entity, position, ..) in &mines {
            if reaches(position.0, 0.0) {
                pending.push_back((Charge::Explosive(entity), culprit));
            }
        }
        for (entity, position) in &crates {
            if reaches(position.0, BLOCK_SIZE / 2.0) {
                pending.push_back((Charge::Explosive(entity), culprit));
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
        for (entity, position, bullet, maker) in &bullets {
            if !blown.contains(&entity) && reaches(position.0, 0.0) {
                blown.push(entity);
                commands.entity(entity).try_despawn();
                // An exploding bullet that a blast destroys goes off in turn.
                if bullet.kind == Kind::Exploding {
                    let shooter = maker.map(|maker| (maker.0, bullet.team));
                    pending.push_back((Charge::Bullet(position.0), shooter));
                }
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
