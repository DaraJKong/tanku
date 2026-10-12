//! A fight of a trial: a reference player against the group of a sector, on the real
//! simulation and without a window.
//!
//! Nothing destroys the tank of the player. What would have is counted as a loss, so that
//! every fight lasts until the group has fallen or the time is up, and one number says what
//! a group costs: the tanks that a player would have lost to it.

use bevy::{
    ecs::{message::Messages, schedule::SingleThreadedExecutor},
    prelude::*,
};
use tanku_core::{
    CorePlugin,
    map::SectorKind,
    modes::Difficulty,
    rules::{BLAST_RADIUS, BULLET_RADIUS, RESPAWN_DELAY, TANK_RADIUS, TICK},
    sim::{
        Barrier, Bullet, Enemy, Gun, Kind, Motion, Player, Playtest, Position, PreviousPosition,
        Protected, Run, RunRequest, Shield, SimEvent, Tank, TankCommand, Team,
    },
    stats::Genome,
};
use tanku_server::{
    ServerPlugin,
    field::{Field, InSector},
};

use crate::reference::{Pilot, Reference, Sight};

/// What a fight is: where, against what and in whose hands.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub seed: String,
    pub sector: u32,
    pub difficulty: Difficulty,
    pub player: Reference,
    /// How long the fight lasts at most, in seconds.
    pub limit: f32,
    /// The tanks that stand in for the group of the sector, if it is not fought as it is.
    pub make: Option<Make>,
}

/// Tanks that stand in for the group of a sector, at the places of some of its tanks and
/// with the brains that the sector gives them.
#[derive(Clone, Debug, PartialEq)]
pub struct Make {
    pub tanks: Vec<Genome>,
    /// The first of the tanks of the group whose places they take.
    pub spot: usize,
}

/// What a loss is to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cause {
    /// A bullet of the enemies that ended on the tank.
    Bullet,
    /// The blast of a mine.
    Mine,
    /// The blast of an exploding bullet.
    Exploding,
    /// The blast of a crate, whoever set it off.
    Crate,
}

impl Cause {
    pub const ALL: [Self; 4] = [Self::Bullet, Self::Mine, Self::Exploding, Self::Crate];
}

/// A tank that the player would have lost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loss {
    pub cause: Cause,
    /// The tank of the group that fired the bullet or laid the mine, by its place in the
    /// group. Nobody is told for a crate.
    pub by: Option<usize>,
}

/// A tank of the group, and how it fared.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fought {
    pub genome: Genome,
    /// How long it lasted, in seconds: the whole fight if it was left.
    pub lasted: f32,
    pub fell: bool,
}

/// How a fight went.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    pub group: Vec<Fought>,
    /// How long the group lasted, in seconds: the whole fight if a tank of it was left.
    pub lasted: f32,
    pub losses: Vec<Loss>,
    /// The shots of the player that came back off a wall and ended on the tank. They are
    /// no losses: they tell of the hands that fired them, not of the group.
    pub own: u32,
}

impl Outcome {
    pub fn tanks(&self) -> usize {
        self.group.len()
    }

    /// The tanks of the group that were left when the fight ended.
    pub fn left(&self) -> usize {
        self.group.iter().filter(|tank| !tank.fell).count()
    }

    pub fn cleared(&self) -> bool {
        self.left() == 0
    }

    /// The losses to one cause.
    pub fn losses_to(&self, cause: Cause) -> usize {
        let to = |loss: &&Loss| loss.cause == cause;
        self.losses.iter().filter(to).count()
    }

    /// The losses that were the doing of one tank of the group.
    pub fn losses_by(&self, tank: usize) -> usize {
        let by = |loss: &&Loss| loss.by == Some(tank);
        self.losses.iter().filter(by).count()
    }
}

fn tick(app: &mut App) {
    app.world_mut().run_schedule(FixedUpdate);
}

fn events(app: &mut App) -> Vec<SimEvent> {
    let mut events = app.world_mut().resource_mut::<Messages<SimEvent>>();
    events.drain().collect()
}

fn ticks(seconds: f32) -> u32 {
    (seconds / TICK).ceil() as u32
}

/// Puts other tanks in the place of the group of a sector, which waits there.
fn stand_in(
    world: &mut World,
    sector: u32,
    group: &[(Entity, Genome)],
    make: &Make,
) -> Vec<(Entity, Genome)> {
    let stood: Vec<(usize, Vec2, Motion)> = (make.spot..make.spot + make.tanks.len())
        .map(|number| {
            let (entity, _) = group[number % group.len()];
            let at = world.get::<Position>(entity).unwrap().0;
            (number, at, *world.get::<Motion>(entity).unwrap())
        })
        .collect();
    for &(entity, _) in group {
        world.despawn(entity);
    }
    let tanks = stood
        .into_iter()
        .zip(&make.tanks)
        .map(|((number, at, motion), &genome)| {
            let (brain, _) = world.resource::<Field>().brain(sector, number as u32);
            let tank = Tank {
                genome,
                team: Team::Enemies,
                sector,
            };
            let enemy = (
                tank,
                Enemy { awake: false },
                Position(at),
                PreviousPosition(at),
                motion,
                Gun::loaded(&genome),
                brain,
                InSector(sector),
            );
            (world.spawn(enemy).id(), genome)
        });
    tanks.collect()
}

/// Fights the group of a sector of a corridor as the game makes it, or the tanks that stand
/// in for it, from the moment the
/// player crosses its barrier. Sectors that are no corridors hold no fight: an arena sends
/// its group in waves, which is another trial.
pub fn corridor(setup: &Setup) -> Option<Outcome> {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
    // The systems of a tick run one after the other, and many fights run side by side.
    app.edit_schedule(FixedUpdate, |schedule| {
        schedule.set_executor(SingleThreadedExecutor::new());
    });
    app.insert_resource(Playtest {
        infinite_lives: true,
    });
    app.world_mut().write_message(RunRequest::Start {
        seed: setup.seed.clone(),
        players: 1,
        drawn: true,
        difficulty: setup.difficulty,
        free: false,
        arena: false,
        tanks: Vec::new(),
    });
    tick(&mut app);
    let sector = setup.sector;
    app.world_mut().write_message(RunRequest::SkipTo { sector });
    tick(&mut app);
    let mut field = app.world_mut().resource_mut::<Field>();
    if field.corridor.plan(sector).kind != SectorKind::Corridor {
        return None;
    }

    let world = app.world_mut();
    let mut made: Vec<(Entity, Genome)> = world
        .query_filtered::<(Entity, &Tank), With<Enemy>>()
        .iter(world)
        .filter(|(_, tank)| tank.sector == sector)
        .map(|(entity, tank)| (entity, tank.genome))
        .collect();
    if let Some(make) = &setup.make {
        made = stand_in(world, sector, &made, make);
    }
    let entity = world
        .query_filtered::<Entity, With<Player>>()
        .single(world)
        .unwrap();
    // The player is carried through the barrier of the sector that it stands in front of.
    let behind = world.resource::<Run>().sector;
    let barrier = *world
        .query::<&Barrier>()
        .iter(world)
        .find(|barrier| barrier.sector == behind)
        .unwrap();
    let middle = (barrier.from + barrier.to) / 2.0;
    for step in [-1.0, 2.0] {
        app.world_mut().get_mut::<Position>(entity).unwrap().0 = middle + barrier.outward * step;
        tick(&mut app);
    }
    app.world_mut()
        .entity_mut(entity)
        .insert(Protected(f32::MAX));
    events(&mut app);

    let mut pilot = Pilot::new(setup.player);
    let limit = setup.limit;
    let mut outcome = Outcome {
        group: made
            .iter()
            .map(|&(_, genome)| Fought {
                genome,
                lasted: limit,
                fell: false,
            })
            .collect(),
        lasted: limit,
        ..default()
    };
    // The tank of the group that a bullet or a mine is of.
    let maker = |owner: Entity| made.iter().position(|(entity, _)| *entity == owner);
    // A tank that is destroyed is gone for a while: what hits it again then is no new loss.
    let mut out = 0.0f32;
    for step in 0..ticks(limit) {
        let place = |world: &World, entity: Entity| world.get::<Position>(entity).unwrap().0;
        let me = place(app.world(), entity);
        let since = step as f32 * TICK;
        for (tank, (foe, _)) in outcome.group.iter_mut().zip(&made) {
            if !tank.fell && app.world().get_entity(*foe).is_err() {
                (tank.fell, tank.lasted) = (true, since);
            }
        }
        if outcome.cleared() {
            outcome.lasted = since;
            break;
        }
        // The player fights the nearest enemy that is left.
        let left = made.iter().zip(&outcome.group);
        let left = left
            .filter(|(_, tank)| !tank.fell)
            .map(|((foe, _), _)| *foe);
        let far = |foe: &Entity| place(app.world(), *foe).distance(me);
        let nearest = left.min_by(|a, b| far(a).total_cmp(&far(b)));
        let sight = Sight::of(app.world_mut());
        let order = pilot.order(app.world(), &sight, entity, nearest);
        *app.world_mut().get_mut::<TankCommand>(entity).unwrap() = order;
        tick(&mut app);

        let me = place(app.world(), entity);
        let shielded = app.world().get::<Shield>(entity).unwrap().is_active();
        // The bullet of a side that was, before the tick, close enough to a place to have
        // ended there.
        let ended = |at: Vec2, team: Team, wanted: fn(&Bullet) -> bool| {
            let near = sight.bullets.iter().filter(|(_, bullet, from)| {
                let flown = bullet.velocity.length() * TICK + 0.1;
                bullet.team == team && wanted(bullet) && from.distance(at) <= flown
            });
            let nearest = near.min_by(|a, b| a.2.distance(at).total_cmp(&b.2.distance(at)));
            nearest.map(|(_, bullet, _)| *bullet)
        };
        out -= TICK;
        for event in events(&mut app) {
            let loss = match event {
                // A bullet that ends on a tank ends where it touches it.
                SimEvent::BulletDestroyed { at, team }
                    if (at.distance(me) - TANK_RADIUS - BULLET_RADIUS).abs() < 0.05 =>
                {
                    // A shot of the player that ends in the barrel, on a wall that the tank
                    // is pressed against, ends as close: only one that has bounced comes back.
                    let came_back = ended(at, Team::Players, |bullet| bullet.bounced).is_some();
                    outcome.own += (team == Team::Players && came_back) as u32;
                    (team == Team::Enemies).then(|| Loss {
                        cause: Cause::Bullet,
                        by: ended(at, team, |_| true).and_then(|bullet| maker(bullet.owner)),
                    })
                }
                SimEvent::Blast { at } => {
                    let field = app.world().resource::<Field>();
                    let walled = field.blocked(at, me, |solid| solid.stops_blasts());
                    let reached = at.distance(me) <= BLAST_RADIUS && !walled && !shielded;
                    // The blast is the one of the mine that lay there, or of the exploding
                    // bullet that ended there, or else of a crate.
                    let mine = sight.mines.iter().find(|(_, lay)| lay.distance(at) < 0.05);
                    let exploding = |bullet: &Bullet| bullet.kind == Kind::Exploding;
                    let shell = ended(at, Team::Enemies, exploding);
                    let (cause, owner) = match (mine, shell) {
                        (Some((mine, _)), _) => (Cause::Mine, Some(mine.owner)),
                        (None, Some(bullet)) => (Cause::Exploding, Some(bullet.owner)),
                        (None, None) => (Cause::Crate, None),
                    };
                    reached.then_some(Loss {
                        cause,
                        by: owner.and_then(maker),
                    })
                }
                _ => None,
            };
            if let Some(loss) = loss
                && out <= 0.0
            {
                outcome.losses.push(loss);
                out = RESPAWN_DELAY;
            }
        }
    }
    for (tank, (foe, _)) in outcome.group.iter_mut().zip(&made) {
        tank.fell = app.world().get_entity(*foe).is_err();
    }
    Some(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(sector: u32, player: Reference) -> Setup {
        Setup {
            seed: "TRIAL0".to_string(),
            sector,
            difficulty: Difficulty::Normal,
            player,
            limit: 20.0,
            make: None,
        }
    }

    /// The first sectors that are corridors, which every seed has.
    fn fights(player: Reference) -> Vec<Outcome> {
        let fights = (1..=4).filter_map(|sector| corridor(&setup(sector, player)));
        fights.collect()
    }

    #[test]
    fn a_fight_goes_the_same_way_twice() {
        let (first, again) = (fights(Reference::Regular), fights(Reference::Regular));
        assert!(!first.is_empty());
        assert_eq!(first, again);
    }

    #[test]
    fn tanks_of_one_make_stand_in_for_the_group() {
        let mut genome = Genome::empty(40);
        genome.set(tanku_core::stats::Stat::Muzzle, 3);
        let make = Make {
            tanks: vec![genome; 2],
            spot: 1,
        };
        let duels = (5..=8).filter_map(|sector| {
            corridor(&Setup {
                make: Some(make.clone()),
                ..setup(sector, Reference::Regular)
            })
        });
        let duels: Vec<Outcome> = duels.collect();
        assert!(!duels.is_empty());
        for duel in duels {
            assert_eq!(duel.tanks(), 2);
            assert!(duel.group.iter().all(|tank| tank.genome == genome));
        }
    }

    #[test]
    fn a_fight_counts_the_tanks_of_its_group_and_those_that_fall() {
        for fight in fights(Reference::Novice) {
            assert!(fight.tanks() >= 1 && fight.left() <= fight.tanks());
            assert_eq!(fight.cleared(), fight.lasted < 20.0);
            for tank in &fight.group {
                assert_eq!(tank.fell, tank.lasted < 20.0);
            }
            // Whoever a loss is blamed on is a tank of the group.
            let known = |loss: &Loss| loss.by.is_none_or(|tank| tank < fight.tanks());
            assert!(fight.losses.iter().all(known));
        }
    }
}
