//! Generation of enemy tanks.
//!
//! The enemies of a sector are a group. The group has a budget, which its tanks share
//! unevenly, so that weak tanks stand next to strong ones. Each tank has a class, which
//! decides what it spends its share on. What a tank can't spend on hardware raises its
//! software.

use crate::{
    balance::{BALANCE, Group},
    modes::Difficulty,
    progression,
    rng::{Purpose, Stream},
    stats::{Class, Genome, Stat},
};

/// How a group shares its points, as the balance says it. Its weights are those by which a
/// group shares what is left once every tank has the least: each tank draws one. Most are
/// light, and now and then one is heavy.
static GROUP: Group = BALANCE.group;
/// How much likelier the stats that a class favors are to receive the next point, all of
/// them together: a class with one favorite stat is as set on it as another on its two.
const FAVORITE_WEIGHT: u32 = GROUP.favorite;
/// What a tank of a group has on average, in percent of the sector budget: a group draws
/// its own between these two.
const GROUP_SHARE: (i32, i32) = GROUP.mean;
/// The least a tank has, in percent of the sector budget.
const LEAST_SHARE: u32 = GROUP.least;
/// The most that the strongest tank of a group can have, in percent of the sector budget.
/// The others stop at the sector budget.
const LEADER_SHARE: u32 = GROUP.leader;

/// The weight of each class when a tank draws its class, in the order of [`Class::ALL`].
const CLASS_WEIGHTS: [u32; Class::ALL.len()] = {
    let mut weights = [0; Class::ALL.len()];
    let mut index = 0;
    while index < weights.len() {
        weights[index] = BALANCE.classes[index].weight;
        index += 1;
    }
    weights
};

/// What a tank must have before a level of a stat is of any use to it: another stat, and the
/// level of it. An enemy without a brain can use neither mines nor a shield.
pub(crate) fn needs(stat: Stat) -> Option<(Stat, u8)> {
    match stat {
        Stat::Tracks => Some((Stat::Engine, 1)),
        Stat::Mines | Stat::Shield => Some((Stat::Brain, 1)),
        Stat::Seeking => Some((Stat::Ricochet, 1)),
        _ => None,
    }
}

/// Whether a class can have what it favors where these are the highest levels of the stats:
/// a first level of each stat that it favors, and of what that stat needs.
fn has_its_favorites(class: Class, most: impl Fn(Stat) -> u8) -> bool {
    class.favorites().iter().all(|&stat| {
        most(stat) > 0 && needs(stat).is_none_or(|(other, level)| most(other) >= level)
    })
}

/// Whether the tanks of a sector can be of a class: the sector must allow a first level of
/// every stat that the class favors, and of what those stats need. A tactician waits for the
/// sector that has a brain and ballistics to give it, and a sapper for the first brain,
/// without which it could use neither its mines nor its shield. The sectors of normal mode
/// decide, whatever the difficulty, so that a tank has the same class in hard mode.
pub fn comes(class: Class, sector: u32) -> bool {
    has_its_favorites(class, |stat| {
        highest_level(stat, Difficulty::Normal, sector, class)
    })
}

/// The class that a tank of a sector draws, among those that the sector has.
fn draw_class(rng: &mut Stream, sector: u32) -> Class {
    let mut weights = CLASS_WEIGHTS;
    for (weight, class) in weights.iter_mut().zip(Class::ALL) {
        if !comes(class, sector) {
            *weight = 0;
        }
    }
    // A balance that left a sector no class at all gets them all back there.
    if weights.iter().all(|weight| *weight == 0) {
        weights = CLASS_WEIGHTS;
    }
    Class::ALL[rng.weighted(&weights)]
}

/// The highest level of a stat that an enemy of a class can have in a sector.
pub(crate) fn highest_level(stat: Stat, difficulty: Difficulty, sector: u32, class: Class) -> u8 {
    // Fast bullets, good brains and good aim wait for later sectors, and so does whatever
    // else the balance holds back.
    let allowed = progression::cap(stat, sector, difficulty);
    allowed.min(class.highest(stat))
}

/// The budgets of the tanks of a group, which depend on the seed, the difficulty, the sector
/// and the size of the group. Together they are what so many tanks have on average. The
/// strongest may have more than the sector budget, and no other does.
///
/// A group shares its budget in the same way whatever the difficulty: the tank that leads it
/// in normal mode leads it in hard mode.
pub fn budgets(seed: u64, difficulty: Difficulty, sector: u32, count: u32) -> Vec<u32> {
    let mut rng = Stream::new(seed, Purpose::Group, sector, count);
    let cap = progression::sector_budget(sector, difficulty);
    let share = rng.range(GROUP_SHARE.0, GROUP_SHARE.1) as u32;
    let total = (count * cap * share + 50) / 100;
    let least = (cap * LEAST_SHARE).div_ceil(100);
    if count <= 1 {
        return vec![total.max(least); count as usize];
    }
    let weights: Vec<u32> = (0..count).map(|_| rng.pick(GROUP.weights())).collect();
    let sum: u32 = weights.iter().sum();
    // The first of the heaviest leads the group.
    let heaviest = weights.iter().copied().max().unwrap_or(0);
    let leader = weights.iter().position(|&weight| weight == heaviest);
    let left = total.saturating_sub(count * least);
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

/// The genomes of the group of a sector, which depend only on the seed, the difficulty, the
/// sector and the size of the group.
pub fn group(seed: u64, difficulty: Difficulty, sector: u32, count: u32) -> Vec<Genome> {
    let budgets = budgets(seed, difficulty, sector, count);
    let tanks = budgets.iter().enumerate();
    tanks
        .map(|(index, &budget)| build(seed, difficulty, sector, index as u32, budget))
        .collect()
}

/// A tank with one more tier of software, if what it has left pays for it: of its brain, or
/// else of its ballistics, with the bounce that the tier needs. No tier is higher than the
/// sector and the class of the tank allow.
fn smarter(genome: &Genome, highest: impl Fn(Stat) -> u8) -> Option<Genome> {
    let budget = genome.budget as u32;
    if genome.brain() < highest(Stat::Brain) {
        let mut raised = *genome;
        raised.set(Stat::Brain, genome.brain() + 1);
        if raised.cost() <= budget {
            return Some(raised);
        }
    }
    let tier = genome.ballistics() + 1;
    if tier <= highest(Stat::Ballistics) {
        let mut raised = *genome;
        raised.set(Stat::Ballistics, tier);
        // A bank shot needs a bullet that bounces.
        raised.set(Stat::Ricochet, genome.bounces().max(tier - 1));
        if raised.cost() <= budget {
            return Some(raised);
        }
    }
    None
}

/// One tank of a group: its class, then what its budget buys. A tank has the same class
/// whatever the difficulty.
fn build(seed: u64, difficulty: Difficulty, sector: u32, index: u32, budget: u32) -> Genome {
    let mut rng = Stream::new(seed, Purpose::Enemy, sector, index);
    let class = draw_class(&mut rng, sector);
    spend(rng, difficulty, sector, class, budget)
}

/// A tank of a class that is given, with a budget that is given: what a tank of a group
/// would be if it drew that class and had those points, under the caps of its sector. No
/// group of the game is made this way: it is for the trials, which try a class alone.
pub fn of_class(
    seed: u64,
    difficulty: Difficulty,
    sector: u32,
    index: u32,
    class: Class,
    budget: u32,
) -> Genome {
    let mut rng = Stream::new(seed, Purpose::Enemy, sector, index);
    // The draw of the class is made and set aside, so that the rest is drawn as in a group.
    draw_class(&mut rng, sector);
    spend(rng, difficulty, sector, class, budget)
}

/// What the budget of a tank of a class buys.
fn spend(
    mut rng: Stream,
    difficulty: Difficulty,
    sector: u32,
    class: Class,
    budget: u32,
) -> Genome {
    let mut genome = Genome::empty(budget as u16);
    genome.class = class;
    let highest = |stat: Stat| highest_level(stat, difficulty, sector, class);

    // A tactician takes the best brain and the best aim that its sector allows. A sapper
    // needs a brain for its mines and its shield. The others take what they draw. No class
    // has a better brain than its sector allows: a sapper that is made for a sector without
    // any, as the trials can ask for one, has none.
    let brain = match class {
        Class::Tactician => highest(Stat::Brain),
        Class::Sapper => {
            (1 + rng.below(highest(Stat::Brain) as u32) as u8).min(highest(Stat::Brain))
        }
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
    let wanted = |genome: &Genome| -> Vec<Stat> {
        let thinks = genome.brain() > 0;
        Stat::HARDWARE
            .into_iter()
            .filter(|stat| thinks || !matches!(stat, Stat::Mines | Stat::Shield))
            .collect()
    };
    let affordable = |genome: &Genome, stat: Stat| {
        let level = genome.level(stat);
        level < highest(stat)
            && stat.price(level + 1) - stat.price(level) <= budget - genome.cost()
            && (stat != Stat::Tracks || genome.level(Stat::Engine) > 0)
            // A bullet that doesn't bounce never seeks.
            && (stat != Stat::Seeking || genome.bounces() > 0)
    };
    // What makes its class comes first: a tank has a level of each stat that it favors, if
    // it can pay for it.
    let favorites: Vec<Stat> = class
        .favorites()
        .iter()
        .copied()
        .filter(|stat| wanted(&genome).contains(stat))
        .collect();
    for &stat in &favorites {
        if genome.level(stat) == 0 && affordable(&genome, stat) {
            genome.set(stat, 1);
        }
    }

    loop {
        let open: Vec<Stat> = wanted(&genome)
            .into_iter()
            .filter(|&stat| affordable(&genome, stat))
            .collect();
        if open.is_empty() {
            // What a tank can't spend on hardware raises its software, a tier at a time.
            // It then goes on buying: a first brain opens mines and a shield.
            match smarter(&genome, highest) {
                Some(raised) => genome = raised,
                None => return genome,
            }
            continue;
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
    use crate::stats::Ammo;
    use Difficulty::{Hard, Normal};

    /// Groups of every size up to nine, over six seeds and sixty sectors: each tank with its
    /// sector.
    fn groups(difficulty: Difficulty) -> impl Iterator<Item = (u32, Vec<Genome>)> {
        (0..6u64).flat_map(move |seed| {
            (1..=60u32).map(move |sector| {
                let count = 1 + (sector + 3) % 9;
                (sector, group(seed, difficulty, sector, count))
            })
        })
    }

    fn sample(difficulty: Difficulty) -> impl Iterator<Item = (u32, Genome)> {
        let tanks =
            |(sector, tanks): (u32, Vec<Genome>)| tanks.into_iter().map(move |tank| (sector, tank));
        groups(difficulty).flat_map(tanks)
    }

    #[test]
    fn same_inputs_same_group() {
        assert_eq!(group(42, Normal, 12, 4), group(42, Normal, 12, 4));
        assert_ne!(group(42, Normal, 12, 4), group(43, Normal, 12, 4));
        assert_ne!(group(42, Normal, 12, 4), group(42, Normal, 13, 4));
        assert_ne!(group(42, Normal, 12, 4), group(42, Hard, 12, 4));
        // A tank keeps its class whatever the size of its group: only its share changes.
        let (four, six) = (group(42, Normal, 12, 4), group(42, Normal, 12, 6));
        for (small, large) in four.iter().zip(&six) {
            assert_eq!(small.class, large.class);
        }
    }

    #[test]
    fn generation_is_stable() {
        // Golden value, as for the map: it must never change by accident.
        let tanks = group(crate::seed::seed_value("TANKU"), Normal, 20, 6);
        assert_eq!(
            format!("{:?}", tanks[0]),
            "Genome { levels: [3, 6, 6, 6, 5, 4, 1, 0, 0, 0, 0, 0, 0, 0, 0], budget: 64, class: None }"
        );
        let classes: Vec<Class> = tanks.iter().map(|tank| tank.class).collect();
        assert_eq!(format!("{classes:?}"), CLASSES);
        let budgets: Vec<u16> = tanks.iter().map(|tank| tank.budget).collect();
        assert_eq!(format!("{budgets:?}"), BUDGETS);
        // The same group in hard mode.
        let tanks = group(crate::seed::seed_value("TANKU"), Hard, 20, 6);
        assert_eq!(format!("{:?}", tanks[0]), HARD_TANK);
        let classes: Vec<Class> = tanks.iter().map(|tank| tank.class).collect();
        assert_eq!(format!("{classes:?}"), CLASSES);
        let budgets: Vec<u16> = tanks.iter().map(|tank| tank.budget).collect();
        assert_eq!(format!("{budgets:?}"), HARD_BUDGETS);
    }

    const CLASSES: &str = "[None, Lancer, Trickster, None, Sapper, Flanker]";
    const BUDGETS: &str = "[64, 112, 35, 48, 35, 48]";
    const HARD_TANK: &str =
        "Genome { levels: [7, 6, 6, 7, 5, 7, 1, 0, 0, 0, 0, 0, 0, 0, 0], budget: 90, class: None }";
    const HARD_BUDGETS: &str = "[90, 157, 49, 67, 49, 67]";

    #[test]
    fn hard_mode_makes_the_same_group_stronger() {
        let pairs = groups(Normal).zip(groups(Hard));
        for ((sector, normal), (_, hard)) in pairs {
            assert_eq!(normal.len(), hard.len());
            for (normal, hard) in normal.iter().zip(&hard) {
                // Each tank is of the same class and has more points, so that a seed is the
                // same level, made harder.
                assert_eq!(normal.class, hard.class, "sector {sector}");
                assert!(hard.budget > normal.budget, "sector {sector}");
            }
            // The tank that has the most points is the same one.
            let leader = |tanks: &[Genome]| {
                let most = tanks.iter().map(|tank| tank.budget).max();
                tanks.iter().position(|tank| Some(tank.budget) == most)
            };
            assert_eq!(leader(&normal), leader(&hard), "sector {sector}");
        }
        // From its first sector, hard mode has the brains and the guns that normal mode
        // only has late.
        let early = |difficulty: Difficulty| sample(difficulty).filter(|(sector, _)| *sector <= 3);
        let aces = |difficulty: Difficulty| early(difficulty).filter(|(_, tank)| tank.brain() == 4);
        assert_eq!(aces(Normal).count(), 0);
        // The tacticians, which take the best brain there is, come from the sector that
        // normal mode gives them.
        assert!(
            aces(Hard).count() * 8 > early(Hard).count(),
            "{} of {}",
            aces(Hard).count(),
            early(Hard).count()
        );
        let fastest = |difficulty: Difficulty| {
            early(difficulty)
                .map(|(_, tank)| tank.level(Stat::Muzzle))
                .max()
        };
        assert_eq!(fastest(Normal), Some(2));
        assert_eq!(fastest(Hard), Some(6));
    }

    /// Prints what the groups of a single player are made of in each mode, by stretch of
    /// sectors: how many tanks have each brain, and how many points they can't spend.
    /// `cargo test -p tanku_core what_the_groups_are_made_of -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn print_what_the_groups_are_made_of() {
        let stretches = [
            (1, 2),
            (3, 5),
            (6, 10),
            (11, 15),
            (16, 20),
            (21, 30),
            (31, 50),
        ];
        for difficulty in Difficulty::ALL {
            println!("\n{difficulty:?} mode, 300 seeds");
            println!(
                "{:<8} {:>6} {:>6} {:>24} {:>9} {:>9} {:>9} {:>9}",
                "sectors",
                "tanks",
                "budget",
                "brains 0 1 2 3 4, in %",
                "left > 4",
                "mean left",
                "most left",
                "leader's"
            );
            for (from, to) in stretches {
                let mut tanks: Vec<(Genome, bool)> = Vec::new();
                for seed in 0..300u64 {
                    for sector in from..=to {
                        let count = progression::enemy_count(sector, 1, false);
                        let group = group(seed, difficulty, sector, count);
                        let most = group.iter().map(|tank| tank.budget).max();
                        let leads = |tank: &Genome| count > 1 && Some(tank.budget) == most;
                        tanks.extend(group.iter().map(|tank| (*tank, leads(tank))));
                    }
                }
                let total = tanks.len() as f32;
                let left = |tank: &Genome| tank.budget as u32 - tank.cost();
                let brains: Vec<String> = (0..=4)
                    .map(|tier| {
                        let with = tanks
                            .iter()
                            .filter(|(tank, _)| tank.brain() == tier)
                            .count();
                        format!("{:>3.0}", with as f32 * 100.0 / total)
                    })
                    .collect();
                let budget = tanks
                    .iter()
                    .map(|(tank, _)| tank.budget as f32)
                    .sum::<f32>()
                    / total;
                let idle = tanks.iter().filter(|(tank, _)| left(tank) > 4).count();
                let mean = tanks.iter().map(|(tank, _)| left(tank) as f32).sum::<f32>() / total;
                let most = tanks.iter().map(|(tank, _)| left(tank)).max().unwrap_or(0);
                let leaders: Vec<&Genome> = tanks
                    .iter()
                    .filter(|(_, leads)| *leads)
                    .map(|(tank, _)| tank)
                    .collect();
                let leaders_left = leaders.iter().map(|tank| left(tank) as f32).sum::<f32>()
                    / leaders.len().max(1) as f32;
                println!(
                    "{:<8} {:>6} {:>6.0} {:>24} {:>8.0}% {:>9.1} {:>9} {:>9.1}",
                    format!("{from}-{to}"),
                    tanks.len(),
                    budget,
                    brains.join(" "),
                    idle as f32 * 100.0 / total,
                    mean,
                    most,
                    leaders_left
                );
            }
        }
    }

    #[test]
    fn a_group_shares_its_budget_unevenly() {
        for difficulty in Difficulty::ALL {
            a_group_shares_its_budget_unevenly_in(difficulty);
        }
    }

    fn a_group_shares_its_budget_unevenly_in(difficulty: Difficulty) {
        let (mut ratios, mut led) = (Vec::new(), 0);
        for (sector, tanks) in groups(difficulty) {
            let cap = progression::sector_budget(sector, difficulty);
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
        let both = Difficulty::ALL.into_iter();
        let tanks =
            both.flat_map(|difficulty| sample(difficulty).map(move |tank| (difficulty, tank)));
        for (difficulty, (sector, genome)) in tanks {
            let left = genome.budget as u32 - genome.cost();
            for stat in Stat::HARDWARE {
                let level = genome.level(stat);
                let allowed = match stat {
                    Stat::Tracks => genome.level(Stat::Engine) > 0,
                    Stat::Mines | Stat::Shield => genome.brain() > 0,
                    Stat::Seeking => genome.bounces() > 0,
                    _ => true,
                };
                assert!(
                    !allowed
                        || level == highest_level(stat, difficulty, sector, genome.class)
                        || stat.price(level + 1) - stat.price(level) > left,
                    "sector {sector}: {left} points left for {stat:?} in {genome:?}"
                );
            }
            // Nor can it afford a tier of software: of its brain, or of its ballistics with
            // the bounce that the tier needs.
            let highest = |stat: Stat| highest_level(stat, difficulty, sector, genome.class);
            let budget = genome.budget as u32;
            let step = |stat: Stat| {
                let tier = genome.level(stat);
                stat.share(tier + 1, budget) - stat.share(tier, budget)
            };
            assert!(
                genome.brain() == highest(Stat::Brain) || step(Stat::Brain) > left,
                "sector {sector}: {left} points left for a brain in {genome:?}"
            );
            if genome.ballistics() < highest(Stat::Ballistics) {
                let bounces = genome.bounces().max(genome.ballistics());
                let bounce = Stat::Ricochet.price(bounces) - Stat::Ricochet.price(genome.bounces());
                assert!(
                    step(Stat::Ballistics) + bounce > left,
                    "sector {sector}: {left} points left for ballistics in {genome:?}"
                );
            }
        }
    }

    #[test]
    fn a_tier_of_software_is_paid_for_and_stays_within_what_the_class_allows() {
        let tank = |class: Class, budget: u16, levels: &[(Stat, u8)]| {
            let mut genome = Genome::empty(budget);
            genome.class = class;
            for &(stat, level) in levels {
                genome.set(stat, level);
            }
            genome
        };
        // A late sector allows every tier, so only the class and the points decide.
        let raised =
            |genome: &Genome| smarter(genome, |stat| highest_level(stat, Normal, 50, genome.class));
        let tiers = |genome: &Genome| (genome.brain(), genome.ballistics(), genome.bounces());
        // The brain comes first.
        let plain = tank(Class::None, 200, &[]);
        assert_eq!(raised(&plain).as_ref().map(tiers), Some((1, 0, 0)));
        // A tank with the best brain sharpens its aim. The second tier needs a bullet that
        // bounces, which comes with it.
        let ace = tank(Class::None, 200, &[(Stat::Brain, 4), (Stat::Ballistics, 1)]);
        let sharper = raised(&ace).unwrap();
        assert_eq!(tiers(&sharper), (4, 2, 1));
        // The third tier is kept for the classes whose bullets bounce twice.
        assert_eq!(raised(&sharper), None);
        let mut trickster = sharper;
        trickster.class = Class::Trickster;
        assert_eq!(raised(&trickster).as_ref().map(tiers), Some((4, 3, 2)));
        // Nothing is raised on credit. A tank of 50 points with 2 left can pay neither for
        // a brain, which costs it 4, nor for an aim, which costs it 3. With 3 left, it
        // takes the aim.
        let parts = [(Stat::Engine, 7), (Stat::Muzzle, 7), (Stat::Turret, 6)];
        let spent = tank(Class::None, 50, &parts);
        assert_eq!(spent.cost(), 48);
        assert_eq!(raised(&spent), None);
        let richer = tank(Class::None, 51, &parts);
        assert_eq!(raised(&richer).as_ref().map(tiers), Some((0, 1, 0)));
        assert_eq!(raised(&richer).map(|tank| tank.cost()), Some(51));
    }

    #[test]
    fn what_a_tank_can_not_spend_on_hardware_raises_its_software() {
        let everyone = || sample(Normal).chain(sample(Hard));
        let left = |tank: &Genome| tank.budget as u32 - tank.cost();
        // No tank is left with what a tier of a brain costs the richest of them, which is
        // 8% of 195 points, and few are left with more than a few points.
        let most = everyone().map(|(_, tank)| left(&tank)).max();
        assert!(most < Some(16), "{most:?}");
        let (idle, all) = (
            everyone().filter(|(_, tank)| left(tank) > 4).count(),
            everyone().count(),
        );
        assert!(idle * 50 < all, "{idle} of {all}");
        // A tank that has more points than all the hardware there is costs has a brain:
        // without one, it could use neither mines nor a shield.
        for (sector, tank) in everyone() {
            assert!(
                tank.budget <= 135 || tank.brain() > 0,
                "sector {sector}: {tank:?}"
            );
        }
        // Two leaders of late sectors, each with every part that a tank without a brain
        // may have. One has enough left for a brain, which opens mines and a shield to
        // it. The other has too little for a brain, and sharpens its aim.
        let leader = |seed: &str, sector: u32, index: usize| {
            let count = progression::enemy_count(sector, 1, false);
            group(crate::seed::seed_value(seed), Normal, sector, count)[index]
        };
        let scout = leader("BRAVO", 21, 3);
        assert_eq!(
            (scout.class, scout.budget, scout.cost()),
            (Class::Scout, 114, 114)
        );
        assert_eq!((scout.brain(), scout.mines()), (1, 3));
        assert_eq!(scout.shield_cooldown(), Some(10.0));
        // A seeker stops at the first tier of ballistics: its bullets need no bank shot.
        let seeker = leader("ECHO", 20, 5);
        assert_eq!(
            (seeker.class, seeker.budget, seeker.cost()),
            (Class::Seeker, 112, 108)
        );
        assert_eq!((seeker.brain(), seeker.ballistics()), (0, 1));
    }

    #[test]
    fn prerequisites_and_caps_hold() {
        let both = Difficulty::ALL.into_iter();
        let tanks =
            both.flat_map(|difficulty| sample(difficulty).map(move |tank| (difficulty, tank)));
        for (difficulty, (sector, genome)) in tanks {
            assert!(genome.brain() <= progression::brain_cap(sector, difficulty));
            assert!(genome.ballistics() <= progression::ballistics_cap(sector, difficulty));
            assert!(genome.level(Stat::Muzzle) <= progression::muzzle_cap(sector, difficulty));
            if genome.level(Stat::Tracks) > 0 {
                assert!(genome.level(Stat::Engine) > 0);
            }
            if genome.brain() == 0 {
                assert_eq!(genome.mines(), 0);
                assert!(genome.shield_cooldown().is_none());
            }
            assert!(genome.bounces() + 1 >= genome.ballistics());
            // A tank fires one type of bullet, and a bullet that seeks bounces.
            let ammos = Ammo::STATS.iter().filter(|stat| genome.level(**stat) > 0);
            assert!(ammos.count() <= 1, "sector {sector}: {genome:?}");
            if genome.ammo() == Ammo::Seeking {
                assert!(genome.bounces() > 0, "sector {sector}: {genome:?}");
            }
        }
    }

    #[test]
    fn the_fastest_gun_of_a_stretch_of_sectors_is_the_one_its_cap_allows() {
        // Between two steps of the cap, some tank has the fastest gun allowed and none has
        // a faster one.
        let stretches = [(1, 3), (4, 8), (9, 13), (14, 18), (19, 23), (24, 60)];
        let hard_stretches = [(1, 3), (4, 60)];
        let stretches = stretches.map(|stretch| (Normal, stretch));
        let hard_stretches = hard_stretches.map(|stretch| (Hard, stretch));
        for (difficulty, (from, to)) in stretches.into_iter().chain(hard_stretches) {
            let cap = |sector: u32| progression::muzzle_cap(sector, difficulty);
            let fastest = sample(difficulty)
                .filter(|(sector, _)| (from..=to).contains(sector))
                .map(|(_, genome)| genome.level(Stat::Muzzle))
                .max();
            assert_eq!(fastest, Some(cap(from)), "from {from} in {difficulty:?}");
            assert_eq!(cap(from), cap(to));
        }
    }

    #[test]
    fn every_class_is_about_as_common_as_the_others() {
        // A tank has the same class whatever the difficulty.
        let total = sample(Normal).count();
        for class in Class::ALL {
            let of_class = |(_, genome): &(u32, Genome)| genome.class == class;
            let tanks = sample(Normal).filter(of_class).count();
            assert!(
                tanks * 16 > total && tanks * 8 < total,
                "{class:?}: {tanks} of {total}"
            );
        }
    }

    #[test]
    fn a_class_waits_for_the_sector_that_has_what_it_favors() {
        // A tactician favors the brain and ballistics: it comes with the first tier of both.
        let first = |class: Class| (1..60).find(|sector| comes(class, *sector));
        let both = (1..60).find(|sector| {
            progression::brain_cap(*sector, Normal) > 0
                && progression::ballistics_cap(*sector, Normal) > 0
        });
        assert_eq!(first(Class::Tactician), both);
        assert!(both > Some(1));
        // What the other classes favor is there from the first sector.
        for class in Class::ALL {
            if class != Class::Tactician {
                assert_eq!(first(class), Some(1), "{class:?}");
            }
        }
        // No tank is of a class before its sector, in either mode, and tanks are of it after.
        for (sector, tank) in sample(Normal).chain(sample(Hard)) {
            assert!(
                comes(tank.class, sector),
                "sector {sector}: {:?}",
                tank.class
            );
        }
        let tacticians = sample(Normal).filter(|(_, tank)| tank.class == Class::Tactician);
        assert!(tacticians.count() > 0);
    }

    #[test]
    fn a_class_waits_for_what_its_favorite_stats_need() {
        // The highest level of every stat but these is the highest there is.
        let with = |held: &'static [(Stat, u8)]| {
            move |stat: Stat| {
                let found = held.iter().find(|(other, _)| *other == stat);
                found.map_or(stat.max_level(), |(_, level)| *level)
            }
        };
        let comes =
            |class: Class, held: &'static [(Stat, u8)]| has_its_favorites(class, with(held));
        // Every class has what it favors where nothing is held back.
        for class in Class::ALL {
            assert!(comes(class, &[]), "{class:?}");
        }
        // A sapper favors mines and a shield, which an enemy without a brain can't use: it
        // waits for the first brain, and so does no other class but the tactician, which
        // favors the brain itself.
        let brainless: &[(Stat, u8)] = &[(Stat::Brain, 0)];
        for class in Class::ALL {
            let waits = matches!(class, Class::Sapper | Class::Tactician);
            assert_eq!(comes(class, brainless), !waits, "{class:?}");
        }
        assert!(comes(Class::Sapper, &[(Stat::Brain, 1)]));
        // A scout waits for an engine, which its tracks need, and a seeker for a bounce,
        // which a bullet that seeks needs.
        assert!(!comes(Class::Scout, &[(Stat::Engine, 0)]));
        assert!(!comes(Class::Seeker, &[(Stat::Ricochet, 0)]));
        assert!(!comes(Class::Trickster, &[(Stat::Ricochet, 0)]));
        assert!(comes(
            Class::Gunner,
            &[(Stat::Engine, 0), (Stat::Ricochet, 0)]
        ));
        // A stat that is held back holds back the class that favors it, and no other.
        assert!(!comes(Class::Grenadier, &[(Stat::Exploding, 0)]));
        assert!(comes(Class::Flanker, &[(Stat::Exploding, 0)]));
    }

    #[test]
    fn special_bullets_and_long_ricochets_belong_to_their_classes() {
        let everyone = || sample(Normal).chain(sample(Hard));
        for (sector, genome) in everyone() {
            let class = genome.class;
            let own = match genome.ammo() {
                Ammo::Normal => None,
                Ammo::Piercing => Some(Class::Lancer),
                Ammo::Seeking => Some(Class::Seeker),
                Ammo::Splitting => Some(Class::Flanker),
                Ammo::Exploding => Some(Class::Grenadier),
            };
            assert!(own.is_none_or(|own| own == class), "sector {sector}");
            if genome.bounces() > 1 {
                let bounces = matches!(class, Class::Trickster | Class::Tactician | Class::Seeker);
                assert!(bounces, "sector {sector}: {genome:?}");
            }
            // A bullet that explodes never bounces, and no bank shot is for these three.
            if class == Class::Grenadier {
                assert_eq!(genome.bounces(), 0, "sector {sector}");
            }
            if matches!(class, Class::Seeker | Class::Flanker | Class::Grenadier) {
                assert!(genome.ballistics() <= 1, "sector {sector}: {genome:?}");
            }
        }
        // A tank has the bullet of its class and a trickster's bullets bounce, as soon as
        // they can pay for it. A seeker pays for the bounce first, then for the seeking.
        for (sector, genome) in everyone() {
            let rich = genome.budget >= 8;
            let ammo = genome.ammo();
            match genome.class {
                Class::Lancer if rich => assert_eq!(ammo, Ammo::Piercing, "sector {sector}"),
                // A seeker that has a brain to pay for needs a little more for both.
                Class::Seeker if genome.budget >= 10 => {
                    assert_eq!(ammo, Ammo::Seeking, "sector {sector}: {genome:?}")
                }
                Class::Flanker if rich => assert_eq!(ammo, Ammo::Splitting, "sector {sector}"),
                Class::Grenadier if rich => assert_eq!(ammo, Ammo::Exploding, "sector {sector}"),
                Class::Trickster if rich => assert!(genome.bounces() > 0, "sector {sector}"),
                _ => {}
            }
        }
        // With the budget of a late sector, most tricksters have all the bounces there are.
        let late = || {
            sample(Normal).filter(|(sector, tank)| *sector >= 20 && tank.class == Class::Trickster)
        };
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
        for difficulty in Difficulty::ALL {
            a_tank_has_what_its_class_favors_in(difficulty);
        }
    }

    fn a_tank_has_what_its_class_favors_in(difficulty: Difficulty) {
        let late = || sample(difficulty).filter(|(sector, _)| *sector >= 10);
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
        let both = Difficulty::ALL.into_iter();
        let tanks =
            both.flat_map(|difficulty| sample(difficulty).map(move |tank| (difficulty, tank)));
        for (difficulty, (sector, genome)) in tanks {
            if genome.class == Class::Tactician {
                assert_eq!(genome.brain(), progression::brain_cap(sector, difficulty));
                // Its aim is the best that it can pay the bounces for.
                let cap = progression::ballistics_cap(sector, difficulty);
                let short = genome.budget as u32 - genome.cost() < 10;
                assert!(
                    genome.ballistics() == cap || short,
                    "sector {sector}: {genome:?}"
                );
            }
        }
    }

    #[test]
    fn a_tank_of_a_class_that_is_given_is_the_one_a_group_would_hold() {
        // A tank of a group, made again with the class that it drew and the points that it
        // had, is the same tank.
        for sector in [3, 12, 30] {
            for (index, tank) in group(7, Difficulty::Normal, sector, 6).iter().enumerate() {
                let budget = tank.budget as u32;
                let index = index as u32;
                let again = of_class(7, Difficulty::Normal, sector, index, tank.class, budget);
                assert_eq!(&again, tank);
            }
        }
        // Another class with the same points is another tank, of that class.
        for class in Class::ALL {
            let tank = of_class(7, Difficulty::Normal, 30, 0, class, 80);
            assert_eq!(tank.class, class);
            assert!(tank.cost() <= 80);
        }
    }
}
