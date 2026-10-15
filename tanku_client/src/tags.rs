//! The tags of the barriers. At each end of a barrier, a balloon split by the line of the
//! barrier tells which sector lies on each side, as a section mark does on a plan, with the
//! arrow of a section that points the way on. A caption next to one of them tells whether the
//! barrier is locked, which its color alone would not tell everyone.
//!
//! The tags are part of the interface, drawn over the game where the barriers end. They keep
//! their size and stay upright whichever way the view is turned.

use bevy::{
    prelude::*,
    ui::{AngularColorStop, ConicGradient, UiPosition},
};
use tanku_core::{
    geom,
    map::SectorKind,
    rules::BARRIER_GATHER_DISTANCE,
    sim::{Barrier, Player, Position, Run, Sector, Wreck},
};

use crate::{
    Screen,
    camera::{self, OnScreen},
    crew::LocalSeat,
    hud,
    lines::Line,
    scene, theme,
};

/// Barriers that can be loaded at once: one at the end of each of three sectors, and the one
/// in front of the oldest of them.
const SLOTS: usize = 4;
/// How far past the end of its barrier a balloon stands, in meters.
const REACH: f32 = 1.1;
/// Radius of a balloon, in pixels: two numbers of the smallest lettering fit in it side by
/// side, when the barrier stands up the screen.
const RADIUS: f32 = 30.0;
/// The width of its line, and of the line that splits it: the wide line of the interface.
const LINE: f32 = 2.0;
/// How far the line that splits a balloon goes on toward its barrier.
const LEAD: f32 = 10.0;
/// The gap between that line and each of the two numbers.
const NUMBER_GAP: f32 = 5.0;
/// How wide a figure is: all of them are as wide.
const FIGURE: f32 = 0.8 * theme::LABEL;
/// The room that a balloon keeps around numbers too wide for it, which stretch it.
const STRETCH_ROOM: f32 = 6.0;
/// The arrow of the section: how far from the balloon it starts, how long it is and how wide.
const ARROW: (f32, f32, f32) = (6.0, 12.0, 14.0);
/// The gap between a balloon and the caption of its barrier.
const CAPTION_GAP: f32 = 10.0;
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
    /// The arrow of the section, which points into the sector behind the barrier.
    Arrow,
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

/// Where the tags of the barriers and their captions stand on the screen, in the pixels of the
/// interface: what the stamps and the alerts of the notices keep clear of.
#[derive(Resource, Default)]
pub struct TagMarks(pub Vec<Rect>);

pub fn plugin(app: &mut App) {
    app.init_resource::<TagMarks>()
        .add_systems(OnEnter(Screen::Playing), build)
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

/// The players that a barrier waits for, among several: its sector is cleared, and it stays
/// locked until every tank that runs is close to it. It is said once a player is at the
/// barrier and others are not, of a barrier after a fight: right after the last kill, nobody
/// is there yet, and at the start of a run, everyone knows the way. They are given by seat,
/// with where their tanks are.
pub fn waiting(run: &Run, barrier: &Barrier, running: &[(u8, Vec2)]) -> Vec<u8> {
    let held = !run.solo && !barrier.unlocked && barrier.sector > 0;
    if !held || barrier.sector != run.sector || run.cleared < barrier.sector {
        return Vec::new();
    }
    let far = |at: &Vec2| {
        geom::distance_to_segment(*at, barrier.from, barrier.to) > BARRIER_GATHER_DISTANCE
    };
    let mut seats: Vec<u8> = running
        .iter()
        .filter(|(_, at)| far(at))
        .map(|(seat, _)| *seat)
        .collect();
    if seats.len() == running.len() {
        return Vec::new();
    }
    seats.sort();
    seats
}

/// The players of some seats, by their numbers, as the player at this screen reads them: "P2",
/// "you and P2", or "P2, P3 and P4".
pub fn names(seats: &[u8], own: u8) -> String {
    let mut seats = seats.to_vec();
    // The player at this screen comes first, as "you".
    seats.sort_by_key(|seat| (*seat != own, *seat));
    let name = |seat: &u8| {
        if *seat == own {
            "you".to_string()
        } else {
            hud::short_name(*seat)
        }
    };
    let names: Vec<String> = seats.iter().map(name).collect();
    match names.as_slice() {
        [first @ .., last] if !first.is_empty() => format!("{} and {last}", first.join(", ")),
        _ => names.concat(),
    }
}

/// What the caption of a barrier says: whether it is locked, and whom for when it waits for
/// players, and whether the sector behind it is an arena, which looks empty from here.
fn caption(barrier: &Barrier, (waits, own): (&[u8], u8), arena: bool) -> String {
    let state = if barrier.unlocked { "OPEN" } else { "LOCKED" };
    let waits = if waits.is_empty() {
        String::new()
    } else {
        format!(" \u{b7} WAITS FOR {}", names(waits, own).to_uppercase())
    };
    let ahead = if arena { " \u{b7} ARENA AHEAD" } else { "" };
    format!("BARRIER {} \u{b7} {state}{waits}{ahead}", barrier.sector)
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

/// How something is held on its point, by its own size: by its middle, or by the middle of
/// the side that looks back when it stands away from the point in a direction.
fn held_from(away: Vec2) -> Val2 {
    let part = |along: f32| percent((-50.0 + 50.0 * along).round());
    Val2::new(part(away.x), part(away.y))
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

/// The arrow of a section: a triangle with its point up, which is the wedge of a cone of
/// color from that point, cut by the box that holds it.
fn arrow(color: Color) -> ConicGradient {
    let (_, long, wide) = ARROW;
    // Straight down from the point is half a turn, and each side is that far from it.
    let side = (wide / 2.0).atan2(long);
    let (from, to) = (std::f32::consts::PI - side, std::f32::consts::PI + side);
    let stops = vec![
        AngularColorStop::new(Color::NONE, 0.0),
        AngularColorStop::new(Color::NONE, from),
        AngularColorStop::new(color, from),
        AngularColorStop::new(color, to),
        AngularColorStop::new(Color::NONE, to),
        AngularColorStop::new(Color::NONE, std::f32::consts::TAU),
    ];
    ConicGradient::new(UiPosition::TOP, stops)
}

fn build(mut commands: Commands) {
    let line = theme::DANGER;
    for slot in 0..SLOTS {
        for end in 0..2 {
            let (ring, ring_place) = centered();
            let (point, point_place) = centered();
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
                            border: UiRect::all(px(LINE)),
                            border_radius: BorderRadius::MAX,
                            ..ring
                        },
                        ring_place,
                        BorderColor::all(line),
                        BackgroundColor(theme::BLOCK_GROUND),
                    ),
                    (
                        Part::Arrow,
                        Node {
                            width: px(ARROW.2),
                            height: px(ARROW.1),
                            ..point
                        },
                        point_place,
                        BackgroundGradient::from(arrow(line)),
                    ),
                    (
                        Part::Divider,
                        Node {
                            width: px(2.0 * RADIUS + LEAD),
                            height: px(Line::new(LINE, line).across()),
                            ..divider
                        },
                        divider_place,
                        Line::new(LINE, line),
                    ),
                    (
                        Part::Near,
                        theme::figure("", theme::LABEL, line),
                        TextLayout::no_wrap(),
                        near,
                        near_place,
                    ),
                    (
                        Part::Far,
                        theme::figure("", theme::LABEL, line),
                        TextLayout::no_wrap(),
                        far,
                        far_place,
                    ),
                ],
            ));
        }
        commands.spawn((
            Caption { slot },
            anchor(),
            children![(
                CaptionText,
                theme::label("", theme::LABEL, line),
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

/// How far the balloon of a barrier reaches from its middle in a direction of the screen: a
/// capsule reaches further along its length.
fn extent(barrier: &Barrier, spot: &Spot, toward: Vec2) -> f32 {
    let (near, far) = numbers(barrier);
    let long = reach((&near, &far), spot.beyond);
    RADIUS + (long - RADIUS) * toward.dot(spot.beyond).abs()
}

/// How far a balloon reaches from its middle each way past its barrier: its radius, or further
/// where its numbers are too wide for it, which stretch it into a capsule. That is a sector of
/// three figures, side by side when the barrier stands up the screen.
fn reach(numbers: (&str, &str), beyond: Vec2) -> f32 {
    let figures = numbers.0.len().max(numbers.1.len()) as f32;
    let (wide, high) = (figures * FIGURE, theme::LABEL * 1.25);
    let extent = NUMBER_GAP + (wide * beyond.x).abs() + (high * beyond.y).abs();
    if extent <= RADIUS {
        RADIUS
    } else {
        extent + STRETCH_ROOM
    }
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

/// Moves a node of the display to a point of the screen, or hides it. What doesn't change is
/// left untouched: the engine draws again what is touched.
fn show(at: Option<Vec2>, transform: &mut Mut<UiTransform>, visibility: &mut Mut<Visibility>) {
    let wanted = match at {
        Some(at) => {
            let place = Val2::px(at.x.round(), at.y.round());
            if transform.translation != place {
                transform.translation = place;
            }
            Visibility::Inherited
        }
        None => Visibility::Hidden,
    };
    visibility.set_if_neq(wanted);
}

/// Puts the tags on the barriers that are loaded, and tells what they close.
fn place(
    screen: OnScreen,
    (run, running, seat): (
        Option<Res<Run>>,
        Query<(&Player, &Position), Without<Wreck>>,
        Res<LocalSeat>,
    ),
    barriers: Query<&Barrier>,
    sectors: Query<&Sector>,
    mut balloons: Query<(&Balloon, &mut UiTransform, &mut Visibility, &Children), Without<Part>>,
    mut parts: Query<
        (
            &Part,
            &mut Node,
            &mut UiTransform,
            Option<&mut Text>,
            Option<&mut TextColor>,
            Option<&mut BorderColor>,
            Option<&mut Line>,
            Option<&mut BackgroundGradient>,
        ),
        (Without<Balloon>, Without<Caption>, Without<CaptionText>),
    >,
    mut captions: Query<
        (&Caption, &mut UiTransform, &mut Visibility, &Children),
        (Without<Balloon>, Without<Part>, Without<CaptionText>),
    >,
    mut texts: Query<
        (&mut Text, &mut TextColor, &mut UiTransform, &ComputedNode),
        (
            With<CaptionText>,
            Without<Part>,
            Without<Balloon>,
            Without<Caption>,
        ),
    >,
    mut marks: ResMut<TagMarks>,
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
    // Where the balloons and their arrows stand, for what keeps clear of them.
    let mut taken: Vec<Rect> = Vec::new();
    for (barrier, spots) in shown.iter().flatten() {
        for spot in spots.iter().filter(|spot| in_view(spot.at)) {
            let reach = extent(barrier, spot, Vec2::X).max(extent(barrier, spot, Vec2::Y));
            let reach = reach + ARROW.0 + ARROW.1;
            taken.push(Rect::from_center_half_size(spot.at, Vec2::splat(reach)));
        }
    }

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
            let Ok((part, mut node, mut turn, text, text_color, border, divider, cone)) =
                parts.get_mut(child)
            else {
                continue;
            };
            let reach = reach((&near, &far), spot.beyond);
            // Where the part lies around the middle of the balloon, on which side of the
            // line that splits it, and what it says.
            let (offset, side, content) = match part {
                Part::Ring => (Vec2::ZERO, Vec2::ZERO, None),
                Part::Arrow => {
                    let (gap, long, _) = ARROW;
                    (spot.beyond * (reach + gap + long / 2.0), Vec2::ZERO, None)
                }
                Part::Divider => (spot.inward * LEAD / 2.0, Vec2::ZERO, None),
                Part::Near => (-spot.beyond * NUMBER_GAP, -spot.beyond, Some(&near)),
                Part::Far => (spot.beyond * NUMBER_GAP, spot.beyond, Some(&far)),
            };
            let (left, top) = (px(offset.x.round()), px(offset.y.round()));
            if node.left != left || node.top != top {
                (node.left, node.top) = (left, top);
            }
            // A number stands clear of the line, however long it is: it is held by the
            // middle of the side that faces the line, not by its own middle.
            let held = held_from(side);
            if turn.translation != held {
                turn.translation = held;
            }
            // The line lies along the barrier, the ring along the way past it, as long as
            // it reaches, and the arrow points that way.
            let along = match part {
                Part::Divider => Some(spot.inward.to_angle()),
                Part::Ring => Some(spot.beyond.to_angle()),
                Part::Arrow => Some(spot.beyond.to_angle() + std::f32::consts::FRAC_PI_2),
                Part::Near | Part::Far => None,
            };
            if let Some(angle) = along {
                let along = Rot2::radians(angle);
                if turn.rotation != along {
                    turn.rotation = along;
                }
            }
            if let Part::Ring = part {
                let length = px((2.0 * reach).round());
                if node.width != length {
                    node.width = length;
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
            match (part, border, divider, cone) {
                (Part::Ring, Some(mut border), ..) if *border != BorderColor::all(line) => {
                    *border = BorderColor::all(line);
                }
                (Part::Divider, _, Some(mut divider), _) if divider.ink != line => {
                    divider.ink = line;
                }
                (Part::Arrow, .., Some(mut cone)) => {
                    let wanted = BackgroundGradient::from(arrow(line));
                    if *cone != wanted {
                        *cone = wanted;
                    }
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
                spots[end].at + side * (extent(barrier, &spots[end], side) + CAPTION_GAP),
                side,
            )
        });
        let placed = placed.filter(|(_, at, _)| in_view(*at));
        show(placed.map(|(_, at, _)| at), &mut transform, &mut visibility);
        let Some((barrier, at, side)) = placed else {
            continue;
        };
        for child in children.iter() {
            let Ok((mut text, mut text_color, mut hold, computed)) = texts.get_mut(child) else {
                continue;
            };
            // As large as it was laid out the frame before.
            let size = computed.size() * computed.inverse_scale_factor();
            taken.push(caption_room(at, side, size));
            let arena = sectors.iter().any(|sector| {
                let plan = &sector.layout.plan;
                plan.index == barrier.sector + 1 && plan.kind == SectorKind::Arena
            });
            let running: Vec<(u8, Vec2)> = running
                .iter()
                .map(|(player, position)| (player.seat, position.0))
                .collect();
            let waits = run
                .as_ref()
                .map_or(Vec::new(), |run| waiting(run, barrier, &running));
            let content = caption(barrier, (&waits, seat.0), arena);
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
    if marks.0 != taken {
        marks.0 = taken;
    }
}

/// The room that a caption this large takes on the screen, from the point that it stands on
/// and the side of that point that it stands on: it ends at a point on its left, and stands
/// centered over a point under it.
fn caption_room(at: Vec2, side: Vec2, size: Vec2) -> Rect {
    if side == Vec2::NEG_X {
        Rect::new(
            at.x - size.x,
            at.y - size.y / 2.0,
            at.x,
            at.y + size.y / 2.0,
        )
    } else {
        Rect::new(
            at.x - size.x / 2.0,
            at.y - size.y,
            at.x + size.x / 2.0,
            at.y,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_caption_takes_the_room_beside_its_point() {
        let (at, size) = (Vec2::new(300.0, 200.0), Vec2::new(160.0, 20.0));
        // On the left of its point, which it ends at.
        let left = caption_room(at, Vec2::NEG_X, size);
        assert_eq!(left, Rect::new(140.0, 190.0, 300.0, 210.0));
        // Over its point, which it stands centered on.
        let over = caption_room(at, Vec2::NEG_Y, size);
        assert_eq!(over, Rect::new(220.0, 180.0, 380.0, 200.0));
    }

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
        assert_eq!(
            caption(&barrier(12, false), (&[], 0), false),
            "BARRIER 12 \u{b7} LOCKED"
        );
        assert_eq!(
            caption(&barrier(12, true), (&[], 0), false),
            "BARRIER 12 \u{b7} OPEN"
        );
        // An arena looks empty through its barrier, so the barrier says what it is.
        assert_eq!(
            caption(&barrier(12, false), (&[], 0), true),
            "BARRIER 12 \u{b7} LOCKED \u{b7} ARENA AHEAD"
        );
        // A barrier that waits for players names them.
        assert_eq!(
            caption(&barrier(12, false), (&[1], 0), false),
            "BARRIER 12 \u{b7} LOCKED \u{b7} WAITS FOR P2"
        );
        assert_eq!(
            caption(&barrier(12, false), (&[1, 3], 3), true),
            "BARRIER 12 \u{b7} LOCKED \u{b7} WAITS FOR YOU AND P2 \u{b7} ARENA AHEAD"
        );
        assert_ne!(color(&barrier(12, false)), color(&barrier(12, true)));
    }

    #[test]
    fn a_barrier_waits_for_the_players_who_are_far_from_it_once_its_sector_is_cleared() {
        let run = |solo: bool, sector: u32, cleared: u32| Run {
            seed_text: "TEST".into(),
            seed: 1,
            difficulty: default(),
            players: 3,
            team: 3,
            solo,
            sector,
            cleared,
            checkpoint: cleared,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase: tanku_core::sim::RunPhase::Playing,
            lost_to: None,
            playtest: false,
            earns: true,
            free: false,
            arena: false,
            lull: None,
            paused: false,
        };
        // The barrier lies across y = 16: P1 is next to it, P2 and P4 are far from it.
        let running = [
            (0, Vec2::new(0.0, 12.0)),
            (1, Vec2::new(0.0, -10.0)),
            (3, Vec2::new(5.0, 0.0)),
        ];
        let locked = barrier(12, false);
        assert_eq!(waiting(&run(false, 12, 12), &locked, &running), [1, 3]);
        // Nobody is at it yet: it is no matter of waiting for anyone.
        let away = [(0, Vec2::new(0.0, -10.0)), (1, Vec2::new(0.0, -12.0))];
        assert!(waiting(&run(false, 12, 12), &locked, &away).is_empty());
        // The barrier of the start bay is left alone.
        let bay = |cleared| Run {
            sector: 0,
            ..run(false, 0, cleared)
        };
        assert!(waiting(&bay(0), &barrier(0, false), &running).is_empty());
        // Not while it is fought, nor once it is open, nor for a single player, nor for the
        // barrier of a sector that is not the one reached.
        assert_eq!(
            waiting(&run(false, 12, 11), &locked, &running),
            [] as [u8; 0]
        );
        assert!(waiting(&run(false, 12, 12), &barrier(12, true), &running).is_empty());
        assert!(waiting(&run(true, 12, 12), &locked, &running).is_empty());
        assert!(waiting(&run(false, 13, 12), &locked, &running).is_empty());
        assert_eq!(names(&[1], 0), "P2");
        assert_eq!(names(&[1, 2, 3], 0), "P2, P3 and P4");
        // The player at this screen reads "you", first.
        assert_eq!(names(&[0, 2], 2), "you and P1");
    }

    #[test]
    fn a_number_stands_on_its_side_of_the_line_whatever_its_length() {
        // Held by its middle when it goes nowhere.
        assert_eq!(held_from(Vec2::ZERO), Val2::new(percent(-50), percent(-50)));
        // Above the line, it rests on it: its bottom is on its point.
        assert_eq!(
            held_from(Vec2::NEG_Y),
            Val2::new(percent(-50), percent(-100))
        );
        // On the right of the line, it starts at its point.
        assert_eq!(held_from(Vec2::X), Val2::new(percent(0), percent(-50)));
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
