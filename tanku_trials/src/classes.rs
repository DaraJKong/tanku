//! The classes, each alone: tanks of one class against a reference player, made as the game
//! makes them, with a budget that grows. What a tank buys besides the stats of its class is
//! drawn anew for each tank, so that what is left over many of them is the class itself:
//! the curve of its danger over its points.

use tanku_core::{
    enemy,
    modes::Difficulty,
    seed::{self, seed_value},
    stats::Class,
};

use crate::{
    census::class_name,
    fight::{Make, Outcome, Setup},
    players::{ratio, run, seed_text},
    reference::Reference,
    report::{Report, Table, percent},
};

/// What the trial of the classes is asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    /// The budgets that the tanks are made with, each in turn.
    pub budgets: Vec<u32>,
    /// The sectors that the tanks are made for and fought in: their caps on software and on
    /// the gun, their corridors and the experience of their brains.
    pub sectors: (u32, u32),
    pub seeds: u32,
    /// How many tanks of the class the player fights at once.
    pub count: usize,
    pub difficulty: Difficulty,
    pub player: Reference,
    /// How long a fight lasts at most, in seconds.
    pub limit: f32,
    /// How many fights run side by side.
    pub threads: usize,
}

/// Reads the budgets as the command line writes them: `20,40,60`.
pub fn budgets(text: &str) -> Result<Vec<u32>, String> {
    let budget = |part: &str| match part.trim().parse::<u32>() {
        Ok(budget) if (1..=1000).contains(&budget) => Ok(budget),
        _ => Err(format!("`{part}` is not a budget")),
    };
    text.split(',').map(budget).collect()
}

/// A table with a row for each class and a column for each budget.
fn curve(title: &str, note: &str, asked: &Asked, cell: impl Fn(usize, usize) -> String) -> Table {
    let mut columns = vec!["class".to_string()];
    columns.extend(
        asked
            .budgets
            .iter()
            .map(|budget| format!("{budget} points")),
    );
    let columns: Vec<&str> = columns.iter().map(String::as_str).collect();
    let mut table = Table::new(title, note, &columns);
    for (row, class) in Class::ALL.into_iter().enumerate() {
        let mut cells = vec![class_name(class).to_string()];
        cells.extend((0..asked.budgets.len()).map(|budget| cell(row, budget)));
        table.row(cells);
    }
    table
}

pub fn classes(asked: &Asked) -> Report {
    let (first, last) = asked.sectors;
    let mut setups = Vec::new();
    let mut of: Vec<(usize, usize)> = Vec::new();
    for (row, class) in Class::ALL.into_iter().enumerate() {
        for (column, &budget) in asked.budgets.iter().enumerate() {
            for number in 0..asked.seeds {
                let seed = seed_text(number);
                for sector in first..=last {
                    // Each tank is drawn as the tank of a group at its place would be.
                    let spot = number as usize;
                    let tank = |index: usize| {
                        let (seed, index) = (seed_value(&seed), (spot + index) as u32);
                        enemy::of_class(seed, asked.difficulty, sector, index, class, budget)
                    };
                    setups.push(Setup {
                        seed: seed.clone(),
                        sector,
                        difficulty: asked.difficulty,
                        player: asked.player,
                        limit: asked.limit,
                        make: Some(Make {
                            tanks: (0..asked.count).map(tank).collect(),
                            spot,
                        }),
                    });
                    of.push((row, column));
                }
            }
        }
    }
    let outcomes = run(&setups, asked.threads);
    let fought = |row: usize, column: usize| -> Vec<&Outcome> {
        let fights = of.iter().zip(&outcomes);
        let wanted = fights.filter(|(of, _)| **of == (row, column));
        wanted.filter_map(|(_, outcome)| outcome.as_ref()).collect()
    };
    let lost = |fights: &[&Outcome]| fights.iter().map(|fight| fight.losses.len()).sum::<usize>();
    let tanks = |fights: &[&Outcome]| fights.iter().map(|fight| fight.tanks()).sum::<usize>();
    let spent = |fights: &[&Outcome]| -> f64 {
        let group = fights.iter().flat_map(|fight| &fight.group);
        group.map(|tank| tank.genome.cost() as f64).sum()
    };

    let tables = vec![
        curve(
            "Losses",
            "The losses of the player in a fight against the tanks of a class, by the budget \
             of each of those tanks. Prices are right when the classes are close to each \
             other in a column: equal budget, equal danger.",
            asked,
            |row, column| {
                let fights = fought(row, column);
                ratio(lost(&fights) as f64, fights.len() as f64, 2)
            },
        ),
        curve(
            "Losses for each hundred points",
            "The losses in a fight for each hundred points that the tanks of the fight \
             spent. A class whose row rises gets more of each point as it has more of them.",
            asked,
            |row, column| {
                let fights = fought(row, column);
                ratio(lost(&fights) as f64 * 100.0, spent(&fights), 2)
            },
        ),
        curve(
            "Seconds",
            "How long a tank of the class lasted, in seconds.",
            asked,
            |row, column| {
                let fights = fought(row, column);
                let group = fights.iter().flat_map(|fight| &fight.group);
                let seconds: f64 = group.map(|tank| tank.lasted as f64).sum();
                ratio(seconds, tanks(&fights) as f64, 1)
            },
        ),
        curve(
            "Fights cleared",
            "The fights in which every tank fell in time.",
            asked,
            |row, column| {
                let fights = fought(row, column);
                percent(
                    fights.iter().filter(|fight| fight.cleared()).count(),
                    fights.len(),
                )
            },
        ),
        curve(
            "Points spent",
            "The points that a tank of the class spent, of those that it had.",
            asked,
            |row, column| {
                let fights = fought(row, column);
                ratio(spent(&fights), tanks(&fights) as f64, 0)
            },
        ),
        curve(
            "Tanks lost",
            "The tanks that the player lost in all, which say how sure a number of the \
             other tables is: twenty-five are known to a fifth, a hundred to a tenth.",
            asked,
            |row, column| lost(&fought(row, column)).to_string(),
        ),
    ];

    let fights = fought(0, 0).len();
    let together = match asked.count {
        1 => "one tank".to_string(),
        count => format!("{count} tanks of the class at once"),
    };
    Report {
        title: "Classes".to_string(),
        asked: format!(
            "Version {}, {} mode, the {} player in the standard tank against {together}, \
             each made as the game makes a tank of that class with the budget of its column, \
             under the caps of sectors {first} to {last}, and fought in the corridors of \
             those sectors with their brains, on {} seeds: {} to {}. That is {fights} fights \
             for each number. A fight lasts {} s at most.",
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

    #[test]
    fn budgets_are_read_as_they_are_written() {
        assert_eq!(budgets("20, 40,60"), Ok(vec![20, 40, 60]));
        assert!(budgets("20,,60").is_err());
        assert!(budgets("0").is_err());
        assert!(budgets("many").is_err());
    }

    #[test]
    fn the_report_has_a_row_for_each_class_and_a_column_for_each_budget() {
        let report = classes(&Asked {
            budgets: vec![30, 90],
            sectors: (21, 22),
            seeds: 1,
            count: 2,
            difficulty: Difficulty::Normal,
            player: Reference::Regular,
            limit: 8.0,
            threads: 8,
        });
        assert_eq!(report.tables.len(), 6);
        for table in &report.tables {
            assert_eq!(
                (table.rows.len(), table.columns.len()),
                (11, 3),
                "{}",
                table.title
            );
        }
        // A tank spends more when it has more, and never more than it has.
        let spent = &report.tables[4];
        for row in &spent.rows {
            let (low, high): (f64, f64) = (row[1].parse().unwrap(), row[2].parse().unwrap());
            assert!(low <= 30.0 && high <= 90.0 && low < high, "{row:?}");
        }
    }
}
