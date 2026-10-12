//! The server's side of the network, for games of several players.
//!
//! The server listens for clients and gives each one a seat while its lobby is open: at once,
//! or once they have proved that they know the password of a game that has one. It sends
//! those who have a seat the lobby, the run and the simulation as they change, and takes what
//! their players ask of their tanks. It also announces its game on the local network, for
//! those who look for one.
//! It needs the server plugins of the network library and the protocol of `tanku_core`, added
//! in that order.
//!
//! Desktop games come over UDP. Browsers can't: they come over WebTransport, on a port of
//! its own, to a certificate that the game makes for itself. A listener takes them in and
//! hands each one to the server, which knows no difference between its guests from then on.

use std::{
    hash::{BuildHasher, Hasher, RandomState},
    io,
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use bevy::prelude::*;
use lightyear::{
    connection::shared::{ConnectionRequestHandler, DeniedReason},
    link::{LinkStart, Unlink, UnlinkReason},
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
    discovery::{Announcement, Announcer, Reach},
    lock::Lock,
    protocol::{Door, Lobby, Member, Reliable, RunState, Told, Word, named, protocol_id},
    rules::{MAX_PLAYERS, TICK_RATE},
    sim::{
        Bullet, Gun, Motion, Player, Position, Run, RunPhase, RunRequest, Shield, SimEvent, Tank,
        TankCommand,
    },
    stats::Genome,
};

use crate::field::InSector;

/// The seat of the player behind a connection. The host has the first seat, and the guests
/// the ones that follow.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat(pub u8);

/// The tank that the player behind a connection built in their garage, for runs in free mode.
/// Until they tell of one, they have the standard tank.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Built(pub Genome);

/// The ports on which a game listens: for the games of other machines, and for browsers if it
/// takes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ports {
    pub game: u16,
    pub web: Option<u16>,
}

/// How a browser reaches a hosted game: the port that listens for browsers, and the print of
/// the certificate that the game shows them. A browser only trusts a certificate that nobody
/// vouches for if it was given its print beforehand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Web {
    pub port: u16,
    /// The SHA-256 of the certificate, as 64 letters.
    pub print: String,
}

/// The seconds of a day, the measure of how long a certificate is good for.
const DAY: u64 = 24 * 60 * 60;
/// A certificate is dated a day back, for a browser whose clock runs behind this machine's,
/// and is good for 12 days from when it was made. A browser takes a certificate on the
/// strength of its print only if it is good for two weeks at most.
const DATED_BACK: Duration = Duration::from_secs(DAY);
const GOOD_FOR: Duration = Duration::from_secs(12 * DAY);
/// A certificate serves the games that open within a week. One that opens later gets a new
/// one, so that no certificate runs out on a game.
const KEPT: Duration = Duration::from_secs(7 * DAY);

/// The certificate that this machine shows to browsers, and when it was made. It is kept
/// from one game to the next, so that what a host hands out stays good for as long as the
/// program runs: players who were given it join the next game with it.
#[derive(Resource, Default)]
pub struct Credentials(Option<(Identity, SystemTime)>);

/// Whether a certificate made at some moment still serves a game that opens now. A clock
/// that was set back since says nothing of its age, so it doesn't.
fn serves(made: SystemTime, now: SystemTime) -> bool {
    now.duration_since(made).is_ok_and(|age| age < KEPT)
}

impl Credentials {
    /// The certificate to show to browsers from now on: the one that was kept, or a new one.
    /// It names no address of this machine, which a browser doesn't look at: the print of
    /// the certificate is all it goes by.
    fn certificate(&mut self) -> Option<Identity> {
        let now = SystemTime::now();
        if let Some((kept, made)) = &self.0
            && serves(*made, now)
        {
            return Some(kept.clone_identity());
        }
        let made = Identity::self_signed_builder()
            .subject_alt_names(["localhost", "127.0.0.1", "::1"])
            .not_before((now - DATED_BACK).into())
            .not_after((now + GOOD_FOR).into())
            .build()
            .ok()?;
        let shown = made.clone_identity();
        self.0 = Some((made, now));
        Some(shown)
    }
}

/// A game that this machine hosts for others to join.
#[derive(Resource)]
pub struct Hosting {
    /// What takes the guests, and listens for desktop games.
    server: Entity,
    /// What listens for browsers, if they are taken. Those who come that way are handed to
    /// the server: the network library takes one server for the guests of a game.
    listener: Option<Entity>,
    /// What tells the guests of the lobby and of the run.
    game: Entity,
    /// The port that it listens on for desktop games.
    port: u16,
    web: Option<Web>,
    /// Whether players can still join: nobody joins a run in progress.
    open: Arc<AtomicBool>,
    /// What is kept of the password of the game, if it has one.
    lock: Option<Lock>,
}

/// A guest at the door of a game that has a password: the number that their proof must
/// answer, and whether they were asked for it yet. They have no seat, and are sent nothing
/// of the game.
#[derive(Component)]
struct Knock {
    nonce: u64,
    asked: bool,
}

/// A number that nobody can tell beforehand: the standard library draws it from the system.
fn drawn() -> u64 {
    RandomState::new().build_hasher().finish()
}

/// How a hosted game is found on the local network: what announces it, and what it last
/// said of the game.
#[derive(Resource)]
struct Beacon {
    announcer: Announcer,
    /// The number by which the game is told from every other.
    game: u64,
    /// The name that its host gave it, or nothing.
    name: String,
    /// Whether it has a password.
    locked: bool,
    /// How many players the game had and whether it took more, when that was last said.
    said: Option<(u8, bool)>,
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

    /// How browsers reach the game, if it takes them.
    pub fn web(&self) -> Option<&Web> {
        self.web.as_ref()
    }
}

/// The server or the listener of a game that was closed, and the seconds that it is still
/// kept for.
#[derive(Component)]
struct Closing(f32);

/// What has said its last words. It goes at the end of the frame, which sends them.
#[derive(Component)]
struct Closed;

/// How long the server and the listener of a closed game are kept, in seconds. The guests
/// learn that the game is closed from its lobby, which goes at once, and that word takes a
/// moment to reach them. The goodbye of the network library doesn't do: it takes the
/// connections away in the frame that sends it, and where browsers are taken, one time in two
/// a guest then only sees its connection end, which a browser can't tell from one that was
/// lost.
const LAST_WORDS: f32 = 0.25;

/// Turns away those who come while a run is in progress.
#[derive(Debug)]
struct Doorkeeper(Arc<AtomicBool>);

impl ConnectionRequestHandler for Doorkeeper {
    fn handle_request(&self, _: PeerId) -> Option<DeniedReason> {
        let open = self.0.load(Ordering::Relaxed);
        (!open).then(|| DeniedReason::Custom("a run is in progress".into()))
    }
}

/// Opens a game on this machine, for others to join, and announces it where games are looked
/// for, under the name that its host gave it, if any. Its lobby starts as the host set the
/// game up: its name, and what its first run is. With a password, it lets in only those who
/// type it. Fails if the port for desktop games is taken. If the one for browsers is, the
/// game opens without them. Returns how browsers reach it, if they do.
pub fn open(
    commands: &mut Commands,
    ports: Ports,
    reach: &Reach,
    credentials: &mut Credentials,
    (game, password): (Lobby, &str),
) -> io::Result<Option<Web>> {
    let on = |port: u16| SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);
    // The network library takes a port a moment later, and stops the game if it can't.
    drop(UdpSocket::bind(on(ports.game))?);
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
            LocalAddr(on(ports.game)),
            ServerUdpIo::default(),
        ))
        .id();
    commands.trigger(Start { entity: server });
    // Browsers come by another way, and to a certificate that the game makes for itself.
    let mut web = None;
    let listener = ports.web.and_then(|port| {
        drop(UdpSocket::bind(on(port)).ok()?);
        let certificate = credentials.certificate()?;
        let print = certificate.certificate_chain().as_slice()[0].hash();
        let print = print.as_ref().iter().map(|byte| format!("{byte:02x}"));
        web = Some(Web {
            port,
            print: print.collect(),
        });
        let listener = commands
            .spawn((
                Name::new("Listener for browsers"),
                LocalAddr(on(port)),
                WebTransportServerIo { certificate },
            ))
            .id();
        // It listens, and no more: the server above takes those who come.
        commands.trigger(LinkStart { entity: listener });
        Some(listener)
    });
    let name = named(&game.name);
    // The password is turned into a key here and kept nowhere.
    let lock = Lock::new(password, drawn());
    let mut lobby = Lobby {
        name: name.clone(),
        locked: lock.is_some(),
        seats: default(),
        ..game
    };
    lobby.seats[0] = Some(Member::Host);
    let game = commands
        .spawn((
            Name::new("Game"),
            lobby,
            Replicate::to_clients(NetworkTarget::All),
        ))
        .id();
    commands.insert_resource(Hosting {
        server,
        listener,
        game,
        port: ports.game,
        web: web.clone(),
        open,
        lock,
    });
    // A game that can't be announced is joined by its address all the same.
    let game = drawn();
    if let Some(announcer) = Announcer::new(*reach, game) {
        commands.insert_resource(Beacon {
            announcer,
            game,
            name,
            locked: lock.is_some(),
            said: None,
        });
    }
    Ok(web)
}

/// Closes the game that this machine hosts. Its lobby goes at once, which tells its guests,
/// and the local network is told that the game is gone. The ports are free again a moment
/// later.
pub fn close(commands: &mut Commands, hosting: &Hosting) {
    commands.entity(hosting.game).despawn();
    for kept in [hosting.server].into_iter().chain(hosting.listener) {
        commands.entity(kept).insert(Closing(LAST_WORDS));
    }
    commands.remove_resource::<Hosting>();
    commands.remove_resource::<Beacon>();
}

/// Gives the guests the seats that follow the host's, in the order they had.
fn rank<'a>(seats: impl Iterator<Item = Mut<'a, Seat>>) {
    let mut ranked: Vec<Mut<Seat>> = seats.collect();
    ranked.sort_by_key(|seat| seat.0);
    for (place, seat) in ranked.iter_mut().enumerate() {
        let wanted = place as u8 + 1;
        if seat.0 != wanted {
            seat.0 = wanted;
        }
    }
}

/// Gathers the guests for a run that is about to start. Their seats have no gap, and nobody
/// joins from now on. Returns the tank that each of them built, in the order of their seats:
/// the standard tank for whoever built none.
pub fn muster(hosting: &Hosting, seats: &mut Query<(&mut Seat, Option<&Built>)>) -> Vec<Genome> {
    hosting.open.store(false, Ordering::Relaxed);
    rank(seats.iter_mut().map(|(seat, _)| seat));
    by_seat(seats.iter().map(|(seat, built)| (*seat, built.copied())))
}

/// The tanks of the guests in the order of their seats, whatever order they are given in:
/// the standard tank for whoever built none.
fn by_seat(guests: impl Iterator<Item = (Seat, Option<Built>)>) -> Vec<Genome> {
    let tank = |built: Option<Built>| built.map_or(Genome::PLAYER, |built| built.0);
    let mut guests: Vec<(u8, Genome)> = guests.map(|(seat, built)| (seat.0, tank(built))).collect();
    guests.sort_by_key(|(seat, _)| *seat);
    guests.into_iter().map(|(_, tank)| tank).collect()
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Reach>()
            .init_resource::<Credentials>()
            .add_observer(greet)
            .add_observer(seat)
            .add_observer(part)
            // A client commands its own tank and no other.
            .add_input_validator(authorize_controlled_targets::<NativeStateSequence<TankCommand>>)
            .add_systems(
                FixedPreUpdate,
                take_commands.after(InputSystems::UpdateActionState),
            )
            .add_systems(
                PreUpdate,
                admit
                    .after(MessageSystems::Receive)
                    .run_if(resource_exists::<Hosting>),
            )
            .add_systems(
                PostUpdate,
                (
                    (close_ranks, announce).chain(),
                    challenge,
                    share,
                    (
                        report::<Position>,
                        report::<Motion>,
                        report::<Bullet>,
                        report::<Gun>,
                        report::<Shield>,
                    ),
                    herald,
                    tell,
                )
                    .run_if(resource_exists::<Hosting>)
                    .before(ReplicationSystems::Send),
            )
            .add_systems(
                Update,
                call_out.run_if(resource_exists::<Hosting>.and_then(resource_exists::<Beacon>)),
            )
            .add_systems(Update, wind_down)
            .add_systems(Last, bury);
    }
}

/// A connection that a browser opened came to the listener for browsers, and is handed to
/// the server, which takes it as any other.
fn greet(
    trigger: On<Add, LinkOf>,
    hosting: Option<Res<Hosting>>,
    links: Query<&LinkOf>,
    mut commands: Commands,
) {
    let came_to = links.get(trigger.entity).map(|link| link.server);
    if let (Some(hosting), Ok(came_to)) = (hosting, came_to)
        && hosting.listener == Some(came_to)
    {
        commands.entity(trigger.entity).insert(LinkOf {
            server: hosting.server,
        });
    }
}

/// Gives a guest the first seat that isn't taken, and with it the game: a guest who has no
/// seat is sent nothing of it.
fn give_seat(commands: &mut Commands, guest: Entity, taken: &mut Vec<u8>) {
    if let Some(free) = (1..MAX_PLAYERS as u8).find(|seat| !taken.contains(seat)) {
        taken.push(free);
        commands
            .entity(guest)
            .insert((Seat(free), ReplicationSender));
    }
}

/// A client that connects while the lobby is open takes the first free seat. Where the game
/// has a password, it waits at the door until it has proved that its player knows it.
fn seat(
    trigger: On<Add, Connected>,
    clients: Query<(), With<ClientOf>>,
    seats: Query<&Seat>,
    hosting: Option<Res<Hosting>>,
    mut commands: Commands,
) {
    let Some(hosting) = hosting.filter(|hosting| hosting.is_open()) else {
        return;
    };
    if !clients.contains(trigger.entity) {
        return;
    }
    if hosting.lock.is_some() {
        commands.entity(trigger.entity).insert(Knock {
            nonce: drawn(),
            asked: false,
        });
        return;
    }
    let mut taken: Vec<u8> = seats.iter().map(|seat| seat.0).collect();
    give_seat(&mut commands, trigger.entity, &mut taken);
}

/// Asks each guest at the door for the proof that their player knows the password.
fn challenge(hosting: Res<Hosting>, mut guests: Query<(&mut Knock, &mut MessageSender<Door>)>) {
    let Some(lock) = &hosting.lock else {
        return;
    };
    for (mut knock, mut door) in &mut guests {
        if !knock.asked {
            knock.asked = true;
            door.send::<Reliable>(Door::Locked(lock.challenge(knock.nonce)));
        }
    }
}

/// Hears what the guests say. Those who prove that their player knows the password of the
/// game are let in, and the others are told that they don't: a challenge takes one answer,
/// so whoever would try another password comes back for another challenge. A guest who has
/// a seat tells of the tank that their player built.
fn admit(
    mut commands: Commands,
    hosting: Res<Hosting>,
    mut guests: Query<
        (
            Entity,
            &RemoteId,
            (Option<&Knock>, Has<Seat>),
            &mut MessageReceiver<Word>,
            &mut MessageSender<Door>,
        ),
        With<ClientOf>,
    >,
    seats: Query<&Seat>,
) {
    let mut taken: Vec<u8> = seats.iter().map(|seat| seat.0).collect();
    for (guest, id, (knock, seated), mut words, mut door) in &mut guests {
        let mut knock = knock;
        for word in words.receive() {
            match word {
                // A guest can say anything: only a tank that a player may build is taken,
                // and only from a guest who is in the game.
                Word::Tank(tank) => {
                    if seated && tank.is_a_players() {
                        commands.entity(guest).insert(Built(tank));
                    }
                }
                Word::Proof(proof) => {
                    // A proof that answers no challenge is nothing: the game has no
                    // password, or this guest was let in or turned away already.
                    let asked = (hosting.lock.as_ref(), knock.take(), id.0);
                    let (Some(lock), Some(knock), PeerId::Netcode(number)) = asked else {
                        continue;
                    };
                    commands.entity(guest).remove::<Knock>();
                    if !lock.opens(knock.nonce, number, &proof) {
                        door.send::<Reliable>(Door::Refused);
                    } else if hosting.is_open() {
                        give_seat(&mut commands, guest, &mut taken);
                    }
                }
            }
        }
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
        rank(seats.iter_mut());
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
/// the place and the motion apart from the rest: they show those in their own time. They show
/// the gun of a player's tank and nothing of the others'. The tank of a guest is theirs to
/// command. It stays when they go: the run takes it away.
fn share(
    mut commands: Commands,
    fresh: Query<
        (
            Entity,
            Option<&Player>,
            Option<(&Position, &Motion, &Shield, &Tank)>,
            Option<&Gun>,
            Option<(&Position, &Bullet)>,
        ),
        (Or<(With<InSector>, With<Player>)>, Without<Replicate>),
    >,
    seats: Query<(Entity, &Seat)>,
) {
    for (entity, player, tank, gun, bullet) in &fresh {
        let mut entity = commands.entity(entity);
        entity.insert(Replicate::to_clients(NetworkTarget::All));
        if let Some((position, motion, shield, _)) = tank {
            entity.insert((Told(*position), Told(*motion), Told(*shield)));
        }
        if let (Some(_), Some(gun)) = (player, gun) {
            entity.insert(Told(*gun));
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

/// Keeps the local network told of the game: its name, whether it has a password, how many
/// players it has, and whether it takes more. Those who look for a game see all of it.
fn call_out(hosting: Res<Hosting>, lobbies: Query<&Lobby>, mut beacon: ResMut<Beacon>) {
    let players = lobbies.get(hosting.game).map_or(1, Lobby::players) as u8;
    let news = Some((players, hosting.is_open()));
    if beacon.said != news {
        beacon.said = news;
        let told = Announcement {
            name: beacon.name.clone(),
            locked: beacon.locked,
            ..Announcement::of(beacon.game, hosting.port, players, hosting.is_open())
        };
        beacon.announcer.tell(&told);
    }
}

/// Stops the server of a closed game once its guests had the time to learn of it. Whoever is
/// still there gets the goodbye of the network library, which this frame sends.
fn wind_down(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut kept: Query<(Entity, &mut Closing, Has<NetcodeServer>), Without<Closed>>,
) {
    for (entity, mut closing, server) in &mut kept {
        closing.0 -= time.delta_secs();
        if closing.0 > 0.0 {
            continue;
        }
        if server {
            commands.trigger(Stop { entity });
        }
        commands.entity(entity).insert(Closed);
    }
}

/// A game that was closed lets go of its ports. The frame has said its last words by now.
fn bury(mut commands: Commands, closed: Query<(Entity, Has<WebTransportServerIo>), With<Closed>>) {
    for (entity, listens) in &closed {
        if listens {
            commands.trigger(Unlink {
                entity,
                reason: UnlinkReason::UserRequested(None),
            });
        }
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_certificate_serves_the_games_of_a_week() {
        let made = SystemTime::now();
        assert!(serves(made, made));
        assert!(serves(made, made + Duration::from_secs(6 * DAY)));
        assert!(!serves(made, made + Duration::from_secs(8 * DAY)));
        // A clock that was set back tells nothing of an age.
        assert!(!serves(made, made - Duration::from_secs(60)));
        // It is good for less than the two weeks that a browser allows, and outlasts by
        // days every game that it serves.
        assert!(DATED_BACK + GOOD_FOR < Duration::from_secs(14 * DAY));
        assert!(GOOD_FOR > KEPT + Duration::from_secs(2 * DAY));
    }

    #[test]
    fn the_tanks_of_the_guests_go_by_their_seats() {
        let mut fast = Genome::PLAYER;
        fast.set(tanku_core::stats::Stat::Engine, 6);
        let mut slow = Genome::PLAYER;
        slow.set(tanku_core::stats::Stat::Engine, 1);
        // The guests come in no order: the game keeps them as it pleases.
        let guests = [
            (Seat(3), Some(Built(slow))),
            (Seat(1), None),
            (Seat(2), Some(Built(fast))),
        ];
        assert_eq!(by_seat(guests.into_iter()), [Genome::PLAYER, fast, slow]);
        assert!(by_seat(std::iter::empty()).is_empty());
    }

    #[test]
    fn a_certificate_is_kept_until_it_is_out_of_date() {
        let print =
            |identity: Identity| *identity.certificate_chain().as_slice()[0].hash().as_ref();
        let mut credentials = Credentials::default();
        let first = print(credentials.certificate().unwrap());
        assert_eq!(print(credentials.certificate().unwrap()), first);
        // One that was made too long ago is replaced.
        credentials.0.as_mut().unwrap().1 -= KEPT;
        let next = print(credentials.certificate().unwrap());
        assert_ne!(next, first);
        assert_eq!(print(credentials.certificate().unwrap()), next);
    }
}
