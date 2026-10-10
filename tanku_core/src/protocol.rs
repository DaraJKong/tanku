//! What the server of a game and its clients tell each other.
//!
//! The server sends the lobby, the run and the state of the simulation: every component listed
//! here goes to the clients as it changes. A client sends what its player asks of their tank,
//! tick by tick.
//!
//! What moves is told apart from what is shown. A client doesn't show a tank or a bullet where
//! the server last said it was: it shows the other tanks a moment late, between two tellings,
//! and its own tank and the bullets where they are by now. So the place and the motion of
//! those travel as [`Told`], and each side keeps its own `Position`, `Motion` and `Bullet`.

use bevy::{ecs::entity::MapEntities, prelude::*};
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    rules::MAX_PLAYERS,
    sim::{
        Barrier, Block, Bullet, Enemy, ExplosiveCrate, Flag, Gun, Mine, Motion, Player, Position,
        Protected, Run, SectorIndex, Shield, SimEvent, Tank, TankCommand, Wreck,
    },
};

/// Counts the changes to what the server and its clients tell each other since the version of
/// the game was last raised. Two builds that don't count the same don't speak the same, so
/// they don't join each other: raise this with every such change.
pub const REVISION: u32 = 2;

/// Who sits in a seat of a game that several players share.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Member {
    /// The player whose machine runs the game.
    Host,
    /// A player who joined, by the number that their machine goes by on the network.
    Guest(u64),
}

/// The lobby of a game that several players share: who is there, and the seed of their next
/// run. The host decides, and the guests are told.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lobby {
    /// Who sits where. A seat is the number of a player in a run.
    pub seats: [Option<Member>; MAX_PLAYERS as usize],
    pub seed: String,
}

impl Lobby {
    /// How many players are there.
    pub fn players(&self) -> u32 {
        self.seats.iter().flatten().count() as u32
    }

    /// The seat of a member, if they have one.
    pub fn seat_of(&self, member: Member) -> Option<u8> {
        let seat = self.seats.iter().position(|taken| *taken == Some(member));
        seat.map(|seat| seat as u8)
    }
}

/// What the server tells of a part of a tank or of a bullet: where it is, and how it moves.
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Told<T: Send + Sync + 'static>(pub T);

/// The run, as the server tells it to the clients.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct RunState(pub Run);

/// The channel of what must arrive, in the order it was sent.
pub struct Reliable;

// A command names no entity.
impl MapEntities for TankCommand {
    fn map_entities<M: EntityMapper>(&mut self, _: &mut M) {}
}

/// Registers what goes over the network. It comes after the plugins of the network library,
/// and is the same on both sides.
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(input::native::InputPlugin::<TankCommand>::default());

        app.register_message::<SimEvent>()
            .add_direction(NetworkDirection::ServerToClient);
        app.add_channel::<Reliable>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);

        app.component::<Lobby>().replicate();
        app.component::<RunState>().replicate();
        app.component::<SectorIndex>().replicate();
        // What stays where it is has its place told as it is.
        app.component::<Position>()
            .replicate_filtered::<(Without<Tank>, Without<Bullet>)>();
        app.component::<Told<Position>>().replicate();
        app.component::<Told<Motion>>().replicate();
        app.component::<Told<Bullet>>().replicate();
        app.component::<Tank>().replicate();
        app.component::<Player>().replicate();
        app.component::<Enemy>().replicate();
        app.component::<Gun>().replicate();
        app.component::<Shield>().replicate();
        app.component::<Wreck>().replicate();
        app.component::<Protected>().replicate();
        app.component::<Mine>().replicate();
        app.component::<Block>().replicate();
        app.component::<ExplosiveCrate>().replicate();
        app.component::<Barrier>().replicate();
        app.component::<Flag>().replicate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lobby_tells_who_sits_where() {
        let mut lobby = Lobby::default();
        assert_eq!(lobby.players(), 0);
        lobby.seats[0] = Some(Member::Host);
        lobby.seats[2] = Some(Member::Guest(7));
        assert_eq!(lobby.players(), 2);
        assert_eq!(lobby.seat_of(Member::Host), Some(0));
        assert_eq!(lobby.seat_of(Member::Guest(7)), Some(2));
        assert_eq!(lobby.seat_of(Member::Guest(8)), None);
    }
}
