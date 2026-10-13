//! Tests of arena mode: a run in one room, against one group after another.

use tanku_core::{
    enemy,
    map::Room,
    rules::{APPEAR_APART, LIFE_GROUPS, LULL, RISE_TIME},
    seed::seed_value,
    sim::{Planned, Reserve},
};

use super::*;
use crate::field::Solid;

/// Starts a run in arena mode. The tanks have just appeared, and the first group is in
/// reserve.
fn open_in(seed: &str, players: u32, difficulty: Difficulty) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
    app.world_mut().write_message(RunRequest::Start {
        seed: seed.into(),
        players,
        drawn: true,
        difficulty,
        free: false,
        arena: true,
        tanks: Vec::new(),
    });
    tick(&mut app, 2);
    events(&mut app);
    app
}

/// Starts the run of a single player in arena mode.
fn open(seed: &str) -> App {
    open_in(seed, 1, Difficulty::Normal)
}

/// Starts a playtest in arena mode in which the lives count, and skips to the lull before a
/// group.
fn before_group(seed: &str, players: u32, group: u32) -> App {
    let mut app = open_in(seed, players, Difficulty::Normal);
    app.insert_resource(Playtest::default());
    skip_to(&mut app, group);
    tick(&mut app, 1);
    events(&mut app);
    app
}

fn room_of(seed: &str) -> Room {
    Room::new(seed_value(seed))
}

fn sorted(mut points: Vec<Vec2>) -> Vec<Vec2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points
}

/// The middles of blocks or of crates, from their lowest corners.
fn centers(corners: &[IVec2]) -> Vec<Vec2> {
    sorted(
        corners
            .iter()
            .map(|corner| corner.as_vec2() + Vec2::ONE)
            .collect(),
    )
}

/// Where the blocks stand, and where the crates do.
fn standing(app: &mut App) -> (Vec<Vec2>, Vec<Vec2>) {
    let world = app.world_mut();
    let mut blocks = world.query_filtered::<&Position, With<Block>>();
    let blocks: Vec<Vec2> = blocks.iter(world).map(|at| at.0).collect();
    let mut crates = world.query_filtered::<&Position, With<ExplosiveCrate>>();
    let crates: Vec<Vec2> = crates.iter(world).map(|at| at.0).collect();
    (sorted(blocks), sorted(crates))
}

/// What is planned and does not stand yet: where, and what is told of it.
fn planned(app: &mut App) -> Vec<(Vec2, Planned)> {
    let world = app.world_mut();
    let mut marks: Vec<(Vec2, Planned)> = world
        .query::<(&Position, &Planned)>()
        .iter(world)
        .map(|(at, planned)| (at.0, *planned))
        .collect();
    marks.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.0.y.total_cmp(&b.0.y)));
    marks
}

/// The tanks that wait in reserve, in the order they come, with where each one stands.
fn reserves(app: &mut App) -> Vec<(Reserve, Vec2)> {
    let world = app.world_mut();
    let mut waiting: Vec<(Reserve, Vec2)> = world
        .query::<(&Reserve, &Position)>()
        .iter(world)
        .map(|(reserve, at)| (*reserve, at.0))
        .collect();
    waiting.sort_by_key(|(reserve, _)| reserve.number);
    waiting
}

fn numbers(app: &mut App) -> Vec<u32> {
    let waiting = reserves(app);
    waiting.iter().map(|(reserve, _)| reserve.number).collect()
}

/// Moves the tanks of the players out of the way of the group that is in reserve: further
/// from each place where one of its tanks appears than a mark asks for.
fn stand_aside(app: &mut App) {
    let aside = {
        let field = app.world().resource::<Field>();
        let state = field.room.as_ref().unwrap();
        let places: Vec<Vec2> = state.next.iter().flatten().map(|tank| tank.at).collect();
        let spots = state.room.places(1000, &state.blocks, &state.crates);
        let clear = |spot: &&IVec2| {
            let far = |place: &Vec2| place.distance(spot.as_vec2()) > APPEAR_ROOM + 3.0;
            places.iter().all(far)
        };
        spots.iter().find(clear).unwrap().as_vec2()
    };
    let world = app.world_mut();
    let mut tanks = world.query_filtered::<Entity, With<Player>>();
    let tanks: Vec<Entity> = tanks.iter(world).collect();
    for (number, tank) in tanks.into_iter().enumerate() {
        teleport(app, tank, aside + Vec2::X * number as f32 * 0.5);
    }
}

/// Waits for the end of the lull.
fn wait_for_the_group(app: &mut App) {
    for _ in 0..seconds(LULL + 1.0) {
        if run(app).lull.is_none() {
            return;
        }
        tick(app, 1);
    }
    panic!("the lull never ended");
}

fn incoming(event: &SimEvent) -> bool {
    matches!(event, SimEvent::TankIncoming { .. })
}

/// A group is announced as the one wave of one.
fn announced(event: &SimEvent) -> bool {
    *event == SimEvent::WaveIncoming { wave: 1, of: 1 }
}

fn cleared(event: &SimEvent) -> bool {
    matches!(event, SimEvent::SectorCleared { .. })
}

/// Which tank of its group an enemy is.
fn number(app: &App, tank: Entity) -> u32 {
    app.world().get::<Home>(tank).unwrap().number
}

/// The numbers of the tanks of a group that are in the room, in order.
fn there(app: &mut App, group: u32) -> Vec<u32> {
    let tanks = enemies(app, group);
    let mut numbers: Vec<u32> = tanks.iter().map(|&tank| number(app, tank)).collect();
    numbers.sort();
    numbers
}

#[test]
fn a_run_in_arena_mode_starts_in_its_room_with_the_first_group_in_reserve() {
    let mut app = open("TEST");
    let state = run(&app);
    assert!(state.arena && state.solo);
    assert_eq!((state.sector, state.cleared, state.checkpoint), (0, 0, 0));
    assert_eq!(state.lull, Some(LULL as u8));
    // What a run unlocks is earned in the corridor.
    assert!(!state.earns);

    // The room is all there is: no sector comes after it, and nothing closes it.
    let room = room_of("TEST");
    let world = app.world_mut();
    let mut sectors = world.query::<&Sector>();
    let kinds: Vec<SectorKind> = sectors
        .iter(world)
        .map(|sector| sector.layout.plan.kind)
        .collect();
    assert_eq!(kinds, [SectorKind::Room]);
    assert_eq!(world.query::<&Barrier>().iter(world).count(), 0);
    assert_eq!(world.query::<&Life>().iter(world).count(), 0);
    // The player stands in its middle, among what the first level of its terrain holds.
    let tank = player(&mut app).unwrap();
    assert_eq!(position(&mut app, tank), room.start().as_vec2());
    let (blocks, crates) = room.obstacles(1);
    assert!(!blocks.is_empty());
    assert_eq!(standing(&mut app), (centers(&blocks), centers(&crates)));
    assert!(planned(&mut app).is_empty());

    // The first group is the group of sector 1. It waits outside the south wall, and nothing
    // of it is in the room.
    let genomes = enemy::group(seed_value("TEST"), Difficulty::Normal, 1, 1);
    let waiting = reserves(&mut app);
    assert_eq!(waiting.len(), 1);
    let (reserve, at) = waiting[0];
    assert_eq!(
        (reserve.genome, reserve.group, reserve.number),
        (genomes[0], 1, 0)
    );
    assert!(at.y < room.plan().min.y as f32);
    assert!(enemies(&mut app, 0).is_empty() && enemies(&mut app, 1).is_empty());
}

#[test]
fn half_of_a_group_comes_after_the_lull_and_a_tank_for_each_one_that_is_destroyed() {
    // Group 12 has four tanks against a single player.
    let mut app = before_group("TEST", 1, 12);
    shelter(&mut app);
    stand_aside(&mut app);
    let state = run(&app);
    assert_eq!(
        (state.sector, state.cleared, state.checkpoint),
        (11, 11, 11)
    );
    // During the lull, the whole group is in reserve, in a row, and the room is empty.
    assert_eq!(numbers(&mut app), [0, 1, 2, 3]);
    let row: Vec<f32> = reserves(&mut app).iter().map(|(_, at)| at.x).collect();
    assert!(row.windows(2).all(|pair| pair[1] - pair[0] >= 2.5));
    // A gap parts the tanks that come first from those that wait.
    assert!(row[2] - row[1] > 1.5 * (row[1] - row[0]));
    assert!(enemies(&mut app, 12).is_empty());

    // Nothing comes before the lull is over.
    tick(&mut app, seconds(LULL) - 12);
    assert_eq!(run(&app).sector, 11);
    let early = |event: &SimEvent| {
        let entered = matches!(event, SimEvent::SectorEntered { .. });
        entered || matches!(event, SimEvent::WaveIncoming { .. }) || incoming(event)
    };
    assert_eq!(told(&mut app, early), 0);
    // Then the group is on: it is announced, and the places of half of its tanks are marked.
    tick(&mut app, 14);
    let news = events(&mut app);
    let entered = SimEvent::SectorEntered { sector: 12 };
    assert_eq!(count(&news, |event| *event == entered), 1);
    assert_eq!(count(&news, announced), 1);
    assert_eq!(count(&news, incoming), 2);
    let state = run(&app);
    assert_eq!((state.sector, state.lull, state.cleared), (12, None, 11));
    // A tank whose place is marked has left the reserve. The others wait there.
    assert_eq!(numbers(&mut app), [2, 3]);
    assert!(enemies(&mut app, 12).is_empty());

    // The tanks appear after their marks: awake, and the player is theirs to fight.
    tick(&mut app, seconds(APPEAR_TIME) + 2);
    let first = enemies(&mut app, 12);
    assert_eq!(there(&mut app, 12), [0, 1]);
    let awake = |app: &App, tank: Entity| app.world().get::<Enemy>(tank).unwrap().awake;
    assert!(first.iter().all(|&tank| awake(&app, tank)));
    let own = player(&mut app).unwrap();
    assert_eq!(app.world().get::<Tank>(own).unwrap().sector, 12);
    let genomes = enemy::group(seed_value("TEST"), Difficulty::Normal, 12, 4);
    for &tank in &first {
        let genome = app.world().get::<Tank>(tank).unwrap().genome;
        assert_eq!(genome, genomes[number(&app, tank) as usize]);
    }

    // Nobody comes while both are there.
    tick(&mut app, seconds(2.0));
    assert_eq!(told(&mut app, incoming), 0);
    assert_eq!(numbers(&mut app), [2, 3]);
    // A tank that is destroyed is replaced: the place of the next one is marked at once,
    // and nothing is announced again.
    let (gone, stays) = (number(&app, first[0]), number(&app, first[1]));
    destroy(&mut app, first[0]);
    tick(&mut app, 2);
    let news = events(&mut app);
    assert_eq!(count(&news, incoming), 1);
    let wave = |event: &SimEvent| matches!(event, SimEvent::WaveIncoming { .. });
    assert_eq!(count(&news, wave), 0);
    assert_eq!(numbers(&mut app), [3]);
    assert_eq!(there(&mut app, 12), [stays]);
    // It appears after its mark, awake, and the room is as full as it was.
    tick(&mut app, seconds(APPEAR_TIME) + 2);
    assert_eq!(there(&mut app, 12), [stays, 2]);
    assert!(!there(&mut app, 12).contains(&gone));
    let all = enemies(&mut app, 12);
    assert!(all.iter().all(|&tank| awake(&app, tank)));
    for &tank in &all {
        let genome = app.world().get::<Tank>(tank).unwrap().genome;
        assert_eq!(genome, genomes[number(&app, tank) as usize]);
    }
    assert_eq!(told(&mut app, incoming), 0);

    // Two tanks fall together, and one is left to come: the room is no longer full.
    destroy_enemies(&mut app, 12);
    tick(&mut app, 2);
    let news = events(&mut app);
    assert_eq!(count(&news, incoming), 1);
    assert_eq!(count(&news, cleared), 0);
    assert!(numbers(&mut app).is_empty());
    assert_eq!(run(&app).cleared, 11);
    tick(&mut app, seconds(APPEAR_TIME) + 2);
    assert_eq!(there(&mut app, 12), [3]);

    // The group is cleared with its last tank. That scores, and the next group goes into
    // reserve for the lull.
    destroy_enemies(&mut app, 12);
    tick(&mut app, 1);
    let news = events(&mut app);
    let over = SimEvent::SectorCleared { sector: 12 };
    assert_eq!(count(&news, |event| *event == over), 1);
    assert_eq!(count(&news, wave), 0);
    let state = run(&app);
    assert_eq!(
        (state.sector, state.cleared, state.checkpoint),
        (12, 12, 12)
    );
    assert_eq!(state.lull, Some(LULL as u8));
    let waiting = reserves(&mut app);
    assert_eq!(waiting.len(), 4);
    assert!(waiting.iter().all(|(reserve, _)| reserve.group == 13));
}

/// Puts the tanks of a group that are in the room to sleep, so that a test counts them
/// without their fight in its way: a crate that one of them sets off destroys the others.
fn put_to_sleep(app: &mut App, group: u32) {
    for tank in enemies(app, group) {
        app.world_mut().get_mut::<Enemy>(tank).unwrap().awake = false;
    }
}

#[test]
fn the_room_holds_half_of_a_group_for_as_long_as_a_tank_waits_in_reserve() {
    for (players, group) in [(1, 15), (1, 7), (4, 15), (2, 3)] {
        let mut app = before_group("BRAVO", players, group);
        for seat_number in 0..players as u8 {
            let tank = seat(&mut app, seat_number);
            app.world_mut().entity_mut(tank).insert(Protected(1.0e9));
        }
        stand_aside(&mut app);
        let size = progression::enemy_count(group, players, false);
        let (most, held) = progression::room_parts(size);
        assert_eq!(reserves(&mut app).len() as u32, size);
        wait_for_the_group(&mut app);
        let mut came: Vec<u32> = Vec::new();
        for fallen in 0..size {
            tick(&mut app, seconds(APPEAR_TIME) + 4);
            put_to_sleep(&mut app, group);
            // As long as a tank waits, the room is as full as when the group came. Then it
            // empties.
            let present = there(&mut app, group);
            let told = format!("{players} players, group {group}, {fallen} fallen");
            assert_eq!(present.len() as u32, most.min(size - fallen), "{told}");
            assert_eq!(
                numbers(&mut app).len() as u32,
                held.saturating_sub(fallen),
                "{told}"
            );
            assert_eq!(run(&app).cleared, group - 1);
            for tank in present {
                if !came.contains(&tank) {
                    came.push(tank);
                }
            }
            // One tank is destroyed: the one that has been there longest.
            let tanks = enemies(&mut app, group);
            let first = tanks.into_iter().min_by_key(|&tank| number(&app, tank));
            destroy(&mut app, first.unwrap());
        }
        tick(&mut app, 2);
        // Every tank of the group came, each once and in its order, and the group is cleared.
        assert_eq!(came, (0..size).collect::<Vec<u32>>());
        assert_eq!(run(&app).cleared, group);
        // It was announced once: the tanks that came later were told by their marks.
        let news = events(&mut app);
        assert_eq!(count(&news, announced), 1);
        assert_eq!(count(&news, incoming) as u32, size);
    }
}

#[test]
fn a_tank_does_not_appear_on_another_tank_of_its_group() {
    let mut app = before_group("TEST", 1, 12);
    shelter(&mut app);
    stand_aside(&mut app);
    // The third tank of the group is the first to wait for another one to be destroyed.
    let (place, spare) = {
        let field = app.world().resource::<Field>();
        let state = field.room.as_ref().unwrap();
        (state.next[1][0].at, state.spare.clone())
    };
    wait_for_the_group(&mut app);
    tick(&mut app, seconds(APPEAR_TIME) + 2);
    put_to_sleep(&mut app, 12);
    let first = enemies(&mut app, 12);
    assert_eq!(first.len(), 2);
    // One of the two that are there stands on its place when the other is destroyed.
    let squatter = first[1];
    teleport(&mut app, squatter, place + Vec2::X * 0.5);
    events(&mut app);
    destroy(&mut app, first[0]);
    tick(&mut app, 2);
    // It takes a place that the group had to spare, clear of every tank.
    let news = events(&mut app);
    let marks: Vec<Vec2> = news
        .iter()
        .filter_map(|event| match event {
            SimEvent::TankIncoming { at, .. } => Some(*at),
            _ => None,
        })
        .collect();
    assert_eq!(marks.len(), 1);
    assert!(spare.contains(&marks[0]) && marks[0] != place);
    assert!(marks[0].distance(place + Vec2::X * 0.5) >= APPEAR_APART);
    // The mark tells which tank comes: it was shown in reserve.
    let told = news.iter().find_map(|event| match event {
        SimEvent::TankIncoming { tank, .. } => Some(*tank),
        _ => None,
    });
    assert!(told.unwrap().is_some());
    tick(&mut app, seconds(APPEAR_TIME) + 2);
    let came = events(&mut app).into_iter().find_map(|event| match event {
        SimEvent::TankAppeared { tank, .. } => Some(tank),
        _ => None,
    });
    assert_eq!(came, told);
    let mut wanted = vec![number(&app, squatter), 2];
    wanted.sort();
    assert_eq!(there(&mut app, 12), wanted);
    let third = enemies(&mut app, 12)
        .into_iter()
        .find(|&tank| number(&app, tank) == 2);
    // It appears there, awake: it may have driven for a tick or two since.
    assert!(position(&mut app, third.unwrap()).distance(marks[0]) < 0.3);

    // A group that has no place to spare waits for the tank to move on.
    let mut app = before_group("TEST", 1, 12);
    shelter(&mut app);
    stand_aside(&mut app);
    let mut field = app.world_mut().resource_mut::<Field>();
    let state = field.room.as_mut().unwrap();
    state.spare.clear();
    let place = state.next[1][0].at;
    wait_for_the_group(&mut app);
    tick(&mut app, seconds(APPEAR_TIME) + 2);
    put_to_sleep(&mut app, 12);
    let first = enemies(&mut app, 12);
    let home = position(&mut app, first[1]);
    teleport(&mut app, first[1], place + Vec2::X * 0.5);
    destroy(&mut app, first[0]);
    tick(&mut app, seconds(APPEAR_TIME) + 4);
    assert_eq!(enemies(&mut app, 12).len(), 1);
    assert_eq!(numbers(&mut app), [2, 3]);
    teleport(&mut app, first[1], home);
    tick(&mut app, seconds(APPEAR_TIME) + 4);
    assert_eq!(enemies(&mut app, 12).len(), 2);
    assert_eq!(numbers(&mut app), [3]);
}

#[test]
fn a_tank_whose_place_a_player_stands_on_takes_another_place_of_its_group() {
    let mut app = before_group("TEST", 1, 12);
    shelter(&mut app);
    // The player stands on the place of the first tank of the group, and stays there.
    let (place, spare) = {
        let field = app.world().resource::<Field>();
        let state = field.room.as_ref().unwrap();
        (state.next[0][0].at, state.spare.clone())
    };
    assert!(spare.len() >= 4);
    let own = player(&mut app).unwrap();
    teleport(&mut app, own, place + Vec2::X);
    wait_for_the_group(&mut app);
    tick(&mut app, 1);
    // Both tanks that come first are marked at once all the same: nobody could tell that
    // one of them waited for the player to move.
    let news = events(&mut app);
    let marks: Vec<Vec2> = news
        .iter()
        .filter_map(|event| match event {
            SimEvent::TankIncoming { at, .. } => Some(*at),
            _ => None,
        })
        .collect();
    assert_eq!(marks.len(), 2);
    assert!(!marks.contains(&place));
    // The tank took a place that the group had to spare, with room around it.
    let taken: Vec<&Vec2> = marks.iter().filter(|mark| spare.contains(mark)).collect();
    assert_eq!(taken.len(), 1);
    assert!(taken[0].distance(place + Vec2::X) > APPEAR_ROOM);
    tick(&mut app, seconds(APPEAR_TIME) + 2);
    let there = enemies(&mut app, 12);
    assert_eq!(there.len(), 2);
    for tank in there {
        assert!(position(&mut app, tank).distance(place + Vec2::X) > APPEAR_ROOM);
    }
    // The place that it left is one to spare now.
    let field = app.world().resource::<Field>();
    let state = field.room.as_ref().unwrap();
    assert!(state.spare.contains(&place) && state.spare.len() == spare.len());

    // A group that has no place to spare waits for room, as a wave of an arena does. The
    // tank that waits counts as one of those that the room holds.
    let mut app = before_group("TEST", 1, 12);
    shelter(&mut app);
    let mut field = app.world_mut().resource_mut::<Field>();
    let state = field.room.as_mut().unwrap();
    state.spare.clear();
    let place = state.next[0][0].at;
    let own = player(&mut app).unwrap();
    teleport(&mut app, own, place + Vec2::X);
    wait_for_the_group(&mut app);
    tick(&mut app, seconds(APPEAR_TIME) + 3);
    assert_eq!(enemies(&mut app, 12).len(), 1);
    assert_eq!(numbers(&mut app), [0, 2, 3]);
    teleport(&mut app, own, place + Vec2::X * (APPEAR_ROOM + 1.0));
    tick(&mut app, seconds(APPEAR_TIME) + 3);
    assert_eq!(enemies(&mut app, 12).len(), 2);
}

#[test]
fn a_group_is_the_group_of_the_sector_of_its_number_for_as_many_players() {
    for (group, players, difficulty) in [
        (1, 1, Difficulty::Normal),
        (7, 1, Difficulty::Normal),
        (20, 1, Difficulty::Hard),
        (7, 3, Difficulty::Normal),
        (30, 4, Difficulty::Hard),
    ] {
        let mut app = open_in("BRAVO", players, difficulty);
        app.insert_resource(Playtest::default());
        skip_to(&mut app, group);
        tick(&mut app, 1);
        let count = progression::enemy_count(group, players, false);
        let genomes = enemy::group(seed_value("BRAVO"), difficulty, group, count);
        let waiting: Vec<Genome> = reserves(&mut app)
            .iter()
            .map(|(reserve, _)| reserve.genome)
            .collect();
        assert_eq!(waiting, genomes, "group {group}, {players} players");
        // The tanks of the players stand around the middle of the room.
        let middle = room_of("BRAVO").start().as_vec2();
        for seat_number in 0..players as u8 {
            let tank = seat(&mut app, seat_number);
            assert!(position(&mut app, tank).distance(middle) < 4.0);
            assert_eq!(app.world().get::<Tank>(tank).unwrap().sector, group - 1);
        }
        // A brain is as experienced as its group is far along.
        stand_aside(&mut app);
        let seats: Vec<Entity> = (0..players as u8).map(|s| seat(&mut app, s)).collect();
        for tank in seats {
            app.world_mut().entity_mut(tank).insert(Protected(1.0e9));
        }
        wait_for_the_group(&mut app);
        tick(&mut app, seconds(APPEAR_TIME) + 3);
        let there = enemies(&mut app, group);
        assert!(!there.is_empty());
        let delay = 0.50 - 0.38 * progression::experience(group, difficulty);
        for tank in there {
            let brain = app.world().get::<Brain>(tank).unwrap();
            assert!(
                (brain.reaction_delay() - delay).abs() < 1e-6,
                "group {group}"
            );
        }
    }
}

#[test]
fn the_room_is_laid_out_anew_between_two_groups() {
    let mut app = open("TEST");
    shelter(&mut app);
    stand_aside(&mut app);
    let room = room_of("TEST");
    let (first_blocks, _) = room.obstacles(1);
    wait_for_the_group(&mut app);
    tick(&mut app, seconds(APPEAR_TIME) + 2);
    assert_eq!(standing(&mut app).0, centers(&first_blocks));

    // When the group is cleared, what stood in the room goes, and what the next group is
    // fought among is planned: its places are inked in, and nothing is solid there yet.
    clear_sector(&mut app, 1);
    tick(&mut app, 2);
    let (blocks, crates) = room.obstacles(2);
    assert_eq!(standing(&mut app), (Vec::new(), Vec::new()));
    let marks = planned(&mut app);
    assert_eq!(marks.len(), blocks.len() + crates.len());
    let places = |explosive: bool| -> Vec<Vec2> {
        let kind = marks.iter().filter(|(_, mark)| mark.explosive == explosive);
        kind.map(|(at, _)| *at).collect()
    };
    assert_eq!(places(false), centers(&blocks));
    assert_eq!(places(true), centers(&crates));
    let solid = |app: &App| {
        let colliders = &app.world().resource::<Field>().colliders;
        let obstacle = |solid: &Solid| matches!(solid, Solid::Block(_) | Solid::Crate(_));
        colliders.iter().filter(|c| obstacle(&c.solid)).count()
    };
    assert_eq!(solid(&app), 0);
    // The tanks of the next group will appear among what is laid out for them.
    let field = app.world().resource::<Field>();
    let state = field.room.as_ref().unwrap();
    assert_eq!((&state.blocks, &state.crates), (&blocks, &crates));
    let wanted = room.places(2, &blocks, &crates);
    let next: Vec<Vec2> = state.next.iter().flatten().map(|tank| tank.at).collect();
    assert_eq!(next, [wanted[0].as_vec2()]);

    // A tank stands on one of the places, and a bullet flies over another.
    let own = player(&mut app).unwrap();
    let (held, shot_over) = (centers(&blocks)[0], centers(&blocks)[1]);
    teleport(&mut app, own, held + Vec2::new(0.4, -0.3));
    let bullet = spawn_bullet(&mut app, Team::Enemies, shot_over, Vec2::X * 0.01, 0);
    events(&mut app);
    tick(&mut app, seconds(RISE_TIME) - 6);
    assert_eq!(standing(&mut app), (Vec::new(), Vec::new()));
    // Everything stands when its time has come, but what has a tank in its place: that waits.
    tick(&mut app, 8);
    let (stood, stood_crates) = standing(&mut app);
    assert_eq!(stood.len(), blocks.len() - 1);
    assert!(!stood.contains(&held) && stood.contains(&shot_over));
    assert_eq!(stood_crates, centers(&crates));
    assert_eq!(planned(&mut app).len(), 1);
    assert_eq!(position(&mut app, own), held + Vec2::new(0.4, -0.3));
    // The bullet that flew where a block now stands ended there.
    assert!(!alive(&app, bullet));
    let ended = |event: &SimEvent| matches!(event, SimEvent::BulletDestroyed { .. });
    assert_eq!(told(&mut app, ended), 1);
    tick(&mut app, 1);
    assert_eq!(solid(&app), 4 * (blocks.len() - 1 + crates.len()));
    // It stands as soon as the tank has left.
    tick(&mut app, seconds(1.0));
    assert_eq!(planned(&mut app).len(), 1);
    teleport(&mut app, own, room.start().as_vec2());
    tick(&mut app, 2);
    assert_eq!(standing(&mut app).0, centers(&blocks));
    assert!(planned(&mut app).is_empty());
}

#[test]
fn a_single_player_gets_a_life_back_every_few_groups() {
    let gained = |event: &SimEvent| matches!(event, SimEvent::LifeTaken { .. });
    let clear = |app: &mut App, group: u32| {
        wait_for_the_group(app);
        events(app);
        clear_sector(app, group);
        tick(app, 1);
    };
    // A player who is short of a life gets one back with the fifth group.
    let mut app = before_group("TEST", 1, LIFE_GROUPS);
    app.world_mut().resource_mut::<Run>().lives = SOLO_LIVES - 2;
    clear(&mut app, LIFE_GROUPS);
    assert_eq!(run(&app).lives, SOLO_LIVES - 1);
    assert_eq!(told(&mut app, gained), 1);
    // The groups between give none.
    clear(&mut app, LIFE_GROUPS + 1);
    assert_eq!(run(&app).lives, SOLO_LIVES - 1);
    assert_eq!(told(&mut app, gained), 0);
    // Nobody has more lives than a run starts with.
    let mut app = before_group("TEST", 1, 2 * LIFE_GROUPS);
    clear(&mut app, 2 * LIFE_GROUPS);
    assert_eq!(run(&app).lives, SOLO_LIVES);
    assert_eq!(told(&mut app, gained), 0);
    // Several players have no lives: they repair each other.
    let mut app = before_group("TEST", 2, LIFE_GROUPS);
    app.world_mut().resource_mut::<Run>().lives = SOLO_LIVES - 1;
    clear(&mut app, LIFE_GROUPS);
    assert_eq!(run(&app).cleared, LIFE_GROUPS);
    assert_eq!(run(&app).lives, SOLO_LIVES - 1);
    assert_eq!(told(&mut app, gained), 0);
}

#[test]
fn a_single_player_who_falls_comes_back_in_the_middle_of_the_room_to_the_same_group() {
    let mut app = before_group("TEST", 1, 12);
    stand_aside(&mut app);
    wait_for_the_group(&mut app);
    tick(&mut app, seconds(APPEAR_TIME) + 2);
    let first = enemies(&mut app, 12);
    assert_eq!(first.len(), 2);
    let kept = app.world().get::<Tank>(first[1]).unwrap().genome;
    let own = player(&mut app).unwrap();
    let at = position(&mut app, own);
    spawn_bullet(&mut app, Team::Enemies, at + Vec2::X * 8.0, Vec2::X, 0);
    let (blocks_before, kills) = (standing(&mut app), run(&app).kills);

    // The player is destroyed with one of the two tanks, and comes back after a moment, a
    // life short.
    app.world_mut()
        .resource_mut::<Casualties>()
        .tanks
        .push(first[0]);
    destroy_player(&mut app);
    assert!(matches!(run(&app).phase, RunPhase::Respawning(_)));
    // Nobody comes for the tank that fell while the player is away.
    events(&mut app);
    tick(&mut app, seconds(RESPAWN_DELAY) - 2);
    assert_eq!(told(&mut app, incoming), 0);
    assert_eq!(numbers(&mut app), [2, 3]);
    assert_eq!(enemies(&mut app, 12).len(), 1);
    tick(&mut app, 3);
    let state = run(&app);
    assert_eq!(
        (state.phase, state.lives),
        (RunPhase::Playing, SOLO_LIVES - 1)
    );
    // The run is where it was: the same group is on, and what was destroyed stays so.
    assert_eq!(
        (state.sector, state.cleared, state.kills),
        (12, 11, kills + 1)
    );
    assert_eq!(state.lull, None);
    let own = player(&mut app).unwrap();
    assert_eq!(position(&mut app, own), room_of("TEST").start().as_vec2());
    assert_eq!(app.world().get::<Tank>(own).unwrap().sector, 12);
    assert_eq!(standing(&mut app), blocks_before);
    assert_eq!(bullets(&mut app), 0);
    // The tank that was left went back into reserve, with the two that had not come.
    assert!(enemies(&mut app, 12).is_empty());
    tick(&mut app, 1);
    let waiting = reserves(&mut app);
    assert_eq!(waiting.len(), 3);
    assert!(waiting.iter().any(|(reserve, _)| reserve.genome == kept));

    // The group comes again as it did the first time: after a moment, announced, and as
    // many tanks at once, each after its mark. The tank that was there is the first of them.
    stand_aside_of_the_wave(&mut app, 12);
    events(&mut app);
    tick(&mut app, seconds(ARENA_DELAY) - 4);
    assert_eq!(told(&mut app, incoming), 0);
    tick(&mut app, 8);
    let news = events(&mut app);
    assert_eq!(count(&news, announced), 1);
    assert_eq!(count(&news, incoming), 2);
    tick(&mut app, seconds(APPEAR_TIME) + 2);
    let again = enemies(&mut app, 12);
    assert_eq!(again.len(), 2);
    let genome = |app: &App, tank: Entity| app.world().get::<Tank>(tank).unwrap().genome;
    assert!(again.iter().any(|&tank| genome(&app, tank) == kept));
    assert_eq!(numbers(&mut app).len(), 1);
    // Then the last one, and the group is cleared as if nothing had happened.
    destroy_enemies(&mut app, 12);
    tick(&mut app, seconds(APPEAR_TIME) + 3);
    assert_eq!(enemies(&mut app, 12).len(), 1);
    destroy_enemies(&mut app, 12);
    tick(&mut app, 1);
    assert_eq!(run(&app).cleared, 12);
}

/// Moves the player out of the way of the tanks that the group that is on still holds back.
fn stand_aside_of_the_wave(app: &mut App, group: u32) {
    let aside = {
        let field = app.world().resource::<Field>();
        let state = field.room.as_ref().unwrap();
        let held = field.arenas[&group].waiting.iter().flatten();
        let places: Vec<Vec2> = held.map(|tank| tank.at).collect();
        let spots = state.room.places(1000, &state.blocks, &state.crates);
        let clear = |spot: &&IVec2| {
            let far = |place: &Vec2| place.distance(spot.as_vec2()) > APPEAR_ROOM + 3.0;
            places.iter().all(far)
        };
        spots.iter().find(clear).unwrap().as_vec2()
    };
    let own = player(app).unwrap();
    teleport(app, own, aside);
}

#[test]
fn the_lull_waits_for_a_player_who_fell_and_the_last_life_ends_the_run() {
    let mut app = before_group("TEST", 1, 3);
    // The player falls a second before the group is due. It waits for them.
    tick(&mut app, seconds(LULL - 1.0));
    assert_eq!(run(&app).lull, Some(1));
    destroy_player(&mut app);
    assert_eq!(run(&app).lives, SOLO_LIVES - 1);
    tick(&mut app, seconds(RESPAWN_DELAY) + 1);
    let state = run(&app);
    assert_eq!((state.phase, state.sector), (RunPhase::Playing, 2));
    assert_eq!(state.lull, Some(1));
    assert!(reserves(&mut app).iter().all(|(tank, _)| tank.group == 3));
    stand_aside(&mut app);
    tick(&mut app, seconds(1.0));
    assert_eq!((run(&app).sector, run(&app).lull), (3, None));
    // The last life ends the run, with the groups that were cleared as its score.
    for lives in (0..SOLO_LIVES - 1).rev() {
        tick(&mut app, seconds(RESPAWN_DELAY) + 1);
        destroy_player(&mut app);
        assert_eq!(run(&app).lives, lives);
    }
    let state = run(&app);
    assert_eq!((state.phase, state.cleared), (RunPhase::Over, 2));
}

#[test]
fn several_players_fight_the_group_that_they_call_for_and_repair_each_other() {
    let mut app = open_in("TEST", 3, Difficulty::Normal);
    // The group of the first sector has two tanks for three players.
    assert_eq!(reserves(&mut app).len(), 2);
    let state = run(&app);
    assert!(state.arena && !state.solo);
    let (first, second, third) = (seat(&mut app, 0), seat(&mut app, 1), seat(&mut app, 2));
    // A tank that is destroyed is a wreck, and the run goes on.
    destroy(&mut app, second);
    tick(&mut app, 1);
    assert!(is_wreck(&app, second));
    assert_eq!(run(&app).phase, RunPhase::Playing);
    // The lull doesn't wait for a wreck.
    stand_aside(&mut app);
    wait_for_the_group(&mut app);
    assert_eq!(run(&app).sector, 1);
    // Every tank is of the group that is on, a wreck too.
    for tank in [first, second, third] {
        assert_eq!(app.world().get::<Tank>(tank).unwrap().sector, 1);
    }
    // The run is over when every tank is a wreck.
    destroy(&mut app, first);
    destroy(&mut app, third);
    tick(&mut app, 1);
    assert_eq!(run(&app).phase, RunPhase::Over);
}

#[test]
fn a_playtest_skips_to_the_lull_before_a_group() {
    let mut app = before_group("TEST", 1, 12);
    let state = run(&app);
    assert_eq!(
        (state.sector, state.cleared, state.checkpoint),
        (11, 11, 11)
    );
    assert_eq!(
        (state.phase, state.lull),
        (RunPhase::Playing, Some(LULL as u8))
    );
    // The room is as that group finds it, and the group is in reserve.
    let room = room_of("TEST");
    let (blocks, crates) = room.obstacles(12);
    assert_eq!(standing(&mut app), (centers(&blocks), centers(&crates)));
    assert!(planned(&mut app).is_empty());
    let waiting = reserves(&mut app);
    assert_eq!(waiting.len() as u32, progression::enemy_count(12, 1, false));
    assert!(waiting.iter().all(|(reserve, _)| reserve.group == 12));
    assert_eq!(players(&mut app), 1);
    // Skipping back works the same way, and leaves one room and one group.
    skip_to(&mut app, 3);
    tick(&mut app, 1);
    let state = run(&app);
    assert_eq!((state.sector, state.checkpoint), (2, 2));
    assert_eq!(state.cleared, 11);
    let world = app.world_mut();
    assert_eq!(world.query::<&Sector>().iter(world).count(), 1);
    let waiting = reserves(&mut app);
    assert_eq!(waiting.len() as u32, progression::enemy_count(3, 1, false));
    assert!(waiting.iter().all(|(reserve, _)| reserve.group == 3));
    let (blocks, crates) = room.obstacles(3);
    assert_eq!(standing(&mut app), (centers(&blocks), centers(&crates)));
    stand_aside(&mut app);
    shelter(&mut app);
    wait_for_the_group(&mut app);
    assert_eq!(run(&app).sector, 3);
}

#[test]
fn a_run_in_arena_mode_goes_through_many_groups_and_keeps_one_room() {
    let mut app = open("ALPHA");
    shelter(&mut app);
    for group in 1..=12 {
        stand_aside(&mut app);
        wait_for_the_group(&mut app);
        assert_eq!(run(&app).sector, group);
        tick(&mut app, seconds(APPEAR_TIME) + 2);
        assert!(!enemies(&mut app, group).is_empty(), "group {group}");
        clear_sector(&mut app, group);
        tick(&mut app, 1);
        assert_eq!(run(&app).cleared, group);
    }
    let world = app.world_mut();
    assert_eq!(world.query::<&Sector>().iter(world).count(), 1);
    assert_eq!(world.query::<&Barrier>().iter(world).count(), 0);
    // Nothing of the groups that are over is left: no tank, and none in reserve but the
    // next group.
    let mut foes = world.query_filtered::<&Tank, With<Enemy>>();
    assert_eq!(foes.iter(world).count(), 0);
    let waiting = reserves(&mut app);
    assert!(waiting.iter().all(|(reserve, _)| reserve.group == 13));
    assert_eq!(waiting.len() as u32, progression::enemy_count(13, 1, false));
}

/// Plays a run in arena mode with a crude bot: it drives in circles and shoots at what it
/// sees, and every few seconds the test destroys what is left of the group.
fn play(seed: &str, groups: u32) -> App {
    let mut app = open(seed);
    let mut step = 0u32;
    while run(&app).cleared < groups && run(&app).phase != RunPhase::Over {
        step += 1;
        let state = run(&app);
        if let Some(entity) = player(&mut app) {
            let at = position(&mut app, entity);
            let target = enemies(&mut app, state.sector)
                .first()
                .map_or(at + Vec2::Y, |&enemy| position(&mut app, enemy));
            command(
                &mut app,
                TankCommand {
                    drive: Vec2::from_angle(step as f32 * 0.05),
                    aim: target,
                    fire: true,
                    mine: step.is_multiple_of(97),
                    shield: step.is_multiple_of(301),
                    repair: false,
                },
            );
        }
        tick(&mut app, 1);
        let world = app.world_mut();
        for position in world.query::<&Position>().iter(world) {
            assert!(
                position.0.is_finite(),
                "seed {seed}: something left the map"
            );
        }
        if step.is_multiple_of(seconds(4.0)) && state.lull.is_none() {
            clear_sector(&mut app, state.sector);
        }
    }
    app
}

#[test]
fn the_same_seed_and_the_same_inputs_give_the_same_run_in_arena_mode() {
    let mut first = play("DELTA", 6);
    let mut second = play("DELTA", 6);
    assert_eq!(run(&first).ticks, run(&second).ticks);
    assert_eq!(fingerprint(&mut first), fingerprint(&mut second));
    let mut other = play("ECHO", 6);
    assert_ne!(fingerprint(&mut first), fingerprint(&mut other));
    // The tanks of the bot stay in the room, whatever happens.
    let room = room_of("DELTA");
    let (min, max) = (room.plan().min.as_vec2(), room.plan().max.as_vec2());
    if let Some(tank) = player(&mut first) {
        let at = position(&mut first, tank);
        assert!(at.cmpgt(min).all() && at.cmplt(max).all(), "{at}");
    }
}
