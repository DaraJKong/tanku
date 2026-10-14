// The engine's cache of pipelines, which `quit` reads, is made of types that are nested
// deeper than the compiler follows by default to tell that it may be shared between threads.
#![recursion_limit = "256"]

mod ahead;
mod audio;
mod camera;
mod cli;
mod crew;
#[cfg(feature = "script")]
mod dev;
#[cfg(not(target_arch = "wasm32"))]
mod discovery;
mod draft;
mod effects;
mod exhibit;
mod garage;
#[cfg(not(target_arch = "wasm32"))]
mod host;
mod hud;
mod input;
#[cfg(not(target_arch = "wasm32"))]
mod journal;
mod life;
mod menu;
mod mirror;
mod models;
mod music;
mod net;
mod notices;
#[cfg(not(target_arch = "wasm32"))]
mod quit;
mod repair;
mod report;
mod ruler;
mod scars;
mod scene;
mod session;
mod settings;
mod sheet;
mod storage;
mod tags;
mod theme;
mod unlocks;
mod wake;

use bevy::{
    app::{TaskPoolOptions, TaskPoolPlugin},
    prelude::*,
};
use tanku_core::{CorePlugin, sim::Playtest};
use tanku_server::ServerPlugin;

/// What the player is looking at.
#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Menu,
    /// Where a single player sets up a run.
    Solo,
    /// Where a player builds their tank for free mode.
    Garage,
    /// Where the player sees what runs unlock, and what theirs have.
    Achievements,
    /// Where a player sets up the game that they host for others.
    Host,
    /// Where a player types the address of a game to join.
    Join,
    /// Where the players of a game wait for its host to start a run.
    Lobby,
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

/// The most threads that the systems of the engine run on side by side.
const COMPUTE_THREADS: usize = 4;

/// The threads of the engine. It runs its systems side by side on the threads of a pool,
/// which has nearly every core of the machine unless it is told otherwise. The work of a
/// frame is small, and waking many threads for its pieces costs more than they save: on a
/// machine of 24 cores, the main schedule of a fight of four players took 6.0 ms a frame on
/// the 16 threads that the engine took, and 4.0 ms on four.
pub fn task_pools() -> TaskPoolPlugin {
    let mut options = TaskPoolOptions::default();
    options.compute.max_threads = COMPUTE_THREADS;
    TaskPoolPlugin {
        task_pool_options: options,
    }
}

fn main() {
    let options = cli::Options::from_command_line();
    let mut app = App::new();
    #[cfg(feature = "script")]
    let scripted = dev::scripted();
    #[cfg(not(feature = "script"))]
    let scripted = false;
    if scripted {
        #[cfg(feature = "script")]
        dev::add_engine(&mut app);
    } else {
        let settings = settings::Settings::load();
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Tanku".into(),
                        fit_canvas_to_parent: true,
                        // Set from the start, so that the window doesn't open in one mode and
                        // jump to the other.
                        mode: settings.window_mode(),
                        present_mode: settings.present_mode(),
                        ..default()
                    }),
                    ..default()
                })
                .set(task_pools()),
        );
    }
    // The client embeds the server for solo and hosted games.
    app.add_plugins((CorePlugin, ServerPlugin))
        .init_state::<Screen>()
        .init_state::<Overlay>()
        .add_plugins((crew::plugin, session::plugin, life::plugin))
        .add_plugins((
            settings::plugin,
            sheet::plugin,
            unlocks::plugin,
            garage::plugin,
        ))
        .add_plugins((
            theme::plugin,
            input::plugin,
            camera::plugin,
            scene::plugin,
            models::plugin,
            effects::plugin,
            ahead::plugin,
            scars::plugin,
            exhibit::plugin,
            hud::plugin,
            tags::plugin,
            ruler::plugin,
            menu::plugin,
            report::plugin,
            repair::plugin,
        ))
        .add_plugins((audio::plugin, music::plugin, notices::plugin));
    // A browser joins games and hosts none.
    app.add_plugins(net::NetPlugin);
    // A page keeps no journal: nothing outside the browser could read it. And a page is
    // closed by its browser, which waits for nothing.
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins((journal::plugin, quit::plugin));
    app.init_resource::<wake::Hold>();
    if options.playtest {
        // The lives count again once the pause menu says so.
        app.insert_resource(Playtest {
            infinite_lives: true,
            fake_achievements: false,
        });
    }
    app.insert_resource(options);
    #[cfg(feature = "script")]
    app.add_plugins(dev::plugin);
    app.run();
}
