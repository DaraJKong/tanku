//! Tests of the special bullets: the ones that seek, split and explode.

use std::f32::consts::FRAC_PI_2;

use tanku_core::{
    rules::{BLAST_RADIUS, BULLET_RADIUS, MARK_TIME, SPLIT_ANGLE, SPLIT_TIME},
    stats::{Ammo, Class},
};

use super::*;
use crate::bullets::{self, Mark};

/// A bullet of a kind in the start bay, with this many bounces left.
fn spawn_special(
    app: &mut App,
    team: Team,
    at: Vec2,
    velocity: Vec2,
    kind: Kind,
    bounces: u8,
) -> Entity {
    let bullet = spawn_bullet(app, team, at, velocity, 0);
    let mut flying = app.world_mut().get_mut::<Bullet>(bullet).unwrap();
    flying.kind = kind;
    flying.bounces_left = bounces;
    bullet
}

/// Every bullet that flies, with where it is.
fn flying(app: &mut App) -> Vec<(Bullet, Vec2)> {
    let world = app.world_mut();
    world
        .query::<(&Bullet, &Position)>()
        .iter(world)
        .map(|(bullet, position)| (*bullet, position.0))
        .collect()
}

/// Where blasts went off since the events were last read.
fn blasts(app: &mut App) -> Vec<Vec2> {
    events(app)
        .iter()
        .filter_map(|event| match event {
            SimEvent::Blast { at } => Some(*at),
            _ => None,
        })
        .collect()
}

/// A tank that a player may build, with this type of bullet and two bounces.
fn tank_with(ammo: Ammo) -> Genome {
    let parts = [(Stat::Engine, 4), (Stat::Ricochet, 2), (Stat::Magazine, 1)];
    let mut tank = Genome::of_the_garage(&parts).unwrap();
    tank.load(ammo);
    assert!(tank.is_a_players());
    tank
}

#[test]
fn a_tank_fires_the_type_of_bullet_that_it_has() {
    for ammo in Ammo::ALL {
        let mut app = begin_as("TEST", 1, Difficulty::Normal, vec![tank_with(ammo)]);
        tick(&mut app, seconds(SPAWN_FIRE_DELAY));
        shoot_at(&mut app, START + Vec2::Y * 5.0);
        let shot = flying(&mut app);
        assert_eq!(shot.len(), 1, "{ammo:?}");
        let (bullet, _) = shot[0];
        assert_eq!(bullet.pierces_left, (ammo == Ammo::Piercing) as u8);
        match (ammo, bullet.kind) {
            // A normal bullet and a piercing one have the bounces of their tank, and so has a
            // seeking one, which seeks at each of them.
            (Ammo::Normal | Ammo::Piercing, Kind::Plain) | (Ammo::Seeking, Kind::Seeking) => {
                assert_eq!(bullet.bounces_left, 2, "{ammo:?}");
            }
            // A splitting bullet keeps them for the two that it becomes, and its time has
            // started.
            (Ammo::Splitting, Kind::Splitting { fuse, bounces }) => {
                assert_eq!((bullet.bounces_left, bounces), (0, 2));
                assert!(fuse < SPLIT_TIME && fuse > SPLIT_TIME - 0.1, "{fuse}");
            }
            // An exploding bullet never bounces.
            (Ammo::Exploding, Kind::Exploding) => assert_eq!(bullet.bounces_left, 0),
            (ammo, kind) => panic!("a tank with {ammo:?} fired {kind:?}"),
        }
    }
}

#[test]
fn a_seeking_bullet_leaves_a_wall_toward_the_tank_that_its_side_fights() {
    let mut app = start("TEST");
    // It flies west, into the wall, and the player is south-east of where it bounces.
    let from = Vec2::new(-4.0, 10.0);
    let bullet = spawn_special(
        &mut app,
        Team::Enemies,
        from,
        Vec2::NEG_X * 9.0,
        Kind::Seeking,
        1,
    );
    tick(&mut app, seconds(0.4));
    let flight = *app.world().get::<Bullet>(bullet).unwrap();
    assert_eq!((flight.bounces_left, flight.bounced), (0, true));
    // It left toward the player's tank, not at the mirror angle, at the speed that it had.
    let bounce = Vec2::new(-7.0 + BULLET_RADIUS, 10.0);
    let toward = (START - bounce).normalize();
    assert!(flight.velocity.normalize().distance(toward) < 1e-3);
    assert!((flight.velocity.length() - 9.0).abs() < 1e-3);
    let ricochets = count(&events(&mut app), |event| {
        matches!(event, SimEvent::Ricochet { .. })
    });
    assert_eq!(ricochets, 1);
    // And it gets there.
    wait_for_the_loss(&mut app, 2.0);

    // A player's bullet seeks the enemies that are awake, never the tank of a player: with
    // none in sight, it bounces as a normal bullet does.
    let mut app = start("TEST");
    let dormant = spawn_enemy(&mut app, Vec2::new(4.0, 13.0), &[]);
    let velocity = Vec2::new(-9.0, 3.0);
    let bullet = spawn_special(&mut app, Team::Players, from, velocity, Kind::Seeking, 1);
    tick(&mut app, seconds(0.4));
    let flight = *app.world().get::<Bullet>(bullet).unwrap();
    assert!(flight.bounced);
    assert!(flight.velocity.distance(Vec2::new(9.0, 3.0)) < 1e-3);
    assert!(alive(&app, dormant));

    // With an enemy that is awake in sight, it leaves toward that one.
    let mut app = start("TEST");
    let awake = spawn_awake(&mut app, Vec2::new(2.0, 13.0), &[]);
    let bullet = spawn_special(&mut app, Team::Players, from, velocity, Kind::Seeking, 1);
    tick(&mut app, seconds(0.4));
    let flight = *app.world().get::<Bullet>(bullet).unwrap();
    let at = position(&mut app, bullet);
    let toward = (Vec2::new(2.0, 13.0) - at).normalize();
    assert!(flight.velocity.normalize().distance(toward) < 0.02);
    tick(&mut app, seconds(1.5));
    assert!(!alive(&app, awake));
}

#[test]
fn a_seeking_bullet_seeks_at_each_bounce_that_it_has_and_never_without_one() {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    let bullet = spawn_special(
        &mut app,
        Team::Enemies,
        Vec2::new(-4.0, 10.0),
        Vec2::NEG_X * 9.0,
        Kind::Seeking,
        2,
    );
    tick(&mut app, seconds(0.4));
    assert_eq!(app.world().get::<Bullet>(bullet).unwrap().bounces_left, 1);
    // The player moves out of its way. It flies on to the south wall, and leaves that one
    // toward where the player is by then.
    let hide = Vec2::new(5.0, 12.0);
    teleport(&mut app, entity, hide);
    for _ in 0..seconds(3.0) {
        if app.world().get::<Bullet>(bullet).unwrap().bounces_left == 0 {
            break;
        }
        tick(&mut app, 1);
    }
    let flight = *app.world().get::<Bullet>(bullet).unwrap();
    let at = position(&mut app, bullet);
    assert_eq!(flight.bounces_left, 0);
    assert!(at.y < 1.0, "it bounced off the south wall, and is at {at}");
    assert!(
        flight
            .velocity
            .normalize()
            .distance((hide - at).normalize())
            < 0.02
    );
    // After its last bounce, it is a bullet that ends on the next wall: the player moves
    // out of its way again.
    teleport(&mut app, entity, Vec2::new(-5.0, 2.0));
    tick(&mut app, seconds(2.0));
    assert!(!alive(&app, bullet));
    assert_eq!(run(&app).lives, 3);

    // A bullet that has no bounce ends on the first wall, whether it seeks or not.
    let bullet = spawn_special(
        &mut app,
        Team::Enemies,
        Vec2::new(-4.0, 10.0),
        Vec2::NEG_X * 9.0,
        Kind::Seeking,
        0,
    );
    tick(&mut app, seconds(0.4));
    assert!(!alive(&app, bullet));
}

#[test]
fn a_splitting_bullet_becomes_two_after_a_second() {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, Vec2::new(-5.0, 2.0));
    let kind = Kind::Splitting {
        fuse: SPLIT_TIME,
        bounces: 2,
    };
    let speed = 9.0;
    let from = Vec2::new(0.0, 2.0);
    let bullet = spawn_special(&mut app, Team::Enemies, from, Vec2::Y * speed, kind, 0);
    tick(&mut app, seconds(SPLIT_TIME) - 2);
    assert!(alive(&app, bullet));
    assert_eq!(bullets(&mut app), 1);
    events(&mut app);
    tick(&mut app, 3);
    assert!(!alive(&app, bullet));
    let split = count(&events(&mut app), |event| {
        matches!(event, SimEvent::BulletSplit { .. })
    });
    assert_eq!(split, 1);
    // Each of the two is a plain bullet with the bounces of the tank, as fast as the bullet
    // was, 30 degrees to one side of its line of flight.
    let halves = flying(&mut app);
    assert_eq!(halves.len(), 2);
    for (half, at) in &halves {
        assert_eq!((half.kind, half.bounces_left), (Kind::Plain, 2));
        assert_eq!((half.pierces_left, half.bounced), (0, false));
        assert!((half.velocity.length() - speed).abs() < 1e-3);
        let off = (half.velocity.to_angle() - FRAC_PI_2).abs();
        assert!((off - SPLIT_ANGLE).abs() < 1e-3, "{off}");
        // The bullet flew for a second before that.
        assert!((at.y - from.y - speed * SPLIT_TIME).abs() < 1.5, "{at}");
    }
    assert!(halves[0].0.velocity.x * halves[1].0.velocity.x < 0.0);
    // They don't destroy each other as they part.
    assert!(halves[0].1.distance(halves[1].1) > 2.0 * BULLET_RADIUS);
    tick(&mut app, seconds(0.3));
    assert_eq!(bullets(&mut app), 2);
}

#[test]
fn a_splitting_bullet_does_not_bounce_before_it_splits() {
    let mut app = start("TEST");
    let kind = Kind::Splitting {
        fuse: SPLIT_TIME,
        bounces: 2,
    };
    let from = Vec2::new(-4.0, 10.0);
    let bullet = spawn_special(&mut app, Team::Enemies, from, Vec2::NEG_X * 9.0, kind, 0);
    tick(&mut app, seconds(0.5));
    // The wall ended it, and nothing came of it.
    assert!(!alive(&app, bullet));
    assert_eq!(bullets(&mut app), 0);
    let news = events(&mut app);
    let bounced = count(&news, |event| matches!(event, SimEvent::Ricochet { .. }));
    let split = count(&news, |event| matches!(event, SimEvent::BulletSplit { .. }));
    assert_eq!((bounced, split), (0, 0));

    // The two that it becomes bounce, since they have the bounces of the tank: one of them
    // meets the wall that the bullet flew along.
    let mut app = start("TEST");
    let kind = Kind::Splitting {
        fuse: 0.01,
        bounces: 1,
    };
    let from = Vec2::new(-7.0 + BULLET_RADIUS + 0.6, 10.0);
    spawn_special(&mut app, Team::Enemies, from, Vec2::Y * 9.0, kind, 0);
    tick(&mut app, 1);
    assert_eq!(flying(&mut app).len(), 2);
    tick(&mut app, seconds(0.2));
    let halves = flying(&mut app);
    assert_eq!(halves.len(), 2);
    let bounced: Vec<&(Bullet, Vec2)> = halves.iter().filter(|(half, _)| half.bounced).collect();
    assert_eq!(bounced.len(), 1);
    assert_eq!(bounced[0].0.bounces_left, 0);
    assert!(bounced[0].0.velocity.x > 0.0);
}

#[test]
fn an_exploding_bullet_goes_off_where_it_ends() {
    let mut app = start("TEST");
    // An enemy stands next to where it ends, and another one out of reach.
    let near = spawn_enemy(&mut app, Vec2::new(5.0, 12.0), &[]);
    let far = spawn_enemy(&mut app, Vec2::new(0.0, 12.0), &[]);
    let bullet = spawn_special(
        &mut app,
        Team::Enemies,
        Vec2::new(3.0, 10.5),
        Vec2::X * 9.0,
        Kind::Exploding,
        0,
    );
    tick(&mut app, seconds(0.6));
    let went_off = blasts(&mut app);
    assert_eq!(went_off.len(), 1);
    let end = Vec2::new(7.0 - BULLET_RADIUS, 10.5);
    assert!(went_off[0].distance(end) < 0.2, "{}", went_off[0]);
    // The blast spares nobody: it destroys what is within 3 m of that place, the tank of an
    // enemy too.
    assert!(Vec2::new(5.0, 12.0).distance(end) < BLAST_RADIUS);
    assert!(!alive(&app, near) && alive(&app, far));
    assert!(!alive(&app, bullet));
    assert_eq!(run(&app).lives, 3);
}

#[test]
fn an_exploding_bullet_goes_off_when_a_bullet_or_a_shield_ends_it() {
    // Two bullets meet north of the player, out of reach of the blast.
    let mut app = start("TEST");
    let at = |x: f32| Vec2::new(x, 10.0);
    let speed = Vec2::X * 9.0;
    spawn_special(&mut app, Team::Enemies, at(-3.0), speed, Kind::Exploding, 0);
    spawn_bullet(&mut app, Team::Players, at(3.0), -speed, 0);
    tick(&mut app, seconds(0.5));
    let went_off = blasts(&mut app);
    assert_eq!(went_off.len(), 1);
    assert!(went_off[0].distance(at(0.0)) < 0.5, "{}", went_off[0]);
    assert_eq!(bullets(&mut app), 0);
    assert_eq!(run(&app).lives, 3);

    // A piercing bullet destroys it and flies on, into the blast.
    let mut app = start("TEST");
    spawn_special(&mut app, Team::Enemies, at(-3.0), speed, Kind::Exploding, 0);
    spawn_bullet(&mut app, Team::Players, at(3.0), -speed, 1);
    tick(&mut app, seconds(0.5));
    assert_eq!(blasts(&mut app).len(), 1);
    assert_eq!(bullets(&mut app), 0);

    // It goes off on a bubble, which protects its tank from the blast.
    let mut app = start("TEST");
    command(
        &mut app,
        TankCommand {
            shield: true,
            ..default()
        },
    );
    tick(&mut app, 1);
    let north = START + Vec2::Y * 3.0;
    spawn_special(
        &mut app,
        Team::Enemies,
        north,
        Vec2::NEG_Y * 9.0,
        Kind::Exploding,
        0,
    );
    tick(&mut app, seconds(0.3));
    let news = events(&mut app);
    let blocked = count(&news, |event| {
        matches!(event, SimEvent::ShieldBlocked { .. })
    });
    let went_off = count(&news, |event| matches!(event, SimEvent::Blast { .. }));
    assert_eq!((blocked, went_off), (1, 1));
    assert_eq!(run(&app).lives, 3);
}

#[test]
fn a_blast_sets_off_the_exploding_bullets_that_it_reaches() {
    let mut app = start("TEST");
    // One ends on the east wall. The other one is within its blast then, and still far from
    // any wall.
    let fast = Vec2::X * 9.0;
    let slow = Vec2::X * 0.01;
    spawn_special(
        &mut app,
        Team::Enemies,
        Vec2::new(5.5, 10.0),
        fast,
        Kind::Exploding,
        0,
    );
    let waiting = Vec2::new(4.5, 8.5);
    spawn_special(&mut app, Team::Enemies, waiting, slow, Kind::Exploding, 0);
    // A mine lies within the second blast, and out of the first.
    let mine = Mine {
        owner: Entity::PLACEHOLDER,
        team: Team::Enemies,
        sector: 0,
        state: MineState::Arming(10.0),
    };
    let mine = (mine, Position(Vec2::new(2.5, 7.0)), InSector(0));
    let mine = app.world_mut().spawn(mine).id();
    tick(&mut app, seconds(0.3));
    let went_off = blasts(&mut app);
    assert_eq!(went_off.len(), 3);
    assert!(went_off[1].distance(waiting) < 0.1);
    assert_eq!(bullets(&mut app), 0);
    assert!(!alive(&app, mine));
    assert_eq!(run(&app).lives, 3);
}

#[test]
fn the_blast_of_a_bullet_is_the_doing_of_the_tank_that_fired_it() {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, Vec2::new(0.0, 2.0));
    let mut maker = Genome::empty(40);
    maker.class = Class::Grenadier;
    maker.set(Stat::Exploding, 1);
    // It flies past the player's tank and ends on the south wall, within 3 m of it.
    let bullet = spawn_special(
        &mut app,
        Team::Enemies,
        Vec2::new(2.0, 6.0),
        Vec2::NEG_Y * 9.0,
        Kind::Exploding,
        0,
    );
    app.world_mut().entity_mut(bullet).insert(Maker(maker));
    let blow = wait_for_the_loss(&mut app, 2.0);
    let wanted = Blow {
        weapon: Weapon::Bullet,
        genome: maker,
        team: Team::Enemies,
    };
    assert_eq!(blow, Some(wanted));
}

#[test]
fn an_exploding_bullet_that_ends_in_the_barrel_destroys_its_own_tank() {
    let tank = tank_with(Ammo::Exploding);
    let mut app = begin_as("TEST", 1, Difficulty::Normal, vec![tank]);
    tick(&mut app, seconds(SPAWN_FIRE_DELAY));
    let entity = player(&mut app).unwrap();
    // Against the east wall, with the barrel in it.
    teleport(&mut app, entity, Vec2::new(6.0, 8.0));
    let aim = Vec2::new(20.0, 8.0);
    command(&mut app, TankCommand { aim, ..default() });
    tick(&mut app, seconds(0.3));
    events(&mut app);
    command(
        &mut app,
        TankCommand {
            aim,
            fire: true,
            ..default()
        },
    );
    tick(&mut app, 2);
    assert_eq!(blasts(&mut app).len(), 1);
    assert_eq!(run(&app).lives, 2);
    assert_eq!(bullets(&mut app), 0);
}

#[test]
fn a_tank_whose_bullets_explode_holds_its_fire_while_the_blast_would_reach_its_side() {
    // A sentry that fires exploding bullets, and the shots that it fires in three seconds at
    // a player who stands at a place. Another enemy may stand somewhere, asleep.
    let shots_at = |player_at: Vec2, ally: Option<Vec2>| {
        let mut app = start("TEST");
        let entity = player(&mut app).unwrap();
        teleport(&mut app, entity, player_at);
        if let Some(ally) = ally {
            let ally = spawn_awake(&mut app, ally, &[]);
            app.world_mut().get_mut::<Enemy>(ally).unwrap().awake = false;
        }
        let levels = [(Stat::Turret, 6), (Stat::Exploding, 1)];
        let grenadier = spawn_awake(&mut app, Vec2::new(0.0, 13.0), &levels);
        tick(&mut app, seconds(3.0));
        let shots = count(&events(&mut app), |event| {
            matches!(
                event,
                SimEvent::Shot {
                    team: Team::Enemies,
                    ..
                }
            )
        });
        (shots, alive(&app, grenadier))
    };
    // Its shot would end on a player who is 2.5 m away, and the blast would reach its own
    // tank.
    assert_eq!(shots_at(Vec2::new(0.0, 10.5), None), (0, true));
    // It fires at a player who is further away.
    let (shots, survived) = shots_at(Vec2::new(0.0, 5.0), None);
    assert!(shots > 0 && survived);
    // And not while another enemy stands next to that player.
    let ally = Vec2::new(1.5, 4.0);
    assert_eq!(shots_at(Vec2::new(0.0, 5.0), Some(ally)), (0, true));
}

#[test]
fn a_grunt_stays_out_of_the_blast_that_an_exploding_bullet_marks() {
    let mut app = start("TEST");
    let entity = player(&mut app).unwrap();
    teleport(&mut app, entity, Vec2::new(0.0, 2.0));
    let levels = [(Stat::Engine, 3), (Stat::Tracks, 4), (Stat::Brain, 1)];
    let at = Vec2::new(5.0, 12.0);
    let grunt = spawn_awake(&mut app, at, &levels);
    // A bullet is on its way to the east wall, next to the grunt: its end is marked.
    let end = Vec2::new(7.0 - BULLET_RADIUS, 13.5);
    spawn_special(
        &mut app,
        Team::Players,
        Vec2::new(-1.0, 13.5),
        Vec2::X * 6.0,
        Kind::Exploding,
        0,
    );
    tick(&mut app, 2);
    assert!(at.distance(end) < BLAST_RADIUS);
    let drive = app.world().get::<TankCommand>(grunt).unwrap().drive;
    let away = (at - end).normalize();
    assert!(
        drive.normalize().dot(away) > 0.7,
        "the grunt drives {drive}"
    );
    // It is out of the blast when the bullet gets there.
    tick(&mut app, seconds(1.5));
    assert_eq!(blasts(&mut app).len(), 1);
    assert!(alive(&app, grunt));
}

#[test]
fn a_bullet_marks_what_it_is_about_to_do() {
    let app = start("TEST");
    let field = app.world().resource::<Field>();
    let bullet = |kind: Kind, bounces: u8, speed: f32| Bullet {
        owner: Entity::PLACEHOLDER,
        team: Team::Enemies,
        sector: 0,
        velocity: Vec2::X * speed,
        bounces_left: bounces,
        pierces_left: 0,
        bounced: false,
        kind,
    };
    let at = Vec2::new(0.0, 10.0);
    let wall = Vec2::new(7.0 - BULLET_RADIUS, 10.0);
    let players = [(START, Team::Players)];
    let mark = |bullet: &Bullet, quarry: &[(Vec2, Team)]| bullets::mark(field, bullet, at, quarry);
    let near = |found: Vec2, wanted: Vec2| found.distance(wanted) < 1e-3;

    // An exploding bullet marks where it ends on the wall, once it gets there within the
    // time that such an end is marked ahead.
    let exploding = bullet(Kind::Exploding, 0, 9.0);
    assert!(matches!(mark(&exploding, &players), Some(Mark::Blast { at }) if near(at, wall)));
    let slow = (wall.x - at.x) / MARK_TIME - 0.5;
    assert_eq!(mark(&bullet(Kind::Exploding, 0, slow), &players), None);

    // A seeking bullet marks its next bounce, and the tank that it would leave toward.
    let seeking = bullet(Kind::Seeking, 1, 9.0);
    let found = mark(&seeking, &players);
    assert!(
        matches!(found, Some(Mark::Bounce { at, toward: Some(tank) }) if near(at, wall) && tank == START)
    );
    // It seeks none of its own side, and marks the bounce alone then.
    let own = [(START, Team::Enemies)];
    assert!(matches!(
        mark(&seeking, &own),
        Some(Mark::Bounce { toward: None, .. })
    ));
    // Without a bounce left, it ends on the wall as any bullet does, and marks nothing.
    assert_eq!(mark(&bullet(Kind::Seeking, 0, 9.0), &players), None);
    assert_eq!(mark(&bullet(Kind::Plain, 1, 9.0), &players), None);
    let splitting = Kind::Splitting {
        fuse: SPLIT_TIME,
        bounces: 1,
    };
    assert_eq!(mark(&bullet(splitting, 0, 9.0), &players), None);
}
