//! The life that an arena leaves on its floor for a single player: a square like those that
//! count the lives on the display, in the circle that a tank drives into to take it. A caption
//! tells what it is, which its shape alone would not.
//!
//! A player who has every life can't take it. It is then the outline of itself in dashes, as
//! everything is that isn't there.

use bevy::prelude::*;
use tanku_core::{
    rules::{LIFE_REACH, SOLO_LIVES},
    sim::{Life, Position, Run},
};

use crate::{
    Screen,
    camera::{self, OnScreen},
    draft::{self, Draft},
    scene::{self, Paints},
    theme,
};

/// The side of the square, and how high it stands.
const SIDE: f32 = 0.9;
const HEIGHT: f32 = 0.45;
/// The gap between the circle and the caption above it, in pixels.
const CAPTION_GAP: f32 = 6.0;

#[derive(Resource)]
struct Drawings {
    offered: Handle<Mesh>,
    withheld: Handle<Mesh>,
    /// The inside of the circle, and the faint light that fills it.
    inside: Handle<Mesh>,
    light: Handle<StandardMaterial>,
}

/// One of the two drawings of a life: the one for a player who can take it, or the other.
#[derive(Component, Clone, Copy)]
struct Offered(bool);

/// What stands for the caption of a life on the screen, and the caption itself.
#[derive(Component)]
struct Caption;

#[derive(Component)]
struct CaptionText;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(OnEnter(Screen::Playing), build)
        .add_systems(Update, (dress, offer).chain().before(scene::place))
        .add_systems(
            Update,
            caption
                .after(camera::follow)
                .run_if(in_state(Screen::Playing)),
        );
}

/// A life that can be taken: a square in the color of the player, in a circle.
fn offered_drawing() -> Draft {
    let mut draft = Draft::default();
    let lit = theme::PLAYER;
    draft.outlined(
        Vec3::Y * HEIGHT / 2.0,
        Vec3::new(SIDE, HEIGHT, SIDE),
        (lit, theme::side(lit), theme::LINE),
    );
    let floor = Vec3::Y * 3.0 * draft::LIFT;
    draft.ring(floor, (LIFE_REACH - 0.06, LIFE_REACH), (1, 1.0), lit);
    draft
}

/// A life that can't be taken: the outline of the same, flat on the floor and in dashes.
fn withheld_drawing() -> Draft {
    let mut draft = Draft::default();
    let floor = Vec3::Y * 3.0 * draft::LIFT;
    draft.dashed_frame(floor, Vec2::splat(SIDE), draft::LINE, theme::TEXT_DIM);
    draft.dashed_ring(
        floor,
        (LIFE_REACH - draft::LINE, LIFE_REACH),
        theme::TEXT_DIM,
    );
    draft
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut inside = Draft::default();
    inside.disc(Vec3::Y * 2.0 * draft::LIFT, LIFE_REACH, Color::WHITE);
    commands.insert_resource(Drawings {
        offered: meshes.add(offered_drawing().build()),
        withheld: meshes.add(withheld_drawing().build()),
        inside: meshes.add(inside.build()),
        light: scene::paint(&mut materials, theme::PLAYER.with_alpha(0.1)),
    });
}

fn dress(
    mut commands: Commands,
    lives: Query<(Entity, &Position), Added<Life>>,
    drawings: Res<Drawings>,
    paints: Res<Paints>,
) {
    for (entity, position) in &lives {
        let part = |offered: bool, mesh: &Handle<Mesh>, paint: &Handle<StandardMaterial>| {
            (
                Offered(offered),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(paint.clone()),
                Visibility::Hidden,
            )
        };
        commands.entity(entity).insert((
            Transform::from_translation(scene::to_world(position.0, 0.0)),
            Visibility::default(),
            children![
                part(true, &drawings.offered, &paints.ink),
                part(true, &drawings.inside, &drawings.light),
                part(false, &drawings.withheld, &paints.ink),
            ],
        ));
    }
}

/// Whether a life on the floor is one that the player can take: nobody has more lives than a
/// run starts with.
fn within_reach(run: &Run) -> bool {
    run.lives < SOLO_LIVES
}

/// Shows the drawing that tells whether the life can be taken.
fn offer(run: Option<Res<Run>>, mut parts: Query<(&Offered, &mut Visibility)>) {
    let Some(run) = run else {
        return;
    };
    for (offered, mut visibility) in &mut parts {
        let wanted = if offered.0 == within_reach(&run) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

/// What the caption of a life says, and in which color.
fn words(run: &Run) -> (&'static str, Color) {
    if within_reach(run) {
        ("+1 LIFE", theme::PLAYER)
    } else {
        ("LIFE \u{b7} YOU HAVE ALL 3", theme::TEXT_DIM)
    }
}

fn build(mut commands: Commands) {
    commands.spawn((
        Caption,
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
        children![(
            CaptionText,
            theme::label("", 12.0, theme::PLAYER),
            // Its point has no width for a line to fit in.
            TextLayout::no_wrap(),
            Node {
                position_type: PositionType::Absolute,
                padding: UiRect::axes(px(5), px(1)),
                ..default()
            },
            BackgroundColor(theme::VOID.with_alpha(0.8)),
            // Held by the middle of its lower side, over the circle.
            UiTransform::from_translation(Val2::new(percent(-50), percent(-100))),
        )],
    ));
}

/// The point of the screen just above the circle of a life, whichever way the view is turned.
fn above(life: Vec2, screen: &OnScreen) -> Option<Vec2> {
    let middle = screen.point(scene::to_world(life, 0.0))?;
    let mut top = middle.y;
    for side in [Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y] {
        let rim = screen.point(scene::to_world(life + side * LIFE_REACH, 0.0))?;
        top = top.min(rim.y);
    }
    Some(Vec2::new(middle.x, top - CAPTION_GAP))
}

/// Holds the caption over the life that lies on the floor. There is one at most: a life is
/// lost with the arena that left it.
fn caption(
    screen: OnScreen,
    run: Option<Res<Run>>,
    lives: Query<&Position, With<Life>>,
    mut anchor: Single<(&mut UiTransform, &mut Visibility), With<Caption>>,
    mut text: Single<(&mut Text, &mut TextColor), With<CaptionText>>,
) {
    let at = lives.iter().next().and_then(|life| above(life.0, &screen));
    let (transform, visibility) = &mut *anchor;
    let (Some(at), Some(run)) = (at, run) else {
        if **visibility != Visibility::Hidden {
            **visibility = Visibility::Hidden;
        }
        return;
    };
    let place = Val2::px(at.x.round(), at.y.round());
    if transform.translation != place {
        transform.translation = place;
    }
    if **visibility != Visibility::Inherited {
        **visibility = Visibility::Inherited;
    }
    let (wanted, color) = words(&run);
    if text.0.0 != wanted {
        text.0.0 = wanted.to_string();
    }
    if text.1.0 != color {
        text.1.0 = color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_life_is_offered_only_to_a_player_who_is_short_of_one() {
        let mut run = Run {
            seed_text: String::new(),
            seed: 0,
            difficulty: default(),
            players: 1,
            team: 1,
            solo: true,
            sector: 8,
            cleared: 8,
            checkpoint: 8,
            lives: SOLO_LIVES,
            kills: 0,
            ticks: 0,
            phase: tanku_core::sim::RunPhase::Playing,
            lost_to: None,
            playtest: false,
            earns: true,
            free: false,
            arena: false,
            lull: None,
        };
        assert!(!within_reach(&run));
        assert_eq!(words(&run).0, "LIFE \u{b7} YOU HAVE ALL 3");
        run.lives = SOLO_LIVES - 1;
        assert!(within_reach(&run));
        assert_eq!(words(&run).0, "+1 LIFE");
    }

    #[test]
    fn the_two_drawings_of_a_life_differ_and_fit_its_circle() {
        let (offered, withheld) = (offered_drawing(), withheld_drawing());
        for drawing in [&offered, &withheld] {
            let (low, high) = drawing.bounds();
            assert!((high.x - LIFE_REACH).abs() < 0.01 && (low.x + LIFE_REACH).abs() < 0.01);
        }
        // One stands, in the color of the player. The other lies flat, in dashes.
        assert!(offered.bounds().1.y > HEIGHT);
        assert!(withheld.bounds().1.y < 0.1);
        assert!(offered.area(theme::PLAYER) > 0.5);
        assert_eq!(withheld.area(theme::PLAYER), 0.0);
    }
}
