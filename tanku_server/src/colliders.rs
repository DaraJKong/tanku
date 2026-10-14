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
use tanku_simd::{Band, Reach, Segments};

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
        let reach = delta.abs() / 2.0 + Vec2::splat(radius);
        let center = origin + delta / 2.0;
        // The circle never leaves the band of its radius around the line it moves along.
        let band = delta.perp().try_normalize().map(|normal| Band {
            point: origin.into(),
            normal: normal.into(),
            half_width: radius,
        });
        let near = Reach {
            center: center.into(),
            half: reach.into(),
            band,
        };
        let mut first: Option<(Hit, Solid)> = None;
        for collider in self.sift(near) {
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
