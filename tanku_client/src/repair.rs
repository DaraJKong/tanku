//! How the repair of a wreck shows, in a run of several players.
//!
//! A wreck lies in a circle in dashes: a teammate repairs it from within. While they do, the
//! circle is inked in from the top, the way a clock goes, and it is whole when the tank comes
//! back. The player at this screen is also told in words what to hold and how long is left,
//! above where the controls were shown at the start of the run.

use bevy::prelude::*;
use tanku_core::{
    rules::{REPAIR_RADIUS, REPAIR_TIME},
    sim::{Player, Wreck},
};

use crate::{
    Screen,
    camera::View,
    draft::{self, Draft, LIFT},
    scene, theme,
};

/// The arcs in which the circle of a wreck is inked in, the width of their line, and the
/// dashes of the circle.
const STEPS: usize = 36;
const INKED: f32 = 0.09;
const DASHES: usize = 72;

/// The circle around a wreck.
#[derive(Component)]
struct Circle;

/// An arc of the circle of a wreck, counted from the top the way a clock goes.
#[derive(Component)]
struct Step(usize);

#[derive(Resource)]
struct Rings {
    dashes: Handle<Mesh>,
    step: Handle<Mesh>,
    /// The paint of the dashes, and the ink of the arcs: both in the color of the players.
    paints: (Handle<StandardMaterial>, Handle<StandardMaterial>),
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(
            Update,
            (circle_wrecks, turn_circles, ink_circles)
                .chain()
                .run_if(in_state(Screen::Playing)),
        )
        .add_systems(OnExit(Screen::Playing), clear_circles);
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // A drawing in white takes the color of its material.
    let mut dashes = Draft::default();
    dashes.ring(
        Vec3::ZERO,
        (REPAIR_RADIUS - draft::LINE, REPAIR_RADIUS),
        (DASHES, 0.5),
        Color::WHITE,
    );
    // One arc, at the right of the wreck: each is turned to its place.
    let mut step = Draft::default();
    step.ring(
        Vec3::ZERO,
        (REPAIR_RADIUS - INKED, REPAIR_RADIUS),
        (1, 1.0 / STEPS as f32),
        Color::WHITE,
    );
    let paints = (
        scene::paint(&mut materials, theme::PLAYER.with_alpha(0.6)),
        scene::paint(&mut materials, theme::PLAYER),
    );
    commands.insert_resource(Rings {
        dashes: meshes.add(dashes.build()),
        step: meshes.add(step.build()),
        paints,
    });
}

/// Where an arc of the circle lies, in degrees around the wreck from the right of the screen
/// toward its bottom. The first arc starts at the top.
fn step_angle(step: usize) -> f32 {
    270.0 + step as f32 * 360.0 / STEPS as f32
}

/// How many arcs of the circle are inked in after so many seconds of repair: one as soon as
/// the repair starts, and all of them when it ends.
fn inked(repair: f32) -> usize {
    ((repair / REPAIR_TIME).clamp(0.0, 1.0) * STEPS as f32).ceil() as usize
}

/// Gives its circle to a tank that became a wreck, in the color of the players, and takes it
/// back from one that was repaired.
fn circle_wrecks(
    mut commands: Commands,
    wrecks: Query<Entity, (With<Player>, Added<Wreck>)>,
    mut repaired: RemovedComponents<Wreck>,
    circles: Query<(Entity, &ChildOf), With<Circle>>,
    rings: Res<Rings>,
) {
    for tank in repaired.read() {
        for (circle, parent) in &circles {
            if parent.parent() == tank {
                commands.entity(circle).despawn();
            }
        }
    }
    for tank in &wrecks {
        let (paint, ink) = &rings.paints;
        let circle = commands
            .spawn((
                Circle,
                Transform::from_xyz(0.0, 5.0 * LIFT, 0.0),
                Visibility::default(),
                ChildOf(tank),
                children![(Mesh3d(rings.dashes.clone()), MeshMaterial3d(paint.clone()),)],
            ))
            .id();
        for step in 0..STEPS {
            let turn = Quat::from_rotation_y(-step_angle(step).to_radians());
            commands.spawn((
                Step(step),
                Mesh3d(rings.step.clone()),
                MeshMaterial3d(ink.clone()),
                // Over the dashes.
                Transform::from_xyz(0.0, LIFT, 0.0).with_rotation(turn),
                Visibility::Hidden,
                ChildOf(circle),
            ));
        }
    }
}

/// Nothing is repaired once the run is over.
fn clear_circles(mut commands: Commands, circles: Query<Entity, With<Circle>>) {
    for circle in &circles {
        commands.entity(circle).despawn();
    }
}

/// Keeps the top of a circle at the top of the screen, whichever way the view is turned.
fn turn_circles(view: Res<View>, mut circles: Query<&mut Transform, With<Circle>>) {
    for mut transform in &mut circles {
        transform.rotation = Quat::from_rotation_y(view.yaw());
    }
}

/// Inks the circle of a wreck in as far as its repair has got. A repair that is interrupted
/// starts over, and so does the circle.
fn ink_circles(
    wrecks: Query<(&Wreck, &Children)>,
    circles: Query<&Children, With<Circle>>,
    mut steps: Query<(&Step, &mut Visibility)>,
) {
    for (wreck, children) in &wrecks {
        let inked = inked(wreck.repair);
        let circles = children.iter().filter_map(|child| circles.get(child).ok());
        for step in circles.flat_map(|circle| circle.iter()) {
            let Ok((step, mut visibility)) = steps.get_mut(step) else {
                continue;
            };
            let wanted = if step.0 < inked {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *visibility != wanted {
                *visibility = wanted;
            }
        }
    }
}

/// What the player at this screen is told about repairs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Word {
    /// Their tank is a wreck that nobody works on.
    Wrecked,
    /// A teammate repairs their tank, and has so many seconds to go.
    Mended(f32),
    /// The tank of a teammate is a wreck somewhere.
    Wanted,
    /// Their tank is in the circle of a wreck.
    Within,
    /// They repair a wreck, and have so many seconds to go.
    Repairing(f32),
}

/// What there is to tell the player whose tank is at a place: whether it is a wreck, and
/// how far its repair has got, or whether its player holds the repair key. The other wrecks
/// are given with how far their repair has got.
pub fn word(own: (Vec2, Option<f32>, bool), wrecks: &[(Vec2, f32)]) -> Option<Word> {
    let (place, wrecked, holds) = own;
    let left = |repair: f32| REPAIR_TIME - repair;
    if let Some(repair) = wrecked {
        return Some(if repair > 0.0 {
            Word::Mended(left(repair))
        } else {
            Word::Wrecked
        });
    }
    // The key repairs every wreck within reach at once. The one that is done first counts.
    let within = wrecks
        .iter()
        .filter(|(wreck, _)| wreck.distance(place) <= REPAIR_RADIUS);
    let furthest = within.map(|(_, repair)| *repair).max_by(f32::total_cmp);
    match furthest {
        Some(repair) if holds => Some(Word::Repairing(left(repair))),
        Some(_) => Some(Word::Within),
        None if !wrecks.is_empty() => Some(Word::Wanted),
        None => None,
    }
}

impl Word {
    /// The sentence, with the name of the repair key.
    pub fn sentence(self, key: &str) -> String {
        let seconds = |left: f32| left.max(0.0).ceil() as u32;
        match self {
            Word::Wrecked => "Your tank is destroyed - a teammate can repair it".to_string(),
            Word::Mended(left) => {
                format!("A teammate repairs your tank - {} s", seconds(left))
            }
            Word::Wanted => format!("Hold {key} in the circle of a wreck to repair it"),
            Word::Within => format!("Hold {key} to repair"),
            Word::Repairing(left) => {
                format!("Repairing - {} s - keep holding {key}", seconds(left))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game as far as the circles go, with the tank of a player in it.
    fn game_with_a_tank() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<View>()
            .add_systems(Startup, setup)
            .add_systems(Update, (circle_wrecks, turn_circles, ink_circles).chain());
        let tank = app.world_mut().spawn(Player { seat: 1 }).id();
        app.update();
        (app, tank)
    }

    /// How many circles there are, and how many of their arcs are inked in.
    fn drawn(app: &mut App) -> (usize, usize) {
        let world = app.world_mut();
        let circles = world.query::<&Circle>().iter(world).count();
        let mut steps = world.query::<(&Step, &Visibility)>();
        let shown = |(_, visibility): &(&Step, &Visibility)| **visibility != Visibility::Hidden;
        (circles, steps.iter(world).filter(shown).count())
    }

    #[test]
    fn a_wreck_has_a_circle_that_follows_its_repair() {
        let (mut app, tank) = game_with_a_tank();
        assert_eq!(drawn(&mut app), (0, 0));
        // The tank is destroyed: its circle is there, in dashes only.
        app.world_mut().entity_mut(tank).insert(Wreck::default());
        app.update();
        assert_eq!(drawn(&mut app), (1, 0));
        let world = app.world_mut();
        assert_eq!(world.query::<&Step>().iter(world).count(), STEPS);
        // Its repair goes on, and is interrupted: the circle starts over.
        app.world_mut().get_mut::<Wreck>(tank).unwrap().repair = REPAIR_TIME / 4.0;
        app.update();
        assert_eq!(drawn(&mut app), (1, STEPS / 4));
        app.world_mut().get_mut::<Wreck>(tank).unwrap().repair = 0.0;
        app.update();
        assert_eq!(drawn(&mut app), (1, 0));
        // The tank is repaired: its circle goes. Destroyed again, it has a new one.
        app.world_mut().entity_mut(tank).remove::<Wreck>();
        app.update();
        assert_eq!(drawn(&mut app), (0, 0));
        app.world_mut()
            .entity_mut(tank)
            .insert(Wreck { repair: 1.0 });
        app.update();
        assert_eq!(drawn(&mut app), (1, STEPS / 4));
    }

    #[test]
    fn the_circle_of_a_wreck_is_inked_in_from_the_top_as_its_repair_goes_on() {
        // Nothing before the repair starts, an arc as soon as it does, and the whole circle
        // at the end.
        assert_eq!(inked(0.0), 0);
        assert_eq!(inked(1.0 / 60.0), 1);
        assert_eq!(inked(REPAIR_TIME / 2.0), STEPS / 2);
        assert_eq!(inked(REPAIR_TIME), STEPS);
        assert_eq!(inked(REPAIR_TIME + 1.0), STEPS);
        // The first arc starts at the top of the screen, and the arcs go round once, the way
        // a clock goes: by the right, then by the bottom.
        assert_eq!(step_angle(0), 270.0);
        assert_eq!(step_angle(STEPS / 4), 360.0);
        assert_eq!(step_angle(STEPS) - step_angle(0), 360.0);
    }

    #[test]
    fn a_player_is_told_what_to_do_about_a_wreck() {
        let here = Vec2::ZERO;
        let near = Vec2::new(REPAIR_RADIUS - 0.1, 0.0);
        let far = Vec2::new(REPAIR_RADIUS + 0.1, 0.0);
        // Nothing while every tank runs.
        assert_eq!(word((here, None, false), &[]), None);
        assert_eq!(word((here, None, true), &[]), None);
        // A wreck somewhere, then within reach, then under repair.
        assert_eq!(word((here, None, false), &[(far, 0.0)]), Some(Word::Wanted));
        assert_eq!(word((here, None, true), &[(far, 0.0)]), Some(Word::Wanted));
        assert_eq!(
            word((here, None, false), &[(near, 0.0)]),
            Some(Word::Within)
        );
        assert_eq!(
            word((here, None, true), &[(near, 1.5)]),
            Some(Word::Repairing(REPAIR_TIME - 1.5))
        );
        // Of two wrecks within reach, the one that is done first counts.
        assert_eq!(
            word((here, None, true), &[(near, 1.0), (-near, 3.0), (far, 3.5)]),
            Some(Word::Repairing(REPAIR_TIME - 3.0))
        );
        // The player whose tank is the wreck waits, and sees when someone is at it.
        assert_eq!(
            word((here, Some(0.0), true), &[(near, 0.0)]),
            Some(Word::Wrecked)
        );
        assert_eq!(
            word((here, Some(1.0), false), &[]),
            Some(Word::Mended(REPAIR_TIME - 1.0))
        );
    }

    #[test]
    fn the_words_name_the_key_and_count_the_seconds_down() {
        assert_eq!(
            Word::Within.sentence("Left Shift"),
            "Hold Left Shift to repair"
        );
        assert_eq!(
            Word::Repairing(4.0).sentence("R"),
            "Repairing - 4 s - keep holding R"
        );
        // A second that has started is still to go.
        assert_eq!(
            Word::Repairing(0.2).sentence("R"),
            "Repairing - 1 s - keep holding R"
        );
        assert_eq!(
            Word::Mended(2.5).sentence("R"),
            "A teammate repairs your tank - 3 s"
        );
        assert!(
            Word::Wanted
                .sentence("R")
                .starts_with("Hold R in the circle")
        );
        assert!(!Word::Wrecked.sentence("R").contains('R'));
    }
}
