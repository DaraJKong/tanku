//! The models of the tanks, assembled from their genomes.
//!
//! Every stat has its own visual channel, so that players can read a tank before fighting it.
//! A model points along `+x` and stands on the ground.
//!
//! A tank is two drawings, its hull and its turret. The parts in the color of the tank are
//! filled with it, and the others are dark shapes edged with the pale lines of the sheet.

use bevy::prelude::*;
use tanku_core::{
    rules::SHIELD_RADIUS,
    sim::{Enemy, Motion, Player, Shield, ShieldState, Tank},
    stats::{Genome, SpendingGroup, Stat},
};

use crate::{
    draft::{Draft, LIFT, LINE},
    scene::{self, Shapes},
    theme,
};

/// Height of the turret ring above the ground.
pub const TURRET_HEIGHT: f32 = 0.65;
/// Height and width of the hull, whose length depends on the tracks.
const HULL: Vec2 = Vec2::new(0.4, 1.15);
/// Radius of the base of a tank that can't move, and how far tracks reach past the hull.
const BASE_RADIUS: f32 = 0.88;
const TRACK_OVERHANG: f32 = 0.25;
/// Width of the white edge that shows a shield, at its first and at its last level.
const SHIELD_EDGE: (f32, f32) = (0.06, 0.12);
/// Width of the lines along the barrel, which is too narrow for the lines of the rest.
const BARREL_LINE: f32 = 0.035;

#[derive(Component)]
struct Hull;

#[derive(Component)]
struct Turret;

#[derive(Component)]
struct Bubble;

/// The ink of a tank, which dims while the tank is dormant.
#[derive(Component)]
struct Livery(Handle<StandardMaterial>);

/// What every shield bubble is made of.
#[derive(Resource)]
struct Glass {
    bubble: Handle<StandardMaterial>,
    edge: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(
        Update,
        (build, (pose, dim, shield)).chain().before(scene::place),
    );
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    // The bubble is faint, so that the tank shows through it. A bright line marks how far
    // it reaches.
    let bubble = materials.add(StandardMaterial {
        base_color: theme::SHIELD.with_alpha(0.1),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let edge = materials.add(StandardMaterial {
        base_color: theme::SHIELD,
        unlit: true,
        ..default()
    });
    commands.insert_resource(Glass { bubble, edge });
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

/// The color of the sight and of the tip of the barrel: a light tint of the hue of the body.
fn accent_color(body: Color) -> Color {
    Color::hsl(Hsla::from(body).hue, 1.0, 0.75)
}

/// Length of the hull: agile tanks are short.
fn hull_length(genome: &Genome) -> f32 {
    2.0 - 0.1 * genome.level(Stat::Tracks) as f32
}

/// Radius of the turret: small turrets turn fast.
fn turret_radius(genome: &Genome) -> f32 {
    0.56 - 0.035 * genome.level(Stat::Turret) as f32
}

/// Width of the edge of the shield disk that shows around the turret. It grows with the level
/// and stays thin, so that it doesn't draw the eye away from everything else.
fn shield_edge(level: u8) -> f32 {
    let steps = (Stat::Shield.max_level() - 1) as f32;
    SHIELD_EDGE.0 + (SHIELD_EDGE.1 - SHIELD_EDGE.0) * (level.max(1) - 1) as f32 / steps
}

/// The drawing of the hull of a tank, with what it carries: tracks or a base, the mine rack,
/// the shield disk and the antennas.
pub fn hull_drawing(genome: &Genome, color: Color) -> Draft {
    let mut draft = Draft::default();
    let level = |stat: Stat| genome.level(stat) as f32;
    let dark = theme::DARK_METAL;
    let length = hull_length(genome);
    if genome.level(Stat::Engine) == 0 {
        // Engine: a tank that can't move sits on a fixed base.
        let (radius, height) = (BASE_RADIUS, 0.26);
        draft.post(Vec3::Y * height / 2.0, radius, height, dark, dark);
        let top = Vec3::Y * (height + LIFT);
        draft.ring(top, (radius - LINE, radius), (1, 1.0), theme::LINE);
        draft.ring(
            Vec3::Y * LIFT,
            (radius, radius + LINE),
            (1, 1.0),
            theme::LINE,
        );
    } else {
        // Engine: the faster the tank, the bigger its tracks.
        let (high, wide) = (
            0.34 + 0.035 * level(Stat::Engine),
            0.24 + 0.03 * level(Stat::Engine),
        );
        for side in [-1.0, 1.0] {
            let center = Vec3::new(0.0, high / 2.0, side * (0.52 + wide / 2.0));
            let size = Vec3::new(length + TRACK_OVERHANG, high, wide);
            draft.outlined(center, size, (dark, dark, theme::LINE));
        }
    }
    let deck = TURRET_HEIGHT;
    draft.solid(
        Vec3::new(0.0, deck - HULL.x / 2.0, 0.0),
        Vec3::new(length, HULL.x, HULL.y),
        color,
        theme::side(color),
    );
    draft.frame(
        Vec3::Y * (deck + LIFT),
        Vec2::new(length, HULL.y),
        LINE,
        theme::LINE,
    );
    // Mines: a rack at the rear, one mine per slot.
    let mines = genome.mines();
    for slot in 0..mines {
        let across = (slot as f32 - (mines as f32 - 1.0) / 2.0) * 0.27;
        let center = Vec3::new(-length / 2.0 + 0.16, deck + 0.04, across);
        draft.post(center, 0.11, 0.08, theme::LINE, theme::LINE);
        draft.disc(center + Vec3::Y * (0.04 + LIFT), 0.075, dark);
    }
    // Shield: a white disk under the turret, of which only the edge shows. The edge is wider
    // with level.
    let shield_level = genome.level(Stat::Shield);
    if shield_level > 0 {
        let radius = turret_radius(genome) + shield_edge(shield_level);
        let center = Vec3::new(0.0, deck + 0.02, 0.0);
        draft.post(center, radius, 0.04, theme::SHIELD, theme::SHIELD);
    }
    // Brain: one antenna per tier.
    for tier in 0..genome.brain() {
        let side = if tier % 2 == 0 { 1.0 } else { -1.0 };
        let back = -length / 2.0 + 0.1 + 0.22 * (tier / 2) as f32;
        let foot = Vec3::new(back, deck, side * 0.47);
        let mast = Vec3::new(LINE, 0.7, LINE);
        draft.solid(foot + Vec3::Y * 0.35, mast, theme::LINE, theme::LINE);
        draft.post(foot + Vec3::Y * 0.71, 0.06, 0.1, Color::WHITE, Color::WHITE);
    }
    draft
}

/// The drawing of the turret of a tank, with what it carries: the barrel with its rings and
/// its tip, the drum, the vents and the sight. Heights are above the turret ring.
pub fn turret_drawing(genome: &Genome, color: Color) -> Draft {
    let mut draft = Draft::default();
    let level = |stat: Stat| genome.level(stat) as f32;
    let (side, accent, dark) = (theme::side(color), accent_color(color), theme::DARK_METAL);
    let radius = turret_radius(genome);
    // Magazine: a drum behind the turret, one segment per round. The segments stand on a
    // plate, whose pale top shows around them and between them.
    let rounds = genome.magazine();
    let (first, pitch) = (-radius - 0.09, 0.12);
    let last = first - pitch * (rounds as f32 - 1.0);
    let segment = Vec3::new(0.09, 0.24, 0.36);
    draft.solid(
        Vec3::new((first + last) / 2.0, 0.155, 0.0),
        Vec3::new(
            first - last + segment.x + 2.0 * LINE,
            0.23,
            segment.z + 2.0 * LINE,
        ),
        theme::LINE,
        dark,
    );
    for round in 0..rounds {
        let back = first - pitch * round as f32;
        draft.solid(Vec3::new(back, 0.16, 0.0), segment, dark, dark);
    }
    // Muzzle: the faster the bullet, the longer the barrel.
    let barrel = 0.6 + 0.1 * level(Stat::Muzzle);
    let tip = radius - 0.1 + barrel;
    let middle = Vec3::new(tip - barrel / 2.0, 0.18, 0.0);
    draft.solid(middle, Vec3::new(barrel, 0.16, 0.16), dark, dark);
    draft.frame(
        middle + Vec3::Y * (0.08 + LIFT),
        Vec2::new(barrel, 0.16),
        BARREL_LINE,
        theme::LINE,
    );
    // Ricochet: one ring around the muzzle per bounce.
    for ring in 0..genome.bounces() {
        let center = Vec3::new(tip - 0.08 - 0.13 * ring as f32, 0.18, 0.0);
        draft.solid(center, Vec3::new(0.06, 0.3, 0.3), color, side);
    }
    // Piercing: a pointed tip on the barrel.
    if genome.piercing() > 0 {
        draft.point(Vec3::new(tip - 0.02, 0.18, 0.0), 0.1, 0.32, accent);
    }
    let roof = 0.32;
    draft.post(Vec3::new(0.0, roof / 2.0, 0.0), radius, roof, color, side);
    draft.ring(
        Vec3::Y * (roof + LIFT),
        (radius - LINE, radius),
        (1, 1.0),
        theme::LINE,
    );
    // Loader: vents on the turret sides, more with level.
    for vent in 0..genome.level(Stat::Loader) {
        let flank = if vent % 2 == 0 { 1.0 } else { -1.0 };
        let along = -0.25 + 0.15 * (vent / 2) as f32;
        let center = Vec3::new(along, roof + 0.04, flank * radius * 0.7);
        draft.solid(center, Vec3::new(0.1, 0.08, 0.12), dark, dark);
    }
    // Ballistics: a sight on the turret, one lens per tier.
    let lenses = genome.ballistics();
    for lens in 0..lenses {
        let across = (lens as f32 - (lenses as f32 - 1.0) / 2.0) * 0.17;
        let center = Vec3::new(radius * 0.45, roof + 0.06, across);
        draft.solid(center, Vec3::new(0.14, 0.11, 0.11), accent, accent);
    }
    draft
}

fn build(
    mut commands: Commands,
    tanks: Query<(Entity, &Tank, &Motion, Has<Player>), Added<Tank>>,
    shapes: Res<Shapes>,
    glass: Res<Glass>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, tank, motion, is_player) in &tanks {
        let genome = &tank.genome;
        let color = if is_player {
            theme::PLAYER
        } else {
            enemy_color(genome)
        };
        let ink = materials.add(scene::ink());
        commands
            .entity(entity)
            .insert((
                Transform::default(),
                Visibility::default(),
                Livery(ink.clone()),
            ))
            .with_children(|root| {
                root.spawn((
                    Hull,
                    Mesh3d(meshes.add(hull_drawing(genome, color).build())),
                    MeshMaterial3d(ink.clone()),
                    Transform::from_rotation(scene::facing(motion.hull)),
                ));
                root.spawn((
                    Turret,
                    Mesh3d(meshes.add(turret_drawing(genome, color).build())),
                    MeshMaterial3d(ink.clone()),
                    Transform {
                        translation: Vec3::Y * TURRET_HEIGHT,
                        rotation: scene::facing(motion.turret),
                        ..default()
                    },
                ));
                root.spawn((
                    Bubble,
                    Mesh3d(shapes.sphere.clone()),
                    MeshMaterial3d(glass.bubble.clone()),
                    Transform::from_xyz(0.0, 0.5, 0.0).with_scale(Vec3::splat(2.0 * SHIELD_RADIUS)),
                    Visibility::Hidden,
                    children![(
                        Mesh3d(shapes.hairline.clone()),
                        MeshMaterial3d(glass.edge.clone()),
                    )],
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

/// Dormant enemies are powered down: their drawing is dull until they wake up.
fn dim(
    enemies: Query<(&Enemy, &Livery), Changed<Enemy>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (enemy, livery) in &enemies {
        if let Some(mut material) = materials.get_mut(&livery.0) {
            material.base_color = if enemy.awake {
                Color::WHITE
            } else {
                theme::dormant()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn close(found: f32, wanted: f32) -> bool {
        (found - wanted).abs() < 1e-3
    }

    #[test]
    fn the_sight_and_the_tip_are_paler_than_the_body_and_of_its_hue() {
        // Hard mode has the largest budgets.
        let enemies = (0..=130).map(|budget| enemy_color(&Genome::empty(budget)));
        for body in enemies.chain([theme::PLAYER]) {
            let (body, accent) = (Hsla::from(body), Hsla::from(accent_color(body)));
            assert_eq!(accent.hue, body.hue);
            assert!(accent.lightness > body.lightness + 0.05, "{body:?}");
        }
    }

    #[test]
    fn the_edge_of_a_shield_grows_with_its_level_and_stays_thin() {
        assert_eq!(shield_edge(1), SHIELD_EDGE.0);
        assert!((shield_edge(Stat::Shield.max_level()) - SHIELD_EDGE.1).abs() < 1e-6);
        for level in 1..Stat::Shield.max_level() {
            assert!(shield_edge(level) < shield_edge(level + 1));
        }
        assert!(SHIELD_EDGE.1 <= 0.12, "the edge stays thin");
    }

    #[test]
    fn the_deck_of_the_hull_is_in_the_color_of_the_tank() {
        let color = theme::PLAYER;
        let draft = hull_drawing(&Genome::PLAYER, color);
        assert!(close(
            draft.area(color),
            hull_length(&Genome::PLAYER) * HULL.y
        ));
        // The top of the deck is where the turret turns.
        assert!(draft.area(theme::side(color)) > 0.0);
    }

    #[test]
    fn a_tank_without_an_engine_stands_on_a_round_base() {
        let still = hull_drawing(&Genome::empty(10), theme::PLAYER);
        let mut moving = Genome::empty(10);
        moving.set(Stat::Engine, 7);
        let moving = hull_drawing(&moving, theme::PLAYER);
        // The base is round, with a line around its foot.
        let (lowest, highest) = still.bounds();
        assert!(close(highest.z - lowest.z, 2.0 * (BASE_RADIUS + LINE)));
        // Tracks reach past both ends of the hull, with a line around their foot too.
        let (lowest, highest) = moving.bounds();
        let tracks = hull_length(&Genome::empty(10)) + TRACK_OVERHANG;
        assert!(close(highest.x - lowest.x, tracks + 2.0 * LINE));
    }

    #[test]
    fn the_barrel_gets_longer_with_the_muzzle() {
        let reach = |level: u8| {
            let mut genome = Genome::empty(50);
            genome.set(Stat::Muzzle, level);
            turret_drawing(&genome, theme::PLAYER).bounds().1.x
        };
        for level in 0..Stat::Muzzle.max_level() {
            assert!(close(reach(level + 1) - reach(level), 0.1));
        }
    }

    #[test]
    fn the_drum_has_one_segment_for_each_round() {
        let back_of_the_drum = |level: u8| {
            let mut genome = Genome::empty(50);
            genome.set(Stat::Magazine, level);
            turret_drawing(&genome, theme::PLAYER).bounds().0.x
        };
        // Each round pushes the back of the drum one segment further.
        for level in 0..Stat::Magazine.max_level() {
            assert!(close(
                back_of_the_drum(level) - back_of_the_drum(level + 1),
                0.12
            ));
        }
    }
}
