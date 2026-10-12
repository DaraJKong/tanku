//! Generation of the corridor: an endless chain of sectors. And of the room of arena mode,
//! which a run never leaves.
//!
//! Every vertex lies on a 1 m lattice and all arithmetic is on integers, so a seed gives the same
//! map on every platform. North is `+y` and east is `+x`.

use std::collections::{HashSet, VecDeque};

use bevy::math::{IVec2, Vec2};

use crate::{
    balance::BALANCE,
    progression,
    rng::{Purpose, Stream},
};

/// Parts of the corridor that aren't neighbors stay at least this far apart.
pub const SECTOR_GAP: i32 = 6;
/// The corridor never runs further south of its northernmost point than this.
pub const MAX_DESCENT: i32 = 60;
/// Enemies are placed at least this far from the entry of their sector.
pub const SPAWN_DISTANCE: i32 = BALANCE.terrain.spawn.0;
/// Enemies are placed at least this far from the barrier of their sector.
pub const SPAWN_CLEARANCE: i32 = BALANCE.terrain.spawn.1;
/// An arena comes every so many sectors: the fewest and the most.
const ARENA_EVERY: (i32, i32) = BALANCE.arena.every;
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
/// An arena is deep enough for a group to start far from the entry and clear of the barrier.
const ARENA_MIN_LENGTH: i32 = 32;
/// An arena that is cleared leaves a life this far inside its barrier.
const LIFE_DEPTH: i32 = BALANCE.arena.life;

/// Squared distances below are in half meters, so that the centers of blocks stay integers.
/// 1.8 m: a lattice path with this clearance is more than 3 m wide everywhere.
const PATH_CLEARANCE_2: i64 = 13;
/// 2.5 m between the center of an obstacle and a wall.
const OBSTACLE_CLEARANCE_2: i64 = 25;
/// 7 m between the center of an obstacle and a gate.
const GATE_CLEARANCE_2: i64 = 196;

/// Narrowest and widest the room of arena mode gets, and how deep it gets from south to
/// north. With the tanks that wait in reserve south of it, it fits on one screen.
const ROOM_WIDTH: (i32, i32) = BALANCE.room.width;
const ROOM_DEPTH: (i32, i32) = BALANCE.room.depth;
/// How far a corner of the room is cut: along the wall that runs east, and along the one
/// that runs north. The cuts follow the slopes of the lattice.
const ROOM_CORNERS: [(i32, i32); 9] = [
    (0, 0),
    (2, 2),
    (3, 3),
    (4, 4),
    (5, 5),
    (4, 2),
    (6, 3),
    (2, 4),
    (3, 6),
];
/// 5 m between the center of an obstacle and the place where the players appear in the room.
const START_CLEARANCE_2: i64 = 100;
/// The room has more floor than a sector of the corridor: its groups of cover are this many
/// percent of those of a sector.
const ROOM_COVER: u32 = BALANCE.room.cover;
/// How far outside the south wall of the room the first row of the tanks that wait in
/// reserve stands, how far behind it the next row does, and how far apart the tanks of a row
/// are.
const RESERVE_ROW: f32 = 2.2;
const RESERVE_ROWS_APART: f32 = 2.6;
const RESERVE_SPACING: f32 = 3.0;
/// The most tanks that a row of the reserve holds, and how far apart its two ends are at
/// most: it stands between the two panels at the bottom of the display.
const RESERVE_ROW_MOST: u32 = 8;
const RESERVE_SPAN: f32 = 20.0;
/// How far south of the room the ground that one row of the reserve takes reaches, and the
/// ground that two rows take.
const RESERVE_DEPTH: (f32, f32) = (4.2, 6.5);

/// How far south of the room of arena mode the ground that the tanks in reserve take
/// reaches. The largest group of a single player stands in one row. Several players call
/// for groups that take two.
pub const fn reserve_depth(solo: bool) -> f32 {
    if solo {
        RESERVE_DEPTH.0
    } else {
        RESERVE_DEPTH.1
    }
}

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
    /// The room of arena mode: it has no way in and no way out.
    Room,
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
    /// Where a life lies once the sector is cleared, if it is an arena: in front of the middle
    /// of the barrier.
    pub life_point: IVec2,
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
            length: BALANCE.corridor,
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
    /// A shallow wall, 2 m sideways per meter forward, then straight.
    Ramp(i32),
    /// A wall that slants over the whole step, at 45 degrees or steeper.
    Slant(i32),
}

impl Change {
    fn shift(self) -> i32 {
        match self {
            Change::None => 0,
            Change::Step(du) | Change::Ramp(du) | Change::Slant(du) => du,
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
        if !self.rng.chance(self.angled) {
            return Change::Step(sign * self.rng.range(2, 4));
        }
        // The three slopes of the lattice. At the shallowest one, a wall that slanted over a
        // whole step would shift by 8 m or more, which is more than the width of a corridor
        // may vary. A ramp therefore stops after 1 to 3 m.
        match self.rng.below(3) {
            0 => Change::Slant(sign * dv / 2),
            1 => Change::Slant(sign * dv),
            _ => Change::Ramp(sign * 2 * self.rng.range(1, 3)),
        }
    }

    fn fits(&self, left: Change, right: Change, dv: i32) -> bool {
        let (l, r) = (self.l + left.shift(), self.r + right.shift());
        // A step moves its wall at once, a ramp within 3 m and a slant little by little, so the
        // width is checked where the changes start as well as where they end.
        let stepped = |u: i32, change: Change| match change {
            Change::Step(du) | Change::Ramp(du) => u + du,
            _ => u,
        };
        let (l_start, r_start) = (stepped(self.l, left), stepped(self.r, right));
        let fitting = |l: i32, r: i32| (self.width.0..=self.width.1).contains(&(r - l));
        // The corner of a step or a ramp also faces the other wall as it was just before.
        let open = |l: i32, r: i32| r - l >= self.width.0;
        open(l_start, self.r)
            && open(self.l, r_start)
            && fitting(l_start, r_start)
            && fitting(l, r)
            && l >= self.reach.0
            && r <= self.reach.1
            && self.apart(
                &Self::corners(self.l, self.v, dv, left),
                &Self::corners(self.r, self.v, dv, right),
            )
    }

    /// Whether the two walls stay apart once these corners are added to them.
    ///
    /// Widths are measured straight across, but a wall that has moved away leaves a corner
    /// behind, and the other wall may come in beside it a few meters further on. The walls
    /// keep the distance of two walls at 45 degrees in the narrowest corridor of the sector.
    fn apart(&self, new_left: &[(i32, i32)], new_right: &[(i32, i32)]) -> bool {
        // In half meters, so that the squared distance stays an integer.
        let gap_2 = 2 * (self.width.0 as i64).pow(2);
        let wall = |old: &[(i32, i32)], new: &[(i32, i32)]| -> Vec<IVec2> {
            let corners = old.iter().chain(new);
            corners.map(|&(u, v)| IVec2::new(u, v) * 2).collect()
        };
        let sides = [
            (wall(&self.left, new_left), self.left.len()),
            (wall(&self.right, new_right), self.right.len()),
        ];
        // The closest points of two walls include a corner of one of them. What was there
        // before this step was checked when it was added.
        [(0, 1), (1, 0)].into_iter().all(|(a, b)| {
            let ((wall, old_wall), (corners, old_corners)) = (&sides[a], &sides[b]);
            wall.windows(2).enumerate().all(|(i, ends)| {
                corners.iter().enumerate().all(|(j, &corner)| {
                    (i + 1 < *old_wall && j < *old_corners)
                        || far_from_segment(corner, ends[0], ends[1], gap_2)
                })
            })
        })
    }

    /// Moves forward by `dv`, with a random change on each wall that may move.
    fn advance(&mut self, dv: i32) {
        let mut changes = (Change::None, Change::None);
        for _ in 0..4 {
            let left = self.draw_change(self.moves.0, dv);
            let right = self.draw_change(self.moves.1, dv);
            if self.fits(left, right, dv) {
                changes = (left, right);
                break;
            }
        }
        self.take(changes.0, changes.1, dv);
    }

    /// Moves forward by `dv` with these changes.
    fn take(&mut self, left: Change, right: Change, dv: i32) {
        self.left.extend(Self::corners(self.l, self.v, dv, left));
        self.right.extend(Self::corners(self.r, self.v, dv, right));
        self.l += left.shift();
        self.r += right.shift();
        self.v += dv;
    }

    /// The corners that a change adds to a wall that is at `u` where the step starts, at `v`.
    fn corners(u: i32, v: i32, dv: i32, change: Change) -> Vec<(i32, i32)> {
        let end = u + change.shift();
        let mut corners = Vec::with_capacity(2);
        match change {
            Change::Step(_) => corners.push((end, v)),
            Change::Ramp(du) => {
                let run = du.abs() / 2;
                debug_assert!(run < dv, "a ramp ends before its step does");
                corners.push((end, v + run));
            }
            Change::None | Change::Slant(_) => {}
        }
        corners.push((end, v + dv));
        corners
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
    let ends = 4 + flare_left.max(flare_right) + taper_left.max(taper_right);
    let length = (ends + rng.range(8, 14)).max(ARENA_MIN_LENGTH);

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
        SectorKind::Room => unreachable!("a corridor has no room"),
    };
    let (length, gate) = (shape.length, shape.gate);
    let (left_end, right_end) = (shape.left.last().unwrap().0, shape.right.last().unwrap().0);

    // Local ends of the barrier, the left end of the next entry first.
    let (gate_from, gate_to) = match spec.exit {
        Exit::Ahead => ((left_end, length), (right_end, length)),
        Exit::Right => ((right_end, length), (right_end, length - gate)),
        Exit::Left => ((left_end, length - gate), (left_end, length)),
    };
    // A point in front of the middle of the barrier, this far inside the sector.
    let inside = |depth: i32| match spec.exit {
        Exit::Ahead => ((left_end + right_end) / 2, length - depth),
        Exit::Right => (right_end - depth, length - gate / 2),
        Exit::Left => (left_end + depth, length - gate / 2),
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
        gate_point: world(&inside(2)),
        life_point: world(&inside(LIFE_DEPTH)),
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
            // The streams of the maneuvers have 0 as their second number.
            arena_in: Stream::new(seed, Purpose::Maneuver, 0, 1).range(ARENA_EVERY.0, ARENA_EVERY.1)
                as u32,
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
            self.arena_in = rng.range(ARENA_EVERY.0, ARENA_EVERY.1) as u32;
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

    /// Draws the groups of obstacles of a sector of the corridor: clear of its walls, and well
    /// clear of its entry and of its barrier.
    fn place_obstacles(&self, rng: &mut Stream) -> Vec<(bool, Vec<IVec2>)> {
        let plan = &self.plan;
        let open = [
            (plan.entry, GATE_CLEARANCE_2),
            (plan.gate, GATE_CLEARANCE_2),
        ];
        let counts = (
            progression::cover_groups(plan.index),
            progression::crate_count(plan.index),
        );
        scatter(plan, rng, &open, counts)
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
        // Distances are measured to the whole width of the entry and of the barrier. A point of
        // the lattice can lie on either line, and a tank that starts there is on neither side.
        let (entry, gate) = (self.plan.entry, self.plan.gate);
        let far_2 = (2 * SPAWN_DISTANCE as i64).pow(2);
        let clear_2 = (2 * SPAWN_CLEARANCE as i64).pow(2);
        let mut candidates: Vec<IVec2> = reachable
            .into_iter()
            .filter(|&point| {
                let half = point * 2;
                far_from_segment(half, entry.0 * 2, entry.1 * 2, far_2)
                    && far_from_segment(half, gate.0 * 2, gate.1 * 2, clear_2)
            })
            .collect();
        // A hash set has no order of its own.
        candidates.sort_by_key(|point| (point.y, point.x));
        self.spawns = spaced(rng, &candidates);
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

/// Draws groups of obstacles on a 2 m grid inside a plan: this many groups of blocks, then
/// between this few and this many crates. They keep clear of the walls, and of each line that
/// must stay open by the distance that goes with it, squared and in half meters. The flag of
/// a group tells crates from blocks.
fn scatter(
    plan: &SectorPlan,
    rng: &mut Stream,
    open: &[((IVec2, IVec2), i64)],
    (covers, crates): (u32, (i32, i32)),
) -> Vec<(bool, Vec<IVec2>)> {
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
                && open.iter().all(|&(line, clearance_2)| {
                    let (a, b) = edges(line);
                    far_from_segment(center, a, b, clearance_2)
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
    let crates = rng.range(crates.0, crates.1) as u32;
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

/// Picks places for tanks among these: 4 m apart where there is room, closer where there
/// isn't, and no more than a sector prepares.
fn spaced(rng: &mut Stream, candidates: &[IVec2]) -> Vec<IVec2> {
    let mut picked: Vec<IVec2> = Vec::new();
    for spacing in [4, 3, 2] {
        let mut left = candidates.to_vec();
        left.retain(|point| picked.iter().all(|taken| taken != point));
        while picked.len() < MAX_SPAWNS && !left.is_empty() {
            let place = left.swap_remove(rng.below(left.len() as u32) as usize);
            let apart = picked
                .iter()
                .all(|other| (*other - place).length_squared() >= spacing * spacing);
            if apart {
                picked.push(place);
            }
        }
    }
    picked
}

/// The room of arena mode: a closed room that a run never leaves. Its walls come from the
/// seed. What stands in it is laid out for a level of the terrain, and the places where tanks
/// appear are drawn for each group.
#[derive(Clone, Debug)]
pub struct Room {
    seed: u64,
    plan: SectorPlan,
}

impl Room {
    pub fn new(seed: u64) -> Self {
        let mut rng = Stream::new(seed, Purpose::Room, 0, 0);
        // Even sizes keep the middle of the room on the lattice.
        let width = rng.range(ROOM_WIDTH.0 / 2, ROOM_WIDTH.1 / 2) * 2;
        let depth = rng.range(ROOM_DEPTH.0 / 2, ROOM_DEPTH.1 / 2) * 2;
        let (west, east) = (-width / 2, width / 2);
        let [sw, nw, ne, se] = [(); 4].map(|_| rng.pick(&ROOM_CORNERS));
        // One time in two, a stretch of a long wall stands out into the room between two
        // slanted sides: where it is along the wall, and how far out. The cuts of the corners
        // at both ends of the wall leave it room.
        let mut buttress = |left: i32, right: i32| -> Vec<(i32, i32)> {
            if !rng.chance(50) {
                return Vec::new();
            }
            let (span, out) = (rng.pick(&[6, 8, 10]), rng.pick(&[2, 3]));
            let from = rng.range(west + left + 3, east - right - 3 - span);
            vec![
                (from, 0),
                (from + out, out),
                (from + span - out, out),
                (from + span, 0),
            ]
        };
        let (north, south) = (buttress(nw.0, ne.0), buttress(sw.0, se.0));

        // The outline starts at the south end of the west wall and runs north, as the outline
        // of a sector does from the left end of its entry.
        let mut corners = vec![(west, sw.1), (west, depth - nw.1), (west + nw.0, depth)];
        corners.extend(north.iter().map(|&(x, out)| (x, depth - out)));
        corners.extend([
            (east - ne.0, depth),
            (east, depth - ne.1),
            (east, se.1),
            (east - se.0, 0),
        ]);
        corners.extend(south.iter().rev());
        corners.push((west + sw.0, 0));
        let corners: Vec<IVec2> = corners.iter().map(|&(x, y)| IVec2::new(x, y)).collect();
        let outline = simplify(&corners, &[]);
        let walls = (0..outline.len())
            .map(|i| (outline[i], outline[(i + 1) % outline.len()]))
            .collect();

        let start = IVec2::new(0, depth / 2);
        let plan = SectorPlan {
            index: 0,
            kind: SectorKind::Room,
            heading: Heading::North,
            outline,
            walls,
            // The room has neither: what is measured from them is measured from its middle.
            entry: (start, start),
            gate: (start, start),
            entry_point: start,
            gate_point: start,
            life_point: start,
            min: IVec2::new(west, 0),
            max: IVec2::new(east, depth),
            next_frame: Frame {
                origin: start,
                heading: Heading::North,
            },
            next_width: 0,
        };
        Self { seed, plan }
    }

    /// The shape of the room, which goes by the number 0. Its walls go all the way around.
    pub fn plan(&self) -> &SectorPlan {
        &self.plan
    }

    /// The room with nothing in it: what stands there comes and goes during a run.
    pub fn shell(&self) -> SectorLayout {
        SectorLayout {
            plan: self.plan.clone(),
            blocks: Vec::new(),
            crates: Vec::new(),
            spawns: Vec::new(),
        }
    }

    /// Where the players appear: the middle of the room.
    pub fn start(&self) -> IVec2 {
        self.plan.entry_point
    }

    /// The blocks and the crates that stand in the room at a level of the terrain, by their
    /// lowest corners. A level is as harsh as the sector of that number: cover gets sparse and
    /// crates plentiful. Nothing stands where the players appear.
    pub fn obstacles(&self, level: u32) -> (Vec<IVec2>, Vec<IVec2>) {
        let mut rng = Stream::new(self.seed, Purpose::Room, level, 1);
        let start = self.start();
        let counts = (
            progression::cover_groups(level) * ROOM_COVER / 100,
            progression::crate_count(level),
        );
        let open = [((start, start), START_CLEARANCE_2)];
        let groups = scatter(&self.plan, &mut rng, &open, counts);
        let of = |crates: bool| -> Vec<IVec2> {
            let kind = groups.iter().filter(|group| group.0 == crates);
            kind.flat_map(|group| group.1.clone()).collect()
        };
        (of(false), of(true))
    }

    /// Where the tanks of a group appear, given what stands in the room: places that a tank
    /// reaches from the middle of the room along a path more than 3 m wide. They are drawn
    /// anew for each group, and a group uses as many as it has tanks, from the front.
    pub fn places(&self, group: u32, blocks: &[IVec2], crates: &[IVec2]) -> Vec<IVec2> {
        let layout = SectorLayout {
            blocks: blocks.to_vec(),
            crates: crates.to_vec(),
            ..self.shell()
        };
        let mut candidates: Vec<IVec2> = layout.reachable().into_iter().collect();
        // A hash set has no order of its own.
        candidates.sort_by_key(|point| (point.y, point.x));
        let mut rng = Stream::new(self.seed, Purpose::Room, group, 2);
        spaced(&mut rng, &candidates)
    }

    /// Where each tank of a group waits in reserve, by its number, for a group whose parts
    /// have this many tanks: those that come with it, then those that each wait for one to be
    /// destroyed. They stand outside the south wall, where the players read the tanks that
    /// have not come yet. A small group stands in one row, in the order it comes, with a gap
    /// between its parts. A larger one stands a part to a row, the first part nearest the
    /// wall, and a part that is too large for a row takes the next one too.
    pub fn reserve(&self, parts: &[u32]) -> Vec<Vec2> {
        let (min, max) = (self.plan.min.as_vec2(), self.plan.max.as_vec2());
        // The place of a tank in a row of this many places.
        let place = |row: u32, place: u32, places: u32| {
            let along = place as f32 - (places - 1) as f32 / 2.0;
            let spacing = RESERVE_SPACING.min(RESERVE_SPAN / (places.max(2) - 1) as f32);
            let behind = RESERVE_ROW + row as f32 * RESERVE_ROWS_APART;
            Vec2::new((min.x + max.x) / 2.0 + along * spacing, min.y - behind)
        };
        let tanks: u32 = parts.iter().sum();
        let gaps = parts.len().saturating_sub(1) as u32;
        let mut places = Vec::new();
        if tanks + gaps <= RESERVE_ROW_MOST {
            let mut next = 0;
            for &part in parts {
                places.extend((next..next + part).map(|at| place(0, at, tanks + gaps)));
                next += part + 1;
            }
            return places;
        }
        let mut row = 0;
        for &part in parts {
            let mut left = part;
            while left > 0 {
                let here = left.min(RESERVE_ROW_MOST);
                places.extend((0..here).map(|at| place(row, at, here)));
                left -= here;
                row += 1;
            }
        }
        places
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
        assert_eq!(combined, 12_171_179_429_706_845_317);
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
    fn corridors_use_every_slope() {
        // Slanted walls by their slope in the frame of their sector: sideways, then forward.
        let (mut steep, mut diagonal, mut shallow) = (0, 0, 0);
        for corridor in corridors() {
            for plan in &corridor.plans {
                if plan.kind != SectorKind::Corridor {
                    continue;
                }
                for &(a, b) in &plan.walls {
                    let across = (b - a).dot(plan.heading.right()).abs();
                    let along = (b - a).dot(plan.heading.forward()).abs();
                    steep += (across > 0 && 2 * across == along) as u32;
                    diagonal += (across > 0 && across == along) as u32;
                    shallow += (along > 0 && across == 2 * along) as u32;
                }
            }
        }
        let slanted = steep + diagonal + shallow;
        for (count, slope) in [(steep, "1:2"), (diagonal, "1:1"), (shallow, "2:1")] {
            assert!(
                count * 10 >= slanted,
                "{count} walls of {slanted} at {slope}"
            );
        }
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

    /// Whether the walls before the barrier of a sector, along its outline, and the walls after
    /// it stay 5.66 m apart. The path check needs a little less: a lattice path that keeps
    /// 1.8 m from both sides exists wherever they are 5 m apart.
    fn sides_stay_apart(plan: &SectorPlan) -> bool {
        let corner = |wanted: IVec2| plan.outline.iter().position(|&point| point == wanted);
        let (before, after) = (corner(plan.gate.0).unwrap(), corner(plan.gate.1).unwrap());
        assert_eq!(before + 1, after, "sector {}", plan.index);
        let sides = [&plan.outline[..=before], &plan.outline[after..]];
        [(0, 1), (1, 0)].into_iter().all(|(a, b)| {
            sides[a].windows(2).all(|ends| {
                sides[b]
                    .iter()
                    .all(|&corner| far_from_segment(corner * 2, ends[0] * 2, ends[1] * 2, 128))
            })
        })
    }

    #[test]
    fn a_life_lies_in_the_open_in_front_of_its_barrier() {
        for corridor in corridors() {
            for plan in &corridor.plans[1..] {
                let at = format!("sector {}", plan.index);
                let life = plan.life_point;
                assert!(plan.contains_half(life * 2), "{at}");
                let (from, to) = (plan.gate.0.as_vec2(), plan.gate.1.as_vec2());
                let to_barrier = crate::geom::distance_to_segment(life.as_vec2(), from, to);
                assert_eq!(to_barrier, LIFE_DEPTH as f32, "{at}");
                for &(a, b) in &plan.walls {
                    let to_wall =
                        crate::geom::distance_to_segment(life.as_vec2(), a.as_vec2(), b.as_vec2());
                    assert!(to_wall >= 3.5, "{at}: {to_wall} m from a wall");
                }
            }
        }
    }

    #[test]
    fn the_two_sides_of_a_sector_stay_apart() {
        for corridor in corridors() {
            for plan in &corridor.plans {
                assert!(sides_stay_apart(plan), "sector {}", plan.index);
            }
        }
    }

    /// The same over six thousand seeds, which takes ten seconds. It runs with the other wide
    /// check: `cargo test -p tanku_core many_seeds -- --ignored`.
    #[test]
    #[ignore]
    fn sides_stay_apart_for_many_seeds() {
        for seed in 2000..8000 {
            let mut corridor = Corridor::new(seed);
            corridor.plan(SECTORS);
            for plan in &corridor.plans {
                assert!(sides_stay_apart(plan), "seed {seed}, sector {}", plan.index);
            }
        }
    }

    /// Checks what the generator promises about the inside of one sector.
    fn check_layout(seed: u64, layout: &SectorLayout) {
        let plan = &layout.plan;
        let at = format!("seed {seed}, sector {}", plan.index);
        assert!(
            layout.reachable().contains(&plan.gate_point),
            "{at}: no way through"
        );
        let arena = plan.kind == SectorKind::Arena;
        // There is room for the group that two players call for. The smallest arenas are too
        // small for the whole group of three or four players, by a few tanks: they hold as
        // many as fit. That happens to about one arena in a thousand with four players.
        let needed = progression::enemy_count(plan.index, 2, arena) as usize;
        assert!(
            layout.spawns.len() >= needed,
            "{at}: {} spawns for {needed} enemies",
            layout.spawns.len()
        );

        let distance = |point: IVec2, ends: (IVec2, IVec2)| {
            crate::geom::distance_to_segment(point.as_vec2(), ends.0.as_vec2(), ends.1.as_vec2())
        };
        for (number, &spawn) in layout.spawns.iter().enumerate() {
            assert!(plan.contains_half(spawn * 2), "{at}: {spawn} is outside");
            assert!(
                distance(spawn, plan.entry) >= SPAWN_DISTANCE as f32,
                "{at}: {spawn} is close to the entry"
            );
            // A tank that starts on the line of its barrier can end up behind it, out of reach.
            // Seed 85GVTHQA had one in sector 7, whose barrier is on a side wall.
            assert!(
                distance(spawn, plan.gate) >= SPAWN_CLEARANCE as f32,
                "{at}: {spawn} is close to the barrier"
            );
            for &wall in &plan.walls {
                assert!(distance(spawn, wall) >= 1.8, "{at}: {spawn} is in a wall");
            }
            for &corner in layout.blocks.iter().chain(&layout.crates) {
                assert!(
                    far_from_square(spawn * 2, corner, PATH_CLEARANCE_2),
                    "{at}: {spawn} is in an obstacle"
                );
            }
            // Two tanks side by side need 1.8 m.
            for &other in &layout.spawns[..number] {
                assert!(
                    (other - spawn).length_squared() >= 4,
                    "{at}: {spawn} overlaps {other}"
                );
            }
        }
    }

    #[test]
    fn every_sector_has_a_way_through_and_fair_spawns() {
        for seed in SEEDS {
            let mut corridor = Corridor::new(seed);
            for index in 1..=SECTORS {
                check_layout(seed, &corridor.layout(index));
            }
        }
    }

    /// The same over many seeds, which takes a few minutes:
    /// `cargo test -p tanku_core many_seeds -- --ignored`.
    #[test]
    #[ignore]
    fn guarantees_hold_for_many_seeds() {
        for seed in 1000..2000 {
            let mut corridor = Corridor::new(seed);
            for index in 1..=SECTORS {
                check_layout(seed, &corridor.layout(index));
            }
            let mut north = i32::MIN;
            for (i, plan) in corridor.plans.iter().enumerate() {
                for other in &corridor.plans[..i.saturating_sub(1)] {
                    assert!(plan.apart_from(other), "seed {seed}, sector {i}");
                }
                assert!(
                    plan.min.y >= north.saturating_sub(MAX_DESCENT),
                    "seed {seed}, sector {i}"
                );
                north = north.max(plan.max.y);
            }
        }
    }

    #[test]
    fn a_step_never_pinches_the_corridor() {
        // Sector 57 is 8 to 14 m wide. A wall that steps 4 m inward while the opposite wall
        // starts to slant away leaves 6 m where the step is, whatever the width further on.
        let mut rng = Stream::new(0, Purpose::Shape, 0, 0);
        let walk = Walk::new(&mut rng, 10, 57, &Spec::corridor(Exit::Ahead, DRIFT));
        assert!(walk.fits(Change::None, Change::Slant(4), 4));
        assert!(walk.fits(Change::Step(2), Change::Slant(4), 4));
        assert!(!walk.fits(Change::Step(4), Change::Slant(4), 4));
        assert!(!walk.fits(Change::Slant(-4), Change::Step(-4), 4));
        // A ramp is as sudden as a step.
        assert!(walk.fits(Change::Ramp(2), Change::Slant(4), 4));
        assert!(!walk.fits(Change::Ramp(4), Change::Slant(4), 4));
        assert!(!walk.fits(Change::Ramp(4), Change::None, 4));
        assert!(walk.fits(Change::Ramp(-4), Change::None, 4));
    }

    #[test]
    fn a_wall_keeps_clear_of_the_corner_the_other_one_left_behind() {
        // In a corridor 8 m wide, the left wall falls back by 6 m and leaves its old corner
        // where it was. Four meters further on, the right wall is 12 m from the left one and
        // could come in by 4 m, but that would bring it within 4.5 m of the corner.
        let mut rng = Stream::new(0, Purpose::Shape, 0, 0);
        let mut walk = Walk::new(&mut rng, 8, 57, &Spec::corridor(Exit::Ahead, DRIFT));
        assert!(walk.fits(Change::Ramp(-6), Change::Slant(-2), 4));
        walk.take(Change::Ramp(-6), Change::Slant(-2), 4);
        assert!(walk.fits(Change::None, Change::Step(-2), 4));
        assert!(!walk.fits(Change::None, Change::Step(-4), 4));
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
mod room_tests {
    use super::*;

    const SEEDS: [u64; 12] = [0, 1, 2, 3, 7, 42, 1234, 99_999, 0xDEAD_BEEF, 5, 6, u64::MAX];
    /// The terrain is as harsh as it gets from level 30 on. The groups go on without end.
    const LEVELS: u32 = 40;
    /// The largest group there is: the one of four players in a late sector.
    const MOST_TANKS: usize = 15;

    fn distance(point: Vec2, ends: (IVec2, IVec2)) -> f32 {
        crate::geom::distance_to_segment(point, ends.0.as_vec2(), ends.1.as_vec2())
    }

    /// Checks what the generator promises about the walls of a room.
    fn check_walls(seed: u64, room: &Room) {
        let plan = room.plan();
        let at = format!("seed {seed}");
        let size = plan.max - plan.min;
        assert!(
            (ROOM_WIDTH.0..=ROOM_WIDTH.1).contains(&size.x),
            "{at}: {size}"
        );
        assert!(
            (ROOM_DEPTH.0..=ROOM_DEPTH.1).contains(&size.y),
            "{at}: {size}"
        );
        assert_eq!((plan.kind, plan.index), (SectorKind::Room, 0), "{at}");
        // The walls go all the way around, on the slopes of the lattice.
        assert_eq!(plan.walls.len(), plan.outline.len(), "{at}");
        for &(a, b) in &plan.walls {
            let d = (b - a).abs();
            let on_a_slope = d.x == 0 || d.y == 0 || d.x == d.y || d.x == 2 * d.y || d.y == 2 * d.x;
            assert!(on_a_slope && d != IVec2::ZERO, "{at}: wall {a} to {b}");
        }
        // No wall crosses another: the room is one piece of floor. Two walls in a row share a
        // corner and no more.
        for (i, &(a, b)) in plan.walls.iter().enumerate() {
            for &(c, d) in &plan.walls[..i] {
                if a == c || a == d || b == c || b == d {
                    continue;
                }
                let [a, b, c, d] = [a, b, c, d].map(|corner| corner.as_vec2());
                let crossing = crate::geom::segments_intersect(a, b, c, d);
                assert!(!crossing, "{at}: {a}-{b} crosses {c}-{d}");
            }
        }
        // The outline runs clockwise seen from above, as the outline of a sector does: its
        // signed area is negative.
        let count = plan.outline.len();
        let doubled: i32 = (0..count)
            .map(|i| plan.outline[i].perp_dot(plan.outline[(i + 1) % count]))
            .sum();
        assert!(doubled < 0, "{at}: {doubled}");
        // The players appear in the open.
        let start = room.start();
        assert!(plan.contains_half(start * 2), "{at}");
        for &wall in &plan.walls {
            assert!(distance(start.as_vec2(), wall) >= 7.0, "{at}");
        }
    }

    /// Checks what the generator promises about what stands in a room at a level, and about
    /// the places where the tanks of a group appear.
    fn check_inside(seed: u64, room: &Room, level: u32) {
        let plan = room.plan();
        let at = format!("seed {seed}, level {level}");
        let (blocks, crates) = room.obstacles(level);
        let start = room.start().as_vec2();
        let every: Vec<IVec2> = blocks.iter().chain(&crates).copied().collect();
        for (number, &corner) in every.iter().enumerate() {
            let center = corner.as_vec2() + Vec2::splat(1.0);
            assert!(plan.contains_half(corner * 2 + IVec2::splat(2)), "{at}");
            for &wall in &plan.walls {
                assert!(distance(center, wall) >= 2.5, "{at}: {corner} is at a wall");
            }
            assert!(
                center.distance(start) >= 5.0,
                "{at}: {corner} is in the way"
            );
            assert!(!every[..number].contains(&corner), "{at}: {corner} twice");
        }

        let places = room.places(level, &blocks, &crates);
        assert!(places.len() >= MOST_TANKS, "{at}: {} places", places.len());
        for (number, &place) in places.iter().enumerate() {
            assert!(plan.contains_half(place * 2), "{at}: {place} is outside");
            for &wall in &plan.walls {
                assert!(distance(place.as_vec2(), wall) >= 1.8, "{at}: {place}");
            }
            for &corner in &every {
                assert!(
                    far_from_square(place * 2, corner, PATH_CLEARANCE_2),
                    "{at}: {place} is in an obstacle"
                );
            }
            // Two tanks side by side need 1.8 m.
            for &other in &places[..number] {
                assert!((other - place).length_squared() >= 4, "{at}: {place}");
            }
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_room() {
        let fingerprint = |seed: u64| -> Vec<IVec2> {
            let room = Room::new(seed);
            let mut points = room.plan().outline.clone();
            for level in 1..=10 {
                let (blocks, crates) = room.obstacles(level);
                points.extend(room.places(level, &blocks, &crates));
                points.extend(blocks);
                points.extend(crates);
            }
            points
        };
        assert_eq!(fingerprint(42), fingerprint(42));
        assert_ne!(fingerprint(42), fingerprint(43));
    }

    #[test]
    fn room_generation_is_stable() {
        // Golden value: it changes when a seed stops generating the same room. That is
        // allowed between game versions, but it must never happen by accident.
        let room = Room::new(crate::seed::seed_value("TANKU"));
        let combined = (1..=10).fold(0u64, |hash, level| {
            let (blocks, crates) = room.obstacles(level);
            let layout = SectorLayout {
                spawns: room.places(level, &blocks, &crates),
                blocks,
                crates,
                ..room.shell()
            };
            hash.rotate_left(7) ^ layout.fingerprint()
        });
        assert_eq!(combined, 16_431_478_961_235_884_410);
    }

    #[test]
    fn a_room_is_closed_and_its_walls_follow_the_lattice() {
        let mut shapes = HashSet::new();
        for seed in SEEDS {
            let room = Room::new(seed);
            check_walls(seed, &room);
            shapes.insert(room.plan().outline.clone());
            // The shell is the room with nothing in it.
            let shell = room.shell();
            assert_eq!(shell.plan.outline, room.plan().outline);
            assert!(shell.blocks.is_empty() && shell.crates.is_empty());
        }
        // Seeds give rooms of their own.
        assert!(shapes.len() >= SEEDS.len() - 1, "{} shapes", shapes.len());
    }

    #[test]
    fn what_stands_in_a_room_leaves_room_for_the_players_and_for_every_group() {
        for seed in SEEDS {
            let room = Room::new(seed);
            for level in 1..=LEVELS {
                check_inside(seed, &room, level);
            }
        }
    }

    /// The same over many seeds, which takes a while:
    /// `cargo test -p tanku_core many_seeds -- --ignored`.
    #[test]
    #[ignore]
    fn rooms_hold_for_many_seeds() {
        for seed in 1000..3000 {
            let room = Room::new(seed);
            check_walls(seed, &room);
            for level in [1, 2, 5, 9, 14, 20, 29, 30, 31, 55] {
                check_inside(seed, &room, level);
            }
        }
    }

    #[test]
    fn a_room_is_laid_out_anew_for_each_group_and_gets_harsher() {
        let (mut early, mut late) = ((0, 0), (0, 0));
        for seed in SEEDS {
            let room = Room::new(seed);
            let (first, second) = (room.obstacles(1), room.obstacles(2));
            assert_ne!(first, second, "seed {seed}");
            // The places of a group are its own, whatever stands in the room.
            let places = |group: u32| room.places(group, &first.0, &first.1);
            assert_ne!(places(1), places(2), "seed {seed}");
            assert_eq!(places(1), places(1), "seed {seed}");
            for level in 1..=5 {
                let (blocks, crates) = room.obstacles(level);
                early = (early.0 + blocks.len(), early.1 + crates.len());
                let (blocks, crates) = room.obstacles(level + 39);
                late = (late.0 + blocks.len(), late.1 + crates.len());
            }
        }
        assert!(early.0 > late.0, "blocks: {early:?} then {late:?}");
        assert!(early.1 < late.1, "crates: {early:?} then {late:?}");
    }

    #[test]
    fn the_reserve_stands_in_rows_outside_the_south_wall() {
        for seed in SEEDS {
            let room = Room::new(seed);
            let (min, max) = (room.plan().min.as_vec2(), room.plan().max.as_vec2());
            let middle = (min.x + max.x) / 2.0;
            let rows = |parts: &[u32]| -> Vec<f32> {
                let mut rows: Vec<f32> = room.reserve(parts).iter().map(|at| at.y).collect();
                rows.dedup();
                rows
            };
            for parts in [
                vec![1],
                vec![1, 1],
                vec![3, 3],
                vec![8, 7],
                vec![6],
                vec![15],
            ] {
                let places = room.reserve(&parts);
                assert_eq!(places.len() as u32, parts.iter().sum::<u32>());
                // The ground that the reserve takes: a single player never calls for more
                // than one row.
                let depth = reserve_depth(rows(&parts).len() == 1);
                for (number, place) in places.iter().enumerate() {
                    // Outside the room, and a tank is 2 m long.
                    assert!(place.y <= min.y - 2.0 && place.y >= min.y - depth + 1.5);
                    assert!((place.x - middle).abs() <= 10.0, "seed {seed}: {parts:?}");
                    // A tank is 1.6 m wide.
                    for other in &places[..number] {
                        assert!(place.distance(*other) >= 2.5, "seed {seed}: {parts:?}");
                    }
                }
                // A row is centered on the room.
                let first_row: Vec<f32> = places
                    .iter()
                    .filter(|place| place.y == places[0].y)
                    .map(|place| place.x)
                    .collect();
                let center = (first_row[0] + first_row[first_row.len() - 1]) / 2.0;
                assert!((center - middle).abs() < 1e-3);
            }
            // A small group stands in one row, with a gap between the tanks that come with it
            // and those that wait for one of them to be destroyed.
            let small = room.reserve(&[3, 3]);
            assert_eq!(rows(&[3, 3]).len(), 1);
            let step = small[1].x - small[0].x;
            assert!((small[3].x - small[2].x - 2.0 * step).abs() < 1e-3);
            // A larger one stands a part to a row, the second part behind the first, and a
            // part that is too large for a row takes two.
            let two = rows(&[5, 4]);
            assert_eq!(two.len(), 2);
            assert!(two[1] < two[0]);
            assert_eq!(rows(&[15]), rows(&[8, 7]));
        }
        // The largest group of a single player has six tanks, which stand in one row.
        assert!(reserve_depth(true) < reserve_depth(false));
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

    /// Draws the room of a seed at a level of its terrain, a letter for each meter:
    /// `TANKU_SEED=TANKU TANKU_LEVEL=12 cargo test -p tanku_core dump_room -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_room() {
        let seed = std::env::var("TANKU_SEED").map_or(42, |text| crate::seed::seed_value(&text));
        let level = std::env::var("TANKU_LEVEL").map_or(1, |text| text.parse().unwrap());
        let room = Room::new(seed);
        let (blocks, crates) = room.obstacles(level);
        let places = room.places(level, &blocks, &crates);
        let plan = room.plan();
        let within = |corners: &[IVec2], x: i32, y: i32| {
            let cell = IVec2::new(x, y);
            corners.iter().any(|&corner| {
                let offset = cell - corner;
                (0..2).contains(&offset.x) && (0..2).contains(&offset.y)
            })
        };
        println!("{} blocks, {} crates", blocks.len(), crates.len());
        for y in (plan.min.y..plan.max.y).rev() {
            let row: String = (plan.min.x..plan.max.x)
                .map(|x| {
                    let center = IVec2::new(x, y) * 2 + IVec2::ONE;
                    let place = places.iter().position(|&place| place == IVec2::new(x, y));
                    match place {
                        _ if !plan.contains_half(center) => ' ',
                        _ if within(&blocks, x, y) => '#',
                        _ if within(&crates, x, y) => 'X',
                        _ if IVec2::new(x, y) == room.start() => '@',
                        Some(number) if number < 15 => (b'a' + number as u8) as char,
                        _ => '.',
                    }
                })
                .collect();
            println!("|{row}|");
        }
    }
}
