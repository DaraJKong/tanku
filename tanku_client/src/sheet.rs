//! The width of the lines of the interface.
//!
//! An interface that is drawn smaller than it was laid out, as when the player sets it to
//! 75% of its size, has lines thinner than a pixel of the screen, which the engine can't
//! draw everywhere: they are kept one pixel wide.

use bevy::{
    prelude::*,
    ui::{DefaultUiCamera, UiSystems},
};

pub fn plugin(app: &mut App) {
    // Before the engine lays the interface out, so that a size that changes is right in
    // the same frame.
    app.add_systems(PostUpdate, keep_edges.before(UiSystems::Prepare));
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
/// pixel, as the fine lines are once the interface is drawn smaller than it was laid out,
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
    /// draws nothing.
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
        .add_plugins(plugin);
        let format = TextureFormat::Rgba8UnormSrgb;
        let canvas = Image::new_target_texture(pixels.x, pixels.y, format, None);
        let canvas = app.world_mut().resource_mut::<Assets<Image>>().add(canvas);
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
        // The smallest size that the settings have, and sizes that fall on the pixels of
        // the screen in other ways, on a screen that enlarges nothing and on one that does.
        for size in [0.75, 0.5, 0.6, 0.8, 0.85, 0.9, 0.95, 1.0] {
            for scale_factor in [1.0, 1.25] {
                let mut app = window(UVec2::new(1920, 1080), scale_factor);
                app.insert_resource(UiScale(size));
                let boxes = ruled_boxes(&mut app);
                app.update();
                for (index, ruled) in boxes.into_iter().enumerate() {
                    let edges = edges(&app, ruled);
                    // No line is missing anywhere, and the finest ones are one pixel of the
                    // screen everywhere.
                    let finest = index % 3 == 0 && size * scale_factor < 1.0;
                    assert!(
                        edges
                            .iter()
                            .all(|edge| *edge >= 1.0 && (!finest || *edge == 1.0)),
                        "the edges of box {index} are {edges:?} wide in an interface at {size} \
                         on a screen at {scale_factor}"
                    );
                }
            }
        }
    }

    #[test]
    fn edges_are_as_they_were_drawn_where_the_interface_has_its_full_size() {
        let mut app = window(UVec2::new(1920, 1080), 1.0);
        app.insert_resource(UiScale(0.75));
        let boxes = ruled_boxes(&mut app);
        app.update();
        let drawn = |app: &App, ruled: Entity| app.world().get::<Node>(ruled).unwrap().border;
        assert_ne!(drawn(&app, boxes[0]), UiRect::all(px(1)));
        // A menu that draws an edge anew is heard.
        let mut node = app.world_mut().get_mut::<Node>(boxes[1]).unwrap();
        node.border = UiRect::all(px(4));
        app.update();
        assert_eq!(drawn(&app, boxes[1]), UiRect::all(px(4)));

        // The interface is given its full size again: nothing is left of what the small
        // one needed.
        app.insert_resource(UiScale(1.0));
        app.update();
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
}
