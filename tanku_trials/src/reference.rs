//! The reference players: the hands that hold the players' tank in a trial.
//!
//! A trial measures the enemies against a player who is always the same, so that two reports
//! differ by what was changed in the game and by nothing else. The three players here fight
//! the same way and differ in one thing, which is what they do about what flies at them:
//!
//! | | Novice | Regular | Expert |
//! |---|---|---|---|
//! | Sees a bullet coming | never | after 0.30 s | after 0.20 s |
//! | Follows a bullet off a wall | no | no | yes |
//! | Shield | never | when the bullet can't be dodged | the same |
//!
//! With the time that the standard tank takes to get out of the way, the regular player
//! reacts in about the 0.55 s that the specification counts for a player who keeps moving.
//!
//! All three keep 8 to 14 m from the nearest enemy and drive around it, lead their shots,
//! find a shot off a wall at an enemy that they don't see, and fire as fast as the gun does.
//! They find their way around what hides an enemy, and around what stands between them and
//! one that they see. When one or two enemies are left and none has fallen for twenty
//! seconds, they close in to 3 to 5 m, as a player does on a tank that dodges every shot or
//! shoots it down: from there a shot leaves it no time for either. Closing in costs them
//! tanks, and ends fights that would not end. They hold a shot that would come back to
//! their own tank off a wall, or go off within reach of it, and those who see bullets coming
//! know where their own go. None lays a mine, takes cover or picks the moment of a fight. They use nothing of the
//! brains of the enemies but how to aim and how to steer around a wall, so that a change to
//! how a brain dodges doesn't move the yardstick.

use bevy::prelude::*;
use tanku_core::{
    geom,
    rules::{BLAST_RADIUS, BULLET_RADIUS, MINE_TRIGGER_RADIUS, SPLIT_ANGLE, TANK_RADIUS, TICK},
    sim::{
        Bullet, Enemy, Gun, Kind, Mine, MineState, Motion, Position, Tank, TankCommand, Team, Wreck,
    },
};
use tanku_server::{
    brain,
    bullets::{self, Flight, Quarry},
    field::{Collider, Field, Solid},
    tanks,
};

/// How far ahead a player sees what a bullet will do, in ticks: one second.
const HORIZON: usize = 60;
/// How far a player sees a bullet from, in meters: about what the screen shows around a tank.
const VIEW: f32 = 20.0;
/// The room that a player wants between the tank and a bullet that passes, in meters.
const MARGIN: f32 = 0.25;
/// A player raises the shield when what can't be dodged is this many ticks away or fewer.
const SHIELD_AHEAD: usize = 12;
/// How far a player follows a shot before firing it, in ticks: three seconds.
const SHOT_AHEAD: usize = 180;
/// The room that a player wants around the tank from a shot of their own that comes back or
/// goes off, in meters.
const SHOT_ROOM: f32 = 1.0;
/// How far around the tank and its enemy a way between them is looked for, in meters.
const ROUTE_MARGIN: f32 = 25.0;
/// How far a player keeps from the enemy that it fights, in meters: no closer than the first
/// and no further than the second.
const KEEP: (f32, f32) = (8.0, 14.0);
/// How far it keeps from the last enemies of a fight that it can't end from afar: so close
/// that a shot leaves its target no time to get out of the way, or to shoot the bullet down.
const PRESS: (f32, f32) = (3.0, 5.0);
/// How long a player fights from afar without any enemy falling, in seconds, before it
/// closes in. A tank that falls to shots from afar falls sooner than that.
const PATIENCE: f32 = 20.0;
/// How many enemies are left, at most, when a player closes in. With more of them around,
/// closing in on one is driving into the fire of the others.
const LAST: usize = 2;
/// A way ends this close to the enemy, in meters: in sight of it, as a rule.
const ROUTE_REACH: f32 = 1.5;
/// A tank has got to a place on its way when it is this close to it, in meters.
const ROUTE_NEAR: f32 = 0.5;
/// The ways that a player tries around the one that was meant, in degrees.
const WAYS: [f32; 7] = [45.0, -45.0, 90.0, -90.0, 135.0, -135.0, 180.0];

/// One of the reference players.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reference {
    Novice,
    Regular,
    Expert,
}

/// What a reference player does about what flies at the tank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Skill {
    /// For how long a bullet must be in view before the player acts on it, in seconds. A
    /// player who has none never dodges.
    pub notice: Option<f32>,
    /// Whether the player sees where a bullet goes after a bounce before it bounces. One who
    /// doesn't takes a bullet that has bounced for a new one.
    pub follows: bool,
    /// Whether the player raises the shield against what can't be dodged.
    pub shields: bool,
}

impl Reference {
    pub const ALL: [Self; 3] = [Self::Novice, Self::Regular, Self::Expert];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Novice => "novice",
            Self::Regular => "regular",
            Self::Expert => "expert",
        }
    }

    pub fn of_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|player| player.name() == name)
    }

    pub const fn skill(self) -> Skill {
        match self {
            Self::Novice => Skill {
                notice: None,
                follows: false,
                shields: false,
            },
            Self::Regular => Skill {
                notice: Some(0.30),
                follows: false,
                shields: true,
            },
            Self::Expert => Skill {
                notice: Some(0.20),
                follows: true,
                shields: true,
            },
        }
    }
}

/// What a player sees of a fight in a tick, besides the tanks: the bullets, the mines, and
/// the tanks that a seeking bullet may leave a wall toward.
pub struct Sight {
    pub bullets: Vec<(Entity, Bullet, Vec2)>,
    pub mines: Vec<(Mine, Vec2)>,
    pub quarry: Vec<Quarry>,
    /// How many enemies are awake.
    pub awake: usize,
}

impl Sight {
    pub fn of(world: &mut World) -> Self {
        let mut bullets = world.query::<(Entity, &Bullet, &Position)>();
        let mut mines = world.query::<(&Mine, &Position)>();
        let mut tanks = world.query::<(&Tank, &Position, Option<&Enemy>, Has<Wreck>)>();
        let mut enemies = world.query::<&Enemy>();
        Self {
            awake: enemies.iter(world).filter(|enemy| enemy.awake).count(),
            bullets: bullets
                .iter(world)
                .map(|(entity, bullet, position)| (entity, *bullet, position.0))
                .collect(),
            mines: mines
                .iter(world)
                .map(|(mine, position)| (*mine, position.0))
                .collect(),
            quarry: bullets::quarry(tanks.iter(world)),
        }
    }
}

/// Where a bullet will be, tick after tick, as far as a player sees it go.
#[derive(Clone, Debug, PartialEq)]
struct Track {
    /// The tick at which it starts, counted from now: later than now for the two bullets
    /// that a splitting bullet becomes.
    from: usize,
    places: Vec<Vec2>,
    /// How many of its first stretches can't hit the tank: those that a bullet of the player
    /// flies before its first bounce.
    safe: usize,
    /// Where it goes off when it gets to the last of its places, if it explodes.
    blast: Option<Vec2>,
}

impl Track {
    /// Follows a bullet from where it is, and the two that it becomes if it splits. A player
    /// who doesn't follow bounces sees it as far as the first wall that it bounces off.
    fn follow(
        field: &Field,
        mut bullet: Bullet,
        mut at: Vec2,
        from: usize,
        quarry: &[Quarry],
        follows: bool,
        tracks: &mut Vec<Track>,
    ) {
        let step = bullet.velocity.length() * TICK;
        let own = bullet.team == Team::Players && !bullet.bounced;
        let mut track = Track {
            from,
            places: vec![at],
            safe: if own { usize::MAX } else { 0 },
            blast: None,
        };
        for tick in from..HORIZON {
            // The two ways of a splitting bullet are drawn ahead of it: every player sees them.
            if let Kind::Splitting { fuse, bounces } = bullet.kind {
                let fuse = fuse - TICK;
                bullet.kind = Kind::Splitting { fuse, bounces };
                if fuse <= 0.0 {
                    for side in [-1.0, 1.0] {
                        let way = Vec2::from_angle(side * SPLIT_ANGLE).rotate(bullet.velocity);
                        let half = bullet.half(way, bounces);
                        Self::follow(field, half, at, tick, quarry, follows, tracks);
                    }
                    break;
                }
            }
            let flight = bullets::fly_past_walls(field, &mut at, &mut bullet, step, quarry);
            if let Flight::Stopped(_) = flight.end {
                track.places.push(at);
                track.blast = (bullet.kind == Kind::Exploding).then_some(at);
                break;
            }
            match flight.bounces.first() {
                Some(&bounce) if !follows => {
                    track.places.push(bounce);
                    break;
                }
                bounce => {
                    // From its first bounce on, a shot of the player can hit their own tank.
                    if bounce.is_some() && track.safe == usize::MAX {
                        track.safe = track.places.len() - 1;
                    }
                    track.places.push(at);
                }
            }
        }
        tracks.push(track);
    }

    /// The least room that the bullet leaves a tank on its course, and the first tick at
    /// which it leaves none. The course holds the place of the tank at each tick from now.
    fn room(&self, course: &[Vec2]) -> (f32, Option<usize>) {
        let (mut least, mut hit) = (f32::MAX, None);
        let mut leave = |room: f32, tick: usize| {
            least = least.min(room);
            if room < 0.0 && hit.is_none() {
                hit = Some(tick);
            }
        };
        for (flown, stretch) in self.places.windows(2).enumerate().skip(self.safe) {
            // In a tick, the tank drives first, and the bullet then flies at where it is.
            let tick = self.from + flown + 1;
            let Some(&tank) = course.get(tick) else {
                break;
            };
            let apart = geom::distance_to_segment(tank, stretch[0], stretch[1]);
            leave(apart - TANK_RADIUS - BULLET_RADIUS, tick);
        }
        if let Some(blast) = self.blast {
            let tick = (self.from + self.places.len() - 1).min(course.len() - 1);
            leave(course[tick].distance(blast) - BLAST_RADIUS, tick);
        }
        (least, hit)
    }
}

/// A mine of the enemies, as a player minds it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hazard {
    at: Vec2,
    /// The tick at which it blows, if it warns. A mine that doesn't yet is kept away from.
    blows: Option<usize>,
}

impl Hazard {
    fn room(&self, course: &[Vec2]) -> (f32, Option<usize>) {
        match self.blows {
            Some(tick) => {
                let tick = tick.min(course.len() - 1);
                let room = course[tick].distance(self.at) - BLAST_RADIUS;
                (room, (room < 0.0).then_some(tick))
            }
            // Driving within its reach sets it off, which destroys nothing yet.
            None => {
                let near = course.iter().take(HORIZON / 2);
                let apart = near.map(|place| place.distance(self.at));
                let least = apart.fold(f32::MAX, f32::min);
                (least - MINE_TRIGGER_RADIUS, None)
            }
        }
    }
}

/// A bullet that a player has in view.
#[derive(Clone, Copy, Debug)]
struct Seen {
    bullet: Entity,
    bounces_left: u8,
    /// For how long the player has seen it on the way that it is on.
    since: f32,
}

/// A way that a player means to drive.
enum Way {
    /// A way that the player would like: a wall right ahead turns the tank from it.
    Meant(Vec2),
    /// The way to a place that the tank can drive straight to, as far as that place.
    Found(Vec2),
}

/// The hands of a reference player on a tank, for one fight.
pub struct Pilot {
    skill: Skill,
    /// The least time between two of its shots: what the gun allows.
    pace: f32,
    since_shot: f32,
    strafe: f32,
    /// Time since it last changed the way it drives around its enemy.
    turned: f32,
    bank_shot: Option<f32>,
    bank_shot_timer: f32,
    /// Where it drives to get around what hides its enemy, and the time before it looks for
    /// the way again.
    waypoint: Option<Vec2>,
    route_timer: f32,
    seen: Vec<Seen>,
    /// How many enemies were awake a tick ago, and for how long none of them has fallen
    /// while there was one to fight.
    awake: usize,
    since_fall: f32,
}

impl Pilot {
    pub fn new(player: Reference) -> Self {
        Self {
            skill: player.skill(),
            pace: 0.25,
            since_shot: 0.25,
            strafe: 1.0,
            turned: 0.0,
            bank_shot: None,
            bank_shot_timer: 0.0,
            waypoint: None,
            route_timer: 0.0,
            seen: Vec::new(),
            awake: 0,
            since_fall: 0.0,
        }
    }

    /// What the player asks of the tank this tick, against the enemy that it fights, if one
    /// is left.
    pub fn order(
        &mut self,
        world: &World,
        sight: &Sight,
        entity: Entity,
        enemy: Option<Entity>,
    ) -> TankCommand {
        let field = world.resource::<Field>();
        let tank = world.get::<Tank>(entity).unwrap();
        let me = world.get::<Position>(entity).unwrap().0;
        let motion = *world.get::<Motion>(entity).unwrap();
        let mut command = TankCommand {
            aim: me + Vec2::from_angle(motion.turret) * 10.0,
            ..default()
        };
        let mut meant = Vec2::ZERO;
        // A fight that goes on without any enemy falling is one to fight another way.
        if sight.awake != self.awake || enemy.is_none() {
            (self.awake, self.since_fall) = (sight.awake, 0.0);
        } else {
            self.since_fall += TICK;
        }
        if let Some(enemy) = enemy {
            let foe = world.get::<Position>(enemy).unwrap().0;
            let foe_velocity = world.get::<Motion>(enemy).unwrap().velocity;
            let rounds = world.get::<Gun>(entity).unwrap().rounds;
            let aim = self.aim(field, tank, me, motion.turret, foe, foe_velocity);
            command.aim = me + Vec2::from_angle(aim.0) * 10.0;
            // A player holds a shot that would come back, or go off too close.
            command.fire = aim.1
                && rounds > 0
                && self.since_shot >= self.pace
                && sound_shot(field, entity, tank, me, motion.turret, foe, &sight.quarry);
            let pressing = self.since_fall > PATIENCE && sight.awake <= LAST;
            meant = match self.around(field, tank, me, foe, pressing) {
                // The way around what hides the enemy is free as far as it was found: a
                // tank that looked further ahead would turn back in front of the wall that
                // the way runs along.
                Way::Found(way) => way.normalize_or_zero(),
                Way::Meant(way) => {
                    let steered = brain::steer(field, tank, me, way);
                    // An enemy that shows past what stands right in front of the tank is
                    // not driven at: the tank would turn back from what is in its way. It
                    // goes around, as it does when the enemy is hidden.
                    let toward = (foe - me).normalize_or_zero();
                    let turned_back = way.dot(toward) > 0.9 && steered.dot(toward) <= 0.0;
                    match turned_back.then(|| self.way_around(field, tank, me, foe)) {
                        Some(Way::Found(around)) => around.normalize_or_zero(),
                        _ => steered,
                    }
                }
            };
        }
        self.since_shot = if command.fire {
            0.0
        } else {
            self.since_shot + TICK
        };

        command.drive = meant;
        let known = self.watch(sight, me);
        // A player who never sees a bullet coming minds no mine either.
        let minds = self.skill.notice.is_some();
        if minds && (!known.is_empty() || !sight.mines.is_empty()) {
            let mut tracks = Vec::new();
            for &(_, bullet, at) in &known {
                // Where their own shot goes off a wall is news to no player.
                let follows = self.skill.follows || bullet.team == Team::Players;
                Track::follow(field, bullet, at, 0, &sight.quarry, follows, &mut tracks);
            }
            let hazards: Vec<Hazard> = sight.mines.iter().filter_map(hazard).collect();
            let (way, hit) = way_out(field, tank, me, &motion, meant, &tracks, &hazards);
            command.drive = way;
            command.shield = self.skill.shields && hit.is_some_and(|tick| tick <= SHIELD_AHEAD);
        }
        command
    }

    /// The turret angle of its shot at an enemy, and whether the gun is on it.
    fn aim(
        &mut self,
        field: &Field,
        tank: &Tank,
        me: Vec2,
        turret: f32,
        foe: Vec2,
        foe_velocity: Vec2,
    ) -> (f32, bool) {
        let target = brain::lead(me, foe, foe_velocity, tank.genome.bullet_speed());
        let mut aim = (target - me).to_angle();
        let mut solution = field.sees(me, target);
        if solution {
            self.bank_shot = None;
        } else {
            self.bank_shot_timer -= TICK;
            if self.bank_shot_timer <= 0.0 {
                self.bank_shot_timer = 0.3;
                let bounces = tank.genome.bounces();
                self.bank_shot = brain::bank_shot(field, me, target, bounces, turret);
            }
            if let Some(angle) = self.bank_shot {
                aim = angle;
                solution = true;
            }
        }
        let aligned = geom::angle_difference(turret, aim).abs() < 2.0f32.to_radians();
        (aim, solution && aligned)
    }

    /// The way that it means to drive: toward an enemy that it doesn't see or that is far,
    /// away from one that is close, and around it otherwise, one way and then the other.
    fn around(&mut self, field: &Field, tank: &Tank, me: Vec2, foe: Vec2, pressing: bool) -> Way {
        self.turned += TICK;
        if self.turned >= 2.0 {
            self.turned = 0.0;
            self.strafe = -self.strafe;
        }
        let distance = me.distance(foe);
        let toward = (foe - me) / distance.max(0.01);
        let (near, far) = if pressing { PRESS } else { KEEP };
        if !field.sees(me, foe) {
            self.way_around(field, tank, me, foe)
        } else if distance > far {
            Way::Meant(toward)
        } else if distance < near {
            Way::Meant(-toward)
        } else {
            Way::Meant(toward.perp() * self.strafe)
        }
    }

    /// The way around what hides an enemy, or what stands between the tank and it. The way is
    /// looked for again now and then.
    fn way_around(&mut self, field: &Field, tank: &Tank, me: Vec2, foe: Vec2) -> Way {
        self.route_timer -= TICK;
        // A tank that has got to where it was going looks at once.
        let there = |waypoint: Vec2| waypoint.distance(me) < ROUTE_NEAR;
        if self.route_timer <= 0.0 || self.waypoint.is_some_and(there) {
            self.route_timer = 0.5;
            self.waypoint = route(field, tank, me, foe);
        }
        match self.waypoint {
            Some(waypoint) => Way::Found(waypoint - me),
            None => Way::Meant(foe - me),
        }
    }

    /// Keeps count of the bullets in view, and tells those that the player has seen for long
    /// enough to act on. Its own are among them: they come back off the walls.
    fn watch(&mut self, sight: &Sight, me: Vec2) -> Vec<(Entity, Bullet, Vec2)> {
        let Some(notice) = self.skill.notice else {
            return Vec::new();
        };
        let in_view = |(_, _, at): &&(Entity, Bullet, Vec2)| at.distance(me) <= VIEW;
        let near: Vec<_> = sight.bullets.iter().filter(in_view).copied().collect();
        self.seen
            .retain(|seen| near.iter().any(|(entity, ..)| *entity == seen.bullet));
        let mut known = Vec::new();
        for &(entity, bullet, at) in &near {
            let seen = match self.seen.iter_mut().find(|seen| seen.bullet == entity) {
                Some(seen) => seen,
                None => {
                    self.seen.push(Seen {
                        bullet: entity,
                        bounces_left: bullet.bounces_left,
                        since: 0.0,
                    });
                    self.seen.last_mut().unwrap()
                }
            };
            // Where a bullet goes off a wall is news to a player who didn't follow it there.
            if seen.bounces_left != bullet.bounces_left {
                seen.bounces_left = bullet.bounces_left;
                if !self.skill.follows && bullet.team != Team::Players {
                    seen.since = 0.0;
                }
            }
            if seen.since >= notice || bullet.team == Team::Players {
                known.push((entity, bullet, at));
            }
            seen.since += TICK;
        }
        known
    }
}

/// Whether a shot that a tank fires now is one that a player would fire: it gets to the enemy
/// before it comes back to the tank off a wall, and doesn't go off within reach of the tank,
/// in a crate or by itself.
fn sound_shot(
    field: &Field,
    entity: Entity,
    tank: &Tank,
    me: Vec2,
    turret: f32,
    foe: Vec2,
    quarry: &[Quarry],
) -> bool {
    let reach = TANK_RADIUS + BULLET_RADIUS;
    let mut bullet = Bullet::fired(
        entity,
        tank,
        Vec2::from_angle(turret) * tank.genome.bullet_speed(),
    );
    let step = bullet.velocity.length() * TICK;
    let mut at = me;
    for _ in 0..SHOT_AHEAD {
        let flight = bullets::fly_past_walls(field, &mut at, &mut bullet, step, quarry);
        if at.distance(foe) < reach {
            return true;
        }
        if let Flight::Stopped(solid) = flight.end {
            let blast = matches!(solid, Solid::Crate(_)) || bullet.kind == Kind::Exploding;
            return !(blast && at.distance(me) < BLAST_RADIUS + SHOT_ROOM);
        }
        if bullet.bounced && at.distance(me) < reach + SHOT_ROOM {
            return false;
        }
    }
    true
}

/// Where a tank drives to get to a place that it doesn't see: the furthest point that it
/// can drive straight to on the shortest way there, around what is solid. The way is found
/// on a grid of one meter, among the squares that a tank fits in.
fn route(field: &Field, tank: &Tank, from: Vec2, to: Vec2) -> Option<Vec2> {
    let low = (from.min(to) - Vec2::splat(ROUTE_MARGIN)).floor();
    let size = ((from.max(to) + Vec2::splat(ROUTE_MARGIN)).ceil() - low).as_uvec2();
    let (width, height) = (size.x as usize, size.y as usize);
    let center = |cell: usize| low + Vec2::new((cell % width) as f32, (cell / width) as f32) + 0.5;
    let cell = |at: Vec2| {
        let within = (at - low).floor().clamp(Vec2::ZERO, (size - 1).as_vec2());
        within.x as usize + within.y as usize * width
    };
    // The squares that a tank doesn't fit in: those too close to something solid.
    let room = TANK_RADIUS + 0.05;
    let mut solid = vec![false; width * height];
    for collider in field
        .colliders
        .iter()
        .filter(|collider| collider.solid.stops(tank))
    {
        let first = (collider.a.min(collider.b) - low - room)
            .floor()
            .max(Vec2::ZERO);
        let last = (collider.a.max(collider.b) - low + room)
            .ceil()
            .min(size.as_vec2());
        for y in first.y as usize..last.y.max(0.0) as usize {
            for x in first.x as usize..last.x.max(0.0) as usize {
                let at = low + Vec2::new(x as f32, y as f32) + 0.5;
                if geom::distance_to_segment(at, collider.a, collider.b) < room {
                    solid[x + y * width] = true;
                }
            }
        }
    }
    let (start, goal) = (cell(from), to);
    let mut before: Vec<Option<usize>> = vec![None; width * height];
    let mut open = std::collections::VecDeque::from([start]);
    before[start] = Some(start);
    let mut end = None;
    while let Some(here) = open.pop_front() {
        if center(here).distance(goal) < ROUTE_REACH {
            end = Some(here);
            break;
        }
        let (x, y) = (here % width, here / width);
        let next = [
            (x > 0).then(|| here - 1),
            (x + 1 < width).then(|| here + 1),
            (y > 0).then(|| here - width),
            (y + 1 < height).then(|| here + width),
        ];
        for next in next.into_iter().flatten() {
            if !solid[next] && before[next].is_none() {
                before[next] = Some(here);
                open.push_back(next);
            }
        }
    }
    // Back from the end of the way to the first square that the tank can drive straight to.
    let mut here = end?;
    let mut first = here;
    while here != start {
        let stopped = field
            .cast(from, center(here) - from, TANK_RADIUS)
            .is_some_and(|(_, solid)| solid.stops(tank));
        if !stopped {
            return Some(center(here));
        }
        first = here;
        here = before[here]?;
    }
    Some(center(first))
}

/// A mine as a player minds it: one of the enemies, which a player's tank sets off.
fn hazard((mine, at): &(Mine, Vec2)) -> Option<Hazard> {
    let blows = match mine.state {
        MineState::Triggered(left) => Some((left / TICK).ceil() as usize),
        MineState::Arming(_) | MineState::Armed(_) => None,
    };
    (mine.team == Team::Enemies || blows.is_some()).then_some(Hazard { at: *at, blows })
}

/// Where a tank is, tick after tick from now, when it drives one way.
fn course(near: &[Collider], tank: &Tank, at: Vec2, motion: &Motion, way: Vec2) -> Vec<Vec2> {
    let command = TankCommand {
        drive: way,
        aim: at,
        ..default()
    };
    let (mut at, mut motion) = (at, *motion);
    let mut places = vec![at];
    for _ in 0..HORIZON {
        tanks::step_among(near, tank, true, &command, &mut at, &mut motion);
        places.push(at);
    }
    places
}

/// The way that a player drives among what it knows of: the way that was meant when that
/// leaves room enough, or else the first of the ways around it that does, or standing still.
/// Failing that, the way that leaves the most room. Tells the tick at which the tank is hit
/// on that way, if it is.
fn way_out(
    field: &Field,
    tank: &Tank,
    at: Vec2,
    motion: &Motion,
    meant: Vec2,
    tracks: &[Track],
    hazards: &[Hazard],
) -> (Vec2, Option<usize>) {
    let reach = tank.genome.top_speed() * HORIZON as f32 * TICK + TANK_RADIUS + 0.5;
    let close =
        |collider: &&Collider| geom::distance_to_segment(at, collider.a, collider.b) < reach;
    let near: Vec<Collider> = field.colliders.iter().filter(close).copied().collect();
    let fare = |way: Vec2| {
        let course = course(&near, tank, at, motion, way);
        let rooms = tracks.iter().map(|track| track.room(&course));
        let rooms = rooms.chain(hazards.iter().map(|hazard| hazard.room(&course)));
        rooms.fold((f32::MAX, None), |(least, first), (room, hit)| {
            let sooner = match (first, hit) {
                (Some(first), Some(hit)) => Some(usize::min(first, hit)),
                (first, hit) => first.or(hit),
            };
            (least.min(room), sooner)
        })
    };
    let ahead = meant.try_normalize().unwrap_or(Vec2::Y);
    let turned = WAYS.map(|turn| Vec2::from_angle(turn.to_radians()).rotate(ahead));
    let mut ways = vec![meant];
    ways.extend(turned);
    if meant != Vec2::ZERO {
        ways.push(Vec2::ZERO);
    }
    let mut best = (meant, f32::MIN, None);
    for way in ways {
        let (room, hit) = fare(way);
        if room > best.1 {
            best = (way, room, hit);
        }
        if room >= MARGIN {
            break;
        }
    }
    (best.0, best.2)
}

#[cfg(test)]
mod tests {
    use tanku_core::stats::Genome;

    use super::*;

    /// A field with one wall, which runs from west to east ten meters north of the middle.
    fn field() -> Field {
        let mut field = Field::new(1, 1, tanku_core::modes::Difficulty::Normal);
        field.colliders.push(Collider {
            a: Vec2::new(-30.0, 10.0),
            b: Vec2::new(30.0, 10.0),
            solid: Solid::Wall,
        });
        field
    }

    fn bullet(velocity: Vec2, bounces_left: u8, kind: Kind) -> Bullet {
        Bullet {
            owner: Entity::PLACEHOLDER,
            team: Team::Enemies,
            sector: 0,
            velocity,
            bounces_left,
            pierces_left: 0,
            bounced: false,
            kind,
        }
    }

    fn tracks(bullet: Bullet, at: Vec2, follows: bool) -> Vec<Track> {
        let mut tracks = Vec::new();
        Track::follow(&field(), bullet, at, 0, &[], follows, &mut tracks);
        tracks
    }

    fn player() -> Tank {
        Tank {
            genome: Genome::PLAYER,
            team: Team::Players,
            sector: 0,
        }
    }

    /// A world with the tank of a player at one place and an enemy at another, awake, on a
    /// field that holds these walls and these blocks, each a square of 2 m from its lowest
    /// corner.
    fn fight(
        walls: &[(Vec2, Vec2)],
        blocks: &[Vec2],
        me: Vec2,
        foe: Vec2,
    ) -> (World, Entity, Entity) {
        let mut field = Field::new(1, 1, tanku_core::modes::Difficulty::Normal);
        field.colliders.clear();
        let mut wall = |a: Vec2, b: Vec2| {
            let solid = Solid::Wall;
            field.colliders.push(Collider { a, b, solid });
        };
        for &(a, b) in walls {
            wall(a, b);
        }
        for &corner in blocks {
            let corners = [Vec2::ZERO, Vec2::X * 2.0, Vec2::splat(2.0), Vec2::Y * 2.0];
            for side in 0..4 {
                wall(corner + corners[side], corner + corners[(side + 1) % 4]);
            }
        }
        let mut world = World::new();
        world.insert_resource(field);
        let tank = player();
        let gun = Gun::loaded(&tank.genome);
        let place = |at: Vec2| (Position(at), Motion::default());
        let player = world.spawn((tank, gun, place(me))).id();
        let enemy = Tank {
            genome: Genome::empty(40),
            team: Team::Enemies,
            sector: 0,
        };
        let enemy = world.spawn((enemy, Enemy { awake: true }, place(foe))).id();
        (world, player, enemy)
    }

    /// What a player sees when nothing flies and nothing lies around, with so many enemies
    /// awake.
    fn quiet(awake: usize) -> Sight {
        Sight {
            bullets: Vec::new(),
            mines: Vec::new(),
            quarry: Vec::new(),
            awake,
        }
    }

    #[test]
    fn a_player_closes_in_on_the_last_enemies_of_a_fight_that_does_not_end() {
        // The enemy is 10 m north, in the open: a distance that a player keeps.
        let (world, player, enemy) = fight(&[], &[], Vec2::ZERO, Vec2::new(0.0, 10.0));
        let toward = Vec2::Y;
        let drives = |pilot: &mut Pilot, awake: usize, seconds: f32| {
            let mut drive = Vec2::ZERO;
            for _ in 0..(seconds / TICK).round() as usize {
                drive = pilot
                    .order(&world, &quiet(awake), player, Some(enemy))
                    .drive;
            }
            drive.normalize_or_zero().dot(toward)
        };
        // It drives around the enemy, for as long as tanks that fall to that do.
        let mut pilot = Pilot::new(Reference::Regular);
        assert!(drives(&mut pilot, 1, 1.0).abs() < 0.1);
        assert!(drives(&mut pilot, 1, PATIENCE - 2.0).abs() < 0.1);
        // Then it closes in on the one that didn't.
        assert!(drives(&mut pilot, 1, 2.0) > 0.9);
        // So it does on the last two, and not among more: that is driving into their fire.
        let mut pilot = Pilot::new(Reference::Regular);
        assert!(drives(&mut pilot, LAST, PATIENCE + 1.0) > 0.9);
        let mut pilot = Pilot::new(Reference::Regular);
        assert!(drives(&mut pilot, LAST + 1, 3.0 * PATIENCE).abs() < 0.1);
        // An enemy that falls gives the others their time again, and so does one that comes.
        assert!(drives(&mut pilot, LAST, 1.0).abs() < 0.1);
        assert!(drives(&mut pilot, LAST, PATIENCE) > 0.9);
        assert!(drives(&mut pilot, LAST + 1, 1.0).abs() < 0.1);
        // Every player does, the one who dodges nothing too.
        let mut pilot = Pilot::new(Reference::Novice);
        assert!(drives(&mut pilot, 1, PATIENCE + 1.0) > 0.9);
    }

    #[test]
    fn a_player_who_closes_in_stops_a_few_meters_from_its_enemy() {
        let way = |distance: f32| {
            let (world, player, enemy) = fight(&[], &[], Vec2::ZERO, Vec2::Y * distance);
            let mut pilot = Pilot::new(Reference::Expert);
            let mut drive = Vec2::ZERO;
            for _ in 0..((PATIENCE + 1.0) / TICK) as usize {
                drive = pilot.order(&world, &quiet(1), player, Some(enemy)).drive;
            }
            drive.normalize_or_zero().dot(Vec2::Y)
        };
        assert!(way(20.0) > 0.9);
        assert!(way(PRESS.1 + 1.0) > 0.9);
        assert!(way((PRESS.0 + PRESS.1) / 2.0).abs() < 0.1);
        assert!(way(PRESS.0 - 1.0) < -0.9);
    }

    #[test]
    fn a_player_drives_straight_to_where_its_way_around_leads() {
        // A gap of 3 m between a wall to the south and a block: the way to an enemy that
        // the block hides leads through it. A tank that looked 2.5 m ahead on that way would
        // see the wall, and turn back.
        let walls = [
            (Vec2::new(28.0, 0.0), Vec2::new(0.0, 0.0)),
            (Vec2::new(0.0, 0.0), Vec2::new(0.0, 4.0)),
            (Vec2::new(0.0, 4.0), Vec2::new(-14.0, 4.0)),
        ];
        let blocks = [Vec2::new(4.0, 3.0), Vec2::new(4.0, 5.0)];
        let (me, foe) = (Vec2::new(2.94, 2.91), Vec2::new(33.0, 4.0));
        let (world, player, enemy) = fight(&walls, &blocks, me, foe);
        let field = world.resource::<Field>();
        assert!(!field.sees(me, foe));
        let waypoint = route(field, &self::player(), me, foe).expect("a way through the gap");
        assert!(waypoint.y < me.y && waypoint.y > 0.9, "{waypoint:?}");
        let through = (waypoint - me).normalize();
        let steered = brain::steer(field, &self::player(), me, through);
        assert!(steered.dot(through) < 0.0, "{steered:?}");
        let mut pilot = Pilot::new(Reference::Regular);
        let drive = pilot.order(&world, &quiet(1), player, Some(enemy)).drive;
        assert!(drive.dot(through) > 0.99, "{drive:?}");
    }

    #[test]
    fn a_player_goes_around_what_stands_between_it_and_an_enemy_that_it_sees() {
        // The tank stands in a corner, a block right in front of it, and the enemy shows
        // past the edge of the block. Driving at the enemy is driving into the block, and
        // every way that a tank turns to from there is walled in.
        let walls = [
            (Vec2::new(-1.5, -2.0), Vec2::new(-1.5, 6.0)),
            (Vec2::new(-1.5, -2.0), Vec2::new(12.0, -2.0)),
        ];
        let blocks = [Vec2::new(0.05, 1.2)];
        let (me, foe) = (Vec2::ZERO, Vec2::new(-0.5, 30.0));
        let (world, player, enemy) = fight(&walls, &blocks, me, foe);
        let field = world.resource::<Field>();
        assert!(field.sees(me, foe));
        let toward = (foe - me).normalize();
        let steered = brain::steer(field, &self::player(), me, toward);
        assert!(steered.dot(toward) <= 0.0, "{steered:?}");
        // It goes around the block: east first, out of its corner.
        let mut pilot = Pilot::new(Reference::Regular);
        let mut drive = Vec2::ZERO;
        for _ in 0..((PATIENCE + 1.0) / TICK) as usize {
            drive = pilot.order(&world, &quiet(1), player, Some(enemy)).drive;
        }
        assert!(drive.x > 0.5, "{drive:?}");
    }

    #[test]
    fn the_players_are_told_by_their_names() {
        for player in Reference::ALL {
            assert_eq!(Reference::of_name(player.name()), Some(player));
        }
        assert_eq!(Reference::of_name("master"), None);
        assert_eq!(Reference::Novice.skill().notice, None);
    }

    #[test]
    fn a_bullet_is_followed_off_a_wall_by_a_player_who_follows_bounces() {
        // It flies north at 12 m/s from 4 m under the wall: it bounces within half a second.
        let shot = bullet(Vec2::new(0.0, 12.0), 1, Kind::Plain);
        let followed = tracks(shot, Vec2::new(0.0, 6.0), true);
        let lost = tracks(shot, Vec2::new(0.0, 6.0), false);
        let end = |tracks: &[Track]| *tracks[0].places.last().unwrap();
        assert_eq!((followed.len(), lost.len()), (1, 1));
        assert!(end(&followed).y < 6.0, "{:?}", end(&followed));
        assert!((end(&lost).y - (10.0 - BULLET_RADIUS)).abs() < 0.01);
    }

    #[test]
    fn a_shot_of_the_player_hits_nothing_of_theirs_before_it_has_bounced() {
        // The tank stands on the line of its own shot, which comes back off the wall.
        let own = Bullet {
            team: Team::Players,
            ..bullet(Vec2::new(0.0, 12.0), 1, Kind::Plain)
        };
        let stood = vec![Vec2::new(0.0, 6.0); HORIZON + 1];
        let back = &tracks(own, Vec2::new(0.0, 7.5), true)[0];
        assert!(back.room(&stood).1.is_some_and(|tick| tick > 20));
        // A player who doesn't follow it sees no further than the wall.
        let lost = &tracks(own, Vec2::new(0.0, 7.5), false)[0];
        assert_eq!(lost.room(&stood), (f32::MAX, None));
    }

    #[test]
    fn a_splitting_bullet_is_followed_on_both_of_its_ways() {
        let kind = Kind::Splitting {
            fuse: 0.5,
            bounces: 0,
        };
        let split = tracks(bullet(Vec2::new(8.0, 0.0), 0, kind), Vec2::ZERO, false);
        assert_eq!(split.len(), 3);
        // The two halves start when the bullet splits, 4 m further, and part from there.
        let halves: Vec<&Track> = split.iter().filter(|track| track.from > 0).collect();
        assert_eq!(halves.len(), 2);
        assert!((halves[0].places[0].x - 4.0).abs() < 0.2);
        let ends: Vec<f32> = halves
            .iter()
            .map(|half| half.places.last().unwrap().y)
            .collect();
        assert!(ends[0] * ends[1] < 0.0, "{ends:?}");
    }

    #[test]
    fn an_exploding_bullet_leaves_no_room_around_where_it_ends() {
        let shot = bullet(Vec2::new(0.0, 12.0), 0, Kind::Exploding);
        let track = &tracks(shot, Vec2::new(0.0, 6.0), false)[0];
        assert!(track.blast.is_some());
        // A tank that stands 2 m from the wall, 5 m east of the line of the shot, is not in
        // the way of the bullet, and is within the blast.
        let near = vec![Vec2::new(2.0, 8.0); HORIZON + 1];
        let far = vec![Vec2::new(5.0, 8.0); HORIZON + 1];
        assert!(track.room(&near).1.is_some());
        assert_eq!(track.room(&far).1, None);
    }

    #[test]
    fn a_player_drives_out_of_the_way_of_a_bullet_that_would_hit() {
        let field = field();
        let tank = player();
        let motion = Motion::default();
        // The bullet comes from the east, 9 m away at 9 m/s, straight at the tank.
        let shot = bullet(Vec2::new(-9.0, 0.0), 0, Kind::Plain);
        let mut coming = Vec::new();
        Track::follow(
            &field,
            shot,
            Vec2::new(9.0, 0.0),
            0,
            &[],
            false,
            &mut coming,
        );
        let (way, hit) = way_out(&field, &tank, Vec2::ZERO, &motion, Vec2::ZERO, &coming, &[]);
        assert_eq!(hit, None);
        assert!(way.y.abs() > 0.5, "{way:?}");
        // From 2.5 m, nothing gets the tank out of its way in time: the shield is for that.
        let mut close = Vec::new();
        Track::follow(&field, shot, Vec2::new(2.5, 0.0), 0, &[], false, &mut close);
        let (_, hit) = way_out(&field, &tank, Vec2::ZERO, &motion, Vec2::ZERO, &close, &[]);
        assert!(hit.is_some());
    }

    #[test]
    fn a_player_holds_a_shot_that_would_come_back() {
        let field = field();
        let tank = player();
        let me = Vec2::new(0.0, 4.0);
        let north = std::f32::consts::FRAC_PI_2;
        let fired = |turret: f32, foe: Vec2| {
            sound_shot(&field, Entity::PLACEHOLDER, &tank, me, turret, foe, &[])
        };
        // Straight at the wall, the shot comes straight back, unless the enemy is in its way.
        assert!(!fired(north, Vec2::new(20.0, 4.0)));
        assert!(fired(north, Vec2::new(0.0, 8.0)));
        // At an angle, it comes down far from the tank.
        assert!(fired(north / 2.0, Vec2::new(20.0, 4.0)));
    }

    #[test]
    fn a_player_finds_the_way_around_a_wall() {
        // A wall from west to east, with its east end at x = 6: the enemy is behind it.
        let mut field = Field::new(1, 1, tanku_core::modes::Difficulty::Normal);
        field.colliders.push(Collider {
            a: Vec2::new(-30.0, 10.0),
            b: Vec2::new(6.0, 10.0),
            solid: Solid::Wall,
        });
        let (me, foe) = (Vec2::new(0.0, 5.0), Vec2::new(0.0, 15.0));
        assert!(!field.sees(me, foe));
        let waypoint = route(&field, &player(), me, foe).unwrap();
        // The way goes east, around the end of the wall, and the tank can drive there.
        assert!(waypoint.x > 6.0 + TANK_RADIUS, "{waypoint:?}");
        assert!(field.cast(me, waypoint - me, TANK_RADIUS).is_none());
        // No way leads into a place that is closed all around.
        for (a, b) in [
            ((-3.0, 12.0), (3.0, 12.0)),
            ((3.0, 12.0), (3.0, 18.0)),
            ((3.0, 18.0), (-3.0, 18.0)),
            ((-3.0, 18.0), (-3.0, 12.0)),
        ] {
            field.colliders.push(Collider {
                a: a.into(),
                b: b.into(),
                solid: Solid::Wall,
            });
        }
        assert_eq!(route(&field, &player(), me, foe), None);
    }

    #[test]
    fn a_player_keeps_out_of_the_reach_of_a_mine() {
        let field = field();
        let tank = player();
        let motion = Motion::default();
        let mine = Hazard {
            at: Vec2::new(4.0, 0.0),
            blows: None,
        };
        // It meant to drive east, over the mine.
        let (way, _) = way_out(&field, &tank, Vec2::ZERO, &motion, Vec2::X, &[], &[mine]);
        let places = course(&field.colliders, &tank, Vec2::ZERO, &motion, way);
        let nearest = places
            .iter()
            .take(HORIZON / 2)
            .map(|place| place.distance(mine.at));
        assert!(nearest.fold(f32::MAX, f32::min) > MINE_TRIGGER_RADIUS);
    }
}
