//! The journal of the runs that were played, next to the reference players: where a person
//! loses their tanks and to what, and which of the three players of the trials plays as that
//! person does.
//!
//! The game adds a line to its journal for each run (see `tanku_core::journal`). The runs
//! that compare with those of the trials are counted: a single player's, in the standard
//! tank, in the corridor, at the difficulty asked for, played with this version of the game
//! and this balance. Most of what a person plays is not a whole run: a playtest has infinite
//! lives and skips sectors, and a run is left when dinner is ready. What every run tells is
//! the tanks that it lost for each sector that it entered, so that is what is compared.
//!
//! The reference players fight for each sector that the journal entered, as they do in the
//! comparison of the players: on many seeds, from the barrier, with a tank that nothing
//! destroys, and each tank that they would have lost is counted. Whole runs would tell less:
//! a player with three lives only tells of the sectors that it gets to, and one with
//! infinite lives falls hundreds of times in a sector that a run would have ended in. Only
//! the corridors hold such a fight, so the arenas of the journal are told apart.

use std::path::PathBuf;

use tanku_core::{
    balance::FINGERPRINT,
    journal::{Cause, Culprit, Entry, Journal, Loss},
    modes::Difficulty,
    seed::VERSION,
    stats::{Ammo, Class},
};

use crate::{
    census::{class_name, stretch_name, stretches},
    fight::{self, Outcome, Setup},
    players::{self, ratio, seed_text, side_by_side},
    reference::Reference,
    report::{Report, Table, percent},
    runs::{self, quantile},
};

/// What the journal is asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    /// The file of the journal, for the report to name.
    pub file: PathBuf,
    pub difficulty: Difficulty,
    /// How many seeds each reference player fights on.
    pub seeds: u32,
    /// The runs that started at this moment or later are counted, as the journal writes a
    /// moment. Every run is, without it.
    pub since: Option<String>,
    /// Whether the runs of every version and of every balance are counted, not only those
    /// that were played with the ones of this build.
    pub every_build: bool,
    /// The furthest sector that the reference players fight in, whatever the journal entered.
    pub last: u32,
    /// How long a fight of a reference player for a sector lasts at most, in seconds.
    pub limit: f32,
    /// How long a reference player may stay in a sector of a whole run without clearing it
    /// or falling, in seconds.
    pub patience: f32,
    pub threads: usize,
}

/// How many times as likely the losses of the journal must be with one player as with another
/// for the report to tell the two apart. A few tanks lost in a few sectors seldom do.
const CLEARLY: f64 = 3.0;

/// Why a run of the journal is not counted. A run is left out for the first of these that
/// holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Apart {
    Earlier,
    Build,
    Team,
    Difficulty,
    Room,
    Garage,
}

impl Apart {
    const ALL: [Apart; 6] = [
        Apart::Earlier,
        Apart::Build,
        Apart::Team,
        Apart::Difficulty,
        Apart::Room,
        Apart::Garage,
    ];

    fn row(self) -> &'static str {
        match self {
            Apart::Earlier => "left out: before the moment asked for",
            Apart::Build => "left out: another version or another balance",
            Apart::Team => "left out: several players",
            Apart::Difficulty => "left out: another difficulty",
            Apart::Room => "left out: arena mode",
            Apart::Garage => "left out: a tank of the garage",
        }
    }

    /// Why a run is not counted, if it isn't.
    fn of(entry: &Entry, asked: &Asked) -> Option<Self> {
        let earlier = |since: &String| entry.when < *since;
        let build = entry.version != VERSION || entry.balance != FINGERPRINT;
        let reasons = [
            (asked.since.as_ref().is_some_and(earlier), Apart::Earlier),
            (build && !asked.every_build, Apart::Build),
            (entry.players != 1, Apart::Team),
            (entry.difficulty != asked.difficulty, Apart::Difficulty),
            (entry.arena, Apart::Room),
            (entry.free, Apart::Garage),
        ];
        reasons
            .into_iter()
            .find(|(holds, _)| *holds)
            .map(|(_, why)| why)
    }
}

/// What some runs or some fights tell the tables: the sectors that they entered, each as
/// often as it was entered, and the tanks that their player lost there.
#[derive(Clone, Debug, Default, PartialEq)]
struct Told {
    entered: Vec<u32>,
    losses: Vec<Loss>,
}

impl Told {
    /// What the runs of a journal tell of their corridors, or of their arenas.
    fn of_entries(entries: &[&Entry], arenas: bool) -> Self {
        let mut told = Self::default();
        for entry in entries {
            let kind = |sector: &u32| entry.arenas.contains(sector) == arenas;
            told.entered
                .extend(entry.entered.iter().copied().filter(kind));
            // A tank that was lost in the start bay was lost in no sector.
            let lost = entry.losses.iter().filter(|loss| loss.sector >= 1);
            told.losses
                .extend(lost.copied().filter(|loss| kind(&loss.sector)));
        }
        told
    }

    /// What the fights of a reference player tell, each with its sector: the tanks that the
    /// player would have lost, and the shots of its own that came back onto its tank.
    fn of_fights(fights: &[(u32, Outcome)]) -> Self {
        let mut told = Self::default();
        for (sector, fight) in fights {
            told.entered.push(*sector);
            for loss in &fight.losses {
                let maker = loss.by.and_then(|tank| fight.group.get(tank));
                let ammo = maker.map_or(Ammo::Normal, |tank| tank.genome.ammo());
                let cause = match loss.cause {
                    fight::Cause::Bullet => Cause::Bullet(ammo),
                    fight::Cause::Exploding => Cause::Bullet(Ammo::Exploding),
                    fight::Cause::Mine => Cause::Mine,
                    fight::Cause::Crate => Cause::Crate,
                };
                let by = match maker {
                    Some(tank) => Culprit::Enemy(tank.genome.class),
                    None => Culprit::Nobody,
                };
                let sector = *sector;
                told.losses.push(Loss { sector, cause, by });
            }
            told.losses.extend((0..fight.own).map(|_| Loss {
                sector: *sector,
                cause: Cause::Bullet(Ammo::Normal),
                by: Culprit::Player,
            }));
        }
        told
    }

    /// How many sectors of a stretch were entered.
    fn entered_in(&self, (first, last): (u32, u32)) -> usize {
        let within = |sector: &&u32| (first..=last).contains(*sector);
        self.entered.iter().filter(within).count()
    }

    /// The tanks that were lost in a stretch of sectors.
    fn lost_in(&self, (first, last): (u32, u32)) -> impl Iterator<Item = &Loss> {
        let within = move |loss: &&Loss| (first..=last).contains(&loss.sector);
        self.losses.iter().filter(within)
    }

    /// The tanks lost for each sector entered, in a stretch that was entered.
    fn rate(&self, stretch: (u32, u32)) -> Option<f64> {
        let entered = self.entered_in(stretch);
        (entered > 0).then(|| self.lost_in(stretch).count() as f64 / entered as f64)
    }
}

/// How well the tanks that a journal lost in some sectors fit a player who loses so many in
/// each: the more, the better. The losses of a player are taken to fall by chance at a steady
/// rate, so the count is the logarithm of how likely the journal's losses are. A player who
/// lost nothing there is taken to lose half a tank over the sectors that it entered, so that
/// one loss of the journal doesn't rule that player out.
fn fit(lost: usize, entered: usize, player: &Told, stretch: (u32, u32)) -> Option<f64> {
    let theirs = player.entered_in(stretch);
    if theirs == 0 || entered == 0 {
        return None;
    }
    let rate = (player.lost_in(stretch).count() as f64).max(0.5) / theirs as f64;
    let expected = rate * entered as f64;
    Some(lost as f64 * expected.ln() - expected)
}

/// The players that the journal plays as over some stretches of sectors, among those who
/// fought wherever the journal went: the one whose losses make those of the journal the most
/// likely, and with it every other that the journal can't be told from. A stretch whose
/// sectors held no fight for a player, as when all of them are arenas on the seeds of the
/// trial, tells nothing of that player.
fn nearest(journal: &Told, players: &[(Reference, Told)], over: &[(u32, u32)]) -> Vec<Reference> {
    let fits = players.iter().filter_map(|(player, told)| {
        let each = over.iter().filter_map(|&stretch| {
            let entered = journal.entered_in(stretch);
            let lost = journal.lost_in(stretch).count();
            (entered > 0).then(|| fit(lost, entered, told, stretch))
        });
        let each: Option<Vec<f64>> = each.collect();
        each.filter(|each| !each.is_empty())
            .map(|each| (*player, each.iter().sum::<f64>()))
    });
    let fits: Vec<(Reference, f64)> = fits.collect();
    let best = fits.iter().map(|(_, fit)| *fit).max_by(f64::total_cmp);
    let near = |fit: f64| best.is_some_and(|best| best - fit <= CLEARLY.ln());
    let near = fits.iter().filter(|(_, fit)| near(*fit));
    near.map(|(player, _)| *player).collect()
}

/// The players that the journal plays as, as a cell names them: `regular`, or `regular or
/// expert` when the journal can't be told from either. A dash when it can't be told from any
/// of several players, which tells nothing, and when nobody fought where it went.
fn named(nearest: &[Reference], among: usize) -> String {
    if nearest.is_empty() || (among > 1 && nearest.len() == among) {
        return "-".to_string();
    }
    let names: Vec<&str> = nearest.iter().map(|player| player.name()).collect();
    names.join(" or ")
}

/// The tanks that a player would lose over the sectors that the journal entered, at what
/// that player loses in a fight for a sector of each stretch, and of them those that this
/// tells apart. Nothing is told of a player who fought in no sector of a stretch that the
/// journal entered.
fn expected(
    journal: &Told,
    player: &Told,
    over: &[(u32, u32)],
    counts: impl Fn(&Loss) -> bool,
) -> Option<f64> {
    let each = over.iter().map(|&stretch| {
        let (ours, theirs) = (journal.entered_in(stretch), player.entered_in(stretch));
        if ours == 0 {
            return Some(0.0);
        }
        let lost = player.lost_in(stretch).filter(|loss| counts(loss)).count();
        (theirs > 0).then(|| ours as f64 * lost as f64 / theirs as f64)
    });
    each.sum()
}

/// The weapon that a tank was lost to, as a row tells it.
const WEAPONS: [&str; 9] = [
    "Bullet, normal",
    "Bullet, piercing",
    "Bullet, seeking",
    "Bullet, splitting",
    "Bullet, exploding",
    "Mine",
    "Crate",
    "The player's own bullet or mine",
    "Unknown",
];

/// The row of a tank that was lost. A crate is one whoever set it off: the fights of the
/// trials don't tell who did.
fn weapon(loss: &Loss) -> &'static str {
    match (loss.cause, loss.by) {
        (Cause::Crate, _) => "Crate",
        (Cause::Unknown, _) => "Unknown",
        (_, Culprit::Player) => "The player's own bullet or mine",
        (Cause::Bullet(Ammo::Normal), _) => "Bullet, normal",
        (Cause::Bullet(Ammo::Piercing), _) => "Bullet, piercing",
        (Cause::Bullet(Ammo::Seeking), _) => "Bullet, seeking",
        (Cause::Bullet(Ammo::Splitting), _) => "Bullet, splitting",
        (Cause::Bullet(Ammo::Exploding), _) => "Bullet, exploding",
        (Cause::Mine, _) => "Mine",
    }
}

fn culprit(loss: &Loss) -> &'static str {
    match loss.by {
        Culprit::Enemy(class) => class_name(class),
        Culprit::Player => "The player",
        Culprit::Nobody => "Nobody",
    }
}

/// The tables that set the runs of a journal next to the reference players: the runs that
/// are counted, what each player told of its fights for the same sectors, and the scores of
/// the runs of the players with three lives, if the journal has whole runs to set next to
/// them.
fn tables(
    counted: &[&Entry],
    players: &[(Reference, Told)],
    scores: &[(Reference, Vec<u32>)],
) -> Vec<Table> {
    let journal = Told::of_entries(counted, false);
    let arenas = Told::of_entries(counted, true);
    let furthest = journal.entered.iter().copied().max().unwrap_or(1);
    let over = stretches((1, furthest), false);
    let names: Vec<&str> = players.iter().map(|(player, _)| player.name()).collect();
    let with_names = |first: &[&str], last: &[&str]| -> Vec<String> {
        let all = first.iter().chain(&names).chain(last);
        all.map(|name| name.to_string()).collect()
    };
    let table = |title: &str, note: &str, columns: Vec<String>| {
        let columns: Vec<&str> = columns.iter().map(String::as_str).collect();
        Table::new(title, note, &columns)
    };
    let dash = || "-".to_string();

    let mut places = table(
        "Tanks lost in each sector",
        "For each stretch of sectors: the corridors that the runs of the journal entered, the \
         tanks that they lost there, and that for each one entered. Then the tanks that each \
         reference player would have lost in a fight for a corridor of the stretch, and the \
         player that the journal is nearest to: the one whose losses make those of the \
         journal the most likely. Two players are named where the journal's losses are not \
         three times as likely with one as with the other, and none where they don't tell \
         any player from another. The arenas of the journal come last: the players fight in \
         none.",
        with_names(
            &["sectors", "entered", "lost", "lost in each"],
            &["nearest"],
        ),
    );
    for &stretch in &over {
        let (entered, lost) = (
            journal.entered_in(stretch),
            journal.lost_in(stretch).count(),
        );
        let mut row = vec![
            stretch_name(stretch),
            entered.to_string(),
            lost.to_string(),
            ratio(lost as f64, entered as f64, 2),
        ];
        for (_, told) in players {
            let rate = told.rate(stretch);
            row.push(rate.map_or(dash(), |rate| format!("{rate:.2}")));
        }
        let near = nearest(&journal, players, &[stretch]);
        row.push(named(&near, players.len()));
        places.row(row);
    }
    let mut row = vec![
        "arenas".to_string(),
        arenas.entered.len().to_string(),
        arenas.losses.len().to_string(),
        ratio(arenas.losses.len() as f64, arenas.entered.len() as f64, 2),
    ];
    row.extend(players.iter().map(|_| dash()));
    row.push(dash());
    places.row(row);

    let mut whole = Table::new(
        "Over the corridors of the journal",
        "The tanks that the runs of the journal lost in their corridors, and those that each \
         reference player would lose over the same ones: what it loses in a fight for a \
         corridor of each stretch, times the corridors of the stretch that the journal \
         entered. Then that count for each tank of the journal. The journal plays as the \
         player whose losses make its own the most likely, stretch by stretch, or as two \
         players that so few tanks don't tell apart.",
        &["player", "lost", "for each of the journal's", "plays as"],
    );
    let lost = journal.losses.len();
    let plays = nearest(&journal, players, &over);
    whole.row(vec![
        "journal".to_string(),
        lost.to_string(),
        ratio(lost as f64, lost as f64, 2),
        named(&plays, players.len()),
    ]);
    for (player, told) in players {
        let theirs = expected(&journal, told, &over, |_| true);
        whole.row(vec![
            player.name().to_string(),
            theirs.map_or(dash(), |theirs| format!("{theirs:.1}")),
            theirs.map_or(dash(), |theirs| ratio(theirs, lost as f64, 2)),
            String::new(),
        ]);
    }

    type Row = fn(&Loss) -> &'static str;
    let shares = |title: &str, note: &str, first: &str, rows: &[&str], row_of: Row| {
        let mut table = table(title, note, with_names(&[first, "journal"], &[]));
        for &name in rows {
            let these = journal.losses.iter().filter(|loss| row_of(loss) == name);
            let mut row = vec![
                name.to_string(),
                percent(these.count(), journal.losses.len()),
            ];
            for (_, told) in players {
                let all = expected(&journal, told, &over, |_| true);
                let these = expected(&journal, told, &over, |loss| row_of(loss) == name);
                row.push(match (these, all) {
                    (Some(these), Some(all)) if all > 0.0 => {
                        format!("{:.0}%", these * 100.0 / all)
                    }
                    _ => dash(),
                });
            }
            table.row(row);
        }
        table
    };
    let weapons = shares(
        "What destroyed the player",
        "The tanks that the runs of the journal lost in their corridors, by what destroyed \
         them, and those that each reference player would lose over the same corridors.",
        "weapon",
        &WEAPONS,
        weapon,
    );
    let class_names: Vec<&str> = Class::ALL
        .into_iter()
        .map(class_name)
        .chain(["The player", "Nobody"])
        .collect();
    let classes = shares(
        "Whose doing it was",
        "The same tanks, by the class of the enemy that fired the bullet, laid the mine or \
         set off the crate. The game tells the journal who set off a crate. The fights of \
         the trials tell nobody, and of what a reference player does to its own tank, they \
         tell its bullets that come back.",
        "class",
        &class_names,
        culprit,
    );

    let mut tables = vec![places, whole, weapons, classes];
    // The scores of whole runs compare with those of the players who have three lives.
    let mut ours: Vec<u32> = counted
        .iter()
        .filter(|entry| entry.whole())
        .map(|entry| entry.cleared)
        .collect();
    if !ours.is_empty() {
        let mut runs = Table::new(
            "Whole runs",
            "The runs of the journal that were played as a record counts one: from the start \
             bay, through each sector in turn, with three lives, until the last one was lost. \
             The sectors that they cleared: their mean, the score that one run in four and \
             one in two stayed under, and so on, and the best. Then the same for the runs of \
             each reference player, with three lives.",
            &[
                "player", "runs", "mean", "1 in 4", "median", "3 in 4", "best",
            ],
        );
        ours.sort();
        let mut row = |name: &str, cleared: &[u32]| {
            let sum: u32 = cleared.iter().sum();
            runs.row(vec![
                name.to_string(),
                cleared.len().to_string(),
                ratio(sum as f64, cleared.len() as f64, 1),
                quantile(cleared, 0.25),
                quantile(cleared, 0.5),
                quantile(cleared, 0.75),
                quantile(cleared, 1.0),
            ]);
        };
        row("journal", &ours);
        for (player, cleared) in scores {
            let mut cleared = cleared.clone();
            cleared.sort();
            row(player.name(), &cleared);
        }
        tables.push(runs);
    }
    tables
}

/// What was read: the runs of the journal, those that are counted and those that are not.
fn read(journal: &Journal, counted: &[&Entry], asked: &Asked) -> Table {
    let mut table = Table::new(
        "What was read",
        "The runs of the journal, those that are left out, each for the first reason that \
         holds, and those that are counted, with what they hold.",
        &["runs", "count"],
    );
    let mut row = |name: &str, count: usize| table.row(vec![name.to_string(), count.to_string()]);
    row("runs of the journal", journal.entries.len());
    row("lines that are not a run", journal.unread);
    for why in Apart::ALL {
        let apart = |entry: &&Entry| Apart::of(entry, asked) == Some(why);
        row(why.row(), journal.entries.iter().filter(apart).count());
    }
    row("counted", counted.len());
    let whole = counted.iter().filter(|entry| entry.whole()).count();
    row("of them whole runs", whole);
    let endless = counted.iter().filter(|entry| entry.endless).count();
    row("of them with infinite lives", endless);
    let (corridors, arenas) = (
        Told::of_entries(counted, false),
        Told::of_entries(counted, true),
    );
    row("corridors that they entered", corridors.entered.len());
    row("arenas that they entered", arenas.entered.len());
    row(
        "tanks that they lost",
        corridors.losses.len() + arenas.losses.len(),
    );
    table
}

/// Sets the runs of a journal, whose text this is, next to the reference players.
pub fn journal(asked: &Asked, text: &str) -> Report {
    let journal = tanku_core::journal::read(text);
    let counted: Vec<&Entry> = journal
        .entries
        .iter()
        .filter(|entry| Apart::of(entry, asked).is_none())
        .collect();
    let furthest = counted
        .iter()
        .flat_map(|entry| &entry.entered)
        .copied()
        .max();
    let whole = counted.iter().any(|entry| entry.whole());
    let mut tables = vec![read(&journal, &counted, asked)];
    if let Some(furthest) = furthest {
        // Each player fights for each sector that the journal got to, on each seed.
        let seeds = || (0..asked.seeds).map(seed_text);
        let setups: Vec<Setup> = Reference::ALL
            .into_iter()
            .flat_map(|player| seeds().map(move |seed| (player, seed)))
            .flat_map(|(player, seed)| {
                (1..=furthest.min(asked.last)).map(move |sector| Setup {
                    seed: seed.clone(),
                    sector,
                    difficulty: asked.difficulty,
                    player,
                    limit: asked.limit,
                    make: None,
                })
            })
            .collect();
        let fought = players::run(&setups, asked.threads);
        let told = |player: Reference| {
            let theirs = setups.iter().zip(&fought);
            let theirs = theirs.filter(|(setup, _)| setup.player == player);
            let fights: Vec<(u32, Outcome)> = theirs
                .filter_map(|(setup, fight)| Some((setup.sector, fight.clone()?)))
                .collect();
            Told::of_fights(&fights)
        };
        let players: Vec<(Reference, Told)> = Reference::ALL
            .into_iter()
            .map(|player| (player, told(player)))
            .collect();
        // The runs with three lives are only played for a journal that has whole runs.
        let mut scores: Vec<(Reference, Vec<u32>)> = Vec::new();
        if whole {
            let plan = runs::Asked {
                seeds: asked.seeds,
                difficulty: asked.difficulty,
                players: Reference::ALL.to_vec(),
                last: asked.last,
                patience: asked.patience,
                threads: asked.threads,
            };
            let jobs: Vec<(Reference, u32)> = Reference::ALL
                .into_iter()
                .flat_map(|player| (0..asked.seeds).map(move |number| (player, number)))
                .collect();
            let cleared = side_by_side(&jobs, asked.threads, |&(player, number)| {
                runs::journey(&seed_text(number), asked.difficulty, player, &plan).cleared
            });
            for player in Reference::ALL {
                let theirs = jobs.iter().zip(&cleared).filter(|(job, _)| job.0 == player);
                scores.push((player, theirs.map(|(_, cleared)| *cleared).collect()));
            }
        }
        tables.extend(self::tables(&counted, &players, &scores));
    }
    let build = match asked.every_build {
        true => "any version of the game and any balance",
        false => "this version of the game and this balance",
    };
    let since = match &asked.since {
        Some(since) => format!(", from {since} on"),
        None => String::new(),
    };
    Report {
        title: "Journal".to_string(),
        asked: format!(
            "Version {VERSION}, {} mode, the journal `{}`{since}. Its runs that are counted \
             are those of a single player in the standard tank, in the corridor, with {build}. \
             Each reference player fights for each sector that those runs got to, and no \
             further than sector {}, on {} seeds: {} to {}. A fight lasts {} s at most, and a \
             fight of a whole run is stopped after {} s.",
            asked.difficulty.key(),
            asked.file.display(),
            asked.last,
            asked.seeds,
            seed_text(0),
            seed_text(asked.seeds - 1),
            asked.limit,
            asked.patience,
        ),
        balance: Some(FINGERPRINT),
        tables,
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::{journal::End, stats::Genome};

    use super::*;
    use crate::fight::Fought;

    fn asked() -> Asked {
        Asked {
            file: "journal.txt".into(),
            difficulty: Difficulty::Normal,
            seeds: 2,
            since: None,
            every_build: false,
            last: 4,
            limit: 20.0,
            patience: 60.0,
            threads: 4,
        }
    }

    /// A run of a single player in the corridor, with this build, that entered some sectors
    /// and lost a tank to a gunner in each of some others.
    fn entry(entered: Vec<u32>, lost: &[u32]) -> Entry {
        let loss = |&sector: &u32| Loss {
            sector,
            cause: Cause::Bullet(Ammo::Normal),
            by: Culprit::Enemy(Class::Gunner),
        };
        Entry {
            when: "2026-10-13T17:46Z".to_string(),
            version: VERSION.to_string(),
            balance: FINGERPRINT,
            seed: "JOURNAL".to_string(),
            difficulty: Difficulty::Normal,
            players: 1,
            arena: false,
            free: false,
            playtest: false,
            endless: false,
            cleared: entered.len() as u32 - 1,
            entered,
            arenas: Vec::new(),
            losses: lost.iter().map(loss).collect(),
            fought: 300,
            kills: 20,
            end: End::Lost,
        }
    }

    /// A player who fought for sectors 1 to 20 on ten seeds and lost so many tanks in those
    /// ten fights for each of the first ten, and twice as many for each of the others.
    fn player(lost_in_ten: usize) -> Told {
        let entered: Vec<u32> = (0..10).flat_map(|_| 1..=20).collect();
        let loss = |sector: u32, cause: Cause, by: Culprit| Loss { sector, cause, by };
        let mut losses = Vec::new();
        for sector in 1..=20u32 {
            let count = if sector <= 10 {
                lost_in_ten
            } else {
                2 * lost_in_ten
            };
            // In the late sectors, every other tank is lost to the mine of a sapper.
            for tank in 0..count {
                let (mine, sapper) = (Cause::Mine, Culprit::Enemy(Class::Sapper));
                let (bullet, gunner) = (Cause::Bullet(Ammo::Normal), Culprit::Enemy(Class::Gunner));
                losses.push(match sector > 10 && tank % 2 == 1 {
                    true => loss(sector, mine, sapper),
                    false => loss(sector, bullet, gunner),
                });
            }
        }
        Told { entered, losses }
    }

    fn players() -> Vec<(Reference, Told)> {
        vec![
            (Reference::Novice, player(8)),
            (Reference::Regular, player(3)),
            (Reference::Expert, player(1)),
        ]
    }

    fn cell<'a>(table: &'a Table, row: &str, column: &str) -> &'a str {
        let column = table.columns.iter().position(|name| name == column);
        let row = table.rows.iter().find(|cells| cells[0] == row).expect(row);
        &row[column.unwrap()]
    }

    #[test]
    fn a_run_is_counted_when_it_compares_with_those_of_the_trials() {
        let asked = asked();
        let counted = entry(vec![1, 2], &[]);
        assert_eq!(Apart::of(&counted, &asked), None);
        let other = |change: fn(&mut Entry)| {
            let mut entry = entry(vec![1, 2], &[]);
            change(&mut entry);
            Apart::of(&entry, &asked)
        };
        assert_eq!(other(|entry| entry.players = 2), Some(Apart::Team));
        let hard = other(|entry| entry.difficulty = Difficulty::Hard);
        assert_eq!(hard, Some(Apart::Difficulty));
        assert_eq!(other(|entry| entry.arena = true), Some(Apart::Room));
        assert_eq!(other(|entry| entry.free = true), Some(Apart::Garage));
        assert_eq!(other(|entry| entry.balance ^= 1), Some(Apart::Build));
        let old = other(|entry| entry.version = "0.1.0".to_string());
        assert_eq!(old, Some(Apart::Build));
        // A playtest and its infinite lives are counted: most of what is played is one.
        let playtest = other(|entry| (entry.playtest, entry.endless) = (true, true));
        assert_eq!(playtest, None);
        // A run of another build is counted when every build is asked for, and a run is left
        // out for the first reason that holds.
        let every = Asked {
            every_build: true,
            ..asked.clone()
        };
        let mut old = entry(vec![1, 2], &[]);
        (old.balance, old.players) = (1, 3);
        assert_eq!(Apart::of(&old, &asked), Some(Apart::Build));
        assert_eq!(Apart::of(&old, &every), Some(Apart::Team));
        old.players = 1;
        assert_eq!(Apart::of(&old, &every), None);
        // The moment asked for is compared as the journal writes one.
        let since = Asked {
            since: Some("2026-10-13T18:00Z".to_string()),
            ..asked
        };
        assert_eq!(Apart::of(&counted, &since), Some(Apart::Earlier));
        let later = Asked {
            since: Some("2026-10-13".to_string()),
            ..since
        };
        assert_eq!(Apart::of(&counted, &later), None);
    }

    #[test]
    fn the_journal_is_set_next_to_what_each_player_loses_in_the_same_sectors() {
        // Two runs: sectors 1 to 20, and a playtest that skipped to sectors 11 to 20 first.
        // They lost 3 tanks in the 20 corridors of the first ten sectors, and 12 in the 20
        // others. The second also entered an arena, and lost two tanks there.
        // Neither is a whole run: the first was left.
        let mut first = entry((1..=20).collect(), &[4, 12, 12, 13, 15, 18, 19]);
        first.end = End::Left;
        let skipped: Vec<u32> = (11..=20).chain(1..=10).chain([21]).collect();
        let mut second = entry(skipped, &[2, 9, 11, 14, 16, 16, 17, 20, 21, 21]);
        second.arenas = vec![21];
        let counted = [&first, &second];
        let tables = tables(&counted, &players(), &[]);
        let titles: Vec<&str> = tables.iter().map(|table| table.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Tanks lost in each sector",
                "Over the corridors of the journal",
                "What destroyed the player",
                "Whose doing it was",
            ]
        );
        let places = &tables[0];
        assert_eq!(cell(places, "6-10", "entered"), "10");
        assert_eq!(cell(places, "11-15", "lost"), "6");
        assert_eq!(cell(places, "11-15", "lost in each"), "0.60");
        // The players lose 0.8, 0.3 and 0.1 tank for each of the first ten sectors, and
        // twice that for the others.
        assert_eq!(cell(places, "3-5", "novice"), "0.80");
        assert_eq!(cell(places, "16-20", "regular"), "0.60");
        assert_eq!(cell(places, "16-20", "expert"), "0.20");
        // 6 tanks in 10 sectors is what the regular player loses there. No tank in the 4
        // corridors of the first stretch tells no player from another.
        assert_eq!(cell(places, "11-15", "nearest"), "regular");
        assert_eq!(cell(places, "1-2", "nearest"), "-");
        // The arena is told apart, and no stretch counts it: the players fight in none.
        assert_eq!(places.rows.len(), 6);
        assert_eq!(cell(places, "arenas", "entered"), "1");
        assert_eq!(cell(places, "arenas", "lost in each"), "2.00");
        assert_eq!(cell(places, "arenas", "regular"), "-");

        // Over its 40 corridors, the journal lost 15 tanks. The regular player would lose
        // 0.3 x 20 + 0.6 x 20 = 18 there, the novice 48 and the expert 6.
        let whole = &tables[1];
        assert_eq!(cell(whole, "journal", "lost"), "15");
        assert_eq!(cell(whole, "journal", "plays as"), "regular");
        assert_eq!(cell(whole, "novice", "plays as"), "");
        assert_eq!(cell(whole, "novice", "lost"), "48.0");
        assert_eq!(cell(whole, "regular", "lost"), "18.0");
        assert_eq!(cell(whole, "regular", "for each of the journal's"), "1.20");
        assert_eq!(cell(whole, "expert", "lost"), "6.0");

        // The journal lost every tank to the bullets of gunners. A player loses every other
        // tank of the late sectors to a mine: 6 of its 18 over the sectors of the journal.
        let weapons = &tables[2];
        assert_eq!(cell(weapons, "Bullet, normal", "journal"), "100%");
        assert_eq!(cell(weapons, "Mine", "journal"), "0%");
        assert_eq!(cell(weapons, "Mine", "regular"), "33%");
        assert_eq!(cell(weapons, "Bullet, normal", "regular"), "67%");
        let classes = &tables[3];
        assert_eq!(cell(classes, "Gunner", "journal"), "100%");
        assert_eq!(cell(classes, "Sapper", "novice"), "33%");
    }

    #[test]
    fn the_nearest_player_is_the_one_whose_losses_make_those_of_the_journal_most_likely() {
        let players = players();
        let over = [(1, 10), (11, 20)];
        let journal = |lost: &[u32]| Told::of_entries(&[&entry((1..=20).collect(), lost)], false);
        // No tank lost in twenty sectors is what the expert comes nearest to, who loses 3
        // there, and a tank in every sector what the novice does.
        let nobody = journal(&[]);
        assert_eq!(nearest(&nobody, &players, &over), [Reference::Expert]);
        let every: Vec<u32> = (1..=20).collect();
        let novice = nearest(&journal(&every), &players, &over);
        assert_eq!(novice, [Reference::Novice]);
        let some = journal(&[3, 8, 12, 13, 14, 15, 16, 17, 18]);
        assert_eq!(nearest(&some, &players, &over), [Reference::Regular]);
        // Where the journal lost five tanks, the expert would lose 3 and the regular player
        // 9: so few don't tell the two apart.
        let between = nearest(&journal(&[4, 9, 12, 15, 19]), &players, &over);
        assert_eq!(between, [Reference::Regular, Reference::Expert]);
        assert_eq!(named(&between, 3), "regular or expert");
        // A journal that entered one sector and lost nothing tells no player from another.
        let short = Told::of_entries(&[&entry(vec![1], &[])], false);
        let any = nearest(&short, &players, &over);
        assert_eq!(any.len(), 3);
        assert_eq!(named(&any, 3), "-");
        // Where the journal entered nothing, nobody is told.
        assert!(nearest(&nobody, &players, &[(30, 40)]).is_empty());
        assert!(nearest(&Told::default(), &players, &over).is_empty());
        assert_eq!(named(&[], 3), "-");
        // A player who fought in no sector of a stretch that the journal entered is not one
        // that the journal plays as, however well it fits the rest, and nothing is told of
        // what it would lose there.
        let mut short = player(1);
        short.entered.retain(|sector| *sector <= 10);
        short.losses.retain(|loss| loss.sector <= 10);
        let lineup = [
            (Reference::Novice, player(8)),
            (Reference::Expert, short.clone()),
        ];
        assert_eq!(nearest(&nobody, &lineup, &over), [Reference::Novice]);
        assert_eq!(nearest(&nobody, &lineup, &[(1, 10)]), [Reference::Expert]);
        assert_eq!(expected(&nobody, &short, &over, |_| true), None);
        assert_eq!(expected(&nobody, &short, &[(1, 10)], |_| true), Some(1.0));
        // A player who lost nothing in a stretch is not ruled out by one loss there.
        let flawless = Told {
            entered: (0..10).flat_map(|_| 1..=20).collect(),
            losses: Vec::new(),
        };
        let only = [(Reference::Expert, flawless)];
        let alone = nearest(&journal(&[5]), &only, &over);
        assert_eq!(alone, [Reference::Expert]);
        assert_eq!(named(&alone, 1), "expert");
    }

    #[test]
    fn a_fight_tells_what_its_player_would_have_lost_and_to_whom() {
        let tank = |class: Class, exploding: bool| {
            let mut genome = Genome::empty(60);
            genome.class = class;
            genome.set(tanku_core::stats::Stat::Exploding, exploding as u8);
            Fought {
                genome,
                lasted: 20.0,
                fell: true,
            }
        };
        let loss = |cause: fight::Cause, by: Option<usize>| fight::Loss { cause, by };
        let fight = Outcome {
            group: vec![tank(Class::Sniper, false), tank(Class::Grenadier, true)],
            lasted: 20.0,
            losses: vec![
                loss(fight::Cause::Bullet, Some(0)),
                loss(fight::Cause::Exploding, Some(1)),
                loss(fight::Cause::Mine, Some(0)),
                loss(fight::Cause::Crate, None),
            ],
            own: 1,
        };
        let told = Told::of_fights(&[(7, fight), (8, Outcome::default())]);
        assert_eq!(told.entered, [7, 8]);
        let rows: Vec<(u32, &str, &str)> = told
            .losses
            .iter()
            .map(|loss| (loss.sector, weapon(loss), culprit(loss)))
            .collect();
        assert_eq!(
            rows,
            [
                (7, "Bullet, normal", "Sniper"),
                (7, "Bullet, exploding", "Grenadier"),
                (7, "Mine", "Sniper"),
                (7, "Crate", "Nobody"),
                (7, "The player's own bullet or mine", "The player"),
            ]
        );
        assert_eq!(told.rate((6, 10)), Some(2.5));
        assert_eq!(told.rate((1, 5)), None);
    }

    #[test]
    fn whole_runs_are_set_next_to_the_runs_of_the_players_with_three_lives() {
        // A whole run that cleared 19 sectors, one that cleared 9, and a playtest.
        let far = entry((1..=20).collect(), &[18, 19, 20]);
        let near = entry((1..=10).collect(), &[4, 9, 10]);
        let mut playtest = entry((1..=30).collect(), &[]);
        (playtest.playtest, playtest.endless, playtest.end) = (true, true, End::Left);
        let scores = vec![(Reference::Regular, vec![17, 12, 20, 15])];
        let tables = tables(&[&far, &near, &playtest], &players(), &scores);
        let runs = tables.last().unwrap();
        assert_eq!(runs.title, "Whole runs");
        assert_eq!(cell(runs, "journal", "runs"), "2");
        assert_eq!(cell(runs, "journal", "mean"), "14.0");
        assert_eq!(cell(runs, "journal", "best"), "19");
        assert_eq!(cell(runs, "regular", "mean"), "16.0");
        assert_eq!(cell(runs, "regular", "median"), "17");
        // A journal without a whole run has no such table.
        let tables = self::tables(&[&playtest], &players(), &[]);
        assert!(tables.iter().all(|table| table.title != "Whole runs"));
    }

    #[test]
    fn the_report_reads_a_journal_and_sends_the_players_to_its_sectors() {
        let mut team = entry(vec![1, 2, 3], &[]);
        team.players = 3;
        let text = format!(
            "{}\n{}\n{}\nnot a run\n",
            tanku_core::journal::HEADING,
            entry(vec![1, 2, 3], &[2]).line(),
            team.line(),
        );
        let report = journal(&asked(), &text);
        assert_eq!(report.title, "Journal");
        assert!(report.asked.contains("journal.txt"));
        let read = &report.tables[0];
        assert_eq!(cell(read, "runs of the journal", "count"), "2");
        assert_eq!(cell(read, "lines that are not a run", "count"), "1");
        assert_eq!(cell(read, "left out: several players", "count"), "1");
        assert_eq!(cell(read, "counted", "count"), "1");
        assert_eq!(cell(read, "of them whole runs", "count"), "1");
        assert_eq!(cell(read, "corridors that they entered", "count"), "3");
        // The players fought for the three sectors of the journal, on two seeds each.
        let places = &report.tables[1];
        assert_eq!(cell(places, "1-2", "entered"), "2");
        assert_eq!(cell(places, "3", "lost in each"), "0.00");
        assert!(cell(places, "3", "novice").parse::<f64>().is_ok());
        assert_eq!(report.tables.last().unwrap().title, "Whole runs");
        assert_eq!(report.csv(), journal(&asked(), &text).csv());
        // A journal with nothing to count tells what it read, and sends nobody anywhere.
        let report = journal(&asked(), &team.line());
        assert_eq!(report.tables.len(), 1);
        assert_eq!(cell(&report.tables[0], "counted", "count"), "0");
    }
}
