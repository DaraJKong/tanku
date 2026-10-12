//! How bullets fly, bounce and hit.

use bevy::prelude::*;
use tanku_core::{
    geom,
    rules::{BULLET_RADIUS, MARK_TIME, MINE_RADIUS, SHIELD_RADIUS, SPLIT_ANGLE, TANK_RADIUS, TICK},
    sim::{
        Blow, Bullet, Enemy, Kind, Mine, Position, PreviousPosition, Protected, Shield, SimEvent,
        Tank, Team, Weapon, Wreck,
    },
    stats::Genome,
};

use crate::{
    Casualties, Maker,
    field::{Field, InSector, Solid},
    tanks,
};

/// How far from the place of the split each of the two bullets of a splitting bullet starts:
/// far enough from the other for the two not to touch, and not to destroy each other there.
const SPLIT_APART: f32 = 0.5;

/// A tank that a seeking bullet may leave a wall toward: where it is, and whose side it is
/// on. A wreck is none, and neither is an enemy that is still dormant.
pub type Quarry = (Vec2, Team);

/// Where a seeking bullet of a side goes when it bounces at a place: toward the nearest tank
/// in sight of that place that its side fights, if there is one.
pub fn sought(field: &Field, at: Vec2, team: Team, quarry: &[Quarry]) -> Option<Vec2> {
    quarry
        .iter()
        .filter(|(place, side)| *side != team && field.sees(at, *place))
        .map(|(place, _)| *place)
        .min_by(|a, b| a.distance_squared(at).total_cmp(&b.distance_squared(at)))
}

/// The way that a bullet leaves a wall or a block that it bounces off at a place. A seeking
/// bullet leaves toward the tank that it seeks, and any other at the mirror angle, as a
/// seeking one does when it sees no tank to seek.
pub fn rebound(field: &Field, bullet: &Bullet, at: Vec2, normal: Vec2, quarry: &[Quarry]) -> Vec2 {
    let toward = (bullet.kind == Kind::Seeking)
        .then(|| sought(field, at, bullet.team, quarry))
        .flatten()
        .and_then(|target| (target - at).try_normalize());
    match toward {
        Some(direction) => direction * bullet.velocity.length(),
        None => geom::reflect(bullet.velocity, normal),
    }
}

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

/// Moves a bullet by `distance`, bouncing off walls and blocks. Tanks and mines are ignored,
/// but for the ones that a seeking bullet leaves a wall toward.
pub fn fly_past_walls(
    field: &Field,
    position: &mut Vec2,
    bullet: &mut Bullet,
    mut distance: f32,
    quarry: &[Quarry],
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
        bullet.velocity = rebound(field, bullet, *position, hit.normal, quarry);
        distance *= 1.0 - hit.t;
        bounces.push(*position);
    }
    FlightReport {
        end: Flight::Flying,
        bounces,
    }
}

/// What a bullet does to the solid thing that stopped it: it sets off a crate, and the blast is
/// the doing of the tank that fired the bullet, when that tank is known.
pub fn strike(casualties: &mut Casualties, solid: Solid, shooter: Option<(Genome, Team)>) {
    if let Solid::Crate(explosive) = solid {
        casualties.detonations.push(explosive);
        if let Some(shooter) = shooter {
            casualties.culprits.push((explosive, shooter));
        }
    }
}

/// What a bullet is about to do, which is drawn in dashes ahead of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    /// An exploding bullet ends at this place, on something that stands still, and goes off.
    Blast { at: Vec2 },
    /// A seeking bullet bounces at this place, and leaves toward the tank that is at the other
    /// one. It seeks none when no tank that its side fights is in sight of the bounce.
    Bounce { at: Vec2, toward: Option<Vec2> },
}

/// What a bullet that is at a place is about to do, if that is marked: the end of an
/// exploding bullet on a wall, a block, a crate or a barrier, and the next bounce of a seeking
/// one, each when the bullet gets there within the time that such a thing is marked ahead.
/// Tanks, mines and bullets that are in the way are not looked at: they move.
pub fn mark(field: &Field, bullet: &Bullet, at: Vec2, quarry: &[Quarry]) -> Option<Mark> {
    let seeks = bullet.kind == Kind::Seeking && bullet.bounces_left > 0;
    if bullet.kind != Kind::Exploding && !seeks {
        return None;
    }
    let delta = bullet.velocity * MARK_TIME;
    let (hit, solid) = field.cast(at, delta, BULLET_RADIUS)?;
    let end = at + delta * hit.t;
    if !seeks {
        return Some(Mark::Blast { at: end });
    }
    let toward = sought(field, end, bullet.team, quarry);
    solid.bounces().then_some(Mark::Bounce { at: end, toward })
}

/// An exploding bullet goes off where it ends, whatever ended it. Its blast is the doing of
/// the tank that fired it, when that tank is known.
pub fn go_off(
    casualties: &mut Casualties,
    bullet: &Bullet,
    at: Vec2,
    shooter: Option<(Genome, Team)>,
) {
    if bullet.kind == Kind::Exploding {
        casualties.blasts.push((at, shooter));
    }
}

/// The tanks that a seeking bullet may seek: the ones that run, and are awake.
pub fn quarry<'a>(
    tanks: impl Iterator<Item = (&'a Tank, &'a Position, Option<&'a Enemy>, bool)>,
) -> Vec<Quarry> {
    tanks
        .filter(|(_, _, enemy, wreck)| !wreck && enemy.is_none_or(|enemy| enemy.awake))
        .map(|(tank, position, ..)| (position.0, tank.team))
        .collect()
}

/// What a bullet can run into, besides walls.
enum Target {
    Tank(Entity),
    Shield(Vec2),
    Mine(Entity),
    /// A tank that was just repaired: the bullet ends there and destroys nothing.
    Hulk,
}

pub fn fly(
    mut commands: Commands,
    field: Res<Field>,
    mut casualties: ResMut<Casualties>,
    mut bullets: Query<(
        Entity,
        &mut Bullet,
        &mut Position,
        &mut PreviousPosition,
        Option<&Maker>,
    )>,
    tanks: Query<
        (
            Entity,
            &Tank,
            &Position,
            &Shield,
            Has<Wreck>,
            Has<Protected>,
            Option<&Enemy>,
        ),
        Without<Bullet>,
    >,
    mines: Query<(Entity, &Position), (With<Mine>, Without<Bullet>)>,
    mut events: MessageWriter<SimEvent>,
) {
    let sought = quarry(
        tanks
            .iter()
            .map(|(_, tank, position, _, wreck, _, enemy)| (tank, position, enemy, wreck)),
    );
    for (entity, mut bullet, mut position, mut previous, maker) in &mut bullets {
        previous.0 = position.0;
        let start = position.0;
        let shooter = maker.map(|maker| (maker.0, bullet.team));
        // A splitting bullet becomes two once its time is up: each a plain bullet, that
        // leaves to one side of its line of flight with the bounces of its tank.
        if let Kind::Splitting { fuse, bounces } = bullet.kind {
            if fuse > TICK {
                bullet.kind = Kind::Splitting {
                    fuse: fuse - TICK,
                    bounces,
                };
            } else {
                events.write(SimEvent::BulletSplit { at: start });
                let (speed, heading) = (bullet.velocity.length(), bullet.velocity.to_angle());
                for side in [-1.0, 1.0] {
                    let direction = Vec2::from_angle(heading + side * SPLIT_ANGLE);
                    let (mut half, mut at) = (bullet.half(direction * speed, bounces), start);
                    let report = fly_past_walls(&field, &mut at, &mut half, SPLIT_APART, &sought);
                    for bounce in report.bounces {
                        events.write(SimEvent::Ricochet { at: bounce });
                    }
                    match report.end {
                        Flight::Flying => {
                            let place = (Position(at), PreviousPosition(at));
                            let mut spawned = commands.spawn((half, place, InSector(half.sector)));
                            if let Some(maker) = maker {
                                spawned.insert(*maker);
                            }
                        }
                        Flight::Stopped(solid) => {
                            strike(&mut casualties, solid, shooter);
                            let team = half.team;
                            events.write(SimEvent::BulletDestroyed { at, team });
                        }
                    }
                }
                commands.entity(entity).try_despawn();
                continue;
            }
        }
        let before = *bullet;
        let distance = bullet.velocity.length() * TICK;
        let report = fly_past_walls(&field, &mut position.0, &mut bullet, distance, &sought);

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
            for (tank_entity, tank, tank_position, shield, wreck, safe, _) in &tanks {
                let reach = |radius: f32| {
                    geom::sweep_circle_point(from, delta, tank_position.0, radius + BULLET_RADIUS)
                };
                // A bullet flies over a wreck, which gives no cover to the tank that repairs it.
                if wreck {
                    continue;
                }
                // A tank's own shot leaves its bubble, but doesn't come back through it.
                if shield.is_active() && (tank_entity != flying.owner || flying.bounced) {
                    consider(reach(SHIELD_RADIUS), Target::Shield(tank_position.0));
                } else if tanks::hits(&flying, tank_entity, tank.team) {
                    let target = if safe {
                        Target::Hulk
                    } else {
                        Target::Tank(tank_entity)
                    };
                    consider(reach(TANK_RADIUS), target);
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
                Target::Tank(tank) => {
                    casualties.tanks.push(tank);
                    if let Some(maker) = maker {
                        let blow = Blow {
                            weapon: Weapon::Bullet,
                            genome: maker.0,
                            team: bullet.team,
                        };
                        casualties.blows.push((tank, blow));
                    }
                }
                Target::Shield(center) => {
                    events.write(SimEvent::ShieldBlocked { at: center });
                }
                Target::Mine(mine) => casualties.detonations.push(mine),
                Target::Hulk => {}
            }
            go_off(&mut casualties, &bullet, at, shooter);
            let team = bullet.team;
            events.write(SimEvent::BulletDestroyed { at, team });
            commands.entity(entity).try_despawn();
            continue;
        }

        for bounce in report.bounces {
            events.write(SimEvent::Ricochet { at: bounce });
        }
        if let Flight::Stopped(solid) = report.end {
            strike(&mut casualties, solid, shooter);
            let (at, team) = (position.0, bullet.team);
            go_off(&mut casualties, &bullet, at, shooter);
            events.write(SimEvent::BulletDestroyed { at, team });
            commands.entity(entity).try_despawn();
        }
    }
}

/// Any two bullets that touch destroy each other. A piercing bullet that meets a normal one
/// destroys it and flies on, but that uses up its piercing. An exploding bullet goes off
/// where it is destroyed.
pub fn collide(
    mut commands: Commands,
    mut casualties: ResMut<Casualties>,
    mut bullets: Query<(
        Entity,
        &mut Bullet,
        &Position,
        &PreviousPosition,
        Option<&Maker>,
    )>,
    mut events: MessageWriter<SimEvent>,
) {
    let mut end = |bullet: &Bullet, at: Vec2, maker: Option<&Maker>| {
        let team = bullet.team;
        go_off(
            &mut casualties,
            bullet,
            at,
            maker.map(|maker| (maker.0, team)),
        );
        events.write(SimEvent::BulletDestroyed { at, team });
    };
    let mut destroyed: Vec<Entity> = Vec::new();
    let mut pairs = bullets.iter_combinations_mut();
    while let Some([mut a, mut b]) = pairs.fetch_next() {
        if destroyed.contains(&a.0) || destroyed.contains(&b.0) {
            continue;
        }
        let distance = geom::closest_approach(a.3.0, a.2.0, b.3.0, b.2.0);
        if distance > 2.0 * BULLET_RADIUS {
            continue;
        }
        let (a_pierces, b_pierces) = (a.1.pierces_left > 0, b.1.pierces_left > 0);
        if a_pierces && !b_pierces {
            a.1.pierces_left -= 1;
        } else {
            destroyed.push(a.0);
            end(&a.1, a.2.0, a.4);
        }
        if b_pierces && !a_pierces {
            b.1.pierces_left -= 1;
        } else {
            destroyed.push(b.0);
            end(&b.1, b.2.0, b.4);
        }
    }
    for entity in destroyed {
        commands.entity(entity).try_despawn();
    }
}
