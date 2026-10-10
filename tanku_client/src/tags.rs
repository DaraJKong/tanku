//! The tags of the barriers. At each end of a barrier, a balloon split by the line of the
//! barrier tells which sector lies on each side, as a section mark does on a plan. A caption
//! next to one of them tells whether the barrier is locked, which its color alone would not
//! tell everyone.
//!
//! The tags are part of the interface, drawn over the game where the barriers end. They keep
//! their size and stay upright whichever way the view is turned.

use bevy::prelude::*;
use tanku_core::sim::Barrier;

use crate::{
    Screen,
    camera::{self, OnScreen},
    scene, theme,
};

/// Barriers that can be loaded at once: one at the end of each of three sectors, and the one
/// in front of the oldest of them.
const SLOTS: usize = 4;
/// How far past the end of its barrier a balloon stands, in meters.
const REACH: f32 = 1.1;
/// Radius of a balloon, in pixels.
const RADIUS: f32 = 16.0;
/// How far the line that splits a balloon goes on toward its barrier.
const LEAD: f32 = 10.0;
/// How far from that line the two numbers stand.
const NUMBER_OFFSET: f32 = 8.0;
/// The gap between a balloon and the caption of its barrier.
const CAPTION_GAP: f32 = 8.0;
/// A tag this far outside the screen isn't drawn.
const OUTSIDE: f32 = 40.0;

/// The balloon at one end of the barrier that a slot shows.
#[derive(Component)]
struct Balloon {
    slot: usize,
    end: usize,
}

/// The parts of a balloon.
#[derive(Component, Clone, Copy)]
enum Part {
    Ring,
    /// The line of the barrier, which splits the balloon in two.
    Divider,
    /// The number of the sector that the barrier closes.
    Near,
    /// The number of the sector behind it.
    Far,
}

/// What stands for the caption of a barrier on the screen, and the caption itself.
#[derive(Component)]
struct Caption {
    slot: usize,
}

#[derive(Component)]
struct CaptionText;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Screen::Playing), build)
        .add_systems(
            Update,
            place
                .after(camera::follow)
                .run_if(in_state(Screen::Playing)),
        );
}

/// The numbers in a balloon: the sector that the barrier closes, and the one behind it.
fn numbers(barrier: &Barrier) -> (String, String) {
    (barrier.sector.to_string(), (barrier.sector + 1).to_string())
}

/// What the caption of a barrier says.
fn caption(barrier: &Barrier) -> String {
    let state = if barrier.unlocked { "OPEN" } else { "LOCKED" };
    format!("BARRIER {} \u{b7} {state}", barrier.sector)
}

/// The color of a barrier, which its tags take.
fn color(barrier: &Barrier) -> Color {
    if barrier.unlocked {
        theme::GOOD
    } else {
        theme::DANGER
    }
}

/// Where the caption of a barrier goes, from the places of its two balloons on the screen:
/// next to which balloon, and on which side of it. It stands outside the corridor, to the left
/// of a barrier that lies across the screen and above one that runs up the screen.
fn caption_place(ends: [Vec2; 2]) -> (usize, Vec2) {
    let across = (ends[0].x - ends[1].x).abs() >= (ends[0].y - ends[1].y).abs();
    if across {
        (usize::from(ends[1].x < ends[0].x), Vec2::NEG_X)
    } else {
        (usize::from(ends[1].y < ends[0].y), Vec2::NEG_Y)
    }
}

/// A node without a size, which is moved to a point of the screen with what it holds.
fn anchor() -> impl Bundle {
    (
        DespawnOnExit(Screen::Playing),
        Node {
            position_type: PositionType::Absolute,
            width: px(0),
            height: px(0),
            ..default()
        },
        UiTransform::IDENTITY,
        Visibility::Hidden,
        // Under the panels of the display and under the menus.
        GlobalZIndex(-1),
        Pickable::IGNORE,
    )
}

/// What holds something by its middle: it is laid out with its corner on a point, and moved
/// back by half of its size.
fn centered() -> (Node, UiTransform) {
    (
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        UiTransform::from_translation(Val2::new(percent(-50), percent(-50))),
    )
}

fn build(mut commands: Commands) {
    let line = theme::DANGER;
    for slot in 0..SLOTS {
        for end in 0..2 {
            let (ring, ring_place) = centered();
            let (divider, divider_place) = centered();
            let (near, near_place) = centered();
            let (far, far_place) = centered();
            commands.spawn((
                Balloon { slot, end },
                anchor(),
                children![
                    (
                        Part::Ring,
                        Node {
                            width: px(2.0 * RADIUS),
                            height: px(2.0 * RADIUS),
                            border: UiRect::all(px(1.5)),
                            border_radius: BorderRadius::MAX,
                            ..ring
                        },
                        ring_place,
                        BorderColor::all(line),
                        BackgroundColor(theme::VOID.with_alpha(0.8)),
                    ),
                    (
                        Part::Divider,
                        Node {
                            width: px(2.0 * RADIUS + LEAD),
                            height: px(1.5),
                            ..divider
                        },
                        divider_place,
                        BackgroundColor(line),
                    ),
                    (Part::Near, theme::figure("", 12.0, line), near, near_place),
                    (Part::Far, theme::figure("", 12.0, line), far, far_place),
                ],
            ));
        }
        commands.spawn((
            Caption { slot },
            anchor(),
            children![(
                CaptionText,
                theme::label("", 12.0, line),
                // Its point has no width for a line to fit in, and it may lie over a wall.
                TextLayout::no_wrap(),
                Node {
                    position_type: PositionType::Absolute,
                    padding: UiRect::axes(px(5), px(1)),
                    ..default()
                },
                BackgroundColor(theme::VOID.with_alpha(0.8)),
                UiTransform::IDENTITY,
            )],
        ));
    }
}

/// Where a balloon is on the screen, and the two directions that lay out what it holds: toward
/// its barrier, and toward the sector behind the barrier.
struct Spot {
    at: Vec2,
    inward: Vec2,
    beyond: Vec2,
}

/// The spots of the two balloons of a barrier, if both have a place on the screen.
fn spots(barrier: &Barrier, screen: &OnScreen) -> Option<[Spot; 2]> {
    let middle = (barrier.from + barrier.to) / 2.0;
    let spot = |end: Vec2| {
        let outward = (end - middle).normalize_or_zero();
        let center = end + outward * REACH;
        // A balloon is level with the top of the wall that its barrier ends on.
        let on_screen = |point: Vec2| screen.point(scene::to_world(point, scene::WALL_HEIGHT));
        let at = on_screen(center)?;
        Some(Spot {
            at,
            inward: (on_screen(center - outward)? - at).normalize_or_zero(),
            beyond: (on_screen(center + barrier.outward)? - at).normalize_or_zero(),
        })
    };
    Some([spot(barrier.from)?, spot(barrier.to)?])
}

/// Moves a node of the display to a point of the screen, or hides it.
fn show(at: Option<Vec2>, transform: &mut UiTransform, visibility: &mut Visibility) {
    let wanted = match at {
        Some(at) => {
            transform.translation = Val2::px(at.x.round(), at.y.round());
            Visibility::Inherited
        }
        None => Visibility::Hidden,
    };
    if *visibility != wanted {
        *visibility = wanted;
    }
}

/// Puts the tags on the barriers that are loaded, and tells what they close.
fn place(
    screen: OnScreen,
    barriers: Query<&Barrier>,
    mut balloons: Query<(&Balloon, &mut UiTransform, &mut Visibility, &Children), Without<Part>>,
    mut parts: Query<
        (
            &Part,
            &mut Node,
            &mut UiTransform,
            Option<&mut Text>,
            Option<&mut TextColor>,
            Option<&mut BorderColor>,
            Option<&mut BackgroundColor>,
        ),
        (Without<Balloon>, Without<Caption>, Without<CaptionText>),
    >,
    mut captions: Query<
        (&Caption, &mut UiTransform, &mut Visibility, &Children),
        (Without<Balloon>, Without<Part>, Without<CaptionText>),
    >,
    mut texts: Query<
        (&mut Text, &mut TextColor, &mut UiTransform),
        (
            With<CaptionText>,
            Without<Part>,
            Without<Balloon>,
            Without<Caption>,
        ),
    >,
) {
    let Some(size) = screen.size() else {
        return;
    };
    // The same barrier keeps the same slot from one frame to the next.
    let mut loaded: Vec<&Barrier> = barriers.iter().collect();
    loaded.sort_by_key(|barrier| barrier.sector);
    let shown: Vec<Option<(&Barrier, [Spot; 2])>> = (0..SLOTS)
        .map(|slot| {
            let barrier = *loaded.get(slot)?;
            Some((barrier, spots(barrier, &screen)?))
        })
        .collect();
    let in_view = |at: Vec2| {
        let outside = Vec2::splat(OUTSIDE);
        at.cmpgt(-outside).all() && at.cmplt(size + outside).all()
    };

    for (balloon, mut transform, mut visibility, children) in &mut balloons {
        let found = shown[balloon.slot].as_ref();
        let spot = found.map(|(barrier, spots)| (*barrier, &spots[balloon.end]));
        let spot = spot.filter(|(_, spot)| in_view(spot.at));
        show(
            spot.map(|(_, spot)| spot.at),
            &mut transform,
            &mut visibility,
        );
        let Some((barrier, spot)) = spot else {
            continue;
        };
        let (line, (near, far)) = (color(barrier), numbers(barrier));
        for child in children.iter() {
            let Ok((part, mut node, mut turn, text, text_color, border, background)) =
                parts.get_mut(child)
            else {
                continue;
            };
            // Where the part lies around the middle of the balloon, and what it says.
            let (offset, content) = match part {
                Part::Ring => (Vec2::ZERO, None),
                Part::Divider => (spot.inward * LEAD / 2.0, None),
                Part::Near => (-spot.beyond * NUMBER_OFFSET, Some(&near)),
                Part::Far => (spot.beyond * NUMBER_OFFSET, Some(&far)),
            };
            let (left, top) = (px(offset.x.round()), px(offset.y.round()));
            if node.left != left || node.top != top {
                (node.left, node.top) = (left, top);
            }
            if let Part::Divider = part {
                let along = Rot2::radians(spot.inward.to_angle());
                if turn.rotation != along {
                    turn.rotation = along;
                }
            }
            if let (Some(mut text), Some(content)) = (text, content)
                && text.0 != *content
            {
                text.0.clone_from(content);
            }
            if let Some(mut text_color) = text_color
                && text_color.0 != line
            {
                text_color.0 = line;
            }
            match (part, border, background) {
                (Part::Ring, Some(mut border), _) if *border != BorderColor::all(line) => {
                    *border = BorderColor::all(line);
                }
                (Part::Divider, _, Some(mut background)) if background.0 != line => {
                    background.0 = line;
                }
                _ => {}
            }
        }
    }

    for (caption_of, mut transform, mut visibility, children) in &mut captions {
        let found = shown[caption_of.slot].as_ref();
        let placed = found.map(|(barrier, spots)| {
            let (end, side) = caption_place([spots[0].at, spots[1].at]);
            (
                *barrier,
                spots[end].at + side * (RADIUS + CAPTION_GAP),
                side,
            )
        });
        let placed = placed.filter(|(_, at, _)| in_view(*at));
        show(placed.map(|(_, at, _)| at), &mut transform, &mut visibility);
        let Some((barrier, _, side)) = placed else {
            continue;
        };
        for child in children.iter() {
            let Ok((mut text, mut text_color, mut hold)) = texts.get_mut(child) else {
                continue;
            };
            let content = caption(barrier);
            if text.0 != content {
                text.0 = content;
            }
            if text_color.0 != color(barrier) {
                text_color.0 = color(barrier);
            }
            // The caption ends at its point when it stands on the left of it, and is centered
            // over its point when it stands above it.
            let held = if side == Vec2::NEG_X {
                Val2::new(percent(-100), percent(-50))
            } else {
                Val2::new(percent(-50), percent(-100))
            };
            if hold.translation != held {
                hold.translation = held;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn barrier(sector: u32, unlocked: bool) -> Barrier {
        Barrier {
            sector,
            from: Vec2::new(-7.0, 16.0),
            to: Vec2::new(7.0, 16.0),
            outward: Vec2::Y,
            unlocked,
        }
    }

    #[test]
    fn a_balloon_names_the_sectors_on_both_sides_of_its_barrier() {
        let (near, far) = numbers(&barrier(12, false));
        assert_eq!((near.as_str(), far.as_str()), ("12", "13"));
        // The barrier of the start bay.
        let (near, far) = numbers(&barrier(0, false));
        assert_eq!((near.as_str(), far.as_str()), ("0", "1"));
    }

    #[test]
    fn a_caption_tells_in_words_whether_its_barrier_is_locked() {
        assert_eq!(caption(&barrier(12, false)), "BARRIER 12 \u{b7} LOCKED");
        assert_eq!(caption(&barrier(12, true)), "BARRIER 12 \u{b7} OPEN");
        assert_ne!(color(&barrier(12, false)), color(&barrier(12, true)));
    }

    #[test]
    fn a_caption_stands_outside_the_corridor() {
        // A barrier across the screen: the caption is on the left of its left balloon.
        let across = [Vec2::new(900.0, 300.0), Vec2::new(300.0, 310.0)];
        assert_eq!(caption_place(across), (1, Vec2::NEG_X));
        // A barrier up the screen: the caption is above its upper balloon.
        let up = [Vec2::new(500.0, 200.0), Vec2::new(510.0, 800.0)];
        assert_eq!(caption_place(up), (0, Vec2::NEG_Y));
    }
}
