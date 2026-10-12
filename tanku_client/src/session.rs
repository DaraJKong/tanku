//! A game that several players share, as the menus see it: whether this machine hosts it or
//! joined it, and what the player asks about it. The network answers in `net`, and in `host`
//! on a machine that can host.

use std::net::Ipv4Addr;

use bevy::prelude::*;
use tanku_core::{discovery::Standing, modes::Difficulty};
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

/// What a host gives a game before it opens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sign {
    /// The name that the game goes by, or nothing.
    pub name: String,
    /// The password that those who join must type, or nothing.
    pub password: String,
    /// What its first run is set to, which the host changes in the lobby: how hard it is,
    /// and whether it is in free mode and in arena mode.
    pub difficulty: Difficulty,
    pub free: bool,
    pub arena: bool,
}

/// What the menus ask about a game of several players.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum SessionRequest {
    /// Open a game on this machine, as its host set it up, and wait in its lobby.
    Host(Sign),
    /// Join the game at an address, as the player typed it.
    Join(String),
    /// Join it with the password that the player typed for it.
    JoinLocked(String, String),
    /// Leave the game. A host closes it for everyone.
    Leave,
    /// The player at this screen is ready for the run of the lobby, or no longer is.
    Ready(bool),
    /// The host starts a run as the lobby is set, with everyone who is there. A run starts
    /// by itself once everyone is ready: this starts the one that a host picks after a run.
    Start,
}

/// Whether the game drew the seed of a host's lobby and no run was played on it yet. A run
/// on such a seed earns achievements: one that is played again can be practiced.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Drawn(pub bool);

/// What the player is told about a game of several players: why it could not be joined, or
/// how it ended.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Notice(pub Option<String>);

/// A game that has a password, which the player must type to join it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shut {
    /// Where it is joined, as it was asked for: an address, or what its host handed out.
    pub at: String,
    /// How the player is told of it: by its name if they found it under one, and by where
    /// it is.
    pub name: String,
    pub address: String,
}

/// The game that asks the player for its password, if one does: the join screen then takes
/// the password instead of an address.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Asked(pub Option<Shut>);

/// What a host tells those who want to join.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Invitation {
    /// The address of this machine on its network, as a guest types it, when it has one.
    pub address: Option<String>,
    /// The port that the game listens on.
    pub port: u16,
    /// What it hands out to players in a browser, if it takes them: where it listens for
    /// browsers, and the print of the certificate that it shows them.
    pub web: Option<String>,
    /// The port on which it listens for browsers, or would if nothing else had taken it.
    pub web_port: Option<u16>,
}

/// A game that was found on the local network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    /// The number that its host gave it.
    pub id: u64,
    /// The name that its host gave it, or nothing.
    pub name: String,
    /// Whether it has a password.
    pub locked: bool,
    /// The machine that its word came from.
    pub machine: Ipv4Addr,
    /// Where it is joined, as a player would type it.
    pub address: String,
    pub players: u8,
    /// What the player can do about it.
    pub standing: Standing,
}

/// The games that the join screen found on the local network. A browser finds none.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Found(pub Vec<Game>);

pub fn plugin(app: &mut App) {
    app.init_resource::<Session>()
        .init_resource::<Notice>()
        .init_resource::<Drawn>()
        .init_resource::<Invitation>()
        .init_resource::<Asked>()
        .init_resource::<Found>()
        .add_message::<SessionRequest>()
        // A guest shows the run of its host and simulates nothing.
        .configure_sets(
            FixedUpdate,
            Simulation.run_if(|session: Res<Session>| *session != Session::Guest),
        );
}
