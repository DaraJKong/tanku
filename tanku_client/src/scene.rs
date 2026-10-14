//! Draws what the simulation contains: the corridor, the obstacles, the mines and the bullets.
//!
//! Gameplay is 2D. The ground plane of the simulation, with `x` east and `y` north, becomes the
//! `x`-`z` plane of the 3D world, with north along `-z`.
//!
//! The corridor is drawn as a plan: a dark sheet with a grid, walls as bands between two lines,
//! hatched blocks and crossed crates. Nothing is lit. In a sector that no player has entered
//! yet, the blocks and the crates are plans, flat on the floor: they stand up when the first
//! player comes in, over the third of a second that [`wake`] describes.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use tanku_core::{
    map::SectorKind,
    rules::{BLAST_RADIUS, BLOCK_SIZE, BULLET_RADIUS, MINE_RADIUS, MINE_TRIGGER_RADIUS},
    sim::{
        Barrier, Block, Bullet, ExplosiveCrate, Kind, Mine, MineState, Planned, Position,
        PreviousPosition, Run, Sector,
    },
    stats::Stat,
};

use crate::{
    draft::{self, Draft, LIFT},
    theme,
    wake::{self, Hold},
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
/// How strong the hatching of a block that is planned is over the floor, and the cross of a
/// crate: a little fainter than on what will stand there.
const PLANNED_STRENGTH: f32 = 0.7;
/// How flat a block is when it starts to rise from its plan, as a share of its height: its
/// lines still lie over its faces.
const FLAT: f32 = 0.1;
/// Seconds for which the place of a plan of the room that ended waits for what stands there:
/// a guest may learn of the block a moment after it learned that the plan was over.
const PLAN_KEPT: f32 = 0.5;
/// How much of a pale sheet lies over the top of a block whose ink is wet, and of a red one
/// over a crate.
const SHEEN: (f32, f32) = (0.35, 0.5);
/// The marks at the corners of the foot of a block that has landed: how far from the foot
/// they start and end, and how long each of their two arms is.
const MARK_REACH: (f32, f32) = (0.08, 0.34);
const MARK_ARM: f32 = 0.28;
const MINE_HEIGHT: f32 = 0.12;
const BARRIER_HEIGHT: f32 = 1.5;
/// The chain line at the foot of a barrier: a long dash, a short one, and the gap after each.
const CHAIN: [(f32, f32); 2] = [(0.9, 0.2), (0.15, 0.2)];
const CHAIN_WIDTH: f32 = 0.12;
/// Height at which bullets fly: the height of a barrel.
pub const BULLET_HEIGHT: f32 = 0.8;
/// Half the length of the straight part of a bullet. Its rounded ends come on top.
const BULLET_BODY: f32 = BULLET_RADIUS;
/// Length of the pointed tip of a piercing bullet.
const DART_TIP: f32 = 0.4;
/// A dark stripe looks thinner than it is on a bright bullet, so the gaps are a little narrower.
const STRIPE_WIDTH: f32 = 0.12;
const STRIPE_GAP: f32 = 0.09;
/// Stripes are dark on a bright bullet. A vertex color multiplies the color of the material.
const STRIPE_COLOR: [f32; 4] = [0.03, 0.03, 0.04, 1.0];
/// How far ahead of its two ends the point of a chevron is, on a bullet that seeks.
const CHEVRON_LEAN: f32 = 0.14;
/// Length and width of the seam along a bullet that splits.
const SEAM: Vec2 = Vec2::new(0.76, 0.06);
/// A bullet that explodes, along its line of flight from its middle: where its round part is
/// cut flat, where the red line across it starts and ends, and where its flat end starts. A
/// gap parts each from the next, through which the floor shows.
const GRENADE_CUT: f32 = 0.11;
const RED_LINE: (f32, f32) = (0.17, 0.29);
const FLAT_END: f32 = 0.35;
/// The red line is a little wider than the bullet that it cuts.
const RED_LINE_RADIUS: f32 = 0.3;
/// Length and width of the trail behind a bullet that explodes, and how much of it shows.
const TRAIL: Vec2 = Vec2::new(1.8, 0.08);
const TRAIL_STRENGTH: f32 = 0.45;
/// The patch of light under a bullet, which shows where it is above the ground.
const POOL_SIZE: f32 = 0.9;
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
    block: Handle<Mesh>,
    explosive: Handle<Mesh>,
    /// The plans of a block and of a crate that are on their way, flat on the floor.
    planned: [Handle<Mesh>; 2],
    /// A square lying flat, 1 on each side.
    plate: Handle<Mesh>,
    /// The marks at the corners of the foot of a block that has landed, and of a crate.
    marks: [Handle<Mesh>; 2],
    mine: Handle<Mesh>,
    /// The circles around a mine, at their real sizes.
    trigger: Handle<Mesh>,
    blast: Handle<Mesh>,
    /// Bullets at their real size, flying along `+x`. The index is the number of bounces left,
    /// which is the number of stripes.
    bullets: Vec<Handle<Mesh>>,
    /// The same for bullets that still pierce: they have a pointed tip.
    darts: Vec<Handle<Mesh>>,
    /// The same for bullets that seek at their bounces: a chevron stands for each stripe.
    seekers: Vec<Handle<Mesh>>,
    /// A bullet that splits and one that explodes. Neither bounces.
    splitter: Handle<Mesh>,
    grenade: Handle<Mesh>,
    /// The trail of a bullet that explodes, 1 long: it runs back from where it is put.
    trail: Handle<Mesh>,
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
    /// The same two while the light of a sector comes up, as a playtest can have it.
    ink_waking: Handle<StandardMaterial>,
    floor_waking: Handle<StandardMaterial>,
    /// The sheet that lies over the top of a block whose ink is wet, and of a crate, and the
    /// marks at the corners of their feet. They fade.
    sheens: [Handle<StandardMaterial>; 2],
    marks: [Handle<StandardMaterial>; 2],
    curtain_locked: Handle<StandardMaterial>,
    curtain_unlocked: Handle<StandardMaterial>,
    barrier_locked: Handle<StandardMaterial>,
    barrier_unlocked: Handle<StandardMaterial>,
    bullet: Handle<StandardMaterial>,
    /// The patch of light under a bullet.
    pool: Handle<StandardMaterial>,
    /// The trail of a bullet that explodes.
    trail: Handle<StandardMaterial>,
    mine_light: Handle<StandardMaterial>,
    trigger: Handle<StandardMaterial>,
    blast: Handle<StandardMaterial>,
    blast_fill: Handle<StandardMaterial>,
}

/// What a part of the corridor is drawn with, which changes when a player enters its sector:
/// its floor, its walls and its obstacles.
#[derive(Component, Clone, Copy)]
struct Ground {
    sector: u32,
    floor: bool,
}

/// The two drawings of a block or of a crate: as it stands, and as the plan of it that lies
/// flat on the floor until a player enters its sector.
#[derive(Component, Clone, PartialEq)]
struct Stands {
    built: Handle<Mesh>,
    plan: Handle<Mesh>,
}

/// The drawing of the walls of a sector, which is done again when its neighbors change.
#[derive(Component)]
struct Walls;

/// A part of the ground of a sector that wakes: for how long it has, and what a block or a
/// crate has around it meanwhile.
#[derive(Component, Default)]
struct Waking {
    age: f32,
    extras: Option<Extras>,
}

/// What a block or a crate has around it while it rises: the sheet of wet ink over its top,
/// and the marks at the corners of its foot.
#[derive(Clone, Copy)]
struct Extras {
    sheen: Entity,
    marks: Entity,
}

/// The marks at the corners of the foot of a block or of a crate.
#[derive(Component)]
struct Marks;

/// The plans of the blocks and the crates of the room of arena mode: those that are drawn,
/// and the places of those that just ended. What stands on such a place rises off its plan, as
/// what stands in a sector that wakes.
#[derive(Resource, Default)]
struct Plans {
    drawn: Vec<(Entity, Vec2)>,
    /// Each with the seconds since its plan ended.
    ended: Vec<(Vec2, f32)>,
}

impl Plans {
    /// Whether what stands at this place stands where a plan is or just was. The place of a
    /// plan that ended serves once.
    fn take(&mut self, at: Vec2) -> bool {
        let here = |place: &Vec2| place.distance_squared(at) < 0.01;
        if let Some(index) = self.ended.iter().position(|(place, _)| here(place)) {
            self.ended.swap_remove(index);
            return true;
        }
        self.drawn.iter().any(|(_, place)| here(place))
    }
}

/// The two parts of a barrier, which change color when it unlocks.
#[derive(Component, Clone, Copy)]
enum BarrierPart {
    Curtain,
    Lines,
}

/// The shell of a bullet: the part that says what the bullet does. It turns around the line
/// of flight to face the camera.
#[derive(Component)]
struct Shell;

/// The trail of a bullet that explodes. It grows from where the bullet was first seen.
#[derive(Component)]
struct Trail {
    from: Vec2,
}

/// What a bullet is drawn as, which says what it does besides flying straight and bouncing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    /// Round at both ends.
    Round,
    /// With a pointed tip: it pierces.
    Dart,
    /// With chevrons in the place of the stripes: it seeks at its bounces.
    Seeker,
    /// With a seam along it and no stripe: it splits.
    Splitter,
    /// With a flat end, and a red line that cuts it near that end: it explodes.
    Grenade,
}

/// The parts of a mine that come and go with its state.
#[derive(Component, Clone, Copy, PartialEq)]
enum MinePart {
    /// The light in its middle, which blinks.
    Light,
    /// The circle inside which another tank than the one that laid it sets it off.
    Trigger,
    /// The circle that its blast reaches.
    Blast,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Plans>();
    app.add_systems(PreStartup, setup).add_systems(
        Update,
        (
            (
                dress_sectors,
                dress_barriers,
                dress_obstacles,
                dress_planned,
                dress_mines,
                dress_bullets,
            ),
            (wake_sectors, paint_barriers, blink_mines, stripe_bullets),
            rise,
            place,
            (roll_bullets, stretch_trails),
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

/// A flat color that is seen through, and that fades as what it is on settles.
fn fading(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        alpha_mode: AlphaMode::Blend,
        ..ink()
    }
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
    let (trigger, blast) = (
        circle(MINE_TRIGGER_RADIUS, draft::LINE, (28, 0.5)),
        circle(BLAST_RADIUS, 0.06, (1, 1.0)),
    );
    let mut disc = Draft::default();
    disc.disc(Vec3::ZERO, 0.5, Color::WHITE);
    let mut trail = Draft::default();
    trail.line(Vec3::ZERO, Vec3::NEG_X, TRAIL.y, Color::WHITE);
    let mut plate = Draft::default();
    plate.plate(Vec3::ZERO, Vec2::ONE, Color::WHITE);
    let bounces = 0..=Stat::Ricochet.max_level();
    let mut bullets = |look: Look| -> Vec<Handle<Mesh>> {
        let bounces = bounces.clone();
        bounces
            .map(|stripes| meshes.add(bullet_mesh(look, stripes)))
            .collect()
    };
    let (rounds, darts, seekers) = (
        bullets(Look::Round),
        bullets(Look::Dart),
        bullets(Look::Seeker),
    );
    commands.insert_resource(Shapes {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        sphere: meshes.add(Sphere::new(0.5)),
        hairline: meshes.add(Torus::new(0.487, 0.5)),
        disc: meshes.add(disc.build()),
        block: meshes.add(block_drawing().build()),
        explosive: meshes.add(crate_drawing().build()),
        planned: [false, true].map(|explosive| meshes.add(planned_drawing(explosive).build())),
        plate: meshes.add(plate.build()),
        marks: [false, true].map(|explosive| meshes.add(mark_drawing(explosive).build())),
        mine: meshes.add(mine_drawing().build()),
        trigger,
        blast,
        bullets: rounds,
        darts,
        seekers,
        splitter: meshes.add(bullet_mesh(Look::Splitter, 0)),
        grenade: meshes.add(bullet_mesh(Look::Grenade, 0)),
        trail: meshes.add(trail.build()),
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
        ink_waking: materials.add(StandardMaterial {
            base_color: theme::dormant(),
            ..ink()
        }),
        floor_waking: materials.add(sheet(theme::dormant())),
        sheens: [theme::LINE, theme::DANGER].map(|color| materials.add(fading(color))),
        marks: [theme::BLOCK_LINE, theme::DANGER].map(|color| materials.add(fading(color))),
        curtain_locked: paint(materials, theme::DANGER.with_alpha(0.2)),
        curtain_unlocked: paint(materials, theme::GOOD.with_alpha(0.12)),
        barrier_locked: paint(materials, theme::DANGER),
        barrier_unlocked: paint(materials, theme::GOOD),
        bullet: paint(materials, theme::BULLET),
        pool: paint(materials, theme::BULLET.with_alpha(0.13)),
        trail: paint(materials, theme::BULLET.with_alpha(TRAIL_STRENGTH)),
        mine_light: paint(materials, theme::DANGER),
        trigger: paint(materials, theme::DANGER.with_alpha(0.6)),
        blast: paint(materials, theme::DANGER),
        blast_fill: paint(materials, theme::DANGER.with_alpha(0.1)),
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

/// How high a block stands, or a crate.
fn obstacle_height(explosive: bool) -> f32 {
    if explosive {
        CRATE_HEIGHT
    } else {
        BLOCK_HEIGHT
    }
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
    hatch(&mut draft, BLOCK_HEIGHT + LIFT / 2.0, theme::HATCH);
    draft
}

/// The hatching of a block at a height: strips at 45 degrees, half a meter apart along the
/// edges, each cut where it meets the edges of the square. Under the outline, which hides
/// their ends.
fn hatch(draft: &mut Draft, top: f32, color: Color) {
    let half = BLOCK_SIZE / 2.0;
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
        draft.face(&corners, color);
    }
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
    cross(&mut draft, CRATE_HEIGHT + LIFT / 2.0, theme::DANGER);
    draft
}

/// The cross of a crate at a height, from corner to corner.
fn cross(draft: &mut Draft, top: f32, color: Color) {
    let reach = CRATE_SIZE / 2.0 - draft::LINE;
    for turn in [-1.0, 1.0] {
        draft.line(
            Vec3::new(-reach, top, -reach * turn),
            Vec3::new(reach, top, reach * turn),
            CROSS_WIDTH,
            color,
        );
    }
}

/// The plan of a block or of a crate that is on its way, flat on the floor: its outline, with
/// the hatching of the block or the cross of the crate, a little fainter than on what will
/// stand there.
fn planned_drawing(explosive: bool) -> Draft {
    let mut draft = Draft::default();
    let (side, line, mark) = if explosive {
        (CRATE_SIZE, theme::DANGER, theme::DANGER)
    } else {
        (BLOCK_SIZE, theme::BLOCK_LINE, theme::HATCH)
    };
    let (at, plan) = (Vec3::Y * 2.0 * LIFT, Vec2::splat(side));
    draft.frame(at, plan, draft::LINE, line);
    let faint = theme::over(theme::FLOOR, mark, PLANNED_STRENGTH);
    if explosive {
        cross(&mut draft, 1.5 * LIFT, faint);
    } else {
        hatch(&mut draft, 1.5 * LIFT, faint);
    }
    draft
}

/// The side of the square that a block stands on, or a crate.
fn footprint(explosive: bool) -> f32 {
    if explosive { CRATE_SIZE } else { BLOCK_SIZE }
}

/// The marks at the corners of the foot of a block that has landed, or of a crate, as they
/// start: at each corner, two short lines along its sides, a little away from it. In white,
/// to take the color of what they are drawn with.
fn mark_drawing(explosive: bool) -> Draft {
    let mut draft = Draft::default();
    let reach = footprint(explosive) / 2.0 + MARK_REACH.0;
    for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let corner = Vec3::new(x * reach, 2.0 * LIFT, z * reach);
        // Each arm starts half a line past the corner, so that the two meet square.
        for (along, side) in [(Vec3::X, x), (Vec3::Z, z)] {
            let from = corner + along * side * draft::LINE / 2.0;
            let to = corner - along * side * MARK_ARM;
            draft.line(from, to, draft::LINE, Color::WHITE);
        }
    }
    draft
}

/// How much larger the marks of a foot are when they have flicked out this far, from 0 to 1.
fn mark_growth(explosive: bool, flicked: f32) -> f32 {
    let foot = footprint(explosive) / 2.0;
    let reach = MARK_REACH.0 + (MARK_REACH.1 - MARK_REACH.0) * wake::ease_out(flicked);
    (foot + reach) / (foot + MARK_REACH.0)
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

/// A solid turned around the line of flight, which is `+x`: the surface between rings around
/// that line, as triangles that carry their colors.
#[derive(Default)]
struct Turned {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    colors: Vec<[f32; 4]>,
}

impl Turned {
    /// Adds the surface between this many rings, from the tail to the tip. `ring` tells how
    /// far along the line of flight a ring is at an angle around it, and how far from the
    /// line. The angles are those of `around`, from the top, and the surface is flat between
    /// two of them. `color` is that of the band after a ring, between an angle and the next.
    fn bands(
        &mut self,
        around: &[f32],
        rings: usize,
        ring: impl Fn(usize, f32) -> (f32, f32),
        color: impl Fn(usize, usize) -> [f32; 4],
    ) {
        let outward = |angle: f32| Vec3::new(0.0, angle.cos(), angle.sin());
        let corner = |index: usize, angle: f32| {
            let (along, radius) = ring(index, angle);
            (Vec3::X * along + outward(angle) * radius, outward(angle))
        };
        for band in 0..rings.saturating_sub(1) {
            for (facet, &here) in around.iter().enumerate() {
                let next = around[(facet + 1) % around.len()];
                let (from, to) = ((band, here), (band, next));
                let (far, near) = ((band + 1, next), (band + 1, here));
                // Two triangles, counterclockwise seen from outside. One of them is flat
                // where a ring is a single point, as at the tail and at the tip.
                for triangle in [[from, to, near], [to, far, near]] {
                    let corners = triangle.map(|(index, angle)| corner(index, angle));
                    let [a, b, c] = corners.map(|(place, _)| place);
                    if (b - a).cross(c - a).length_squared() < 1e-12 {
                        continue;
                    }
                    for (place, normal) in corners {
                        self.positions.push(place);
                        self.normals.push(normal);
                        self.colors.push(color(band, facet));
                    }
                }
            }
        }
    }

    fn build(self) -> Mesh {
        let count = self.positions.len();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![Vec2::ZERO; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
    }
}

/// One of the two round ends of a bullet, from the line of flight outward: the tail, or the
/// tip, as distances along the line of flight and from it.
fn round_end(forward: f32) -> impl Iterator<Item = (f32, f32)> {
    const STEPS: usize = 4;
    (0..=STEPS).map(move |step| {
        let angle = step as f32 / STEPS as f32 * std::f32::consts::FRAC_PI_2;
        let (along, radius) = (angle.cos(), angle.sin());
        (
            forward * (BULLET_BODY + BULLET_RADIUS * along),
            BULLET_RADIUS * radius,
        )
    })
}

/// The outline of a bullet from its tail to its tip, which is round or pointed.
fn bullet_outline(pointed: bool) -> Vec<(f32, f32)> {
    let mut outline: Vec<(f32, f32)> = round_end(-1.0).collect();
    if pointed {
        outline.push((BULLET_BODY, BULLET_RADIUS));
        outline.push((BULLET_BODY + DART_TIP, 0.0));
    } else {
        outline.extend(round_end(1.0).collect::<Vec<_>>().into_iter().rev());
    }
    outline
}

/// How far from its line of flight the skin of a round bullet is, this far along the line
/// from the middle of the bullet.
fn girth(along: f32) -> f32 {
    let past = (along.abs() - BULLET_BODY).max(0.0);
    (BULLET_RADIUS * BULLET_RADIUS - past * past)
        .max(0.0)
        .sqrt()
}

/// Where the stripes of a bullet start and end along it, from its middle. They are evenly
/// spaced and centered on the middle of the bullet.
fn stripe_edges(stripes: u8) -> Vec<f32> {
    (0..stripes)
        .flat_map(|stripe| {
            let spacing = STRIPE_WIDTH + STRIPE_GAP;
            let center = (stripe as f32 - (stripes as f32 - 1.0) / 2.0) * spacing;
            [center - STRIPE_WIDTH / 2.0, center + STRIPE_WIDTH / 2.0]
        })
        .collect()
}

/// Whether a place along a bullet is inside one of its stripes.
fn striped(edges: &[f32], along: f32) -> bool {
    edges
        .chunks(2)
        .any(|stripe| along > stripe[0] && along < stripe[1])
}

/// An outline with a ring more wherever it crosses one of these places along the line of
/// flight, so that every band of it lies on one side of each place.
fn cut(outline: &[(f32, f32)], places: &[f32]) -> Vec<(f32, f32)> {
    let mut rings: Vec<(f32, f32)> = Vec::new();
    for ends in outline.windows(2) {
        let (from, to) = (ends[0], ends[1]);
        rings.push(from);
        for &place in places {
            if place > from.0 && place < to.0 {
                let share = (place - from.0) / (to.0 - from.0);
                rings.push((place, from.1 + (to.1 - from.1) * share));
            }
        }
    }
    rings.push(outline[outline.len() - 1]);
    rings
}

/// The mesh of a bullet: a shape turned around its line of flight, with what says what the
/// bullet does. A round bullet and a pointed one have a stripe for each bounce they have
/// left, and a bullet that seeks has a chevron in the place of each stripe.
fn bullet_mesh(look: Look, stripes: u8) -> Mesh {
    const AROUND: usize = 12;
    const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    let even: Vec<f32> = (0..AROUND)
        .map(|side| side as f32 / AROUND as f32 * std::f32::consts::TAU)
        .collect();
    let edges = stripe_edges(stripes);
    let paint = |dark: bool| if dark { STRIPE_COLOR } else { WHITE };
    let mut turned = Turned::default();
    match look {
        // Its stripes lean: each is ahead on top of the bullet and behind on its sides, which
        // makes a chevron seen from above. Every ring leans as they do, on the skin of a
        // round bullet: wholly where a stripe can be, and less and less toward the two ends,
        // where a ring is a point.
        Look::Seeker if stripes > 0 => {
            const RINGS: usize = 30;
            let reach = BULLET_BODY + BULLET_RADIUS;
            let most = stripe_edges(Stat::Ricochet.max_level());
            let whole = most.iter().copied().fold(0.0, f32::max);
            let lean = |angle: f32, place: f32| {
                let fade = ((reach - place.abs()) / (reach - whole)).clamp(0.0, 1.0);
                CHEVRON_LEAN * (0.5 - angle.sin().abs()) * fade
            };
            let spread = (0..=RINGS).map(|ring| reach * (2.0 * ring as f32 / RINGS as f32 - 1.0));
            let mut places: Vec<f32> = spread.chain(edges.iter().copied()).collect();
            places.sort_by(f32::total_cmp);
            places.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
            let middle = |band: usize| (places[band] + places[band + 1]) / 2.0;
            turned.bands(
                &even,
                places.len(),
                |ring, angle| {
                    let along = places[ring] + lean(angle, places[ring]);
                    (along, girth(along))
                },
                |band, _| paint(striped(&edges, middle(band))),
            );
        }
        // A bullet that seeks and has no bounce left is a round bullet.
        Look::Round | Look::Dart | Look::Seeker => {
            let rings = cut(&bullet_outline(look == Look::Dart), &edges);
            let middle = |band: usize| (rings[band].0 + rings[band + 1].0) / 2.0;
            turned.bands(
                &even,
                rings.len(),
                |ring, _| rings[ring],
                |band, _| paint(striped(&edges, middle(band))),
            );
        }
        // A seam along the top of the bullet: a narrow strip there, dark for the length of
        // the seam.
        Look::Splitter => {
            let half = (SEAM.y / 2.0 / BULLET_RADIUS).asin();
            let mut around = even.clone();
            around[0] = half;
            around.push(std::f32::consts::TAU - half);
            let seam = around.len() - 1;
            let ends = [-SEAM.x / 2.0, SEAM.x / 2.0];
            let rings = cut(&bullet_outline(false), &ends);
            let middle = |band: usize| (rings[band].0 + rings[band + 1].0) / 2.0;
            turned.bands(
                &around,
                rings.len(),
                |ring, _| rings[ring],
                |band, facet| paint(facet == seam && middle(band).abs() < SEAM.x / 2.0),
            );
        }
        // Three pieces with flat ends: the round tail, the red line and the flat end.
        Look::Grenade => {
            let red = theme::DANGER.to_linear().to_f32_array();
            let slab = |from: f32, to: f32, radius: f32| {
                vec![(from, 0.0), (from, radius), (to, radius), (to, 0.0)]
            };
            let mut tail: Vec<(f32, f32)> = round_end(-1.0).collect();
            tail.extend([(GRENADE_CUT, BULLET_RADIUS), (GRENADE_CUT, 0.0)]);
            let pieces = [
                (tail, WHITE),
                (slab(RED_LINE.0, RED_LINE.1, RED_LINE_RADIUS), red),
                (
                    slab(FLAT_END, BULLET_BODY + BULLET_RADIUS, BULLET_RADIUS),
                    WHITE,
                ),
            ];
            for (rings, color) in pieces {
                turned.bands(&even, rings.len(), |ring, _| rings[ring], |_, _| color);
            }
        }
    }
    turned.build()
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

/// The radius of the outer circle on the floor of an arena whose walls fit in a box, and of
/// the inner one: they stay clear of the walls of the smallest arena and don't grow past a
/// size that reads at a glance.
fn arena_rings(min: Vec2, max: Vec2) -> (f32, f32) {
    let outer = ((max - min).min_element() / 2.0 - 3.0).clamp(4.0, 11.0);
    (outer, outer * 0.55)
}

/// What is drawn on the floor of an arena: two circles in dashes around its middle, in the
/// red of what is dangerous.
fn arena_drawing(min: Vec2, max: Vec2) -> Draft {
    let mut draft = Draft::default();
    let middle = to_world((min + max) / 2.0, 2.0 * LIFT);
    let (outer, inner) = arena_rings(min, max);
    let ink = theme::DANGER.mix(&theme::FLOOR, 0.45);
    for (radius, dashes) in [(outer, 48), (inner, 28)] {
        draft.ring(middle, (radius - 0.12, radius), (dashes, 0.5), ink);
    }
    draft
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
        // An arena is empty until its enemies appear: circles on its floor tell it from a
        // sector that was cleared.
        let rings = (plan.kind == SectorKind::Arena)
            .then(|| meshes.add(arena_drawing(plan.min.as_vec2(), plan.max.as_vec2()).build()));
        commands
            .entity(entity)
            .insert((Transform::default(), Visibility::default()))
            .with_children(|parent| {
                if let Some(rings) = rings {
                    parent.spawn((
                        Ground {
                            sector: index,
                            floor: false,
                        },
                        Mesh3d(rings),
                        MeshMaterial3d(paints.ink_dormant.clone()),
                    ));
                }
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
    time: Res<Time>,
    mut plans: ResMut<Plans>,
    mut over: RemovedComponents<Planned>,
    blocks: Query<(Entity, &Block, &Position), Added<Block>>,
    crates: Query<(Entity, &ExplosiveCrate, &Position), Added<ExplosiveCrate>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
) {
    // The place of a plan that ended waits a moment for what stands there.
    let seconds = time.delta_secs();
    plans.ended.retain_mut(|(_, since)| {
        *since += seconds;
        *since < PLAN_KEPT
    });
    for entity in over.read() {
        if let Some(index) = plans.drawn.iter().position(|(drawn, _)| *drawn == entity) {
            let (_, at) = plans.drawn.swap_remove(index);
            plans.ended.push((at, 0.0));
        }
    }
    let stands = |built: &Handle<Mesh>, explosive: bool| Stands {
        built: built.clone(),
        plan: shapes.planned[explosive as usize].clone(),
    };
    let blocks = blocks.iter().map(|(entity, block, position)| {
        (entity, block.sector, position, stands(&shapes.block, false))
    });
    let crates = crates.iter().map(|(entity, explosive, position)| {
        let stands = stands(&shapes.explosive, true);
        (entity, explosive.sector, position, stands)
    });
    // Which of its two drawings each one shows is for `wake_sectors` to say, but for what
    // stands where a plan of the room was: it rises off that plan from the first frame.
    for (entity, sector, position, stands) in blocks.chain(crates) {
        let mut placed = Transform::from_translation(to_world(position.0, 0.0));
        let shown = if plans.take(position.0) {
            placed.scale.y = FLAT;
            commands.entity(entity).insert(Waking::default());
            stands.built.clone()
        } else {
            stands.plan.clone()
        };
        commands.entity(entity).insert((
            Ground {
                sector,
                floor: false,
            },
            Mesh3d(shown),
            stands,
            MeshMaterial3d(paints.ink.clone()),
            placed,
        ));
    }
}

/// Draws the plan of a block or of a crate that is on its way.
fn dress_planned(
    mut commands: Commands,
    mut plans: ResMut<Plans>,
    planned: Query<(Entity, &Planned, &Position), Added<Planned>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
) {
    for (entity, plan, position) in &planned {
        plans.drawn.push((entity, position.0));
        let shape = &shapes.planned[plan.explosive as usize];
        commands.entity(entity).insert((
            Mesh3d(shape.clone()),
            MeshMaterial3d(paints.ink.clone()),
            Transform::from_translation(to_world(position.0, 0.0)),
        ));
    }
}

/// Whether a sector is still waiting for its first player: the players are in an earlier one.
pub fn is_dormant(sector: u32, run: Option<&Run>) -> bool {
    run.is_some_and(|run| sector > run.sector)
}

/// A sector is drawn dull until a player enters it, and its blocks and its crates are plans,
/// flat on its floor and in full ink. Then the drawing comes to life, and they stand up. A
/// sector that a single player comes back in front of is as it was before they entered it.
///
/// A sector that wakes takes its time: [`rise`] draws it until it is at rest. What is loaded
/// awake is drawn awake at once, and so is a sector that sleeps again.
fn wake_sectors(
    mut commands: Commands,
    run: Option<Res<Run>>,
    paints: Res<Paints>,
    mut grounds: Query<(
        Entity,
        Ref<Ground>,
        &mut MeshMaterial3d<StandardMaterial>,
        Option<(&Stands, &mut Mesh3d)>,
        Option<&Waking>,
        &mut Transform,
    )>,
) {
    for (entity, ground, mut material, mut stands, waking, mut transform) in &mut grounds {
        let dormant = is_dormant(ground.sector, run.as_deref());
        if let Some(waking) = waking {
            if !dormant {
                continue;
            }
            // It sleeps again before it was up.
            if let Some(extras) = waking.extras {
                commands.entity(extras.sheen).despawn();
                commands.entity(extras.marks).despawn();
            }
            commands.entity(entity).remove::<Waking>();
            transform.scale = Vec3::ONE;
        }
        let planned = |stands: &Option<(&Stands, Mut<Mesh3d>)>| {
            let shown = stands.as_ref();
            shown.is_some_and(|(stands, mesh)| mesh.0 == stands.plan)
        };
        let asleep =
            [&paints.floor_dormant, &paints.ink_dormant].contains(&&material.0) || planned(&stands);
        if !dormant && asleep && !ground.is_added() {
            // It was drawn as asleep, and wakes: a block shows what stands, flat at first.
            if let Some((stands, mesh)) = &mut stands {
                mesh.0 = stands.built.clone();
                transform.scale.y = FLAT;
            }
            commands.entity(entity).insert(Waking::default());
            continue;
        }
        let wanted = match (ground.floor, dormant) {
            (true, false) => &paints.floor,
            (true, true) => &paints.floor_dormant,
            // A plan is drawn as sharply as what stands.
            (false, true) if stands.is_none() => &paints.ink_dormant,
            (false, _) => &paints.ink,
        };
        if material.0 != *wanted {
            material.0 = wanted.clone();
        }
        if let Some((stands, mut mesh)) = stands {
            let wanted = if dormant { &stands.plan } else { &stands.built };
            if mesh.0 != *wanted {
                mesh.0 = wanted.clone();
            }
        }
    }
}

/// Draws what takes its time to wake, moment by moment: the light of the floor and of the
/// walls comes up, and each block and each crate is pulled up off its plan, a little past its
/// height and back, under a sheet of wet ink that dries. Marks flick out from the corners of
/// its foot when it lands. Everything of a sector wakes at the same moment, so one paint
/// serves all that fades the same way. The blocks and the crates of the room of arena mode
/// rise together too, but for one whose place was taken when its plan ended: the paint then
/// follows the last of them to rise.
fn rise(
    mut commands: Commands,
    time: Res<Time>,
    hold: Option<Res<Hold>>,
    paints: Res<Paints>,
    shapes: Res<Shapes>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut waking: Query<(
        Entity,
        &Ground,
        &mut Waking,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
        Has<Stands>,
        Has<ExplosiveCrate>,
    )>,
    mut marks: Query<(&mut Transform, &mut Visibility), (With<Marks>, Without<Waking>)>,
) {
    let hold = hold.map_or_else(Hold::default, |hold| *hold);
    let mut youngest: Option<f32> = None;
    for (entity, ground, mut waking, mut transform, mut material, stands, explosive) in &mut waking
    {
        waking.age = hold.later(waking.age, time.delta_secs());
        if hold.over(waking.age) {
            // At rest: it is drawn as everything that is awake.
            if let Some(extras) = waking.extras {
                commands.entity(extras.sheen).despawn();
                commands.entity(extras.marks).despawn();
            }
            commands.entity(entity).remove::<Waking>();
            transform.scale = Vec3::ONE;
            material.0 = if ground.floor {
                paints.floor.clone()
            } else {
                paints.ink.clone()
            };
            continue;
        }
        youngest = Some(youngest.map_or(waking.age, |young| young.min(waking.age)));
        if !stands {
            let coming_up = if ground.floor {
                &paints.floor_waking
            } else {
                &paints.ink_waking
            };
            if material.0 != *coming_up {
                material.0 = coming_up.clone();
            }
            continue;
        }
        let kind = explosive as usize;
        let now = wake::rise(waking.age, explosive);
        transform.scale.y = now.height.max(FLAT);
        let grown = now.marks.map(|flicked| mark_growth(explosive, flicked));
        let size = grown.map_or(Vec3::ONE, |grown| Vec3::new(grown, 1.0, grown));
        let shown = if grown.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        match waking.extras {
            Some(extras) => {
                if let Ok((mut placed, mut visibility)) = marks.get_mut(extras.marks) {
                    placed.scale = size;
                    visibility.set_if_neq(shown);
                }
            }
            None => {
                // The sheet lies over the top, and rises with it.
                let (side, top) = (footprint(explosive), obstacle_height(explosive));
                let sheen = (
                    Mesh3d(shapes.plate.clone()),
                    MeshMaterial3d(paints.sheens[kind].clone()),
                    Transform::from_xyz(0.0, top + 2.0 * LIFT, 0.0)
                        .with_scale(Vec3::new(side, 1.0, side)),
                );
                let flicking = (
                    Marks,
                    Mesh3d(shapes.marks[kind].clone()),
                    MeshMaterial3d(paints.marks[kind].clone()),
                    Transform::from_scale(size),
                    shown,
                );
                let extras = Extras {
                    sheen: commands.spawn(sheen).id(),
                    marks: commands.spawn(flicking).id(),
                };
                commands
                    .entity(entity)
                    .add_children(&[extras.sheen, extras.marks]);
                waking.extras = Some(extras);
            }
        }
    }
    let Some(age) = youngest else {
        return;
    };
    let mut paint = |handle: &Handle<StandardMaterial>, color: Color| {
        if let Some(mut material) = materials.get_mut(handle) {
            material.base_color = color;
        }
    };
    let light = wake::light(age);
    for handle in [&paints.floor_waking, &paints.ink_waking] {
        paint(handle, LinearRgba::rgb(light, light, light).into());
    }
    for (kind, explosive) in [false, true].into_iter().enumerate() {
        let now = wake::rise(age, explosive);
        let (sheen, strength) = [(theme::LINE, SHEEN.0), (theme::DANGER, SHEEN.1)][kind];
        paint(&paints.sheens[kind], sheen.with_alpha(strength * now.wet));
        let mark = [theme::BLOCK_LINE, theme::DANGER][kind];
        let left = now.marks.map_or(0.0, |flicked| 1.0 - flicked);
        paint(&paints.marks[kind], mark.with_alpha(left));
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

/// The shape of a bullet shows what it does and what it can still do: a stripe for each
/// bounce it has left, or a chevron if it seeks at its bounces, a pointed tip as long as it
/// pierces, a seam until it splits, and a flat end with a red line if it explodes.
fn bullet_shape<'a>(shapes: &'a Shapes, bullet: &Bullet) -> &'a Handle<Mesh> {
    let shapes = match bullet.kind {
        Kind::Splitting { .. } => return &shapes.splitter,
        Kind::Exploding => return &shapes.grenade,
        Kind::Seeking => &shapes.seekers,
        Kind::Plain if bullet.pierces_left > 0 => &shapes.darts,
        Kind::Plain => &shapes.bullets,
    };
    &shapes[(bullet.bounces_left as usize).min(shapes.len() - 1)]
}

/// Every bullet is white, whoever fired it: the brightest thing on the sheet, with a patch
/// of light on the floor under it. A bullet that explodes leaves a thin trail.
fn dress_bullets(
    mut commands: Commands,
    bullets: Query<(Entity, &Bullet, &Position), Added<Bullet>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
) {
    for (entity, bullet, position) in &bullets {
        let mut dressed = commands.entity(entity);
        dressed.insert((
            Transform {
                translation: to_world(position.0, BULLET_HEIGHT),
                rotation: facing(bullet.velocity.to_angle()),
                ..default()
            },
            children![
                (
                    Shell,
                    Mesh3d(bullet_shape(&shapes, bullet).clone()),
                    MeshMaterial3d(paints.bullet.clone()),
                ),
                // On the floor, under the bullet.
                (
                    Mesh3d(shapes.disc.clone()),
                    MeshMaterial3d(paints.pool.clone()),
                    Transform::from_xyz(0.0, 4.0 * LIFT - BULLET_HEIGHT, 0.0)
                        .with_scale(Vec3::new(POOL_SIZE, 1.0, POOL_SIZE)),
                ),
            ],
        ));
        // A guest may have hidden the bullet already.
        dressed.insert_if_new(Visibility::default());
        if bullet.kind == Kind::Exploding {
            // Behind the tail of the bullet, and of no length yet.
            let tail = Vec3::NEG_X * (BULLET_BODY + BULLET_RADIUS);
            dressed.with_child((
                Trail { from: position.0 },
                Mesh3d(shapes.trail.clone()),
                MeshMaterial3d(paints.trail.clone()),
                Transform::from_translation(tail).with_scale(Vec3::new(0.0, 1.0, 1.0)),
            ));
        }
    }
}

/// A bullet loses a stripe when it bounces and its tip when it pierces.
fn stripe_bullets(
    bullets: Query<(&Bullet, &Children), Changed<Bullet>>,
    mut shells: Query<&mut Mesh3d, With<Shell>>,
    shapes: Res<Shapes>,
) {
    for (bullet, children) in &bullets {
        let wanted = bullet_shape(&shapes, bullet);
        for child in children.iter() {
            if let Ok(mut mesh) = shells.get_mut(child)
                && mesh.0 != *wanted
            {
                mesh.0 = wanted.clone();
            }
        }
    }
}

/// How far from its place something is drawn for a moment. A guest moves its player's tank
/// ahead of what its host tells, and brings the tank to its place this way when the host
/// tells otherwise.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Nudge(pub Vec2);

/// Moves everything to where it is between two ticks of the simulation.
pub fn place(
    fixed: Res<Time<Fixed>>,
    mut moving: Query<(
        &Position,
        &PreviousPosition,
        &mut Transform,
        Option<&Bullet>,
        Option<&Nudge>,
    )>,
) {
    let progress = fixed.overstep_fraction();
    for (position, previous, mut transform, bullet, nudge) in &mut moving {
        let at = previous.0.lerp(position.0, progress) + nudge.map_or(Vec2::ZERO, |nudge| nudge.0);
        transform.translation = to_world(at, transform.translation.y);
        if let Some(bullet) = bullet {
            transform.rotation = facing(bullet.velocity.to_angle());
        }
    }
}

/// The turn around its line of flight that brings the top of a bullet to face a point: the
/// bullet is here and flies this way, and the point is there.
fn roll_toward(bullet: &Transform, eye: Vec3) -> Quat {
    let toward = bullet.rotation.inverse() * (eye - bullet.translation);
    Quat::from_rotation_x(toward.z.atan2(toward.y))
}

/// The shell of a bullet turns its top to the camera, so that a seam and the point of a
/// chevron are seen in the middle of the bullet whichever way it flies.
fn roll_bullets(
    cameras: Query<&Transform, With<Camera3d>>,
    bullets: Query<(&Transform, &Children), With<Bullet>>,
    mut shells: Query<&mut Transform, (With<Shell>, Without<Bullet>, Without<Camera3d>)>,
) {
    let Ok(camera) = cameras.single() else {
        return;
    };
    for (bullet, children) in &bullets {
        for child in children.iter() {
            if let Ok(mut shell) = shells.get_mut(child) {
                shell.rotation = roll_toward(bullet, camera.translation);
            }
        }
    }
}

/// How long the trail of a bullet is when the bullet is this far from where it started: it
/// grows behind the tail of the bullet, up to its full length.
fn trail_length(flown: f32) -> f32 {
    (flown - BULLET_BODY - BULLET_RADIUS).clamp(0.0, TRAIL.x)
}

/// The trail of a bullet that explodes follows it from where it was fired.
fn stretch_trails(
    bullets: Query<&Transform, With<Bullet>>,
    mut trails: Query<(&Trail, &ChildOf, &mut Transform), Without<Bullet>>,
) {
    for (trail, child_of, mut transform) in &mut trails {
        let Ok(bullet) = bullets.get(child_of.parent()) else {
            continue;
        };
        let flown = to_ground(bullet.translation).distance(trail.from);
        transform.scale.x = trail_length(flown);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

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
    fn triangles_cover_every_room_exactly() {
        for seed in 0..400 {
            let room = tanku_core::map::Room::new(seed);
            let corners = room.plan().outline.iter();
            let outline: Vec<Vec2> = corners.map(|point| point.as_vec2()).collect();
            let triangles = triangulate(&outline);
            let covered: f32 = triangles
                .iter()
                .map(|triangle| area(&triangle.map(|corner| outline[corner as usize])))
                .sum();
            assert!(
                (covered - area(&outline)).abs() < 0.01,
                "seed {seed}: {covered} of {}",
                area(&outline)
            );
        }
    }

    #[test]
    fn what_is_on_its_way_is_drawn_flat_on_the_floor() {
        for explosive in [false, true] {
            let plan = planned_drawing(explosive);
            // The plan lies on the floor, within the square where the thing will stand.
            let (low, high) = plan.bounds();
            let half = BLOCK_SIZE / 2.0 + 1e-3;
            assert!(low.y > 0.0 && high.y < 0.05, "{low} {high}");
            assert!(low.x >= -half && low.z >= -half && high.x <= half && high.z <= half);
            // Its outline is whole, with the hatching of a block or the cross of a crate
            // inside it, fainter than on what will stand there.
            let (line, mark) = if explosive {
                (theme::DANGER, theme::DANGER)
            } else {
                (theme::BLOCK_LINE, theme::HATCH)
            };
            let side = if explosive { CRATE_SIZE } else { BLOCK_SIZE };
            let whole = 4.0 * (side - draft::LINE) * draft::LINE;
            assert!(close(plan.area(line), whole), "{}", plan.area(line));
            assert!(plan.area(theme::over(theme::FLOOR, mark, PLANNED_STRENGTH)) > 0.1);
        }
    }

    #[test]
    fn a_planned_block_and_a_planned_crate_show_their_plans() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Plans>()
            .add_systems(PreStartup, setup)
            .add_systems(Update, dress_planned);
        let at = Vec2::new(5.0, 9.0);
        let plan = Planned { explosive: false };
        let block = app.world_mut().spawn((plan, Position(at))).id();
        let other = Planned { explosive: true };
        let explosive = app.world_mut().spawn((other, Position(at + Vec2::X * 4.0)));
        let explosive = explosive.id();
        app.update();
        let shown =
            |app: &App, entity: Entity| app.world().get::<Mesh3d>(entity).unwrap().0.clone();
        let shapes = |app: &App| app.world().resource::<Shapes>().planned.clone();
        assert_eq!(shown(&app, block), shapes(&app)[0]);
        assert_eq!(shown(&app, explosive), shapes(&app)[1]);
        assert_ne!(shapes(&app)[0], shapes(&app)[1]);
        let stands = app.world().get::<Transform>(block).unwrap().translation;
        assert_eq!(stands, to_world(at, 0.0));
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
            .init_resource::<Time>()
            .init_resource::<Plans>()
            .add_systems(PreStartup, setup)
            .add_systems(
                Update,
                (
                    dress_sectors,
                    dress_planned,
                    dress_obstacles,
                    wake_sectors,
                    rise,
                )
                    .chain(),
            );
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

    /// A run of a single player that has reached a sector.
    fn run_in(sector: u32) -> Run {
        Run {
            seed_text: "TEST".into(),
            seed: 1,
            difficulty: default(),
            players: 1,
            team: 1,
            solo: true,
            sector,
            cleared: 0,
            checkpoint: 0,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase: tanku_core::sim::RunPhase::Playing,
            lost_to: None,
            playtest: false,
            earns: true,
            free: false,
            arena: false,
            lull: None,
            paused: false,
        }
    }

    #[test]
    fn a_sector_is_dormant_until_the_players_reach_it() {
        let run = run_in;
        assert!(is_dormant(5, Some(&run(4))));
        assert!(!is_dormant(5, Some(&run(5))));
        // The sectors behind the players stay lit until they are unloaded.
        assert!(!is_dormant(4, Some(&run(5))));
        // Without a run, nothing waits for anybody.
        assert!(!is_dormant(5, None));
    }

    /// How a sector is drawn: whether each of its blocks and crates is its plan, and
    /// whether its walls and its floor are dull.
    struct Drawn {
        plans: Vec<bool>,
        dull_walls: bool,
        dull_floor: bool,
    }

    fn look_of(app: &mut App, sector: u32) -> Drawn {
        let paints = app.world().resource::<Paints>();
        let (ink, ink_dormant) = (paints.ink.clone(), paints.ink_dormant.clone());
        let (floor, floor_dormant) = (paints.floor.clone(), paints.floor_dormant.clone());
        let world = app.world_mut();
        let mut obstacles =
            world.query::<(&Ground, &Stands, &Mesh3d, &MeshMaterial3d<StandardMaterial>)>();
        let plans = obstacles
            .iter(world)
            .filter(|(ground, ..)| ground.sector == sector)
            .map(|(_, stands, mesh, material)| {
                // A plan is in full ink, as what stands is.
                assert!(material.0 == ink);
                assert!(mesh.0 == stands.plan || mesh.0 == stands.built);
                mesh.0 == stands.plan
            })
            .collect();
        let mut grounds = world.query::<(&Ground, &MeshMaterial3d<StandardMaterial>, Has<Walls>)>();
        let of_sector = |(ground, ..): &(&Ground, _, bool)| ground.sector == sector;
        let grounds: Vec<_> = grounds.iter(world).filter(of_sector).collect();
        let walls = grounds.iter().find(|(.., walls)| *walls);
        let walls = &walls.expect("a sector with walls").1.0;
        assert!(*walls == ink || *walls == ink_dormant);
        let ground = grounds.iter().find(|(ground, ..)| ground.floor);
        let ground = &ground.expect("a sector with a floor").1.0;
        assert!(*ground == floor || *ground == floor_dormant);
        Drawn {
            plans,
            dull_walls: *walls == ink_dormant,
            dull_floor: *ground == floor_dormant,
        }
    }

    #[test]
    fn the_blocks_and_the_crates_of_a_sector_are_plans_until_a_player_enters_it() {
        // The first sectors of this seed hold blocks.
        let mut corridor = Corridor::new(seed_value("3ZXHW3M8"));
        let mut app = dressing_world();
        app.insert_resource(run_in(0));
        let sectors: Vec<Entity> = (0..2)
            .map(|index| load(&mut app, &mut corridor, index))
            .collect();
        let explosive = (ExplosiveCrate { sector: 1 }, Position(Vec2::new(1.0, 40.0)));
        let explosive = app.world_mut().spawn(explosive).id();
        let behind = (Block { sector: 0 }, Position(Vec2::new(1.0, 4.0)));
        let behind = app.world_mut().spawn(behind).id();
        app.update();

        // What stands in the bay stands, on its lit floor.
        let bay = look_of(&mut app, 0);
        assert!(!bay.dull_walls && !bay.dull_floor);
        assert_eq!(bay.plans, [false]);
        // The sector behind its barrier is dull, and its blocks and its crate are plans.
        let ahead = look_of(&mut app, 1);
        assert!(ahead.dull_walls && ahead.dull_floor);
        assert!(ahead.plans.len() >= 2 && ahead.plans.iter().all(|plan| *plan));
        let shapes = app.world().resource::<Shapes>();
        let (crate_plan, crate_built) = (shapes.planned[1].clone(), shapes.explosive.clone());
        let (block_plan, block_built) = (shapes.planned[0].clone(), shapes.block.clone());
        let shown =
            |app: &App, entity: Entity| app.world().get::<Mesh3d>(entity).unwrap().0.clone();
        assert!(shown(&app, explosive) == crate_plan && shown(&app, behind) == block_built);
        let world = app.world_mut();
        let mut blocks = world.query::<(Entity, &Block)>();
        let block = blocks.iter(world).find(|(_, block)| block.sector == 1);
        let block = block.expect("a block in the first sector").0;
        assert!(shown(&app, block) == block_plan);
        // A plan lies flat on the floor, and what stands doesn't.
        let height = |app: &App, mesh: &Handle<Mesh>| {
            let mesh = app.world().resource::<Assets<Mesh>>().get(mesh).unwrap();
            let heights = corners_of(mesh).into_iter().map(|corner| corner[1]);
            heights.fold(0.0, f32::max)
        };
        assert!(height(&app, &block_plan) < 0.05 && height(&app, &crate_plan) < 0.05);
        assert!(height(&app, &block_built) > 1.0 && height(&app, &crate_built) > 0.8);
        // The walls of a sector stand whether a player has entered it or not: each sector
        // has one drawing of them, which is no plan.
        let walls_of = |app: &App, sector: Entity| -> Handle<Mesh> {
            let world = app.world();
            let children = world.entity(sector).get::<Children>().unwrap();
            let mut walls = children
                .iter()
                .filter(|&child| world.get::<Walls>(child).is_some());
            let drawn = walls.next().expect("a sector with walls");
            assert!(world.get::<Stands>(drawn).is_none() && walls.next().is_none());
            world.get::<Mesh3d>(drawn).unwrap().0.clone()
        };
        let walls: Vec<Handle<Mesh>> = sectors.iter().map(|&s| walls_of(&app, s)).collect();
        assert!(walls.iter().all(|mesh| height(&app, mesh) > 1.0));

        // The first player crosses: the sector comes to life, and its blocks and its crate
        // stand up, which takes them a third of a second. Its walls are the ones that stood
        // there.
        app.world_mut().resource_mut::<Run>().sector = 1;
        app.update();
        let time = Duration::from_secs_f32(wake::LENGTH);
        app.world_mut().resource_mut::<Time>().advance_by(time);
        app.update();
        let entered = look_of(&mut app, 1);
        assert!(!entered.dull_walls && !entered.dull_floor);
        assert!(entered.plans.iter().all(|plan| !*plan));
        assert!(shown(&app, explosive) == crate_built && shown(&app, block) == block_built);
        assert_eq!(look_of(&mut app, 0).plans, [false]);
        let after: Vec<Handle<Mesh>> = sectors.iter().map(|&s| walls_of(&app, s)).collect();
        assert!(after == walls);

        // A single player who fell comes back in front of the barrier, and the sector is as
        // it was before they entered it.
        app.world_mut().resource_mut::<Run>().sector = 0;
        app.update();
        let again = look_of(&mut app, 1);
        assert!(again.dull_walls && again.dull_floor);
        assert!(again.plans.iter().all(|plan| *plan));
        assert!(shown(&app, explosive) == crate_plan && shown(&app, block) == block_plan);
    }

    /// How many parts of the ground of a sector still take their time to wake, how tall each
    /// of its blocks and of its crates is as a share of its height, and how many of them show
    /// the marks at the corners of their feet.
    fn waking(app: &mut App, sector: u32) -> (usize, Vec<f32>, usize) {
        let world = app.world_mut();
        let mut grounds = world.query::<(&Ground, Option<&Waking>, Has<Stands>, &Transform)>();
        let of_sector = |(ground, ..): &(&Ground, _, _, _)| ground.sector == sector;
        let found: Vec<_> = grounds.iter(world).filter(of_sector).collect();
        let late = found.iter().filter(|(_, waking, ..)| waking.is_some());
        let heights = found.iter().filter(|(.., stands, _)| *stands);
        let heights = heights.map(|(.., placed)| placed.scale.y).collect();
        let late = late.count();
        let mut marks = world.query_filtered::<&Visibility, With<Marks>>();
        let shown = marks
            .iter(world)
            .filter(|shown| **shown != Visibility::Hidden);
        (late, heights, shown.count())
    }

    #[test]
    fn a_sector_takes_its_time_to_wake() {
        // The first sectors of this seed hold blocks.
        let mut corridor = Corridor::new(seed_value("3ZXHW3M8"));
        let mut app = dressing_world();
        app.insert_resource(run_in(0));
        for index in 0..2 {
            load(&mut app, &mut corridor, index);
        }
        let explosive = (ExplosiveCrate { sector: 1 }, Position(Vec2::new(1.0, 40.0)));
        let explosive = app.world_mut().spawn(explosive).id();
        let frame = |app: &mut App, seconds: f32| {
            let time = Duration::from_secs_f32(seconds);
            app.world_mut().resource_mut::<Time>().advance_by(time);
            app.update();
        };
        frame(&mut app, 0.0);
        // What is loaded awake is drawn awake at once, and what waits is a plan.
        assert_eq!(waking(&mut app, 0), (0, vec![], 0));
        let (late, heights, _) = waking(&mut app, 1);
        assert!(late == 0 && heights.len() >= 2 && heights.iter().all(|high| *high == 1.0));
        assert!(look_of(&mut app, 1).plans.iter().all(|plan| *plan));

        // The first player crosses. A frame later, nothing of the sector is at rest: its
        // floor and its walls are on their way to their light, and each block and the crate
        // show what stands, low still, under ink that is wet.
        app.world_mut().resource_mut::<Run>().sector = 1;
        frame(&mut app, 1.0 / 60.0);
        let (late, heights, marked) = waking(&mut app, 1);
        assert_eq!((late, marked), (heights.len() + 2, 0));
        assert!(heights.iter().all(|high| *high >= FLAT && *high < 0.5));
        let world = app.world_mut();
        let paints = world.resource::<Paints>();
        let (coming_up, wet) = (paints.floor_waking.clone(), paints.sheens[1].clone());
        let built = world.resource::<Shapes>().explosive.clone();
        assert!(world.get::<Mesh3d>(explosive).unwrap().0 == built);
        let mut floors = world.query::<(&Ground, &MeshMaterial3d<StandardMaterial>)>();
        let mut floors = floors.iter(world).filter(|(ground, _)| ground.floor);
        assert!(floors.any(|(ground, paint)| ground.sector == 1 && paint.0 == coming_up));
        let light = |world: &World, paint: &Handle<StandardMaterial>| {
            let paints = world.resource::<Assets<StandardMaterial>>();
            paints.get(paint).unwrap().base_color.to_linear()
        };
        assert!(light(world, &coming_up).red > theme::DORMANT && light(world, &wet).alpha > 0.4);
        // The bay sleeps no more than it did.
        assert_eq!(waking(&mut app, 0).0, 0);

        // A tenth of a second later, they are past their height, and marks have flicked out
        // from the corners of their feet.
        frame(&mut app, 0.1);
        let (late, heights, marked) = waking(&mut app, 1);
        assert_eq!((late, marked), (heights.len() + 2, heights.len()));
        assert!(heights.iter().all(|high| *high > 1.0 && *high < 1.15));

        // A third of a second after the crossing, the sector is drawn as any that is awake.
        frame(&mut app, 0.25);
        let (late, heights, _) = waking(&mut app, 1);
        assert!(late == 0 && heights.iter().all(|high| *high == 1.0));
        let entered = look_of(&mut app, 1);
        assert!(!entered.dull_walls && !entered.dull_floor);
        assert!(entered.plans.iter().all(|plan| !*plan));
        let world = app.world_mut();
        assert_eq!(world.query::<&Marks>().iter(world).count(), 0);
        assert!(
            world
                .get::<Children>(explosive)
                .is_none_or(|kept| kept.is_empty())
        );

        // A single player who falls while a sector wakes finds it asleep again, as it was.
        app.world_mut().resource_mut::<Run>().sector = 0;
        frame(&mut app, 1.0);
        app.world_mut().resource_mut::<Run>().sector = 1;
        frame(&mut app, 0.11);
        assert!(waking(&mut app, 1).2 > 0);
        app.world_mut().resource_mut::<Run>().sector = 0;
        frame(&mut app, 1.0 / 60.0);
        let (late, heights, _) = waking(&mut app, 1);
        assert!(late == 0 && heights.iter().all(|high| *high == 1.0));
        let again = look_of(&mut app, 1);
        assert!(again.dull_walls && again.dull_floor && again.plans.iter().all(|plan| *plan));
        let world = app.world_mut();
        assert_eq!(world.query::<&Marks>().iter(world).count(), 0);
    }

    #[test]
    fn what_stands_on_a_plan_of_the_room_rises_off_it() {
        // The room of arena mode is the sector 0 of its run.
        let mut app = dressing_world();
        app.insert_resource(run_in(0));
        let frame = |app: &mut App, seconds: f32| {
            let time = Duration::from_secs_f32(seconds);
            app.world_mut().resource_mut::<Time>().advance_by(time);
            app.update();
        };
        let block = |app: &mut App, at: Vec2| {
            let world = app.world_mut();
            world.spawn((Block { sector: 0 }, Position(at))).id()
        };
        let plan = |app: &mut App, at: Vec2, explosive: bool| {
            let world = app.world_mut();
            world.spawn((Planned { explosive }, Position(at))).id()
        };
        let places = [3.0, 7.0, 11.0, 15.0, 19.0].map(|x| Vec2::new(x, 3.0));
        // What the room holds when a run opens it stands at once: it had no plan.
        block(&mut app, places[0]);
        // What the next group is fought among is planned.
        let plans: Vec<Entity> = (1..4)
            .map(|index| plan(&mut app, places[index], index == 2))
            .collect();
        frame(&mut app, 0.0);
        assert_eq!(waking(&mut app, 0), (0, vec![1.0], 0));

        // Their time comes: the server takes a plan away and stands a block or a crate in its
        // place, in one tick. They rise off their plans.
        for &planned in &plans[..2] {
            app.world_mut().despawn(planned);
        }
        let risen = block(&mut app, places[1]);
        let explosive = (ExplosiveCrate { sector: 0 }, Position(places[2]));
        app.world_mut().spawn(explosive);
        frame(&mut app, 1.0 / 60.0);
        let (late, heights, _) = waking(&mut app, 0);
        assert_eq!(late, 2);
        assert_eq!(heights.iter().filter(|high| **high == 1.0).count(), 1);
        assert!(heights.iter().all(|high| *high == 1.0 || *high < 0.5));
        let built = app.world().resource::<Shapes>().block.clone();
        assert!(app.world().get::<Mesh3d>(risen).unwrap().0 == built);
        // A tenth of a second later, they are past their height, and their feet are marked.
        frame(&mut app, 0.1);
        let (_, heights, marked) = waking(&mut app, 0);
        assert_eq!(heights.iter().filter(|high| **high > 1.0).count(), 2);
        assert_eq!(marked, 2);

        // A guest may learn of a block a moment after it learned that its plan was over. It
        // rises all the same.
        app.world_mut().despawn(plans[2]);
        frame(&mut app, 1.0 / 60.0);
        let later = block(&mut app, places[3]);
        frame(&mut app, 1.0 / 60.0);
        assert!(app.world().get::<Waking>(later).is_some());

        // A third of a second after they stood, all of it is at rest.
        frame(&mut app, wake::LENGTH);
        let (late, heights, marked) = waking(&mut app, 0);
        assert_eq!((late, marked), (0, 0));
        assert!(heights.len() == 4 && heights.iter().all(|high| *high == 1.0));

        // A place whose plan ended long before is no plan anymore: a block that comes there
        // stands at once.
        let gone = plan(&mut app, places[4], false);
        frame(&mut app, 0.0);
        app.world_mut().despawn(gone);
        frame(&mut app, 1.0 / 60.0);
        frame(&mut app, 2.0 * PLAN_KEPT);
        block(&mut app, places[4]);
        frame(&mut app, 1.0 / 60.0);
        let (late, heights, _) = waking(&mut app, 0);
        assert!(late == 0 && heights.iter().all(|high| *high == 1.0));
    }

    #[test]
    fn the_marks_of_a_foot_start_next_to_it_and_flick_out() {
        for explosive in [false, true] {
            let (lowest, highest) = mark_drawing(explosive).bounds();
            // They lie on the floor, around the square of the block or of the crate.
            let foot = footprint(explosive) / 2.0;
            assert!(highest.y < 0.05 && lowest.y > 0.0);
            let reach = foot + MARK_REACH.0 + draft::LINE / 2.0;
            assert!(close(highest.x, reach) && close(lowest.z, -reach));
            assert!(close(mark_growth(explosive, 0.0), 1.0));
            let grown = (foot + MARK_REACH.1) / (foot + MARK_REACH.0);
            assert!(close(mark_growth(explosive, 1.0), grown));
        }
    }

    #[test]
    fn the_circles_of_an_arena_fit_its_floor() {
        // The smallest arena is 16 m across, and the largest 36 m.
        for across in [16.0, 24.0, 36.0] {
            let (outer, inner) = arena_rings(Vec2::ZERO, Vec2::new(across, 34.0));
            assert!(
                outer <= across / 2.0 - 3.0 || outer == 4.0,
                "{across}: {outer}"
            );
            assert!(outer <= 11.0 && inner < outer && inner > 2.0);
        }
        // They are drawn around the middle of the arena, on the floor.
        let drawing = arena_drawing(Vec2::new(10.0, 0.0), Vec2::new(40.0, 34.0));
        let (low, high) = drawing.bounds();
        let middle = to_ground((low + high) / 2.0);
        assert!(middle.distance(Vec2::new(25.0, 17.0)) < 0.2, "{middle}");
        assert!(high.y < 0.1);
    }

    /// The triangles of a bullet, each with its color.
    fn painted_triangles(look: Look, stripes: u8) -> Vec<([Vec3; 3], [f32; 4])> {
        let mesh = bullet_mesh(look, stripes);
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
                (corners, colors[0])
            })
            .collect()
    }

    /// The triangles of a bullet, and whether each is part of a stripe.
    fn bullet_triangles(look: Look, stripes: u8) -> Vec<([Vec3; 3], bool)> {
        let painted = painted_triangles(look, stripes).into_iter();
        painted
            .map(|(corners, color)| (corners, color == STRIPE_COLOR))
            .collect()
    }

    /// The stretches along the line of flight that these stretches cover, joined where they
    /// touch.
    fn joined(mut stretches: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
        stretches.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut bands: Vec<(f32, f32)> = Vec::new();
        for (from, to) in stretches {
            match bands.last_mut() {
                Some(band) if from <= band.1 + 1e-4 => band.1 = band.1.max(to),
                _ => bands.push((from, to)),
            }
        }
        bands
    }

    #[test]
    fn bullets_show_one_stripe_for_each_bounce_left() {
        for (look, stripes) in [Look::Round, Look::Dart]
            .into_iter()
            .flat_map(|look| (0..=3u8).map(move |stripes| (look, stripes)))
        {
            // The dark stretches along the axis.
            let dark: Vec<(f32, f32)> = bullet_triangles(look, stripes)
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
            let bands = joined(dark);
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
    fn a_bullet_that_seeks_shows_a_chevron_for_each_bounce_left() {
        // Without a bounce left, it is a round bullet.
        let plain = |look: Look| painted_triangles(look, 0);
        assert_eq!(plain(Look::Seeker), plain(Look::Round));
        for stripes in 1..=3u8 {
            let triangles = bullet_triangles(Look::Seeker, stripes);
            // Its skin is that of a round bullet, from end to end.
            let reach = BULLET_BODY + BULLET_RADIUS;
            let corners = triangles.iter().flat_map(|(corners, _)| *corners);
            for corner in corners.clone() {
                let radius = Vec2::new(corner.y, corner.z).length();
                assert!((radius - girth(corner.x)).abs() < 1e-4, "{corner}");
            }
            let along = corners.map(|corner| corner.x);
            assert!((along.clone().fold(f32::MAX, f32::min) + reach).abs() < 1e-5);
            assert!((along.fold(f32::MIN, f32::max) - reach).abs() < 1e-5);
            // The dark stretches along a line on the skin: the top of the bullet, or one of
            // its sides.
            let dark_along =
                |on_the_line: fn(Vec3) -> bool| {
                    let edges = triangles.iter().filter(|(_, striped)| *striped).filter_map(
                        |(corners, _)| {
                            let on: Vec<f32> = corners
                                .iter()
                                .filter(|corner| on_the_line(**corner))
                                .map(|corner| corner.x)
                                .collect();
                            (on.len() == 2).then(|| (on[0].min(on[1]), on[0].max(on[1])))
                        },
                    );
                    joined(edges.collect())
                };
            let top = dark_along(|corner| corner.z.abs() < 1e-5 && corner.y > 0.0);
            let side = dark_along(|corner| corner.y.abs() < 1e-5 && corner.z > 0.0);
            assert_eq!(
                (top.len(), side.len()),
                (stripes as usize, stripes as usize)
            );
            // A chevron is as wide as a stripe and stands where the stripe would, but for its
            // lean: its point is ahead on top of the bullet, and its ends are behind on the
            // sides.
            let centers = |bands: &[(f32, f32)]| -> Vec<f32> {
                bands.iter().map(|band| (band.0 + band.1) / 2.0).collect()
            };
            for (top, side) in top.iter().zip(&side) {
                assert!((top.1 - top.0 - STRIPE_WIDTH).abs() < 1e-4);
                assert!((side.1 - side.0 - STRIPE_WIDTH).abs() < 1e-4);
                assert!((top.0 - side.0 - CHEVRON_LEAN).abs() < 1e-4);
            }
            let (top, side) = (centers(&top), centers(&side));
            assert!((top.iter().sum::<f32>() / stripes as f32 - CHEVRON_LEAN / 2.0).abs() < 1e-4);
            assert!((side.iter().sum::<f32>() / stripes as f32 + CHEVRON_LEAN / 2.0).abs() < 1e-4);
            for pair in top.windows(2) {
                assert!((pair[1] - pair[0] - STRIPE_WIDTH - STRIPE_GAP).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn a_bullet_that_splits_shows_a_seam_and_no_stripe() {
        let triangles = bullet_triangles(Look::Splitter, 0);
        let dark: Vec<[Vec3; 3]> = triangles
            .iter()
            .filter(|(_, striped)| *striped)
            .map(|(corners, _)| *corners)
            .collect();
        assert!(!dark.is_empty());
        // The seam runs along the top of the bullet, as long and as wide as it is said to be.
        let corners = dark.iter().flatten();
        let reach = |of: fn(&Vec3) -> f32| corners.clone().map(of).fold(f32::MIN, f32::max);
        assert!((reach(|corner| corner.x) - SEAM.x / 2.0).abs() < 1e-4);
        assert!((reach(|corner| -corner.x) - SEAM.x / 2.0).abs() < 1e-4);
        assert!((reach(|corner| corner.z) - SEAM.y / 2.0).abs() < 1e-4);
        assert!((reach(|corner| -corner.z) - SEAM.y / 2.0).abs() < 1e-4);
        assert!(corners.clone().all(|corner| corner.y > 0.0));
        // The rest is the round bullet, white all around.
        let white = triangles.iter().filter(|(_, striped)| !*striped);
        let below = white.filter(|(corners, _)| corners.iter().all(|corner| corner.y < -0.1));
        assert!(below.count() > 0);
        let reach = BULLET_BODY + BULLET_RADIUS;
        let along = triangles
            .iter()
            .flat_map(|(corners, _)| corners.map(|corner| corner.x));
        assert!((along.clone().fold(f32::MAX, f32::min) + reach).abs() < 1e-5);
        assert!((along.fold(f32::MIN, f32::max) - reach).abs() < 1e-5);
    }

    #[test]
    fn a_bullet_that_explodes_shows_a_flat_end_and_a_red_line_that_cuts_it() {
        let red = theme::DANGER.to_linear().to_f32_array();
        let triangles = painted_triangles(Look::Grenade, 0);
        let radius = |corner: &Vec3| Vec2::new(corner.y, corner.z).length();
        let reach = BULLET_BODY + BULLET_RADIUS;
        // The stretches along the line of flight that each color covers.
        let stretches = |wanted: [f32; 4]| {
            let of_the_color = triangles.iter().filter(|(_, color)| *color == wanted);
            joined(
                of_the_color
                    .map(|(corners, _)| {
                        let along = corners.map(|corner| corner.x);
                        (
                            along.into_iter().fold(f32::MAX, f32::min),
                            along.into_iter().fold(f32::MIN, f32::max),
                        )
                    })
                    .collect(),
            )
        };
        let close = |found: (f32, f32), wanted: (f32, f32)| {
            (found.0 - wanted.0).abs() < 1e-5 && (found.1 - wanted.1).abs() < 1e-5
        };
        // The round part and the flat end are white, with the red line between them and a
        // gap on each side of it.
        let white = stretches([1.0, 1.0, 1.0, 1.0]);
        assert_eq!(white.len(), 2);
        assert!(close(white[0], (-reach, GRENADE_CUT)), "{white:?}");
        assert!(close(white[1], (FLAT_END, reach)), "{white:?}");
        let line = stretches(red);
        assert_eq!(line.len(), 1);
        assert!(close(line[0], RED_LINE), "{line:?}");
        assert!(RED_LINE.0 > GRENADE_CUT && FLAT_END > RED_LINE.1);
        assert_eq!(triangles.len(), {
            let colored = |(_, color): &&([Vec3; 3], [f32; 4])| *color == red || color[0] == 1.0;
            triangles.iter().filter(colored).count()
        });
        // The line is a little wider than the bullet, and the bullet ends flat: as wide at
        // its very end as anywhere.
        let corners = triangles.iter().flat_map(|(corners, color)| {
            let color = *color;
            corners.iter().map(move |corner| (corner, color))
        });
        let widest = |wanted: fn(&Vec3, [f32; 4]) -> bool| {
            let radii = corners
                .clone()
                .filter(|(corner, color)| wanted(corner, *color))
                .map(|(corner, _)| radius(corner));
            radii.fold(0.0, f32::max)
        };
        assert!((widest(|_, color| color[0] == 1.0) - BULLET_RADIUS).abs() < 1e-5);
        let red_width = widest(|_, color| color[0] != 1.0);
        assert!((red_width - RED_LINE_RADIUS).abs() < 1e-5);
        assert!(red_width > BULLET_RADIUS && red_width < 1.5 * BULLET_RADIUS);
        let end = widest(|corner, _| corner.x > BULLET_BODY + BULLET_RADIUS - 1e-5);
        assert!((end - BULLET_RADIUS).abs() < 1e-5);
    }

    #[test]
    fn bullets_show_their_outside() {
        // A face is drawn from the side on which its corners turn counterclockwise.
        for look in [Look::Round, Look::Dart, Look::Seeker, Look::Splitter] {
            for (corners, _) in bullet_triangles(look, 2) {
                let facing = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
                let center = (corners[0] + corners[1] + corners[2]) / 3.0;
                let away_from_axis = Vec3::new(0.0, center.y, center.z);
                assert!(facing.dot(away_from_axis) > 0.0, "{look:?} {corners:?}");
            }
        }
        // The pieces of a bullet that explodes have flat ends, which face along the line of
        // flight: away from the middle of the piece.
        let reach = BULLET_BODY + BULLET_RADIUS;
        let pieces = [(-reach, GRENADE_CUT), RED_LINE, (FLAT_END, reach)];
        for (corners, _) in bullet_triangles(Look::Grenade, 0) {
            let facing = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            let center = (corners[0] + corners[1] + corners[2]) / 3.0;
            let holds =
                |piece: &&(f32, f32)| center.x > piece.0 - 1e-5 && center.x < piece.1 + 1e-5;
            let piece = pieces.iter().find(holds).unwrap();
            let outward = Vec3::new(center.x - (piece.0 + piece.1) / 2.0, center.y, center.z);
            assert!(facing.dot(outward) > 0.0, "{corners:?}");
        }
    }

    #[test]
    fn the_shell_of_a_bullet_turns_its_top_to_the_camera() {
        // A camera to the south of the bullets, high above the ground, as in the game.
        let eye = Vec3::new(0.0, 20.0, 10.0);
        for heading in [0.0, 0.7, 1.6, 3.0, -2.2] {
            let bullet = Transform {
                translation: to_world(Vec2::new(3.0, 4.0), BULLET_HEIGHT),
                rotation: facing(heading),
                ..default()
            };
            let roll = roll_toward(&bullet, eye);
            // The bullet still flies the same way, and its top is as near to the camera as a
            // turn around its line of flight can bring it.
            let (along, top) = (bullet.rotation * Vec3::X, bullet.rotation * roll * Vec3::Y);
            assert!((bullet.rotation * roll * Vec3::X).distance(along) < 1e-5);
            let toward = eye - bullet.translation;
            let across = (toward - along * toward.dot(along)).normalize();
            assert!(top.distance(across) < 1e-4, "{heading}: {top} {across}");
        }
    }

    #[test]
    fn the_trail_of_a_bullet_that_explodes_grows_behind_it() {
        // Nothing while the bullet is no further than its own length from where it started,
        // then as much as it has flown, up to the length of the trail.
        assert_eq!(trail_length(0.0), 0.0);
        assert_eq!(trail_length(BULLET_BODY + BULLET_RADIUS), 0.0);
        assert!(close(trail_length(1.0), 1.0 - BULLET_BODY - BULLET_RADIUS));
        assert_eq!(trail_length(40.0), TRAIL.x);

        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Time<Fixed>>()
            .add_systems(PreStartup, setup)
            .add_systems(
                Update,
                (dress_bullets, stripe_bullets, place, stretch_trails).chain(),
            );
        let bullet = |kind: Kind| Bullet {
            owner: Entity::PLACEHOLDER,
            team: tanku_core::sim::Team::Enemies,
            sector: 1,
            velocity: Vec2::X * 9.0,
            bounces_left: 0,
            pierces_left: 0,
            bounced: false,
            kind,
        };
        let at = Vec2::new(2.0, 5.0);
        let place = (Position(at), PreviousPosition(at));
        let exploding = app.world_mut().spawn((bullet(Kind::Exploding), place)).id();
        let plain = app.world_mut().spawn((bullet(Kind::Plain), place)).id();
        app.update();
        let trails = |app: &mut App, bullet: Entity| -> Vec<f32> {
            let world = app.world_mut();
            let children: Vec<Entity> = world.get::<Children>(bullet).unwrap().iter().collect();
            let trail = |child: &Entity| world.get::<Trail>(*child).is_some();
            let length = |child: Entity| world.get::<Transform>(child).unwrap().scale.x;
            children.into_iter().filter(trail).map(length).collect()
        };
        // Only the bullet that explodes has a trail, which is of no length at first.
        assert_eq!(trails(&mut app, exploding), [0.0]);
        assert_eq!(trails(&mut app, plain), [] as [f32; 0]);
        for (flown, wanted) in [(1.0, 0.55), (3.0, TRAIL.x)] {
            let there = at + Vec2::X * flown;
            let mut entity = app.world_mut().entity_mut(exploding);
            entity.insert((Position(there), PreviousPosition(there)));
            app.update();
            let found = trails(&mut app, exploding);
            assert!(close(found[0], wanted), "{found:?}");
        }
    }

    #[test]
    fn the_shell_of_a_bullet_follows_what_the_bullet_can_still_do() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .add_systems(PreStartup, setup)
            .add_systems(Update, (dress_bullets, stripe_bullets).chain());
        let bullet = |kind: Kind, bounces_left: u8, pierces_left: u8| Bullet {
            owner: Entity::PLACEHOLDER,
            team: tanku_core::sim::Team::Enemies,
            sector: 1,
            velocity: Vec2::X * 9.0,
            bounces_left,
            pierces_left,
            bounced: false,
            kind,
        };
        let shell = |app: &mut App, bullet: Entity| -> Handle<Mesh> {
            let world = app.world_mut();
            let children: Vec<Entity> = world.get::<Children>(bullet).unwrap().iter().collect();
            let shells: Vec<Entity> = children
                .into_iter()
                .filter(|child| world.get::<Shell>(*child).is_some())
                .collect();
            assert_eq!(shells.len(), 1);
            world.get::<Mesh3d>(shells[0]).unwrap().0.clone()
        };
        let splitting = Kind::Splitting {
            fuse: 1.0,
            bounces: 2,
        };
        let spawned = [
            bullet(Kind::Plain, 2, 0),
            bullet(Kind::Plain, 1, 1),
            bullet(Kind::Seeking, 3, 0),
            bullet(splitting, 0, 0),
            bullet(Kind::Exploding, 0, 0),
        ]
        .map(|bullet| app.world_mut().spawn(bullet).id());
        app.update();
        let shapes = |app: &App| -> [Handle<Mesh>; 5] {
            let shapes = app.world().resource::<Shapes>();
            [
                shapes.bullets[2].clone(),
                shapes.darts[1].clone(),
                shapes.seekers[3].clone(),
                shapes.splitter.clone(),
                shapes.grenade.clone(),
            ]
        };
        let wanted = shapes(&app);
        for (entity, wanted) in spawned.into_iter().zip(wanted) {
            assert_eq!(shell(&mut app, entity), wanted);
            // The bullet itself is no drawing: its shell is, and turns on it.
            assert!(app.world().get::<Mesh3d>(entity).is_none());
            assert!(app.world().get::<Visibility>(entity).is_some());
        }
        // A bounce takes a chevron off a bullet that seeks, as it takes a stripe off another.
        let seeker = spawned[2];
        app.world_mut()
            .get_mut::<Bullet>(seeker)
            .unwrap()
            .bounces_left = 1;
        app.update();
        let one_chevron = app.world().resource::<Shapes>().seekers[1].clone();
        assert_eq!(shell(&mut app, seeker), one_chevron);
        // A bullet that a guest has hidden stays hidden when it is dressed.
        let hidden = (bullet(Kind::Plain, 0, 0), Visibility::Hidden);
        let hidden = app.world_mut().spawn(hidden).id();
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(hidden),
            Some(&Visibility::Hidden)
        );
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
