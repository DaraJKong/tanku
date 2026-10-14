//! Tests of the teammates that the game drives in a playtest.

use tanku_core::{geom, reference::Reference};

use crate::field::Solid;

use super::*;

/// Starts a run of so many players, of which the game drives the last ones, in a playtest or
/// not.
fn with_teammates(players: u32, count: u8, player: Reference, playtest: bool) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
    if playtest {
        app.insert_resource(Playtest::default());
    }
    app.world_mut().write_message(RunRequest::Start {
        seed: "TEST".into(),
        players,
        drawn: true,
        difficulty: Difficulty::Normal,
        free: false,
        arena: false,
        tanks: Vec::new(),
        teammates: Teammates { count, player },
    });
    tick(&mut app, 1);
    events(&mut app);
    app
}

/// A playtest of two players, the second of whom the game drives.
fn playtest() -> App {
    with_teammates(2, 1, Reference::Regular, true)
}

/// Ticks until something holds, for at most so many seconds of the run. Tells whether it
/// did.
fn within(app: &mut App, limit: f32, done: impl Fn(&mut App) -> bool) -> bool {
    for _ in 0..seconds(limit) {
        if done(app) {
            return true;
        }
        tick(app, 1);
    }
    done(app)
}

fn sector_of(app: &App, tank: Entity) -> u32 {
    app.world().get::<Tank>(tank).unwrap().sector
}

/// Nothing destroys this tank: a test of the way of a teammate needs nobody to fall.
fn shelter_tank(app: &mut App, tank: Entity) {
    app.world_mut().entity_mut(tank).insert(Protected(1.0e9));
}

#[test]
fn the_game_drives_teammates_in_a_playtest_alone() {
    // Outside a playtest, nobody drives the second seat: its tank stays where it started.
    let mut app = with_teammates(2, 1, Reference::Regular, false);
    let mate = seat(&mut app, 1);
    let start = position(&mut app, mate);
    tick(&mut app, seconds(3.0));
    assert_eq!(position(&mut app, mate), start);
    // In a playtest, it drives off, and the first seat is still the player's alone.
    let mut app = playtest();
    let (player, mate) = (seat(&mut app, 0), seat(&mut app, 1));
    let (from, start) = (position(&mut app, player), position(&mut app, mate));
    tick(&mut app, seconds(3.0));
    assert!(position(&mut app, mate).distance(start) > 2.0);
    assert_eq!(position(&mut app, player), from);
}

#[test]
fn a_teammate_waits_at_the_barrier_and_crosses_after_the_player() {
    let mut app = playtest();
    let (player, mate) = (seat(&mut app, 0), seat(&mut app, 1));
    shelter_tank(&mut app, player);
    // The start bay holds no enemy: the teammate drives to its barrier, and waits in front of
    // it, within the reach that unlocks it.
    let gate = barrier(&mut app, 0);
    let reach = BARRIER_GATHER_DISTANCE - 2.0;
    let waits = |app: &mut App| {
        let at = position(app, mate);
        geom::distance_to_segment(at, gate.from, gate.to) <= reach
    };
    assert!(within(&mut app, 20.0, waits));
    tick(&mut app, seconds(5.0));
    assert!(waits(&mut app));
    assert_eq!(sector_of(&app, mate), 0);
    // The player is still far from it: the barrier stays locked.
    assert!(!barrier(&mut app, 0).unlocked);
    // Once the player is there too, it unlocks, and the teammate still lets the player go
    // first: it never wakes a sector.
    carry(&mut app, player, 0, false);
    tick(&mut app, 2);
    assert!(barrier(&mut app, 0).unlocked);
    tick(&mut app, seconds(3.0));
    assert_eq!((sector_of(&app, mate), run(&app).sector), (0, 0));
    // The player crosses, and the teammate follows.
    carry(&mut app, player, 0, true);
    tick(&mut app, 2);
    assert_eq!(run(&app).sector, 1);
    assert!(within(&mut app, 10.0, |app| sector_of(app, mate) == 1));
}

#[test]
fn a_teammate_fights_the_enemies_of_its_sector() {
    let mut app = with_teammates(2, 1, Reference::Expert, true);
    let (player, mate) = (seat(&mut app, 0), seat(&mut app, 1));
    shelter_tank(&mut app, player);
    shelter_tank(&mut app, mate);
    // The player crosses into the first sector once the teammate is at the barrier, and
    // stands there: the teammate follows, and destroys every enemy of the sector.
    let gate = barrier(&mut app, 0);
    let close = |app: &mut App| {
        let at = position(app, mate);
        geom::distance_to_segment(at, gate.from, gate.to) <= BARRIER_GATHER_DISTANCE
    };
    assert!(within(&mut app, 20.0, close));
    carry(&mut app, player, 0, false);
    tick(&mut app, 2);
    carry(&mut app, player, 0, true);
    tick(&mut app, 2);
    assert_eq!(run(&app).sector, 1);
    assert!(!enemies(&mut app, 1).is_empty());
    let cleared = |app: &mut App| app.world().resource::<Field>().cleared.contains(&1);
    assert!(within(&mut app, 120.0, cleared));
}

#[test]
fn a_teammate_repairs_the_wreck_of_the_player() {
    let mut app = playtest();
    let player = seat(&mut app, 0);
    destroy(&mut app, player);
    assert!(is_wreck(&app, player));
    // The teammate comes to the wreck, and the repair takes 2 s once it is there.
    assert!(within(&mut app, 15.0, |app| !is_wreck(app, player)));
    assert_eq!(run(&app).phase, RunPhase::Playing);
}

#[test]
fn the_nearest_teammate_repairs_a_wreck_and_the_others_go_on() {
    let mut app = with_teammates(3, 2, Reference::Regular, true);
    let (player, near, far) = (seat(&mut app, 0), seat(&mut app, 1), seat(&mut app, 2));
    // The second seat is next to the wreck of the player, the third at the barrier.
    let at = position(&mut app, player);
    teleport(&mut app, near, at + Vec2::X * 4.0);
    carry(&mut app, far, 0, false);
    destroy(&mut app, player);
    let mended = |app: &mut App| {
        let wreck = app.world().get::<Wreck>(player).copied();
        wreck.is_some_and(|wreck| wreck.repairers != 0)
    };
    assert!(within(&mut app, 5.0, mended));
    // The one at work is the second seat alone, and the third stays where it waits.
    let wreck = *app.world().get::<Wreck>(player).unwrap();
    assert_eq!(wreck.repairers, 1 << 1);
    assert!(within(&mut app, 5.0, |app| !is_wreck(app, player)));
    assert!(position(&mut app, far).distance(at) > 2.0 * REPAIR_RADIUS);
}

#[test]
fn a_team_without_a_person_goes_from_sector_to_sector() {
    // Three teammates and nobody else: the first of them leads through the barriers.
    let mut app = with_teammates(3, 3, Reference::Expert, true);
    for seat_number in 0..3 {
        let tank = seat(&mut app, seat_number);
        shelter_tank(&mut app, tank);
    }
    assert!(within(&mut app, 300.0, |app| run(app).cleared >= 3));
    // They stayed together: every tank is in the sector that the run has reached, or behind
    // the barrier that it is crossing.
    let front = run(&app).sector;
    for seat_number in 0..3 {
        let tank = seat(&mut app, seat_number);
        assert!(sector_of(&app, tank) + 1 >= front);
    }
}

/// How a run of a team of teammates ended.
#[derive(Clone, Debug, PartialEq)]
enum Ending {
    /// Every tank of the team was a wreck at once.
    Lost,
    /// Nothing was cleared for a long time: what held the team.
    Stalled(String),
    /// The time of the trial ran out.
    Finished,
}

/// What a team that stopped clearing sectors was doing: where each tank stood, and what was
/// left to fight.
fn held(app: &mut App) -> String {
    let front = run(app).sector;
    let gate = {
        let world = app.world_mut();
        let mut barriers = world.query::<&Barrier>();
        barriers
            .iter(world)
            .find(|gate| gate.sector == front)
            .copied()
    };
    let world = app.world_mut();
    let mut tanks = world.query::<(&Player, &Tank, &Position, Has<Wreck>)>();
    let mut told: Vec<String> = tanks
        .iter(world)
        .map(|(player, tank, at, wreck)| {
            let away = gate.map_or(f32::NAN, |gate| {
                geom::distance_to_segment(at.0, gate.from, gate.to)
            });
            let state = if wreck { " wreck" } else { "" };
            format!(
                "P{} in {}{state}, {away:.1} m from the barrier",
                player.seat + 1,
                tank.sector
            )
        })
        .collect();
    told.sort();
    let mut foes = world.query::<(&Tank, &Enemy)>();
    let awake = foes
        .iter(world)
        .filter(|(tank, enemy)| tank.sector == front && enemy.awake);
    let field = world.resource::<Field>();
    let waiting = field
        .arenas
        .get(&front)
        .map_or(0, |arena| arena.incoming.len());
    let unlocked = gate.is_some_and(|gate| gate.unlocked);
    format!(
        "sector {front}, cleared {}, barrier {}, {} enemies awake, {waiting} waiting for room: {}",
        field.cleared.contains(&front),
        if unlocked { "open" } else { "locked" },
        awake.count(),
        told.join("; ")
    )
}

/// A run of so many teammates and nobody else, in the hands of one reference player: the
/// sectors that it cleared, and how it ended.
fn team_run(seed: &str, players: u32, player: Reference) -> (u32, Ending) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
    app.edit_schedule(FixedUpdate, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    app.insert_resource(Playtest::default());
    let count = players as u8;
    app.world_mut().write_message(RunRequest::Start {
        seed: seed.into(),
        players,
        drawn: true,
        difficulty: Difficulty::Normal,
        free: false,
        arena: false,
        tanks: Vec::new(),
        teammates: Teammates { count, player },
    });
    // A stall is a long time without a sector cleared: longer than any fight that ends.
    let (limit, patience) = (15.0 * 60.0, 180.0);
    let (mut cleared, mut since) = (0, 0.0);
    let mut played = 0.0;
    while played < limit {
        tick(&mut app, seconds(1.0));
        played += 1.0;
        let state = run(&app);
        if state.phase == RunPhase::Over {
            return (state.cleared, Ending::Lost);
        }
        if state.cleared > cleared {
            (cleared, since) = (state.cleared, 0.0);
        } else {
            since += 1.0;
        }
        if since > patience {
            return (cleared, Ending::Stalled(held(&mut app)));
        }
    }
    (cleared, Ending::Finished)
}

/// Teams of two to four teammates alone, against the groups of as many players, on a dozen
/// seeds: how far they get, and whether a team ever stops, held at a barrier or in a fight
/// that it can't end. Nothing protects them. It takes a few minutes.
#[test]
#[ignore = "takes minutes and prints a table"]
fn teams_of_teammates_on_many_seeds() {
    let seeds: Vec<String> = (0..12).map(|number| format!("TRIAL{number}")).collect();
    println!(
        "\nTeams of regular teammates alone, on {} seeds.",
        seeds.len()
    );
    for players in 2..=4 {
        let ended: Vec<(u32, Ending)> = std::thread::scope(|scope| {
            let runs: Vec<_> = seeds
                .iter()
                .map(|seed| scope.spawn(move || team_run(seed, players, Reference::Regular)))
                .collect();
            runs.into_iter().map(|run| run.join().unwrap()).collect()
        });
        let mut scores: Vec<u32> = ended.iter().map(|(cleared, _)| *cleared).collect();
        scores.sort();
        let mean = scores.iter().sum::<u32>() as f32 / scores.len() as f32;
        let stalled = ended
            .iter()
            .filter(|(_, end)| matches!(end, Ending::Stalled(_)));
        println!(
            "{players} players: mean {mean:.1} sectors, scores {scores:?}, {} stalled",
            stalled.clone().count()
        );
        for ((cleared, end), seed) in ended.iter().zip(&seeds) {
            if let Ending::Stalled(why) = end {
                println!("  {seed}, after {cleared} sectors: {why}");
            }
        }
    }
}

/// The x of the wall on the west side of the start bay, where it passes the start.
fn west_wall(app: &mut App) -> f32 {
    let field = app.world().resource::<Field>();
    let upright = field.colliders.iter().filter(|collider| {
        let spans =
            collider.a.y.min(collider.b.y) <= START.y && collider.a.y.max(collider.b.y) >= START.y;
        collider.solid == Solid::Wall
            && collider.a.x == collider.b.x
            && collider.a.x < START.x
            && spans
    });
    upright
        .map(|collider| collider.a.x)
        .fold(f32::MIN, f32::max)
}

#[test]
fn a_teammate_repairs_a_wreck_in_a_gap_that_a_tank_just_fits_in() {
    // A scene that a playtest found: the wreck of the player lies between a wall and a column
    // of three blocks 2 m from it, and the teammate stands on the other side of the column. It
    // found no way into the gap, and turned one way along the column and then the other.
    let mut app = playtest();
    let (player, mate) = (seat(&mut app, 0), seat(&mut app, 1));
    let west = west_wall(&mut app);
    for y in [5.0, 7.0, 9.0] {
        let at = Vec2::new(west + 3.0, y);
        app.world_mut()
            .spawn((Block { sector: 0 }, Position(at), InSector(0)));
    }
    app.world_mut().resource_mut::<Field>().stale = true;
    teleport(&mut app, player, Vec2::new(west + 1.0, 8.0));
    teleport(&mut app, mate, Vec2::new(west + 5.0, 8.0));
    tick(&mut app, 2);
    destroy(&mut app, player);
    assert!(is_wreck(&app, player));
    assert!(within(&mut app, 20.0, |app| !is_wreck(app, player)));
}
