//! The lines of the interface that lie at any angle: the leaders of the balloons and of the
//! alerts, the line of a tag that lies along its barrier, the line that strikes a balloon out,
//! and the frames of the stamps, which are turned a few degrees.
//!
//! The engine draws a node of the interface as a quad whose pixels end at its edge, so the
//! edge of a node that is turned steps from pixel to pixel, and the anti-aliasing of the scene
//! never reaches the interface. A line is therefore a node a little wider than the line,
//! filled across with its ink, which fades out over a pixel at each of its edges: the fade
//! draws the edge, wherever it lies. Without anti-aliasing, its edges are as sharp as those of
//! the rest of the picture.

use bevy::{prelude::*, ui::UiSystems};

use crate::settings::Settings;

/// How far the node of a line reaches past each of its edges: the fade is a pixel wide, and
/// centered on the edge.
pub const FEATHER: f32 = 1.0;

/// A line of the interface, at least a pixel wide: how wide it is, in pixels, and its ink.
/// Its node is as long as the line and [`Line::across`] high, and it turns around its middle.
/// A line that is upright runs up its node instead, which is [`Line::across`] wide.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[require(BackgroundGradient)]
pub struct Line {
    pub width: f32,
    pub ink: Color,
    pub upright: bool,
}

impl Line {
    pub const fn new(width: f32, ink: Color) -> Self {
        Self {
            width,
            ink,
            upright: false,
        }
    }

    pub const fn upright(width: f32, ink: Color) -> Self {
        Self {
            upright: true,
            ..Self::new(width, ink)
        }
    }

    /// How high its node is, or how wide if it is upright: the line, and the fade on each side
    /// of it.
    pub fn across(self) -> f32 {
        self.width + 2.0 * FEATHER
    }

    /// Where the node of the line goes, by its top left corner before it turns, for a line
    /// this long to have its middle here. Its edges lie between whole pixels, so that a line
    /// that lies level or upright is as sharp as the rest of the interface.
    pub fn corner(self, middle: Vec2, length: f32) -> Vec2 {
        (middle - Vec2::new(length, self.width) / 2.0).round() - Vec2::new(0.0, FEATHER)
    }

    /// Its ink from one side of its node to the other: fading in and out over a pixel around
    /// each edge, or starting and stopping at the edges.
    fn stops(self, smooth: bool) -> Vec<ColorStop> {
        let (ink, clear) = (self.ink, self.ink.with_alpha(0.0));
        let (start, end) = (FEATHER, FEATHER + self.width);
        if smooth {
            vec![
                ColorStop::px(clear, start - 0.5),
                ColorStop::px(ink, start + 0.5),
                ColorStop::px(ink, end - 0.5),
                ColorStop::px(clear, end + 0.5),
            ]
        } else {
            // The first stop is at the top: the engine fills what is before the first stretch
            // that it draws with the color that the stretch starts with.
            vec![
                ColorStop::px(clear, 0.0),
                ColorStop::px(clear, start),
                ColorStop::px(ink, start),
                ColorStop::px(ink, end),
                ColorStop::px(clear, end),
                ColorStop::px(clear, end + FEATHER),
            ]
        }
    }
}

/// The border of a node that is turned, drawn in lines: the engine would step the outer edge
/// of the border itself. The node keeps its border for its layout, in no color, and holds
/// this. `widths` are those of its border, at its top, right, bottom and left, and `each` goes
/// on every line that is drawn. The lines across the node run from one side of it to the
/// other, and those up the node run between them, so that a frame that fades doesn't darken
/// at its corners.
pub fn border(widths: [f32; 4], ink: Color, each: impl Bundle + Clone) -> impl Bundle {
    let [top, right, bottom, left] = widths;
    let side = |width: f32, upright: bool, node: Node| {
        let line = if upright {
            Line::upright(width, ink)
        } else {
            Line::new(width, ink)
        };
        let node = if upright {
            Node {
                top: px(top),
                bottom: px(bottom),
                width: px(line.across()),
                ..node
            }
        } else {
            Node {
                left: px(0),
                right: px(0),
                height: px(line.across()),
                ..node
            }
        };
        (width, line, node)
    };
    let held = |node: Node| Node {
        position_type: PositionType::Absolute,
        ..node
    };
    let sides = [
        side(
            top,
            false,
            held(Node {
                top: px(-FEATHER),
                ..default()
            }),
        ),
        side(
            bottom,
            false,
            held(Node {
                bottom: px(-FEATHER),
                ..default()
            }),
        ),
        side(
            left,
            true,
            held(Node {
                left: px(-FEATHER),
                ..default()
            }),
        ),
        side(
            right,
            true,
            held(Node {
                right: px(-FEATHER),
                ..default()
            }),
        ),
    ];
    let drawn = sides
        .into_iter()
        .filter(|(width, ..)| *width > 0.0)
        .map(move |(_, line, node)| (each.clone(), line, node, Pickable::IGNORE));
    (
        held(Node {
            left: px(-left),
            right: px(-right),
            top: px(-top),
            bottom: px(-bottom),
            ..default()
        }),
        Pickable::IGNORE,
        Children::spawn(SpawnIter(drawn)),
    )
}

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, paint.before(UiSystems::Prepare));
}

/// Paints a line when it changes, and every line when anti-aliasing is turned on or off.
fn paint(settings: Option<Res<Settings>>, mut lines: Query<(Ref<Line>, &mut BackgroundGradient)>) {
    let smooth = settings
        .as_ref()
        .is_none_or(|settings| settings.anti_aliasing);
    let again = settings.as_ref().is_some_and(Res::is_changed);
    for (line, mut gradient) in &mut lines {
        if again || line.is_changed() {
            let stops = line.stops(smooth);
            *gradient = if line.upright {
                LinearGradient::to_right(stops)
            } else {
                LinearGradient::to_bottom(stops)
            }
            .into();
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ui::Gradient;

    use super::*;

    /// How much ink a line puts at this depth into its node, from none to all of it.
    fn ink_at(stops: &[ColorStop], depth: f32) -> f32 {
        let at = |stop: &ColorStop| match stop.point {
            Val::Px(point) => point,
            _ => panic!("a stop in pixels"),
        };
        let alpha = |stop: &ColorStop| stop.color.alpha();
        // What is before the first stop and after the last is filled with their colors.
        if depth <= at(&stops[0]) {
            return alpha(&stops[0]);
        }
        for pair in stops.windows(2) {
            let (from, to) = (at(&pair[0]), at(&pair[1]));
            if depth <= to && to > from {
                let share = ((depth - from) / (to - from)).clamp(0.0, 1.0);
                return alpha(&pair[0]) + (alpha(&pair[1]) - alpha(&pair[0])) * share;
            }
        }
        alpha(&stops[stops.len() - 1])
    }

    #[test]
    fn a_line_is_as_wide_as_it_is_said_to_be() {
        let line = Line::new(2.0, Color::WHITE);
        for smooth in [true, false] {
            let stops = line.stops(smooth);
            // Its ink, summed across the node, a hundredth of a pixel at a time.
            let steps = (line.across() * 100.0) as usize;
            let ink: f32 = (0..steps)
                .map(|step| ink_at(&stops, (step as f32 + 0.5) / 100.0) / 100.0)
                .sum();
            assert!((ink - line.width).abs() < 0.01, "smooth {smooth}: {ink}");
        }
    }

    #[test]
    fn a_line_that_lies_level_on_whole_pixels_is_sharp() {
        // The middle of a pixel of the node, from its top, lies half a pixel into it.
        let line = Line::new(2.0, Color::WHITE);
        let corner = line.corner(Vec2::new(50.0, 20.0), 40.0);
        assert_eq!(corner, Vec2::new(30.0, 18.0));
        for smooth in [true, false] {
            let stops = line.stops(smooth);
            let pixels: Vec<f32> = (0..4).map(|row| ink_at(&stops, row as f32 + 0.5)).collect();
            assert_eq!(pixels, [0.0, 1.0, 1.0, 0.0], "smooth {smooth}");
        }
    }

    #[test]
    fn a_border_drawn_in_lines_lies_on_the_border() {
        let mut app = crate::sheet::tests::window(UVec2::new(1920, 1080), 1.0);
        let framed = |app: &mut App, widths: [f32; 4]| {
            let [top, right, bottom, left] = widths.map(px);
            let world = app.world_mut();
            let node = Node {
                position_type: PositionType::Absolute,
                left: px(50),
                top: px(40),
                width: px(120),
                height: px(60),
                border: UiRect::new(left, right, top, bottom),
                ..default()
            };
            let framed = world.spawn(node).id();
            world.spawn((ChildOf(framed), border(widths, Color::WHITE, ())));
            framed
        };
        let whole = framed(&mut app, [3.0, 1.0, 4.0, 2.0]);
        let parted = framed(&mut app, [0.0, 1.0, 0.0, 0.0]);
        app.update();
        app.update();
        // Where each line of a node lies, by its node's edges, left, top, right and bottom.
        let drawn = |app: &mut App, framed: Entity| -> Vec<(bool, [f32; 4])> {
            let world = app.world_mut();
            let overlay = world.get::<Children>(framed).unwrap()[0];
            let lines: Vec<Entity> = world.get::<Children>(overlay).unwrap().to_vec();
            lines
                .into_iter()
                .map(|line| {
                    let size = world.get::<ComputedNode>(line).unwrap().size();
                    let middle = world.get::<UiGlobalTransform>(line).unwrap().translation;
                    let (low, high) = (middle - size / 2.0, middle + size / 2.0);
                    let upright = world.get::<Line>(line).unwrap().upright;
                    (upright, [low.x, low.y, high.x, high.y])
                })
                .collect()
        };
        // The node spans 50 to 170 across and 40 to 100 down. Each line reaches a pixel past
        // both of its edges, and those up the node run between those across it.
        assert_eq!(
            drawn(&mut app, whole),
            [
                (false, [50.0, 39.0, 170.0, 44.0]),
                (false, [50.0, 95.0, 170.0, 101.0]),
                (true, [49.0, 43.0, 53.0, 96.0]),
                (true, [168.0, 43.0, 171.0, 96.0]),
            ]
        );
        // A border on one side has one line.
        assert_eq!(
            drawn(&mut app, parted),
            [(true, [168.0, 40.0, 171.0, 100.0])]
        );
    }

    #[test]
    fn turning_anti_aliasing_off_paints_every_line_again() {
        let mut app = App::new();
        app.insert_resource(Settings::default())
            .add_systems(Update, paint);
        let line = app.world_mut().spawn(Line::new(2.0, Color::WHITE)).id();
        let stops = |app: &App| match &app.world().get::<BackgroundGradient>(line).unwrap().0[..] {
            [Gradient::Linear(gradient)] => gradient.stops.len(),
            other => panic!("one linear gradient, not {other:?}"),
        };
        app.update();
        assert_eq!(stops(&app), 4);
        app.world_mut().resource_mut::<Settings>().anti_aliasing = false;
        app.update();
        assert_eq!(stops(&app), 6);
    }
}
