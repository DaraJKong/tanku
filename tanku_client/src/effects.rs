//! Short-lived effects: blasts, sparks and flashes.
//!
//! Each one is a circle that is drawn in the air, grows to its full size and closes again.

use bevy::prelude::*;
use tanku_core::{
    rules::BLAST_RADIUS,
    sim::{SimEvent, Team},
};

use crate::{camera::View, draft::Draft, scene, theme};

/// How much of the radius of a burst its line takes, and how see-through its inside is.
const LINE_SHARE: f32 = 0.12;
const FILL: f32 = 0.22;

/// A circle that grows to its full size, then shrinks to nothing.
#[derive(Component)]
struct Burst {
    age: f32,
    life: f32,
    size: f32,
}

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
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (spawn, animate).chain());
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
    let (fire, spark, cold, good) = (
        glow(theme::ENEMY_BULLET),
        glow(Color::WHITE),
        glow(theme::PLAYER_BULLET),
        glow(theme::GOOD),
    );
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
    });
}

fn spawn(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    mut view: ResMut<View>,
    glows: Res<Glows>,
) {
    for event in events.read() {
        let (at, height, size, life, glow) = match *event {
            SimEvent::Blast { at } => (at, 0.6, 2.0 * BLAST_RADIUS, 0.45, &glows.fire),
            SimEvent::TankDestroyed { at, .. } => (at, 0.6, 3.2, 0.5, &glows.fire),
            SimEvent::BlockDestroyed { at } => (at, 0.5, 2.4, 0.3, &glows.spark),
            SimEvent::Shot { at, team } => {
                let glow = match team {
                    Team::Players => &glows.cold,
                    Team::Enemies => &glows.fire,
                };
                (at, 0.8, 0.7, 0.1, glow)
            }
            SimEvent::Ricochet { at } => (at, 0.8, 0.6, 0.12, &glows.spark),
            SimEvent::BulletDestroyed { at } => (at, 0.8, 0.8, 0.15, &glows.spark),
            SimEvent::ShieldBlocked { at } => (at, 0.6, 3.4, 0.2, &glows.spark),
            SimEvent::PlayerRespawned { at } => (at, 0.6, 4.0, 0.5, &glows.cold),
            SimEvent::FlagCaptured { at } => (at, 0.6, 5.0, 0.5, &glows.good),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_swells_then_closes() {
        assert_eq!(swell(0.0), 0.0);
        assert!((swell(0.33) - 1.0).abs() < 1e-6);
        assert!(swell(0.2) < swell(0.3));
        assert!(swell(0.9) < swell(0.5));
        assert!(swell(0.999) < 0.01);
    }
}
