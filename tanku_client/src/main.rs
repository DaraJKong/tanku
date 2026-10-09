use bevy::prelude::*;
use tanku_core::CorePlugin;
use tanku_server::ServerPlugin;

fn main() {
    App::new()
        // The client embeds the server for solo and hosted games.
        .add_plugins((DefaultPlugins, CorePlugin, ServerPlugin))
        .run();
}
