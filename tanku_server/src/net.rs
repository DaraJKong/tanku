//! The server's side of the network, for games of several players.
//!
//! The server listens for clients and gives each one a seat while its lobby is open. It sends
//! them the lobby, the run and the simulation as they change, and takes what their players ask
//! of their tanks. It needs the server plugins of the network library and the protocol of
//! `tanku_core`, added in that order.

use std::{
    io,
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use bevy::prelude::*;
use lightyear::{
    connection::shared::{ConnectionRequestHandler, DeniedReason},
    netcode::NetcodeServer,
    prelude::{
        input::{
            native::{ActionState, NativeStateSequence},
            server::{InputSystems, InputValidationAppExt, authorize_controlled_targets},
        },
        server::*,
        *,
    },
};
use tanku_core::{
    protocol::{Lobby, Member, Reliable, RunState, Told},
    rules::{MAX_PLAYERS, TICK_RATE},
    seed::VERSION,
    sim::{
        Bullet, Motion, Player, Position, Run, RunPhase, RunRequest, SimEvent, Tank, TankCommand,
    },
};

use crate::field::InSector;

/// The port a game listens on unless it is told otherwise.
pub const PORT: u16 = 28405;

/// The seat of the player behind a connection. The host has the first seat, and the guests
/// the ones that follow.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat(pub u8);

/// A game that this machine hosts for others to join.
#[derive(Resource)]
pub struct Hosting {
    /// What listens for the guests.
    server: Entity,
    /// What tells the guests of the lobby and of the run.
    game: Entity,
    /// Whether players can still join: nobody joins a run in progress.
    open: Arc<AtomicBool>,
}

impl Hosting {
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Relaxed)
    }

    /// Lets players join again, once the run is over.
    pub fn reopen(&self) {
        self.open.store(true, Ordering::Relaxed);
    }

    /// The entity that carries the lobby.
    pub fn game(&self) -> Entity {
        self.game
    }
}

/// A server that was told to stop. It goes at the end of the frame, which has told its guests.
#[derive(Component)]
struct Closing;

/// Turns away those who come while a run is in progress.
#[derive(Debug)]
struct Doorkeeper(Arc<AtomicBool>);

impl ConnectionRequestHandler for Doorkeeper {
    fn handle_request(&self, _: PeerId) -> Option<DeniedReason> {
        let open = self.0.load(Ordering::Relaxed);
        (!open).then(|| DeniedReason::Custom("a run is in progress".into()))
    }
}

/// What tells one version of the game from another on the network: players must run the same.
pub fn protocol_id() -> u64 {
    VERSION.bytes().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01B3)
    })
}

/// Opens a game on a port of this machine, for others to join. Fails if the port is taken.
pub fn open(commands: &mut Commands, port: u16) -> io::Result<()> {
    let address = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);
    // The network library takes the port a moment later, and stops the game if it can't.
    drop(UdpSocket::bind(address)?);
    let open = Arc::new(AtomicBool::new(true));
    let server = commands
        .spawn((
            Name::new("Server"),
            Server::default(),
            NetcodeServer::new(NetcodeConfig {
                protocol_id: protocol_id(),
                // Every player but the host is a client.
                max_clients: MAX_PLAYERS as usize - 1,
                connection_request_handler: Some(Arc::new(Doorkeeper(open.clone()))),
                // A guest names the address by which it found this machine: the one it has
                // on its network, or the one of its router. The game listens on all of them
                // and knows none as its own, so it must not turn away those who name one.
                server_addr_check: false,
                ..default()
            }),
            LocalAddr(address),
            ServerUdpIo::default(),
        ))
        .id();
    commands.trigger(Start { entity: server });
    let mut lobby = Lobby::default();
    lobby.seats[0] = Some(Member::Host);
    let game = commands
        .spawn((
            Name::new("Game"),
            lobby,
            Replicate::to_clients(NetworkTarget::All),
        ))
        .id();
    commands.insert_resource(Hosting { server, game, open });
    Ok(())
}

/// Closes the game that this machine hosts. Its guests are told, and the port is free again
/// at the end of the frame.
pub fn close(commands: &mut Commands, hosting: &Hosting) {
    commands.trigger(Stop {
        entity: hosting.server,
    });
    commands.entity(hosting.server).insert(Closing);
    commands.entity(hosting.game).despawn();
    commands.remove_resource::<Hosting>();
}

/// Gives the guests the seats that follow the host's, in the order they had. Returns how many
/// players there are.
fn rank(seats: &mut Query<&mut Seat>) -> u32 {
    let mut ranked: Vec<Mut<Seat>> = seats.iter_mut().collect();
    ranked.sort_by_key(|seat| seat.0);
    for (place, seat) in ranked.iter_mut().enumerate() {
        let wanted = place as u8 + 1;
        if seat.0 != wanted {
            seat.0 = wanted;
        }
    }
    ranked.len() as u32 + 1
}

/// Counts the players for a run that is about to start. Their seats have no gap, and nobody
/// joins from now on.
pub fn muster(hosting: &Hosting, seats: &mut Query<&mut Seat>) -> u32 {
    hosting.open.store(false, Ordering::Relaxed);
    rank(seats)
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(greet)
            .add_observer(seat)
            .add_observer(part)
            // A client commands its own tank and no other.
            .add_input_validator(authorize_controlled_targets::<NativeStateSequence<TankCommand>>)
            .add_systems(
                FixedPreUpdate,
                take_commands.after(InputSystems::UpdateActionState),
            )
            .add_systems(
                PostUpdate,
                (
                    (close_ranks, announce).chain(),
                    share,
                    (report::<Position>, report::<Motion>, report::<Bullet>),
                    herald,
                    tell,
                )
                    .run_if(resource_exists::<Hosting>)
                    .before(ReplicationSystems::Send),
            )
            .add_systems(Last, bury);
    }
}

/// A connection that opens can be sent the simulation.
fn greet(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert(ReplicationSender);
}

/// A client that connects while the lobby is open takes the first free seat.
fn seat(
    trigger: On<Add, Connected>,
    clients: Query<(), With<ClientOf>>,
    seats: Query<&Seat>,
    hosting: Option<Res<Hosting>>,
    mut commands: Commands,
) {
    if !hosting.is_some_and(|hosting| hosting.is_open()) || !clients.contains(trigger.entity) {
        return;
    }
    let taken: Vec<u8> = seats.iter().map(|seat| seat.0).collect();
    if let Some(free) = (1..MAX_PLAYERS as u8).find(|seat| !taken.contains(seat)) {
        commands.entity(trigger.entity).insert(Seat(free));
    }
}

/// A guest who goes during a run takes their tank along.
fn part(
    trigger: On<Remove, Seat>,
    seats: Query<&Seat>,
    run: Option<Res<Run>>,
    mut requests: MessageWriter<RunRequest>,
) {
    let in_progress = run.is_some_and(|run| run.phase != RunPhase::Over);
    if let (true, Ok(seat)) = (in_progress, seats.get(trigger.entity)) {
        requests.write(RunRequest::Leave { seat: seat.0 });
    }
}

/// While the lobby is open, the seats have no gap: a guest who leaves makes room at the end.
fn close_ranks(hosting: Res<Hosting>, mut seats: Query<&mut Seat>) {
    if hosting.is_open() {
        rank(&mut seats);
    }
}

/// Tells the guests who sits where.
fn announce(
    hosting: Res<Hosting>,
    seats: Query<(&Seat, &RemoteId)>,
    mut lobbies: Query<&mut Lobby>,
) {
    let Ok(mut lobby) = lobbies.get_mut(hosting.game) else {
        return;
    };
    let mut wanted = [None; MAX_PLAYERS as usize];
    wanted[0] = Some(Member::Host);
    for (seat, guest) in &seats {
        if let (Some(place), PeerId::Netcode(id)) = (wanted.get_mut(seat.0 as usize), guest.0) {
            *place = Some(Member::Guest(id));
        }
    }
    if lobby.seats != wanted {
        lobby.seats = wanted;
    }
}

/// Sends the guests everything the simulation owns. Of a tank and of a bullet, they are told
/// the place and the motion apart from the rest: they show those in their own time. The tank
/// of a guest is theirs to command. It stays when they go: the run takes it away.
fn share(
    mut commands: Commands,
    fresh: Query<
        (
            Entity,
            Option<&Player>,
            Option<(&Position, &Motion, &Tank)>,
            Option<(&Position, &Bullet)>,
        ),
        (Or<(With<InSector>, With<Player>)>, Without<Replicate>),
    >,
    seats: Query<(Entity, &Seat)>,
) {
    for (entity, player, tank, bullet) in &fresh {
        let mut entity = commands.entity(entity);
        entity.insert(Replicate::to_clients(NetworkTarget::All));
        if let Some((position, motion, _)) = tank {
            entity.insert((Told(*position), Told(*motion)));
        }
        if let Some((position, bullet)) = bullet {
            entity.insert((Told(*position), Told(*bullet)));
        }
        let owner = player.and_then(|player| seats.iter().find(|(_, seat)| seat.0 == player.seat));
        if let Some((owner, _)) = owner {
            entity.insert(ControlledBy {
                owner,
                lifetime: Lifetime::Persistent,
            });
        }
    }
}

/// Keeps what the guests are told of a tank or of a bullet as the simulation has it.
fn report<T: Component + Copy + PartialEq>(mut things: Query<(&T, &mut Told<T>), Changed<T>>) {
    for (now, mut told) in &mut things {
        if told.0 != *now {
            told.0 = *now;
        }
    }
}

/// The run as the guests are told it. Its clock moves by whole seconds until the run is over,
/// so that the run isn't sent again at every tick.
fn brief(run: &Run) -> Run {
    let mut told = run.clone();
    if told.phase != RunPhase::Over {
        told.ticks -= told.ticks % TICK_RATE as u64;
    }
    told
}

/// Keeps the guests told of the run.
fn herald(
    mut commands: Commands,
    hosting: Res<Hosting>,
    run: Option<Res<Run>>,
    mut states: Query<&mut RunState>,
) {
    match (run, states.get_mut(hosting.game)) {
        (Some(run), Ok(mut state)) => {
            let told = brief(&run);
            if state.0 != told {
                state.0 = told;
            }
        }
        (Some(run), Err(_)) => {
            commands.entity(hosting.game).insert(RunState(brief(&run)));
        }
        (None, Ok(_)) => {
            commands.entity(hosting.game).remove::<RunState>();
        }
        (None, Err(_)) => {}
    }
}

/// Tells the guests what happens in the simulation, for their effects and their sounds.
fn tell(
    hosting: Res<Hosting>,
    mut events: MessageReader<SimEvent>,
    mut sender: ServerMultiMessageSender,
    servers: Query<&Server>,
) {
    let Ok(server) = servers.get(hosting.server) else {
        return;
    };
    for event in events.read() {
        let _ = sender.send::<_, Reliable>(event, server, &NetworkTarget::All);
    }
}

/// What the guests ask of their tanks this tick, as their clients sent it.
fn take_commands(mut tanks: Query<(&ActionState<TankCommand>, &mut TankCommand), With<Player>>) {
    for (asked, mut command) in &mut tanks {
        *command = asked.0;
    }
}

/// A game that was closed lets go of its port. The frame has told its guests by now.
fn bury(mut commands: Commands, closed: Query<Entity, With<Closing>>) {
    for server in &closed {
        commands.entity(server).despawn();
    }
}
