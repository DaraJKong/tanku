//! The journal of the runs that are played on a machine: a text file with one line for each
//! run.
//!
//! The game keeps the best run of each league and forgets the others. The journal keeps every
//! run that entered a sector: what was played, how far it got, and where each tank of the
//! players was lost and to what. The machine that runs the simulation of a run writes its
//! line when the run ends. The trials read the file and print it next to their reference
//! players, which tells how a person plays.
//!
//! The file starts with `tanku-journal 1`. A line is `run` followed by pairs, each a key and
//! one word:
//!
//! | Key | Its word |
//! |---|---|
//! | `when` | When the run started, in universal time: `2026-10-13T17:46Z` |
//! | `version`, `balance` | The version of the game, and the fingerprint of its balance |
//! | `seed` | The seed of the run |
//! | `mode` | `normal` or `hard` |
//! | `players` | How many players the run started with |
//! | `options` | `arena`, `free`, `playtest` and `endless` for infinite lives, with commas between them, or `-` |
//! | `entered` | The sectors that the run entered, in the order it entered them: `1-17`, or `1-3,15-22` after a skip |
//! | `arenas` | Those of them that are arenas, or `-` |
//! | `cleared` | The score of the run |
//! | `lost` | Each tank that the players lost, as `sector:weapon:whose`, or `-` |
//! | `fought` | The seconds that the run lasted: `512s` |
//! | `kills` | The enemies destroyed |
//! | `ended` | `lost` when the run was lost, `left` when its players left it |
//!
//! A weapon is `bullet`, `piercing`, `seeking`, `splitting`, `exploding`, `mine`, `crate` or
//! `unknown`. Whose it was is the class of an enemy, `player` or `nobody`.
//!
//! A reader takes the lines that it can read and counts the others: a journal is written by
//! many versions of the game, and nothing else depends on it.

use crate::{
    modes::Difficulty,
    sim::{Blow, Team, Weapon},
    stats::{Ammo, Class},
};

/// The name of the file, in the directory of the player's files.
pub const FILE: &str = "journal.txt";
/// The first line of the file.
pub const HEADING: &str = "tanku-journal 1";

/// What destroyed a tank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cause {
    Bullet(Ammo),
    Mine,
    Crate,
    Unknown,
}

impl Cause {
    pub const ALL: [Cause; 8] = [
        Cause::Bullet(Ammo::Normal),
        Cause::Bullet(Ammo::Piercing),
        Cause::Bullet(Ammo::Seeking),
        Cause::Bullet(Ammo::Splitting),
        Cause::Bullet(Ammo::Exploding),
        Cause::Mine,
        Cause::Crate,
        Cause::Unknown,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Cause::Bullet(Ammo::Normal) => "bullet",
            Cause::Bullet(Ammo::Piercing) => "piercing",
            Cause::Bullet(Ammo::Seeking) => "seeking",
            Cause::Bullet(Ammo::Splitting) => "splitting",
            Cause::Bullet(Ammo::Exploding) => "exploding",
            Cause::Mine => "mine",
            Cause::Crate => "crate",
            Cause::Unknown => "unknown",
        }
    }

    fn named(name: &str) -> Option<Self> {
        Cause::ALL.into_iter().find(|cause| cause.name() == name)
    }
}

/// Whose doing the loss of a tank was.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Culprit {
    /// An enemy of this class fired the bullet, laid the mine or set off the crate.
    Enemy(Class),
    /// A player did: the one whose tank it was, or a teammate.
    Player,
    Nobody,
}

impl Culprit {
    pub const fn name(self) -> &'static str {
        match self {
            Culprit::Enemy(class) => class.name(),
            Culprit::Player => "player",
            Culprit::Nobody => "nobody",
        }
    }

    fn named(name: &str) -> Option<Self> {
        let class = Class::ALL.into_iter().find(|class| class.name() == name);
        match (class, name) {
            (Some(class), _) => Some(Culprit::Enemy(class)),
            (None, "player") => Some(Culprit::Player),
            (None, "nobody") => Some(Culprit::Nobody),
            (None, _) => None,
        }
    }
}

/// A tank that the players lost: where, to what, and whose doing it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Loss {
    pub sector: u32,
    pub cause: Cause,
    pub by: Culprit,
}

impl Loss {
    /// The loss of a tank in a sector, to the blow that the run tells of it.
    pub fn of(sector: u32, blow: Option<Blow>) -> Self {
        let Some(blow) = blow else {
            return Self {
                sector,
                cause: Cause::Unknown,
                by: Culprit::Nobody,
            };
        };
        let cause = match blow.weapon {
            Weapon::Bullet => Cause::Bullet(blow.genome.ammo()),
            Weapon::Mine => Cause::Mine,
            Weapon::Crate => Cause::Crate,
        };
        let by = match blow.team {
            Team::Players => Culprit::Player,
            Team::Enemies => Culprit::Enemy(blow.genome.class),
        };
        Self { sector, cause, by }
    }

    fn word(&self) -> String {
        format!("{}:{}:{}", self.sector, self.cause.name(), self.by.name())
    }

    fn read(word: &str) -> Option<Self> {
        let mut parts = word.split(':');
        let loss = Self {
            sector: parts.next()?.parse().ok()?,
            cause: Cause::named(parts.next()?)?,
            by: Culprit::named(parts.next()?)?,
        };
        parts.next().is_none().then_some(loss)
    }
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum End {
    /// With the last life of a single player, or with every tank of a team a wreck.
    Lost,
    /// Its players left it, started another one or closed the game.
    Left,
}

/// A run, as its line of the journal tells it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// When the run started, as [`stamp`] writes it.
    pub when: String,
    pub version: String,
    /// The fingerprint of the balance that the game was compiled with.
    pub balance: u64,
    pub seed: String,
    pub difficulty: Difficulty,
    /// The players at the start of the run.
    pub players: u32,
    /// Whether the run was in arena mode: its sectors are then the groups of its room.
    pub arena: bool,
    /// Whether the players built their own tanks.
    pub free: bool,
    pub playtest: bool,
    /// Whether the lives were infinite at some time of the run.
    pub endless: bool,
    /// The sectors that the run entered, in the order it entered them. A playtest that
    /// skips back enters a sector twice.
    pub entered: Vec<u32>,
    /// Those of them that are arenas.
    pub arenas: Vec<u32>,
    /// The score: the furthest sector cleared, or reached by a skip.
    pub cleared: u32,
    pub losses: Vec<Loss>,
    /// The seconds that the run lasted.
    pub fought: u32,
    pub kills: u32,
    pub end: End,
}

/// The sectors of a list as ranges: `1-3,15-22`. A range goes on for as long as each sector
/// is the one after the last.
fn ranges(sectors: &[u32]) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut rest = sectors;
    while let Some(&first) = rest.first() {
        let follows = |(index, sector): (usize, &u32)| *sector == first + index as u32;
        let length = rest
            .iter()
            .enumerate()
            .take_while(|pair| follows(*pair))
            .count();
        words.push(match length {
            1 => first.to_string(),
            _ => format!("{first}-{}", first + length as u32 - 1),
        });
        rest = &rest[length..];
    }
    list(words)
}

/// Words with commas between them, or a dash for none.
fn list(words: Vec<String>) -> String {
    match words.is_empty() {
        true => "-".to_string(),
        false => words.join(","),
    }
}

/// The words of a list that [`list`] wrote.
fn items(word: &str) -> impl Iterator<Item = &str> {
    word.split(',')
        .filter(|item| !item.is_empty() && *item != "-")
}

/// The sectors that ranges stand for. A journal that says a run entered a million sectors is
/// damaged.
fn sectors(word: &str) -> Option<Vec<u32>> {
    let mut sectors = Vec::new();
    for item in items(word) {
        let (first, last): (u32, u32) = match item.split_once('-') {
            Some((first, last)) => (first.parse().ok()?, last.parse().ok()?),
            None => (item.parse().ok()?, item.parse().ok()?),
        };
        if first > last || last - first > 10_000 {
            return None;
        }
        sectors.extend(first..=last);
    }
    Some(sectors)
}

impl Entry {
    /// The line of the journal that tells this run.
    pub fn line(&self) -> String {
        // A seed is one word of the line, whatever a command line gave.
        let seed: Vec<&str> = self.seed.split_whitespace().collect();
        let seed = match seed.is_empty() {
            true => "-".to_string(),
            false => seed.join("_"),
        };
        let options = [
            (self.arena, "arena"),
            (self.free, "free"),
            (self.playtest, "playtest"),
            (self.endless, "endless"),
        ];
        let options = options.into_iter().filter(|(set, _)| *set);
        let arenas = self.arenas.iter().map(u32::to_string).collect();
        let end = match self.end {
            End::Lost => "lost",
            End::Left => "left",
        };
        format!(
            "run when {} version {} balance {:016x} seed {seed} mode {} players {} options {} \
             entered {} arenas {} cleared {} lost {} fought {}s kills {} ended {end}",
            self.when,
            self.version,
            self.balance,
            self.difficulty.key(),
            self.players,
            list(options.map(|(_, name)| name.to_string()).collect()),
            ranges(&self.entered),
            list(arenas),
            self.cleared,
            list(self.losses.iter().map(Loss::word).collect()),
            self.fought,
            self.kills,
        )
    }

    /// The run that a line of the journal tells, if it is one that this reader knows.
    pub fn read(line: &str) -> Option<Self> {
        let mut words = line.split_whitespace();
        if words.next() != Some("run") {
            return None;
        }
        // Each key has one word. A key that this reader doesn't know is of a later version.
        let words: Vec<&str> = words.collect();
        let word = |key: &str| {
            let (pairs, _) = words.as_chunks::<2>();
            pairs
                .iter()
                .rev()
                .find(|pair| pair[0] == key)
                .map(|pair| pair[1])
        };
        let options: Vec<&str> = items(word("options")?).collect();
        let losses: Option<Vec<Loss>> = items(word("lost")?).map(Loss::read).collect();
        let arenas: Option<Vec<u32>> = items(word("arenas")?)
            .map(|sector| sector.parse().ok())
            .collect();
        Some(Self {
            when: word("when")?.to_string(),
            version: word("version")?.to_string(),
            balance: u64::from_str_radix(word("balance")?, 16).ok()?,
            seed: word("seed")?.to_string(),
            difficulty: Difficulty::of_key(word("mode")?)?,
            players: word("players")?.parse().ok()?,
            arena: options.contains(&"arena"),
            free: options.contains(&"free"),
            playtest: options.contains(&"playtest"),
            endless: options.contains(&"endless"),
            entered: sectors(word("entered")?)?,
            arenas: arenas?,
            cleared: word("cleared")?.parse().ok()?,
            losses: losses?,
            fought: word("fought")?.strip_suffix('s')?.parse().ok()?,
            kills: word("kills")?.parse().ok()?,
            end: match word("ended")? {
                "lost" => End::Lost,
                "left" => End::Left,
                _ => return None,
            },
        })
    }

    /// Whether the run was played as a record counts one: from the start bay, through each
    /// sector in turn, with the lives that a run has, until it was lost. Its score then
    /// compares with the scores of other such runs.
    pub fn whole(&self) -> bool {
        let in_turn = self
            .entered
            .iter()
            .copied()
            .eq(1..=self.entered.len() as u32);
        in_turn && !self.endless && self.end == End::Lost
    }
}

/// What a journal holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Journal {
    pub entries: Vec<Entry>,
    /// The lines that are neither a run that this reader knows, nor the heading, nor empty.
    pub unread: usize,
}

/// Reads the text of a journal.
pub fn read(text: &str) -> Journal {
    let mut journal = Journal::default();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("tanku-journal") {
            continue;
        }
        match Entry::read(line) {
            Some(entry) => journal.entries.push(entry),
            None => journal.unread += 1,
        }
    }
    journal
}

/// A moment as the journal writes it, from the seconds since 1970 in universal time:
/// `2026-10-13T17:46Z`. Lines sort by it as texts do.
pub fn stamp(seconds: u64) -> String {
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // The date of a day, counted from March so that the leap day ends a year: a year of
    // 365 days, with one more every 4 years, less every 100 and more every 400.
    let shifted = days + 719_468;
    let (era, day_of_era) = (shifted / 146_097, shifted % 146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = (month_from_march + 2) % 12 + 1;
    let year = era * 400 + year_of_era + (month <= 2) as u64;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Genome;

    fn entry() -> Entry {
        Entry {
            when: "2026-10-13T17:46Z".to_string(),
            version: "0.7.0".to_string(),
            balance: 0x9033_050d_20f5_0f0b,
            seed: "85GVTHQA".to_string(),
            difficulty: Difficulty::Normal,
            players: 1,
            arena: false,
            free: false,
            playtest: true,
            endless: true,
            entered: vec![1, 2, 3, 15, 16, 17],
            arenas: vec![16],
            cleared: 16,
            losses: vec![
                Loss {
                    sector: 3,
                    cause: Cause::Bullet(Ammo::Normal),
                    by: Culprit::Enemy(Class::Gunner),
                },
                Loss {
                    sector: 16,
                    cause: Cause::Mine,
                    by: Culprit::Player,
                },
                Loss {
                    sector: 17,
                    cause: Cause::Unknown,
                    by: Culprit::Nobody,
                },
            ],
            fought: 512,
            kills: 43,
            end: End::Left,
        }
    }

    #[test]
    fn a_run_is_one_line_that_reads_back_as_it_was() {
        let entry = entry();
        let line = entry.line();
        assert_eq!(
            line,
            "run when 2026-10-13T17:46Z version 0.7.0 balance 9033050d20f50f0b seed 85GVTHQA \
             mode normal players 1 options playtest,endless entered 1-3,15-17 arenas 16 cleared \
             16 lost 3:bullet:gunner,16:mine:player,17:unknown:nobody fought 512s kills 43 \
             ended left"
        );
        assert!(!line.contains('\n'));
        assert_eq!(Entry::read(&line), Some(entry));
        // A run with none of what a list holds writes a dash, which reads as nothing.
        let bare = Entry {
            playtest: false,
            endless: false,
            entered: vec![1],
            arenas: Vec::new(),
            losses: Vec::new(),
            end: End::Lost,
            ..self::entry()
        };
        let line = bare.line();
        assert!(line.contains("options - entered 1 arenas - cleared 16 lost - fought"));
        assert_eq!(Entry::read(&line), Some(bare));
    }

    #[test]
    fn every_weapon_and_every_class_has_a_name_that_reads_back() {
        for cause in Cause::ALL {
            assert_eq!(Cause::named(cause.name()), Some(cause));
        }
        let enemies = Class::ALL.into_iter().map(Culprit::Enemy);
        for by in enemies.chain([Culprit::Player, Culprit::Nobody]) {
            assert_eq!(Culprit::named(by.name()), Some(by), "{by:?}");
        }
        // No class goes by the name of a player or of nobody.
        let names: Vec<&str> = Class::ALL.into_iter().map(Class::name).collect();
        assert!(!names.contains(&"player") && !names.contains(&"nobody"));
    }

    #[test]
    fn a_loss_is_told_by_the_blow_that_the_run_tells_of_it() {
        let mut grenadier = Genome::empty(60);
        grenadier.class = Class::Grenadier;
        grenadier.set(crate::stats::Stat::Exploding, 1);
        let blow = |weapon: Weapon, team: Team| Blow {
            weapon,
            genome: grenadier,
            team,
        };
        let shell = Loss::of(7, Some(blow(Weapon::Bullet, Team::Enemies)));
        assert_eq!(shell.word(), "7:exploding:grenadier");
        let mine = Loss::of(7, Some(blow(Weapon::Mine, Team::Enemies)));
        assert_eq!(mine.word(), "7:mine:grenadier");
        // What a player's own weapon did is the doing of a player, whatever the tank.
        let own = Loss::of(8, Some(blow(Weapon::Crate, Team::Players)));
        assert_eq!(own.word(), "8:crate:player");
        assert_eq!(Loss::of(9, None).word(), "9:unknown:nobody");
        for loss in [shell, mine, own, Loss::of(9, None)] {
            assert_eq!(Loss::read(&loss.word()), Some(loss));
        }
    }

    #[test]
    fn sectors_are_written_as_ranges_in_the_order_they_were_entered() {
        assert_eq!(ranges(&[]), "-");
        assert_eq!(ranges(&[4]), "4");
        assert_eq!(ranges(&[1, 2, 3, 4]), "1-4");
        // A playtest skips ahead, and back.
        assert_eq!(ranges(&[1, 2, 15, 16, 17, 3, 9]), "1-2,15-17,3,9");
        for entered in [vec![], vec![4], vec![1, 2, 15, 16, 17, 3, 9]] {
            assert_eq!(sectors(&ranges(&entered)), Some(entered));
        }
        assert_eq!(sectors("5-3"), None);
        assert_eq!(sectors("1-4000000000"), None);
        assert_eq!(sectors("one"), None);
    }

    #[test]
    fn a_reader_takes_what_it_can_read_and_counts_the_rest() {
        let run = entry().line();
        // A later version may add a key, and an older one lacks one.
        let later = format!("{run} weather rain");
        let older = run.replace(" kills 43", "");
        let text = format!(
            "{HEADING}\n\n{run}\n{later}\nrun when never\n{older}\nwhat is this\n  {run}  \n"
        );
        let journal = read(&text);
        assert_eq!(journal.entries, vec![entry(), entry(), entry()]);
        assert_eq!(journal.unread, 3);
        assert_eq!(read(""), Journal::default());
        // The last word of a key wins, as a later line of a file would.
        let twice = format!("{run} cleared 20");
        assert_eq!(Entry::read(&twice).unwrap().cleared, 20);
        // A value that is no value of its key is not read as another.
        for (from, to) in [
            ("mode normal", "mode easy"),
            ("ended left", "ended well"),
            ("balance 9033050d20f50f0b", "balance none"),
            ("3:bullet:gunner", "3:arrow:gunner"),
            ("3:bullet:gunner", "3:bullet:gunner:twice"),
            ("fought 512s", "fought 512"),
        ] {
            assert_eq!(Entry::read(&run.replace(from, to)), None, "{to}");
        }
    }

    #[test]
    fn a_whole_run_goes_from_the_bay_through_each_sector_with_its_lives_until_it_is_lost() {
        let whole = Entry {
            playtest: false,
            endless: false,
            entered: (1..=17).collect(),
            end: End::Lost,
            ..entry()
        };
        assert!(whole.whole());
        // A skip, infinite lives and a run that was left each make another kind of run.
        assert!(!entry().whole());
        let skipped = Entry {
            entered: vec![1, 2, 5],
            ..whole.clone()
        };
        let endless = Entry {
            endless: true,
            ..whole.clone()
        };
        let left = Entry {
            end: End::Left,
            ..whole.clone()
        };
        let late = Entry {
            entered: vec![2, 3],
            ..whole
        };
        for other in [skipped, endless, left, late] {
            assert!(!other.whole(), "{:?}", other.entered);
        }
    }

    #[test]
    fn a_moment_is_written_as_its_date_and_time() {
        assert_eq!(stamp(0), "1970-01-01T00:00Z");
        assert_eq!(stamp(59), "1970-01-01T00:00Z");
        assert_eq!(stamp(86_399), "1970-01-01T23:59Z");
        // The last day of February in a leap year, and the day after.
        assert_eq!(stamp(951_782_400), "2000-02-29T00:00Z");
        assert_eq!(stamp(951_868_800), "2000-03-01T00:00Z");
        assert_eq!(stamp(1_709_251_199), "2024-02-29T23:59Z");
        assert_eq!(stamp(1_791_913_560), "2026-10-13T17:46Z");
        assert_eq!(stamp(4_102_444_800), "2100-01-01T00:00Z");
        // Later moments sort after earlier ones, as texts.
        assert!(stamp(1_791_913_560) < stamp(1_791_913_620));
        assert!(stamp(1_791_913_560) < stamp(1_800_000_000));
    }
}
