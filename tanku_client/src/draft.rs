//! Drawings made of flat-colored faces: how the game draws walls, obstacles and tanks.
//!
//! A drawing is a mesh that carries its colors in its vertices. Nothing is lit: a face shows
//! the color it was given, the sides of a solid are darker than its top, and a line is a thin
//! strip of geometry laid on a face.

use std::f32::consts::TAU;

use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};

/// Width of the lines of a drawing, in meters: a pixel and a half at 1080p.
pub const LINE: f32 = 0.04;
/// How far a line lies above the face it is drawn on, so that the two don't fight.
pub const LIFT: f32 = 0.004;
/// Sides of the polygon that stands for a circle of one meter in radius. Smaller circles
/// get fewer, down to [`FEWEST_SIDES`].
const SIDES_PER_METER: f32 = 40.0;
const FEWEST_SIDES: usize = 10;

fn sides(radius: f32) -> usize {
    ((radius * SIDES_PER_METER) as usize).clamp(FEWEST_SIDES, 96)
}

/// The point of a circle around the vertical axis: `turn` is a share of a full turn.
fn around(center: Vec3, radius: f32, turn: f32) -> Vec3 {
    let (sin, cos) = (turn * TAU).sin_cos();
    center + Vec3::new(cos, 0.0, sin) * radius
}

#[derive(Default)]
pub struct Draft {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    colors: Vec<[f32; 4]>,
}

impl Draft {
    /// A flat face with three corners or more. It must be convex.
    pub fn face(&mut self, corners: &[Vec3], color: Color) {
        let paint = color.to_linear().to_f32_array();
        let normal = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or_zero();
        for next in 2..corners.len() {
            for corner in [corners[0], corners[next - 1], corners[next]] {
                self.positions.push(corner);
                self.normals.push(normal);
                self.colors.push(paint);
            }
        }
    }

    /// A box: `top` on its top face and `side` on the four around it. Its underside is
    /// never seen, so it has none.
    pub fn solid(&mut self, center: Vec3, size: Vec3, top: Color, side: Color) {
        let half = size / 2.0;
        let corner = |x: f32, y: f32, z: f32| center + half * Vec3::new(x, y, z);
        self.face(
            &[
                corner(-1.0, 1.0, -1.0),
                corner(-1.0, 1.0, 1.0),
                corner(1.0, 1.0, 1.0),
                corner(1.0, 1.0, -1.0),
            ],
            top,
        );
        for (x, z) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            // The two ends of the edge that this side stands on.
            let (from, to) = if x != 0.0 {
                ((x, -1.0), (x, 1.0))
            } else {
                ((-1.0, z), (1.0, z))
            };
            self.face(
                &[
                    corner(from.0, -1.0, from.1),
                    corner(to.0, -1.0, to.1),
                    corner(to.0, 1.0, to.1),
                    corner(from.0, 1.0, from.1),
                ],
                side,
            );
        }
    }

    /// An upright cylinder: `top` on its top face and `side` around it.
    pub fn post(&mut self, center: Vec3, radius: f32, height: f32, top: Color, side: Color) {
        let count = sides(radius);
        let (low, high) = (
            center - Vec3::Y * height / 2.0,
            center + Vec3::Y * height / 2.0,
        );
        let turn = |step: usize| step as f32 / count as f32;
        let rim: Vec<Vec3> = (0..count)
            .map(|step| around(high, radius, turn(step)))
            .collect();
        self.face(&rim, top);
        for step in 0..count {
            self.face(
                &[
                    around(low, radius, turn(step)),
                    around(low, radius, turn(step + 1)),
                    around(high, radius, turn(step + 1)),
                    around(high, radius, turn(step)),
                ],
                side,
            );
        }
    }

    /// A cone that points along `x` from the middle of its base.
    pub fn point(&mut self, base: Vec3, radius: f32, length: f32, color: Color) {
        let count = FEWEST_SIDES;
        let tip = base + Vec3::X * length;
        let rim = |step: usize| {
            let (sin, cos) = (step as f32 / count as f32 * TAU).sin_cos();
            base + Vec3::new(0.0, cos, sin) * radius
        };
        for step in 0..count {
            self.face(&[rim(step), rim(step + 1), tip], color);
        }
    }

    /// A flat disc lying at the height of its center.
    pub fn disc(&mut self, center: Vec3, radius: f32, color: Color) {
        let count = sides(radius);
        let rim: Vec<Vec3> = (0..count)
            .map(|step| around(center, radius, step as f32 / count as f32))
            .collect();
        self.face(&rim, color);
    }

    /// A flat ring lying at the height of its center, cut into `dashes` equal arcs of which
    /// each fills this `share` of its place. One dash that fills its place is a whole ring.
    pub fn ring(
        &mut self,
        center: Vec3,
        (inner, outer): (f32, f32),
        (dashes, share): (usize, f32),
        color: Color,
    ) {
        // Short steps, so that an arc stays round.
        let steps = (sides(outer) / dashes).max(1);
        for dash in 0..dashes {
            for step in 0..steps {
                let turn = |step: usize| {
                    (dash as f32 + share * step as f32 / steps as f32) / dashes as f32
                };
                self.face(
                    &[
                        around(center, inner, turn(step)),
                        around(center, outer, turn(step)),
                        around(center, outer, turn(step + 1)),
                        around(center, inner, turn(step + 1)),
                    ],
                    color,
                );
            }
        }
    }

    /// A flat strip between two points of the same height.
    pub fn line(&mut self, from: Vec3, to: Vec3, width: f32, color: Color) {
        let along = (to - from).normalize_or_zero();
        let across = Vec3::new(-along.z, 0.0, along.x) * width / 2.0;
        self.face(
            &[from - across, from + across, to + across, to - across],
            color,
        );
    }

    /// The outline of a rectangle lying flat around `center`, as a band of this width. A
    /// positive width draws inside the rectangle, a negative one outside it.
    pub fn frame(&mut self, center: Vec3, size: Vec2, width: f32, color: Color) {
        let half = size / 2.0;
        let inset = half - Vec2::splat(width);
        let corner =
            |reach: Vec2, x: f32, z: f32| center + Vec3::new(reach.x * x, 0.0, reach.y * z);
        let turns = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];
        for side in 0..4 {
            let (here, next) = (turns[side], turns[(side + 1) % 4]);
            self.face(
                &[
                    corner(half, here.0, here.1),
                    corner(half, next.0, next.1),
                    corner(inset, next.0, next.1),
                    corner(inset, here.0, here.1),
                ],
                color,
            );
        }
    }

    /// A box with its edges drawn: a line inside the edge of its top face, and one around
    /// its foot.
    pub fn outlined(&mut self, center: Vec3, size: Vec3, (top, side, line): (Color, Color, Color)) {
        self.solid(center, size, top, side);
        let plan = Vec2::new(size.x, size.z);
        let lid = center + Vec3::Y * (size.y / 2.0 + LIFT);
        self.frame(lid, plan, LINE, line);
        let foot = center - Vec3::Y * (size.y / 2.0 - LIFT);
        self.frame(foot, plan, -LINE, line);
    }

    /// A line up each of the four corners of a box. It stands a little proud of the faces it
    /// joins, so that it shows on both.
    pub fn corners(&mut self, center: Vec3, size: Vec3, color: Color) {
        let post = Vec3::new(LINE, size.y, LINE);
        for (x, z) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let corner = center + Vec3::new(x * size.x, 0.0, z * size.z) / 2.0;
            self.solid(corner, post, color, color);
        }
    }

    pub fn build(self) -> Mesh {
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

    /// The lowest and the highest corner of everything drawn so far.
    #[cfg(test)]
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let lowest = self.positions.iter().copied().reduce(Vec3::min);
        let highest = self.positions.iter().copied().reduce(Vec3::max);
        (lowest.unwrap_or_default(), highest.unwrap_or_default())
    }

    /// The area of the faces of this color, whichever way they face.
    #[cfg(test)]
    pub fn area(&self, color: Color) -> f32 {
        let paint = color.to_linear().to_f32_array();
        let triangles = self.positions.chunks(3).zip(self.colors.chunks(3));
        triangles
            .filter(|(_, colors)| colors[0] == paint)
            .map(|(corners, _)| {
                (corners[1] - corners[0])
                    .cross(corners[2] - corners[0])
                    .length()
                    / 2.0
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOP: Color = Color::srgb(1.0, 0.0, 0.0);
    const SIDE: Color = Color::srgb(0.0, 1.0, 0.0);
    const INK: Color = Color::srgb(0.0, 0.0, 1.0);

    fn close(found: f32, wanted: f32) -> bool {
        (found - wanted).abs() < 1e-4
    }

    #[test]
    fn a_solid_has_a_top_and_four_sides() {
        let mut draft = Draft::default();
        draft.solid(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(2.0, 4.0, 6.0),
            TOP,
            SIDE,
        );
        assert!(close(draft.area(TOP), 12.0));
        assert!(close(draft.area(SIDE), 2.0 * (8.0 + 24.0)));
        assert_eq!(
            draft.bounds(),
            (Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 4.0, 6.0))
        );
    }

    #[test]
    fn a_post_is_as_wide_and_as_tall_as_asked() {
        let mut draft = Draft::default();
        draft.post(Vec3::Y, 0.5, 2.0, TOP, SIDE);
        let (lowest, highest) = draft.bounds();
        assert!(close(lowest.y, 0.0) && close(highest.y, 2.0));
        assert!(close(highest.x, 0.5) && close(lowest.x, -0.5));
        // The polygon is a little smaller than the circle it stands for.
        let disc = std::f32::consts::PI * 0.25;
        assert!(draft.area(TOP) > 0.97 * disc && draft.area(TOP) < disc);
    }

    #[test]
    fn a_frame_is_drawn_inside_or_outside_its_rectangle() {
        let size = Vec2::new(2.0, 1.0);
        let mut inside = Draft::default();
        inside.frame(Vec3::ZERO, size, 0.1, INK);
        assert!(close(inside.area(INK), 2.0 - 1.8 * 0.8));
        assert_eq!(inside.bounds().1, Vec3::new(1.0, 0.0, 0.5));

        let mut outside = Draft::default();
        outside.frame(Vec3::ZERO, size, -0.1, INK);
        assert!(close(outside.area(INK), 2.2 * 1.2 - 2.0));
        assert!(close(outside.bounds().1.x, 1.1));
    }

    #[test]
    fn a_dashed_ring_leaves_gaps() {
        let mut whole = Draft::default();
        whole.ring(Vec3::ZERO, (1.9, 2.0), (1, 1.0), INK);
        let mut dashed = Draft::default();
        dashed.ring(Vec3::ZERO, (1.9, 2.0), (24, 0.5), INK);
        let share = dashed.area(INK) / whole.area(INK);
        assert!((share - 0.5).abs() < 0.01, "{share}");
    }

    #[test]
    fn a_line_is_as_long_and_as_wide_as_asked() {
        let mut draft = Draft::default();
        draft.line(Vec3::ZERO, Vec3::new(3.0, 0.0, 4.0), 0.2, INK);
        assert!(close(draft.area(INK), 1.0));
    }

    #[test]
    fn a_drawing_becomes_a_mesh_with_its_colors() {
        let mut draft = Draft::default();
        draft.outlined(Vec3::Y, Vec3::splat(2.0), (TOP, SIDE, INK));
        let mesh = draft.build();
        let corners = mesh.count_vertices();
        assert!(corners > 0 && corners.is_multiple_of(3));
        assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some());
    }
}
