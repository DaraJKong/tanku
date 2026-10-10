//! The command line of the game.

const USAGE: &str = "\
Usage: tanku_client [OPTIONS]

Options:
      --playtest  Start a playtest, which lets the player cheat: the lives are infinite and the
                  pause menu skips to any sector. A playtest sets no record.
  -h, --help      Show this help.";

/// What the command line asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Start a playtest, which lets the player cheat.
    pub playtest: bool,
}

/// Why the game doesn't start.
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Help,
    Unknown(String),
}

impl Options {
    fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self, Refusal> {
        let mut options = Self::default();
        for argument in arguments {
            match argument.as_str() {
                "--playtest" => options.playtest = true,
                "-h" | "--help" => return Err(Refusal::Help),
                _ => return Err(Refusal::Unknown(argument)),
            }
        }
        Ok(options)
    }

    /// Reads the command line. Asking for help, or for something the game doesn't know, shows
    /// the options and ends the program.
    pub fn from_command_line() -> Self {
        match Self::parse(std::env::args().skip(1)) {
            Ok(options) => options,
            Err(Refusal::Help) => {
                println!("{USAGE}");
                std::process::exit(0)
            }
            Err(Refusal::Unknown(argument)) => {
                eprintln!("Unknown option: {argument}\n\n{USAGE}");
                std::process::exit(2)
            }
        }
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
        assert_eq!(parse(""), Ok(Options { playtest: false }));
        assert_eq!(parse("--playtest"), Ok(Options { playtest: true }));
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
    }
}
