//! The authoritative Tanku server.
//!
//! The client embeds this library for solo and hosted games, and the `tanku_server` binary runs
//! it headless.

use bevy::prelude::*;

/// The authoritative simulation. Needs `tanku_core::CorePlugin`.
pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, _app: &mut App) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use tanku_core::CorePlugin;

    #[test]
    fn headless_app_ticks() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, CorePlugin, ServerPlugin));
        app.update();
    }
}
