//! The command line of the game.

use bevy::prelude::Resource;

const USAGE: &str = "\
Usage: tanku_client [OPTIONS]

Options:
      --playtest   Start a playtest, which lets the player cheat: the lives are infinite and the
                   pause menu skips to any sector. A playtest sets no record.
      --seed TEXT  With --playtest: start every run on this seed instead of a random one.
  -h, --help       Show this help.";

/// What the command line asks for.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Start a playtest, which lets the player cheat.
    pub playtest: bool,
    /// The seed of every run of a playtest, as runs show it.
    pub seed: Option<String>,
}

/// Why the game doesn't start.
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Help,
    Unknown(String),
    /// `--seed` came without a seed.
    NoSeed,
    /// `--seed` came without `--playtest`.
    NoPlaytest,
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
                "-h" | "--help" => return Err(Refusal::Help),
                _ => return Err(Refusal::Unknown(argument)),
            }
        }
        // Choosing the seed is a mode that players unlock. Only a playtest gets it for free.
        if options.seed.is_some() && !options.playtest {
            return Err(Refusal::NoPlaytest);
        }
        Ok(options)
    }

    /// Reads the command line. Asking for help, or for something the game can't do, shows the
    /// options and ends the program.
    pub fn from_command_line() -> Self {
        let mistake = match Self::parse(std::env::args().skip(1)) {
            Ok(options) => return options,
            Err(Refusal::Help) => {
                println!("{USAGE}");
                std::process::exit(0)
            }
            Err(Refusal::Unknown(argument)) => format!("Unknown option: {argument}"),
            Err(Refusal::NoSeed) => "The option --seed needs a seed after it.".to_string(),
            Err(Refusal::NoPlaytest) => "The option --seed needs --playtest.".to_string(),
        };
        eprintln!("{mistake}\n\n{USAGE}");
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
        assert_eq!((playtest.playtest, playtest.seed), (true, None));
    }

    #[test]
    fn a_playtest_can_choose_its_seed() {
        let wanted = Options {
            playtest: true,
            seed: Some("85GVTHQA".into()),
        };
        assert_eq!(parse("--playtest --seed 85gvthqa"), Ok(wanted.clone()));
        assert_eq!(parse("--seed 85GVTHQA --playtest"), Ok(wanted));
        // The last seed wins.
        let last = parse("--playtest --seed FIRST --seed LAST").unwrap();
        assert_eq!(last.seed.as_deref(), Some("LAST"));

        assert_eq!(parse("--seed 85GVTHQA"), Err(Refusal::NoPlaytest));
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
