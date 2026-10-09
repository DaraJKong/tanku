//! How bullets fly, bounce and hit.

use bevy::prelude::*;
use tanku_core::{
    geom,
    rules::{BULLET_RADIUS, MINE_RADIUS, SHIELD_RADIUS, TANK_RADIUS, TICK},
    sim::{Bullet, Mine, Position, PreviousPosition, Shield, SimEvent, Tank},
};

use crate::{
    Casualties,
    field::{Field, Solid},
    tanks,
};

/// How a stretch of flight ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Flight {
    Flying,
    /// The bullet hit something it doesn't bounce off, or it had no bounce left.
    Stopped(Solid),
}

pub struct FlightReport {
    pub end: Flight,
    pub bounces: Vec<Vec2>,
}

/// Moves a bullet by `distance`, bouncing off walls and blocks. Tanks and mines are ignored.
pub fn fly_past_walls(
    field: &Field,
    position: &mut Vec2,
    bullet: &mut Bullet,
    mut distance: f32,
) -> FlightReport {
    let mut bounces = Vec::new();
    // A bullet in a corner bounces twice in one tick, rarely more.
    for _ in 0..4 {
        let direction = bullet.velocity.normalize_or_zero();
        let delta = direction * distance;
        let Some((hit, solid)) = field.cast(*position, delta, BULLET_RADIUS) else {
            *position += delta;
            break;
        };
        *position += delta * hit.t;
        if !solid.bounces() || bullet.bounces_left == 0 {
            return FlightReport {
                end: Flight::Stopped(solid),
                bounces,
            };
        }
        bullet.bounces_left -= 1;
        bullet.bounced = true;
        bullet.velocity = geom::reflect(bullet.velocity, hit.normal);
        distance *= 1.0 - hit.t;
        bounces.push(*position);
    }
    FlightReport {
        end: Flight::Flying,
        bounces,
    }
}

/// What a bullet can run into, besides walls.
enum Target {
    Tank(Entity),
    Shield(Vec2),
    Mine(Entity),
}

pub fn fly(
    mut commands: Commands,
    field: Res<Field>,
    mut casualties: ResMut<Casualties>,
    mut bullets: Query<(Entity, &mut Bullet, &mut Position, &mut PreviousPosition)>,
    tanks: Query<(Entity, &Tank, &Position, &Shield), Without<Bullet>>,
    mines: Query<(Entity, &Position), (With<Mine>, Without<Bullet>)>,
    mut events: MessageWriter<SimEvent>,
) {
    for (entity, mut bullet, mut position, mut previous) in &mut bullets {
        previous.0 = position.0;
        let start = position.0;
        let before = *bullet;
        let distance = bullet.velocity.length() * TICK;
        let report = fly_past_walls(&field, &mut position.0, &mut bullet, distance);

        // The first tank, shield or mine along the path, which bends at every bounce.
        let mut path = vec![start];
        path.extend(&report.bounces);
        path.push(position.0);
        let mut struck: Option<(usize, Vec2, Target)> = None;
        for (leg, ends) in path.windows(2).enumerate() {
            let (from, delta) = (ends[0], ends[1] - ends[0]);
            let flying = Bullet {
                bounced: before.bounced || leg > 0,
                ..before
            };
            let mut first: Option<(f32, Target)> = None;
            let mut consider = |t: Option<f32>, target: Target| {
                if let Some(t) = t
                    && first.as_ref().is_none_or(|(best, _)| t < *best)
                {
                    first = Some((t, target));
                }
            };
            for (tank_entity, tank, tank_position, shield) in &tanks {
                let reach = |radius: f32| {
                    geom::sweep_circle_point(from, delta, tank_position.0, radius + BULLET_RADIUS)
                };
                if shield.is_active() && tank_entity != flying.owner {
                    consider(reach(SHIELD_RADIUS), Target::Shield(tank_position.0));
                } else if tanks::hits(&flying, tank_entity, tank.team) {
                    consider(reach(TANK_RADIUS), Target::Tank(tank_entity));
                }
            }
            for (mine_entity, mine_position) in &mines {
                let reach = MINE_RADIUS + BULLET_RADIUS;
                let t = geom::sweep_circle_point(from, delta, mine_position.0, reach);
                consider(t, Target::Mine(mine_entity));
            }
            if let Some((t, target)) = first {
                struck = Some((leg, from + delta * t, target));
                break;
            }
        }

        if let Some((leg, at, target)) = struck {
            for bounce in report.bounces.iter().take(leg) {
                events.write(SimEvent::Ricochet { at: *bounce });
            }
            match target {
                Target::Tank(tank) => casualties.tanks.push(tank),
                Target::Shield(center) => {
                    events.write(SimEvent::ShieldBlocked { at: center });
                }
                Target::Mine(mine) => casualties.detonations.push(mine),
            }
            events.write(SimEvent::BulletDestroyed { at });
            commands.entity(entity).try_despawn();
            continue;
        }

        for bounce in report.bounces {
            events.write(SimEvent::Ricochet { at: bounce });
        }
        if let Flight::Stopped(solid) = report.end {
            if let Solid::Crate(explosive) = solid {
                casualties.detonations.push(explosive);
            }
            events.write(SimEvent::BulletDestroyed { at: position.0 });
            commands.entity(entity).try_despawn();
        }
    }
}

/// Any two bullets that touch destroy each other, unless exactly one of them pierces.
pub fn collide(
    mut commands: Commands,
    bullets: Query<(Entity, &Bullet, &Position, &PreviousPosition)>,
    mut events: MessageWriter<SimEvent>,
) {
    let mut destroyed: Vec<Entity> = Vec::new();
    for [a, b] in bullets.iter_combinations() {
        if destroyed.contains(&a.0) || destroyed.contains(&b.0) {
            continue;
        }
        let distance = geom::closest_approach(a.3.0, a.2.0, b.3.0, b.2.0);
        if distance > 2.0 * BULLET_RADIUS {
            continue;
        }
        for (this, other) in [(a, b), (b, a)] {
            if !this.1.piercing || other.1.piercing {
                destroyed.push(this.0);
                events.write(SimEvent::BulletDestroyed { at: this.2.0 });
            }
        }
    }
    for entity in destroyed {
        commands.entity(entity).try_despawn();
    }
}
