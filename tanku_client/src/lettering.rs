//! The lettering that is drawn on the sheet itself, as a draftsman letters a plan: the
//! figures and the capitals of what is written in the void around the corridor.
//!
//! Each character is drawn with the pen of the drawing, in strokes, after lettering type B of
//! ISO 3098: the line is a tenth of the height of a capital, two lines part two characters and
//! six part two words. Only capitals, figures and the signs of dimensions are drawn, as on a
//! plan.

use std::{collections::HashMap, f32::consts::TAU};

use bevy::prelude::*;

use crate::draft::Draft;

/// The strokes of each character, in tenths of the height of a capital, with the baseline at
/// 0 and the line of the capitals at 10. `M` lifts the pen to a point, `L` draws a line to a
/// point, and `A` draws an arc of an ellipse: its center, its two radii, and the angles in
/// degrees, counterclockwise, from which and to which it goes. An arc that starts where the
/// pen is goes on from there. The number before the strokes is the width of the character,
/// from the middle of its leftmost line to the middle of its rightmost one.
const GLYPHS: &[(char, f32, &str)] = &[
    ('0', 5.0, "A2.5,5,2.5,5,0,360"),
    ('1', 5.0, "M0.9,7.6 L2.5,10 L2.5,0"),
    ('2', 5.0, "A2.5,7.5,2.5,2.5,165,-38 L0,0 L5,0"),
    (
        '3',
        5.0,
        "A2.5,7.6,2.4,2.4,155,-90 A2.5,2.6,2.5,2.6,90,-155",
    ),
    ('4', 5.0, "M3.3,10 L0,2.8 L5,2.8 M3.9,6.4 L3.9,0"),
    (
        '5',
        5.0,
        "M4.7,10 L0.7,10 L0.45,5.35 A2.5,3.05,2.5,3.05,130,-152",
    ),
    (
        '6',
        5.0,
        "A2.5,2.7,2.5,2.7,0,360 M0,2.7 A4.9,2.7,4.9,7.3,180,103",
    ),
    ('7', 5.0, "M0,10 L5,10 L1.4,0"),
    (
        '8',
        5.0,
        "A2.5,7.65,2.2,2.35,0,360 A2.5,2.65,2.5,2.65,0,360",
    ),
    (
        '9',
        5.0,
        "A2.5,7.3,2.5,2.7,0,360 M5,7.3 A0.1,7.3,4.9,7.3,0,-77",
    ),
    ('A', 6.0, "M0,0 L3,10 L6,0 M0.95,3.2 L5.05,3.2"),
    (
        'B',
        5.0,
        "M0,0 L0,10 L3.2,10 A3.2,7.6,1.6,2.4,90,-90 L0,5.2 M0,5.2 L3.3,5.2 \
         A3.3,2.6,1.7,2.6,90,-90 L0,0",
    ),
    ('C', 5.0, "A3,5,3,5,47,313"),
    ('D', 5.0, "M0,0 L0,10 L2,10 A2,5,3,5,90,-90 L0,0"),
    ('E', 4.5, "M4.5,10 L0,10 L0,0 L4.5,0 M0,5.2 L3.6,5.2"),
    ('F', 4.5, "M4.5,10 L0,10 L0,0 M0,5.2 L3.6,5.2"),
    ('G', 5.5, "A3.1,5,3.1,5,48,318 L5.45,4.5 L3.2,4.5"),
    ('H', 5.0, "M0,0 L0,10 M5,0 L5,10 M0,5.2 L5,5.2"),
    ('I', 0.0, "M0,0 L0,10"),
    ('J', 4.5, "M4.5,10 L4.5,2.4 A2.25,2.4,2.25,2.4,0,-172"),
    ('K', 5.0, "M0,0 L0,10 M5,10 L0,3.6 M1.9,6.03 L5,0"),
    ('L', 4.5, "M0,10 L0,0 L4.5,0"),
    ('M', 6.5, "M0,0 L0,10 L3.25,2.2 L6.5,10 L6.5,0"),
    ('N', 5.0, "M0,0 L0,10 L5,0 L5,10"),
    ('O', 6.5, "A3.25,5,3.25,5,0,360"),
    (
        'P',
        5.0,
        "M0,0 L0,10 L3.2,10 A3.2,7.3,1.8,2.7,90,-90 L0,4.6",
    ),
    ('Q', 6.5, "A3.25,5,3.25,5,0,360 M3.9,2.3 L6.5,-0.4"),
    (
        'R',
        5.0,
        "M0,0 L0,10 L3.2,10 A3.2,7.4,1.8,2.6,90,-90 L0,4.8 M2.7,4.8 L5,0",
    ),
    (
        'S',
        5.0,
        "A2.5,7.55,2.4,2.45,18,270 A2.5,2.55,2.5,2.55,90,-162",
    ),
    ('T', 5.0, "M0,10 L5,10 M2.5,10 L2.5,0"),
    ('U', 5.0, "M0,10 L0,2.5 A2.5,2.5,2.5,2.5,180,360 L5,10"),
    ('V', 6.0, "M0,10 L3,0 L6,10"),
    ('W', 8.0, "M0,10 L2,0 L4,7.5 L6,0 L8,10"),
    ('X', 5.5, "M0,0 L5.5,10 M0,10 L5.5,0"),
    ('Y', 6.0, "M0,10 L3,5 L6,10 M3,5 L3,0"),
    ('Z', 5.0, "M0,10 L5,10 L0,0 L5,0"),
    (' ', 3.0, ""),
    ('.', 0.0, "M0,0 L0,0.01"),
    (',', 0.8, "M0.8,0.4 L0,-1.8"),
    (':', 0.0, "M0,0 L0,0.01 M0,6 L0,6.01"),
    ('-', 3.0, "M0,5 L3,5"),
    ('+', 5.0, "M0,5 L5,5 M2.5,2.5 L2.5,7.5"),
    ('=', 5.0, "M0,3.6 L5,3.6 M0,6.4 L5,6.4"),
    ('/', 4.0, "M0,-0.5 L4,10.5"),
    ('(', 2.0, "A4.5,5,4.5,6.4,128,232"),
    (')', 2.0, "A-2.5,5,4.5,6.4,52,-52"),
    ('°', 2.5, "A1.25,8.75,1.25,1.25,0,360"),
    ('′', 1.0, "M1,10 L0,7.4"),
    ('″', 3.0, "M1,10 L0,7.4 M3,10 L2,7.4"),
    ('×', 4.0, "M0,3 L4,7 M0,7 L4,3"),
    ('·', 0.0, "M0,5 L0,5.01"),
    ('⌀', 6.0, "A3,5,3,3.3,0,360 M0.5,0.7 L5.5,9.3"),
    (
        '%',
        6.0,
        "M0,0 L6,10 A1.3,8.2,1.3,1.8,0,360 A4.7,1.8,1.3,1.8,0,360",
    ),
    (
        '?',
        4.5,
        "A2.25,7.6,2.25,2.4,165,-62 L2.25,4.2 L2.25,2.8 M2.25,0 L2.25,0.01",
    ),
    ('!', 0.0, "M0,10 L0,3 M0,0 L0,0.01"),
    ('∞', 8.0, "A2,5,2,2.2,0,360 A6,5,2,2.2,180,-180"),
    ('\'', 0.0, "M0,10 L0,7.4"),
];

/// The line of the pen, the gap between two characters and the width of a space, in tenths
/// of the height of a capital.
const PEN: f32 = 1.0;
const GAP: f32 = 2.0;
/// The steps in which an arc is drawn, in degrees.
const ARC_STEP: f32 = 7.5;

/// A character: its width and the lines that draw it.
struct Glyph {
    width: f32,
    strokes: Vec<Vec<Vec2>>,
}

/// The characters that the sheet is lettered with.
#[derive(Resource)]
pub struct Lettering {
    glyphs: HashMap<char, Glyph>,
}

impl Default for Lettering {
    fn default() -> Self {
        let glyphs = GLYPHS
            .iter()
            .map(|&(character, width, path)| {
                let strokes = strokes(path);
                (character, Glyph { width, strokes })
            })
            .collect();
        Self { glyphs }
    }
}

/// The lines of a path of [`GLYPHS`].
fn strokes(path: &str) -> Vec<Vec<Vec2>> {
    let mut lines: Vec<Vec<Vec2>> = Vec::new();
    let numbers = |token: &str| -> Vec<f32> {
        token[1..]
            .split(',')
            .filter_map(|number| number.parse().ok())
            .collect()
    };
    for token in path.split_whitespace() {
        let args = numbers(token);
        match (token.as_bytes()[0], args.as_slice()) {
            (b'M', &[x, y]) => lines.push(vec![Vec2::new(x, y)]),
            (b'L', &[x, y]) => {
                if let Some(line) = lines.last_mut() {
                    line.push(Vec2::new(x, y));
                }
            }
            (b'A', &[x, y, across, up, from, to]) => {
                let steps = (((to - from).abs() / ARC_STEP).ceil() as usize).max(4);
                let points = (0..=steps).map(|step| {
                    let angle = (from + (to - from) * step as f32 / steps as f32).to_radians();
                    Vec2::new(x + across * angle.cos(), y + up * angle.sin())
                });
                let points: Vec<Vec2> = points.collect();
                let joined = lines
                    .last()
                    .and_then(|line| line.last())
                    .is_some_and(|end| end.distance(points[0]) < 0.6);
                match lines.last_mut() {
                    Some(line) if joined => line.extend(points),
                    _ => lines.push(points),
                }
            }
            _ => {}
        }
    }
    lines
}

impl Lettering {
    fn glyph(&self, character: char) -> Option<&Glyph> {
        let upper = character.to_ascii_uppercase();
        self.glyphs.get(&upper).or_else(|| self.glyphs.get(&'?'))
    }

    /// How far the pen moves for a character, in tenths of the height of a capital.
    fn advance(&self, character: char) -> f32 {
        self.glyph(character)
            .map_or(0.0, |glyph| glyph.width + PEN + GAP)
    }

    /// The width of a line of text whose capitals are this high, from the left edge of its
    /// first line of ink to the right edge of its last one.
    pub fn width(&self, text: &str, height: f32) -> f32 {
        let pen: f32 = text.chars().map(|character| self.advance(character)).sum();
        (pen - GAP).max(0.0) * height / 10.0
    }

    /// The lines that draw a text whose capitals are this high, on a baseline along `+x` with
    /// the capitals up `+y`, with the middle of its baseline at the origin.
    pub fn lines(&self, text: &str, height: f32) -> Vec<Vec<Vec2>> {
        let scale = height / 10.0;
        let shift = -self.width(text, height) / 2.0;
        // The middle of the first line of ink is half a pen in from the left edge.
        let mut pen = PEN / 2.0;
        let mut lines = Vec::new();
        for character in text.chars() {
            let Some(glyph) = self.glyph(character) else {
                continue;
            };
            for stroke in &glyph.strokes {
                let line = stroke
                    .iter()
                    .map(|point| Vec2::new(shift + (pen + point.x) * scale, point.y * scale));
                lines.push(line.collect());
            }
            pen += glyph.width + PEN + GAP;
        }
        lines
    }
}

/// Where a text lies on the ground: the point it is set from, the direction of its baseline,
/// and the height above the ground at which it is drawn.
#[derive(Clone, Copy, Debug)]
pub struct Setting {
    pub at: Vec3,
    /// The direction of the baseline on the ground, as a unit vector of `x` and `z`.
    pub along: Vec3,
}

impl Setting {
    /// The point of the ground for a point of the text.
    fn place(&self, point: Vec2) -> Vec3 {
        // The capitals stand up to the left of the baseline, seen from above.
        let up = Vec3::new(self.along.z, 0.0, -self.along.x);
        self.at + self.along * point.x + up * point.y
    }
}

impl Draft {
    /// Draws lines of the pen, `width` wide, with round ends, where `setting` lays them on the
    /// ground.
    pub fn pen(&mut self, lines: &[Vec<Vec2>], setting: Setting, width: f32, color: Color) {
        for line in lines {
            let points: Vec<Vec3> = line.iter().map(|&point| setting.place(point)).collect();
            for pair in points.windows(2) {
                if pair[0].distance(pair[1]) > 0.001 {
                    self.line(pair[0], pair[1], width, color);
                }
            }
            // Round ends and joints, as a pen leaves them.
            for &point in &points {
                self.dot(point, width / 2.0, color);
            }
        }
    }

    /// A small round dot lying flat.
    pub fn dot(&mut self, center: Vec3, radius: f32, color: Color) {
        const SIDES: usize = 8;
        let rim: Vec<Vec3> = (0..SIDES)
            .map(|side| {
                let (sin, cos) = (side as f32 / SIDES as f32 * TAU).sin_cos();
                center + Vec3::new(cos, 0.0, sin) * radius
            })
            .collect();
        self.face(&rim, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_character_has_lines_but_the_space() {
        let lettering = Lettering::default();
        for &(character, _, _) in GLYPHS {
            let lines = lettering.lines(&character.to_string(), 1.0);
            assert_eq!(lines.is_empty(), character == ' ', "{character:?}");
        }
    }

    #[test]
    fn a_text_is_as_wide_as_its_lines_of_ink() {
        let lettering = Lettering::default();
        let lines = lettering.lines("H", 10.0);
        let (left, right) = lines
            .iter()
            .flatten()
            .fold((f32::MAX, f32::MIN), |(left, right), point| {
                (left.min(point.x), right.max(point.x))
            });
        // The middle of the lines of an H is 5 apart, and the pen adds half a line on each side.
        assert!((right - left - 5.0).abs() < 1e-4);
        assert!((left + right).abs() < 1e-4);
        assert!((lettering.width("H", 10.0) - 6.0).abs() < 1e-4);
    }

    #[test]
    fn a_text_is_set_from_its_middle() {
        let lettering = Lettering::default();
        let lines = lettering.lines("26°34′", 0.5);
        let (left, right) = lines
            .iter()
            .flatten()
            .fold((f32::MAX, f32::MIN), |(left, right), point| {
                (left.min(point.x), right.max(point.x))
            });
        let half_pen = 0.5 / 20.0;
        assert!((left - half_pen + right + half_pen).abs() < 1e-3);
    }

    #[test]
    fn small_letters_are_drawn_as_capitals() {
        let lettering = Lettering::default();
        assert_eq!(
            lettering.lines("sector", 1.0),
            lettering.lines("SECTOR", 1.0)
        );
    }
}
