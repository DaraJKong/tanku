//! The colors of the game and of its interface: a dark sheet drawn in pale lines, on which
//! only tanks and bullets are bright.
//!
//! Nothing is lit, so a color on screen is the color written here.

use bevy::prelude::*;

/// A color from its hexadecimal code.
const fn hex(code: u32) -> Color {
    Color::srgb(
        ((code >> 16) & 0xFF) as f32 / 255.0,
        ((code >> 8) & 0xFF) as f32 / 255.0,
        (code & 0xFF) as f32 / 255.0,
    )
}

pub const VOID: Color = hex(0x0A0F16);
pub const FLOOR: Color = hex(0x17212E);
/// The lines of the 1 m grid on the floor, which are faint, and of its 5 m grid, less so.
pub const GRID: Color = hex(0x8FB0E0);
pub const GRID_STRENGTH: (f32, f32) = (0.1, 0.2);
/// The lines of the drawing: the edges of walls, obstacles and tanks.
pub const LINE: Color = hex(0xC9D8EC);
pub const WALL: Color = hex(0x6B7F9E);
pub const WALL_SIDE: Color = hex(0x33425A);
pub const BLOCK: Color = hex(0x2A3A51);
pub const BLOCK_SIDE: Color = hex(0x222F43);
pub const BLOCK_LINE: Color = hex(0xA9BCD6);
pub const HATCH: Color = hex(0x6F86A8);
pub const CRATE: Color = hex(0x371719);
pub const CRATE_SIDE: Color = hex(0x2A1214);
/// The parts of a tank that aren't in its color: tracks, barrel, drum and vents.
pub const DARK_METAL: Color = hex(0x0B1119);

/// The hue reserved for the first player. No enemy uses it.
pub const PLAYER: Color = hex(0x2980F2);
pub const PLAYER_BULLET: Color = hex(0x73F2FF);
pub const ENEMY_BULLET: Color = hex(0xFF8C26);

/// The background of a panel, through which the game shows a little.
pub const SURFACE: Color = Color::srgba(0.039, 0.059, 0.086, 0.86);
pub const SURFACE_RAISED: Color = hex(0x111A26);
pub const SURFACE_HOVERED: Color = hex(0x1D2A3C);
/// The lines of the interface: the edges of panels, cells and buttons.
pub const RULE: Color = hex(0x5E7391);
pub const RULE_DIM: Color = hex(0x2E3B4C);
pub const SCRIM: Color = Color::srgba(0.039, 0.059, 0.086, 0.8);
pub const TEXT: Color = hex(0xE6EBF2);
pub const TEXT_DIM: Color = hex(0x8C9BB0);
/// Text on a button filled with the accent.
pub const TEXT_ON_ACCENT: Color = VOID;
pub const ACCENT: Color = PLAYER;
pub const ACCENT_HOVERED: Color = hex(0x5C9FF5);
pub const DANGER: Color = hex(0xF24D40);
pub const GOOD: Color = hex(0x4DD980);
/// Shields: the disk under the turret of a tank that has one, and the bubble when it is up.
pub const SHIELD: Color = Color::WHITE;
/// A flag that nobody has captured.
pub const FLAG: Color = hex(0xA8ADB8);

/// How much of its light a sector keeps until a player enters it, and a tank until it wakes
/// up. Darker, but of the same hue: the hue is how players read a tank.
pub const DORMANT: f32 = 0.45;

/// The light of what is dormant, to multiply a color with.
pub fn dormant() -> Color {
    LinearRgba::rgb(DORMANT, DORMANT, DORMANT).into()
}

/// How much darker the sides of a solid are than its top. With no light, this is what shows
/// its height.
const SIDE_SHADE: f32 = 0.6;

/// The color of the sides of a solid whose top has this color.
pub fn side(top: Color) -> Color {
    let top = top.to_srgba();
    Color::srgb(
        top.red * SIDE_SHADE,
        top.green * SIDE_SHADE,
        top.blue * SIDE_SHADE,
    )
}

/// One color laid over another, as a line of this strength is over the sheet.
pub fn over(ground: Color, line: Color, strength: f32) -> Color {
    let (ground, line) = (ground.to_srgba(), line.to_srgba());
    let blend = |from: f32, to: f32| from + (to - from) * strength;
    Color::srgb(
        blend(ground.red, line.red),
        blend(ground.green, line.green),
        blend(ground.blue, line.blue),
    )
}

pub fn text(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_read_from_their_codes() {
        let player = PLAYER.to_srgba();
        assert_eq!(
            (player.red, player.green, player.blue),
            (41.0 / 255.0, 128.0 / 255.0, 242.0 / 255.0)
        );
    }

    #[test]
    fn sides_are_darker_and_keep_their_hue() {
        let (top, side) = (Hsla::from(PLAYER), Hsla::from(side(PLAYER)));
        assert!((top.hue - side.hue).abs() < 0.5);
        assert!(side.lightness < top.lightness);
    }

    #[test]
    fn a_line_at_full_strength_hides_the_ground() {
        assert_eq!(over(FLOOR, GRID, 1.0).to_srgba(), GRID.to_srgba());
        assert_eq!(over(FLOOR, GRID, 0.0).to_srgba(), FLOOR.to_srgba());
    }
}
