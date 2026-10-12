//! The census: what the rules make of each stretch of sectors, counted on many seeds.
//!
//! Nothing is fought. The census draws the corridor and the groups of every seed as the game
//! does, and counts: what the levers of the progression are worth, how a group shares its
//! points, which classes come and what they really carry, which bullets fly, how smart the
//! tanks are and what stands on the floor.

use tanku_core::{
    enemy,
    map::{Corridor, SectorKind},
    modes::Difficulty,
    progression,
    seed::{self, seed_value},
    stats::{Ammo, Class, Genome, Stat},
};

use crate::report::{Report, Table, mean, percent};

/// The stretches of sectors that a census counts, unless it counts each sector apart.
const STRETCHES: [(u32, u32); 7] = [
    (1, 2),
    (3, 5),
    (6, 10),
    (11, 15),
    (16, 20),
    (21, 30),
    (31, 50),
];

/// What a census is asked to count.
pub struct Asked {
    pub sectors: (u32, u32),
    pub seeds: u32,
    pub players: u32,
    pub difficulty: Difficulty,
    /// One row for each sector, in place of one for each stretch.
    pub each: bool,
}

/// One sector of one seed, as the game makes it.
struct Sector {
    kind: SectorKind,
    tanks: Vec<Genome>,
    /// The floor of the sector, in square meters.
    floor: f64,
    blocks: usize,
    crates: usize,
}

/// The points of each tank of a sector.
fn points(sector: &Sector) -> impl Iterator<Item = u32> + '_ {
    sector.tanks.iter().map(|tank| tank.budget as u32)
}

/// The sectors of one row of the tables, on every seed.
struct Stretch {
    first: u32,
    last: u32,
    sectors: Vec<Sector>,
}

impl Stretch {
    fn name(&self) -> String {
        if self.first == self.last {
            self.first.to_string()
        } else {
            format!("{}-{}", self.first, self.last)
        }
    }

    /// The sectors that hold a group: every one but the start bay.
    fn groups(&self) -> impl Iterator<Item = &Sector> + Clone {
        self.sectors
            .iter()
            .filter(|sector| !sector.tanks.is_empty())
    }

    fn tanks(&self) -> impl Iterator<Item = &Genome> + Clone {
        self.groups().flat_map(|sector| &sector.tanks)
    }
}

/// The rows of a census: the stretches that lie in the sectors asked for, cut to them.
fn rows(asked: &Asked) -> Vec<(u32, u32)> {
    let (first, last) = asked.sectors;
    if asked.each {
        return (first..=last).map(|sector| (sector, sector)).collect();
    }
    let mut rows: Vec<(u32, u32)> = STRETCHES
        .into_iter()
        .filter(|stretch| stretch.1 >= first && stretch.0 <= last)
        .map(|stretch| (stretch.0.max(first), stretch.1.min(last)))
        .collect();
    // Past the last stretch, the rest is one row.
    let end = STRETCHES[STRETCHES.len() - 1].1;
    if last > end {
        rows.push((first.max(end + 1), last));
    }
    rows
}

/// The seed of a census by its number, as a player types it.
pub fn seed_text(number: u32) -> String {
    format!("CENSUS{number}")
}

/// The floor of a sector from its outline, in square meters.
fn floor(outline: &[(i64, i64)]) -> f64 {
    let count = outline.len();
    let doubled: i64 = (0..count)
        .map(|corner| {
            let (here, next) = (outline[corner], outline[(corner + 1) % count]);
            here.0 * next.1 - next.0 * here.1
        })
        .sum();
    doubled.abs() as f64 / 2.0
}

/// Draws every sector of every seed as the game does.
fn sample(asked: &Asked) -> Vec<Stretch> {
    let mut stretches: Vec<Stretch> = rows(asked)
        .into_iter()
        .map(|(first, last)| Stretch {
            first,
            last,
            sectors: Vec::new(),
        })
        .collect();
    for number in 0..asked.seeds {
        let seed = seed_value(&seed_text(number));
        let mut corridor = Corridor::new(seed);
        for stretch in &mut stretches {
            for index in stretch.first..=stretch.last {
                let layout = corridor.layout(index);
                let kind = layout.plan.kind;
                let arena = kind == SectorKind::Arena;
                let count = progression::enemy_count(index, asked.players, arena);
                let mut tanks = enemy::group(seed, asked.difficulty, index, count);
                // A sector that is too small for its whole group holds as many as fit.
                tanks.truncate(layout.spawns.len());
                let corners = layout.plan.outline.iter();
                let outline: Vec<(i64, i64)> = corners
                    .map(|corner| (corner.x as i64, corner.y as i64))
                    .collect();
                stretch.sectors.push(Sector {
                    kind,
                    tanks,
                    floor: floor(&outline),
                    blocks: layout.blocks.len(),
                    crates: layout.crates.len(),
                });
            }
        }
    }
    stretches
}

/// A value at the two ends of a stretch: one number where they are the same.
fn span<T: PartialEq + ToString>(first: T, last: T) -> String {
    if first == last {
        first.to_string()
    } else {
        format!("{} to {}", first.to_string(), last.to_string())
    }
}

/// What the levers of the progression are worth: these are formulas, and no seed changes
/// them.
fn levers(asked: &Asked, stretches: &[Stretch]) -> Table {
    let mut table = Table::new(
        "Levers",
        "What the progression gives each sector, at the first and at the last sector of a row: \
         the sector budget, the tanks of a group in a corridor, the highest tiers of software, \
         the fastest gun, the reaction and the aiming error of the brains, and the terrain.",
        &[
            "sectors",
            "budget",
            "tanks",
            "brain",
            "ballistics",
            "gun, m/s",
            "reaction, s",
            "aim error",
            "width, m",
            "cover",
            "crates",
            "angled",
        ],
    );
    let difficulty = asked.difficulty;
    for stretch in stretches {
        let ends = |value: &dyn Fn(u32) -> String| span(value(stretch.first), value(stretch.last));
        let gun = |sector: u32| {
            let mut tank = Genome::empty(0);
            tank.set(Stat::Muzzle, progression::muzzle_cap(sector, difficulty));
            tank.bullet_speed().to_string()
        };
        // What the brains make of their experience.
        let experience = |sector: u32| progression::experience(sector, difficulty);
        let width = |sector: u32| {
            let (narrow, wide) = progression::corridor_width(sector);
            format!("{narrow}-{wide}")
        };
        let crates = |sector: u32| {
            let (fewest, most) = progression::crate_count(sector);
            format!("{fewest}-{most}")
        };
        table.row(vec![
            stretch.name(),
            ends(&|sector| progression::sector_budget(sector, difficulty).to_string()),
            ends(&|sector| progression::enemy_count(sector, asked.players, false).to_string()),
            ends(&|sector| progression::brain_cap(sector, difficulty).to_string()),
            ends(&|sector| progression::ballistics_cap(sector, difficulty).to_string()),
            ends(&gun),
            ends(&|sector| format!("{:.2}", 0.50 - 0.38 * experience(sector))),
            ends(&|sector| format!("{:.1}\u{b0}", 8.0 - 7.0 * experience(sector))),
            ends(&width),
            ends(&|sector| progression::cover_groups(sector).to_string()),
            ends(&crates),
            ends(&|sector| format!("{}%", progression::angled_wall_chance(sector))),
        ]);
    }
    table
}

fn median(mut values: Vec<u32>) -> String {
    if values.is_empty() {
        return "-".to_string();
    }
    values.sort_unstable();
    values[values.len() / 2].to_string()
}

/// How the groups share their points.
fn groups(stretches: &[Stretch]) -> Table {
    let mut table = Table::new(
        "Groups",
        "How many groups were counted, how many of them are in an arena, the tanks and the \
         points of a group, the points of a tank (the least, the median and the most), those \
         of the leader of a group of several, how many times the points of its weakest tank \
         its strongest has, and the points that a tank leaves unspent.",
        &[
            "sectors",
            "groups",
            "arenas",
            "tanks",
            "points",
            "least",
            "median",
            "most",
            "leader",
            "strongest / weakest",
            "unspent",
        ],
    );
    for stretch in stretches {
        let groups = stretch.groups();
        let count = groups.clone().count();
        let arenas = groups
            .clone()
            .filter(|sector| sector.kind == SectorKind::Arena);
        let budgets: Vec<u32> = stretch.tanks().map(|tank| tank.budget as u32).collect();
        let several = groups.clone().filter(|sector| sector.tanks.len() > 1);
        let leaders = several.clone().filter_map(|sector| points(sector).max());
        let spreads = several.filter_map(|sector| {
            let (most, least) = (points(sector).max()?, points(sector).min()?);
            Some(most as f64 / least.max(1) as f64)
        });
        let unspent = stretch
            .tanks()
            .map(|tank| tank.budget as f64 - tank.cost() as f64);
        table.row(vec![
            stretch.name(),
            count.to_string(),
            percent(arenas.count(), count),
            mean(groups.clone().map(|sector| sector.tanks.len() as f64)),
            mean(groups.map(|sector| points(sector).sum::<u32>() as f64)),
            budgets.iter().min().map_or("-".to_string(), u32::to_string),
            median(budgets.clone()),
            budgets.iter().max().map_or("-".to_string(), u32::to_string),
            mean(leaders.map(f64::from)),
            mean(spreads),
            mean(unspent),
        ]);
    }
    table
}

/// The name of a class, as the players read it.
fn class_name(class: Class) -> &'static str {
    match class {
        Class::None => "All-rounder",
        Class::Sniper => "Sniper",
        Class::Scout => "Scout",
        Class::Tactician => "Tactician",
        Class::Trickster => "Trickster",
        Class::Lancer => "Lancer",
        Class::Sapper => "Sapper",
        Class::Gunner => "Gunner",
        Class::Seeker => "Seeker",
        Class::Flanker => "Flanker",
        Class::Grenadier => "Grenadier",
    }
}

/// A table with a row for each class and a column for each stretch of sectors.
fn by_class(
    title: &str,
    note: &str,
    stretches: &[Stretch],
    cell: impl Fn(Class, &Stretch) -> String,
) -> Table {
    let names: Vec<String> = stretches.iter().map(Stretch::name).collect();
    let mut columns = vec!["class"];
    columns.extend(names.iter().map(String::as_str));
    let mut table = Table::new(title, note, &columns);
    for class in Class::ALL {
        let mut row = vec![class_name(class).to_string()];
        row.extend(stretches.iter().map(|stretch| cell(class, stretch)));
        table.row(row);
    }
    table
}

/// Whether a tank has what its color announces: a level of every stat that its class favors.
/// A tank without a class announces nothing.
fn shows_its_class(tank: &Genome) -> bool {
    let favorites = tank.class.favorites().iter();
    favorites.clone().all(|stat| tank.level(*stat) > 0)
}

/// Which classes come, and what they really carry.
fn classes(stretches: &[Stretch]) -> [Table; 3] {
    let of = |class: Class, stretch: &Stretch| -> Vec<Genome> {
        let tanks = stretch.tanks().filter(|tank| tank.class == class);
        tanks.copied().collect()
    };
    let share = by_class(
        "Classes",
        "The share of the tanks that each class has, by sectors.",
        stretches,
        |class, stretch| percent(of(class, stretch).len(), stretch.tanks().count()),
    );
    let shown = by_class(
        "Classes that show",
        "The share of the tanks of a class that have at least one level of every stat that \
         the class favors. The color of a tank announces those stats: a tank that lacks one \
         wears a color that it doesn't keep.",
        stretches,
        |class, stretch| {
            let tanks = of(class, stretch);
            if class == Class::None {
                return "-".to_string();
            }
            percent(
                tanks.iter().filter(|tank| shows_its_class(tank)).count(),
                tanks.len(),
            )
        },
    );
    let points = by_class(
        "Points of a class",
        "The mean of the points that a tank of each class spends, by sectors.",
        stretches,
        |class, stretch| mean(of(class, stretch).iter().map(|tank| tank.cost() as f64)),
    );
    [share, shown, points]
}

/// Which bullets fly.
fn bullets(stretches: &[Stretch]) -> Table {
    let mut table = Table::new(
        "Bullets",
        "The share of the tanks that fire each type of bullet, the share of the groups that \
         hold a special type and of those that hold two different ones, the bounces of a \
         bullet, and the speed of the bullets: their mean and the fastest.",
        &[
            "sectors",
            "normal",
            "piercing",
            "seeking",
            "splitting",
            "exploding",
            "groups with one",
            "with two types",
            "bounces",
            "speed, m/s",
            "fastest",
        ],
    );
    for stretch in stretches {
        let tanks = stretch.tanks().count();
        let firing = |ammo: Ammo| stretch.tanks().filter(|tank| tank.ammo() == ammo).count();
        // The special types that a group holds, each counted once.
        let types = |sector: &Sector| {
            let held = |ammo: &Ammo| sector.tanks.iter().any(|tank| tank.ammo() == *ammo);
            Ammo::ALL
                .iter()
                .filter(|ammo| **ammo != Ammo::Normal && held(ammo))
                .count()
        };
        let groups = stretch.groups().count();
        let with = |least: usize| {
            stretch
                .groups()
                .filter(|sector| types(sector) >= least)
                .count()
        };
        let fastest = stretch
            .tanks()
            .map(Genome::bullet_speed)
            .fold(0.0, f32::max);
        let mut row = vec![stretch.name()];
        row.extend(Ammo::ALL.map(|ammo| percent(firing(ammo), tanks)));
        row.extend([
            percent(with(1), groups),
            percent(with(2), groups),
            mean(stretch.tanks().map(|tank| tank.bounces() as f64)),
            mean(stretch.tanks().map(|tank| tank.bullet_speed() as f64)),
            fastest.to_string(),
        ]);
        table.row(row);
    }
    table
}

/// How smart the tanks are.
fn software(stretches: &[Stretch]) -> Table {
    let mut table = Table::new(
        "Software",
        "The share of the tanks that have each tier of brain, then each tier of ballistics.",
        &[
            "sectors",
            "brain 0",
            "brain 1",
            "brain 2",
            "brain 3",
            "brain 4",
            "ballistics 0",
            "ballistics 1",
            "ballistics 2",
            "ballistics 3",
        ],
    );
    for stretch in stretches {
        let tanks = stretch.tanks().count();
        let with = |tier: fn(&Genome) -> u8, wanted: u8| {
            percent(
                stretch.tanks().filter(|tank| tier(tank) == wanted).count(),
                tanks,
            )
        };
        let mut row = vec![stretch.name()];
        row.extend((0..=4).map(|tier| with(Genome::brain, tier)));
        row.extend((0..=3).map(|tier| with(Genome::ballistics, tier)));
        table.row(row);
    }
    table
}

/// What the tanks are made of.
fn hardware(stretches: &[Stretch]) -> Table {
    let mut table = Table::new(
        "Hardware",
        "The share of the tanks that can't move, the speed of those that can, the rounds of a \
         magazine, the time to reload one, and the share of the tanks that have mines and of \
         those that have a shield.",
        &[
            "sectors",
            "fixed",
            "speed, m/s",
            "rounds",
            "reload, s",
            "mines",
            "shield",
        ],
    );
    for stretch in stretches {
        let tanks = stretch.tanks().count();
        let share = |has: fn(&Genome) -> bool| {
            percent(stretch.tanks().filter(|tank| has(tank)).count(), tanks)
        };
        let moving = stretch.tanks().filter(|tank| tank.top_speed() > 0.0);
        table.row(vec![
            stretch.name(),
            share(|tank| tank.top_speed() == 0.0),
            mean(moving.map(|tank| tank.top_speed() as f64)),
            mean(stretch.tanks().map(|tank| tank.magazine() as f64)),
            mean(stretch.tanks().map(|tank| tank.reload_time() as f64)),
            share(|tank| tank.mines() > 0),
            share(|tank| tank.shield_cooldown().is_some()),
        ]);
    }
    table
}

/// What stands on the floor.
fn terrain(stretches: &[Stretch]) -> Table {
    let mut table = Table::new(
        "Terrain",
        "The floor of a sector that is a corridor, its blocks and its crates as the seeds draw \
         them, how many blocks stand on 100 square meters of it, and the floor that each tank \
         of its group has.",
        &[
            "sectors",
            "floor, m\u{b2}",
            "blocks",
            "crates",
            "blocks per 100 m\u{b2}",
            "floor per tank, m\u{b2}",
        ],
    );
    for stretch in stretches {
        let corridors = || {
            let groups = stretch.groups();
            groups.filter(|sector| sector.kind == SectorKind::Corridor)
        };
        let per_tank = corridors().map(|sector| sector.floor / sector.tanks.len() as f64);
        let density = corridors().map(|sector| sector.blocks as f64 * 100.0 / sector.floor);
        table.row(vec![
            stretch.name(),
            mean(corridors().map(|sector| sector.floor)),
            mean(corridors().map(|sector| sector.blocks as f64)),
            mean(corridors().map(|sector| sector.crates as f64)),
            mean(density),
            mean(per_tank),
        ]);
    }
    table
}

/// Counts what the rules make of the sectors asked for.
pub fn census(asked: &Asked) -> Report {
    let stretches = sample(asked);
    let (first, last) = asked.sectors;
    let players = match asked.players {
        1 => "one player".to_string(),
        players => format!("{players} players"),
    };
    let told = format!(
        "Version {}, {} mode, {players}, sectors {first} to {last}, on {} seeds: {} to {}.",
        seed::VERSION,
        asked.difficulty.key(),
        asked.seeds,
        seed_text(0),
        seed_text(asked.seeds - 1),
    );
    let mut tables = vec![levers(asked, &stretches), groups(&stretches)];
    tables.extend(classes(&stretches));
    tables.extend([
        bullets(&stretches),
        software(&stretches),
        hardware(&stretches),
        terrain(&stretches),
    ]);
    Report {
        title: "Census".to_string(),
        asked: told,
        tables,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked(sectors: (u32, u32), each: bool) -> Asked {
        Asked {
            sectors,
            seeds: 12,
            players: 1,
            difficulty: Difficulty::Normal,
            each,
        }
    }

    fn number(cell: &str) -> f64 {
        cell.trim_end_matches('%').parse().unwrap()
    }

    #[test]
    fn the_rows_are_the_stretches_that_were_asked_for() {
        assert_eq!(rows(&asked((1, 50), false)), STRETCHES);
        assert_eq!(rows(&asked((4, 12), false)), [(4, 5), (6, 10), (11, 12)]);
        assert_eq!(rows(&asked((45, 70), false)), [(45, 50), (51, 70)]);
        assert_eq!(rows(&asked((60, 70), false)), [(60, 70)]);
        assert_eq!(rows(&asked((3, 5), true)), [(3, 3), (4, 4), (5, 5)]);
    }

    #[test]
    fn the_floor_of_a_sector_is_the_area_of_its_outline() {
        assert_eq!(floor(&[(0, 0), (14, 0), (14, 16), (0, 16)]), 224.0);
        assert_eq!(floor(&[(0, 0), (0, 4), (4, 0)]), 8.0);
    }

    #[test]
    fn the_census_counts_what_the_game_makes() {
        let report = census(&asked((1, 20), false));
        let table = |title: &str| {
            let found = report.tables.iter().find(|table| table.title == title);
            found.unwrap_or_else(|| panic!("no table: {title}"))
        };
        for table in &report.tables {
            for row in &table.rows {
                assert_eq!(row.len(), table.columns.len(), "{}", table.title);
            }
        }
        // The levers are those of the specification.
        let levers = table("Levers");
        assert_eq!(levers.rows[0][..5], ["1-2", "10 to 16", "1", "1", "0"]);
        assert_eq!(levers.rows[4][..3], ["16-20", "67 to 75", "6"]);
        // Every seed gives a group for every sector, of as many tanks as the rules say:
        // six from sector 15 on, and half as many again in an arena.
        let groups = table("Groups");
        assert_eq!(groups.rows[0][1], "24");
        assert_eq!(groups.rows[0][3], "1.0");
        assert!(number(&groups.rows[4][3]) >= 6.0);
        // No tank has more points than the leader of its sector may.
        for (row, stretch) in groups.rows.iter().zip(STRETCHES) {
            let most = progression::sector_budget(stretch.1, Difficulty::Normal) * 3 / 2;
            assert!(number(&row[7]) <= most as f64, "{row:?}");
            assert!(number(&row[5]) <= number(&row[6]) && number(&row[6]) <= number(&row[7]));
        }
        // The shares of the classes make a whole, and so do those of the bullets.
        let classes = table("Classes");
        assert_eq!(classes.rows.len(), Class::ALL.len());
        for column in 1..classes.columns.len() {
            let whole: f64 = classes.rows.iter().map(|row| number(&row[column])).sum();
            assert!((whole - 100.0).abs() <= 6.0, "{whole}");
        }
        for row in &table("Bullets").rows {
            let whole: f64 = row[1..6].iter().map(|cell| number(cell)).sum();
            assert!((whole - 100.0).abs() <= 3.0, "{row:?}");
        }
        // The first sectors have no brain above the first tier, and no fast gun.
        let software = table("Software");
        assert_eq!(software.rows[0][3..6], ["0%", "0%", "0%"]);
        assert_eq!(table("Bullets").rows[0][10], "9");
        // The same census counts the same.
        assert_eq!(report.csv(), census(&asked((1, 20), false)).csv());
    }

    #[test]
    fn a_tank_shows_its_class_when_it_has_what_the_class_favors() {
        let mut grenadier = Genome::empty(7);
        grenadier.class = Class::Grenadier;
        assert!(!shows_its_class(&grenadier));
        grenadier.set(Stat::Exploding, 1);
        assert!(shows_its_class(&grenadier));
        // A seeker favors two stats, and shows with both.
        let mut seeker = Genome::empty(9);
        seeker.class = Class::Seeker;
        seeker.set(Stat::Ricochet, 1);
        assert!(!shows_its_class(&seeker));
        seeker.set(Stat::Seeking, 1);
        assert!(shows_its_class(&seeker));
    }

    #[test]
    fn groups_grow_with_the_players() {
        let four = Asked {
            players: 4,
            ..asked((16, 20), false)
        };
        let report = census(&four);
        let groups = &report.tables[1];
        assert!(number(&groups.rows[0][3]) >= 15.0, "{:?}", groups.rows[0]);
    }
}
