//! The size of the interface, and the width of its lines.
//!
//! The interface is laid out in pixels of its own, for a sheet of 1600 by 900 of them at
//! least: every menu fits a sheet of that size, and the tests of the menus see to it. A
//! window that shows less gets the whole interface smaller, until it shows that much. What
//! the player chose in the settings comes on top of it.
//!
//! An interface that is drawn small has lines thinner than a pixel of the screen, which the
//! engine can't draw everywhere: they are kept one pixel wide.

use bevy::{
    camera::CameraUpdateSystems,
    prelude::*,
    ui::{DefaultUiCamera, UiSystems},
};

use crate::{hud::SHEET_EDGE, settings::Settings, theme};

/// The smallest sheet that the interface is drawn for, in its own pixels.
pub const LEAST: Vec2 = Vec2::new(1600.0, 900.0);
/// A small window takes the interface down a step at a time, and this many steps make its
/// whole size. The engine keeps every size of text that it ever drew, so a window that is
/// dragged must not ask for a new one at each pixel.
const STEPS: f32 = 20.0;
/// The fewest steps that a small window leaves. Nothing could be read below them.
const FEWEST: f32 = 10.0;

/// How much of its size the interface keeps in a window of this many pixels: all of it
/// where the least sheet fits, and else the most steps at which that sheet does.
pub fn fit(window: Vec2) -> f32 {
    // A step is 80 by 45 pixels of the window.
    let steps = (window / (LEAST / STEPS)).min_element().floor();
    steps.clamp(FEWEST, STEPS) / STEPS
}

pub fn plugin(app: &mut App) {
    // Between the moment the engine knows the size of the window and the moment it lays
    // the interface out, so that a window that changes is right in the same frame.
    app.add_systems(
        PostUpdate,
        (size, keep_edges)
            .chain()
            .after(CameraUpdateSystems)
            .before(UiSystems::Prepare),
    );
}

/// Gives the interface its size: the one that the player chose, less in a small window.
fn size(
    settings: Res<Settings>,
    shown_by: DefaultUiCamera,
    cameras: Query<&Camera>,
    mut scale: ResMut<UiScale>,
) {
    let camera = shown_by.get().and_then(|camera| cameras.get(camera).ok());
    // A window whose size isn't known yet takes nothing away.
    let window = camera.and_then(Camera::logical_viewport_size);
    let wanted = settings.ui_scale * window.map_or(1.0, fit);
    if scale.0 != wanted {
        scale.0 = wanted;
    }
}

/// The day, as a title block dates a sheet and a stamp dates a release: by the clock of the
/// machine, or of the browser, in universal time.
pub fn day_of_issue() -> String {
    #[cfg(target_arch = "wasm32")]
    let seconds = (web_sys::js_sys::Date::now() / 1000.0) as u64;
    #[cfg(not(target_arch = "wasm32"))]
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let stamp = tanku_core::journal::stamp(seconds);
    stamp.chars().take(10).collect()
}

/// The width of the frame of the sheet: the wide line of the interface.
pub const FRAME_WIDTH: f32 = 2.0;
/// How many zones the frame is divided into, across and down, as ISO 5457 divides a sheet:
/// numbered across the top, lettered down the sides.
const ZONES: (usize, usize) = (8, 6);
/// The centring marks: how wide they are, and how far into the sheet they reach from the
/// edge of the screen.
const CENTRING: (f32, f32) = (3.0, 34.0);
/// The trimming marks: how far along each edge the triangle in a corner reaches.
const TRIMMING: f32 = 12.0;

/// What belongs to the frame of the sheet: the line, the zones in the margin and the marks.
/// It is drawn in the margin, where nothing else goes.
#[derive(Component)]
pub struct Frame;

/// The frame around the screen, as around a drawing after ISO 5457: a wide line at the edge
/// of the sheet, the numbers and the letters of the zones in the margin, a centring mark at the
/// middle of each side and a trimming mark in each corner.
///
/// The line is four strokes, one for each side. As the border of a box as large as the sheet,
/// it was about a sixth of what the graphics card did in each frame (measured 2026-10-14): the
/// engine works out every pixel of a box that has a border to find those that are on it. The
/// strokes of the left and the right stand between those of the top and the bottom, so that
/// no corner is drawn twice.
///
/// They are laid out on a grid as large as the screen: the margin, a stroke, the sheet, a
/// stroke and the margin, across and down, and each stroke as wide as its edge. The engine
/// rounds where a piece lies from where the one that holds it lies, so a stroke held by a box
/// that starts inside a pixel, as the sheet does in a small window, would be a pixel off.
pub fn frame() -> impl Bundle {
    let stroke = |border, grid_row, grid_column| {
        (
            Node {
                border,
                grid_row,
                grid_column,
                ..default()
            },
            BorderColor::all(theme::RULE),
        )
    };
    let tracks = || {
        vec![
            GridTrack::px(SHEET_EDGE),
            GridTrack::auto(),
            GridTrack::flex(1.0),
            GridTrack::auto(),
            GridTrack::px(SHEET_EDGE),
        ]
    };
    let (first, between, last) = (
        GridPlacement::start(2),
        GridPlacement::start(3),
        GridPlacement::start(4),
    );
    let across = GridPlacement::start_end(2, 5);
    let width = px(FRAME_WIDTH);
    (
        Frame,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            display: Display::Grid,
            grid_template_columns: tracks(),
            grid_template_rows: tracks(),
            ..default()
        },
        Pickable::IGNORE,
        children![
            stroke(UiRect::top(width), first, across),
            stroke(UiRect::bottom(width), last, across),
            stroke(UiRect::left(width), between, first),
            stroke(UiRect::right(width), between, last),
            zones(),
            marks(),
        ],
    )
}

/// The numbers of the zones across the top margin and their letters down the two side ones,
/// each in the middle of its zone.
fn zones() -> impl Bundle {
    let name = |content: String| {
        (
            Node {
                flex_grow: 1.0,
                flex_basis: px(0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            children![(
                theme::label(content, theme::LABEL, theme::RULE),
                TextLayout::no_wrap()
            )],
        )
    };
    let strip = |node: Node, names: Vec<String>| {
        (
            Node {
                position_type: PositionType::Absolute,
                ..node
            },
            Children::spawn(SpawnIter(names.into_iter().map(name))),
        )
    };
    let numbers = (1..=ZONES.0).map(|zone| zone.to_string()).collect();
    let letters = || {
        (0..ZONES.1)
            .map(|zone| char::from(b'A' + zone as u8).to_string())
            .collect()
    };
    let side = |left: Val, right: Val| Node {
        left,
        right,
        top: px(SHEET_EDGE),
        bottom: px(SHEET_EDGE),
        width: px(SHEET_EDGE),
        flex_direction: FlexDirection::Column,
        ..default()
    };
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            ..default()
        },
        children![
            strip(
                Node {
                    left: px(SHEET_EDGE),
                    right: px(SHEET_EDGE),
                    top: px(0),
                    height: px(SHEET_EDGE),
                    ..default()
                },
                numbers,
            ),
            strip(side(px(0), auto()), letters()),
            strip(side(auto(), px(0)), letters()),
        ],
    )
}

/// The centring mark at the middle of each side, from the edge of the screen across the line,
/// and the trimming mark in each corner: a triangle, which is a square turned on its corner
/// with its middle on the corner of the screen, the rest of it off the screen.
fn marks() -> impl Bundle {
    let (thick, reach) = CENTRING;
    // A mark is held by its middle where it has to be, as the engine can't lay out half of
    // the screen less half of a mark.
    let mark = |node: Node, held: Val2| {
        (
            Node {
                position_type: PositionType::Absolute,
                ..node
            },
            UiTransform::from_translation(held),
            BackgroundColor(theme::RULE),
        )
    };
    let across = Val2::new(percent(-50), px(0));
    let down = Val2::new(px(0), percent(-50));
    let corner = |left: Val, top: Val| {
        let side = px(TRIMMING * std::f32::consts::SQRT_2);
        let (node, held) = (
            Node {
                left,
                top,
                width: side,
                height: side,
                ..default()
            },
            Val2::new(percent(-50), percent(-50)),
        );
        let (node, transform, color) = mark(node, held);
        (
            node,
            UiTransform {
                rotation: Rot2::degrees(45.0),
                ..transform
            },
            color,
        )
    };
    (
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            overflow: Overflow::clip(),
            ..default()
        },
        children![
            mark(
                Node {
                    left: percent(50),
                    top: px(0),
                    width: px(thick),
                    height: px(reach),
                    ..default()
                },
                across,
            ),
            mark(
                Node {
                    left: percent(50),
                    bottom: px(0),
                    width: px(thick),
                    height: px(reach),
                    ..default()
                },
                across,
            ),
            mark(
                Node {
                    top: percent(50),
                    left: px(0),
                    width: px(reach),
                    height: px(thick),
                    ..default()
                },
                down,
            ),
            mark(
                Node {
                    top: percent(50),
                    right: px(0),
                    width: px(reach),
                    height: px(thick),
                    ..default()
                },
                down,
            ),
            corner(px(0), px(0)),
            corner(percent(100), px(0)),
            corner(px(0), percent(100)),
            corner(percent(100), percent(100)),
        ],
    )
}

/// The edges of a piece of the interface as its menu drew them, kept for as long as the
/// screen is given wider ones.
#[derive(Component)]
struct Edges {
    drawn: UiRect,
    shown: UiRect,
}

/// Edges that are at least this wide, where there are edges at all.
fn widened(drawn: UiRect, least: f32) -> UiRect {
    let side = |width: Val| match width {
        Val::Px(width) if width > 0.0 && width < least => Val::Px(least),
        width => width,
    };
    UiRect {
        left: side(drawn.left),
        right: side(drawn.right),
        top: side(drawn.top),
        bottom: side(drawn.bottom),
    }
}

/// Keeps every edge of the interface at least one pixel of the screen wide.
///
/// The engine lays edges out on whole pixels of the screen. A line that is thinner than a
/// pixel, as the fine lines are once the interface is drawn smaller than its full size,
/// comes out one pixel wide in some places and not at all in others: a box loses a side,
/// a table a rule.
fn keep_edges(
    mut commands: Commands,
    scale: Res<UiScale>,
    shown_by: DefaultUiCamera,
    cameras: Query<&Camera>,
    mut pieces: Query<(Entity, &mut Node, Option<&mut Edges>)>,
    mut last: Local<f32>,
) {
    let camera = shown_by.get().and_then(|camera| cameras.get(camera).ok());
    // How many pixels of the screen a pixel of the interface takes.
    let screen = camera.and_then(Camera::target_scaling_factor);
    let drawn_at = scale.0 * screen.unwrap_or(1.0);
    // One pixel of the screen and no more: an edge that is a hair wider comes out two
    // pixels wide wherever it ends half way into a pixel.
    let least = if drawn_at < 1.0 { 1.0 / drawn_at } else { 0.0 };
    let resized = *last != least;
    *last = least;
    for (piece, mut node, edges) in &mut pieces {
        if !resized && !node.is_changed() {
            continue;
        }
        match edges {
            Some(mut edges) => {
                // An edge that isn't the one the screen was given was drawn anew.
                if node.border != edges.shown {
                    edges.drawn = node.border;
                }
                edges.shown = widened(edges.drawn, least);
                if node.border != edges.shown {
                    node.border = edges.shown;
                }
                if edges.shown == edges.drawn {
                    commands.entity(piece).remove::<Edges>();
                }
            }
            None => {
                let (drawn, shown) = (node.border, widened(node.border, least));
                if shown != drawn {
                    node.border = shown;
                    commands.entity(piece).insert(Edges { drawn, shown });
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use bevy::{
        app::TerminalCtrlCHandlerPlugin,
        audio::AudioPlugin,
        camera::{ImageRenderTarget, RenderTarget},
        clipboard::ClipboardPlugin,
        log::LogPlugin,
        render::{RenderPlugin, render_resource::TextureFormat, settings::WgpuSettings},
        window::ExitCondition,
        winit::WinitPlugin,
    };

    use super::*;

    /// The engine without a screen, for a test: it lays the interface out as a window of
    /// this many pixels shows it, on a screen that enlarges everything by this scale, and
    /// draws nothing. The settings are the default ones.
    pub(crate) fn window(pixels: UVec2, scale_factor: f32) -> App {
        let mut app = App::new();
        let no_window = WindowPlugin {
            primary_window: None,
            exit_condition: ExitCondition::DontExit,
            ..default()
        };
        let no_renderer = RenderPlugin {
            render_creation: WgpuSettings {
                backends: None,
                ..default()
            }
            .into(),
            ..default()
        };
        app.add_plugins(
            DefaultPlugins
                .set(no_window)
                .set(no_renderer)
                .disable::<WinitPlugin>()
                // A test leaves the sound and the clipboard of the desktop alone.
                .disable::<AudioPlugin>()
                .disable::<ClipboardPlugin>()
                // It neither logs nor keeps Ctrl+C from ending it.
                .disable::<LogPlugin>()
                .disable::<TerminalCtrlCHandlerPlugin>(),
        )
        .insert_resource(Settings::default())
        .add_plugins(plugin);
        let canvas = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(blank(pixels));
        app.world_mut().spawn((
            Camera2d,
            RenderTarget::Image(ImageRenderTarget {
                handle: canvas,
                scale_factor,
            }),
            IsDefaultUiCamera,
        ));
        // What `App::run` does before the first frame.
        app.finish();
        app.cleanup();
        app
    }

    /// An image that stands in for a window of this many pixels.
    fn blank(pixels: UVec2) -> Image {
        Image::new_target_texture(pixels.x, pixels.y, TextureFormat::Rgba8UnormSrgb, None)
    }

    /// Gives the window of a test another size.
    fn resize(app: &mut App, pixels: UVec2) {
        let world = app.world_mut();
        let mut targets = world.query::<&RenderTarget>();
        let canvas = targets.single(world).unwrap().as_image().unwrap().clone();
        let mut images = world.resource_mut::<Assets<Image>>();
        *images.get_mut(&canvas).unwrap() = blank(pixels);
        // The engine sees a frame later that an image changed.
        app.update();
        app.update();
    }

    #[test]
    fn a_window_that_holds_the_least_sheet_leaves_the_interface_as_it_is() {
        assert_eq!(fit(LEAST), 1.0);
        assert_eq!(fit(Vec2::new(1920.0, 1080.0)), 1.0);
        assert_eq!(fit(Vec2::new(1680.0, 1050.0)), 1.0);
        assert_eq!(fit(Vec2::new(3840.0, 2160.0)), 1.0);
    }

    #[test]
    fn a_smaller_window_takes_the_interface_down_by_steps() {
        assert_eq!(fit(Vec2::new(1280.0, 720.0)), 0.8);
        assert_eq!(fit(Vec2::new(1366.0, 768.0)), 0.85);
        assert_eq!(fit(Vec2::new(1536.0, 864.0)), 0.95);
        // The side that is shortest for the sheet decides.
        assert_eq!(fit(Vec2::new(1280.0, 1024.0)), 0.8);
        assert_eq!(fit(Vec2::new(1440.0, 900.0)), 0.9);
        assert_eq!(fit(Vec2::new(2560.0, 720.0)), 0.8);
        // A window too small to read anything in stops there, and so does one with no
        // size at all.
        assert_eq!(fit(Vec2::new(640.0, 360.0)), 0.5);
        assert_eq!(fit(Vec2::ZERO), 0.5);
    }

    #[test]
    fn the_least_sheet_fits_every_window_that_is_not_tiny() {
        for width in (800..=2600).step_by(7) {
            for height in (450..=1500).step_by(7) {
                let window = Vec2::new(width as f32, height as f32);
                // What the window shows of the interface, in the pixels of the interface.
                let sheet = window / fit(window);
                assert!(
                    sheet.cmpge(LEAST - 1e-3).all(),
                    "a window of {window} shows {sheet}"
                );
                // No step is lost: one more would not fit.
                let more = fit(window) + 1.0 / STEPS;
                assert!(
                    more > 1.0 + 1e-6 || (window / more).cmplt(LEAST - 1e-3).any(),
                    "a window of {window} could show the interface at {more}"
                );
            }
        }
    }

    /// The size that the engine lays the interface out at, from its first frame, when the
    /// player chose this size in the settings: the width of a box of 100 pixels.
    fn laid_out(pixels: UVec2, scale_factor: f32, chosen: f32) -> f32 {
        let mut app = window(pixels, scale_factor);
        app.world_mut().resource_mut::<Settings>().ui_scale = chosen;
        let node = Node {
            width: px(100),
            height: px(100),
            ..default()
        };
        let ruler = app.world_mut().spawn(node).id();
        app.update();
        app.world().get::<ComputedNode>(ruler).unwrap().size.x
    }

    #[test]
    fn the_window_and_the_player_both_size_the_interface() {
        let (large, small) = (UVec2::new(1920, 1080), UVec2::new(1280, 720));
        assert_eq!(laid_out(large, 1.0, 1.0), 100.0);
        assert_eq!(laid_out(small, 1.0, 1.0), 80.0);
        // A screen that enlarges everything leaves the game fewer pixels of its own: 1280
        // by 720 here, each drawn on one and a half of the screen's.
        assert_eq!(laid_out(large, 1.5, 1.0), 120.0);
        // What the player chose comes on top.
        assert_eq!(laid_out(large, 1.0, 1.5), 150.0);
        assert_eq!(laid_out(small, 1.0, 1.25), 100.0);
        assert_eq!(laid_out(small, 1.0, 0.75), 60.0);
    }

    /// A column of boxes that are ruled all around, with every width of edge that the
    /// interface has.
    fn ruled_boxes(app: &mut App) -> Vec<Entity> {
        let column = Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(7),
            padding: UiRect::all(px(3)),
            ..default()
        };
        let column = app.world_mut().spawn(column).id();
        let boxes = (0..24).map(|index| {
            let ruled = Node {
                width: px(100 + index),
                height: px(13 + index % 5),
                border: UiRect::all(px([1.0, 1.5, 2.0][index as usize % 3])),
                ..default()
            };
            app.world_mut().spawn((ruled, ChildOf(column))).id()
        });
        boxes.collect()
    }

    /// How wide the engine laid out the edges of a box, in pixels of the screen.
    fn edges(app: &App, ruled: Entity) -> [f32; 4] {
        let border = app.world().get::<ComputedNode>(ruled).unwrap().border;
        [
            border.min_inset.x,
            border.max_inset.x,
            border.min_inset.y,
            border.max_inset.y,
        ]
    }

    #[test]
    fn no_edge_is_lost_when_the_interface_is_drawn_small() {
        // Every size that a small window gives the interface, with the size that the player
        // chose and with the smallest one that the settings have.
        for steps in FEWEST as u32..=STEPS as u32 {
            for chosen in [1.0, 0.75] {
                let pixels = (LEAST / STEPS).as_uvec2() * steps;
                let mut app = window(pixels, 1.0);
                app.world_mut().resource_mut::<Settings>().ui_scale = chosen;
                let boxes = ruled_boxes(&mut app);
                app.update();
                let size = app.world().resource::<UiScale>().0;
                for (index, ruled) in boxes.into_iter().enumerate() {
                    let edges = edges(&app, ruled);
                    // No line is missing anywhere, and the finest ones are one pixel of the
                    // screen everywhere.
                    let finest = index % 3 == 0 && size < 1.0;
                    assert!(
                        edges
                            .iter()
                            .all(|edge| *edge >= 1.0 && (!finest || *edge == 1.0)),
                        "the edges of box {index} are {edges:?} wide in an interface at {size}"
                    );
                }
            }
        }
    }

    #[test]
    fn edges_are_as_they_were_drawn_where_the_interface_has_its_full_size() {
        let mut app = window(UVec2::new(1280, 720), 1.0);
        let boxes = ruled_boxes(&mut app);
        app.update();
        let drawn = |app: &App, ruled: Entity| app.world().get::<Node>(ruled).unwrap().border;
        assert_ne!(drawn(&app, boxes[0]), UiRect::all(px(1)));
        // A menu that draws an edge anew is heard.
        let mut node = app.world_mut().get_mut::<Node>(boxes[1]).unwrap();
        node.border = UiRect::all(px(4));
        app.update();
        assert_eq!(drawn(&app, boxes[1]), UiRect::all(px(4)));

        // The window grows: nothing is left of what the small one needed.
        resize(&mut app, UVec2::new(1920, 1080));
        for (index, ruled) in boxes.iter().enumerate() {
            let width = if index == 1 {
                4.0
            } else {
                [1.0, 1.5, 2.0][index % 3]
            };
            assert_eq!(drawn(&app, *ruled), UiRect::all(px(width)));
            assert!(app.world().get::<Edges>(*ruled).is_none());
        }
    }

    /// How many edges of these pieces paint each pixel of a window of this size, row by row
    /// from the top. A pixel is painted where its middle is.
    fn painted(app: &App, pieces: &[Entity], pixels: UVec2) -> Vec<u8> {
        let mut painted = vec![0; (pixels.x * pixels.y) as usize];
        for piece in pieces {
            let computed = app.world().get::<ComputedNode>(*piece).unwrap();
            let center = app.world().get::<UiGlobalTransform>(*piece).unwrap();
            let outer = Rect::from_center_size(center.translation, computed.size);
            let (near, far) = (computed.border.min_inset, computed.border.max_inset);
            let inner = Rect::from_corners(outer.min + near, outer.max - far);
            for side in [outer.min, outer.max, inner.min, inner.max] {
                assert!(
                    side.round().distance(side) < 1e-3,
                    "an edge ends inside a pixel, at {side}, in a window of {pixels}"
                );
            }
            let span = |from: f32, to: f32| from.round() as u32..to.round() as u32;
            for y in span(outer.min.y, outer.max.y) {
                let middle = y as f32 + 0.5;
                // A row between the edges at the top and the bottom has its sides alone.
                let across = if middle > inner.min.y && middle < inner.max.y {
                    [
                        span(outer.min.x, inner.min.x),
                        span(inner.max.x, outer.max.x),
                    ]
                } else {
                    [span(outer.min.x, outer.max.x), 0..0]
                };
                for x in across.into_iter().flatten() {
                    painted[(y * pixels.x + x) as usize] += 1;
                }
            }
        }
        painted
    }

    #[test]
    fn the_frame_paints_what_the_border_of_one_box_did() {
        let windows = [
            (UVec2::new(1280, 720), 1.0, 1.0),
            (UVec2::new(1366, 768), 1.0, 1.0),
            (UVec2::new(1440, 900), 1.0, 1.0),
            (UVec2::new(1536, 864), 1.0, 1.0),
            (UVec2::new(1920, 1080), 1.0, 1.0),
            (UVec2::new(1920, 1080), 1.5, 1.0),
            (UVec2::new(1920, 1080), 1.0, 0.75),
            (UVec2::new(1280, 720), 1.0, 0.75),
        ];
        for (pixels, scale_factor, chosen) in windows {
            let mut app = window(pixels, scale_factor);
            app.world_mut().resource_mut::<Settings>().ui_scale = chosen;
            let ruled = Node {
                position_type: PositionType::Absolute,
                left: px(SHEET_EDGE),
                right: px(SHEET_EDGE),
                top: px(SHEET_EDGE),
                bottom: px(SHEET_EDGE),
                border: UiRect::all(px(FRAME_WIDTH)),
                ..default()
            };
            let ruled = app.world_mut().spawn(ruled).id();
            app.world_mut().spawn(frame());
            app.update();
            let world = app.world_mut();
            let mut pieces = world.query::<(Entity, &Node)>();
            let strokes: Vec<Entity> = pieces
                .iter(world)
                .filter(|(piece, node)| *piece != ruled && node.border != UiRect::ZERO)
                .map(|(piece, _)| piece)
                .collect();
            assert_eq!(strokes.len(), 4);
            let line = painted(&app, &strokes, pixels);
            let border = painted(&app, &[ruled], pixels);
            let at = format!("in a window of {pixels} at {scale_factor}, chosen {chosen}");
            assert!(
                line.iter().all(|times| *times <= 1),
                "a pixel is painted twice {at}"
            );
            assert!(border.contains(&1), "the border is drawn nowhere {at}");
            assert!(line == border, "the line is not the border {at}");
        }
    }

    #[test]
    fn a_window_that_changes_is_fitted_again() {
        let mut app = window(UVec2::new(1920, 1080), 1.0);
        app.update();
        assert_eq!(app.world().resource::<UiScale>().0, 1.0);
        resize(&mut app, UVec2::new(1366, 768));
        assert_eq!(app.world().resource::<UiScale>().0, 0.85);
        resize(&mut app, UVec2::new(1920, 1080));
        assert_eq!(app.world().resource::<UiScale>().0, 1.0);
    }
}
