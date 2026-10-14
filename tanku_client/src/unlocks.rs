//! What the player has unlocked for solo. It is kept on their machine, and a run adds to it
//! the moment it has earned something.
//!
//! A playtest can fake it: with its switch on, the machine sets what it keeps aside, with its
//! records, and plays as on its first time. What the playtest earns then is kept in memory
//! alone, until the game is closed. With the switch off again, the machine has its own back.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use tanku_core::{
    modes::{self, Difficulty, Unlock},
    sim::{Playtest, Run, RunRequest, SimEvent},
};

use crate::{settings::Records, storage};

const UNLOCKS_FILE: &str = "unlocks.txt";

/// What the player has unlocked, and what the run that is on has added to it.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Unlocks {
    earned: BTreeSet<Unlock>,
    /// What the run that is on has earned, for its report.
    pub new: Vec<Unlock>,
    /// Whether this is what a playtest that fakes achievements has earned, which no file
    /// keeps. A playtest opens every mode and option to the player, but not this one.
    pub fake: bool,
    /// What the player has unlocked and not used yet, which the solo screen tags as new.
    unused: BTreeSet<Unlock>,
    /// What the player has unlocked and not seen on the screen of the achievements yet, which
    /// that screen tags as new.
    unseen: BTreeSet<Unlock>,
}

impl Unlocks {
    /// What a player has who earned these.
    #[cfg(any(test, feature = "script"))]
    pub fn of(earned: &[Unlock]) -> Self {
        Self {
            earned: earned.iter().copied().collect(),
            ..default()
        }
    }

    /// What a player has who has just earned these, and has neither used nor seen them yet.
    #[cfg(test)]
    pub fn fresh(earned: &[Unlock]) -> Self {
        let mut unlocks = Self::default();
        unlocks.take(earned.iter().copied());
        unlocks.new.clear();
        unlocks
    }

    /// What a playtest that fakes achievements has to start with: nothing, as on the first
    /// time.
    pub fn fake() -> Self {
        Self {
            fake: true,
            ..default()
        }
    }

    /// Whether a mode or an option is open to the player. A difficulty that is unlocked
    /// unlocks the ones below it. And arena mode, which the first sectors unlock, came to
    /// the game after the others: whoever has earned any of them had cleared those sectors
    /// already, and has it.
    pub fn has(&self, unlock: Unlock) -> bool {
        let above = unlock == Unlock::HardMode && self.earned.contains(&Unlock::ImpossibleMode);
        let before = unlock == Unlock::ArenaMode && !self.earned.is_empty();
        self.earned.contains(&unlock) || above || before
    }

    /// Whether the player has unlocked a mode or an option and not used it yet.
    pub fn is_unused(&self, unlock: Unlock) -> bool {
        self.unused.contains(&unlock)
    }

    /// Whether the player has unlocked it and not seen it on the screen of the achievements.
    pub fn is_unseen(&self, unlock: Unlock) -> bool {
        self.unseen.contains(&unlock)
    }

    /// The player uses a mode or an option. Returns whether it was new to them.
    pub fn used(&mut self, unlock: Unlock) -> bool {
        self.unused.remove(&unlock)
    }

    /// The player has seen the screen of the achievements. Returns whether anything on it was
    /// new to them.
    pub fn seen(&mut self) -> bool {
        let new = !self.unseen.is_empty();
        self.unseen.clear();
        new
    }

    /// Keeps what the player has unlocked on their machine, unless a playtest fakes it.
    pub fn save(&self) {
        if !self.fake {
            storage::save(UNLOCKS_FILE, &self.to_pairs());
        }
    }

    /// Whether something is left for a run to earn.
    pub fn wanting(&self) -> bool {
        Unlock::ALL.iter().any(|&unlock| !self.has(unlock))
    }

    /// Adds what a run has earned so far. Returns what is new in it: what the player
    /// didn't have yet.
    fn take(&mut self, earned: impl Iterator<Item = Unlock>) -> Vec<Unlock> {
        let earned: Vec<Unlock> = earned.collect();
        let new: Vec<Unlock> = earned
            .iter()
            .copied()
            .filter(|&unlock| !self.has(unlock))
            .collect();
        self.earned.extend(earned);
        self.new.extend(&new);
        self.unused.extend(&new);
        self.unseen.extend(&new);
        new
    }

    fn from_pairs(pairs: &BTreeMap<String, String>) -> Self {
        let kept = |unlock: &Unlock| pairs.contains_key(unlock.key());
        let earned: BTreeSet<Unlock> = Unlock::ALL.into_iter().filter(kept).collect();
        let marked = |mark: &str| -> BTreeSet<Unlock> {
            let marked = |unlock: &&Unlock| pairs.contains_key(&format!("{}.{mark}", unlock.key()));
            earned.iter().filter(marked).copied().collect()
        };
        Self {
            unused: marked("unused"),
            unseen: marked("unseen"),
            earned,
            ..default()
        }
    }

    fn to_pairs(&self) -> BTreeMap<String, String> {
        let pair = |unlock: &Unlock| (unlock.key().to_string(), "unlocked".to_string());
        let mark = |mark: &'static str| {
            move |unlock: &Unlock| (format!("{}.{mark}", unlock.key()), "yes".to_string())
        };
        let earned = self.earned.iter().map(pair);
        let unused = self.unused.iter().map(mark("unused"));
        let unseen = self.unseen.iter().map(mark("unseen"));
        earned.chain(unused).chain(unseen).collect()
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

/// Whether the last run that this machine started would earn achievements if it were no
/// playtest: a playtest that fakes them earns as such a run does.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fair(pub bool);

/// What this machine sets aside while it uses the other: the fake progress of a playtest
/// until its switch is turned on, and then its own, until it is turned off.
#[derive(Resource)]
struct Aside {
    unlocks: Unlocks,
    records: Records,
}

impl Default for Aside {
    fn default() -> Self {
        Self {
            unlocks: Unlocks::fake(),
            records: Records::fake(),
        }
    }
}

/// Whether a run earns achievements on this machine: as the simulation says, or as a fair run
/// would if this machine fakes them in a playtest. A guest's machine never fakes them: the
/// switch of a playtest is its host's.
pub fn earns(run: &Run, unlocks: &Unlocks, fair: Fair) -> bool {
    run.earns || (run.playtest && unlocks.fake && fair.0)
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Unlocks::from_pairs(&storage::load(UNLOCKS_FILE)))
        .init_resource::<Fair>()
        .init_resource::<Aside>()
        .add_message::<Unlocked>()
        .add_systems(Update, (judge, swap, earn).chain());
}

/// Notes whether the run that this machine starts is fair: what the simulation is not told of
/// in a playtest.
fn judge(mut requests: MessageReader<RunRequest>, mut fair: ResMut<Fair>) {
    for request in requests.read() {
        if let RunRequest::Start {
            drawn, free, arena, ..
        } = request
        {
            fair.0 = modes::fair(*drawn, *free, *arena);
        }
    }
}

/// Swaps this machine's own progress for the fake one of a playtest when its switch is turned
/// on, and back when it is turned off.
fn swap(
    playtest: Option<Res<Playtest>>,
    mut unlocks: ResMut<Unlocks>,
    mut records: ResMut<Records>,
    mut aside: ResMut<Aside>,
) {
    let faked = playtest.is_some_and(|playtest| playtest.fake_achievements);
    if unlocks.fake != faked {
        std::mem::swap(&mut *unlocks, &mut aside.unlocks);
        std::mem::swap(&mut *records, &mut aside.records);
    }
}

/// Gives the player what their run has earned, as soon as it has, and keeps it. A run of
/// several players gives it to each of them, on their own machine. What a playtest fakes is
/// kept in memory alone.
pub fn earn(
    run: Option<Res<Run>>,
    fair: Res<Fair>,
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
    let earns = earns(&run, &unlocks, *fair);
    let earned = || modes::earned_by(&run, earns);
    if earned().all(|unlock| unlocks.earned.contains(&unlock)) {
        return;
    }
    for unlock in unlocks.take(earned()) {
        info!("unlocked: {}", name(unlock));
        told.write(Unlocked(unlock));
    }
    unlocks.save();
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
    fn what_a_player_unlocked_is_new_until_they_use_it_and_see_it() {
        let mut unlocks = Unlocks::default();
        unlocks.take([Unlock::ArenaMode, Unlock::FreeMode].into_iter());
        for unlock in [Unlock::ArenaMode, Unlock::FreeMode] {
            assert!(unlocks.is_unused(unlock) && unlocks.is_unseen(unlock));
        }
        // What was not unlocked is no news, and neither is what was unlocked before.
        assert!(!unlocks.is_unused(Unlock::ChosenSeed) && !unlocks.is_unseen(Unlock::ChosenSeed));
        let mut older = Unlocks::of(&[Unlock::ChosenSeed]);
        older.take([Unlock::ArenaMode, Unlock::ChosenSeed].into_iter());
        assert!(!older.is_unused(Unlock::ArenaMode) && !older.is_unused(Unlock::ChosenSeed));
        // Kept on the machine as it is.
        assert_eq!(
            Unlocks::from_pairs(&unlocks.to_pairs()).unused,
            unlocks.unused
        );
        assert_eq!(
            Unlocks::from_pairs(&unlocks.to_pairs()).unseen,
            unlocks.unseen
        );
        // Used once, an option is no longer new on the solo screen; seen once, the screen of
        // the achievements has nothing new.
        assert!(unlocks.used(Unlock::ArenaMode));
        assert!(!unlocks.used(Unlock::ArenaMode));
        assert!(!unlocks.is_unused(Unlock::ArenaMode) && unlocks.is_unused(Unlock::FreeMode));
        assert!(unlocks.seen() && !unlocks.seen());
        assert!(!unlocks.is_unseen(Unlock::FreeMode));
        // A file from before these marks has nothing new.
        let mut old = BTreeMap::new();
        old.insert("arena".to_string(), "unlocked".to_string());
        let old = Unlocks::from_pairs(&old);
        assert!(old.has(Unlock::ArenaMode) && !old.is_unused(Unlock::ArenaMode));
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
    fn whoever_earned_anything_before_arena_mode_existed_has_arena_mode() {
        // A player who could type a seed had cleared the sector that unlocks arena mode, and
        // more. They aren't told of arena mode as of something new when a run earns it again.
        let mut unlocks = Unlocks::of(&[Unlock::ChosenSeed]);
        assert!(unlocks.has(Unlock::ArenaMode));
        assert!(!unlocks.has(Unlock::FreeMode) && !unlocks.has(Unlock::HardMode));
        let earned = [Unlock::ArenaMode, Unlock::ChosenSeed, Unlock::FreeMode];
        assert_eq!(unlocks.take(earned.into_iter()), [Unlock::FreeMode]);
        assert_eq!(unlocks.take(earned.into_iter()), []);
        // What is kept is all that was earned.
        let kept = unlocks.to_pairs();
        assert_eq!(kept.values().filter(|kept| *kept == "unlocked").count(), 3);
        for earlier in [Unlock::FreeMode, Unlock::HardMode, Unlock::ImpossibleMode] {
            assert!(Unlocks::of(&[earlier]).has(Unlock::ArenaMode));
        }
        // Whoever has earned nothing has nothing, and arena mode brings nothing else.
        assert!(!Unlocks::default().has(Unlock::ArenaMode));
        let first = Unlocks::of(&[Unlock::ArenaMode]);
        assert!(!first.has(Unlock::ChosenSeed) && first.wanting());
        let mut fresh = Unlocks::default();
        assert_eq!(
            fresh.take([Unlock::ArenaMode].into_iter()),
            [Unlock::ArenaMode]
        );
    }

    /// A playtest of a single player on a seed that the game drew for it, which has cleared
    /// this many sectors, with what this machine keeps.
    fn playtest(cleared: u32, kept: Unlocks, records: Records) -> App {
        let mut app = App::new();
        app.insert_resource(kept)
            .insert_resource(records)
            .init_resource::<Fair>()
            .init_resource::<Aside>()
            .insert_resource(Playtest {
                infinite_lives: true,
                fake_achievements: false,
            })
            .insert_resource(Run {
                seed_text: "TEST".into(),
                seed: 1,
                difficulty: default(),
                players: 1,
                team: 1,
                solo: true,
                sector: cleared + 1,
                cleared,
                checkpoint: cleared,
                lives: 3,
                kills: 0,
                ticks: 0,
                phase: tanku_core::sim::RunPhase::Playing,
                lost_to: None,
                playtest: true,
                earns: false,
                free: false,
                arena: false,
                lull: None,
                paused: false,
            })
            .add_message::<SimEvent>()
            .add_message::<RunRequest>()
            .add_message::<Unlocked>()
            .add_systems(Update, (judge, swap, earn).chain());
        app.world_mut().write_message(RunRequest::Start {
            seed: "TEST".into(),
            players: 1,
            drawn: true,
            difficulty: default(),
            free: false,
            arena: false,
            tanks: Vec::new(),
        });
        app
    }

    fn told(app: &mut App) -> Vec<Unlock> {
        let world = app.world_mut();
        let mut told = world.resource_mut::<Messages<Unlocked>>();
        told.drain().map(|Unlocked(unlock)| unlock).collect()
    }

    #[test]
    fn a_playtest_that_fakes_achievements_plays_as_on_the_first_time() {
        let mut records = Records::default();
        records.submit(crate::settings::League::default(), 1, 25, "OWN");
        let own = Unlocks::of(&[Unlock::ArenaMode, Unlock::ChosenSeed]);
        let mut app = playtest(10, own.clone(), records.clone());
        // Without the switch, a playtest earns nothing, and the machine keeps its own.
        app.update();
        assert_eq!(told(&mut app), []);
        assert_eq!(*app.world().resource::<Unlocks>(), own);
        // With it, the machine has nothing yet, as on its first time, and the sector that the
        // playtest has cleared earns what it would in a run that counts.
        app.world_mut().resource_mut::<Playtest>().fake_achievements = true;
        app.update();
        assert_eq!(told(&mut app), [Unlock::ArenaMode]);
        let fake = app.world().resource::<Unlocks>().clone();
        assert!(fake.fake && fake.has(Unlock::ArenaMode) && !fake.has(Unlock::ChosenSeed));
        let fake_records = app.world().resource::<Records>();
        assert!(fake_records.fake);
        assert_eq!(fake_records.best(Default::default(), 1), None);
        // Off again, the machine has its own back, untouched.
        app.world_mut().resource_mut::<Playtest>().fake_achievements = false;
        app.update();
        assert_eq!(*app.world().resource::<Unlocks>(), own);
        assert_eq!(*app.world().resource::<Records>(), records);
        // On again, the fake progress is as it was left: kept in memory until the game is
        // closed.
        app.world_mut().resource_mut::<Playtest>().fake_achievements = true;
        app.update();
        assert_eq!(*app.world().resource::<Unlocks>(), fake);
        assert_eq!(told(&mut app), []);
    }

    #[test]
    fn a_playtest_fakes_only_what_a_run_that_counts_would_earn() {
        // On a seed that the playtest typed, nothing is earned, as for anyone.
        let mut app = playtest(10, Unlocks::default(), Records::default());
        app.world_mut().resource_mut::<Playtest>().fake_achievements = true;
        app.world_mut().write_message(RunRequest::Start {
            seed: "TYPED".into(),
            players: 1,
            drawn: false,
            difficulty: default(),
            free: false,
            arena: false,
            tanks: Vec::new(),
        });
        app.update();
        assert_eq!(told(&mut app), []);
        // A run that is no playtest earns as the simulation says, whatever the switch.
        let run = Run {
            playtest: false,
            earns: false,
            ..app.world().resource::<Run>().clone()
        };
        assert!(!earns(&run, &Unlocks::fake(), Fair(true)));
        assert!(earns(
            &Run { earns: true, ..run },
            &Unlocks::default(),
            Fair(false)
        ));
    }

    #[test]
    fn a_difficulty_that_is_unlocked_unlocks_the_ones_below_it() {
        let unlocks = Unlocks::of(&[Unlock::ImpossibleMode]);
        assert!(unlocks.has(Unlock::HardMode));
        assert!(!unlocks.has(Unlock::ChosenSeed));
        assert!(unlocks.wanting());
        assert!(!Unlocks::of(&Unlock::ALL).wanting());
        // With the impossible mode, only the two options that later sectors unlock are left
        // to earn.
        let unlocks = Unlocks::of(&[Unlock::ChosenSeed, Unlock::FreeMode, Unlock::ImpossibleMode]);
        assert!(!unlocks.wanting());
    }
}
