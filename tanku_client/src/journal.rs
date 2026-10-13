//! The journal of the runs that are played on this machine: a line for each run, added to a
//! file when the run ends. `tanku_core::journal` says what a line holds.
//!
//! The machine that runs the simulation of a run keeps its line: a single player's, and the
//! host's of a game of several players, whose line tells the run of the whole team. A guest
//! keeps none: what it knows of a run is what its host tells it, a moment late. A run that
//! entered no sector has nothing to tell and leaves no line.
//!
//! The file is the player's own, next to their settings. A game that was started with
//! `--journal FILE` keeps it in that file instead, which is how a run that leaves the
//! player's files alone still keeps one.

use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::prelude::*;
use tanku_core::{
    balance::FINGERPRINT,
    journal::{self, End, Entry, Loss},
    map::SectorKind,
    seed::VERSION,
    sim::{Playtest, Run, Sector, SimEvent, Team},
};

use crate::{cli::Options, session::Session, storage};

/// The run that is being played, as the journal will tell it, and the runs that have ended
/// and are not written yet.
#[derive(Resource, Default)]
struct Logbook {
    open: Option<Entry>,
    closed: Vec<Entry>,
}

impl Logbook {
    /// Ends the run that is being played. One that entered no sector leaves nothing.
    fn close(&mut self, end: End) {
        if let Some(mut entry) = self.open.take() {
            entry.end = end;
            if !entry.entered.is_empty() {
                self.closed.push(entry);
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Logbook>()
        .add_systems(Update, (keep, write).chain())
        // A game that is closed in the middle of a run still tells of it.
        .add_systems(Last, (leave, write).chain());
}

/// The run that starts now, before anything has happened in it.
fn begin(run: &Run, now: u64) -> Entry {
    Entry {
        when: journal::stamp(now),
        version: VERSION.to_string(),
        balance: FINGERPRINT,
        seed: run.seed_text.clone(),
        difficulty: run.difficulty,
        players: run.team,
        arena: run.arena,
        free: run.free,
        playtest: run.playtest,
        endless: false,
        entered: Vec::new(),
        arenas: Vec::new(),
        cleared: 0,
        losses: Vec::new(),
        fought: 0,
        kills: 0,
        end: End::Left,
    }
}

/// Takes from the run what it counts itself.
fn follow(entry: &mut Entry, run: &Run, endless: bool) {
    entry.cleared = run.cleared;
    entry.kills = run.kills;
    entry.fought = (run.ticks as f32 * tanku_core::rules::TICK).round() as u32;
    entry.endless |= endless;
}

/// The seconds since 1970, as the clock of the machine has them.
fn now() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH);
    since.map_or(0, |since| since.as_secs())
}

/// Follows the run that is played, and closes it when it is lost, left or replaced.
fn keep(
    run: Option<Res<Run>>,
    session: Res<Session>,
    playtest: Option<Res<Playtest>>,
    sectors: Query<&Sector>,
    mut events: MessageReader<SimEvent>,
    mut logbook: ResMut<Logbook>,
) {
    if *session == Session::Guest {
        events.clear();
        logbook.open = None;
        return;
    }
    let run = run.as_deref();
    let endless = playtest.is_some_and(|playtest| playtest.infinite_lives);
    let news: Vec<SimEvent> = events.read().copied().collect();
    // Until another run takes its place, the run in the world is the one that is followed.
    let replaced = news.contains(&SimEvent::RunStarted);
    if let (false, Some(run), Some(open)) = (replaced, run, &mut logbook.open) {
        follow(open, run, endless);
    }
    for event in news {
        match event {
            SimEvent::RunStarted => {
                logbook.close(End::Left);
                logbook.open = run.map(|run| begin(run, now()));
            }
            SimEvent::SectorEntered { sector } => {
                let plans = sectors.iter().map(|sector| &sector.layout.plan);
                let arena = plans
                    .into_iter()
                    .any(|plan| plan.index == sector && plan.kind == SectorKind::Arena);
                if let Some(open) = &mut logbook.open {
                    open.entered.push(sector);
                    if arena {
                        open.arenas.push(sector);
                    }
                }
            }
            SimEvent::TankDestroyed {
                team: Team::Players,
                ..
            } => {
                if let (Some(open), Some(run)) = (&mut logbook.open, run) {
                    open.losses.push(Loss::of(run.sector, run.lost_to));
                }
            }
            SimEvent::RunOver => logbook.close(End::Lost),
            _ => {}
        }
    }
    match (run, &mut logbook.open) {
        (Some(run), Some(open)) => follow(open, run, endless),
        // A run that is gone was left.
        (None, Some(_)) => logbook.close(End::Left),
        _ => {}
    }
}

/// Closes the run that is being played when the game is closed.
fn leave(mut exits: MessageReader<AppExit>, mut logbook: ResMut<Logbook>) {
    if exits.read().next().is_some() {
        logbook.close(End::Left);
    }
}

/// Adds the runs that have ended to the journal.
fn write(options: Option<Res<Options>>, mut logbook: ResMut<Logbook>) {
    if logbook.closed.is_empty() {
        return;
    }
    let file: Option<PathBuf> = options.and_then(|options| options.journal.clone());
    for entry in logbook.closed.drain(..) {
        let line = entry.line();
        match &file {
            Some(file) => storage::append_to(file, journal::HEADING, &line),
            None => storage::append(journal::FILE, journal::HEADING, &line),
        }
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::{
        journal::{Cause, Culprit},
        map::Corridor,
        modes::Difficulty,
        sim::{Blow, RunPhase, Weapon},
        stats::{Class, Genome},
    };

    use super::*;

    /// A world in which a run can be told by hand: its events, and what it says of itself.
    fn world() -> App {
        let mut app = App::new();
        app.init_resource::<Logbook>()
            .init_resource::<Session>()
            .add_message::<SimEvent>()
            .add_message::<AppExit>()
            .add_systems(Update, keep)
            .add_systems(Last, leave);
        app
    }

    fn run(seed: &str) -> Run {
        Run {
            seed_text: seed.to_string(),
            seed: tanku_core::seed::seed_value(seed),
            difficulty: Difficulty::Normal,
            players: 1,
            team: 1,
            solo: true,
            sector: 0,
            cleared: 0,
            checkpoint: 0,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase: RunPhase::Playing,
            lost_to: None,
            playtest: false,
            earns: true,
            free: false,
            arena: false,
            lull: None,
            paused: false,
        }
    }

    fn tell(app: &mut App, event: SimEvent) {
        app.world_mut().write_message(event);
    }

    /// Starts a run on a seed, as the server does: the run, then the word of it.
    fn start(app: &mut App, seed: &str) {
        app.insert_resource(run(seed));
        tell(app, SimEvent::RunStarted);
        app.update();
    }

    /// The front of the run comes into a sector.
    fn enter(app: &mut App, sector: u32) {
        app.world_mut().resource_mut::<Run>().sector = sector;
        tell(app, SimEvent::SectorEntered { sector });
        app.update();
    }

    /// The player's tank is destroyed, by a blow if the run knows of one.
    fn lose(app: &mut App, blow: Option<Blow>) {
        app.world_mut().resource_mut::<Run>().lost_to = blow;
        let (at, team) = (Vec2::ZERO, Team::Players);
        tell(app, SimEvent::TankDestroyed { at, team });
        app.update();
    }

    fn closed(app: &App) -> Vec<Entry> {
        app.world().resource::<Logbook>().closed.clone()
    }

    fn blow(class: Class, weapon: Weapon) -> Blow {
        let mut genome = Genome::empty(40);
        genome.class = class;
        Blow {
            weapon,
            genome,
            team: Team::Enemies,
        }
    }

    #[test]
    fn a_run_that_is_lost_leaves_a_line_with_what_happened_in_it() {
        let mut app = world();
        start(&mut app, "LOGBOOK");
        assert!(closed(&app).is_empty());
        // The second sector is an arena, as the map of the run says.
        let mut corridor = Corridor::new(tanku_core::seed::seed_value("LOGBOOK"));
        let arena = (1..60).find(|sector| corridor.plan(*sector).kind == SectorKind::Arena);
        let arena = arena.expect("an arena in sixty sectors");
        let layout = corridor.layout(arena);
        app.world_mut().spawn(Sector { layout });
        enter(&mut app, 1);
        lose(&mut app, Some(blow(Class::Gunner, Weapon::Bullet)));
        enter(&mut app, arena);
        lose(&mut app, Some(blow(Class::Sapper, Weapon::Mine)));
        lose(&mut app, None);
        // The run counts its own score, its kills and its time.
        let mut told = app.world_mut().resource_mut::<Run>();
        (told.cleared, told.kills, told.ticks) = (arena - 1, 12, 60 * 95 + 20);
        told.phase = RunPhase::Over;
        tell(&mut app, SimEvent::RunOver);
        app.update();

        let lines = closed(&app);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!((line.seed.as_str(), line.players), ("LOGBOOK", 1));
        assert_eq!(
            (line.version.as_str(), line.balance),
            (VERSION, FINGERPRINT)
        );
        assert_eq!(line.entered, [1, arena]);
        assert_eq!(line.arenas, [arena]);
        assert_eq!((line.cleared, line.kills, line.fought), (arena - 1, 12, 95));
        assert_eq!(line.end, End::Lost);
        let losses: Vec<(u32, Cause, Culprit)> = line
            .losses
            .iter()
            .map(|loss| (loss.sector, loss.cause, loss.by))
            .collect();
        let bullet = Cause::Bullet(tanku_core::stats::Ammo::Normal);
        assert_eq!(
            losses,
            [
                (1, bullet, Culprit::Enemy(Class::Gunner)),
                (arena, Cause::Mine, Culprit::Enemy(Class::Sapper)),
                (arena, Cause::Unknown, Culprit::Nobody),
            ]
        );
        assert!(!line.playtest && !line.endless && !line.arena && !line.free);
        // It is told once: the run that stays on the screen adds nothing.
        app.update();
        app.update();
        assert_eq!(closed(&app).len(), 1);
        assert!(
            line.when.ends_with('Z') && line.when.len() == 17,
            "{}",
            line.when
        );
    }

    #[test]
    fn a_run_that_is_left_or_replaced_is_told_as_it_was_then() {
        let mut app = world();
        // A run that entered no sector has nothing to tell.
        start(&mut app, "FIRST");
        app.world_mut().remove_resource::<Run>();
        app.update();
        assert!(closed(&app).is_empty());
        // A run that its player quits.
        start(&mut app, "SECOND");
        enter(&mut app, 1);
        app.world_mut().resource_mut::<Run>().cleared = 1;
        app.update();
        app.world_mut().remove_resource::<Run>();
        app.update();
        // A run that another one replaces, as a retry does: the new one is not mistaken
        // for the one before it.
        start(&mut app, "THIRD");
        enter(&mut app, 1);
        enter(&mut app, 2);
        app.world_mut().resource_mut::<Run>().cleared = 2;
        app.update();
        start(&mut app, "FOURTH");
        enter(&mut app, 1);
        // The game is closed in the middle of the last one.
        app.world_mut().write_message(AppExit::Success);
        app.update();

        let told: Vec<(String, Vec<u32>, u32, End)> = closed(&app)
            .into_iter()
            .map(|line| (line.seed, line.entered, line.cleared, line.end))
            .collect();
        assert_eq!(
            told,
            [
                ("SECOND".to_string(), vec![1], 1, End::Left),
                ("THIRD".to_string(), vec![1, 2], 2, End::Left),
                ("FOURTH".to_string(), vec![1], 0, End::Left),
            ]
        );
    }

    #[test]
    fn a_playtest_tells_its_skips_and_its_infinite_lives() {
        let mut app = world();
        app.insert_resource(Playtest {
            infinite_lives: true,
        });
        let mut playtest = run("PLAYTEST");
        (playtest.playtest, playtest.earns) = (true, false);
        app.insert_resource(playtest);
        tell(&mut app, SimEvent::RunStarted);
        app.update();
        enter(&mut app, 1);
        // The player skips to sector 15, then turns the infinite lives off.
        enter(&mut app, 15);
        app.world_mut().resource_mut::<Playtest>().infinite_lives = false;
        enter(&mut app, 16);
        app.world_mut().remove_resource::<Run>();
        app.update();
        let lines = closed(&app);
        assert_eq!(lines[0].entered, [1, 15, 16]);
        assert!(lines[0].playtest && lines[0].endless);
        assert!(
            lines[0]
                .line()
                .contains("options playtest,endless entered 1,15-16")
        );
        assert!(!lines[0].whole());
    }

    #[test]
    fn the_host_tells_the_run_of_the_team_and_a_guest_tells_nothing() {
        let mut app = world();
        app.insert_resource(Session::Host);
        let mut team = run("TEAM");
        (team.players, team.team, team.solo) = (3, 3, false);
        app.insert_resource(team);
        tell(&mut app, SimEvent::RunStarted);
        app.update();
        enter(&mut app, 1);
        // A player leaves: the run is still one of three, as its record is.
        app.world_mut().resource_mut::<Run>().players = 2;
        lose(&mut app, Some(blow(Class::Sniper, Weapon::Bullet)));
        lose(&mut app, Some(blow(Class::Sniper, Weapon::Bullet)));
        app.world_mut().remove_resource::<Run>();
        app.update();
        let lines = closed(&app);
        assert_eq!((lines[0].players, lines[0].losses.len()), (3, 2));

        let mut app = world();
        app.insert_resource(Session::Guest);
        start(&mut app, "GUEST");
        enter(&mut app, 1);
        app.world_mut().remove_resource::<Run>();
        app.update();
        assert!(closed(&app).is_empty());
    }
}
