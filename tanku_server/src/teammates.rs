//! Teammates that the game drives, so that one person can playtest a game of several players.
//!
//! A playtest can start a run with up to three teammates beside the player, in the last seats.
//! The run is then one of several players, with wrecks, repairs and the groups of as many
//! players. The hands on each of their tanks are those of a reference player, which fights,
//! dodges and finds its way around walls (see `pilot`). What a teammate does besides is this:
//!
//! - It fights the nearest enemy that is awake in its sector. It holds a shot that would pass
//!   the tank of a teammate or go off next to one, keeps out of the reach of their mines, and
//!   minds their bullets as it minds the enemies'.
//! - The teammate that is nearest to a wreck goes to repair it, and fights on from there.
//! - Once its sector is cleared, it waits in front of the barrier, where it counts for the
//!   barrier to unlock, and crosses after the player: it never wakes a sector first.
//! - Otherwise, it keeps near the player, off the places where the tanks of an arena wait for
//!   room.
//!
//! A team without a person, as in a test, follows its first seat, which crosses a barrier as
//! soon as it unlocks.

use bevy::prelude::*;
use tanku_core::{
    reference::{Reference, Teammates},
    rules::{REPAIR_RADIUS, TANK_RADIUS},
    sim::{Barrier, Enemy, Player, Position, Run, Tank, TankCommand, Wreck},
};

use crate::{
    field::Field,
    pilot::{self, Goal, Pilot, Sight},
};

/// How far in front of the middle of its barrier a teammate waits for the player, in meters:
/// well within the reach that unlocks it.
const WAIT: f32 = 3.0;
/// How far past a barrier a teammate drives to cross it.
const PAST: f32 = 3.0;
/// How far apart the teammates stand along a barrier.
const SIDE_BY_SIDE: f32 = 2.5;
/// A teammate that has nothing to fight goes back to the player once it is further than the
/// first distance, in meters, and stops at the second.
const FOLLOW: (f32, f32) = (7.0, 4.0);
/// How close a teammate drives to a wreck that it repairs: well within the reach of a repair,
/// so that the way around a bullet doesn't end it.
const MEND: f32 = 1.5;
/// How close a teammate stops to a place by a barrier.
const NEAR: f32 = 1.0;
/// How far inside the reach of a repair a teammate holds the key: a tank that a wall keeps
/// further from a wreck than it drives to repairs it from there.
const MARGIN: f32 = 0.25;
/// How far a teammate drives at a time from the places where the tanks of an arena wait.
const ASIDE: f32 = 3.0;

/// The teammates of the run, each with the hands on its tank. A run inserts it when it
/// starts. The server doesn't when it is built: a type registered then renumbers the
/// components registered after it, which changes the order in which the engine works through
/// them, and with it the last bits of some sums and a few runs of the trials in a hundred.
#[derive(Resource, Default)]
pub struct Crew {
    hands: Vec<Hands>,
}

/// The hands on the tank of a seat.
struct Hands {
    seat: u8,
    player: Reference,
    /// The tank that they held last: a tank that comes back is another one, and is taken in
    /// fresh hands.
    tank: Option<Entity>,
    pilot: Pilot,
}

impl Crew {
    /// The teammates of a run of so many players: they have its last seats.
    pub fn new(teammates: Teammates, players: u32) -> Self {
        let count = u32::from(teammates.count).min(players);
        let hands = ((players - count)..players).map(|seat| Hands {
            seat: seat as u8,
            player: teammates.player,
            tank: None,
            pilot: Pilot::new(teammates.player),
        });
        Self {
            hands: hands.collect(),
        }
    }

    /// Whether the game drives the tank of a seat.
    pub fn drives(&self, seat: u8) -> bool {
        self.hands.iter().any(|hands| hands.seat == seat)
    }
}

/// The tank of a player, as the teammates see it.
#[derive(Clone, Copy, Debug)]
struct Mate {
    entity: Entity,
    seat: u8,
    sector: u32,
    at: Vec2,
    wreck: bool,
    /// Whether the game drives it.
    driven: bool,
}

/// An enemy that is awake.
#[derive(Clone, Copy, Debug)]
struct Foe {
    entity: Entity,
    sector: u32,
    at: Vec2,
}

/// What a teammate does in a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Plan {
    /// The enemy that it fights.
    enemy: Option<Entity>,
    /// Where it drives, in place of the way of its fight.
    goal: Option<Goal>,
    /// Whether it repairs the wreck that it is at.
    mend: bool,
}

/// What the teammates know of the run in a tick.
struct Around<'a> {
    mates: &'a [Mate],
    foes: &'a [Foe],
    barriers: &'a [Barrier],
    field: &'a Field,
    /// The sector that the first of the players has reached.
    front: u32,
}

impl Around<'_> {
    /// What a teammate does now.
    fn plan(&self, me: &Mate) -> Plan {
        let foes = self.foes.iter().filter(|foe| foe.sector == me.sector);
        let enemy = foes
            .min_by(|a, b| a.at.distance(me.at).total_cmp(&b.at.distance(me.at)))
            .map(|foe| foe.entity);
        if let Some(wreck) = self.wreck_for(me) {
            let mend = me.at.distance(wreck) <= REPAIR_RADIUS - MARGIN;
            let goal = Goal {
                at: wreck,
                within: MEND,
            };
            return Plan {
                enemy,
                goal: Some(goal),
                mend,
            };
        }
        let barrier = self
            .barriers
            .iter()
            .find(|barrier| barrier.sector == me.sector);
        if let Some(barrier) = barrier {
            // The others have crossed: it follows them through.
            let behind = me.sector < self.front;
            // A sector that is cleared: it waits in front of the barrier until the player
            // is through, unless it leads.
            let cleared = self.field.cleared.contains(&me.sector);
            let first = self.leads(me) && barrier.unlocked;
            if behind || (cleared && first) {
                let goal = Some(spot(barrier, me.seat, PAST));
                return Plan {
                    enemy,
                    goal,
                    mend: false,
                };
            }
            if cleared {
                let goal = Some(spot(barrier, me.seat, -WAIT));
                return Plan {
                    enemy,
                    goal,
                    mend: false,
                };
            }
        }
        if enemy.is_some() {
            return Plan { enemy, ..default() };
        }
        // Nothing to fight: off the places where tanks wait for room, and near the player.
        let waiting = pilot::waiting_for_room(self.field, me.sector);
        if let Some(away) = pilot::off_the_marks(&waiting, me.at) {
            let goal = Goal {
                at: me.at + away.normalize_or_zero() * ASIDE,
                within: 0.5,
            };
            return Plan {
                goal: Some(goal),
                ..default()
            };
        }
        let leader = self.leader().filter(|leader| leader.entity != me.entity);
        let far = leader.filter(|leader| leader.at.distance(me.at) > FOLLOW.0);
        Plan {
            goal: far.map(|leader| Goal {
                at: leader.at,
                within: FOLLOW.1,
            }),
            ..default()
        }
    }

    /// The tank that the teammates follow: the player's, or else the first of theirs that
    /// runs.
    fn leader(&self) -> Option<&Mate> {
        let person = self.mates.iter().filter(|mate| !mate.driven);
        let person = person.min_by_key(|mate| mate.seat);
        let running = || self.mates.iter().filter(|mate| !mate.wreck);
        person.or_else(|| running().min_by_key(|mate| mate.seat))
    }

    /// Whether a teammate leads the others through the barriers: in a team without a person,
    /// the first of them that runs does.
    fn leads(&self, me: &Mate) -> bool {
        let leader = self.leader();
        leader.is_some_and(|leader| leader.entity == me.entity && leader.driven)
    }

    /// The wreck that a teammate goes to repair: one of its sector to which it is the nearest
    /// of the teammates whose tanks run, and the nearest such wreck if it has several.
    fn wreck_for(&self, me: &Mate) -> Option<Vec2> {
        let running = || self.mates.iter().filter(|mate| mate.driven && !mate.wreck);
        let wrecks = self.mates.iter();
        let wrecks = wrecks.filter(|mate| mate.wreck && mate.sector == me.sector);
        let mine = wrecks.filter(|wreck| {
            let distance = |mate: &&Mate| mate.at.distance(wreck.at);
            let nearest = running().min_by(|a, b| distance(a).total_cmp(&distance(b)));
            nearest.is_some_and(|nearest| nearest.entity == me.entity)
        });
        let places = mine.map(|wreck| wreck.at);
        places.min_by(|a, b| a.distance(me.at).total_cmp(&b.distance(me.at)))
    }
}

/// The place of a seat by a barrier: so far past it, or in front of it for a depth below
/// zero, and along it by seat, so that the teammates stand side by side.
fn spot(barrier: &Barrier, seat: u8, depth: f32) -> Goal {
    let middle = (barrier.from + barrier.to) / 2.0;
    let along = (barrier.to - barrier.from).normalize_or_zero();
    let room = (barrier.from.distance(barrier.to) / 2.0 - TANK_RADIUS - 0.5).max(0.0);
    let side = ((f32::from(seat) - 1.5) * SIDE_BY_SIDE).clamp(-room, room);
    Goal {
        at: middle + barrier.outward * depth + along * side,
        within: NEAR,
    }
}

/// Whether the run has teammates. A run without them is not touched by them in any way.
pub fn any(crew: Option<Res<Crew>>) -> bool {
    crew.is_some_and(|crew| !crew.hands.is_empty())
}

/// Drives the tanks of the teammates for a tick.
pub fn drive(world: &mut World) {
    world.resource_scope(|world, mut crew: Mut<Crew>| {
        let front = world.resource::<Run>().sector;
        let mut players = world.query::<(Entity, &Player, &Tank, &Position, Has<Wreck>)>();
        let mates: Vec<Mate> = players
            .iter(world)
            .map(|(entity, player, tank, at, wreck)| Mate {
                entity,
                seat: player.seat,
                sector: tank.sector,
                at: at.0,
                wreck,
                driven: crew.drives(player.seat),
            })
            .collect();
        let mut enemies =
            world.query_filtered::<(Entity, &Tank, &Position, &Enemy), Without<Player>>();
        let foes: Vec<Foe> = enemies
            .iter(world)
            .filter(|(.., enemy)| enemy.awake)
            .map(|(entity, tank, at, _)| Foe {
                entity,
                sector: tank.sector,
                at: at.0,
            })
            .collect();
        let mut barriers = world.query::<&Barrier>();
        let barriers: Vec<Barrier> = barriers.iter(world).copied().collect();
        let sight = Sight::of(world);
        let around = Around {
            mates: &mates,
            foes: &foes,
            barriers: &barriers,
            field: world.resource::<Field>(),
            front,
        };
        let mut orders = Vec::new();
        for hands in &mut crew.hands {
            let Some(me) = mates.iter().find(|mate| mate.seat == hands.seat) else {
                continue;
            };
            if hands.tank != Some(me.entity) {
                hands.tank = Some(me.entity);
                hands.pilot = Pilot::new(hands.player);
            }
            // A wreck does nothing until a teammate has repaired it.
            if me.wreck {
                continue;
            }
            let plan = around.plan(me);
            let others = mates
                .iter()
                .filter(|mate| mate.entity != me.entity && !mate.wreck);
            let others: Vec<Vec2> = others.map(|mate| mate.at).collect();
            let (enemy, goal) = (plan.enemy, plan.goal);
            let pilot = &mut hands.pilot;
            let mut command = pilot.order_among(world, &sight, me.entity, enemy, goal, &others);
            command.repair = plan.mend;
            orders.push((me.entity, command));
        }
        for (entity, command) in orders {
            if let Some(mut held) = world.get_mut::<TankCommand>(entity) {
                *held = command;
            }
        }
    });
}
