//! The skirmishes: one reference player against the group of every sector that is a
//! corridor, as the game makes it. They say what a stretch of sectors costs, and which of
//! its tanks the cost comes from: by their points, their class, their bullets and their
//! software.

use std::collections::BTreeMap;

use tanku_core::{
    modes::Difficulty,
    seed,
    stats::{Ammo, Genome},
};

use crate::{
    census::{class_name, stretch_name, stretches},
    fight::{Cause, Outcome, Setup},
    players::{ratio, run, seed_text},
    reference::Reference,
    report::{Report, Table, percent},
};

/// What the skirmishes are asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    /// The first and the last of the sectors whose groups are fought.
    pub sectors: (u32, u32),
    pub seeds: u32,
    pub difficulty: Difficulty,
    pub player: Reference,
    /// How long a fight lasts at most, in seconds.
    pub limit: f32,
    /// One row for each sector, in place of one for each stretch of sectors.
    pub each: bool,
    /// How many fights run side by side.
    pub threads: usize,
}

/// A tank of a group that was fought, with what it did.
struct Tank {
    genome: Genome,
    lasted: f32,
    fell: bool,
    /// The losses that were its doing.
    losses: usize,
}

/// The fights of a stretch of sectors, as a row.
fn stretch(name: String, fights: &[&Outcome]) -> Vec<String> {
    let count = fights.len() as f64;
    let sum = |of: &dyn Fn(&Outcome) -> usize| fights.iter().map(|fight| of(fight)).sum::<usize>();
    let fallen = sum(&|fight| fight.tanks() - fight.left());
    let seconds: f64 = fights.iter().map(|fight| fight.lasted as f64).sum();
    let losses = sum(&|fight| fight.losses.len()) as f64;
    let mut lasted: Vec<f32> = fights.iter().map(|fight| fight.lasted).collect();
    lasted.sort_by(f32::total_cmp);
    let median = lasted.get(fights.len() / 2);
    let mut row = vec![
        name,
        fights.len().to_string(),
        ratio(sum(&|fight| fight.tanks()) as f64, count, 1),
        percent(sum(&|fight| fight.cleared() as usize), fights.len()),
        median.map_or("-".to_string(), |median| format!("{median:.1}")),
        ratio(losses, count, 2),
        ratio(losses * 60.0, seconds, 1),
        ratio(losses, fallen as f64, 2),
    ];
    for cause in Cause::ALL {
        row.push(ratio(sum(&|fight| fight.losses_to(cause)) as f64, count, 2));
    }
    row.push(ratio(sum(&|fight| fight.own as usize) as f64, count, 2));
    row
}

/// A table of the tanks by something that they have: for each kind, how many there were,
/// the points that they spent, how many fell, how long they lasted, and the losses that
/// were their doing, for each tank, for each minute that they lasted and for each hundred
/// points that they spent.
fn breakdown(
    title: &str,
    note: &str,
    tanks: &[Tank],
    kind: impl Fn(&Genome) -> (u32, String),
) -> Table {
    let mut table = Table::new(
        title,
        note,
        &[
            "tanks of",
            "tanks",
            "points",
            "fell",
            "lasted s",
            "losses",
            "per min",
            "per 100 points",
        ],
    );
    let mut kinds: BTreeMap<(u32, String), Vec<&Tank>> = BTreeMap::new();
    for tank in tanks {
        kinds.entry(kind(&tank.genome)).or_default().push(tank);
    }
    for ((_, name), tanks) in kinds {
        let count = tanks.len() as f64;
        let points: f64 = tanks.iter().map(|tank| tank.genome.cost() as f64).sum();
        let seconds: f64 = tanks.iter().map(|tank| tank.lasted as f64).sum();
        let losses = tanks.iter().map(|tank| tank.losses).sum::<usize>() as f64;
        table.row(vec![
            name,
            tanks.len().to_string(),
            ratio(points, count, 0),
            percent(tanks.iter().filter(|tank| tank.fell).count(), tanks.len()),
            ratio(seconds, count, 1),
            ratio(losses, count, 2),
            ratio(losses * 60.0, seconds, 2),
            ratio(losses * 100.0, points, 2),
        ]);
    }
    table
}

fn ammo_name(ammo: Ammo) -> &'static str {
    match ammo {
        Ammo::Normal => "Normal",
        Ammo::Piercing => "Piercing",
        Ammo::Seeking => "Seeking",
        Ammo::Splitting => "Splitting",
        Ammo::Exploding => "Exploding",
    }
}

/// The tables of the tanks. A loss counts for the tank that fired the bullet or laid the
/// mine, whoever was in the group with it.
fn breakdowns(tanks: &[Tank]) -> Vec<Table> {
    let numbered = |number: u32, of: &str| (number, format!("{of} {number}"));
    vec![
        breakdown(
            "Points",
            "The tanks by the points that they spent. Prices are right when the losses for \
             each hundred points are about the same in every row of the tables below, and \
             they say nothing here of a tank that was strong among weak ones.",
            tanks,
            |genome| {
                let band = (genome.cost() / 20).min(5);
                let name = match band {
                    5 => "100 and more".to_string(),
                    band => format!("{}-{}", band * 20, band * 20 + 19),
                };
                (band, name)
            },
        ),
        breakdown("Classes", "The tanks by their class.", tanks, |genome| {
            (genome.class as u32, class_name(genome.class).to_string())
        }),
        breakdown(
            "Bullets",
            "The tanks by the type of their bullets.",
            tanks,
            |genome| (genome.ammo() as u32, ammo_name(genome.ammo()).to_string()),
        ),
        breakdown(
            "Brains",
            "The tanks by the tier of their brain.",
            tanks,
            |genome| numbered(genome.brain() as u32, "Brain"),
        ),
        breakdown(
            "Ballistics",
            "The tanks by the tier of their ballistics.",
            tanks,
            |genome| numbered(genome.ballistics() as u32, "Ballistics"),
        ),
        breakdown(
            "Guns",
            "The tanks by the speed of their bullets.",
            tanks,
            |genome| {
                let speed = genome.bullet_speed();
                ((speed * 10.0) as u32, format!("{speed} m/s"))
            },
        ),
        breakdown(
            "Engines",
            "The tanks by their top speed.",
            tanks,
            |genome| {
                let speed = genome.top_speed();
                let name = match speed {
                    0.0 => "Stands still".to_string(),
                    speed => format!("{speed} m/s"),
                };
                (speed as u32, name)
            },
        ),
    ]
}

pub fn skirmish(asked: &Asked) -> Report {
    let rows = stretches(asked.sectors, asked.each);
    let mut setups = Vec::new();
    let mut of: Vec<usize> = Vec::new();
    for (row, &(first, last)) in rows.iter().enumerate() {
        for number in 0..asked.seeds {
            for sector in first..=last {
                setups.push(Setup {
                    seed: seed_text(number),
                    sector,
                    difficulty: asked.difficulty,
                    player: asked.player,
                    limit: asked.limit,
                });
                of.push(row);
            }
        }
    }
    let outcomes = run(&setups, asked.threads);
    let fought = || of.iter().zip(&outcomes);
    let fought = || fought().filter_map(|(row, outcome)| Some((*row, outcome.as_ref()?)));

    let mut sectors = Table::new(
        "Sectors",
        "The group of each sector that is a corridor, from the moment the barrier is crossed. \
         Nothing destroys the tank of the player: a loss is a tank that the player would have \
         lost, and what hits the tank within 1.5 s of a loss is no other one. The columns are \
         the fights, the tanks of a group, the fights in which the group fell in time, the \
         median of how long it lasted in seconds, the losses in a fight, in a minute and for \
         each enemy that fell, the losses in a fight to bullets, to mines, to exploding \
         bullets and to crates, and the shots of the player in a fight that came back off a \
         wall onto the tank, which are no losses.",
        &[
            "sectors",
            "fights",
            "tanks",
            "cleared",
            "median s",
            "losses",
            "per min",
            "per kill",
            "bullets",
            "mines",
            "exploding",
            "crates",
            "own shots",
        ],
    );
    for (row, &stretched) in rows.iter().enumerate() {
        let fights: Vec<&Outcome> = fought()
            .filter(|(of, _)| *of == row)
            .map(|(_, fight)| fight)
            .collect();
        sectors.row(stretch(stretch_name(stretched), &fights));
    }

    let mut tanks = Vec::new();
    for (_, fight) in fought() {
        for (number, tank) in fight.group.iter().enumerate() {
            tanks.push(Tank {
                genome: tank.genome,
                lasted: tank.lasted,
                fell: tank.fell,
                losses: fight.losses_by(number),
            });
        }
    }
    let mut tables = vec![sectors];
    tables.extend(breakdowns(&tanks));

    let (first, last) = asked.sectors;
    Report {
        title: "Skirmishes".to_string(),
        asked: format!(
            "Version {}, {} mode, the {} player in the standard tank, sectors {first} to \
             {last}, on {} seeds: {} to {}. A fight lasts {} s at most. The tables of the \
             tanks hold every tank of these sectors together: a late sector has more of them, \
             with more points and better brains, so a narrower `--sectors` compares like \
             with like.",
            seed::VERSION,
            asked.difficulty.key(),
            asked.player.name(),
            asked.seeds,
            seed_text(0),
            seed_text(asked.seeds - 1),
            asked.limit,
        ),
        tables,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Report {
        skirmish(&Asked {
            sectors: (3, 8),
            seeds: 2,
            difficulty: Difficulty::Normal,
            player: Reference::Novice,
            limit: 15.0,
            each: false,
            threads: 4,
        })
    }

    fn number(cell: &str) -> f64 {
        cell.parse().unwrap()
    }

    #[test]
    fn the_report_has_a_row_for_each_stretch_and_a_table_for_each_way_to_tell_tanks_apart() {
        let report = report();
        let titles: Vec<&str> = report
            .tables
            .iter()
            .map(|table| table.title.as_str())
            .collect();
        let wanted = [
            "Sectors",
            "Points",
            "Classes",
            "Bullets",
            "Brains",
            "Ballistics",
            "Guns",
            "Engines",
        ];
        assert_eq!(titles, wanted);
        // Sectors 3 to 8 lie in two stretches: 3-5 and 6-8.
        assert_eq!(report.tables[0].rows.len(), 2);
    }

    #[test]
    fn every_table_of_the_tanks_holds_every_tank_once() {
        let report = report();
        let tanks = |table: &Table| table.rows.iter().map(|row| number(&row[1])).sum::<f64>();
        let fought: f64 = report.tables[0]
            .rows
            .iter()
            .map(|row| number(&row[1]) * number(&row[2]))
            .sum();
        for table in &report.tables[1..] {
            assert!((tanks(table) - fought).abs() < 1.0, "{}", table.title);
        }
    }

    #[test]
    fn the_same_skirmishes_give_the_same_report() {
        assert_eq!(report().csv(), report().csv());
    }
}
