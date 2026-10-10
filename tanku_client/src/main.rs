mod audio;
mod camera;
mod cli;
#[cfg(feature = "development")]
mod dev;
mod draft;
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
use tanku_core::{CorePlugin, sim::Playtest};
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
    let options = cli::Options::from_command_line();
    let mut app = App::new();
    #[cfg(feature = "development")]
    let scripted = dev::scripted();
    #[cfg(not(feature = "development"))]
    let scripted = false;
    if scripted {
        #[cfg(feature = "development")]
        dev::add_engine(&mut app);
    } else {
        let settings = settings::Settings::load();
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Tanku".into(),
                fit_canvas_to_parent: true,
                // Set from the start, so that the window doesn't open in one mode and jump
                // to the other.
                mode: settings.window_mode(),
                present_mode: settings.present_mode(),
                ..default()
            }),
            ..default()
        }));
    }
    // The client embeds the server for solo and hosted games.
    app.add_plugins((CorePlugin, ServerPlugin))
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
    if options.playtest {
        // The lives count again once the pause menu says so.
        app.insert_resource(Playtest {
            infinite_lives: true,
        });
    }
    app.insert_resource(options);
    #[cfg(feature = "development")]
    app.add_plugins(dev::plugin);
    app.run();
}
