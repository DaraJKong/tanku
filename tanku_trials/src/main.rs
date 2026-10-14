//! The trials of the balance of the game: what its rules make, counted and measured without
//! anyone at the keyboard, and told in reports that can be compared from one change to the
//! next.
//!
//! `census` fights nothing: it counts what the rules produce for each stretch of sectors, on
//! many seeds. `players` puts the reference players, whose hands hold the players' tank in
//! every fight of a trial, against the same groups, to tell what that yardstick is worth.
//! `skirmish` puts one of them against the group of every sector, and tells what each
//! stretch of sectors costs and which tanks the cost comes from. `duels` puts one of them
//! against tanks of one make, with one stat at a time at each of its levels, and tells what
//! a level adds to the danger of a tank next to what it adds to its price. `classes` puts
//! one of them against the tanks of each class alone, made as the game makes them with a
//! budget that grows. `runs` plays whole runs, with their lives, their checkpoints and their
//! arenas, and tells how far each player gets. `journal` sets the runs that a person played
//! next to those players. `breed` breeds the tank that costs a player the most for its
//! points, to show which points are worth more than their price. `compare` tells what
//! changed between two reports of the same trial.
//!
//! ```text
//! cargo run --release -p tanku_trials -- census --sectors 1-30 --seeds 300 --out target/trials
//! cargo run --release -p tanku_trials -- players --sectors 1-30 --seeds 40 --out target/trials
//! cargo run --release -p tanku_trials -- skirmish --sectors 1-30 --seeds 40 --out target/trials
//! cargo run --release -p tanku_trials -- duels --stat muzzle --stat loader --out target/trials
//! cargo run --release -p tanku_trials -- classes --budgets 20,40,60,80,100,120 --out target/trials
//! cargo run --release -p tanku_trials -- runs --seeds 100 --out target/trials
//! cargo run --release -p tanku_trials -- journal --out target/trials
//! cargo run --release -p tanku_trials -- breed --budget 60 --out target/trials
//! cargo run --release -p tanku_trials -- compare before/skirmish.csv after/skirmish.csv
//! ```

// A trial that finds a tank without what every tank has can't go on, and stops where it
// stands: a number made up in its place would be wrong in every report after it.
#![expect(
    clippy::unwrap_used,
    reason = "a trial that can't go on stops: nobody plays it, and its numbers must be right"
)]

mod breed;
mod census;
mod classes;
mod compare;
mod duels;
mod fight;
mod journal;
mod players;
mod report;
mod runs;
mod skirmish;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use tanku_core::{
    modes::Difficulty,
    reference::Reference,
    stats::{Class, Genome, Stat},
};

use crate::report::Report;

#[derive(Parser)]
#[command(
    name = "tanku_trials",
    version,
    about = "Trials of the balance of Tanku"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Counts what the rules make of each stretch of sectors: the groups, their tanks and
    /// the terrain. It fights nothing, and takes seconds.
    Census(CensusArgs),
    /// Compares the reference players: each of them fights the same groups, on the same
    /// seeds. It takes about half a minute on every core of the machine.
    Players(PlayersArgs),
    /// Fights the group of every sector as the game makes it, with one reference player:
    /// what each stretch of sectors costs, and which tanks the cost comes from. It takes
    /// seconds on every core of the machine.
    Skirmish(SkirmishArgs),
    /// Fights tanks of one make, with one stat at a time at each of its levels: what a
    /// level adds to the danger of a tank, next to what it adds to its price. It takes
    /// about a minute on every core of the machine.
    Duels(DuelsArgs),
    /// Fights the tanks of each class alone, made as the game makes them, with a budget
    /// that grows: the curve of the danger of a class over its points. It takes about a
    /// minute on every core of the machine.
    Classes(ClassesArgs),
    /// Plays whole runs of a single player, with the lives, the checkpoints and the arenas
    /// of the game: how far each reference player gets, where the tanks are lost and to
    /// what. It takes minutes on every core of the machine.
    Runs(RunsArgs),
    /// Sets the journal of the runs that were played next to the reference players: where
    /// a person loses their tanks and to what, and which player they play as. It takes a
    /// minute or two on every core of the machine.
    Journal(JournalArgs),
    /// Breeds the tank that costs a reference player the most for its points: generations
    /// of tanks that fight, the best of which have children. It tells the best makes next
    /// to the tanks that the game makes with the same points, and where their points go.
    /// It takes minutes on every core of the machine.
    Breed(BreedArgs),
    /// Tells what changed between two reports of the same trial, from their CSV files.
    Compare(CompareArgs),
}

#[derive(Args)]
struct CensusArgs {
    /// The sectors to count, as FIRST-LAST.
    #[arg(long, default_value = "1-50", value_parser = sectors)]
    sectors: (u32, u32),
    /// How many seeds to count them on: CENSUS0, CENSUS1 and so on, which the game takes too.
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u32).range(1..))]
    seeds: u32,
    /// The number of players that the groups are made for.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=4))]
    players: u32,
    /// The difficulty: normal or hard.
    #[arg(long, default_value = "normal", value_parser = difficulty)]
    difficulty: Difficulty,
    /// One row for each sector, in place of one for each stretch of sectors.
    #[arg(long)]
    each: bool,
    /// A directory to write the report to, as census.md and census.csv. It is printed
    /// either way.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct PlayersArgs {
    /// The sectors whose groups are fought, as FIRST-LAST. Only corridors hold a fight.
    #[arg(long, default_value = "1-30", value_parser = sectors)]
    sectors: (u32, u32),
    /// How many seeds to fight them on: TRIAL0, TRIAL1 and so on, which the game takes too.
    #[arg(long, default_value_t = 40, value_parser = clap::value_parser!(u32).range(1..))]
    seeds: u32,
    /// The difficulty: normal or hard.
    #[arg(long, default_value = "normal", value_parser = difficulty)]
    difficulty: Difficulty,
    /// The players to compare: novice, regular and expert, all three unless some are named.
    #[arg(long = "player", value_parser = player)]
    players: Vec<Reference>,
    /// How long a fight lasts at most, in seconds.
    #[arg(long, default_value_t = 60.0)]
    limit: f32,
    /// How many fights run side by side: as many as the machine has cores, unless told.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    threads: Option<u32>,
    /// A directory to write the report to, as players.md and players.csv. It is printed
    /// either way.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct SkirmishArgs {
    /// The sectors whose groups are fought, as FIRST-LAST. Only corridors hold a fight.
    #[arg(long, default_value = "1-30", value_parser = sectors)]
    sectors: (u32, u32),
    /// How many seeds to fight them on: TRIAL0, TRIAL1 and so on, which the game takes too.
    #[arg(long, default_value_t = 40, value_parser = clap::value_parser!(u32).range(1..))]
    seeds: u32,
    /// The difficulty: normal or hard.
    #[arg(long, default_value = "normal", value_parser = difficulty)]
    difficulty: Difficulty,
    /// The player who fights: novice, regular or expert.
    #[arg(long, default_value = "regular", value_parser = player)]
    player: Reference,
    /// How long a fight lasts at most, in seconds.
    #[arg(long, default_value_t = 60.0)]
    limit: f32,
    /// One row for each sector, in place of one for each stretch of sectors.
    #[arg(long)]
    each: bool,
    /// How many fights run side by side: as many as the machine has cores, unless told.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    threads: Option<u32>,
    /// A directory to write the report to, as skirmish.md and skirmish.csv. It is printed
    /// either way.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct DuelsArgs {
    /// The tank that every duel starts from, as its stats with their levels. A stat that
    /// isn't named is at level 0.
    #[arg(long, default_value = duels::BASE, value_parser = duels::tank)]
    tank: Genome,
    /// A stat to try at each of its levels. It can be given several times, and every stat
    /// is tried without it.
    #[arg(long = "stat", value_parser = duels::stat)]
    stats: Vec<Stat>,
    /// The sectors that the duels are fought in, as FIRST-LAST: their corridors, and the
    /// experience of their brains.
    #[arg(long, default_value = "11-14", value_parser = sectors)]
    sectors: (u32, u32),
    /// How many seeds to fight on: TRIAL0, TRIAL1 and so on.
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u32).range(1..))]
    seeds: u32,
    /// How many tanks of the make the player fights at once. One alone falls too fast to
    /// tell much.
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..=6))]
    count: u32,
    /// The difficulty: normal or hard. It sets the experience of the brains.
    #[arg(long, default_value = "normal", value_parser = difficulty)]
    difficulty: Difficulty,
    /// The player who fights: novice, regular or expert.
    #[arg(long, default_value = "regular", value_parser = player)]
    player: Reference,
    /// How long a duel lasts at most, in seconds.
    #[arg(long, default_value_t = 40.0)]
    limit: f32,
    /// How many duels run side by side: as many as the machine has cores, unless told.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    threads: Option<u32>,
    /// A directory to write the report to, as duels.md and duels.csv. It is printed either
    /// way.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct ClassesArgs {
    /// The budgets that the tanks are made with, each in turn.
    #[arg(long, default_value = "20,40,60,80,100,120", value_parser = classes::budgets)]
    budgets: std::vec::Vec<u32>,
    /// The sectors that the tanks are made for and fought in, as FIRST-LAST: their caps on
    /// software and on the gun, their corridors and the experience of their brains.
    #[arg(long, default_value = "21-30", value_parser = sectors)]
    sectors: (u32, u32),
    /// How many seeds to fight on: TRIAL0, TRIAL1 and so on.
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..))]
    seeds: u32,
    /// How many tanks of the class the player fights at once, each made apart.
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..=6))]
    count: u32,
    /// The difficulty: normal or hard.
    #[arg(long, default_value = "normal", value_parser = difficulty)]
    difficulty: Difficulty,
    /// The player who fights: novice, regular or expert.
    #[arg(long, default_value = "regular", value_parser = player)]
    player: Reference,
    /// How long a fight lasts at most, in seconds.
    #[arg(long, default_value_t = 40.0)]
    limit: f32,
    /// How many fights run side by side: as many as the machine has cores, unless told.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    threads: Option<u32>,
    /// A directory to write the report to, as classes.md and classes.csv. It is printed
    /// either way.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct RunsArgs {
    /// How many seeds to play: TRIAL0, TRIAL1 and so on, which the game takes too.
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..))]
    seeds: u32,
    /// The difficulty: normal or hard.
    #[arg(long, default_value = "normal", value_parser = difficulty)]
    difficulty: Difficulty,
    /// A player to play them: novice, regular or expert. It can be given several times,
    /// and all three play without it.
    #[arg(long = "player", value_parser = player)]
    players: Vec<Reference>,
    /// The sector at which a run is stopped if it gets there.
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u32).range(1..=500))]
    last: u32,
    /// How long a player may stay in a sector without clearing it or falling, in seconds.
    /// A run that does is stopped.
    #[arg(long, default_value_t = 180.0)]
    patience: f32,
    /// How many runs go side by side: as many as the machine has cores, unless told.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    threads: Option<u32>,
    /// A directory to write the report to, as runs.md and runs.csv. It is printed either
    /// way.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct JournalArgs {
    /// The file of the journal. It is the player's own without it, where the game keeps it.
    file: Option<PathBuf>,
    /// How many seeds each reference player plays: TRIAL0, TRIAL1 and so on.
    #[arg(long, default_value_t = 40, value_parser = clap::value_parser!(u32).range(1..))]
    seeds: u32,
    /// The difficulty of the runs to count: normal or hard.
    #[arg(long, default_value = "normal", value_parser = difficulty)]
    difficulty: Difficulty,
    /// Count the runs that started at this moment or later, as the journal writes one:
    /// 2026-10-13, or 2026-10-13T17:46Z.
    #[arg(long)]
    since: Option<String>,
    /// Count the runs of every version of the game and of every balance, not only those
    /// of the version and of the balance that this tool was compiled with.
    #[arg(long)]
    every_build: bool,
    /// The furthest sector that the reference players fight in.
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u32).range(1..=500))]
    last: u32,
    /// How long a fight of a reference player for a sector lasts at most, in seconds.
    #[arg(long, default_value_t = 60.0)]
    limit: f32,
    /// How long a reference player may stay in a sector of a whole run without clearing it
    /// or falling, in seconds.
    #[arg(long, default_value_t = 180.0)]
    patience: f32,
    /// How many runs go side by side: as many as the machine has cores, unless told.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    threads: Option<u32>,
    /// A directory to write the report to, as journal.md and journal.csv. It is printed
    /// either way.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct BreedArgs {
    /// The points of every tank of the trial.
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u32).range(1..=1000))]
    budget: u32,
    /// The sectors that the tanks fight in, as FIRST-LAST, with their corridors and the
    /// experience of their brains. The tanks are bred under the caps of the last one.
    #[arg(long, default_value = "21-30", value_parser = sectors)]
    sectors: (u32, u32),
    /// How many tanks of one make the player fights at once.
    #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..=6))]
    count: u32,
    /// The difficulty: normal or hard.
    #[arg(long, default_value = "normal", value_parser = difficulty)]
    difficulty: Difficulty,
    /// The player who fights: novice, regular or expert.
    #[arg(long, default_value = "regular", value_parser = player)]
    player: Reference,
    /// The one class that the makes are bred of, as the balance names it: all-rounder,
    /// sniper and so on. Without it, they are bred of every class of the sector, and a
    /// child may take another class than its parents.
    #[arg(long, value_parser = class)]
    class: Option<Class>,
    /// How long a fight lasts at most, in seconds.
    #[arg(long, default_value_t = 40.0)]
    limit: f32,
    /// How many makes a generation holds. The best quarter has the children of the next.
    #[arg(long, default_value_t = 32, value_parser = clap::value_parser!(u32).range(4..=500))]
    population: u32,
    /// How many generations fight, the first one included, which the game makes.
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=500))]
    generations: u32,
    /// How many fights each make fights in each generation that it is in.
    #[arg(long, default_value_t = 24, value_parser = clap::value_parser!(u32).range(1..=1000))]
    fights: u32,
    /// How many fights each of the best makes fights at the end, and each class of the
    /// game with it.
    #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u32).range(2..=5000))]
    finals: u32,
    /// How many of the best makes are fought again at the end, and told.
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=100))]
    best: u32,
    /// The text that decides what is drawn: the first tanks, the parents of each child and
    /// what changes in it. Another text breeds other tanks.
    #[arg(long, default_value = "BREED")]
    seed: String,
    /// How many fights run side by side: as many as the machine has cores, unless told.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    threads: Option<u32>,
    /// A directory to write the report to, as breed.md and breed.csv. It is printed either
    /// way.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Args)]
struct CompareArgs {
    /// The CSV file of the report from before the change.
    before: PathBuf,
    /// The CSV file of the report of the same trial from after it.
    after: PathBuf,
    /// The least change that is told, in percent of the value before.
    #[arg(long, default_value_t = 10.0)]
    least: f64,
    /// A table to compare. It can be given several times, and every table is compared
    /// without it.
    #[arg(long = "table")]
    tables: Vec<String>,
    /// A directory to write the comparison to, as compare.md and compare.csv. It is printed
    /// either way.
    #[arg(long)]
    out: Option<PathBuf>,
}

/// Reads a stretch of sectors as the command line writes it: `6-10`, or `6` for one sector.
fn sectors(text: &str) -> Result<(u32, u32), String> {
    let number = |text: &str| {
        let sector = text.trim().parse::<u32>();
        sector.map_err(|_| format!("`{text}` is not the number of a sector"))
    };
    let (first, last) = match text.split_once('-') {
        Some((first, last)) => (number(first)?, number(last)?),
        None => (number(text)?, number(text)?),
    };
    if first == 0 || last < first {
        return Err("sectors start at 1, and the first one comes first".to_string());
    }
    Ok((first, last))
}

fn difficulty(text: &str) -> Result<Difficulty, String> {
    Difficulty::of_key(&text.to_lowercase()).ok_or_else(|| format!("`{text}` is no difficulty"))
}

fn class(text: &str) -> Result<Class, String> {
    let named = |class: &Class| class.name() == text.trim().to_lowercase();
    let class = Class::ALL.into_iter().find(named);
    class.ok_or_else(|| format!("`{text}` is no class"))
}

fn player(text: &str) -> Result<Reference, String> {
    Reference::of_name(&text.to_lowercase())
        .ok_or_else(|| format!("`{text}` is no reference player"))
}

/// Prints a report, and writes it to a directory under a name if one is given.
fn tell(report: &Report, out: Option<PathBuf>, name: &str) {
    print!("{}", report.markdown());
    if let Some(directory) = out {
        match report.write(&directory, name) {
            Ok(()) => eprintln!("Written to {}.", directory.display()),
            Err(error) => {
                eprintln!("The report was not written: {error}");
                std::process::exit(1);
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let threads = |asked: Option<u32>| asked.map_or(cores, |threads| threads as usize);
    match cli.command {
        Command::Census(args) => {
            let asked = census::Asked {
                sectors: args.sectors,
                seeds: args.seeds,
                players: args.players,
                difficulty: args.difficulty,
                each: args.each,
            };
            tell(&census::census(&asked), args.out, "census");
        }
        Command::Players(args) => {
            let asked = players::Asked {
                sectors: args.sectors,
                seeds: args.seeds,
                difficulty: args.difficulty,
                players: match args.players.is_empty() {
                    true => Reference::ALL.to_vec(),
                    false => args.players,
                },
                limit: args.limit,
                threads: threads(args.threads),
            };
            tell(&players::players(&asked), args.out, "players");
        }
        Command::Skirmish(args) => {
            let asked = skirmish::Asked {
                sectors: args.sectors,
                seeds: args.seeds,
                difficulty: args.difficulty,
                player: args.player,
                limit: args.limit,
                each: args.each,
                threads: threads(args.threads),
            };
            tell(&skirmish::skirmish(&asked), args.out, "skirmish");
        }
        Command::Duels(args) => {
            let asked = duels::Asked {
                base: args.tank,
                stats: args.stats,
                sectors: args.sectors,
                seeds: args.seeds,
                count: args.count as usize,
                difficulty: args.difficulty,
                player: args.player,
                limit: args.limit,
                threads: threads(args.threads),
            };
            tell(&duels::duels(&asked), args.out, "duels");
        }
        Command::Classes(args) => {
            let asked = classes::Asked {
                budgets: args.budgets,
                sectors: args.sectors,
                seeds: args.seeds,
                count: args.count as usize,
                difficulty: args.difficulty,
                player: args.player,
                limit: args.limit,
                threads: threads(args.threads),
            };
            tell(&classes::classes(&asked), args.out, "classes");
        }
        Command::Runs(args) => {
            let asked = runs::Asked {
                seeds: args.seeds,
                difficulty: args.difficulty,
                players: match args.players.is_empty() {
                    true => Reference::ALL.to_vec(),
                    false => args.players,
                },
                last: args.last,
                patience: args.patience,
                threads: threads(args.threads),
            };
            tell(&runs::runs(&asked), args.out, "runs");
        }
        Command::Journal(args) => {
            let own = tanku_core::files::directory().map(|own| own.join(tanku_core::journal::FILE));
            let Some(file) = args.file.or(own) else {
                eprintln!("This machine has no place for the files of the game: name the journal.");
                std::process::exit(1);
            };
            let text = match std::fs::read_to_string(&file) {
                Ok(text) => text,
                Err(error) => {
                    eprintln!("The journal {} was not read: {error}", file.display());
                    std::process::exit(1);
                }
            };
            let asked = journal::Asked {
                file,
                difficulty: args.difficulty,
                seeds: args.seeds,
                since: args.since,
                every_build: args.every_build,
                last: args.last,
                limit: args.limit,
                patience: args.patience,
                threads: threads(args.threads),
            };
            tell(&journal::journal(&asked, &text), args.out, "journal");
        }
        Command::Breed(args) => {
            let asked = breed::Asked {
                budget: args.budget,
                sectors: args.sectors,
                count: args.count as usize,
                difficulty: args.difficulty,
                player: args.player,
                class: args.class,
                limit: args.limit,
                population: args.population,
                generations: args.generations,
                fights: args.fights,
                finals: args.finals,
                best: args.best,
                seed: args.seed,
                threads: threads(args.threads),
            };
            tell(&breed::breed(&asked), args.out, "breed");
        }
        Command::Compare(args) => {
            let asked = compare::Asked {
                least: args.least,
                tables: args.tables,
            };
            match compare::compare(&args.before, &args.after, &asked) {
                Ok(report) => tell(&report, args.out, "compare"),
                Err(error) => {
                    eprintln!("The reports were not compared: {error}");
                    std::process::exit(1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn the_command_line_is_sound() {
        Cli::command().debug_assert();
    }

    #[test]
    fn a_stretch_of_sectors_is_read_as_it_is_written() {
        assert_eq!(sectors("6-10"), Ok((6, 10)));
        assert_eq!(sectors("7"), Ok((7, 7)));
        assert_eq!(sectors(" 1 - 50 "), Ok((1, 50)));
        assert!(sectors("10-6").is_err());
        assert!(sectors("0-3").is_err());
        assert!(sectors("a-3").is_err());
        assert_eq!(difficulty("Hard"), Ok(Difficulty::Hard));
        assert!(difficulty("easy").is_err());
    }

    #[test]
    fn the_census_takes_its_options() {
        let line = [
            "tanku_trials",
            "census",
            "--sectors",
            "3-9",
            "--players",
            "4",
            "--each",
        ];
        let Command::Census(args) = Cli::try_parse_from(line).unwrap().command else {
            panic!("not a census");
        };
        assert_eq!((args.sectors, args.players, args.each), ((3, 9), 4, true));
        assert_eq!((args.seeds, args.difficulty), (200, Difficulty::Normal));
        // No game has five players.
        let five = ["tanku_trials", "census", "--players", "5"];
        assert!(Cli::try_parse_from(five).is_err());
    }

    #[test]
    fn the_players_are_all_compared_unless_some_are_named() {
        let line = [
            "tanku_trials",
            "players",
            "--player",
            "expert",
            "--seeds",
            "2",
        ];
        let Command::Players(args) = Cli::try_parse_from(line).unwrap().command else {
            panic!("not a comparison of the players");
        };
        assert_eq!(args.players, vec![Reference::Expert]);
        assert_eq!((args.sectors, args.seeds, args.limit), ((1, 30), 2, 60.0));
        let nobody = ["tanku_trials", "players", "--player", "master"];
        assert!(Cli::try_parse_from(nobody).is_err());
    }

    #[test]
    fn a_skirmish_is_fought_by_the_regular_player_unless_another_is_named() {
        let Command::Skirmish(args) = Cli::try_parse_from(["tanku_trials", "skirmish"])
            .unwrap()
            .command
        else {
            panic!("not a skirmish");
        };
        assert_eq!(
            (args.player, args.seeds, args.each),
            (Reference::Regular, 40, false)
        );
        let line = [
            "tanku_trials",
            "skirmish",
            "--player",
            "novice",
            "--difficulty",
            "hard",
        ];
        let Command::Skirmish(args) = Cli::try_parse_from(line).unwrap().command else {
            panic!("not a skirmish");
        };
        assert_eq!(
            (args.player, args.difficulty),
            (Reference::Novice, Difficulty::Hard)
        );
    }

    #[test]
    fn the_duels_take_a_tank_and_the_stats_to_try() {
        let line = [
            "tanku_trials",
            "duels",
            "--tank",
            "engine=2,brain=1",
            "--stat",
            "muzzle",
            "--stat",
            "Loader",
            "--count",
            "2",
        ];
        let Command::Duels(args) = Cli::try_parse_from(line).unwrap().command else {
            panic!("no duels");
        };
        assert_eq!(duels::written(&args.tank), "engine=2,brain=1");
        assert_eq!(
            (args.stats, args.count),
            (vec![Stat::Muzzle, Stat::Loader], 2)
        );
        let Command::Duels(args) = Cli::try_parse_from(["tanku_trials", "duels"])
            .unwrap()
            .command
        else {
            panic!("no duels");
        };
        assert_eq!(duels::written(&args.tank), duels::BASE);
        assert!(Cli::try_parse_from(["tanku_trials", "duels", "--stat", "wheels"]).is_err());
    }

    #[test]
    fn the_classes_take_their_budgets_and_a_comparison_takes_two_reports() {
        let line = ["tanku_trials", "classes", "--budgets", "30,90"];
        let Command::Classes(args) = Cli::try_parse_from(line).unwrap().command else {
            panic!("no classes");
        };
        assert_eq!(
            (args.budgets, args.sectors, args.count),
            (vec![30, 90], (21, 30), 3)
        );
        let line = [
            "tanku_trials",
            "compare",
            "a.csv",
            "b.csv",
            "--table",
            "Sectors",
        ];
        let Command::Compare(args) = Cli::try_parse_from(line).unwrap().command else {
            panic!("no comparison");
        };
        assert_eq!(
            (args.least, args.tables),
            (10.0, vec!["Sectors".to_string()])
        );
        assert!(Cli::try_parse_from(["tanku_trials", "compare", "a.csv"]).is_err());
    }

    #[test]
    fn the_runs_are_played_by_every_player_unless_some_are_named() {
        let Command::Runs(args) = Cli::try_parse_from(["tanku_trials", "runs"])
            .unwrap()
            .command
        else {
            panic!("no runs");
        };
        assert!(args.players.is_empty());
        assert_eq!((args.seeds, args.last, args.patience), (100, 50, 180.0));
        let line = ["tanku_trials", "runs", "--player", "regular", "--last", "0"];
        assert!(Cli::try_parse_from(line).is_err());
    }
}
