//! The models of the tanks, assembled from their genomes.
//!
//! Every stat has its own visual channel, so that players can read a tank before fighting it.
//! A model points along `+x` and stands on the ground.

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use tanku_core::{
    rules::SHIELD_RADIUS,
    sim::{Enemy, Motion, Player, Shield, ShieldState, Tank},
    stats::{Genome, SpendingGroup, Stat},
};

use crate::{
    scene::{self, Paints, Shapes},
    theme,
};

/// Height of the turret ring above the ground.
const TURRET_HEIGHT: f32 = 0.65;

#[derive(Component)]
struct Hull;

#[derive(Component)]
struct Turret;

#[derive(Component)]
struct Bubble;

/// The paint of a tank, which dims while the tank is dormant.
#[derive(Component)]
struct Livery {
    paint: Handle<StandardMaterial>,
    color: Color,
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (build, (pose, dim, shield)).chain().before(scene::place),
    );
}

/// The color of an enemy: its hue tells where most of its points went, and its intensity tells
/// how many points it has.
pub fn enemy_color(genome: &Genome) -> Color {
    let hue = match genome.main_group() {
        SpendingGroup::Mobility => 55.0,
        SpendingGroup::Firepower => 5.0,
        SpendingGroup::Utility => 285.0,
        SpendingGroup::Software => 150.0,
    };
    let strength = (genome.budget as f32 / 100.0).clamp(0.0, 1.0);
    Color::hsl(hue, 0.2 + 0.75 * strength, 0.66 - 0.2 * strength)
}

/// Adds the parts of a tank model as children of `parent`.
struct Assembly<'a, 'w> {
    parent: &'a mut ChildSpawnerCommands<'w>,
    shapes: &'a Shapes,
}

impl Assembly<'_, '_> {
    fn part(
        &mut self,
        shape: &Handle<Mesh>,
        paint: &Handle<StandardMaterial>,
        transform: Transform,
    ) {
        self.parent.spawn((
            Mesh3d(shape.clone()),
            MeshMaterial3d(paint.clone()),
            transform,
        ));
    }

    fn cuboid(&mut self, paint: &Handle<StandardMaterial>, center: Vec3, size: Vec3) {
        let shape = self.shapes.cube.clone();
        self.part(
            &shape,
            paint,
            Transform::from_translation(center).with_scale(size),
        );
    }

    /// A cylinder standing on its axis.
    fn post(&mut self, paint: &Handle<StandardMaterial>, center: Vec3, radius: f32, height: f32) {
        let shape = self.shapes.cylinder.clone();
        let scale = Vec3::new(2.0 * radius, height, 2.0 * radius);
        self.part(
            &shape,
            paint,
            Transform::from_translation(center).with_scale(scale),
        );
    }

    /// A cylinder lying along `x`.
    fn tube(&mut self, paint: &Handle<StandardMaterial>, center: Vec3, radius: f32, length: f32) {
        let shape = self.shapes.cylinder.clone();
        let transform = Transform {
            translation: center,
            rotation: Quat::from_rotation_z(-FRAC_PI_2),
            scale: Vec3::new(2.0 * radius, length, 2.0 * radius),
        };
        self.part(&shape, paint, transform);
    }

    fn ball(&mut self, paint: &Handle<StandardMaterial>, center: Vec3, radius: f32) {
        let shape = self.shapes.sphere.clone();
        let transform = Transform::from_translation(center).with_scale(Vec3::splat(2.0 * radius));
        self.part(&shape, paint, transform);
    }
}

fn build(
    mut commands: Commands,
    tanks: Query<(Entity, &Tank, &Motion, Has<Player>), Added<Tank>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, tank, motion, is_player) in &tanks {
        let genome = &tank.genome;
        let color = if is_player {
            theme::PLAYER
        } else {
            enemy_color(genome)
        };
        let body = materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.6,
            ..default()
        });
        let mut light = |color: Color| {
            materials.add(StandardMaterial {
                base_color: color,
                emissive: color.to_linear() * 3.0,
                ..default()
            })
        };
        let (blue, white, red) = (
            light(theme::PLAYER_BULLET),
            light(Color::WHITE),
            light(theme::DANGER),
        );
        let bubble = materials.add(StandardMaterial {
            base_color: theme::PLAYER_BULLET.with_alpha(0.3),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let dark = &paints.dark;
        let level = |stat: Stat| genome.level(stat) as f32;

        commands
            .entity(entity)
            .insert((
                Transform::default(),
                Visibility::default(),
                Livery {
                    paint: body.clone(),
                    color,
                },
            ))
            .with_children(|root| {
                // Tracks: agile tanks are short.
                let length = 2.0 - 0.1 * level(Stat::Tracks);
                root.spawn((
                    Hull,
                    Transform::from_rotation(scene::facing(motion.hull)),
                    Visibility::default(),
                ))
                .with_children(|hull| {
                    let mut hull = Assembly {
                        parent: hull,
                        shapes: &shapes,
                    };
                    hull.cuboid(
                        &body,
                        Vec3::new(0.0, 0.45, 0.0),
                        Vec3::new(length, 0.4, 1.15),
                    );
                    if genome.level(Stat::Engine) == 0 {
                        // Engine: a tank that can't move sits on a fixed base.
                        hull.post(dark, Vec3::new(0.0, 0.13, 0.0), 0.88, 0.26);
                    } else {
                        // Engine: the faster the tank, the bigger its tracks.
                        let (high, wide) = (
                            0.34 + 0.035 * level(Stat::Engine),
                            0.24 + 0.03 * level(Stat::Engine),
                        );
                        for side in [-1.0, 1.0] {
                            let center = Vec3::new(0.0, high / 2.0, side * (0.52 + wide / 2.0));
                            hull.cuboid(dark, center, Vec3::new(length + 0.25, high, wide));
                        }
                    }
                    // Mines: a rack at the rear, one mine per slot.
                    let mines = genome.mines();
                    for slot in 0..mines {
                        let across = (slot as f32 - (mines as f32 - 1.0) / 2.0) * 0.27;
                        let center = Vec3::new(-length / 2.0 + 0.16, 0.69, across);
                        hull.post(dark, center, 0.11, 0.08);
                    }
                    // Shield: an emitter dome at the front, larger with level.
                    if genome.shield().is_some() {
                        let radius = 0.09 + 0.035 * level(Stat::Shield);
                        hull.ball(&blue, Vec3::new(length / 2.0 - 0.22, 0.66, 0.0), radius);
                    }
                    // Brain: one antenna per tier.
                    for tier in 0..genome.brain() {
                        let side = if tier % 2 == 0 { 1.0 } else { -1.0 };
                        let back = -length / 2.0 + 0.1 + 0.22 * (tier / 2) as f32;
                        hull.post(dark, Vec3::new(back, 1.0, side * 0.47), 0.02, 0.7);
                        hull.ball(&white, Vec3::new(back, 1.36, side * 0.47), 0.05);
                    }
                });

                let pivot = Transform {
                    translation: Vec3::Y * TURRET_HEIGHT,
                    rotation: scene::facing(motion.turret),
                    ..default()
                };
                root.spawn((Turret, pivot, Visibility::default()))
                    .with_children(|turret| {
                        let mut turret = Assembly {
                            parent: turret,
                            shapes: &shapes,
                        };
                        // Turret: small turrets turn fast.
                        let radius = 0.56 - 0.035 * level(Stat::Turret);
                        turret.post(&body, Vec3::new(0.0, 0.16, 0.0), radius, 0.32);
                        // Muzzle: the faster the bullet, the longer the barrel.
                        let barrel = 0.6 + 0.1 * level(Stat::Muzzle);
                        let tip = radius - 0.1 + barrel;
                        turret.tube(dark, Vec3::new(tip - barrel / 2.0, 0.18, 0.0), 0.08, barrel);
                        // Ricochet: one ring around the muzzle per bounce.
                        for ring in 0..genome.bounces() {
                            let transform = Transform {
                                translation: Vec3::new(tip - 0.08 - 0.13 * ring as f32, 0.18, 0.0),
                                rotation: Quat::from_rotation_z(FRAC_PI_2),
                                scale: Vec3::splat(0.3),
                            };
                            turret.part(&shapes.ring, &body, transform);
                        }
                        // Piercing: a pointed tip on the barrel.
                        if genome.piercing() {
                            let transform = Transform {
                                translation: Vec3::new(tip + 0.14, 0.18, 0.0),
                                rotation: Quat::from_rotation_z(-FRAC_PI_2),
                                scale: Vec3::new(0.2, 0.32, 0.2),
                            };
                            turret.part(&shapes.cone, &red, transform);
                        }
                        // Magazine: a drum behind the turret, one segment per round.
                        for round in 0..genome.magazine() {
                            let back = -radius - 0.09 - 0.12 * round as f32;
                            turret.cuboid(
                                dark,
                                Vec3::new(back, 0.16, 0.0),
                                Vec3::new(0.09, 0.24, 0.36),
                            );
                        }
                        // Loader: vents on the turret sides, more with level.
                        for vent in 0..genome.level(Stat::Loader) {
                            let side = if vent % 2 == 0 { 1.0 } else { -1.0 };
                            let along = -0.25 + 0.15 * (vent / 2) as f32;
                            let center = Vec3::new(along, 0.36, side * radius * 0.7);
                            turret.cuboid(dark, center, Vec3::new(0.1, 0.08, 0.12));
                        }
                        // Ballistics: a sight on the turret, one lens per tier.
                        let lenses = genome.ballistics();
                        for lens in 0..lenses {
                            let across = (lens as f32 - (lenses as f32 - 1.0) / 2.0) * 0.17;
                            turret.tube(&red, Vec3::new(radius * 0.45, 0.38, across), 0.055, 0.14);
                        }
                    });

                root.spawn((
                    Bubble,
                    Mesh3d(shapes.sphere.clone()),
                    MeshMaterial3d(bubble),
                    Transform::from_xyz(0.0, 0.5, 0.0).with_scale(Vec3::splat(2.0 * SHIELD_RADIUS)),
                    Visibility::Hidden,
                ));
            });
    }
}

/// Turns the hulls and the turrets.
fn pose(
    tanks: Query<(&Motion, &Children), With<Tank>>,
    mut hulls: Query<&mut Transform, (With<Hull>, Without<Turret>)>,
    mut turrets: Query<&mut Transform, (With<Turret>, Without<Hull>)>,
) {
    for (motion, children) in &tanks {
        for child in children.iter() {
            if let Ok(mut hull) = hulls.get_mut(child) {
                hull.rotation = scene::facing(motion.hull);
            }
            if let Ok(mut turret) = turrets.get_mut(child) {
                turret.rotation = scene::facing(motion.turret);
            }
        }
    }
}

/// Dormant enemies are powered down: their paint is dull until they wake up.
fn dim(
    enemies: Query<(&Enemy, &Livery), Changed<Enemy>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (enemy, livery) in &enemies {
        if let Some(mut material) = materials.get_mut(&livery.paint) {
            // Darker, but of the same hue: the hue is how players read a tank.
            let dull = livery.color.to_linear() * 0.45;
            material.base_color = if enemy.awake {
                livery.color
            } else {
                dull.with_alpha(1.0).into()
            };
        }
    }
}

/// Shows the bubble of a raised shield, and makes an overloaded tank flicker.
fn shield(
    time: Res<Time>,
    tanks: Query<(&Shield, &Children), With<Tank>>,
    mut bubbles: Query<(&mut Visibility, &mut Transform), With<Bubble>>,
) {
    for (shield, children) in &tanks {
        for child in children.iter() {
            let Ok((mut visibility, mut transform)) = bubbles.get_mut(child) else {
                continue;
            };
            let (shown, size) = match shield.state {
                ShieldState::Active(_) => (true, 2.0 * SHIELD_RADIUS),
                // Sparks of a shield that has just dropped.
                ShieldState::Overloaded(_) => (time.elapsed_secs() % 0.16 < 0.05, 1.5),
                ShieldState::Ready => (false, 1.0),
            };
            *visibility = if shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            transform.scale = Vec3::splat(size);
        }
    }
}
