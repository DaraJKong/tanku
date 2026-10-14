//! The colors of the game and of its interface, and the lettering of the interface: a dark
//! sheet drawn in pale lines, on which only tanks and bullets are bright.
//!
//! Nothing is lit, so a color on screen is the color written here.

use bevy::{prelude::*, text::LineHeight, ui::UiSystems};

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
/// The marks that a fight leaves on the floor, which are fainter than the drawing itself.
pub const SCAR: Color = hex(0x8FA6C4);

/// The color of the players' tanks, whatever their seat: a number tells the players apart.
/// No enemy uses this hue.
pub const PLAYER: Color = hex(0x2980F2);
/// Every bullet is white, whoever fired it: every bullet destroys a player's tank.
pub const BULLET: Color = hex(0xFFFFFF);
/// The fire of a blast and of a tank that is destroyed.
pub const FIRE: Color = hex(0xFF8C26);

pub const SURFACE_RAISED: Color = hex(0x111A26);
/// The ground of a block of the sheet, a title block or a parts list, and of a balloon,
/// through which the lettering of the void doesn't show. The engine blends colors in linear
/// light, where a ground of 92 % lets a quarter of a pale line through to the eye: 98 % lets
/// as little through as 92 % does in a browser.
pub const BLOCK_GROUND: Color = Color::srgba(0.039, 0.059, 0.086, 0.98);
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
/// The fill of a square that tells a loss for a moment.
pub const DANGER_FILL: Color = hex(0x2B1418);
pub const GOOD: Color = hex(0x4DD980);
/// The fill of a square that tells a gain for a moment.
pub const GOOD_FILL: Color = hex(0x10261A);
/// Shields: the disk under the turret of a tank that has one, and the bubble when it is up.
pub const SHIELD: Color = Color::WHITE;

/// How much of its light a sector keeps until a player enters it.
pub const DORMANT: f32 = 0.45;

/// The light of what is dormant, to multiply a color with.
pub fn dormant() -> Color {
    LinearRgba::rgb(DORMANT, DORMANT, DORMANT).into()
}

/// A color as it shows in a sector that no player has entered yet.
pub fn dull(color: Color) -> Color {
    let lit = color.to_linear();
    LinearRgba::rgb(lit.red * DORMANT, lit.green * DORMANT, lit.blue * DORMANT).into()
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

/// The lettering of the interface: Tanku Lettering, the strokes that the floor is lettered
/// with made into a font, so that the sheet and its margin are lettered by one hand. It has
/// one weight, the pen of ISO 3098 type B, a tenth of the height of a capital: a larger text
/// is a heavier one. Its figures are all as wide, its 1 has a flag and its 0 is oval, so that
/// numbers don't jitter and a seed is not misread. Its letters stand as far apart as ISO 3098
/// sets them.
const LETTERING: &str = "Tanku Lettering";

/// The font file. The game carries it in itself, so that there is nothing to load, on a
/// desktop or in a browser. `dev/lettering_font.py` makes it from the strokes.
const FACE: &[u8] = include_bytes!("../assets/fonts/tanku_lettering/TankuLettering-Regular.ttf");

/// The heights of the capitals of the interface, in its pixels: the steps of ISO 3098 from
/// 3.5 mm to 20 mm, at 4.2 pixels to a millimeter of the sheet, which is a 1080p screen at
/// the size of the interface that the settings start from. The size of a text in this font is
/// the height of its capitals. Nothing is lettered smaller than the first, which reads.
///
/// The labels of fields, captions, hints and notes: 3.5 mm.
pub const LABEL: f32 = 14.8;
/// Values, buttons, the rows of a list and the figures of balloons: 5 mm.
pub const VALUE: f32 = 21.0;
/// Alerts and stamps: 7 mm.
pub const ALERT: f32 = 29.7;
/// The sector, and the figures of a report: 10 mm.
pub const LARGE: f32 = 42.0;
/// The title of a screen: 14 mm.
pub const HEADING: f32 = 59.0;
/// The name of the game: 20 mm.
pub const NAME: f32 = 84.0;

/// The pitch of the lines of a text, in heights of its capitals: those of capitals alone, and
/// those of sentences, whose small letters have tails, after ISO 3098.
const CAPITALS_PITCH: f32 = 1.25;
const SENTENCE_PITCH: f32 = 1.6;

/// How high the box of a line of capitals of this size is: its capitals stand in its middle.
pub fn capitals_height(size: f32) -> f32 {
    size * CAPITALS_PITCH
}

/// Keeps the faces at hand: text asks for them by the name of their family.
#[derive(Resource)]
struct Faces(#[expect(dead_code, reason = "held so that the fonts stay loaded")] Vec<Handle<Font>>);

pub fn plugin(app: &mut App) {
    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let faces = [FACE].into_iter();
    let faces = faces.map(|file| fonts.add(Font::from_bytes(file.to_vec())));
    let faces = Faces(faces.collect());
    app.insert_resource(faces)
        .add_systems(PostUpdate, follow_edges.before(UiSystems::Prepare));
}

fn lettered(content: String, size: f32, pitch: f32, color: Color) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font: LETTERING.into(),
            font_size: FontSize::Px(size),
            ..default()
        },
        LineHeight::RelativeToFont(pitch),
        TextColor(color),
    )
}

/// Running text: a sentence, a hint, as it is written.
pub fn text(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    lettered(content.into(), size, SENTENCE_PITCH, color)
}

/// A label: the name of what stands next to it, in capitals.
pub fn label(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    lettered(content.into().to_uppercase(), size, CAPITALS_PITCH, color)
}

/// A title, or the name of a button, in capitals: a title is told by its size.
pub fn title(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    label(content, size, color)
}

/// A number, or a word that stands among numbers, as it is written.
pub fn figure(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    lettered(content.into(), size, CAPITALS_PITCH, color)
}

/// A number alone on its line, in a box as high as its figures: its baseline is the bottom of
/// the box, to line up with what stands next to it.
pub fn lone(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    lettered(content.into(), size, 1.0, color)
}

/// A seed: letters and figures that must not be taken for each other. The lettering tells
/// them apart: the 1 has a flag and the 0 is oval.
pub fn code(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    figure(content, size, color)
}

/// How a text is set: as a label, a title, or a number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Label,
    Title,
    Figure,
}

impl Face {
    pub fn set(self, content: String, size: f32, color: Color) -> impl Bundle {
        let content = match self {
            Face::Label | Face::Title => content.to_uppercase(),
            Face::Figure => content,
        };
        lettered(content, size, CAPITALS_PITCH, color)
    }
}

/// Where the copies of a text that make its dark edge stand, in pixels from the text: two
/// pixels to each side, and one along each diagonal.
const EDGE: [(f32, f32); 8] = [
    (-2.0, 0.0),
    (2.0, 0.0),
    (0.0, -2.0),
    (0.0, 2.0),
    (-1.0, -1.0),
    (1.0, -1.0),
    (-1.0, 1.0),
    (1.0, 1.0),
];

/// A copy of a text in the color of the void, under it: a part of its dark edge.
#[derive(Component)]
struct Edge;

/// A text that has a dark edge. Its copies follow its words, and how see-through it is.
#[derive(Component)]
pub struct Edged;

/// Text that is drawn over the game, with a dark edge that keeps it readable over blocks and
/// crates, and without a panel that would hide a bullet. The edge is the text drawn again in the
/// color of the void, a little to each side, under itself. `marker` goes on the text itself.
pub fn edged(
    marker: impl Bundle,
    face: Face,
    content: impl Into<String>,
    size: f32,
    color: Color,
) -> impl Bundle {
    edged_with(
        marker,
        move |words, ink| face.set(words, size, ink),
        content.into(),
        color,
    )
}

/// Text with a dark edge, set by `set` from its words and its ink.
fn edged_with<B: Bundle>(
    marker: impl Bundle,
    set: impl Fn(String, Color) -> B,
    content: String,
    color: Color,
) -> impl Bundle {
    let ink = VOID.with_alpha(color.alpha());
    let copy = |(left, top): (f32, f32)| {
        (
            Edge,
            Node {
                position_type: PositionType::Absolute,
                left: px(left),
                top: px(top),
                ..default()
            },
            set(content.clone(), ink),
        )
    };
    (
        Node::default(),
        children![
            copy(EDGE[0]),
            copy(EDGE[1]),
            copy(EDGE[2]),
            copy(EDGE[3]),
            copy(EDGE[4]),
            copy(EDGE[5]),
            copy(EDGE[6]),
            copy(EDGE[7]),
            (marker, Edged, set(content.clone(), color)),
        ],
    )
}

/// Keeps the dark edge of a text with its words, and as see-through as it is.
fn follow_edges(
    texts: Query<
        (&Text, &TextColor, &ChildOf),
        (With<Edged>, Or<(Changed<Text>, Changed<TextColor>)>),
    >,
    families: Query<&Children>,
    mut edges: Query<(&mut Text, &mut TextColor), (With<Edge>, Without<Edged>)>,
) {
    for (text, color, parent) in &texts {
        let Ok(children) = families.get(parent.parent()) else {
            continue;
        };
        let ink = VOID.with_alpha(color.0.alpha());
        for child in children.iter() {
            let Ok((mut copy, mut copy_ink)) = edges.get_mut(child) else {
                continue;
            };
            if copy.0 != text.0 {
                copy.0.clone_from(&text.0);
            }
            if copy_ink.0 != ink {
                copy_ink.0 = ink;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::text::{FontCx, load_font_assets_into_font_collection};

    use super::*;

    #[test]
    fn the_font_file_holds_the_face_that_text_asks_for() {
        let mut app = App::new();
        app.init_resource::<Assets<Font>>()
            .init_resource::<FontCx>()
            .add_systems(Update, load_font_assets_into_font_collection);
        plugin(&mut app);
        app.update();
        let mut fonts = app.world_mut().resource_mut::<FontCx>();
        let mut weights = |family: &str| {
            let found = fonts.collection.family_id(family);
            let family = found.and_then(|id| fonts.collection.family(id));
            let faces = family.expect("a family by that name");
            let mut weights: Vec<f32> = faces
                .fonts()
                .iter()
                .map(|face| face.weight().value())
                .collect();
            weights.sort_by(f32::total_cmp);
            weights
        };
        assert_eq!(weights(LETTERING), [FontWeight::NORMAL.0 as f32]);
        // The faces that the menus were lettered in before are the music editor's alone.
        for gone in ["Barlow Semi Condensed", "Share Tech Mono"] {
            assert!(fonts.collection.family_id(gone).is_none(), "{gone}");
        }
    }

    /// FNV-1a of a text, with the ends of its lines as Unix writes them, as the script that
    /// makes the font hashes the strokes.
    fn hashed(text: &str) -> String {
        let text = text.replace("\r\n", "\n");
        let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        });
        format!("{hash:016x}")
    }

    #[test]
    fn the_font_is_made_from_the_strokes_of_the_floor() {
        // The script that makes the font writes the hash of the strokes into the name of its
        // version, which a font file keeps in UTF-16. A font that was not made again after a
        // change of the strokes doesn't have it.
        let version = format!("strokes {}", hashed(crate::lettering::STROKES));
        let wide: Vec<u8> = version.encode_utf16().flat_map(u16::to_be_bytes).collect();
        let found = FACE
            .windows(wide.len())
            .any(|window| window == wide.as_slice());
        assert!(
            found,
            "run tanku_client/dev/lettering_font.py: the font is older than the strokes"
        );
    }

    #[test]
    fn the_dark_edge_of_a_text_follows_its_words_and_its_ink() {
        #[derive(Component)]
        struct Shown;
        let mut app = App::new();
        app.add_systems(Update, follow_edges);
        let edged = edged(Shown, Face::Title, "Sector 3 cleared", 20.0, TEXT);
        app.world_mut().spawn(edged);
        app.update();
        let copies = |app: &mut App| {
            let world = app.world_mut();
            let mut copies = world.query_filtered::<(&Text, &TextColor), With<Edge>>();
            let copies = copies.iter(world);
            copies
                .map(|(text, ink)| (text.0.clone(), ink.0))
                .collect::<Vec<_>>()
        };
        // Eight copies in the color of the void, as the text is written: a title is in
        // capitals.
        let first = copies(&mut app);
        assert_eq!(first.len(), EDGE.len());
        assert!(
            first
                .iter()
                .all(|copy| *copy == ("SECTOR 3 CLEARED".into(), VOID))
        );
        // The words change and the text fades: its edge goes with it.
        let world = app.world_mut();
        let mut shown = world.query_filtered::<(&mut Text, &mut TextColor), With<Shown>>();
        let (mut text, mut ink) = shown.single_mut(world).unwrap();
        text.0 = "WAVE 2 OF 3".into();
        ink.0 = TEXT.with_alpha(0.25);
        app.update();
        let faded = (String::from("WAVE 2 OF 3"), VOID.with_alpha(0.25));
        assert!(copies(&mut app).iter().all(|copy| *copy == faded));
    }

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
