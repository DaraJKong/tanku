//! Breeding at a fixed budget: the tank that costs a player the most for its points.
//!
//! The generator spends the points of a tank by the weights of its class. Breeding spends
//! them by what they turn out to be worth: tanks fight a reference player, the ones that
//! cost it the most tanks have children, with their levels recombined and slightly changed
//! (see `tanku_core::breed`), and so on for some generations. The best are then fought
//! again, in fights that none of them was chosen by, next to the tanks that the game makes
//! with the same points.
//!
//! What it is for is the prices. Every tank of the trial has the same points, so a tank
//! that does twice what the game's own do has found points that are worth twice their
//! price. The stats that the best tanks spend far more on than the game does are the ones
//! to look at, and those that they leave out do less than they cost.
//!
//! It is the breeding step of impossible mode, with a reference player in the place of
//! the players.

use std::collections::HashSet;

use tanku_core::{
    balance::{BALANCE, FINGERPRINT},
    breed::{self, Place},
    enemy,
    map::{Corridor, SectorKind},
    modes::Difficulty,
    rng::{Purpose, Stream},
    seed::{self, seed_value},
    stats::{Class, Genome, Stat},
};

use crate::{
    census::class_name,
    duels::written,
    fight::{Make, Outcome, Setup},
    players::{ratio, run, seed_text},
    reference::Reference,
    report::{Report, Table, percent},
};

/// What the breeding is asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    /// The points of every tank of the trial.
    pub budget: u32,
    /// The sectors that the tanks fight in, with their corridors and the experience of
    /// their brains. The tanks are bred under the caps of the last of them.
    pub sectors: (u32, u32),
    /// How many tanks of one make the player fights at once.
    pub count: usize,
    pub difficulty: Difficulty,
    pub player: Reference,
    /// The one class that the makes are bred of, if they are not bred of every class of the
    /// sector.
    pub class: Option<Class>,
    /// How long a fight lasts at most, in seconds.
    pub limit: f32,
    /// How many makes a generation holds.
    pub population: u32,
    pub generations: u32,
    /// How many fights each make of a generation fights.
    pub fights: u32,
    /// How many fights each of the best makes, and each class of the game, fights at the
    /// end.
    pub finals: u32,
    /// How many of the best makes are fought again and told.
    pub best: u32,
    /// The text that decides what is drawn: the first tanks, the parents of each child and
    /// what changes in it.
    pub seed: String,
    /// How many fights run side by side.
    pub threads: usize,
}

/// A fight of the trial: the number of its seed, and its sector.
type Slot = (u32, u32);

/// What makes a make: the levels of its stats. Two tanks that differ by their class alone
/// fight the same fights, so they are one make, and it is bred once.
type Levels = [u8; Stat::COUNT];

fn levels(genome: &Genome) -> Levels {
    Stat::ALL.map(|stat| genome.level(stat))
}

/// The fights of the trial, in the order they are fought: each on a seed of its own, through
/// the sectors in turn. A sector that is no corridor on its seed holds no fight, and is
/// passed over.
fn slots((first, last): (u32, u32)) -> impl Iterator<Item = Slot> {
    let span = last - first + 1;
    (0u32..).filter_map(move |number| {
        let sector = first + number % span;
        let mut corridor = Corridor::new(seed_value(&seed_text(number)));
        (corridor.plan(sector).kind == SectorKind::Corridor).then_some((number, sector))
    })
}

/// What some fights told of the tanks that fought them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Tally {
    fights: u32,
    /// The tanks that the player lost.
    losses: u32,
    /// The squares of the losses of each fight, summed: for how sure their mean is.
    squares: u32,
    tanks: u32,
    /// The seconds that the tanks lasted, all together.
    seconds: f64,
    /// The points that the tanks spent, all together.
    spent: u32,
    cleared: u32,
}

impl Tally {
    fn of(fights: &[Outcome]) -> Self {
        let mut tally = Self::default();
        for fight in fights {
            let lost = fight.losses.len() as u32;
            tally.fights += 1;
            tally.losses += lost;
            tally.squares += lost * lost;
            tally.tanks += fight.tanks() as u32;
            tally.cleared += fight.cleared() as u32;
            for tank in &fight.group {
                tally.seconds += tank.lasted as f64;
                tally.spent += tank.genome.cost();
            }
        }
        tally
    }

    fn with(self, other: Tally) -> Self {
        Self {
            fights: self.fights + other.fights,
            losses: self.losses + other.losses,
            squares: self.squares + other.squares,
            tanks: self.tanks + other.tanks,
            seconds: self.seconds + other.seconds,
            spent: self.spent + other.spent,
            cleared: self.cleared + other.cleared,
        }
    }

    /// The tanks that the player lost in a fight, which is what a make is bred for.
    fn score(&self) -> f64 {
        match self.fights {
            0 => 0.0,
            fights => self.losses as f64 / fights as f64,
        }
    }

    /// How long a tank lasted, in seconds.
    fn lasted(&self) -> f64 {
        self.seconds / self.tanks.max(1) as f64
    }

    /// The points that a tank spent.
    fn points(&self) -> f64 {
        self.spent as f64 / self.tanks.max(1) as f64
    }

    /// How far the score may be from what many more fights would tell: the standard error
    /// of the mean of the losses of the fights.
    fn error(&self) -> Option<f64> {
        let count = self.fights as f64;
        if self.fights < 2 {
            return None;
        }
        let spread = (self.squares as f64 - count * self.score().powi(2)) / (count - 1.0);
        Some((spread.max(0.0) / count).sqrt())
    }
}

/// A make of the trial, with what its fights have told so far.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Bred {
    genome: Genome,
    /// The generation that it first fought in. The tanks of the first are the game's own.
    born: u32,
    tally: Tally,
}

/// What a generation told.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Generation {
    makes: usize,
    /// The fights of the generation, for all its makes and for those that are new in it.
    all: Tally,
    new: Tally,
    /// The make that leads once the generation has fought, with all its fights so far.
    best: Bred,
    /// The makes that are kept, with all their fights so far, and how many of them are new.
    kept: Tally,
    entered: usize,
}

/// Fights each of some groups in each of some fights, side by side, and gives the fights of
/// each group.
fn fight(
    asked: &Asked,
    slots: &[Slot],
    groups: usize,
    tanks: impl Fn(usize, Slot) -> Vec<Genome>,
) -> Vec<Vec<Outcome>> {
    let mut setups = Vec::new();
    for group in 0..groups {
        for &(number, sector) in slots {
            setups.push(Setup {
                seed: seed_text(number),
                sector,
                difficulty: asked.difficulty,
                player: asked.player,
                limit: asked.limit,
                make: Some(Make {
                    tanks: tanks(group, (number, sector)),
                    spot: number as usize,
                }),
            });
        }
    }
    let mut outcomes = run(&setups, asked.threads).into_iter();
    let each = |_| outcomes.by_ref().take(slots.len()).flatten().collect();
    (0..groups).map(each).collect()
}

/// The classes that the makes are bred of: the one that was asked for, or every class of
/// the sector.
fn bred_of(asked: &Asked, place: Place) -> Vec<Class> {
    match asked.class {
        Some(class) => vec![class],
        None => place.classes(),
    }
}

/// The makes of the first generation: tanks as the game makes them with these points, class
/// after class, each one different from the others.
fn first(asked: &Asked, place: Place, seen: &mut HashSet<Levels>) -> Vec<Bred> {
    let classes = bred_of(asked, place);
    let seed = seed_value(&asked.seed);
    let mut makes = Vec::new();
    // A small budget buys few different tanks: the generation is then smaller.
    for index in 0..20 * asked.population {
        if makes.len() == asked.population as usize {
            break;
        }
        let class = classes[index as usize % classes.len()];
        let (difficulty, sector) = (place.difficulty, place.sector);
        let genome = enemy::of_class(seed, difficulty, sector, index, class, asked.budget);
        if seen.insert(levels(&genome)) {
            makes.push(Bred {
                genome,
                born: 1,
                tally: Tally::default(),
            });
        }
    }
    makes
}

/// How many makes of a generation have children: the best quarter, and two at least.
fn parents(population: usize) -> usize {
    (population / 4).max(2).min(population)
}

/// The next generation: the best makes of this one, which are sorted, and children of
/// theirs. A child that was already bred once is not bred again.
fn next(
    asked: &Asked,
    place: Place,
    mut makes: Vec<Bred>,
    generation: u32,
    seen: &mut HashSet<Levels>,
) -> Vec<Bred> {
    let kept = parents(makes.len());
    makes.truncate(kept);
    let mut rng = Stream::new(seed_value(&asked.seed), Purpose::Breed, generation, 0);
    for _ in 0..50 * asked.population {
        if makes.len() >= asked.population as usize {
            break;
        }
        // Two of the best, and not the same one twice.
        let one = rng.below(kept as u32) as usize;
        let other = (one + 1 + rng.below(kept as u32 - 1) as usize) % kept;
        let (one, other) = (makes[one].genome, makes[other].genome);
        let genome = breed::child(&one, &other, asked.budget, place, &mut rng);
        // A child that took a class that is not bred here is no child of this trial.
        if asked.class.is_some_and(|class| genome.class != class) {
            continue;
        }
        if seen.insert(levels(&genome)) {
            makes.push(Bred {
                genome,
                born: generation,
                tally: Tally::default(),
            });
        }
    }
    makes
}

/// Sorts makes, the one that costs the player the most first. Of two that cost as much, the
/// older comes first: it has more fights behind its score.
fn rank(makes: &mut [Bred]) {
    makes.sort_by(|a, b| {
        let by_score = b.tally.score().total_cmp(&a.tally.score());
        by_score.then(a.born.cmp(&b.born))
    });
}

/// The class that a make is told as: the first of those that may have its levels, where
/// the sector decides. A child keeps the class of a parent whatever it kept of that parent,
/// so a tank that was bred of seekers may have no bullet that seeks: it is then told as an
/// all-rounder, which is what any class can make.
fn told_as(genome: &Genome, place: Place) -> Class {
    let may = |class: &Class| {
        let within = |stat: Stat| genome.level(stat) <= place.highest(*class, stat);
        Stat::ALL.into_iter().all(within)
    };
    let first = place.classes().into_iter().find(may);
    first.unwrap_or(genome.class)
}

/// The points that a tank spent on a stat.
fn points(genome: &Genome, stat: Stat) -> u32 {
    let level = genome.level(stat);
    match stat.is_software() {
        true => stat.share(level, genome.budget as u32),
        false => stat.price(level),
    }
}

/// The weight of a class when the game draws the class of a tank.
fn weight(class: Class) -> f64 {
    BALANCE.classes[class as usize].weight as f64
}

/// The mean of what each class has of something, by the weights of the classes: what a tank
/// of the game has of it, whatever its class.
fn of_the_game(classes: &[Class], of: impl Fn(usize) -> f64) -> f64 {
    let weights: f64 = classes.iter().map(|class| weight(*class)).sum();
    let all = classes.iter().enumerate();
    let sum: f64 = all.map(|(index, class)| weight(*class) * of(index)).sum();
    match weights > 0.0 {
        true => sum / weights,
        false => 0.0,
    }
}

fn give_or_take(tally: &Tally) -> String {
    tally
        .error()
        .map_or("-".to_string(), |error| format!("{error:.2}"))
}

/// The table of the generations.
fn generations(told: &[Generation], place: Place) -> Table {
    let mut table = Table::new(
        "Generations",
        "For each generation: its makes, and the tanks that the player lost in a fight \
         against a make of it, a make that is new in it, a make that is kept for the next \
         one, and the make that leads. The first two count the fights of the generation, \
         and the last two every fight of those makes so far: a make that is kept fights \
         again in each generation, so its number gets surer, and a new make that leads by \
         luck falls back. The first generation is made of tanks of the game.",
        &[
            "generation",
            "makes",
            "lost to a make",
            "lost to a new make",
            "lost to a make that is kept",
            "lost to the best",
            "fights of the best",
            "class of the best",
            "new among those kept",
        ],
    );
    for (number, generation) in told.iter().enumerate() {
        table.row(vec![
            (number + 1).to_string(),
            generation.makes.to_string(),
            format!("{:.2}", generation.all.score()),
            format!("{:.2}", generation.new.score()),
            format!("{:.2}", generation.kept.score()),
            format!("{:.2}", generation.best.tally.score()),
            generation.best.tally.fights.to_string(),
            class_name(told_as(&generation.best.genome, place)).to_string(),
            generation.entered.to_string(),
        ]);
    }
    table
}

/// The mean of something over some tanks.
fn mean_of<'a>(tanks: impl Iterator<Item = &'a Genome>, of: impl Fn(&Genome) -> f64) -> f64 {
    let (sum, count) = tanks.fold((0.0, 0usize), |(sum, count), tank| {
        (sum + of(tank), count + 1)
    });
    sum / count.max(1) as f64
}

/// The tables of the makes that were fought at the end: the best of the breeding, each with
/// its fights, and the tanks of the game, class by class, each class with its fights.
fn finals(
    best: &[(Bred, Vec<Outcome>)],
    own: &[(Class, Vec<Outcome>)],
    place: Place,
) -> Vec<Table> {
    let classes: Vec<Class> = own.iter().map(|(class, _)| *class).collect();
    let tallies: Vec<Tally> = own.iter().map(|(_, fights)| Tally::of(fights)).collect();
    // What a tank of the game costs the player, whatever its class.
    let games = of_the_game(&classes, |class| tallies[class].score());
    let cells = |tally: &Tally| {
        vec![
            tally.fights.to_string(),
            format!("{:.2}", tally.score()),
            give_or_take(tally),
            ratio(tally.score(), games, 2),
            format!("{:.1}", tally.lasted()),
            percent(tally.cleared as usize, tally.fights as usize),
        ]
    };

    let mut makes = Table::new(
        "The best makes",
        "The makes that the breeding ended with, the one that costs the player the most \
         first: its class, its stats with their levels, the points that it spends and the \
         generation that it first fought in, which is 1 for a tank of the game. The class \
         is the first that may have those levels: the all-rounder for a make that any class \
         can have, whatever its parents were. Then what \
         the breeding knew of it, the tanks that the player lost in a fight against it over \
         all its fights there, and what the fights of the end tell, which none of the makes \
         was chosen by: the fights, the tanks lost in one, how far that number may be from \
         the truth, that number for each tank that a fight against tanks of the game costs, \
         how long a tank of the make lasted and the fights in which all of them fell. A \
         make that led by luck does less at the end than while it was bred.",
        &[
            "make",
            "class",
            "stats",
            "points",
            "since generation",
            "fights while bred",
            "lost while bred",
            "fights",
            "lost in a fight",
            "give or take",
            "for each of the game's",
            "seconds",
            "cleared",
        ],
    );
    for (number, (bred, fights)) in best.iter().enumerate() {
        let mut row = vec![
            (number + 1).to_string(),
            class_name(told_as(&bred.genome, place)).to_string(),
            written(&bred.genome),
            bred.genome.cost().to_string(),
            bred.born.to_string(),
            bred.tally.fights.to_string(),
            format!("{:.2}", bred.tally.score()),
        ];
        row.extend(cells(&Tally::of(fights)));
        makes.row(row);
    }

    let mut games_own = Table::new(
        "The tanks of the game",
        "The same fights against tanks that the game makes with the same points, class by \
         class, each tank made apart: the weight of the class when a tank draws its class, \
         the fights, the tanks that the player lost in one, how far that number may be from \
         the truth, that number for each tank that a fight against tanks of the game costs \
         whatever their class, how long a tank lasted, the fights in which all of them fell \
         and the points that a tank spent. The last row is a tank of the game whatever its \
         class: each class by its weight.",
        &[
            "class",
            "weight",
            "fights",
            "lost in a fight",
            "give or take",
            "for each of the game's",
            "seconds",
            "cleared",
            "points",
        ],
    );
    for (class, tally) in classes.iter().zip(&tallies) {
        let mut row = vec![class_name(*class).to_string(), weight(*class).to_string()];
        row.extend(cells(tally));
        row.push(format!("{:.0}", tally.points()));
        games_own.row(row);
    }
    let fights: u32 = tallies.iter().map(|tally| tally.fights).sum();
    let weights: f64 = classes.iter().map(|class| weight(*class)).sum();
    games_own.row(vec![
        "All, by their weights".to_string(),
        weights.to_string(),
        fights.to_string(),
        format!("{games:.2}"),
        "-".to_string(),
        ratio(games, games, 2),
        format!(
            "{:.1}",
            of_the_game(&classes, |class| tallies[class].lasted())
        ),
        "-".to_string(),
        format!(
            "{:.0}",
            of_the_game(&classes, |class| tallies[class].points())
        ),
    ]);

    let mut spending = Table::new(
        "Where the points go",
        "For each stat: its level on the best makes and on the tanks of the game, each as a \
         mean, the points that they spend on it, and the points of the best makes for each \
         point of the game's. A tank of the game is one of the fights above, whatever its \
         class: each class by its weight. A stat that the best makes spend far more on than \
         the game does more for its price than the others, at these points and against \
         this player, and a stat that they leave out does less. Software is paid as a share \
         of the budget.",
        &[
            "stat",
            "level, the best",
            "level, the game's",
            "points, the best",
            "points, the game's",
            "points for each of the game's",
        ],
    );
    // The tanks that each class sent to its fights.
    let sent: Vec<Vec<Genome>> = own
        .iter()
        .map(|(_, fights)| fights.iter().flat_map(|fight| &fight.group))
        .map(|tanks| tanks.map(|tank| tank.genome).collect())
        .collect();
    let the_best =
        |of: &dyn Fn(&Genome) -> f64| mean_of(best.iter().map(|(bred, _)| &bred.genome), of);
    let the_games = |of: &dyn Fn(&Genome) -> f64| {
        of_the_game(&classes, |class| mean_of(sent[class].iter(), of))
    };
    let one = |value: f64| format!("{value:.1}");
    for stat in Stat::ALL {
        let level = |genome: &Genome| genome.level(stat) as f64;
        let cost = |genome: &Genome| points(genome, stat) as f64;
        spending.row(vec![
            stat.name().to_string(),
            one(the_best(&level)),
            one(the_games(&level)),
            one(the_best(&cost)),
            one(the_games(&cost)),
            ratio(the_best(&cost), the_games(&cost), 2),
        ]);
    }
    let all = |genome: &Genome| genome.cost() as f64;
    spending.row(vec![
        "all".to_string(),
        "-".to_string(),
        "-".to_string(),
        one(the_best(&all)),
        one(the_games(&all)),
        ratio(the_best(&all), the_games(&all), 2),
    ]);

    vec![makes, games_own, spending]
}

pub fn breed(asked: &Asked) -> Report {
    let (first_sector, last_sector) = asked.sectors;
    let place = Place {
        difficulty: asked.difficulty,
        sector: last_sector,
    };
    let mut slots = slots(asked.sectors);
    let mut seen = HashSet::new();
    let mut makes = first(asked, place, &mut seen);
    let mut told: Vec<Generation> = Vec::new();
    for generation in 1..=asked.generations {
        let fights: Vec<Slot> = slots.by_ref().take(asked.fights as usize).collect();
        let fought = fight(asked, &fights, makes.len(), |make, _| {
            vec![makes[make].genome; asked.count]
        });
        let (mut all, mut new) = (Tally::default(), Tally::default());
        for (bred, fights) in makes.iter_mut().zip(&fought) {
            let tally = Tally::of(fights);
            bred.tally = bred.tally.with(tally);
            all = all.with(tally);
            if bred.born == generation {
                new = new.with(tally);
            }
        }
        rank(&mut makes);
        let kept = &makes[..parents(makes.len())];
        told.push(Generation {
            makes: makes.len(),
            all,
            new,
            best: makes[0],
            kept: kept
                .iter()
                .fold(Tally::default(), |sum, bred| sum.with(bred.tally)),
            entered: kept.iter().filter(|bred| bred.born == generation).count(),
        });
        if generation < asked.generations {
            makes = next(asked, place, makes, generation + 1, &mut seen);
        }
    }

    // The best makes and the tanks of the game, in fights that chose none of them.
    makes.truncate(asked.best as usize);
    let fights: Vec<Slot> = slots.by_ref().take(asked.finals as usize).collect();
    let fought = fight(asked, &fights, makes.len(), |make, _| {
        vec![makes[make].genome; asked.count]
    });
    let mut best: Vec<(Bred, Vec<Outcome>)> = makes.into_iter().zip(fought).collect();
    best.sort_by(|a, b| {
        let (ours, theirs) = (Tally::of(&a.1).score(), Tally::of(&b.1).score());
        theirs.total_cmp(&ours)
    });
    let classes = place.classes();
    let made = |class: usize, (number, _): Slot| -> Vec<Genome> {
        let seed = seed_value(&seed_text(number));
        let tank = |index: usize| {
            let (difficulty, sector) = (place.difficulty, place.sector);
            enemy::of_class(
                seed,
                difficulty,
                sector,
                index as u32,
                classes[class],
                asked.budget,
            )
        };
        (0..asked.count).map(tank).collect()
    };
    let fought = fight(asked, &fights, classes.len(), made);
    let own: Vec<(Class, Vec<Outcome>)> = classes.iter().copied().zip(fought).collect();

    let mut tables = finals(&best, &own, place);
    tables.push(generations(&told, place));
    let together = match asked.count {
        1 => "one tank".to_string(),
        count => format!("{count} tanks of one make at once"),
    };
    let of = match asked.class {
        Some(class) => format!("the class {} alone", class.name()),
        None => "every class of that sector".to_string(),
    };
    Report {
        title: "Breeding".to_string(),
        asked: format!(
            "Version {}, {} mode, the {} player in the standard tank against {together}, \
             each with {} points, bred of {of} under the caps of sector {last_sector}, in \
             the corridors of sectors {first_sector} to {last_sector} with their brains. {} \
             generations of {} makes, drawn from the text `{}`: the first is made as the \
             game makes its \
             tanks, and each of the others is the best quarter of the one before it and \
             children of theirs. A make fights {} fights in each generation that it is in, \
             each on a seed of its own, from {} on. The {} best then fight {} fights more, \
             and so do tanks of each class of the game. A fight lasts {} s at most.",
            seed::VERSION,
            asked.difficulty.key(),
            asked.player.name(),
            asked.budget,
            asked.generations,
            asked.population,
            asked.seed,
            asked.fights,
            seed_text(0),
            asked.best,
            asked.finals,
            asked.limit,
        ),
        balance: Some(FINGERPRINT),
        tables,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked() -> Asked {
        Asked {
            budget: 50,
            sectors: (21, 24),
            count: 2,
            difficulty: Difficulty::Normal,
            player: Reference::Regular,
            class: None,
            limit: 6.0,
            population: 8,
            generations: 2,
            fights: 2,
            finals: 2,
            best: 3,
            seed: "BREED".to_string(),
            threads: 8,
        }
    }

    fn place() -> Place {
        Place {
            difficulty: Difficulty::Normal,
            sector: 24,
        }
    }

    /// A fight in which the player lost so many tanks to two tanks of a make.
    fn outcome(lost: usize, genome: Genome) -> Outcome {
        use crate::fight::{Cause, Fought, Loss};
        let loss = Loss {
            cause: Cause::Bullet,
            by: Some(0),
        };
        Outcome {
            group: vec![
                Fought {
                    genome,
                    lasted: 10.0,
                    fell: true,
                };
                2
            ],
            lasted: 10.0,
            losses: vec![loss; lost],
            own: 0,
        }
    }

    #[test]
    fn the_fights_of_the_trial_are_corridors_each_on_a_seed_of_its_own() {
        let fights: Vec<Slot> = slots((21, 24)).take(40).collect();
        let mut numbers: Vec<u32> = fights.iter().map(|(number, _)| *number).collect();
        numbers.dedup();
        assert_eq!(numbers.len(), 40);
        assert!(numbers.is_sorted());
        for (number, sector) in fights {
            assert_eq!(sector, 21 + number % 4);
            let mut corridor = Corridor::new(seed_value(&seed_text(number)));
            assert_eq!(corridor.plan(sector).kind, SectorKind::Corridor);
        }
        // Some seeds have an arena there, which is passed over.
        let last = slots((21, 24)).nth(39).unwrap().0;
        assert!(last > 39, "{last}");
    }

    #[test]
    fn a_tally_tells_what_a_fight_costs_and_how_sure_that_is() {
        let genome = Genome::empty(50);
        let fights: Vec<Outcome> = [0, 1, 1, 2].map(|lost| outcome(lost, genome)).to_vec();
        let tally = Tally::of(&fights);
        assert_eq!((tally.fights, tally.losses, tally.tanks), (4, 4, 8));
        assert_eq!(tally.score(), 1.0);
        assert_eq!(tally.seconds, 80.0);
        assert_eq!(tally.cleared, 4);
        // The losses are 0, 1, 1 and 2: they spread by two thirds, over four fights.
        let error = tally.error().unwrap();
        assert!(
            (error - (2.0f64 / 3.0 / 4.0).sqrt()).abs() < 1e-9,
            "{error}"
        );
        // Two halves tell what the whole does, and one fight tells nothing of the spread.
        let (some, others) = (Tally::of(&fights[..1]), Tally::of(&fights[1..]));
        assert_eq!(some.with(others), tally);
        assert_eq!(some.error(), None);
        assert_eq!(Tally::default().score(), 0.0);
    }

    #[test]
    fn the_first_generation_is_made_of_different_tanks_of_the_game() {
        let mut seen = HashSet::new();
        let asked = Asked {
            population: 22,
            ..asked()
        };
        let makes = first(&asked, place(), &mut seen);
        assert_eq!(makes.len(), 22);
        assert_eq!(seen.len(), 22);
        // Every class of the sector is there, twice.
        for class in place().classes() {
            let of_it = makes.iter().filter(|bred| bred.genome.class == class);
            assert_eq!(of_it.count(), 2, "{class:?}");
        }
        for bred in &makes {
            assert_eq!((bred.born, bred.tally), (1, Tally::default()));
            assert_eq!(bred.genome.budget, 50);
            assert!(breed::sound(&bred.genome, place()));
        }
        // Another text draws other tanks.
        let other = Asked {
            seed: "OTHER".to_string(),
            ..asked
        };
        let others = first(&other, place(), &mut HashSet::new());
        let same = |(one, other): (&Bred, &Bred)| one.genome == other.genome;
        assert!(!makes.iter().zip(&others).all(same));
    }

    #[test]
    fn the_best_makes_are_kept_and_have_the_children_of_the_next_generation() {
        let asked = Asked {
            population: 12,
            ..asked()
        };
        let mut seen = HashSet::new();
        let mut makes = first(&asked, place(), &mut seen);
        // The makes did as well as their place in the list is far: the last is the best.
        for (number, bred) in makes.iter_mut().enumerate() {
            let fights: Vec<Outcome> = (0..2).map(|_| outcome(number, bred.genome)).collect();
            bred.tally = Tally::of(&fights);
        }
        let best: Vec<Genome> = makes.iter().rev().take(3).map(|bred| bred.genome).collect();
        rank(&mut makes);
        let ranked: Vec<Genome> = makes.iter().take(3).map(|bred| bred.genome).collect();
        assert_eq!(ranked, best);
        let next = next(&asked, place(), makes, 2, &mut seen);
        assert_eq!(next.len(), 12);
        // A quarter is kept, with what its fights told.
        let kept: Vec<Genome> = next.iter().take(3).map(|bred| bred.genome).collect();
        assert_eq!(kept, best);
        assert!(
            next[..3]
                .iter()
                .all(|bred| bred.born == 1 && bred.tally.fights == 2)
        );
        // The rest is new, sound, and like no make that was there before.
        for bred in &next[3..] {
            assert_eq!((bred.born, bred.tally.fights), (2, 0));
            assert!(breed::sound(&bred.genome, place()), "{:?}", bred.genome);
            assert!(breed::spent(&bred.genome, place()), "{:?}", bred.genome);
        }
        assert_eq!(seen.len(), 12 + 9);
        // Of two makes that did as well, the older leads.
        let mut pair = [next[5], next[0]];
        (pair[0].tally, pair[1].tally) = (pair[1].tally, pair[1].tally);
        rank(&mut pair);
        assert_eq!(pair[0].born, 1);
    }

    #[test]
    fn a_make_is_told_as_the_first_class_that_may_have_it() {
        // A tank that was bred of seekers and kept nothing that only a seeker has.
        let mut plain = Genome::empty(50);
        plain.class = Class::Seeker;
        plain.set(Stat::Muzzle, 4);
        plain.set(Stat::Ricochet, 1);
        assert_eq!(told_as(&plain, place()), Class::None);
        // One that seeks is a seeker, and one that bounces twice is of the first class that
        // has such bullets.
        let mut seeker = plain;
        seeker.set(Stat::Seeking, 1);
        assert_eq!(told_as(&seeker, place()), Class::Seeker);
        let mut bouncer = plain;
        bouncer.set(Stat::Ricochet, 2);
        assert_eq!(told_as(&bouncer, place()), Class::Tactician);
        // What no class may have keeps the class that it came with.
        let mut odd = plain;
        odd.set(Stat::Seeking, 1);
        odd.set(Stat::Exploding, 1);
        assert_eq!(told_as(&odd, place()), Class::Seeker);
    }

    #[test]
    fn makes_can_be_bred_of_one_class_alone() {
        let asked = Asked {
            population: 12,
            class: Some(Class::Sniper),
            ..asked()
        };
        let mut seen = HashSet::new();
        let mut makes = first(&asked, place(), &mut seen);
        assert_eq!(makes.len(), 12);
        for generation in 2..6 {
            rank(&mut makes);
            makes = next(&asked, place(), makes, generation, &mut seen);
            assert_eq!(makes.len(), 12);
            assert!(makes.iter().all(|bred| bred.genome.class == Class::Sniper));
            // No sniper has a special bullet, or a bullet that bounces twice.
            for bred in &makes {
                assert_eq!(bred.genome.ammo(), tanku_core::stats::Ammo::Normal);
                assert!(bred.genome.bounces() <= 1);
            }
        }
    }

    #[test]
    fn the_report_sets_the_best_makes_next_to_the_tanks_of_the_game() {
        // A breeding that is small enough for a test: six makes, two generations, a fight
        // each, and two makes that fight twice at the end.
        let asked = Asked {
            count: 1,
            limit: 4.0,
            population: 6,
            fights: 1,
            best: 2,
            ..asked()
        };
        let report = breed(&asked);
        assert_eq!(report.title, "Breeding");
        let titles: Vec<&str> = report
            .tables
            .iter()
            .map(|table| table.title.as_str())
            .collect();
        assert_eq!(
            titles,
            [
                "The best makes",
                "The tanks of the game",
                "Where the points go",
                "Generations",
            ]
        );
        let cell = |table: usize, row: usize, column: &str| -> &str {
            let table = &report.tables[table];
            let at = table.columns.iter().position(|name| name == column);
            &table.rows[row][at.expect(column)]
        };
        // Two makes, each within its points, fought twice at the end.
        assert_eq!(report.tables[0].rows.len(), 2);
        for row in 0..2 {
            let points: u32 = cell(0, row, "points").parse().unwrap();
            assert!(points <= 50, "{points}");
            assert_eq!(cell(0, row, "fights"), "2");
            assert!(!cell(0, row, "stats").is_empty());
        }
        // The best of the end comes first.
        let lost = |row: usize| cell(0, row, "lost in a fight").parse::<f64>().unwrap();
        assert!(lost(0) >= lost(1));
        // A row for each class of the sector, and one for all of them.
        let classes = place().classes().len();
        assert_eq!(report.tables[1].rows.len(), classes + 1);
        assert_eq!(cell(1, classes, "class"), "All, by their weights");
        // A row for each stat, and one for all of them: no tank spends more than it has.
        assert_eq!(report.tables[2].rows.len(), Stat::COUNT + 1);
        let spent: f64 = cell(2, Stat::COUNT, "points, the best").parse().unwrap();
        assert!(spent <= 50.0 && spent > 40.0, "{spent}");
        // Two generations of six makes. The best of the first has fought once.
        assert_eq!(report.tables[3].rows.len(), 2);
        assert_eq!(cell(3, 0, "makes"), "6");
        assert_eq!(cell(3, 0, "fights of the best"), "1");
        // The same breeding is the same report.
        assert_eq!(report.csv(), breed(&asked).csv());
    }
}
