//! What the player has unlocked for solo. It is kept on their machine, and a run adds to it
//! the moment it has earned something.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use tanku_core::{
    modes::{self, Difficulty, Unlock},
    sim::{Run, SimEvent},
};

use crate::storage;

const UNLOCKS_FILE: &str = "unlocks.txt";

/// What the player has unlocked, and what the run that is on has added to it.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Unlocks {
    earned: BTreeSet<Unlock>,
    /// What the run that is on has earned, for its report.
    pub new: Vec<Unlock>,
}

impl Unlocks {
    /// What a player has who earned these.
    #[cfg(any(test, feature = "script"))]
    pub fn of(earned: &[Unlock]) -> Self {
        Self {
            earned: earned.iter().copied().collect(),
            new: Vec::new(),
        }
    }

    /// Whether a mode or an option is open to the player. A difficulty that is unlocked
    /// unlocks the ones below it.
    pub fn has(&self, unlock: Unlock) -> bool {
        let above = unlock == Unlock::HardMode && self.earned.contains(&Unlock::ImpossibleMode);
        self.earned.contains(&unlock) || above
    }

    /// Whether something is left for a run to earn.
    pub fn wanting(&self) -> bool {
        Unlock::ALL.iter().any(|&unlock| !self.has(unlock))
    }

    /// Adds what a run has earned so far. Returns what is new in it.
    fn take(&mut self, earned: impl Iterator<Item = Unlock>) -> Vec<Unlock> {
        let new: Vec<Unlock> = earned
            .filter(|&unlock| self.earned.insert(unlock))
            .collect();
        self.new.extend(&new);
        new
    }

    fn from_pairs(pairs: &BTreeMap<String, String>) -> Self {
        let kept = |unlock: &Unlock| pairs.contains_key(unlock.key());
        Self {
            earned: Unlock::ALL.into_iter().filter(kept).collect(),
            new: Vec::new(),
        }
    }

    fn to_pairs(&self) -> BTreeMap<String, String> {
        let pair = |unlock: &Unlock| (unlock.key().to_string(), "unlocked".to_string());
        self.earned.iter().map(pair).collect()
    }
}

/// What a mode or an option is called on screen.
pub fn name(unlock: Unlock) -> &'static str {
    match unlock {
        Unlock::ArenaMode => "Arena mode",
        Unlock::ChosenSeed => "Chosen seed",
        Unlock::FreeMode => "Free mode",
        Unlock::HardMode => "Hard mode",
        Unlock::ImpossibleMode => "Impossible mode",
    }
}

/// What a difficulty is called on screen.
pub fn difficulty_name(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Normal => "Normal",
        Difficulty::Hard => "Hard",
    }
}

/// Whether what an achievement unlocks is in the game. Until it is, the achievement is
/// earned and kept, and the player is told that nothing comes of it yet.
pub fn built(unlock: Unlock) -> bool {
    !matches!(unlock, Unlock::ImpossibleMode)
}

/// What a run must do to unlock it.
pub fn condition(unlock: Unlock) -> String {
    let sector = unlock.sector();
    if unlock.in_hard_mode() {
        format!("Clear sector {sector} in hard mode")
    } else {
        format!("Clear sector {sector}")
    }
}

/// Told when a run has just earned something.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unlocked(pub Unlock);

pub fn plugin(app: &mut App) {
    app.insert_resource(Unlocks::from_pairs(&storage::load(UNLOCKS_FILE)))
        .add_message::<Unlocked>()
        .add_systems(Update, earn);
}

/// Gives the player what their run has earned, as soon as it has, and keeps it. A run of
/// several players gives it to each of them, on their own machine.
fn earn(
    run: Option<Res<Run>>,
    mut events: MessageReader<SimEvent>,
    mut unlocks: ResMut<Unlocks>,
    mut told: MessageWriter<Unlocked>,
) {
    // A run that starts has earned nothing yet.
    let started = events.read().any(|event| *event == SimEvent::RunStarted);
    if started && !unlocks.new.is_empty() {
        unlocks.new.clear();
    }
    let Some(run) = run else {
        return;
    };
    if modes::earned(&run).all(|unlock| unlocks.earned.contains(&unlock)) {
        return;
    }
    for unlock in unlocks.take(modes::earned(&run)) {
        info!("unlocked: {}", name(unlock));
        told.write(Unlocked(unlock));
    }
    storage::save(UNLOCKS_FILE, &unlocks.to_pairs());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlocks_survive_being_written_and_read() {
        let unlocks = Unlocks::of(&[Unlock::ChosenSeed, Unlock::HardMode]);
        assert_eq!(Unlocks::from_pairs(&unlocks.to_pairs()), unlocks);
        assert_eq!(Unlocks::from_pairs(&BTreeMap::new()), Unlocks::default());
    }

    #[test]
    fn what_is_unlocked_is_in_the_game_except_the_impossible_mode() {
        let waiting: Vec<Unlock> = Unlock::ALL
            .into_iter()
            .filter(|unlock| !built(*unlock))
            .collect();
        assert_eq!(waiting, [Unlock::ImpossibleMode]);
    }

    #[test]
    fn a_run_adds_what_it_earned_once() {
        let mut unlocks = Unlocks::of(&[Unlock::ChosenSeed]);
        let earned = [Unlock::ChosenSeed, Unlock::FreeMode];
        assert_eq!(unlocks.take(earned.into_iter()), [Unlock::FreeMode]);
        assert_eq!(unlocks.take(earned.into_iter()), []);
        assert_eq!(unlocks.new, [Unlock::FreeMode]);
        assert!(unlocks.has(Unlock::FreeMode) && !unlocks.has(Unlock::HardMode));
    }

    #[test]
    fn a_difficulty_that_is_unlocked_unlocks_the_ones_below_it() {
        let unlocks = Unlocks::of(&[Unlock::ImpossibleMode]);
        assert!(unlocks.has(Unlock::HardMode));
        assert!(!unlocks.has(Unlock::ChosenSeed));
        assert!(unlocks.wanting());
        assert!(!Unlocks::of(&Unlock::ALL).wanting());
        // With the impossible mode, only the options are left to earn.
        let options = [Unlock::ArenaMode, Unlock::ChosenSeed, Unlock::FreeMode];
        assert!(Unlocks::of(&[Unlock::ImpossibleMode]).wanting());
        let mut every = options.to_vec();
        every.push(Unlock::ImpossibleMode);
        assert!(!Unlocks::of(&every).wanting());
        assert!(Unlocks::of(&every[1..]).wanting());
    }
}
