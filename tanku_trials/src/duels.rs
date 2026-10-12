//! The duels: tanks of one make against a reference player, with one stat at a time at each
//! of its levels and everything else as it is. They say what a level of a stat adds to the
//! danger of a tank, next to what it adds to its price, without a group around the tank.

use tanku_core::{
    modes::Difficulty,
    seed,
    stats::{Ammo, Genome, Stat},
};

use crate::{
    fight::{Make, Outcome, Setup},
    players::{ratio, run, seed_text},
    reference::Reference,
    report::{Report, Table, percent},
};

/// The tank that the duels start from, unless another is given: a plain one of the middle
/// of a run, that drives, sees a shot coming and leads its own.
pub const BASE: &str =
    "engine=3,tracks=3,turret=3,muzzle=2,magazine=2,loader=3,ricochet=1,brain=2,ballistics=1";

/// Every stat, in the order of the tables.
const STATS: [Stat; 15] = [
    Stat::Engine,
    Stat::Tracks,
    Stat::Turret,
    Stat::Muzzle,
    Stat::Magazine,
    Stat::Loader,
    Stat::Ricochet,
    Stat::Mines,
    Stat::Shield,
    Stat::Piercing,
    Stat::Seeking,
    Stat::Splitting,
    Stat::Exploding,
    Stat::Brain,
    Stat::Ballistics,
];

/// What the duels are asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    /// The tank that every duel starts from.
    pub base: Genome,
    /// The stats that are tried at each of their levels: all of them if none is named.
    pub stats: Vec<Stat>,
    /// The sectors that the duels are fought in: their terrain, and the experience of their
    /// brains.
    pub sectors: (u32, u32),
    pub seeds: u32,
    /// How many tanks of the make the player fights at once.
    pub count: usize,
    pub difficulty: Difficulty,
    pub player: Reference,
    /// How long a duel lasts at most, in seconds.
    pub limit: f32,
    /// How many duels run side by side.
    pub threads: usize,
}

pub fn stat(text: &str) -> Result<Stat, String> {
    let named = STATS
        .into_iter()
        .find(|stat| stat.name() == text.trim().to_lowercase());
    named.ok_or_else(|| format!("`{text}` is no stat"))
}

/// Reads a tank of the enemies as it is written: its stats with their levels, such as
/// `engine=3,muzzle=2,brain=1`. A stat that isn't named is at level 0.
pub fn tank(text: &str) -> Result<Genome, String> {
    let mut genome = Genome::empty(0);
    for part in text.split(',').filter(|part| !part.trim().is_empty()) {
        let Some((name, level)) = part.split_once('=') else {
            return Err(format!(
                "`{part}` is not a stat with its level, such as `engine=3`"
            ));
        };
        let stat = stat(name)?;
        let level: u8 = level
            .trim()
            .parse()
            .map_err(|_| format!("`{level}` is not a level"))?;
        if level > stat.max_level() {
            return Err(format!("{} goes up to {}", stat.name(), stat.max_level()));
        }
        genome.set(stat, level);
    }
    let ammos = Ammo::STATS
        .into_iter()
        .filter(|stat| genome.level(*stat) > 0);
    if ammos.count() > 1 {
        return Err("a tank fires one type of bullet".to_string());
    }
    Ok(paid(genome))
}

/// A tank as it is written.
pub fn written(genome: &Genome) -> String {
    let held = STATS.into_iter().filter(|stat| genome.level(*stat) > 0);
    let parts: Vec<String> = held
        .map(|stat| format!("{}={}", stat.name(), genome.level(stat)))
        .collect();
    parts.join(",")
}

/// A tank with the least budget that pays for what it has. Its software costs a share of
/// that budget.
fn paid(mut genome: Genome) -> Genome {
    genome.budget = genome.hardware_cost() as u16;
    while genome.cost() > genome.budget as u32 {
        genome.budget += 1;
    }
    genome
}

/// What a stat needs of another one, for a tank to make use of it.
fn needs(genome: &Genome) -> Vec<(Stat, Stat, u8)> {
    let has = |stat: Stat| genome.level(stat) > 0;
    let mut needs = Vec::new();
    if has(Stat::Tracks) {
        needs.push((Stat::Tracks, Stat::Engine, 1));
    }
    for stat in [Stat::Mines, Stat::Shield] {
        if has(stat) {
            needs.push((stat, Stat::Brain, 1));
        }
    }
    if genome.ballistics() >= 2 {
        needs.push((Stat::Ballistics, Stat::Ricochet, genome.ballistics() - 1));
    }
    if has(Stat::Seeking) {
        needs.push((Stat::Seeking, Stat::Ricochet, 1));
    }
    needs
}

/// The base with one stat at a level, and what that takes: a stat that needs another raises
/// it, and one that is needed by another lowers that one when it is lowered itself. A tank
/// whose bullets explode has no ricochet, which they couldn't use.
fn variant(base: &Genome, stat: Stat, level: u8) -> Genome {
    let mut genome = *base;
    if Ammo::STATS.contains(&stat) {
        for ammo in Ammo::STATS {
            genome.set(ammo, 0);
        }
    }
    genome.set(stat, level);
    if genome.ammo() == Ammo::Exploding {
        match stat {
            Stat::Ricochet if level > 0 => genome.set(Stat::Exploding, 0),
            _ => genome.set(Stat::Ricochet, 0),
        }
    }
    // Each pass settles one need, and none undoes another.
    for _ in 0..8 {
        let unmet = |&&(_, needed, least): &&(Stat, Stat, u8)| genome.level(needed) < least;
        let needs = needs(&genome);
        let Some(&(wants, needed, least)) = needs.iter().find(unmet) else {
            break;
        };
        if needed == stat {
            // The stat that is tried stays where it is: what needs more of it gives way.
            let kept = match wants {
                Stat::Ballistics => genome.level(needed) + 1,
                _ => 0,
            };
            genome.set(wants, kept);
        } else {
            genome.set(needed, least);
        }
    }
    paid(genome)
}

/// What a level of a stat gives, as the specification tells it.
fn gives(genome: &Genome, stat: Stat) -> String {
    let yes = |has: bool| if has { "yes" } else { "no" }.to_string();
    match stat {
        Stat::Engine => format!("{} m/s", genome.top_speed()),
        Stat::Tracks => format!("{:.0}°/s", genome.hull_turn_rate().to_degrees()),
        Stat::Turret => format!("{:.0}°/s", genome.turret_turn_rate().to_degrees()),
        Stat::Muzzle => format!("{} m/s", genome.bullet_speed()),
        Stat::Magazine => format!("{} rounds", genome.magazine()),
        Stat::Loader => format!("{} s", genome.reload_time()),
        Stat::Ricochet => format!("{} bounces", genome.bounces()),
        Stat::Mines => format!("{} mines", genome.mines()),
        Stat::Shield => match genome.shield_cooldown() {
            Some(cooldown) => format!("every {cooldown} s"),
            None => "none".to_string(),
        },
        Stat::Piercing | Stat::Seeking | Stat::Splitting | Stat::Exploding => {
            yes(genome.level(stat) > 0)
        }
        Stat::Brain | Stat::Ballistics => format!("tier {}", genome.level(stat)),
    }
}

/// The tanks that are tried for a stat, each with the name of its row: the stat at each of
/// its levels. The four special bullets are told in one table, which starts with the normal
/// one.
fn variants(base: &Genome, stat: Stat) -> Vec<(String, Genome)> {
    let levels = (0..=stat.max_level()).map(|level| {
        let genome = variant(base, stat, level);
        let name = format!("{} {level}: {}", stat.name(), gives(&genome, stat));
        (name, genome)
    });
    levels.collect()
}

fn bullets(base: &Genome) -> Vec<(String, Genome)> {
    let mut tanks = vec![("normal".to_string(), variant(base, Stat::Piercing, 0))];
    for ammo in Ammo::STATS {
        tanks.push((ammo.name().to_string(), variant(base, ammo, 1)));
    }
    tanks
}

/// The duels of one tank, as a row: the tank, its points, the duels, how many it lost, how
/// long it lasted, the losses in all, in a duel and in a minute, and the slope from the base
/// to it: the losses in a duel that each hundred points of difference made.
fn row(name: String, genome: &Genome, duels: &[&Outcome], base: Option<(f64, f64)>) -> Vec<String> {
    let count = duels.len() as f64;
    let losses = duels.iter().map(|duel| duel.losses.len()).sum::<usize>() as f64;
    let seconds: f64 = duels.iter().map(|duel| duel.lasted as f64).sum();
    let points = genome.cost() as f64;
    let added = match base {
        Some((base_points, base_losses)) if count > 0.0 && points != base_points => {
            let more = losses / count - base_losses;
            format!("{:+.2}", more * 100.0 / (points - base_points))
        }
        _ => "-".to_string(),
    };
    vec![
        name,
        written(genome),
        genome.cost().to_string(),
        duels.len().to_string(),
        percent(
            duels.iter().filter(|duel| duel.cleared()).count(),
            duels.len(),
        ),
        ratio(seconds, count, 1),
        format!("{losses:.0}"),
        ratio(losses, count, 2),
        ratio(losses * 60.0, seconds, 2),
        ratio(losses * 100.0, count * points, 2),
        added,
    ]
}

pub fn duels(asked: &Asked) -> Report {
    let base = asked.base;
    let stats = match asked.stats.is_empty() {
        true => STATS.to_vec(),
        false => asked.stats.clone(),
    };
    // The tables: one for each stat that isn't a type of bullet, and one for the bullets.
    let mut tables: Vec<(String, Vec<(String, Genome)>)> =
        vec![("Base".to_string(), vec![("base".to_string(), base)])];
    let mut told_bullets = false;
    for stat in stats {
        if !Ammo::STATS.contains(&stat) {
            let mut title = stat.name().to_string();
            title[..1].make_ascii_uppercase();
            tables.push((title, variants(&base, stat)));
        } else if !told_bullets {
            told_bullets = true;
            tables.push(("Bullets".to_string(), bullets(&base)));
        }
    }

    let (first, last) = asked.sectors;
    let mut setups = Vec::new();
    let mut of: Vec<(usize, usize)> = Vec::new();
    for (table, (_, tanks)) in tables.iter().enumerate() {
        for (tank, &(_, genome)) in tanks.iter().enumerate() {
            for number in 0..asked.seeds {
                for sector in first..=last {
                    setups.push(Setup {
                        seed: seed_text(number),
                        sector,
                        difficulty: asked.difficulty,
                        player: asked.player,
                        limit: asked.limit,
                        make: Some(Make {
                            tanks: vec![genome; asked.count],
                            spot: number as usize,
                        }),
                    });
                    of.push((table, tank));
                }
            }
        }
    }
    let outcomes = run(&setups, asked.threads);
    let fought = |wanted: (usize, usize)| -> Vec<&Outcome> {
        let duels = of.iter().zip(&outcomes).filter(|(of, _)| **of == wanted);
        duels.filter_map(|(_, outcome)| outcome.as_ref()).collect()
    };
    let base_duels = fought((0, 0));
    let base_losses = base_duels
        .iter()
        .map(|duel| duel.losses.len())
        .sum::<usize>() as f64
        / base_duels.len().max(1) as f64;
    let against = Some((base.cost() as f64, base_losses));

    let columns = [
        "tank",
        "made of",
        "points",
        "duels",
        "fell",
        "lasted s",
        "lost",
        "losses",
        "per min",
        "per 100 points",
        "slope",
    ];
    let mut report = Vec::new();
    for (number, (title, tanks)) in tables.iter().enumerate() {
        let note = match number {
            0 => "The tank that every other one is made from. The columns are what it is made \
                 of, its points, the duels, those that it lost in time, how long it lasted, \
                 the tanks that the player lost in all, the losses in a duel and in a minute, \
                 the losses in a duel for each hundred of its points, and the slope from the \
                 base to the tank: the losses in a duel that each hundred points of \
                 difference made. Prices are right when that slope is about the same for \
                 every stat. A row is as sure as it has tanks lost: twenty-five are known to \
                 a fifth, a hundred to a tenth."
                .to_string(),
            _ => format!(
                "The base with its {} at each level, and with what that takes of its other \
                 stats.",
                title.to_lowercase()
            ),
        };
        let mut table = Table::new(title, &note, &columns);
        for (tank, (name, genome)) in tanks.iter().enumerate() {
            let against = against.filter(|_| number > 0);
            table.row(row(name.clone(), genome, &fought((number, tank)), against));
        }
        report.push(table);
    }

    let tanks = match asked.count {
        1 => "one tank".to_string(),
        count => format!("{count} tanks of the same make"),
    };
    Report {
        title: "Duels".to_string(),
        asked: format!(
            "Version {}, {} mode, the {} player in the standard tank against {tanks}, in the \
             corridors of sectors {first} to {last} with the brains of those sectors, on {} \
             seeds: {} to {}. A duel lasts {} s at most. The base is `{}`.",
            seed::VERSION,
            asked.difficulty.key(),
            asked.player.name(),
            asked.seeds,
            seed_text(0),
            seed_text(asked.seeds - 1),
            asked.limit,
            written(&base),
        ),
        balance: Some(tanku_core::balance::FINGERPRINT),
        tables: report,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Genome {
        tank(BASE).unwrap()
    }

    #[test]
    fn a_tank_is_read_as_it_is_written_and_pays_for_what_it_has() {
        let base = base();
        assert_eq!(written(&base), BASE);
        // 29 points of hardware, and 21% of its budget for a brain and for ballistics.
        assert_eq!(
            (base.hardware_cost(), base.budget, base.cost()),
            (29, 37, 37)
        );
        assert_eq!(
            tank(" Engine = 2 , brain=1 ")
                .map(|tank| written(&tank))
                .unwrap(),
            "engine=2,brain=1"
        );
        assert!(tank("engine=9").is_err());
        assert!(tank("wheels=2").is_err());
        assert!(tank("piercing=1,seeking=1").is_err());
        assert!(tank("engine").is_err());
    }

    #[test]
    fn a_level_that_is_tried_changes_nothing_else_unless_it_must() {
        let base = base();
        let level = |genome: &Genome, stat: Stat| genome.level(stat);
        let fast = variant(&base, Stat::Muzzle, 7);
        assert_eq!(written(&fast), BASE.replace("muzzle=2", "muzzle=7"));
        // A tank without an engine has no use for tracks, and mines need a brain.
        assert_eq!(level(&variant(&base, Stat::Engine, 0), Stat::Tracks), 0);
        let plain = tank("engine=2").unwrap();
        assert_eq!(level(&variant(&plain, Stat::Mines, 2), Stat::Brain), 1);
        assert_eq!(level(&variant(&plain, Stat::Tracks, 3), Stat::Engine), 2);
        // Bank shots need the bounces, and lose their tiers with them.
        assert_eq!(
            level(&variant(&base, Stat::Ballistics, 3), Stat::Ricochet),
            2
        );
        let banker = variant(&base, Stat::Ballistics, 3);
        assert_eq!(
            level(&variant(&banker, Stat::Ricochet, 0), Stat::Ballistics),
            1
        );
        // A bullet that explodes doesn't bounce, and a tank fires one type.
        let grenadier = variant(&base, Stat::Exploding, 1);
        assert_eq!(
            (level(&grenadier, Stat::Ricochet), grenadier.ammo()),
            (0, Ammo::Exploding)
        );
        let seeker = variant(&grenadier, Stat::Seeking, 1);
        assert_eq!(
            (level(&seeker, Stat::Ricochet), seeker.ammo()),
            (1, Ammo::Seeking)
        );
        // Each of them pays for what it has, and for no more.
        for tank in [fast, banker, grenadier, seeker] {
            assert_eq!(tank.cost(), tank.budget as u32);
        }
    }

    #[test]
    fn the_report_has_a_table_for_the_base_and_one_for_each_stat_that_is_tried() {
        let report = duels(&Asked {
            base: base(),
            stats: vec![Stat::Ricochet, Stat::Piercing, Stat::Exploding],
            sectors: (6, 8),
            seeds: 1,
            count: 1,
            difficulty: Difficulty::Normal,
            player: Reference::Regular,
            limit: 10.0,
            threads: 4,
        });
        let titles: Vec<&str> = report
            .tables
            .iter()
            .map(|table| table.title.as_str())
            .collect();
        assert_eq!(titles, ["Base", "Ricochet", "Bullets"]);
        assert_eq!(report.tables[1].rows.len(), 4);
        assert_eq!(report.tables[2].rows.len(), 5);
        // The row of the base level tells the same duels as the base.
        assert_eq!(
            report.tables[1].rows[1][3..10],
            report.tables[0].rows[0][3..10]
        );
    }
}
