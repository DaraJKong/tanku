//! Wrecks and their repair, in a run of several players.

use bevy::prelude::*;
use tanku_core::{
    rules::{REPAIR_PROTECTION, REPAIR_RADIUS, REPAIR_TIME, TICK},
    sim::{Gun, Player, Position, Protected, Shield, SimEvent, Tank, TankCommand, Wreck},
};

/// The tanks that are repairing a wreck this tick. They can't shoot, place mines or shield.
#[derive(Resource, Default)]
pub struct Repairers(pub Vec<Entity>);

/// Repairs the wrecks that a teammate works on from close by, and lets the protection of the
/// tanks that were just repaired run out.
///
/// A repair that is interrupted starts over. A repaired tank comes back with a full magazine
/// and a moment during which nothing destroys it.
pub fn repair(
    mut commands: Commands,
    mut repairers: ResMut<Repairers>,
    mut wrecks: Query<(Entity, &Tank, &Position, &mut Wreck, &mut Gun, &mut Shield)>,
    crews: Query<(Entity, &Position, &TankCommand), (With<Player>, Without<Wreck>)>,
    mut protected: Query<(Entity, &mut Protected)>,
    mut events: MessageWriter<SimEvent>,
) {
    repairers.0.clear();
    for (entity, tank, position, mut wreck, mut gun, mut shield) in &mut wrecks {
        let mut at_work = false;
        for (crew, crew_position, command) in &crews {
            if command.repair && crew_position.0.distance(position.0) <= REPAIR_RADIUS {
                at_work = true;
                if !repairers.0.contains(&crew) {
                    repairers.0.push(crew);
                }
            }
        }
        if !at_work {
            if wreck.repair != 0.0 {
                wreck.repair = 0.0;
            }
            continue;
        }
        wreck.repair += TICK;
        if wreck.repair >= REPAIR_TIME {
            *gun = Gun::loaded(&tank.genome);
            *shield = Shield::default();
            commands
                .entity(entity)
                .remove::<Wreck>()
                .insert(Protected(REPAIR_PROTECTION));
            events.write(SimEvent::TankRepaired { at: position.0 });
        }
    }
    for (entity, mut protected) in &mut protected {
        protected.0 -= TICK;
        if protected.0 <= 0.0 {
            commands.entity(entity).remove::<Protected>();
        }
    }
}
