//! A game that several players share, as the menus see it: whether this machine hosts it or
//! joined it, and what the player asks about it. The network answers in `net`, which a browser
//! doesn't have yet.

use bevy::prelude::*;
use tanku_server::Simulation;

/// How the game at this screen is played.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Session {
    /// Alone: nothing goes over the network.
    #[default]
    Solo,
    /// This machine runs a game that others join.
    Host,
    /// This machine joined a game on another one, or is trying to.
    Guest,
}

/// What the menus ask about a game of several players.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum SessionRequest {
    /// Open a game on this machine and wait in its lobby.
    Host,
    /// Join the game at an address, as the player typed it.
    Join(String),
    /// Leave the game. A host closes it for everyone.
    Leave,
    /// The host starts a run on the seed of the lobby, with everyone who is there.
    Start,
}

/// What the player is told about a game of several players: why it could not be joined, or
/// how it ended.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Notice(pub Option<String>);

/// What a host tells those who want to join.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Invitation {
    /// The address of this machine on its network, as a guest types it, when it has one.
    pub address: Option<String>,
    /// The port that the game listens on.
    pub port: u16,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Session>()
        .init_resource::<Notice>()
        .init_resource::<Invitation>()
        .add_message::<SessionRequest>()
        // A guest shows the run of its host and simulates nothing.
        .configure_sets(
            FixedUpdate,
            Simulation.run_if(|session: Res<Session>| *session != Session::Guest),
        );
}
