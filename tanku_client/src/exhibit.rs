//! A tank shown large on the sheet, behind a menu: the player's own behind the main menu, and
//! the tank that ended a run behind the report of that run.
//!
//! It stands far from any run, so that nothing of the game shows around it, and the view
//! goes there for as long as its screen lasts. Where a screen draws it as an assembly, a
//! balloon names each part that tells a stat, on a leader that ends in a dot on the part
//! (ISO 6433), with the number of the part in the parts list of the screen.

use std::f32::consts::TAU;

use bevy::prelude::*;
use tanku_core::{
    rules::{SHIELD_RADIUS, TANK_RADIUS},
    stats::Genome,
};

use crate::{
    Screen,
    camera::{self, OnScreen, View},
    draft::{Draft, LIFT, LINE},
    models,
    scene::{self, Paints},
    theme,
};

/// The balloon of a part: how wide it is, the width of its line and of its leader, and the
/// dot at the end of the leader, and those of the part that is lit.
const CALLOUT: f32 = 48.0;
const CALLOUT_LINE: f32 = 2.0;
const DOT: f32 = 8.0;
const LIT_LINE: f32 = 3.0;
const LIT_DOT: f32 = 10.0;
/// How far from the middle of the tank the balloons stand at most, in meters: where the room
/// of the screen is short, they stand on an ellipse that it holds.
const CIRCLE: f32 = 2.2;
/// The least room between the middles of two balloons along their ellipse, and between a
/// balloon and the edge of the room.
const LEAST_GAP: f32 = CALLOUT + 8.0;
const ROOM_EDGE: f32 = CALLOUT / 2.0 + 4.0;
/// How much longer than the balloons need the ellipse is at least, where the room allows it.
const CROWDED: f32 = 1.25;

/// What points at the parts of the tank on show: where the tank stands, where each part is in
/// the world and the number of its balloon, the room of the screen that the balloons keep
/// inside, in the pixels of the interface, and the part whose balloon is lit, if one is, by
/// its place in the list.
#[derive(Resource)]
pub struct Callouts {
    stand: Stand,
    parts: Vec<Vec3>,
    numbers: Vec<usize>,
    room: Rect,
    lit: Option<usize>,
}

impl Callouts {
    /// Lights the balloon of this number, or none.
    pub fn light(&mut self, number: Option<usize>) {
        let lit = number.and_then(|number| self.numbers.iter().position(|own| *own == number));
        if self.lit != lit {
            self.lit = lit;
        }
    }
}

/// A piece of the balloon of a part: the line to the part, the dot on it and the ring with its
/// number.
#[derive(Component, Clone, Copy)]
enum Callout {
    Leader(usize),
    Dot(usize),
    Ring(usize),
}

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
    app.add_systems(
        Update,
        (
            sway,
            point
                .after(camera::follow)
                .run_if(resource_exists::<Callouts>),
        ),
    );
}

/// Names the parts of the tank on show, until `screen` is left: a balloon on each one that the
/// list of anchors gives, with the number that goes with it, inside the room of the screen.
pub fn call_out(
    commands: &mut Commands,
    (stand, room): (Stand, Rect),
    anchors: &[(usize, models::Anchor)],
    screen: Screen,
) {
    let parts = anchors
        .iter()
        .map(|(_, anchor)| stand.point(anchor.on_turret, anchor.at))
        .collect();
    let numbers: Vec<usize> = anchors.iter().map(|(number, _)| *number).collect();
    commands.insert_resource(Callouts {
        stand,
        parts,
        numbers: numbers.clone(),
        room,
        lit: None,
    });
    let piece = |piece: Callout, node: Node| {
        (
            piece,
            Node {
                position_type: PositionType::Absolute,
                ..node
            },
            UiTransform::IDENTITY,
            Visibility::Hidden,
        )
    };
    commands
        .spawn((
            DespawnOnExit(screen),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|sheet| {
            // The leaders lie under the dots, and the dots under the rings.
            for index in 0..anchors.len() {
                let line = Node {
                    height: px(CALLOUT_LINE),
                    ..default()
                };
                sheet.spawn((
                    piece(Callout::Leader(index), line),
                    BackgroundColor(theme::LINE),
                ));
            }
            for index in 0..anchors.len() {
                let dot = Node {
                    width: px(DOT),
                    height: px(DOT),
                    border_radius: BorderRadius::MAX,
                    ..default()
                };
                sheet.spawn((
                    piece(Callout::Dot(index), dot),
                    BackgroundColor(theme::LINE),
                ));
            }
            for (index, number) in numbers.into_iter().enumerate() {
                let ring = Node {
                    width: px(CALLOUT),
                    height: px(CALLOUT),
                    border: UiRect::all(px(CALLOUT_LINE)),
                    border_radius: BorderRadius::MAX,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                };
                sheet.spawn((
                    piece(Callout::Ring(index), ring),
                    BorderColor::all(theme::LINE),
                    BackgroundColor(theme::VOID),
                    children![theme::figure(number.to_string(), theme::VALUE, theme::TEXT)],
                ));
            }
        });
}

/// Moves places on a loop this long apart until no two are closer than `least`, without
/// changing their order around it.
fn spread(places: &[f32], least: f32, around: f32) -> Vec<f32> {
    let count = places.len();
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by(|&a, &b| places[a].total_cmp(&places[b]));
    let mut sorted: Vec<f32> = order.iter().map(|&index| places[index]).collect();
    // On a loop too short for them all at that distance, they stand evenly apart in their
    // order, as near as that lets them to where they were.
    let crowded = count as f32 * least >= around;
    if crowded {
        let step = around / count.max(1) as f32;
        let ranked = sorted.iter().enumerate();
        let shift = ranked
            .map(|(rank, place)| place - rank as f32 * step)
            .sum::<f32>();
        let shift = shift / count.max(1) as f32;
        sorted = (0..count).map(|rank| shift + rank as f32 * step).collect();
    }
    for _ in 0..if crowded { 0 } else { 64 } {
        let mut moved = false;
        for here in 0..count {
            let next = (here + 1) % count;
            if next == here {
                break;
            }
            // The last one is followed by the first, once around further.
            let turn = if next == 0 { around } else { 0.0 };
            let gap = sorted[next] + turn - sorted[here];
            if gap < least - 1e-4 {
                let push = (least - gap) / 2.0;
                sorted[here] -= push;
                sorted[next] += push;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    let mut spread = vec![0.0; count];
    for (rank, &index) in order.iter().enumerate() {
        spread[index] = sorted[rank];
    }
    spread
}

/// An ellipse that the balloons stand on, as points along it with how far along it each one
/// is, from its right end and down.
struct Ellipse {
    points: Vec<Vec2>,
    along: Vec<f32>,
}

impl Ellipse {
    /// An ellipse around a point, with these radii.
    fn new(center: Vec2, radii: Vec2) -> Self {
        const SAMPLES: usize = 360;
        let points: Vec<Vec2> = (0..=SAMPLES)
            .map(|step| center + radii * Vec2::from_angle(step as f32 / SAMPLES as f32 * TAU))
            .collect();
        let mut along = Vec::with_capacity(points.len());
        let mut length = 0.0;
        for (index, point) in points.iter().enumerate() {
            if index > 0 {
                length += point.distance(points[index - 1]);
            }
            along.push(length);
        }
        Self { points, along }
    }

    /// How long it is all around.
    fn around(&self) -> f32 {
        self.along[self.along.len() - 1]
    }

    /// How far along it is the point in the way of an angle from its middle.
    fn at_angle(&self, angle: f32, radii: Vec2) -> f32 {
        // The angle of the point on the circle that the ellipse is squeezed from.
        let squeezed = Vec2::from_angle(angle) / radii.max(Vec2::splat(1e-3));
        let turn = squeezed.to_angle().rem_euclid(TAU);
        let index = turn / TAU * (self.points.len() - 1) as f32;
        let (low, high) = (
            index.floor() as usize,
            (index.ceil() as usize).min(self.points.len() - 1),
        );
        let share = index - index.floor();
        self.along[low] + (self.along[high] - self.along[low]) * share
    }

    /// The point that is this far along it.
    fn point(&self, along: f32) -> Vec2 {
        let along = along.rem_euclid(self.around());
        let next = self.along.partition_point(|reach| *reach < along).max(1);
        let (from, to) = (self.along[next - 1], self.along[next]);
        let share = if to > from {
            (along - from) / (to - from)
        } else {
            0.0
        };
        self.points[next - 1].lerp(self.points[next], share)
    }
}

/// Where the pieces of one balloon go on the screen: the ends of its leader, from the ring
/// to the part.
struct Pointer {
    ring: Vec2,
    part: Vec2,
}

/// Lays out the balloons around the tank: each one stands on an ellipse around it, on the side
/// of its part, inside the room of the screen, and no two stand on each other. The ellipse is
/// a circle where the room is large enough.
fn pointers(callouts: &Callouts, screen: &OnScreen) -> Option<Vec<Pointer>> {
    let middle = callouts.stand.point(false, Vec3::Y * models::TURRET_HEIGHT);
    let center = screen.point(middle)?;
    let meter = screen.point(middle + Vec3::X)?.distance(center);
    // The ellipse keeps inside the room around the middle of the tank, wherever in the room
    // that is.
    let room = callouts.room;
    let reach = (center - room.min).min(room.max - center) - ROOM_EDGE;
    let reach = reach.max(Vec2::splat(CALLOUT));
    let mut radii = Vec2::splat(meter * CIRCLE).min(reach);
    let mut parts = Vec::with_capacity(callouts.parts.len());
    for part in &callouts.parts {
        parts.push(screen.point(*part)?);
    }
    let mut ellipse = Ellipse::new(center, radii);
    // A tank of many parts drawn small gets a wider ellipse, as far as the room holds one, so
    // that its balloons have room to keep apart and some to spare.
    let wanted = callouts.parts.len() as f32 * LEAST_GAP * CROWDED;
    for _ in 0..8 {
        if ellipse.around() >= wanted || radii == reach {
            break;
        }
        radii = (radii * wanted / ellipse.around()).min(reach);
        ellipse = Ellipse::new(center, radii);
    }
    let places: Vec<f32> = parts
        .iter()
        .map(|part| ellipse.at_angle((*part - center).to_angle(), radii))
        .collect();
    let places = spread(&places, LEAST_GAP, ellipse.around());
    Some(
        parts
            .iter()
            .zip(places)
            .map(|(&part, along)| Pointer {
                ring: ellipse.point(along),
                part,
            })
            .collect(),
    )
}

/// Keeps the balloons on the parts they point at, and lights the one of the part that is lit.
fn point(
    screen: OnScreen,
    callouts: Res<Callouts>,
    mut pieces: Query<(
        &Callout,
        &mut Node,
        &mut UiTransform,
        &mut Visibility,
        &mut BackgroundColor,
        Option<&mut BorderColor>,
        Option<&Children>,
    )>,
    mut figures: Query<&mut TextColor>,
) {
    let pointers = pointers(&callouts, &screen);
    for (piece, mut node, mut transform, mut visibility, mut fill, border, children) in &mut pieces
    {
        let index = match *piece {
            Callout::Leader(index) | Callout::Dot(index) | Callout::Ring(index) => index,
        };
        let pointer = pointers.as_ref().and_then(|pointers| pointers.get(index));
        let wanted = if pointer.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        visibility.set_if_neq(wanted);
        let Some(pointer) = pointer else {
            continue;
        };
        let lit = callouts.lit == Some(index);
        let ink = if lit { theme::ACCENT } else { theme::LINE };
        let corner = match *piece {
            Callout::Ring(_) => {
                fill.set_if_neq(BackgroundColor(if lit {
                    theme::ACCENT
                } else {
                    theme::VOID
                }));
                if let Some(mut border) = border {
                    border.set_if_neq(BorderColor::all(ink));
                }
                let figure = if lit {
                    theme::TEXT_ON_ACCENT
                } else {
                    theme::TEXT
                };
                for child in children.into_iter().flatten() {
                    if let Ok(mut color) = figures.get_mut(*child) {
                        color.set_if_neq(TextColor(figure));
                    }
                }
                pointer.ring - Vec2::splat(CALLOUT / 2.0)
            }
            Callout::Dot(_) => {
                fill.set_if_neq(BackgroundColor(ink));
                let size = if lit { LIT_DOT } else { DOT };
                if node.width != px(size) {
                    (node.width, node.height) = (px(size), px(size));
                }
                pointer.part - Vec2::splat(size / 2.0)
            }
            Callout::Leader(_) => {
                fill.set_if_neq(BackgroundColor(ink));
                // A line from the edge of the ring to the part, turned around its middle to lie
                // along that way.
                let along = pointer.part - pointer.ring;
                let start = pointer.ring + along.normalize_or_zero() * CALLOUT / 2.0;
                let along = pointer.part - start;
                let width = if lit { LIT_LINE } else { CALLOUT_LINE };
                let length = px(along.length().round());
                if node.width != length || node.height != px(width) {
                    (node.width, node.height) = (length, px(width));
                }
                let turn = Rot2::radians(along.to_angle());
                if transform.rotation != turn {
                    transform.rotation = turn;
                }
                (start + pointer.part) / 2.0 - Vec2::new(along.length(), width) / 2.0
            }
        };
        let held = Val2::px(corner.x.round(), corner.y.round());
        if transform.translation != held {
            transform.translation = held;
        }
    }
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
    fn balloons_make_room_for_each_other() {
        let apart = |angles: &[f32]| {
            let mut sorted = angles.to_vec();
            sorted.sort_by(f32::total_cmp);
            let around = sorted[0] + TAU - sorted[sorted.len() - 1];
            let between = sorted.windows(2).map(|pair| pair[1] - pair[0]);
            between.fold(around, f32::min)
        };
        let least = 0.42;
        // Parts that are far apart keep their balloons where they are.
        let far = [0.0, 1.5, 3.0, -1.5];
        assert_eq!(spread(&far, least, TAU), far);
        // Parts on the same side push their balloons apart, in the same order.
        let close = [0.5, 0.52, 0.48, -2.0, 0.51];
        let spread_out = spread(&close, least, TAU);
        assert!(apart(&spread_out) > least - 1e-3);
        assert!(spread_out[2] < spread_out[0] && spread_out[0] < spread_out[4]);
        assert!(spread_out[4] < spread_out[1]);
        assert_eq!(spread_out[3], -2.0);
        // A tank has twelve parts at most: on a loop that has room for them, their balloons
        // keep apart.
        let crowded: Vec<f32> = (0..12).map(|part| 0.01 * part as f32).collect();
        assert!(apart(&spread(&crowded, least, TAU)) > least - 1e-2);
        // On a loop too short for them, they stand evenly apart, in their order.
        let evenly = spread(&crowded, TAU / 10.0, TAU);
        assert!((apart(&evenly) - TAU / 12.0).abs() < 1e-3);
        assert!(evenly.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn balloons_stand_on_an_ellipse_that_the_room_holds() {
        let (center, radii) = (Vec2::new(500.0, 300.0), Vec2::new(300.0, 80.0));
        let ellipse = Ellipse::new(center, radii);
        // About as long around as Ramanujan has it.
        let (a, b) = (radii.x, radii.y);
        let h = ((a - b) / (a + b)).powi(2);
        let ramanujan =
            std::f32::consts::PI * (a + b) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()));
        assert!((ellipse.around() - ramanujan).abs() < 1.0);
        // The points of the ways to the right, down, left and up.
        for (angle, point) in [
            (0.0, Vec2::new(800.0, 300.0)),
            (TAU / 4.0, Vec2::new(500.0, 380.0)),
            (TAU / 2.0, Vec2::new(200.0, 300.0)),
            (TAU * 0.75, Vec2::new(500.0, 220.0)),
        ] {
            let along = ellipse.at_angle(angle, radii);
            assert!(ellipse.point(along).distance(point) < 1.0, "{angle}");
        }
        // Fifteen balloons fit on an ellipse as flat as a short room leaves.
        assert!(ellipse.around() > 15.0 * LEAST_GAP);
    }

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
