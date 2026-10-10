//! Tests of the rules, played on the real simulation without a window.

use bevy::{ecs::message::Messages, prelude::*};
use tanku_core::{
    CorePlugin,
    rng::{Purpose, Stream},
    rules::{BARRIER_GATHER_DISTANCE, RESPAWN_DELAY, SHIELD_DURATION, TICK_RATE},
    sim::{
        Barrier, Block, Bullet, Enemy, Gun, Mine, Motion, Player, Position, PreviousPosition, Run,
        RunPhase, RunRequest, Sector, Shield, SimEvent, Tank, TankCommand, Team,
    },
    stats::{Genome, Stat},
};

use crate::{
    Casualties, ServerPlugin,
    brain::Brain,
    field::{Field, InSector},
};

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

fn start(seed: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
    app.world_mut()
        .write_message(RunRequest::Start { seed: seed.into() });
    tick(&mut app, 1);
    events(&mut app);
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
fn spawn_veteran(app: &mut App, at: Vec2, levels: &[(Stat, u8)]) -> Entity {
    let enemy = spawn_enemy(app, at, levels);
    let brain = Brain::new(Stream::new(1, Purpose::Brain, 0, 0), 60);
    app.world_mut()
        .entity_mut(enemy)
        .insert((brain, Enemy { awake: true }));
    enemy
}

/// Destroys every enemy of a sector, as a player would.
fn clear_sector(app: &mut App, sector: u32) {
    let destroyed = enemies(app, sector);
    app.world_mut()
        .resource_mut::<Casualties>()
        .tanks
        .extend(destroyed);
    tick(app, 1);
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
    teleport(app, entity, middle - barrier.outward * 2.0);
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

    // The first bullet is destroyed and blunts the dart, which flies on as a normal bullet.
    tick(&mut app, seconds(0.25));
    assert!(alive(&app, dart) && !alive(&app, first) && alive(&app, second));
    assert_eq!(app.world().get::<Bullet>(dart).unwrap().pierces_left, 0);
    // The second one destroys it.
    tick(&mut app, seconds(0.25));
    assert!(!alive(&app, dart) && !alive(&app, second));
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
fn a_shield_stops_bullets_then_overloads() {
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
    tick(&mut app, seconds(SHIELD_DURATION) + 1);
    assert_eq!(run(&app).lives, 3);
    assert_eq!(bullets(&mut app), 0);
    let events = events(&mut app);
    assert_eq!(
        count(&events, |event| matches!(
            event,
            SimEvent::ShieldBlocked { .. }
        )),
        1
    );

    // Overloaded for 1.2 s: the tank can't move.
    command(
        &mut app,
        TankCommand {
            drive: Vec2::Y,
            ..default()
        },
    );
    tick(&mut app, seconds(1.0));
    assert!(app.world().get::<Shield>(entity).unwrap().is_overloaded());
    assert_eq!(position(&mut app, entity), START);
    tick(&mut app, seconds(0.4));
    assert!(position(&mut app, entity).y > START.y);

    // The shield stays down for 9 s after it dropped.
    command(
        &mut app,
        TankCommand {
            shield: true,
            ..default()
        },
    );
    tick(&mut app, seconds(7.0));
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
fn clearing_a_sector_scores_and_sets_the_checkpoint() {
    let mut app = start("TEST");
    cross_barrier(&mut app);
    assert_eq!(run(&app).sector, 1);
    clear_sector(&mut app, 1);
    let state = run(&app);
    assert_eq!((state.cleared, state.checkpoint, state.kills), (1, 1, 1));

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
fn the_player_comes_back_at_the_last_checkpoint_three_times_at_most() {
    let mut app = start("TEST");
    cross_barrier(&mut app);
    clear_sector(&mut app, 1);
    cross_barrier(&mut app);
    let before = enemies(&mut app, 2).len();
    assert!(before > 0);

    let destroy_player = |app: &mut App| {
        let entity = player(app).unwrap();
        app.world_mut()
            .resource_mut::<Casualties>()
            .tanks
            .push(entity);
        tick(app, 1);
    };
    destroy_player(&mut app);
    assert_eq!(run(&app).lives, 2);
    tick(&mut app, seconds(RESPAWN_DELAY) + 1);
    let state = run(&app);
    assert_eq!(
        (state.sector, state.cleared, state.phase),
        (1, 1, RunPhase::Playing)
    );
    assert!(
        enemies(&mut app, 1).is_empty(),
        "the checkpoint sector stays cleared"
    );
    let regenerated = enemies(&mut app, 2);
    assert_eq!(regenerated.len(), before);
    assert!(
        regenerated
            .iter()
            .all(|&enemy| !app.world().get::<Enemy>(enemy).unwrap().awake)
    );

    destroy_player(&mut app);
    tick(&mut app, seconds(RESPAWN_DELAY) + 1);
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
    spawn_veteran(
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
    let enemy = spawn_veteran(&mut app, Vec2::new(0.0, 8.0), levels);
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
