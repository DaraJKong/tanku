//! Draws what the simulation contains: the corridor, the obstacles, the mines and the bullets.
//!
//! Gameplay is 2D. The ground plane of the simulation, with `x` east and `y` north, becomes the
//! `x`-`z` plane of the 3D world, with north along `-z`.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use tanku_core::{
    rules::{BLOCK_SIZE, BULLET_RADIUS, MINE_RADIUS},
    sim::{
        Barrier, Block, Bullet, ExplosiveCrate, Mine, MineState, Position, PreviousPosition,
        Sector, Team,
    },
    stats::Stat,
};

use crate::theme;

const WALL_HEIGHT: f32 = 1.2;
const WALL_THICKNESS: f32 = 0.4;
/// Height at which bullets fly: the height of a barrel.
const BULLET_HEIGHT: f32 = 0.8;
/// Half the length of the straight part of a bullet. Its rounded ends come on top.
const BULLET_BODY: f32 = BULLET_RADIUS;
/// Length of the pointed tip of a piercing bullet.
const DART_TIP: f32 = 0.4;
/// A dark stripe looks thinner than it is on a bright bullet, so the gaps are a little narrower.
const STRIPE_WIDTH: f32 = 0.12;
const STRIPE_GAP: f32 = 0.09;
/// Stripes are dark on a bright bullet. A vertex color multiplies the color of the material.
const STRIPE_COLOR: [f32; 4] = [0.03, 0.03, 0.04, 1.0];

pub fn to_world(point: Vec2, height: f32) -> Vec3 {
    Vec3::new(point.x, height, -point.y)
}

pub fn to_ground(world: Vec3) -> Vec2 {
    Vec2::new(world.x, -world.z)
}

/// The rotation that points a model's `+x` axis along an angle on the ground.
pub fn facing(angle: f32) -> Quat {
    Quat::from_rotation_y(angle)
}

/// Meshes of unit size, scaled where they are used.
#[derive(Resource)]
pub struct Shapes {
    pub cube: Handle<Mesh>,
    /// Diameter 1 and height 1, standing on its axis.
    pub cylinder: Handle<Mesh>,
    /// Diameter 1.
    pub sphere: Handle<Mesh>,
    /// Diameter 1 and height 1, pointing up.
    pub cone: Handle<Mesh>,
    /// Lying flat, 1 across.
    pub ring: Handle<Mesh>,
    /// Bullets at their real size, flying along `+x`. The index is the number of bounces left,
    /// which is the number of stripes.
    pub bullets: Vec<Handle<Mesh>>,
    /// The same for bullets that still pierce: they have a pointed tip.
    pub darts: Vec<Handle<Mesh>>,
}

#[derive(Resource)]
pub struct Paints {
    pub floor: Handle<StandardMaterial>,
    pub wall: Handle<StandardMaterial>,
    pub block: Handle<StandardMaterial>,
    pub explosive: Handle<StandardMaterial>,
    pub dark: Handle<StandardMaterial>,
    pub barrier_locked: Handle<StandardMaterial>,
    pub barrier_unlocked: Handle<StandardMaterial>,
    pub player_bullet: Handle<StandardMaterial>,
    pub enemy_bullet: Handle<StandardMaterial>,
    pub mine_off: Handle<StandardMaterial>,
    pub mine_on: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(PreStartup, setup).add_systems(
        Update,
        (
            (
                dress_sectors,
                dress_barriers,
                dress_obstacles,
                dress_mines,
                dress_bullets,
            ),
            (paint_barriers, blink_mines, stripe_bullets),
            place,
        )
            .chain(),
    );
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(Shapes {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cylinder: meshes.add(Cylinder::new(0.5, 1.0)),
        sphere: meshes.add(Sphere::new(0.5)),
        cone: meshes.add(Cone::new(0.5, 1.0)),
        ring: meshes.add(Torus::new(0.3, 0.5)),
        bullets: (0..=Stat::Ricochet.max_level())
            .map(|stripes| meshes.add(bullet_mesh(false, stripes)))
            .collect(),
        darts: (0..=Stat::Ricochet.max_level())
            .map(|stripes| meshes.add(bullet_mesh(true, stripes)))
            .collect(),
    });
    let mut matte = |color: Color| {
        materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.9,
            ..default()
        })
    };
    let (floor, wall, block, explosive, dark, mine_off) = (
        matte(theme::FLOOR),
        matte(theme::WALL),
        matte(theme::BLOCK),
        matte(theme::CRATE),
        matte(theme::DARK_METAL),
        matte(theme::DARK_METAL),
    );
    let mut glowing = |color: Color, alpha: f32| {
        materials.add(StandardMaterial {
            base_color: color.with_alpha(alpha),
            alpha_mode: if alpha < 1.0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            unlit: true,
            ..default()
        })
    };
    commands.insert_resource(Paints {
        floor,
        wall,
        block,
        explosive,
        dark,
        mine_off,
        barrier_locked: glowing(theme::DANGER, 0.45),
        barrier_unlocked: glowing(theme::GOOD, 0.3),
        player_bullet: glowing(theme::PLAYER_BULLET, 1.0),
        enemy_bullet: glowing(theme::ENEMY_BULLET, 1.0),
        mine_on: glowing(theme::DANGER, 1.0),
    });
}

/// The mesh of a bullet: a shape turned around its line of flight, with its stripes.
///
/// Stripes are evenly spaced and centered on the middle of the bullet.
fn bullet_mesh(pointed: bool, stripes: u8) -> Mesh {
    const AROUND: usize = 12;
    const CAP: usize = 4;
    // The outline from the tail to the tip, as a distance along the axis and a radius.
    let rounded = |forward: f32| {
        (0..=CAP).map(move |step| {
            let angle = step as f32 / CAP as f32 * std::f32::consts::FRAC_PI_2;
            let (along, radius) = (angle.cos(), angle.sin());
            (
                forward * (BULLET_BODY + BULLET_RADIUS * along),
                BULLET_RADIUS * radius,
            )
        })
    };
    let mut outline: Vec<(f32, f32)> = rounded(-1.0).collect();
    if pointed {
        outline.push((BULLET_BODY, BULLET_RADIUS));
        outline.push((BULLET_BODY + DART_TIP, 0.0));
    } else {
        outline.extend(rounded(1.0).collect::<Vec<_>>().into_iter().rev());
    }

    let edges: Vec<f32> = (0..stripes)
        .flat_map(|stripe| {
            let spacing = STRIPE_WIDTH + STRIPE_GAP;
            let center = (stripe as f32 - (stripes as f32 - 1.0) / 2.0) * spacing;
            [center - STRIPE_WIDTH / 2.0, center + STRIPE_WIDTH / 2.0]
        })
        .collect();
    // The outline is cut where a stripe starts or ends, so that each band has one color.
    let mut rings: Vec<(f32, f32)> = Vec::new();
    for ends in outline.windows(2) {
        let (from, to) = (ends[0], ends[1]);
        rings.push(from);
        for &edge in &edges {
            if edge > from.0 && edge < to.0 {
                let share = (edge - from.0) / (to.0 - from.0);
                rings.push((edge, from.1 + (to.1 - from.1) * share));
            }
        }
    }
    rings.push(outline[outline.len() - 1]);

    let (mut positions, mut normals, mut colors) = (Vec::new(), Vec::new(), Vec::new());
    for ends in rings.windows(2) {
        let (from, to) = (ends[0], ends[1]);
        let middle = (from.0 + to.0) / 2.0;
        let striped = edges
            .chunks(2)
            .any(|stripe| middle > stripe[0] && middle < stripe[1]);
        let color = if striped {
            STRIPE_COLOR
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };
        for side in 0..AROUND {
            let outward = |side: usize| {
                let angle = side as f32 / AROUND as f32 * std::f32::consts::TAU;
                Vec3::new(0.0, angle.cos(), angle.sin())
            };
            let (here, next) = (outward(side), outward(side + 1));
            let corner = |ring: (f32, f32), outward: Vec3| Vec3::X * ring.0 + outward * ring.1;
            // Two triangles, counterclockwise seen from outside. One of them is flat at the tail
            // and at the tip, where a ring is a single point.
            let triangles = [
                (from.1 > 0.0).then_some([(from, here), (from, next), (to, here)]),
                (to.1 > 0.0).then_some([(from, next), (to, next), (to, here)]),
            ];
            for (ring, outward) in triangles.into_iter().flatten().flatten() {
                positions.push(corner(ring, outward));
                normals.push(outward);
                colors.push(color);
            }
        }
    }
    let count = positions.len();
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; count])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
}

/// Cuts a simple polygon into triangles by clipping its ears.
pub fn triangulate(points: &[Vec2]) -> Vec<[u32; 3]> {
    let count = points.len();
    let area: f32 = (0..count)
        .map(|i| points[i].perp_dot(points[(i + 1) % count]))
        .sum();
    let winding = area.signum();
    // A corner on the edge of a triangle counts as inside it: cutting such an ear would leave a
    // polygon that touches itself.
    let inside = |p: Vec2, a: Vec2, b: Vec2, c: Vec2| {
        p != a
            && p != b
            && p != c
            && [(a, b), (b, c), (c, a)]
                .iter()
                .all(|&(from, to)| (to - from).perp_dot(p - from) * winding >= 0.0)
    };
    let mut left: Vec<usize> = (0..count).collect();
    let mut triangles = Vec::new();
    while left.len() > 3 {
        let size = left.len();
        let corner = |i: usize| (left[(i + size - 1) % size], left[i], left[(i + 1) % size]);
        let ear = (0..size).find(|&i| {
            let (a, b, c) = corner(i);
            let (a, b, c) = (points[a], points[b], points[c]);
            (b - a).perp_dot(c - b) * winding > 0.0
                && !left.iter().any(|&other| inside(points[other], a, b, c))
        });
        // A polygon that isn't simple has no ear left: stop with what there is.
        let Some(ear) = ear else {
            break;
        };
        let (a, b, c) = corner(ear);
        triangles.push([a as u32, b as u32, c as u32]);
        left.remove(ear);
    }
    if let [a, b, c] = left[..] {
        triangles.push([a as u32, b as u32, c as u32]);
    }
    triangles
}

fn dress_sectors(
    mut commands: Commands,
    sectors: Query<(Entity, &Sector), Added<Sector>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, sector) in &sectors {
        let plan = &sector.layout.plan;
        let outline: Vec<Vec2> = plan.outline.iter().map(|point| point.as_vec2()).collect();
        let floor = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            outline
                .iter()
                .map(|&point| to_world(point, 0.0))
                .collect::<Vec<_>>(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![Vec3::Y; outline.len()])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; outline.len()])
        .with_inserted_indices(Indices::U32(
            // The outline runs clockwise seen from above, and a face shows its
            // counterclockwise side: every triangle is turned over.
            triangulate(&outline)
                .into_iter()
                .flat_map(|[a, b, c]| [a, c, b])
                .collect(),
        ));
        commands
            .entity(entity)
            .insert((Transform::default(), Visibility::default()))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(meshes.add(floor)),
                    MeshMaterial3d(paints.floor.clone()),
                ));
                for &(a, b) in &plan.walls {
                    let (a, b) = (a.as_vec2(), b.as_vec2());
                    let along = b - a;
                    // The inner face of a wall is the line bullets bounce off. The outline runs
                    // clockwise, so the outside is on the left.
                    let outside = along.perp().normalize() * (WALL_THICKNESS / 2.0);
                    parent.spawn((
                        Mesh3d(shapes.cube.clone()),
                        MeshMaterial3d(paints.wall.clone()),
                        Transform {
                            translation: to_world((a + b) / 2.0 + outside, WALL_HEIGHT / 2.0),
                            rotation: facing(along.to_angle()),
                            scale: Vec3::new(
                                along.length() + WALL_THICKNESS,
                                WALL_HEIGHT,
                                WALL_THICKNESS,
                            ),
                        },
                    ));
                }
            });
    }
}

fn dress_barriers(
    mut commands: Commands,
    barriers: Query<(Entity, &Barrier), Added<Barrier>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
) {
    for (entity, barrier) in &barriers {
        let along = barrier.to - barrier.from;
        commands.entity(entity).insert((
            Mesh3d(shapes.cube.clone()),
            MeshMaterial3d(paints.barrier_locked.clone()),
            Transform {
                translation: to_world((barrier.from + barrier.to) / 2.0, 0.75),
                rotation: facing(along.to_angle()),
                scale: Vec3::new(along.length(), 1.5, 0.12),
            },
        ));
    }
}

fn paint_barriers(
    mut barriers: Query<(&Barrier, &mut MeshMaterial3d<StandardMaterial>), Changed<Barrier>>,
    paints: Res<Paints>,
) {
    for (barrier, mut material) in &mut barriers {
        material.0 = if barrier.unlocked {
            paints.barrier_unlocked.clone()
        } else {
            paints.barrier_locked.clone()
        };
    }
}

fn dress_obstacles(
    mut commands: Commands,
    blocks: Query<(Entity, &Position), Added<Block>>,
    crates: Query<(Entity, &Position), Added<ExplosiveCrate>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
) {
    let obstacles = blocks
        .iter()
        .map(|block| (block, &paints.block, 1.1, BLOCK_SIZE))
        .chain(
            crates
                .iter()
                .map(|explosive| (explosive, &paints.explosive, 0.9, BLOCK_SIZE - 0.2)),
        );
    for ((entity, position), paint, height, side) in obstacles {
        commands.entity(entity).insert((
            Mesh3d(shapes.cube.clone()),
            MeshMaterial3d(paint.clone()),
            Transform {
                translation: to_world(position.0, height / 2.0),
                scale: Vec3::new(side, height, side),
                ..default()
            },
        ));
    }
}

fn dress_mines(
    mut commands: Commands,
    mines: Query<(Entity, &Position), Added<Mine>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
) {
    for (entity, position) in &mines {
        commands.entity(entity).insert((
            Mesh3d(shapes.cylinder.clone()),
            MeshMaterial3d(paints.mine_off.clone()),
            Transform {
                translation: to_world(position.0, 0.06),
                scale: Vec3::new(2.0 * MINE_RADIUS, 0.12, 2.0 * MINE_RADIUS),
                ..default()
            },
        ));
    }
}

/// An armed mine blinks, and a triggered one blinks fast.
fn blink_mines(
    time: Res<Time>,
    mut mines: Query<(&Mine, &mut MeshMaterial3d<StandardMaterial>)>,
    paints: Res<Paints>,
) {
    for (mine, mut material) in &mut mines {
        let period = match mine.state {
            MineState::Arming(_) => None,
            // The blinking speeds up over the last two seconds of the fuse.
            MineState::Armed(fuse) => Some(if fuse < 2.0 { 0.25 } else { 1.0 }),
            MineState::Triggered(_) => Some(0.1),
        };
        let lit = period.is_some_and(|period| time.elapsed_secs() % period < period / 2.0);
        let wanted = if lit {
            &paints.mine_on
        } else {
            &paints.mine_off
        };
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
    }
}

/// The shape of a bullet shows what it can still do: a stripe for each bounce it has left,
/// and a pointed tip as long as it pierces.
fn bullet_shape<'a>(shapes: &'a Shapes, bullet: &Bullet) -> &'a Handle<Mesh> {
    let shapes = if bullet.pierces_left > 0 {
        &shapes.darts
    } else {
        &shapes.bullets
    };
    &shapes[(bullet.bounces_left as usize).min(shapes.len() - 1)]
}

fn dress_bullets(
    mut commands: Commands,
    bullets: Query<(Entity, &Bullet, &Position), Added<Bullet>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
) {
    for (entity, bullet, position) in &bullets {
        let paint = match bullet.team {
            Team::Players => &paints.player_bullet,
            Team::Enemies => &paints.enemy_bullet,
        };
        commands.entity(entity).insert((
            Mesh3d(bullet_shape(&shapes, bullet).clone()),
            MeshMaterial3d(paint.clone()),
            Transform {
                translation: to_world(position.0, BULLET_HEIGHT),
                rotation: facing(bullet.velocity.to_angle()),
                ..default()
            },
        ));
    }
}

/// A bullet loses a stripe when it bounces and its tip when it pierces.
fn stripe_bullets(
    mut bullets: Query<(&Bullet, &mut Mesh3d), Changed<Bullet>>,
    shapes: Res<Shapes>,
) {
    for (bullet, mut mesh) in &mut bullets {
        let wanted = bullet_shape(&shapes, bullet);
        if mesh.0 != *wanted {
            mesh.0 = wanted.clone();
        }
    }
}

/// Moves everything to where it is between two ticks of the simulation.
pub fn place(
    fixed: Res<Time<Fixed>>,
    mut moving: Query<(
        &Position,
        &PreviousPosition,
        &mut Transform,
        Option<&Bullet>,
    )>,
) {
    let progress = fixed.overstep_fraction();
    for (position, previous, mut transform, bullet) in &mut moving {
        let at = previous.0.lerp(position.0, progress);
        transform.translation = to_world(at, transform.translation.y);
        if let Some(bullet) = bullet {
            transform.rotation = facing(bullet.velocity.to_angle());
        }
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::map::Corridor;

    use super::*;

    fn area(points: &[Vec2]) -> f32 {
        let count = points.len();
        let doubled: f32 = (0..count)
            .map(|i| points[i].perp_dot(points[(i + 1) % count]))
            .sum();
        doubled.abs() / 2.0
    }

    #[test]
    fn triangles_cover_every_sector_exactly() {
        for (seed, index) in (0..8).flat_map(|seed| (0..150).map(move |index| (seed, index))) {
            let mut corridor = Corridor::new(seed);
            let plan = corridor.plan(index);
            let outline: Vec<Vec2> = plan.outline.iter().map(|point| point.as_vec2()).collect();
            // Corners in a straight line can be left over: they enclose nothing.
            let triangles = triangulate(&outline);
            let covered: f32 = triangles
                .iter()
                .map(|triangle| area(&triangle.map(|corner| outline[corner as usize])))
                .sum();
            assert!(
                (covered - area(&outline)).abs() < 0.01,
                "seed {seed}, sector {index}"
            );
        }
    }

    #[test]
    fn ground_and_world_agree() {
        let point = Vec2::new(3.0, 5.0);
        assert_eq!(to_ground(to_world(point, 2.0)), point);
        // North is away from the default camera, east is to its right.
        assert_eq!(to_world(Vec2::Y, 0.0), Vec3::NEG_Z);
        assert!((facing(std::f32::consts::FRAC_PI_2) * Vec3::X).distance(Vec3::NEG_Z) < 1e-6);
    }
}
