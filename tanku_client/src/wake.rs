//! How a sector wakes.
//!
//! A sector that the first player enters takes a third of a second to wake, in the way that
//! its design calls the Lift. The light comes up. A plan takes its face, pale at first, as wet
//! ink, and is pulled up off the sheet, a little past its height and back, and marks flick out
//! from the corners of its foot when it lands. A tank is inked in from under its turret while
//! its tracks, its hull and its turret rise one after the other, and the beads of its antennas
//! ride up their masts last.
//!
//! Everything of a sector starts at once, when the first player is through its barrier. The
//! simulation knows nothing of it: the enemies are awake and the blocks are in the way from
//! the first tick. Only what was drawn asleep rises. A tank that appears later, in an arena or
//! in the room of arena mode, is built as it stands, and so are the blocks and the crates of
//! that room.
//!
//! This module holds the times. What is drawn at each moment is the doing of `scene`, for the
//! ground and what stands on it, and of `models`, for the tanks.

use bevy::prelude::*;

use crate::theme;

/// Seconds that a block or a crate takes to rise and to settle.
const RISE: f32 = 0.24;
/// How far past its height a block goes, as a share of that height, and a crate, which is
/// livelier.
const PAST: (f32, f32) = (0.08, 0.14);
/// The share of its rise during which the face of a block is paler than it will be.
const WET: f32 = 0.85;
/// Seconds that the marks at the corners of a foot take to flick out and fade.
const MARKS: f32 = 0.16;
/// Seconds that the light of a sector takes to come up.
const LIGHT: f32 = 0.2;
/// When the ink starts to spread over a tank, and how long it takes.
const INK: (f32, f32) = (0.015, 0.11);
/// When each part of a tank starts to rise, and how long it takes: its tracks, its hull, its
/// turret, and its antennas. The base of a tank that can't move rises with its hull.
const GEAR: (f32, f32) = (0.04, 0.15);
const BODY: (f32, f32) = (0.07, 0.15);
const TURRET: (f32, f32) = (0.10, 0.15);
const MASTS: (f32, f32) = (0.17, 0.13);
/// How far past their height the parts of a tank go, and its antennas.
const TANK_PAST: (f32, f32) = (0.04, 0.12);
/// Seconds after which nothing of a sector that wakes still moves.
pub const LENGTH: f32 = 0.32;

/// A moment at which everything that wakes stays, in seconds since it woke. A script sets
/// it, to look at that moment. Without one, what wakes takes its time and is done.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Hold(pub Option<f32>);

impl Hold {
    /// For how long something has been waking, a frame of this many seconds after it had been
    /// for `age`.
    pub fn later(&self, age: f32, seconds: f32) -> f32 {
        self.0.unwrap_or(age + seconds)
    }

    /// Whether what has been waking for this long is done. What a script holds never is.
    pub fn over(&self, age: f32) -> bool {
        self.0.is_none() && age >= LENGTH
    }
}

/// A share of a way that starts fast and ends slowly.
pub fn ease_out(share: f32) -> f32 {
    1.0 - (1.0 - share.clamp(0.0, 1.0)).powi(3)
}

/// The pull of a way that goes past its end by `past` and comes back. It is the one number of
/// [`back`], found by halving: the further past, the harder the pull.
fn pull(past: f32) -> f32 {
    if past <= 0.0 {
        return 0.0;
    }
    let (mut low, mut high) = (0.0_f32, 8.0_f32);
    for _ in 0..32 {
        let middle = (low + high) / 2.0;
        if 4.0 * middle.powi(3) / (27.0 * (middle + 1.0).powi(2)) < past {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) / 2.0
}

/// A share of a way that goes past its end by `past`, as a share of the way, and comes back
/// to it: how a solid is pulled up off the sheet.
fn back(share: f32, past: f32) -> f32 {
    let (pull, left) = (pull(past), share.clamp(0.0, 1.0) - 1.0);
    1.0 + (pull + 1.0) * left.powi(3) + pull * left.powi(2)
}

/// The share of such a way after which it first reaches its end, before it goes past it.
fn reached(past: f32) -> f32 {
    1.0 / (pull(past) + 1.0)
}

/// A part that starts at a moment and takes some time, this long after its sector woke.
fn part(age: f32, (start, length): (f32, f32), past: f32) -> f32 {
    back((age - start) / length, past)
}

/// How much of its light a sector has, this long after it woke.
pub fn light(age: f32) -> f32 {
    theme::DORMANT + (1.0 - theme::DORMANT) * ease_out(age / LIGHT)
}

/// A block or a crate at a moment of its rise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rise {
    /// Its height, as a share of the height it will have. It goes a little past 1.
    pub height: f32,
    /// How wet the ink of its face still is, from 1 to 0.
    pub wet: f32,
    /// How far the marks at the corners of its foot have flicked out, from 0 to 1, while they
    /// show.
    pub marks: Option<f32>,
}

/// How far past its height a block goes, or a crate.
fn past(explosive: bool) -> f32 {
    if explosive { PAST.1 } else { PAST.0 }
}

/// Seconds after which a block that wakes is first at its height, or a crate: when it lands.
pub fn landing(explosive: bool) -> f32 {
    RISE * reached(past(explosive))
}

/// A block, or a crate, this long after its sector woke.
pub fn rise(age: f32, explosive: bool) -> Rise {
    let flicked = (age - landing(explosive)) / MARKS;
    Rise {
        height: back(age / RISE, past(explosive)),
        wet: (1.0 - age / (WET * RISE)).clamp(0.0, 1.0),
        marks: (0.0..=1.0).contains(&flicked).then_some(flicked),
    }
}

/// A tank at a moment of its rise. Each height is a share of the height that the part will
/// have, and goes a little past 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stand {
    /// How far the ink has spread from under the turret, from 0 to 1.
    pub ink: f32,
    /// Its tracks.
    pub gear: f32,
    pub body: f32,
    pub turret: f32,
    /// How far the beads of its antennas have ridden up their masts.
    pub masts: f32,
}

/// A tank this long after its sector woke.
pub fn stand(age: f32) -> Stand {
    Stand {
        ink: ease_out((age - INK.0) / INK.1),
        gear: part(age, GEAR, TANK_PAST.0),
        body: part(age, BODY, TANK_PAST.0),
        turret: part(age, TURRET, TANK_PAST.0),
        masts: part(age, MASTS, TANK_PAST.1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(found: f32, wanted: f32) -> bool {
        (found - wanted).abs() < 1e-3
    }

    /// Every sixtieth of a second of a wake, and a little after it.
    fn frames() -> impl Iterator<Item = f32> {
        (0..=24).map(|frame| frame as f32 / 60.0)
    }

    #[test]
    fn a_way_past_its_end_goes_as_far_as_asked_and_comes_back() {
        for past in [0.0, 0.04, 0.08, 0.14] {
            let shares = (0..=1000).map(|step| back(step as f32 / 1000.0, past));
            let furthest = shares.fold(0.0, f32::max);
            assert!(close(furthest, 1.0 + past), "{past}: {furthest}");
            assert!(close(back(0.0, past), 0.0) && close(back(1.0, past), 1.0));
            // It is at its end for the first time when it is said to be, and past it after.
            let first = reached(past);
            assert!(close(back(first, past), 1.0), "{past}");
            assert!(past == 0.0 || back((first + 1.0) / 2.0, past) > 1.0);
        }
        // Before its start and after its end, it stays where it is.
        assert!(close(back(-1.0, 0.08), 0.0) && close(back(2.0, 0.08), 1.0));
    }

    #[test]
    fn a_block_lands_a_tenth_of_a_second_after_it_wakes() {
        // Cover is not half there for long: a bullet of the fastest gun, from the nearest
        // place where an enemy starts, needs eight times as long.
        assert!(close(landing(false), 0.096));
        assert!(landing(true) < landing(false));
        let block = rise(landing(false), false);
        assert!(close(block.height, 1.0) && block.marks == Some(0.0));
        // It starts flat, with a face that is as wet as it gets and no mark yet.
        let flat = rise(0.0, false);
        assert!(close(flat.height, 0.0) && close(flat.wet, 1.0) && flat.marks.is_none());
    }

    #[test]
    fn a_crate_goes_further_past_its_height_than_a_block() {
        let highest = |explosive: bool| {
            let heights = frames().map(|age| rise(age, explosive).height);
            heights.fold(0.0, f32::max)
        };
        let (block, explosive) = (highest(false), highest(true));
        assert!(block > 1.05 && block < 1.09, "{block}");
        assert!(explosive > 1.10 && explosive < 1.15, "{explosive}");
    }

    #[test]
    fn the_parts_of_a_tank_rise_one_after_the_other() {
        // A twentieth of a second in, the ink spreads and nothing has left the floor.
        let early = stand(0.03);
        assert!(early.ink > 0.0 && early.ink < 1.0);
        assert!(close(early.gear, 0.0) && close(early.turret, 0.0));
        // Each part is ahead of the one that it carries, while they are on their way up.
        for age in frames().filter(|age| (0.08..0.16).contains(age)) {
            let tank = stand(age);
            assert!(tank.gear > tank.body && tank.body > tank.turret, "{age}");
            assert!(tank.turret >= tank.masts, "{age}");
        }
        // The ink is all there while the turret is still on its way.
        let inked = stand(INK.0 + INK.1);
        assert!(close(inked.ink, 1.0) && inked.turret < 0.6 && close(inked.masts, 0.0));
    }

    #[test]
    fn nothing_moves_once_the_wake_is_over() {
        for age in [LENGTH, LENGTH + 0.1, 10.0] {
            for explosive in [false, true] {
                let stood = rise(age, explosive);
                assert!(close(stood.height, 1.0) && stood.wet == 0.0);
                assert!(stood.marks.is_none(), "{age}");
            }
            let tank = stand(age);
            let parts = [tank.ink, tank.gear, tank.body, tank.turret, tank.masts];
            assert!(parts.iter().all(|part| close(*part, 1.0)), "{age}");
            assert!(close(light(age), 1.0));
        }
        // The light starts from that of a sector that waits.
        assert!(close(light(0.0), theme::DORMANT));
    }

    #[test]
    fn a_script_holds_one_moment_of_a_wake() {
        let running = Hold::default();
        assert!(close(running.later(0.1, 0.016), 0.116));
        assert!(!running.over(0.1) && running.over(LENGTH));
        let held = Hold(Some(0.06));
        assert!(close(held.later(0.1, 0.016), 0.06) && close(held.later(5.0, 0.016), 0.06));
        assert!(!held.over(LENGTH + 1.0));
    }
}
