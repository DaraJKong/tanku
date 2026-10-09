use std::time::Duration;

use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*};
use tanku_core::CorePlugin;
use tanku_server::ServerPlugin;

const TICKS_PER_SECOND: f64 = 60.0;

fn main() {
    App::new()
        .add_plugins((
            // `MinimalPlugins` stays headless even when a workspace build unifies Bevy's features
            // with the client's.
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / TICKS_PER_SECOND,
            ))),
            LogPlugin::default(),
            CorePlugin,
            ServerPlugin,
        ))
        .run();
}
