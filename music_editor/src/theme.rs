//! The colors and the lettering of the editor, which are those of the interface of the game:
//! a dark sheet drawn in pale lines, with one accent, the blue of the players.
//!
//! The game keeps its own copy in `tanku_client/src/theme.rs`. A color that changes there
//! changes here by hand.

use bevy::{prelude::*, text::LetterSpacing};

/// A color from its hexadecimal code.
const fn hex(code: u32) -> Color {
    Color::srgb(
        ((code >> 16) & 0xFF) as f32 / 255.0,
        ((code >> 8) & 0xFF) as f32 / 255.0,
        (code & 0xFF) as f32 / 255.0,
    )
}

/// The sheet, and the ground of a drawing on it.
pub const VOID: Color = hex(0x0A0F16);
pub const FLOOR: Color = hex(0x17212E);
/// The lines of a grid, which are faint.
pub const GRID: Color = hex(0x8FB0E0);
/// What is written by hand is drawn in this line, solid. What a rule derives is drawn in it
/// too, in dashes.
pub const LINE: Color = hex(0xC9D8EC);
pub const HATCH: Color = hex(0x6F86A8);

/// The background of a panel.
pub const SURFACE: Color = Color::srgba(0.039, 0.059, 0.086, 0.86);
pub const SURFACE_RAISED: Color = hex(0x111A26);
pub const SURFACE_HOVERED: Color = hex(0x1D2A3C);
/// The lines of the interface: the edges of panels, cells and buttons.
pub const RULE: Color = hex(0x5E7391);
pub const RULE_DIM: Color = hex(0x2E3B4C);
pub const TEXT: Color = hex(0xE6EBF2);
/// Text that says less than what stands next to it: a hint, the value of a row.
pub const TEXT_SOFT: Color = hex(0xB9C9DE);
pub const TEXT_DIM: Color = hex(0x8C9BB0);
/// Text on a button filled with the accent.
pub const TEXT_ON_ACCENT: Color = VOID;
/// The selection.
pub const ACCENT: Color = hex(0x2980F2);
pub const ACCENT_HOVERED: Color = hex(0x5C9FF5);
/// What is not saved yet.
pub const UNSAVED: Color = hex(0xFF8C26);
pub const DANGER: Color = hex(0xF24D40);
/// A check that holds, as a mark and as text.
pub const GOOD: Color = hex(0x4DD980);
pub const GOOD_TEXT: Color = hex(0x8DF0B4);

/// The lettering of a drawing: a narrow face, for everything that is read as words.
const LETTERING: &str = "Barlow Semi Condensed";
const PLAIN: FontWeight = FontWeight::MEDIUM;
const HEAVY: FontWeight = FontWeight::SEMIBOLD;
/// A face whose characters all have the same width, for numbers. It draws B like 8, S like 5
/// and Z like 2, so nothing that mixes letters and figures is set in it: a line of the file
/// is in the lettering.
const FIGURES: &str = "Share Tech Mono";
/// How far apart the letters of a label or of a title stand, as a share of their size.
const TRACKING: f32 = 0.14;

/// The font files of the game, which the editor carries in itself too.
const FACES: [&[u8]; 3] = [
    include_bytes!(
        "../../tanku_client/assets/fonts/barlow_semi_condensed/BarlowSemiCondensed-Medium.ttf"
    ),
    include_bytes!(
        "../../tanku_client/assets/fonts/barlow_semi_condensed/BarlowSemiCondensed-SemiBold.ttf"
    ),
    include_bytes!("../../tanku_client/assets/fonts/share_tech_mono/ShareTechMono-Regular.ttf"),
];

/// Keeps the faces at hand: text asks for them by the name of their family.
#[derive(Resource)]
struct Faces(#[expect(dead_code, reason = "held so that the fonts stay loaded")] [Handle<Font>; 3]);

pub fn plugin(app: &mut App) {
    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let faces = FACES.map(|file| fonts.add(Font::from_bytes(file.to_vec())));
    app.insert_resource(Faces(faces));
}

fn lettered(
    content: String,
    (family, weight): (&str, FontWeight),
    (size, tracking): (f32, f32),
    color: Color,
) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font: family.into(),
            font_size: FontSize::Px(size),
            weight,
            ..default()
        },
        // Whole pixels: small letters would stand unevenly apart otherwise.
        LetterSpacing::Px((size * tracking).round()),
        TextColor(color),
    )
}

/// Text that stays on its line: every piece of the editor has its place.
fn on_a_line(words: impl Bundle) -> impl Bundle {
    (words, TextLayout::new(Justify::Left, LineBreak::NoWrap))
}

/// Running text: the name of a row, a value, a hint that fits its line.
pub fn text(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    on_a_line(lettered(
        content.into(),
        (LETTERING, PLAIN),
        (size, 0.0),
        color,
    ))
}

/// A sentence that goes on to the next line where its box ends.
pub fn wrapped(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        lettered(content.into(), (LETTERING, PLAIN), (size, 0.0), color),
        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
    )
}

/// Heavier text: a value that stands out in a row.
pub fn strong(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    on_a_line(lettered(
        content.into(),
        (LETTERING, HEAVY),
        (size, 0.0),
        color,
    ))
}

/// A line of the file, or a piece of one. It is in the lettering: the face of the figures
/// would draw motif B like degree 8.
pub fn written(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    text(content, size, color)
}

/// A label: the name of what stands next to it, in capitals that stand apart.
pub fn label(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    let face = (LETTERING, PLAIN);
    on_a_line(lettered(
        content.into().to_uppercase(),
        face,
        (size, TRACKING),
        color,
    ))
}

/// A title, or the name of a button: a heavier label.
pub fn title(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    let face = (LETTERING, HEAVY);
    on_a_line(lettered(
        content.into().to_uppercase(),
        face,
        (size, TRACKING),
        color,
    ))
}

/// A number, or a symbol of a bar among others.
pub fn figure(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    let face = (FIGURES, FontWeight::NORMAL);
    on_a_line(lettered(content.into(), face, (size, 0.0), color))
}
