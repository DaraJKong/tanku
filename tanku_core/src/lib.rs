//! Code shared by the Tanku client and server.

use bevy::prelude::*;

/// Everything the client and the server have in common.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, _app: &mut App) {}
}
