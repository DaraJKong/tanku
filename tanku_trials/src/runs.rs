//! The runs: a reference player alone, from the start bay until the last life is lost, with
//! the lives, the checkpoints and the arenas of the game. They say how far a player gets,
//! where the tanks are lost and to what.
//!
//! The player fights and doesn't drive the corridor: once a sector is cleared, the tank is
//! put through its barrier, over the life that an arena left if one is missing. The time of
//! a run is the time of its fights, and the drive between them is an estimate.

use bevy::prelude::*;
use tanku_core::{
    CorePlugin,
    map::SectorKind,
    modes::{Difficulty, Unlock},
    rules::{APPEAR_ROOM, SOLO_LIVES, TICK},
    seed,
    sim::{
        Barrier, Blow, Enemy, Life, Player, Position, Run, RunPhase, RunRequest, SimEvent, Tank,
        TankCommand, Team, Weapon,
    },
    stats::{Ammo, Class, Genome},
};
use tanku_server::{ServerPlugin, brain, field::Field};

use crate::{
    census::{class_name, stretch_name, stretches},
    fight::{events, tick},
    players::{ratio, seed_text, side_by_side},
    reference::{Pilot, Reference, Sight},
    report::{Report, Table, percent},
};

/// What the runs are asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    pub seeds: u32,
    pub difficulty: Difficulty,
    pub players: Vec<Reference>,
    /// The sector at which a run is stopped if it gets there.
    pub last: u32,
    /// How long a player may stay in a sector without clearing it or falling, in seconds. A
    /// run that does is stopped: its player and the enemies that are left can't end it.
    pub patience: f32,
    /// How many runs go side by side.
    pub threads: usize,
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// The last life was lost.
    Lost,
    /// A fight went on for longer than the patience of the trial.
    Stalled,
    /// The last sector of the trial was cleared.
    Finished,
}

/// A tank that the player lost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Death {
    pub sector: u32,
    /// What destroyed it, when the game knows whose doing it was.
    pub blow: Option<Blow>,
}

/// How a run went.
#[derive(Clone, Debug, PartialEq)]
pub struct Journey {
    /// The score: the furthest sector cleared.
    pub cleared: u32,
    pub end: End,
    /// The time of the fights, in seconds, with the waits after a tank was lost.
    pub fought: f32,
    /// An estimate of the drive between the fights, in seconds: the way from one barrier to
    /// the next in a straight line, at the speed of the standard tank.
    pub drive: f32,
    pub deaths: Vec<Death>,
    /// Whether each sector that was entered is an arena, from sector 1.
    pub arenas: Vec<bool>,
    /// What was left in the sector of a run that stalled: the fight that its player and
    /// the enemies could not end.
    pub held: Option<String>,
}

impl Journey {
    /// How many sectors the player entered.
    pub fn entered(&self) -> u32 {
        self.arenas.len() as u32
    }

    pub fn minutes(&self) -> f64 {
        (self.fought + self.drive) as f64 / 60.0
    }
}

/// The barrier at the end of a sector.
fn barrier_of(world: &mut World, sector: u32) -> Option<Barrier> {
    let mut barriers = world.query::<&Barrier>();
    barriers
        .iter(world)
        .find(|barrier| barrier.sector == sector)
        .copied()
}

/// What is left in a sector that a run stalls in: each enemy with what makes it hard to end,
/// and the tanks of an arena that wait for room.
fn held(world: &mut World, sector: u32) -> String {
    let mut players = world.query_filtered::<&Position, With<Player>>();
    let me = players.iter(world).next().map(|at| at.0);
    let mut foes = world.query_filtered::<(&Tank, &Position), With<Enemy>>();
    let foes: Vec<(Genome, Vec2)> = foes
        .iter(world)
        .filter(|(tank, _)| tank.sector == sector)
        .map(|(tank, at)| (tank.genome, at.0))
        .collect();
    let field = world.resource::<Field>();
    let mut left: Vec<String> = foes
        .iter()
        .map(|(genome, at)| {
            let engine = match genome.top_speed() {
                0.0 => "no engine".to_string(),
                speed => format!("{speed} m/s"),
            };
            let seen = match me {
                Some(me) if field.sees(me, *at) => format!("{:.0} m away", me.distance(*at)),
                Some(me) => format!("{:.0} m away and out of sight", me.distance(*at)),
                None => "with the player away".to_string(),
            };
            let class = class_name(genome.class).to_lowercase();
            format!("{class} with brain {}, {engine}, {seen}", genome.brain())
        })
        .collect();
    let waiting = field
        .arenas
        .get(&sector)
        .map_or(0, |arena| arena.incoming.len());
    if waiting > 0 {
        left.push(format!("{waiting} tanks of the arena that wait for room"));
    }
    match left.is_empty() {
        true => "nothing".to_string(),
        false => left.join("; "),
    }
}

/// Where a player who has nobody to fight drives, to leave room to the tanks of an arena
/// that wait for it: away from the places that are not marked yet and that the tank is too
/// close to, all of them at once. Away from the nearest one alone, a tank between two places
/// drives back and forth between them and leaves neither.
fn off_the_marks(waiting: &[Vec2], me: Vec2) -> Option<Vec2> {
    let near: Vec<Vec2> = waiting
        .iter()
        .copied()
        .filter(|place| place.distance(me) < APPEAR_ROOM + 2.0)
        .collect();
    let first = *near.first()?;
    let middle = near.iter().sum::<Vec2>() / near.len() as f32;
    let away = me - middle;
    if away.length() >= 0.5 {
        return Some(away);
    }
    // In the middle of them, the way out is across the line that joins them.
    let across = (first - middle).perp();
    Some(if across.length() > 0.1 {
        across
    } else {
        Vec2::X
    })
}

/// Plays a run of a single player on a seed, in the hands of a reference player.
pub fn journey(seed: &str, difficulty: Difficulty, player: Reference, asked: &Asked) -> Journey {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
    app.edit_schedule(FixedUpdate, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    app.world_mut().write_message(RunRequest::Start {
        seed: seed.to_string(),
        players: 1,
        drawn: true,
        difficulty,
        free: false,
        arena: false,
        tanks: Vec::new(),
    });
    tick(&mut app);
    events(&mut app);

    let mut journey = Journey {
        cleared: 0,
        end: End::Lost,
        fought: 0.0,
        drive: 0.0,
        deaths: Vec::new(),
        arenas: Vec::new(),
        held: None,
    };
    let mut pilot = Pilot::new(player);
    // How long the player has been in the sector that is fought.
    let mut stayed = 0.0f32;
    loop {
        let run = app.world().resource::<Run>().clone();
        journey.cleared = run.cleared;
        if run.phase == RunPhase::Over {
            break;
        }
        if run.cleared >= asked.last {
            journey.end = End::Finished;
            break;
        }
        if stayed > asked.patience {
            journey.end = End::Stalled;
            journey.held = Some(held(app.world_mut(), run.sector));
            break;
        }
        let world = app.world_mut();
        let front = run.sector;
        let entity = world
            .query_filtered::<Entity, With<Player>>()
            .iter(world)
            .next();
        let cleared = world.resource::<Field>().cleared.contains(&front);
        match entity {
            // The tank comes back after a moment.
            None => {}
            // The sector is cleared, or the player is back in front of it: the tank is put
            // through the barrier, over the life that an arena left if one is missing.
            Some(entity) if cleared => {
                let Some(barrier) = barrier_of(world, front) else {
                    journey.end = End::Stalled;
                    break;
                };
                let life = world.query::<(&Life, &Position)>().iter(world).next();
                let life = life.map(|(_, at)| at.0).filter(|_| run.lives < SOLO_LIVES);
                let middle = (barrier.from + barrier.to) / 2.0;
                let places = [middle - barrier.outward, middle + barrier.outward * 2.0];
                for at in life.into_iter().chain(places) {
                    // What still flies or lies there can destroy the tank on its way.
                    let Ok(mut tank) = app.world_mut().get_entity_mut(entity) else {
                        break;
                    };
                    tank.get_mut::<Position>().unwrap().0 = at;
                    *tank.get_mut::<TankCommand>().unwrap() = default();
                    tick(&mut app);
                }
                (pilot, stayed) = (Pilot::new(player), 0.0);
                // A sector that is entered for the first time: its kind, and the way to
                // its own barrier.
                let entered = app.world().resource::<Run>().sector;
                if entered as usize > journey.arenas.len() {
                    let world = app.world_mut();
                    let mut field = world.resource_mut::<Field>();
                    journey
                        .arenas
                        .push(field.corridor.plan(entered).kind == SectorKind::Arena);
                    if let Some(next) = barrier_of(world, entered) {
                        let way = middle.distance((next.from + next.to) / 2.0);
                        journey.drive += way / Genome::PLAYER.top_speed();
                    }
                }
                continue;
            }
            // The player fights the nearest enemy of the sector, if one is there: the tanks
            // of an arena come in waves.
            Some(entity) => {
                let me = world.get::<Position>(entity).unwrap().0;
                let mut foes = world.query_filtered::<(Entity, &Tank, &Position), With<Enemy>>();
                let near = foes.iter(world).filter(|(_, tank, _)| tank.sector == front);
                let nearest = near
                    .min_by(|a, b| a.2.0.distance(me).total_cmp(&b.2.0.distance(me)))
                    .map(|(foe, ..)| foe);
                let sight = Sight::of(world);
                let mut order = pilot.order(world, &sight, entity, nearest);
                // A tank of an arena waits for room: with nobody to fight and nothing to
                // dodge, the player drives off the places that wait for it.
                let field = world.resource::<Field>();
                let waiting: Vec<Vec2> = field
                    .arenas
                    .get(&front)
                    .into_iter()
                    .flat_map(|arena| &arena.incoming)
                    .filter(|(_, marked)| marked.is_none())
                    .map(|(tank, _)| tank.at)
                    .collect();
                if let Some(away) = off_the_marks(&waiting, me)
                    && nearest.is_none()
                    && order.drive == Vec2::ZERO
                {
                    let tank = world.get::<Tank>(entity).unwrap();
                    order.drive = brain::steer(field, tank, me, away);
                }
                *world.get_mut::<TankCommand>(entity).unwrap() = order;
            }
        }
        tick(&mut app);
        stayed += TICK;
        for event in events(&mut app) {
            if let SimEvent::TankDestroyed {
                team: Team::Players,
                ..
            } = event
            {
                let blow = app.world().resource::<Run>().lost_to;
                journey.deaths.push(Death {
                    sector: front,
                    blow,
                });
                stayed = 0.0;
            }
        }
    }
    journey.fought = app.world().resource::<Run>().ticks as f32 * TICK;
    journey
}

/// The value below which a share of some sorted values lies.
fn quantile(sorted: &[u32], share: f64) -> String {
    match sorted.len() {
        0 => "-".to_string(),
        count => sorted[((count - 1) as f64 * share).round() as usize].to_string(),
    }
}

/// What destroyed a tank of the player, as a row of a table tells it.
fn weapon(blow: Option<Blow>) -> &'static str {
    match blow {
        None => "Nobody's doing",
        Some(blow) if blow.team == Team::Players => "A shot of the player",
        Some(blow) => match (blow.weapon, blow.genome.ammo()) {
            (Weapon::Mine, _) => "Mine",
            (Weapon::Crate, _) => "Crate",
            (Weapon::Bullet, Ammo::Normal) => "Bullet, normal",
            (Weapon::Bullet, Ammo::Piercing) => "Bullet, piercing",
            (Weapon::Bullet, Ammo::Seeking) => "Bullet, seeking",
            (Weapon::Bullet, Ammo::Splitting) => "Bullet, splitting",
            (Weapon::Bullet, Ammo::Exploding) => "Bullet, exploding",
        },
    }
}

const WEAPONS: [&str; 9] = [
    "Bullet, normal",
    "Bullet, piercing",
    "Bullet, seeking",
    "Bullet, splitting",
    "Bullet, exploding",
    "Mine",
    "Crate",
    "A shot of the player",
    "Nobody's doing",
];

pub fn runs(asked: &Asked) -> Report {
    let mut jobs: Vec<(Reference, u32)> = Vec::new();
    for &player in &asked.players {
        jobs.extend((0..asked.seeds).map(|number| (player, number)));
    }
    let played = side_by_side(&jobs, asked.threads, |&(player, number)| {
        journey(&seed_text(number), asked.difficulty, player, asked)
    });
    let of = |player: Reference| -> Vec<&Journey> {
        let runs = jobs.iter().zip(&played).filter(|(job, _)| job.0 == player);
        runs.map(|(_, journey)| journey).collect()
    };
    let named = |columns: &[&str]| -> Vec<String> {
        let mut all = vec!["player".to_string()];
        all.extend(columns.iter().map(|column| column.to_string()));
        all
    };
    let table = |title: &str, note: &str, columns: Vec<String>| {
        let columns: Vec<&str> = columns.iter().map(String::as_str).collect();
        Table::new(title, note, &columns)
    };

    let mut scores = table(
        "Runs",
        &format!(
            "The sectors that a run cleared: their mean, the score that one run in ten, in \
             four, in two stayed under, and so on, and the best. Then how the runs ended: \
             with the last of the {SOLO_LIVES} lives of a single player, in a fight that \
             neither side could end, or at the last sector of the trial. Then the median of \
             their length in minutes: of their fights alone, and with an estimate of the \
             drive between them."
        ),
        named(&[
            "runs",
            "mean",
            "1 in 10",
            "1 in 4",
            "median",
            "3 in 4",
            "9 in 10",
            "best",
            "lost",
            "stalled",
            "finished",
            "minutes fought",
            "minutes",
        ]),
    );
    const MARKS: [u32; 7] = [5, 10, 15, 20, 30, 40, 50];
    let marks: Vec<u32> = MARKS
        .into_iter()
        .filter(|mark| *mark <= asked.last)
        .collect();
    let mut reached = table(
        "Sectors cleared",
        &format!(
            "The runs that cleared a sector. Clearing sector {} unlocks arena mode, sector {} \
             the chosen seed, sector {} free mode and sector {} hard mode.",
            Unlock::ArenaMode.sector(),
            Unlock::ChosenSeed.sector(),
            Unlock::FreeMode.sector(),
            Unlock::HardMode.sector(),
        ),
        named(
            &marks
                .iter()
                .map(|mark| format!("sector {mark}"))
                .collect::<Vec<_>>()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        ),
    );
    let mut kinds = table(
        "Corridors and arenas",
        "The sectors of each kind that the runs entered, and the tanks that the player lost \
         for each sector entered.",
        named(&["corridors", "lost in each", "arenas", "lost in each arena"]),
    );
    for &player in &asked.players {
        let runs = of(player);
        let count = runs.len();
        let mut cleared: Vec<u32> = runs.iter().map(|run| run.cleared).collect();
        cleared.sort();
        let ended = |end: End| percent(runs.iter().filter(|run| run.end == end).count(), count);
        let median = |of: &dyn Fn(&Journey) -> f64| {
            let mut values: Vec<f64> = runs.iter().map(|run| of(run)).collect();
            values.sort_by(f64::total_cmp);
            values
                .get(count / 2)
                .map_or("-".to_string(), |value| format!("{value:.1}"))
        };
        scores.row(vec![
            player.name().to_string(),
            count.to_string(),
            ratio(cleared.iter().sum::<u32>() as f64, count as f64, 1),
            quantile(&cleared, 0.1),
            quantile(&cleared, 0.25),
            quantile(&cleared, 0.5),
            quantile(&cleared, 0.75),
            quantile(&cleared, 0.9),
            quantile(&cleared, 1.0),
            ended(End::Lost),
            ended(End::Stalled),
            ended(End::Finished),
            median(&|run| run.fought as f64 / 60.0),
            median(&|run| run.minutes()),
        ]);
        let mut row = vec![player.name().to_string()];
        for &mark in &marks {
            row.push(percent(
                cleared.iter().filter(|score| **score >= mark).count(),
                count,
            ));
        }
        reached.row(row);

        let mut row = vec![player.name().to_string()];
        for arena in [false, true] {
            let entered: usize = runs
                .iter()
                .map(|run| run.arenas.iter().filter(|kind| **kind == arena).count())
                .sum();
            let is = |run: &Journey, death: &Death| {
                run.arenas
                    .get(death.sector as usize - 1)
                    .is_some_and(|kind| *kind == arena)
            };
            let lost: usize = runs
                .iter()
                .map(|run| run.deaths.iter().filter(|death| is(run, death)).count())
                .sum();
            row.extend([entered.to_string(), ratio(lost as f64, entered as f64, 2)]);
        }
        kinds.row(row);
    }

    let mut places = Table::new(
        "Where the tanks are lost",
        "For each stretch of sectors: the sectors that the runs entered, the tanks that the \
         player lost there, for each sector entered, and the runs that ended there with \
         their last life, of all the runs.",
        &[
            "sectors, player",
            "entered",
            "lost",
            "lost in each",
            "runs ended",
        ],
    );
    for sectors in stretches((1, asked.last), false) {
        let within = |sector: u32| (sectors.0..=sectors.1).contains(&sector);
        for &player in &asked.players {
            let runs = of(player);
            let entered: u32 = runs
                .iter()
                .map(|run| {
                    (sectors.0..=sectors.1)
                        .filter(|sector| *sector <= run.entered())
                        .count() as u32
                })
                .sum();
            let lost: usize = runs
                .iter()
                .map(|run| {
                    run.deaths
                        .iter()
                        .filter(|death| within(death.sector))
                        .count()
                })
                .sum();
            let ended = runs
                .iter()
                .filter(|run| run.end == End::Lost && within(run.cleared + 1))
                .count();
            places.row(vec![
                format!("{}, {}", stretch_name(sectors), player.name()),
                entered.to_string(),
                lost.to_string(),
                ratio(lost as f64, entered as f64, 2),
                percent(ended, runs.len()),
            ]);
        }
    }

    let columns: Vec<String> = asked
        .players
        .iter()
        .map(|player| player.name().to_string())
        .collect();
    let shares = |title: &str,
                  note: &str,
                  first: &str,
                  rows: &[&str],
                  row_of: &dyn Fn(&Death) -> &'static str| {
        let mut all = vec![first.to_string()];
        all.extend(columns.clone());
        let mut table = table(title, note, all);
        for name in rows {
            let mut row = vec![name.to_string()];
            for &player in &asked.players {
                let runs = of(player);
                let deaths: Vec<&Death> = runs.iter().flat_map(|run| &run.deaths).collect();
                let these = deaths.iter().filter(|death| row_of(death) == *name).count();
                row.push(percent(these, deaths.len()));
            }
            table.row(row);
        }
        table
    };
    let weapons = shares(
        "What destroyed the player",
        "The tanks that each player lost, by what destroyed them.",
        "weapon",
        &WEAPONS,
        &|death| weapon(death.blow),
    );
    let class_names: Vec<&str> = Class::ALL
        .into_iter()
        .map(class_name)
        .chain(["Nobody"])
        .collect();
    let classes = shares(
        "Whose doing it was",
        "The tanks that each player lost, by the class of the enemy that fired the bullet, \
         laid the mine or set off the crate. The census tells how many of the tanks each \
         class is, sector by sector.",
        "class",
        &class_names,
        &|death| match death.blow {
            Some(blow) if blow.team == Team::Enemies => class_name(blow.genome.class),
            _ => "Nobody",
        },
    );

    let mut stalls = Table::new(
        "Runs that stalled",
        "Each run that was stopped in a fight that neither side could end: its sector, and \
         what was left in it.",
        &["player, seed", "sector", "left"],
    );
    for (job, journey) in jobs.iter().zip(&played) {
        if let Some(left) = &journey.held {
            stalls.row(vec![
                format!("{}, {}", job.0.name(), seed_text(job.1)),
                journey.entered().to_string(),
                left.clone(),
            ]);
        }
    }

    Report {
        title: "Runs".to_string(),
        asked: format!(
            "Version {}, {} mode, one player in the standard tank, from the start bay, on {} \
             seeds: {} to {}. A run is stopped once it has cleared sector {}, or when a fight \
             lasts {} s. The player doesn't drive the corridor: the tank is put through each \
             barrier once its sector is cleared.",
            seed::VERSION,
            asked.difficulty.key(),
            asked.seeds,
            seed_text(0),
            seed_text(asked.seeds - 1),
            asked.last,
            asked.patience,
        ),
        balance: Some(tanku_core::balance::FINGERPRINT),
        tables: vec![scores, reached, places, kinds, weapons, classes, stalls],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked() -> Asked {
        Asked {
            seeds: 3,
            difficulty: Difficulty::Normal,
            players: vec![Reference::Novice, Reference::Regular],
            last: 6,
            patience: 60.0,
            threads: 6,
        }
    }

    #[test]
    fn a_run_goes_the_same_way_twice_and_ends() {
        let asked = asked();
        let run = journey("TRIAL0", Difficulty::Normal, Reference::Regular, &asked);
        assert_eq!(
            run,
            journey("TRIAL0", Difficulty::Normal, Reference::Regular, &asked)
        );
        assert!(run.cleared <= 6 && run.fought > 0.0);
        // A run that was lost lost a tank for each life, give or take the lives that
        // arenas left.
        match run.end {
            End::Lost => assert!(run.deaths.len() >= SOLO_LIVES as usize),
            End::Finished => assert_eq!(run.cleared, 6),
            End::Stalled => {}
        }
        // Every sector that was cleared was entered, and so was the one after it unless
        // the run was stopped at the last one.
        assert!(run.entered() >= run.cleared);
        assert!(run.deaths.iter().all(|death| death.sector <= run.entered()));
    }

    #[test]
    fn a_player_with_nobody_to_fight_leaves_room_to_the_tanks_that_wait() {
        let reach = APPEAR_ROOM + 2.0;
        // Nothing waits near: the player stays.
        assert_eq!(off_the_marks(&[], Vec2::ZERO), None);
        assert_eq!(off_the_marks(&[Vec2::X * (reach + 1.0)], Vec2::ZERO), None);
        // Away from a place that waits, and from the middle of several.
        let away = off_the_marks(&[Vec2::X * 2.0], Vec2::ZERO).unwrap();
        assert!(away.normalize().dot(Vec2::NEG_X) > 0.99);
        // Between two places, the way out is across the line that joins them, and it never
        // turns back from one tick to the next: a tank that drove away from the nearer one
        // would come back to it from the other.
        let places = [Vec2::new(-2.5, 0.0), Vec2::new(2.5, 0.0)];
        let mut me = Vec2::new(0.3, 0.1);
        let first = off_the_marks(&places, me).unwrap().normalize();
        assert!(first.x.abs() < 0.2, "{first:?}");
        for _ in 0..120 {
            let Some(away) = off_the_marks(&places, me) else {
                break;
            };
            assert!(away.normalize().dot(first) > 0.5);
            me += away.normalize() * 5.0 * TICK;
        }
        assert!(places.iter().all(|place| place.distance(me) > APPEAR_ROOM));
        // On the one place that waits, any way is a way out.
        assert!(off_the_marks(&[Vec2::ZERO], Vec2::ZERO).is_some());
    }

    #[test]
    fn a_run_that_stalls_tells_what_held_it() {
        // No fight ends within a twentieth of a second: the run stalls in its first sector.
        let asked = Asked {
            patience: 0.05,
            ..asked()
        };
        let run = journey("TRIAL0", Difficulty::Normal, Reference::Regular, &asked);
        assert_eq!((run.end, run.cleared), (End::Stalled, 0));
        let held = run.held.expect("what held the run");
        assert!(
            held.contains("with brain") && held.contains("m away"),
            "{held}"
        );
        // A run that ends another way was held by nothing.
        let run = journey(
            "TRIAL0",
            Difficulty::Normal,
            Reference::Regular,
            &self::asked(),
        );
        assert!(run.end != End::Stalled && run.held.is_none());
    }

    #[test]
    fn the_report_has_a_row_for_each_player() {
        let report = runs(&asked());
        let titles: Vec<&str> = report
            .tables
            .iter()
            .map(|table| table.title.as_str())
            .collect();
        let wanted = [
            "Runs",
            "Sectors cleared",
            "Where the tanks are lost",
            "Corridors and arenas",
            "What destroyed the player",
            "Whose doing it was",
            "Runs that stalled",
        ];
        assert_eq!(titles, wanted);
        assert_eq!(report.tables[0].rows.len(), 2);
        // Sector 5 is the only mark within six sectors.
        assert_eq!(report.tables[1].columns, ["player", "sector 5"]);
        assert_eq!(report.tables[4].columns, ["weapon", "novice", "regular"]);
    }
}
