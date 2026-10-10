//! Turns keys and buttons into commands for the player's tank.

use bevy::{input::InputSystems, prelude::*};
use tanku_core::sim::{Player, TankCommand};

use crate::{
    Overlay, Screen,
    camera::{self, View},
    settings::{Action, Settings},
};

pub fn plugin(app: &mut App) {
    app.add_systems(
        PreUpdate,
        (camera::aim, command_player).chain().after(InputSystems),
    );
}

/// Whether an action goes through. One that is held when a menu closes waits until it is let
/// go: the click on "Solo" or on "Resume" must not fire the gun.
fn passes(held: bool, let_go: &mut bool) -> bool {
    *let_go |= !held;
    held && *let_go
}

pub fn command_player(
    settings: Res<Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    screen: Res<State<Screen>>,
    overlay: Res<State<Overlay>>,
    mut view: ResMut<View>,
    mut player: Query<&mut TankCommand, With<Player>>,
    // For shooting, placing a mine and shielding: whether it was let go since the last menu.
    mut let_go: Local<[bool; 3]>,
) {
    let playing = *screen.get() == Screen::Playing && *overlay.get() == Overlay::None;
    if playing {
        if settings.just_pressed(Action::RotateLeft, &keys, &mouse) {
            view.turns += 1;
        }
        if settings.just_pressed(Action::RotateRight, &keys, &mouse) {
            view.turns -= 1;
        }
    }
    if !playing {
        *let_go = [false; 3];
    }
    let Ok(mut command) = player.single_mut() else {
        return;
    };
    if !playing {
        *command = TankCommand {
            aim: command.aim,
            ..default()
        };
        return;
    }
    let held = |action| settings.held(action, &keys, &mouse);
    let axis = |positive, negative| held(positive) as i32 as f32 - held(negative) as i32 as f32;
    let screen_direction = Vec2::new(
        axis(Action::Right, Action::Left),
        axis(Action::Up, Action::Down),
    );
    *command = TankCommand {
        // The movement keys follow the screen, whichever way the view is turned.
        drive: view.to_ground(screen_direction).normalize_or_zero(),
        aim: view.aim,
        fire: passes(held(Action::Shoot), &mut let_go[0]),
        mine: passes(held(Action::Mine), &mut let_go[1]),
        shield: passes(held(Action::Shield), &mut let_go[2]),
        repair: false,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_button_held_since_a_menu_waits_until_it_is_let_go() {
        let mut let_go = false;
        // The click on the menu is still down when the game starts.
        assert!(!passes(true, &mut let_go));
        assert!(!passes(true, &mut let_go));
        assert!(!passes(false, &mut let_go));
        // A new press shoots, and holding it keeps shooting.
        assert!(passes(true, &mut let_go));
        assert!(passes(true, &mut let_go));
    }
}
