//! What a draftsman writes around a plan, in the void outside the corridor: the length of the
//! walls, the angle of each wall that slopes, and the hatching of the ground behind the walls.
//! It is drawn in the fine line of the drawing, in the colors of the margin of the sheet, and
//! never inside the corridor, where it would be in the way of a fight.
//!
//! The dimensions follow ISO 129: an extension line leaves a gap at the outline and goes a
//! little past the dimension line, which ends in arrowheads, and the value stands above the
//! line, near its middle, so that it reads from the bottom or from the right of the screen,
//! whichever way the view is turned. An angle is dimensioned with an arc. A dimension that
//! would cross the corridor or another dimension is left out, and so is one that the others
//! already tell: each length is given once, on the right side of the corridor before its left.
//! Where a corner of a wall hides under another wall, as where the corridor turns into the
//! next sector, the dimension ends on the outer face of that wall and gives what it spans.
//!
//! A sector gets its margin when it comes, and keeps it until it goes: what the margin keeps
//! clear of is the sector's neighbors as the seed plans them, whether they are loaded or not,
//! so nothing in it comes or goes as the players move on. It is all drawn again only when the
//! view turns, which turns its lettering.

use std::{collections::HashMap, f32::consts::FRAC_PI_2};

use bevy::prelude::*;
use tanku_core::{
    map::{Corridor, SectorKind, SectorPlan},
    sim::{Run, Sector},
};

use crate::{
    camera::View,
    draft::{Draft, LIFT},
    lettering::{Lettering, Setting},
    scene::{self, Ground, Paints, WALL_THICKNESS},
    theme,
};

/// Whether the margin is drawn. A script takes it away to see the void as it was.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Margin(pub bool);

impl Default for Margin {
    fn default() -> Self {
        Self(true)
    }
}

/// The fine line of the drawing, in meters.
const NARROW: f32 = 0.03;
/// How far the dimension line of a wall is from the outside of the wall, and the gap that an
/// extension line leaves at the wall and how far it goes past the dimension line.
const OFFSET: f32 = 1.75;
const EXTENSION: (f32, f32) = (0.2, 0.3);
/// The length and the width of an arrowhead: an included angle of about 19 degrees.
const ARROW: Vec2 = Vec2::new(0.36, 0.12);
/// The height of the capitals of a value, and its gap to the dimension line.
const VALUE_HEIGHT: f32 = 0.5;
const VALUE_GAP: f32 = 0.15;
/// The shortest wall whose length is given.
const SHORTEST: f32 = 2.0;
/// The radius of the arc of an angle: shorter on a short wall, within these bounds.
const ARC: (f32, f32) = (1.4, 2.6);
/// The strokes along the outside of a wall: how far apart, and how long.
const HATCH: (f32, f32) = (0.45, 0.42);
/// How far a dimension keeps from the walls and from another dimension.
const CLEARANCE: f32 = 0.15;
/// The farthest that a dimension goes out past a step to be in line with the next one: past a
/// wall that stands out farther, the dimension would be far from the wall that it measures.
const CHAIN_REACH: f32 = 6.0;
/// What is kept clear around the tag at the end of a barrier: its balloon stands 1.1 m past
/// the end, on the line of the barrier.
const TAG_REACH: f32 = 1.1;
const TAG_ROOM: f32 = 0.75;

/// The drawing of the margin of a sector.
#[derive(Component)]
struct Annotation;

/// What the margin of a sector drew, kept on the sector while it is loaded, for the margins
/// that come after it: the room its drawing takes, its extension lines, which the dimensions
/// of a chain share, and its walls, with whether each slopes and whether its length is given.
#[derive(Component, Clone, Default, Debug, PartialEq)]
struct Margined {
    taken: Vec<([Vec2; 4], Option<(Vec2, Vec2)>)>,
    extensions: Vec<(Vec2, Vec2)>,
    told: Vec<((IVec2, IVec2), bool, bool)>,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Margin>()
        .init_resource::<Lettering>()
        .add_systems(
            Update,
            // After the sectors are dressed and woken: what wakes is told so by a command, which
            // must find the margin that this draws again still there.
            annotate.after(scene::Drawing),
        );
}

/// Draws the margin of a sector that comes, and the margin of every sector again when the view
/// turns or when the margin is taken away or given back.
#[expect(clippy::too_many_arguments, reason = "a system")]
fn annotate(
    mut commands: Commands,
    sectors: Query<(Entity, &Sector, Option<&Margined>)>,
    drawn: Query<Entity, With<Annotation>>,
    margin: Res<Margin>,
    view: Res<View>,
    (lettering, paints): (Res<Lettering>, Res<Paints>),
    run: Option<Res<Run>>,
    mut meshes: ResMut<Assets<Mesh>>,
    (mut last, mut planned): (Local<Option<(Margin, i32)>>, Local<Option<(u64, Corridor)>>),
) {
    let now = (*margin, view.turns);
    let anew = *last != Some(now);
    *last = Some(now);
    if anew {
        for entity in &drawn {
            commands.entity(entity).despawn();
        }
    }
    if !margin.0 {
        return;
    }
    let mut loaded: Vec<(Entity, &Sector, Option<&Margined>)> = sectors.iter().collect();
    loaded.sort_by_key(|(_, sector, _)| sector.layout.plan.index);
    if !anew && loaded.iter().all(|(.., margined)| margined.is_some()) {
        return;
    }
    // What the margins that stay drew, which those that come keep clear of.
    let mut kept: Vec<Margined> = Vec::new();
    if !anew {
        kept.extend(loaded.iter().filter_map(|(.., margined)| margined.cloned()));
    }
    // A corridor's sectors have neighbors that the seed plans. The room of arena mode has none.
    let seed = run.as_ref().filter(|run| !run.arena).map(|run| run.seed);
    let paper = Paper {
        lettering: &lettering,
        turns: view.turns,
        ink: Ink::new(),
    };
    for &(entity, sector, margined) in &loaded {
        if !anew && margined.is_some() {
            continue;
        }
        let plan = &sector.layout.plan;
        let mut around: Vec<SectorPlan> = loaded
            .iter()
            .map(|(_, other, _)| other.layout.plan.clone())
            .filter(|other| other.index != plan.index)
            .collect();
        if let Some(seed) = seed {
            let known = planned.take().filter(|(known, _)| *known == seed);
            let (seed, mut corridor) = known.unwrap_or_else(|| (seed, Corridor::new(seed)));
            let neighbors = [plan.index.checked_sub(1), plan.index.checked_add(1)];
            for neighbor in neighbors.into_iter().flatten() {
                if around.iter().all(|other| other.index != neighbor) {
                    around.push(corridor.plan(neighbor).clone());
                }
            }
            *planned = Some((seed, corridor));
        }
        let (draft, margined) = margin_of(plan, &around, &kept, &paper);
        kept.push(margined.clone());
        commands.entity(entity).insert(margined);
        if draft.is_empty() {
            continue;
        }
        let dormant = scene::is_dormant(plan.index, run.as_deref());
        let material = if dormant {
            paints.ink_dormant.clone()
        } else {
            paints.ink.clone()
        };
        let mesh = meshes.add(draft.build());
        commands.entity(entity).with_children(|parent| {
            parent.spawn((
                Annotation,
                Ground::ink(plan.index),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::default(),
            ));
        });
    }
}

/// The margin of a sector among the sectors around it, which it keeps clear of, after the
/// margins that are drawn already: its drawing, and what it leaves to those that come after.
fn margin_of(
    plan: &SectorPlan,
    around: &[SectorPlan],
    drawn: &[Margined],
    paper: &Paper,
) -> (Draft, Margined) {
    let mut plans: Vec<&SectorPlan> = around.iter().collect();
    plans.push(plan);
    let mut room = Room::new(&plans);
    let mut known = Known::default();
    for other in &plans {
        known.outline(&other.outline);
    }
    for margined in drawn {
        room.taken.extend(margined.taken.iter().copied());
        room.extensions.extend(margined.extensions.iter().copied());
        for &(wall, angled, told) in &margined.told {
            known.learn(wall, angled, told);
        }
    }
    let (taken, extensions) = (room.taken.len(), room.extensions.len());
    let mut draft = Draft::default();
    // The walls of the sector first, then those of the sectors around: where a wall meets a
    // barrier or an entry, the wall of the next sector goes on from its corner, which turns as
    // that wall does.
    let own = plan.walls.len();
    let mut walls: Vec<(Vec2, Vec2)> = walls(plan);
    walls.extend(around.iter().flat_map(self::walls));
    // The outline runs up the left side of the corridor and down its right side: backward,
    // the right side comes first, and of two walls whose lengths tell the same thing, the one
    // on the right keeps its dimension.
    let order: Vec<usize> = (0..own).rev().collect();
    for &wall in &walls[..own] {
        hatch(&mut draft, wall, &room, paper.ink);
    }
    let mut angled = vec![false; own];
    for &index in &order {
        angled[index] = angle(&mut draft, (&walls, index), &mut room, paper);
    }
    let mut told = Vec::new();
    for &index in &order {
        // In arena mode, the tanks in reserve wait south of the room.
        let reserve = plan.kind == SectorKind::Room && outward(walls[index]).y < -0.5;
        let wall = plan.walls[index];
        let drawn = if reserve || known.redundant(wall, angled[index]) {
            false
        } else {
            length(&mut draft, (&walls, index), &mut room, paper)
        };
        known.learn(wall, angled[index], drawn);
        told.push((wall, angled[index], drawn));
    }
    let margined = Margined {
        taken: room.taken[taken..].to_vec(),
        extensions: room.extensions[extensions..].to_vec(),
        told,
    };
    (draft, margined)
}

/// The inks of the margin: the lines, and the lettering, which is a little brighter.
#[derive(Clone, Copy)]
struct Ink {
    line: Color,
    letter: Color,
    hatch: Color,
}

impl Ink {
    fn new() -> Self {
        Self {
            line: theme::RULE,
            letter: theme::TEXT_DIM,
            hatch: theme::over(theme::VOID, theme::RULE, 0.7),
        }
    }
}

/// What the lettering of the margin needs: the characters, which way the view is turned, and
/// the inks.
struct Paper<'a> {
    lettering: &'a Lettering,
    turns: i32,
    ink: Ink,
}

impl Paper<'_> {
    /// The direction of a text along `direction` that reads from the bottom or from the right
    /// of the screen: a text that would read from the top or from the left is turned around.
    fn readable(&self, direction: Vec2) -> Vec2 {
        let on_screen = Vec2::from_angle(-(self.turns as f32) * FRAC_PI_2).rotate(direction);
        if on_screen.x > 0.01 || (on_screen.x.abs() <= 0.01 && on_screen.y > 0.0) {
            direction
        } else {
            -direction
        }
    }

    /// The direction on the ground of the bottom edge of the screen, from left to right.
    fn level(&self) -> Vec2 {
        Vec2::from_angle(self.turns as f32 * FRAC_PI_2)
    }

    /// The lines of a text whose middle is at `middle`, along `along`, with the outline of the
    /// room it takes.
    fn letter(&self, text: &str, (middle, along): (Vec2, Vec2), height: f32) -> Lettered {
        let up = along.perp();
        let width = self.lettering.width(text, height);
        let baseline = middle - up * height / 2.0;
        let (half_width, half_height) = (along * width / 2.0, up * height / 2.0);
        Lettered {
            lines: self.lettering.lines(text, height),
            setting: Setting {
                at: scene::to_world(baseline, 3.0 * LIFT),
                along: Vec3::new(along.x, 0.0, -along.y),
            },
            pen: height / 10.0,
            outline: [
                middle - half_width - half_height,
                middle + half_width - half_height,
                middle + half_width + half_height,
                middle - half_width + half_height,
            ],
        }
    }
}

/// A text, ready to be drawn.
struct Lettered {
    lines: Vec<Vec<Vec2>>,
    setting: Setting,
    pen: f32,
    outline: [Vec2; 4],
}

impl Lettered {
    fn draw(&self, draft: &mut Draft, color: Color) {
        draft.pen(&self.lines, self.setting, self.pen, color);
    }
}

/// The walls of a sector on the ground.
fn walls(plan: &SectorPlan) -> Vec<(Vec2, Vec2)> {
    let walls = plan.walls.iter();
    walls.map(|&(a, b)| (a.as_vec2(), b.as_vec2())).collect()
}

/// The direction away from the corridor on the outside of a wall. The outline of a sector runs
/// clockwise, so the outside is on the left.
fn outward((from, to): (Vec2, Vec2)) -> Vec2 {
    (to - from).perp().normalize()
}

/// What the margin must keep clear of: the floors and the walls of the sectors around, the
/// tags of their barriers, and what is already drawn there.
struct Room {
    floors: Vec<Vec<Vec2>>,
    walls: Vec<(Vec2, Vec2)>,
    /// Where the tags of the barriers stand.
    tags: Vec<Vec2>,
    /// The room that what is drawn takes, with the line drawn there, if it is a line.
    taken: Vec<([Vec2; 4], Option<(Vec2, Vec2)>)>,
    /// The extension lines drawn, which the dimensions of a chain share.
    extensions: Vec<(Vec2, Vec2)>,
}

impl Room {
    fn new(plans: &[&SectorPlan]) -> Self {
        let mut room = Self {
            floors: Vec::new(),
            walls: Vec::new(),
            tags: Vec::new(),
            taken: Vec::new(),
            extensions: Vec::new(),
        };
        for plan in plans {
            room.floors
                .push(plan.outline.iter().map(|point| point.as_vec2()).collect());
            room.walls.extend(walls(plan));
            if plan.kind == SectorKind::Room {
                continue;
            }
            let (a, b) = (plan.gate.0.as_vec2(), plan.gate.1.as_vec2());
            let along = (b - a).normalize_or_zero();
            room.tags
                .extend([a - along * TAG_REACH, b + along * TAG_REACH]);
        }
        room
    }

    /// Whether a point is under the tag of a barrier.
    fn tagged(&self, point: Vec2) -> bool {
        self.tags.iter().any(|tag| tag.distance(point) < TAG_ROOM)
    }

    /// Whether a point is clear of the floors and the walls, and of what is drawn.
    fn clear(&self, point: Vec2) -> bool {
        self.clear_beside(point, None)
    }

    /// Whether a point of a line is clear. Lines drawn on the same line are no obstacle: two
    /// dimensions of a chain share their dimension line, and the extension line between them.
    fn clear_beside(&self, point: Vec2, line: Option<(Vec2, Vec2)>) -> bool {
        let floor = self.floors.iter().any(|outline| inside(outline, point));
        let wall = self
            .walls
            .iter()
            .any(|&(a, b)| distance_to_segment(point, a, b) < WALL_THICKNESS + CLEARANCE);
        let taken = self.taken.iter().any(|(quad, drawn)| {
            let shared =
                matches!((line, drawn), (Some(line), Some(drawn)) if collinear(line, *drawn));
            !shared && in_quad(quad, point)
        });
        !floor && !wall && !taken && !self.tagged(point)
    }

    /// Whether every point of a line, every quarter of a meter, is clear.
    fn line_clear(&self, from: Vec2, to: Vec2) -> bool {
        let steps = ((from.distance(to) / 0.25).ceil() as usize).max(1);
        (0..=steps).all(|step| {
            self.clear_beside(from.lerp(to, step as f32 / steps as f32), Some((from, to)))
        })
    }

    /// The part of an extension line that is not drawn yet: another dimension may have drawn
    /// it from the same foot, as far as its own dimension line or further.
    fn undrawn(&self, (from, to): (Vec2, Vec2)) -> Option<(Vec2, Vec2)> {
        let length = from.distance(to);
        let drawn = self
            .extensions
            .iter()
            .filter(|&&(start, stop)| {
                collinear((start, stop), (from, to)) && start.distance(from) < 1.0
            })
            .map(|&(_, stop)| (stop - from).dot((to - from) / length))
            .fold(0.0, f32::max);
        (drawn < length - 0.05).then(|| (from + (to - from) * (drawn / length), to))
    }

    /// Whether a point is within the band of a wall that has an end at `corner`.
    fn near_corner(&self, point: Vec2, corner: Vec2) -> bool {
        self.walls.iter().any(|&(a, b)| {
            (a.distance(corner) < 0.01 || b.distance(corner) < 0.01)
                && distance_to_segment(point, a, b) < WALL_THICKNESS + CLEARANCE
        })
    }

    /// Whether the room a text takes is clear, inside and on its edges.
    fn text_clear(&self, outline: &[Vec2; 4]) -> bool {
        let center = (outline[0] + outline[2]) / 2.0;
        self.clear(center)
            && (0..4).all(|side| self.line_clear(outline[side], outline[(side + 1) % 4]))
    }

    /// Takes the room around a line, with some clearance on each side.
    fn take_line(&mut self, from: Vec2, to: Vec2) {
        let along = (to - from).normalize_or_zero() * CLEARANCE;
        let across = along.perp();
        let quad = [
            from - along - across,
            to + along - across,
            to + along + across,
            from - along + across,
        ];
        self.taken.push((quad, Some((from, to))));
    }

    fn take(&mut self, outline: [Vec2; 4]) {
        let center = (outline[0] + outline[2]) / 2.0;
        let grown =
            outline.map(|corner| corner + (corner - center).normalize_or_zero() * CLEARANCE);
        self.taken.push((grown, None));
    }
}

/// Whether two lines lie on the same line.
fn collinear((a, b): (Vec2, Vec2), (c, d): (Vec2, Vec2)) -> bool {
    let along = (b - a).normalize_or_zero();
    let other = (d - c).normalize_or_zero();
    along.perp_dot(other).abs() < 0.01 && along.perp_dot(c - a).abs() < 0.02
}

/// Which corners of the outlines are known to lie at a given distance from each other along
/// each axis, from what the drawing shows. A line that runs along one axis shows that its two
/// ends lie at the same place along the other, as a square corner needs no dimension. A wall
/// whose length is given ties its two ends along its axis, and a wall that slopes ties them
/// along both once two of its length, its angle and its two legs are known. A length between
/// two corners that are already tied is redundant: ISO 129 dimensions each feature once.
#[derive(Default)]
struct Known {
    /// Along x, then along y: the corner that each corner is tied to, up to the one that
    /// stands for all of them.
    axes: [HashMap<IVec2, IVec2>; 2],
}

impl Known {
    fn root(&mut self, axis: usize, corner: IVec2) -> IVec2 {
        let ties = &mut self.axes[axis];
        let mut root = corner;
        while let Some(&next) = ties.get(&root) {
            root = next;
        }
        let mut node = corner;
        while let Some(&next) = ties.get(&node) {
            if next != root {
                ties.insert(node, root);
            }
            node = next;
        }
        root
    }

    fn tie(&mut self, axis: usize, a: IVec2, b: IVec2) {
        let (a, b) = (self.root(axis, a), self.root(axis, b));
        if a != b {
            self.axes[axis].insert(a, b);
        }
    }

    fn tied(&mut self, axis: usize, a: IVec2, b: IVec2) -> bool {
        self.root(axis, a) == self.root(axis, b)
    }

    /// What the outline of a sector shows by itself, its barrier and its entry included.
    fn outline(&mut self, outline: &[IVec2]) {
        for index in 0..outline.len() {
            let (a, b) = (outline[index], outline[(index + 1) % outline.len()]);
            if a.x == b.x {
                self.tie(0, a, b);
            }
            if a.y == b.y {
                self.tie(1, a, b);
            }
        }
    }

    /// Whether the length of a wall follows from what is known.
    fn redundant(&mut self, (a, b): (IVec2, IVec2), angled: bool) -> bool {
        let known = [self.tied(0, a, b), self.tied(1, a, b)];
        if a.x == b.x {
            known[1]
        } else if a.y == b.y {
            known[0]
        } else {
            (known[0] && known[1]) || (angled && (known[0] || known[1]))
        }
    }

    /// Records what is known of a wall once its length is drawn, or left out.
    fn learn(&mut self, (a, b): (IVec2, IVec2), angled: bool, drawn: bool) {
        if a.x == b.x {
            if drawn {
                self.tie(1, a, b);
            }
        } else if a.y == b.y {
            if drawn {
                self.tie(0, a, b);
            }
        } else {
            let known = [self.tied(0, a, b), self.tied(1, a, b)];
            let told = [angled, drawn, known[0], known[1]];
            if told.iter().filter(|&&told| told).count() >= 2 {
                self.tie(0, a, b);
                self.tie(1, a, b);
            }
        }
    }
}

/// Whether a point is inside a polygon.
fn inside(outline: &[Vec2], point: Vec2) -> bool {
    let mut inside = false;
    for index in 0..outline.len() {
        let (a, b) = (outline[index], outline[(index + 1) % outline.len()]);
        if (a.y > point.y) != (b.y > point.y) {
            let x = a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x);
            if point.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

/// Whether a point is inside a convex quadrilateral, whichever way its corners turn.
fn in_quad(quad: &[Vec2; 4], point: Vec2) -> bool {
    let sides: Vec<f32> = (0..4)
        .map(|side| {
            let (a, b) = (quad[side], quad[(side + 1) % 4]);
            (b - a).perp_dot(point - a)
        })
        .collect();
    sides.iter().all(|side| *side >= 0.0) || sides.iter().all(|side| *side <= 0.0)
}

fn distance_to_segment(point: Vec2, a: Vec2, b: Vec2) -> f32 {
    let along = b - a;
    let share = ((point - a).dot(along) / along.length_squared()).clamp(0.0, 1.0);
    point.distance(a + along * share)
}

/// A point of the margin on the ground, as the drawing places it.
fn at(point: Vec2) -> Vec3 {
    scene::to_world(point, 2.0 * LIFT)
}

/// The strokes along the outside of a wall, leaning back, as the ground is drawn behind a
/// fixed outline.
fn hatch(draft: &mut Draft, wall: (Vec2, Vec2), room: &Room, ink: Ink) {
    let (a, b) = wall;
    let (length, along, out) = (a.distance(b), (b - a).normalize(), outward(wall));
    let lean = (out - along).normalize() * HATCH.1;
    let count = (length / HATCH.0).floor() as usize;
    let start = (length - (count as f32 - 1.0) * HATCH.0) / 2.0;
    for step in 0..count {
        let foot = a + along * (start + step as f32 * HATCH.0) + out * (WALL_THICKNESS + 0.02);
        let tip = foot + lean;
        let clear = |point: Vec2| {
            let floor = room.floors.iter().any(|outline| inside(outline, point));
            let wall = room
                .walls
                .iter()
                .any(|&(c, d)| distance_to_segment(point, c, d) < WALL_THICKNESS - 0.01);
            !floor && !wall
        };
        if clear(foot + lean * 0.1) && clear(tip) {
            draft.line(at(foot), at(tip), NARROW, ink.hatch);
        }
    }
}

/// An arrowhead whose point is at `tip`, pointing along `pointing`.
fn arrowhead(draft: &mut Draft, tip: Vec2, pointing: Vec2, color: Color) {
    let back = tip - pointing * ARROW.x;
    let side = pointing.perp() * ARROW.y / 2.0;
    draft.face(&[at(tip), at(back + side), at(back - side)], color);
}

/// How a corner of the outline turns, seen along the walls in the order of the outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Turn {
    /// The corridor turns away from the outside: the corner sticks out into the void.
    Out,
    /// The corner points into the corridor.
    In,
    /// No wall goes on there: a barrier or an entry.
    Open,
}

/// How the corner at the start of a wall turns, and the wall before it.
fn turn_before(walls: &[(Vec2, Vec2)], index: usize) -> (Turn, Option<(Vec2, Vec2)>) {
    let (a, b) = walls[index];
    match walls.iter().find(|other| other.1 == a) {
        None => (Turn::Open, None),
        Some(&before) => {
            let bend = (before.1 - before.0).perp_dot(b - a);
            (if bend < 0.0 { Turn::Out } else { Turn::In }, Some(before))
        }
    }
}

/// How the corner at the end of a wall turns, and the wall after it.
fn turn_after(walls: &[(Vec2, Vec2)], index: usize) -> (Turn, Option<(Vec2, Vec2)>) {
    let (a, b) = walls[index];
    match walls.iter().find(|other| other.0 == b) {
        None => (Turn::Open, None),
        Some(&after) => {
            let bend = (b - a).perp_dot(after.1 - after.0);
            (if bend < 0.0 { Turn::Out } else { Turn::In }, Some(after))
        }
    }
}

/// A length in meters as a drawing gives it: whole when it is whole, else to the centimeter,
/// without the zeros that end it.
fn meters(length: f32) -> String {
    if (length - length.round()).abs() < 0.005 {
        format!("{}", length.round() as i64)
    } else {
        let centimeters = format!("{length:.2}");
        centimeters.trim_end_matches('0').to_string()
    }
}

/// An angle in degrees and minutes, as a drawing gives it.
fn degrees(angle: f32) -> String {
    let total = (angle.to_degrees() * 60.0).round() as i64;
    let (whole, minutes) = (total / 60, total % 60);
    if minutes == 0 {
        format!("{whole}°")
    } else {
        format!("{whole}°{minutes:02}′")
    }
}

/// The direction of a wall.
fn direction((from, to): (Vec2, Vec2)) -> Vec2 {
    (to - from).normalize()
}

/// The corner where the outer faces of two walls meet, the walls meeting at `corner`.
fn outer_corner(corner: Vec2, incoming: Vec2, outgoing: Vec2) -> Vec2 {
    let (out_in, out_out) = (incoming.perp(), outgoing.perp());
    corner + (out_in + out_out) / (1.0 + out_in.dot(out_out)) * WALL_THICKNESS
}

/// Where the dimension line of a wall can end, at one end of the wall.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stop {
    /// On an extension line that goes out from `foot`, square to the wall, starting `gap`
    /// from it. The dimension line is at least `reach` from the wall.
    Extension { foot: Vec2, gap: f32, reach: f32 },
    /// On the outer face of another wall, `extent` long, that runs from `point` along
    /// `direction`.
    Face {
        point: Vec2,
        direction: Vec2,
        extent: f32,
    },
}

impl Stop {
    fn reach(self) -> f32 {
        match self {
            Stop::Extension { reach, .. } => reach,
            Stop::Face { .. } => WALL_THICKNESS + OFFSET,
        }
    }

    /// How far along the wall from its start the dimension line ends there, when it is
    /// `reach` from the wall.
    fn along(self, (a, b): (Vec2, Vec2), reach: f32) -> Option<f32> {
        let (along, out) = (direction((a, b)), outward((a, b)));
        match self {
            Stop::Extension { foot, .. } => Some((foot - a).dot(along)),
            Stop::Face {
                point,
                direction,
                extent,
            } => {
                let base = a + out * reach;
                let square = along.perp_dot(direction);
                if square.abs() < 0.1 {
                    return None;
                }
                let share = (point - base).perp_dot(direction) / square;
                let on_face = (base + along * share - point).dot(direction);
                (-WALL_THICKNESS..=extent + WALL_THICKNESS)
                    .contains(&on_face)
                    .then_some(share)
            }
        }
    }
}

/// Where the dimension line of a wall can end, at its start or at its end. The corner of the
/// wall itself is one place, which an extension line takes out to the dimension line where it
/// is in sight. Where the corner points into the corridor, it is hidden under the other wall
/// unless that wall slopes away, so the dimension line can end on the outer face of the other
/// wall instead, a wall's thickness short of the corner; it then starts on the outer corner of
/// the faces at its other end, a wall's thickness past that corner, if that corner sticks out.
/// It can also end in line with the far end of the other wall, which stands out of this one, and
/// then goes on past it.
fn stops(walls: &[(Vec2, Vec2)], index: usize, at_end: bool) -> Vec<Stop> {
    let wall = walls[index];
    let (along, out) = (direction(wall), outward(wall));
    let reach = WALL_THICKNESS + OFFSET;
    let (corner, turn) = if at_end {
        (wall.1, turn_after(walls, index))
    } else {
        (wall.0, turn_before(walls, index))
    };
    let mut stops = vec![Stop::Extension {
        foot: corner,
        gap: WALL_THICKNESS + EXTENSION.0,
        reach,
    }];
    match turn {
        (Turn::Out, Some(other)) => {
            let (incoming, outgoing) = if at_end {
                (along, direction(other))
            } else {
                (direction(other), along)
            };
            stops.push(Stop::Extension {
                foot: outer_corner(corner, incoming, outgoing),
                gap: EXTENSION.0,
                reach,
            });
        }
        (Turn::In, Some(other)) => {
            stops.push(Stop::Face {
                point: other.0 + outward(other) * WALL_THICKNESS,
                direction: direction(other),
                extent: other.0.distance(other.1),
            });
            let far = if at_end { other.1 } else { other.0 };
            let standing = (far - corner).dot(out);
            if standing > 0.5 && standing <= CHAIN_REACH {
                stops.push(Stop::Extension {
                    foot: far,
                    gap: WALL_THICKNESS + EXTENSION.0,
                    reach: standing + reach,
                });
            }
        }
        _ => {}
    }
    stops
}

/// The ways to dimension a wall, the nearest to the wall first: the line's distance from the
/// wall, its stops, and how far along the wall from its start it ends at each. First those
/// whose dimension line measures exactly the length of the wall, then those that end on the
/// outer face of another wall, under which a corner of this one hides: they measure what they
/// span, which their value then says. Every dimension measures exactly its value.
fn ways(walls: &[(Vec2, Vec2)], index: usize) -> Vec<(f32, [Stop; 2], [f32; 2])> {
    let wall = walls[index];
    let (length, along) = (wall.0.distance(wall.1), direction(wall));
    let (mut exact, mut faced) = (Vec::new(), Vec::new());
    for start in stops(walls, index, false) {
        for end in stops(walls, index, true) {
            let reach = start.reach().max(end.reach());
            let (Some(from), Some(to)) = (start.along(wall, reach), end.along(wall, reach)) else {
                continue;
            };
            // A face that is square to the wall: the line then spans the length of the wall
            // less the thickness of the wall that it ends on.
            let square = |stop: &Stop| match *stop {
                Stop::Face { direction, .. } => direction.dot(along).abs() < 0.01,
                Stop::Extension { .. } => false,
            };
            let on_face = [start, end].iter().any(square);
            if (to - from - length).abs() < 0.01 {
                exact.push((reach, [start, end], [from, to]));
            } else if on_face && to - from >= SHORTEST {
                faced.push((reach, [start, end], [from, to]));
            }
        }
    }
    exact.sort_by(|a, b| a.0.total_cmp(&b.0));
    faced.sort_by(|a, b| a.0.total_cmp(&b.0));
    exact.extend(faced);
    exact
}

/// The length of a wall, dimensioned on its outside, the first way that has room. Returns
/// whether it is drawn.
fn length(
    draft: &mut Draft,
    (walls, index): (&[(Vec2, Vec2)], usize),
    room: &mut Room,
    paper: &Paper,
) -> bool {
    let (a, b) = walls[index];
    let length = a.distance(b);
    if length < SHORTEST {
        return false;
    }
    ways(walls, index)
        .into_iter()
        .any(|way| dimension(draft, (a, b), way, room, paper))
}

/// Draws the dimension of a wall one way, if it has room. Returns whether it is drawn.
fn dimension(
    draft: &mut Draft,
    (a, b): (Vec2, Vec2),
    (reach, stops, ends): (f32, [Stop; 2], [f32; 2]),
    room: &mut Room,
    paper: &Paper,
) -> bool {
    let (along, out) = (direction((a, b)), outward((a, b)));
    let (from, to) = (
        a + out * reach + along * ends[0],
        a + out * reach + along * ends[1],
    );
    if ends[1] - ends[0] < 2.0 * ARROW.x + 0.2 {
        return false;
    }
    // Each extension line starts past what is in its way where it leaves its foot: the tag of
    // a barrier, which goes on the line of the barrier, or the wall over a corner that points
    // into the corridor.
    let mut extensions: Vec<(Vec2, Vec2)> = Vec::new();
    for (stop, end) in stops.into_iter().zip(ends) {
        let Stop::Extension { foot, gap, .. } = stop else {
            continue;
        };
        let line = a + out * reach + along * end;
        let span = (line - foot).dot(out);
        let point = |start: f32| foot + out * start;
        // Past a wall that slopes away from a hidden corner, but not along a whole wall.
        let past_wall = |start: f32| start < gap + 1.0 && room.near_corner(point(start), foot);
        let mut start = gap;
        while start < span - 0.2 && (room.tagged(point(start)) || past_wall(start)) {
            start += 0.05;
        }
        if start >= span - 0.2 || room.near_corner(point(start), foot) {
            return false;
        }
        extensions.push((point(start), line + out * EXTENSION.1));
    }
    let text = meters(ends[1] - ends[0]);
    let reading = paper.readable(along);
    let middle = (from + to) / 2.0;
    let lift = reading.perp() * (VALUE_GAP + VALUE_HEIGHT / 2.0);
    let value = paper.letter(&text, (middle + lift, reading), VALUE_HEIGHT);
    // A dimension line is not checked where it ends: next to the outside of a wall, or on an
    // extension line, which another dimension of a chain may have drawn already.
    let on_face = stops.map(|stop| matches!(stop, Stop::Face { .. }));
    let short = |on_wall: bool| {
        if on_wall {
            WALL_THICKNESS / 2.0 + CLEARANCE
        } else {
            CLEARANCE + 0.05
        }
    };
    let undrawn: Vec<(Vec2, Vec2)> = extensions
        .iter()
        .filter_map(|&extension| room.undrawn(extension))
        .collect();
    let clear = room.line_clear(
        from + along * short(on_face[0]),
        to - along * short(on_face[1]),
    ) && undrawn
        .iter()
        .all(|&(start, stop)| room.line_clear(start, stop))
        && room.text_clear(&value.outline);
    if !clear {
        return false;
    }
    let color = paper.ink.line;
    draft.line(at(from), at(to), NARROW, color);
    arrowhead(draft, from, -along, color);
    arrowhead(draft, to, along, color);
    for &(start, stop) in &undrawn {
        draft.line(at(start), at(stop), NARROW, color);
        room.take_line(start, stop);
    }
    room.extensions.extend(extensions);
    value.draw(draft, paper.ink.letter);
    room.take_line(from, to);
    room.take(value.outline);
    true
}

/// The angle of a wall that slopes, at a corner of it that sticks out into the void: an arc
/// from the line of the wall before it, which goes on past the corner, to the wall itself.
/// Returns whether it is drawn.
fn angle(
    draft: &mut Draft,
    (walls, index): (&[(Vec2, Vec2)], usize),
    room: &mut Room,
    paper: &Paper,
) -> bool {
    let (a, b) = walls[index];
    let along = (b - a).normalize();
    if along.x.abs() < 0.01 || along.y.abs() < 0.01 {
        return false;
    }
    let radius = (a.distance(b) * 0.75).clamp(ARC.0, ARC.1);
    // The corner at its start, or else the one at its end, measured from the wall before.
    let corners = [
        (turn_before(walls, index), a, true),
        (turn_after(walls, index), b, false),
    ];
    for ((turn, other), corner, starts) in corners {
        let (Turn::Out, Some(other)) = (turn, other) else {
            continue;
        };
        let other_along = (other.1 - other.0).normalize();
        let (incoming, outgoing) = if starts {
            (other_along, along)
        } else {
            (along, other_along)
        };
        let swept = incoming.angle_to(outgoing);
        if swept.abs() < 0.05 {
            continue;
        }
        // The angle is drawn on the outside of the two walls, where their outer faces meet.
        let corner = outer_corner(corner, incoming, outgoing);
        let reference = (
            corner + incoming * 0.15,
            corner + incoming * (radius + 0.35),
        );
        let steps = ((swept.abs() * radius / 0.25).ceil() as usize).max(4);
        let arc: Vec<Vec2> = (0..=steps)
            .map(|step| {
                let share = step as f32 / steps as f32;
                corner + Vec2::from_angle(incoming.to_angle() + swept * share) * radius
            })
            .collect();
        let middle = Vec2::from_angle(incoming.to_angle() + swept / 2.0);
        let text = degrees(swept.abs());
        let width = paper.lettering.width(&text, VALUE_HEIGHT);
        // The value stands out from the middle of the arc, or else further out, or past the
        // end of the line of the wall before.
        let places = [
            corner + middle * (radius + 0.35 + width / 2.0),
            corner + middle * (radius + 1.0 + width / 2.0),
            corner + incoming * (radius + 0.6 + width / 2.0),
        ];
        let Some(value) = places
            .into_iter()
            .map(|place| paper.letter(&text, (place, paper.level()), VALUE_HEIGHT))
            .find(|value| room.text_clear(&value.outline))
        else {
            continue;
        };
        // The arc ends on the outer face of the wall: what is near it is not checked.
        let open = |point: &Vec2| point.distance(arc[steps]) > 0.3 && point.distance(arc[0]) > 0.3;
        let free = |from: Vec2, to: Vec2| {
            let steps = ((from.distance(to) / 0.2).ceil() as usize).max(1);
            (0..=steps)
                .map(|step| from.lerp(to, step as f32 / steps as f32))
                .filter(open)
                .all(|point| room.clear(point))
        };
        let clear = free(reference.0 + incoming * 0.3, reference.1)
            && arc.windows(2).all(|pair| free(pair[0], pair[1]));
        if !clear {
            continue;
        }
        let color = paper.ink.line;
        draft.line(at(reference.0), at(reference.1), NARROW, color);
        for pair in arc.windows(2) {
            draft.line(at(pair[0]), at(pair[1]), NARROW, color);
        }
        let tangent = |point: Vec2| (point - corner).perp().normalize() * swept.signum();
        arrowhead(draft, arc[steps], tangent(arc[steps]), color);
        arrowhead(draft, arc[0], -tangent(arc[0]), color);
        value.draw(draft, paper.ink.letter);
        room.take_line(reference.0, reference.1);
        for pair in arc.windows(2) {
            room.take_line(pair[0], pair[1]);
        }
        room.take(value.outline);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use std::f32::consts::TAU;

    use tanku_core::map::Corridor;

    use super::*;

    /// A world that draws the margin of the sectors of a seed's corridor, in a run that has
    /// reached a sector.
    fn margin_world(seed: &str, sector: u32) -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Time>()
            .init_resource::<View>()
            .init_resource::<Margin>()
            .init_resource::<Lettering>()
            .insert_resource(Run {
                seed_text: seed.into(),
                seed: tanku_core::seed::seed_value(seed),
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
            })
            .add_systems(PreStartup, scene::setup)
            .add_systems(Update, annotate);
        app
    }

    /// The margin drawn for a sector, and what it told.
    fn margin_of_sector(app: &mut App, sector: Entity) -> (Option<Handle<Mesh>>, Margined) {
        let world = app.world_mut();
        let mut drawings = world.query_filtered::<(&ChildOf, &Mesh3d), With<Annotation>>();
        let mesh = drawings
            .iter(world)
            .find(|(parent, _)| parent.parent() == sector)
            .map(|(_, mesh)| mesh.0.clone());
        let told = world.get::<Margined>(sector).cloned().expect("a margin");
        (mesh, told)
    }

    #[test]
    fn a_sector_keeps_its_margin_when_the_next_one_comes() {
        // On this seed the left wall of sector 12 ends where the next sector turns west: a
        // margin drawn without sector 13 measured it with an extension line that the wall of
        // sector 13 then lay on, and its dimension went when sector 13 came.
        let seed = "HTVH9SEE";
        let mut corridor = Corridor::new(tanku_core::seed::seed_value(seed));
        let mut app = margin_world(seed, 11);
        let mut load = |app: &mut App, index: u32| {
            let layout = corridor.layout(index);
            app.world_mut().spawn(Sector { layout }).id()
        };
        load(&mut app, 11);
        let twelve = load(&mut app, 12);
        app.update();
        let before = margin_of_sector(&mut app, twelve);
        assert!(before.0.is_some());
        let thirteen = load(&mut app, 13);
        app.update();
        // The margin of sector 12 is the very drawing it was, and the next one has its own.
        assert_eq!(margin_of_sector(&mut app, twelve), before);
        assert!(margin_of_sector(&mut app, thirteen).0.is_some());
        // Drawn again, as when the view turns, it is the same.
        app.world_mut().resource_mut::<View>().turns = 1;
        app.update();
        app.world_mut().resource_mut::<View>().turns = 0;
        app.update();
        assert_eq!(margin_of_sector(&mut app, twelve).1, before.1);
    }

    #[test]
    fn lengths_are_whole_when_they_are_whole() {
        assert_eq!(meters(24.0), "24");
        assert_eq!(meters(20f32.sqrt()), "4.47");
        assert_eq!(meters(18f32.sqrt()), "4.24");
        assert_eq!(meters(51.6), "51.6");
    }

    #[test]
    fn a_corner_under_the_wall_of_the_next_sector_is_measured_to_its_face() {
        // Sector 12 of this seed runs north and sector 13 turns west from its barrier: the
        // corner at the top of the left wall of sector 12 is under the wall of sector 13.
        let mut corridor = Corridor::new(tanku_core::seed::seed_value("HTVH9SEE"));
        let (twelve, thirteen) = (corridor.plan(12).clone(), corridor.plan(13).clone());
        let left = twelve.walls[0];
        assert_eq!(left, (IVec2::new(-6, 585), IVec2::new(-6, 637)));
        let mut walls = super::walls(&twelve);
        walls.extend(super::walls(&thirteen));
        let ways = ways(&walls, 0);
        // First the way that measures the 52 m of the wall, with an extension line from the
        // corner, which the wall of sector 13 lies on: it has no room once that wall is there.
        let (_, stops, ends) = ways[0];
        assert!(matches!(stops[1], Stop::Extension { .. }));
        assert!((ends[1] - ends[0] - 52.0).abs() < 1e-3);
        // Then a way that ends on the outer face of that wall, and says that it spans 51.6.
        let faced = ways
            .iter()
            .find(|(_, stops, _)| matches!(stops[1], Stop::Face { .. }));
        let spans = faced.map(|(_, _, ends)| ends[1] - ends[0]);
        assert!(spans.is_some_and(|span| (span - (52.0 - WALL_THICKNESS)).abs() < 1e-3));
        // Drawn among its neighbors, the wall has its length.
        let lettering = Lettering::default();
        let paper = Paper {
            lettering: &lettering,
            turns: 0,
            ink: Ink::new(),
        };
        let around = [corridor.plan(11).clone(), thirteen];
        let (_, margined) = margin_of(&twelve, &around, &[], &paper);
        assert!(
            margined
                .told
                .iter()
                .any(|&(wall, _, told)| wall == left && told)
        );
    }

    #[test]
    fn angles_are_given_in_degrees_and_minutes() {
        assert_eq!(degrees(45f32.to_radians()), "45°");
        assert_eq!(degrees((0.5f32).atan()), "26°34′");
        assert_eq!(degrees((2.0f32).atan()), "63°26′");
        assert_eq!(degrees(TAU / 4.0), "90°");
    }

    #[test]
    fn a_text_reads_from_the_bottom_or_from_the_right() {
        let lettering = Lettering::default();
        let paper = |turns| Paper {
            lettering: &lettering,
            turns,
            ink: Ink::new(),
        };
        // North at the top: a text along a wall that runs south reads from the bottom up.
        assert_eq!(paper(0).readable(Vec2::NEG_Y), Vec2::Y);
        assert_eq!(paper(0).readable(Vec2::NEG_X), Vec2::X);
        // North on the right: a text along a wall that runs north reads from left to right as
        // it is, and one along a wall that runs east would read down the screen.
        assert_eq!(paper(1).readable(Vec2::Y), Vec2::Y);
        assert_eq!(paper(1).readable(Vec2::X), Vec2::NEG_X);
    }

    #[test]
    fn corners_turn_out_where_the_corridor_turns_away() {
        // A corridor that runs north and turns east at its top, clockwise.
        let walls = [
            (Vec2::new(0.0, 0.0), Vec2::new(0.0, 10.0)),
            (Vec2::new(0.0, 10.0), Vec2::new(10.0, 10.0)),
        ];
        assert_eq!(turn_after(&walls, 0).0, Turn::Out);
        assert_eq!(turn_before(&walls, 0).0, Turn::Open);
        // A step that widens it to the west points into it.
        let step = [
            (Vec2::new(0.0, 0.0), Vec2::new(0.0, 10.0)),
            (Vec2::new(0.0, 10.0), Vec2::new(-4.0, 10.0)),
        ];
        assert_eq!(turn_after(&step, 0).0, Turn::In);
    }

    #[test]
    fn opposite_walls_of_the_bay_are_dimensioned_once() {
        let corner = |x, y| IVec2::new(x, y);
        let mut known = Known::default();
        known.outline(&[corner(0, 0), corner(0, 16), corner(14, 16), corner(14, 0)]);
        let left = (corner(0, 0), corner(0, 16));
        let right = (corner(14, 16), corner(14, 0));
        let south = (corner(14, 0), corner(0, 0));
        // Once the right side is given, the left side follows from the square corners.
        assert!(!known.redundant(right, false));
        known.learn(right, false, true);
        assert!(known.redundant(left, false));
        assert!(!known.redundant(south, false));
    }

    #[test]
    fn a_chain_that_the_other_side_closes_loses_one_link() {
        // A corridor whose left side steps 2 m to the west halfway up.
        let corner = |x, y| IVec2::new(x, y);
        let mut known = Known::default();
        known.outline(&[
            corner(0, 0),
            corner(0, 10),
            corner(-2, 10),
            corner(-2, 20),
            corner(12, 20),
            corner(12, 0),
        ]);
        known.learn((corner(12, 20), corner(12, 0)), false, true);
        for wall in [
            (corner(-2, 10), corner(-2, 20)),
            (corner(0, 10), corner(-2, 10)),
        ] {
            assert!(!known.redundant(wall, false));
            known.learn(wall, false, true);
        }
        assert!(known.redundant((corner(0, 0), corner(0, 10)), false));
    }

    #[test]
    fn a_wall_that_slopes_is_told_by_its_angle_and_one_leg() {
        let slope = (IVec2::new(0, 10), IVec2::new(-4, 12));
        let mut known = Known::default();
        // How far it rises is known from other walls.
        known.tie(1, slope.0, slope.1);
        assert!(!known.redundant(slope, false));
        assert!(known.redundant(slope, true));
    }

    #[test]
    fn a_step_is_measured_between_the_outer_faces_of_its_walls() {
        // Up the left side of a corridor, a step 2 m to the west.
        let walls = [
            (Vec2::new(0.0, 0.0), Vec2::new(0.0, 10.0)),
            (Vec2::new(0.0, 10.0), Vec2::new(-2.0, 10.0)),
            (Vec2::new(-2.0, 10.0), Vec2::new(-2.0, 20.0)),
        ];
        let ways = ways(&walls, 1);
        // No way measures 1.6 m and calls it 2.
        assert!(!ways.is_empty());
        assert!(
            ways.iter()
                .all(|(_, _, ends)| (ends[1] - ends[0] - 2.0).abs() < 0.01)
        );
        // The corner where it starts is hidden under the wall below, so the nearest way ends on
        // the outer face of that wall and starts on the outer corner at the other end, both a
        // wall's thickness along.
        let faces = ways.iter().find(|(reach, stops, _)| {
            *reach == WALL_THICKNESS + OFFSET && matches!(stops[0], Stop::Face { .. })
        });
        let ends = faces.map(|(_, _, ends)| *ends);
        let expected = [WALL_THICKNESS, 2.0 + WALL_THICKNESS];
        assert!(
            ends.is_some_and(|ends| ends.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-4))
        );
    }

    #[test]
    fn a_wall_that_ends_on_a_step_is_measured_in_line_with_the_step() {
        // The right side of a corridor that steps 2 m to the east 8 m from its entry.
        let walls = [
            (Vec2::new(0.0, 0.0), Vec2::new(0.0, 20.0)),
            (Vec2::new(14.0, 20.0), Vec2::new(14.0, 8.0)),
            (Vec2::new(14.0, 8.0), Vec2::new(12.0, 8.0)),
            (Vec2::new(12.0, 8.0), Vec2::new(12.0, 0.0)),
        ];
        // The corner of the step is hidden: the dimension goes out past the step, on the line
        // of the dimension of the wall above it.
        let step = Vec2::new(14.0, 8.0);
        let reach = ways(&walls, 3).into_iter().find_map(|(reach, stops, _)| {
            matches!(stops[0], Stop::Extension { foot, .. } if foot == step).then_some(reach)
        });
        assert_eq!(reach, Some(2.0 + WALL_THICKNESS + OFFSET));
    }

    #[test]
    fn a_point_inside_an_outline_is_found_inside() {
        let outline = [
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 10.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(10.0, 0.0),
        ];
        assert!(inside(&outline, Vec2::new(5.0, 5.0)));
        assert!(!inside(&outline, Vec2::new(-1.0, 5.0)));
    }
}
