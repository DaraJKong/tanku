//! The trials of the balance of the game: what its rules make, counted and measured without
//! anyone at the keyboard, and told in reports that can be compared from one change to the
//! next.
//!
//! `census` is the first of them. It fights nothing: it counts what the rules produce for
//! each stretch of sectors, on many seeds.
//!
//! ```text
//! cargo run --release -p tanku_trials -- census --sectors 1-30 --seeds 300 --out target/trials
//! ```

mod census;
mod report;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use tanku_core::modes::Difficulty;

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

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Census(args) => {
            let asked = census::Asked {
                sectors: args.sectors,
                seeds: args.seeds,
                players: args.players,
                difficulty: args.difficulty,
                each: args.each,
            };
            let report = census::census(&asked);
            print!("{}", report.markdown());
            if let Some(directory) = args.out {
                match report.write(&directory, "census") {
                    Ok(()) => eprintln!("Written to {}.", directory.display()),
                    Err(error) => {
                        eprintln!("The report was not written: {error}");
                        std::process::exit(1);
                    }
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
        let Command::Census(args) = Cli::try_parse_from(line).unwrap().command;
        assert_eq!((args.sectors, args.players, args.each), ((3, 9), 4, true));
        assert_eq!((args.seeds, args.difficulty), (200, Difficulty::Normal));
        // No game has five players.
        let five = ["tanku_trials", "census", "--players", "5"];
        assert!(Cli::try_parse_from(five).is_err());
    }
}
