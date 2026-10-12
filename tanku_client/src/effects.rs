//! Short-lived effects: blasts, sparks and flashes.
//!
//! Most are a circle that is drawn in the air, grows to its full size and closes again. A
//! bullet that is destroyed leaves a burst of short white strokes instead: they shoot
//! out from where it was, then shorten until nothing is left.

use std::f32::consts::TAU;

use bevy::prelude::*;
use tanku_core::{
    rules::{APPEAR_TIME, BLAST_RADIUS, TANK_RADIUS},
    sim::SimEvent,
};

use crate::{camera::View, draft::Draft, scene, theme};

/// How much of the radius of a burst its line takes, and how see-through its inside is.
const LINE_SHARE: f32 = 0.12;
const FILL: f32 = 0.22;
/// The strokes that a destroyed bullet leaves: how many, how far from the place of the bullet
/// they start and end, and how wide they are.
const STROKES: usize = 8;
const STROKE_REACH: (f32, f32) = (0.15, 0.35);
const STROKE_WIDTH: f32 = 0.05;
/// How long the strokes last, and the share of that time they take to shoot out.
const STROKE_LIFE: f32 = 0.22;
const STROKE_RISE: f32 = 0.35;
/// The drawings that the strokes go through, one after the other.
const STROKE_FRAMES: usize = 12;
/// Height at which the strokes are drawn: just over the bullets, so that the one that flies
/// on through a bullet it has pierced doesn't hide them.
const STROKE_HEIGHT: f32 = 1.05;
/// Height at which the mark of a tank that is about to appear is drawn: on the floor.
const MARK_HEIGHT: f32 = 0.08;

/// A circle that grows to its full size, then shrinks to nothing.
#[derive(Component)]
struct Burst {
    age: f32,
    life: f32,
    size: f32,
}

/// The strokes that a destroyed bullet leaves.
#[derive(Component)]
struct Strokes {
    age: f32,
}

/// The mark of a tank of an arena that is about to appear: a circle on the floor that closes
/// in on the place where the tank will stand, for as long as the warning lasts.
#[derive(Component)]
struct Mark {
    age: f32,
}

/// How wide the mark of a tank that is about to appear starts, across, and how wide it ends:
/// around the tank.
const MARK_SIZE: (f32, f32) = (7.0, 2.0 * TANK_RADIUS);

/// The color of a burst: its line, and the faint fill inside it.
struct Glow {
    line: Handle<StandardMaterial>,
    fill: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct Glows {
    /// A circle and a disc lying flat, 1 across.
    circle: Handle<Mesh>,
    disc: Handle<Mesh>,
    fire: Glow,
    spark: Glow,
    cold: Glow,
    good: Glow,
    /// The warning of a tank that is about to appear.
    alarm: Glow,
    /// The strokes of a destroyed bullet, from the first drawing to the last.
    strokes: Vec<Handle<Mesh>>,
    /// What they are drawn with: the color of the players' bullets, and of the enemies'.
    stroke_paint: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (spawn, (animate, fly_apart, close_in)).chain());
}

/// Where the strokes of a destroyed bullet start and end at this share of their life. They
/// shoot out to their full length, fast at first, then shorten from the inside.
fn stroke_ends(progress: f32) -> (f32, f32) {
    let (inner, outer) = STROKE_REACH;
    if progress < STROKE_RISE {
        let left = 1.0 - progress / STROKE_RISE;
        (inner, outer - (outer - inner) * left * left)
    } else {
        let gone = (progress - STROKE_RISE) / (1.0 - STROKE_RISE);
        (inner + (outer - inner) * gone.min(1.0), outer)
    }
}

/// The strokes of a destroyed bullet at this share of their life: a star lying flat.
fn strokes(progress: f32) -> Draft {
    let mut draft = Draft::default();
    let (inner, outer) = stroke_ends(progress);
    for stroke in 0..STROKES {
        let (sin, cos) = (stroke as f32 / STROKES as f32 * TAU).sin_cos();
        let along = Vec3::new(cos, 0.0, sin);
        draft.line(along * inner, along * outer, STROKE_WIDTH, Color::WHITE);
    }
    draft
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut paint = |color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        })
    };
    let mut glow = |color: Color| Glow {
        line: paint(color.with_alpha(0.9)),
        fill: paint(color.with_alpha(FILL)),
    };
    let (fire, spark, cold, good, alarm) = (
        glow(theme::FIRE),
        glow(Color::WHITE),
        glow(theme::PLAYER),
        glow(theme::GOOD),
        glow(theme::DANGER),
    );
    let stroke_paint = paint(theme::BULLET);
    // Each drawing stands for the middle of its share of the life of the strokes.
    let frames = (0..STROKE_FRAMES).map(|frame| (frame as f32 + 0.5) / STROKE_FRAMES as f32);
    let strokes = frames
        .map(|progress| meshes.add(strokes(progress).build()))
        .collect();
    let mut circle = Draft::default();
    circle.ring(
        Vec3::ZERO,
        (0.5 * (1.0 - LINE_SHARE), 0.5),
        (1, 1.0),
        Color::WHITE,
    );
    let mut disc = Draft::default();
    disc.disc(Vec3::ZERO, 0.5 * (1.0 - LINE_SHARE), Color::WHITE);
    commands.insert_resource(Glows {
        circle: meshes.add(circle.build()),
        disc: meshes.add(disc.build()),
        fire,
        spark,
        cold,
        good,
        alarm,
        strokes,
        stroke_paint,
    });
}

fn spawn(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    mut view: ResMut<View>,
    glows: Res<Glows>,
) {
    for event in events.read() {
        if let SimEvent::BulletDestroyed { at, .. } = *event {
            commands.spawn((
                Strokes { age: 0.0 },
                Mesh3d(glows.strokes[0].clone()),
                MeshMaterial3d(glows.stroke_paint.clone()),
                Transform::from_translation(scene::to_world(at, STROKE_HEIGHT)),
            ));
            continue;
        }
        if let SimEvent::TankIncoming { at } = *event {
            commands.spawn((
                Mark { age: 0.0 },
                Transform::from_translation(scene::to_world(at, MARK_HEIGHT))
                    .with_scale(Vec3::splat(MARK_SIZE.0)),
                Visibility::default(),
                children![
                    (
                        Mesh3d(glows.circle.clone()),
                        MeshMaterial3d(glows.alarm.line.clone())
                    ),
                    (
                        Mesh3d(glows.disc.clone()),
                        MeshMaterial3d(glows.alarm.fill.clone())
                    ),
                ],
            ));
            continue;
        }
        let (at, height, size, life, glow) = match *event {
            SimEvent::Blast { at } => (at, 0.6, 2.0 * BLAST_RADIUS, 0.45, &glows.fire),
            SimEvent::TankAppeared { at } => (at, 0.6, 3.6, 0.35, &glows.alarm),
            SimEvent::TankDestroyed { at, .. } => (at, 0.6, 3.2, 0.5, &glows.fire),
            SimEvent::BlockDestroyed { at } => (at, 0.5, 2.4, 0.3, &glows.spark),
            SimEvent::Shot { at, .. } => (at, 0.8, 0.7, 0.1, &glows.spark),
            SimEvent::Ricochet { at } => (at, 0.8, 0.6, 0.12, &glows.spark),
            SimEvent::BulletSplit { at } => (at, 0.8, 1.0, 0.14, &glows.spark),
            SimEvent::ShieldBlocked { at } => (at, 0.6, 3.4, 0.2, &glows.spark),
            SimEvent::PlayerRespawned { at } | SimEvent::TankRepaired { at } => {
                (at, 0.6, 4.0, 0.5, &glows.cold)
            }
            SimEvent::LifeTaken { at } => (at, 0.6, 5.0, 0.5, &glows.good),
            _ => continue,
        };
        // Blasts shake the camera, the more the closer they are.
        if matches!(
            event,
            SimEvent::Blast { .. } | SimEvent::TankDestroyed { .. }
        ) {
            let distance = at.distance(view.focus());
            view.shake(0.8 * (1.0 - distance / 40.0).clamp(0.0, 1.0));
        }
        commands.spawn((
            Burst {
                age: 0.0,
                life,
                size,
            },
            Transform::from_translation(scene::to_world(at, height)).with_scale(Vec3::ZERO),
            Visibility::default(),
            children![
                (
                    Mesh3d(glows.circle.clone()),
                    MeshMaterial3d(glow.line.clone())
                ),
                (
                    Mesh3d(glows.disc.clone()),
                    MeshMaterial3d(glow.fill.clone())
                ),
            ],
        ));
    }
}

/// How large a burst is at this share of its life: full size after a third of it, gone at
/// the end.
fn swell(progress: f32) -> f32 {
    if progress < 0.33 {
        progress / 0.33
    } else {
        1.0 - (progress - 0.33) / 0.67
    }
}

fn animate(
    mut commands: Commands,
    time: Res<Time>,
    mut bursts: Query<(Entity, &mut Burst, &mut Transform)>,
) {
    for (entity, mut burst, mut transform) in &mut bursts {
        burst.age += time.delta_secs();
        let progress = burst.age / burst.life;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.scale = Vec3::splat(burst.size * swell(progress));
    }
}

/// How wide the mark of a tank that is about to appear is, this long into the warning.
fn mark_size(age: f32) -> f32 {
    let progress = (age / APPEAR_TIME).clamp(0.0, 1.0);
    MARK_SIZE.0 + (MARK_SIZE.1 - MARK_SIZE.0) * progress
}

/// The mark of a tank that is about to appear closes in on its place, and is gone when the
/// tank is there.
fn close_in(
    mut commands: Commands,
    time: Res<Time>,
    mut marks: Query<(Entity, &mut Mark, &mut Transform)>,
) {
    for (entity, mut mark, mut transform) in &mut marks {
        mark.age += time.delta_secs();
        if mark.age >= APPEAR_TIME {
            commands.entity(entity).despawn();
            continue;
        }
        transform.scale = Vec3::splat(mark_size(mark.age));
    }
}

/// The strokes of a destroyed bullet go through their drawings, then they are gone.
fn fly_apart(
    mut commands: Commands,
    time: Res<Time>,
    glows: Res<Glows>,
    mut strokes: Query<(Entity, &mut Strokes, &mut Mesh3d)>,
) {
    for (entity, mut strokes, mut mesh) in &mut strokes {
        strokes.age += time.delta_secs();
        let frame = (strokes.age / STROKE_LIFE * STROKE_FRAMES as f32) as usize;
        match glows.strokes.get(frame) {
            Some(drawing) if mesh.0 != *drawing => mesh.0 = drawing.clone(),
            Some(_) => {}
            None => commands.entity(entity).despawn(),
        }
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::sim::Team;

    use super::*;

    fn close(found: f32, wanted: f32) -> bool {
        (found - wanted).abs() < 1e-5
    }

    #[test]
    fn the_strokes_of_a_destroyed_bullet_shoot_out_then_shorten() {
        let (inner, outer) = STROKE_REACH;
        // They start as nothing, where they are closest to the bullet.
        assert_eq!(stroke_ends(0.0), (inner, inner));
        // Then they are at their longest.
        let (from, to) = stroke_ends(STROKE_RISE);
        assert!(close(from, inner) && close(to, outer));
        // They end as nothing, where they are farthest from it.
        let (from, to) = stroke_ends(1.0);
        assert!(close(from, outer) && close(to, outer));
        // They never reach further than that, and never point backward.
        for step in 0..=100 {
            let (from, to) = stroke_ends(step as f32 / 100.0);
            assert!(inner <= from && from <= to && to <= outer + 1e-6, "{step}");
        }
        // They shoot out fast at first.
        assert!(stroke_ends(STROKE_RISE / 2.0).1 > (inner + outer) / 2.0);
    }

    #[test]
    fn a_destroyed_bullet_leaves_eight_strokes_around_its_place() {
        let draft = strokes(STROKE_RISE);
        let (inner, outer) = STROKE_REACH;
        let ink = STROKES as f32 * (outer - inner) * STROKE_WIDTH;
        assert!(close(draft.area(Color::WHITE), ink));
        // The star lies flat, and reaches as far to every side.
        let (lowest, highest) = draft.bounds();
        assert!(close(lowest.y, 0.0) && close(highest.y, 0.0));
        for reach in [highest.x, highest.z, -lowest.x, -lowest.z] {
            assert!(close(reach, outer), "{reach}");
        }
    }

    /// A world that shows the effects of what happens in the simulation.
    fn world_of_effects() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<View>()
            .init_resource::<Time>()
            .add_message::<SimEvent>()
            .add_systems(Startup, setup)
            .add_systems(Update, (spawn, (animate, fly_apart, close_in)).chain());
        app
    }

    #[test]
    fn the_mark_of_a_tank_that_is_about_to_appear_closes_in_on_its_place() {
        assert_eq!(mark_size(0.0), MARK_SIZE.0);
        assert!(close(mark_size(APPEAR_TIME), 2.0 * TANK_RADIUS));
        assert!(mark_size(0.5) > mark_size(1.0));

        let mut app = world_of_effects();
        app.update();
        let at = Vec2::new(3.0, 4.0);
        app.world_mut().write_message(SimEvent::TankIncoming { at });
        app.update();
        let world = app.world_mut();
        let mut marks = world.query::<(&Mark, &Transform)>();
        let (_, place) = marks.single(world).unwrap();
        // It lies on the floor, where the tank will stand.
        assert_eq!(scene::to_ground(place.translation), at);
        assert!(place.translation.y < 0.2);
        // It is gone when the tank is there, which shows with a flash.
        let whole = std::time::Duration::from_secs_f32(APPEAR_TIME + 0.1);
        app.world_mut().resource_mut::<Time>().advance_by(whole);
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&Mark>().iter(world).count(), 0);
        let moment = std::time::Duration::from_secs_f32(0.01);
        world.resource_mut::<Time>().advance_by(moment);
        world.write_message(SimEvent::TankAppeared { at });
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&Burst>().iter(world).count(), 1);
    }

    #[test]
    fn a_destroyed_bullet_leaves_white_strokes_whoever_fired_it() {
        let mut app = world_of_effects();
        app.update();
        for team in [Team::Players, Team::Enemies] {
            let at = Vec2::new(3.0, 4.0);
            app.world_mut()
                .write_message(SimEvent::BulletDestroyed { at, team });
            app.update();
            let world = app.world_mut();
            let mut marks = world.query::<(Entity, &Strokes, &MeshMaterial3d<StandardMaterial>)>();
            let (mark, _, paint) = marks.single(world).unwrap();
            let color = world.resource::<Assets<StandardMaterial>>().get(&paint.0);
            // Whoever fired it, a bullet is white, and so is what it leaves.
            assert_eq!(color.unwrap().base_color, theme::BULLET);
            // They are where the bullet was, and no circle is drawn there.
            let place = world.get::<Transform>(mark).unwrap().translation;
            assert_eq!(scene::to_ground(place), at);
            assert_eq!(world.query::<&Burst>().iter(world).count(), 0);
            world.despawn(mark);
        }
    }

    #[test]
    fn the_strokes_are_gone_when_their_time_is_up() {
        let mut app = world_of_effects();
        app.update();
        let event = SimEvent::BulletDestroyed {
            at: Vec2::ZERO,
            team: Team::Players,
        };
        app.world_mut().write_message(event);
        app.update();
        let marks = |app: &mut App| {
            let world = app.world_mut();
            world.query::<&Strokes>().iter(world).count()
        };
        assert_eq!(marks(&mut app), 1);
        let age = |app: &mut App, seconds: f32| {
            let step = std::time::Duration::from_secs_f32(seconds);
            app.world_mut().resource_mut::<Time>().advance_by(step);
            app.update();
        };
        age(&mut app, STROKE_LIFE * 0.9);
        assert_eq!(marks(&mut app), 1);
        age(&mut app, STROKE_LIFE * 0.2);
        assert_eq!(marks(&mut app), 0);
    }

    #[test]
    fn a_burst_swells_then_closes() {
        assert_eq!(swell(0.0), 0.0);
        assert!((swell(0.33) - 1.0).abs() < 1e-6);
        assert!(swell(0.2) < swell(0.3));
        assert!(swell(0.9) < swell(0.5));
        assert!(swell(0.999) < 0.01);
    }
}
