// The engine's cache of pipelines, which `quit` reads, is made of types that are nested
// deeper than the compiler follows by default to tell that it may be shared between threads.
#![recursion_limit = "256"]

mod ahead;
mod audio;
mod balloons;
mod camera;
mod cli;
mod crew;
#[cfg(feature = "script")]
mod dev;
#[cfg(not(target_arch = "wasm32"))]
mod discovery;
mod draft;
mod drawing;
mod effects;
mod exhibit;
mod garage;
#[cfg(not(target_arch = "wasm32"))]
mod host;
mod hud;
mod input;
#[cfg(not(target_arch = "wasm32"))]
mod journal;
mod lettering;
mod life;
mod margin;
mod menu;
mod mirror;
mod models;
mod music;
mod net;
mod notices;
#[cfg(feature = "script")]
mod profile;
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
    ecs::schedule::SingleThreadedExecutor,
    prelude::*,
    render::RenderApp,
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

/// The most threads in the pool that a system of the engine shares its work out on.
const COMPUTE_THREADS: usize = 1;

/// The threads of the engine. A system can share its work out on the threads of a pool,
/// which has nearly every core of the machine unless it is told otherwise. The work of a
/// frame is small, and waking threads for its pieces costs more than they save: on a machine
/// of 24 threads, with every schedule on one thread (see `OneThread`), the main schedule of a
/// fight of four players took 2.3 ms a frame with eight threads in the pool, 2.1 ms with
/// four and 1.7 ms with one (measured 2026-10-14).
pub fn task_pools() -> TaskPoolPlugin {
    let mut options = TaskPoolOptions::default();
    options.compute.max_threads = COMPUTE_THREADS;
    TaskPoolPlugin {
        task_pool_options: options,
    }
}

/// Runs the systems of every schedule one after another, on the thread that runs the
/// schedule, as a browser does, which gives the game one thread. The engine otherwise hands
/// them out to the threads of its pool, and the systems of a frame are too many and too small
/// for that to pay: in a fight of four players on a machine of 24 threads, the main schedule
/// took 3.8 ms a frame at 60 frames a second, and 2.1 ms on one thread. With the schedules of
/// the drawing, which has its own thread, run the same way too, the game drew 45% more frames
/// when nothing held it back (measured 2026-10-14).
struct OneThread;

impl Plugin for OneThread {
    fn build(&self, _app: &mut App) {}

    /// Once every plugin has made its schedules, and before the drawing takes its own away to
    /// its thread.
    fn finish(&self, app: &mut App) {
        one_thread(app.world_mut());
        if let Some(drawing) = app.get_sub_app_mut(RenderApp) {
            one_thread(drawing.world_mut());
        }
    }
}

fn one_thread(world: &mut World) {
    for (_, schedule) in world.resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
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
        .add_plugins((audio::plugin, music::plugin, notices::plugin))
        .add_plugins((margin::plugin, balloons::plugin));
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
    // Last, so that it finishes after every other plugin.
    app.add_plugins(OneThread);
    app.run();
}
