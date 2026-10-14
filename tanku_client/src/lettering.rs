//! The lettering that is drawn on the sheet itself, as a draftsman letters a plan: the
//! figures and the capitals of what is written in the void around the corridor.
//!
//! Each character is drawn with the pen of the drawing, in strokes, after lettering type B of
//! ISO 3098: the line is a tenth of the height of a capital, two lines part two characters and
//! six part two words. The strokes are those of the font of the interface, which is made from
//! the same file, so that the sheet and its margin are lettered by one hand. The floor writes
//! in capitals, as a plan does.

use std::{collections::HashMap, f32::consts::TAU};

use bevy::prelude::*;

use crate::draft::Draft;

/// The strokes of every character, in tenths of the height of a capital, with the baseline
/// at 0 and the line of the capitals at 10. The file says how they are written. The floor
/// draws the characters that are drawn as themselves; those made of others, the accented
/// letters, are the interface's.
pub const STROKES: &str = include_str!("../assets/fonts/tanku_lettering/strokes.txt");

/// The character that a line of [`STROKES`] is about, written as itself or as U+ and its
/// code.
pub fn character(spec: &str) -> Option<char> {
    match spec.strip_prefix("U+") {
        Some(code) if !code.is_empty() => {
            u32::from_str_radix(code, 16).ok().and_then(char::from_u32)
        }
        _ => {
            let mut chars = spec.chars();
            let first = chars.next()?;
            chars.next().is_none().then_some(first)
        }
    }
}

/// The lines of [`STROKES`] that say something: neither empty nor notes.
pub fn written() -> impl Iterator<Item = &'static str> {
    STROKES
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
}

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
        let mut glyphs = HashMap::new();
        for line in written() {
            let mut words = line.split_whitespace();
            let (Some(spec), Some(width)) = (words.next(), words.next()) else {
                continue;
            };
            // Marks, and characters made of others, are not drawn as themselves.
            let (Some(character), Ok(width)) = (character(spec), width.parse()) else {
                continue;
            };
            let path: Vec<&str> = words.collect();
            let strokes = strokes(&path.join(" "));
            glyphs.insert(character, Glyph { width, strokes });
        }
        Self { glyphs }
    }
}

/// The lines of a path of [`STROKES`].
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
    fn every_character_has_lines_but_the_spaces() {
        let lettering = Lettering::default();
        assert!(lettering.glyphs.len() > 100);
        for (character, glyph) in &lettering.glyphs {
            let space = matches!(character, ' ' | '\u{a0}');
            assert_eq!(glyph.strokes.is_empty(), space, "{character:?}");
        }
        // The floor letters in capitals, so it needs every capital, figure and sign of a plan.
        for character in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .,:-+=/()%?!'\u{b0}\u{b7}\u{d7}\u{2032}\u{2033}\u{221e}\u{2300}".chars() {
            assert!(lettering.glyphs.contains_key(&character), "{character:?}");
        }
    }

    #[test]
    fn every_letter_that_a_player_types_for_others_is_drawn() {
        // What the interface draws: the characters of the file, as themselves or of others.
        let drawn: Vec<char> = written()
            .filter_map(|line| line.split_whitespace().next().and_then(character))
            .collect();
        for letter in (' '..='\u{17f}').filter(|letter| tanku_core::protocol::lettered(*letter)) {
            assert!(drawn.contains(&letter), "{letter:?} is typed and not drawn");
        }
        // And the dots that stand for the letters of a hidden password.
        assert!(drawn.contains(&'\u{2022}'));
    }

    #[test]
    fn a_character_is_written_as_itself_or_by_its_code() {
        assert_eq!(character("A"), Some('A'));
        assert_eq!(character("\u{e9}"), Some('\u{e9}'));
        assert_eq!(character("U+0020"), Some(' '));
        assert_eq!(character("U+0023"), Some('#'));
        assert_eq!(character("U+"), None);
        assert_eq!(character("mark"), None);
        assert_eq!(character(""), None);
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
