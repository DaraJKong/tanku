//! The authoritative Tanku server.
//!
//! The client embeds this library for solo and hosted games, and the `tanku_server` binary runs
//! it headless.

mod arena;
pub mod brain;
pub mod bullets;
pub mod field;
mod mines;
#[cfg(feature = "net")]
pub mod net;
pub mod pilot;
mod repair;
pub mod room;
mod run;
mod sectors;
pub mod tanks;
pub mod teammates;
#[cfg(test)]
mod tests;

use bevy::prelude::*;
use tanku_core::{
    sim::{Blow, Playtest, Run, RunPhase, Team},
    stats::Genome,
};

/// The tank that fired a bullet or laid a mine, as it was built. The tank may be gone when
/// its weapon strikes, and what it destroys must still be told whose doing it was.
#[derive(Component, Clone, Copy, Debug)]
pub struct Maker(pub Genome);

/// What was hit this tick and still has to be removed.
#[derive(Resource, Default)]
pub struct Casualties {
    /// Destroyed tanks.
    pub tanks: Vec<Entity>,
    /// What destroyed some of them, when it is known whose doing it was.
    pub blows: Vec<(Entity, Blow)>,
    /// Mines and crates that were set off.
    pub detonations: Vec<Entity>,
    /// The tanks whose bullets set off some of the crates.
    pub culprits: Vec<(Entity, (Genome, Team))>,
    /// Where exploding bullets ended and go off, each with the tank that fired it when that
    /// tank is known.
    pub blasts: Vec<(Vec2, Option<(Genome, Team)>)>,
}

/// Everything the simulation does in a tick. A game that shows a run simulated on another
/// machine keeps this from running.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Simulation;

/// The authoritative simulation. Needs `tanku_core::CorePlugin`.
pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        // Nothing moves while the host of a game holds it.
        let in_progress = |run: Option<Res<Run>>| {
            run.is_some_and(|run| run.phase != RunPhase::Over && !run.paused)
        };
        // A run in arena mode never leaves its room: it has no sector to cross into.
        let in_a_room = |run: Option<Res<Run>>| run.is_some_and(|run| run.arena);
        app.init_resource::<Casualties>().add_systems(
            FixedUpdate,
            (
                run::handle_requests,
                (
                    run::skip.run_if(resource_exists::<Playtest>),
                    run::leave,
                    field::rebuild_colliders,
                    tanks::idle,
                    brain::think,
                    teammates::drive.run_if(teammates::any),
                    repair::repair,
                    tanks::shields,
                    tanks::drive,
                    tanks::separate,
                    tanks::operate,
                    bullets::fly,
                    bullets::collide,
                    mines::tick,
                    mines::blast,
                    run::casualties,
                    (
                        sectors::advance.run_if(not(in_a_room)),
                        room::advance.run_if(in_a_room),
                        arena::waves,
                        (room::raise, room::reserve).chain().run_if(in_a_room),
                    )
                        .chain(),
                    field::unload,
                    run::clock,
                )
                    .chain()
                    .run_if(in_progress),
            )
                .chain()
                .in_set(Simulation),
        );
    }
}
