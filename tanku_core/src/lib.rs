//! Code shared by the Tanku client and server.

pub mod enemy;
pub mod geom;
pub mod map;
pub mod progression;
pub mod rng;
pub mod rules;
pub mod seed;
pub mod sim;
pub mod stats;

use bevy::prelude::*;

/// Everything the client and the server have in common.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(rules::TICK_RATE))
            .add_message::<sim::RunRequest>()
            .add_message::<sim::SimEvent>();
    }
}
