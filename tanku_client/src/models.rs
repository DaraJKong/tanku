//! The models of the tanks, assembled from their genomes.
//!
//! Every stat has its own visual channel, so that players can read a tank before fighting it.
//! A model points along `+x` and stands on the ground.
//!
//! A tank is two drawings, its hull and its turret. The parts in the color of the tank are
//! filled with it, and the others are dark shapes edged with the pale lines of the sheet.
//!
//! A tank that isn't there is an outline instead: its plan in dashes, flat on the sheet, in
//! its color. A dormant enemy isn't there yet, and is inked in when it wakes up.

use bevy::prelude::*;
use tanku_core::{
    rules::SHIELD_RADIUS,
    sim::{Enemy, Motion, Player, Run, Shield, Tank, Wreck},
    stats::{Class, Genome, Stat},
};

use crate::{
    draft::{Draft, LIFT, LINE},
    scene::{self, Paints, Shapes},
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
/// Height of the roof of the turret above its ring.
const ROOF: f32 = 0.32;
/// Height of a fixed base, and how far apart the segments of the drum are.
const BASE_HEIGHT: f32 = 0.26;
const DRUM_PITCH: f32 = 0.12;
/// How far from the middle of the hull its tracks start, on each side.
const TRACK_GAUGE: f32 = 0.52;
/// Radius of a mine of the rack, and of the bead on top of an antenna.
const RACK_MINE: f32 = 0.11;
const BEAD: f32 = 0.06;
/// Length, height and width of a segment of the drum.
const SEGMENT: Vec3 = Vec3::new(0.09, 0.24, 0.36);
/// Width of the barrel.
const BARREL: f32 = 0.16;
/// Length and width of a ring around the muzzle, of a vent and of a lens of the sight.
const RING: Vec2 = Vec2::new(0.06, 0.3);
const VENT: Vec2 = Vec2::new(0.1, 0.12);
const LENS: Vec2 = Vec2::new(0.14, 0.11);
/// Radius and length of the pointed tip of a barrel, and how far into the barrel it starts.
const TIP: Vec2 = Vec2::new(0.1, 0.32);
const TIP_SEAT: f32 = 0.02;

/// How far above the sheet an outline lies, and its turret above its hull: over the marks
/// on the floor, and too little to show.
const OUTLINE_HEIGHT: f32 = 5.0 * LIFT;
const OUTLINE_TURRET: f32 = 5.0 * LIFT;
/// How much of the color of a tank fills its body and its turret in an outline.
const OUTLINE_TINT: f32 = 0.1;

#[derive(Component)]
struct Hull;

#[derive(Component)]
struct Turret;

#[derive(Component)]
struct Bubble;

/// How a tank is drawn: as it stands, or as the outline of a tank that isn't there.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
enum Look {
    Solid,
    Outline,
}

/// The colors of the outline of a tank.
#[derive(Clone, Copy)]
pub struct Outline {
    /// Its lines.
    pub line: Color,
    /// What fills its body and the roof of its turret.
    pub body: Color,
    /// The sheet it lies on, which fills the parts that lie over others.
    pub sheet: Color,
}

impl Outline {
    /// The outline of a tank of this color that isn't there yet, on a sheet of that color:
    /// its body and its turret are filled with a little of its color.
    pub fn dormant(color: Color, sheet: Color) -> Self {
        Self {
            line: color,
            body: theme::over(sheet, color, OUTLINE_TINT),
            sheet,
        }
    }
}

/// What every shield bubble is made of.
#[derive(Resource)]
struct Glass {
    bubble: Handle<StandardMaterial>,
    edge: Handle<StandardMaterial>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(
        Update,
        (build, ink_in, (pose, shield)).chain().before(scene::place),
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

/// The hue of a class, in degrees: teal for a sniper, yellow for a scout, green for a
/// tactician, purple for a trickster, orange for a lancer, pink for a sapper and red for a
/// gunner. A tank without a class has none: it is gray. Blue is the players'.
pub fn class_hue(class: Class) -> Option<f32> {
    match class {
        Class::None => None,
        Class::Sniper => Some(176.0),
        Class::Scout => Some(52.0),
        Class::Tactician => Some(125.0),
        Class::Trickster => Some(272.0),
        Class::Lancer => Some(26.0),
        Class::Sapper => Some(322.0),
        Class::Gunner => Some(0.0),
    }
}

/// The color of an enemy: its hue tells its class, and its intensity tells how many points
/// it has. Even the weakest tank shows its hue.
pub fn enemy_color(genome: &Genome) -> Color {
    let strength = (genome.budget as f32 / 100.0).clamp(0.0, 1.0);
    let lightness = 0.68 - 0.22 * strength;
    match class_hue(genome.class) {
        Some(hue) => Color::hsl(hue, 0.4 + 0.55 * strength, lightness),
        None => Color::hsl(0.0, 0.0, lightness),
    }
}

/// The color of the sight and of the tip of the barrel: a light tint of the hue of the body,
/// or a light gray on a gray body.
fn accent_color(body: Color) -> Color {
    let body = Hsla::from(body);
    let saturation = if body.saturation > 0.01 { 1.0 } else { 0.0 };
    Color::hsl(body.hue, saturation, 0.8)
}

/// Length of the hull: agile tanks are short.
fn hull_length(genome: &Genome) -> f32 {
    2.0 - 0.1 * genome.level(Stat::Tracks) as f32
}

/// Radius of the turret: small turrets turn fast.
fn turret_radius(genome: &Genome) -> f32 {
    0.56 - 0.035 * genome.level(Stat::Turret) as f32
}

/// Height and width of the tracks: the faster the tank, the bigger they are.
fn track_size(genome: &Genome) -> (f32, f32) {
    let level = genome.level(Stat::Engine) as f32;
    (0.34 + 0.035 * level, 0.24 + 0.03 * level)
}

/// Length of the barrel: the faster the bullet, the longer it is.
fn barrel_length(genome: &Genome) -> f32 {
    0.6 + 0.1 * genome.level(Stat::Muzzle) as f32
}

/// How far in front of the middle of the turret the barrel ends.
fn barrel_tip(genome: &Genome) -> f32 {
    turret_radius(genome) - 0.1 + barrel_length(genome)
}

/// How far behind the middle of the turret the first and the last segment of the drum are.
fn drum_ends(genome: &Genome) -> (f32, f32) {
    let first = -turret_radius(genome) - 0.09;
    (first, first - DRUM_PITCH * (genome.magazine() as f32 - 1.0))
}

/// Where the mines of the rack are on the deck: along the hull and across it, from its
/// middle.
fn rack(genome: &Genome) -> impl Iterator<Item = Vec2> {
    let (mines, rear) = (genome.mines(), -hull_length(genome) / 2.0 + 0.16);
    (0..mines).map(move |slot| {
        let across = (slot as f32 - (mines as f32 - 1.0) / 2.0) * 0.27;
        Vec2::new(rear, across)
    })
}

/// Where the antennas stand on the deck, one per tier of the brain.
fn antennas(genome: &Genome) -> impl Iterator<Item = Vec2> {
    let length = hull_length(genome);
    (0..genome.brain()).map(move |tier| {
        let side = if tier % 2 == 0 { 1.0 } else { -1.0 };
        let back = -length / 2.0 + 0.1 + 0.22 * (tier / 2) as f32;
        Vec2::new(back, side * 0.47)
    })
}

/// How far in front of the middle of the turret each ring around the muzzle is.
fn muzzle_rings(genome: &Genome) -> impl Iterator<Item = f32> {
    let tip = barrel_tip(genome);
    (0..genome.bounces()).map(move |ring| tip - 0.08 - 0.13 * ring as f32)
}

/// Where the vents are on the roof of the turret: along it and across it, from its middle.
fn vents(genome: &Genome) -> impl Iterator<Item = Vec2> {
    let radius = turret_radius(genome);
    (0..genome.level(Stat::Loader)).map(move |vent| {
        let flank = if vent % 2 == 0 { 1.0 } else { -1.0 };
        let along = -0.25 + 0.15 * (vent / 2) as f32;
        Vec2::new(along, flank * radius * 0.7)
    })
}

/// Where the lenses of the sight are on the roof of the turret.
fn lenses(genome: &Genome) -> impl Iterator<Item = Vec2> {
    let (lenses, radius) = (genome.ballistics(), turret_radius(genome));
    (0..lenses).map(move |lens| {
        let across = (lens as f32 - (lenses as f32 - 1.0) / 2.0) * 0.17;
        Vec2::new(radius * 0.45, across)
    })
}

/// A place on a hull or on a turret, at this height.
fn raised(place: Vec2, height: f32) -> Vec3 {
    Vec3::new(place.x, height, place.y)
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
    let dark = theme::DARK_METAL;
    let length = hull_length(genome);
    if genome.level(Stat::Engine) == 0 {
        // Engine: a tank that can't move sits on a fixed base.
        let (radius, height) = (BASE_RADIUS, BASE_HEIGHT);
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
        let (high, wide) = track_size(genome);
        for side in [-1.0, 1.0] {
            let center = Vec3::new(0.0, high / 2.0, side * (TRACK_GAUGE + wide / 2.0));
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
    for slot in rack(genome) {
        let center = raised(slot, deck + 0.04);
        draft.post(center, RACK_MINE, 0.08, theme::LINE, theme::LINE);
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
    for antenna in antennas(genome) {
        let foot = raised(antenna, deck);
        let mast = Vec3::new(LINE, 0.7, LINE);
        draft.solid(foot + Vec3::Y * 0.35, mast, theme::LINE, theme::LINE);
        draft.post(foot + Vec3::Y * 0.71, BEAD, 0.1, Color::WHITE, Color::WHITE);
    }
    draft
}

/// The drawing of the turret of a tank, with what it carries: the barrel with its rings and
/// its tip, the drum, the vents and the sight. Heights are above the turret ring.
pub fn turret_drawing(genome: &Genome, color: Color) -> Draft {
    let mut draft = Draft::default();
    let (side, accent, dark) = (theme::side(color), accent_color(color), theme::DARK_METAL);
    let radius = turret_radius(genome);
    // Magazine: a drum behind the turret, one segment per round. The segments stand on a
    // plate, whose pale top shows around them and between them.
    let rounds = genome.magazine();
    let ((first, last), pitch) = (drum_ends(genome), DRUM_PITCH);
    let segment = SEGMENT;
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
    let (barrel, tip) = (barrel_length(genome), barrel_tip(genome));
    let middle = Vec3::new(tip - barrel / 2.0, 0.18, 0.0);
    draft.solid(middle, Vec3::new(barrel, BARREL, BARREL), dark, dark);
    draft.frame(
        middle + Vec3::Y * (0.08 + LIFT),
        Vec2::new(barrel, BARREL),
        BARREL_LINE,
        theme::LINE,
    );
    // Ricochet: one ring around the muzzle per bounce.
    for ring in muzzle_rings(genome) {
        let center = Vec3::new(ring, 0.18, 0.0);
        draft.solid(center, Vec3::new(RING.x, RING.y, RING.y), color, side);
    }
    // Piercing: a pointed tip on the barrel.
    if genome.piercing() > 0 {
        draft.point(Vec3::new(tip - TIP_SEAT, 0.18, 0.0), TIP.x, TIP.y, accent);
    }
    let roof = ROOF;
    draft.post(Vec3::new(0.0, roof / 2.0, 0.0), radius, roof, color, side);
    draft.ring(
        Vec3::Y * (roof + LIFT),
        (radius - LINE, radius),
        (1, 1.0),
        theme::LINE,
    );
    // Loader: vents on the turret sides, more with level.
    for vent in vents(genome) {
        let center = raised(vent, roof + 0.04);
        draft.solid(center, Vec3::new(VENT.x, 0.08, VENT.y), dark, dark);
    }
    // Ballistics: a sight on the turret, one lens per tier.
    for lens in lenses(genome) {
        let center = raised(lens, roof + 0.06);
        draft.solid(center, Vec3::new(LENS.x, 0.11, LENS.y), accent, accent);
    }
    draft
}

/// A layer of an outline: what lies over something else is a little higher.
fn layer(layer: u8) -> Vec3 {
    Vec3::Y * layer as f32 * LIFT
}

/// The outline of the hull of a tank, with what it carries. It lies flat, and every part
/// that tells a stat is on it: the large ones in dashes, the small ones as marks.
pub fn hull_outline(genome: &Genome, outline: &Outline) -> Draft {
    let mut draft = Draft::default();
    let length = hull_length(genome);
    // Engine: tracks, or the round base of a tank that can't move. The body lies over them.
    if genome.level(Stat::Engine) == 0 {
        let ring = (BASE_RADIUS - LINE, BASE_RADIUS);
        draft.dashed_ring(layer(0), ring, outline.line);
    } else {
        let (_, wide) = track_size(genome);
        for side in [-1.0, 1.0] {
            let center = layer(0) + Vec3::Z * side * (TRACK_GAUGE + wide / 2.0);
            let size = Vec2::new(length + TRACK_OVERHANG, wide);
            draft.dashed_frame(center, size, LINE, outline.line);
        }
    }
    let body = Vec2::new(length, HULL.y);
    draft.plate(layer(1), body, outline.body);
    draft.dashed_frame(layer(2), body, LINE, outline.line);
    // Shield: the white edge around the turret, as wide as on the tank.
    let shield_level = genome.level(Stat::Shield);
    if shield_level > 0 {
        let radius = turret_radius(genome);
        let ring = (radius, radius + shield_edge(shield_level));
        draft.dashed_ring(layer(2), ring, theme::SHIELD);
    }
    // Mines: a small circle per slot of the rack.
    for slot in rack(genome) {
        let ring = (RACK_MINE - LINE, RACK_MINE);
        draft.dashed_ring(raised(slot, 0.0) + layer(3), ring, outline.line);
    }
    // Brain: the bead of each antenna, seen from above.
    for antenna in antennas(genome) {
        draft.disc(raised(antenna, 0.0) + layer(3), BEAD, Color::WHITE);
    }
    draft
}

/// The outline of the turret of a tank, with what it carries. Its parts hide what lies under
/// them, as those of the tank do.
pub fn turret_outline(genome: &Genome, outline: &Outline) -> Draft {
    let mut draft = Draft::default();
    let accent = accent_color(outline.line);
    let radius = turret_radius(genome);
    // Magazine: the drum, with a rung between each two rounds.
    let (first, last) = drum_ends(genome);
    let drum = Vec2::new(
        first - last + SEGMENT.x + 2.0 * LINE,
        SEGMENT.z + 2.0 * LINE,
    );
    let middle = Vec3::X * (first + last) / 2.0;
    draft.plate(middle + layer(0), drum, outline.sheet);
    draft.dashed_frame(middle + layer(1), drum, LINE, outline.line);
    for round in 1..genome.magazine() {
        let rung = Vec3::X * (first - DRUM_PITCH * (round as f32 - 0.5)) + layer(1);
        let reach = Vec3::Z * SEGMENT.z / 2.0;
        draft.dashed(rung - reach, rung + reach, BARREL_LINE, outline.line);
    }
    // Muzzle: the barrel, whose root is under the roof.
    let (barrel, tip) = (barrel_length(genome), barrel_tip(genome));
    let middle = Vec3::X * (tip - barrel / 2.0);
    let size = Vec2::new(barrel, BARREL);
    draft.plate(middle + layer(0), size, outline.sheet);
    draft.dashed_frame(middle + layer(1), size, BARREL_LINE, outline.line);
    // Ricochet: a bar across the muzzle per bounce.
    for ring in muzzle_rings(genome) {
        draft.plate(Vec3::X * ring + layer(2), RING, outline.line);
    }
    // Piercing: the pointed tip.
    if genome.piercing() > 0 {
        let base = Vec3::X * (tip - TIP_SEAT) + layer(2);
        let corners = [
            base - Vec3::Z * TIP.x,
            base + Vec3::Z * TIP.x,
            base + Vec3::X * TIP.y,
        ];
        draft.face(&corners, accent);
    }
    draft.disc(layer(2), radius, outline.body);
    draft.dashed_ring(layer(3), (radius - LINE, radius), outline.line);
    // Loader: a mark per vent.
    for vent in vents(genome) {
        draft.plate(raised(vent, 0.0) + layer(3), VENT, outline.line);
    }
    // Ballistics: a mark per lens of the sight.
    for lens in lenses(genome) {
        draft.plate(raised(lens, 0.0) + layer(3), LENS, accent);
    }
    draft
}

/// A part of a tank that tells one of its stats, for a drawing that points at it.
pub struct Anchor {
    pub stat: Stat,
    /// Whether the part is on the turret, and turns with it. It is on the hull otherwise.
    pub on_turret: bool,
    /// Where the part is: from the middle of the tank on the ground for a part of the hull,
    /// from the middle of the turret ring for a part of the turret.
    pub at: Vec3,
}

/// The parts that tell the stats of a tank, in the order of the stats. A stat that shows as
/// a part the tank lacks isn't there: a tank without a shield has no disk to point at.
pub fn anchors(genome: &Genome) -> Vec<Anchor> {
    let (length, deck, radius) = (hull_length(genome), TURRET_HEIGHT, turret_radius(genome));
    let (tip, drum) = (barrel_tip(genome), drum_ends(genome));
    let has = |stat: Stat| genome.level(stat) > 0;
    let on_hull = |stat, at| Anchor {
        stat,
        on_turret: false,
        at,
    };
    let on_turret = |stat, at| Anchor {
        stat,
        on_turret: true,
        at,
    };
    let mut anchors = Vec::with_capacity(Stat::COUNT);
    // Engine: the top of a track, or the rim of the fixed base.
    anchors.push(on_hull(
        Stat::Engine,
        if has(Stat::Engine) {
            let (high, wide) = track_size(genome);
            Vec3::new(length * 0.3, high, 0.52 + wide / 2.0)
        } else {
            Vec3::new(BASE_RADIUS * 0.6, BASE_HEIGHT, BASE_RADIUS * 0.6)
        },
    ));
    // Tracks: the hull, whose length they set, by a front corner of its deck.
    let corner = Vec3::new(length / 2.0 - 0.08, deck, -HULL.y / 2.0 + 0.1);
    anchors.push(on_hull(Stat::Tracks, corner));
    // Turret: its roof, near the edge.
    let edge = Vec3::new(-radius * 0.45, ROOF, -radius * 0.6);
    anchors.push(on_turret(Stat::Turret, edge));
    let barrel = Vec3::new(tip - barrel_length(genome) / 2.0, 0.26, 0.0);
    anchors.push(on_turret(Stat::Muzzle, barrel));
    let segments = Vec3::new((drum.0 + drum.1) / 2.0, 0.28, 0.0);
    anchors.push(on_turret(Stat::Magazine, segments));
    // Loader: the flank of the turret, where its vents are. A slow loader has none.
    let vents = if has(Stat::Loader) { 0.08 } else { 0.0 };
    let flank = Vec3::new(-0.25, ROOF + vents, radius * 0.7);
    anchors.push(on_turret(Stat::Loader, flank));
    if has(Stat::Ricochet) {
        anchors.push(on_turret(Stat::Ricochet, Vec3::new(tip - 0.08, 0.33, 0.0)));
    }
    if has(Stat::Mines) {
        let rack = Vec3::new(-length / 2.0 + 0.16, deck + 0.08, 0.0);
        anchors.push(on_hull(Stat::Mines, rack));
    }
    if has(Stat::Shield) {
        // The edge of the disk, behind the turret and to a side.
        let reach = radius + shield_edge(genome.level(Stat::Shield)) / 2.0;
        let rim = Vec3::new(-0.6, 0.0, -0.8) * reach + Vec3::Y * (deck + 0.04);
        anchors.push(on_hull(Stat::Shield, rim));
    }
    if has(Stat::Piercing) {
        anchors.push(on_turret(Stat::Piercing, Vec3::new(tip + 0.14, 0.18, 0.0)));
    }
    if has(Stat::Brain) {
        // The bead on top of the first antenna.
        let bead = Vec3::new(-length / 2.0 + 0.1, deck + 0.76, 0.47);
        anchors.push(on_hull(Stat::Brain, bead));
    }
    if has(Stat::Ballistics) {
        let sight = Vec3::new(radius * 0.45, ROOF + 0.12, 0.0);
        anchors.push(on_turret(Stat::Ballistics, sight));
    }
    anchors
}

/// The sheet as it is under a tank: dull until a player enters the sector.
fn sheet_under(tank: &Tank, run: Option<&Run>) -> Color {
    if scene::is_dormant(tank.sector, run) {
        theme::dull(theme::FLOOR)
    } else {
        theme::FLOOR
    }
}

/// How an enemy is drawn: a dormant one isn't there yet.
fn look_of(enemy: Option<&Enemy>, wreck: bool) -> Look {
    match enemy {
        Some(Enemy { awake: false }) => Look::Outline,
        // The wreck of a player's tank is the outline of the tank that stood there.
        None if wreck => Look::Outline,
        _ => Look::Solid,
    }
}

/// The two drawings of a tank, its hull and its turret, for a way of drawing it.
fn drawings(
    tank: &Tank,
    player: Option<&Player>,
    look: Look,
    run: Option<&Run>,
    meshes: &mut Assets<Mesh>,
) -> [(Handle<Mesh>, f32); 2] {
    let genome = &tank.genome;
    let color = match player {
        Some(_) => theme::PLAYER,
        None => enemy_color(genome),
    };
    let (hull, turret, height) = match look {
        Look::Solid => (
            hull_drawing(genome, color),
            turret_drawing(genome, color),
            (0.0, TURRET_HEIGHT),
        ),
        Look::Outline => {
            let outline = Outline::dormant(color, sheet_under(tank, run));
            (
                hull_outline(genome, &outline),
                turret_outline(genome, &outline),
                (OUTLINE_HEIGHT, OUTLINE_HEIGHT + OUTLINE_TURRET),
            )
        }
    };
    [
        (meshes.add(hull.build()), height.0),
        (meshes.add(turret.build()), height.1),
    ]
}

/// Gives a tank its two drawings, as its children.
fn draw(
    commands: &mut Commands,
    entity: Entity,
    motion: &Motion,
    [hull, turret]: [(Handle<Mesh>, f32); 2],
    ink: &Handle<StandardMaterial>,
) {
    commands.entity(entity).with_children(|root| {
        root.spawn((
            Hull,
            Mesh3d(hull.0),
            MeshMaterial3d(ink.clone()),
            Transform {
                translation: Vec3::Y * hull.1,
                rotation: scene::facing(motion.hull),
                ..default()
            },
        ));
        root.spawn((
            Turret,
            Mesh3d(turret.0),
            MeshMaterial3d(ink.clone()),
            Transform {
                translation: Vec3::Y * turret.1,
                rotation: scene::facing(motion.turret),
                ..default()
            },
        ));
    });
}

fn build(
    mut commands: Commands,
    tanks: Query<
        (
            Entity,
            &Tank,
            &Motion,
            Option<&Player>,
            Option<&Enemy>,
            Has<Wreck>,
        ),
        Added<Tank>,
    >,
    run: Option<Res<Run>>,
    shapes: Res<Shapes>,
    paints: Res<Paints>,
    glass: Res<Glass>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, tank, motion, player, enemy, wreck) in &tanks {
        let look = look_of(enemy, wreck);
        commands
            .entity(entity)
            .insert((Transform::default(), Visibility::default(), look))
            .with_children(|root| {
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
        let drawings = drawings(tank, player, look, run.as_deref(), &mut meshes);
        draw(&mut commands, entity, motion, drawings, &paints.ink);
    }
}

/// Draws a tank again when its look changes. An enemy that wakes up is inked in: its outline
/// gives way to the tank itself. A player's tank that is destroyed among several players
/// leaves its outline, and is inked in again once it is repaired.
fn ink_in(
    mut commands: Commands,
    mut tanks: Query<(
        Entity,
        &Tank,
        &Motion,
        Option<&Player>,
        Option<&Enemy>,
        Has<Wreck>,
        &mut Look,
        &Children,
    )>,
    parts: Query<(), Or<(With<Hull>, With<Turret>)>>,
    run: Option<Res<Run>>,
    paints: Res<Paints>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, tank, motion, player, enemy, wreck, mut look, children) in &mut tanks {
        let wanted = look_of(enemy, wreck);
        if *look == wanted {
            continue;
        }
        *look = wanted;
        for child in children.iter().filter(|child| parts.contains(*child)) {
            commands.entity(child).despawn();
        }
        let drawings = drawings(tank, player, wanted, run.as_deref(), &mut meshes);
        draw(&mut commands, entity, motion, drawings, &paints.ink);
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

/// Shows the bubble of a raised shield.
fn shield(
    tanks: Query<(&Shield, &Children), With<Tank>>,
    mut bubbles: Query<&mut Visibility, With<Bubble>>,
) {
    for (shield, children) in &tanks {
        for child in children.iter() {
            if let Ok(mut visibility) = bubbles.get_mut(child) {
                *visibility = if shield.is_active() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
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
        // The leader of a late group has the largest budget.
        let enemies = Class::ALL.into_iter().flat_map(|class| {
            (0..=150).map(move |budget| {
                let mut genome = Genome::empty(budget);
                genome.class = class;
                enemy_color(&genome)
            })
        });
        for body in enemies.chain([theme::PLAYER]) {
            let (body, accent) = (Hsla::from(body), Hsla::from(accent_color(body)));
            assert_eq!(accent.hue, body.hue);
            assert!(accent.lightness > body.lightness + 0.05, "{body:?}");
            // A gray tank has a gray sight, not a red one.
            assert_eq!(accent.saturation > 0.0, body.saturation > 0.0);
        }
    }

    #[test]
    fn every_class_has_a_hue_of_its_own_and_none_has_the_players() {
        let player = Hsla::from(theme::PLAYER).hue;
        let hues: Vec<f32> = Class::ALL.into_iter().filter_map(class_hue).collect();
        assert_eq!(hues.len(), Class::ALL.len() - 1);
        let apart = |a: f32, b: f32| ((a - b).abs()).min(360.0 - (a - b).abs());
        for (index, &hue) in hues.iter().enumerate() {
            assert!(
                apart(hue, player) > 35.0,
                "{hue} is close to the players' blue"
            );
            for &other in &hues[index + 1..] {
                assert!(apart(hue, other) > 20.0, "{hue} and {other}");
            }
        }
        // A tank without a class is gray, however strong, and the weakest tank of a class
        // still shows its hue.
        let mut weak = Genome::empty(3);
        assert_eq!(Hsla::from(enemy_color(&weak)).saturation, 0.0);
        weak.class = Class::Sapper;
        assert!(Hsla::from(enemy_color(&weak)).saturation > 0.35);
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
    fn every_part_that_tells_a_stat_is_on_the_drawing() {
        // A tank with everything, one with nothing, and the player's.
        let mut full = Genome::empty(128);
        for stat in Stat::HARDWARE
            .into_iter()
            .chain([Stat::Brain, Stat::Ballistics])
        {
            full.set(stat, stat.max_level());
        }
        for genome in [full, Genome::empty(10), Genome::PLAYER] {
            let hull = hull_drawing(&genome, theme::PLAYER).bounds();
            let turret = turret_drawing(&genome, theme::PLAYER).bounds();
            let anchors = anchors(&genome);
            for anchor in &anchors {
                let (lowest, highest) = if anchor.on_turret { turret } else { hull };
                let margin = Vec3::splat(0.02);
                assert!(
                    anchor.at.cmpge(lowest - margin).all()
                        && anchor.at.cmple(highest + margin).all(),
                    "{:?} points off the drawing",
                    anchor.stat
                );
            }
            // A part is there if the tank has it, and the six that every tank has always are.
            let shown = |stat: Stat| anchors.iter().any(|anchor| anchor.stat == stat);
            for stat in [Stat::Ricochet, Stat::Mines, Stat::Shield, Stat::Piercing] {
                assert_eq!(shown(stat), genome.level(stat) > 0, "{stat:?}");
            }
            assert!(shown(Stat::Engine) && shown(Stat::Loader) && shown(Stat::Magazine));
        }
    }

    /// A tank with everything.
    fn full() -> Genome {
        let mut genome = Genome::empty(128);
        for stat in Stat::HARDWARE
            .into_iter()
            .chain([Stat::Brain, Stat::Ballistics])
        {
            genome.set(stat, stat.max_level());
        }
        genome
    }

    #[test]
    fn an_outline_lies_flat_where_its_tank_stands() {
        let outline = Outline::dormant(theme::PLAYER, theme::FLOOR);
        let enemies = (0..5).flat_map(|seed| tanku_core::enemy::group(seed, default(), 20, 6));
        for genome in enemies.chain([full(), Genome::empty(10), Genome::PLAYER]) {
            let pairs = [
                (
                    hull_outline(&genome, &outline),
                    hull_drawing(&genome, theme::PLAYER),
                ),
                (
                    turret_outline(&genome, &outline),
                    turret_drawing(&genome, theme::PLAYER),
                ),
            ];
            for (flat, solid) in pairs {
                let ((low, high), (lowest, highest)) = (flat.bounds(), solid.bounds());
                assert!(low.y >= 0.0 && high.y < OUTLINE_TURRET, "{high}");
                // The tank covers its outline, give or take the line around its foot and
                // the corners that a round part is drawn with.
                let plan = |corner: Vec3| Vec2::new(corner.x, corner.z);
                let margin = 1.5 * LINE;
                assert!(plan(low).distance(plan(lowest)) < margin, "{low} {lowest}");
                assert!(
                    plan(high).distance(plan(highest)) < margin,
                    "{high} {highest}"
                );
            }
        }
    }

    #[test]
    fn every_level_of_every_stat_shows_on_the_outline() {
        let outline = Outline::dormant(theme::PLAYER, theme::FLOOR);
        // What an outline is made of: how much there is of each color, and how far it reaches.
        let looks = |genome: &Genome| {
            let drafts = [
                hull_outline(genome, &outline),
                turret_outline(genome, &outline),
            ];
            let colors = [
                outline.line,
                outline.body,
                outline.sheet,
                theme::SHIELD,
                accent_color(outline.line),
            ];
            let mut looks = Vec::new();
            for draft in &drafts {
                looks.extend(colors.map(|color| draft.area(color)));
                let (lowest, highest) = draft.bounds();
                looks.extend(lowest.to_array());
                looks.extend(highest.to_array());
            }
            looks
        };
        let differ = |a: &[f32], b: &[f32]| a.iter().zip(b).any(|(a, b)| (a - b).abs() > 1e-4);
        for stat in Stat::HARDWARE
            .into_iter()
            .chain([Stat::Brain, Stat::Ballistics])
        {
            for level in 0..stat.max_level() {
                // A tank that moves, so that its tracks show their own stat.
                let mut genome = Genome::empty(60);
                genome.set(Stat::Engine, 2);
                genome.set(stat, level);
                let lower = looks(&genome);
                genome.set(stat, level + 1);
                assert!(differ(&lower, &looks(&genome)), "{stat:?} {level}");
            }
        }
    }

    #[test]
    fn the_small_parts_of_an_outline_are_as_many_as_on_the_tank() {
        let outline = Outline::dormant(theme::PLAYER, theme::FLOOR);
        let genome = full();
        let (hull, turret) = (
            hull_outline(&genome, &outline),
            turret_outline(&genome, &outline),
        );
        // The beads of the antennas and the edge of the shield are white, as on the tank.
        let beads = genome.brain() as f32 * std::f32::consts::PI * BEAD * BEAD;
        assert!(hull.area(theme::SHIELD) > beads);
        // The lenses of the sight and the pointed tip are in the pale tint of the tank.
        let lenses = genome.ballistics() as f32 * LENS.x * LENS.y;
        let tip = TIP.x * TIP.y;
        assert!(close(turret.area(accent_color(outline.line)), lenses + tip));
        // A tank without them shows none.
        let bare = Genome::empty(10);
        assert_eq!(hull_outline(&bare, &outline).area(theme::SHIELD), 0.0);
        let turret = turret_outline(&bare, &outline);
        assert_eq!(turret.area(accent_color(outline.line)), 0.0);
    }

    /// A world that draws tanks as the game does.
    fn drawing_world() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .add_systems(PreStartup, scene::setup)
            .add_systems(Startup, setup)
            .add_systems(Update, (build, ink_in, pose).chain());
        app
    }

    /// The heights of the hull and of the turret of a tank, and how many of each it has.
    fn parts(app: &mut App, tank: Entity) -> ((usize, usize), (f32, f32)) {
        let world = app.world_mut();
        let children: Vec<Entity> = world.get::<Children>(tank).unwrap().iter().collect();
        let height =
            |world: &World, part: Entity| world.get::<Transform>(part).unwrap().translation.y;
        let hulls: Vec<f32> = children
            .iter()
            .filter(|part| world.get::<Hull>(**part).is_some())
            .map(|part| height(world, *part))
            .collect();
        let turrets: Vec<f32> = children
            .iter()
            .filter(|part| world.get::<Turret>(**part).is_some())
            .map(|part| height(world, *part))
            .collect();
        ((hulls.len(), turrets.len()), (hulls[0], turrets[0]))
    }

    #[test]
    fn a_dormant_enemy_is_an_outline_until_it_wakes_up() {
        let mut app = drawing_world();
        let genome = full();
        let tank = Tank {
            genome,
            team: tanku_core::sim::Team::Enemies,
            sector: 3,
        };
        let enemy = app.world_mut().spawn((tank, Enemy { awake: false })).id();
        app.update();
        assert_eq!(app.world().get::<Look>(enemy), Some(&Look::Outline));
        let flat = (OUTLINE_HEIGHT, OUTLINE_HEIGHT + OUTLINE_TURRET);
        assert_eq!(parts(&mut app, enemy), ((1, 1), flat));
        // Nothing changes while it sleeps.
        app.update();
        assert_eq!(parts(&mut app, enemy), ((1, 1), flat));

        app.world_mut().get_mut::<Enemy>(enemy).unwrap().awake = true;
        app.update();
        assert_eq!(app.world().get::<Look>(enemy), Some(&Look::Solid));
        assert_eq!(parts(&mut app, enemy), ((1, 1), (0.0, TURRET_HEIGHT)));
    }

    #[test]
    fn a_tank_that_is_there_is_drawn_as_it_stands() {
        let mut app = drawing_world();
        let tank = |team| Tank {
            genome: Genome::PLAYER,
            team,
            sector: 0,
        };
        let player = (tank(tanku_core::sim::Team::Players), Player { seat: 0 });
        let player = app.world_mut().spawn(player).id();
        let awake = (tank(tanku_core::sim::Team::Enemies), Enemy { awake: true });
        let awake = app.world_mut().spawn(awake).id();
        app.update();
        for entity in [player, awake] {
            assert_eq!(app.world().get::<Look>(entity), Some(&Look::Solid));
            assert_eq!(parts(&mut app, entity), ((1, 1), (0.0, TURRET_HEIGHT)));
        }
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
