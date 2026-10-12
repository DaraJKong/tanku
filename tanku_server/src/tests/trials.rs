//! Fights between one or two enemies and a crude player, to compare what the brains make of
//! the same tank.
//!
//! They take minutes and print tables, so they only run when asked for:
//!
//! ```text
//! cargo test -p tanku_server trials -- --ignored --nocapture
//! ```
//!
//! The tables say little about a player of flesh and blood, who closes in, takes cover and
//! lays mines. They tell the brains apart on the same tank, against the same shots.

use std::time::Instant;

use tanku_core::{
    geom,
    rules::{BLAST_RADIUS, BULLET_RADIUS, TICK},
};

use super::*;
use crate::brain;

/// How long a fight lasts at most, in seconds.
const LIMIT: f32 = 40.0;
/// The seeds and the sectors of the fights. Each sector that is a corridor holds one fight,
/// at the place of one of its own enemies.
const SEEDS: [&str; 6] = ["ALPHA", "BRAVO", "CHARLIE", "DELTA", "ECHO", "FOXTROT"];
const SECTORS: [u32; 4] = [11, 12, 13, 14];

/// The tanks that are tried, by what they are made of besides their brain.
const BODIES: [(&str, &[(Stat, u8)]); 5] = [
    (
        "Rover: 4 m/s, 3 rounds",
        &[
            (Stat::Engine, 3),
            (Stat::Tracks, 3),
            (Stat::Turret, 3),
            (Stat::Muzzle, 3),
            (Stat::Magazine, 2),
            (Stat::Loader, 3),
            (Stat::Ricochet, 1),
        ],
    ),
    (
        "Guard: the rover with a shield",
        &[
            (Stat::Engine, 3),
            (Stat::Tracks, 3),
            (Stat::Turret, 3),
            (Stat::Muzzle, 3),
            (Stat::Magazine, 2),
            (Stat::Loader, 3),
            (Stat::Ricochet, 1),
            (Stat::Shield, 2),
        ],
    ),
    (
        "Sapper: 5 m/s, 3 mines",
        &[
            (Stat::Engine, 4),
            (Stat::Tracks, 3),
            (Stat::Turret, 2),
            (Stat::Muzzle, 2),
            (Stat::Magazine, 1),
            (Stat::Loader, 2),
            (Stat::Mines, 3),
        ],
    ),
    (
        "Post: no engine, 4 rounds",
        &[
            (Stat::Turret, 5),
            (Stat::Muzzle, 4),
            (Stat::Magazine, 3),
            (Stat::Loader, 4),
        ],
    ),
    (
        "Scout: 7 m/s, 2 rounds",
        &[
            (Stat::Engine, 6),
            (Stat::Tracks, 5),
            (Stat::Turret, 3),
            (Stat::Muzzle, 2),
            (Stat::Magazine, 1),
            (Stat::Loader, 2),
        ],
    ),
];

/// The crude player of a trial. It keeps its distance, drives around its enemy and fires at it
/// when its gun is on it, off a wall when it doesn't see it. Nothing destroys its tank: what
/// would have is counted instead, so that every fight lasts until the enemy falls.
struct Bot {
    /// The least time between two of its shots. A gun fires every quarter of a second while
    /// it has rounds, and the tank of a player reloads one round a second.
    pace: f32,
    since_shot: f32,
    strafe: f32,
    /// Time since it last changed the way it drives around its enemy.
    turned: f32,
    bank_shot: Option<f32>,
    bank_shot_timer: f32,
}

impl Bot {
    fn new(pace: f32) -> Self {
        Self {
            pace,
            since_shot: pace,
            strafe: 1.0,
            turned: 0.0,
            bank_shot: None,
            bank_shot_timer: 0.0,
        }
    }

    fn order(&mut self, app: &App, entity: Entity, enemy: Entity) -> TankCommand {
        let world = app.world();
        let field = world.resource::<Field>();
        let tank = world.get::<Tank>(entity).unwrap();
        let me = world.get::<Position>(entity).unwrap().0;
        let turret = world.get::<Motion>(entity).unwrap().turret;
        let foe = world.get::<Position>(enemy).unwrap().0;
        let foe_velocity = world.get::<Motion>(enemy).unwrap().velocity;

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
        self.since_shot += TICK;
        let rounds = world.get::<Gun>(entity).unwrap().rounds;
        let fire = solution && aligned && rounds > 0 && self.since_shot >= self.pace;
        if fire {
            self.since_shot = 0.0;
        }

        self.turned += TICK;
        if self.turned >= 2.0 {
            self.turned = 0.0;
            self.strafe = -self.strafe;
        }
        let distance = me.distance(foe);
        let toward = (foe - me) / distance.max(0.01);
        let wanted = if !field.sees(me, foe) || distance > 14.0 {
            toward
        } else if distance < 8.0 {
            -toward
        } else {
            toward.perp() * self.strafe
        };
        TankCommand {
            drive: brain::steer(field, tank, me, wanted),
            aim: me + Vec2::from_angle(aim) * 10.0,
            fire,
            ..default()
        }
    }
}

/// How a fight went.
#[derive(Clone, Copy, Debug)]
struct Trial {
    /// How long the enemies lasted, in seconds: the whole fight if one of them was left.
    lasted: f32,
    fell: bool,
    /// The bullets of the enemies that ended on the tank of the bot.
    hits: u32,
    /// The blasts that reached the tank of the bot.
    blasts: u32,
}

/// A fight in a sector of a seed, between the bot and so many enemies of one make that stand
/// in for the group of that sector, at the places of some of its tanks. Sectors that are no
/// corridors hold none.
fn fight(
    seed: &str,
    sector: u32,
    spot: usize,
    genome: Genome,
    count: usize,
    pace: f32,
) -> Option<Trial> {
    let mut app = begin(seed);
    app.insert_resource(Playtest {
        infinite_lives: true,
    });
    skip_to(&mut app, sector);
    let mut field = app.world_mut().resource_mut::<Field>();
    if field.corridor.plan(sector).kind != SectorKind::Corridor {
        return None;
    }
    let group = enemies(&mut app, sector);
    let stood: Vec<(Vec2, Motion)> = (spot..spot + count)
        .map(|number| group[number % group.len()])
        .map(|stood| {
            (
                position(&mut app, stood),
                *app.world().get::<Motion>(stood).unwrap(),
            )
        })
        .collect();
    for enemy in group {
        app.world_mut().despawn(enemy);
    }
    let mut foes: Vec<Entity> = Vec::new();
    for (number, (at, motion)) in stood.into_iter().enumerate() {
        let number = (spot + number) as u32;
        let (brain, _) = app.world().resource::<Field>().brain(sector, number);
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
        foes.push(app.world_mut().spawn(enemy).id());
    }
    cross_barrier(&mut app);
    let entity = player(&mut app).unwrap();
    app.world_mut()
        .entity_mut(entity)
        .insert(Protected(f32::MAX));
    events(&mut app);

    let mut bot = Bot::new(pace);
    let mut trial = Trial {
        lasted: LIMIT,
        fell: false,
        hits: 0,
        blasts: 0,
    };
    for step in 0..seconds(LIMIT) {
        // The bot fights the nearest enemy that is left.
        let me = position(&mut app, entity);
        foes.retain(|&foe| alive(&app, foe));
        let far = |foe: &Entity| app.world().get::<Position>(*foe).unwrap().0.distance(me);
        let Some(&foe) = foes.iter().min_by(|a, b| far(a).total_cmp(&far(b))) else {
            trial.lasted = step as f32 * TICK;
            trial.fell = true;
            break;
        };
        let order = bot.order(&app, entity, foe);
        *app.world_mut().get_mut::<TankCommand>(entity).unwrap() = order;
        tick(&mut app, 1);
        let me = position(&mut app, entity);
        for event in events(&mut app) {
            match event {
                // A bullet that ends on a tank ends where it touches it.
                SimEvent::BulletDestroyed {
                    at,
                    team: Team::Enemies,
                } if (at.distance(me) - TANK_RADIUS - BULLET_RADIUS).abs() < 0.05 => {
                    trial.hits += 1;
                }
                SimEvent::Blast { at } => {
                    let field = app.world().resource::<Field>();
                    let walled = field.blocked(at, me, |solid| solid.stops_blasts());
                    if at.distance(me) <= BLAST_RADIUS && !walled {
                        trial.blasts += 1;
                    }
                }
                _ => {}
            }
        }
    }
    Some(trial)
}

/// A tank of a body, with a brain of a tier that leads its shots.
fn build(body: &[(Stat, u8)], brain: u8) -> Genome {
    let mut genome = Genome::empty(100);
    for &(stat, level) in body {
        genome.set(stat, level);
    }
    genome.set(Stat::Brain, brain);
    genome.set(Stat::Ballistics, 1);
    genome
}

/// Prints a table of the fights of so many enemies of each make against a bot that fires at
/// a pace: for each tank and each brain, how many fights there were, in how many of them the
/// enemies fell, the median of how long they lasted, how many of their bullets ended on the
/// bot in a fight and in a minute, how many blasts reached the bot in a minute, and what a
/// tick of the whole simulation cost in this build.
fn table(count: usize, pace: f32) {
    println!(
        "{:<32} {:>5} {:>6} {:>5} {:>8} {:>10} {:>8} {:>10} {:>7}",
        "tank",
        "brain",
        "fights",
        "fell",
        "median s",
        "hits/fight",
        "hits/min",
        "blasts/min",
        "ms/tick"
    );
    for (name, body) in BODIES {
        for brain in [2, 3, 4] {
            let started = Instant::now();
            let mut trials: Vec<Trial> = Vec::new();
            for (number, seed) in SEEDS.iter().enumerate() {
                for sector in SECTORS {
                    trials.extend(fight(seed, sector, number, build(body, brain), count, pace));
                }
            }
            let spent = started.elapsed().as_secs_f32();
            let fights = trials.len();
            let seconds: f32 = trials.iter().map(|trial| trial.lasted).sum();
            let fell = trials.iter().filter(|trial| trial.fell).count();
            let mut lasted: Vec<f32> = trials.iter().map(|trial| trial.lasted).collect();
            lasted.sort_by(f32::total_cmp);
            let median = lasted[fights / 2];
            let hits: u32 = trials.iter().map(|trial| trial.hits).sum();
            let blasts: u32 = trials.iter().map(|trial| trial.blasts).sum();
            let per_fight = hits as f32 / fights as f32;
            let per_minute = |count: u32| count as f32 * 60.0 / seconds;
            let cost = spent * 1000.0 / (seconds / TICK);
            println!(
                "{name:<32} {brain:>5} {fights:>6} {fell:>5} {median:>8.1} {per_fight:>10.2} \
                 {:>8.1} {:>10.1} {cost:>7.3}",
                per_minute(hits),
                per_minute(blasts)
            );
        }
    }
}

/// One enemy against the bot. An ace that is alone retreats.
#[test]
#[ignore = "takes minutes and prints tables"]
fn a_tank_alone() {
    for (pace, shots) in [(0.25, "bursts"), (1.0, "single shots")] {
        println!("\nOne enemy. The bot fires {shots}: one shot every {pace} s at most.");
        table(1, pace);
    }
}

/// Two enemies of one make against the bot: an ace has an ally to cross its fire with.
#[test]
#[ignore = "takes minutes and prints a table"]
fn two_tanks_together() {
    println!("\nTwo enemies. The bot fires bursts: one shot every 0.25 s at most.");
    table(2, 0.25);
}
