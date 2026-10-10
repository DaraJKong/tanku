//! The camera: it looks down at the player's tank and turns in quarter turns.

use std::f32::consts::FRAC_PI_2;

use bevy::{light::GlobalAmbientLight, prelude::*, window::PrimaryWindow};
use tanku_core::sim::Player;

use crate::{scene, settings::Settings, theme};

/// Angle between the ground and the line of sight.
const PITCH: f32 = 65.0;
/// A narrow field of view, to keep the perspective mild.
const FIELD_OF_VIEW: f32 = 20.0;
/// Meters of ground visible from the bottom to the top of the screen.
const VISIBLE_GROUND: f32 = 30.0;
/// How far the view shifts toward the cursor, as a share of the distance and in meters at most.
const LOOK_AHEAD: (f32, f32) = (0.03125, 0.75);

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource, Default)]
pub struct View {
    /// Quarter turns of the view. North is at the top of the screen at 0 and on the right at 1.
    pub turns: i32,
    /// The point on the ground under the cursor.
    pub aim: Vec2,
    /// Meters of ground from the bottom to the top of the screen, when a script zooms.
    pub span: Option<f32>,
    yaw: f32,
    focus: Vec2,
    /// Time during which the view stays where it was told to look.
    hold: f32,
    shake: f32,
    noise: u32,
}

impl View {
    /// The direction on the ground of a direction on the screen.
    pub fn to_ground(&self, screen: Vec2) -> Vec2 {
        Vec2::from_angle(self.turns as f32 * FRAC_PI_2).rotate(screen)
    }

    /// Shakes the camera, by an amount from 0 to 1.
    pub fn shake(&mut self, amount: f32) {
        self.shake = (self.shake + amount).min(1.0);
    }

    /// The point on the ground the camera looks at.
    pub fn focus(&self) -> Vec2 {
        self.focus
    }

    /// Looks at a point for a while instead of following the player.
    #[cfg(feature = "development")]
    pub fn look_at(&mut self, point: Vec2, seconds: f32) {
        self.focus = point;
        self.hold = seconds;
    }

    fn jitter(&mut self) -> f32 {
        self.noise = self
            .noise
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.noise >> 8) as f32 / (1 << 23) as f32 - 1.0
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<View>()
        .insert_resource(ClearColor(theme::VOID))
        .insert_resource(GlobalAmbientLight {
            brightness: 600.0,
            ..default()
        })
        .add_systems(Startup, setup)
        .add_systems(Update, follow.after(scene::place));
}

fn setup(mut commands: Commands) {
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: FIELD_OF_VIEW.to_radians(),
            ..default()
        }),
        Transform::from_xyz(0.0, 70.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-6.0, 20.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Finds the point on the ground under the cursor.
pub fn aim(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut view: ResMut<View>,
) {
    let (camera, transform) = *camera;
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(transform, cursor) else {
        return;
    };
    if ray.direction.y < -1e-4 {
        let ground = ray.origin + *ray.direction * (-ray.origin.y / ray.direction.y);
        view.aim = scene::to_ground(ground);
    }
}

fn follow(
    time: Res<Time>,
    settings: Res<Settings>,
    mut view: ResMut<View>,
    player: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut camera: Single<&mut Transform, With<MainCamera>>,
) {
    let seconds = time.delta_secs();
    view.hold = (view.hold - seconds).max(0.0);
    if let Ok(player) = player.single()
        && view.hold == 0.0
    {
        let at = scene::to_ground(player.translation);
        let ahead = ((view.aim - at) * LOOK_AHEAD.0).clamp_length_max(LOOK_AHEAD.1);
        let target = at + ahead;
        // A jump means a new run or a respawn: go there at once.
        view.focus = if view.focus.distance(target) > 40.0 {
            target
        } else {
            view.focus.lerp(target, 1.0 - (-8.0 * seconds).exp())
        };
    }
    let target_yaw = view.turns as f32 * FRAC_PI_2;
    view.yaw += (target_yaw - view.yaw) * (1.0 - (-12.0 * seconds).exp());
    view.shake = (view.shake - 2.0 * seconds).max(0.0);
    let strength = if settings.camera_shake {
        view.shake * view.shake * 0.7
    } else {
        0.0
    };
    let jitter = Vec2::new(view.jitter(), view.jitter()) * strength;

    let pitch = PITCH.to_radians();
    let span = view.span.unwrap_or(VISIBLE_GROUND);
    let distance = span * pitch.sin() / (2.0 * (FIELD_OF_VIEW.to_radians() / 2.0).tan());
    let offset =
        Quat::from_rotation_y(view.yaw) * Vec3::new(0.0, pitch.sin(), pitch.cos()) * distance;
    let focus = scene::to_world(view.focus + jitter, 0.0);
    **camera = Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y);
}
