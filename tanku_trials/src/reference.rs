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
//! None lays a mine, takes cover or picks the moment of a fight. They use nothing of the
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
    field::{Collider, Field},
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
}

impl Sight {
    pub fn of(world: &mut World) -> Self {
        let mut bullets = world.query::<(Entity, &Bullet, &Position)>();
        let mut mines = world.query::<(&Mine, &Position)>();
        let mut tanks = world.query::<(&Tank, &Position, Option<&Enemy>, Has<Wreck>)>();
        Self {
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
    seen: Vec<Seen>,
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
            seen: Vec::new(),
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
        if let Some(enemy) = enemy {
            let foe = world.get::<Position>(enemy).unwrap().0;
            let foe_velocity = world.get::<Motion>(enemy).unwrap().velocity;
            let rounds = world.get::<Gun>(entity).unwrap().rounds;
            let aim = self.aim(field, tank, me, motion.turret, foe, foe_velocity);
            command.aim = me + Vec2::from_angle(aim.0) * 10.0;
            command.fire = aim.1 && rounds > 0 && self.since_shot >= self.pace;
            meant = brain::steer(field, tank, me, self.around(field, me, foe));
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
            let follows = self.skill.follows;
            let mut tracks = Vec::new();
            for &(_, bullet, at) in &known {
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
    fn around(&mut self, field: &Field, me: Vec2, foe: Vec2) -> Vec2 {
        self.turned += TICK;
        if self.turned >= 2.0 {
            self.turned = 0.0;
            self.strafe = -self.strafe;
        }
        let distance = me.distance(foe);
        let toward = (foe - me) / distance.max(0.01);
        if !field.sees(me, foe) || distance > 14.0 {
            toward
        } else if distance < 8.0 {
            -toward
        } else {
            toward.perp() * self.strafe
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
                if !self.skill.follows {
                    seen.since = 0.0;
                }
            }
            if seen.since >= notice {
                known.push((entity, bullet, at));
            }
            seen.since += TICK;
        }
        known
    }
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
            solid: tanku_server::field::Solid::Wall,
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
