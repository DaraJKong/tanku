//! How difficulty grows with the sector number, in normal mode and in hard mode.
//!
//! Everything that feeds generation is an integer formula or a lookup table, so that a seed
//! gives the same enemies on every platform.

#[cfg(test)]
use crate::balance::Budget;
use crate::{modes::Difficulty, rules::ROOM_SHARE};

/// What hard mode adds to the budget of every sector: its budgets go from 40 to 130.
const HARD_BUDGET: u32 = 30;
/// In hard mode, brains are as experienced and the fastest guns as fast as they are this
/// many sectors further in normal mode.
const HARD_LEAD: u32 = 20;

/// `round(100 - 90 * exp(-(n - 1) / 15))` for sectors 1 to 79. It stays at 100 afterward.
const SECTOR_BUDGETS: [u8; 79] = [
    10, 16, 21, 26, 31, 36, 40, 44, 47, 51, 54, 57, 60, 62, 65, 67, 69, 71, 73, 75, 76, 78, 79, 81,
    82, 83, 84, 85, 86, 87, 88, 89, 89, 90, 91, 91, 92, 92, 93, 93, 94, 94, 95, 95, 95, 96, 96, 96,
    96, 97, 97, 97, 97, 97, 98, 98, 98, 98, 98, 98, 98, 98, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 100,
];

/// How many sectors have a budget of their own. The sectors after them have the last one.
#[cfg(test)]
const TABLED: usize = 256;

/// `exp(-x)` for a number that isn't negative. It is worked out while the game is compiled,
/// for the table of the budgets, so its result is the same whatever machine compiles it.
#[cfg(test)]
const fn decay(x: f64) -> f64 {
    // The power is halved until a short series is exact, and squared back as often.
    let (mut small, mut halvings) = (x, 0);
    while small > 0.5 {
        small /= 2.0;
        halvings += 1;
    }
    let (mut sum, mut term, mut index) = (1.0, 1.0, 1);
    while index <= 20 {
        term *= small / index as f64;
        sum += term;
        index += 1;
    }
    while halvings > 0 {
        sum *= sum;
        halvings -= 1;
    }
    1.0 / sum
}

/// The budget of each sector, from the first one on: it goes from its first value toward its
/// cap, by 63% of what is left every so many sectors.
#[cfg(test)]
const fn budgets(budget: Budget) -> [u16; TABLED] {
    let mut table = [0; TABLED];
    let mut index = 0;
    while index < TABLED {
        let left = (budget.cap - budget.first) as f64 * decay(index as f64 / budget.every as f64);
        // Rounded to the nearest point.
        table[index] = (budget.cap as f64 - left + 0.5) as u16;
        index += 1;
    }
    table
}

/// Sector from which the terrain is as harsh as it gets.
const HARSHEST_SECTOR: u32 = 30;

/// The sector of normal mode whose experience and whose fastest gun the enemies of a sector
/// have.
fn ahead(sector: u32, difficulty: Difficulty) -> u32 {
    match difficulty {
        Difficulty::Normal => sector,
        Difficulty::Hard => sector + HARD_LEAD,
    }
}

/// The most points an enemy of this sector can have, unless it leads its group. Sectors start
/// at 1. In hard mode, the budgets follow the same curve from 40 to 130: each is 30 above the
/// one of normal mode.
pub fn sector_budget(sector: u32, difficulty: Difficulty) -> u32 {
    let index = (sector.max(1) as usize - 1).min(SECTOR_BUDGETS.len() - 1);
    let more = match difficulty {
        Difficulty::Normal => 0,
        Difficulty::Hard => HARD_BUDGET,
    };
    SECTOR_BUDGETS[index] as u32 + more
}

/// Number of enemies in a sector.
pub fn enemy_count(sector: u32, players: u32, arena: bool) -> u32 {
    let solo = (1 + sector / 3).min(6);
    // `solo * (1 + 0.5 * (players - 1))`, rounded up.
    let team = (solo * (players + 1)).div_ceil(2);
    if arena { (team * 3).div_ceil(2) } else { team }
}

/// How many tanks each wave of a group has, for a group of this size that comes in this many
/// waves at most: each wave is as large as the others or one tank larger, the larger ones
/// first. A group that is smaller than the number of waves comes one tank at a time.
pub fn wave_sizes(count: u32, waves: u32) -> Vec<u32> {
    let waves = waves.min(count);
    (0..waves)
        .map(|wave| count / waves + (wave < count % waves) as u32)
        .collect()
}

/// How many tanks of a group of arena mode come with it, and how many wait in reserve, each
/// to take the place of one that is destroyed. The first are half of the group, rounded up:
/// the room holds no more of a group at once, which is as many tanks as a wave of an arena
/// has.
pub fn room_parts(count: u32) -> (u32, u32) {
    let first = count.div_ceil(ROOM_SHARE);
    (first, count - first)
}

/// Highest brain tier an enemy of this sector can have. Hard mode has every tier from the
/// first sector.
pub fn brain_cap(sector: u32, difficulty: Difficulty) -> u8 {
    match difficulty {
        Difficulty::Normal => (1 + (sector.max(1) - 1) / 5).min(4) as u8,
        Difficulty::Hard => 4,
    }
}

/// Highest ballistics tier an enemy of this sector can have. Hard mode has every tier from
/// the first sector.
pub fn ballistics_cap(sector: u32, difficulty: Difficulty) -> u8 {
    match difficulty {
        Difficulty::Normal => ((sector + 2) / 5).min(3) as u8,
        Difficulty::Hard => 3,
    }
}

/// Highest level of Muzzle an enemy of this sector can have: the level of the players' own
/// tank at first, then one more every five sectors. Hard mode has the guns of twenty sectors
/// further.
pub fn muzzle_cap(sector: u32, difficulty: Difficulty) -> u8 {
    (2 + (ahead(sector, difficulty) + 1) / 5).min(7) as u8
}

/// How well brains execute, from 0 for a novice toward 1 for a flawless brain. In hard mode,
/// they execute as they do twenty sectors further.
///
/// Experience drives behavior only, never generation, so floating point is fine here.
pub fn experience(sector: u32, difficulty: Difficulty) -> f32 {
    1.0 - (-(ahead(sector.max(1), difficulty) as f32 - 1.0) / 20.0).exp()
}

/// A value that goes from `easy` in sector 1 to `harsh` in sector 30 and stays there.
fn harshness(sector: u32, easy: i32, harsh: i32) -> i32 {
    let step = (sector.clamp(1, HARSHEST_SECTOR) - 1) as i32;
    easy + (harsh - easy) * step / (HARSHEST_SECTOR as i32 - 1)
}

/// Narrowest and widest the corridor gets in this sector, in meters.
pub fn corridor_width(sector: u32) -> (i32, i32) {
    let narrowest = harshness(sector, 14, 8);
    (narrowest, narrowest + 6)
}

/// Groups of cover blocks placed in a sector.
pub fn cover_groups(sector: u32) -> u32 {
    harshness(sector, 9, 3) as u32
}

/// Fewest and most explosive crates in a sector.
pub fn crate_count(sector: u32) -> (i32, i32) {
    (harshness(sector, 0, 3), harshness(sector, 1, 6))
}

/// Chance, in percent, that a change of corridor width is an angled wall and not a square step.
pub fn angled_wall_chance(sector: u32) -> u32 {
    harshness(sector, 10, 70) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_group_of_an_arena_comes_in_waves_of_about_the_same_size() {
        let wave_sizes = |count: u32| wave_sizes(count, ARENA_WAVES);
        assert_eq!(wave_sizes(9), [3, 3, 3]);
        assert_eq!(wave_sizes(8), [3, 3, 2]);
        assert_eq!(wave_sizes(5), [2, 2, 1]);
        assert_eq!(wave_sizes(23), [8, 8, 7]);
        // Fewer tanks than waves come one at a time, and nobody comes to an empty arena.
        assert_eq!(wave_sizes(2), [1, 1]);
        assert_eq!(wave_sizes(1), [1]);
        assert!(wave_sizes(0).is_empty());
        for count in 0..40 {
            assert_eq!(wave_sizes(count).iter().sum::<u32>(), count);
        }
    }

    #[test]
    fn the_room_of_arena_mode_holds_as_much_of_a_group_as_a_wave_of_an_arena_has() {
        assert_eq!(room_parts(6), (3, 3));
        assert_eq!(room_parts(5), (3, 2));
        assert_eq!(room_parts(15), (8, 7));
        // A tank that is alone has nobody to take its place.
        assert_eq!(room_parts(1), (1, 0));
        assert_eq!(room_parts(0), (0, 0));
        // The group of a sector, of which half is there at once, and the group of an arena,
        // which is half as large again and comes in three waves: the most tanks that the
        // players face together are the same. And nobody waits longer than a tank is there
        // to be destroyed for them.
        for sector in 1..=40 {
            for players in 1..=4 {
                let count = enemy_count(sector, players, false);
                let (first, held) = room_parts(count);
                let arena = wave_sizes(enemy_count(sector, players, true), ARENA_WAVES);
                assert_eq!(first, arena[0], "sector {sector}, {players} players");
                assert!(first + held == count && held <= first);
            }
        }
    }

    use crate::rules::ARENA_WAVES;
    use Difficulty::{Hard, Normal};

    #[test]
    fn budget_table_follows_the_formula() {
        // The budgets of a mode go from the first of them toward their cap.
        for (difficulty, first, cap) in [(Normal, 10.0, 100.0), (Hard, 40.0, 130.0)] {
            for sector in 1..=120u32 {
                let exact = cap - (cap - first) * (-(sector as f64 - 1.0) / 15.0).exp();
                assert_eq!(
                    sector_budget(sector, difficulty),
                    exact.round() as u32,
                    "sector {sector} in {difficulty:?}"
                );
            }
        }
    }

    #[test]
    fn hard_mode_has_every_tier_at_once_and_what_comes_twenty_sectors_further() {
        assert_eq!(sector_budget(1, Hard), 40);
        assert_eq!(sector_budget(1000, Hard), 130);
        for sector in 1..=100u32 {
            // The best brain and the best aim of normal mode, which stop growing there.
            assert_eq!(brain_cap(sector, Hard), brain_cap(1000, Normal));
            assert_eq!(ballistics_cap(sector, Hard), ballistics_cap(1000, Normal));
            assert_eq!(muzzle_cap(sector, Hard), muzzle_cap(sector + 20, Normal));
            assert_eq!(experience(sector, Hard), experience(sector + 20, Normal));
            // It is harder than normal mode in every sector, and gets harder itself.
            assert!(sector_budget(sector, Hard) > sector_budget(sector, Normal));
            assert!(experience(sector, Hard) > experience(sector, Normal));
            assert!(experience(sector + 1, Hard) > experience(sector, Hard));
            // The number of enemies is the one of normal mode: `enemy_count` knows no mode.
        }
        // A brain of its first sector reacts about twice as fast as one of normal mode.
        assert!((experience(1, Hard) - 0.63).abs() < 0.006);
        // Its guns are the fastest but one in the first three sectors, and the fastest then.
        let muzzles: Vec<u8> = (1..=5).map(|sector| muzzle_cap(sector, Hard)).collect();
        assert_eq!(muzzles, [6, 6, 6, 7, 7]);
    }

    #[test]
    fn progression_matches_the_specification_table() {
        // Sector, budget, enemies in solo, brain cap, ballistics cap, Muzzle cap, experience.
        let table = [
            (1, 10, 1, 1, 0, 2, 0.00),
            (3, 21, 2, 1, 1, 2, 0.10),
            (5, 31, 2, 1, 1, 3, 0.18),
            (10, 51, 4, 2, 2, 4, 0.36),
            (15, 65, 6, 3, 3, 5, 0.50),
            (20, 75, 6, 4, 3, 6, 0.61),
            (30, 87, 6, 4, 3, 7, 0.77),
            (50, 97, 6, 4, 3, 7, 0.91),
        ];
        for (sector, budget, enemies, brain, ballistics, muzzle, skill) in table {
            assert_eq!(sector_budget(sector, Normal), budget);
            assert_eq!(enemy_count(sector, 1, false), enemies);
            assert_eq!(brain_cap(sector, Normal), brain);
            assert_eq!(ballistics_cap(sector, Normal), ballistics);
            assert_eq!(muzzle_cap(sector, Normal), muzzle);
            assert!(
                (experience(sector, Normal) - skill).abs() < 0.006,
                "sector {sector}"
            );
        }
    }

    #[test]
    fn the_fastest_gun_starts_at_the_players_own_and_gains_a_level_every_five_sectors() {
        use crate::stats::{Genome, Stat};
        assert_eq!(muzzle_cap(1, Normal), Genome::PLAYER.level(Stat::Muzzle));
        let steps: Vec<u32> = (2..=100)
            .filter(|&sector| muzzle_cap(sector, Normal) > muzzle_cap(sector - 1, Normal))
            .collect();
        assert_eq!(steps, [4, 9, 14, 19, 24]);
        assert_eq!(muzzle_cap(24, Normal), Stat::Muzzle.max_level());
        assert_eq!(muzzle_cap(1000, Normal), Stat::Muzzle.max_level());
        assert_eq!(muzzle_cap(1000, Hard), Stat::Muzzle.max_level());
    }

    #[test]
    fn enemy_groups_grow_with_players_and_arenas() {
        assert_eq!(enemy_count(15, 2, false), 9);
        assert_eq!(enemy_count(15, 4, false), 15);
        assert_eq!(enemy_count(1, 2, false), 2);
        assert_eq!(enemy_count(15, 1, true), 9);
        assert_eq!(enemy_count(3, 1, true), 3);
    }

    #[test]
    fn terrain_gets_harsher_until_sector_30() {
        assert_eq!(corridor_width(1), (14, 20));
        assert_eq!(corridor_width(30), (8, 14));
        assert_eq!(corridor_width(90), (8, 14));
        assert_eq!(crate_count(1), (0, 1));
        assert_eq!(crate_count(30), (3, 6));
        assert!(cover_groups(1) > cover_groups(30));
        assert!(angled_wall_chance(1) < angled_wall_chance(30));
    }

    #[test]
    fn the_balance_file_holds_the_numbers_of_the_progression() {
        let balance = crate::balance::BALANCE;
        // The table that is worked out from the three numbers of the budget is the one that
        // was written by hand.
        let table = budgets(balance.budget);
        for (index, budget) in table.iter().enumerate() {
            let written = SECTOR_BUDGETS[index.min(SECTOR_BUDGETS.len() - 1)];
            assert_eq!(*budget, written as u16, "sector {}", index + 1);
        }
        assert_eq!(
            (balance.hard.budget, balance.hard.ahead),
            (HARD_BUDGET, HARD_LEAD)
        );
        assert_eq!(balance.terrain.harshest, HARSHEST_SECTOR);
        let (steps, terrain) = (balance.steps, balance.terrain);
        for sector in 0..=300 {
            // What grows by steps.
            let solo = steps.tanks.at(sector);
            assert_eq!(solo, enemy_count(sector, 1, false), "sector {sector}");
            assert_eq!(steps.brain.at(sector) as u8, brain_cap(sector, Normal));
            let ballistics = steps.ballistics.at(sector) as u8;
            assert_eq!(ballistics, ballistics_cap(sector, Normal));
            assert_eq!(steps.muzzle.at(sector) as u8, muzzle_cap(sector, Normal));
            let ahead = steps.muzzle.at(sector + balance.hard.ahead) as u8;
            assert_eq!(ahead, muzzle_cap(sector, Hard));
            // What the players and an arena add.
            for players in 1..=4 {
                let team = (solo * (100 + balance.team * (players - 1))).div_ceil(100);
                assert_eq!(team, enemy_count(sector, players, false));
                let arena = (team * balance.arena.tanks).div_ceil(100);
                assert_eq!(arena, enemy_count(sector, players, true));
            }
            // The terrain.
            let slope = |(easy, harsh): (i32, i32)| harshness(sector, easy, harsh);
            let narrowest = slope(terrain.width);
            let width = (narrowest, narrowest + terrain.spread);
            assert_eq!(width, corridor_width(sector));
            assert_eq!(slope(terrain.cover) as u32, cover_groups(sector));
            let (first, last) = terrain.crates;
            let crates = (slope((first.0, last.0)), slope((first.1, last.1)));
            assert_eq!(crates, crate_count(sector));
            assert_eq!(slope(terrain.angled) as u32, angled_wall_chance(sector));
        }
        // Hard mode has every tier at once, which is where the steps end.
        assert_eq!(steps.brain.last() as u8, brain_cap(1, Hard));
        assert_eq!(steps.ballistics.last() as u8, ballistics_cap(1, Hard));
        // Experience.
        let every = balance.experience.every as f32;
        for sector in 1..=200u32 {
            let written = 1.0 - (-(sector as f32 - 1.0) / every).exp();
            assert_eq!(written, experience(sector, Normal), "sector {sector}");
        }
    }
}
