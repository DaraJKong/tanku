//! 2D geometry for the simulation: gameplay happens on the ground plane.

use std::f32::consts::{PI, TAU};

use bevy::math::Vec2;

/// Where and how a moving circle first touches something.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    /// Fraction of the movement done before the contact, from 0 to 1.
    pub t: f32,
    /// Unit vector pointing from the obstacle toward the circle at the contact.
    pub normal: Vec2,
}

pub fn closest_point_on_segment(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let length_squared = ab.length_squared();
    if length_squared == 0.0 {
        return a;
    }
    a + ab * ((p - a).dot(ab) / length_squared).clamp(0.0, 1.0)
}

pub fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    p.distance(closest_point_on_segment(p, a, b))
}

/// First contact of a circle that moves from `origin` by `delta` with the segment `a`-`b`.
///
/// A circle that already overlaps the segment only hits it if it moves further in.
pub fn sweep_circle_segment(
    origin: Vec2,
    delta: Vec2,
    radius: f32,
    a: Vec2,
    b: Vec2,
) -> Option<Hit> {
    let closest = closest_point_on_segment(origin, a, b);
    let offset = origin - closest;
    if offset.length_squared() < radius * radius {
        let normal = offset
            .try_normalize()
            .unwrap_or((b - a).perp().normalize_or_zero());
        return (delta.dot(normal) < 0.0).then_some(Hit { t: 0.0, normal });
    }

    let mut best: Option<Hit> = None;
    let mut consider = |hit: Hit| {
        if best.is_none_or(|current| hit.t < current.t) {
            best = Some(hit);
        }
    };

    // The side of the segment that faces the circle.
    let ab = b - a;
    if let Some(side) = ab.perp().try_normalize() {
        let side = if (origin - a).dot(side) < 0.0 {
            -side
        } else {
            side
        };
        let approach = -delta.dot(side);
        let distance = (origin - a).dot(side) - radius;
        if approach > 0.0 && distance >= 0.0 && distance <= approach {
            let t = distance / approach;
            let along = (origin + delta * t - a).dot(ab) / ab.length_squared();
            if (0.0..=1.0).contains(&along) {
                consider(Hit { t, normal: side });
            }
        }
    }

    // The two ends.
    for end in [a, b] {
        if let Some(t) = sweep_circle_point(origin, delta, end, radius) {
            let normal = (origin + delta * t - end).normalize_or_zero();
            consider(Hit { t, normal });
        }
    }
    best
}

/// Fraction of the movement after which a point that moves from `origin` by `delta` comes
/// within `radius` of `center`. A point that starts inside hits at once if it moves further in.
pub fn sweep_circle_point(origin: Vec2, delta: Vec2, center: Vec2, radius: f32) -> Option<f32> {
    let offset = origin - center;
    let c = offset.length_squared() - radius * radius;
    let b = offset.dot(delta);
    if c < 0.0 {
        return (b < 0.0).then_some(0.0);
    }
    let a = delta.length_squared();
    if a == 0.0 || b >= 0.0 {
        return None;
    }
    let discriminant = b * b - a * c;
    if discriminant < 0.0 {
        return None;
    }
    let t = (-b - discriminant.sqrt()) / a;
    (0.0..=1.0).contains(&t).then_some(t)
}

/// Smallest distance between two points that each move in a straight line during one tick.
pub fn closest_approach(a_from: Vec2, a_to: Vec2, b_from: Vec2, b_to: Vec2) -> f32 {
    let offset = a_from - b_from;
    let relative = (a_to - a_from) - (b_to - b_from);
    let speed_squared = relative.length_squared();
    let t = if speed_squared == 0.0 {
        0.0
    } else {
        (-offset.dot(relative) / speed_squared).clamp(0.0, 1.0)
    };
    (offset + relative * t).length()
}

/// Whether the segments `p`-`q` and `a`-`b` cross.
pub fn segments_intersect(p: Vec2, q: Vec2, a: Vec2, b: Vec2) -> bool {
    let side = |from: Vec2, to: Vec2, point: Vec2| (to - from).perp_dot(point - from);
    let d1 = side(a, b, p);
    let d2 = side(a, b, q);
    let d3 = side(p, q, a);
    let d4 = side(p, q, b);
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

pub fn reflect(velocity: Vec2, normal: Vec2) -> Vec2 {
    velocity - 2.0 * velocity.dot(normal) * normal
}

/// The signed angle to add to `from` to reach `to`, in `-PI..=PI`.
pub fn angle_difference(from: f32, to: f32) -> f32 {
    let difference = (to - from).rem_euclid(TAU);
    if difference > PI {
        difference - TAU
    } else {
        difference
    }
}

/// Turns `current` toward `target` by at most `max_step`.
pub fn rotate_toward(current: f32, target: f32, max_step: f32) -> f32 {
    let difference = angle_difference(current, target);
    if difference.abs() <= max_step {
        target
    } else {
        current + max_step.copysign(difference)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALL: (Vec2, Vec2) = (Vec2::new(-5.0, 2.0), Vec2::new(5.0, 2.0));

    #[test]
    fn circle_stops_at_its_radius_from_a_wall() {
        let hit =
            sweep_circle_segment(Vec2::ZERO, Vec2::new(0.0, 4.0), 0.5, WALL.0, WALL.1).unwrap();
        assert!((hit.t - 0.375).abs() < 1e-5);
        assert!(hit.normal.distance(Vec2::NEG_Y) < 1e-5);
    }

    #[test]
    fn circle_moving_away_or_falling_short_misses() {
        assert!(
            sweep_circle_segment(Vec2::ZERO, Vec2::new(0.0, -4.0), 0.5, WALL.0, WALL.1).is_none()
        );
        assert!(
            sweep_circle_segment(Vec2::ZERO, Vec2::new(0.0, 1.0), 0.5, WALL.0, WALL.1).is_none()
        );
    }

    #[test]
    fn circle_hits_the_end_of_a_wall() {
        let origin = Vec2::new(5.3, 0.0);
        let hit = sweep_circle_segment(origin, Vec2::new(0.0, 4.0), 0.5, WALL.0, WALL.1).unwrap();
        let contact = origin + Vec2::new(0.0, 4.0) * hit.t;
        assert!((contact.distance(WALL.1) - 0.5).abs() < 1e-4);
        assert!(hit.normal.x > 0.0 && hit.normal.y < 0.0);
    }

    #[test]
    fn overlapping_circle_only_hits_when_moving_in() {
        let origin = Vec2::new(0.0, 1.8);
        let inward = sweep_circle_segment(origin, Vec2::Y, 0.5, WALL.0, WALL.1).unwrap();
        assert_eq!(inward.t, 0.0);
        assert!(sweep_circle_segment(origin, Vec2::NEG_Y, 0.5, WALL.0, WALL.1).is_none());
    }

    #[test]
    fn reflection_keeps_the_speed() {
        let reflected = reflect(Vec2::new(3.0, 4.0), Vec2::NEG_Y);
        assert_eq!(reflected, Vec2::new(3.0, -4.0));
    }

    #[test]
    fn crossing_bullets_approach_each_other() {
        let distance = closest_approach(
            Vec2::new(-1.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, -1.0),
            Vec2::new(0.0, 1.0),
        );
        assert!(distance < 1e-5);
    }

    #[test]
    fn segments_cross_or_not() {
        let (p, q) = (Vec2::new(0.0, 0.0), Vec2::new(0.0, 4.0));
        assert!(segments_intersect(p, q, WALL.0, WALL.1));
        assert!(!segments_intersect(p, Vec2::new(0.0, 1.0), WALL.0, WALL.1));
    }

    #[test]
    fn angles_wrap() {
        assert!((angle_difference(3.0, -3.0) - (TAU - 6.0)).abs() < 1e-5);
        assert!((rotate_toward(3.0, -3.0, 0.1) - 3.1).abs() < 1e-5);
        assert_eq!(rotate_toward(0.0, 0.05, 0.1), 0.05);
    }
}
