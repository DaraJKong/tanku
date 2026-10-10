//! Tests of the rules, played on the real simulation without a window.

use bevy::{ecs::message::Messages, prelude::*};
use tanku_core::{
    CorePlugin,
    rng::{Purpose, Stream},
    rules::{
        BARRIER_GATHER_DISTANCE, PLAYTEST_LAST_SECTOR, RESPAWN_DELAY, SHIELD_DURATION, SOLO_LIVES,
        SPAWN_FIRE_DELAY, TICK_RATE,
    },
    sim::{
        Barrier, Block, Blow, Bullet, Enemy, ExplosiveCrate, Flag, Gun, Mine, MineState, Motion,
        Player, Playtest, Position, PreviousPosition, Run, RunPhase, RunRequest, Sector, Shield,
        SimEvent, Tank, TankCommand, Team, Weapon,
    },
    stats::{Genome, Stat},
};

use crate::{
    Casualties, Maker, ServerPlugin,
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

/// Starts a run. The tank has just appeared and can't shoot yet.
fn begin(seed: &str) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
    app.world_mut()
        .write_message(RunRequest::Start { seed: seed.into() });
    tick(&mut app, 1);
    events(&mut app);
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
    // Close to the barrier, and out of reach of a flag.
    teleport(app, entity, middle - barrier.outward);
    tick(app, 1);
    teleport(app, entity, middle + barrier.outward * 2.0);
    tick(app, 1);
}

/// Where the flag of a sector stands, if it is loaded and has one.
fn flag(app: &mut App, sector: u32) -> Option<(Flag, Vec2)> {
    let world = app.world_mut();
    world
        .query::<(&Flag, &Position)>()
        .iter(world)
        .find(|(flag, _)| flag.sector == sector)
        .map(|(flag, position)| (*flag, position.0))
}

/// Drives the player onto the flag of the sector it is in.
fn touch_flag(app: &mut App) {
    let sector = run(app).sector;
    let (_, at) = flag(app, sector).unwrap();
    let entity = player(app).unwrap();
    teleport(app, entity, at + Vec2::X);
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
    let mut told = events(&mut app);
    assert_eq!(
        count(&told, |event| matches!(
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
    // The game told of the shield going up, and of its overload, once each, where the tank is.
    told.extend(events(&mut app));
    let raised = SimEvent::ShieldRaised { at: START };
    let overloaded = SimEvent::ShieldOverloaded { at: START };
    assert_eq!(count(&told, |event| *event == raised), 1);
    assert_eq!(count(&told, |event| *event == overloaded), 1);
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
fn a_mine_warns_only_when_an_opposing_tank_sets_it_off() {
    let warnings = |events: &[SimEvent]| {
        let near = |at: &Vec2| at.distance(START) < 0.1;
        count(
            events,
            |event| matches!(event, SimEvent::MineTriggered { at } if near(at)),
        )
    };
    let blasts =
        |events: &[SimEvent]| count(events, |event| matches!(event, SimEvent::Blast { .. }));
    // The player lays a mine where the run starts, and waits at a safe distance.
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

    // An enemy comes close: the warning sounds once, at the mine, and the blast follows 0.6 s
    // later.
    let mut app = lay();
    let enemy = spawn_enemy(&mut app, START + Vec2::X * 1.5, &[]);
    app.world_mut().get_mut::<Enemy>(enemy).unwrap().awake = true;
    tick(&mut app, seconds(0.5));
    let heard = events(&mut app);
    assert_eq!((warnings(&heard), blasts(&heard)), (1, 0));
    tick(&mut app, seconds(0.2));
    let heard = events(&mut app);
    assert_eq!((warnings(&heard), blasts(&heard)), (0, 1));

    // The fuse runs out: the mine blows without a warning.
    let mut app = lay();
    tick(&mut app, seconds(10.0));
    let heard = events(&mut app);
    assert_eq!((warnings(&heard), blasts(&heard)), (0, 1));

    // A bullet hits it: no warning either.
    let mut app = lay();
    shoot_at(&mut app, START);
    tick(&mut app, seconds(0.8));
    let heard = events(&mut app);
    assert_eq!((warnings(&heard), blasts(&heard)), (0, 1));
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
    let sentry = spawn_veteran(&mut app, Vec2::new(0.0, 13.0), &levels);
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
    let sentry = spawn_veteran(&mut app, Vec2::new(0.0, 13.0), &[(Stat::Turret, 6)]);
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
fn a_flag_is_captured_by_driving_up_to_it_once_its_sector_is_cleared() {
    let mut app = start("TEST");
    assert!(flag(&mut app, 0).is_none(), "the start bay has no flag");
    cross_barrier(&mut app);
    let (state, at) = flag(&mut app, 1).unwrap();
    assert!(!state.captured);
    let barrier = {
        let world = app.world_mut();
        *world
            .query::<&Barrier>()
            .iter(world)
            .find(|barrier| barrier.sector == 1)
            .unwrap()
    };
    let to_barrier = tanku_core::geom::distance_to_segment(at, barrier.from, barrier.to);
    assert_eq!(to_barrier, 4.0);

    // With an enemy left in the sector, the flag stays as it is.
    touch_flag(&mut app);
    assert!(!flag(&mut app, 1).unwrap().0.captured);
    assert_eq!(run(&app).checkpoint, 0);

    // Clearing the sector is not enough either: the player has to come to the flag.
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, at - barrier.outward * 10.0);
    clear_sector(&mut app, 1);
    assert_eq!(run(&app).checkpoint, 0);
    events(&mut app);
    touch_flag(&mut app);
    assert!(flag(&mut app, 1).unwrap().0.captured);
    assert_eq!(run(&app).checkpoint, 1);
    let captured = count(&events(&mut app), |event| {
        matches!(event, SimEvent::FlagCaptured { .. })
    });
    assert_eq!(captured, 1);
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
fn the_player_comes_back_at_the_last_flag_captured_three_times_at_most() {
    let mut app = start("TEST");
    cross_barrier(&mut app);
    clear_sector(&mut app, 1);
    touch_flag(&mut app);
    cross_barrier(&mut app);
    let before = enemies(&mut app, 2).len();
    assert!(before > 0);

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
        "the sector of the flag stays cleared"
    );
    let (state, at) = flag(&mut app, 1).unwrap();
    assert!(state.captured, "the flag is still captured");
    let entity = player(&mut app).unwrap();
    assert!(position(&mut app, entity).distance(at) < 3.0);
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
fn a_flag_left_behind_is_no_checkpoint() {
    let mut app = start("TEST");
    cross_barrier(&mut app);
    clear_sector(&mut app, 1);
    cross_barrier(&mut app);
    assert_eq!(run(&app).sector, 2);

    destroy_player(&mut app);
    tick(&mut app, seconds(RESPAWN_DELAY) + 1);
    let state = run(&app);
    assert_eq!((state.sector, state.checkpoint), (0, 0));
    let entity = player(&mut app).unwrap();
    assert_eq!(position(&mut app, entity), START);
    assert_eq!(enemies(&mut app, 1).len(), 1, "sector 1 is to fight again");
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
    // The tank stands between the flag and the barrier of the sector before, which is cleared.
    let (state, at) = flag(&mut app, 6).unwrap();
    assert!(state.captured);
    let entity = player(&mut app).unwrap();
    assert!(position(&mut app, entity).distance(at) < 3.0);
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

    // A sector without a flag is skipped to as well: sector 9 has none.
    skip_to(&mut app, 10);
    assert_eq!((run(&app).sector, loaded(&app)), (9, vec![9, 10]));
    assert!(flag(&mut app, 9).is_none());
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
            if player(&mut app).is_some() && flag(&mut app, sector).is_some() {
                touch_flag(&mut app);
            }
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
