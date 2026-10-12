//! The command line of the game.

use bevy::prelude::Resource;
use tanku_core::stats::{Genome, PLAYER_BUDGET, Stat};

const USAGE: &str = "\
Usage: tanku_client [OPTIONS]

Options:
      --playtest   Start a playtest, which lets the player cheat: the lives are infinite and the
                   pause menu skips to any sector. A playtest sets no record.
      --seed TEXT  With --playtest: start every run on this seed instead of a random one.
      --tank TANK  With --playtest: play in a tank of your own instead of the standard one.
  -h, --help       Show this help.";

/// A tank as the command line takes it.
const A_TANK: &str = "engine=4,tracks=5,muzzle=7,magazine=4,loader=6";

/// How a tank is written on the command line, and what the levels of its stats cost.
fn tanks_explained() -> String {
    let mut told = format!(
        "A tank is written as its stats with their levels, such as {A_TANK}.\n\
         A stat that isn't named is at level 0. A tank needs an engine and has \
         {PLAYER_BUDGET} points.\n\
         What each level costs, from level 0:\n"
    );
    for stat in Stat::GARAGE {
        let prices: Vec<String> = (0..=stat.max_level())
            .map(|level| stat.price(level).to_string())
            .collect();
        told.push_str(&format!("  {:<9} {}\n", stat.name(), prices.join(" ")));
    }
    told.trim_end().to_string()
}

/// Reads a tank as the command line writes it.
fn tank(text: &str) -> Result<Genome, String> {
    let mut levels: Vec<(Stat, u8)> = Vec::new();
    let parts = text.split(',').map(str::trim);
    for part in parts.filter(|part| !part.is_empty()) {
        let Some((name, level)) = part.split_once('=') else {
            return Err(format!("`{part}` is not a stat with its level"));
        };
        let name = name.trim().to_lowercase();
        let mut every = Stat::HARDWARE
            .into_iter()
            .chain([Stat::Brain, Stat::Ballistics]);
        let Some(stat) = every.find(|stat| stat.name() == name) else {
            return Err(format!("`{name}` is not a stat"));
        };
        let Ok(level) = level.trim().parse::<u8>() else {
            return Err(format!("`{}` is not a level", level.trim()));
        };
        if levels.iter().any(|(named, _)| *named == stat) {
            return Err(format!("{name} is named twice"));
        }
        levels.push((stat, level));
    }
    Genome::of_the_garage(&levels).map_err(|flaw| flaw.to_string())
}

/// What the command line asks for.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Start a playtest, which lets the player cheat.
    pub playtest: bool,
    /// The seed of every run of a playtest, as runs show it.
    pub seed: Option<String>,
    /// The tank of the player in every run of a playtest, in place of the standard one.
    pub tank: Option<Genome>,
}

/// Why the game doesn't start.
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Help,
    Unknown(String),
    /// `--seed` came without a seed.
    NoSeed,
    /// `--tank` came without a tank.
    NoTank,
    /// The tank that `--tank` asks for is none that a player may build: why.
    BadTank(String),
    /// An option that is a playtest's came without `--playtest`.
    NoPlaytest(&'static str),
}

impl Options {
    fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, Refusal> {
        let mut options = Self::default();
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--playtest" => options.playtest = true,
                "--seed" => {
                    // Runs show their seed in capitals and without the spaces around it.
                    let seed = arguments.next().unwrap_or_default().trim().to_uppercase();
                    // An option where the seed should be means that the seed was forgotten.
                    if seed.is_empty() || seed.starts_with('-') {
                        return Err(Refusal::NoSeed);
                    }
                    options.seed = Some(seed);
                }
                "--tank" => {
                    let written = arguments.next().unwrap_or_default();
                    if written.trim().is_empty() || written.starts_with('-') {
                        return Err(Refusal::NoTank);
                    }
                    options.tank = Some(tank(&written).map_err(Refusal::BadTank)?);
                }
                "-h" | "--help" => return Err(Refusal::Help),
                _ => return Err(Refusal::Unknown(argument)),
            }
        }
        // Choosing the seed and building a tank are what players unlock. Only a playtest
        // gets them for free.
        if options.seed.is_some() && !options.playtest {
            return Err(Refusal::NoPlaytest("--seed"));
        }
        if options.tank.is_some() && !options.playtest {
            return Err(Refusal::NoPlaytest("--tank"));
        }
        Ok(options)
    }

    /// The tanks that a run of this game starts with, from the first seat: the one of the
    /// command line, which is the player's own, and their host's among several players.
    pub fn tanks(&self) -> Vec<Genome> {
        self.tank.into_iter().collect()
    }

    /// Reads the command line. Asking for help, or for something the game can't do, shows the
    /// options and ends the program.
    pub fn from_command_line() -> Self {
        let mistake = match Self::parse(std::env::args().skip(1)) {
            Ok(options) => return options,
            Err(Refusal::Help) => {
                println!("{USAGE}\n\n{}", tanks_explained());
                std::process::exit(0)
            }
            Err(Refusal::Unknown(argument)) => format!("Unknown option: {argument}"),
            Err(Refusal::NoSeed) => "The option --seed needs a seed after it.".to_string(),
            Err(Refusal::NoTank) => "The option --tank needs a tank after it.".to_string(),
            Err(Refusal::BadTank(why)) => format!("No tank can be built as --tank asks: {why}."),
            Err(Refusal::NoPlaytest(option)) => format!("The option {option} needs --playtest."),
        };
        eprintln!("{mistake}\n\n{USAGE}\n\n{}", tanks_explained());
        std::process::exit(2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<Options, Refusal> {
        Options::parse(line.split_whitespace().map(String::from))
    }

    #[test]
    fn a_playtest_is_asked_for_by_name() {
        assert_eq!(parse(""), Ok(Options::default()));
        assert!(!Options::default().playtest);
        let playtest = parse("--playtest").unwrap();
        assert_eq!((playtest.tank, playtest.tanks()), (None, Vec::new()));
        assert_eq!((playtest.playtest, playtest.seed), (true, None));
    }

    #[test]
    fn a_playtest_can_bring_a_tank_of_its_own() {
        let options = parse("--playtest --tank engine=4,tracks=5,muzzle=7,magazine=4,loader=6");
        let tank = options.unwrap().tank.unwrap();
        assert_eq!(super::tank(A_TANK), Ok(tank));
        assert_eq!((tank.top_speed(), tank.bullet_speed()), (5.0, 19.0));
        assert_eq!((tank.magazine(), tank.reload_time()), (5, 1.0));
        // What it doesn't name, it doesn't have.
        assert_eq!(
            (tank.bounces(), tank.mines(), tank.shield_cooldown()),
            (0, 0, None)
        );
        assert_eq!((tank.cost(), tank.budget), (58, PLAYER_BUDGET));
        // It is the tank of the first seat of every run.
        let options = parse(&format!("--tank {A_TANK} --playtest --seed TANKU")).unwrap();
        assert_eq!(options.tanks(), [tank]);
        assert_eq!(options.seed.as_deref(), Some("TANKU"));
        // Capitals and spaces do no harm, and the standard tank is a tank like another.
        let standard = "Engine=4, Tracks=5, Muzzle=2, Magazine=4, Loader=6, Ricochet=1, \
                        Mines=2, Shield = 2";
        assert_eq!(super::tank(standard), Ok(Genome::PLAYER));
        let spaced = ["--playtest", "--tank", standard].map(String::from);
        assert_eq!(Options::parse(spaced).unwrap().tank, Some(Genome::PLAYER));
    }

    #[test]
    fn a_tank_that_no_player_may_build_stops_the_game_from_starting() {
        let refused = |written: &str| match parse(&format!("--playtest --tank {written}")) {
            Err(Refusal::BadTank(why)) => why,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            refused("muzzle=7"),
            "it needs engine 1 at least, to reach the barriers"
        );
        assert_eq!(
            refused("engine=7,muzzle=7,loader=7,magazine=2"),
            "it costs 60 points, and a player has 59"
        );
        assert_eq!(
            refused("engine=1,turret=3"),
            "the tank of a player has no turret"
        );
        assert_eq!(
            refused("engine=1,brain=4"),
            "the tank of a player has no brain"
        );
        assert_eq!(refused("engine=9"), "engine goes up to 7");
        assert_eq!(refused("engine=1,armor=2"), "`armor` is not a stat");
        assert_eq!(refused("engine=fast"), "`fast` is not a level");
        assert_eq!(refused("engine=1,engine=2"), "engine is named twice");
        assert_eq!(refused("engine"), "`engine` is not a stat with its level");
        // Building a tank is what players unlock: only a playtest gets it for free.
        assert_eq!(parse("--tank engine=1"), Err(Refusal::NoPlaytest("--tank")));
        assert_eq!(parse("--playtest --tank"), Err(Refusal::NoTank));
        assert_eq!(parse("--tank --playtest"), Err(Refusal::NoTank));
    }

    #[test]
    fn the_help_says_what_the_levels_of_a_tank_cost() {
        let told = tanks_explained();
        assert!(told.contains(A_TANK));
        assert!(told.contains("  engine    0 2 4 6 8 11 14 17"));
        assert!(told.contains("  piercing  0 4"));
        assert!(!told.contains("turret") && !told.contains("brain"));
        assert_eq!(told.lines().count(), 3 + Stat::GARAGE.len());
    }

    #[test]
    fn a_playtest_can_choose_its_seed() {
        let wanted = Options {
            playtest: true,
            seed: Some("85GVTHQA".into()),
            tank: None,
        };
        assert_eq!(parse("--playtest --seed 85gvthqa"), Ok(wanted.clone()));
        assert_eq!(parse("--seed 85GVTHQA --playtest"), Ok(wanted));
        // The last seed wins.
        let last = parse("--playtest --seed FIRST --seed LAST").unwrap();
        assert_eq!(last.seed.as_deref(), Some("LAST"));

        assert_eq!(parse("--seed 85GVTHQA"), Err(Refusal::NoPlaytest("--seed")));
        assert_eq!(parse("--playtest --seed"), Err(Refusal::NoSeed));
        assert_eq!(parse("--seed --playtest"), Err(Refusal::NoSeed));
        let blank = ["--playtest", "--seed", "  "].map(String::from);
        assert_eq!(Options::parse(blank), Err(Refusal::NoSeed));
    }

    #[test]
    fn help_and_unknown_options_stop_the_game_from_starting() {
        assert_eq!(parse("--help"), Err(Refusal::Help));
        assert_eq!(parse("--playtest -h"), Err(Refusal::Help));
        assert_eq!(
            parse("--playtset"),
            Err(Refusal::Unknown("--playtset".into()))
        );
        assert_eq!(parse("12"), Err(Refusal::Unknown("12".into())));
        assert_eq!(
            parse("--seed=ABC"),
            Err(Refusal::Unknown("--seed=ABC".into()))
        );
    }
}
