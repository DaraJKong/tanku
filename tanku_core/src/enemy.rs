//! Generation of enemy tanks.

use crate::{
    progression,
    rng::{Purpose, Stream},
    stats::{Genome, Stat},
};

/// How much likelier a favorite stat is to receive the next point.
const FAVORITE_WEIGHT: u32 = 10;

/// The highest level of a stat that an enemy of a sector can have.
fn highest_level(stat: Stat, sector: u32) -> u8 {
    match stat {
        // Fast bullets wait for later sectors.
        Stat::Muzzle => progression::muzzle_cap(sector),
        _ => stat.max_level(),
    }
}

/// The genome of one enemy, which depends only on the seed, its sector and its index there.
pub fn generate(seed: u64, sector: u32, index: u32) -> Genome {
    let mut rng = Stream::new(seed, Purpose::Enemy, sector, index);
    let cap = progression::sector_budget(sector);
    let budget = rng.range((cap * 6).div_ceil(10) as i32, cap as i32) as u32;
    let mut genome = Genome::empty(budget as u16);

    let brain = rng.below(progression::brain_cap(sector) as u32 + 1) as u8;
    genome.set(Stat::Brain, brain);
    // A bank shot needs a bullet that bounces. The tier drops until its ricochet is affordable.
    let mut ballistics = rng.below(progression::ballistics_cap(sector) as u32 + 1) as u8;
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
    let mut favorites: Vec<Stat> = Vec::new();
    let favorite_count = rng.range(2, 3) as usize;
    while favorites.len() < favorite_count {
        let stat = rng.pick(&wanted);
        if !favorites.contains(&stat) {
            favorites.push(stat);
        }
    }

    loop {
        let left = budget - genome.cost();
        let affordable: Vec<Stat> = wanted
            .iter()
            .copied()
            .filter(|&stat| {
                let level = genome.level(stat);
                level < highest_level(stat, sector)
                    && stat.price(level + 1) - stat.price(level) <= left
                    && (stat != Stat::Tracks || genome.level(Stat::Engine) > 0)
            })
            .collect();
        if affordable.is_empty() {
            return genome;
        }
        let weights: Vec<u32> = affordable
            .iter()
            .map(|stat| {
                if favorites.contains(stat) {
                    FAVORITE_WEIGHT
                } else {
                    1
                }
            })
            .collect();
        let stat = affordable[rng.weighted(&weights)];
        genome.set(stat, genome.level(stat) + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> impl Iterator<Item = (u32, Genome)> {
        (0..6u64).flat_map(|seed| {
            (1..=60u32).flat_map(move |sector| {
                (0..9).map(move |index| (sector, generate(seed, sector, index)))
            })
        })
    }

    #[test]
    fn same_inputs_same_tank() {
        assert_eq!(generate(42, 12, 3), generate(42, 12, 3));
        assert_ne!(generate(42, 12, 3), generate(42, 12, 4));
        assert_ne!(generate(42, 12, 3), generate(43, 12, 3));
    }

    #[test]
    fn generation_is_stable() {
        // Golden value, as for the map: it must never change by accident.
        let genome = generate(crate::seed::seed_value("TANKU"), 20, 0);
        assert_eq!(
            format!("{genome:?}"),
            "Genome { levels: [1, 3, 6, 6, 0, 0, 2, 0, 0, 1, 0, 0], budget: 45 }"
        );
    }

    #[test]
    fn budgets_stay_within_the_sector_budget() {
        for (sector, genome) in sample() {
            let cap = progression::sector_budget(sector);
            let budget = genome.budget as u32;
            assert!(
                budget * 10 >= cap * 6 && budget <= cap,
                "sector {sector}: {budget}"
            );
            assert!(genome.cost() <= budget, "sector {sector}: overspent");
        }
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
                        || level == highest_level(stat, sector)
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
    fn tanks_specialize() {
        // Most tanks put most of their hardware points in three stats.
        let (mut specialized, mut total) = (0, 0);
        for (sector, genome) in sample() {
            if sector < 10 {
                continue;
            }
            let mut spent: Vec<u32> = Stat::HARDWARE
                .iter()
                .map(|&stat| stat.price(genome.level(stat)))
                .collect();
            spent.sort_unstable_by(|a, b| b.cmp(a));
            let top: u32 = spent[..3].iter().sum();
            specialized += (top * 5 >= genome.hardware_cost() * 3) as u32;
            total += 1;
        }
        assert!(specialized * 4 >= total * 3, "{specialized} of {total}");
    }
}
