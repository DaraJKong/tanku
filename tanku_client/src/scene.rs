//! Draws what the simulation contains: the corridor, the obstacles, the mines and the bullets.
//!
//! Gameplay is 2D. The ground plane of the simulation, with `x` east and `y` north, becomes the
//! `x`-`z` plane of the 3D world, with north along `-z`.
//!
//! The corridor is drawn as a plan: a dark sheet with a grid, walls as bands between two lines,
//! hatched blocks and crossed crates. Nothing is lit.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use tanku_core::{
    rules::{
        BLAST_RADIUS, BLOCK_SIZE, BULLET_RADIUS, FLAG_CAPTURE_RADIUS, MINE_RADIUS,
        MINE_TRIGGER_RADIUS,
    },
    sim::{
        Barrier, Block, Bullet, ExplosiveCrate, Flag, Mine, MineState, Position, PreviousPosition,
        Run, Sector, Team,
    },
    stats::Stat,
};

use crate::{
    draft::{self, Draft, LIFT},
    theme,
};

pub const WALL_HEIGHT: f32 = 1.2;
const WALL_THICKNESS: f32 = 0.4;
const BLOCK_HEIGHT: f32 = 1.1;
/// An explosive crate is lower than a block, and a little smaller than the square it stands on.
const CRATE_HEIGHT: f32 = 0.9;
const CRATE_SIZE: f32 = BLOCK_SIZE - 0.2;
/// Width of the strips that hatch a block and of those that cross a crate.
const HATCH_WIDTH: f32 = 0.03;
const CROSS_WIDTH: f32 = 0.05;
const MINE_HEIGHT: f32 = 0.12;
const BARRIER_HEIGHT: f32 = 1.5;
/// The chain line at the foot of a barrier: a long dash, a short one, and the gap after each.
const CHAIN: [(f32, f32); 2] = [(0.9, 0.2), (0.15, 0.2)];
const CHAIN_WIDTH: f32 = 0.12;
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
/// The line that a bullet leaves behind: how long it gets and how wide it is.
const TRAIL_LENGTH: f32 = 1.8;
const TRAIL_WIDTH: f32 = 0.08;
/// The patch of light under a bullet, which shows where it is above the ground.
const POOL_SIZE: f32 = 0.9;
const FLAG_HEIGHT: f32 = 2.4;
/// Width and height of the cloth of a flag.
const FLAG_CLOTH: Vec2 = Vec2::new(1.3, 0.8);
/// One copy of the floor texture covers the square between two heavy lines of the grid.
const GRID_TILE: u32 = 5;
const GRID_TEXELS: u32 = 512;
/// Widths of the light and of the heavy lines of the grid, in meters.
const GRID_LINES: (f32, f32) = (0.03, 0.045);

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

/// Meshes shared by everything that looks the same.
#[derive(Resource)]
pub struct Shapes {
    /// 1 on each side.
    pub cube: Handle<Mesh>,
    /// Diameter 1.
    pub sphere: Handle<Mesh>,
    /// A fine ring lying flat, 1 across: the edge of a shield bubble.
    pub hairline: Handle<Mesh>,
    /// A disc lying flat, 1 across.
    disc: Handle<Mesh>,
    /// A strip lying flat, which reaches 1 behind its origin and is 1 wide.
    trail: Handle<Mesh>,
    block: Handle<Mesh>,
    explosive: Handle<Mesh>,
    mine: Handle<Mesh>,
    /// The circles around a mine and around a flag, at their real sizes.
    trigger: Handle<Mesh>,
    blast: Handle<Mesh>,
    capture: Handle<Mesh>,
    /// Bullets at their real size, flying along `+x`. The index is the number of bounces left,
    /// which is the number of stripes.
    bullets: Vec<Handle<Mesh>>,
    /// The same for bullets that still pierce: they have a pointed tip.
    darts: Vec<Handle<Mesh>>,
}

#[derive(Resource)]
pub struct Paints {
    /// Shows a drawing in the colors that it carries.
    pub ink: Handle<StandardMaterial>,
    /// The sheet with its grid.
    pub floor: Handle<StandardMaterial>,
    /// The same two for a sector that no player has entered yet.
    ink_dormant: Handle<StandardMaterial>,
    floor_dormant: Handle<StandardMaterial>,
    pole: Handle<StandardMaterial>,
    curtain_locked: Handle<StandardMaterial>,
    curtain_unlocked: Handle<StandardMaterial>,
    barrier_locked: Handle<StandardMaterial>,
    barrier_unlocked: Handle<StandardMaterial>,
    player_bullet: Handle<StandardMaterial>,
    enemy_bullet: Handle<StandardMaterial>,
    player_trail: Handle<StandardMaterial>,
    enemy_trail: Handle<StandardMaterial>,
    player_pool: Handle<StandardMaterial>,
    enemy_pool: Handle<StandardMaterial>,
    mine_light: Handle<StandardMaterial>,
    trigger: Handle<StandardMaterial>,
    blast: Handle<StandardMaterial>,
    blast_fill: Handle<StandardMaterial>,
    flag: Handle<StandardMaterial>,
    flag_captured: Handle<StandardMaterial>,
    flag_fill: Handle<StandardMaterial>,
    flag_fill_captured: Handle<StandardMaterial>,
}

/// What a part of the corridor is drawn with, which is dull until a player enters its sector:
/// its floor, its walls and its obstacles.
#[derive(Component, Clone, Copy)]
struct Ground {
    sector: u32,
    floor: bool,
}

/// The drawing of the walls of a sector, which is done again when its neighbors change.
#[derive(Component)]
struct Walls;

/// The two parts of a barrier, which change color when it unlocks.
#[derive(Component, Clone, Copy)]
enum BarrierPart {
    Curtain,
    Lines,
}

/// The parts of a mine that come and go with its state.
#[derive(Component, Clone, Copy, PartialEq)]
enum MinePart {
    /// The light in its middle, which blinks.
    Light,
    /// The circle inside which an opposing tank sets it off.
    Trigger,
    /// The circle that its blast reaches.
    Blast,
}

/// The parts of a flag that change color when it is captured: its cloth and its circle, and
/// the faint fill of the circle.
#[derive(Component, Clone, Copy)]
enum FlagPart {
    Solid,
    Fill,
}

/// The line behind a bullet.
#[derive(Component, Default)]
struct Trail {
    length: f32,
    /// The way the bullet flew when the line started.
    heading: Vec2,
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
                dress_flags,
            ),
            (
                wake_sectors,
                paint_barriers,
                blink_mines,
                stripe_bullets,
                trail_bullets,
                paint_flags,
            ),
            place,
        )
            .chain(),
    );
}

/// The material that shows a drawing in the colors that it carries, from both sides.
pub fn ink() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        cull_mode: None,
        ..default()
    }
}

/// A flat color, see-through if it has an alpha.
pub fn paint(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        alpha_mode: if color.alpha() < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        unlit: true,
        cull_mode: None,
        ..default()
    })
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // A drawing in white takes the color of its material.
    let mut circle = |radius: f32, width: f32, dashes: (usize, f32)| {
        let mut draft = Draft::default();
        draft.ring(Vec3::ZERO, (radius - width, radius), dashes, Color::WHITE);
        meshes.add(draft.build())
    };
    let (trigger, blast, capture) = (
        circle(MINE_TRIGGER_RADIUS, draft::LINE, (28, 0.5)),
        circle(BLAST_RADIUS, 0.06, (1, 1.0)),
        circle(FLAG_CAPTURE_RADIUS, 0.06, (1, 1.0)),
    );
    let mut disc = Draft::default();
    disc.disc(Vec3::ZERO, 0.5, Color::WHITE);
    let mut trail = Draft::default();
    trail.line(Vec3::NEG_X, Vec3::ZERO, 1.0, Color::WHITE);
    commands.insert_resource(Shapes {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        sphere: meshes.add(Sphere::new(0.5)),
        hairline: meshes.add(Torus::new(0.487, 0.5)),
        disc: meshes.add(disc.build()),
        trail: meshes.add(trail.build()),
        block: meshes.add(block_drawing().build()),
        explosive: meshes.add(crate_drawing().build()),
        mine: meshes.add(mine_drawing().build()),
        trigger,
        blast,
        capture,
        bullets: (0..=Stat::Ricochet.max_level())
            .map(|stripes| meshes.add(bullet_mesh(false, stripes)))
            .collect(),
        darts: (0..=Stat::Ricochet.max_level())
            .map(|stripes| meshes.add(bullet_mesh(true, stripes)))
            .collect(),
    });

    let grid = images.add(grid_texture());
    let sheet = |light: Color| StandardMaterial {
        base_color: light,
        base_color_texture: Some(grid.clone()),
        ..ink()
    };
    let materials = &mut *materials;
    commands.insert_resource(Paints {
        ink: materials.add(ink()),
        floor: materials.add(sheet(Color::WHITE)),
        ink_dormant: materials.add(StandardMaterial {
            base_color: theme::dormant(),
            ..ink()
        }),
        floor_dormant: materials.add(sheet(theme::dormant())),
        pole: paint(materials, theme::WALL),
        curtain_locked: paint(materials, theme::DANGER.with_alpha(0.2)),
        curtain_unlocked: paint(materials, theme::GOOD.with_alpha(0.12)),
        barrier_locked: paint(materials, theme::DANGER),
        barrier_unlocked: paint(materials, theme::GOOD),
        player_bullet: paint(materials, theme::PLAYER_BULLET),
        enemy_bullet: paint(materials, theme::ENEMY_BULLET),
        player_trail: paint(materials, theme::PLAYER_BULLET.with_alpha(0.45)),
        enemy_trail: paint(materials, theme::ENEMY_BULLET.with_alpha(0.45)),
        player_pool: paint(materials, theme::PLAYER_BULLET.with_alpha(0.13)),
        enemy_pool: paint(materials, theme::ENEMY_BULLET.with_alpha(0.13)),
        mine_light: paint(materials, theme::DANGER),
        trigger: paint(materials, theme::DANGER.with_alpha(0.6)),
        blast: paint(materials, theme::DANGER),
        blast_fill: paint(materials, theme::DANGER.with_alpha(0.1)),
        flag: paint(materials, theme::FLAG),
        flag_captured: paint(materials, theme::GOOD),
        flag_fill: paint(materials, theme::FLAG.with_alpha(0.06)),
        flag_fill_captured: paint(materials, theme::GOOD.with_alpha(0.08)),
    });
}

/// How much of a stretch of floor, from `from` to `to` meters along one axis of the tile, the
/// lines of the grid cover: 0 for none, and the strength of a line where it covers it all.
fn grid_cover(from: f32, to: f32) -> f32 {
    (0..=GRID_TILE)
        .map(|line| {
            // The two ends of the tile are the two halves of the same heavy line.
            let heavy = line == 0 || line == GRID_TILE;
            let (width, strength) = if heavy {
                (GRID_LINES.1, theme::GRID_STRENGTH.1)
            } else {
                (GRID_LINES.0, theme::GRID_STRENGTH.0)
            };
            let (low, high) = (line as f32 - width / 2.0, line as f32 + width / 2.0);
            strength * (to.min(high) - from.max(low)).max(0.0) / (to - from)
        })
        .sum()
}

/// The floor: the sheet with its grid, a line every meter and a heavier one every five.
///
/// Every level of detail is computed from the lines themselves, so that they fade as they
/// get thinner than a pixel instead of flickering.
fn grid_texture() -> Image {
    let (mut data, mut levels, mut size) = (Vec::new(), 0, GRID_TEXELS);
    loop {
        let texel = GRID_TILE as f32 / size as f32;
        let cover: Vec<f32> = (0..size)
            .map(|index| grid_cover(index as f32 * texel, (index + 1) as f32 * texel))
            .collect();
        for &down in &cover {
            for &across in &cover {
                // Where two lines cross, they don't add up.
                let strength = across + down - across * down;
                let color = theme::over(theme::FLOOR, theme::GRID, strength).to_srgba();
                data.extend(color.to_u8_array());
            }
        }
        levels += 1;
        if size == 1 {
            break;
        }
        size /= 2;
    }
    let mut image = Image::new_uninit(
        Extent3d {
            width: GRID_TEXELS,
            height: GRID_TEXELS,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

/// A wall as it is drawn: a band along the outside of the line that bullets bounce off.
struct Band {
    /// The two ends of that line.
    ends: (Vec2, Vec2),
    /// Where the outer edge of a band 1 m thick would end, seen from each end of the line. The
    /// two bands of a corner meet on its bisector, and a wall that stops is cut square.
    reach: (Vec2, Vec2),
    /// Whether the wall stops at each end, as it does at a gate, with no wall that goes on.
    stops: (bool, bool),
}

/// The bands of the walls of a sector. `around` holds the walls of the sectors next to it: a
/// wall that goes on in another sector is cut to fit the wall it meets there, as at any other
/// corner. Two walls that were each cut square would cover the same ground where a sector ends
/// on a corner.
///
/// The outline of a sector runs clockwise, so the outside of a wall is on its left.
fn wall_bands(walls: &[(Vec2, Vec2)], around: &[(Vec2, Vec2)]) -> Vec<Band> {
    let outward = |(from, to): (Vec2, Vec2)| (to - from).perp().normalize();
    let band = |&wall: &(Vec2, Vec2)| {
        let normal = outward(wall);
        let reach = |neighbor: Option<&(Vec2, Vec2)>| {
            let Some(&neighbor) = neighbor else {
                return normal;
            };
            let fold = 1.0 + normal.dot(outward(neighbor));
            if fold < 0.05 {
                return normal;
            }
            ((normal + outward(neighbor)) / fold).clamp_length_max(3.0)
        };
        let before = walls.iter().chain(around).find(|other| other.1 == wall.0);
        let after = walls.iter().chain(around).find(|other| other.0 == wall.1);
        Band {
            ends: wall,
            reach: (reach(before), reach(after)),
            stops: (before.is_none(), after.is_none()),
        }
    };
    walls.iter().map(band).collect()
}

/// The drawing of the walls of a sector, next to the walls of its neighbors: each wall is a
/// band between two lines, and a third line marks its foot on the floor.
fn wall_drawing(walls: &[(Vec2, Vec2)], around: &[(Vec2, Vec2)]) -> Draft {
    let mut draft = Draft::default();
    for Band { ends, reach, stops } in wall_bands(walls, around) {
        let (a, b) = ends;
        let (a_out, b_out) = (a + reach.0 * WALL_THICKNESS, b + reach.1 * WALL_THICKNESS);
        let band = |draft: &mut Draft, ends: [Vec2; 4], height: f32, color: Color| {
            draft.face(&ends.map(|point| to_world(point, height)), color);
        };
        let rise = |draft: &mut Draft, from: Vec2, to: Vec2| {
            let corners = [
                (from, 0.0),
                (to, 0.0),
                (to, WALL_HEIGHT),
                (from, WALL_HEIGHT),
            ];
            draft.face(
                &corners.map(|(point, height)| to_world(point, height)),
                theme::WALL_SIDE,
            );
        };
        band(&mut draft, [a, b, b_out, a_out], WALL_HEIGHT, theme::WALL);
        rise(&mut draft, a, b);
        rise(&mut draft, a_out, b_out);
        if stops.0 {
            rise(&mut draft, a, a_out);
        }
        if stops.1 {
            rise(&mut draft, b, b_out);
        }
        let (inside_a, inside_b) = (reach.0 * draft::LINE, reach.1 * draft::LINE);
        let top = WALL_HEIGHT + LIFT;
        band(
            &mut draft,
            [a, b, b + inside_b, a + inside_a],
            top,
            theme::LINE,
        );
        band(
            &mut draft,
            [a_out, b_out, b_out - inside_b, a_out - inside_a],
            top,
            theme::LINE,
        );
        band(
            &mut draft,
            [a, b, b - inside_b, a - inside_a],
            LIFT,
            theme::LINE,
        );
    }
    draft
}

/// The drawing of a destructible block: hatched, as a wall that can be taken away.
fn block_drawing() -> Draft {
    let mut draft = Draft::default();
    let (center, size) = (
        Vec3::Y * BLOCK_HEIGHT / 2.0,
        Vec3::new(BLOCK_SIZE, BLOCK_HEIGHT, BLOCK_SIZE),
    );
    draft.outlined(
        center,
        size,
        (theme::BLOCK, theme::BLOCK_SIDE, theme::BLOCK_LINE),
    );
    draft.corners(center, size, theme::BLOCK_LINE);
    // The hatching: strips at 45 degrees, half a meter apart along the edges, each cut where
    // it meets the edges of the top. Under the outline, which hides their ends.
    let (half, top) = (BLOCK_SIZE / 2.0, BLOCK_HEIGHT + LIFT / 2.0);
    let cut = HATCH_WIDTH / std::f32::consts::SQRT_2;
    for step in 0..8 {
        // The strip runs along the line `x + z = shift`, from one edge to the next.
        let shift = -1.75 * half + 0.5 * step as f32;
        let (first, second) = if shift < 0.0 {
            // From the west edge to the north edge.
            let (z, x) = (shift + half, shift + half);
            ((-half, z), (x, -half))
        } else {
            // From the south edge to the east edge.
            let (x, z) = (shift - half, shift - half);
            ((x, half), (half, z))
        };
        let along_edges = if shift < 0.0 {
            [(0.0, -cut), (0.0, cut), (cut, 0.0), (-cut, 0.0)]
        } else {
            [(-cut, 0.0), (cut, 0.0), (0.0, cut), (0.0, -cut)]
        };
        let ends = [first, first, second, second];
        let corners: Vec<Vec3> = ends
            .iter()
            .zip(along_edges)
            .map(|(&(x, z), (dx, dz))| Vec3::new(x + dx, top, z + dz))
            .collect();
        draft.face(&corners, theme::HATCH);
    }
    draft
}

/// The drawing of an explosive crate: a box with a cross, in the color of danger.
fn crate_drawing() -> Draft {
    let mut draft = Draft::default();
    let (center, size) = (
        Vec3::Y * CRATE_HEIGHT / 2.0,
        Vec3::new(CRATE_SIZE, CRATE_HEIGHT, CRATE_SIZE),
    );
    draft.outlined(
        center,
        size,
        (theme::CRATE, theme::CRATE_SIDE, theme::DANGER),
    );
    draft.corners(center, size, theme::DANGER);
    let (reach, top) = (CRATE_SIZE / 2.0 - draft::LINE, CRATE_HEIGHT + LIFT / 2.0);
    for turn in [-1.0, 1.0] {
        draft.line(
            Vec3::new(-reach, top, -reach * turn),
            Vec3::new(reach, top, reach * turn),
            CROSS_WIDTH,
            theme::DANGER,
        );
    }
    draft
}

/// The drawing of a mine, without its light: a dark disc with two rings.
fn mine_drawing() -> Draft {
    let mut draft = Draft::default();
    draft.post(
        Vec3::Y * MINE_HEIGHT / 2.0,
        MINE_RADIUS,
        MINE_HEIGHT,
        theme::DARK_METAL,
        theme::DARK_METAL,
    );
    let top = Vec3::Y * (MINE_HEIGHT + LIFT);
    draft.ring(
        top,
        (MINE_RADIUS - draft::LINE, MINE_RADIUS),
        (1, 1.0),
        theme::LINE,
    );
    let faint = theme::over(theme::DARK_METAL, theme::LINE, 0.5);
    draft.ring(top, (0.17, 0.2), (1, 1.0), faint);
    draft
}

/// The lines of a barrier of this length, drawn along `x` in white: a chain line at its foot
/// and a fine line along the top of its curtain.
fn barrier_drawing(length: f32) -> Draft {
    let mut draft = Draft::default();
    let half = length / 2.0;
    let mut from = -half;
    'chain: loop {
        for (dash, gap) in CHAIN {
            if from >= half {
                break 'chain;
            }
            let to = (from + dash).min(half);
            draft.line(
                Vec3::new(from, 2.0 * LIFT, 0.0),
                Vec3::new(to, 2.0 * LIFT, 0.0),
                CHAIN_WIDTH,
                Color::WHITE,
            );
            from = to + gap;
        }
    }
    draft.line(
        Vec3::new(-half, BARRIER_HEIGHT, 0.0),
        Vec3::new(half, BARRIER_HEIGHT, 0.0),
        draft::LINE,
        Color::WHITE,
    );
    draft
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

/// The floor inside an outline, for the paint that lays the grid on it.
pub fn floor_mesh(outline: &[Vec2]) -> Mesh {
    Mesh::new(
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
    // The grid is laid on the lattice of the map: a line on every meter.
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        outline
            .iter()
            .map(|&point| point / GRID_TILE as f32)
            .collect::<Vec<_>>(),
    )
    .with_inserted_indices(Indices::U32(
        // The outline runs clockwise seen from above, and a face shows its
        // counterclockwise side: every triangle is turned over.
        triangulate(outline)
            .into_iter()
            .flat_map(|[a, b, c]| [a, c, b])
            .collect(),
    ))
}

/// The walls of a sector, as the drawing takes them.
fn walls_of(sector: &Sector) -> Vec<(Vec2, Vec2)> {
    let walls = sector.layout.plan.walls.iter();
    walls.map(|&(a, b)| (a.as_vec2(), b.as_vec2())).collect()
}

/// Gives a new sector its floor and its walls. The walls of a sector are cut to fit those of
/// the sectors next to it, so all of them are drawn again when a sector comes or goes.
fn dress_sectors(
    mut commands: Commands,
    sectors: Query<(Entity, &Sector, Option<&Children>)>,
    arrivals: Query<(), Added<Sector>>,
    mut departures: RemovedComponents<Sector>,
    mut drawn: Query<&mut Mesh3d, With<Walls>>,
    paints: Res<Paints>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let gone = departures.read().count();
    if arrivals.is_empty() && gone == 0 {
        return;
    }
    let loaded: Vec<(u32, Vec<(Vec2, Vec2)>)> = sectors
        .iter()
        .map(|(_, sector, _)| (sector.layout.plan.index, walls_of(sector)))
        .collect();
    for (entity, sector, children) in &sectors {
        let plan = &sector.layout.plan;
        let index = plan.index;
        let around: Vec<(Vec2, Vec2)> = loaded
            .iter()
            .filter(|(other, _)| other.abs_diff(index) == 1)
            .flat_map(|(_, walls)| walls.iter().copied())
            .collect();
        let walls = meshes.add(wall_drawing(&walls_of(sector), &around).build());
        if !arrivals.contains(entity) {
            // Only the walls of this sector take the new drawing. Its obstacles are part of
            // its ground too, and they keep theirs.
            for child in children.into_iter().flatten() {
                if let Ok(mut mesh) = drawn.get_mut(*child) {
                    mesh.0 = walls.clone();
                }
            }
            continue;
        }
        let outline: Vec<Vec2> = plan.outline.iter().map(|point| point.as_vec2()).collect();
        let floor = meshes.add(floor_mesh(&outline));
        commands
            .entity(entity)
            .insert((Transform::default(), Visibility::default()))
            .with_children(|parent| {
                parent.spawn((
                    Ground {
                        sector: index,
                        floor: true,
                    },
                    Mesh3d(floor),
                    MeshMaterial3d(paints.floor_dormant.clone()),
                ));
                parent.spawn((
                    Walls,
                    Ground {
                        sector: index,
                        floor: false,
                    },
                    Mesh3d(walls),
                    MeshMaterial3d(paints.ink_dormant.clone()),
                ));
            });
    }
}

fn dress_barriers(
    mut commands: Commands,
    barriers: Query<(Entity, &Barrier), Added<Barrier>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, barrier) in &barriers {
        let along = barrier.to - barrier.from;
        let length = along.length();
        commands.entity(entity).insert((
            Transform {
                translation: to_world((barrier.from + barrier.to) / 2.0, 0.0),
                rotation: facing(along.to_angle()),
                ..default()
            },
            Visibility::default(),
            children![
                (
                    BarrierPart::Curtain,
                    Mesh3d(shapes.cube.clone()),
                    MeshMaterial3d(paints.curtain_locked.clone()),
                    Transform::from_xyz(0.0, BARRIER_HEIGHT / 2.0, 0.0).with_scale(Vec3::new(
                        length,
                        BARRIER_HEIGHT,
                        CHAIN_WIDTH
                    )),
                ),
                (
                    BarrierPart::Lines,
                    Mesh3d(meshes.add(barrier_drawing(length).build())),
                    MeshMaterial3d(paints.barrier_locked.clone()),
                ),
            ],
        ));
    }
}

/// A barrier changes color when it unlocks.
fn paint_barriers(
    barriers: Query<(&Barrier, &Children), Changed<Barrier>>,
    mut parts: Query<(&BarrierPart, &mut MeshMaterial3d<StandardMaterial>)>,
    paints: Res<Paints>,
) {
    for (barrier, children) in &barriers {
        for child in children.iter() {
            let Ok((part, mut material)) = parts.get_mut(child) else {
                continue;
            };
            material.0 = match (part, barrier.unlocked) {
                (BarrierPart::Curtain, false) => &paints.curtain_locked,
                (BarrierPart::Curtain, true) => &paints.curtain_unlocked,
                (BarrierPart::Lines, false) => &paints.barrier_locked,
                (BarrierPart::Lines, true) => &paints.barrier_unlocked,
            }
            .clone();
        }
    }
}

fn dress_obstacles(
    mut commands: Commands,
    blocks: Query<(Entity, &Block, &Position), Added<Block>>,
    crates: Query<(Entity, &ExplosiveCrate, &Position), Added<ExplosiveCrate>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
) {
    let blocks = blocks
        .iter()
        .map(|(entity, block, position)| (entity, block.sector, position, &shapes.block));
    let crates = crates.iter().map(|(entity, explosive, position)| {
        (entity, explosive.sector, position, &shapes.explosive)
    });
    for (entity, sector, position, shape) in blocks.chain(crates) {
        commands.entity(entity).insert((
            Ground {
                sector,
                floor: false,
            },
            Mesh3d(shape.clone()),
            MeshMaterial3d(paints.ink_dormant.clone()),
            Transform::from_translation(to_world(position.0, 0.0)),
        ));
    }
}

/// Whether a sector is still waiting for its first player: the players are in an earlier one.
pub fn is_dormant(sector: u32, run: Option<&Run>) -> bool {
    run.is_some_and(|run| sector > run.sector)
}

/// A sector is drawn dull until a player enters it. Then the drawing comes to life.
fn wake_sectors(
    run: Option<Res<Run>>,
    paints: Res<Paints>,
    mut grounds: Query<(&Ground, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (ground, mut material) in &mut grounds {
        let wanted = match (ground.floor, is_dormant(ground.sector, run.as_deref())) {
            (true, false) => &paints.floor,
            (true, true) => &paints.floor_dormant,
            (false, false) => &paints.ink,
            (false, true) => &paints.ink_dormant,
        };
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
    }
}

fn dress_mines(
    mut commands: Commands,
    mines: Query<(Entity, &Position), Added<Mine>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
) {
    for (entity, position) in &mines {
        // A circle lies just above the floor, and the blast one has a faint fill.
        let reach = Transform::from_xyz(0.0, 3.0 * LIFT, 0.0);
        commands.entity(entity).insert((
            Mesh3d(shapes.mine.clone()),
            MeshMaterial3d(paints.ink.clone()),
            Transform::from_translation(to_world(position.0, 0.0)),
            children![
                (
                    MinePart::Light,
                    Mesh3d(shapes.disc.clone()),
                    MeshMaterial3d(paints.mine_light.clone()),
                    Transform::from_xyz(0.0, MINE_HEIGHT + 2.0 * LIFT, 0.0)
                        .with_scale(Vec3::new(0.24, 1.0, 0.24)),
                    Visibility::Hidden,
                ),
                (
                    MinePart::Trigger,
                    Mesh3d(shapes.trigger.clone()),
                    MeshMaterial3d(paints.trigger.clone()),
                    reach,
                    Visibility::Hidden,
                ),
                (
                    MinePart::Blast,
                    Mesh3d(shapes.blast.clone()),
                    MeshMaterial3d(paints.blast.clone()),
                    reach,
                    Visibility::Hidden,
                ),
                (
                    MinePart::Blast,
                    Mesh3d(shapes.disc.clone()),
                    MeshMaterial3d(paints.blast_fill.clone()),
                    reach.with_scale(Vec3::new(2.0 * BLAST_RADIUS, 1.0, 2.0 * BLAST_RADIUS)),
                    Visibility::Hidden,
                ),
            ],
        ));
    }
}

/// An armed mine blinks inside the circle that sets it off. A triggered one blinks fast, and
/// shows how far its blast will reach.
fn blink_mines(
    time: Res<Time>,
    mines: Query<(&Mine, &Children)>,
    mut parts: Query<(&MinePart, &mut Visibility)>,
) {
    for (mine, children) in &mines {
        let period = match mine.state {
            MineState::Arming(_) => None,
            // The blinking speeds up over the last two seconds of the fuse.
            MineState::Armed(fuse) => Some(if fuse < 2.0 { 0.25 } else { 1.0 }),
            MineState::Triggered(_) => Some(0.1),
        };
        let lit = period.is_some_and(|period| time.elapsed_secs() % period < period / 2.0);
        for child in children.iter() {
            let Ok((part, mut visibility)) = parts.get_mut(child) else {
                continue;
            };
            let shown = match part {
                MinePart::Light => lit,
                MinePart::Trigger => matches!(mine.state, MineState::Armed(_)),
                MinePart::Blast => matches!(mine.state, MineState::Triggered(_)),
            };
            let wanted = if shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility != wanted {
                *visibility = wanted;
            }
        }
    }
}

fn flag_paint<'a>(paints: &'a Paints, part: FlagPart, flag: &Flag) -> &'a Handle<StandardMaterial> {
    match (part, flag.captured) {
        (FlagPart::Solid, false) => &paints.flag,
        (FlagPart::Solid, true) => &paints.flag_captured,
        (FlagPart::Fill, false) => &paints.flag_fill,
        (FlagPart::Fill, true) => &paints.flag_fill_captured,
    }
}

fn dress_flags(
    mut commands: Commands,
    flags: Query<(Entity, &Flag, &Position), Added<Flag>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
) {
    for (entity, flag, position) in &flags {
        let solid = flag_paint(&paints, FlagPart::Solid, flag);
        commands.entity(entity).insert((
            Transform::from_translation(to_world(position.0, 0.0)),
            Visibility::default(),
            children![
                // The pole.
                (
                    Mesh3d(shapes.cube.clone()),
                    MeshMaterial3d(paints.pole.clone()),
                    Transform {
                        translation: Vec3::Y * FLAG_HEIGHT / 2.0,
                        scale: Vec3::new(0.1, FLAG_HEIGHT, 0.1),
                        ..default()
                    },
                ),
                // The cloth leans back, so that it shows from above whichever way the view
                // is turned.
                (
                    FlagPart::Solid,
                    Mesh3d(shapes.cube.clone()),
                    MeshMaterial3d(solid.clone()),
                    Transform {
                        translation: Vec3::new(FLAG_CLOTH.x / 2.0 + 0.07, FLAG_HEIGHT - 0.25, 0.0),
                        rotation: Quat::from_rotation_x(-0.95),
                        scale: Vec3::new(FLAG_CLOTH.x, FLAG_CLOTH.y, 0.06),
                    },
                ),
                // The circle on the ground: a tank inside it captures the flag.
                (
                    FlagPart::Solid,
                    Mesh3d(shapes.capture.clone()),
                    MeshMaterial3d(solid.clone()),
                    Transform::from_xyz(0.0, 3.0 * LIFT, 0.0),
                ),
                (
                    FlagPart::Fill,
                    Mesh3d(shapes.disc.clone()),
                    MeshMaterial3d(flag_paint(&paints, FlagPart::Fill, flag).clone()),
                    Transform::from_xyz(0.0, 2.0 * LIFT, 0.0).with_scale(Vec3::new(
                        2.0 * FLAG_CAPTURE_RADIUS,
                        1.0,
                        2.0 * FLAG_CAPTURE_RADIUS
                    )),
                ),
            ],
        ));
    }
}

/// A captured flag changes color.
fn paint_flags(
    flags: Query<(&Flag, &Children), Changed<Flag>>,
    mut parts: Query<(&FlagPart, &mut MeshMaterial3d<StandardMaterial>)>,
    paints: Res<Paints>,
) {
    for (flag, children) in &flags {
        for child in children.iter() {
            if let Ok((part, mut material)) = parts.get_mut(child) {
                material.0 = flag_paint(&paints, *part, flag).clone();
            }
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
        let (paint, trail, pool) = match bullet.team {
            Team::Players => (
                &paints.player_bullet,
                &paints.player_trail,
                &paints.player_pool,
            ),
            Team::Enemies => (
                &paints.enemy_bullet,
                &paints.enemy_trail,
                &paints.enemy_pool,
            ),
        };
        commands.entity(entity).insert((
            Mesh3d(bullet_shape(&shapes, bullet).clone()),
            MeshMaterial3d(paint.clone()),
            Transform {
                translation: to_world(position.0, BULLET_HEIGHT),
                rotation: facing(bullet.velocity.to_angle()),
                ..default()
            },
            children![
                (
                    Trail::default(),
                    Mesh3d(shapes.trail.clone()),
                    MeshMaterial3d(trail.clone()),
                    Transform::from_scale(Vec3::new(0.0, 1.0, TRAIL_WIDTH)),
                ),
                // On the floor, under the bullet.
                (
                    Mesh3d(shapes.disc.clone()),
                    MeshMaterial3d(pool.clone()),
                    Transform::from_xyz(0.0, 4.0 * LIFT - BULLET_HEIGHT, 0.0)
                        .with_scale(Vec3::new(POOL_SIZE, 1.0, POOL_SIZE)),
                ),
            ],
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

/// How long the line behind a bullet is after this much more flight. The line starts over when
/// the bullet turns, which it only does by bouncing: a line never crosses a wall.
fn trail_after(trail: &mut Trail, velocity: Vec2, seconds: f32) -> f32 {
    let heading = velocity.normalize_or_zero();
    if heading.dot(trail.heading) < 0.999 {
        trail.length = 0.0;
        trail.heading = heading;
    }
    trail.length = (trail.length + velocity.length() * seconds).min(TRAIL_LENGTH);
    trail.length
}

fn trail_bullets(
    time: Res<Time>,
    bullets: Query<(&Bullet, &Children)>,
    mut trails: Query<(&mut Trail, &mut Transform)>,
) {
    for (bullet, children) in &bullets {
        for child in children.iter() {
            if let Ok((mut trail, mut transform)) = trails.get_mut(child) {
                transform.scale.x = trail_after(&mut trail, bullet.velocity, time.delta_secs());
            }
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
    use bevy::mesh::VertexAttributeValues;
    use tanku_core::{map::Corridor, seed::seed_value};

    use super::*;

    fn area(points: &[Vec2]) -> f32 {
        let count = points.len();
        let doubled: f32 = (0..count)
            .map(|i| points[i].perp_dot(points[(i + 1) % count]))
            .sum();
        doubled.abs() / 2.0
    }

    fn close(found: f32, wanted: f32) -> bool {
        (found - wanted).abs() < 1e-3
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
    fn the_grid_has_a_line_every_meter_and_a_heavier_one_every_five() {
        let (light, heavy) = theme::GRID_STRENGTH;
        // A stretch inside a line is covered at the strength of that line.
        assert!(close(grid_cover(0.99, 1.01), light));
        assert!(close(grid_cover(0.0, 0.01), heavy));
        assert!(close(grid_cover(4.99, 5.0), heavy));
        // Between two lines there is nothing.
        assert_eq!(grid_cover(1.4, 1.6), 0.0);
        // Seen from far away, the lines blend into the floor.
        let tile = grid_cover(0.0, GRID_TILE as f32);
        assert!(tile > 0.0 && tile < 0.01, "{tile}");
    }

    #[test]
    fn the_grid_texture_holds_every_level_of_detail() {
        let image = grid_texture();
        let texels: u32 = (0..image.texture_descriptor.mip_level_count)
            .map(|level| (GRID_TEXELS >> level).pow(2))
            .sum();
        assert_eq!(
            GRID_TEXELS >> (image.texture_descriptor.mip_level_count - 1),
            1
        );
        assert_eq!(image.data.as_ref().unwrap().len() as u32, 4 * texels);
    }

    #[test]
    fn the_walls_of_a_room_meet_at_its_corners() {
        // A square room of 10 m, clockwise from its south-west corner.
        let corners = [(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)].map(Vec2::from);
        let walls: Vec<(Vec2, Vec2)> = (0..4).map(|i| (corners[i], corners[(i + 1) % 4])).collect();
        let draft = wall_drawing(&walls, &[]);
        // The bands form a square ring around the room, with nothing missing at the corners
        // and nothing twice.
        let outside = 10.0 + 2.0 * WALL_THICKNESS;
        assert!(close(draft.area(theme::WALL), outside * outside - 100.0));
        let (lowest, highest) = draft.bounds();
        assert!(close(lowest.x, -WALL_THICKNESS) && close(highest.x, 10.0 + WALL_THICKNESS));
        // North is along `-z`.
        assert!(close(lowest.z, -10.0 - WALL_THICKNESS) && close(highest.z, WALL_THICKNESS));
    }

    #[test]
    fn a_wall_that_stops_is_cut_square() {
        let draft = wall_drawing(&[(Vec2::ZERO, Vec2::new(0.0, 6.0))], &[]);
        assert!(close(draft.area(theme::WALL), 6.0 * WALL_THICKNESS));
        // Its two faces, and one at each end.
        let faces = 2.0 * 6.0 * WALL_HEIGHT + 2.0 * WALL_THICKNESS * WALL_HEIGHT;
        assert!(close(draft.area(theme::WALL_SIDE), faces));
    }

    #[test]
    fn the_walls_of_two_sectors_are_cut_to_fit_each_other() {
        // A sector ends on a corner: its wall runs east to the end of its barrier, and the
        // wall of the next sector goes north from there. The outsides are on the left.
        let first = [(Vec2::new(-6.0, 0.0), Vec2::ZERO)];
        let second = [(Vec2::ZERO, Vec2::new(0.0, 6.0))];
        for (walls, around) in [(first, second), (second, first)] {
            let draft = wall_drawing(&walls, &around);
            // Each band gives up half of the square that both would cover.
            let band = 6.0 * WALL_THICKNESS - WALL_THICKNESS * WALL_THICKNESS / 2.0;
            assert!(close(draft.area(theme::WALL), band));
            // Its inner face, its shorter outer one, and one face at the end where it stops.
            let faces = (6.0 + (6.0 - WALL_THICKNESS) + WALL_THICKNESS) * WALL_HEIGHT;
            assert!(close(draft.area(theme::WALL_SIDE), faces));
        }
    }

    #[test]
    fn generated_walls_meet_their_neighbors_on_one_cut() {
        let walls_of = |corridor: &mut Corridor, index: u32| -> Vec<(Vec2, Vec2)> {
            let walls = corridor.plan(index).walls.iter();
            walls.map(|&(a, b)| (a.as_vec2(), b.as_vec2())).collect()
        };
        let (seeds, sectors) = (40, 30);
        let (mut joints, mut corners) = (0, 0);
        for seed in 0..seeds {
            let mut corridor = Corridor::new(seed);
            let walls: Vec<_> = (0..sectors)
                .map(|index| walls_of(&mut corridor, index))
                .collect();
            for pair in walls.windows(2) {
                let near = wall_bands(&pair[0], &pair[1]);
                let far = wall_bands(&pair[1], &pair[0]);
                // On each side of a barrier, a wall of one sector ends where a wall of the
                // other one starts.
                let pairs = near.iter().flat_map(|a| far.iter().map(move |b| (a, b)));
                for (from, to) in pairs.flat_map(|(a, b)| [(a, b), (b, a)]) {
                    if from.ends.1 != to.ends.0 {
                        continue;
                    }
                    joints += 1;
                    let square = (from.ends.1 - from.ends.0).perp().normalize();
                    corners += u32::from(from.reach.1.distance(square) > 1e-5);
                    assert!(!from.stops.1 && !to.stops.0);
                    assert!(from.reach.1.distance(to.reach.0) < 1e-5);
                }
            }
        }
        // Every barrier has two ends, and some sectors end on a corner.
        assert_eq!(joints, seeds as u32 * (sectors - 1) * 2);
        assert!(corners > 0, "no sector ends on a corner");
    }

    /// A world that dresses sectors and obstacles as the game does.
    fn dressing_world() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .add_systems(PreStartup, setup)
            .add_systems(Update, (dress_sectors, dress_obstacles));
        app
    }

    /// Loads a sector with its blocks, as the server does.
    fn load(app: &mut App, corridor: &mut Corridor, index: u32) -> Entity {
        let layout = corridor.layout(index);
        for corner in &layout.blocks {
            let center = corner.as_vec2() + Vec2::splat(BLOCK_SIZE / 2.0);
            let block = (Block { sector: index }, Position(center));
            app.world_mut().spawn(block);
        }
        app.world_mut().spawn(Sector { layout }).id()
    }

    fn corners_of(mesh: &Mesh) -> Vec<[f32; 3]> {
        match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(VertexAttributeValues::Float32x3(corners)) => corners.clone(),
            _ => panic!("a mesh without positions"),
        }
    }

    /// Checks what is drawn: each sector shows its walls as they fit its loaded neighbors, and
    /// every block shows a block.
    fn check_the_drawing(app: &mut App, corridor: &mut Corridor, loaded: &[(u32, Entity)]) {
        let planned = |corridor: &mut Corridor, index: u32| -> Vec<(Vec2, Vec2)> {
            let walls = corridor.plan(index).walls.iter();
            walls.map(|&(a, b)| (a.as_vec2(), b.as_vec2())).collect()
        };
        for &(index, entity) in loaded {
            let around: Vec<(Vec2, Vec2)> = loaded
                .iter()
                .filter(|(other, _)| other.abs_diff(index) == 1)
                .flat_map(|&(other, _)| planned(corridor, other))
                .collect();
            let wanted = wall_drawing(&planned(corridor, index), &around).build();
            let world = app.world();
            let children = world
                .entity(entity)
                .get::<Children>()
                .expect("a dressed sector");
            let walls: Vec<&Mesh3d> = children
                .iter()
                .filter(|&child| {
                    world
                        .get::<Ground>(child)
                        .is_some_and(|ground| !ground.floor)
                })
                .filter_map(|child| world.get::<Mesh3d>(child))
                .collect();
            assert_eq!(
                walls.len(),
                1,
                "sector {index} has one drawing of its walls"
            );
            let drawn = world.resource::<Assets<Mesh>>().get(&walls[0].0).unwrap();
            assert!(
                corners_of(drawn) == corners_of(&wanted),
                "the walls of sector {index} don't fit its neighbors"
            );
        }
        let block = app.world().resource::<Shapes>().block.clone();
        let world = app.world_mut();
        let mut blocks = world.query_filtered::<&Mesh3d, With<Block>>();
        assert!(blocks.iter(world).count() > 0);
        assert!(
            blocks.iter(world).all(|mesh| mesh.0 == block),
            "a block shows something else than a block"
        );
    }

    #[test]
    fn walls_are_drawn_again_on_the_walls_and_nowhere_else() {
        // The first sectors of this seed hold blocks.
        let mut corridor = Corridor::new(seed_value("3ZXHW3M8"));
        let mut app = dressing_world();
        // A block may be dressed before any wall is: the order must not matter.
        let lone = (Block { sector: 1 }, Position(Vec2::new(0.0, 30.0)));
        app.world_mut().spawn(lone);
        app.update();

        // A run starts in the bay, with the first sector behind its barrier.
        let mut loaded: Vec<(u32, Entity)> = (0..2)
            .map(|index| (index, load(&mut app, &mut corridor, index)))
            .collect();
        app.update();
        check_the_drawing(&mut app, &mut corridor, &loaded);

        // The player enters the first sector, and the second one is loaded.
        loaded.push((2, load(&mut app, &mut corridor, 2)));
        app.update();
        check_the_drawing(&mut app, &mut corridor, &loaded);

        // The player enters the second sector: the third one comes and the bay goes.
        loaded.push((3, load(&mut app, &mut corridor, 3)));
        let (_, bay) = loaded.remove(0);
        app.world_mut().entity_mut(bay).despawn();
        app.update();
        check_the_drawing(&mut app, &mut corridor, &loaded);
    }

    #[test]
    fn obstacles_stay_on_their_squares() {
        for (draft, size, height) in [
            (block_drawing(), BLOCK_SIZE, BLOCK_HEIGHT),
            (crate_drawing(), CRATE_SIZE, CRATE_HEIGHT),
        ] {
            let (lowest, highest) = draft.bounds();
            // The line around the foot is the only thing outside the footprint.
            let reach = size / 2.0 + draft::LINE;
            assert!(close(lowest.x, -reach) && close(highest.x, reach));
            assert!(close(lowest.z, -reach) && close(highest.z, reach));
            assert!(reach <= BLOCK_SIZE / 2.0 + draft::LINE);
            assert!(close(lowest.y, 0.0) && close(highest.y, height + LIFT));
        }
        // Eight strips cross the top of a block.
        assert!(block_drawing().area(theme::HATCH) > 8.0 * HATCH_WIDTH);
    }

    #[test]
    fn the_chain_of_a_barrier_spans_it() {
        for length in [8.0, 14.0, 17.3] {
            let (lowest, highest) = barrier_drawing(length).bounds();
            assert!(close(lowest.x, -length / 2.0) && close(highest.x, length / 2.0));
            assert!(close(highest.y, BARRIER_HEIGHT));
        }
    }

    #[test]
    fn a_sector_is_dormant_until_the_players_reach_it() {
        let run = |sector| Run {
            seed_text: "TEST".into(),
            seed: 1,
            players: 1,
            solo: true,
            sector,
            cleared: 0,
            checkpoint: 0,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase: tanku_core::sim::RunPhase::Playing,
            lost_to: None,
        };
        assert!(is_dormant(5, Some(&run(4))));
        assert!(!is_dormant(5, Some(&run(5))));
        // The sectors behind the players stay lit until they are unloaded.
        assert!(!is_dormant(4, Some(&run(5))));
        // Without a run, nothing waits for anybody.
        assert!(!is_dormant(5, None));
    }

    #[test]
    fn a_trail_grows_as_the_bullet_flies_and_starts_over_at_a_bounce() {
        let mut trail = Trail::default();
        let east = Vec2::X * 9.0;
        assert!(close(trail_after(&mut trail, east, 0.1), 0.9));
        assert!(close(trail_after(&mut trail, east, 0.05), 1.35));
        // It stops growing at its full length.
        assert!(close(trail_after(&mut trail, east, 1.0), TRAIL_LENGTH));
        // A bounce turns the bullet: the line behind it is on the other side of the wall.
        let north_east = Vec2::new(9.0, 9.0).normalize() * 9.0;
        assert!(close(trail_after(&mut trail, north_east, 0.1), 0.9));
    }

    /// The triangles of a bullet, and whether each is part of a stripe.
    fn bullet_triangles(pointed: bool, stripes: u8) -> Vec<([Vec3; 3], bool)> {
        let mesh = bullet_mesh(pointed, stripes);
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("a bullet has vertex colors");
        };
        let corners = positions.as_float3().unwrap().chunks(3);
        corners
            .zip(colors.chunks(3))
            .map(|(corners, colors)| {
                let corners = [corners[0], corners[1], corners[2]].map(Vec3::from);
                (corners, colors[0] == STRIPE_COLOR)
            })
            .collect()
    }

    #[test]
    fn bullets_show_one_stripe_for_each_bounce_left() {
        for (pointed, stripes) in [false, true]
            .into_iter()
            .flat_map(|pointed| (0..=3u8).map(move |stripes| (pointed, stripes)))
        {
            // The dark stretches along the axis, joined where they touch.
            let mut dark: Vec<(f32, f32)> = bullet_triangles(pointed, stripes)
                .into_iter()
                .filter(|(_, striped)| *striped)
                .map(|(corners, _)| {
                    let along = corners.map(|corner| corner.x);
                    (
                        along.into_iter().fold(f32::MAX, f32::min),
                        along.into_iter().fold(f32::MIN, f32::max),
                    )
                })
                .collect();
            dark.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut bands: Vec<(f32, f32)> = Vec::new();
            for (from, to) in dark {
                match bands.last_mut() {
                    Some(band) if from <= band.1 + 1e-4 => band.1 = band.1.max(to),
                    _ => bands.push((from, to)),
                }
            }
            assert_eq!(bands.len(), stripes as usize);

            // Stripes have the same width, are evenly spaced and centered on the bullet.
            let centers: Vec<f32> = bands.iter().map(|band| (band.0 + band.1) / 2.0).collect();
            assert!(centers.iter().sum::<f32>().abs() < 1e-4);
            for band in &bands {
                assert!((band.1 - band.0 - STRIPE_WIDTH).abs() < 1e-4);
            }
            for pair in centers.windows(2) {
                assert!((pair[1] - pair[0] - STRIPE_WIDTH - STRIPE_GAP).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn bullets_show_their_outside() {
        // A face is drawn from the side on which its corners turn counterclockwise.
        for pointed in [false, true] {
            for (corners, _) in bullet_triangles(pointed, 2) {
                let facing = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
                let center = (corners[0] + corners[1] + corners[2]) / 3.0;
                let away_from_axis = Vec3::new(0.0, center.y, center.z);
                assert!(facing.dot(away_from_axis) > 0.0, "{corners:?}");
            }
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
