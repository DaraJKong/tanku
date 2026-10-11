//! Generation of enemy tanks.
//!
//! The enemies of a sector are a group. The group has a budget, which its tanks share
//! unevenly, so that weak tanks stand next to strong ones. Each tank has a class, which
//! decides what it spends its share on.

use crate::{
    progression,
    rng::{Purpose, Stream},
    stats::{Class, Genome, Stat},
};

/// How much likelier the stats that a class favors are to receive the next point, all of
/// them together: a class with one favorite stat is as set on it as another on its two.
const FAVORITE_WEIGHT: u32 = 20;
/// What a tank of a group has on average, in percent of the sector budget: a group draws
/// its own between these two.
const GROUP_SHARE: (i32, i32) = (70, 90);
/// The least a tank has, in percent of the sector budget.
const LEAST_SHARE: u32 = 30;
/// The most that the strongest tank of a group can have, in percent of the sector budget.
/// The others stop at the sector budget.
const LEADER_SHARE: u32 = 150;
/// The weights by which a group shares what is left once every tank has the least: each tank
/// draws one. Most are light, and now and then one is heavy.
const WEIGHTS: [u32; 6] = [1, 1, 2, 2, 3, 8];

/// The highest level of a stat that an enemy of a class can have in a sector.
fn highest_level(stat: Stat, sector: u32, class: Class) -> u8 {
    let allowed = match stat {
        // Fast bullets, good brains and good aim wait for later sectors.
        Stat::Muzzle => progression::muzzle_cap(sector),
        Stat::Brain => progression::brain_cap(sector),
        Stat::Ballistics => progression::ballistics_cap(sector),
        _ => stat.max_level(),
    };
    allowed.min(class.highest(stat))
}

/// The budgets of the tanks of a group, which depend on the seed, the sector and the size of
/// the group. Together they are what so many tanks have on average. The strongest may have
/// more than the sector budget, and no other does.
pub fn budgets(seed: u64, sector: u32, count: u32) -> Vec<u32> {
    let mut rng = Stream::new(seed, Purpose::Group, sector, count);
    let cap = progression::sector_budget(sector);
    let share = rng.range(GROUP_SHARE.0, GROUP_SHARE.1) as u32;
    let total = (count * cap * share + 50) / 100;
    let least = (cap * LEAST_SHARE).div_ceil(100);
    if count <= 1 {
        return vec![total.max(least); count as usize];
    }
    let weights: Vec<u32> = (0..count).map(|_| rng.pick(&WEIGHTS)).collect();
    let sum: u32 = weights.iter().sum();
    // The first of the heaviest leads the group.
    let heaviest = weights.iter().copied().max().unwrap_or(0);
    let leader = weights.iter().position(|&weight| weight == heaviest);
    let left = total - count * least;
    let mut budgets: Vec<u32> = weights
        .iter()
        .map(|weight| least + left * weight / sum)
        .collect();
    let limit = |index: usize| {
        if Some(index) == leader {
            cap * LEADER_SHARE / 100
        } else {
            cap
        }
    };
    // What the rounding left over goes to the leader, and what is over a tank's limit to
    // the tanks after it.
    let mut spare = total - budgets.iter().sum::<u32>();
    for index in leader.into_iter().chain(0..budgets.len()) {
        let room = limit(index).saturating_sub(budgets[index]);
        let over = budgets[index].saturating_sub(limit(index));
        budgets[index] = budgets[index] - over + room.min(spare);
        spare = spare - room.min(spare) + over;
    }
    for (index, budget) in budgets.iter_mut().enumerate() {
        let room = limit(index) - *budget;
        *budget += room.min(spare);
        spare -= room.min(spare);
    }
    budgets
}

/// The genomes of the group of a sector, which depend only on the seed, the sector and the
/// size of the group.
pub fn group(seed: u64, sector: u32, count: u32) -> Vec<Genome> {
    let budgets = budgets(seed, sector, count);
    let tanks = budgets.iter().enumerate();
    tanks
        .map(|(index, &budget)| build(seed, sector, index as u32, budget))
        .collect()
}

/// One tank of a group: its class, then what its budget buys.
fn build(seed: u64, sector: u32, index: u32, budget: u32) -> Genome {
    let mut rng = Stream::new(seed, Purpose::Enemy, sector, index);
    let class = rng.pick(&Class::ALL);
    let mut genome = Genome::empty(budget as u16);
    genome.class = class;
    let highest = |stat: Stat| highest_level(stat, sector, class);

    // A tactician takes the best brain and the best aim that its sector allows. A sapper
    // needs a brain for its mines and its shield. The others take what they draw.
    let brain = match class {
        Class::Tactician => highest(Stat::Brain),
        Class::Sapper => 1 + rng.below(highest(Stat::Brain) as u32) as u8,
        _ => rng.below(highest(Stat::Brain) as u32 + 1) as u8,
    };
    genome.set(Stat::Brain, brain);
    let mut ballistics = match class {
        Class::Tactician => highest(Stat::Ballistics),
        _ => rng.below(highest(Stat::Ballistics) as u32 + 1) as u8,
    };
    // A bank shot needs a bullet that bounces. The tier drops until its ricochet is affordable.
    loop {
        genome.set(Stat::Ballistics, ballistics);
        let ricochet = ballistics.saturating_sub(1);
        if genome.software_cost() + Stat::Ricochet.price(ricochet) <= budget {
            genome.set(Stat::Ricochet, ricochet);
            break;
        }
        ballistics -= 1;
    }

    // A tank without a brain can't use mines or a shield.
    let wanted: Vec<Stat> = Stat::HARDWARE
        .into_iter()
        .filter(|stat| brain > 0 || !matches!(stat, Stat::Mines | Stat::Shield))
        .collect();
    let affordable = |genome: &Genome, stat: Stat| {
        let level = genome.level(stat);
        level < highest(stat)
            && stat.price(level + 1) - stat.price(level) <= budget - genome.cost()
            && (stat != Stat::Tracks || genome.level(Stat::Engine) > 0)
    };
    // What makes its class comes first: a tank has a level of each stat that it favors, if
    // it can pay for it.
    let favorites: Vec<Stat> = class
        .favorites()
        .iter()
        .copied()
        .filter(|stat| wanted.contains(stat))
        .collect();
    for &stat in &favorites {
        if genome.level(stat) == 0 && affordable(&genome, stat) {
            genome.set(stat, 1);
        }
    }

    loop {
        let open: Vec<Stat> = wanted
            .iter()
            .copied()
            .filter(|&stat| affordable(&genome, stat))
            .collect();
        if open.is_empty() {
            return genome;
        }
        let favored = open.iter().filter(|stat| favorites.contains(stat)).count() as u32;
        let weights: Vec<u32> = open
            .iter()
            .map(|stat| {
                if favorites.contains(stat) {
                    FAVORITE_WEIGHT / favored
                } else {
                    1
                }
            })
            .collect();
        let stat = open[rng.weighted(&weights)];
        genome.set(stat, genome.level(stat) + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Groups of every size up to nine, over six seeds and sixty sectors: each tank with its
    /// sector.
    fn groups() -> impl Iterator<Item = (u32, Vec<Genome>)> {
        (0..6u64).flat_map(|seed| {
            (1..=60u32).map(move |sector| (sector, group(seed, sector, 1 + (sector + 3) % 9)))
        })
    }

    fn sample() -> impl Iterator<Item = (u32, Genome)> {
        groups().flat_map(|(sector, tanks)| tanks.into_iter().map(move |tank| (sector, tank)))
    }

    #[test]
    fn same_inputs_same_group() {
        assert_eq!(group(42, 12, 4), group(42, 12, 4));
        assert_ne!(group(42, 12, 4), group(43, 12, 4));
        assert_ne!(group(42, 12, 4), group(42, 13, 4));
        // A tank keeps its class whatever the size of its group: only its share changes.
        let (four, six) = (group(42, 12, 4), group(42, 12, 6));
        for (small, large) in four.iter().zip(&six) {
            assert_eq!(small.class, large.class);
        }
    }

    #[test]
    fn generation_is_stable() {
        // Golden value, as for the map: it must never change by accident.
        let tanks = group(crate::seed::seed_value("TANKU"), 20, 6);
        assert_eq!(
            format!("{:?}", tanks[0]),
            "Genome { levels: [3, 6, 6, 6, 5, 4, 1, 0, 0, 0, 0, 0], budget: 64, class: None }"
        );
        let classes: Vec<Class> = tanks.iter().map(|tank| tank.class).collect();
        assert_eq!(format!("{classes:?}"), CLASSES);
        let budgets: Vec<u16> = tanks.iter().map(|tank| tank.budget).collect();
        assert_eq!(format!("{budgets:?}"), BUDGETS);
    }

    const CLASSES: &str = "[None, Tactician, Tactician, None, Lancer, Sapper]";
    const BUDGETS: &str = "[64, 112, 35, 48, 35, 48]";

    #[test]
    fn a_group_shares_its_budget_unevenly() {
        let (mut ratios, mut led) = (Vec::new(), 0);
        for (sector, tanks) in groups() {
            let cap = progression::sector_budget(sector);
            let count = tanks.len() as u32;
            let total: u32 = tanks.iter().map(|tank| tank.budget as u32).sum();
            // Together, the tanks have what so many have on average.
            let (low, high) = (GROUP_SHARE.0 as u32, GROUP_SHARE.1 as u32);
            assert!(
                total * 100 + 50 >= count * cap * low && total * 100 <= count * cap * high + 50,
                "sector {sector}: {total} for {count} tanks"
            );
            // None is weaker than the least, one at most is stronger than the sector budget,
            // and that one stops at one and a half times that budget.
            let above = tanks.iter().filter(|tank| tank.budget as u32 > cap).count();
            assert!(above <= 1, "sector {sector}: {above} tanks above {cap}");
            for tank in &tanks {
                let budget = tank.budget as u32;
                assert!(
                    budget * 100 >= cap * LEAST_SHARE,
                    "sector {sector}: {budget}"
                );
                assert!(
                    budget * 100 <= cap * LEADER_SHARE,
                    "sector {sector}: {budget}"
                );
                assert!(tank.cost() <= budget, "sector {sector}: overspent");
            }
            led += above;
            if count == 6 {
                let budgets = tanks.iter().map(|tank| tank.budget as f32);
                let (weakest, strongest) = (
                    budgets.clone().fold(f32::MAX, f32::min),
                    budgets.fold(0.0, f32::max),
                );
                ratios.push(strongest / weakest);
            }
        }
        // In a group of six, the strongest tank has well over twice what the weakest has.
        let ratio = ratios.iter().sum::<f32>() / ratios.len() as f32;
        assert!(ratio > 2.2, "{ratio}");
        assert!(
            led > 100,
            "{led} groups have a tank above the sector budget"
        );
    }

    #[test]
    fn points_are_spent() {
        // A tank stops buying when it can afford nothing it is allowed to have.
        for (sector, genome) in sample() {
            let left = genome.budget as u32 - genome.cost();
            for stat in Stat::HARDWARE {
                let level = genome.level(stat);
                let allowed = match stat {
                    Stat::Tracks => genome.level(Stat::Engine) > 0,
                    Stat::Mines | Stat::Shield => genome.brain() > 0,
                    _ => true,
                };
                assert!(
                    !allowed
                        || level == highest_level(stat, sector, genome.class)
                        || stat.price(level + 1) - stat.price(level) > left,
                    "sector {sector}: {left} points left for {stat:?} in {genome:?}"
                );
            }
        }
    }

    #[test]
    fn prerequisites_and_caps_hold() {
        for (sector, genome) in sample() {
            assert!(genome.brain() <= progression::brain_cap(sector));
            assert!(genome.ballistics() <= progression::ballistics_cap(sector));
            assert!(genome.level(Stat::Muzzle) <= progression::muzzle_cap(sector));
            if genome.level(Stat::Tracks) > 0 {
                assert!(genome.level(Stat::Engine) > 0);
            }
            if genome.brain() == 0 {
                assert_eq!(genome.mines(), 0);
                assert!(genome.shield_cooldown().is_none());
            }
            assert!(genome.bounces() + 1 >= genome.ballistics());
        }
    }

    #[test]
    fn the_fastest_gun_of_a_stretch_of_sectors_is_the_one_its_cap_allows() {
        // Between two steps of the cap, some tank has the fastest gun allowed and none has
        // a faster one.
        let stretches = [(1, 3), (4, 8), (9, 13), (14, 18), (19, 23), (24, 60)];
        for (from, to) in stretches {
            let fastest = sample()
                .filter(|(sector, _)| (from..=to).contains(sector))
                .map(|(_, genome)| genome.level(Stat::Muzzle))
                .max();
            assert_eq!(fastest, Some(progression::muzzle_cap(from)), "from {from}");
            assert_eq!(progression::muzzle_cap(from), progression::muzzle_cap(to));
        }
    }

    #[test]
    fn every_class_is_about_as_common_as_the_others() {
        let total = sample().count();
        for class in Class::ALL {
            let tanks = sample().filter(|(_, genome)| genome.class == class).count();
            assert!(
                tanks * 12 > total && tanks * 6 < total,
                "{class:?}: {tanks} of {total}"
            );
        }
    }

    #[test]
    fn piercing_and_long_ricochets_belong_to_their_classes() {
        for (sector, genome) in sample() {
            let class = genome.class;
            if genome.piercing() > 0 {
                assert_eq!(class, Class::Lancer, "sector {sector}");
            }
            if genome.bounces() > 1 {
                let bounces = matches!(class, Class::Trickster | Class::Tactician);
                assert!(bounces, "sector {sector}: {genome:?}");
            }
        }
        // A lancer pierces and a trickster's bullets bounce, as soon as they can pay for it.
        for (sector, genome) in sample() {
            let rich = genome.budget >= 8;
            match genome.class {
                Class::Lancer if rich => assert_eq!(genome.piercing(), 1, "sector {sector}"),
                Class::Trickster if rich => assert!(genome.bounces() > 0, "sector {sector}"),
                _ => {}
            }
        }
        // With the budget of a late sector, most tricksters have all the bounces there are.
        let late =
            || sample().filter(|(sector, tank)| *sector >= 20 && tank.class == Class::Trickster);
        let full = late().filter(|(_, tank)| tank.bounces() == 3).count();
        assert!(
            full * 4 >= late().count() * 3,
            "{full} of {}",
            late().count()
        );
    }

    #[test]
    fn a_tank_has_what_its_class_favors() {
        // From sector 10 on, a tank can pay for a level of each stat of its class, and has
        // more of them than the tanks of the other classes do.
        let late = || sample().filter(|(sector, _)| *sector >= 10);
        for class in Class::ALL {
            for &stat in class.favorites() {
                let level = |genome: &Genome| genome.level(stat) as f32;
                let (mut own, mut others) = (Vec::new(), Vec::new());
                for (sector, genome) in late() {
                    if genome.class == class {
                        assert!(genome.level(stat) > 0, "sector {sector}: {genome:?}");
                        own.push(level(&genome));
                    } else {
                        others.push(level(&genome));
                    }
                }
                let mean = |levels: &[f32]| levels.iter().sum::<f32>() / levels.len() as f32;
                assert!(
                    mean(&own) > mean(&others) + 0.5,
                    "{class:?} and {stat:?}: {} against {}",
                    mean(&own),
                    mean(&others)
                );
            }
        }
    }

    #[test]
    fn a_tactician_has_the_best_brain_of_its_sector() {
        for (sector, genome) in sample() {
            if genome.class == Class::Tactician {
                assert_eq!(genome.brain(), progression::brain_cap(sector));
                // Its aim is the best that it can pay the bounces for.
                let cap = progression::ballistics_cap(sector);
                let short = genome.budget as u32 - genome.cost() < 10;
                assert!(
                    genome.ballistics() == cap || short,
                    "sector {sector}: {genome:?}"
                );
            }
        }
    }
}
