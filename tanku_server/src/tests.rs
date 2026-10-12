//! Tests of the rules, played on the real simulation without a window.

use bevy::{ecs::message::Messages, prelude::*};
use tanku_core::{
    CorePlugin,
    map::SectorKind,
    modes::Difficulty,
    progression,
    rng::{Purpose, Stream},
    rules::{
        APPEAR_ROOM, APPEAR_TIME, ARENA_DELAY, ARENA_WAVES, BARRIER_GATHER_DISTANCE, LIFE_REACH,
        PLAYTEST_LAST_SECTOR, REPAIR_PROTECTION, REPAIR_RADIUS, REPAIR_TIME, RESPAWN_DELAY,
        SHIELD_DURATION, SOLO_LIVES, SPAWN_FIRE_DELAY, TANK_PUSH, TANK_RADIUS, TICK_RATE,
    },
    sim::{
        Barrier, Block, Blow, Bullet, Enemy, ExplosiveCrate, Gun, Life, Mine, MineState, Motion,
        Player, Playtest, Position, PreviousPosition, Protected, Run, RunPhase, RunRequest, Sector,
        Shield, SimEvent, Tank, TankCommand, Team, Weapon, Wreck,
    },
    stats::{Genome, Stat},
};

use crate::{
    Casualties, Maker, ServerPlugin,
    brain::Brain,
    field::{Field, Fuse, Home, InSector},
};

mod trials;

/// Where a run starts: the start bay spans x from -7 to 7 and y from 0 to 16.
const START: Vec2 = Vec2::new(0.0, 5.0);

fn seconds(seconds: f32) -> u32 {
    (seconds * TICK_RATE as f32).ceil() as u32
}

fn tick(app: &mut App, ticks: u32) {
    for _ in 0..ticks {
        app.world_mut().run_schedule(FixedUpdate);
    }
}

/// Starts a run of this many players. The tanks have just appeared and can't shoot yet.
fn begin_with(seed: &str, players: u32) -> App {
    begin_in(seed, players, Difficulty::Normal)
}

/// Starts a run of this many players and of this difficulty.
fn begin_in(seed: &str, players: u32, difficulty: Difficulty) -> App {
    begin_as(seed, players, difficulty, Vec::new())
}

/// Starts a run in free mode in which the first seats have tanks of their own, or a run in
/// the standard tank if none is given.
fn begin_as(seed: &str, players: u32, difficulty: Difficulty, tanks: Vec<Genome>) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
    app.world_mut().write_message(RunRequest::Start {
        seed: seed.into(),
        players,
        drawn: true,
        difficulty,
        free: !tanks.is_empty(),
        tanks,
    });
    tick(&mut app, 1);
    events(&mut app);
    app
}

/// Starts the run of a single player. The tank has just appeared and can't shoot yet.
fn begin(seed: &str) -> App {
    begin_with(seed, 1)
}

/// Starts a run of this many players and waits until the tanks may shoot.
fn team(seed: &str, players: u32) -> App {
    let mut app = begin_with(seed, players);
    tick(&mut app, seconds(SPAWN_FIRE_DELAY));
    app
}

/// Starts a run and waits until the tank may shoot.
fn start(seed: &str) -> App {
    let mut app = begin(seed);
    tick(&mut app, seconds(SPAWN_FIRE_DELAY));
    app
}

fn events(app: &mut App) -> Vec<SimEvent> {
    app.world_mut()
        .resource_mut::<Messages<SimEvent>>()
        .drain()
        .collect()
}

fn count(events: &[SimEvent], wanted: impl Fn(&SimEvent) -> bool) -> usize {
    events.iter().filter(|event| wanted(event)).count()
}

fn run(app: &App) -> Run {
    app.world().resource::<Run>().clone()
}

fn player(app: &mut App) -> Option<Entity> {
    let world = app.world_mut();
    world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
}

fn position(app: &mut App, entity: Entity) -> Vec2 {
    app.world().get::<Position>(entity).unwrap().0
}

fn teleport(app: &mut App, entity: Entity, to: Vec2) {
    app.world_mut().get_mut::<Position>(entity).unwrap().0 = to;
}

fn command(app: &mut App, command: TankCommand) {
    let entity = player(app).unwrap();
    *app.world_mut().get_mut::<TankCommand>(entity).unwrap() = command;
}

/// Turns the turret toward a point, then fires one round at it.
fn shoot_at(app: &mut App, aim: Vec2) {
    command(app, TankCommand { aim, ..default() });
    tick(app, seconds(0.3));
    command(
        app,
        TankCommand {
            aim,
            fire: true,
            ..default()
        },
    );
    tick(app, 1);
    command(app, TankCommand { aim, ..default() });
}

fn enemies(app: &mut App, sector: u32) -> Vec<Entity> {
    let world = app.world_mut();
    world
        .query_filtered::<(Entity, &Tank), With<Enemy>>()
        .iter(world)
        .filter(|(_, tank)| tank.sector == sector)
        .map(|(entity, _)| entity)
        .collect()
}

fn alive(app: &App, entity: Entity) -> bool {
    app.world().get_entity(entity).is_ok()
}

fn bullets(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query::<&Bullet>().iter(world).count()
}

fn spawn_bullet(app: &mut App, team: Team, at: Vec2, velocity: Vec2, pierces: u8) -> Entity {
    app.world_mut()
        .spawn((
            Bullet {
                owner: Entity::PLACEHOLDER,
                team,
                sector: 0,
                velocity,
                bounces_left: 0,
                pierces_left: pierces,
                bounced: false,
            },
            Position(at),
            PreviousPosition(at),
            InSector(0),
        ))
        .id()
}

fn spawn_enemy(app: &mut App, at: Vec2, levels: &[(Stat, u8)]) -> Entity {
    let mut genome = Genome::empty(100);
    for &(stat, level) in levels {
        genome.set(stat, level);
    }
    app.world_mut()
        .spawn((
            Tank {
                genome,
                team: Team::Enemies,
                sector: 0,
            },
            Enemy { awake: false },
            Position(at),
            PreviousPosition(at),
            Gun::loaded(&genome),
            InSector(0),
        ))
        .id()
}

/// An enemy that is awake and thinks with the experience of a late sector.
fn spawn_awake(app: &mut App, at: Vec2, levels: &[(Stat, u8)]) -> Entity {
    spawn_thinker(app, at, levels, 60)
}

/// An enemy that is awake and thinks with the experience of a sector.
fn spawn_thinker(app: &mut App, at: Vec2, levels: &[(Stat, u8)], sector: u32) -> Entity {
    let enemy = spawn_enemy(app, at, levels);
    let stream = Stream::new(1, Purpose::Brain, 0, 0);
    let brain = Brain::new(stream, sector, Difficulty::Normal);
    app.world_mut()
        .entity_mut(enemy)
        .insert((brain, Enemy { awake: true }));
    enemy
}

/// Destroys the enemies that are in a sector, as a player would.
fn destroy_enemies(app: &mut App, sector: u32) {
    let destroyed = enemies(app, sector);
    app.world_mut()
        .resource_mut::<Casualties>()
        .tanks
        .extend(destroyed);
    tick(app, 1);
}

/// Destroys every enemy of a sector, those that an arena still holds back included.
fn clear_sector(app: &mut App, sector: u32) {
    if let Some(arena) = app
        .world_mut()
        .resource_mut::<Field>()
        .arenas
        .get_mut(&sector)
    {
        arena.waiting.clear();
        arena.incoming.clear();
    }
    destroy_enemies(app, sector);
}

/// Carries the player through the barrier of the sector it is in.
fn cross_barrier(app: &mut App) {
    let sector = run(app).sector;
    let world = app.world_mut();
    let barrier = *world
        .query::<&Barrier>()
        .iter(world)
        .find(|barrier| barrier.sector == sector)
        .unwrap();
    let middle = (barrier.from + barrier.to) / 2.0;
    let entity = player(app).unwrap();
    // Close to the barrier.
    teleport(app, entity, middle - barrier.outward);
    tick(app, 1);
    teleport(app, entity, middle + barrier.outward * 2.0);
    tick(app, 1);
}

#[test]
fn a_run_starts_in_the_bay_with_the_next_sector_dormant() {
    let mut app = start("TEST");
    let state = run(&app);
    assert_eq!(
        (state.sector, state.lives, state.phase),
        (0, 3, RunPhase::Playing)
    );
    let entity = player(&mut app).unwrap();
    assert_eq!(position(&mut app, entity), START);

    let world = app.world_mut();
    let mut loaded: Vec<u32> = world
        .query::<&Sector>()
        .iter(world)
        .map(|sector| sector.layout.plan.index)
        .collect();
    loaded.sort();
    assert_eq!(loaded, [0, 1]);
    let dormant = world
        .query::<&Enemy>()
        .iter(world)
        .all(|enemy| !enemy.awake);
    assert!(dormant);
    assert_eq!(enemies(&mut app, 1).len(), 1);
}

#[test]
fn a_bullet_bounces_as_often_as_its_ricochet_stat_says() {
    let mut app = start("TEST");
    // Down and to the east: off the east wall, then into the south wall.
    shoot_at(&mut app, START + Vec2::new(7.0, -3.0));
    assert_eq!(bullets(&mut app), 1);
    tick(&mut app, seconds(2.0));
    let events = events(&mut app);
    assert_eq!(
        count(&events, |event| matches!(event, SimEvent::Shot { .. })),
        1
    );
    assert_eq!(
        count(&events, |event| matches!(event, SimEvent::Ricochet { .. })),
        1
    );
    assert_eq!(
        count(&events, |event| matches!(
            event,
            SimEvent::BulletDestroyed { .. }
        )),
        1
    );
    assert_eq!(bullets(&mut app), 0);
    assert_eq!(run(&app).lives, 3);
}

#[test]
fn a_ricochet_can_destroy_the_player_who_fired() {
    let mut app = start("TEST");
    shoot_at(&mut app, START + Vec2::X);
    tick(&mut app, seconds(2.0));
    let state = run(&app);
    assert_eq!(state.lives, 2);
    assert!(matches!(state.phase, RunPhase::Respawning(_)));
    assert!(player(&mut app).is_none());

    tick(&mut app, seconds(RESPAWN_DELAY));
    assert_eq!(run(&app).phase, RunPhase::Playing);
    let entity = player(&mut app).unwrap();
    assert_eq!(position(&mut app, entity), START);
}

#[test]
fn a_bullet_ends_on_what_blocks_the_barrel_and_bounces_off_what_is_further() {
    // The east wall of the bay is at x = 7. From 0.5 m off it, the tip of the barrel is in
    // the wall: the bullet ends there, though the tank's bullets bounce once.
    let mut app = start("TEST");
    let tank = player(&mut app).unwrap();
    teleport(&mut app, tank, Vec2::new(5.6, 5.0));
    shoot_at(&mut app, Vec2::new(7.0, 5.0));
    let told = events(&mut app);
    assert_eq!(bullets(&mut app), 0);
    let shots = count(&told, |event| matches!(event, SimEvent::Shot { .. }));
    let bounces = count(&told, |event| matches!(event, SimEvent::Ricochet { .. }));
    let ended = count(&told, |event| {
        matches!(event, SimEvent::BulletDestroyed { .. })
    });
    assert_eq!((shots, bounces, ended), (1, 0, 1));
    // The round is spent all the same, and the tank is unharmed.
    assert_eq!(app.world().get::<Gun>(tank).unwrap().rounds, 4);
    tick(&mut app, seconds(1.0));
    assert_eq!(run(&app).lives, 3);
    assert!(alive(&app, tank));

    // From a little further, the bullet is out of the barrel when it meets the wall: it
    // bounces, and comes back at the tank that fired it.
    let mut app = start("TEST");
    let tank = player(&mut app).unwrap();
    teleport(&mut app, tank, Vec2::new(5.0, 5.0));
    shoot_at(&mut app, Vec2::new(7.0, 5.0));
    assert_eq!(bullets(&mut app), 1);
    tick(&mut app, seconds(1.0));
    let told = events(&mut app);
    let bounces = count(&told, |event| matches!(event, SimEvent::Ricochet { .. }));
    assert_eq!(bounces, 1);
    assert_eq!(run(&app).lives, 2);
}

#[test]
fn a_block_that_blocks_the_barrel_stops_the_bullet_and_stays() {
    let mut app = start("TEST");
    // The near side of the block is 0.3 m in front of the tank.
    let block_at = START + Vec2::new(2.2, 0.0);
    let block = app
        .world_mut()
        .spawn((Block { sector: 0 }, Position(block_at), InSector(0)))
        .id();
    app.world_mut().resource_mut::<Field>().stale = true;
    shoot_at(&mut app, block_at);
    let told = events(&mut app);
    assert_eq!(bullets(&mut app), 0);
    let bounces = count(&told, |event| matches!(event, SimEvent::Ricochet { .. }));
    assert_eq!(bounces, 0);
    tick(&mut app, seconds(1.0));
    assert!(alive(&app, block));
    assert_eq!(run(&app).lives, 3);
}

#[test]
fn the_magazine_empties_and_reloads_one_round_at_a_time() {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    let aim = START + Vec2::new(7.0, -3.0);
    command(&mut app, TankCommand { aim, ..default() });
    tick(&mut app, seconds(0.3));
    command(
        &mut app,
        TankCommand {
            aim,
            fire: true,
            ..default()
        },
    );
    // Five rounds leave at 0.25 s intervals: at 0, 0.25, 0.5, 0.75 and 1 s.
    tick(&mut app, seconds(0.9));
    assert_eq!(
        count(&events(&mut app), |event| matches!(
            event,
            SimEvent::Shot { .. }
        )),
        4
    );
    tick(&mut app, seconds(0.2));
    assert_eq!(
        count(&events(&mut app), |event| matches!(
            event,
            SimEvent::Shot { .. }
        )),
        1
    );
    // The loader has been working since the first shot and delivers a round every second:
    // the next shots leave at 1.25, 2, 3 and 4 s.
    tick(&mut app, seconds(3.0));
    assert_eq!(
        count(&events(&mut app), |event| matches!(
            event,
            SimEvent::Shot { .. }
        )),
        4
    );
    command(&mut app, TankCommand::default());
    tick(&mut app, seconds(5.5));
    assert_eq!(app.world().get::<Gun>(entity).unwrap().rounds, 5);
}

#[test]
fn bullets_destroy_each_other_unless_one_pierces() {
    let head_on = |first: u8, second: u8| {
        let mut app = start("TEST");
        let a = spawn_bullet(
            &mut app,
            Team::Players,
            Vec2::new(-3.0, 12.0),
            Vec2::X * 10.0,
            first,
        );
        let b = spawn_bullet(
            &mut app,
            Team::Enemies,
            Vec2::new(3.0, 12.0),
            Vec2::NEG_X * 10.0,
            second,
        );
        tick(&mut app, seconds(0.5));
        (alive(&app, a), alive(&app, b))
    };
    assert_eq!(head_on(0, 0), (false, false));
    assert_eq!(head_on(1, 0), (true, false));
    assert_eq!(head_on(0, 1), (false, true));
    assert_eq!(head_on(1, 1), (false, false));
}

#[test]
fn a_piercing_bullet_pierces_one_bullet_only() {
    let mut app = start("TEST");
    let dart = spawn_bullet(
        &mut app,
        Team::Enemies,
        Vec2::new(-5.0, 12.0),
        Vec2::X * 10.0,
        1,
    );
    let mut answer = |x: f32| {
        spawn_bullet(
            &mut app,
            Team::Players,
            Vec2::new(x, 12.0),
            Vec2::NEG_X * 10.0,
            0,
        )
    };
    let (first, second) = (answer(-1.0), answer(3.0));
    events(&mut app);
    // The bullets that were destroyed, by the team that fired them.
    let lost = |app: &mut App| {
        let events = events(app);
        [Team::Players, Team::Enemies].map(|team| {
            count(&events, |event| {
                matches!(event, SimEvent::BulletDestroyed { team: whose, .. } if *whose == team)
            })
        })
    };

    // The first bullet is destroyed and blunts the dart, which flies on as a normal bullet.
    tick(&mut app, seconds(0.25));
    assert!(alive(&app, dart) && !alive(&app, first) && alive(&app, second));
    assert_eq!(app.world().get::<Bullet>(dart).unwrap().pierces_left, 0);
    assert_eq!(lost(&mut app), [1, 0]);
    // The second one destroys it.
    tick(&mut app, seconds(0.25));
    assert!(!alive(&app, dart) && !alive(&app, second));
    assert_eq!(lost(&mut app), [1, 1]);
}

#[test]
fn enemy_bullets_destroy_each_other_too() {
    let mut app = start("TEST");
    let a = spawn_bullet(
        &mut app,
        Team::Enemies,
        Vec2::new(-3.0, 12.0),
        Vec2::X * 10.0,
        0,
    );
    let b = spawn_bullet(
        &mut app,
        Team::Enemies,
        Vec2::new(3.0, 12.0),
        Vec2::NEG_X * 10.0,
        0,
    );
    tick(&mut app, seconds(0.5));
    assert!(!alive(&app, a) && !alive(&app, b));
}

#[test]
fn an_enemy_bullet_passes_through_enemies_and_destroys_the_player() {
    let mut app = start("TEST");
    let enemy = spawn_enemy(&mut app, Vec2::new(3.0, 5.0), &[]);
    spawn_bullet(
        &mut app,
        Team::Enemies,
        Vec2::new(6.0, 5.0),
        Vec2::NEG_X * 10.0,
        0,
    );
    tick(&mut app, seconds(1.0));
    assert!(alive(&app, enemy));
    assert_eq!(run(&app).lives, 2);
}

#[test]
fn a_player_bullet_destroys_an_enemy() {
    let mut app = start("TEST");
    let enemy = spawn_enemy(&mut app, Vec2::new(0.0, 12.0), &[]);
    shoot_at(&mut app, Vec2::new(0.0, 12.0));
    tick(&mut app, seconds(1.0));
    assert!(!alive(&app, enemy));
    assert_eq!(run(&app).kills, 1);
}

#[test]
fn a_shield_stops_bullets_and_costs_only_its_cooldown() {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    spawn_bullet(
        &mut app,
        Team::Enemies,
        START + Vec2::X * 3.0,
        Vec2::NEG_X * 10.0,
        1,
    );
    command(
        &mut app,
        TankCommand {
            shield: true,
            ..default()
        },
    );
    tick(&mut app, seconds(SHIELD_DURATION) + 2);
    assert_eq!(run(&app).lives, 3);
    assert_eq!(bullets(&mut app), 0);
    assert!(!app.world().get::<Shield>(entity).unwrap().is_active());
    // The game told of the bullet, and of the bubble going up and coming down, once each,
    // where the tank is.
    let told = events(&mut app);
    let raised = SimEvent::ShieldRaised { at: START };
    let dropped = SimEvent::ShieldDropped { at: START };
    assert_eq!(count(&told, |event| *event == raised), 1);
    assert_eq!(count(&told, |event| *event == dropped), 1);
    assert_eq!(
        count(&told, |event| matches!(
            event,
            SimEvent::ShieldBlocked { .. }
        )),
        1
    );

    // The tank is as free as before: it drives off and fires at once.
    command(
        &mut app,
        TankCommand {
            drive: Vec2::Y,
            aim: START + Vec2::Y * 9.0,
            fire: true,
            ..default()
        },
    );
    tick(&mut app, seconds(0.4));
    assert!(position(&mut app, entity).y > START.y + 1.0);
    let shots = count(&events(&mut app), |event| {
        matches!(event, SimEvent::Shot { .. })
    });
    assert!(shots >= 1);

    // The shield stays down for 9 s after it dropped.
    command(
        &mut app,
        TankCommand {
            shield: true,
            ..default()
        },
    );
    tick(&mut app, seconds(8.0));
    assert!(!app.world().get::<Shield>(entity).unwrap().is_active());
    tick(&mut app, seconds(0.7));
    assert!(app.world().get::<Shield>(entity).unwrap().is_active());
}

#[test]
fn a_shield_lets_its_own_shots_out_but_not_back_in() {
    let mut app = start("TEST");
    let raise = |app: &mut App, aim: Vec2| {
        command(
            app,
            TankCommand {
                aim,
                shield: true,
                ..default()
            },
        );
    };
    // A shot fired from under the bubble flies off. This one ends in the barrier.
    raise(&mut app, START + Vec2::Y);
    tick(&mut app, 2);
    shoot_at(&mut app, START + Vec2::Y);
    assert_eq!(bullets(&mut app), 1);
    tick(&mut app, seconds(10.0));
    events(&mut app);

    // A shot at the east wall comes back about 1.2 s later, and the bubble stops it.
    shoot_at(&mut app, START + Vec2::X);
    tick(&mut app, seconds(0.9));
    raise(&mut app, START + Vec2::X);
    tick(&mut app, seconds(0.6));
    assert_eq!(run(&app).lives, 3);
    let blocked = count(&events(&mut app), |event| {
        matches!(event, SimEvent::ShieldBlocked { .. })
    });
    assert_eq!(blocked, 1);
    assert_eq!(bullets(&mut app), 0);
}

#[test]
fn a_mine_arms_and_blows_when_its_fuse_runs_out() {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    let block = app
        .world_mut()
        .spawn((
            Block { sector: 0 },
            Position(START + Vec2::new(2.5, 0.0)),
            InSector(0),
        ))
        .id();
    command(
        &mut app,
        TankCommand {
            mine: true,
            ..default()
        },
    );
    tick(&mut app, 1);
    // Drive away while it arms.
    command(
        &mut app,
        TankCommand {
            drive: Vec2::Y,
            ..default()
        },
    );
    tick(&mut app, seconds(2.0));
    command(&mut app, TankCommand::default());
    assert!(position(&mut app, entity).distance(START) > 3.0);
    tick(&mut app, seconds(8.5));
    assert_eq!(
        count(&events(&mut app), |event| matches!(
            event,
            SimEvent::Blast { .. }
        )),
        0
    );
    tick(&mut app, seconds(1.0));
    let events = events(&mut app);
    assert_eq!(
        count(&events, |event| matches!(event, SimEvent::Blast { .. })),
        1
    );
    assert!(
        !alive(&app, block),
        "the blast destroys the block next to it"
    );
    assert_eq!(run(&app).lives, 3);
}

#[test]
fn a_tank_has_only_as_many_mines_as_its_stat_allows() {
    let mut app = start("TEST");
    command(
        &mut app,
        TankCommand {
            mine: true,
            drive: Vec2::Y,
            ..default()
        },
    );
    tick(&mut app, seconds(1.8));
    let world = app.world_mut();
    assert_eq!(world.query::<&Mine>().iter(world).count(), 2);
    // The game told of each mine as it was placed.
    let placed = |event: &SimEvent| matches!(event, SimEvent::MinePlaced { .. });
    assert_eq!(count(&events(&mut app), placed), 2);
}

#[test]
fn a_mine_blows_when_an_opposing_tank_comes_close_or_a_bullet_hits_it() {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    command(
        &mut app,
        TankCommand {
            mine: true,
            ..default()
        },
    );
    tick(&mut app, 1);
    command(&mut app, TankCommand::default());
    teleport(&mut app, entity, Vec2::new(0.0, 12.0));
    tick(&mut app, seconds(1.1));
    let mut enemy = spawn_enemy(&mut app, START + Vec2::X * 1.5, &[]);
    tick(&mut app, 2);
    assert!(alive(&app, enemy), "a dormant enemy doesn't trigger mines");
    app.world_mut().get_mut::<Enemy>(enemy).unwrap().awake = true;
    tick(&mut app, seconds(0.7));
    assert!(!alive(&app, enemy));

    // A second mine, shot by its owner from a safe distance.
    teleport(&mut app, entity, START);
    command(
        &mut app,
        TankCommand {
            mine: true,
            ..default()
        },
    );
    tick(&mut app, 1);
    teleport(&mut app, entity, Vec2::new(0.0, 12.0));
    enemy = spawn_enemy(&mut app, START + Vec2::X * 2.5, &[]);
    shoot_at(&mut app, START);
    tick(&mut app, seconds(0.8));
    assert!(!alive(&app, enemy), "the blast reaches a dormant enemy");
    assert_eq!(run(&app).lives, 3);
}

#[test]
fn a_mine_warns_before_it_blows_unless_a_bullet_sets_it_off() {
    let warned = |event: &SimEvent| matches!(event, SimEvent::MineTriggered { at } if at.distance(START) < 0.1);
    let blew = |event: &SimEvent| matches!(event, SimEvent::Blast { .. });
    // The player lays a mine where the run starts, and waits at a safe distance while it
    // arms, which takes 1 s.
    let lay = || {
        let mut app = start("TEST");
        let entity = player(&mut app).unwrap();
        command(
            &mut app,
            TankCommand {
                mine: true,
                ..default()
            },
        );
        tick(&mut app, 1);
        command(&mut app, TankCommand::default());
        teleport(&mut app, entity, Vec2::new(0.0, 12.0));
        tick(&mut app, seconds(1.1));
        events(&mut app);
        app
    };
    // Ticks until the game tells of something, and what else it told on the way.
    let wait_for = |app: &mut App, wanted: fn(&SimEvent) -> bool| {
        let mut told = Vec::new();
        for ticks in 1..=seconds(12.0) {
            tick(app, 1);
            told.extend(events(app));
            if told.iter().any(wanted) {
                return (ticks, told);
            }
        }
        panic!("the game never told of it: {told:?}");
    };
    let warning = seconds(0.6);

    // An enemy comes close: the warning sounds at once, at the mine, and the blast follows
    // 0.6 s later.
    let mut app = lay();
    let enemy = spawn_enemy(&mut app, START + Vec2::X * 1.5, &[]);
    app.world_mut().get_mut::<Enemy>(enemy).unwrap().awake = true;
    let (ticks, told) = wait_for(&mut app, warned);
    assert_eq!((ticks, count(&told, warned), count(&told, blew)), (1, 1, 0));
    let (ticks, told) = wait_for(&mut app, blew);
    assert!(ticks.abs_diff(warning) <= 1, "{ticks} ticks");
    assert_eq!(count(&told, warned), 0);

    // Nobody comes close: the last 0.6 s of the fuse are the same warning, and the mine still
    // blows 10 s after it was armed.
    let mut app = lay();
    let armed_for = seconds(0.1) + 1;
    let (ticks, told) = wait_for(&mut app, warned);
    assert!(
        (armed_for + ticks).abs_diff(seconds(9.4)) <= 2,
        "{ticks} ticks"
    );
    assert_eq!((count(&told, warned), count(&told, blew)), (1, 0));
    let (ticks, told) = wait_for(&mut app, blew);
    assert!(ticks.abs_diff(warning) <= 1, "{ticks} ticks");
    assert_eq!(count(&told, warned), 0);

    // A bullet hits it: no warning.
    let mut app = lay();
    shoot_at(&mut app, START);
    let (_, told) = wait_for(&mut app, blew);
    assert_eq!(count(&told, warned), 0);
}

#[test]
fn a_tank_fires_over_the_mine_it_stands_on() {
    let mut app = start("TEST");
    command(
        &mut app,
        TankCommand {
            mine: true,
            ..default()
        },
    );
    tick(&mut app, 1);
    // The bullet starts in the turret, right above the mine, and leaves it alone.
    shoot_at(&mut app, START + Vec2::Y * 6.0);
    tick(&mut app, seconds(0.5));
    let events = events(&mut app);
    let blasts = count(&events, |event| matches!(event, SimEvent::Blast { .. }));
    assert_eq!(blasts, 0);
    let world = app.world_mut();
    assert_eq!(world.query::<&Mine>().iter(world).count(), 1);
}

/// Waits until the player's tank is destroyed, and tells what the run says destroyed it.
fn wait_for_the_loss(app: &mut App, at_most: f32) -> Option<Blow> {
    for _ in 0..seconds(at_most) {
        if run(app).lives < SOLO_LIVES {
            break;
        }
        tick(app, 1);
    }
    assert!(run(app).lives < SOLO_LIVES, "the player's tank is intact");
    run(app).lost_to
}

/// An enemy bullet that remembers the tank that fired it.
fn spawn_bullet_of(app: &mut App, maker: Genome, at: Vec2, velocity: Vec2) {
    let bullet = spawn_bullet(app, Team::Enemies, at, velocity, 0);
    app.world_mut().entity_mut(bullet).insert(Maker(maker));
}

/// A destructible block in the start bay.
fn spawn_block(app: &mut App, at: Vec2) {
    let block = (Block { sector: 0 }, Position(at), InSector(0));
    app.world_mut().spawn(block);
    app.world_mut().resource_mut::<Field>().stale = true;
}

/// An explosive crate in the start bay.
fn spawn_crate(app: &mut App, at: Vec2) -> Entity {
    let explosive = (ExplosiveCrate { sector: 0 }, Position(at), InSector(0));
    let explosive = app.world_mut().spawn(explosive).id();
    app.world_mut().resource_mut::<Field>().stale = true;
    explosive
}

#[test]
fn a_run_tells_whose_bullet_destroyed_the_player() {
    let mut app = start("TEST");
    let levels = [(Stat::Turret, 6), (Stat::Muzzle, 4)];
    let sentry = spawn_awake(&mut app, Vec2::new(0.0, 13.0), &levels);
    let genome = app.world().get::<Tank>(sentry).unwrap().genome;
    assert_eq!(run(&app).lost_to, None);
    let blow = Blow {
        weapon: Weapon::Bullet,
        genome,
        team: Team::Enemies,
    };
    assert_eq!(wait_for_the_loss(&mut app, 6.0), Some(blow));
    // The run remembers it after the player has come back.
    tick(&mut app, seconds(RESPAWN_DELAY) + 2);
    assert!(player(&mut app).is_some());
    assert_eq!(run(&app).lost_to, Some(blow));
}

#[test]
fn a_bullet_tells_who_fired_it_after_that_tank_is_gone() {
    let mut app = start("TEST");
    // Its bullets are slow: the first one is still on its way when the tank is destroyed.
    let sentry = spawn_awake(&mut app, Vec2::new(0.0, 13.0), &[(Stat::Turret, 6)]);
    let genome = app.world().get::<Tank>(sentry).unwrap().genome;
    for _ in 0..seconds(6.0) {
        if bullets(&mut app) > 0 {
            break;
        }
        tick(&mut app, 1);
    }
    assert_eq!(bullets(&mut app), 1);
    app.world_mut()
        .resource_mut::<Casualties>()
        .tanks
        .push(sentry);
    tick(&mut app, 1);
    assert!(!alive(&app, sentry));
    let blow = Blow {
        weapon: Weapon::Bullet,
        genome,
        team: Team::Enemies,
    };
    assert_eq!(wait_for_the_loss(&mut app, 3.0), Some(blow));
}

#[test]
fn a_mine_is_the_doing_of_the_tank_that_laid_it() {
    let mut app = start("TEST");
    command(
        &mut app,
        TankCommand {
            mine: true,
            ..default()
        },
    );
    tick(&mut app, 1);
    command(&mut app, TankCommand::default());
    // The player stays on its own mine until the fuse runs out.
    let blow = Blow {
        weapon: Weapon::Mine,
        genome: Genome::PLAYER,
        team: Team::Players,
    };
    assert_eq!(wait_for_the_loss(&mut app, 12.0), Some(blow));
}

#[test]
fn a_crate_is_the_doing_of_the_tank_that_set_it_off() {
    let mut app = start("TEST");
    // A crate next to the player, and an enemy bullet that flies into its far side.
    let crate_at = START + Vec2::new(2.5, 0.0);
    spawn_crate(&mut app, crate_at);
    let mut genome = Genome::empty(40);
    genome.set(Stat::Muzzle, 3);
    spawn_bullet_of(
        &mut app,
        genome,
        crate_at + Vec2::new(3.0, 0.0),
        Vec2::NEG_X * 10.0,
    );
    let blow = Blow {
        weapon: Weapon::Crate,
        genome,
        team: Team::Enemies,
    };
    assert_eq!(wait_for_the_loss(&mut app, 2.0), Some(blow));
}

#[test]
fn a_shot_that_ends_in_the_barrel_sets_off_the_crate_that_stopped_it() {
    let mut app = start("TEST");
    // The crate stands 0.1 m in front of the tank, so the bullet never leaves the barrel.
    let crate_at = START + Vec2::new(2.0, 0.0);
    let explosive = spawn_crate(&mut app, crate_at);
    let aim = TankCommand {
        aim: crate_at,
        ..default()
    };
    command(&mut app, aim);
    tick(&mut app, seconds(0.3));
    events(&mut app);
    command(&mut app, TankCommand { fire: true, ..aim });
    tick(&mut app, 1);
    let events = events(&mut app);
    assert_eq!(bullets(&mut app), 0);
    let blasts = count(
        &events,
        |event| matches!(event, SimEvent::Blast { at } if *at == crate_at),
    );
    assert_eq!(blasts, 1);
    assert!(!alive(&app, explosive));
    // The tank is within the blast, and that is its own doing.
    let blow = Blow {
        weapon: Weapon::Crate,
        genome: Genome::PLAYER,
        team: Team::Players,
    };
    assert_eq!(wait_for_the_loss(&mut app, 0.1), Some(blow));
}

#[test]
fn a_chain_of_blasts_is_the_doing_of_the_tank_that_started_it() {
    let mut app = start("TEST");
    // An enemy mine blows too far from the player to reach it, but close enough to a crate
    // that stands next to the player.
    let crate_at = START + Vec2::new(2.5, 0.0);
    spawn_crate(&mut app, crate_at);
    let mut genome = Genome::empty(40);
    genome.set(Stat::Mines, 1);
    app.world_mut().spawn((
        Mine {
            owner: Entity::PLACEHOLDER,
            team: Team::Enemies,
            sector: 0,
            state: MineState::Triggered(0.0),
        },
        Maker(genome),
        Position(crate_at + Vec2::new(3.5, 0.0)),
        InSector(0),
    ));
    let blow = Blow {
        weapon: Weapon::Crate,
        genome,
        team: Team::Enemies,
    };
    assert_eq!(wait_for_the_loss(&mut app, 1.0), Some(blow));
}

#[test]
fn a_loss_that_is_nobody_s_doing_is_not_blamed_on_the_last_one() {
    let mut app = start("TEST");
    let mut genome = Genome::empty(40);
    genome.set(Stat::Muzzle, 3);
    spawn_bullet_of(&mut app, genome, START + Vec2::X * 4.0, Vec2::NEG_X * 10.0);
    assert!(wait_for_the_loss(&mut app, 2.0).is_some());
    tick(&mut app, seconds(RESPAWN_DELAY) + 2);
    // The tank is taken away without a blow, as a test does.
    destroy_player(&mut app);
    assert_eq!(run(&app).lives, SOLO_LIVES - 2);
    assert_eq!(run(&app).lost_to, None);
}

#[test]
fn a_barrier_absorbs_bullets_and_unlocks_when_the_player_comes_close() {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    shoot_at(&mut app, START + Vec2::Y);
    tick(&mut app, seconds(1.5));
    let events_so_far = events(&mut app);
    assert_eq!(
        count(&events_so_far, |event| matches!(
            event,
            SimEvent::Ricochet { .. }
        )),
        0
    );
    assert_eq!(bullets(&mut app), 0);
    assert_eq!(
        count(&events_so_far, |event| matches!(
            event,
            SimEvent::BarrierUnlocked { .. }
        )),
        0
    );

    command(
        &mut app,
        TankCommand {
            drive: Vec2::Y,
            ..default()
        },
    );
    tick(&mut app, seconds(1.0));
    let unlocked = count(&events(&mut app), |event| {
        matches!(event, SimEvent::BarrierUnlocked { .. })
    });
    assert_eq!(unlocked, 1);
    assert!(position(&mut app, entity).y > 16.0 - BARRIER_GATHER_DISTANCE);
    assert_eq!(run(&app).sector, 0);

    tick(&mut app, seconds(2.0));
    assert_eq!(run(&app).sector, 1);
    assert_eq!(app.world().get::<Tank>(entity).unwrap().sector, 1);
    let awake = enemies(&mut app, 1);
    assert!(
        awake
            .iter()
            .all(|&enemy| app.world().get::<Enemy>(enemy).unwrap().awake)
    );
    assert!(app.world().resource::<Field>().loaded.contains(&2));

    // There is no way back.
    command(
        &mut app,
        TankCommand {
            drive: Vec2::NEG_Y,
            ..default()
        },
    );
    tick(&mut app, seconds(2.0));
    assert!(position(&mut app, entity).y > 16.0);
}

#[test]
fn a_dormant_enemy_keeps_its_turret_on_the_way_in() {
    // The sixth sector of this seed runs west: the way in is to the east of its enemies, and
    // the place where the run started is far to the south.
    let mut app = start("KTVD3CA6");
    for _ in 0..5 {
        let sector = run(&app).sector;
        clear_sector(&mut app, sector);
        cross_barrier(&mut app);
    }
    assert_eq!(run(&app).sector, 5);
    let dormant = enemies(&mut app, 6);
    assert!(!dormant.is_empty());
    tick(&mut app, seconds(8.0));
    let mut field = app.world_mut().resource_mut::<Field>();
    let plan = field.corridor.plan(6).clone();
    let entry = (plan.entry.0 + plan.entry.1).as_vec2() / 2.0;
    for enemy in dormant {
        let to_the_way_in = (entry - position(&mut app, enemy)).to_angle();
        let turret = app.world().get::<Motion>(enemy).unwrap().turret;
        let off = tanku_core::geom::angle_difference(turret, to_the_way_in).abs();
        assert!(off < 1e-3, "the turret is {off} rad off the way in");
    }
}

#[test]
fn an_enemy_never_crosses_a_barrier() {
    let mut app = start("TEST");
    let enemy = spawn_enemy(&mut app, Vec2::new(0.0, 14.0), &[(Stat::Engine, 7)]);
    app.world_mut().get_mut::<TankCommand>(enemy).unwrap().drive = Vec2::Y;
    app.world_mut().get_mut::<Motion>(enemy).unwrap().hull = std::f32::consts::FRAC_PI_2;
    // Unlock the barrier of the bay, so that only the rule holds the enemy back.
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, Vec2::new(4.0, 12.0));
    app.world_mut().resource_mut::<Casualties>().tanks.clear();
    for _ in 0..seconds(2.0) {
        app.world_mut().get_mut::<Enemy>(enemy).unwrap().awake = false;
        app.world_mut().get_mut::<TankCommand>(enemy).unwrap().drive = Vec2::Y;
        // `idle` resets the command of a dormant enemy, so the push comes from its velocity.
        app.world_mut().get_mut::<Motion>(enemy).unwrap().velocity = Vec2::Y * 8.0;
        tick(&mut app, 1);
    }
    assert!(position(&mut app, enemy).y < 16.0);
}

#[test]
fn clearing_a_sector_scores_and_the_corridor_follows_the_player() {
    let mut app = start("TEST");
    cross_barrier(&mut app);
    assert_eq!(run(&app).sector, 1);
    events(&mut app);
    clear_sector(&mut app, 1);
    let state = run(&app);
    assert_eq!((state.cleared, state.kills), (1, 1));
    // The game told of the tank that was destroyed, and of the sector that is cleared.
    let told = events(&mut app);
    let destroyed = |event: &SimEvent| {
        let enemies = Team::Enemies;
        matches!(event, SimEvent::TankDestroyed { team, .. } if *team == enemies)
    };
    assert_eq!(count(&told, destroyed), 1);
    let cleared = SimEvent::SectorCleared { sector: 1 };
    assert_eq!(count(&told, |event| *event == cleared), 1);

    cross_barrier(&mut app);
    assert_eq!(run(&app).sector, 2);
    let field = app.world().resource::<Field>();
    assert_eq!(field.loaded.iter().copied().collect::<Vec<_>>(), [1, 2, 3]);
    tick(&mut app, 1);
    let world = app.world_mut();
    let left_behind = world
        .query::<&InSector>()
        .iter(world)
        .any(|sector| sector.0 == 0);
    assert!(!left_behind, "the start bay is unloaded");
}

#[test]
fn every_sector_that_is_cleared_is_a_checkpoint() {
    let mut app = start("TEST");
    assert_eq!(run(&app).checkpoint, 0);
    cross_barrier(&mut app);
    // With an enemy left in the sector, the player would come back in the start bay.
    assert_eq!((run(&app).sector, run(&app).checkpoint), (1, 0));
    clear_sector(&mut app, 1);
    assert_eq!(run(&app).checkpoint, 1);
    cross_barrier(&mut app);
    clear_sector(&mut app, 2);
    assert_eq!(run(&app).checkpoint, 2);
    // Neither of them is an arena, and only an arena leaves a life.
    assert!(first_arena(&mut app) > 2);
    assert!(lives_on_the_floor(&mut app).is_empty());
}

/// Carries the player through cleared sectors up to a sector, which it enters.
fn advance_to(app: &mut App, sector: u32) {
    loop {
        cross_barrier(app);
        let reached = run(app).sector;
        if reached >= sector {
            return;
        }
        clear_sector(app, reached);
    }
}

/// Destroys the player's tank.
fn destroy_player(app: &mut App) {
    let entity = player(app).unwrap();
    app.world_mut()
        .resource_mut::<Casualties>()
        .tanks
        .push(entity);
    tick(app, 1);
}

#[test]
fn a_destroyed_player_comes_back_where_the_sector_begins_and_finds_it_as_it_was_left() {
    let mut app = start("TEST");
    advance_to(&mut app, 6);
    shelter(&mut app);
    let foes = enemies(&mut app, 6);
    assert_eq!(foes.len(), 3);
    let home = |app: &App, foe: Entity| app.world().get::<Home>(foe).unwrap().at;
    let homes: Vec<Vec2> = foes.iter().map(|&foe| home(&app, foe)).collect();
    // The fight goes on for a while: the enemies move, fire and lay mines.
    tick(&mut app, seconds(3.0));
    // One of them is destroyed, and so is a block of the sector.
    destroy(&mut app, foes[0]);
    let block = {
        let world = app.world_mut();
        let mut blocks = world.query::<(Entity, &Block)>();
        let found = blocks.iter(world).find(|(_, block)| block.sector == 6);
        found.unwrap().0
    };
    app.world_mut().despawn(block);
    // A bullet hangs in the air and a mine lies on the floor.
    let bullet = spawn_bullet(&mut app, Team::Enemies, homes[1], Vec2::X * 0.01, 0);
    app.world_mut().entity_mut(bullet).insert(InSector(6));
    let mine = Mine {
        owner: foes[1],
        team: Team::Enemies,
        sector: 6,
        state: MineState::Arming(60.0),
    };
    let mine = (mine, Position(homes[2]), InSector(6));
    let mine = app.world_mut().spawn(mine).id();
    tick(&mut app, 1);
    assert!(alive(&app, bullet) && alive(&app, mine));

    // The player is destroyed, and comes back in front of the barrier of sector 5.
    let entity = player(&mut app).unwrap();
    app.world_mut().entity_mut(entity).remove::<Protected>();
    destroy_player(&mut app);
    assert_eq!(run(&app).lives, 2);
    tick(&mut app, seconds(RESPAWN_DELAY) + 1);
    let state = run(&app);
    assert_eq!(
        (state.sector, state.checkpoint, state.cleared, state.phase),
        (5, 5, 5, RunPhase::Playing)
    );
    let gate = barrier(&mut app, 5);
    let entity = player(&mut app).unwrap();
    let at = position(&mut app, entity);
    let to_gate = tanku_core::geom::distance_to_segment(at, gate.from, gate.to);
    assert!(to_gate < 3.0, "{to_gate} m from the barrier");
    assert_eq!(app.world().get::<Tank>(entity).unwrap().sector, 5);

    // The enemy that was destroyed stays destroyed. The two others are back where they
    // started, asleep, with their guns loaded.
    assert!(!alive(&app, foes[0]));
    assert_eq!(enemies(&mut app, 6).len(), 2);
    for (index, &foe) in foes.iter().enumerate().skip(1) {
        assert_eq!(position(&mut app, foe), homes[index]);
        let world = app.world();
        assert!(!world.get::<Enemy>(foe).unwrap().awake);
        let genome = world.get::<Tank>(foe).unwrap().genome;
        assert_eq!(world.get::<Gun>(foe).unwrap().rounds, genome.magazine());
        assert_eq!(world.get::<Motion>(foe).unwrap().velocity, Vec2::ZERO);
    }
    // Nothing flies and nothing lies in wait, and the block is still gone.
    assert_eq!(bullets(&mut app), 0);
    let world = app.world_mut();
    assert_eq!(world.query::<&Mine>().iter(world).count(), 0);
    assert!(!alive(&app, block));

    // They wake again when the player crosses the barrier again.
    cross_barrier(&mut app);
    assert_eq!(run(&app).sector, 6);
    for &foe in &foes[1..] {
        assert!(app.world().get::<Enemy>(foe).unwrap().awake);
    }
}

#[test]
fn a_single_player_has_three_lives() {
    let mut app = start("TEST");
    cross_barrier(&mut app);
    for lives in [2, 1] {
        destroy_player(&mut app);
        assert_eq!(run(&app).lives, lives);
        tick(&mut app, seconds(RESPAWN_DELAY) + 1);
        // Before the first sector is cleared, the tank comes back in the start bay.
        let state = run(&app);
        assert_eq!((state.sector, state.phase), (0, RunPhase::Playing));
        let entity = player(&mut app).unwrap();
        assert_eq!(position(&mut app, entity), START);
        cross_barrier(&mut app);
    }
    destroy_player(&mut app);
    let state = run(&app);
    assert_eq!((state.lives, state.phase), (0, RunPhase::Over));
    assert_eq!(
        count(&events(&mut app), |event| matches!(
            event,
            SimEvent::RunOver
        )),
        1
    );
    // The simulation stops with the run.
    let ticks = state.ticks;
    tick(&mut app, 10);
    assert_eq!(run(&app).ticks, ticks);
}

/// The lives that lie on the floor, each with its place.
fn lives_on_the_floor(app: &mut App) -> Vec<(Entity, Vec2)> {
    let world = app.world_mut();
    let mut lives = world.query_filtered::<(Entity, &Position), With<Life>>();
    lives.iter(world).map(|(life, at)| (life, at.0)).collect()
}

#[test]
fn an_arena_that_is_cleared_leaves_a_life_for_a_player_who_is_short_of_one() {
    let (mut app, arena, _) = enter_arena("TEST");
    // A corridor leaves nothing, and neither does an arena that still holds a wave back.
    assert!(lives_on_the_floor(&mut app).is_empty());
    tick(&mut app, seconds(ARENA_DELAY + APPEAR_TIME) + 6);
    destroy_enemies(&mut app, arena);
    assert!(lives_on_the_floor(&mut app).is_empty());
    events(&mut app);
    clear_sector(&mut app, arena);
    let dropped = |event: &SimEvent| matches!(event, SimEvent::LifeDropped { .. });
    assert_eq!(told(&mut app, dropped), 1);
    let lives = lives_on_the_floor(&mut app);
    assert_eq!(lives.len(), 1);
    let (life, at) = lives[0];
    // It lies 4 m in front of the barrier.
    let gate = barrier(&mut app, arena);
    let to_gate = tanku_core::geom::distance_to_segment(at, gate.from, gate.to);
    assert_eq!(to_gate, 4.0);

    // A player who has every life drives over it for nothing.
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, at + Vec2::X * (LIFE_REACH - 0.2));
    tick(&mut app, 3);
    assert!(alive(&app, life));
    assert_eq!(run(&app).lives, SOLO_LIVES);
    // One who is short of a life takes it, and has no more than a run starts with.
    app.world_mut().resource_mut::<Run>().lives = SOLO_LIVES - 1;
    events(&mut app);
    tick(&mut app, 1);
    assert!(!alive(&app, life));
    assert_eq!(run(&app).lives, SOLO_LIVES);
    let taken = |event: &SimEvent| matches!(event, SimEvent::LifeTaken { .. });
    assert_eq!(told(&mut app, taken), 1);
}

#[test]
fn a_life_lies_out_of_reach_until_the_player_comes_to_it() {
    let (mut app, arena, _) = enter_arena("TEST");
    app.world_mut().resource_mut::<Run>().lives = 1;
    clear_sector(&mut app, arena);
    // The player is at the other end of the arena.
    tick(&mut app, seconds(1.0));
    assert_eq!(lives_on_the_floor(&mut app).len(), 1);
    assert_eq!(run(&app).lives, 1);
}

#[test]
fn a_life_that_is_left_on_the_floor_is_lost_when_the_player_moves_on() {
    let (mut app, arena, _) = enter_arena("TEST");
    clear_sector(&mut app, arena);
    assert_eq!(lives_on_the_floor(&mut app).len(), 1);
    cross_barrier(&mut app);
    assert_eq!(run(&app).sector, arena + 1);
    assert!(lives_on_the_floor(&mut app).is_empty());
    // A player who is destroyed in the next sector comes back in the arena, and finds nothing.
    app.insert_resource(Playtest {
        infinite_lives: false,
    });
    destroy_player(&mut app);
    tick(&mut app, seconds(RESPAWN_DELAY) + 1);
    assert_eq!((run(&app).sector, run(&app).lives), (arena, SOLO_LIVES - 1));
    assert!(lives_on_the_floor(&mut app).is_empty());
}

#[test]
fn several_players_are_left_no_life() {
    let mut app = team("TEST", 2);
    app.insert_resource(Playtest {
        infinite_lives: true,
    });
    let arena = first_arena(&mut app);
    skip_to(&mut app, arena);
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    carry(&mut app, first, arena - 1, false);
    carry(&mut app, second, arena - 1, false);
    tick(&mut app, 2);
    carry(&mut app, first, arena - 1, true);
    tick(&mut app, 1);
    assert_eq!(run(&app).sector, arena);
    events(&mut app);
    clear_sector(&mut app, arena);
    let cleared = SimEvent::SectorCleared { sector: arena };
    assert_eq!(told(&mut app, |event| *event == cleared), 1);
    assert!(lives_on_the_floor(&mut app).is_empty());
}

/// The first arena of the corridor of a run.
fn first_arena(app: &mut App) -> u32 {
    let mut field = app.world_mut().resource_mut::<Field>();
    let arena = (1..60).find(|&sector| field.corridor.plan(sector).kind == SectorKind::Arena);
    arena.unwrap()
}

/// Starts the run of a playtest and puts the player in the first arena of its corridor, just
/// behind the barrier. Returns the sector of the arena and how many tanks each wave has.
fn enter_arena(seed: &str) -> (App, u32, Vec<u32>) {
    let mut app = start(seed);
    app.insert_resource(Playtest {
        infinite_lives: true,
    });
    let arena = first_arena(&mut app);
    skip_to(&mut app, arena);
    // Seen through its barrier, an arena is empty.
    assert!(enemies(&mut app, arena).is_empty());
    cross_barrier(&mut app);
    assert_eq!(run(&app).sector, arena);
    let sizes = progression::wave_sizes(progression::enemy_count(arena, 1, true));
    (app, arena, sizes)
}

/// Keeps the player's tank from harm, so that a test of the enemies isn't cut short by one
/// of their bullets.
fn shelter(app: &mut App) {
    let entity = player(app).unwrap();
    app.world_mut().entity_mut(entity).insert(Protected(1.0e9));
}

/// How many of the events that happened are the ones wanted.
fn told(app: &mut App, wanted: impl Fn(&SimEvent) -> bool) -> usize {
    count(&events(app), wanted)
}

#[test]
fn the_enemies_of_an_arena_come_in_waves_after_a_warning() {
    let (mut app, arena, sizes) = enter_arena("TEST");
    shelter(&mut app);
    assert_eq!(sizes.len(), ARENA_WAVES as usize);
    let incoming = |event: &SimEvent| matches!(event, SimEvent::TankIncoming { .. });
    let appeared = |event: &SimEvent| matches!(event, SimEvent::TankAppeared { .. });

    // Nothing happens for a moment after the player comes in.
    events(&mut app);
    tick(&mut app, seconds(ARENA_DELAY) - 5);
    assert_eq!(told(&mut app, |_| true), 0);
    assert!(enemies(&mut app, arena).is_empty());
    // What was told since the wave before was destroyed.
    let mut news: Vec<SimEvent> = Vec::new();
    for (wave, &size) in sizes.iter().enumerate() {
        // Then the wave is announced, and the place of each of its tanks is marked.
        tick(&mut app, 8);
        news.extend(events(&mut app));
        let announced = SimEvent::WaveIncoming {
            wave: wave as u32 + 1,
            of: sizes.len() as u32,
        };
        assert_eq!(count(&news, |event| *event == announced), 1, "wave {wave}");
        assert_eq!(count(&news, incoming), size as usize);
        assert!(enemies(&mut app, arena).is_empty());
        // The tanks are there a moment later, awake, and nothing of the sort is said twice.
        tick(&mut app, seconds(APPEAR_TIME) - 12);
        assert!(enemies(&mut app, arena).is_empty());
        tick(&mut app, 16);
        let there = enemies(&mut app, arena);
        assert_eq!(there.len(), size as usize);
        assert!(
            there
                .iter()
                .all(|&tank| app.world().get::<Enemy>(tank).unwrap().awake)
        );
        let there_news = events(&mut app);
        assert_eq!(count(&there_news, appeared), size as usize);
        assert_eq!(count(&there_news, incoming), 0);

        // The sector isn't cleared while the arena holds a wave back, and the next wave waits
        // for the last tank of this one.
        let cleared = SimEvent::SectorCleared { sector: arena };
        if size > 1 {
            let one = there[0];
            destroy(&mut app, one);
            tick(&mut app, seconds(1.0));
            assert_eq!(told(&mut app, incoming), 0);
        }
        destroy_enemies(&mut app, arena);
        let last = wave + 1 == sizes.len();
        news = events(&mut app);
        assert_eq!(count(&news, |event| *event == cleared), last as usize);
        assert_eq!(run(&app).cleared >= arena, last);
    }
    // Once the last wave is destroyed, the arena is cleared like any sector.
    let entity = player(&mut app).unwrap();
    let gate = barrier(&mut app, arena);
    teleport(&mut app, entity, (gate.from + gate.to) / 2.0 - gate.outward);
    tick(&mut app, 2);
    assert!(barrier(&mut app, arena).unlocked);
}

#[test]
fn a_tank_of_an_arena_waits_for_room_to_appear() {
    let (mut app, arena, sizes) = enter_arena("TEST");
    shelter(&mut app);
    // The player stands on the place of the first tank of the first wave.
    let place = {
        let field = app.world().resource::<Field>();
        field.arenas[&arena].waiting[0][0].at
    };
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, place + Vec2::X);
    tick(&mut app, seconds(ARENA_DELAY + APPEAR_TIME) + 6);
    // The others are there. That one isn't, and its place was never marked.
    assert_eq!(enemies(&mut app, arena).len(), sizes[0] as usize - 1);
    let marked = |event: &SimEvent| matches!(event, SimEvent::TankIncoming { at } if *at == place);
    assert_eq!(told(&mut app, marked), 0);
    // It comes once the player has moved away, and the wave isn't over before.
    destroy_enemies(&mut app, arena);
    tick(&mut app, seconds(1.0));
    assert!(enemies(&mut app, arena).is_empty());
    assert_eq!(
        told(&mut app, |event| matches!(
            event,
            SimEvent::WaveIncoming { .. }
        )),
        0
    );
    teleport(&mut app, entity, place + Vec2::X * (APPEAR_ROOM + 1.0));
    tick(&mut app, 2);
    assert_eq!(told(&mut app, marked), 1);
    tick(&mut app, seconds(APPEAR_TIME) + 4);
    assert_eq!(enemies(&mut app, arena).len(), 1);
}

#[test]
fn an_arena_sends_what_is_left_of_its_wave_again_when_the_player_comes_back() {
    let (mut app, arena, sizes) = enter_arena("TEST");
    assert!(sizes[0] > 1);
    tick(&mut app, seconds(ARENA_DELAY + APPEAR_TIME) + 6);
    let wave = enemies(&mut app, arena);
    assert_eq!(wave.len(), sizes[0] as usize);
    destroy(&mut app, wave[0]);
    let survivors = sizes[0] - 1;

    // The player is destroyed, and comes back in front of the arena: it is empty again, and
    // holds back what was left of its first wave, with the waves that had not come.
    destroy_player(&mut app);
    tick(&mut app, seconds(RESPAWN_DELAY) + 1);
    assert_eq!(run(&app).sector, arena - 1);
    assert!(enemies(&mut app, arena).is_empty());
    let field = app.world().resource::<Field>();
    let held: Vec<u32> = field.arenas[&arena]
        .waiting
        .iter()
        .map(|wave| wave.len() as u32)
        .collect();
    assert_eq!(held[0], survivors);
    assert_eq!(held[1..], sizes[1..]);
    assert_eq!(field.arenas[&arena].fuse, Fuse::Unlit);

    // The wave comes again as it did the first time: after a moment, and after its marks.
    cross_barrier(&mut app);
    events(&mut app);
    tick(&mut app, seconds(ARENA_DELAY) + 4);
    let news = events(&mut app);
    let again = SimEvent::WaveIncoming {
        wave: 1,
        of: sizes.len() as u32,
    };
    assert_eq!(count(&news, |event| *event == again), 1);
    let marks = count(&news, |event| {
        matches!(event, SimEvent::TankIncoming { .. })
    });
    assert_eq!(marks as u32, survivors);
    tick(&mut app, seconds(APPEAR_TIME) + 4);
    assert_eq!(enemies(&mut app, arena).len() as u32, survivors);
}

#[test]
fn an_arena_holds_the_group_that_its_players_call_for() {
    let mut app = team("TEST", 4);
    app.insert_resource(Playtest {
        infinite_lives: true,
    });
    let arena = first_arena(&mut app);
    skip_to(&mut app, arena);
    // Four players call for a larger group, which comes in as many waves. The smallest
    // arenas hold as many tanks as fit.
    let mut field = app.world_mut().resource_mut::<Field>();
    let room = field.corridor.layout(arena).spawns.len() as u32;
    let expected = progression::enemy_count(arena, 4, true).min(room);
    assert!(expected > progression::enemy_count(arena, 1, true));
    let held: Vec<u32> = field.arenas[&arena]
        .waiting
        .iter()
        .map(|wave| wave.len() as u32)
        .collect();
    assert_eq!(held, progression::wave_sizes(expected));
    assert!(enemies(&mut app, arena).is_empty());
}

/// Asks to skip to a sector, as the menu of a playtest does.
fn skip_to(app: &mut App, sector: u32) {
    app.world_mut().write_message(RunRequest::SkipTo { sector });
    tick(app, 1);
}

fn players(app: &mut App) -> usize {
    let world = app.world_mut();
    world
        .query_filtered::<(), With<Player>>()
        .iter(world)
        .count()
}

fn loaded(app: &App) -> Vec<u32> {
    let field = app.world().resource::<Field>();
    field.loaded.iter().copied().collect()
}

#[test]
fn a_playtest_skips_to_a_sector_and_comes_back() {
    let mut app = start("TEST");
    app.insert_resource(Playtest {
        infinite_lives: false,
    });
    skip_to(&mut app, 7);
    let state = run(&app);
    assert_eq!(
        (state.sector, state.checkpoint, state.cleared, state.phase),
        (6, 6, 6, RunPhase::Playing)
    );
    assert_eq!((players(&mut app), loaded(&app)), (1, vec![6, 7]));
    // The tank stands in front of the barrier of the sector before, which is cleared.
    let gate = barrier(&mut app, 6);
    let entity = player(&mut app).unwrap();
    let at = position(&mut app, entity);
    assert!(tanku_core::geom::distance_to_segment(at, gate.from, gate.to) < 3.0);
    assert!(enemies(&mut app, 6).is_empty());
    let ahead = enemies(&mut app, 7);
    assert!(!ahead.is_empty());
    assert!(
        ahead
            .iter()
            .all(|&enemy| !app.world().get::<Enemy>(enemy).unwrap().awake)
    );
    // Its lives are those it had.
    assert_eq!(run(&app).lives, 3);
    cross_barrier(&mut app);
    assert_eq!(run(&app).sector, 7);

    skip_to(&mut app, 10);
    assert_eq!((run(&app).sector, loaded(&app)), (9, vec![9, 10]));
    cross_barrier(&mut app);
    assert_eq!(run(&app).sector, 10);

    // Backward, down to the start bay. The score keeps the furthest sector.
    skip_to(&mut app, 1);
    let state = run(&app);
    assert_eq!((state.sector, state.checkpoint, state.cleared), (0, 0, 9));
    let entity = player(&mut app).unwrap();
    assert_eq!(position(&mut app, entity), START);
    assert_eq!(enemies(&mut app, 1).len(), 1);
    assert_eq!((players(&mut app), loaded(&app)), (1, vec![0, 1]));

    // The tank comes back there when it is destroyed.
    skip_to(&mut app, 4);
    destroy_player(&mut app);
    tick(&mut app, seconds(RESPAWN_DELAY) + 1);
    let state = run(&app);
    assert_eq!((state.sector, state.lives), (3, 2));
}

#[test]
fn a_skip_goes_no_further_than_the_last_sector_and_the_last_one_asked_wins() {
    let mut app = start("TEST");
    app.insert_resource(Playtest {
        infinite_lives: true,
    });
    skip_to(&mut app, u32::MAX);
    assert_eq!(run(&app).sector, PLAYTEST_LAST_SECTOR - 1);
    skip_to(&mut app, 0);
    assert_eq!(run(&app).sector, 0);

    app.world_mut()
        .write_message(RunRequest::SkipTo { sector: 3 });
    skip_to(&mut app, 12);
    assert_eq!((run(&app).sector, loaded(&app)), (11, vec![11, 12]));
    assert_eq!(players(&mut app), 1);

    // A skip asked while the tank is gone brings it back at once.
    destroy_player(&mut app);
    assert!(player(&mut app).is_none());
    skip_to(&mut app, 5);
    assert_eq!((run(&app).sector, run(&app).phase), (4, RunPhase::Playing));
    assert_eq!(players(&mut app), 1);
}

#[test]
fn a_skip_asked_with_the_start_of_a_run_applies_to_that_run() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin))
        .insert_resource(Playtest {
            infinite_lives: true,
        });
    app.world_mut().write_message(RunRequest::Start {
        seed: "TEST".into(),
        players: 1,
        drawn: true,
        difficulty: Difficulty::Normal,
        free: false,
        tanks: Vec::new(),
    });
    skip_to(&mut app, 5);
    assert_eq!((run(&app).sector, loaded(&app)), (4, vec![4, 5]));
    assert_eq!(players(&mut app), 1);
}

#[test]
fn only_a_playtest_skips_sectors() {
    let mut app = start("TEST");
    skip_to(&mut app, 7);
    tick(&mut app, 3);
    let state = run(&app);
    assert_eq!((state.sector, state.checkpoint, state.cleared), (0, 0, 0));
    let entity = player(&mut app).unwrap();
    assert_eq!(position(&mut app, entity), START);
}

#[test]
fn infinite_lives_keep_a_playtest_going() {
    let mut app = start("TEST");
    app.insert_resource(Playtest {
        infinite_lives: true,
    });
    for _ in 0..5 {
        destroy_player(&mut app);
        let state = run(&app);
        assert_eq!(state.lives, 3);
        assert!(matches!(state.phase, RunPhase::Respawning(_)));
        tick(&mut app, seconds(RESPAWN_DELAY) + 1);
        assert_eq!(run(&app).phase, RunPhase::Playing);
    }
    let events = events(&mut app);
    assert_eq!(count(&events, |event| *event == SimEvent::RunOver), 0);
    assert_eq!(
        count(&events, |event| matches!(
            event,
            SimEvent::PlayerRespawned { .. }
        )),
        5
    );

    // Turned off, the lives count again.
    app.insert_resource(Playtest {
        infinite_lives: false,
    });
    destroy_player(&mut app);
    assert_eq!(run(&app).lives, 2);
}

#[test]
fn a_tank_that_has_just_appeared_holds_its_fire() {
    let mut app = begin("TEST");
    let fire = TankCommand {
        aim: START + Vec2::Y,
        fire: true,
        ..default()
    };
    // The button that started the run is still down.
    command(&mut app, fire);
    tick(&mut app, seconds(SPAWN_FIRE_DELAY) - 2);
    assert_eq!(bullets(&mut app), 0);
    tick(&mut app, 4);
    assert_eq!(bullets(&mut app), 1);

    // The same after the tank comes back.
    destroy_player(&mut app);
    tick(&mut app, seconds(RESPAWN_DELAY) + 1);
    events(&mut app);
    command(&mut app, fire);
    tick(&mut app, seconds(SPAWN_FIRE_DELAY) - 2);
    let shots = |app: &mut App| count(&events(app), |event| matches!(event, SimEvent::Shot { .. }));
    assert_eq!(shots(&mut app), 0);
    tick(&mut app, 4);
    assert_eq!(shots(&mut app), 1);
}

#[test]
fn abandoning_a_run_removes_everything() {
    let mut app = start("TEST");
    app.world_mut().write_message(RunRequest::Abandon);
    tick(&mut app, 1);
    assert!(app.world().get_resource::<Run>().is_none());
    let world = app.world_mut();
    assert_eq!(world.query::<&InSector>().iter(world).count(), 0);
    assert!(player(&mut app).is_none());
}

#[test]
fn a_sentry_shoots_the_player_it_sees() {
    let mut app = start("TEST");
    spawn_awake(
        &mut app,
        Vec2::new(0.0, 13.0),
        &[(Stat::Turret, 6), (Stat::Muzzle, 4)],
    );
    tick(&mut app, seconds(6.0));
    assert!(run(&app).lives < 3);
    let shots = count(&events(&mut app), |event| {
        matches!(
            event,
            SimEvent::Shot {
                team: Team::Enemies,
                ..
            }
        )
    });
    assert!(shots > 0);
}

/// Fires a player's bullet straight at an enemy from the north. Returns the enemy and the app
/// a third of a second later, when even a slow brain has seen the bullet coming.
fn shoot_at_enemy(levels: &[(Stat, u8)]) -> (App, Entity) {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    // The player waits out of the way, where no mine reflex is triggered.
    teleport(&mut app, entity, Vec2::new(-5.0, 2.0));
    let enemy = spawn_awake(&mut app, Vec2::new(0.0, 8.0), levels);
    spawn_bullet(
        &mut app,
        Team::Players,
        Vec2::new(0.0, 15.5),
        Vec2::NEG_Y * 10.0,
        0,
    );
    tick(&mut app, seconds(0.33));
    (app, enemy)
}

#[test]
fn a_soldier_dodges_a_bullet_flying_straight_at_it() {
    let body = [(Stat::Engine, 5), (Stat::Tracks, 6)];
    let drive_of = |brain: u8| {
        let levels = [body.as_slice(), &[(Stat::Brain, brain)]].concat();
        let (mut app, enemy) = shoot_at_enemy(&levels);
        let drive = app.world().get::<TankCommand>(enemy).unwrap().drive;
        tick(&mut app, seconds(1.2));
        (drive, alive(&app, enemy))
    };
    // The bullet flies south, so the way out is east or west. A grunt keeps to its own plan,
    // which here is to back away from the player.
    let (grunt, _) = drive_of(1);
    assert!(grunt.y.abs() > 0.5, "the grunt drives {grunt}");
    let (soldier, survived) = drive_of(2);
    assert!(soldier.x.abs() > 0.9, "the soldier drives {soldier}");
    assert!(survived);
}

#[test]
fn a_grunt_raises_its_shield_by_reflex() {
    let (mut app, enemy) = shoot_at_enemy(&[(Stat::Shield, 3), (Stat::Brain, 1)]);
    tick(&mut app, seconds(1.2));
    assert!(alive(&app, enemy));
    let blocked = count(&events(&mut app), |event| {
        matches!(event, SimEvent::ShieldBlocked { .. })
    });
    assert_eq!(blocked, 1);
}

#[test]
fn a_veteran_sees_a_bullet_come_off_a_wall() {
    // A tank without an engine stands near the east wall of the bay, with a shield and a
    // turret that is too slow to turn on a bullet. A bullet flies past it at the wall, and
    // comes back at it from 2.4 m: too late for a brain that only then sees it coming.
    let fares = |brain: u8| {
        let mut app = start("TEST");
        let entity = player(&mut app).unwrap();
        teleport(&mut app, entity, Vec2::new(-5.0, 2.0));
        let at = Vec2::new(5.0, 8.0);
        let enemy = spawn_awake(&mut app, at, &[(Stat::Shield, 3), (Stat::Brain, brain)]);
        let bounce = Vec2::new(6.775, 9.6);
        let back = (at - bounce).normalize();
        let out = Vec2::new(-back.x, back.y);
        let bullet = spawn_bullet(&mut app, Team::Players, bounce - out * 7.0, out * 10.0, 0);
        app.world_mut()
            .get_mut::<Bullet>(bullet)
            .unwrap()
            .bounces_left = 1;
        tick(&mut app, seconds(1.5));
        let blocked = count(&events(&mut app), |event| {
            matches!(event, SimEvent::ShieldBlocked { .. })
        });
        (alive(&app, enemy), blocked)
    };
    assert_eq!(fares(2), (false, 0));
    // A veteran has followed the bullet off the wall, and has its shield up in time.
    assert_eq!(fares(3), (true, 1));
}

#[test]
fn a_veteran_shoots_down_a_bullet_that_it_can_not_dodge() {
    // A tank without an engine. A block hides the player from it, so it holds its fire, with
    // its gun turned toward the player. Bullets are fired at it, a quarter of a second apart:
    // each from a place, at a velocity and with a number of bounces left. Tells whether the
    // tank is still there, how many shots it fired and how many bullets ended.
    let fares = |brain: u8, sector: u32, shots: &[(Vec2, Vec2, u8)]| {
        let mut app = start("TEST");
        let entity = player(&mut app).unwrap();
        teleport(&mut app, entity, Vec2::new(-3.0, 14.5));
        spawn_block(&mut app, Vec2::new(-1.8, 10.25));
        let levels = [(Stat::Turret, 6), (Stat::Magazine, 2), (Stat::Brain, brain)];
        let enemy = spawn_thinker(&mut app, Vec2::new(0.0, 6.0), &levels, sector);
        tick(&mut app, seconds(1.0));
        for &(from, velocity, bounces) in shots {
            let bullet = spawn_bullet(&mut app, Team::Players, from, velocity, 0);
            app.world_mut()
                .get_mut::<Bullet>(bullet)
                .unwrap()
                .bounces_left = bounces;
            tick(&mut app, seconds(0.25));
        }
        tick(&mut app, seconds(1.5));
        let events = events(&mut app);
        let shots = count(&events, |event| matches!(event, SimEvent::Shot { .. }));
        let ended = count(&events, |event| {
            matches!(event, SimEvent::BulletDestroyed { .. })
        });
        (alive(&app, enemy), shots, ended)
    };
    // A bullet that flies straight at the tank from the north, from about where the player
    // is. A soldier is destroyed, and the bullet ends on it.
    let north = (Vec2::new(0.0, 15.0), Vec2::NEG_Y * 9.0, 0);
    assert_eq!(fares(2, 60, &[north]), (false, 0, 1));
    // A veteran fires, and the two bullets destroy each other. It fires once, though its gun
    // is ready again well before they meet: its own bullets are slow.
    assert_eq!(fares(3, 60, &[north]), (true, 1, 2));
    // It has seen the second and the third bullet of a burst coming while it dealt with the
    // first, and has a shot for each. A brain of sector 11 is too slow to wait for one
    // bullet to be gone before it looks at the next.
    assert_eq!(fares(3, 11, &[north; 3]), (true, 3, 6));
    // A bullet from 5 m is there in 0.43 s. A veteran of a late sector acts on it after
    // 0.14 s. One of the first sector takes 0.5 s, and is destroyed.
    let close = (Vec2::new(0.0, 11.0), Vec2::NEG_Y * 9.0, 0);
    assert_eq!(fares(3, 60, &[close]), (true, 1, 2));
    assert_eq!(fares(3, 1, &[close]), (false, 0, 1));
    // Of two bullets, it takes the one that gets to it first: here the second one, which is
    // fired from the east, from closer.
    let east = (Vec2::new(6.5, 6.0), Vec2::NEG_X * 9.0, 0);
    let far_north = (Vec2::new(0.0, 15.5), Vec2::NEG_Y * 9.0, 0);
    assert_eq!(fares(3, 60, &[far_north, east]), (true, 2, 4));
    // A bullet that is fired away from the tank, at the east wall of the bay, and comes back
    // at it. The veteran waits until it has bounced, and shoots it down then.
    let off_the_wall = (Vec2::new(3.0, 6.0), Vec2::X * 9.0, 1);
    assert_eq!(fares(2, 60, &[off_the_wall]), (false, 0, 1));
    assert_eq!(fares(3, 60, &[off_the_wall]), (true, 1, 2));
}

/// A fast tank wakes up in the start bay, alone, with a brain of a tier that has the
/// experience of a sector, while bullets of a player are fired: each from a place and at a
/// velocity. Tells where the tank is after some time, if it is still there, and whether it is
/// there a moment later.
fn drives_among(brain: u8, sector: u32, time: f32, shots: &[(Vec2, Vec2)]) -> (Option<Vec2>, bool) {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, Vec2::new(-5.0, 2.0));
    let levels = [(Stat::Engine, 5), (Stat::Tracks, 6), (Stat::Brain, brain)];
    let enemy = spawn_thinker(&mut app, Vec2::new(2.0, 8.0), &levels, sector);
    for &(from, velocity) in shots {
        spawn_bullet(&mut app, Team::Players, from, velocity, 0);
    }
    tick(&mut app, seconds(time));
    let there = alive(&app, enemy).then(|| position(&mut app, enemy));
    tick(&mut app, seconds(0.3));
    (there, alive(&app, enemy))
}

/// A bullet that is fired along the bay so that it is at a place after some time: from the
/// north of the place, or from the south where the north wall is too close.
fn shot_to(at: Vec2, time: f32) -> (Vec2, Vec2) {
    let reach = 10.0 * time;
    let way = if at.y + reach < 15.5 {
        Vec2::NEG_Y
    } else {
        Vec2::Y
    };
    (at - way * reach, way * 10.0)
}

#[test]
fn an_ace_dodges_a_bullet_that_is_aimed_ahead_of_it() {
    // A bullet is fired so that it is where the tank meant to be 0.7 s later.
    let survives = |brain: u8, sector: u32| {
        let there = drives_among(brain, sector, 0.7, &[]).0.unwrap();
        drives_among(brain, sector, 0.7, &[shot_to(there, 0.7)]).1
    };
    // A soldier and a veteran only see a bullet that flies at where their tank is, and drive
    // into this one.
    assert!(!survives(2, 60));
    assert!(!survives(3, 60));
    // An ace looks as far around its tank as it drives in a second. One of a late sector acts
    // on the bullet after 0.14 s, and is missed.
    assert!(survives(4, 60));
    // One of the first sector takes 0.5 s, which leaves it too little time.
    assert!(!survives(4, 1));
}

/// What a tank with a brain of a tier means to do about a player, two ticks after it woke up
/// at a place in the start bay: its drive. The player stands at a place with so many rounds,
/// and an ally without an engine may be at another, awake or not.
fn means(brain: u8, at: Vec2, player_at: Vec2, rounds: u8, ally: Option<(Vec2, bool)>) -> Vec2 {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, player_at);
    app.world_mut().get_mut::<Gun>(entity).unwrap().rounds = rounds;
    if let Some((ally, awake)) = ally {
        let ally = spawn_awake(&mut app, ally, &[]);
        app.world_mut().get_mut::<Enemy>(ally).unwrap().awake = awake;
    }
    let levels = [(Stat::Engine, 3), (Stat::Tracks, 4), (Stat::Brain, brain)];
    let enemy = spawn_awake(&mut app, at, &levels);
    tick(&mut app, 2);
    app.world().get::<TankCommand>(enemy).unwrap().drive
}

#[test]
fn an_ace_drives_around_the_player_away_from_its_ally() {
    // The player stands in the south of the bay and an ally 12 m north of it. A tank wakes up
    // as far from the player, 15 degrees to one side of the ally. Tells which way around the
    // player it drives: counterclockwise when the number is positive.
    let player_at = Vec2::new(0.0, 1.5);
    let around = |brain: u8, side: f32| {
        let out = Vec2::from_angle((90.0 + 15.0 * side).to_radians());
        let ally = Some((player_at + Vec2::Y * 12.0, true));
        out.perp_dot(means(brain, player_at + out * 12.0, player_at, 5, ally))
    };
    // A veteran drives the same way around on either side of its ally.
    assert!(around(3, 1.0) * around(3, -1.0) > 0.0);
    // An ace drives away from its ally on either side, so that their fire crosses.
    assert!(around(4, 1.0) > 0.5, "{}", around(4, 1.0));
    assert!(around(4, -1.0) < -0.5, "{}", around(4, -1.0));
}

#[test]
fn an_ace_baits_a_player_who_has_rounds_and_presses_one_who_has_none() {
    // A tank wakes up 9.5 m north of the player, with an ally elsewhere. Tells how much of
    // its drive is toward the player.
    let (player_at, at) = (Vec2::new(0.0, 1.5), Vec2::new(0.0, 11.0));
    let ally = Vec2::new(-5.0, 13.0);
    let toward = |brain: u8, rounds: u8| {
        means(brain, at, player_at, rounds, Some((ally, true))).dot(Vec2::NEG_Y)
    };
    // A veteran drives around the player at that distance, whatever rounds they have.
    assert!(toward(3, 5).abs() < 0.3 && toward(3, 0).abs() < 0.3);
    // An ace backs away from a player who has rounds, to where it sees their shots coming,
    // and comes at a player who has none.
    assert!(toward(4, 5) < -0.7);
    assert!(toward(4, 0) > 0.7);

    // It presses until the player has three rounds again.
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, player_at);
    spawn_awake(&mut app, ally, &[]);
    let levels = [(Stat::Engine, 3), (Stat::Tracks, 4), (Stat::Brain, 4)];
    let enemy = spawn_awake(&mut app, at, &levels);
    let mut comes = |rounds: u8| {
        app.world_mut().get_mut::<Gun>(entity).unwrap().rounds = rounds;
        tick(&mut app, 2);
        let drive = app.world().get::<TankCommand>(enemy).unwrap().drive;
        drive.dot((player_at - position(&mut app, enemy)).normalize()) > 0.7
    };
    assert!(!comes(5));
    assert!(comes(0));
    assert!(comes(2));
    assert!(!comes(3));
}

#[test]
fn an_ace_retreats_when_it_is_the_last_of_its_group() {
    // A tank wakes up 12 m from the player, with room behind it. Tells how much of its drive
    // is toward the player.
    let player_at = Vec2::new(-5.5, 14.5);
    let out = Vec2::new(11.0, -13.0).normalize();
    let toward = |brain: u8, ally: Option<(Vec2, bool)>| {
        means(brain, player_at + out * 12.0, player_at, 5, ally).dot(-out)
    };
    // A veteran drives around the player at that distance, alone or not, and so does an ace
    // that has an ally.
    let ally = Vec2::new(5.0, 14.0);
    assert!(toward(3, None).abs() < 0.3 && toward(3, Some((ally, true))).abs() < 0.3);
    assert!(toward(4, Some((ally, true))).abs() < 0.3);
    // An ace that is alone backs away. An ally that is not awake is no company.
    assert!(toward(4, None) < -0.7);
    assert!(toward(4, Some((ally, false))) < -0.7);
}

#[test]
fn a_veteran_keeps_its_shield_for_the_bullet_that_it_can_not_dodge() {
    // A slow tank with a shield, and a bullet that flies straight at it from the north. Its
    // turret is too slow to turn on the bullet. The faster the bullet, the less the tank gets
    // out of its way.
    let fares = |brain: u8, speed: f32| {
        let levels = [
            (Stat::Engine, 2),
            (Stat::Tracks, 6),
            (Stat::Shield, 3),
            (Stat::Brain, brain),
        ];
        let mut app = start("TEST");
        let entity = player(&mut app).unwrap();
        teleport(&mut app, entity, Vec2::new(-5.0, 2.0));
        let enemy = spawn_awake(&mut app, Vec2::new(0.0, 8.0), &levels);
        spawn_bullet(
            &mut app,
            Team::Players,
            Vec2::new(0.0, 15.5),
            Vec2::NEG_Y * speed,
            0,
        );
        tick(&mut app, seconds(1.6));
        let events = events(&mut app);
        let raised = count(&events, |event| {
            matches!(event, SimEvent::ShieldRaised { .. })
        });
        let blocked = count(&events, |event| {
            matches!(event, SimEvent::ShieldBlocked { .. })
        });
        (alive(&app, enemy), raised, blocked)
    };
    let speeds = [6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 15.0, 17.0];
    let soldier = speeds.map(|speed| fares(2, speed));
    let veteran = speeds.map(|speed| fares(3, speed));
    // A soldier raises its shield at the last moment whenever the bullet is still close to
    // its tank, and sometimes for a bullet that was going to miss.
    assert!(soldier.iter().any(|(_, raised, blocked)| raised > blocked));
    // A veteran only raises it for a bullet that would hit, and survives every time. It gets
    // out of the way of bullets for which a soldier needs its shield.
    for (speed, (survived, raised, blocked)) in speeds.iter().zip(veteran) {
        assert!(survived, "at {speed} m/s");
        assert_eq!(raised, blocked, "at {speed} m/s");
    }
    let raises = |tank: &[(bool, usize, usize)]| tank.iter().map(|fared| fared.1).sum::<usize>();
    assert!(raises(&veteran) >= 1);
    assert!(raises(&veteran) < raises(&soldier));

    // It raises it at the last moment. A tank without an engine sees a slow bullet coming
    // for a second, and its bubble only lasts half of one.
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, Vec2::new(-5.0, 2.0));
    let levels = [(Stat::Shield, 3), (Stat::Brain, 3)];
    let enemy = spawn_awake(&mut app, Vec2::new(0.0, 8.0), &levels);
    spawn_bullet(
        &mut app,
        Team::Players,
        Vec2::new(0.0, 15.5),
        Vec2::NEG_Y * 6.0,
        0,
    );
    tick(&mut app, seconds(1.6));
    let blocked = count(&events(&mut app), |event| {
        matches!(event, SimEvent::ShieldBlocked { .. })
    });
    assert!(alive(&app, enemy));
    assert_eq!(blocked, 1);
}

#[test]
fn a_veteran_lays_its_mines_where_the_player_is_likely_to_come() {
    // The player is out of the reach of the reflex that lays mines, and stays out of it. A
    // place is a narrow one between two blocks that are 4 m apart.
    let lays = |brain: u8, at: Vec2, narrow: bool, charged: bool, ally: bool| {
        let mut app = start("TEST");
        let entity = player(&mut app).unwrap();
        teleport(&mut app, entity, Vec2::new(0.0, 1.5));
        if narrow {
            spawn_block(&mut app, at - Vec2::X * 3.0);
            spawn_block(&mut app, at + Vec2::X * 3.0);
        }
        let levels = [
            (Stat::Engine, 3),
            (Stat::Tracks, 4),
            (Stat::Mines, 2),
            (Stat::Brain, brain),
        ];
        spawn_awake(&mut app, at, &levels);
        if ally {
            spawn_awake(&mut app, at + Vec2::Y * 2.0, &[]);
        }
        if charged {
            let drive = Vec2::Y;
            command(&mut app, TankCommand { drive, ..default() });
        }
        // Long enough for a tank to lay a second mine, and too short for it to get far
        // enough from the first one.
        tick(&mut app, seconds(0.8));
        count(&events(&mut app), |event| {
            matches!(event, SimEvent::MinePlaced { .. })
        })
    };
    // In the open, a veteran keeps its mines.
    let (near, far) = (Vec2::new(0.0, 9.0), Vec2::new(0.0, 13.0));
    assert_eq!(lays(3, near, false, false, false), 0);
    // In a narrow place it lays one: no tank gets through without setting it off. A soldier
    // lays none.
    assert_eq!(lays(3, near, true, false, false), 1);
    assert_eq!(lays(2, near, true, false, false), 0);
    // It lays none next to an ally, which the blast would reach.
    assert_eq!(lays(3, near, true, false, true), 0);
    // It lays one in the way of a player who drives at it.
    assert_eq!(lays(3, far, false, true, false), 1);
    assert_eq!(lays(2, far, false, true, false), 0);
}

#[test]
fn a_run_earns_achievements_on_a_seed_that_was_drawn_for_it_unless_it_is_a_playtest() {
    let earns = |drawn: bool, playtest: bool, free: bool| {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
        if playtest {
            app.insert_resource(Playtest {
                infinite_lives: true,
            });
        }
        app.world_mut().write_message(RunRequest::Start {
            seed: "TEST".into(),
            players: 1,
            drawn,
            difficulty: Difficulty::Normal,
            free,
            tanks: Vec::new(),
        });
        tick(&mut app, 1);
        assert_eq!(run(&app).free, free);
        run(&app).earns
    };
    assert!(earns(true, false, false));
    // A seed that a player chose, or plays again, can be practiced.
    assert!(!earns(false, false, false));
    assert!(!earns(true, true, false));
    // A run in free mode earns nothing either, even in the standard tank: a tank can be
    // built for a seed.
    assert!(!earns(true, false, true));
}

/// A tank that a player may build: a fast gun whose bullets pierce and bounce three times,
/// and neither mines nor a shield.
fn gun_tank() -> Genome {
    let parts = [
        (Stat::Engine, 4),
        (Stat::Tracks, 4),
        (Stat::Muzzle, 7),
        (Stat::Ricochet, 3),
        (Stat::Piercing, 1),
        (Stat::Magazine, 1),
    ];
    Genome::of_the_garage(&parts).unwrap()
}

/// The tank of the player in a seat.
fn tank_of(app: &mut App, seat: u8) -> Genome {
    let world = app.world_mut();
    let mut players = world.query::<(&Player, &Tank)>();
    let found = players.iter(world).find(|(player, _)| player.seat == seat);
    found.unwrap().1.genome
}

#[test]
fn a_player_fights_in_the_tank_that_the_run_was_started_with() {
    let tank = gun_tank();
    assert_ne!(tank, Genome::PLAYER);
    // The first seat has a tank of its own, and the second one the standard tank.
    let mut team = begin_as("TEST", 2, Difficulty::Normal, vec![tank]);
    assert_eq!(tank_of(&mut team, 0), tank);
    assert_eq!(tank_of(&mut team, 1), Genome::PLAYER);

    let mut app = begin_as("TEST", 1, Difficulty::Normal, vec![tank]);
    tick(&mut app, seconds(SPAWN_FIRE_DELAY));
    let entity = player(&mut app).unwrap();
    assert_eq!(tank_of(&mut app, 0), tank);
    let rounds = |app: &App, entity: Entity| app.world().get::<Gun>(entity).unwrap().rounds;
    assert_eq!(rounds(&app, entity), 2);
    // It fires what its gun makes: a bullet at 19 m/s that pierces and bounces three times.
    shoot_at(&mut app, START + Vec2::Y * 8.0);
    let world = app.world_mut();
    let bullets: Vec<Bullet> = world.query::<&Bullet>().iter(world).copied().collect();
    let [bullet] = bullets.as_slice() else {
        panic!("{} bullets", bullets.len());
    };
    assert!((bullet.velocity.length() - 19.0).abs() < 1e-3);
    assert_eq!((bullet.bounces_left, bullet.pierces_left), (3, 1));
    assert_eq!(rounds(&app, entity), 1);
    // It has neither mines nor a shield, whatever its player asks.
    command(
        &mut app,
        TankCommand {
            aim: START + Vec2::Y * 8.0,
            mine: true,
            shield: true,
            ..default()
        },
    );
    tick(&mut app, 2);
    let world = app.world_mut();
    assert_eq!(world.query::<&Mine>().iter(world).count(), 0);
    assert!(!app.world().get::<Shield>(entity).unwrap().is_active());

    // The player comes back in the same tank, with its magazine full.
    destroy_player(&mut app);
    tick(&mut app, seconds(RESPAWN_DELAY) + 1);
    let entity = player(&mut app).unwrap();
    assert_eq!(tank_of(&mut app, 0), tank);
    assert_eq!(rounds(&app, entity), 2);
}

#[test]
fn a_seat_keeps_the_standard_tank_unless_it_is_given_one_that_a_player_may_build() {
    // A tank that costs more than a player has, one of an enemy, and one that can't drive.
    let mut dear = Genome::PLAYER;
    dear.set(Stat::Muzzle, 7);
    let mut enemy = Genome::empty(150);
    enemy.set(Stat::Engine, 7);
    enemy.set(Stat::Brain, 4);
    let mut post = Genome::PLAYER;
    post.set(Stat::Engine, 0);
    assert!(dear.flaw().is_some() && enemy.flaw().is_some() && post.flaw().is_some());
    let given = vec![dear, gun_tank(), enemy, post];
    let mut app = begin_as("TEST", 4, Difficulty::Normal, given);
    let tanks = [0, 1, 2, 3].map(|seat| tank_of(&mut app, seat));
    assert_eq!(
        tanks,
        [Genome::PLAYER, gun_tank(), Genome::PLAYER, Genome::PLAYER]
    );
    // More tanks than there are seats do no harm.
    let mut app = begin_as("TEST", 1, Difficulty::Normal, vec![gun_tank(); 9]);
    assert_eq!(tank_of(&mut app, 0), gun_tank());
}

#[test]
fn a_run_in_hard_mode_has_the_corridor_of_its_seed_and_the_enemies_of_hard_mode() {
    let mut normal = begin_in("TEST", 2, Difficulty::Normal);
    let mut hard = begin_in("TEST", 2, Difficulty::Hard);
    assert_eq!(run(&normal).difficulty, Difficulty::Normal);
    assert_eq!(run(&hard).difficulty, Difficulty::Hard);
    // Everything stands where it stands in normal mode: the walls, the blocks, the crates
    // and the enemies.
    assert_eq!(fingerprint(&mut hard), fingerprint(&mut normal));
    let solids = |app: &App| app.world().resource::<Field>().colliders.len();
    assert_eq!(solids(&hard), solids(&normal));
    let group = |app: &mut App| -> Vec<(u32, Genome)> {
        let world = app.world_mut();
        let mut tanks: Vec<(u32, Genome)> = world
            .query_filtered::<(&Home, &Tank), With<Enemy>>()
            .iter(world)
            .map(|(home, tank)| (home.number, tank.genome))
            .collect();
        tanks.sort_by_key(|(number, _)| *number);
        tanks
    };
    let (weak, strong) = (group(&mut normal), group(&mut hard));
    assert_eq!(weak.len(), 2);
    assert_eq!(strong.len(), weak.len());
    // They are the group that hard mode makes of that sector for so many players: of the
    // same classes, with more points.
    let seed = tanku_core::seed::seed_value("TEST");
    let made = tanku_core::enemy::group(seed, Difficulty::Hard, 1, 2);
    for ((_, weak), ((number, strong), made)) in weak.iter().zip(strong.iter().zip(&made)) {
        assert_eq!(strong, made, "tank {number}");
        assert_eq!(strong.class, weak.class);
        assert!(strong.budget > weak.budget);
    }
}

#[test]
fn a_player_who_comes_back_in_hard_mode_finds_the_brains_of_hard_mode() {
    let mut app = begin_in("TEST", 1, Difficulty::Hard);
    advance_to(&mut app, 1);
    shelter(&mut app);
    tick(&mut app, 2);
    // How long each brain takes to react, with the sector of its tank.
    let quick = |app: &mut App| -> Vec<(u32, f32)> {
        let world = app.world_mut();
        let mut brains = world.query::<(&Tank, &Brain)>();
        let delays = brains.iter(world);
        let mut delays: Vec<(u32, f32)> = delays
            .map(|(tank, brain)| (tank.sector, brain.reaction_delay()))
            .collect();
        delays.sort_by(|one, other| one.partial_cmp(other).unwrap());
        delays
    };
    let before = quick(&mut app);
    assert!(before.iter().any(|(sector, _)| *sector == 1));
    // A brain of hard mode is as quick as one of twenty sectors further in normal mode.
    for &(sector, delay) in &before {
        let late = progression::experience(sector + 20, Difficulty::Normal);
        assert!((delay - (0.50 - 0.38 * late)).abs() < 1e-6, "{delay}");
        assert!(delay < 0.27, "{delay}");
    }
    // The enemies that are left go back to where they started, with the brains they had.
    let entity = player(&mut app).unwrap();
    app.world_mut().entity_mut(entity).remove::<Protected>();
    destroy_player(&mut app);
    tick(&mut app, seconds(RESPAWN_DELAY) + 2);
    assert_eq!((run(&app).phase, run(&app).lives), (RunPhase::Playing, 2));
    assert_eq!(quick(&mut app), before);
}

/// A position that changes with everything that moves.
fn fingerprint(app: &mut App) -> Vec<(i32, i32)> {
    let world = app.world_mut();
    let mut positions: Vec<(i32, i32)> = world
        .query::<&Position>()
        .iter(world)
        .map(|position| {
            (
                (position.0.x * 1000.0) as i32,
                (position.0.y * 1000.0) as i32,
            )
        })
        .collect();
    positions.sort();
    positions
}

/// Plays a run with a crude bot: it shoots at what it sees, and every two seconds the test
/// clears the sector and carries it through the barrier.
fn play(seed: &str, sectors: u32) -> App {
    let mut app = start(seed);
    while run(&app).sector < sectors {
        for step in 0..seconds(2.0) {
            let state = run(&app);
            if state.phase == RunPhase::Over {
                return app;
            }
            if let Some(entity) = player(&mut app) {
                let at = position(&mut app, entity);
                let target = enemies(&mut app, state.sector)
                    .first()
                    .map_or(at + Vec2::Y, |&enemy| position(&mut app, enemy));
                let drive = Vec2::from_angle(step as f32 * 0.05);
                let mine = step % 97 == 0;
                command(
                    &mut app,
                    TankCommand {
                        drive,
                        aim: target,
                        fire: true,
                        mine,
                        shield: step % 301 == 0,
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
        }
        if player(&mut app).is_some() && run(&app).phase == RunPhase::Playing {
            let sector = run(&app).sector;
            clear_sector(&mut app, sector);
            // A stray bullet may destroy the bot on the very tick that clears the sector.
            if player(&mut app).is_some() {
                cross_barrier(&mut app);
            }
        }
    }
    app
}

#[test]
fn runs_play_through_many_sectors() {
    for seed in ["ALPHA", "BRAVO", "CHARLIE"] {
        let mut app = play(seed, 18);
        let state = run(&app);
        assert!(
            state.phase == RunPhase::Over || state.sector >= 18,
            "seed {seed}: {state:?}"
        );
        // Whatever happened, only the sectors around the player are still loaded.
        let field = app.world().resource::<Field>();
        assert!(field.loaded.len() <= 3);
        let loaded = field.loaded.clone();
        let world = app.world_mut();
        for sector in world.query::<&InSector>().iter(world) {
            assert!(loaded.contains(&sector.0) || loaded.contains(&(sector.0.saturating_sub(1))));
        }
    }
}

#[test]
fn the_same_seed_and_the_same_inputs_give_the_same_run() {
    let mut first = play("DELTA", 6);
    let mut second = play("DELTA", 6);
    assert_eq!(run(&first).ticks, run(&second).ticks);
    assert_eq!(fingerprint(&mut first), fingerprint(&mut second));
    let mut other = play("ECHO", 6);
    assert_ne!(fingerprint(&mut first), fingerprint(&mut other));
}

/// The tank of a seat, in a run of several players.
fn seat(app: &mut App, seat: u8) -> Entity {
    let world = app.world_mut();
    let mut tanks = world.query::<(Entity, &Player)>();
    let found = tanks.iter(world).find(|(_, player)| player.seat == seat);
    found.unwrap().0
}

/// What the player of a seat asks of their tank from now on.
fn order(app: &mut App, entity: Entity, command: TankCommand) {
    *app.world_mut().get_mut::<TankCommand>(entity).unwrap() = command;
}

/// Destroys a tank, as a blast would.
fn destroy(app: &mut App, entity: Entity) {
    let mut casualties = app.world_mut().resource_mut::<Casualties>();
    casualties.tanks.push(entity);
    tick(app, 1);
}

fn is_wreck(app: &App, entity: Entity) -> bool {
    app.world().get::<Wreck>(entity).is_some()
}

fn barrier(app: &mut App, sector: u32) -> Barrier {
    let world = app.world_mut();
    let mut barriers = world.query::<&Barrier>();
    *barriers
        .iter(world)
        .find(|barrier| barrier.sector == sector)
        .unwrap()
}

/// Puts a tank in front of the barrier of a sector, or just past it.
fn carry(app: &mut App, entity: Entity, sector: u32, past: bool) {
    let barrier = barrier(app, sector);
    let middle = (barrier.from + barrier.to) / 2.0;
    let along = (barrier.to - barrier.from).normalize();
    // Tanks stand side by side there, one seat next to the other.
    let seat = app.world().get::<Player>(entity).unwrap().seat as f32;
    let side = along * (seat - 1.5) * 2.0;
    let depth = if past { 2.0 } else { -1.0 };
    teleport(app, entity, middle + side + barrier.outward * depth);
}

#[test]
fn several_players_start_side_by_side_against_larger_groups() {
    let mut app = team("TEST", 2);
    let state = run(&app);
    assert_eq!((state.players, state.solo, state.sector), (2, false, 0));
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    assert_eq!(position(&mut app, first), START + Vec2::new(-1.25, 0.0));
    assert_eq!(position(&mut app, second), START + Vec2::new(1.25, 0.0));
    // One enemy waits for a single player in sector 1, and two for two players.
    assert_eq!(enemies(&mut app, 1).len(), 2);

    let mut app = team("TEST", 4);
    let places: Vec<Vec2> = (0..4)
        .map(|number| {
            let entity = seat(&mut app, number);
            position(&mut app, entity) - START
        })
        .collect();
    let rows = [(-1.25, 0.0), (1.25, 0.0), (-1.25, -2.5), (1.25, -2.5)];
    assert_eq!(places, rows.map(|(x, y)| Vec2::new(x, y)));
    assert_eq!(enemies(&mut app, 1).len(), 3);
    // The same map, whoever plays.
    let walls = |app: &mut App| {
        let world = app.world_mut();
        let mut sectors = world.query::<&Sector>();
        let mut walls: Vec<_> = sectors
            .iter(world)
            .map(|sector| (sector.layout.plan.index, sector.layout.plan.walls.clone()))
            .collect();
        walls.sort_by_key(|(index, _)| *index);
        walls
    };
    assert_eq!(walls(&mut app), walls(&mut start("TEST")));

    // A run has four players at most.
    let mut app = team("TEST", 9);
    assert_eq!((run(&app).players, players(&mut app)), (4, 4));
}

#[test]
fn a_destroyed_tank_of_several_players_becomes_a_wreck() {
    let mut app = team("TEST", 2);
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    let place = position(&mut app, second);
    destroy(&mut app, second);
    let state = run(&app);
    assert_eq!((state.phase, state.lives), (RunPhase::Playing, SOLO_LIVES));
    assert!(is_wreck(&app, second) && !is_wreck(&app, first));
    let destroyed = SimEvent::TankDestroyed {
        at: place,
        team: Team::Players,
    };
    assert_eq!(count(&events(&mut app), |event| *event == destroyed), 1);

    // The wreck obeys nobody.
    let orders = TankCommand {
        drive: Vec2::Y,
        aim: place + Vec2::Y * 9.0,
        fire: true,
        mine: true,
        shield: true,
        repair: false,
    };
    order(&mut app, second, orders);
    tick(&mut app, seconds(1.0));
    assert_eq!(position(&mut app, second), place);
    assert_eq!(bullets(&mut app), 0);
    assert!(!app.world().get::<Shield>(second).unwrap().is_active());

    // The bullets of both sides fly over it, and nothing destroys it again.
    for team in [Team::Enemies, Team::Players] {
        let bullet = spawn_bullet(&mut app, team, place + Vec2::Y * 4.0, Vec2::NEG_Y * 10.0, 0);
        tick(&mut app, seconds(0.6));
        let past = place.y - position(&mut app, bullet).y;
        assert!(alive(&app, bullet) && past > 1.5, "{past} m past the wreck");
        tick(&mut app, seconds(1.0));
        assert_eq!(bullets(&mut app), 0);
    }
    assert!(is_wreck(&app, second));
    destroy(&mut app, second);
    let told = events(&mut app);
    assert_eq!(
        count(&told, |event| matches!(
            event,
            SimEvent::TankDestroyed { .. }
        )),
        0
    );
    assert_eq!(run(&app).phase, RunPhase::Playing);

    // It is in nobody's way: a tank that drives straight at it drives over it, and the wreck
    // stays.
    teleport(&mut app, first, place + Vec2::NEG_X * 4.0);
    app.world_mut().get_mut::<Motion>(first).unwrap().hull = 0.0;
    order(
        &mut app,
        first,
        TankCommand {
            drive: Vec2::X,
            ..default()
        },
    );
    let mut nearest = f32::MAX;
    for _ in 0..seconds(1.5) {
        tick(&mut app, 1);
        nearest = nearest.min(position(&mut app, first).distance(place));
    }
    assert_eq!(position(&mut app, second), place);
    assert!(nearest < 0.1, "the tank came no nearer than {nearest} m");
    let beyond = position(&mut app, first).x - place.x;
    assert!(beyond > 2.0 * TANK_RADIUS, "{beyond} m beyond the wreck");
}

#[test]
fn a_tank_that_comes_back_under_another_pushes_it_aside_without_a_jump() {
    let mut app = team("TEST", 2);
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    destroy(&mut app, second);
    let place = position(&mut app, second);
    // A teammate repairs the wreck from right on top of it.
    let on_top = place + Vec2::X * 0.1;
    teleport(&mut app, first, on_top);
    let hold = TankCommand {
        repair: true,
        ..default()
    };
    order(&mut app, first, hold);
    tick(&mut app, seconds(REPAIR_TIME) - 2);
    assert!(is_wreck(&app, second));
    assert_eq!(position(&mut app, first), on_top);

    // The tank is back under the other one. They part over a few ticks, each a little at a
    // time, until they stand side by side.
    let places = |app: &mut App| [position(app, first), position(app, second)];
    let mut last = places(&mut app);
    let mut parting = 0;
    for _ in 0..30 {
        tick(&mut app, 1);
        let now = places(&mut app);
        for tank in 0..2 {
            let moved = now[tank].distance(last[tank]);
            assert!(moved <= TANK_PUSH / 2.0 + 1e-4, "a jump of {moved} m");
        }
        parting += u32::from(now != last);
        last = now;
    }
    assert!(!is_wreck(&app, second));
    assert!(parting >= 5, "they parted in {parting} ticks");
    let gap = last[0].distance(last[1]);
    assert!((gap - 2.0 * TANK_RADIUS).abs() < 1e-3, "{gap} m apart");
}

#[test]
fn tanks_that_run_stay_solid_to_each_other() {
    // No two tanks close in on each other by more in a tick than they are moved apart.
    let mut fastest = Genome::empty(0);
    fastest.set(Stat::Engine, Stat::Engine.max_level());
    assert!(2.0 * fastest.top_speed() / (TICK_RATE as f32) < TANK_PUSH);

    // A tank that drives into another one pushes it along, and never gets into it.
    let mut app = team("TEST", 2);
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    let place = position(&mut app, second);
    teleport(&mut app, first, place + Vec2::NEG_X * 3.0);
    app.world_mut().get_mut::<Motion>(first).unwrap().hull = 0.0;
    order(
        &mut app,
        first,
        TankCommand {
            drive: Vec2::X,
            ..default()
        },
    );
    for _ in 0..seconds(1.0) {
        tick(&mut app, 1);
        let gap = position(&mut app, first).distance(position(&mut app, second));
        assert!(gap > 2.0 * TANK_RADIUS - 1e-3, "{gap} m apart");
    }
    assert!(position(&mut app, second).x > place.x + 0.5);
}

#[test]
fn a_teammate_repairs_a_wreck_in_two_seconds() {
    let mut app = team("TEST", 2);
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    destroy(&mut app, second);
    events(&mut app);
    let progress = |app: &App| app.world().get::<Wreck>(second).map(|wreck| wreck.repair);
    let hold = TankCommand {
        repair: true,
        ..default()
    };

    // From too far, nothing happens.
    let wreck_at = position(&mut app, second);
    teleport(
        &mut app,
        first,
        wreck_at + Vec2::NEG_X * (REPAIR_RADIUS + 0.5),
    );
    order(&mut app, first, hold);
    tick(&mut app, seconds(1.0));
    assert_eq!(progress(&app), Some(0.0));

    // From close by, the repair goes on for as long as the key is held, and starts over
    // when it is let go.
    teleport(
        &mut app,
        first,
        wreck_at + Vec2::NEG_X * (REPAIR_RADIUS - 0.5),
    );
    tick(&mut app, seconds(1.0));
    assert!((progress(&app).unwrap() - 1.0).abs() < 0.05);
    order(&mut app, first, TankCommand::default());
    tick(&mut app, 1);
    assert_eq!(progress(&app), Some(0.0));

    // After two seconds the tank is back, with a full magazine.
    order(&mut app, first, hold);
    tick(&mut app, seconds(REPAIR_TIME) - 2);
    assert!(is_wreck(&app, second));
    tick(&mut app, 3);
    assert!(!is_wreck(&app, second));
    let rounds = app.world().get::<Gun>(second).unwrap().rounds;
    assert_eq!(rounds, Genome::PLAYER.magazine());
    let repaired = SimEvent::TankRepaired { at: wreck_at };
    assert_eq!(count(&events(&mut app), |event| *event == repaired), 1);

    // Nothing destroys it for a second: a bullet ends on it, and a blast leaves it alone.
    assert!(app.world().get::<Protected>(second).is_some());
    spawn_bullet(
        &mut app,
        Team::Enemies,
        wreck_at + Vec2::Y * 2.0,
        Vec2::NEG_Y * 10.0,
        0,
    );
    tick(&mut app, seconds(0.4));
    destroy(&mut app, second);
    assert_eq!(bullets(&mut app), 0);
    assert!(!is_wreck(&app, second));
    tick(&mut app, seconds(REPAIR_PROTECTION));
    assert!(app.world().get::<Protected>(second).is_none());
    destroy(&mut app, second);
    assert!(is_wreck(&app, second));
}

#[test]
fn a_tank_that_repairs_keeps_its_hands_free() {
    let mut app = team("TEST", 2);
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    destroy(&mut app, second);
    let wreck_at = position(&mut app, second);
    let from = wreck_at + Vec2::NEG_X * 2.5;
    teleport(&mut app, first, from);
    events(&mut app);

    // While it repairs, the tank drives, shoots, lays a mine and shields as it always does.
    let busy = TankCommand {
        drive: Vec2::Y,
        aim: wreck_at + Vec2::Y * 9.0,
        fire: true,
        mine: true,
        shield: true,
        repair: true,
    };
    order(&mut app, first, busy);
    tick(&mut app, seconds(0.3));
    assert!(position(&mut app, first).distance(from) > 0.5);
    order(
        &mut app,
        first,
        TankCommand {
            drive: Vec2::ZERO,
            ..busy
        },
    );
    tick(&mut app, seconds(0.7));
    let told = events(&mut app);
    let shots = count(&told, |event| matches!(event, SimEvent::Shot { .. }));
    let mines = count(&told, |event| matches!(event, SimEvent::MinePlaced { .. }));
    let bubbles = count(&told, |event| {
        matches!(event, SimEvent::ShieldRaised { .. })
    });
    assert!(shots >= 3, "{shots} shots");
    assert_eq!((mines, bubbles), (2, 1));
    // The repair went on all the while.
    let repair = app.world().get::<Wreck>(second).unwrap().repair;
    assert!((repair - 1.0).abs() < 0.05, "{repair} s of repair");
}

/// How far the repair of the tank of a seat has got, and which seats are at it, if the tank
/// is a wreck.
fn repair_of(app: &App, tank: Entity) -> Option<(f32, u8)> {
    let wreck = app.world().get::<Wreck>(tank)?;
    Some((wreck.repair, wreck.repairers))
}

#[test]
fn a_tank_repairs_one_wreck_at_a_time() {
    let mut app = team("TEST", 3);
    let [first, second, third] = [0, 1, 2].map(|number| seat(&mut app, number));
    // Two tanks are destroyed two and a half meters from each other, and the third stands
    // between them, within reach of both and nearer to one.
    let middle = position(&mut app, first);
    let (left, right) = (middle + Vec2::NEG_X, middle + Vec2::X * 1.5);
    teleport(&mut app, second, left);
    teleport(&mut app, third, right);
    destroy(&mut app, second);
    destroy(&mut app, third);
    assert_eq!(run(&app).phase, RunPhase::Playing);
    let hold = TankCommand {
        repair: true,
        ..default()
    };
    order(&mut app, first, hold);
    tick(&mut app, seconds(1.0));
    // Only the nearer one is being repaired, by the player of the first seat.
    let (repair, repairers) = repair_of(&app, second).unwrap();
    assert!((repair - 1.0).abs() < 0.05, "{repair} s of repair");
    assert_eq!(repairers, 0b001);
    assert!(app.world().get::<Wreck>(second).unwrap().repaired_by(0));
    assert_eq!(repair_of(&app, third), Some((0.0, 0)));

    // The tank stays with the wreck it is at, though it drives nearer to the other one.
    teleport(&mut app, first, middle + Vec2::X);
    tick(&mut app, seconds(0.5));
    let (repair, _) = repair_of(&app, second).unwrap();
    assert!((repair - 1.5).abs() < 0.05, "{repair} s of repair");
    assert_eq!(repair_of(&app, third), Some((0.0, 0)));

    // Once that tank is back, the same hold starts on the other wreck, from nothing.
    tick(&mut app, seconds(0.5) + 2);
    assert!(!is_wreck(&app, second));
    let (repair, repairers) = repair_of(&app, third).unwrap();
    assert!(repair > 0.0 && repair < 0.1, "{repair} s of repair");
    assert_eq!(repairers, 0b001);
    tick(&mut app, seconds(REPAIR_TIME) - 6);
    assert!(is_wreck(&app, third));
    tick(&mut app, 6);
    assert!(!is_wreck(&app, third));
}

#[test]
fn a_repair_that_loses_its_wreck_moves_on_to_another_and_starts_over() {
    let mut app = team("TEST", 3);
    let [first, second, third] = [0, 1, 2].map(|number| seat(&mut app, number));
    let middle = position(&mut app, first);
    // Two wrecks five meters apart: a tank between them can leave the reach of one and
    // stay within the reach of the other.
    let (left, right) = (middle + Vec2::NEG_X * 2.0, middle + Vec2::X * 3.0);
    teleport(&mut app, second, left);
    teleport(&mut app, third, right);
    destroy(&mut app, second);
    destroy(&mut app, third);
    let hold = TankCommand {
        repair: true,
        ..default()
    };
    order(&mut app, first, hold);
    tick(&mut app, seconds(1.0));
    assert!(repair_of(&app, second).unwrap().0 > 0.9);
    assert_eq!(repair_of(&app, third), Some((0.0, 0)));
    // Out of reach of the one it was at, the tank is at the other, and the first repair is
    // lost.
    teleport(&mut app, first, right + Vec2::NEG_X);
    tick(&mut app, 2);
    assert_eq!(repair_of(&app, second), Some((0.0, 0)));
    let (repair, repairers) = repair_of(&app, third).unwrap();
    assert!(repair > 0.0 && repair < 0.1, "{repair} s of repair");
    assert_eq!(repairers, 0b001);
    // Letting the key go ends that one too.
    order(&mut app, first, TankCommand::default());
    tick(&mut app, 1);
    assert_eq!(repair_of(&app, third), Some((0.0, 0)));
}

#[test]
fn two_tanks_at_one_wreck_repair_it_no_faster_than_one() {
    let mut app = team("TEST", 3);
    let [first, second, third] = [0, 1, 2].map(|number| seat(&mut app, number));
    destroy(&mut app, third);
    let wreck_at = position(&mut app, third);
    teleport(&mut app, first, wreck_at + Vec2::NEG_X * 2.0);
    teleport(&mut app, second, wreck_at + Vec2::Y * 2.0);
    let hold = TankCommand {
        repair: true,
        ..default()
    };
    order(&mut app, first, hold);
    order(&mut app, second, hold);
    tick(&mut app, seconds(1.0));
    let (repair, repairers) = repair_of(&app, third).unwrap();
    assert!((repair - 1.0).abs() < 0.05, "{repair} s of repair");
    assert_eq!(repairers, 0b011);
    // One of them lets go, and the repair goes on with the other.
    order(&mut app, first, TankCommand::default());
    tick(&mut app, seconds(0.5));
    let (repair, repairers) = repair_of(&app, third).unwrap();
    assert!((repair - 1.5).abs() < 0.05, "{repair} s of repair");
    assert_eq!(repairers, 0b010);
    tick(&mut app, seconds(0.5) + 2);
    assert!(!is_wreck(&app, third));
}

#[test]
fn the_run_of_several_players_ends_when_every_tank_is_a_wreck() {
    let mut app = team("TEST", 3);
    for number in 0..2 {
        let entity = seat(&mut app, number);
        destroy(&mut app, entity);
        assert_eq!(run(&app).phase, RunPhase::Playing);
    }
    let last = seat(&mut app, 2);
    destroy(&mut app, last);
    assert_eq!(run(&app).phase, RunPhase::Over);
    assert_eq!(
        count(&events(&mut app), |event| *event == SimEvent::RunOver),
        1
    );
    // The simulation stops with the run.
    let ticks = run(&app).ticks;
    tick(&mut app, 10);
    assert_eq!(run(&app).ticks, ticks);

    // Two tanks that are destroyed at once end it too.
    let mut app = team("TEST", 2);
    let both = [seat(&mut app, 0), seat(&mut app, 1)];
    let mut casualties = app.world_mut().resource_mut::<Casualties>();
    casualties.tanks.extend(both);
    tick(&mut app, 1);
    assert_eq!(run(&app).phase, RunPhase::Over);
}

#[test]
fn a_teammate_s_bullet_wrecks_a_tank() {
    let mut app = team("TEST", 2);
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    let target = position(&mut app, second);
    order(
        &mut app,
        first,
        TankCommand {
            aim: target,
            ..default()
        },
    );
    tick(&mut app, seconds(0.3));
    order(
        &mut app,
        first,
        TankCommand {
            aim: target,
            fire: true,
            ..default()
        },
    );
    tick(&mut app, seconds(0.2));
    assert!(is_wreck(&app, second) && !is_wreck(&app, first));
    let blow = run(&app).lost_to.unwrap();
    assert_eq!((blow.weapon, blow.team), (Weapon::Bullet, Team::Players));
}

#[test]
fn a_barrier_waits_for_every_running_tank_and_each_player_crosses_alone() {
    let mut app = team("TEST", 2);
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    // The start bay has no enemy, but its barrier stays locked until both players are close.
    carry(&mut app, first, 0, false);
    tick(&mut app, 2);
    assert!(!barrier(&mut app, 0).unlocked);
    let far = position(&mut app, second);
    let reach = (barrier(&mut app, 0).from.y - far.y).abs();
    assert!(reach > BARRIER_GATHER_DISTANCE);
    carry(&mut app, second, 0, false);
    tick(&mut app, 2);
    assert!(barrier(&mut app, 0).unlocked);

    // The first player through wakes the enemies. The other one is still in the bay, which
    // stays loaded for it.
    let sector_of = |app: &App, entity| app.world().get::<Tank>(entity).unwrap().sector;
    carry(&mut app, first, 0, true);
    tick(&mut app, 1);
    assert_eq!(run(&app).sector, 1);
    assert_eq!((sector_of(&app, first), sector_of(&app, second)), (1, 0));
    assert!(
        enemies(&mut app, 1)
            .iter()
            .all(|&enemy| app.world().get::<Enemy>(enemy).unwrap().awake)
    );
    assert_eq!(loaded(&app), [0, 1, 2]);
    carry(&mut app, second, 0, true);
    tick(&mut app, 1);
    assert_eq!((sector_of(&app, second), loaded(&app)), (1, vec![0, 1, 2]));
    let entered = SimEvent::SectorEntered { sector: 1 };
    assert_eq!(count(&events(&mut app), |event| *event == entered), 1);

    // A wreck holds no barrier back. When the last running tank has crossed, the wreck is
    // towed across and the sectors further back are unloaded.
    clear_sector(&mut app, 1);
    destroy(&mut app, second);
    carry(&mut app, first, 1, false);
    tick(&mut app, 2);
    assert!(barrier(&mut app, 1).unlocked);
    carry(&mut app, first, 1, true);
    tick(&mut app, 1);
    assert_eq!((run(&app).sector, loaded(&app)), (2, vec![1, 2, 3]));
    assert!(is_wreck(&app, second));
    assert_eq!(sector_of(&app, second), 2);
    let towed = position(&mut app, second);
    let gate = barrier(&mut app, 1);
    let depth = (towed - gate.from).dot(gate.outward);
    assert!(depth > TANK_RADIUS && depth < 6.0, "{depth}");
    // It can be repaired there, once the enemies of that sector leave the repairer alone.
    clear_sector(&mut app, 2);
    teleport(&mut app, first, towed + gate.outward * 2.0);
    order(
        &mut app,
        first,
        TankCommand {
            repair: true,
            ..default()
        },
    );
    tick(&mut app, seconds(REPAIR_TIME) + 1);
    assert!(!is_wreck(&app, second));
}

#[test]
fn an_enemy_fights_the_nearest_player_whose_tank_still_runs() {
    let mut app = team("TEST", 2);
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    teleport(&mut app, first, Vec2::new(-5.0, 3.0));
    teleport(&mut app, second, Vec2::new(5.0, 9.0));
    let at = Vec2::new(4.0, 13.0);
    let enemy = spawn_awake(&mut app, at, &[(Stat::Turret, 6), (Stat::Brain, 1)]);
    let aims_at = |app: &mut App, target: Entity| {
        let aim = app.world().get::<TankCommand>(enemy).unwrap().aim;
        let wanted = position(app, target) - at;
        (aim - at).angle_to(wanted).abs() < 0.2
    };
    tick(&mut app, seconds(0.5));
    assert!(aims_at(&mut app, second));
    // A wreck is no target.
    destroy(&mut app, second);
    tick(&mut app, seconds(0.5));
    assert!(aims_at(&mut app, first));
}

#[test]
fn a_player_who_leaves_takes_their_tank_and_later_groups_shrink() {
    let mut app = team("TEST", 4);
    assert_eq!(enemies(&mut app, 1).len(), 3);
    for number in [2, 3] {
        app.world_mut()
            .write_message(RunRequest::Leave { seat: number });
    }
    // A request that comes twice counts once.
    app.world_mut().write_message(RunRequest::Leave { seat: 3 });
    tick(&mut app, 1);
    assert_eq!((run(&app).players, players(&mut app)), (2, 2));
    assert_eq!(run(&app).phase, RunPhase::Playing);

    // The sectors that are loaded keep their groups. The next ones are for two players.
    let (first, second) = (seat(&mut app, 0), seat(&mut app, 1));
    for entity in [first, second] {
        carry(&mut app, entity, 0, false);
    }
    tick(&mut app, 2);
    for entity in [first, second] {
        carry(&mut app, entity, 0, true);
    }
    tick(&mut app, 1);
    assert_eq!(run(&app).sector, 1);
    assert_eq!(enemies(&mut app, 1).len(), 3);
    assert_eq!(enemies(&mut app, 2).len(), 2);

    // When the last running tank leaves, the run is over.
    destroy(&mut app, first);
    app.world_mut().write_message(RunRequest::Leave { seat: 1 });
    tick(&mut app, 1);
    assert_eq!(run(&app).phase, RunPhase::Over);

    // A single player doesn't leave: the run is abandoned instead.
    let mut app = start("TEST");
    app.world_mut().write_message(RunRequest::Leave { seat: 0 });
    tick(&mut app, 2);
    assert_eq!((run(&app).phase, players(&mut app)), (RunPhase::Playing, 1));
}

#[test]
fn a_playtest_brings_several_players_back_together() {
    let mut app = team("TEST", 3);
    app.insert_resource(Playtest {
        infinite_lives: true,
    });
    let wrecked = seat(&mut app, 2);
    destroy(&mut app, wrecked);
    skip_to(&mut app, 5);
    assert_eq!((run(&app).sector, players(&mut app)), (4, 3));
    let world = app.world_mut();
    assert_eq!(world.query::<&Wreck>().iter(world).count(), 0);
    let mut tanks = world.query::<(&Player, &Tank)>();
    assert!(tanks.iter(world).all(|(_, tank)| tank.sector == 4));
}

#[test]
fn the_same_seed_and_inputs_give_the_same_run_of_several_players() {
    let played = |seed: &str| {
        let mut app = team(seed, 2);
        for step in 0..seconds(12.0) {
            for number in 0..2u8 {
                let entity = seat(&mut app, number);
                let turn = step as f32 * 0.03 + number as f32 * 2.0;
                let at = position(&mut app, entity);
                order(
                    &mut app,
                    entity,
                    TankCommand {
                        drive: Vec2::from_angle(turn),
                        aim: at + Vec2::from_angle(-turn) * 8.0,
                        fire: step % 40 == 0,
                        mine: step % 211 == 0,
                        shield: step % 307 == 0,
                        repair: step % 2 == 0,
                    },
                );
            }
            tick(&mut app, 1);
        }
        fingerprint(&mut app)
    };
    assert_eq!(played("ECHO"), played("ECHO"));
    assert_ne!(played("ECHO"), played("FOXTROT"));
}

/// What was touched during a tick: guests of a game are told of it, changed or not.
#[derive(Resource, Default)]
struct Touched {
    guns: usize,
    shields: usize,
}

fn note_what_was_touched(
    mut touched: ResMut<Touched>,
    guns: Query<(), Changed<Gun>>,
    shields: Query<(), Changed<Shield>>,
) {
    touched.guns += guns.iter().count();
    touched.shields += shields.iter().count();
}

#[test]
fn a_gun_and_a_shield_that_do_not_change_are_left_alone() {
    // Two players and the dormant enemies of the first sector, all at rest.
    let mut app = team("TANKU", 2);
    app.init_resource::<Touched>()
        .add_systems(FixedPostUpdate, note_what_was_touched);
    let step = |app: &mut App, ticks: u32| {
        for _ in 0..ticks {
            app.world_mut().run_schedule(FixedUpdate);
            app.world_mut().run_schedule(FixedPostUpdate);
        }
        std::mem::take(&mut *app.world_mut().resource_mut::<Touched>())
    };
    // Whatever still ran down after the start has run down by now.
    step(&mut app, seconds(1.0));
    let idle = step(&mut app, seconds(2.0));
    assert_eq!((idle.guns, idle.shields), (0, 0));

    // A shot and a raised shield are changes, and so is what follows them: the reload, and
    // the cooldown of the shield.
    command(
        &mut app,
        TankCommand {
            aim: START + Vec2::new(-2.0, 9.0),
            fire: true,
            shield: true,
            ..default()
        },
    );
    let busy = step(&mut app, 1);
    assert!(busy.guns >= 1 && busy.shields >= 1, "{} guns", busy.guns);
    command(&mut app, TankCommand::default());
    // Once the round is back and the shield is ready again, nothing is touched any more.
    step(&mut app, seconds(11.0));
    let idle = step(&mut app, seconds(2.0));
    assert_eq!((idle.guns, idle.shields), (0, 0));
}
