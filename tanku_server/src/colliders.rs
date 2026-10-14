//! What is solid in the field, and the questions that the simulation asks of it.
//!
//! Each question runs a sieve through the colliders first (see `tanku_simd`), which keeps the
//! few that it may be about, and is then answered for those as it was for all of them: in
//! the same order and with the same tests, so that the answer is the same to the last bit.

use std::ops::Deref;

use bevy::prelude::*;
use tanku_core::{
    geom::{self, Hit},
    rules::TANK_RADIUS,
    sim::Tank,
};
use tanku_simd::{Band, Kept, Reach, Segments};

use crate::field::{Collider, Solid};

/// How far a pass of `push_out` may move a tank before the colliders that its sieve left out
/// are looked at too, in meters: the sieve keeps those that come within this much of the
/// tank's reach, twice over.
const LEEWAY: f32 = 1.0;

/// The colliders of a field, or of a part of it, in the order in which they were added.
#[derive(Clone, Debug, Default)]
pub struct Colliders {
    list: Vec<Collider>,
    /// The same, laid out for the sieves.
    segments: Segments,
}

impl Colliders {
    pub fn push(&mut self, collider: Collider) {
        self.segments.push(collider.a.into(), collider.b.into());
        self.list.push(collider);
    }

    pub fn clear(&mut self) {
        self.list.clear();
        self.segments.clear();
    }

    /// The colliders that a sieve keeps for a reach, in their order.
    fn sift(&self, reach: Reach) -> impl Iterator<Item = &Collider> {
        self.segments.sift(&reach).map(|index| &self.list[index])
    }

    /// First thing a circle touches when it moves from `origin` by `delta`.
    pub fn cast(&self, origin: Vec2, delta: Vec2, radius: f32) -> Option<(Hit, Solid)> {
        let kept = self.segments.sift(&swept(origin, delta, radius));
        self.cast_among(kept, origin, delta, radius)
    }

    /// The colliders that a circle may touch anywhere on its way when it moves from `origin`
    /// by `delta`. A cast that starts on that way and ends on it, as a bullet does tick
    /// after tick while it flies straight, has the same answer among them as among all.
    pub fn lane(&self, origin: Vec2, delta: Vec2, radius: f32) -> Lane {
        let kept = self.segments.sift(&swept(origin, delta, radius));
        let along = delta.normalize_or_zero();
        let reach = radius + NEAR_LANE;
        let mut near: Vec<(f32, f32)> = kept
            .clone()
            .filter_map(|index| {
                let collider = &self.list[index];
                stretch((origin, along), (collider.a, collider.b), reach)
            })
            .collect();
        near.sort_by(|one, other| one.0.total_cmp(&other.0));
        // Stretches that overlap are one.
        let mut merged: Vec<(f32, f32)> = Vec::with_capacity(near.len());
        for (from, to) in near {
            match merged.last_mut() {
                Some(last) if from <= last.1 => last.1 = last.1.max(to),
                _ => merged.push((from, to)),
            }
        }
        Lane {
            kept,
            origin,
            along,
            near: merged,
        }
    }

    /// The same as `cast`, for a cast that starts and ends on the way of a lane. A cast whose
    /// way along the lane comes near none of its colliders meets nothing, and none of them is
    /// asked.
    pub fn cast_in(
        &self,
        lane: &Lane,
        origin: Vec2,
        delta: Vec2,
        radius: f32,
    ) -> Option<(Hit, Solid)> {
        let from = origin - lane.origin;
        let (start, end) = (from.dot(lane.along), (from + delta).dot(lane.along));
        let (low, high) = (start.min(end), start.max(end));
        // A cast off the line of the lane by more than the rounding of numbers would move it
        // is asked of the colliders, and so is one that comes near one of them.
        let on_line = from.perp_dot(lane.along).abs() <= NEAR_LANE / 2.0;
        let first = lane.near.partition_point(|&(_, to)| to < low);
        let clear = lane.near.get(first).is_none_or(|&(from, _)| from > high);
        if on_line && clear {
            return None;
        }
        self.cast_among(lane.kept.clone(), origin, delta, radius)
    }

    /// What `cast` finds among the colliders that a sieve kept.
    fn cast_among(
        &self,
        kept: Kept,
        origin: Vec2,
        delta: Vec2,
        radius: f32,
    ) -> Option<(Hit, Solid)> {
        let reach = delta.abs() / 2.0 + Vec2::splat(radius);
        let center = origin + delta / 2.0;
        let mut first: Option<(Hit, Solid)> = None;
        for collider in kept.map(|index| &self.list[index]) {
            // The test of boxes that the sieve made with some slack, made exactly: a
            // collider that only the slack kept is left out, as it always was.
            let middle = (collider.a + collider.b) / 2.0;
            let extent = (collider.b - collider.a).abs() / 2.0;
            if ((middle - center).abs() - extent - reach).max_element() > 0.0 {
                continue;
            }
            let hit = geom::sweep_circle_segment(origin, delta, radius, collider.a, collider.b);
            if let Some(hit) = hit
                && first.is_none_or(|(best, _)| hit.t < best.t)
            {
                first = Some((hit, collider.solid));
            }
        }
        first
    }

    /// Whether anything that matches `stops` lies between two points.
    pub fn blocked(&self, from: Vec2, to: Vec2, stops: impl Fn(Solid) -> bool) -> bool {
        // What crosses the way between them has its box on the box of the way, and is on
        // both sides of the line that the way runs along.
        let band = (to - from).perp().try_normalize().map(|normal| Band {
            point: from.into(),
            normal: normal.into(),
            half_width: 0.0,
        });
        let across = Reach {
            center: ((from + to) / 2.0).into(),
            half: ((to - from).abs() / 2.0).into(),
            band,
        };
        self.sift(across).any(|collider| {
            stops(collider.solid) && geom::segments_intersect(from, to, collider.a, collider.b)
        })
    }

    /// Moves a tank out of what it overlaps.
    pub fn push_out(&self, tank: &Tank, position: &mut Vec2, velocity: &mut Vec2) {
        for _ in 0..3 {
            let start = *position;
            let reach = Reach {
                center: start.into(),
                half: [TANK_RADIUS + LEEWAY; 2],
                band: None,
            };
            let mut moved = false;
            // A tank that a collider pushed far from where it was may then overlap one that
            // the sieve left out: from there on, each one is looked at.
            let mut rest = None;
            for index in self.segments.sift(&reach) {
                if push(&self.list[index], tank, position, velocity) {
                    moved = true;
                    if position.distance(start) > LEEWAY / 2.0 {
                        rest = Some(index + 1);
                        break;
                    }
                }
            }
            for collider in rest.map_or(&[][..], |rest| &self.list[rest..]) {
                moved |= push(collider, tank, position, velocity);
            }
            if !moved {
                break;
            }
        }
    }

    /// The colliders that come within some distance of a place, in their order.
    pub fn near(&self, at: Vec2, distance: f32) -> Colliders {
        let reach = Reach {
            center: at.into(),
            half: [distance; 2],
            band: None,
        };
        self.sift(reach)
            .filter(|collider| geom::distance_to_segment(at, collider.a, collider.b) < distance)
            .copied()
            .collect()
    }
}

/// How much further than its radius a circle in a lane may come to a collider of the lane
/// before a cast there is asked of the colliders, in meters: half of it for the rounding of
/// the places of a bullet that flies straight tick after tick, which is far less, and half for
/// what the lane leaves off the line of its way.
const NEAR_LANE: f32 = 0.02;

/// The colliders that a sieve kept for the way of a circle that moves straight, and the
/// stretches of that way where the circle comes near one of them.
#[derive(Clone, Debug)]
pub struct Lane {
    kept: Kept,
    /// Where the way starts, and the unit vector along it.
    origin: Vec2,
    along: Vec2,
    /// The stretches where the circle comes within its radius and `NEAR_LANE` of a collider
    /// of the lane, as distances from the start of the way, in their order and merged.
    near: Vec<(f32, f32)>,
}

/// The stretch of a line where it comes within `reach` of a segment, as distances along it
/// from a point: a little longer than that stretch is, and none if the line keeps further.
/// The line is given by the point and the unit vector along it.
fn stretch((origin, along): (Vec2, Vec2), (a, b): (Vec2, Vec2), reach: f32) -> Option<(f32, f32)> {
    let across = along.perp();
    let (to_a, to_b) = (a - origin, b - origin);
    let (along_a, along_b) = (to_a.dot(along), to_b.dot(along));
    let (off_a, off_b) = (to_a.dot(across), to_b.dot(across));
    // The part of the segment that lies within reach of the line, as shares of it from `a`.
    let (low, high) = if (off_b - off_a).abs() < 1e-6 {
        if off_a.abs() >= reach {
            return None;
        }
        (0.0, 1.0)
    } else {
        let one = (-reach - off_a) / (off_b - off_a);
        let other = (reach - off_a) / (off_b - off_a);
        (one.min(other).max(0.0), one.max(other).min(1.0))
    };
    if low > high {
        return None;
    }
    let (first, last) = (
        along_a + (along_b - along_a) * low,
        along_a + (along_b - along_a) * high,
    );
    // A point of the line within reach of a point of that part is no further along it than
    // the reach.
    Some((first.min(last) - reach, first.max(last) + reach))
}

/// Where a circle may touch something when it moves from `origin` by `delta`: in the box of
/// its way, and in the band of its radius around the line it moves along, which it never
/// leaves.
fn swept(origin: Vec2, delta: Vec2, radius: f32) -> Reach {
    let reach = delta.abs() / 2.0 + Vec2::splat(radius);
    let center = origin + delta / 2.0;
    let band = delta.perp().try_normalize().map(|normal| Band {
        point: origin.into(),
        normal: normal.into(),
        half_width: radius,
    });
    Reach {
        center: center.into(),
        half: reach.into(),
        band,
    }
}

/// Moves a tank out of a collider if it overlaps it, and tells whether it did.
fn push(collider: &Collider, tank: &Tank, position: &mut Vec2, velocity: &mut Vec2) -> bool {
    if !collider.solid.stops(tank) {
        return false;
    }
    let closest = geom::closest_point_on_segment(*position, collider.a, collider.b);
    let offset = *position - closest;
    let distance = offset.length();
    if distance < TANK_RADIUS && distance > 1e-5 {
        let normal = offset / distance;
        *position += normal * (TANK_RADIUS - distance);
        *velocity -= normal * velocity.dot(normal).min(0.0);
        return true;
    }
    false
}

impl Deref for Colliders {
    type Target = [Collider];

    fn deref(&self) -> &[Collider] {
        &self.list
    }
}

impl FromIterator<Collider> for Colliders {
    fn from_iter<I: IntoIterator<Item = Collider>>(colliders: I) -> Self {
        let mut all = Self::default();
        for collider in colliders {
            all.push(collider);
        }
        all
    }
}

impl From<Vec<Collider>> for Colliders {
    fn from(colliders: Vec<Collider>) -> Self {
        colliders.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::{
        map::Corridor,
        rules::{BLOCK_SIZE, BULLET_RADIUS},
        sim::Team,
        stats::Genome,
    };

    use super::*;

    const TICK_TEST: f32 = tanku_core::rules::TICK;

    /// Numbers that look random and are the same on every run.
    struct Numbers(u64);

    impl Numbers {
        fn unit(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 40) as f32 / (1u64 << 24) as f32
        }

        fn within(&mut self, low: f32, high: f32) -> f32 {
            low + (high - low) * self.unit()
        }

        /// A way of a length between two, in a direction of its own.
        fn way(&mut self, shortest: f32, longest: f32) -> Vec2 {
            let length = self.within(shortest, longest);
            Vec2::from_angle(self.within(0.0, std::f32::consts::TAU)) * length
        }
    }

    /// Where a circle first touches one of the colliders, as every collider was asked before
    /// the sieves.
    fn cast_each(
        colliders: &[Collider],
        origin: Vec2,
        delta: Vec2,
        radius: f32,
    ) -> Option<(Hit, Solid)> {
        let reach = delta.abs() / 2.0 + Vec2::splat(radius);
        let center = origin + delta / 2.0;
        let mut first: Option<(Hit, Solid)> = None;
        for collider in colliders {
            let middle = (collider.a + collider.b) / 2.0;
            let extent = (collider.b - collider.a).abs() / 2.0;
            if ((middle - center).abs() - extent - reach).max_element() > 0.0 {
                continue;
            }
            let hit = geom::sweep_circle_segment(origin, delta, radius, collider.a, collider.b);
            if let Some(hit) = hit
                && first.is_none_or(|(best, _)| hit.t < best.t)
            {
                first = Some((hit, collider.solid));
            }
        }
        first
    }

    fn push_out_each(
        colliders: &[Collider],
        tank: &Tank,
        position: &mut Vec2,
        velocity: &mut Vec2,
    ) {
        for _ in 0..3 {
            let mut moved = false;
            for collider in colliders {
                moved |= push(collider, tank, position, velocity);
            }
            if !moved {
                break;
            }
        }
    }

    /// A hit as its bits, to tell two answers apart to the last one.
    fn bits(hit: Option<(Hit, Solid)>) -> Option<(u32, [u32; 2], Solid)> {
        hit.map(|(hit, solid)| {
            (
                hit.t.to_bits(),
                hit.normal.to_array().map(f32::to_bits),
                solid,
            )
        })
    }

    /// The walls, the blocks, the crates and a barrier of three sectors of a corridor, as the
    /// field makes them.
    fn sectors(corridor: &mut Corridor, middle: u32) -> Vec<Collider> {
        let mut colliders = Vec::new();
        for index in middle - 1..=middle + 1 {
            let layout = corridor.layout(index);
            for &(a, b) in &layout.plan.walls {
                let (a, b) = (a.as_vec2(), b.as_vec2());
                colliders.push(Collider {
                    a,
                    b,
                    solid: Solid::Wall,
                });
            }
            let squares = layout.blocks.iter().map(|&corner| (corner, false));
            for (corner, explosive) in
                squares.chain(layout.crates.iter().map(|&corner| (corner, true)))
            {
                let (low, size) = (corner.as_vec2(), BLOCK_SIZE);
                let corners = [
                    low,
                    low + Vec2::X * size,
                    low + Vec2::splat(size),
                    low + Vec2::Y * size,
                ];
                for side in 0..4 {
                    let solid = if explosive {
                        Solid::Crate(Entity::PLACEHOLDER)
                    } else {
                        Solid::Block(Entity::PLACEHOLDER)
                    };
                    let (a, b) = (corners[side], corners[(side + 1) % 4]);
                    colliders.push(Collider { a, b, solid });
                }
            }
            let (a, b) = (layout.plan.gate.0.as_vec2(), layout.plan.gate.1.as_vec2());
            let solid = Solid::Barrier {
                sector: index,
                unlocked: index % 2 == 0,
            };
            colliders.push(Collider { a, b, solid });
        }
        colliders
    }

    #[test]
    fn the_sieves_change_no_answer_anywhere_along_a_corridor() {
        let mut numbers = Numbers(0x2545_F491_4F6C_DD1D);
        let tanks = [Team::Players, Team::Enemies].map(|team| Tank {
            genome: Genome::PLAYER,
            team,
            sector: 4,
        });
        for seed in [1, 7] {
            let mut corridor = Corridor::new(seed);
            for middle in [2, 4, 25, 61] {
                let each = sectors(&mut corridor, middle);
                let sieved = Colliders::from(each.clone());
                assert_eq!(sieved.len(), each.len());
                // Places around the colliders: on them, near them and further away.
                let place = |numbers: &mut Numbers| {
                    let collider = each[(numbers.unit() * each.len() as f32) as usize % each.len()];
                    let along = collider.a.lerp(collider.b, numbers.unit());
                    let off = numbers.unit().powi(2) * 12.0;
                    along + numbers.way(0.0, off)
                };
                for _ in 0..4000 {
                    let origin = place(&mut numbers);
                    let length =
                        [0.1, 0.32, 2.5, 13.0, 60.0, 80.0][(numbers.unit() * 6.0) as usize % 6];
                    let delta = numbers.way(length / 2.0, length);
                    let radius =
                        [BULLET_RADIUS, TANK_RADIUS, 0.1][(numbers.unit() * 3.0) as usize % 3];
                    assert_eq!(
                        bits(sieved.cast(origin, delta, radius)),
                        bits(cast_each(&each, origin, delta, radius)),
                        "a cast from {origin} by {delta} with {radius}",
                    );

                    let to = origin + numbers.way(0.0, 40.0);
                    let each_blocks = each.iter().any(|collider| {
                        geom::segments_intersect(origin, to, collider.a, collider.b)
                    });
                    assert_eq!(sieved.blocked(origin, to, |_| true), each_blocks);
                    let walls = |solid: Solid| solid.stops_blasts();
                    let each_walls = each.iter().any(|collider| {
                        walls(collider.solid)
                            && geom::segments_intersect(origin, to, collider.a, collider.b)
                    });
                    assert_eq!(sieved.blocked(origin, to, walls), each_walls);

                    let tank = &tanks[(numbers.unit() * 2.0) as usize % 2];
                    let velocity = numbers.way(0.0, 8.0);
                    let (mut sieved_at, mut sieved_velocity) = (origin, velocity);
                    let (mut each_at, mut each_velocity) = (origin, velocity);
                    sieved.push_out(tank, &mut sieved_at, &mut sieved_velocity);
                    push_out_each(&each, tank, &mut each_at, &mut each_velocity);
                    assert_eq!(
                        sieved_at.to_array().map(f32::to_bits),
                        each_at.to_array().map(f32::to_bits)
                    );
                    assert_eq!(
                        sieved_velocity.to_array().map(f32::to_bits),
                        each_velocity.to_array().map(f32::to_bits)
                    );

                    let distance = numbers.within(0.0, 10.0);
                    let near = |collider: &&Collider| {
                        geom::distance_to_segment(origin, collider.a, collider.b) < distance
                    };
                    let each_near: Vec<(Vec2, Vec2)> = each
                        .iter()
                        .filter(near)
                        .map(|collider| (collider.a, collider.b))
                        .collect();
                    let sieved_near: Vec<(Vec2, Vec2)> = sieved
                        .near(origin, distance)
                        .iter()
                        .map(|collider| (collider.a, collider.b))
                        .collect();
                    assert_eq!(sieved_near, each_near);
                }
            }
        }
    }

    #[test]
    fn a_bullet_flies_the_same_way_in_a_lane() {
        use tanku_core::sim::{Bullet, Kind};

        use crate::{
            bullets::{self, Flight},
            field::Field,
        };

        let mut numbers = Numbers(0x5DEE_CE66_D1CE_4E5B);
        for seed in [3, 11] {
            let mut corridor = Corridor::new(seed);
            for middle in [3, 30, 58] {
                let each = sectors(&mut corridor, middle);
                let mut field = Field::new(seed, 1, tanku_core::modes::Difficulty::Normal);
                field.colliders = Colliders::from(each.clone());
                // Tanks that a seeking bullet may leave a wall toward.
                let quarry: Vec<(Vec2, Team)> = (0..4)
                    .map(|_| {
                        let collider =
                            each[(numbers.unit() * each.len() as f32) as usize % each.len()];
                        let team =
                            [Team::Players, Team::Enemies][(numbers.unit() * 2.0) as usize % 2];
                        (collider.a + numbers.way(2.0, 6.0), team)
                    })
                    .collect();
                for round in 0..600 {
                    let collider = each[(numbers.unit() * each.len() as f32) as usize % each.len()];
                    let along = (collider.b - collider.a).normalize_or_zero();
                    let on = collider.a.lerp(collider.b, numbers.unit());
                    // Half of them graze a wall: they start a radius from it, or a hair more or
                    // less, and fly along it or nearly.
                    let grazing = round % 2 == 1;
                    let start = if grazing {
                        on + along.perp() * (BULLET_RADIUS + numbers.within(-1e-3, 1e-3))
                    } else {
                        on + numbers.way(1.0, 8.0)
                    };
                    let kind = [Kind::Plain, Kind::Seeking, Kind::Exploding]
                        [(numbers.unit() * 3.0) as usize % 3];
                    let bullet = Bullet {
                        owner: Entity::PLACEHOLDER,
                        team: Team::Enemies,
                        sector: middle,
                        velocity: if grazing {
                            let tilt = numbers.within(-0.02, 0.02);
                            Vec2::from_angle(tilt).rotate(along) * numbers.within(6.0, 19.0)
                        } else {
                            numbers.way(6.0, 19.0)
                        },
                        bounces_left: (numbers.unit() * 4.0) as u8 % 4,
                        pierces_left: 0,
                        bounced: false,
                        kind,
                    };
                    let step = bullet.velocity.length() * TICK_TEST;
                    let ticks = 180;
                    let (mut plain, mut laned) = ((start, bullet), (start, bullet));
                    let mut lane = None;
                    for tick in 0..ticks {
                        let one = bullets::fly_past_walls(
                            &field,
                            &mut plain.0,
                            &mut plain.1,
                            step,
                            &quarry,
                        );
                        let way = step * (ticks - tick) as f32;
                        let flying = (&mut laned.0, &mut laned.1);
                        let other =
                            bullets::fly_in_lane(&field, &mut lane, flying, (step, way), &quarry);
                        assert_eq!(
                            plain.0.to_array().map(f32::to_bits),
                            laned.0.to_array().map(f32::to_bits)
                        );
                        assert_eq!(plain.1, laned.1);
                        assert_eq!(one.bounces, other.bounces);
                        assert_eq!(one.end, other.end);
                        if let Flight::Stopped(_) = one.end {
                            break;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_course_worked_out_ahead_is_the_one_that_the_tank_drives() {
        use tanku_core::{sim::Motion, stats::Stat};

        use crate::tanks::{self, Course};

        let mut numbers = Numbers(0x2F6B_A3C1_9E08_D47B);
        let tanks: Vec<Tank> = [(5, 5), (2, 1), (7, 6), (0, 0)]
            .into_iter()
            .map(|(engine, tracks)| {
                let mut genome = Genome::empty(100);
                genome.set(Stat::Engine, engine);
                genome.set(Stat::Tracks, tracks);
                Tank {
                    genome,
                    team: Team::Players,
                    sector: 4,
                }
            })
            .collect();
        let mut corridor = Corridor::new(5);
        for middle in [2, 4, 25] {
            let all = Colliders::from(sectors(&mut corridor, middle));
            for _ in 0..1500 {
                let collider = all[(numbers.unit() * all.len() as f32) as usize % all.len()];
                // Most of them start a tank's width from a wall or closer.
                let reach = numbers.unit().powi(2) * 4.0;
                let from = collider.a.lerp(collider.b, numbers.unit()) + numbers.way(0.0, reach);
                let tank = &tanks[(numbers.unit() * 4.0) as usize % 4];
                let motion = Motion {
                    velocity: numbers.way(0.0, tank.genome.top_speed()),
                    hull: numbers.within(-4.0, 4.0),
                    turret: 0.0,
                };
                let way = if numbers.unit() < 0.1 {
                    Vec2::ZERO
                } else {
                    numbers.way(0.5, 1.2)
                };
                let near = all.near(from, 9.0);
                let ahead = Course::among(near.clone(), tank, from).places(&motion, way, 60);
                let (mut at, mut driven) = (from, motion);
                let mut places = vec![at];
                for _ in 0..60 {
                    tanks::drive_among(&near, tank, way, &mut at, &mut driven);
                    places.push(at);
                }
                let bits = |places: &[Vec2]| -> Vec<[u32; 2]> {
                    places
                        .iter()
                        .map(|at| at.to_array().map(f32::to_bits))
                        .collect()
                };
                assert_eq!(bits(&ahead), bits(&places), "from {from} by {way}");
            }
        }
    }

    #[test]
    fn colliders_that_are_pushed_and_cleared_are_the_ones_asked() {
        let wall = |a: Vec2, b: Vec2| Collider {
            a,
            b,
            solid: Solid::Wall,
        };
        let mut colliders = Colliders::default();
        colliders.push(wall(Vec2::new(-5.0, 2.0), Vec2::new(5.0, 2.0)));
        let hit = colliders
            .cast(Vec2::ZERO, Vec2::Y * 4.0, 0.5)
            .map(|(hit, _)| hit.t);
        assert!(hit.is_some_and(|t| (t - 0.375).abs() < 1e-5));
        assert!(colliders.blocked(Vec2::ZERO, Vec2::Y * 4.0, |_| true));
        colliders.clear();
        assert!(colliders.is_empty());
        assert!(colliders.cast(Vec2::ZERO, Vec2::Y * 4.0, 0.5).is_none());
        assert!(!colliders.blocked(Vec2::ZERO, Vec2::Y * 4.0, |_| true));
    }
}
