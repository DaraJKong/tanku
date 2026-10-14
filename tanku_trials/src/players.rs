//! How the reference players compare: each of them against the same groups, on the same
//! seeds. It says what the yardstick of the other trials is worth, and how much of what a
//! group costs depends on the hands that fight it.

use std::{
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

use tanku_core::{modes::Difficulty, reference::Reference, seed};

use crate::{
    census::{stretch_name, stretches},
    fight::{self, Cause, Outcome, Setup},
    report::{Report, Table, percent},
};

/// What a comparison of the players is asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    /// The first and the last of the sectors whose groups are fought.
    pub sectors: (u32, u32),
    pub seeds: u32,
    pub difficulty: Difficulty,
    pub players: Vec<Reference>,
    /// How long a fight lasts at most, in seconds.
    pub limit: f32,
    /// How many fights run side by side.
    pub threads: usize,
}

/// The seed of a fight by its number, as a player types it.
pub fn seed_text(number: u32) -> String {
    format!("TRIAL{number}")
}

/// Does one thing for each of some items, many side by side, and gives what came of each in
/// the order of the items. Each depends on nothing but its item, so the order they are done
/// in changes nothing.
pub fn side_by_side<Item: Sync, Done: Send>(
    items: &[Item],
    threads: usize,
    work: impl Fn(&Item) -> Done + Sync,
) -> Vec<Done> {
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<Option<Done>>> = Mutex::new(items.iter().map(|_| None).collect());
    thread::scope(|scope| {
        for _ in 0..threads.clamp(1, items.len().max(1)) {
            scope.spawn(|| {
                loop {
                    let number = next.fetch_add(1, Ordering::Relaxed);
                    let Some(item) = items.get(number) else {
                        break;
                    };
                    let made = work(item);
                    done.lock().unwrap()[number] = Some(made);
                }
            });
        }
    });
    let done = done.into_inner().unwrap();
    done.into_iter().map(Option::unwrap).collect()
}

/// Runs fights side by side, and gives their outcomes in the order of the fights.
pub fn run(setups: &[Setup], threads: usize) -> Vec<Option<Outcome>> {
    side_by_side(setups, threads, fight::corridor)
}

/// A number with so many decimals, or a dash where there is nothing to divide by.
pub fn ratio(part: f64, whole: f64, decimals: usize) -> String {
    if whole == 0.0 {
        return "-".to_string();
    }
    format!("{:.decimals$}", part / whole)
}

/// The fights of one player in one stretch of sectors, as a row.
fn row(name: String, fights: &[Outcome]) -> Vec<String> {
    let count = fights.len();
    let sum = |of: fn(&Outcome) -> f64| fights.iter().map(of).sum::<f64>();
    let tanks = sum(|fight| fight.tanks() as f64);
    let fallen = sum(|fight| (fight.tanks() - fight.left()) as f64);
    let seconds = sum(|fight| fight.lasted as f64);
    let losses = sum(|fight| fight.losses.len() as f64);
    let mut lasted: Vec<f32> = fights.iter().map(|fight| fight.lasted).collect();
    lasted.sort_by(f32::total_cmp);
    let median = lasted.get(count / 2);
    let blasts = |fight: &Outcome| (fight.losses.len() - fight.losses_to(Cause::Bullet)) as f64;
    vec![
        name,
        count.to_string(),
        ratio(tanks, count as f64, 1),
        percent(fights.iter().filter(|fight| fight.cleared()).count(), count),
        median.map_or("-".to_string(), |median| format!("{median:.1}")),
        ratio(losses, count as f64, 2),
        ratio(losses * 60.0, seconds, 1),
        ratio(losses, fallen, 2),
        ratio(
            sum(|fight| fight.losses_to(Cause::Bullet) as f64),
            count as f64,
            2,
        ),
        ratio(sum(blasts), count as f64, 2),
        ratio(sum(|fight| fight.own as f64), count as f64, 2),
    ]
}

pub fn players(asked: &Asked) -> Report {
    let rows = stretches(asked.sectors, false);
    // Every fight of every player, stretch after stretch.
    let mut setups = Vec::new();
    let mut of: Vec<(usize, Reference)> = Vec::new();
    for (stretch, &(first, last)) in rows.iter().enumerate() {
        for &player in &asked.players {
            for number in 0..asked.seeds {
                for sector in first..=last {
                    setups.push(Setup {
                        seed: seed_text(number),
                        sector,
                        difficulty: asked.difficulty,
                        player,
                        limit: asked.limit,
                        make: None,
                    });
                    of.push((stretch, player));
                }
            }
        }
    }
    let outcomes = run(&setups, asked.threads);
    let fights = |stretch: usize, player: Reference| -> Vec<Outcome> {
        let fought = of.iter().zip(&outcomes);
        let wanted = fought.filter(|(of, _)| **of == (stretch, player));
        wanted.filter_map(|(_, outcome)| outcome.clone()).collect()
    };

    let mut each = Table::new(
        "Fights",
        "Each player against the group of each sector that is a corridor, from the moment the \
         barrier is crossed. Nothing destroys the tank of the player: a loss is a tank that \
         the player would have lost, to a bullet of the enemies or to a blast, and what hits \
         the tank within 1.5 s of a loss is no other one. The columns are the fights, the \
         tanks of a group, the fights in which the group fell in time, the median of how long \
         it lasted in seconds, the losses in a fight, in a minute and for each enemy that \
         fell, the losses in a fight to bullets and to blasts, and the shots of the player \
         in a fight that came back off a wall onto the tank, which are no losses: they tell \
         of the hands, not of the group.",
        &[
            "sectors, player",
            "fights",
            "tanks",
            "cleared",
            "median s",
            "losses",
            "per min",
            "per kill",
            "bullets",
            "blasts",
            "own shots",
        ],
    );
    let mut columns = vec!["sectors".to_string()];
    columns.extend(asked.players.iter().map(|player| player.name().to_string()));
    let columns: Vec<&str> = columns.iter().map(String::as_str).collect();
    let mut side = Table::new(
        "Losses side by side",
        "The losses in a fight of each player, in the same fights.",
        &columns,
    );
    for (stretch, &sectors) in rows.iter().enumerate() {
        let mut cells = vec![stretch_name(sectors)];
        for &player in &asked.players {
            let fights = fights(stretch, player);
            let name = format!("{}, {}", stretch_name(sectors), player.name());
            each.row(row(name, &fights));
            let losses: usize = fights.iter().map(|fight| fight.losses.len()).sum();
            cells.push(ratio(losses as f64, fights.len() as f64, 2));
        }
        side.row(cells);
    }

    let (first, last) = asked.sectors;
    Report {
        title: "Reference players".to_string(),
        asked: format!(
            "Version {}, {} mode, one player in the standard tank, sectors {first} to {last}, \
             on {} seeds: {} to {}. A fight lasts {} s at most.",
            seed::VERSION,
            asked.difficulty.key(),
            asked.seeds,
            seed_text(0),
            seed_text(asked.seeds - 1),
            asked.limit,
        ),
        balance: Some(tanku_core::balance::FINGERPRINT),
        tables: vec![side, each],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fights_side_by_side_give_what_they_give_one_by_one() {
        let setups: Vec<Setup> = (1..=4)
            .map(|sector| Setup {
                seed: seed_text(1),
                sector,
                difficulty: Difficulty::Normal,
                player: Reference::Regular,
                limit: 10.0,
                make: None,
            })
            .collect();
        assert_eq!(run(&setups, 4), run(&setups, 1));
    }

    #[test]
    fn the_report_has_a_row_for_each_player_in_each_stretch() {
        let asked = Asked {
            sectors: (1, 4),
            seeds: 1,
            difficulty: Difficulty::Normal,
            players: vec![Reference::Novice, Reference::Regular],
            limit: 10.0,
            threads: 4,
        };
        let report = players(&asked);
        // Sectors 1 to 4 lie in two stretches: 1-2 and 3-4.
        assert_eq!(report.tables[0].rows.len(), 2);
        assert_eq!(report.tables[0].columns.len(), 3);
        assert_eq!(report.tables[1].rows.len(), 4);
    }
}
