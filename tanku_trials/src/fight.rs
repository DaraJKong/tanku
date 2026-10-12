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
        Barrier, Enemy, Player, Playtest, Position, Protected, Run, RunRequest, Shield, SimEvent,
        Tank, TankCommand, Team,
    },
};
use tanku_server::{ServerPlugin, field::Field};

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
}

/// How a fight went.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Outcome {
    /// The tanks of the group.
    pub tanks: usize,
    /// Those that were left when the fight ended.
    pub left: usize,
    /// How long the group lasted, in seconds: the whole fight if a tank of it was left.
    pub lasted: f32,
    /// The losses to a bullet of the enemies that ended on the tank.
    pub bullets: u32,
    /// The losses to a blast that reached the tank: of a mine, a crate or an exploding bullet.
    pub blasts: u32,
    /// The shots of the player that came back off a wall and ended on the tank. They are
    /// no losses: they tell of the hands that fired them, not of the group.
    pub own: u32,
}

impl Outcome {
    pub fn cleared(&self) -> bool {
        self.left == 0
    }

    /// The tanks that the player would have lost to the group.
    pub fn losses(&self) -> u32 {
        self.bullets + self.blasts
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

/// Fights the group of a sector of a corridor as the game makes it, from the moment the
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
    let mut foes: Vec<Entity> = world
        .query_filtered::<(Entity, &Tank), With<Enemy>>()
        .iter(world)
        .filter(|(_, tank)| tank.sector == sector)
        .map(|(entity, _)| entity)
        .collect();
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
    let mut outcome = Outcome {
        tanks: foes.len(),
        lasted: setup.limit,
        ..default()
    };
    // A tank that is destroyed is gone for a while: what hits it again then is no new loss.
    let mut out = 0.0f32;
    for step in 0..ticks(setup.limit) {
        let place = |world: &World, entity: Entity| world.get::<Position>(entity).unwrap().0;
        let me = place(app.world(), entity);
        foes.retain(|&foe| app.world().get_entity(foe).is_ok());
        if foes.is_empty() {
            outcome.lasted = step as f32 * TICK;
            break;
        }
        // The player fights the nearest enemy that is left.
        let far = |foe: &Entity| place(app.world(), *foe).distance(me);
        let nearest = foes
            .iter()
            .min_by(|a, b| far(a).total_cmp(&far(b)))
            .copied();
        let sight = Sight::of(app.world_mut());
        let order = pilot.order(app.world(), &sight, entity, nearest);
        *app.world_mut().get_mut::<TankCommand>(entity).unwrap() = order;
        tick(&mut app);

        let me = place(app.world(), entity);
        let shielded = app.world().get::<Shield>(entity).unwrap().is_active();
        out -= TICK;
        for event in events(&mut app) {
            let loss = match event {
                // A bullet that ends on a tank ends where it touches it.
                SimEvent::BulletDestroyed { at, team }
                    if (at.distance(me) - TANK_RADIUS - BULLET_RADIUS).abs() < 0.05 =>
                {
                    // A shot of the player that ends in the barrel, on a wall that the tank
                    // is pressed against, ends as close: only one that has bounced comes back.
                    let came_back = sight.bullets.iter().any(|(_, bullet, from)| {
                        let flown = bullet.velocity.length() * TICK + 0.1;
                        bullet.team == Team::Players && bullet.bounced && from.distance(at) <= flown
                    });
                    outcome.own += (team == Team::Players && came_back) as u32;
                    (team == Team::Enemies).then_some(&mut outcome.bullets)
                }
                SimEvent::Blast { at } => {
                    let field = app.world().resource::<Field>();
                    let walled = field.blocked(at, me, |solid| solid.stops_blasts());
                    let reached = at.distance(me) <= BLAST_RADIUS && !walled && !shielded;
                    reached.then_some(&mut outcome.blasts)
                }
                _ => None,
            };
            if let Some(count) = loss
                && out <= 0.0
            {
                *count += 1;
                out = RESPAWN_DELAY;
            }
        }
    }
    foes.retain(|&foe| app.world().get_entity(foe).is_ok());
    outcome.left = foes.len();
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
    fn a_fight_counts_the_tanks_of_its_group_and_those_that_fall() {
        for fight in fights(Reference::Novice) {
            assert!(fight.tanks >= 1 && fight.left <= fight.tanks);
            assert_eq!(fight.cleared(), fight.lasted < 20.0);
        }
    }
}
