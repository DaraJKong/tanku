//! The server's side of the network, for games of several players.
//!
//! The server listens for clients, gives each one a seat, sends them the simulation as it
//! changes, and takes what their players ask of their tanks. It needs the server plugins of the
//! network library and the protocol of `tanku_core`, added in that order.

use std::net::{Ipv4Addr, SocketAddr};

use bevy::prelude::*;
use lightyear::{
    netcode::NetcodeServer,
    prelude::{input::native::ActionState, server::*, *},
};
use tanku_core::{
    protocol::{Reliable, RunState},
    seed::VERSION,
    sim::{Player, Run, RunRequest, SimEvent, TankCommand},
};

use crate::field::InSector;

/// The port a game listens on unless it is told otherwise.
pub const PORT: u16 = 28405;

/// The seat of the player behind a connection.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat(pub u8);

/// The entity that carries the run to the clients.
#[derive(Component)]
struct Herald;

/// What tells one version of the game from another on the network: players must run the same.
pub fn protocol_id() -> u64 {
    VERSION.bytes().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01B3)
    })
}

/// Starts to listen for clients on a port.
pub fn open(commands: &mut Commands, port: u16) -> Entity {
    let address = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);
    let server = commands
        .spawn((
            Name::new("Server"),
            Server::default(),
            NetcodeServer::new(NetcodeConfig {
                protocol_id: protocol_id(),
                ..default()
            }),
            LocalAddr(address),
            ServerUdpIo::default(),
        ))
        .id();
    commands.trigger(Start { entity: server });
    server
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(greet)
            .add_observer(seat)
            .add_systems(FixedPreUpdate, take_commands)
            .add_systems(
                PostUpdate,
                (share, herald, tell).run_if(any_with_component::<Server>),
            )
            .add_systems(
                PreUpdate,
                hear.after(MessageSystems::Receive)
                    .run_if(any_with_component::<Server>),
            );
    }
}

/// A connection that opens can be sent the simulation.
fn greet(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert(ReplicationSender);
}

/// A client that is connected takes the first free seat.
fn seat(
    trigger: On<Add, Connected>,
    clients: Query<(), With<ClientOf>>,
    seats: Query<&Seat>,
    mut commands: Commands,
) {
    if !clients.contains(trigger.entity) {
        return;
    }
    let taken: Vec<u8> = seats.iter().map(|seat| seat.0).collect();
    if let Some(free) = (0..4).find(|seat| !taken.contains(seat)) {
        commands.entity(trigger.entity).insert(Seat(free));
    }
}

/// Sends the clients everything the simulation owns. The tank of a player is theirs to
/// command.
fn share(
    mut commands: Commands,
    fresh: Query<
        (Entity, Option<&Player>),
        (Or<(With<InSector>, With<Player>)>, Without<Replicate>),
    >,
    seats: Query<(Entity, &Seat)>,
) {
    for (entity, player) in &fresh {
        let mut entity = commands.entity(entity);
        entity.insert(Replicate::to_clients(NetworkTarget::All));
        let owner = player.and_then(|player| seats.iter().find(|(_, seat)| seat.0 == player.seat));
        if let Some((owner, _)) = owner {
            entity.insert(ControlledBy {
                owner,
                lifetime: Default::default(),
            });
        }
    }
}

/// Keeps the clients told of the run.
fn herald(
    mut commands: Commands,
    run: Option<Res<Run>>,
    mut heralds: Query<(Entity, &mut RunState), With<Herald>>,
) {
    match (run, heralds.single_mut()) {
        (Some(run), Ok((_, mut state))) => {
            if run.is_changed() {
                state.0 = run.clone();
            }
        }
        (Some(run), Err(_)) => {
            commands.spawn((
                Herald,
                RunState(run.clone()),
                Replicate::to_clients(NetworkTarget::All),
            ));
        }
        (None, Ok((herald, _))) => commands.entity(herald).despawn(),
        (None, Err(_)) => {}
    }
}

/// Tells the clients what happens in the simulation, for their effects and their sounds.
fn tell(
    mut events: MessageReader<SimEvent>,
    mut sender: ServerMultiMessageSender,
    server: Single<&Server>,
) {
    for event in events.read() {
        let _ = sender.send::<_, Reliable>(event, *server, &NetworkTarget::All);
    }
}

/// Takes the requests of the clients about the run. A client decides for its own tank only:
/// it can leave the run. The rest is up to whoever runs the server.
fn hear(
    mut receivers: Query<(&Seat, &mut MessageReceiver<RunRequest>), With<ClientOf>>,
    mut requests: MessageWriter<RunRequest>,
) {
    for (seat, mut receiver) in &mut receivers {
        for request in receiver.receive() {
            if matches!(request, RunRequest::Leave { .. }) {
                requests.write(RunRequest::Leave { seat: seat.0 });
            }
        }
    }
}

/// What the players ask of their tanks this tick, as their clients sent it.
fn take_commands(mut tanks: Query<(&ActionState<TankCommand>, &mut TankCommand), With<Player>>) {
    for (asked, mut command) in &mut tanks {
        *command = asked.0;
    }
}
