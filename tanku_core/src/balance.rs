//! The balance of the game: the numbers that decide how hard it is.
//!
//! They are written by hand in `game.balance`, which `docs/BALANCE.md` describes. The crate
//! reads the file while it is compiled, so the game carries its balance in itself and opens no
//! file when it starts. A file that can't be read stops the build, and the error says on
//! which line and why.
//!
//! The file holds numbers and no rules: which stats, classes and tiers there are, and what
//! each of them does, is said by the code that reads these numbers.
//!
//! The tests have a balance of their own, `test.balance`: this crate is compiled with it for
//! its own tests, and for those of the other crates, which ask for it with the feature
//! `testing`. A test that holds a number holds one of that file, so a change to the balance
//! of the game fails no test of a rule. The feature `game-balance` runs the tests on the
//! balance of the game all the same.

mod read;

pub use read::{Fault, fingerprint, read};

use crate::stats::{Class, Stat};

/// The most levels that a stat has, level 0 included.
pub const LEVELS: usize = 8;
/// The most sectors at which something that grows by steps goes up.
pub const RISES: usize = 8;
/// The most weights that a group draws from.
pub const WEIGHTS: usize = 8;

/// The file of the game, and the file of the tests.
const GAME_FILE: &str = include_str!("../game.balance");
#[cfg(any(test, feature = "testing"))]
const TEST_FILE: &str = include_str!("../test.balance");

/// The file that this build is compiled with, and its name: the one of the tests in a build
/// for tests, unless it was asked for the one of the game.
#[cfg(all(any(test, feature = "testing"), not(feature = "game-balance")))]
const FILE: (&str, &str) = (TEST_FILE, "test.balance");
#[cfg(not(all(any(test, feature = "testing"), not(feature = "game-balance"))))]
const FILE: (&str, &str) = (GAME_FILE, "game.balance");

/// The balance of the game, as its file writes it. The file is read while the crate is
/// compiled, in every build: one that can't be read stops the build, and the error says on
/// which line.
pub const GAME: Balance = match read(GAME_FILE) {
    Ok(balance) => balance,
    Err(fault) => fault.stop("game.balance"),
};

/// The balance that this build plays by: the one of the game, or the one of the tests in a
/// build for tests.
pub const BALANCE: Balance = match read(FILE.0) {
    Ok(balance) => balance,
    Err(fault) => fault.stop(FILE.1),
};

/// What tells the balance of this build from another: a number made of the words of its
/// file. Comments and the room between two words change nothing of it.
pub const FINGERPRINT: u64 = fingerprint(FILE.0);

/// Every number of `game.balance`. Times are in seconds, lengths in meters and speeds in
/// meters per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Balance {
    pub stats: Stats,
    /// The levels of the standard tank, by stat.
    pub standard: [u8; Stat::COUNT],
    /// The points of a tank of the garage.
    pub garage: u16,
    pub solo: Solo,
    pub tank: Chassis,
    pub gun: Gun,
    pub mine: Mine,
    /// How far the blast of a mine, of a crate and of an exploding bullet reaches.
    pub blast: f32,
    pub shield: Shield,
    pub split: Split,
    /// How long before it happens the bounce of a seeking bullet is marked, and the end of an
    /// exploding one on what stands still.
    pub mark: f32,
    pub repair: Repair,
    pub budget: Budget,
    pub experience: Experience,
    pub steps: Steps,
    /// What each player after the first adds to the tanks of a group, in percent.
    pub team: u32,
    pub hard: Hard,
    pub group: Group,
    /// The rule of each class, in the order of [`Class::ALL`].
    pub classes: [ClassRule; Class::ALL.len()],
    pub terrain: Terrain,
    /// The shortest and the longest that a sector of the corridor gets.
    pub corridor: (i32, i32),
    pub arena: Arena,
    pub room: Room,
    pub enemy: Enemy,
    pub unlock: Unlock,
}

/// What the levels of the stats give and cost. Both tables are by stat, then by level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    /// What a level gives: the speed of an engine and of a muzzle, the turn of tracks and of
    /// a turret in degrees per second, the reload of a loader and the cooldown of a shield.
    /// The level of the other stats is what they give, and they have nothing here.
    pub gives: [[f32; LEVELS]; Stat::COUNT],
    /// What a level of a piece of hardware costs in all, in points, and what a tier of
    /// software costs, in percent of the budget of its tank.
    pub costs: [[u8; LEVELS]; Stat::COUNT],
}

/// A single player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solo {
    /// The lives of a run, which are also the most a player has.
    pub lives: u32,
    /// How long after its tank was destroyed a player is back.
    pub respawn: f32,
}

/// What a tank does whatever its stats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chassis {
    /// The turn rate of the turret of a player, in radians per second.
    pub turret: f32,
    /// How long a tank takes to reach its top speed.
    pub speed_up: f32,
    /// A tank pivots in place while its hull is further than this from where it wants to
    /// go, in radians.
    pub pivot: f32,
    /// How long the tank of a player can't shoot after it appears.
    pub hold_fire: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gun {
    /// The time between two shots of a gun.
    pub cooldown: f32,
    /// The radius of a bullet.
    pub bullet: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mine {
    /// How long a mine takes to arm.
    pub arm: f32,
    /// How long after it was laid a mine goes off by itself.
    pub fuse: f32,
    /// How long a mine warns before it goes off.
    pub warn: f32,
    /// How close another tank than the one that laid it comes to set it off.
    pub trigger: f32,
    /// The time between two mines of a tank.
    pub every: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shield {
    /// How long the bubble lasts.
    pub lasts: f32,
    pub radius: f32,
}

/// A bullet that splits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Split {
    /// How long it flies before it becomes two.
    pub after: f32,
    /// How far to each side of its line of flight the two leave, in radians.
    pub angle: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Repair {
    /// How close to a wreck a tank repairs it.
    pub within: f32,
    pub takes: f32,
    /// How long nothing destroys a tank that was just repaired.
    pub safe: f32,
}

/// The budget of a sector: it goes from its first value toward its cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    pub first: u32,
    pub cap: u32,
    /// The sectors over which it covers 63% of what is left to its cap.
    pub every: u32,
}

/// The experience of the brains, which goes from 0 toward 1 along the corridor, and what it
/// changes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Experience {
    /// The sectors over which it covers 63% of what is left to 1.
    pub every: u32,
    /// The delay before a brain reacts: with no experience, and with all of it.
    pub reaction: (f32, f32),
    /// The largest aiming error, in degrees: with no experience, and with all of it.
    pub aim: (f32, f32),
}

/// Something that grows by steps along the corridor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// Its value in sector 1.
    pub first: u32,
    sectors: [u32; RISES],
    rises: u8,
}

impl Step {
    /// The sectors where it goes up by one, in rising order.
    pub const fn sectors(&self) -> &[u32] {
        self.sectors.split_at(self.rises as usize).0
    }

    /// Its value in a sector.
    pub const fn at(&self, sector: u32) -> u32 {
        let (mut value, mut index) = (self.first, 0);
        while index < self.rises as usize && self.sectors[index] <= sector {
            value += 1;
            index += 1;
        }
        value
    }

    /// The highest value that it gets.
    pub const fn last(&self) -> u32 {
        self.first + self.rises as u32
    }
}

/// What grows by steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Steps {
    /// The tanks of the group of a single player.
    pub tanks: Step,
    /// The highest tier of brain, and of ballistics.
    pub brain: Step,
    pub ballistics: Step,
    /// The highest level of Muzzle.
    pub muzzle: Step,
    /// The highest level of each other stat that has a step of its own, in the order of
    /// [`Stat::ALL`]: a stat without one has every level from the first sector.
    pub others: [Option<Step>; Stat::COUNT],
}

impl Steps {
    /// The step of a stat, if the balance gives it one.
    pub const fn of(&self, stat: Stat) -> Option<Step> {
        match stat {
            Stat::Brain => Some(self.brain),
            Stat::Ballistics => Some(self.ballistics),
            Stat::Muzzle => Some(self.muzzle),
            _ => self.others[stat as usize],
        }
    }
}

/// What hard mode adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hard {
    /// Points on top of the budget of every sector.
    pub budget: u32,
    /// Its brains are as experienced and its fastest guns as fast as they are this many
    /// sectors further in normal mode.
    pub ahead: u32,
}

/// How a group shares its points. The shares are in percent of the budget of its sector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Group {
    /// What a tank of a group has on average: a group draws its own between these two.
    pub mean: (i32, i32),
    /// The least that a tank has.
    pub least: u32,
    /// The most that the leader of a group has. The others stop at the budget of the sector.
    pub leader: u32,
    weights: [u32; WEIGHTS],
    drawn: u8,
    /// How much likelier the stats that a class favors are to receive the next point, all
    /// of them together.
    pub favorite: u32,
}

impl Group {
    /// The weights by which a group shares what is left once every tank has the least:
    /// each tank draws one.
    pub const fn weights(&self) -> &[u32] {
        self.weights.split_at(self.drawn as usize).0
    }
}

/// What the file says of a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassRule {
    /// Its weight when a tank draws its class.
    pub weight: u32,
    favorites: [Stat; 2],
    favored: u8,
    /// The highest level of Ricochet that it has, and the highest tier of ballistics.
    pub ricochet: u8,
    pub ballistics: u8,
}

impl ClassRule {
    /// The stats that it favors, the one it takes first in front.
    pub const fn favorites(&self) -> &[Stat] {
        self.favorites.split_at(self.favored as usize).0
    }

    /// Whether it favors a stat.
    pub const fn favors(&self, stat: Stat) -> bool {
        let mut index = 0;
        while index < self.favored as usize {
            if self.favorites[index] as usize == stat as usize {
                return true;
            }
            index += 1;
        }
        false
    }
}

/// The terrain. What gets harsher goes from its value in sector 1 to its value in the
/// harshest sector, and stays there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terrain {
    /// The sector from which the terrain is as harsh as it gets.
    pub harshest: u32,
    /// The narrowest that a corridor gets.
    pub width: (i32, i32),
    /// How much wider than its narrowest a corridor gets.
    pub spread: i32,
    /// The groups of cover blocks of a sector.
    pub cover: (i32, i32),
    /// The fewest and the most crates of a sector, in sector 1 and in the harshest.
    pub crates: ((i32, i32), (i32, i32)),
    /// The chance that a change of width is an angled wall, in percent.
    pub angled: (i32, i32),
    /// How far from the entry of their sector the enemies start at least, and how far from
    /// its barrier.
    pub spawn: (i32, i32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arena {
    /// An arena comes every so many sectors: the fewest and the most.
    pub every: (i32, i32),
    /// The tanks of its group, in percent of those of a corridor.
    pub tanks: u32,
    pub waves: u32,
    /// The time between the entry of the first player and the warning of the first wave.
    pub wait: f32,
    /// How long the place of a tank is marked before the tank appears.
    pub mark: f32,
    /// A mark waits while the tank of a player is this close to its place.
    pub clear: f32,
    /// How far inside its barrier an arena that is cleared leaves a life.
    pub life: i32,
    /// How close a tank comes to a life to take it.
    pub reach: f32,
}

/// The room of arena mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    /// The share of a group that the room holds at once, in percent.
    pub at_once: u32,
    /// The time before a group comes.
    pub lull: f32,
    /// A single player gets a life back for every so many groups cleared.
    pub life_every: u32,
    /// How long the place of a block or of a crate is drawn before it stands there.
    pub rise: f32,
    /// A tank doesn't appear this close to a tank of its group.
    pub apart: f32,
    /// Its groups of cover, in percent of those of a sector.
    pub cover: u32,
    /// The narrowest and the widest it gets, and the least and the most deep.
    pub width: (i32, i32),
    pub depth: (i32, i32),
}

/// How the brains fight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Enemy {
    /// The distances that a brain of tier 1 or more keeps from the player.
    pub keep: (f32, f32),
    /// A brain keeps its target until another player is this many times nearer.
    pub hold: f32,
    /// How far ahead a brain sees a bullet coming, in seconds.
    pub foresight: f32,
    /// How long before a bullet hits a soldier or a veteran raises its shield.
    pub last_moment: f32,
    /// The room that a veteran wants between its tank and a bullet to count on a miss.
    pub room: f32,
    /// A veteran fires at a bullet once its shot will pass this close to the middle of it.
    pub parry: f32,
    /// A brain lays mines by reflex when the player is this close.
    pub mine_reflex: f32,
    /// A veteran lays mines in the way of a player who is no further than this.
    pub mine_range: f32,
    /// A veteran lays no mine this close to another one.
    pub mine_apart: f32,
    /// A player who comes at a tank this fast charges it.
    pub charge: f32,
    /// The distances that an ace keeps from a player who has rounds.
    pub bait: (f32, f32),
    /// The distances that it keeps from a player who has none.
    pub press: (f32, f32),
    /// It presses a player until the player has this many rounds again.
    pub until: u8,
    /// The distances that it keeps when it is the last of its group.
    pub retreat: (f32, f32),
    /// The turn around the player that an ace wants between itself and the nearest ally, in
    /// radians.
    pub crossfire: f32,
    /// How far an enemy fires at a player, if the balance says: further than that, it holds
    /// its fire, and still knows where the player is. Without it, it fires however far.
    pub sight: Option<f32>,
}

/// The sector that a run must clear to unlock each mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unlock {
    pub arena: u32,
    pub seed: u32,
    pub free: u32,
    pub hard: u32,
    /// In hard mode.
    pub impossible: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fingerprint of the balance that `docs/PROJECT.md` describes.
    const SPECIFIED: u64 = 0xf411_4153_0081_323d;

    #[test]
    fn the_balance_of_the_game_is_the_one_that_the_specification_describes() {
        let game = fingerprint(GAME_FILE);
        assert!(
            game == SPECIFIED,
            "game.balance changed. Write its new numbers into docs/PROJECT.md, then set \
             SPECIFIED to {game:#018x} in tanku_core/src/balance.rs."
        );
    }

    #[test]
    fn the_tests_have_a_balance_of_their_own() {
        // Both files read, and a build for tests plays by its own unless it is asked for
        // the one of the game.
        assert_eq!(read(GAME_FILE), Ok(GAME));
        let tests = read(TEST_FILE).unwrap();
        if cfg!(feature = "game-balance") {
            assert_eq!((BALANCE, FINGERPRINT), (GAME, fingerprint(GAME_FILE)));
        } else {
            assert_eq!((BALANCE, FINGERPRINT), (tests, fingerprint(TEST_FILE)));
        }
        assert_eq!(
            FILE.1.ends_with("test.balance"),
            !cfg!(feature = "game-balance")
        );
    }
}
