//! The colors of the game and of its interface, and the lettering of the interface: a dark
//! sheet drawn in pale lines, on which only tanks and bullets are bright.
//!
//! Nothing is lit, so a color on screen is the color written here.

use bevy::{prelude::*, text::LetterSpacing, ui::UiSystems};

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
/// The fill of a square that tells a loss for a moment.
pub const DANGER_FILL: Color = hex(0x2B1418);
pub const GOOD: Color = hex(0x4DD980);
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

/// The lettering of a drawing: a narrow face, for everything that is read as words. Its
/// regular weight is too thin for the small labels of the interface, so the medium one stands
/// in for it.
const LETTERING: &str = "Barlow Semi Condensed";
const PLAIN: FontWeight = FontWeight::MEDIUM;
const HEAVY: FontWeight = FontWeight::SEMIBOLD;
/// A face whose characters all have the same width, for numbers: counts, times and versions.
/// It draws B like 8, S like 5 and Z like 2, so nothing that mixes letters and figures is set
/// in it: a seed that is read wrong gives another map.
const FIGURES: &str = "Share Tech Mono";
/// How far apart the letters of a label or of a title stand, as a share of their size.
const TRACKING: f32 = 0.14;

/// The font files. The game carries them in itself, so that there is nothing to load, on a
/// desktop or in a browser. Their licenses are next to them.
const FACES: [&[u8]; 3] = [
    include_bytes!("../assets/fonts/barlow_semi_condensed/BarlowSemiCondensed-Medium.ttf"),
    include_bytes!("../assets/fonts/barlow_semi_condensed/BarlowSemiCondensed-SemiBold.ttf"),
    include_bytes!("../assets/fonts/share_tech_mono/ShareTechMono-Regular.ttf"),
];

/// Keeps the faces at hand: text asks for them by the name of their family.
#[derive(Resource)]
struct Faces(#[expect(dead_code, reason = "held so that the fonts stay loaded")] [Handle<Font>; 3]);

pub fn plugin(app: &mut App) {
    let mut fonts = app.world_mut().resource_mut::<Assets<Font>>();
    let faces = FACES.map(|file| fonts.add(Font::from_bytes(file.to_vec())));
    app.insert_resource(Faces(faces))
        .add_systems(PostUpdate, follow_edges.before(UiSystems::Prepare));
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

/// Running text: a sentence, a hint.
pub fn text(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    lettered(content.into(), (LETTERING, PLAIN), (size, 0.0), color)
}

/// A label: the name of what stands next to it, in capitals that stand apart.
pub fn label(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    let face = (LETTERING, PLAIN);
    lettered(content.into().to_uppercase(), face, (size, TRACKING), color)
}

/// A title, or the name of a button: a heavier label.
pub fn title(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    let face = (LETTERING, HEAVY);
    lettered(content.into().to_uppercase(), face, (size, TRACKING), color)
}

/// A number, or a word that stands among numbers.
pub fn figure(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    let face = (FIGURES, FontWeight::NORMAL);
    lettered(content.into(), face, (size, 0.0), color)
}

/// A seed: letters and figures that must not be taken for each other, so they are set in the
/// lettering, heavy and apart.
pub fn code(content: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    lettered(content.into(), (LETTERING, HEAVY), (size, TRACKING), color)
}

/// How a text is set: as a label, a title, or a number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Label,
    Title,
    Figure,
}

impl Face {
    fn set(self, content: String, size: f32, color: Color) -> impl Bundle {
        let (face, tracking) = match self {
            Face::Label => ((LETTERING, PLAIN), TRACKING),
            Face::Title => ((LETTERING, HEAVY), TRACKING),
            Face::Figure => ((FIGURES, FontWeight::NORMAL), 0.0),
        };
        let content = match self {
            Face::Label | Face::Title => content.to_uppercase(),
            Face::Figure => content,
        };
        lettered(content, face, (size, tracking), color)
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
    let content: String = content.into();
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
            face.set(content.clone(), size, ink),
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
            (marker, Edged, face.set(content.clone(), size, color)),
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
    fn the_font_files_hold_the_faces_that_text_asks_for() {
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
        assert_eq!(weights(LETTERING), [PLAIN.0 as f32, HEAVY.0 as f32]);
        assert_eq!(weights(FIGURES), [FontWeight::NORMAL.0 as f32]);
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
