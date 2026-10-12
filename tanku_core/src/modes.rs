//! What a player unlocks for solo, and the runs that earn it.

use crate::sim::Run;

/// A mode or an option of a run that a player unlocks for solo by getting far enough in a
/// run. The host of a game of several players has all of them from the start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unlock {
    /// Typing the seed of a run.
    ChosenSeed,
    /// Spending the points of the tank before a run.
    FreeMode,
    HardMode,
    ImpossibleMode,
}

impl Unlock {
    pub const ALL: [Unlock; 4] = [
        Unlock::ChosenSeed,
        Unlock::FreeMode,
        Unlock::HardMode,
        Unlock::ImpossibleMode,
    ];

    /// The sector that a run must clear to earn it.
    pub const fn sector(self) -> u32 {
        match self {
            Unlock::ChosenSeed => 10,
            Unlock::FreeMode | Unlock::ImpossibleMode => 20,
            Unlock::HardMode => 30,
        }
    }

    /// Whether that run must be one in hard mode.
    pub const fn in_hard_mode(self) -> bool {
        matches!(self, Unlock::ImpossibleMode)
    }

    /// The name that a player's machine keeps it by.
    pub const fn key(self) -> &'static str {
        match self {
            Unlock::ChosenSeed => "seed",
            Unlock::FreeMode => "free",
            Unlock::HardMode => "hard",
            Unlock::ImpossibleMode => "impossible",
        }
    }
}

/// What a run has earned so far. A run that can't earn achievements earns nothing, however
/// far it gets. No run is in hard mode yet, so nothing that takes one is earned.
pub fn earned(run: &Run) -> impl Iterator<Item = Unlock> {
    let far = (run.earns, run.cleared);
    let reached =
        move |unlock: &Unlock| far.0 && !unlock.in_hard_mode() && far.1 >= unlock.sector();
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
        }
    }

    #[test]
    fn a_run_earns_what_the_sectors_it_cleared_are_worth() {
        let earned = |cleared: u32, earns: bool| earned(&run(cleared, earns)).collect::<Vec<_>>();
        assert_eq!(earned(9, true), []);
        assert_eq!(earned(10, true), [Unlock::ChosenSeed]);
        assert_eq!(earned(20, true), [Unlock::ChosenSeed, Unlock::FreeMode]);
        // The impossible mode takes a run in hard mode, which no run is yet.
        let all_of_normal = [Unlock::ChosenSeed, Unlock::FreeMode, Unlock::HardMode];
        assert_eq!(earned(30, true), all_of_normal);
        assert_eq!(earned(99, true), all_of_normal);
        // A run on a chosen seed, or a playtest, earns nothing.
        assert_eq!(earned(99, false), []);
    }

    #[test]
    fn the_names_that_unlocks_are_kept_by_differ() {
        let mut keys: Vec<&str> = Unlock::ALL.iter().map(|unlock| unlock.key()).collect();
        keys.dedup();
        assert_eq!(keys.len(), Unlock::ALL.len());
    }
}
