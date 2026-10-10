//! The client's side of the network, for games of several players.
//!
//! A client that joins a game receives the simulation from its server, and sends what its
//! player asks of their tank. It needs the client plugins of the network library and the
//! protocol of `tanku_core`, added in that order.
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "nothing joins a game yet but the test below: the lobby comes next"
    )
)]

use std::net::{Ipv4Addr, SocketAddr};

use bevy::prelude::*;
use lightyear::{
    netcode::{NetcodeClient, client_plugin::NetcodeConfig},
    prelude::{
        client::{input::InputSystems, *},
        input::native::{ActionState, InputMarker},
        *,
    },
};
use tanku_core::{
    map::Corridor,
    protocol::RunState,
    sim::{Player, Run, Sector, SectorIndex, SimEvent, TankCommand},
};
use tanku_server::net::protocol_id;

use crate::crew::LocalSeat;

/// Present while this client plays a run that another machine simulates.
#[derive(Resource)]
pub struct Joined;

/// Joins the game that a server runs at an address.
pub fn join(commands: &mut Commands, server: SocketAddr, id: u64) -> Result<Entity> {
    let auth = Authentication::Manual {
        server_addr: server,
        client_id: id,
        private_key: default(),
        protocol_id: protocol_id(),
    };
    let client = commands
        .spawn((
            Name::new("Client"),
            Client,
            ReplicationReceiver,
            Link::default(),
            LocalAddr(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0)),
            PeerAddr(server),
            NetcodeClient::new(auth, NetcodeConfig::default())?,
            UdpIo::default(),
        ))
        .id();
    commands.insert_resource(Joined);
    commands.trigger(Connect { entity: client });
    Ok(client)
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedPreUpdate,
            send_commands.in_set(InputSystems::WriteClientInputs),
        )
        .add_systems(
            PreUpdate,
            (take_the_controls, hear, follow_run, lay_out_sectors)
                .chain()
                .after(MessageSystems::Receive)
                .run_if(resource_exists::<Joined>),
        );
    }
}

/// The tank that the server says is this client's is the one its player commands.
fn take_the_controls(
    mut commands: Commands,
    tanks: Query<(Entity, &Player), (With<Controlled>, Without<InputMarker<TankCommand>>)>,
    mut seat: ResMut<LocalSeat>,
) {
    for (entity, player) in &tanks {
        commands
            .entity(entity)
            .insert(InputMarker::<TankCommand>::default());
        seat.0 = player.seat;
    }
}

/// Sends what the player asks of their tank this tick.
fn send_commands(
    mut tanks: Query<(&TankCommand, &mut ActionState<TankCommand>), With<InputMarker<TankCommand>>>,
) {
    for (command, mut asked) in &mut tanks {
        asked.0 = *command;
    }
}

/// What happens in the simulation of the server, for the effects and the sounds here.
fn hear(
    mut receivers: Query<&mut MessageReceiver<SimEvent>, With<Client>>,
    mut events: MessageWriter<SimEvent>,
) {
    for mut receiver in &mut receivers {
        for event in receiver.receive() {
            events.write(event);
        }
    }
}

/// The run of the server is the run that this client shows.
fn follow_run(mut commands: Commands, told: Query<&RunState>, run: Option<ResMut<Run>>) {
    match (told.single(), run) {
        (Ok(state), Some(mut run)) => *run = state.0.clone(),
        (Ok(state), None) => commands.insert_resource(state.0.clone()),
        (Err(_), Some(_)) => commands.remove_resource::<Run>(),
        (Err(_), None) => {}
    }
}

/// The server only tells which sectors are loaded. Their walls and all they hold come from the
/// seed, which gives the same on every machine.
fn lay_out_sectors(
    mut commands: Commands,
    run: Option<Res<Run>>,
    bare: Query<(Entity, &SectorIndex), Without<Sector>>,
    mut corridor: Local<Option<(u64, Corridor)>>,
) {
    let Some(run) = run else {
        return;
    };
    for (entity, index) in &bare {
        let known = corridor.take().filter(|(seed, _)| *seed == run.seed);
        let (seed, mut planned) = known.unwrap_or_else(|| (run.seed, Corridor::new(run.seed)));
        let layout = planned.layout(index.0);
        commands.entity(entity).insert(Sector { layout });
        *corridor = Some((seed, planned));
    }
}

#[cfg(test)]
mod tests {
    use std::{net::UdpSocket, time::Duration};

    use bevy::state::app::StatesPlugin;
    use lightyear::prelude::server::ServerPlugins;
    use tanku_core::{
        CorePlugin,
        protocol::ProtocolPlugin,
        rules::TICK_RATE,
        sim::{Position, RunRequest, Tank},
    };
    use tanku_server::{ServerPlugin, net};

    use super::*;

    const TICK: Duration = Duration::from_nanos((1e9 / TICK_RATE) as u64);

    /// Runs both sides for a while, in real time, until something is true of them.
    fn run_until(
        what: &str,
        server: &mut App,
        client: &mut App,
        done: impl Fn(&mut App, &mut App) -> bool,
    ) {
        for _ in 0..1500 {
            server.update();
            client.update();
            if done(server, client) {
                return;
            }
            std::thread::sleep(Duration::from_millis(4));
        }
        let world = server.world_mut();
        let shared = world.query::<&Replicate>().iter(world).count();
        let seats = world.query::<&net::Seat>().iter(world).count();
        let senders = world.query::<&ReplicationSender>().iter(world).count();
        let world = client.world_mut();
        let received = world.query::<&Remote>().iter(world).count();
        let states = world.query::<&RunState>().iter(world).count();
        let marks = world.query::<&SectorIndex>().iter(world).count();
        let tanks = world.query::<&Tank>().iter(world).count();
        panic!(
            "the two sides never got there: {what}. The server shares {shared} entities with \
             {seats} seats over {senders} senders; the client has {received} of them, \
             {states} runs, {marks} sectors and {tanks} tanks"
        );
    }

    #[test]
    fn a_client_joins_a_server_and_drives_its_tank_there() {
        // A port that is free on this machine.
        let port = UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        let mut server = App::new();
        server.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            CorePlugin,
            ServerPlugin,
            ServerPlugins {
                tick_duration: TICK,
            },
            ProtocolPlugin,
            net::NetPlugin,
        ));
        // What `App::run` does before the first frame: the network library sets itself up there.
        server.finish();
        server.cleanup();
        let mut commands = server.world_mut().commands();
        net::open(&mut commands, port);

        let mut client = App::new();
        client.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            CorePlugin,
            ClientPlugins {
                tick_duration: TICK,
            },
            ProtocolPlugin,
            crate::crew::plugin,
            NetPlugin,
        ));
        client.finish();
        client.cleanup();
        let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
        let mut commands = client.world_mut().commands();
        join(&mut commands, address, 7).unwrap();

        // The client connects and is given a seat.
        run_until("a seat", &mut server, &mut client, |server, _| {
            let world = server.world_mut();
            world.query::<&net::Seat>().iter(world).count() == 1
        });

        // The server starts a run, and the client sees it: the run, its sectors and its tank.
        server.world_mut().write_message(RunRequest::Start {
            seed: "NET".into(),
            players: 1,
        });
        run_until("the run", &mut server, &mut client, |_, client| {
            let world = client.world_mut();
            let tanks = world.query::<(&Tank, &Player)>().iter(world).count();
            let sectors = world.query::<&Sector>().iter(world).count();
            world.get_resource::<Run>().is_some() && tanks == 1 && sectors == 2
        });
        assert_eq!(client.world().resource::<Run>().seed_text, "NET");

        // What the player asks here moves the tank there, and the client sees it move.
        let own = |app: &mut App| {
            let world = app.world_mut();
            let mut tanks = world.query_filtered::<(Entity, &Position), With<Player>>();
            let (entity, position) = tanks.single(world).unwrap();
            (entity, position.0)
        };
        let (_, start) = own(&mut server);
        run_until("the drive", &mut server, &mut client, |server, client| {
            let (tank, _) = own(client);
            let mut command = client.world_mut().get_mut::<TankCommand>(tank).unwrap();
            command.drive = Vec2::Y;
            own(server).1.y > start.y + 2.0 && own(client).1.y > start.y + 1.0
        });
    }
}
