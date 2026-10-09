mod audio;
mod camera;
#[cfg(feature = "development")]
mod dev;
mod effects;
mod hud;
mod input;
mod menu;
mod models;
mod scene;
mod settings;
mod storage;
mod theme;

use bevy::prelude::*;
use tanku_core::CorePlugin;
use tanku_server::ServerPlugin;

/// What the player is looking at.
#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Menu,
    Playing,
    GameOver,
}

/// A menu drawn over the screen. In solo, the game is paused while one is open.
#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Overlay {
    #[default]
    None,
    Pause,
    Settings,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Tanku".into(),
            fit_canvas_to_parent: true,
            ..default()
        }),
        ..default()
    }))
    // The client embeds the server for solo and hosted games.
    .add_plugins((CorePlugin, ServerPlugin))
    .init_state::<Screen>()
    .init_state::<Overlay>()
    .add_plugins((
        settings::plugin,
        input::plugin,
        camera::plugin,
        scene::plugin,
        models::plugin,
        effects::plugin,
        hud::plugin,
        menu::plugin,
        audio::plugin,
    ));
    #[cfg(feature = "development")]
    app.add_plugins(dev::plugin);
    app.run();
}
