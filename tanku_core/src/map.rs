//! Generation of the corridor: an endless chain of sectors.
//!
//! Every vertex lies on a 1 m lattice and all arithmetic is on integers, so a seed gives the same
//! map on every platform. North is `+y` and east is `+x`.

use std::collections::{HashSet, VecDeque};

use bevy::math::IVec2;

use crate::{
    progression,
    rng::{Purpose, Stream},
};

/// Parts of the corridor that aren't neighbors stay at least this far apart.
pub const SECTOR_GAP: i32 = 6;
/// The corridor never runs further south of its northernmost point than this.
pub const MAX_DESCENT: i32 = 60;
/// Enemies are placed at least this far from the entry of their sector.
pub const SPAWN_DISTANCE: i32 = 15;
/// Spawn points prepared per sector, enough for an arena with four players.
pub const MAX_SPAWNS: usize = 24;

const BAY_WIDTH: i32 = 14;
const BAY_LENGTH: i32 = 16;
/// How far a wall may wander sideways from where its sector started.
const DRIFT: i32 = 12;
/// The same for sectors that run east, west or south, which have less room.
const SIDE_DRIFT: i32 = 6;
/// Straight stretch behind an entry and in front of an exit.
const APRON: i32 = 4;

/// Squared distances below are in half meters, so that the centers of blocks stay integers.
/// 1.8 m: a lattice path with this clearance is more than 3 m wide everywhere.
const PATH_CLEARANCE_2: i64 = 13;
/// 2.5 m between the center of an obstacle and a wall.
const OBSTACLE_CLEARANCE_2: i64 = 25;
/// 7 m between the center of an obstacle and a gate.
const GATE_CLEARANCE_2: i64 = 196;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Heading {
    North,
    East,
    South,
    West,
}

impl Heading {
    pub const fn forward(self) -> IVec2 {
        match self {
            Heading::North => IVec2::new(0, 1),
            Heading::East => IVec2::new(1, 0),
            Heading::South => IVec2::new(0, -1),
            Heading::West => IVec2::new(-1, 0),
        }
    }

    pub const fn right(self) -> IVec2 {
        match self {
            Heading::North => IVec2::new(1, 0),
            Heading::East => IVec2::new(0, -1),
            Heading::South => IVec2::new(-1, 0),
            Heading::West => IVec2::new(0, 1),
        }
    }

    const fn turned(self, exit: Exit) -> Heading {
        const CLOCKWISE: [Heading; 4] =
            [Heading::North, Heading::East, Heading::South, Heading::West];
        let step = match exit {
            Exit::Ahead => 0,
            Exit::Right => 1,
            Exit::Left => 3,
        };
        CLOCKWISE[(self as usize + step) % 4]
    }
}

/// The side of a sector where its barrier is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Exit {
    Ahead,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SectorKind {
    /// The empty sector where a run starts.
    Bay,
    Corridor,
    Arena,
}

/// The local frame of a sector: `u` runs to the right along the entry and `v` runs forward.
#[derive(Clone, Copy, Debug)]
struct Frame {
    /// The left end of the entry.
    origin: IVec2,
    heading: Heading,
}

impl Frame {
    fn at(&self, u: i32, v: i32) -> IVec2 {
        self.origin + self.heading.right() * u + self.heading.forward() * v
    }
}

/// The shape of one sector.
#[derive(Clone, Debug)]
pub struct SectorPlan {
    pub index: u32,
    pub kind: SectorKind,
    pub heading: Heading,
    /// The closed outline, starting at the left end of the entry.
    pub outline: Vec<IVec2>,
    /// Every edge of the outline that is a wall.
    pub walls: Vec<(IVec2, IVec2)>,
    /// The two ends of the entry. The start bay has a wall there.
    pub entry: (IVec2, IVec2),
    /// The two ends of the barrier.
    pub gate: (IVec2, IVec2),
    /// A point just inside the entry.
    pub entry_point: IVec2,
    /// A point just inside the barrier.
    pub gate_point: IVec2,
    pub min: IVec2,
    pub max: IVec2,
    next_frame: Frame,
    next_width: i32,
}

/// What the planner asks of a sector.
#[derive(Clone, Copy, Debug)]
struct Spec {
    kind: SectorKind,
    exit: Exit,
    /// Shortest and longest the sector may be.
    length: (i32, i32),
    /// The sector must reach north of everything planned so far.
    climb: bool,
    drift: i32,
}

impl Spec {
    const fn corridor(exit: Exit, drift: i32) -> Self {
        Self {
            kind: SectorKind::Corridor,
            exit,
            length: (40, 80),
            climb: false,
            drift,
        }
    }
}

/// The outline of a sector in its local frame.
struct Shape {
    left: Vec<(i32, i32)>,
    right: Vec<(i32, i32)>,
    length: i32,
    /// Width of the barrier, which is the width of the next entry.
    gate: i32,
}

/// Builds the two walls of a sector, station by station.
struct Walk<'a> {
    rng: &'a mut Stream,
    left: Vec<(i32, i32)>,
    right: Vec<(i32, i32)>,
    l: i32,
    r: i32,
    v: i32,
    /// Narrowest and widest the sector may get.
    width: (i32, i32),
    /// Leftmost and rightmost a wall may go.
    reach: (i32, i32),
    /// Which walls may move. The wall that holds a side exit stays straight.
    moves: (bool, bool),
    angled: u32,
}

/// A change of one wall over one step.
#[derive(Clone, Copy, PartialEq)]
enum Change {
    None,
    /// A wall across the corridor, then straight.
    Step(i32),
    /// A slanted wall. The flag is set for the shallowest slope, 2 m sideways per meter forward.
    Slant(i32, bool),
}

impl Change {
    fn shift(self) -> i32 {
        match self {
            Change::None => 0,
            Change::Step(du) | Change::Slant(du, _) => du,
        }
    }
}

impl<'a> Walk<'a> {
    fn new(rng: &'a mut Stream, entry: i32, sector: u32, spec: &Spec) -> Self {
        Self {
            rng,
            left: vec![(0, 0)],
            right: vec![(entry, 0)],
            l: 0,
            r: entry,
            v: 0,
            width: progression::corridor_width(sector),
            reach: (-spec.drift, entry + spec.drift),
            moves: (spec.exit != Exit::Left, spec.exit != Exit::Right),
            angled: progression::angled_wall_chance(sector),
        }
    }

    fn straight(&mut self, dv: i32) {
        self.v += dv;
        self.left.push((self.l, self.v));
        self.right.push((self.r, self.v));
    }

    fn draw_change(&mut self, moves: bool, dv: i32) -> Change {
        if !moves || self.rng.chance(60) {
            return Change::None;
        }
        let sign = if self.rng.chance(50) { 1 } else { -1 };
        if self.rng.chance(self.angled) {
            let (du, shallow) = self
                .rng
                .pick(&[(dv / 2, false), (dv, false), (2 * dv, true)]);
            Change::Slant(sign * du, shallow)
        } else {
            Change::Step(sign * self.rng.range(2, 4))
        }
    }

    fn fits(&self, left: Change, right: Change) -> bool {
        let (l, r) = (self.l + left.shift(), self.r + right.shift());
        // Two walls on the shallowest slope would leave a passage narrower than it measures.
        let shallow = |change| matches!(change, Change::Slant(_, true));
        let pinched =
            (shallow(left) && right != Change::None) || (shallow(right) && left != Change::None);
        (self.width.0..=self.width.1).contains(&(r - l))
            && l >= self.reach.0
            && r <= self.reach.1
            && !pinched
    }

    /// Moves forward by `dv`, with a random change on each wall that may move.
    fn advance(&mut self, dv: i32) {
        let mut changes = (Change::None, Change::None);
        for _ in 0..4 {
            let left = self.draw_change(self.moves.0, dv);
            let right = self.draw_change(self.moves.1, dv);
            if self.fits(left, right) {
                changes = (left, right);
                break;
            }
        }
        Self::apply(&mut self.left, &mut self.l, self.v, dv, changes.0);
        Self::apply(&mut self.right, &mut self.r, self.v, dv, changes.1);
        self.v += dv;
    }

    fn apply(wall: &mut Vec<(i32, i32)>, u: &mut i32, v: i32, dv: i32, change: Change) {
        *u += change.shift();
        if let Change::Step(_) = change {
            wall.push((*u, v));
        }
        wall.push((*u, v + dv));
    }

    /// Advances to `end` in steps of 4 to 8 m.
    fn wander(&mut self, end: i32) {
        while self.v < end {
            let left = end - self.v;
            let fitting: Vec<i32> = [4, 6, 8]
                .into_iter()
                .filter(|dv| left - dv == 0 || left - dv >= 4)
                .collect();
            let dv = if fitting.is_empty() {
                left
            } else {
                self.rng.pick(&fitting)
            };
            self.advance(dv);
        }
    }

    /// Steps a wall sideways so that the width lands in the range of the next sector.
    fn fit_width(&mut self, range: (i32, i32)) {
        let width = self.r - self.l;
        let change = width.clamp(range.0, range.1) - width;
        if change == 0 {
            return;
        }
        if self.moves.1 {
            self.r += change;
            self.right.push((self.r, self.v));
        } else {
            self.l -= change;
            self.left.push((self.l, self.v));
        }
    }
}

fn corridor_shape(rng: &mut Stream, sector: u32, entry: i32, spec: &Spec) -> Shape {
    let next_width = progression::corridor_width(sector + 1);
    // Even lengths keep every slanted wall on the lattice.
    let length = rng.range((spec.length.0 + 1) / 2, spec.length.1 / 2) * 2;
    let mut walk = Walk::new(rng, entry, sector, spec);
    walk.straight(APRON);
    let gate = match spec.exit {
        Exit::Ahead => {
            walk.wander(length - APRON);
            walk.fit_width(next_width);
            walk.straight(APRON);
            walk.r - walk.l
        }
        Exit::Left | Exit::Right => {
            // Even, so that the straight stretch along the barrier ends on a station.
            let gate = walk.rng.range((next_width.0 + 1) / 2, next_width.1 / 2) * 2;
            walk.wander(length - gate);
            walk.straight(gate);
            gate
        }
    };
    Shape {
        left: walk.left,
        right: walk.right,
        length,
        gate,
    }
}

/// An arena: the walls flare out behind the entry and come back in before the barrier.
fn arena_shape(rng: &mut Stream, sector: u32, entry: i32) -> Shape {
    let next_width = progression::corridor_width(sector + 1);
    let gate = rng.range(next_width.0, next_width.1);
    // The flares keep the arena within 36 m and leave no more than the tapers can take back.
    let narrowing = entry - gate;
    let room = (36 - entry).min(18 - narrowing);
    let flare_left = rng.range(4, (room - 4).clamp(4, 9));
    let flare_right = rng.range(
        (-narrowing - flare_left).max(4),
        (room - flare_left).clamp(4, 9),
    );
    let taper = narrowing + flare_left + flare_right;
    let taper_left = rng.range((taper - 9).max(0), taper.min(9));
    let taper_right = taper - taper_left;
    let body = rng.range(8, 14);
    let length = 4 + flare_left.max(flare_right) + body + taper_left.max(taper_right);

    let wall = |start: i32, flare: i32, taper: i32, side: i32| {
        let wide = start + side * flare;
        vec![
            (start, 0),
            (start, 2),
            (wide, 2 + flare),
            (wide, length - 2 - taper),
            (wide - side * taper, length - 2),
            (wide - side * taper, length),
        ]
    };
    Shape {
        left: wall(0, flare_left, taper_left, -1),
        right: wall(entry, flare_right, taper_right, 1),
        length,
        gate,
    }
}

fn bay_shape() -> Shape {
    Shape {
        left: vec![(0, 0), (0, BAY_LENGTH)],
        right: vec![(BAY_WIDTH, 0), (BAY_WIDTH, BAY_LENGTH)],
        length: BAY_LENGTH,
        gate: BAY_WIDTH,
    }
}

/// Drops repeated points and points in the middle of a straight line, except the `kept` ones.
fn simplify(points: &[IVec2], kept: &[IVec2]) -> Vec<IVec2> {
    let mut distinct: Vec<IVec2> = Vec::with_capacity(points.len());
    for &point in points {
        if distinct.last() != Some(&point) {
            distinct.push(point);
        }
    }
    if distinct.first() == distinct.last() {
        distinct.pop();
    }
    let count = distinct.len();
    (0..count)
        .filter(|&i| {
            let (before, here, after) = (
                distinct[(i + count - 1) % count],
                distinct[i],
                distinct[(i + 1) % count],
            );
            kept.contains(&here) || (here - before).perp_dot(after - here) != 0
        })
        .map(|i| distinct[i])
        .collect()
}

fn plan_sector(seed: u64, index: u32, frame: Frame, entry: i32, spec: &Spec) -> SectorPlan {
    let mut rng = Stream::new(seed, Purpose::Shape, index, 0);
    let shape = match spec.kind {
        SectorKind::Bay => bay_shape(),
        SectorKind::Arena => arena_shape(&mut rng, index, entry),
        SectorKind::Corridor => corridor_shape(&mut rng, index, entry, spec),
    };
    let (length, gate) = (shape.length, shape.gate);
    let (left_end, right_end) = (shape.left.last().unwrap().0, shape.right.last().unwrap().0);

    // Local ends of the barrier, the left end of the next entry first.
    let (gate_from, gate_to, gate_point) = match spec.exit {
        Exit::Ahead => (
            (left_end, length),
            (right_end, length),
            ((left_end + right_end) / 2, length - 2),
        ),
        Exit::Right => (
            (right_end, length),
            (right_end, length - gate),
            (right_end - 2, length - gate / 2),
        ),
        Exit::Left => (
            (left_end, length - gate),
            (left_end, length),
            (left_end + 2, length - gate / 2),
        ),
    };

    let world = |&(u, v): &(i32, i32)| frame.at(u, v);
    let entry_ends = (frame.at(0, 0), frame.at(entry, 0));
    let gate_ends = (world(&gate_from), world(&gate_to));
    let outline = simplify(
        &shape
            .left
            .iter()
            .chain(shape.right.iter().rev())
            .map(world)
            .collect::<Vec<_>>(),
        &[gate_ends.0, gate_ends.1],
    );

    let same_edge = |a: IVec2, b: IVec2, ends: (IVec2, IVec2)| (a, b) == ends || (b, a) == ends;
    let walls = (0..outline.len())
        .map(|i| (outline[i], outline[(i + 1) % outline.len()]))
        .filter(|&(a, b)| {
            let is_entry = same_edge(a, b, entry_ends) && spec.kind != SectorKind::Bay;
            !is_entry && !same_edge(a, b, gate_ends)
        })
        .collect();

    let min = outline.iter().copied().reduce(IVec2::min).unwrap();
    let max = outline.iter().copied().reduce(IVec2::max).unwrap();
    SectorPlan {
        index,
        kind: spec.kind,
        heading: frame.heading,
        outline,
        walls,
        entry: entry_ends,
        gate: gate_ends,
        entry_point: frame.at(entry / 2, 2),
        gate_point: world(&gate_point),
        min,
        max,
        next_frame: Frame {
            origin: gate_ends.0,
            heading: frame.heading.turned(spec.exit),
        },
        next_width: gate,
    }
}

impl SectorPlan {
    fn apart_from(&self, other: &SectorPlan) -> bool {
        self.min.x - SECTOR_GAP >= other.max.x
            || other.min.x - SECTOR_GAP >= self.max.x
            || self.min.y - SECTOR_GAP >= other.max.y
            || other.min.y - SECTOR_GAP >= self.max.y
    }

    /// Whether a point, given in half meters, is strictly inside the outline.
    fn contains_half(&self, point: IVec2) -> bool {
        let mut inside = false;
        let count = self.outline.len();
        for i in 0..count {
            let (a, b) = (self.outline[i] * 2, self.outline[(i + 1) % count] * 2);
            if (a.y > point.y) != (b.y > point.y) {
                // Side of the point relative to the edge, without dividing.
                let cross = (b.x - a.x) as i64 * (point.y - a.y) as i64
                    - (point.x - a.x) as i64 * (b.y - a.y) as i64;
                if (cross > 0) == (b.y > a.y) {
                    inside = !inside;
                }
            }
        }
        inside
    }
}

/// Whether `point` is at least `sqrt(min_2)` away from the segment. Any integer unit works.
fn far_from_segment(point: IVec2, a: IVec2, b: IVec2, min_2: i64) -> bool {
    let (abx, aby) = ((b.x - a.x) as i64, (b.y - a.y) as i64);
    let (apx, apy) = ((point.x - a.x) as i64, (point.y - a.y) as i64);
    let length_2 = abx * abx + aby * aby;
    let along = apx * abx + apy * aby;
    if along <= 0 {
        apx * apx + apy * apy >= min_2
    } else if along >= length_2 {
        let (bpx, bpy) = ((point.x - b.x) as i64, (point.y - b.y) as i64);
        bpx * bpx + bpy * bpy >= min_2
    } else {
        let cross = apx * aby - apy * abx;
        cross * cross >= min_2 * length_2
    }
}

/// Whether `point` is at least `sqrt(min_2)` away from the 2 m square whose lowest corner is
/// `corner`. The point and the distance are in half meters, the corner in meters.
fn far_from_square(point: IVec2, corner: IVec2, min_2: i64) -> bool {
    let (low, high) = (corner * 2, corner * 2 + IVec2::splat(4));
    let gap = (low - point).max(point - high).max(IVec2::ZERO);
    (gap.x as i64).pow(2) + (gap.y as i64).pow(2) >= min_2
}

/// The endless corridor of one seed, planned sector after sector.
#[derive(Clone, Debug)]
pub struct Corridor {
    seed: u64,
    plans: Vec<SectorPlan>,
    maneuvers: u32,
    /// Sectors left before the next arena.
    arena_in: u32,
}

impl Corridor {
    pub fn new(seed: u64) -> Self {
        let frame = Frame {
            origin: IVec2::new(-BAY_WIDTH / 2, 0),
            heading: Heading::North,
        };
        let spec = Spec {
            kind: SectorKind::Bay,
            exit: Exit::Ahead,
            length: (BAY_LENGTH, BAY_LENGTH),
            climb: false,
            drift: 0,
        };
        Self {
            seed,
            plans: vec![plan_sector(seed, 0, frame, BAY_WIDTH, &spec)],
            maneuvers: 0,
            arena_in: Stream::new(seed, Purpose::Maneuver, u32::MAX, 0).range(5, 8) as u32,
        }
    }

    /// The shape of a sector. Sector 0 is the start bay.
    pub fn plan(&mut self, index: u32) -> &SectorPlan {
        while self.plans.len() <= index as usize {
            self.extend();
        }
        &self.plans[index as usize]
    }

    /// Everything inside a sector: obstacles and spawn points.
    pub fn layout(&mut self, index: u32) -> SectorLayout {
        let seed = self.seed;
        SectorLayout::new(seed, self.plan(index).clone())
    }

    /// Plans the next maneuver. Every maneuver starts and ends heading north, north of
    /// everything planned before it.
    fn extend(&mut self) {
        let mut rng = Stream::new(self.seed, Purpose::Maneuver, self.maneuvers, 0);
        self.maneuvers += 1;

        let straight = vec![Spec::corridor(Exit::Ahead, DRIFT)];
        let mut options = Vec::new();
        if self.arena_in == 0 {
            self.arena_in = rng.range(5, 8) as u32;
            options.push(vec![Spec {
                kind: SectorKind::Arena,
                ..straight[0]
            }]);
        } else {
            let (out, back) = if rng.chance(50) {
                (Exit::Right, Exit::Left)
            } else {
                (Exit::Left, Exit::Right)
            };
            let climb = Spec {
                climb: true,
                ..straight[0]
            };
            match rng.below(100) {
                // A jog: sideways for one sector, then north again.
                0..33 => options.push(vec![
                    Spec::corridor(out, DRIFT),
                    Spec::corridor(back, SIDE_DRIFT),
                    climb,
                ]),
                // A switchback: sideways, south, sideways again, then north.
                33..39 if self.plans.len() > 4 => options.push(vec![
                    Spec::corridor(out, DRIFT),
                    Spec {
                        length: (40, 60),
                        ..Spec::corridor(out, 0)
                    },
                    // Short and straight-sided, to stay within the allowed descent.
                    Spec {
                        length: (40, 40),
                        ..Spec::corridor(back, 0)
                    },
                    Spec::corridor(back, 0),
                    climb,
                ]),
                _ => {}
            }
        }
        options.push(straight);

        for specs in options {
            if let Some(mut planned) = self.try_maneuver(&specs) {
                self.arena_in = self.arena_in.saturating_sub(planned.len() as u32);
                self.plans.append(&mut planned);
                return;
            }
        }
        unreachable!("a straight sector heading north always fits");
    }

    fn try_maneuver(&self, specs: &[Spec]) -> Option<Vec<SectorPlan>> {
        let last = self.plans.last().unwrap();
        let mut north = self.plans.iter().map(|plan| plan.max.y).max().unwrap();
        let mut planned: Vec<SectorPlan> = Vec::with_capacity(specs.len());
        for spec in specs {
            let previous = planned.last().unwrap_or(last);
            let (frame, entry) = (previous.next_frame, previous.next_width);
            let mut spec = *spec;
            if spec.climb {
                spec.length.0 = spec.length.0.max(north + SECTOR_GAP - frame.origin.y);
                if spec.length.0 > spec.length.1 {
                    return None;
                }
            }
            let index = (self.plans.len() + planned.len()) as u32;
            let plan = plan_sector(self.seed, index, frame, entry, &spec);

            let earlier = self.plans.iter().chain(&planned);
            let fits = earlier
                .filter(|other| other.index + 1 != plan.index)
                .all(|other| plan.apart_from(other));
            if !fits || plan.min.y < north - MAX_DESCENT {
                return None;
            }
            north = north.max(plan.max.y);
            planned.push(plan);
        }
        Some(planned)
    }
}

/// A sector with everything in it.
#[derive(Clone, Debug)]
pub struct SectorLayout {
    pub plan: SectorPlan,
    /// Lowest corners of the destructible blocks, which are 2 m squares.
    pub blocks: Vec<IVec2>,
    /// Lowest corners of the explosive crates.
    pub crates: Vec<IVec2>,
    /// Where enemies start. A sector uses as many as it has enemies, from the front.
    pub spawns: Vec<IVec2>,
}

impl SectorLayout {
    fn new(seed: u64, plan: SectorPlan) -> Self {
        let mut layout = Self {
            plan,
            blocks: Vec::new(),
            crates: Vec::new(),
            spawns: Vec::new(),
        };
        if layout.plan.kind == SectorKind::Bay {
            return layout;
        }
        let mut rng = Stream::new(seed, Purpose::Obstacles, layout.plan.index, 0);
        let mut groups = layout.place_obstacles(&mut rng);
        // Obstacles go, last placed first, until a wide path leads from the entry to the barrier.
        let reachable = loop {
            layout.blocks = groups
                .iter()
                .filter(|g| !g.0)
                .flat_map(|g| g.1.clone())
                .collect();
            layout.crates = groups
                .iter()
                .filter(|g| g.0)
                .flat_map(|g| g.1.clone())
                .collect();
            let reachable = layout.reachable();
            if reachable.contains(&layout.plan.gate_point) || groups.pop().is_none() {
                break reachable;
            }
        };
        debug_assert!(reachable.contains(&layout.plan.gate_point));
        layout.place_spawns(&mut rng, reachable);
        layout
    }

    /// Draws groups of obstacles on a 2 m grid. The flag of a group tells crates from blocks.
    fn place_obstacles(&self, rng: &mut Stream) -> Vec<(bool, Vec<IVec2>)> {
        let plan = &self.plan;
        let edges = |ends: (IVec2, IVec2)| (ends.0 * 2, ends.1 * 2);
        let mut free: Vec<IVec2> = Vec::new();
        for y in (plan.min.y.div_euclid(2) * 2..plan.max.y).step_by(2) {
            for x in (plan.min.x.div_euclid(2) * 2..plan.max.x).step_by(2) {
                let center = IVec2::new(x, y) * 2 + IVec2::splat(2);
                let clear = plan.contains_half(center)
                    && plan.walls.iter().all(|&wall| {
                        let (a, b) = edges(wall);
                        far_from_segment(center, a, b, OBSTACLE_CLEARANCE_2)
                    })
                    && [plan.entry, plan.gate].into_iter().all(|gate| {
                        let (a, b) = edges(gate);
                        far_from_segment(center, a, b, GATE_CLEARANCE_2)
                    });
                if clear {
                    free.push(IVec2::new(x, y));
                }
            }
        }

        const SHAPES: [&[(i32, i32)]; 6] = [
            &[(0, 0)],
            &[(0, 0), (2, 0)],
            &[(0, 0), (0, 2)],
            &[(0, 0), (2, 0), (0, 2)],
            &[(0, 0), (2, 0), (4, 0)],
            &[(0, 0), (0, 2), (0, 4)],
        ];
        let crates = progression::crate_count(plan.index);
        let crates = rng.range(crates.0, crates.1) as u32;
        let covers = progression::cover_groups(plan.index);
        let mut groups: Vec<(bool, Vec<IVec2>)> = Vec::new();
        for group in 0..covers + crates {
            if free.is_empty() {
                break;
            }
            let is_crate = group >= covers;
            let anchor = free[rng.below(free.len() as u32) as usize];
            let shape = if is_crate {
                SHAPES[0]
            } else {
                rng.pick(&SHAPES)
            };
            let cells: Vec<IVec2> = shape
                .iter()
                .map(|&(dx, dy)| anchor + IVec2::new(dx, dy))
                .collect();
            if !cells.iter().all(|cell| free.contains(cell)) {
                continue;
            }
            // Different groups keep two free cells between them, which a tank can drive through.
            free.retain(|cell| {
                cells
                    .iter()
                    .all(|taken| (*cell - *taken).abs().max_element() > 4)
            });
            groups.push((is_crate, cells));
        }
        groups
    }

    /// Lattice points a tank can reach from the entry along a path more than 3 m wide.
    fn reachable(&self) -> HashSet<IVec2> {
        let plan = &self.plan;
        let passable = |point: IVec2| {
            let half = point * 2;
            plan.contains_half(half)
                && plan
                    .walls
                    .iter()
                    .all(|&(a, b)| far_from_segment(half, a * 2, b * 2, PATH_CLEARANCE_2))
                && self
                    .blocks
                    .iter()
                    .chain(&self.crates)
                    .all(|&corner| far_from_square(half, corner, PATH_CLEARANCE_2))
        };
        let mut reached = HashSet::new();
        let mut queue = VecDeque::new();
        if passable(plan.entry_point) {
            reached.insert(plan.entry_point);
            queue.push_back(plan.entry_point);
        }
        while let Some(point) = queue.pop_front() {
            for step in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
                let next = point + step;
                if !reached.contains(&next) && passable(next) {
                    reached.insert(next);
                    queue.push_back(next);
                }
            }
        }
        reached
    }

    fn place_spawns(&mut self, rng: &mut Stream, reachable: HashSet<IVec2>) {
        let entry = self.plan.entry.0 + self.plan.entry.1;
        let far_2 = (2 * SPAWN_DISTANCE as i64).pow(2);
        let mut candidates: Vec<IVec2> = reachable
            .into_iter()
            .filter(|&point| {
                let offset = point * 2 - entry;
                (offset.x as i64).pow(2) + (offset.y as i64).pow(2) >= far_2
            })
            .collect();
        // A hash set has no order of its own.
        candidates.sort_by_key(|point| (point.y, point.x));

        // Enemies start 4 m apart where there is room, closer where there isn't.
        for spacing in [4, 3, 2] {
            let mut left = candidates.clone();
            left.retain(|point| self.spawns.iter().all(|spawn| spawn != point));
            while self.spawns.len() < MAX_SPAWNS && !left.is_empty() {
                let spawn = left.swap_remove(rng.below(left.len() as u32) as usize);
                let spaced = self
                    .spawns
                    .iter()
                    .all(|other| (*other - spawn).length_squared() >= spacing * spacing);
                if spaced {
                    self.spawns.push(spawn);
                }
            }
        }
    }

    /// A number that changes whenever anything in the layout does.
    pub fn fingerprint(&self) -> u64 {
        let gate = [self.plan.gate.0, self.plan.gate.1];
        let points = self
            .plan
            .outline
            .iter()
            .chain(&gate)
            .chain(&self.blocks)
            .chain(&self.crates)
            .chain(&self.spawns);
        let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
        for value in points.flat_map(|point| [point.x, point.y]) {
            for byte in value.to_le_bytes() {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
            }
        }
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEEDS: [u64; 12] = [0, 1, 2, 3, 7, 42, 1234, 99_999, 0xDEAD_BEEF, 5, 6, u64::MAX];
    const SECTORS: u32 = 80;

    fn corridors() -> impl Iterator<Item = Corridor> {
        SEEDS.into_iter().map(|seed| {
            let mut corridor = Corridor::new(seed);
            corridor.plan(SECTORS);
            corridor
        })
    }

    #[test]
    fn same_seed_same_corridor() {
        let fingerprints = |seed| -> Vec<u64> {
            let mut corridor = Corridor::new(seed);
            (0..40).map(|i| corridor.layout(i).fingerprint()).collect()
        };
        assert_eq!(fingerprints(42), fingerprints(42));
        assert_ne!(fingerprints(42), fingerprints(43));
    }

    #[test]
    fn sectors_can_be_asked_for_in_any_order() {
        let mut forward = Corridor::new(7);
        let mut backward = Corridor::new(7);
        let late = backward.layout(30).fingerprint();
        let early = backward.layout(3).fingerprint();
        assert_eq!(forward.layout(3).fingerprint(), early);
        assert_eq!(forward.layout(30).fingerprint(), late);
    }

    #[test]
    fn generation_is_stable() {
        // Golden value: it changes when a seed stops generating the same map. That is allowed
        // between game versions, but it must never happen by accident.
        let mut corridor = Corridor::new(crate::seed::seed_value("TANKU"));
        let combined = (0..30).fold(0u64, |hash, i| {
            hash.rotate_left(7) ^ corridor.layout(i).fingerprint()
        });
        assert_eq!(combined, 14_109_271_918_055_966_551);
    }

    #[test]
    fn sectors_chain_through_their_barriers() {
        for corridor in corridors() {
            for pair in corridor.plans.windows(2) {
                assert_eq!(pair[0].gate, pair[1].entry, "sector {}", pair[1].index);
                assert_eq!(pair[0].next_frame.heading, pair[1].heading);
            }
        }
    }

    #[test]
    fn sectors_that_are_not_neighbors_stay_apart() {
        for corridor in corridors() {
            for (i, a) in corridor.plans.iter().enumerate() {
                for b in &corridor.plans[..i.saturating_sub(1)] {
                    assert!(a.apart_from(b), "sectors {} and {}", a.index, b.index);
                }
            }
        }
    }

    #[test]
    fn corridor_never_runs_far_south() {
        for corridor in corridors() {
            let mut north = i32::MIN;
            for plan in &corridor.plans {
                assert!(
                    plan.min.y >= north.saturating_sub(MAX_DESCENT),
                    "sector {}",
                    plan.index
                );
                north = north.max(plan.max.y);
            }
        }
    }

    #[test]
    fn corridors_turn_and_open_into_arenas() {
        let (mut sideways, mut south, mut arenas) = (0, 0, 0);
        for corridor in corridors() {
            for plan in &corridor.plans {
                sideways += matches!(plan.heading, Heading::East | Heading::West) as u32;
                south += (plan.heading == Heading::South) as u32;
                arenas += (plan.kind == SectorKind::Arena) as u32;
            }
        }
        let total = SEEDS.len() as u32 * SECTORS;
        assert!(
            sideways > total / 20 && sideways < total / 2,
            "{sideways} sideways"
        );
        assert!(south > 0 && south < total / 20, "{south} south");
        assert!(
            arenas >= total / 10 && arenas <= total / 5,
            "{arenas} arenas"
        );
    }

    #[test]
    fn walls_follow_the_lattice_slopes() {
        for corridor in corridors() {
            for plan in &corridor.plans {
                for &(a, b) in &plan.walls {
                    let d = (b - a).abs();
                    let slope_ok =
                        d.x == 0 || d.y == 0 || d.x == d.y || d.x == 2 * d.y || d.y == 2 * d.x;
                    assert!(slope_ok, "sector {}: wall {a} to {b}", plan.index);
                }
            }
        }
    }

    #[test]
    fn sizes_stay_within_the_specification() {
        for corridor in corridors() {
            for plan in &corridor.plans[1..] {
                let size = plan.max - plan.min;
                match plan.kind {
                    SectorKind::Arena => assert!(size.max_element() <= 36, "arena {}", plan.index),
                    _ => assert!(size.max_element() <= 80, "sector {}", plan.index),
                }
                let gate = (plan.gate.1 - plan.gate.0).abs().max_element();
                let (narrowest, widest) = progression::corridor_width(plan.index + 1);
                assert!(
                    (narrowest..=widest).contains(&gate),
                    "sector {}",
                    plan.index
                );
            }
        }
    }

    #[test]
    fn every_sector_has_a_way_through_and_fair_spawns() {
        for seed in SEEDS {
            let mut corridor = Corridor::new(seed);
            for index in 1..=SECTORS {
                let layout = corridor.layout(index);
                assert!(
                    layout.reachable().contains(&layout.plan.gate_point),
                    "seed {seed}, sector {index}: no way through"
                );
                assert!(
                    layout.spawns.len() >= 9,
                    "seed {seed}, sector {index}: {} spawns",
                    layout.spawns.len()
                );
                let entry = (layout.plan.entry.0 + layout.plan.entry.1).as_vec2() / 2.0;
                for spawn in &layout.spawns {
                    assert!(spawn.as_vec2().distance(entry) >= SPAWN_DISTANCE as f32);
                }
            }
        }
    }

    #[test]
    fn terrain_gets_harsher() {
        let count = |sectors: std::ops::RangeInclusive<u32>| {
            let (mut blocks, mut crates) = (0, 0);
            for seed in SEEDS {
                let mut corridor = Corridor::new(seed);
                for index in sectors.clone() {
                    let layout = corridor.layout(index);
                    blocks += layout.blocks.len();
                    crates += layout.crates.len();
                }
            }
            (blocks, crates)
        };
        let (early, late) = (count(1..=5), count(40..=44));
        assert!(early.0 > late.0, "blocks: {early:?} then {late:?}");
        assert!(early.1 < late.1, "crates: {early:?} then {late:?}");
    }
}

#[cfg(test)]
mod dump {
    use super::*;

    /// Prints a corridor for a look at it: `cargo test -p tanku_core dump -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_corridor() {
        let seed = std::env::var("TANKU_SEED").map_or(42, |text| crate::seed::seed_value(&text));
        let mut corridor = Corridor::new(seed);
        for index in 0..40 {
            let layout = corridor.layout(index);
            let points = |points: &[IVec2]| {
                points
                    .iter()
                    .map(|p| format!("{},{}", p.x, p.y))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            println!("S {:?} {}", layout.plan.kind, points(&layout.plan.outline));
            println!("G {}", points(&[layout.plan.gate.0, layout.plan.gate.1]));
            println!("B {}", points(&layout.blocks));
            println!("C {}", points(&layout.crates));
            println!("E {}", points(&layout.spawns[..layout.spawns.len().min(6)]));
        }
    }
}
