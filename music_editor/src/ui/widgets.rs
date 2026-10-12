//! The pieces that every tab is made of: ruled panels with square corners, rows, buttons,
//! steppers, and the dashes that stand for what a rule derives.

use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::{
    state::{Field, Press, View},
    theme,
};

/// The textures that the drawings are made with: a dash to repeat along a line, and a stroke
/// to repeat over what a motif doesn't use.
#[derive(Resource)]
pub struct Kit {
    dash_across: Handle<Image>,
    dash_down: Handle<Image>,
    hatch: Handle<Image>,
}

/// An image from a pattern of pixels that are white or not there.
fn pattern(width: u32, height: u32, lit: impl Fn(u32, u32) -> bool) -> Image {
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let alpha = if lit(x, y) { 255 } else { 0 };
            data.extend([255, 255, 255, alpha]);
        }
    }
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new(
        size,
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    // Lines of a drawing: nothing between a pixel that is drawn and one that isn't.
    image.sampler = ImageSampler::nearest();
    image
}

pub fn plugin(app: &mut App) {
    let mut images = app.world_mut().resource_mut::<Assets<Image>>();
    // Four pixels of line, three of nothing.
    let kit = Kit {
        dash_across: images.add(pattern(7, 1, |x, _| x < 4)),
        dash_down: images.add(pattern(1, 7, |_, y| y < 4)),
        hatch: images.add(pattern(8, 8, |x, y| (x + y) % 8 == 0)),
    };
    app.insert_resource(kit).add_systems(Update, hover);
}

/// How a button looks when the cursor is on it, and when it isn't.
#[derive(Component, Clone, Copy)]
pub struct Look {
    fill: Color,
    hovered: Color,
}

impl Look {
    pub fn new(fill: Color, hovered: Color) -> Self {
        Self { fill, hovered }
    }

    /// The look of a cell of a grid: nothing, until the cursor is on it.
    pub fn cell() -> Self {
        Self {
            fill: Color::NONE,
            hovered: theme::GRID.with_alpha(0.18),
        }
    }
}

/// Lights the button under the cursor.
fn hover(mut buttons: Query<(&Interaction, &Look, &mut BackgroundColor), Changed<Interaction>>) {
    for (interaction, look, mut color) in &mut buttons {
        color.0 = match interaction {
            Interaction::None => look.fill,
            _ => look.hovered,
        };
    }
}

/// What a button is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A button like any other.
    Plain,
    /// The one that is selected: filled with the accent.
    Accent,
    /// The one that is on among those of the game: place, fight, threat.
    Lit,
    /// A row of a list.
    Row,
    /// The row of a list that is selected.
    Picked,
    /// Something that isn't there yet, or silence: edged in dashes.
    Hollow,
}

impl Kind {
    /// Its fill, its fill under the cursor, its edge and the color of its words.
    fn colors(self) -> (Color, Color, Color, Color) {
        match self {
            Self::Plain => (
                theme::SURFACE_RAISED,
                theme::SURFACE_HOVERED,
                theme::RULE,
                theme::TEXT,
            ),
            Self::Accent => (
                theme::ACCENT,
                theme::ACCENT_HOVERED,
                theme::ACCENT,
                theme::TEXT_ON_ACCENT,
            ),
            Self::Lit => (theme::LINE, theme::TEXT, theme::LINE, theme::TEXT_ON_ACCENT),
            Self::Row => (
                Color::NONE,
                theme::SURFACE_RAISED,
                theme::RULE_DIM,
                theme::TEXT,
            ),
            Self::Picked => (
                theme::SURFACE_HOVERED,
                theme::SURFACE_HOVERED,
                theme::RULE_DIM,
                theme::TEXT,
            ),
            Self::Hollow => (
                Color::NONE,
                theme::SURFACE_RAISED,
                Color::NONE,
                theme::TEXT_SOFT,
            ),
        }
    }

    pub fn text(self) -> Color {
        self.colors().3
    }

    /// The one that is selected among several, or one of the others.
    pub fn chosen(chosen: bool) -> Self {
        if chosen { Self::Accent } else { Self::Plain }
    }

    /// The one that is on among those of the game, or one of the others.
    pub fn lit(lit: bool) -> Self {
        if lit { Self::Lit } else { Self::Plain }
    }
}

/// The parts of a button that don't depend on what is in it.
pub fn pressed(press: Press, kind: Kind) -> impl Bundle {
    let (fill, hovered, edge, _) = kind.colors();
    (
        Button,
        press,
        Look { fill, hovered },
        BackgroundColor(fill),
        BorderColor::all(edge),
    )
}

/// A node in the middle of which its content stands.
pub fn centered(width: Val, height: f32) -> Node {
    Node {
        width,
        height: px(height),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border: UiRect::all(px(1)),
        flex_shrink: 0.0,
        ..default()
    }
}

/// A button with a name in capitals.
pub fn button(
    parent: &mut ChildSpawnerCommands,
    name: &str,
    press: Press,
    kind: Kind,
    size: (Val, f32),
) {
    let node = Node {
        padding: UiRect::horizontal(px(12)),
        ..centered(size.0, size.1)
    };
    parent
        .spawn((pressed(press, kind), node))
        .with_children(|button| {
            button.spawn(theme::title(name, 13.0, kind.text()));
            if kind == Kind::Hollow {
                dashed_frame(button, theme::RULE);
            }
        });
}

/// A button with a few words or a number on it, as they are written.
pub fn key(
    parent: &mut ChildSpawnerCommands,
    words: impl Bundle,
    press: Press,
    kind: Kind,
    size: (Val, f32),
) {
    // A key without a width is as wide as its words, and a little more.
    let padding = if size.0 == Val::Auto { px(9) } else { px(0) };
    let node = Node {
        padding: UiRect::horizontal(padding),
        ..centered(size.0, size.1)
    };
    parent.spawn((pressed(press, kind), node, children![words]));
}

/// A name between a button that goes to the one before it and one that goes to the next:
/// for what has a few names to turn through, and no number.
pub fn turner(
    parent: &mut ChildSpawnerCommands,
    value: impl Into<String>,
    last: Press,
    next: Press,
    width: f32,
) {
    let sign = |sign: &str| theme::figure(sign.to_string(), 16.0, theme::TEXT);
    key(parent, sign("<"), last, Kind::Plain, (px(28), 28.0));
    parent.spawn((
        Node {
            width: px(width),
            justify_content: JustifyContent::Center,
            overflow: Overflow::clip(),
            ..default()
        },
        children![theme::text(value, 15.0, theme::TEXT)],
    ));
    key(parent, sign(">"), next, Kind::Plain, (px(28), 28.0));
}

/// A value between a button that takes from it and one that adds to it.
pub fn stepper(
    parent: &mut ChildSpawnerCommands,
    value: impl Into<String>,
    less: Press,
    more: Press,
    width: f32,
) {
    let sign = |sign: &str| theme::text(sign.to_string(), 18.0, theme::TEXT);
    key(parent, sign("-"), less, Kind::Plain, (px(34), 30.0));
    parent.spawn((
        Node {
            width: px(width),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![theme::figure(value, 17.0, theme::TEXT)],
    ));
    key(parent, sign("+"), more, Kind::Plain, (px(34), 30.0));
}

/// Buttons side by side, of which one is on.
pub fn choice<'a>(
    parent: &mut ChildSpawnerCommands,
    options: impl IntoIterator<Item = (&'a str, Press, bool)>,
    lit: bool,
    height: f32,
) {
    parent.spawn(Node::default()).with_children(|row| {
        for (name, press, on) in options {
            let kind = if lit { Kind::lit(on) } else { Kind::chosen(on) };
            button(row, name, press, kind, (Val::Auto, height));
        }
    });
}

/// A row of the list at the left of a tab: a name, and a word at its right. The one that is
/// selected has a square of the accent before its name.
pub fn list_row(
    parent: &mut ChildSpawnerCommands,
    name: &str,
    word: &str,
    press: Press,
    picked: bool,
    height: f32,
) {
    let kind = if picked { Kind::Picked } else { Kind::Row };
    parent
        .spawn((
            pressed(press, kind),
            Node {
                height: px(height),
                flex_shrink: 0.0,
                padding: UiRect::horizontal(px(12)),
                align_items: AlignItems::Center,
                column_gap: px(10),
                border: UiRect::bottom(px(1)),
                ..default()
            },
        ))
        .with_children(|row| {
            if picked {
                row.spawn((
                    Node {
                        width: px(10),
                        height: px(10),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(theme::ACCENT),
                ));
            }
            row.spawn((theme::text(name, 16.0, theme::TEXT), grow()));
            row.spawn(theme::written(word, 12.0, theme::TEXT_SOFT));
        });
}

/// A name that is typed into: a click starts it, Enter ends it and Escape gives it up.
pub fn field(
    parent: &mut ChildSpawnerCommands,
    view: &View,
    field: Field,
    value: &str,
    width: f32,
) {
    let typing = view.typing.as_ref().filter(|typing| typing.field == field);
    let (words, edge) = match typing {
        Some(typing) => (format!("{}_", typing.text), theme::ACCENT),
        None => (value.to_string(), theme::RULE),
    };
    parent.spawn((
        Button,
        Press::Type(field),
        Look {
            fill: theme::SURFACE_RAISED,
            hovered: theme::SURFACE_HOVERED,
        },
        BackgroundColor(theme::SURFACE_RAISED),
        BorderColor::all(edge),
        Node {
            width: px(width),
            height: px(32),
            flex_shrink: 0.0,
            padding: UiRect::horizontal(px(8)),
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            overflow: Overflow::clip(),
            ..default()
        },
        children![theme::text(words, 16.0, theme::TEXT)],
    ));
}

/// A box with a ruled edge, which holds a column.
pub fn panel(node: Node) -> impl Bundle {
    strip(Node {
        flex_direction: FlexDirection::Column,
        ..node
    })
}

/// A box with a ruled edge, which holds a row.
pub fn strip(node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(px(1)),
            ..node
        },
        BackgroundColor(theme::SURFACE),
        BorderColor::all(theme::RULE),
    )
}

/// The first line of a panel: its name, and what stands at its right.
pub fn head(
    parent: &mut ChildSpawnerCommands,
    name: &str,
    right: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            Node {
                height: px(34),
                flex_shrink: 0.0,
                padding: UiRect::horizontal(px(12)),
                align_items: AlignItems::Center,
                column_gap: px(8),
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BorderColor::all(theme::RULE),
        ))
        .with_children(|head| {
            head.spawn((theme::label(name, 12.0, theme::TEXT_DIM), grow()));
            right(head);
        });
}

/// What takes the room that is left in its row.
pub fn grow() -> Node {
    Node {
        flex_grow: 1.0,
        flex_basis: px(0),
        ..default()
    }
}

/// A row of a panel: a name at its left, and what stands at its right.
pub fn row(
    parent: &mut ChildSpawnerCommands,
    name: &str,
    height: f32,
    right: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent.spawn(line(height)).with_children(|row| {
        row.spawn((theme::text(name, 16.0, theme::TEXT), grow()));
        right(row);
    });
}

/// A row with a line under it, for whatever goes into it.
pub fn line(height: f32) -> impl Bundle {
    (
        Node {
            height: px(height),
            flex_shrink: 0.0,
            padding: UiRect::horizontal(px(12)),
            align_items: AlignItems::Center,
            column_gap: px(8),
            border: UiRect::bottom(px(1)),
            ..default()
        },
        BorderColor::all(theme::RULE_DIM),
    )
}

/// A row that says something and can't be changed: what a rule derives, or what the code
/// holds.
pub fn told(parent: &mut ChildSpawnerCommands, name: &str, value: impl Into<String>, height: f32) {
    parent.spawn(line(height)).with_children(|row| {
        row.spawn((theme::text(name, 16.0, theme::TEXT_SOFT), grow()));
        row.spawn(theme::text(value, 15.0, theme::TEXT_SOFT));
    });
}

/// The name of a group of rows.
pub fn caption(parent: &mut ChildSpawnerCommands, name: &str) {
    parent.spawn((
        Node {
            height: px(26),
            flex_shrink: 0.0,
            padding: UiRect::horizontal(px(12)),
            align_items: AlignItems::Center,
            border: UiRect::bottom(px(1)),
            ..default()
        },
        BackgroundColor(theme::SURFACE_RAISED),
        BorderColor::all(theme::RULE_DIM),
        children![theme::label(name, 11.0, theme::TEXT_DIM)],
    ));
}

/// The name of a table, and what it is.
pub fn heading(parent: &mut ChildSpawnerCommands, name: &str, words: &str, top: bool) {
    parent
        .spawn((
            Node {
                height: px(30),
                flex_shrink: 0.0,
                padding: UiRect::horizontal(px(14)),
                align_items: AlignItems::Center,
                column_gap: px(14),
                border: UiRect::top(px(if top { 1.0 } else { 0.0 })),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(theme::RULE),
        ))
        .with_children(|caption| {
            caption.spawn(theme::title(name, 14.0, theme::TEXT));
            caption.spawn(theme::text(words, 14.0, theme::TEXT_SOFT));
        });
}

/// A sentence at the foot of a panel or of a list.
pub fn note(parent: &mut ChildSpawnerCommands, words: &str) {
    parent.spawn((
        Node {
            padding: UiRect::axes(px(12), px(10)),
            ..default()
        },
        children![theme::wrapped(words, 14.0, theme::TEXT_SOFT)],
    ));
}

/// A line of dashes, across or down, from a point of its parent.
fn dashes(kit_image: Handle<Image>, color: Color, across: bool, node: Node) -> impl Bundle {
    (
        ImageNode::new(kit_image)
            .with_color(color)
            .with_mode(NodeImageMode::Tiled {
                tile_x: across,
                tile_y: !across,
                stretch_value: 1.0,
            }),
        Node {
            position_type: PositionType::Absolute,
            ..node
        },
    )
}

/// The edge of something that isn't there, around its parent.
pub fn dashed_frame(parent: &mut ChildSpawnerCommands, color: Color) {
    parent.spawn(Dashed {
        color,
        left: 0.0,
        top: 0.0,
        size: None,
    });
}

/// A rectangle in dashes that waits for the textures: it is drawn by `draw_dashes` once it
/// is there. Its place is in the pixels of its parent, which it fills if it has no size.
#[derive(Component, Clone, Copy)]
#[require(Node)]
pub struct Dashed {
    pub color: Color,
    pub left: f32,
    pub top: f32,
    pub size: Option<Vec2>,
}

/// Draws the rectangles in dashes that were just put down.
pub fn draw_dashes(
    mut commands: Commands,
    kit: Res<Kit>,
    fresh: Query<(Entity, &Dashed), Added<Dashed>>,
) {
    for (entity, dashed) in &fresh {
        let frame = match dashed.size {
            Some(size) => Node {
                position_type: PositionType::Absolute,
                left: px(dashed.left),
                top: px(dashed.top),
                width: px(size.x),
                height: px(size.y),
                ..default()
            },
            None => Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                ..default()
            },
        };
        let across = |node: Node| dashes(kit.dash_across.clone(), dashed.color, true, node);
        let down = |node: Node| dashes(kit.dash_down.clone(), dashed.color, false, node);
        let full = percent(100);
        commands
            .entity(entity)
            .insert(frame)
            .with_children(|frame| {
                frame.spawn(across(Node {
                    left: px(0),
                    top: px(0),
                    width: full,
                    height: px(1),
                    ..default()
                }));
                frame.spawn(across(Node {
                    left: px(0),
                    bottom: px(0),
                    width: full,
                    height: px(1),
                    ..default()
                }));
                frame.spawn(down(Node {
                    left: px(0),
                    top: px(0),
                    width: px(1),
                    height: full,
                    ..default()
                }));
                frame.spawn(down(Node {
                    right: px(0),
                    top: px(0),
                    width: px(1),
                    height: full,
                    ..default()
                }));
            });
    }
}

/// Strokes over what isn't used, filling their parent.
#[derive(Component, Clone, Copy)]
#[require(Node)]
pub struct Hatched;

/// Draws the strokes that were just put down.
pub fn draw_hatches(mut commands: Commands, kit: Res<Kit>, fresh: Query<Entity, Added<Hatched>>) {
    for entity in &fresh {
        commands.entity(entity).insert((
            ImageNode::new(kit.hatch.clone())
                .with_color(theme::HATCH.with_alpha(0.4))
                .with_mode(NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: 1.0,
                }),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                ..default()
            },
        ));
    }
}

/// A square that says how a check stands: full and green when it holds, red when it doesn't,
/// and hollow while it is still running.
pub fn mark(holds: Option<bool>) -> impl Bundle {
    light(holds, 4.0)
}

/// The same square in a row whose middle it stands on.
pub fn square(holds: Option<bool>) -> impl Bundle {
    light(holds, 0.0)
}

fn light(holds: Option<bool>, above: f32) -> impl Bundle {
    let (fill, edge) = match holds {
        Some(true) => (theme::GOOD, theme::GOOD),
        Some(false) => (theme::DANGER, theme::DANGER),
        None => (Color::NONE, theme::TEXT_DIM),
    };
    (
        Node {
            width: px(12),
            height: px(12),
            flex_shrink: 0.0,
            border: UiRect::all(px(1)),
            margin: UiRect::top(px(above)),
            ..default()
        },
        BackgroundColor(fill),
        BorderColor::all(edge),
    )
}
