//! The hand-written brains of the enemies.
//!
//! A brain issues the same commands a player has and its tank obeys the same rules. The tiers of
//! its software decide what it is able to do, and experience decides how well it does it.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use tanku_core::{
    geom, progression,
    rng::Stream,
    rules::{BLAST_RADIUS, BULLET_RADIUS, MINE_TRIGGER_RADIUS, MUZZLE_DISTANCE, TANK_RADIUS, TICK},
    sim::{
        Bullet, Enemy, ExplosiveCrate, Gun, Mine, Motion, Player, Position, Shield, ShieldState,
        Tank, TankCommand, Team, Wreck,
    },
};

use crate::{
    field::{Collider, Field},
    tanks,
};

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
/// How far ahead a brain sees a bullet coming, in seconds.
const FORESIGHT: f32 = 1.0;
/// A bullet whose path comes this close to the middle of a tank is on course to hit it.
const ON_COURSE: f32 = TANK_RADIUS + 0.4;
/// A soldier or a veteran raises its shield this long before a bullet hits.
const LAST_MOMENT: f32 = 0.2;
/// The room that a veteran wants between its tank and a bullet to count on being missed.
const DODGE_ROOM: f32 = 0.15;
/// The ways that a veteran tries when the way it meant to drive brings a bullet too close:
/// turned from that way by so many degrees, the nearest to it first.
const WAYS_OUT: [f32; 7] = [45.0, -45.0, 90.0, -90.0, 135.0, -135.0, 180.0];
/// A veteran fires at a bullet once its shot will pass this close to the middle of it.
const PARRY_AIM: f32 = 0.3;
/// A brain lays mines by reflex when the player is this close.
const MINE_REFLEX: f32 = 6.0;
/// A veteran lays mines in the way of a player who is no further than this.
const MINING_RANGE: f32 = 20.0;
/// A veteran lays no mine this close to another one.
const MINE_SPACING: f32 = 5.0;
/// A player who comes at a tank this fast charges it.
const CHARGE_SPEED: f32 = 3.0;
/// The distances that an ace keeps from a player who has rounds: far enough to see their
/// shots coming and get out of the way.
const BAIT: (f32, f32) = (11.0, 14.0);
/// The distances that it keeps from a player who has none: close enough for its own shots
/// to be hard to dodge.
const PRESS: (f32, f32) = (5.0, 8.0);
/// It presses a player until the player has this many rounds again.
const PRESS_UNTIL: u8 = 3;
/// The distances that it keeps when it is the last of its group.
const RETREAT: (f32, f32) = (16.0, 22.0);
/// An ace wants at least this much of a turn around the player between itself and the
/// nearest ally, so that their fire crosses.
const CROSSFIRE: f32 = PI / 2.0;

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
    /// The bullet that a grunt or a soldier is watching, and for how long it has seen it
    /// coming.
    threat: Option<(Entity, f32)>,
    /// The player the brain fights.
    target: Option<Entity>,
    /// The bullets that a veteran watches: all those that come near its tank, where a
    /// soldier watches the one that is about to hit.
    watched: Vec<Watch>,
    /// Whether an ace presses the player it fights, who has run out of rounds.
    pressing: bool,
}

/// A bullet that a veteran watches.
struct Watch {
    bullet: Entity,
    /// For how long the brain has seen it.
    seen: f32,
    /// Whether the brain has fired at it to shoot it down. One shot is all a bullet gets.
    answered: bool,
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
            watched: Vec::new(),
            pressing: false,
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

    /// Keeps an eye on the bullets that come near the tank, each for as long as it does.
    fn watch(&mut self, near: &[Entity]) {
        self.watched.retain(|watch| near.contains(&watch.bullet));
        for watch in &mut self.watched {
            watch.seen += TICK;
        }
        for &bullet in near {
            if self.watched.iter().all(|watch| watch.bullet != bullet) {
                self.watched.push(Watch {
                    bullet,
                    seen: 0.0,
                    answered: false,
                });
            }
        }
    }

    /// What the brain knows of a bullet that it watches.
    fn watching(&mut self, bullet: Entity) -> Option<&mut Watch> {
        self.watched.iter_mut().find(|watch| watch.bullet == bullet)
    }

    /// Whether the brain has watched a bullet for long enough to act on it.
    fn minds(&self, bullet: Entity) -> bool {
        let watch = self.watched.iter().find(|watch| watch.bullet == bullet);
        watch.is_some_and(|watch| watch.seen >= self.reaction_delay())
    }
}

/// Where a target will be when a bullet fired now gets there.
pub(crate) fn lead(origin: Vec2, target: Vec2, velocity: Vec2, bullet_speed: f32) -> Vec2 {
    let mut predicted = target;
    for _ in 0..2 {
        predicted = target + velocity * (origin.distance(predicted) / bullet_speed);
    }
    predicted
}

/// The turret angle, closest to the current one, of a shot that reaches the target after one
/// bounce or more.
pub(crate) fn bank_shot(
    field: &Field,
    origin: Vec2,
    target: Vec2,
    bounces: u8,
    turret: f32,
) -> Option<f32> {
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
pub(crate) fn steer(field: &Field, tank: &Tank, from: Vec2, wanted: Vec2) -> Vec2 {
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

/// A bullet that is about to hit a tank.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Threat {
    bullet: Entity,
    /// Seconds until it hits.
    time: f32,
    /// The way out of its path: across it, on the side that the tank is on.
    escape: Vec2,
}

/// The bullet of a player that flies straight at a tank and gets there first, if any.
fn oncoming<'a>(
    field: &Field,
    me: Vec2,
    bullets: impl Iterator<Item = (Entity, &'a Bullet, &'a Position)>,
) -> Option<Threat> {
    let mut incoming: Option<Threat> = None;
    for (entity, bullet, position) in bullets {
        if bullet.team != Team::Players {
            continue;
        }
        let speed = bullet.velocity.length();
        let heading = bullet.velocity / speed;
        let offset = me - position.0;
        let along = offset.dot(heading);
        let aside = heading.perp_dot(offset);
        let on_course = along > 0.0 && along < speed * FORESIGHT && aside.abs() < ON_COURSE;
        if on_course
            && incoming.is_none_or(|threat| along / speed < threat.time)
            && field.sees(position.0, me)
        {
            // The side of its path the tank is on is the way out.
            let escape = heading.perp() * if aside < 0.0 { -1.0 } else { 1.0 };
            incoming = Some(Threat {
                bullet: entity,
                time: along / speed,
                escape,
            });
        }
    }
    incoming
}

/// A stretch of the path of a bullet: straight, from where the bullet is or bounces to where
/// it bounces next or ends.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Leg {
    from: Vec2,
    to: Vec2,
    /// Seconds until the bullet starts on it.
    start: f32,
}

/// The path of a player's bullet over the time that a brain sees ahead, bounces included. A
/// veteran follows every bullet this way.
#[derive(Clone, Debug)]
struct Path {
    bullet: Entity,
    at: Vec2,
    velocity: Vec2,
    legs: Vec<Leg>,
    /// Whether the bullet flies on after the time that a brain sees ahead: its path doesn't
    /// end on something solid before that.
    flies_on: bool,
}

impl Path {
    /// Follows a bullet from where it is, off the walls and the blocks that it bounces off.
    fn of(field: &Field, entity: Entity, bullet: &Bullet, at: Vec2) -> Self {
        let speed = bullet.velocity.length();
        let mut legs = Vec::new();
        let (mut from, mut direction) = (at, bullet.velocity / speed);
        let (mut left, mut start) = (speed * FORESIGHT, 0.0);
        let mut flies_on = false;
        for bounce in 0..=bullet.bounces_left {
            let delta = direction * left;
            let hit = field.cast(from, delta, BULLET_RADIUS);
            let done = hit.map_or(1.0, |(hit, _)| hit.t);
            let to = from + delta * done;
            legs.push(Leg { from, to, start });
            match hit {
                Some((hit, solid)) if solid.bounces() && bounce < bullet.bounces_left => {
                    start += left * done / speed;
                    left *= 1.0 - done;
                    direction = geom::reflect(direction, hit.normal);
                    from = to;
                }
                Some(_) => break,
                None => {
                    flies_on = true;
                    break;
                }
            }
        }
        Self {
            bullet: entity,
            at,
            velocity: bullet.velocity,
            legs,
            flies_on,
        }
    }

    /// Seconds until the bullet ends, or until it is out of sight.
    fn over(&self) -> f32 {
        let last = self.legs[self.legs.len() - 1];
        last.start + last.from.distance(last.to) / self.velocity.length()
    }

    /// Where the bullet is after some time, which is no longer than its path lasts.
    fn place(&self, time: f32) -> Vec2 {
        let leg = self.legs.iter().rev().find(|leg| leg.start <= time);
        let leg = leg.unwrap_or(&self.legs[0]);
        let along = (time - leg.start) * self.velocity.length();
        let stretch = leg.to - leg.from;
        leg.from + stretch.clamp_length_max(along.max(0.0))
    }

    /// How close the bullet comes to a place within the time that a brain sees ahead. A
    /// bullet that is still coming closer at the end of that time is too far to tell.
    fn nears(&self, place: Vec2) -> Option<f32> {
        let (mut least, mut coming) = (f32::MAX, false);
        for (index, leg) in self.legs.iter().enumerate() {
            let distance = geom::distance_to_segment(place, leg.from, leg.to);
            if distance < least {
                // The place lies past the end of the last stretch, where the bullet flies on.
                let stretch = leg.to - leg.from;
                let past = (place - leg.from).dot(stretch) >= stretch.length_squared();
                coming = self.flies_on && index + 1 == self.legs.len() && past;
                least = distance;
            }
        }
        (!coming).then_some(least)
    }

    /// How close the bullet comes to a tank on its course: the least room that is left
    /// between them, and the seconds until too little is left, if it comes to that.
    fn approach(&self, course: &[Vec2]) -> (f32, Option<f32>) {
        let over = self.over();
        let (mut room, mut close) = (f32::MAX, None);
        let mut bullet = self.at;
        for (tick, places) in course.windows(2).enumerate() {
            // A bullet that has ended is in nobody's way.
            let since = tick as f32 * TICK;
            if since >= over {
                break;
            }
            // In a tick, the tank drives first, and the bullet then flies at where it is.
            let flown = self.place((since + TICK).min(over));
            let apart = geom::distance_to_segment(places[1], bullet, flown);
            let left = apart - TANK_RADIUS - BULLET_RADIUS;
            room = room.min(left);
            if left < DODGE_ROOM && close.is_none() {
                close = Some(since + TICK);
            }
            bullet = flown;
        }
        (room, close)
    }
}

/// A tank as its brain works out where it gets to: what it is, where it is, how it moves and
/// what is solid around it, as far as it gets in the time that a brain sees ahead.
struct Mover<'a> {
    tank: &'a Tank,
    at: Vec2,
    motion: &'a Motion,
    near: Vec<Collider>,
}

impl<'a> Mover<'a> {
    fn new(field: &Field, tank: &'a Tank, at: Vec2, motion: &'a Motion) -> Self {
        let reach = tank.genome.top_speed() * FORESIGHT + TANK_RADIUS + 0.5;
        let near =
            |collider: &&Collider| geom::distance_to_segment(at, collider.a, collider.b) < reach;
        Self {
            tank,
            at,
            motion,
            near: field.colliders.iter().filter(near).copied().collect(),
        }
    }

    /// Where the tank is, tick after tick, when it drives one way from now on.
    fn course(&self, way: Vec2) -> Vec<Vec2> {
        let command = TankCommand {
            drive: way,
            aim: self.at,
            ..default()
        };
        let (mut at, mut motion) = (self.at, *self.motion);
        let ticks = (FORESIGHT / TICK).round() as usize;
        let mut places = vec![at];
        for _ in 0..ticks {
            tanks::step_among(&self.near, self.tank, false, &command, &mut at, &mut motion);
            places.push(at);
        }
        places
    }
}

/// How a tank fares on a way against the bullets that its brain knows of.
#[derive(Clone, Copy, Debug)]
struct Fare<'a> {
    way: Vec2,
    /// The least room that is left between the tank and a bullet. A bullet hits when there
    /// is none.
    room: f32,
    /// The bullet that comes too close first, and the seconds until it does.
    first: Option<(&'a Path, f32)>,
}

impl<'a> Fare<'a> {
    fn of(way: Vec2, course: &[Vec2], bullets: &[&'a Path]) -> Self {
        let mut fare = Self {
            way,
            room: f32::MAX,
            first: None,
        };
        for path in bullets {
            let (room, close) = path.approach(course);
            fare.room = fare.room.min(room);
            let sooner = |time: &f32| fare.first.is_none_or(|(_, first)| *time < first);
            if let Some(time) = close.filter(sooner) {
                fare.first = Some((path, time));
            }
        }
        fare
    }
}

/// The way that a veteran drives among the bullets that it knows of: the way it meant to
/// drive, with the course that this gives its tank, when no bullet gets too close on it.
/// Otherwise it tries the ways around that one, and standing still, and takes the first on
/// which none does. Failing that, it takes the way that leaves the most room. A tank that
/// meant to stand still turns from the way it is given instead.
fn way_out<'a>(
    mover: &Mover,
    meant: Vec2,
    plan: &[Vec2],
    otherwise: Vec2,
    bullets: &[&'a Path],
) -> Fare<'a> {
    let mut best = Fare::of(meant, plan, bullets);
    let ahead = meant.try_normalize().unwrap_or(otherwise);
    let turned = |turn: f32| Vec2::from_angle(turn.to_radians()).rotate(ahead);
    for way in WAYS_OUT.map(turned).into_iter().chain([Vec2::ZERO]) {
        if best.room >= DODGE_ROOM {
            break;
        }
        let fare = Fare::of(way, &mover.course(way), bullets);
        if fare.room > best.room {
            best = fare;
        }
    }
    best
}

/// Where a shot that a tank fires now meets a bullet, if it can.
fn intercept(me: Vec2, at: Vec2, velocity: Vec2, speed: f32) -> Option<Vec2> {
    // The shot starts at the tip of the barrel. The two meet at the time when the shot is as
    // far from the tank as the bullet is.
    let offset = at - me;
    let a = velocity.length_squared() - speed * speed;
    let b = 2.0 * (offset.dot(velocity) - MUZZLE_DISTANCE * speed);
    let c = offset.length_squared() - MUZZLE_DISTANCE * MUZZLE_DISTANCE;
    if c <= 0.0 {
        return None;
    }
    let time = if a.abs() < 1e-4 {
        (b < 0.0).then(|| -c / b)?
    } else {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            return None;
        }
        let root = discriminant.sqrt();
        let times = [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)];
        let ahead = times.into_iter().filter(|&time| time >= 0.0);
        ahead.min_by(f32::total_cmp)?
    };
    Some(at + velocity * time)
}

/// Where a tank aims to shoot down a bullet: at the place where its shot meets it, when
/// nothing solid is between its gun and that place.
fn answer(field: &Field, me: Vec2, path: &Path, speed: f32) -> Option<Vec2> {
    intercept(me, path.at, path.velocity, speed).filter(|&meeting| field.sees(me, meeting))
}

/// Whether a place lies between two walls or blocks that are so close that no tank gets
/// through without setting off a mine that lies there.
fn narrow(field: &Field, at: Vec2) -> bool {
    let reach = MINE_TRIGGER_RADIUS + TANK_RADIUS;
    let wall = |way: Vec2| Some(field.cast(at, way * (reach - 0.1), 0.1)?.0.normal);
    (0..4).any(|step| {
        let across = Vec2::from_angle(step as f32 * PI / 4.0);
        // The two face each other. A corner has a wall on two sides as well, and leads nowhere.
        matches!(
            (wall(across), wall(-across)),
            (Some(one), Some(other)) if one.dot(other) < -0.5
        )
    })
}

pub fn think(
    field: Res<Field>,
    players: Query<(Entity, &Player, &Tank, &Position, &Motion, &Gun), Without<Wreck>>,
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
    // Every enemy: where it is, and the sector that it fights in if it is awake.
    let allies: Vec<(Entity, Vec2, Option<u32>)> = enemies
        .iter()
        .map(|(entity, tank, position, .., enemy, _, _)| {
            (entity, position.0, enemy.awake.then_some(tank.sector))
        })
        .collect();
    // The tanks of the players that still run: where each one is, in its sector, how it
    // moves and how many rounds it has. They are in the order of their seats, so that the
    // same fight goes the same way.
    let mut rivals: Vec<(u8, Entity, u32, Vec2, Vec2, u8)> = players
        .iter()
        .map(|(entity, player, tank, position, motion, gun)| {
            let (at, velocity) = (position.0, motion.velocity);
            (player.seat, entity, tank.sector, at, velocity, gun.rounds)
        })
        .collect();
    rivals.sort_by_key(|rival| rival.0);
    // Where the bullets of the players go, bounces included, for the brains that follow them.
    let followed = enemies
        .iter()
        .any(|(_, tank, .., enemy, _, _)| enemy.awake && tank.genome.brain() >= 3);
    let paths: Vec<Path> = bullets
        .iter()
        .filter(|(_, bullet, _)| followed && bullet.team == Team::Players)
        .map(|(entity, bullet, position)| Path::of(&field, entity, bullet, position.0))
        .collect();

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
        let &(_, target, _, player_at, player_velocity, player_rounds) = kept.unwrap_or(nearest);
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
        let mut fire = solution && aligned && brain.reaction == 0.0 && gun.rounds > 0;

        // A soldier sees the player bullet that is about to hit first, once it flies straight
        // at its tank, and for how long the brain has seen it. A veteran looks further: see
        // below.
        let delay = brain.reaction_delay();
        let incoming = if tier >= 3 {
            None
        } else {
            oncoming(&field, me, bullets.iter())
        };
        brain.threat = incoming.map(|threat| match brain.threat {
            Some((watched, seen)) if watched == threat.bullet => (watched, seen + TICK),
            _ => (threat.bullet, 0.0),
        });
        let reacting = brain.threat.is_some_and(|(_, seen)| seen >= delay);

        // Tactics.
        let to_player = player_at - me;
        let distance = to_player.length();
        let toward = to_player / distance.max(0.01);
        let mut drive;
        let mut raise_shield = false;
        // The bullet that a veteran shoots down, and where it aims for that.
        let mut parry: Option<(Entity, Vec2)> = None;
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
            let mut band = (NEAR, FAR);
            if tier >= 4 {
                // An ace looks at who fights beside it, and counts the rounds of the player.
                let beside = |&&(ally, _, fights): &&(Entity, Vec2, Option<u32>)| {
                    ally != entity && fights == Some(tank.sector)
                };
                let fellows: Vec<Vec2> = allies.iter().filter(beside).map(|ally| ally.1).collect();
                if player_rounds == 0 {
                    brain.pressing = true;
                } else if player_rounds >= PRESS_UNTIL {
                    brain.pressing = false;
                }
                // The last of its group retreats. Otherwise it baits the shots of a player
                // who has rounds from where it sees them coming, and presses a player who
                // has none.
                band = if fellows.is_empty() {
                    RETREAT
                } else if brain.pressing {
                    PRESS
                } else {
                    BAIT
                };
                // It drives around the player, away from the nearest ally, until their fire
                // crosses.
                let bearing = |at: Vec2| (at - player_at).to_angle();
                let turn = |at: &Vec2| geom::angle_difference(bearing(*at), bearing(me));
                let nearest = fellows
                    .iter()
                    .map(turn)
                    .min_by(|a, b| a.abs().total_cmp(&b.abs()));
                if let Some(turn) = nearest.filter(|turn| turn.abs() < CROSSFIRE) {
                    brain.strafe = if turn > 0.0 { -1.0 } else { 1.0 };
                }
            }
            drive = if !sees_player || distance > band.1 {
                toward
            } else if distance < band.0 {
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
                for &(ally, at, _) in &allies {
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

            if tier >= 3 {
                // A veteran follows the bullets off the walls, and watches every one that is a
                // second or less from passing over its tank: one that flies straight at it,
                // and one that will come at it off a wall. An ace watches as far around its
                // tank as it drives in that time, so it also sees a shot that is aimed ahead
                // of it. Either minds each of them once it has watched it for long enough.
                let ahead = if tier >= 4 {
                    genome.top_speed() * FORESIGHT
                } else {
                    0.0
                };
                let passes =
                    |path: &&Path| path.nears(me).is_some_and(|near| near < ahead + ON_COURSE);
                let near: Vec<&Path> = paths.iter().filter(passes).collect();
                let bullets: Vec<Entity> = near.iter().map(|path| path.bullet).collect();
                brain.watch(&bullets);
                let minded: Vec<&Path> = near
                    .into_iter()
                    .filter(|path| brain.minds(path.bullet))
                    .collect();
                let meant = steer(&field, tank, me, drive);
                drive = meant;
                if !minded.is_empty() {
                    // It works out where its tank gets to and how close those bullets come to
                    // it meanwhile. It drives as it meant to when that takes it out of their
                    // way, and another way when it must.
                    let mover = Mover::new(&field, tank, me, motion);
                    let plan = mover.course(meant);
                    let fare = way_out(&mover, meant, &plan, toward, &minded);
                    drive = fare.way;
                    if let (true, Some((path, time))) = (fare.room < DODGE_ROOM, fare.first) {
                        // No way leaves room. It shoots down the bullet that gets to it first,
                        // with one shot, once nothing solid is between its gun and the place
                        // where the two meet: a bullet that is about to bounce is met after
                        // its bounce. It raises its shield at the last moment, when the
                        // bullet is still there.
                        raise_shield = time < LAST_MOMENT;
                        let watch = brain.watching(path.bullet);
                        if !watch.is_some_and(|watch| watch.answered) {
                            let meeting = answer(&field, me, path, genome.bullet_speed());
                            parry = meeting.map(|meeting| (path.bullet, meeting));
                        }
                    }
                }
            } else {
                if let (true, Some(threat)) = (reacting, incoming) {
                    if tier >= 2 {
                        // Dodge a bullet that flies straight at the tank, to the other side of
                        // its path if the near side is walled in.
                        let open = |way: Vec2| steer(&field, tank, me, way) == way;
                        drive = if open(threat.escape) || !open(-threat.escape) {
                            threat.escape
                        } else {
                            -threat.escape
                        };
                    }
                    let too_late = if tier >= 2 { LAST_MOMENT } else { 0.3 };
                    raise_shield = threat.time < too_late;
                }
                drive = steer(&field, tank, me, drive);
            }
        }

        // A veteran turns its gun on the bullet that it can't get out of the way of, and fires
        // once its shot will meet it.
        if let Some((bullet, meeting)) = parry {
            aim = (meeting - me).to_angle() + brain.aim_error;
            let cone = (PARRY_AIM / me.distance(meeting)).min(1.0).asin();
            let off = geom::angle_difference(motion.turret, aim).abs();
            fire = gun.rounds > 0 && off < cone.max(2.0f32.to_radians());
            if let (true, Some(watch)) = (fire && gun.cooldown <= TICK, brain.watching(bullet)) {
                watch.answered = true;
            }
        }

        // Mines are dropped by reflex when the player comes close. A veteran also lays one
        // where a player is likely to come: in a narrow place, and in the way of a player who
        // charges it. It leaves room between two mines, and lays none next to an ally.
        let mut lay = tier >= 1 && distance < MINE_REFLEX;
        let spare = gun.mines < genome.mines() && gun.mine_cooldown <= TICK;
        if tier >= 3 && !lay && spare && genome.top_speed() > 0.0 && distance < MINING_RANGE {
            let closing = player_velocity.dot(-toward);
            let charged =
                sees_player && closing > CHARGE_SPEED && closing > 0.94 * player_velocity.length();
            let apart = mines.iter().all(|mine| mine.0.distance(me) > MINE_SPACING);
            let alone = allies
                .iter()
                .all(|&(ally, at, _)| ally == entity || at.distance(me) > BLAST_RADIUS + 1.0);
            lay = apart && alone && (charged || narrow(&field, me));
        }

        let can_shield = shield.state == ShieldState::Ready && shield.cooldown == 0.0;
        *command = TankCommand {
            drive,
            aim: me + Vec2::from_angle(aim) * 10.0,
            fire,
            mine: lay,
            shield: raise_shield && can_shield,
            repair: false,
        };
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::stats::{Genome, Stat};

    use super::*;
    use crate::field::Solid;

    fn wall(a: (f32, f32), b: (f32, f32)) -> Collider {
        Collider {
            a: a.into(),
            b: b.into(),
            solid: Solid::Wall,
        }
    }

    /// A room 20 m wide and 10 m deep.
    fn room() -> Field {
        let mut field = Field::new(0, 1);
        field.colliders = vec![
            wall((0.0, 0.0), (20.0, 0.0)),
            wall((20.0, 0.0), (20.0, 10.0)),
            wall((20.0, 10.0), (0.0, 10.0)),
            wall((0.0, 10.0), (0.0, 0.0)),
        ];
        field
    }

    /// The same room, cut in two by a wall that leaves a gap at the top.
    fn divided_room() -> Field {
        let mut field = room();
        field.colliders.push(wall((10.0, 0.0), (10.0, 6.0)));
        field
    }

    /// A player's bullet that flies so, with this many bounces left.
    fn bullet(velocity: Vec2, bounces_left: u8) -> Bullet {
        Bullet {
            owner: Entity::PLACEHOLDER,
            team: Team::Players,
            sector: 0,
            velocity,
            bounces_left,
            pierces_left: 0,
            bounced: false,
        }
    }

    /// The path of a bullet that flies south-east from the middle of the west half of the
    /// room. It comes off the south wall at (9.775, 0.225), 6.753 m further.
    fn path_off_the_south_wall(bounces_left: u8) -> (Path, Bullet, Position) {
        let flying = bullet(Vec2::new(1.0, -1.0).normalize() * 10.0, bounces_left);
        let from = Vec2::new(5.0, 5.0);
        let path = Path::of(&room(), Entity::PLACEHOLDER, &flying, from);
        (path, flying, Position(from))
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
    fn a_path_follows_a_bullet_off_a_wall() {
        let (path, ..) = path_off_the_south_wall(1);
        assert_eq!(path.legs.len(), 2);
        assert!(path.legs[0].to.distance(Vec2::new(9.775, 0.225)) < 1e-3);
        assert!((path.legs[1].start - 0.6753).abs() < 1e-3);
        // It lasts as long as a brain sees ahead: one second, in which the bullet flies 10 m.
        assert!((path.over() - FORESIGHT).abs() < 1e-3);
        assert!(path.place(0.0).distance(Vec2::new(5.0, 5.0)) < 1e-3);
        assert!(path.place(path.over()).distance(Vec2::new(12.071, 2.521)) < 1e-2);

        // A bullet that has no bounce left ends on the wall.
        let (path, ..) = path_off_the_south_wall(0);
        assert_eq!(path.legs.len(), 1);
        assert!((path.over() - 0.6753).abs() < 1e-3);
    }

    #[test]
    fn a_ricochet_is_seen_coming_by_a_brain_that_follows_the_bullet() {
        let (path, flying, from) = path_off_the_south_wall(1);
        // The bullet comes over this place after its bounce, 0.92 s from now.
        let me = Vec2::new(11.5, 2.0);
        assert!(path.nears(me).unwrap() < 0.1);
        assert!(path.place(0.92).distance(me) < 0.1);
        // A brain that only looks straight ahead of a bullet sees nothing coming there.
        let straight_ahead = [(Entity::PLACEHOLDER, &flying, &from)].into_iter();
        assert_eq!(oncoming(&room(), me, straight_ahead), None);

        // The bullet passes 2.1 m from this place before its bounce.
        let passed = path.nears(Vec2::new(5.0, 2.0)).unwrap();
        assert!((passed - 2.121).abs() < 1e-2);
        // It is still on its way to the far end of the room when a second is over: too far
        // to tell how close it will come.
        assert_eq!(path.nears(Vec2::new(15.0, 8.0)), None);
        // A bullet that ends on the wall has come as close to a place as it ever will.
        let (ends, ..) = path_off_the_south_wall(0);
        assert!((ends.nears(Vec2::new(12.0, 0.225)).unwrap() - 2.225).abs() < 1e-2);
    }

    #[test]
    fn a_bullet_comes_as_close_to_a_tank_as_it_is_to_where_the_tank_is_then() {
        // In a tick, a bullet flies 1 m east while a tank drives from 5 m north of its path to
        // 1 m north of it. The tank has driven when the bullet flies: they touch.
        let flying = bullet(Vec2::X * 60.0, 0);
        let path = Path::of(&room(), Entity::PLACEHOLDER, &flying, Vec2::new(5.0, 3.0));
        let course = [Vec2::new(5.5, 8.0), Vec2::new(5.5, 4.0)];
        let (room, close) = path.approach(&course);
        assert!((room - (1.0 - TANK_RADIUS - BULLET_RADIUS)).abs() < 1e-4);
        assert_eq!(close, Some(TICK));

        // A bullet that ends on the south wall after 0.68 s is in nobody's way afterward: a
        // tank that is far away until then, and on that very place a moment later, has had
        // all the room it needs.
        let (ends, ..) = path_off_the_south_wall(0);
        let far = [Vec2::new(15.0, 8.0); 45];
        let there = [Vec2::new(9.775, 1.0); 16];
        let (room, close) = ends.approach(&[far.as_slice(), &there].concat());
        assert!(room > 3.0);
        assert_eq!(close, None);
    }

    #[test]
    fn a_brain_watches_a_bullet_for_as_long_as_it_is_near() {
        let mut world = World::new();
        let [first, second] = [(); 2].map(|()| world.spawn_empty().id());
        // A brain of the first sector takes half a second to act on what it sees.
        let mut brain = Brain::new(Stream::new(1, tanku_core::rng::Purpose::Brain, 0, 0), 1);
        let seen =
            |brain: &mut Brain, bullet: Entity| brain.watching(bullet).map(|watch| watch.seen);
        let both = [first, second];
        brain.watch(&[first]);
        brain.watch(&both);
        brain.watch(&both);
        assert_eq!(seen(&mut brain, first), Some(2.0 * TICK));
        assert_eq!(seen(&mut brain, second), Some(TICK));
        assert!(!brain.minds(first));
        for _ in 0..28 {
            brain.watch(&both);
        }
        assert!(brain.minds(first) && !brain.minds(second));
        // A bullet that is gone is forgotten, with what was done about it. One that comes
        // later is new to the brain.
        brain.watching(first).unwrap().answered = true;
        brain.watch(&[second]);
        assert_eq!(seen(&mut brain, first), None);
        brain.watch(&both);
        assert_eq!(seen(&mut brain, first), Some(0.0));
        assert!(!brain.watching(first).unwrap().answered);
        assert_eq!(brain.watched.len(), 2);
    }

    /// A tank in the middle of the room, with an engine and tracks of these levels.
    fn tank(engine: u8, tracks: u8) -> Tank {
        let mut genome = Genome::empty(100);
        genome.set(Stat::Engine, engine);
        genome.set(Stat::Tracks, tracks);
        Tank {
            genome,
            team: Team::Enemies,
            sector: 0,
        }
    }

    const MIDDLE: Vec2 = Vec2::new(10.0, 5.0);

    /// The path of a bullet that is fired from a place at another, at 10 m/s.
    fn shot(field: &Field, from: Vec2, at: Vec2) -> Path {
        let flying = bullet((at - from).normalize() * 10.0, 0);
        Path::of(field, Entity::PLACEHOLDER, &flying, from)
    }

    #[test]
    fn a_tank_gets_out_of_the_way_of_a_bullet_when_it_has_the_time() {
        let field = room();
        let motion = Motion::default();
        // Bullets fly straight at a tank from this far east, and from this far north. What is
        // told of them is the least room they leave it, and when the first of them comes too
        // close, with where that one was fired from.
        let fares = |tank: Tank, way: Vec2, east: f32, north: Option<f32>| {
            let from = [Some(east * Vec2::X), north.map(|north| north * Vec2::Y)];
            let from = from.into_iter().flatten();
            let paths: Vec<Path> = from
                .map(|from| shot(&field, MIDDLE + from, MIDDLE))
                .collect();
            let paths: Vec<&Path> = paths.iter().collect();
            let mover = Mover::new(&field, &tank, MIDDLE, &motion);
            let fare = Fare::of(way, &mover.course(way), &paths);
            let first = fare.first.map(|(path, time)| (path.at - MIDDLE, time));
            (fare.room, first)
        };
        // A tank that drives north is out of the way of a bullet that comes from 9 m.
        let (room, first) = fares(tank(5, 6), Vec2::Y, 9.0, None);
        assert!(room > DODGE_ROOM);
        assert_eq!(first, None);
        // From 3 m, the bullet is there first.
        let (room, first) = fares(tank(5, 6), Vec2::Y, 3.0, None);
        assert!(room < 0.0);
        assert!(matches!(first, Some((_, time)) if time < 0.25));
        // A tank that stays is hit, and so is one without an engine, whichever way it is told.
        assert!(fares(tank(5, 6), Vec2::ZERO, 9.0, None).0 < 0.0);
        assert!(fares(tank(0, 0), Vec2::Y, 9.0, None).0 < 0.0);
        // Of two bullets, the one that gets to the tank first is told: the one from the
        // north here, which the tank drives into.
        let (_, first) = fares(tank(5, 6), Vec2::Y, 9.0, Some(4.5));
        assert!(matches!(first, Some((from, time)) if from == Vec2::Y * 4.5 && time < 0.4));
    }

    #[test]
    fn a_veteran_takes_the_nearest_way_on_which_no_bullet_gets_too_close() {
        let field = room();
        let (tank, motion) = (tank(5, 6), Motion::default());
        let mover = Mover::new(&field, &tank, MIDDLE, &motion);
        // The tank means to drive east.
        let (meant, plan) = (Vec2::X, mover.course(Vec2::X));
        assert_eq!(way_out(&mover, meant, &plan, Vec2::Y, &[]).way, meant);
        // A bullet comes from the east: the tank turns a little, to the north-east.
        let turned = |turn: f32| Vec2::from_angle(turn.to_radians());
        let east = shot(&field, MIDDLE + Vec2::X * 9.0, MIDDLE);
        let alone = way_out(&mover, meant, &plan, Vec2::Y, &[&east]);
        assert!(alone.way.distance(turned(45.0)) < 1e-5);
        assert!(alone.room >= DODGE_ROOM);
        // Another one comes from the north-east, which is in nobody's way further east: the
        // tank turns to the south-east instead.
        let north_east = shot(&field, Vec2::new(16.4, 9.9), MIDDLE);
        assert!(Fare::of(meant, &plan, &[&north_east]).room >= DODGE_ROOM);
        let both = way_out(&mover, meant, &plan, Vec2::Y, &[&east, &north_east]);
        assert!(both.way.distance(turned(-45.0)) < 1e-5);
        assert!(both.room >= DODGE_ROOM);
        // A bullet from 2 m leaves no way out: the tank takes the way that leaves the most
        // room, and knows the bullet that gets to it.
        let close = shot(&field, MIDDLE + Vec2::X * 2.0, MIDDLE);
        let hit = way_out(&mover, meant, &plan, Vec2::Y, &[&close]);
        assert!(hit.room < DODGE_ROOM);
        assert!(matches!(hit.first, Some((path, time)) if path.at == close.at && time < 0.3));
        let stays = Fare::of(Vec2::ZERO, &mover.course(Vec2::ZERO), &[&close]);
        assert!(hit.room > stays.room);

        // A tank that drives east along the north wall has no way out to the north-east: the
        // wall keeps it in the path of the bullet. It turns to the south-east.
        let at = Vec2::new(10.0, 8.8);
        let mover = Mover::new(&field, &tank, at, &motion);
        let plan = mover.course(meant);
        let east = shot(&field, at + Vec2::X * 9.0, at);
        let walled = way_out(&mover, meant, &plan, Vec2::Y, &[&east]);
        assert!(walled.way.distance(turned(-45.0)) < 1e-5);
        assert!(walled.room >= DODGE_ROOM);
    }

    #[test]
    fn a_bullet_behind_a_wall_is_not_fired_at() {
        let field = divided_room();
        // Over the wall that cuts the room in two, a tank and a bullet that flies at it from
        // the other half meet in the open.
        let (me, from) = (Vec2::new(5.0, 8.0), Vec2::new(15.0, 8.0));
        let meeting = answer(&field, me, &shot(&field, from, me), 10.0).unwrap();
        assert!(meeting.distance(Vec2::new(10.65, 8.0)) < 1e-2);
        // Further south the wall is between them, and the bullet will end on it.
        let (me, from) = (Vec2::new(5.0, 3.0), Vec2::new(15.0, 3.0));
        assert_eq!(answer(&field, me, &shot(&field, from, me), 10.0), None);
    }

    #[test]
    fn a_shot_meets_a_bullet_where_both_are_at_the_same_time() {
        // Head on, 10 m apart, both at 10 m/s. The shot starts at the tip of the barrel.
        let meeting = intercept(Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::NEG_X * 10.0, 10.0);
        assert!(meeting.unwrap().distance(Vec2::new(5.65, 0.0)) < 1e-3);
        // A bullet that flies across: the shot is on its way for as long as the bullet.
        let (at, velocity, speed) = (Vec2::new(8.0, -6.0), Vec2::new(-2.0, 9.0), 13.0);
        let meeting = intercept(Vec2::ZERO, at, velocity, speed).unwrap();
        let time = meeting.distance(at) / velocity.length();
        assert!((meeting.length() - (MUZZLE_DISTANCE + speed * time)).abs() < 1e-3);
        // A bullet that flies away faster than the shot is never met.
        let away = intercept(Vec2::ZERO, Vec2::new(5.0, 0.0), Vec2::X * 15.0, 9.0);
        assert_eq!(away, None);
        // Nor is one that is at the barrel already.
        let there = intercept(Vec2::ZERO, Vec2::new(1.0, 0.0), Vec2::NEG_X * 9.0, 9.0);
        assert_eq!(there, None);
    }

    #[test]
    fn a_place_between_two_walls_is_narrow() {
        let field = divided_room();
        // The gap above the divider is 4 m wide.
        assert!(narrow(&field, Vec2::new(10.0, 8.0)));
        // The middle of the west half is 5 m from every wall.
        assert!(!narrow(&field, Vec2::new(5.0, 5.0)));
        // A corner has walls on two sides, and they don't face each other.
        assert!(!narrow(&field, Vec2::new(1.5, 1.5)));
    }

    #[test]
    fn steering_goes_around_what_is_right_ahead() {
        let field = divided_room();
        let tank = Tank {
            genome: Genome::empty(0),
            team: Team::Enemies,
            sector: 0,
        };
        let from = Vec2::new(8.0, 3.0);
        assert_eq!(steer(&field, &tank, from, Vec2::NEG_X), Vec2::NEG_X);
        let around = steer(&field, &tank, from, Vec2::X);
        assert!(around != Vec2::X && around != Vec2::ZERO);
    }
}
