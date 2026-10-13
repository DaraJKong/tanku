//! Breeding of enemy tanks: a tank that descends from two others, with their levels
//! recombined and slightly changed, within the points that it is given and under the limits
//! of its class and of its sector.
//!
//! It is the step that impossible mode will make from one group to the next, and the one
//! that the trials make to look for the tank that does the most with its points. No run of
//! the game breeds a tank yet.
//!
//! A tank that was bred is sound, as one that the generator made: no level is above what its
//! class and its sector allow, no stat is there without the one that it needs, and its
//! budget pays for all of it. It is not held to what its class favors. A class is what a
//! tank of the generator spends its points on, and what breeding finds may be something
//! else: the class of a tank that was bred only says which bullets it may fire, and how far
//! its ricochet and its ballistics may go.

use crate::{
    enemy::{comes, highest_level, needs},
    modes::Difficulty,
    rng::Stream,
    stats::{Class, Genome, Stat},
};

/// How often the one change of a child is to its class, and not to a level: one time in so
/// many.
const CLASS_CHANGE: u32 = 8;

/// Where a tank is bred, which decides the highest level that it may have of each stat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub difficulty: Difficulty,
    pub sector: u32,
}

impl Place {
    /// The highest level of a stat that a tank of a class may have there.
    pub fn highest(self, class: Class, stat: Stat) -> u8 {
        highest_level(stat, self.difficulty, self.sector, class)
    }

    /// The classes that a tank of the sector can be of. A balance that left a sector no
    /// class at all gets them all back there, as when the generator draws one.
    pub fn classes(self) -> Vec<Class> {
        let come = |class: &Class| comes(*class, self.sector);
        let classes: Vec<Class> = Class::ALL.into_iter().filter(come).collect();
        match classes.is_empty() {
            true => Class::ALL.to_vec(),
            false => classes,
        }
    }
}

/// Whether a level of a stat is of no use to a tank, for want of another stat: tracks without
/// an engine, mines or a shield without a brain, seeking bullets that don't bounce, and a
/// bank shot with more bounces than the bullets have.
fn lacks(genome: &Genome, stat: Stat, level: u8) -> bool {
    if level == 0 {
        return false;
    }
    let unmet = |(other, least): (Stat, u8)| genome.level(other) < least;
    needs(stat).is_some_and(unmet) || (stat == Stat::Ballistics && genome.bounces() + 1 < level)
}

/// Whether a tank is one that could stand in a place: no level of it is above what its class
/// and the sector allow, no stat is there without the one that it needs, and its budget pays
/// for all of it. Every tank of the generator is sound, and so is every tank that is bred.
pub fn sound(genome: &Genome, place: Place) -> bool {
    let within = |stat: Stat| genome.level(stat) <= place.highest(genome.class, stat);
    let served = |stat: Stat| !lacks(genome, stat, genome.level(stat));
    let allowed = Stat::ALL
        .into_iter()
        .all(|stat| within(stat) && served(stat));
    allowed && genome.cost() <= genome.budget as u32
}

/// Takes from a tank what it has no use of: the levels of a stat that lacks the one it
/// needs. None of the stats that others need has a need of its own, so once through is
/// enough.
fn settle(genome: &mut Genome) {
    for stat in Stat::ALL {
        let mut level = genome.level(stat);
        while lacks(genome, stat, level) {
            level -= 1;
        }
        genome.set(stat, level);
    }
}

/// Makes a tank sound: each level comes down to what its class and its sector allow, and
/// then levels that are drawn at random go down, one at a time, until its budget pays for
/// what is left.
fn mend(genome: &mut Genome, place: Place, rng: &mut Stream) {
    for stat in Stat::ALL {
        let most = place.highest(genome.class, stat);
        genome.set(stat, genome.level(stat).min(most));
    }
    settle(genome);
    while genome.cost() > genome.budget as u32 {
        let has = |stat: &Stat| genome.level(*stat) > 0;
        let held: Vec<Stat> = Stat::ALL.into_iter().filter(has).collect();
        // A tank that costs anything holds a level of something.
        let stat = rng.pick(&held);
        genome.set(stat, genome.level(stat) - 1);
        settle(genome);
    }
}

/// Whether a tank can take one more level of a stat: its class and its sector allow it, the
/// tank has a use of it, and what it has left pays for it.
fn takes(genome: &Genome, stat: Stat, place: Place) -> bool {
    let level = genome.level(stat) + 1;
    if level > place.highest(genome.class, stat) || lacks(genome, stat, level) {
        return false;
    }
    let mut raised = *genome;
    raised.set(stat, level);
    raised.cost() <= genome.budget as u32
}

/// Spends what a tank has left, a level at a time, each drawn at random among those that
/// the tank can take, until it can take none. The stats of its class weigh no more than the
/// others: what a tank that is bred spends on comes from its parents.
fn fill(genome: &mut Genome, place: Place, rng: &mut Stream) {
    loop {
        let open: Vec<Stat> = Stat::ALL
            .into_iter()
            .filter(|&stat| takes(genome, stat, place))
            .collect();
        if open.is_empty() {
            return;
        }
        let stat = rng.pick(&open);
        genome.set(stat, genome.level(stat) + 1);
    }
}

/// Whether a tank has spent its points: what it has left pays for no level that it could
/// take.
pub fn spent(genome: &Genome, place: Place) -> bool {
    !Stat::ALL.into_iter().any(|stat| takes(genome, stat, place))
}

/// The levels and the class of two tanks, recombined: each level is that of one parent or
/// of the other, as a coin falls, and so is the class. Nothing says yet that the tank is
/// sound.
fn cross(one: &Genome, other: &Genome, budget: u32, rng: &mut Stream) -> Genome {
    let mut child = Genome::empty(budget as u16);
    let mut parent = || if rng.chance(50) { one } else { other };
    child.class = parent().class;
    for stat in Stat::ALL {
        child.set(stat, parent().level(stat));
    }
    child
}

/// Changes one thing about a tank: a level of a stat that its class may have goes up or
/// down by one, or, now and then, its class becomes one of those that the sector has.
fn change(genome: &mut Genome, place: Place, rng: &mut Stream) {
    if rng.below(CLASS_CHANGE) == 0 {
        genome.class = rng.pick(&place.classes());
        return;
    }
    let may = |stat: &Stat| place.highest(genome.class, *stat) > 0;
    let open: Vec<Stat> = Stat::ALL.into_iter().filter(may).collect();
    if open.is_empty() {
        return;
    }
    let stat = rng.pick(&open);
    let (level, most) = (genome.level(stat), place.highest(genome.class, stat));
    let up = level == 0 || (level < most && rng.chance(50));
    let changed = if up { level + 1 } else { level - 1 };
    genome.set(stat, changed.min(stat.max_level()));
}

/// A tank that descends from two others, with so many points to spend. Its levels and its
/// class are those of its parents, recombined. One thing about it is then changed, it is
/// made sound for the place, and what it has left is spent. The stream decides all of it:
/// the same parents and the same stream give the same child.
pub fn child(one: &Genome, other: &Genome, budget: u32, place: Place, rng: &mut Stream) -> Genome {
    let mut child = cross(one, other, budget, rng);
    change(&mut child, place, rng);
    mend(&mut child, place, rng);
    fill(&mut child, place, rng);
    child
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        enemy,
        progression::{enemy_count, sector_budget},
        rng::Purpose,
        seed::seed_value,
    };

    const NORMAL: Difficulty = Difficulty::Normal;

    fn place(sector: u32) -> Place {
        Place {
            difficulty: NORMAL,
            sector,
        }
    }

    fn stream(number: u32) -> Stream {
        Stream::new(seed_value("BREED"), Purpose::Breed, number, 0)
    }

    /// Tanks as the generator makes them for a sector, with so many points each.
    fn made(sector: u32, budget: u32) -> Vec<Genome> {
        let tanks = Class::ALL.into_iter().enumerate();
        let made = tanks.filter(|(_, class)| comes(*class, sector));
        made.map(|(index, class)| {
            enemy::of_class(
                seed_value("BREED"),
                NORMAL,
                sector,
                index as u32,
                class,
                budget,
            )
        })
        .collect()
    }

    /// Children of every pair of the tanks that the generator makes for a sector.
    fn children(sector: u32, budget: u32) -> Vec<(Genome, Genome, Genome)> {
        let parents = made(sector, budget);
        let mut rng = stream(sector);
        let mut children = Vec::new();
        for one in &parents {
            for other in &parents {
                let child = child(one, other, budget, place(sector), &mut rng);
                children.push((*one, *other, child));
            }
        }
        children
    }

    fn levels(genome: &Genome) -> Vec<u8> {
        Stat::ALL.map(|stat| genome.level(stat)).to_vec()
    }

    #[test]
    fn every_tank_of_the_generator_is_sound() {
        // What breeding holds a tank to is what the generator holds its own to.
        for difficulty in [Difficulty::Normal, Difficulty::Hard] {
            for sector in (1..=60).chain([100, 300]) {
                let count = enemy_count(sector, 4, true);
                let place = Place { difficulty, sector };
                for seed in ["TANKU", "BREED", "SOUND"] {
                    for tank in enemy::group(seed_value(seed), difficulty, sector, count) {
                        assert!(sound(&tank, place), "sector {sector}: {tank:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_child_is_sound_and_has_spent_its_points() {
        for sector in [1, 4, 9, 16, 25, 40] {
            let budget = sector_budget(sector, NORMAL);
            let children = children(sector, budget);
            assert!(children.len() >= 4, "sector {sector}");
            for (one, other, child) in children {
                let said = format!("sector {sector}: {one:?} and {other:?} gave {child:?}");
                assert!(sound(&child, place(sector)), "{said}");
                assert!(spent(&child, place(sector)), "{said}");
                assert_eq!(child.budget as u32, budget);
                assert!(place(sector).classes().contains(&child.class), "{said}");
            }
        }
    }

    #[test]
    fn a_child_fits_the_points_that_it_is_given() {
        // The parents have a hundred points. A child that is given thirty costs no more, and
        // one that is given two hundred spends what the levels of its class can take.
        let parents = made(30, 100);
        let mut rng = stream(1);
        for one in &parents {
            for other in &parents {
                let poor = child(one, other, 30, place(30), &mut rng);
                assert!(poor.cost() <= 30 && sound(&poor, place(30)), "{poor:?}");
                assert!(spent(&poor, place(30)), "{poor:?}");
                let rich = child(one, other, 200, place(30), &mut rng);
                assert!(sound(&rich, place(30)) && spent(&rich, place(30)));
                assert!(rich.cost() > one.cost(), "{rich:?}");
            }
        }
    }

    #[test]
    fn two_tanks_are_recombined_level_by_level() {
        let parents = made(30, 80);
        let (one, other) = (parents[1], parents[7]);
        assert_ne!(levels(&one), levels(&other));
        let mut rng = stream(2);
        let mut from = [0u32; 2];
        let mut classes = [0u32; 2];
        for _ in 0..200 {
            let crossed = cross(&one, &other, 80, &mut rng);
            assert_eq!(crossed.budget, 80);
            for stat in Stat::ALL {
                let level = crossed.level(stat);
                let (first, second) = (one.level(stat), other.level(stat));
                assert!(level == first || level == second, "{stat:?}");
                if first != second {
                    from[(level == second) as usize] += 1;
                }
            }
            assert!(crossed.class == one.class || crossed.class == other.class);
            classes[(crossed.class == other.class) as usize] += 1;
        }
        // Both parents give about half of what they differ in.
        let share = from[0] as f32 / (from[0] + from[1]) as f32;
        assert!((0.4..0.6).contains(&share), "{from:?}");
        assert!(classes[0] > 70 && classes[1] > 70, "{classes:?}");
    }

    #[test]
    fn a_child_of_a_tank_and_itself_differs_from_it_by_little() {
        // With one tank for both parents, what differs is the one change and what it took
        // to pay for it.
        let mut rng = stream(3);
        let mut far = 0;
        let mut touched = [false; Stat::COUNT];
        let mut other_class = 0;
        let parents = made(30, 80);
        let rounds = 60;
        for parent in &parents {
            for _ in 0..rounds {
                let child = child(parent, parent, 80, place(30), &mut rng);
                if child.class != parent.class {
                    other_class += 1;
                    continue;
                }
                let moved: u32 = Stat::ALL
                    .into_iter()
                    .map(|stat| child.level(stat).abs_diff(parent.level(stat)) as u32)
                    .sum();
                far += (moved > 6) as u32;
                for stat in Stat::ALL {
                    touched[stat as usize] |= child.level(stat) != parent.level(stat);
                }
            }
        }
        let all = (parents.len() * rounds) as u32;
        // Few children are far from their parent, and breeding reaches every stat.
        assert!(far * 20 < all, "{far} of {all}");
        assert_eq!(touched, [true; Stat::COUNT]);
        // One child in eight draws its class anew, and one in eleven of those draws the same.
        let expected = all / CLASS_CHANGE;
        assert!(
            (expected / 2..expected * 3 / 2).contains(&other_class),
            "{other_class} of {all}"
        );
    }

    #[test]
    fn a_child_that_takes_another_class_gives_up_what_that_class_may_not_have() {
        // A trickster with three bounces and the best ballistics, bred with itself until
        // its children are of other classes.
        let trickster = made(30, 100)
            .into_iter()
            .find(|tank| tank.class == Class::Trickster)
            .unwrap();
        assert!(trickster.bounces() >= 2, "{trickster:?}");
        let mut rng = stream(4);
        let others: Vec<Genome> = (0..400)
            .map(|_| child(&trickster, &trickster, 100, place(30), &mut rng))
            .filter(|child| child.class != Class::Trickster)
            .collect();
        assert!(others.len() > 20);
        for child in others {
            assert!(child.bounces() <= child.class.highest(Stat::Ricochet));
            assert!(sound(&child, place(30)), "{child:?}");
        }
    }

    #[test]
    fn the_same_stream_breeds_the_same_child() {
        let parents = made(20, 60);
        let breed = |number: u32| {
            let mut rng = stream(number);
            let pairs = parents.iter().zip(parents.iter().rev());
            let bred = pairs.map(|(one, other)| child(one, other, 60, place(20), &mut rng));
            bred.collect::<Vec<Genome>>()
        };
        assert_eq!(breed(5), breed(5));
        assert_ne!(breed(5), breed(6));
    }

    #[test]
    fn what_a_stat_needs_is_kept_or_the_stat_goes() {
        let mut tank = Genome::empty(100);
        tank.class = Class::Seeker;
        for (stat, level) in [
            (Stat::Tracks, 3),
            (Stat::Mines, 2),
            (Stat::Shield, 1),
            (Stat::Seeking, 1),
            (Stat::Ballistics, 1),
            (Stat::Muzzle, 2),
        ] {
            tank.set(stat, level);
        }
        assert!(!sound(&tank, place(30)));
        settle(&mut tank);
        // No engine, no brain and no bounce: what needed them is gone, and the rest stays.
        let left: Vec<(Stat, u8)> = Stat::ALL
            .into_iter()
            .map(|stat| (stat, tank.level(stat)))
            .filter(|(_, level)| *level > 0)
            .collect();
        assert_eq!(left, [(Stat::Muzzle, 2), (Stat::Ballistics, 1)]);
        assert!(sound(&tank, place(30)));
        // A bank shot off two walls needs two bounces.
        let mut banker = Genome::empty(100);
        banker.class = Class::Trickster;
        banker.set(Stat::Ballistics, 3);
        banker.set(Stat::Ricochet, 1);
        assert!(!sound(&banker, place(30)));
        settle(&mut banker);
        assert_eq!(banker.ballistics(), 2);
        assert!(sound(&banker, place(30)));
    }
}
