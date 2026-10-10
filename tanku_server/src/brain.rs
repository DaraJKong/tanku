//! The hand-written brains of the enemies.
//!
//! A brain issues the same commands a player has and its tank obeys the same rules. The tiers of
//! its software decide what it is able to do, and experience decides how well it does it.

use std::f32::consts::TAU;

use bevy::prelude::*;
use tanku_core::{
    geom, progression,
    rng::Stream,
    rules::{BLAST_RADIUS, BULLET_RADIUS, TANK_RADIUS, TICK},
    sim::{
        Bullet, Enemy, ExplosiveCrate, Gun, Mine, Motion, Player, Position, Shield, ShieldState,
        Tank, TankCommand, Team, Wreck,
    },
};

use crate::field::Field;

/// Distances a brain of tier 1 or more tries to keep from the player.
const NEAR: f32 = 8.0;
const FAR: f32 = 14.0;
/// A brain keeps its target until another player is this many times nearer.
const TARGET_HOLD: f32 = 1.25;
/// How far ahead a brain looks for obstacles.
const LOOKAHEAD: f32 = 2.5;
/// Directions tried when looking for a bank shot.
const BANK_SHOT_STEPS: u32 = 180;
/// How far a bullet is followed when looking for a bank shot.
const BANK_SHOT_RANGE: f32 = 60.0;

#[derive(Component)]
pub struct Brain {
    rng: Stream,
    experience: f32,
    awake_for: f32,
    /// Time before the brain acts on a line of fire it has just found.
    reaction: f32,
    had_solution: bool,
    aim_error: f32,
    aim_timer: f32,
    /// Turret angle of the bank shot the brain is trying, if any.
    bank_shot: Option<f32>,
    bank_shot_timer: f32,
    wander: Vec2,
    wander_timer: f32,
    strafe: f32,
    /// The bullet the brain is watching and for how long it has seen it coming.
    threat: Option<(Entity, f32)>,
    /// The player the brain fights.
    target: Option<Entity>,
}

impl Brain {
    pub fn new(rng: Stream, sector: u32) -> Self {
        Self {
            rng,
            experience: progression::experience(sector),
            awake_for: 0.0,
            reaction: 0.0,
            had_solution: false,
            aim_error: 0.0,
            aim_timer: 0.0,
            bank_shot: None,
            bank_shot_timer: 0.0,
            wander: Vec2::ZERO,
            wander_timer: 0.0,
            strafe: 1.0,
            threat: None,
            target: None,
        }
    }

    /// Delay before the brain reacts: 0.50 s for a novice, 0.12 s for a flawless brain.
    fn reaction_delay(&self) -> f32 {
        0.50 - 0.38 * self.experience
    }

    /// Largest aiming error: 8 degrees for a novice, 1 degree for a flawless brain.
    fn max_aim_error(&self) -> f32 {
        (8.0 - 7.0 * self.experience).to_radians()
    }
}

/// Where a target will be when a bullet fired now gets there.
fn lead(origin: Vec2, target: Vec2, velocity: Vec2, bullet_speed: f32) -> Vec2 {
    let mut predicted = target;
    for _ in 0..2 {
        predicted = target + velocity * (origin.distance(predicted) / bullet_speed);
    }
    predicted
}

/// The turret angle, closest to the current one, of a shot that reaches the target after one
/// bounce or more.
fn bank_shot(field: &Field, origin: Vec2, target: Vec2, bounces: u8, turret: f32) -> Option<f32> {
    let mut best: Option<f32> = None;
    for step in 0..BANK_SHOT_STEPS {
        let angle = step as f32 * TAU / BANK_SHOT_STEPS as f32;
        let mut direction = Vec2::from_angle(angle);
        let mut from = origin;
        for leg in 0..=bounces {
            let delta = direction * BANK_SHOT_RANGE;
            let hit = field.cast(from, delta, BULLET_RADIUS);
            let to = from + delta * hit.map_or(1.0, |(hit, _)| hit.t);
            if leg > 0 && geom::distance_to_segment(target, from, to) < TANK_RADIUS * 0.7 {
                let closer = best.is_none_or(|best| {
                    geom::angle_difference(turret, angle).abs()
                        < geom::angle_difference(turret, best).abs()
                });
                if closer {
                    best = Some(angle);
                }
                break;
            }
            match hit {
                Some((hit, solid)) if solid.bounces() => {
                    direction = geom::reflect(direction, hit.normal);
                    from = to;
                }
                _ => break,
            }
        }
    }
    best
}

/// A direction close to the wanted one that isn't blocked right ahead.
fn steer(field: &Field, tank: &Tank, from: Vec2, wanted: Vec2) -> Vec2 {
    let Some(wanted) = wanted.try_normalize() else {
        return Vec2::ZERO;
    };
    let free = |direction: Vec2| {
        field
            .cast(from, direction * LOOKAHEAD, TANK_RADIUS)
            .is_none_or(|(_, solid)| !solid.stops(tank))
    };
    [0.0f32, 40.0, -40.0, 80.0, -80.0, 120.0, -120.0, 180.0]
        .into_iter()
        .map(|turn| Vec2::from_angle(turn.to_radians()).rotate(wanted))
        .find(|&direction| free(direction))
        .unwrap_or(Vec2::ZERO)
}

pub fn think(
    field: Res<Field>,
    players: Query<(Entity, &Player, &Tank, &Position, &Motion), Without<Wreck>>,
    mut enemies: Query<
        (
            Entity,
            &Tank,
            &Position,
            &Motion,
            &Gun,
            &Shield,
            &Enemy,
            &mut Brain,
            &mut TankCommand,
        ),
        Without<Player>,
    >,
    bullets: Query<(Entity, &Bullet, &Position)>,
    mines: Query<&Position, With<Mine>>,
    crates: Query<&Position, With<ExplosiveCrate>>,
) {
    let allies: Vec<(Entity, Vec2)> = enemies
        .iter()
        .map(|(entity, _, position, ..)| (entity, position.0))
        .collect();
    // The tanks of the players that still run: where each one is, in its sector, and how it
    // moves. They are in the order of their seats, so that the same fight goes the same way.
    let mut rivals: Vec<(u8, Entity, u32, Vec2, Vec2)> = players
        .iter()
        .map(|(entity, player, tank, position, motion)| {
            let (at, velocity) = (position.0, motion.velocity);
            (player.seat, entity, tank.sector, at, velocity)
        })
        .collect();
    rivals.sort_by_key(|rival| rival.0);

    for (entity, tank, position, motion, gun, shield, enemy, mut brain, mut command) in &mut enemies
    {
        if !enemy.awake {
            continue;
        }
        let me = position.0;
        let brain = &mut *brain;
        // An enemy fights the players that are in its sector: the nearest one, and it keeps
        // that one until another is clearly nearer.
        let here = || rivals.iter().filter(|rival| rival.2 == tank.sector);
        let nearest = here().min_by(|a, b| me.distance(a.3).total_cmp(&me.distance(b.3)));
        let Some(nearest) = nearest else {
            // Nobody to fight: hold still and hold fire.
            brain.target = None;
            *command = TankCommand {
                aim: command.aim,
                ..default()
            };
            continue;
        };
        let kept = here()
            .find(|rival| Some(rival.1) == brain.target)
            .filter(|kept| me.distance(kept.3) <= me.distance(nearest.3) * TARGET_HOLD);
        let &(_, target, _, player_at, player_velocity) = kept.unwrap_or(nearest);
        brain.target = Some(target);
        let genome = &tank.genome;
        if brain.awake_for == 0.0 {
            brain.reaction = brain.reaction_delay();
        }
        brain.awake_for += TICK;
        let (tier, ballistics) = (genome.brain(), genome.ballistics());
        let sees_player = field.sees(me, player_at);

        // Gunnery. The brain aims where it believes the target is, and its belief is off by an
        // error that changes a few times per second.
        let target = if ballistics >= 1 {
            lead(me, player_at, player_velocity, genome.bullet_speed())
        } else {
            player_at
        };
        let mut aim = (target - me).to_angle();
        let mut solution = field.sees(me, target);
        if !solution && ballistics >= 2 {
            brain.bank_shot_timer -= TICK;
            if brain.bank_shot_timer <= 0.0 {
                brain.bank_shot_timer = 0.3 + 0.1 * brain.rng.unit();
                let bounces = genome.bounces().min(ballistics - 1);
                brain.bank_shot = bank_shot(&field, me, target, bounces, motion.turret);
            }
            if let Some(angle) = brain.bank_shot {
                aim = angle;
                solution = true;
            }
        } else {
            brain.bank_shot = None;
        }
        if ballistics >= 3 {
            // A mine or a crate next to the player is a bigger target than the player.
            let explosive = mines
                .iter()
                .chain(&crates)
                .map(|position| position.0)
                .find(|&at| {
                    at.distance(player_at) < BLAST_RADIUS * 0.8
                        && at.distance(me) > BLAST_RADIUS + TANK_RADIUS
                        && field.sees(me, at)
                });
            if let Some(at) = explosive {
                aim = (at - me).to_angle();
                solution = true;
            }
        }
        brain.aim_timer -= TICK;
        if brain.aim_timer <= 0.0 {
            brain.aim_timer = 0.4;
            brain.aim_error = brain.rng.signed_unit() * brain.max_aim_error();
        }
        aim += brain.aim_error;

        if solution && !brain.had_solution {
            brain.reaction = brain.reaction.max(brain.reaction_delay());
        }
        brain.had_solution = solution;
        brain.reaction = (brain.reaction - TICK).max(0.0);
        let aligned = geom::angle_difference(motion.turret, aim).abs() < 2.0f32.to_radians();
        let fire = solution && aligned && brain.reaction == 0.0 && gun.rounds > 0;

        // The nearest player bullet that is about to hit, and for how long the brain has seen it.
        let mut incoming: Option<(Entity, f32, Vec2)> = None;
        for (bullet_entity, bullet, bullet_position) in &bullets {
            if bullet.team != Team::Players {
                continue;
            }
            let speed = bullet.velocity.length();
            let heading = bullet.velocity / speed;
            let offset = me - bullet_position.0;
            let along = offset.dot(heading);
            let aside = heading.perp_dot(offset);
            let on_course = along > 0.0 && along < speed && aside.abs() < TANK_RADIUS + 0.4;
            if on_course
                && incoming.is_none_or(|(_, time, _)| along / speed < time)
                && field.sees(bullet_position.0, me)
            {
                // The side of its path the tank is on is the way out.
                let escape = heading.perp() * if aside < 0.0 { -1.0 } else { 1.0 };
                incoming = Some((bullet_entity, along / speed, escape));
            }
        }
        brain.threat = incoming.map(|(bullet_entity, ..)| match brain.threat {
            Some((watched, seen)) if watched == bullet_entity => (watched, seen + TICK),
            _ => (bullet_entity, 0.0),
        });
        let reacting = brain
            .threat
            .is_some_and(|(_, seen)| seen >= brain.reaction_delay());

        // Tactics.
        let to_player = player_at - me;
        let distance = to_player.length();
        let toward = to_player / distance.max(0.01);
        let mut drive;
        let mut raise_shield = false;
        if tier == 0 {
            // A sentry wanders, if it can move at all.
            brain.wander_timer -= TICK;
            let blocked = steer(&field, tank, me, brain.wander) != brain.wander;
            if brain.wander_timer <= 0.0 || blocked {
                brain.wander_timer = 1.5 + 2.0 * brain.rng.unit();
                brain.wander = Vec2::from_angle(brain.rng.unit() * TAU);
            }
            drive = brain.wander;
        } else {
            // Keep the distance, in sight of the player.
            if brain.rng.chance(1) {
                brain.strafe = -brain.strafe;
            }
            drive = if !sees_player || distance > FAR {
                toward
            } else if distance < NEAR {
                -toward
            } else {
                toward.perp() * brain.strafe
            };
            // Stay away from mines.
            for mine in &mines {
                let away = me - mine.0;
                if away.length() < BLAST_RADIUS + 1.0 {
                    drive += away.normalize_or_zero() * 1.5;
                }
            }
            if tier >= 2 {
                // Spread out from allies.
                for &(ally, at) in &allies {
                    let away = me - at;
                    if ally != entity && away.length() < 6.0 {
                        drive += away.normalize_or_zero() * 0.6;
                    }
                }
                // Reload out of sight.
                if gun.rounds == 0 && sees_player {
                    let cover = (0..12)
                        .map(|step| me + Vec2::from_angle(step as f32 * TAU / 12.0) * 5.0)
                        .filter(|&spot| {
                            !field.sees(spot, player_at)
                                && field.cast(me, spot - me, TANK_RADIUS).is_none()
                        })
                        .min_by(|a, b| a.distance(player_at).total_cmp(&b.distance(player_at)));
                    drive = cover.map_or(-toward, |spot| (spot - me).normalize_or_zero());
                }
            }

            if let (true, Some((_, time, escape))) = (reacting, incoming) {
                if tier >= 2 {
                    // Dodge a bullet that flies straight at the tank, to the other side of its
                    // path if the near side is walled in.
                    let open = |way: Vec2| steer(&field, tank, me, way) == way;
                    drive = if open(escape) || !open(-escape) {
                        escape
                    } else {
                        -escape
                    };
                }
                let too_late = if tier >= 2 { 0.2 } else { 0.3 };
                raise_shield = time < too_late;
            }
            drive = steer(&field, tank, me, drive);
        }

        let can_shield = shield.state == ShieldState::Ready && shield.cooldown == 0.0;
        *command = TankCommand {
            drive,
            aim: me + Vec2::from_angle(aim) * 10.0,
            fire,
            // Mines are dropped by reflex when the player comes close.
            mine: tier >= 1 && distance < 6.0,
            shield: raise_shield && can_shield,
            repair: false,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Collider, Solid};

    /// A room 20 m wide and 10 m deep, cut in two by a wall that leaves a gap at the top.
    fn divided_room() -> Field {
        let wall = |a: (f32, f32), b: (f32, f32)| Collider {
            a: a.into(),
            b: b.into(),
            solid: Solid::Wall,
        };
        let mut field = Field::new(0, 1);
        field.colliders = vec![
            wall((0.0, 0.0), (20.0, 0.0)),
            wall((20.0, 0.0), (20.0, 10.0)),
            wall((20.0, 10.0), (0.0, 10.0)),
            wall((0.0, 10.0), (0.0, 0.0)),
            wall((10.0, 0.0), (10.0, 6.0)),
        ];
        field
    }

    #[test]
    fn a_bank_shot_goes_around_a_wall() {
        let field = divided_room();
        let (origin, target) = (Vec2::new(5.0, 3.0), Vec2::new(15.0, 3.0));
        assert!(!field.sees(origin, target));
        assert_eq!(bank_shot(&field, origin, target, 0, 0.0), None);
        // Off the far wall, over the divider: the mirror image of the target is at (15, 17).
        let angle = bank_shot(&field, origin, target, 1, 0.0).unwrap();
        let expected = (Vec2::new(15.0, 17.0) - origin).to_angle();
        assert!(geom::angle_difference(angle, expected).abs() < 4.0f32.to_radians());
    }

    #[test]
    fn leading_a_target_aims_ahead_of_it() {
        let predicted = lead(Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(0.0, 5.0), 10.0);
        // The bullet needs a little more than a second, during which the target moves 5 m/s.
        assert!(predicted.y > 5.0 && predicted.y < 6.5);
        assert_eq!(predicted.x, 10.0);
    }

    #[test]
    fn steering_goes_around_what_is_right_ahead() {
        let field = divided_room();
        let tank = Tank {
            genome: tanku_core::stats::Genome::empty(0),
            team: Team::Enemies,
            sector: 0,
        };
        let from = Vec2::new(8.0, 3.0);
        assert_eq!(steer(&field, &tank, from, Vec2::NEG_X), Vec2::NEG_X);
        let around = steer(&field, &tank, from, Vec2::X);
        assert!(around != Vec2::X && around != Vec2::ZERO);
    }
}
