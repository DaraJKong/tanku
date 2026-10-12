//! The ruler: ticks along the edge of the sheet that give the distance to the barrier of the
//! sector the player is in.
//!
//! The distance is measured along the sector, the way it runs. The ruler lies along the edge
//! of the screen that runs the same way: the left edge while the sector goes up or down the
//! screen, the top edge while it goes across, whether the corridor turned or the view did.
//! Each tick stands where its distance meets that edge, so the ticks slide with the ground.

use bevy::prelude::*;
use tanku_core::{
    map::{SectorKind, SectorPlan},
    sim::{Barrier, Run, Sector},
};

use crate::{
    Screen,
    camera::{self, OnScreen, View},
    hud::SHEET_EDGE,
    scene, theme,
};

/// How far the short and the long ticks stick out of the edge of the sheet, toward the edge
/// of the screen.
const TICK: (f32, f32) = (5.0, 10.0);
/// The steps between two ticks, in meters, and how many of them make a long tick with its
/// number. The first one whose ticks are far enough apart is used.
const STEPS: [(f32, u32); 4] = [(1.0, 5), (5.0, 2), (10.0, 5), (50.0, 2)];
/// Ticks are at least this many pixels apart.
const TICK_SPACING: f32 = 10.0;
/// How many short and long ticks there are to place.
const SHORT_TICKS: usize = 120;
const LONG_TICKS: usize = 40;
/// Thickness of the mark that shows where the barrier is along the ruler, and its least
/// length.
const BARRIER_MARK: (f32, f32) = (3.0, 8.0);
/// The gap between the edge of the sheet and the numbers.
const NUMBER_GAP: f32 = 5.0;
/// The room that the caption takes in the corner of the sheet, where no number goes.
const CAPTION_ROOM: Vec2 = Vec2::new(120.0, 24.0);

/// A part of the ruler.
#[derive(Component, Clone, Copy)]
enum Mark {
    Short(usize),
    Long(usize),
    /// The number of a long tick.
    Number(usize),
    /// Where the barrier is.
    Barrier,
    Caption,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Screen::Playing), build)
        .add_systems(
            Update,
            measure
                .after(camera::follow)
                .run_if(in_state(Screen::Playing)),
        );
}

/// The distance from a point to the barrier of a sector, along the sector. Where the barrier
/// lies along a side of its sector, this is the distance to its middle.
fn distance(plan: &SectorPlan, point: Vec2) -> f32 {
    let middle = (plan.gate.0 + plan.gate.1).as_vec2() / 2.0;
    (middle - point).dot(plan.heading.forward().as_vec2())
}

/// How far the barrier of a sector reaches on each side of its middle, along the sector:
/// nothing for a barrier across the corridor, half its length for one along a side.
fn barrier_reach(plan: &SectorPlan) -> f32 {
    let span = (plan.gate.1 - plan.gate.0).as_vec2();
    span.dot(plan.heading.forward().as_vec2()).abs() / 2.0
}

/// The distances between which the sector lies: from the end that is furthest past its
/// barrier, which is at the barrier itself unless the barrier lies along a side, to its entry.
fn extent(plan: &SectorPlan) -> (f32, f32) {
    let corners = plan
        .outline
        .iter()
        .map(|corner| distance(plan, corner.as_vec2()));
    corners.fold((f32::MAX, f32::MIN), |(low, high), far| {
        (low.min(far), high.max(far))
    })
}

/// The step between two ticks and the number of steps between two long ones, for a ruler
/// that shows a meter in this many pixels.
fn steps(pixels_per_meter: f32) -> (f32, u32) {
    let fits = |&&(step, _): &&(f32, u32)| step * pixels_per_meter >= TICK_SPACING;
    *STEPS.iter().find(fits).unwrap_or(&STEPS[STEPS.len() - 1])
}

/// The ticks between two distances: how many steps each is from the barrier.
fn ticks(from: f32, to: f32, step: f32) -> std::ops::RangeInclusive<i32> {
    let (low, high) = (from.min(to), from.max(to));
    (low / step).ceil() as i32..=(high / step).floor() as i32
}

/// What the ruler shows this frame.
struct Reading {
    /// Whether it lies along the left edge. It lies along the top one otherwise.
    upright: bool,
    /// Where the short ticks are along that edge, in pixels.
    short: Vec<f32>,
    /// The same for the long ticks, with the meters that each one tells.
    long: Vec<(f32, u32)>,
    /// The ends of the stretch of the edge that is level with the barrier.
    barrier: Option<(f32, f32)>,
}

/// Reads the ruler off the ground: the distances that the edge of the sheet passes over.
fn read(plan: &SectorPlan, focus: Vec2, screen: &OnScreen) -> Option<Reading> {
    let size = screen.size()?;
    let on_screen = |point: Vec2| screen.point(scene::to_world(point, 0.0));
    // The way the sector runs on the screen, seen where the player is looking.
    let forward = plan.heading.forward().as_vec2();
    let runs = on_screen(focus + forward)? - on_screen(focus)?;
    let upright = runs.y.abs() >= runs.x.abs();
    let corner = Vec2::splat(SHEET_EDGE);
    let end = if upright {
        Vec2::new(SHEET_EDGE, size.y - SHEET_EDGE)
    } else {
        Vec2::new(size.x - SHEET_EDGE, SHEET_EDGE)
    };
    let along = |at: Vec2| if upright { at.y } else { at.x };
    // The edge of the sheet lies over a straight line of the ground, on which the distance
    // to the barrier changes evenly.
    let (ground_a, ground_b) = (screen.ground(corner)?, screen.ground(end)?);
    let (far_a, far_b) = (distance(plan, ground_a), distance(plan, ground_b));
    if (far_b - far_a).abs() < 1e-3 {
        return None;
    }
    let place = |far: f32| {
        let ground = ground_a.lerp(ground_b, (far - far_a) / (far_b - far_a));
        on_screen(ground).map(along)
    };
    let (step, long_every) = steps((along(end) - along(corner)).abs() / (far_b - far_a).abs());
    // The ruler is as long as the sector: the next one has its own barrier to measure to.
    let (nearest, furthest) = extent(plan);
    let within = |far: f32| far.clamp(nearest, furthest);
    let (mut short, mut long) = (Vec::new(), Vec::new());
    for tick in ticks(within(far_a), within(far_b), step) {
        let far = tick as f32 * step;
        let at = place(far)?;
        if tick.rem_euclid(long_every as i32) == 0 {
            long.push((at, far.abs().round() as u32));
        } else {
            short.push(at);
        }
    }
    // The barrier shows where it is level with the ruler, if that is on the screen.
    let reach = barrier_reach(plan);
    let (low, high) = (far_a.min(far_b), far_a.max(far_b));
    let barrier = (reach >= low && -reach <= high)
        .then(|| Some((place((-reach).max(low))?, place(reach.min(high))?)))
        .flatten();
    Some(Reading {
        upright,
        short,
        long,
        barrier,
    })
}

fn build(mut commands: Commands) {
    let hidden = |mark: Mark| {
        (
            mark,
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            UiTransform::IDENTITY,
            Visibility::Hidden,
        )
    };
    commands
        .spawn((
            DespawnOnExit(Screen::Playing),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|ruler| {
            for index in 0..SHORT_TICKS {
                ruler.spawn((hidden(Mark::Short(index)), BackgroundColor(theme::RULE)));
            }
            for index in 0..LONG_TICKS {
                let tick = (hidden(Mark::Long(index)), BackgroundColor(theme::TEXT_DIM));
                ruler.spawn(tick);
                // A number is held by a point without a size, which its tick moves.
                ruler.spawn((
                    hidden(Mark::Number(index)),
                    children![(
                        theme::figure("", 11.0, theme::TEXT_DIM),
                        TextLayout::no_wrap(),
                        Node {
                            position_type: PositionType::Absolute,
                            ..default()
                        },
                        UiTransform::IDENTITY,
                    )],
                ));
            }
            ruler.spawn((hidden(Mark::Barrier), BackgroundColor(theme::DANGER)));
            ruler.spawn((
                hidden(Mark::Caption),
                theme::label("m to barrier", 11.0, theme::TEXT_DIM),
                TextLayout::no_wrap(),
            ));
        });
}

/// A bar of the ruler: where its corner goes and how large it is, from its place along the
/// edge, its length along it and how far it sticks out of the sheet.
fn bar(upright: bool, from: f32, length: f32, out: f32) -> (Vec2, Vec2) {
    let (corner, size) = (Vec2::new(SHEET_EDGE - out, from), Vec2::new(out, length));
    if upright {
        (corner, size)
    } else {
        (corner.yx(), size.yx())
    }
}

/// Where a mark of the ruler goes and, for a bar, how large it is. Nothing if it doesn't show.
fn lay(mark: Mark, reading: &Reading) -> Option<(Vec2, Option<Vec2>)> {
    let upright = reading.upright;
    let inside = SHEET_EDGE + NUMBER_GAP;
    let sized = |(corner, size): (Vec2, Vec2)| (corner, Some(size));
    match mark {
        Mark::Short(index) => {
            let at = *reading.short.get(index)?;
            Some(sized(bar(upright, at.round(), 1.0, TICK.0)))
        }
        Mark::Long(index) => {
            let (at, _) = *reading.long.get(index)?;
            Some(sized(bar(upright, at.round(), 1.0, TICK.1)))
        }
        Mark::Number(index) => {
            let (at, _) = *reading.long.get(index)?;
            let at = if upright {
                Vec2::new(inside, at)
            } else {
                Vec2::new(at, inside)
            };
            // The caption has the corner of the sheet to itself.
            let free = at.x > SHEET_EDGE + CAPTION_ROOM.x || at.y > SHEET_EDGE + CAPTION_ROOM.y;
            free.then_some((at.round(), None))
        }
        Mark::Barrier => {
            let (from, to) = reading.barrier?;
            let (from, to) = (from.min(to), from.max(to));
            let length = (to - from).max(BARRIER_MARK.1);
            let from = (from + to - length) / 2.0;
            Some(sized(bar(
                upright,
                from.round(),
                length.round(),
                BARRIER_MARK.0,
            )))
        }
        Mark::Caption => Some((Vec2::new(inside + 1.0, inside), None)),
    }
}

/// The sector that the ruler measures, among those that are loaded: the one that the run has
/// reached. The room of arena mode has no barrier to measure a distance to.
fn measured<'a>(
    mut plans: impl Iterator<Item = &'a SectorPlan>,
    sector: u32,
) -> Option<&'a SectorPlan> {
    let plan = plans.find(|plan| plan.index == sector);
    plan.filter(|plan| plan.kind != SectorKind::Room)
}

/// Lays the ruler along the edge of the sheet.
fn measure(
    screen: OnScreen,
    view: Res<View>,
    run: Option<Res<Run>>,
    sectors: Query<&Sector>,
    barriers: Query<&Barrier>,
    mut marks: Query<(
        &Mark,
        &mut Node,
        &mut UiTransform,
        &mut Visibility,
        Option<&mut BackgroundColor>,
        Option<&Children>,
    )>,
    mut numbers: Query<(&mut Text, &mut UiTransform), Without<Mark>>,
) {
    let sector = run.as_ref().map(|run| run.sector);
    let plans = sectors.iter().map(|sector| &sector.layout.plan);
    let plan = sector.and_then(|sector| measured(plans, sector));
    let reading = plan.and_then(|plan| read(plan, view.focus(), &screen));
    let mut closing = barriers
        .iter()
        .filter(|barrier| Some(barrier.sector) == sector);
    let unlocked = closing.any(|barrier| barrier.unlocked);
    for (mark, mut node, mut transform, mut visibility, background, children) in &mut marks {
        let laid = reading.as_ref().and_then(|reading| lay(*mark, reading));
        let wanted = if laid.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        let (Some((corner, size)), Some(reading)) = (laid, reading.as_ref()) else {
            continue;
        };
        let held = Val2::px(corner.x, corner.y);
        if transform.translation != held {
            transform.translation = held;
        }
        if let Some(size) = size {
            let (width, height) = (px(size.x), px(size.y));
            if node.width != width || node.height != height {
                (node.width, node.height) = (width, height);
            }
        }
        match (*mark, background) {
            (Mark::Barrier, Some(mut background)) => {
                let color = if unlocked { theme::GOOD } else { theme::DANGER };
                if background.0 != color {
                    background.0 = color;
                }
            }
            (Mark::Number(index), _) => {
                let Some((_, meters)) = reading.long.get(index) else {
                    continue;
                };
                // Along the left edge a number starts at its point and is centered on its
                // tick. Along the top edge it hangs under its point, centered on its tick.
                let hold = if reading.upright {
                    Val2::new(px(0), percent(-50))
                } else {
                    Val2::new(percent(-50), px(0))
                };
                for child in children.into_iter().flatten() {
                    let Ok((mut text, mut place)) = numbers.get_mut(*child) else {
                        continue;
                    };
                    let content = meters.to_string();
                    if text.0 != content {
                        text.0 = content;
                    }
                    if place.translation != hold {
                        place.translation = hold;
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::map::Corridor;

    use super::*;

    #[test]
    fn the_ruler_measures_the_sector_that_the_run_has_reached_and_no_room() {
        let mut corridor = Corridor::new(7);
        let loaded: Vec<SectorPlan> = (2..5).map(|index| corridor.plan(index).clone()).collect();
        let found = |sector: u32| measured(loaded.iter(), sector).map(|plan| plan.index);
        assert_eq!(found(3), Some(3));
        assert_eq!(found(7), None);
        // The room of arena mode goes by the number 0, which is the sector of a run before
        // its first group. Nothing is measured there.
        let room = tanku_core::map::Room::new(7);
        assert!(measured([room.plan()].into_iter(), 0).is_none());
        assert!(measured([corridor.plan(0)].into_iter(), 0).is_some());
    }

    #[test]
    fn the_distance_is_measured_along_the_sector() {
        // The start bay of every corridor heads north, from the origin to its barrier 16 m
        // further.
        let mut corridor = Corridor::new(7);
        let bay = corridor.plan(0).clone();
        assert_eq!(distance(&bay, Vec2::new(0.0, 5.0)), 11.0);
        // Being off to a side changes nothing.
        assert_eq!(distance(&bay, Vec2::new(6.0, 5.0)), 11.0);
        assert_eq!(barrier_reach(&bay), 0.0);
    }

    #[test]
    fn a_barrier_along_a_side_is_measured_to_its_middle() {
        // Some sectors leave by a side: their barrier lies along the way they run.
        let mut found = 0;
        for seed in 0..20 {
            let mut corridor = Corridor::new(seed);
            for index in 0..30 {
                let plan = corridor.plan(index);
                let reach = barrier_reach(plan);
                if reach == 0.0 {
                    continue;
                }
                found += 1;
                // Each end of the barrier is that far from its middle, one on each side.
                let ends = [plan.gate.0, plan.gate.1].map(|end| distance(plan, end.as_vec2()));
                assert_eq!(ends[0].abs(), reach);
                assert_eq!(ends[0], -ends[1]);
            }
        }
        assert!(found > 0, "no sector leaves by a side");
    }

    #[test]
    fn the_ruler_is_as_long_as_its_sector() {
        let mut corridor = Corridor::new(7);
        // The start bay is 16 m long, and its barrier is its far end.
        assert_eq!(extent(corridor.plan(0)), (0.0, 16.0));
        for index in 1..40 {
            let plan = corridor.plan(index);
            let (nearest, furthest) = extent(plan);
            // A sector ends at its barrier, or half the barrier further when it leaves by a
            // side, and it is 32 to 80 m long.
            assert_eq!(nearest, -barrier_reach(plan));
            assert!(
                (32.0..=80.0).contains(&(furthest - nearest)),
                "sector {index}"
            );
        }
    }

    #[test]
    fn ticks_stay_apart_whatever_the_ruler_shows() {
        // The game shows 30 m on 1080 pixels: a tick every meter, a number every 5 m.
        assert_eq!(steps(36.0), (1.0, 5));
        // Seen from further away, ticks get scarcer.
        assert_eq!(steps(9.0), (5.0, 2));
        assert_eq!(steps(1.5), (10.0, 5));
        assert_eq!(steps(0.01), (50.0, 2));
        for pixels_per_meter in [0.5, 1.0, 3.0, 8.0, 20.0, 150.0] {
            assert!(steps(pixels_per_meter).0 * pixels_per_meter >= TICK_SPACING);
        }
    }

    #[test]
    fn ticks_count_the_steps_between_two_distances() {
        assert_eq!(ticks(2.5, 12.2, 1.0), 3..=12);
        // The ruler may run either way, and past the barrier.
        assert_eq!(ticks(12.2, -3.5, 1.0), -3..=12);
        assert_eq!(ticks(4.0, 31.0, 5.0), 1..=6);
    }

    #[test]
    fn a_bar_lies_along_the_edge_it_belongs_to() {
        // Along the left edge, a tick sticks out to the left of the sheet.
        let (corner, size) = bar(true, 300.0, 1.0, 5.0);
        assert_eq!(
            (corner, size),
            (Vec2::new(SHEET_EDGE - 5.0, 300.0), Vec2::new(5.0, 1.0))
        );
        // Along the top edge, it sticks out above it.
        let (corner, size) = bar(false, 300.0, 1.0, 5.0);
        assert_eq!(
            (corner, size),
            (Vec2::new(300.0, SHEET_EDGE - 5.0), Vec2::new(1.0, 5.0))
        );
    }
}
