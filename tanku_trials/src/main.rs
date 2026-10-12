//! The trials of the balance of the game: what its rules make, counted and measured without
//! anyone at the keyboard, and told in reports that can be compared from one change to the
//! next.
//!
//! `census` fights nothing: it counts what the rules produce for each stretch of sectors, on
//! many seeds. `players` puts the reference players, whose hands hold the players' tank in
//! every fight of a trial, against the same groups, to tell what that yardstick is worth.
//! `skirmish` puts one of them against the group of every sector, and tells what each
//! stretch of sectors costs and which tanks the cost comes from.
//!
//! ```text
//! cargo run --release -p tanku_trials -- census --sectors 1-30 --seeds 300 --out target/trials
//! cargo run --release -p tanku_trials -- players --sectors 1-30 --seeds 40 --out target/trials
//! cargo run --release -p tanku_trials -- skirmish --sectors 1-30 --seeds 40 --out target/trials
//! ```

mod census;
mod fight;
mod players;
mod reference;
mod report;
mod skirmish;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use tanku_core::modes::Difficulty;

use crate::{reference::Reference, report::Report};

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
}
