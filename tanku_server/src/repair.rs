//! Wrecks and their repair, in a run of several players.

use bevy::prelude::*;
use tanku_core::{
    rules::{REPAIR_PROTECTION, REPAIR_RADIUS, REPAIR_TIME, TICK},
    sim::{Gun, Player, Position, Protected, Shield, SimEvent, Tank, TankCommand, Wreck},
};

/// The wreck that a tank at a place works on while its player holds the repair key: the one
/// it worked on a tick ago, for as long as that one is within reach, and else the nearest one
/// within reach.
///
/// A tank repairs one wreck at a time, however many lie within its reach, and it doesn't
/// change wrecks as it drives around: a repair that changes wrecks starts over.
fn pick(at: Vec2, before: Option<Entity>, wrecks: &[(Entity, Vec2)]) -> Option<Entity> {
    let within = || {
        let near = |(_, wreck): &&(Entity, Vec2)| wreck.distance(at) <= REPAIR_RADIUS;
        wrecks.iter().filter(near)
    };
    let kept = before.filter(|before| within().any(|(wreck, _)| wreck == before));
    kept.or_else(|| {
        let nearest = within().min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at)));
        nearest.map(|(wreck, _)| *wreck)
    })
}

/// Repairs the wrecks that a teammate works on from close by, and lets the protection of the
/// tanks that were just repaired run out.
///
/// A repair that is interrupted starts over. A repaired tank comes back with a full magazine
/// and a moment during which nothing destroys it. The tank that repairs does everything else
/// as usual meanwhile: it drives, shoots, lays mines and shields. Several tanks at one wreck
/// repair it no faster than one.
pub fn repair(
    mut commands: Commands,
    mut wrecks: Query<(Entity, &Tank, &Position, &mut Wreck, &mut Gun, &mut Shield)>,
    crews: Query<(&Player, &Position, &TankCommand), Without<Wreck>>,
    mut protected: Query<(Entity, &mut Protected)>,
    mut events: MessageWriter<SimEvent>,
) {
    // Which wreck each tank that repairs is at, by the seat of its player.
    let places: Vec<(Entity, Vec2)> = wrecks
        .iter()
        .map(|(entity, _, position, ..)| (entity, position.0))
        .collect();
    let mut at_work: Vec<(Entity, u8)> = Vec::new();
    for (player, position, command) in &crews {
        if !command.repair {
            continue;
        }
        let seat = 1 << player.seat;
        let mut before = wrecks
            .iter()
            .filter(|(.., wreck, _, _)| wreck.repairers & seat != 0);
        let before = before.next().map(|(entity, ..)| entity);
        if let Some(wreck) = pick(position.0, before, &places) {
            at_work.push((wreck, seat));
        }
    }
    for (entity, tank, position, mut wreck, mut gun, mut shield) in &mut wrecks {
        let seats = at_work.iter().filter(|(mended, _)| *mended == entity);
        let repairers = seats.fold(0, |all, (_, seat)| all | seat);
        // What doesn't change is left alone: guests are told of what was touched.
        if wreck.repairers != repairers {
            wreck.repairers = repairers;
        }
        if repairers == 0 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tank_picks_the_nearest_wreck_within_reach_and_stays_with_it() {
        let mut world = World::new();
        let [near, far, away] = [(); 3].map(|()| world.spawn_empty().id());
        let wrecks = [
            (near, Vec2::new(1.0, 0.0)),
            (far, Vec2::new(-2.5, 0.0)),
            (away, Vec2::new(0.0, REPAIR_RADIUS + 0.1)),
        ];
        let at = Vec2::ZERO;
        // The nearest of those within reach, and none when none is.
        assert_eq!(pick(at, None, &wrecks), Some(near));
        assert_eq!(pick(at, None, &wrecks[1..]), Some(far));
        assert_eq!(pick(at, None, &wrecks[2..]), None);
        assert_eq!(pick(at, None, &[]), None);
        // The one it is at already, though another is nearer, for as long as it is within
        // reach and still a wreck.
        assert_eq!(pick(at, Some(far), &wrecks), Some(far));
        assert_eq!(pick(at, Some(away), &wrecks), Some(near));
        assert_eq!(pick(at, Some(far), &wrecks[..1]), Some(near));
    }
}
