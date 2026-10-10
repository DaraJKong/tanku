//! What the server of a game and its clients tell each other.
//!
//! The server sends the state of the simulation: every component listed here goes to the
//! clients as it changes. A client sends what its player asks of their tank, tick by tick,
//! and requests about the run.

use bevy::{ecs::entity::MapEntities, prelude::*};
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

use crate::sim::{
    Barrier, Block, Bullet, Enemy, ExplosiveCrate, Flag, Gun, Mine, Motion, Player, Position,
    Protected, Run, RunRequest, SectorIndex, Shield, SimEvent, Tank, TankCommand, Wreck,
};

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
        app.register_message::<RunRequest>()
            .add_direction(NetworkDirection::ClientToServer);
        app.add_channel::<Reliable>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);

        app.component::<RunState>().replicate();
        app.component::<SectorIndex>().replicate();
        app.component::<Position>().replicate();
        app.component::<Motion>().replicate();
        app.component::<Tank>().replicate();
        app.component::<Player>().replicate();
        app.component::<Enemy>().replicate();
        app.component::<Gun>().replicate();
        app.component::<Shield>().replicate();
        app.component::<Wreck>().replicate();
        app.component::<Protected>().replicate();
        app.component::<Bullet>().replicate();
        app.component::<Mine>().replicate();
        app.component::<Block>().replicate();
        app.component::<ExplosiveCrate>().replicate();
        app.component::<Barrier>().replicate();
        app.component::<Flag>().replicate();
    }
}
