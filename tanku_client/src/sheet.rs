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

use crate::settings::Settings;

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
