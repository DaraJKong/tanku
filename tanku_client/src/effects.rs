//! Short-lived effects: blasts, sparks and flashes.

use bevy::prelude::*;
use tanku_core::{
    rules::BLAST_RADIUS,
    sim::{SimEvent, Team},
};

use crate::{
    camera::View,
    scene::{self, Shapes},
    theme,
};

/// A ball that grows to its full size, then shrinks to nothing.
#[derive(Component)]
struct Burst {
    age: f32,
    life: f32,
    size: f32,
}

#[derive(Resource)]
struct Glows {
    fire: Handle<StandardMaterial>,
    spark: Handle<StandardMaterial>,
    cold: Handle<StandardMaterial>,
    good: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (spawn, animate).chain());
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut glow = |color: Color, alpha: f32| {
        materials.add(StandardMaterial {
            base_color: color.with_alpha(alpha),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })
    };
    commands.insert_resource(Glows {
        fire: glow(theme::ENEMY_BULLET, 0.75),
        spark: glow(Color::WHITE, 0.9),
        cold: glow(theme::PLAYER_BULLET, 0.6),
        good: glow(theme::GOOD, 0.5),
    });
}

fn spawn(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    mut view: ResMut<View>,
    shapes: Res<Shapes>,
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
            Mesh3d(shapes.sphere.clone()),
            MeshMaterial3d(glow.clone()),
            Transform::from_translation(scene::to_world(at, height)).with_scale(Vec3::ZERO),
        ));
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
        // Full size after a third of its life, gone at the end.
        let scale = if progress < 0.33 {
            progress / 0.33
        } else {
            1.0 - (progress - 0.33) / 0.67
        };
        transform.scale = Vec3::splat(burst.size * scale);
    }
}
