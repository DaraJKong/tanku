//! The balloons that name the players, as ISO 6433 names the items of an assembly: the
//! number of a player's seat in a circle. The circle tells how the player's tank is: a full
//! line while it runs, a line in dashes for a wreck, inked in from the top as the wreck is
//! repaired, red for a moment when it is destroyed, and struck out for a moment when its
//! player leaves the game.

use std::f32::consts::TAU;

use bevy::{
    prelude::*,
    ui::{AngularColorStop, BorderGradient, ConicGradient, UiPosition, UiSystems},
};

use crate::theme;

/// The width of the line of a balloon: the wide line of the interface.
pub const LINE: f32 = 2.0;
/// The gap after each dash of a balloon in dashes, in pixels. Each dash is an eighth of the
/// width of the balloon: 6 px on one of 48.
const GAP: f32 = 3.0;
/// The red ring that closes on the balloon of a tank that was just destroyed: how far from
/// the balloon it starts, how wide it is, and how long it takes to close.
const RING_REACH: f32 = 10.0;
const RING_WIDTH: f32 = 1.5;
pub const RING_TIME: f32 = 0.6;

/// How a player's tank is, as its balloon draws it.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum State {
    /// The tank runs.
    Runs,
    /// The tank is a wreck, repaired this far, from nothing to done.
    Wreck(f32),
    /// The tank was destroyed this many seconds ago.
    Lost(f32),
    /// The player left the game.
    Left,
    /// Nobody sits on the seat.
    Open,
}

/// The balloon of the player of a seat.
#[derive(Component, Clone, Copy, Debug)]
pub struct Balloon {
    pub seat: u8,
    /// How wide it is, in pixels.
    width: f32,
}

/// The figure of a balloon.
#[derive(Component)]
struct Figure;

/// The line that strikes a balloon out.
#[derive(Component)]
struct Strike;

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, paint.before(UiSystems::Prepare));
}

/// The balloon of the player of a seat, this wide, its figure this high, in a tank that runs
/// until its state is set.
pub fn balloon(seat: u8, width: f32, figure: f32) -> impl Bundle {
    (
        Balloon { seat, width },
        State::Runs,
        Node {
            width: px(width),
            height: px(width),
            border: UiRect::all(px(LINE)),
            border_radius: BorderRadius::MAX,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            flex_shrink: 0.0,
            ..default()
        },
        BorderColor::all(Color::NONE),
        BorderGradient::default(),
        BackgroundColor(theme::BLOCK_GROUND),
        Outline::new(px(0), px(0), Color::NONE),
        children![
            (
                Figure,
                theme::figure((seat + 1).to_string(), figure, theme::TEXT),
                TextLayout::no_wrap(),
            ),
            (
                Strike,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(width * 0.9),
                    height: px(LINE),
                    ..default()
                },
                BackgroundColor(theme::TEXT_DIM),
                UiTransform::from_rotation(Rot2::degrees(-45.0)),
                Visibility::Hidden,
            ),
        ],
    )
}

/// How a balloon is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    /// The color of its line all the way around, from its top clockwise.
    pub line: Vec<AngularColorStop>,
    pub figure: Color,
    /// The ring around it.
    pub ring: Outline,
    pub struck: bool,
}

/// How a balloon this wide is drawn for a tank in this state.
pub fn look(state: State, width: f32) -> Look {
    let full = |color: Color| {
        vec![
            AngularColorStop::new(color, 0.0),
            AngularColorStop::new(color, TAU),
        ]
    };
    let none = Outline::new(px(0), px(0), Color::NONE);
    match state {
        State::Runs => Look {
            line: full(theme::PLAYER),
            figure: theme::TEXT,
            ring: none,
            struck: false,
        },
        State::Wreck(repaired) => Look {
            line: dashed(theme::PLAYER, width, repaired),
            figure: theme::TEXT_DIM,
            ring: none,
            struck: false,
        },
        State::Lost(since) => {
            let closes = (1.0 - since / RING_TIME).clamp(0.0, 1.0);
            let ring = if closes > 0.0 {
                Outline::new(
                    px(RING_WIDTH),
                    px(1.0 + (RING_REACH - 1.0) * closes),
                    theme::DANGER,
                )
            } else {
                none
            };
            Look {
                line: full(theme::DANGER),
                figure: theme::DANGER,
                ring,
                struck: false,
            }
        }
        State::Left => Look {
            line: full(theme::RULE_DIM),
            figure: theme::RULE,
            ring: none,
            struck: true,
        },
        State::Open => Look {
            line: dashed(theme::RULE_DIM, width, 0.0),
            figure: theme::RULE,
            ring: none,
            struck: false,
        },
    }
}

/// A line in dashes around a balloon this wide, inked in from its top clockwise this far: the
/// dashes stay where they are as the ink comes over them.
fn dashed(color: Color, width: f32, inked: f32) -> Vec<AngularColorStop> {
    // The dashes are counted along the middle of the line, and stretched to go around whole.
    let around = std::f32::consts::PI * (width - LINE);
    let pitch = width / 8.0 + GAP;
    let count = (around / pitch).round().max(1.0);
    let (period, dash) = (TAU / count, TAU / count * (width / 8.0) / pitch);
    let inked = inked.clamp(0.0, 1.0) * TAU;
    let mut stops = Vec::new();
    // Each stretch of one color is two stops, so that the colors don't blend.
    let mut stretch = |color: Color, from: f32, to: f32| {
        if to > from {
            stops.push(AngularColorStop::new(color, from));
            stops.push(AngularColorStop::new(color, to));
        }
    };
    for index in 0..count as usize {
        // Each period ends where the next starts, to the last bit.
        let (start, end) = (index as f32 * period, (index + 1) as f32 * period);
        let dash_end = start + dash;
        // The dash, then the gap, which is inked where the ink has come.
        stretch(color, start, dash_end);
        stretch(color, dash_end, inked.clamp(dash_end, end));
        stretch(Color::NONE, inked.clamp(dash_end, end), end);
    }
    stops
}

/// Draws every balloon as its state says, when the state changes.
fn paint(
    mut balloons: Query<
        (
            &Balloon,
            &State,
            &mut BorderGradient,
            &mut Outline,
            &Children,
        ),
        Changed<State>,
    >,
    mut figures: Query<&mut TextColor, With<Figure>>,
    mut strikes: Query<&mut Visibility, With<Strike>>,
) {
    for (balloon, state, mut line, mut ring, children) in &mut balloons {
        let look = look(*state, balloon.width);
        // The top of the balloon is where the line starts.
        let wanted = BorderGradient::from(ConicGradient::new(UiPosition::CENTER, look.line));
        if *line != wanted {
            *line = wanted;
        }
        if *ring != look.ring {
            *ring = look.ring;
        }
        for child in children.iter() {
            if let Ok(mut ink) = figures.get_mut(child) {
                ink.set_if_neq(TextColor(look.figure));
            }
            if let Ok(mut visibility) = strikes.get_mut(child) {
                let shown = if look.struck {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                visibility.set_if_neq(shown);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The share of the line that is drawn, and the share that is drawn in a color.
    fn shares(stops: &[AngularColorStop], color: Color) -> f32 {
        let drawn = stops.chunks(2).filter(|pair| pair[0].color == color);
        let drawn: f32 = drawn
            .map(|pair| pair[1].angle.unwrap() - pair[0].angle.unwrap())
            .sum();
        drawn / TAU
    }

    #[test]
    fn a_wreck_is_drawn_in_dashes_that_the_repair_inks_in() {
        let blue = theme::PLAYER;
        let wreck = look(State::Wreck(0.0), 48.0).line;
        // Dashes of 6 px for gaps of 3: two thirds of the line.
        assert!((shares(&wreck, blue) - 2.0 / 3.0).abs() < 0.02);
        // The stops go around once, in order, and each stretch has one color.
        let angles: Vec<f32> = wreck.iter().map(|stop| stop.angle.unwrap()).collect();
        assert!(angles.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!((angles[angles.len() - 1] - TAU).abs() < 1e-3);
        assert!(wreck.chunks(2).all(|pair| pair[0].color == pair[1].color));
        // Inked 40 %: what is past the top by less than 40 % of the way around is whole.
        let inked = look(State::Wreck(0.4), 48.0).line;
        let share = shares(&inked, blue);
        assert!((share - (0.4 + 0.6 * 2.0 / 3.0)).abs() < 0.03, "{share}");
        let inked_part = inked
            .chunks(2)
            .filter(|pair| pair[1].angle.unwrap() <= 0.4 * TAU);
        assert!(inked_part.clone().count() > 0);
        assert!(inked_part.clone().all(|pair| pair[0].color == blue));
        // Whole again once repaired.
        assert!((shares(&look(State::Wreck(1.0), 48.0).line, blue) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn a_balloon_tells_how_the_tank_is() {
        let runs = look(State::Runs, 36.0);
        assert_eq!((runs.figure, runs.struck), (theme::TEXT, false));
        assert!((shares(&runs.line, theme::PLAYER) - 1.0).abs() < 1e-4);
        // A wreck's figure is dim.
        assert_eq!(look(State::Wreck(0.0), 36.0).figure, theme::TEXT_DIM);
        // A tank just destroyed is red, and a red ring closes on it.
        let lost = look(State::Lost(0.0), 36.0);
        assert_eq!(lost.figure, theme::DANGER);
        assert_eq!(lost.ring.color, theme::DANGER);
        let reach = |ring: Outline| match ring.offset {
            Val::Px(reach) => reach,
            _ => f32::NAN,
        };
        let closing = look(State::Lost(RING_TIME / 2.0), 36.0).ring;
        assert!(reach(closing) < reach(lost.ring));
        assert_eq!(look(State::Lost(RING_TIME), 36.0).ring.color, Color::NONE);
        // A player who left is struck out.
        assert!(look(State::Left, 36.0).struck);
        // An open seat is drawn in dashes, dim.
        let open = look(State::Open, 48.0);
        assert!((shares(&open.line, theme::RULE_DIM) - 2.0 / 3.0).abs() < 0.02);
        assert_eq!((open.figure, open.struck), (theme::RULE, false));
    }
}
