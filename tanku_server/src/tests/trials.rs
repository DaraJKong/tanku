//! Fights between enemies and a crude player: one or two tanks of the same make, to compare
//! what the brains make of it, the groups of the first sectors as they are, to compare
//! normal mode with hard mode, and the same groups against a player in tanks of several
//! makes, to compare what a player may build in the garage.
//!
//! They take minutes and print tables, so they only run when asked for, each kind apart:
//!
//! ```text
//! cargo test -p tanku_server trials::brains -- --ignored --nocapture
//! cargo test -p tanku_server trials::modes -- --ignored --nocapture
//! cargo test -p tanku_server trials::garage -- --ignored --nocapture
//! cargo test -p tanku_server trials::arena -- --ignored --nocapture
//! ```
//!
//! The tables say little about a player of flesh and blood, who closes in, takes cover and
//! lays mines. They tell the brains apart on the same tank, the modes on the same seed and
//! the tanks of a player in the same hands, against the same shots.

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
    /// Whether it raises the shield of its tank just before a bullet ends on it, whenever
    /// the shield is ready. A trial of the tanks of a player needs it: a tank that gives
    /// up its shield must pay for that.
    shields: bool,
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
            shields: false,
        }
    }

    /// A bot that fires bursts and uses the shield of its tank.
    fn with_a_shield() -> Self {
        Self {
            shields: true,
            ..Self::new(0.25)
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
    /// The enemies that were left when the fight ended.
    left: usize,
}

/// Whether a bullet of the enemies is about to end on a tank that stands at a place: within
/// a quarter of a second, on the course that the bullet is on.
fn threatened(app: &mut App, me: Vec2) -> bool {
    let world = app.world_mut();
    let mut bullets = world.query::<(&Bullet, &Position)>();
    bullets.iter(world).any(|(bullet, position)| {
        let (apart, velocity) = (position.0 - me, bullet.velocity);
        let time = -apart.dot(velocity) / velocity.length_squared().max(0.01);
        let nearest = apart + velocity * time.max(0.0);
        bullet.team == Team::Enemies
            && (0.0..0.25).contains(&time)
            && nearest.length() < TANK_RADIUS + BULLET_RADIUS + 0.2
    })
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
        ..default()
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
    Some(duel(&mut app, foes, Bot::new(pace), LIMIT))
}

/// The group of a sector of a seed as a difficulty makes it, against the bot in the standard
/// tank, which fires bursts. Sectors that are no corridors hold no fight. Tells what the
/// group was made of besides.
fn skirmish(
    seed: &str,
    sector: u32,
    difficulty: Difficulty,
    limit: f32,
) -> Option<(Vec<Genome>, Trial)> {
    skirmish_in(seed, sector, difficulty, None, Bot::new(0.25), limit)
}

/// The group of a sector against a bot, in a tank of its own if it is given one.
fn skirmish_in(
    seed: &str,
    sector: u32,
    difficulty: Difficulty,
    tank: Option<Genome>,
    bot: Bot,
    limit: f32,
) -> Option<(Vec<Genome>, Trial)> {
    let mut app = begin_as(seed, 1, difficulty, tank.into_iter().collect());
    app.insert_resource(Playtest {
        infinite_lives: true,
        ..default()
    });
    skip_to(&mut app, sector);
    let mut field = app.world_mut().resource_mut::<Field>();
    if field.corridor.plan(sector).kind != SectorKind::Corridor {
        return None;
    }
    let foes = enemies(&mut app, sector);
    let genome = |foe: &Entity| app.world().get::<Tank>(*foe).unwrap().genome;
    let group: Vec<Genome> = foes.iter().map(genome).collect();
    Some((group, duel(&mut app, foes, bot, limit)))
}

/// The bot crosses the barrier of the sector that it stands in front of, and fights the
/// enemies there until they have fallen or the time is up.
fn duel(app: &mut App, mut foes: Vec<Entity>, mut bot: Bot, limit: f32) -> Trial {
    cross_barrier(app);
    let entity = player(app).unwrap();
    app.world_mut()
        .entity_mut(entity)
        .insert(Protected(f32::MAX));
    events(app);

    let mut trial = Trial {
        lasted: limit,
        fell: false,
        hits: 0,
        blasts: 0,
        left: 0,
    };
    for step in 0..seconds(limit) {
        // The bot fights the nearest enemy that is left.
        let me = position(app, entity);
        foes.retain(|&foe| alive(app, foe));
        let far = |foe: &Entity| app.world().get::<Position>(*foe).unwrap().0.distance(me);
        let Some(&foe) = foes.iter().min_by(|a, b| far(a).total_cmp(&far(b))) else {
            trial.lasted = step as f32 * TICK;
            trial.fell = true;
            break;
        };
        let mut order = bot.order(app, entity, foe);
        order.shield = bot.shields && threatened(app, me);
        *app.world_mut().get_mut::<TankCommand>(entity).unwrap() = order;
        tick(app, 1);
        let me = position(app, entity);
        for event in events(app) {
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
    trial.left = foes.iter().filter(|&&foe| alive(app, foe)).count();
    trial
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

/// What each brain makes of the same tank.
mod brains {
    use super::*;

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
}

/// Prints a row of a table of skirmishes: how many there were, how many tanks each held, in
/// how many the group fell, the median of how long it lasted, how many of its bullets and
/// of its blasts reached the bot in a minute, and how many of its bullets reached the bot
/// for each of its tanks that fell.
fn skirmishes(name: &str, fights: &[(Vec<Genome>, Trial)]) {
    if fights.is_empty() {
        return println!("{name:<32} {:>6}", 0);
    }
    let count = fights.len();
    let tanks: usize = fights.iter().map(|(group, _)| group.len()).sum();
    let seconds: f32 = fights.iter().map(|(_, trial)| trial.lasted).sum();
    let fell = fights.iter().filter(|(_, trial)| trial.fell).count();
    let mut lasted: Vec<f32> = fights.iter().map(|(_, trial)| trial.lasted).collect();
    lasted.sort_by(f32::total_cmp);
    let hits: u32 = fights.iter().map(|(_, trial)| trial.hits).sum();
    let blasts: u32 = fights.iter().map(|(_, trial)| trial.blasts).sum();
    let left: usize = fights.iter().map(|(_, trial)| trial.left).sum();
    println!(
        "{name:<32} {count:>6} {:>6.1} {:>5.0}% {:>8.1} {:>8.1} {:>10.1} {:>9.2}",
        tanks as f32 / count as f32,
        fell as f32 * 100.0 / count as f32,
        lasted[count / 2],
        hits as f32 * 60.0 / seconds,
        blasts as f32 * 60.0 / seconds,
        hits as f32 / (tanks - left).max(1) as f32,
    );
}

/// How long a skirmish lasts at most, in seconds: a group takes longer than a tank.
const SKIRMISH: f32 = 60.0;

/// The seeds of the skirmishes.
fn trial_seeds(count: u32) -> impl Iterator<Item = String> {
    (0..count).map(|number| format!("TRIAL{number}"))
}

fn skirmish_header() {
    println!(
        "{:<32} {:>6} {:>6} {:>6} {:>8} {:>8} {:>10} {:>9}",
        "", "fights", "tanks", "fell", "median s", "hits/min", "blasts/min", "hits/kill"
    );
}

/// Prints a table of the skirmishes of each mode in stretches of sectors, on so many seeds.
fn modes_table(stretches: &[(u32, u32)], seeds: u32) {
    println!(
        "\nThe group of a sector as it is, on {seeds} seeds. The bot fires bursts, and a \
         fight lasts {SKIRMISH} s at most."
    );
    skirmish_header();
    for &(from, to) in stretches {
        for difficulty in Difficulty::ALL {
            let mut fights = Vec::new();
            for seed in trial_seeds(seeds) {
                for sector in from..=to {
                    fights.extend(skirmish(&seed, sector, difficulty, SKIRMISH));
                }
            }
            skirmishes(&format!("{from}-{to}, {difficulty:?}"), &fights);
        }
    }
}

/// What a player may build in the garage, in the same hands.
mod garage {
    use super::*;

    /// The standard tank, and three that spend the same points on one thing each.
    const TANKS: [(&str, &[(Stat, u8)]); 4] = [
        (
            "Standard",
            &[
                (Stat::Engine, 4),
                (Stat::Tracks, 5),
                (Stat::Muzzle, 2),
                (Stat::Magazine, 4),
                (Stat::Loader, 6),
                (Stat::Ricochet, 1),
                (Stat::Mines, 2),
                (Stat::Shield, 2),
            ],
        ),
        (
            "Gun: bullets at 19 m/s",
            &[
                (Stat::Engine, 4),
                (Stat::Tracks, 5),
                (Stat::Muzzle, 7),
                (Stat::Magazine, 4),
                (Stat::Loader, 6),
            ],
        ),
        (
            "Bouncer: 3 bounces, piercing",
            &[
                (Stat::Engine, 4),
                (Stat::Tracks, 4),
                (Stat::Muzzle, 3),
                (Stat::Magazine, 3),
                (Stat::Loader, 5),
                (Stat::Ricochet, 3),
                (Stat::Piercing, 1),
            ],
        ),
        (
            "Runner: 8 m/s, shield every 6 s",
            &[
                (Stat::Engine, 7),
                (Stat::Tracks, 6),
                (Stat::Muzzle, 2),
                (Stat::Magazine, 2),
                (Stat::Loader, 5),
                (Stat::Shield, 5),
            ],
        ),
    ];

    /// The bot in each of the tanks against the groups of two stretches of sectors, in
    /// normal mode. It fires bursts, raises its shield just before a bullet ends on its
    /// tank, and lays no mine.
    #[test]
    #[ignore = "takes minutes and prints tables"]
    fn four_tanks() {
        assert_eq!(Genome::of_the_garage(TANKS[0].1), Ok(Genome::PLAYER));
        for (from, to) in [(6, 10), (11, 15)] {
            println!(
                "\nSectors {from} to {to} on 6 seeds. A fight lasts {SKIRMISH} s at most, \
                 and the bot uses its shield."
            );
            skirmish_header();
            for (name, parts) in TANKS {
                let tank = Genome::of_the_garage(parts).unwrap();
                let mut fights = Vec::new();
                for seed in trial_seeds(6) {
                    for sector in from..=to {
                        let bot = Bot::with_a_shield();
                        let normal = Difficulty::Normal;
                        fights.extend(skirmish_in(
                            &seed,
                            sector,
                            normal,
                            Some(tank),
                            bot,
                            SKIRMISH,
                        ));
                    }
                }
                let points = format!("{name} ({})", tank.cost());
                skirmishes(&points, &fights);
            }
        }
    }
}

/// The same groups in the corridor and in the room of arena mode.
mod arena {
    use tanku_core::rules::LULL;

    use super::*;

    /// A group of arena mode against the bot in the standard tank, which fires bursts: in
    /// two waves, or all at once. The fight is timed from the end of the lull.
    fn room_skirmish(seed: &str, group: u32, whole: bool, limit: f32) -> (Vec<Genome>, Trial) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
        app.insert_resource(Playtest {
            infinite_lives: true,
            whole_groups: whole,
        });
        app.world_mut().write_message(RunRequest::Start {
            seed: seed.into(),
            players: 1,
            drawn: true,
            difficulty: Difficulty::Normal,
            free: false,
            arena: true,
            tanks: Vec::new(),
        });
        tick(&mut app, 1);
        skip_to(&mut app, group);
        tick(&mut app, 1);
        let entity = player(&mut app).unwrap();
        app.world_mut()
            .entity_mut(entity)
            .insert(Protected(f32::MAX));
        let made: Vec<Genome> = {
            let field = app.world().resource::<Field>();
            let state = field.room.as_ref().unwrap();
            state
                .next
                .iter()
                .flatten()
                .map(|tank| tank.genome)
                .collect()
        };
        tick(&mut app, seconds(LULL));
        events(&mut app);

        let mut bot = Bot::new(0.25);
        let mut trial = Trial {
            lasted: limit,
            fell: false,
            hits: 0,
            blasts: 0,
            left: 0,
        };
        for step in 0..seconds(limit) {
            if run(&app).cleared >= group {
                trial.lasted = step as f32 * TICK;
                trial.fell = true;
                break;
            }
            // The bot fights the nearest tank that is in the room, and waits for the next
            // where it stands.
            let me = position(&mut app, entity);
            let foes = enemies(&mut app, group);
            let far = |foe: &Entity| app.world().get::<Position>(*foe).unwrap().0.distance(me);
            let nearest = foes
                .iter()
                .min_by(|a, b| far(a).total_cmp(&far(b)))
                .copied();
            let order = match nearest {
                Some(foe) => bot.order(&app, entity, foe),
                None => TankCommand {
                    aim: me + Vec2::Y,
                    ..default()
                },
            };
            *app.world_mut().get_mut::<TankCommand>(entity).unwrap() = order;
            tick(&mut app, 1);
            let me = position(&mut app, entity);
            for event in events(&mut app) {
                match event {
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
        if !trial.fell {
            let field = app.world().resource::<Field>();
            let held = field.arenas.get(&group).map_or(0, |arena| {
                arena.waiting.iter().flatten().count() + arena.incoming.len()
            });
            trial.left = held + enemies(&mut app, group).len();
        }
        (made, trial)
    }

    /// The groups of two stretches of sectors, each fought three ways on the same seeds: in
    /// its sector of the corridor, in the room in two waves, and in the room all at once.
    /// Only the sectors that are corridors are fought: the group is then the same, tank for
    /// tank. It takes about ten minutes.
    #[test]
    #[ignore = "takes minutes and prints a table"]
    fn the_same_groups_in_the_corridor_and_in_the_room() {
        println!(
            "\nThe group of a sector on 8 seeds: in its corridor, and in the room of arena \
             mode. The bot fires bursts, and a fight lasts {SKIRMISH} s at most."
        );
        skirmish_header();
        for (from, to) in [(6, 9), (15, 20)] {
            let (mut corridor, mut waves, mut whole) = (Vec::new(), Vec::new(), Vec::new());
            for seed in trial_seeds(8) {
                for sector in from..=to {
                    let Some(fight) = skirmish(&seed, sector, Difficulty::Normal, SKIRMISH) else {
                        continue;
                    };
                    let in_waves = room_skirmish(&seed, sector, false, SKIRMISH);
                    assert_eq!(fight.0, in_waves.0, "seed {seed}, sector {sector}");
                    corridor.push(fight);
                    waves.push(in_waves);
                    whole.push(room_skirmish(&seed, sector, true, SKIRMISH));
                }
            }
            skirmishes(&format!("{from}-{to}, corridor"), &corridor);
            skirmishes(&format!("{from}-{to}, room, two waves"), &waves);
            skirmishes(&format!("{from}-{to}, room, at once"), &whole);
        }
    }
}

/// What each mode makes of the same sectors.
mod modes {
    use super::*;

    /// The groups of the first sectors as each mode makes them, against the bot. Then the
    /// tank that is alone in the first two sectors of hard mode, by its brain: an ace that
    /// is alone retreats.
    #[test]
    #[ignore = "takes minutes and prints tables"]
    fn the_first_groups() {
        modes_table(&[(1, 2), (3, 5), (6, 8), (9, 12)], 12);
        println!("\nThe tank of sectors 1 and 2 of hard mode, by its brain, on 60 seeds.");
        skirmish_header();
        let mut fights = Vec::new();
        for seed in trial_seeds(60) {
            for sector in [1, 2] {
                fights.extend(skirmish(&seed, sector, Difficulty::Hard, SKIRMISH));
            }
        }
        for brain in 0..=4 {
            let of_brain = |(group, _): &&(Vec<Genome>, Trial)| group[0].brain() == brain;
            let fights: Vec<_> = fights.iter().filter(of_brain).cloned().collect();
            skirmishes(&format!("brain {brain}"), &fights);
        }
    }

    /// The groups of later sectors as each mode makes them: where hard mode gets past the
    /// sector of normal mode that unlocks it.
    #[test]
    #[ignore = "takes minutes and prints a table"]
    fn the_later_groups() {
        modes_table(&[(13, 16), (17, 20), (21, 25), (26, 30)], 6);
    }
}
