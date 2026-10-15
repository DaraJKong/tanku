//! The lines of the interface that lie at any angle: the leaders of the balloons and of the
//! alerts, the line of a tag that lies along its barrier, and the line that strikes a balloon
//! out.
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
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[require(BackgroundGradient)]
pub struct Line {
    pub width: f32,
    pub ink: Color,
}

impl Line {
    pub const fn new(width: f32, ink: Color) -> Self {
        Self { width, ink }
    }

    /// How high its node is: the line, and the fade on each side of it.
    pub fn across(self) -> f32 {
        self.width + 2.0 * FEATHER
    }

    /// Where the node of the line goes, by its top left corner before it turns, for a line
    /// this long to have its middle here. Its edges lie between whole pixels, so that a line
    /// that lies level or upright is as sharp as the rest of the interface.
    pub fn corner(self, middle: Vec2, length: f32) -> Vec2 {
        (middle - Vec2::new(length, self.width) / 2.0).round() - Vec2::new(0.0, FEATHER)
    }

    /// Its ink from the top of its node to the bottom: fading in and out over a pixel around
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
            *gradient = LinearGradient::to_bottom(line.stops(smooth)).into();
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
