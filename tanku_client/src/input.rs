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

pub fn command_player(
    settings: Res<Settings>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    screen: Res<State<Screen>>,
    overlay: Res<State<Overlay>>,
    mut view: ResMut<View>,
    mut player: Query<&mut TankCommand, With<Player>>,
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
        fire: held(Action::Shoot),
        mine: held(Action::Mine),
        shield: held(Action::Shield),
    };
}
