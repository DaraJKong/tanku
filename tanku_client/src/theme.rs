//! The colors of the game and of its interface: dark and flat, with one accent per player.

use bevy::prelude::*;

pub const VOID: Color = Color::srgb(0.02, 0.022, 0.03);
pub const FLOOR: Color = Color::srgb(0.16, 0.17, 0.2);
pub const WALL: Color = Color::srgb(0.42, 0.45, 0.52);
pub const BLOCK: Color = Color::srgb(0.5, 0.4, 0.28);
pub const CRATE: Color = Color::srgb(0.75, 0.16, 0.1);
pub const DARK_METAL: Color = Color::srgb(0.1, 0.105, 0.12);

/// The hue reserved for the first player. No enemy uses it.
pub const PLAYER: Color = Color::srgb(0.16, 0.5, 0.95);
pub const PLAYER_BULLET: Color = Color::srgb(0.45, 0.95, 1.0);
pub const ENEMY_BULLET: Color = Color::srgb(1.0, 0.55, 0.15);

pub const SURFACE: Color = Color::srgb(0.085, 0.09, 0.105);
pub const SURFACE_RAISED: Color = Color::srgb(0.13, 0.14, 0.165);
pub const SURFACE_HOVERED: Color = Color::srgb(0.19, 0.205, 0.24);
pub const SCRIM: Color = Color::srgba(0.02, 0.022, 0.03, 0.82);
pub const TEXT: Color = Color::srgb(0.9, 0.92, 0.95);
pub const TEXT_DIM: Color = Color::srgb(0.55, 0.58, 0.64);
pub const ACCENT: Color = PLAYER;
pub const DANGER: Color = Color::srgb(0.95, 0.3, 0.25);
pub const GOOD: Color = Color::srgb(0.3, 0.85, 0.5);
/// Shields: the disk under the turret of a tank that has one.
pub const SHIELD: Color = Color::WHITE;
/// A flag that nobody has captured.
pub const FLAG: Color = Color::srgb(0.66, 0.68, 0.72);

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
