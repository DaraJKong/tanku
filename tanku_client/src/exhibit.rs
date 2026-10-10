//! A tank shown large on the sheet, behind a menu: the player's own behind the main menu, and
//! the tank that ended a run behind the report of that run.
//!
//! It stands far from any run, so that nothing of the game shows around it, and the view
//! goes there for as long as its screen lasts.

use bevy::prelude::*;
use tanku_core::{
    rules::{SHIELD_RADIUS, TANK_RADIUS},
    stats::Genome,
};

use crate::{
    Screen,
    camera::View,
    draft::{Draft, LIFT, LINE},
    models,
    scene::{self, Paints},
    theme,
};

/// Where the tank stands: far from any run, so that the view jumps back to the game when one
/// starts.
const FAR_AWAY: Vec2 = Vec2::new(0.0, -500.0);

/// Something that is on show.
#[derive(Component)]
pub struct Exhibit;

/// A turret that looks around, with the angle that it looks around.
#[derive(Component)]
struct Sway(f32);

/// How a tank is shown.
pub struct Show {
    pub genome: Genome,
    pub color: Color,
    /// Meters of ground from the bottom to the top of the screen.
    pub span: f32,
    /// Where the tank stands on the screen, in meters from its middle.
    pub place: Vec2,
    /// Where the turret points, from the way the hull faces.
    pub turret: f32,
    /// Whether the turret looks around.
    pub sways: bool,
    /// Whether the circle that the shield of the tank reaches is drawn around it.
    pub shield: bool,
}

/// Where a tank on show stands, for what points at it.
#[derive(Clone, Copy)]
pub struct Stand {
    /// The middle of the tank on the ground.
    pub at: Vec2,
    /// The way its hull faces and the way its turret points.
    pub hull: f32,
    pub turret: f32,
}

impl Stand {
    /// The place in the world of a point of the hull, or of the turret.
    pub fn point(&self, on_turret: bool, at: Vec3) -> Vec3 {
        let origin = scene::to_world(self.at, 0.0);
        if on_turret {
            origin + Vec3::Y * models::TURRET_HEIGHT + scene::facing(self.turret) * at
        } else {
            origin + scene::facing(self.hull) * at
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, sway);
}

/// Lays the sheet under the view, until `screen` is left. `span` is how many meters of it
/// show from the bottom to the top of the screen.
pub fn sheet(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    paints: &Paints,
    view: &mut View,
    screen: Screen,
    span: f32,
) {
    view.show(FAR_AWAY, Some(span));
    // The sheet covers the screen, whichever way the view was left turned.
    let corners = [(-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (1.0, -1.0)]
        .map(|(x, y)| FAR_AWAY + Vec2::new(x, y) * 40.0);
    commands.spawn((
        Exhibit,
        DespawnOnExit(screen),
        Mesh3d(meshes.add(scene::floor_mesh(&corners))),
        MeshMaterial3d(paints.floor.clone()),
    ));
}

/// Lays the sheet and stands a tank on it, until `screen` is left. The tank faces the right
/// of the screen, inside the circle that stands for it when something hits it.
pub fn put(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    paints: &Paints,
    view: &mut View,
    screen: Screen,
    show: &Show,
) -> Stand {
    sheet(commands, meshes, paints, view, screen, show.span);
    let mut circles = Draft::default();
    let faint = theme::over(theme::FLOOR, theme::LINE, 0.6);
    circles.ring(
        Vec3::ZERO,
        (TANK_RADIUS - LINE, TANK_RADIUS),
        (24, 0.55),
        faint,
    );
    if show.shield {
        circles.ring(
            Vec3::ZERO,
            (SHIELD_RADIUS - LINE, SHIELD_RADIUS),
            (12, 0.85),
            theme::SHIELD,
        );
    }
    let hull = view.to_ground(Vec2::X).to_angle();
    let stand = Stand {
        at: FAR_AWAY + view.to_ground(show.place),
        hull,
        turret: hull + show.turret,
    };
    let tank = commands
        .spawn((
            Exhibit,
            DespawnOnExit(screen),
            Transform::from_translation(scene::to_world(stand.at, 0.0)),
            Visibility::default(),
            children![
                (
                    Mesh3d(meshes.add(circles.build())),
                    MeshMaterial3d(paints.ink.clone()),
                    Transform::from_xyz(0.0, 2.0 * LIFT, 0.0),
                ),
                (
                    Mesh3d(meshes.add(models::hull_drawing(&show.genome, show.color).build())),
                    MeshMaterial3d(paints.ink.clone()),
                    Transform::from_rotation(scene::facing(stand.hull)),
                ),
            ],
        ))
        .id();
    let turret = commands
        .spawn((
            Mesh3d(meshes.add(models::turret_drawing(&show.genome, show.color).build())),
            MeshMaterial3d(paints.ink.clone()),
            Transform {
                translation: Vec3::Y * models::TURRET_HEIGHT,
                rotation: scene::facing(stand.turret),
                ..default()
            },
            ChildOf(tank),
        ))
        .id();
    if show.sways {
        commands.entity(turret).insert(Sway(stand.turret));
    }
    stand
}

/// A turret that sways looks around.
fn sway(time: Res<Time>, mut turrets: Query<(&Sway, &mut Transform)>) {
    for (sway, mut transform) in &mut turrets {
        let look = 0.5 * (0.5 * time.elapsed_secs()).sin();
        transform.rotation = scene::facing(sway.0 + look);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stand_tells_where_the_parts_of_its_tank_are() {
        let stand = Stand {
            at: Vec2::new(10.0, -500.0),
            hull: 0.0,
            turret: std::f32::consts::FRAC_PI_2,
        };
        // A point of the hull, one meter in front of the middle of the tank.
        let front = stand.point(false, Vec3::X);
        assert!(front.distance(scene::to_world(Vec2::new(11.0, -500.0), 0.0)) < 1e-4);
        // The same point of the turret is a quarter turn further, at the height of the ring.
        let barrel = stand.point(true, Vec3::X);
        let north = scene::to_world(Vec2::new(10.0, -499.0), models::TURRET_HEIGHT);
        assert!(barrel.distance(north) < 1e-4);
    }
}
