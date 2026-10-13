//! How hard a run is, what a player unlocks for solo, and the runs that earn it.

use serde::{Deserialize, Serialize};

use crate::{balance::BALANCE, sim::Run};

/// How hard the enemies of a run are. The map of a seed is the same for all of them, and so
/// are the number of its enemies, their classes and their places.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum Difficulty {
    /// The rules as they are.
    #[default]
    Normal,
    /// Enemies have more points, every tier of software from the first sector, and the
    /// experience and the guns of twenty sectors further.
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 2] = [Difficulty::Normal, Difficulty::Hard];

    /// The name that a player's machine keeps it by, and the records of its runs.
    pub const fn key(self) -> &'static str {
        match self {
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
        }
    }

    /// The difficulty that goes by a name, if there is one.
    pub fn of_key(key: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|difficulty| difficulty.key() == key)
    }

    /// What a single player must have unlocked to play it.
    pub const fn unlock(self) -> Option<Unlock> {
        match self {
            Difficulty::Normal => None,
            Difficulty::Hard => Some(Unlock::HardMode),
        }
    }
}

/// A mode or an option of a run that a player unlocks for solo by getting far enough in a
/// run. The host of a game of several players has all of them from the start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unlock {
    /// Playing in one room, against one group after another.
    ArenaMode,
    /// Typing the seed of a run.
    ChosenSeed,
    /// Spending the points of the tank before a run.
    FreeMode,
    HardMode,
    ImpossibleMode,
}

impl Unlock {
    pub const ALL: [Unlock; 5] = [
        Unlock::ArenaMode,
        Unlock::ChosenSeed,
        Unlock::FreeMode,
        Unlock::HardMode,
        Unlock::ImpossibleMode,
    ];

    /// The sector that a run must clear to earn it.
    pub const fn sector(self) -> u32 {
        let unlock = BALANCE.unlock;
        match self {
            Unlock::ArenaMode => unlock.arena,
            Unlock::ChosenSeed => unlock.seed,
            Unlock::FreeMode => unlock.free,
            Unlock::HardMode => unlock.hard,
            Unlock::ImpossibleMode => unlock.impossible,
        }
    }

    /// Whether that run must be one in hard mode.
    pub const fn in_hard_mode(self) -> bool {
        matches!(self, Unlock::ImpossibleMode)
    }

    /// The name that a player's machine keeps it by.
    pub const fn key(self) -> &'static str {
        match self {
            Unlock::ArenaMode => "arena",
            Unlock::ChosenSeed => "seed",
            Unlock::FreeMode => "free",
            Unlock::HardMode => "hard",
            Unlock::ImpossibleMode => "impossible",
        }
    }
}

/// What a run has earned so far. A run that can't earn achievements earns nothing, however
/// far it gets: the groups that a run in arena mode clears are no sectors of the corridor. A
/// run in hard mode earns what a run in normal mode does, and what takes one in hard mode.
pub fn earned(run: &Run) -> impl Iterator<Item = Unlock> {
    let (earns, cleared, hard) = (run.earns, run.cleared, run.difficulty == Difficulty::Hard);
    let reached = move |unlock: &Unlock| {
        earns && cleared >= unlock.sector() && (hard || !unlock.in_hard_mode())
    };
    Unlock::ALL.into_iter().filter(reached)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::RunPhase;

    fn run(cleared: u32, earns: bool) -> Run {
        Run {
            seed_text: "TEST".into(),
            seed: 0,
            difficulty: Difficulty::Normal,
            players: 1,
            team: 1,
            solo: true,
            sector: cleared + 1,
            cleared,
            checkpoint: cleared,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase: RunPhase::Playing,
            lost_to: None,
            playtest: false,
            earns,
            free: false,
            arena: false,
            lull: None,
            paused: false,
        }
    }

    #[test]
    fn a_run_earns_what_the_sectors_it_cleared_are_worth() {
        let earned = |cleared: u32, earns: bool| earned(&run(cleared, earns)).collect::<Vec<_>>();
        let (arena, seed, free) = (Unlock::ArenaMode, Unlock::ChosenSeed, Unlock::FreeMode);
        assert_eq!(earned(4, true), []);
        assert_eq!(earned(5, true), [arena]);
        assert_eq!(earned(9, true), [arena]);
        assert_eq!(earned(10, true), [arena, seed]);
        assert_eq!(earned(20, true), [arena, seed, free]);
        // The impossible mode takes a run in hard mode, however far a run in normal mode gets.
        let all_of_normal = [arena, seed, free, Unlock::HardMode];
        assert_eq!(earned(30, true), all_of_normal);
        assert_eq!(earned(99, true), all_of_normal);
        // A run on a chosen seed, or a playtest, earns nothing.
        assert_eq!(earned(99, false), []);
    }

    #[test]
    fn a_run_in_hard_mode_earns_what_one_in_normal_mode_does_and_the_impossible_mode() {
        let earned = |cleared: u32, earns: bool| {
            let run = Run {
                difficulty: Difficulty::Hard,
                ..run(cleared, earns)
            };
            earned(&run).collect::<Vec<_>>()
        };
        let (arena, seed, free) = (Unlock::ArenaMode, Unlock::ChosenSeed, Unlock::FreeMode);
        assert_eq!(earned(4, true), []);
        assert_eq!(earned(9, true), [arena]);
        assert_eq!(earned(10, true), [arena, seed]);
        assert_eq!(earned(19, true), [arena, seed]);
        // A team may get there before any of its members has unlocked hard mode.
        let at_twenty = [arena, seed, free, Unlock::ImpossibleMode];
        assert_eq!(earned(20, true), at_twenty);
        assert_eq!(earned(30, true), Unlock::ALL);
        assert_eq!(earned(99, false), []);
    }

    #[test]
    fn a_difficulty_goes_by_a_name_of_its_own() {
        for difficulty in Difficulty::ALL {
            assert_eq!(Difficulty::of_key(difficulty.key()), Some(difficulty));
        }
        assert_eq!(Difficulty::of_key("easy"), None);
        assert_eq!(Difficulty::default(), Difficulty::Normal);
        // A single player has the normal difficulty from the start, and unlocks the hard one.
        assert_eq!(Difficulty::Normal.unlock(), None);
        assert_eq!(Difficulty::Hard.unlock(), Some(Unlock::HardMode));
    }

    #[test]
    fn the_names_that_unlocks_are_kept_by_differ() {
        let mut keys: Vec<&str> = Unlock::ALL.iter().map(|unlock| unlock.key()).collect();
        keys.dedup();
        assert_eq!(keys.len(), Unlock::ALL.len());
    }
}
