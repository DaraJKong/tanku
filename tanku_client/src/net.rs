//! The network of a game that several players share, for a machine that joins one.
//!
//! A guest simulates nothing: it receives the lobby, the run and the simulation of its host,
//! and sends what its player asks of their tank. A desktop game joins at the address of its
//! host. A browser can't: it only speaks to a server that shows a certificate, and takes the
//! one that a host makes for itself only with the print of that certificate. So a browser
//! joins by what its host hands out, an address and a print. Hosting is in `host`, which a
//! browser doesn't have.
//!
//! A game that has a password asks whoever knocks for a proof of it. A guest whose player
//! typed one answers with it. One whose player typed none, or the wrong one, leaves, and the
//! player is asked.

#[cfg(not(target_arch = "wasm32"))]
use std::net::ToSocketAddrs;
use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    time::Duration,
};

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use bevy::tasks::{IoTaskPool, Task, futures::check_ready};
use lightyear::{
    netcode::{NetcodeClient, client_plugin::NetcodeConfig},
    prelude::{
        client::{input::InputSystems, *},
        input::native::{ActionState, InputMarker},
        *,
    },
};
use tanku_core::{
    lock::{prove, worded},
    map::{Corridor, Room},
    protocol::{
        Door, Lobby, Member, PORT, ProtocolPlugin, Reliable, RunState, WEB_PORT, Word, protocol_id,
    },
    rules::TICK_RATE,
    seed::VERSION,
    sim::{Player, Run, RunPhase, Sector, SectorIndex, SimEvent, TankCommand},
    stats::Genome,
};
use tanku_server::field::Field;

use crate::{
    Overlay, Screen,
    crew::{LocalSeat, Own},
    garage::Garage,
    mirror,
    session::{Asked, Notice, Session, SessionRequest, Shut},
};

/// How long a guest stays connected without a seat before it takes the game for full.
const SEAT_WAIT: f32 = 3.0;

/// How many seconds a guest and its host wait for each other before they call the connection
/// lost. It is also how long a guest tries to reach a game that doesn't answer.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Patience(pub i32);

/// What a player asked to join: where, in their words or in those of a host, and with which
/// password if they typed one.
#[derive(Clone, Debug)]
struct Wanted {
    at: String,
    password: Option<String>,
}

/// What a host said at the door of a game that has a password, to a guest that it didn't
/// let in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Turned {
    /// The game has a password, and the player typed none.
    Asked,
    /// The password that the player typed is not the one.
    Refused,
}

/// The game that this machine joined, or is trying to.
#[derive(Resource)]
struct Joined {
    client: Entity,
    /// The number that this machine goes by in that game.
    id: u64,
    /// Where the game is, as the player is told.
    address: String,
    /// What the player asked for: it joins the game again.
    wanted: Wanted,
    /// Whether it is joined the way browsers join, by what its host handed out.
    by_link: bool,
    /// Whether the game gave this machine a seat.
    seated: bool,
    /// Seconds spent connected without a seat.
    waited: f32,
    /// Whether the host turned this machine away at its door, and how.
    turned: Option<Turned>,
    /// The tank that the host was last told of, as the one that the player built.
    told: Option<Genome>,
}

/// The name of a host being looked up, before its game can be joined.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct Lookup {
    name: String,
    found: Task<Option<SocketAddr>>,
    wanted: Wanted,
}

/// A connection that the player ended, and the seconds that it is still kept for. It goes
/// once the host was told.
#[derive(Component)]
struct Leaving(f32);

/// How long a connection that ended is kept, in seconds, for its last words to go out. Over
/// UDP they are out by the end of the frame. The way browsers join, they leave a moment
/// later, and a connection that is gone before them leaves its host waiting for a guest who
/// has left.
const LAST_WORDS: f32 = 0.25;

/// How many letters the print of a certificate has: the 32 bytes of a SHA-256, two a byte.
const PRINT_LENGTH: usize = 64;
/// What a player in a browser is told when they give the address of a host and no print.
#[cfg(target_arch = "wasm32")]
const LINK_ONLY: &str = "A browser joins by the link that the host hands out, not by an address.";
/// What they are told where the browser lets the page speak to no game.
#[cfg(target_arch = "wasm32")]
const NO_WAY: &str = "This page can't join a game here. A browser lets a page join one only if it \
                      loaded that page over HTTPS or from its own machine, and some browsers \
                      can't join at all.";

/// Where a game is, from what a player typed or was given.
#[derive(Debug, PartialEq, Eq)]
enum Target {
    /// The address at which the game takes desktop games.
    Address(SocketAddr),
    /// The address at which it takes browsers, and the print of the certificate that it
    /// shows them there.
    Web(SocketAddr, String),
    /// A machine known by a name, which has to be looked up, and its port.
    Name(String, u16),
}

/// The address of a machine as numbers, with a port unless it is the one that is usual.
fn numbered(text: &str, usual: u16) -> Option<SocketAddr> {
    if let Ok(machine) = text.parse::<Ipv4Addr>() {
        return Some(SocketAddr::new(machine.into(), usual));
    }
    let address = text.parse::<SocketAddrV4>().ok()?;
    (address.port() != 0).then_some(address.into())
}

/// Reads where a game is, as a player types it or as its host hands it out.
///
/// What a player types is the number of a machine or its name, followed by a port unless
/// the game uses the usual one. What a host hands out for browsers is such a number, a
/// stroke and the print of a certificate: only an address that comes with a print leads to
/// where browsers are taken. It may come at the end of a link, after the last `#`.
fn target(text: &str) -> Option<Target> {
    let text = text.trim().rsplit('#').next()?;
    if let Some((address, print)) = text.rsplit_once('/') {
        let hex = print.len() == PRINT_LENGTH && print.chars().all(|c| c.is_ascii_hexdigit());
        let address = numbered(address, WEB_PORT).filter(|_| hex)?;
        return Some(Target::Web(address, print.to_ascii_lowercase()));
    }
    if let Some(address) = numbered(text, PORT) {
        return Some(Target::Address(address));
    }
    let (name, port) = match text.rsplit_once(':') {
        Some((name, port)) => (name, port.parse().ok().filter(|port| *port != 0)?),
        None => (text, PORT),
    };
    let named = |letter: char| letter.is_ascii_alphanumeric() || letter == '-' || letter == '.';
    (!name.is_empty() && name.chars().all(named)).then(|| Target::Name(name.to_string(), port))
}

/// What a host hands out for browsers: where it takes them, and the print of its certificate.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn web_code(machine: impl std::fmt::Display, port: u16, print: &str) -> String {
    format!("{machine}:{port}/{print}")
}

/// An address as a player types it: the port is left out when it is the usual one.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn written(machine: impl std::fmt::Display, port: u16) -> String {
    if port == PORT {
        machine.to_string()
    } else {
        format!("{machine}:{port}")
    }
}

/// A number for this machine to go by in a game: any one will do, as long as no other guest
/// has the same.
#[cfg(not(target_arch = "wasm32"))]
fn fresh_id() -> u64 {
    use std::hash::{BuildHasher, Hasher, RandomState};
    RandomState::new().build_hasher().finish().max(1)
}

/// In a browser, the number is drawn by the browser: what the standard library draws there
/// is the same in every page.
#[cfg(target_arch = "wasm32")]
fn fresh_id() -> u64 {
    let mut drawn = [0u8; 8];
    let crypto = web_sys::window().and_then(|window| window.crypto().ok());
    let random =
        crypto.is_some_and(|crypto| crypto.get_random_values_with_u8_array(&mut drawn).is_ok());
    if !random {
        // Without that, the moment that the page asks is all that tells it apart.
        let now = web_sys::window().and_then(|window| window.performance());
        drawn = now.map_or(1.0, |clock| clock.now()).to_bits().to_le_bytes();
    }
    u64::from_le_bytes(drawn).max(1)
}

/// Whether the browser lets this page speak to a game: it must know how, and it only lets a
/// page that it loaded safely do so.
#[cfg(target_arch = "wasm32")]
fn speaks() -> bool {
    use web_sys::{js_sys::Reflect, wasm_bindgen::JsValue};
    let known = |window| Reflect::has(&window, &JsValue::from_str("WebTransport"));
    web_sys::window().is_some_and(|window| known(window.into()).unwrap_or(false))
}

/// Starts to join the game at an address: the one for desktop games, or the one for
/// browsers when it comes with the print of the game's certificate.
fn join(
    commands: &mut Commands,
    (address, print): (SocketAddr, Option<String>),
    wanted: Wanted,
    patience: Patience,
    notice: &mut Notice,
) -> bool {
    #[cfg(target_arch = "wasm32")]
    if !speaks() {
        notice.0 = Some(NO_WAY.to_string());
        return false;
    }
    let id = fresh_id();
    let auth = Authentication::Manual {
        server_addr: address,
        client_id: id,
        private_key: default(),
        protocol_id: protocol_id(),
    };
    let config = NetcodeConfig {
        client_timeout_secs: patience.0,
        // The host would read a time limit by its own clock, and the clocks of two machines
        // needn't agree. The patience above is the limit.
        token_expire_secs: -1,
        ..default()
    };
    let Ok(netcode) = NetcodeClient::new(auth, config) else {
        notice.0 = Some("The game could not be joined.".to_string());
        return false;
    };
    let mut client = commands.spawn((
        Name::new("Client"),
        Client,
        ReplicationReceiver,
        Link::default(),
        LocalAddr(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0)),
        PeerAddr(address),
        netcode,
    ));
    let by_link = print.is_some();
    let told = match print {
        Some(print) => {
            client.insert(WebTransportClientIo {
                certificate_digest: print,
                target: None,
            });
            address.to_string()
        }
        #[cfg(not(target_arch = "wasm32"))]
        None => {
            client.insert(UdpIo::default());
            written(address.ip(), address.port())
        }
        // A browser has no other way than the one that comes with a print.
        #[cfg(target_arch = "wasm32")]
        None => {
            client.despawn();
            notice.0 = Some(LINK_ONLY.to_string());
            return false;
        }
    };
    let client = client.id();
    commands.trigger(Connect { entity: client });
    commands.insert_resource(Joined {
        client,
        id,
        address: told,
        wanted,
        by_link,
        seated: false,
        waited: 0.0,
        turned: None,
        told: None,
    });
    true
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        let tick_duration = Duration::from_secs_f64(1.0 / TICK_RATE);
        // The plugins of the network library come first, then the protocol, then what
        // listens and what joins.
        #[cfg(not(target_arch = "wasm32"))]
        app.add_plugins(lightyear::prelude::server::ServerPlugins { tick_duration });
        app.add_plugins((ClientPlugins { tick_duration }, ProtocolPlugin));
        #[cfg(not(target_arch = "wasm32"))]
        app.add_plugins((crate::host::plugin, crate::discovery::plugin))
            .add_systems(Update, find_the_host.after(direct));
        #[cfg(target_arch = "wasm32")]
        app.add_systems(Update, follow_the_link.before(direct));
        app.add_plugins(mirror::plugin)
            // A guest makes its picture of the run from what its host told, once that is in.
            .configure_sets(PreUpdate, mirror::Picture.after(lay_out_sectors))
            .insert_resource(Patience(5))
            // A game without a garage has the standard tank in it.
            .init_resource::<Garage>()
            .add_systems(Update, direct)
            .add_systems(
                FixedPreUpdate,
                send_commands.in_set(InputSystems::WriteClientInputs),
            )
            .add_systems(
                PreUpdate,
                (
                    knock,
                    watch,
                    (
                        tell_of_the_tank,
                        take_the_controls,
                        hear,
                        follow_run,
                        lay_out_sectors,
                        follow_host,
                    )
                        .chain()
                        .run_if(resource_exists::<Joined>),
                )
                    .chain()
                    .after(MessageSystems::Receive)
                    .run_if(resource_exists::<Joined>),
            )
            .add_systems(Last, let_go);
    }
}

/// What the address of this page ends with, when that is what a host hands out for
/// browsers. A desktop game has no page, and so no such link.
pub(crate) fn link() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let end = web_sys::window()?.location().hash().ok()?;
        matches!(target(&end), Some(Target::Web(..))).then_some(end)
    }
    #[cfg(not(target_arch = "wasm32"))]
    None
}

/// A page that is opened by the link of a host joins its game at once: what the host handed
/// out is at the end of the address of the page. A link that is pasted into the page later,
/// or put at the end of its address, is followed too, from the menus: no link takes a
/// player out of a run. A link is followed once. The join screen offers it again.
#[cfg(target_arch = "wasm32")]
fn follow_the_link(
    session: Res<Session>,
    screen: Res<State<Screen>>,
    mut followed: Local<Option<String>>,
    mut requests: MessageWriter<SessionRequest>,
    mut next_screen: ResMut<NextState<Screen>>,
) {
    if *session != Session::Solo || !matches!(*screen.get(), Screen::Menu | Screen::Join) {
        return;
    }
    let link = link();
    if link == *followed {
        return;
    }
    followed.clone_from(&link);
    if let Some(link) = link {
        next_screen.set(Screen::Join);
        requests.write(SessionRequest::Join(link));
    }
}

/// Does what the menus ask of a guest: to join a game, and to leave it.
fn direct(
    mut commands: Commands,
    mut requests: MessageReader<SessionRequest>,
    (mut session, mut notice, mut seat): (ResMut<Session>, ResMut<Notice>, ResMut<LocalSeat>),
    patience: Res<Patience>,
    joined: Option<Res<Joined>>,
    mut words: Query<&mut MessageSender<Word>, With<Client>>,
) {
    for request in requests.read() {
        match (request, *session) {
            // The host is told, and tells everyone who is ready.
            (SessionRequest::Ready(ready), Session::Guest) => {
                let client = joined.as_ref().filter(|joined| joined.seated);
                let words = client.and_then(|joined| words.get_mut(joined.client).ok());
                if let Some(mut words) = words {
                    words.send::<Reliable>(Word::Ready(*ready));
                }
            }
            (SessionRequest::Join(text) | SessionRequest::JoinLocked(text, _), Session::Solo) => {
                notice.0 = None;
                // A password that holds nothing is none: the host then asks for one.
                let password = match request {
                    SessionRequest::JoinLocked(_, password) => Some(worded(password)),
                    _ => None,
                };
                let wanted = Wanted {
                    at: text.clone(),
                    password: password.filter(|password| !password.is_empty()),
                };
                let started = match target(text) {
                    Some(Target::Address(address)) => {
                        let at = (address, None);
                        join(&mut commands, at, wanted, *patience, &mut notice)
                    }
                    Some(Target::Web(address, print)) => {
                        let at = (address, Some(print));
                        join(&mut commands, at, wanted, *patience, &mut notice)
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    Some(Target::Name(name, port)) => {
                        let host = name.clone();
                        let found = IoTaskPool::get().spawn(async move {
                            let found = (host.as_str(), port).to_socket_addrs().ok()?;
                            found.into_iter().find(SocketAddr::is_ipv4)
                        });
                        commands.insert_resource(Lookup {
                            name,
                            found,
                            wanted,
                        });
                        true
                    }
                    #[cfg(target_arch = "wasm32")]
                    Some(Target::Name(..)) => {
                        notice.0 = Some(LINK_ONLY.to_string());
                        false
                    }
                    None => {
                        let hint = "That is not an address. The lobby of the host shows it.";
                        notice.0 = Some(hint.to_string());
                        false
                    }
                };
                if started {
                    *session = Session::Guest;
                }
            }
            (SessionRequest::Leave, Session::Guest) => {
                if let Some(joined) = &joined {
                    // The host is told at the end of the frame, and the connection goes then.
                    commands.trigger(Disconnect {
                        entity: joined.client,
                    });
                    commands.entity(joined.client).insert(Leaving(LAST_WORDS));
                    commands.remove_resource::<Joined>();
                    commands.remove_resource::<Run>();
                    commands.remove_resource::<Field>();
                }
                #[cfg(not(target_arch = "wasm32"))]
                commands.remove_resource::<Lookup>();
                *session = Session::Solo;
                seat.0 = 0;
            }
            // The rest is not a guest's to do: opening a game and starting its run are a
            // host's, and nobody joins a second game.
            _ => {}
        }
    }
}

/// Joins the game of a host once its name was looked up.
#[cfg(not(target_arch = "wasm32"))]
fn find_the_host(
    mut commands: Commands,
    lookup: Option<ResMut<Lookup>>,
    patience: Res<Patience>,
    mut session: ResMut<Session>,
    mut notice: ResMut<Notice>,
) {
    let Some(mut lookup) = lookup else {
        return;
    };
    let Some(found) = check_ready(&mut lookup.found) else {
        return;
    };
    commands.remove_resource::<Lookup>();
    let joined = match found {
        Some(address) => {
            let wanted = lookup.wanted.clone();
            join(
                &mut commands,
                (address, None),
                wanted,
                *patience,
                &mut notice,
            )
        }
        None => {
            notice.0 = Some(format!("No machine is known as {}.", lookup.name));
            false
        }
    };
    if !joined {
        *session = Session::Solo;
    }
}

/// What a guest is told when its host closed the game.
const CLOSED: &str = "The host closed the game.";

/// Why a connection ended, as the player is told.
fn farewell(reason: &DisconnectedReason, joined: &Joined) -> String {
    let address = &joined.address;
    match reason {
        // A host that closes its game says so. Where browsers are taken, its line may close
        // before its word is through, which says the same.
        DisconnectedReason::ByPeer(_) | DisconnectedReason::LinkFailed(UnlinkReason::ByPeer(_)) => {
            CLOSED.to_string()
        }
        _ if joined.seated => "The connection to the host was lost.".to_string(),
        DisconnectedReason::TransportError(error) if error.contains("Denied") => {
            format!("The game at {address} is full, or its run has started.")
        }
        // What a host hands out for browsers is good while its program runs: one that is
        // started again hands out another.
        _ if joined.by_link => format!(
            "No game answers at {address}. Its host must run version {VERSION} too, and the \
             link must be the one that its lobby shows now."
        ),
        _ => format!("No game answers at {address}. Its host must run version {VERSION} too."),
    }
}

/// What a player is told when the password that they typed is not the one of the game.
const WRONG_PASSWORD: &str = "That is not the password of the game.";

/// Answers a host that asks for the password of its game: with the proof of the one that the
/// player typed, if they typed one. What the host says otherwise is kept for `watch`.
fn knock(
    mut joined: ResMut<Joined>,
    mut doors: Query<(&mut MessageReceiver<Door>, &mut MessageSender<Word>), With<Client>>,
) {
    let Ok((mut door, mut words)) = doors.get_mut(joined.client) else {
        return;
    };
    for said in door.receive() {
        match (said, joined.wanted.password.clone()) {
            (Door::Locked(challenge), Some(password)) => {
                let proof = prove(challenge, joined.id, &password);
                words.send::<Reliable>(Word::Proof(proof));
            }
            (Door::Locked(_), None) => joined.turned = Some(Turned::Asked),
            (Door::Refused, _) => joined.turned = Some(Turned::Refused),
        }
    }
}

/// Watches the connection to the host: takes the seat that the host gives, and goes back to
/// the menus when the connection ends. A host that asks for a password which the player
/// didn't type, or typed wrong, has the player asked for it.
fn watch(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut joined: ResMut<Joined>,
    clients: Query<(Has<Connected>, Option<&Disconnected>)>,
    lobbies: Query<&Lobby>,
    (mut seat, mut session, mut notice, mut asked): (
        ResMut<LocalSeat>,
        ResMut<Session>,
        ResMut<Notice>,
        ResMut<Asked>,
    ),
    screen: Res<State<Screen>>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut next_overlay: ResMut<NextState<Overlay>>,
) {
    let Ok((connected, disconnected)) = clients.get(joined.client) else {
        return;
    };
    let lobby = lobbies.single().ok();
    let seated = lobby.and_then(|lobby| lobby.seat_of(Member::Guest(joined.id)));
    let ended = match (joined.turned, disconnected, connected, seated) {
        // The player is asked for the password, of the game by the name that they found it
        // under if they did.
        (Some(turned), ..) => {
            let known = asked.0.take().filter(|shut| shut.at == joined.wanted.at);
            asked.0 = Some(Shut {
                at: joined.wanted.at.clone(),
                name: known.map(|shut| shut.name).unwrap_or_default(),
                address: joined.address.clone(),
            });
            Some((turned == Turned::Refused).then(|| WRONG_PASSWORD.to_string()))
        }
        (None, Some(disconnected), ..) => Some(Some(farewell(&disconnected.reason, &joined))),
        // A host that closes its game takes the lobby away first, and its guests leave on
        // that: the connection is still whole then, and carries the word to every guest.
        (None, None, true, None) if joined.seated && lobby.is_none() => {
            Some(Some(CLOSED.to_string()))
        }
        (None, None, true, Some(given)) => {
            // The lobby is where a guest comes in. Whoever is in is asked nothing more.
            if !joined.seated {
                next_screen.set(Screen::Lobby);
                asked.0 = None;
            }
            joined.seated = true;
            joined.waited = 0.0;
            seat.set_if_neq(LocalSeat(given));
            None
        }
        // A host that has no seat to give says so by giving none.
        (None, None, true, None) => {
            joined.waited += time.delta_secs();
            (joined.waited > SEAT_WAIT).then(|| {
                let denied = DisconnectedReason::TransportError("Denied".to_string());
                Some(farewell(&denied, &joined))
            })
        }
        (None, None, false, _) => None,
    };
    let Some(told) = ended else {
        return;
    };
    notice.0 = told;
    // The connection may still be up when the host gave no seat.
    commands.trigger(Disconnect {
        entity: joined.client,
    });
    commands.entity(joined.client).insert(Leaving(LAST_WORDS));
    commands.remove_resource::<Joined>();
    commands.remove_resource::<Run>();
    commands.remove_resource::<Field>();
    *session = Session::Solo;
    seat.0 = 0;
    // Whoever was still asking to join can ask again where they are, and whoever is asked
    // for a password is asked on the join screen.
    let wanted = match joined.turned {
        Some(_) => Screen::Join,
        None => Screen::Menu,
    };
    if *screen.get() != Screen::Join {
        next_screen.set(wanted);
    }
    next_overlay.set(Overlay::None);
}

/// Tells the host of the tank that the player built, whenever a run in free mode is ahead
/// and the host doesn't know that tank: when the host turns free mode on, and when the player
/// changes the tank in their garage.
fn tell_of_the_tank(
    mut joined: ResMut<Joined>,
    garage: Res<Garage>,
    lobbies: Query<&Lobby>,
    mut words: Query<&mut MessageSender<Word>, With<Client>>,
) {
    let free = lobbies.single().is_ok_and(|lobby| lobby.free);
    if !joined.seated || !free || joined.told == Some(garage.tank) {
        return;
    }
    if let Ok(mut words) = words.get_mut(joined.client) {
        words.send::<Reliable>(Word::Tank(garage.tank));
        joined.told = Some(garage.tank);
    }
}

/// A connection that ended goes once its last words are out. What it received went when it
/// ended.
fn let_go(mut commands: Commands, time: Res<Time<Real>>, mut gone: Query<(Entity, &mut Leaving)>) {
    for (client, mut leaving) in &mut gone {
        leaving.0 -= time.delta_secs();
        if leaving.0 <= 0.0 {
            commands.entity(client).despawn();
        }
    }
}

/// The player's own tank is the one that this machine commands over the network.
fn take_the_controls(
    mut commands: Commands,
    tanks: Query<(Entity, Has<Own>, Has<InputMarker<TankCommand>>), With<Player>>,
) {
    for (entity, own, commanded) in &tanks {
        match (own, commanded) {
            (true, false) => {
                commands
                    .entity(entity)
                    .insert(InputMarker::<TankCommand>::default());
            }
            (false, true) => {
                commands.entity(entity).remove::<InputMarker<TankCommand>>();
            }
            _ => {}
        }
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

/// What happens in the simulation of the host, for the effects and the sounds here. What
/// this machine has shown already, ahead of its host, doesn't show again.
fn hear(
    timeline: Res<LocalTimeline>,
    mut receivers: Query<&mut MessageReceiver<SimEvent>, With<Client>>,
    mut echoes: ResMut<mirror::Echoes>,
    mut events: MessageWriter<SimEvent>,
) {
    let now = timeline.tick().0;
    for mut receiver in &mut receivers {
        for event in receiver.receive() {
            if !echoes.claims(now, &event) {
                events.write(event);
            }
        }
    }
}

/// The run of the host is the run that this machine shows.
fn follow_run(mut commands: Commands, told: Query<&RunState>, run: Option<ResMut<Run>>) {
    match (told.single(), run) {
        (Ok(state), Some(mut run)) => {
            if *run != state.0 {
                *run = state.0.clone();
            }
        }
        (Ok(state), None) => commands.insert_resource(state.0.clone()),
        (Err(_), Some(_)) => commands.remove_resource::<Run>(),
        (Err(_), None) => {}
    }
}

/// The host only tells which sectors are loaded. Their walls and all they hold come from the
/// seed, which gives the same on every machine. A run in arena mode has the room of its seed
/// and nothing else.
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
        if run.arena {
            let layout = Room::new(run.seed).shell();
            commands.entity(entity).insert(Sector { layout });
            continue;
        }
        let known = corridor.take().filter(|(seed, _)| *seed == run.seed);
        let (seed, mut planned) = known.unwrap_or_else(|| (run.seed, Corridor::new(run.seed)));
        let layout = planned.layout(index.0);
        commands.entity(entity).insert(Sector { layout });
        *corridor = Some((seed, planned));
    }
}

/// Shows what the host is at: its lobby, or the run it started.
fn follow_host(
    joined: Res<Joined>,
    run: Option<Res<Run>>,
    screen: Res<State<Screen>>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut next_overlay: ResMut<NextState<Overlay>>,
) {
    if !joined.seated {
        return;
    }
    let in_progress = run.as_ref().is_some_and(|run| run.phase != RunPhase::Over);
    // A run may start while the player is in their garage.
    let wanted = match (*screen.get(), run.is_some()) {
        (Screen::Lobby | Screen::Garage | Screen::GameOver, _) if in_progress => Screen::Playing,
        (Screen::Playing | Screen::GameOver, false) => Screen::Lobby,
        _ => return,
    };
    next_screen.set(wanted);
    next_overlay.set(Overlay::None);
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod tests {
    use std::net::UdpSocket;

    use bevy::state::app::StatesPlugin;
    use lightyear::link::LinkSystems;
    use tanku_core::modes::{Difficulty, Unlock};
    use tanku_core::{
        CorePlugin,
        sim::{Position, Tank, Wreck},
        stats::{Genome, Stat},
    };
    use tanku_core::{discovery::Reach, sim::RunRequest};
    use tanku_server::{
        ServerPlugin,
        net::{Built, Hosting, Seat},
    };

    use super::*;
    use crate::{
        cli::Options,
        crew,
        host::{Countdown, HostPort, WebPort},
        session::{self, Sign},
        unlocks::Unlocks,
    };

    /// The game without its drawings: what plays a run, alone or with others.
    pub(crate) fn game() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, CorePlugin, ServerPlugin))
            .init_state::<Screen>()
            .init_state::<Overlay>()
            .add_plugins((crew::plugin, session::plugin, NetPlugin))
            // Tests don't wait long for a game that isn't there, and keep to themselves:
            // nothing is announced to the machines around, and nothing is asked of them.
            .insert_resource(Patience(1))
            .insert_resource(Reach::Nowhere)
            // And they take no browsers, unless they are about taking them.
            .insert_resource(WebPort(None));
        // What `App::run` does before the first frame: the network library sets itself up there.
        app.finish();
        app.cleanup();
        app
    }

    /// A port that is free on this machine, which no other test is given and which the
    /// system hands to nobody.
    pub(crate) use tanku_core::testing::free_port;

    /// Runs the games for a while, in real time, until something is true of them.
    pub(crate) fn run_until(
        what: &str,
        games: &mut [&mut App],
        done: impl Fn(&mut [&mut App]) -> bool,
    ) {
        for _ in 0..2500 {
            for game in games.iter_mut() {
                game.update();
            }
            if done(games) {
                return;
            }
            std::thread::sleep(Duration::from_millis(4));
        }
        let told: Vec<String> = games.iter_mut().map(|game| describe(game)).collect();
        panic!("the games never got there: {what}.\n{}", told.join("\n"));
    }

    fn describe(game: &mut App) -> String {
        let world = game.world_mut();
        let tanks = world.query::<&Tank>().iter(world).count();
        let sectors = world.query::<&Sector>().iter(world).count();
        let lobby = world.query::<&Lobby>().iter(world).next().cloned();
        format!(
            "{:?} on {:?}, seat {}, {tanks} tanks, {sectors} sectors, run {:?}, notice {:?}, \
             lobby {lobby:?}",
            *world.resource::<Session>(),
            **world.resource::<State<Screen>>(),
            world.resource::<LocalSeat>().0,
            world
                .get_resource::<Run>()
                .map(|run| (run.players, run.phase)),
            world.resource::<Notice>().0,
        )
    }

    pub(crate) fn screen(game: &App) -> Screen {
        **game.world().resource::<State<Screen>>()
    }

    pub(crate) fn ask(game: &mut App, request: SessionRequest) {
        game.world_mut().write_message(request);
    }

    /// Opens a game and gives its seed, as the host's menus do.
    pub(crate) fn host(port: u16) -> App {
        host_with(port, None)
    }

    /// Opens a game that takes browsers on a port too, if one is given.
    pub(crate) fn host_with(port: u16, web: Option<u16>) -> App {
        let mut game = game();
        game.insert_resource(HostPort(port))
            .insert_resource(WebPort(web));
        ask(&mut game, SessionRequest::Host(default()));
        run_until("a lobby", &mut [&mut game], |games| {
            screen(games[0]) == Screen::Lobby
        });
        let world = game.world_mut();
        let mut lobby = world.query::<&mut Lobby>().single_mut(world).unwrap();
        lobby.seed = "NET".to_string();
        game
    }

    /// Joins a game on this machine, as the menus of a guest do.
    pub(crate) fn guest(port: u16) -> App {
        let mut game = game();
        ask(&mut game, SessionRequest::Join(format!("127.0.0.1:{port}")));
        game
    }

    /// The seats of the players' tanks that a game holds, in order.
    pub(crate) fn seats(game: &mut App) -> Vec<u8> {
        let world = game.world_mut();
        let mut seats: Vec<u8> = world
            .query::<&Player>()
            .iter(world)
            .map(|player| player.seat)
            .collect();
        seats.sort_unstable();
        seats
    }

    /// Where the tank of a seat is.
    pub(crate) fn place(game: &mut App, seat: u8) -> Vec2 {
        let world = game.world_mut();
        let mut tanks = world.query::<(&Player, &Position)>();
        let tank = tanks.iter(world).find(|(player, _)| player.seat == seat);
        tank.unwrap().1.0
    }

    /// Asks something of the tank of this game's own player.
    pub(crate) fn order(game: &mut App, command: TankCommand) {
        let world = game.world_mut();
        let mut tanks = world.query_filtered::<&mut TankCommand, With<Own>>();
        if let Ok(mut asked) = tanks.single_mut(world) {
            *asked = command;
        }
    }

    #[test]
    fn addresses_are_read_as_players_type_them() {
        let at = |text: &str| Target::Address(text.parse().unwrap());
        assert_eq!(target(" 192.168.1.20 "), Some(at("192.168.1.20:28405")));
        assert_eq!(target("10.0.0.7:4000"), Some(at("10.0.0.7:4000")));
        assert_eq!(
            target("tanku.example.org"),
            Some(Target::Name("tanku.example.org".to_string(), PORT))
        );
        assert_eq!(
            target("localhost:4000"),
            Some(Target::Name("localhost".to_string(), 4000))
        );
        for junk in [
            "",
            "   ",
            "10.0.0.7:0",
            "host:port",
            "two words",
            "[::1]:28405",
        ] {
            assert_eq!(target(junk), None, "{junk}");
        }
        // What a lobby shows is what a guest types.
        assert_eq!(written("192.168.1.20", PORT), "192.168.1.20");
        assert_eq!(written("192.168.1.20", 4000), "192.168.1.20:4000");
    }

    #[test]
    fn what_a_host_hands_out_is_read_from_the_end_of_a_link() {
        let print = "0123456789abcdef".repeat(4);
        let at = |text: &str| Some(Target::Web(text.parse().unwrap(), print.clone()));
        assert_eq!(
            target(&format!("10.0.0.7:4000/{print}")),
            at("10.0.0.7:4000")
        );
        // What a host hands out is read as it was written.
        let handed = web_code("10.0.0.7", 4000, &print);
        assert_eq!(target(&handed), at("10.0.0.7:4000"));
        // It comes at the end of a link, and a browser hands that end over with its mark.
        let link = format!("https://example.org/tanku/#{handed}");
        assert_eq!(target(&link), at("10.0.0.7:4000"));
        assert_eq!(target(&format!(" #{handed} ")), at("10.0.0.7:4000"));
        // The port is left out when it is the usual one, and capitals are the same print.
        assert_eq!(target(&format!("10.0.0.7/{print}")), at("10.0.0.7:28406"));
        let capitals = format!("10.0.0.7:4000/{}", print.to_uppercase());
        assert_eq!(target(&capitals), at("10.0.0.7:4000"));
        // A print that is short, or that is none, leads nowhere, and neither does a name.
        for junk in [
            format!("10.0.0.7:4000/{}", &print[1..]),
            format!("10.0.0.7:4000/{}g", &print[1..]),
            format!("10.0.0.7:0/{print}"),
            format!("example.org/{print}"),
            "10.0.0.7:4000/".to_string(),
        ] {
            assert_eq!(target(&junk), None, "{junk}");
        }
    }

    #[test]
    fn a_guest_joins_a_lobby_plays_a_run_and_leaves() {
        let port = free_port();
        let mut host = host(port);
        let mut guest = guest(port);

        // The guest takes the seat after the host's, and both see who is there.
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        assert_eq!(*guest.world().resource::<Session>(), Session::Guest);
        assert_eq!(guest.world().resource::<LocalSeat>().0, 1);
        for game in [&mut host, &mut guest] {
            let world = game.world_mut();
            let lobby = world.query::<&Lobby>().single(world).unwrap();
            assert_eq!(lobby.players(), 2);
            assert_eq!(lobby.seats[0], Some(Member::Host));
            assert_eq!(lobby.seed, "NET");
        }

        // The host starts the run, and it starts for both.
        ask(&mut host, SessionRequest::Start);
        run_until("the run", &mut [&mut host, &mut guest], |games| {
            let sectors = |game: &mut App| {
                let world = game.world_mut();
                world.query::<&Sector>().iter(world).count()
            };
            screen(games[1]) == Screen::Playing
                && seats(games[1]) == [0, 1]
                && sectors(games[1]) == 2
        });
        assert_eq!(screen(&host), Screen::Playing);
        let run = guest.world().resource::<Run>().clone();
        assert_eq!(
            (run.seed_text.as_str(), run.players, run.solo),
            ("NET", 2, false)
        );

        // Each player drives their own tank: the host's goes east and the guest's north.
        let (host_start, guest_start) = (place(&mut host, 0), place(&mut host, 1));
        run_until("the drive", &mut [&mut host, &mut guest], |games| {
            let drive = |drive| TankCommand { drive, ..default() };
            order(games[0], drive(Vec2::X));
            order(games[1], drive(Vec2::Y));
            let moved = |game: &mut App| {
                let east = place(game, 0).x - host_start.x;
                let north = place(game, 1).y - guest_start.y;
                east > 1.0 && north > 1.0
            };
            moved(games[0]) && moved(games[1])
        });
        // Neither tank did what the other was asked.
        assert!(place(&mut host, 0).y - host_start.y < 0.5);
        assert!(place(&mut host, 1).x - guest_start.x < 0.5);

        // The guest leaves. Their tank goes, and the run goes on for the host alone.
        ask(&mut guest, SessionRequest::Leave);
        run_until("the departure", &mut [&mut host, &mut guest], |games| {
            seats(games[0]) == [0] && seats(games[1]).is_empty()
        });
        assert_eq!(host.world().resource::<Run>().players, 1);
        assert_eq!(*guest.world().resource::<Session>(), Session::Solo);
        assert!(guest.world().get_resource::<Run>().is_none());
        let world = guest.world_mut();
        assert_eq!(world.query::<&Sector>().iter(world).count(), 0);
    }

    /// The lobby that a game holds, its own or the one that its host told it of.
    pub(crate) fn lobby(game: &mut App) -> Lobby {
        let world = game.world_mut();
        world.query::<&Lobby>().single(world).unwrap().clone()
    }

    #[test]
    fn a_guest_is_told_the_name_that_the_host_gave_the_game() {
        let port = free_port();
        let mut host = game();
        host.insert_resource(HostPort(port));
        // A name is kept as a host could have typed it, whatever asks for the game.
        let sign = Sign {
            name: "  Friday night\t ".into(),
            ..default()
        };
        ask(&mut host, SessionRequest::Host(sign));
        let mut guest = guest(port);
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        assert_eq!(lobby(&mut host).name, "Friday night");
        assert_eq!(lobby(&mut guest).name, "Friday night");
    }

    /// The game whose password a game asks its player for, if it asks.
    fn asked(game: &App) -> Option<Shut> {
        game.world().resource::<Asked>().0.clone()
    }

    fn notice(game: &App) -> Option<String> {
        game.world().resource::<Notice>().0.clone()
    }

    /// How many times a game was told of a lobby.
    #[derive(Resource, Default)]
    struct Lobbies(u32);

    /// What a host said at its door to a guest that a test speaks for.
    #[derive(Resource, Default)]
    struct Heard(Vec<Door>);

    #[test]
    fn a_game_that_has_a_password_lets_in_only_those_who_type_it() {
        let port = free_port();
        let mut host = game();
        host.insert_resource(HostPort(port));
        let sign = Sign {
            name: "Friday".into(),
            password: "open sesame".into(),
            ..default()
        };
        ask(&mut host, SessionRequest::Host(sign));
        let at = format!("127.0.0.1:{port}");
        let mut guest = guest(port);
        guest
            .init_resource::<Lobbies>()
            .add_observer(|_: On<Add, Lobby>, mut told: ResMut<Lobbies>| told.0 += 1);

        // Whoever comes without the password is asked for it, and is not in.
        run_until("the question", &mut [&mut host, &mut guest], |games| {
            asked(games[1]).is_some()
        });
        let shut = asked(&guest).unwrap();
        assert_eq!((shut.at, shut.address), (at.clone(), at.clone()));
        assert_eq!(notice(&guest), None);
        assert_eq!(*guest.world().resource::<Session>(), Session::Solo);
        assert_eq!(screen(&guest), Screen::Join);

        // A password that is not the one is refused, and the player is told so.
        let wrong = SessionRequest::JoinLocked(at.clone(), "open sesam".into());
        ask(&mut guest, wrong);
        run_until("the refusal", &mut [&mut host, &mut guest], |games| {
            notice(games[1]).is_some()
        });
        assert_eq!(notice(&guest).as_deref(), Some(WRONG_PASSWORD));
        assert!(asked(&guest).is_some());
        assert_eq!(*guest.world().resource::<Session>(), Session::Solo);
        // Nobody got in, and the guest was told nothing of the game.
        let since = std::time::Instant::now();
        run_until("a while", &mut [&mut host, &mut guest], |_| {
            since.elapsed().as_secs_f32() > 0.5
        });
        assert_eq!(lobby(&mut host).players(), 1);
        assert_eq!(guest.world().resource::<Lobbies>().0, 0);

        // The password lets the guest in, typed as the host typed it or with spaces around.
        let right = SessionRequest::JoinLocked(at, " open sesame ".into());
        ask(&mut guest, right);
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        assert_eq!(guest.world().resource::<Lobbies>().0, 1);
        assert_eq!((asked(&guest), notice(&guest)), (None, None));
        for game in [&mut host, &mut guest] {
            let lobby = lobby(game);
            assert_eq!((lobby.players(), lobby.locked), (2, true));
        }
    }

    #[test]
    fn the_password_is_asked_the_way_browsers_come_too() {
        let (port, web) = (free_port(), free_port());
        let mut host = game();
        host.insert_resource(HostPort(port))
            .insert_resource(WebPort(Some(web)));
        let sign = Sign {
            name: String::new(),
            password: "sesame".into(),
            ..default()
        };
        ask(&mut host, SessionRequest::Host(sign));
        run_until("a lobby", &mut [&mut host], |games| {
            screen(games[0]) == Screen::Lobby
        });
        // What the host hands out carries no password: whoever follows it is asked.
        let link = code(&host);
        assert!(!link.contains("sesame"));
        let mut guest = game();
        ask(&mut guest, SessionRequest::Join(link.clone()));
        run_until("the question", &mut [&mut host, &mut guest], |games| {
            asked(games[1]).is_some()
        });
        // The game is joined again by what the host handed out, with the password.
        assert_eq!(asked(&guest).unwrap().at, link);
        ask(
            &mut guest,
            SessionRequest::JoinLocked(link, "sesame".into()),
        );
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        assert_eq!(lobby(&mut guest).players(), 2);
    }

    #[test]
    fn a_guest_at_the_door_gets_one_answer_and_nothing_of_the_game() {
        let port = free_port();
        let mut host = game();
        host.insert_resource(HostPort(port));
        let sign = Sign {
            name: String::new(),
            password: "sesame".into(),
            ..default()
        };
        ask(&mut host, SessionRequest::Host(sign));
        // A guest that doesn't do as the game does: it stays at the door when it is turned
        // away, and answers again. The game is told to forget it, so that this test speaks
        // to the host in its place.
        let mut guest = guest(port);
        guest.update();
        let joined = guest.world_mut().remove_resource::<Joined>().unwrap();
        let (client, id) = (joined.client, joined.id);
        // What the host says is there for one frame: the guest notes it as it comes.
        fn listen(mut doors: Query<&mut MessageReceiver<Door>>, mut heard: ResMut<Heard>) {
            for mut door in &mut doors {
                heard.0.extend(door.receive());
            }
        }
        guest
            .init_resource::<Heard>()
            .add_systems(PreUpdate, listen.after(MessageSystems::Receive));
        let hears = |guest: &mut App| -> Vec<Door> {
            std::mem::take(&mut guest.world_mut().resource_mut::<Heard>().0)
        };
        let says = |guest: &mut App, proof| {
            let mut client = guest.world_mut().entity_mut(client);
            let mut words = client.get_mut::<MessageSender<Word>>().unwrap();
            words.send::<Reliable>(Word::Proof(proof));
        };
        let step = |host: &mut App, guest: &mut App| {
            host.update();
            guest.update();
            std::thread::sleep(Duration::from_millis(4));
        };
        let mut challenge = None;
        for _ in 0..1000 {
            step(&mut host, &mut guest);
            if let Some(Door::Locked(asked)) = hears(&mut guest).first() {
                challenge = Some(*asked);
                break;
            }
        }
        let challenge = challenge.expect("a challenge");
        // A wrong answer is refused.
        says(&mut guest, prove(challenge, id, "sesamf"));
        let mut refused = false;
        for _ in 0..1000 {
            step(&mut host, &mut guest);
            refused |= hears(&mut guest).contains(&Door::Refused);
            if refused {
                break;
            }
        }
        assert!(refused);
        // The right one, to the same challenge, comes too late: a challenge takes one
        // answer, so that nobody tries a list of passwords at one door.
        says(&mut guest, prove(challenge, id, "sesame"));
        for _ in 0..150 {
            step(&mut host, &mut guest);
        }
        assert_eq!(lobby(&mut host).players(), 1);
        let world = host.world_mut();
        assert_eq!(world.query::<&Seat>().iter(world).count(), 0);
        // All that time at the door, the guest was told nothing of the game.
        let world = guest.world_mut();
        assert_eq!(world.query::<&Lobby>().iter(world).count(), 0);
        assert!(hears(&mut guest).is_empty());
    }

    /// The tank of a seat in the run that a game plays or shows.
    fn tank(game: &mut App, seat: u8) -> Genome {
        let world = game.world_mut();
        let mut tanks = world.query::<(&Player, &Tank)>();
        let tank = tanks.iter(world).find(|(player, _)| player.seat == seat);
        tank.unwrap().1.genome
    }

    /// Sets the lobby of a host as its menus do.
    fn set(host: &mut App, change: impl FnOnce(&mut Lobby)) {
        let world = host.world_mut();
        change(&mut world.query::<&mut Lobby>().single_mut(world).unwrap());
    }

    /// A tank that a player builds in their garage: a fast one with a fast gun.
    fn racer() -> Genome {
        let parts = [(Stat::Engine, 7), (Stat::Muzzle, 7), (Stat::Loader, 6)];
        Genome::of_the_garage(&parts).unwrap()
    }

    /// Another one: a slow tank whose bullets pierce and bounce three times.
    fn bouncer() -> Genome {
        let parts = [(Stat::Engine, 1), (Stat::Ricochet, 3), (Stat::Piercing, 1)];
        Genome::of_the_garage(&parts).unwrap()
    }

    /// Waits until the run of a host has started for its guest too, with a tank for each.
    fn until_both_play(host: &mut App, guest: &mut App) {
        run_until("the run", &mut [host, guest], |games| {
            screen(games[1]) == Screen::Playing && seats(games[1]) == [0, 1]
        });
    }

    #[test]
    fn in_free_mode_each_player_has_the_tank_that_they_built() {
        let port = free_port();
        let mut host = host(port);
        host.insert_resource(Garage { tank: bouncer() });
        let mut guest = guest(port);
        guest.insert_resource(Garage { tank: racer() });
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        // Outside free mode, nobody tells of a tank, and everyone has the standard one.
        let built = |host: &mut App| -> Vec<Genome> {
            let world = host.world_mut();
            let mut built = world.query::<&Built>();
            built.iter(world).map(|built| built.0).collect()
        };
        ask(&mut host, SessionRequest::Start);
        until_both_play(&mut host, &mut guest);
        assert!(built(&mut host).is_empty());
        for game in [&mut host, &mut guest] {
            assert_eq!(
                (tank(game, 0), tank(game, 1)),
                (Genome::PLAYER, Genome::PLAYER)
            );
            assert!(!game.world().resource::<Run>().free);
        }

        // The host turns free mode on in the lobby: the guest tells of their tank.
        host.world_mut().write_message(RunRequest::Abandon);
        let mut next = host.world_mut().resource_mut::<NextState<Screen>>();
        next.set(Screen::Lobby);
        run_until("the lobby again", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        set(&mut host, |lobby| lobby.free = true);
        run_until("the tank", &mut [&mut host, &mut guest], |games| {
            built(games[0]) == [racer()]
        });
        // The guest changes it in their garage, and the host is told again.
        let slower = Genome::of_the_garage(&[(Stat::Engine, 2), (Stat::Muzzle, 7)]).unwrap();
        guest.insert_resource(Garage { tank: slower });
        run_until("the other tank", &mut [&mut host, &mut guest], |games| {
            built(games[0]) == [slower]
        });
        // The run is in free mode, each player in their own tank, on both machines. It
        // starts for a guest who is still in their garage.
        let mut next = guest.world_mut().resource_mut::<NextState<Screen>>();
        next.set(Screen::Garage);
        run_until("the garage", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Garage
        });
        ask(&mut host, SessionRequest::Start);
        until_both_play(&mut host, &mut guest);
        for game in [&mut host, &mut guest] {
            assert_eq!((tank(game, 0), tank(game, 1)), (bouncer(), slower));
            let run = game.world().resource::<Run>();
            assert!(run.free && !run.earns);
        }
    }

    #[test]
    fn a_tank_that_no_player_may_build_is_not_taken() {
        let port = free_port();
        let mut host = host(port);
        // A guest whose game claims a tank with a brain, at a price that no garage allows.
        let mut cheat = Genome::empty(200);
        for stat in Stat::GARAGE.into_iter().chain([Stat::Brain]) {
            cheat.set(stat, stat.max_level());
        }
        assert!(!cheat.is_a_players());
        let mut guest = guest(port);
        guest.insert_resource(Garage { tank: cheat });
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        set(&mut host, |lobby| lobby.free = true);
        // The guest tells of it, and the host keeps none of it.
        run_until("the claim", &mut [&mut host, &mut guest], |games| {
            games[1].world().resource::<Joined>().told == Some(cheat)
        });
        let since = std::time::Instant::now();
        run_until("a while", &mut [&mut host, &mut guest], |_| {
            since.elapsed().as_secs_f32() > 0.3
        });
        let world = host.world_mut();
        assert_eq!(world.query::<&Built>().iter(world).count(), 0);
        ask(&mut host, SessionRequest::Start);
        until_both_play(&mut host, &mut guest);
        assert_eq!(tank(&mut host, 1), Genome::PLAYER);
    }

    #[test]
    fn a_run_is_as_the_host_set_it_in_the_lobby() {
        let port = free_port();
        let mut host = host(port);
        let mut guest = guest(port);
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        // The guest is told what the run is set to.
        set(&mut host, |lobby| {
            lobby.difficulty = Difficulty::Hard;
            lobby.seed = "FRIDAY".into();
            lobby.typed = true;
        });
        run_until("the options", &mut [&mut host, &mut guest], |games| {
            let lobby = lobby(games[1]);
            (lobby.difficulty, lobby.seed.as_str(), lobby.typed)
                == (Difficulty::Hard, "FRIDAY", true)
        });
        // A run on a seed that the host typed earns nothing, though no run was played on
        // it: it is a chosen seed.
        host.insert_resource(session::Drawn(true));
        ask(&mut host, SessionRequest::Start);
        until_both_play(&mut host, &mut guest);
        for game in [&mut host, &mut guest] {
            let run = game.world().resource::<Run>();
            assert_eq!(
                (run.seed_text.as_str(), run.difficulty, run.earns, run.free),
                ("FRIDAY", Difficulty::Hard, false, false)
            );
        }
        // The next one is on a seed that the game drew, in normal mode: it earns.
        host.world_mut().write_message(RunRequest::Abandon);
        let mut next = host.world_mut().resource_mut::<NextState<Screen>>();
        next.set(Screen::Lobby);
        run_until("the lobby again", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        set(&mut host, |lobby| {
            lobby.difficulty = Difficulty::Normal;
            lobby.seed = "DRAWN".into();
            lobby.typed = false;
        });
        host.insert_resource(session::Drawn(true));
        ask(&mut host, SessionRequest::Start);
        until_both_play(&mut host, &mut guest);
        let run = guest.world().resource::<Run>();
        assert_eq!(
            (run.seed_text.as_str(), run.difficulty, run.earns),
            ("DRAWN", Difficulty::Normal, true)
        );
    }

    /// Who is ready in the lobby that a game holds or was told of, and what it counts down.
    fn ready(game: &mut App) -> ([bool; 4], Option<u8>) {
        let lobby = lobby(game);
        (lobby.ready, lobby.countdown)
    }

    /// Lets the games go on for so many seconds.
    fn a_while(games: &mut [&mut App], seconds: f32) {
        let since = std::time::Instant::now();
        run_until("a while", games, |_| {
            since.elapsed().as_secs_f32() > seconds
        });
    }

    /// Brings the players of a run back to the lobby, as the host does from its menu.
    fn regroup(host: &mut App, guest: &mut App) {
        host.world_mut().write_message(RunRequest::Abandon);
        let mut next = host.world_mut().resource_mut::<NextState<Screen>>();
        next.set(Screen::Lobby);
        run_until("the lobby again", &mut [host, guest], |games| {
            screen(games[0]) == Screen::Lobby && screen(games[1]) == Screen::Lobby
        });
    }

    #[test]
    fn a_run_starts_by_itself_once_everyone_is_ready() {
        let port = free_port();
        let mut host = host(port);
        let mut guest = guest(port);
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        // Nobody is ready at first. The host says that they are: everyone is told, and
        // the run waits for the guest.
        assert_eq!(ready(&mut guest), ([false; 4], None));
        ask(&mut host, SessionRequest::Ready(true));
        run_until("the host's word", &mut [&mut host, &mut guest], |games| {
            ready(games[1]).0 == [true, false, false, false]
        });
        a_while(&mut [&mut host, &mut guest], 0.6);
        assert_eq!(ready(&mut guest), ([true, false, false, false], None));
        assert_eq!(screen(&host), Screen::Lobby);

        // The guest says so too: the lobby counts down from three on both machines, and
        // the run starts by itself when the count is over.
        ask(&mut guest, SessionRequest::Ready(true));
        run_until("the countdown", &mut [&mut host, &mut guest], |games| {
            let counting = ([true, true, false, false], Some(3));
            ready(games[0]) == counting && ready(games[1]) == counting
        });
        let counting_since = std::time::Instant::now();
        run_until("the run", &mut [&mut host, &mut guest], |games| {
            screen(games[0]) == Screen::Playing && screen(games[1]) == Screen::Playing
        });
        let counted = counting_since.elapsed().as_secs_f32();
        assert!(
            (2.5..4.0).contains(&counted),
            "the run started {counted} s after the count did"
        );
        assert_eq!(guest.world().resource::<Run>().players, 2);

        // Back in the lobby after the run, nobody is ready until they say so again.
        regroup(&mut host, &mut guest);
        a_while(&mut [&mut host, &mut guest], 0.3);
        assert_eq!(ready(&mut host), ([false; 4], None));
        assert_eq!(ready(&mut guest), ([false; 4], None));
    }

    #[test]
    fn a_word_that_is_taken_back_stops_the_countdown() {
        let port = free_port();
        let mut host = host(port);
        host.insert_resource(Countdown(1.0));
        let mut guest = guest(port);
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        ask(&mut host, SessionRequest::Ready(true));
        ask(&mut guest, SessionRequest::Ready(true));
        run_until("the countdown", &mut [&mut host, &mut guest], |games| {
            ready(games[1]).1.is_some()
        });
        // The guest takes their word back before the count is over: it stops, and no run
        // starts however long the host stays ready.
        ask(&mut guest, SessionRequest::Ready(false));
        run_until("the stop", &mut [&mut host, &mut guest], |games| {
            ready(games[0]) == ([true, false, false, false], None)
        });
        a_while(&mut [&mut host, &mut guest], 1.5);
        assert_eq!(screen(&host), Screen::Lobby);
        assert!(host.world().get_resource::<Run>().is_none());
        // The count starts over when they are ready again, and the run starts after it.
        ask(&mut guest, SessionRequest::Ready(true));
        let since = std::time::Instant::now();
        until_both_play(&mut host, &mut guest);
        assert!(since.elapsed().as_secs_f32() > 0.9);
    }

    #[test]
    fn a_change_to_the_run_or_to_a_tank_takes_words_back() {
        let port = free_port();
        let mut host = host(port);
        // A count that is never over: this test looks at who is ready.
        host.insert_resource(Countdown(600.0));
        let mut guest = guest(port);
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        let both_ready = |host: &mut App, guest: &mut App| {
            ask(host, SessionRequest::Ready(true));
            ask(guest, SessionRequest::Ready(true));
            run_until("both words", &mut [host, guest], |games| {
                ready(games[0]).0 == [true, true, false, false]
            });
        };
        let until_ready = |host: &mut App, guest: &mut App, wanted: [bool; 2]| {
            run_until("the words", &mut [host, guest], |games| {
                let told = |game: &mut App| ready(game).0;
                let wanted = [wanted[0], wanted[1], false, false];
                told(games[0]) == wanted && told(games[1]) == wanted
            });
        };
        // When the host changes what the run is set to, nobody is ready any more: its
        // difficulty, its seed, its mode.
        let changes: [fn(&mut Lobby); 4] = [
            |lobby| lobby.difficulty = Difficulty::Hard,
            |lobby| lobby.seed = "OTHER".into(),
            |lobby| lobby.typed = true,
            |lobby| lobby.free = true,
        ];
        for change in changes {
            both_ready(&mut host, &mut guest);
            set(&mut host, change);
            until_ready(&mut host, &mut guest, [false, false]);
        }
        // The run is in free mode by now. A player who changes their tank is no longer
        // ready, and the others still are: the guest, then the host.
        run_until("the tank", &mut [&mut host, &mut guest], |games| {
            let world = games[0].world_mut();
            world.query::<&Built>().iter(world).count() == 1
        });
        both_ready(&mut host, &mut guest);
        guest.insert_resource(Garage { tank: racer() });
        until_ready(&mut host, &mut guest, [true, false]);
        both_ready(&mut host, &mut guest);
        host.insert_resource(Garage { tank: bouncer() });
        until_ready(&mut host, &mut guest, [false, true]);
        // A garage that is opened and closed without a change takes nothing back, nor does
        // a guest's game that tells of the same tank again.
        both_ready(&mut host, &mut guest);
        guest.insert_resource(Garage { tank: racer() });
        host.insert_resource(Garage { tank: bouncer() });
        guest.world_mut().resource_mut::<Joined>().told = None;
        run_until("the tank, again", &mut [&mut host, &mut guest], |games| {
            games[1].world().resource::<Joined>().told.is_some()
        });
        a_while(&mut [&mut host, &mut guest], 0.4);
        assert_eq!(ready(&mut host).0, [true, true, false, false]);
    }

    #[test]
    fn whoever_joins_is_not_ready_and_whoever_moves_up_still_is() {
        let port = free_port();
        let mut host = host(port);
        host.insert_resource(Countdown(600.0));
        // A host who is alone and ready is on their way to a run of their own.
        ask(&mut host, SessionRequest::Ready(true));
        run_until("the countdown", &mut [&mut host], |games| {
            ready(games[0]).1.is_some()
        });
        // A player who joins isn't ready: the count stops.
        let mut first = guest(port);
        run_until("the first guest", &mut [&mut host, &mut first], |games| {
            screen(games[1]) == Screen::Lobby
                && ready(games[0]) == ([true, false, false, false], None)
        });
        // A second one comes and says that they are.
        let mut second = guest(port);
        let all = &mut [&mut host, &mut first, &mut second];
        run_until("the second guest", all, |games| {
            screen(games[2]) == Screen::Lobby
        });
        ask(&mut second, SessionRequest::Ready(true));
        let all = &mut [&mut host, &mut first, &mut second];
        run_until("their word", all, |games| {
            ready(games[0]) == ([true, false, true, false], None)
        });
        // The first one leaves: the second one moves up to their seat, and is still ready.
        // Everyone who is left is ready, so the lobby counts down.
        ask(&mut first, SessionRequest::Leave);
        let all = &mut [&mut host, &mut first, &mut second];
        run_until("the move", all, |games| {
            let told = ready(games[2]);
            own_seat(games[2]) == 1 && told.0 == [true, true, false, false] && told.1.is_some()
        });
    }

    #[test]
    fn a_host_who_is_alone_plays_a_solo_run_with_its_locks() {
        let port = free_port();
        let mut host = host(port);
        host.insert_resource(Countdown(0.3));
        // A host who has unlocked nothing sets the run to hard mode: alone, they can't be
        // ready for it.
        set(&mut host, |lobby| lobby.difficulty = Difficulty::Hard);
        a_while(&mut [&mut host], 0.1);
        ask(&mut host, SessionRequest::Ready(true));
        a_while(&mut [&mut host], 0.8);
        assert_eq!(ready(&mut host), ([false; 4], None));
        assert_eq!(screen(&host), Screen::Lobby);
        // A guest comes: nothing is locked for a host who has company.
        let mut guest = guest(port);
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        ask(&mut host, SessionRequest::Ready(true));
        run_until("the host's word", &mut [&mut host, &mut guest], |games| {
            ready(games[1]).0 == [true, false, false, false]
        });
        // The guest leaves again: the host is alone, and is no longer ready.
        ask(&mut guest, SessionRequest::Leave);
        run_until("the lock", &mut [&mut host, &mut guest], |games| {
            let lobby = lobby(games[0]);
            lobby.players() == 1 && lobby.ready == [false; 4]
        });
        a_while(&mut [&mut host], 0.6);
        assert_eq!(screen(&host), Screen::Lobby);
        // Once they have unlocked hard mode, the host starts alone, and plays a solo run.
        host.insert_resource(Unlocks::of(&[Unlock::HardMode]));
        ask(&mut host, SessionRequest::Ready(true));
        run_until("the run", &mut [&mut host], |games| {
            screen(games[0]) == Screen::Playing && seats(games[0]) == [0]
        });
        let run = host.world().resource::<Run>();
        assert_eq!(
            (run.players, run.solo, run.difficulty),
            (1, true, Difficulty::Hard)
        );
    }

    #[test]
    fn a_game_without_a_password_asks_for_none() {
        let port = free_port();
        let mut host = host(port);
        // A player who typed a password for it gets in all the same.
        let mut guest = game();
        let at = format!("127.0.0.1:{port}");
        ask(&mut guest, SessionRequest::JoinLocked(at, "sesame".into()));
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        assert!(!lobby(&mut guest).locked);
        assert_eq!(asked(&guest), None);
    }

    #[test]
    fn the_tank_that_a_playtest_brought_is_its_host_s_alone() {
        let parts = [(Stat::Engine, 7), (Stat::Muzzle, 7), (Stat::Piercing, 1)];
        let tank = Genome::of_the_garage(&parts).unwrap();
        let port = free_port();
        let mut host = host(port);
        host.insert_resource(Options {
            playtest: true,
            tank: Some(tank),
            ..default()
        });
        let mut guest = guest(port);
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        ask(&mut host, SessionRequest::Start);
        run_until("the run", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Playing && seats(games[1]) == [0, 1]
        });
        // Both games hold the tank of the host as it was built, and the standard tank for
        // the guest.
        for game in [&mut host, &mut guest] {
            let world = game.world_mut();
            let mut tanks = world.query::<(&Player, &Tank)>();
            let mut held: Vec<(u8, Genome)> = tanks
                .iter(world)
                .map(|(player, tank)| (player.seat, tank.genome))
                .collect();
            held.sort_by_key(|(seat, _)| *seat);
            assert_eq!(held, [(0, tank), (1, Genome::PLAYER)]);
        }
    }

    /// Destroys the tanks of all the players of a run, which ends it.
    pub(crate) fn wreck_everyone(host: &mut App) {
        let world = host.world_mut();
        let tanks: Vec<Entity> = world
            .query_filtered::<Entity, With<Player>>()
            .iter(world)
            .collect();
        let mut casualties = world.resource_mut::<tanku_server::Casualties>();
        casualties.tanks.extend(tanks);
    }

    pub(crate) fn over(game: &App) -> bool {
        let run = game.world().get_resource::<Run>();
        run.is_some_and(|run| run.phase == RunPhase::Over)
    }

    pub(crate) fn own_seat(game: &App) -> u8 {
        game.world().resource::<LocalSeat>().0
    }

    #[test]
    fn seats_stay_through_a_run_and_close_ranks_for_the_next_one() {
        let port = free_port();
        let mut host = host(port);
        let mut first = guest(port);
        run_until("the first guest", &mut [&mut host, &mut first], |games| {
            screen(games[1]) == Screen::Lobby
        });
        let mut second = guest(port);
        let all = &mut [&mut host, &mut first, &mut second];
        run_until("the second guest", all, |games| {
            screen(games[2]) == Screen::Lobby
        });
        assert_eq!((own_seat(&first), own_seat(&second)), (1, 2));

        ask(&mut host, SessionRequest::Start);
        let all = &mut [&mut host, &mut first, &mut second];
        run_until("the run", all, |games| seats(games[2]) == [0, 1, 2]);

        // The first guest leaves during the run. The second one keeps their seat and their
        // tank, and later sectors are for two players.
        ask(&mut first, SessionRequest::Leave);
        let all = &mut [&mut host, &mut first, &mut second];
        run_until("the departure", all, |games| {
            seats(games[0]) == [0, 2] && seats(games[2]) == [0, 2]
        });
        assert_eq!(host.world().resource::<Run>().players, 2);
        assert_eq!(own_seat(&second), 2);

        // The run ends and the host starts another. The seats have no gap any more, and the
        // guest who stayed drives the tank of their new seat.
        wreck_everyone(&mut host);
        run_until("the end", &mut [&mut host, &mut second], |games| {
            over(games[0]) && over(games[1])
        });
        ask(&mut host, SessionRequest::Start);
        run_until("the next run", &mut [&mut host, &mut second], |games| {
            !over(games[1]) && seats(games[1]) == [0, 1] && own_seat(games[1]) == 1
        });
        let run = host.world().resource::<Run>().clone();
        assert_eq!((run.players, run.team, run.solo), (2, 2, false));
        let start = place(&mut host, 1);
        run_until("the drive", &mut [&mut host, &mut second], |games| {
            let north = TankCommand {
                drive: Vec2::Y,
                ..default()
            };
            order(games[1], north);
            place(games[0], 1).y > start.y + 1.0
        });

        // Back in the lobby, the game takes players again.
        host.world_mut().write_message(RunRequest::Abandon);
        let mut next = host.world_mut().resource_mut::<NextState<Screen>>();
        next.set(Screen::Lobby);
        run_until("the lobby", &mut [&mut host, &mut second], |games| {
            screen(games[1]) == Screen::Lobby
        });
        let mut third = guest(port);
        let all = &mut [&mut host, &mut second, &mut third];
        run_until("the third guest", all, |games| {
            screen(games[2]) == Screen::Lobby
        });
        assert_eq!((own_seat(&second), own_seat(&third)), (1, 2));
    }

    /// Brings what a guest sends to the game on this machine, whatever address the guest
    /// asked for. That is what a network does between two machines: the address by which a
    /// guest finds its host is not one that the host's game knows as its own.
    fn reroute(guest: &mut App, port: u16) {
        let here = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
        let turn = move |mut peers: Query<&mut PeerAddr, With<Client>>| {
            for mut peer in &mut peers {
                peer.0 = here;
            }
        };
        guest.add_systems(PostUpdate, turn.before(LinkSystems::Send));
    }

    #[test]
    fn a_guest_joins_by_an_address_that_the_host_does_not_know_as_its_own() {
        let port = free_port();
        let mut host = host(port);
        let mut guest = game();
        reroute(&mut guest, port);
        // An address kept for documents, which no machine has.
        let asked = format!("192.0.2.1:{port}");
        ask(&mut guest, SessionRequest::Join(asked));
        run_until("an answer", &mut [&mut host, &mut guest], |games| {
            let refused = games[1].world().resource::<Notice>().0.is_some();
            refused || screen(games[1]) == Screen::Lobby
        });
        assert_eq!(guest.world().resource::<Notice>().0, None);
        assert_eq!(own_seat(&guest), 1);
    }

    /// What a host hands out for browsers, for one on its own machine.
    pub(crate) fn code(host: &App) -> String {
        let web = host.world().resource::<Hosting>().web().unwrap();
        web_code("127.0.0.1", web.port, &web.print)
    }

    #[test]
    fn a_guest_joins_the_way_browsers_do_and_plays() {
        let mut host = host_with(free_port(), Some(free_port()));
        let mut guest = game();
        ask(&mut guest, SessionRequest::Join(code(&host)));
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        assert_eq!(own_seat(&guest), 1);
        ask(&mut host, SessionRequest::Start);
        run_until("the run", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Playing && seats(games[1]) == [0, 1]
        });
        // Each player drives their own tank, and both see both.
        let (host_start, guest_start) = (place(&mut host, 0), place(&mut host, 1));
        run_until("the drive", &mut [&mut host, &mut guest], |games| {
            let drive = |drive| TankCommand { drive, ..default() };
            order(games[0], drive(Vec2::X));
            order(games[1], drive(Vec2::Y));
            let moved = |game: &mut App| {
                let east = place(game, 0).x - host_start.x;
                let north = place(game, 1).y - guest_start.y;
                east > 1.0 && north > 1.0
            };
            moved(games[0]) && moved(games[1])
        });
        // The host closes the game, and the guest is told.
        ask(&mut host, SessionRequest::Leave);
        run_until("the way home", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Menu && seats(games[1]).is_empty()
        });
        let told = guest.world().resource::<Notice>().0.clone().unwrap();
        assert_eq!(told, "The host closed the game.");
    }

    #[test]
    fn a_desktop_and_a_browser_play_the_same_game() {
        let port = free_port();
        let mut host = host_with(port, Some(free_port()));
        let mut desktop = guest(port);
        run_until("the first guest", &mut [&mut host, &mut desktop], |games| {
            screen(games[1]) == Screen::Lobby
        });
        let mut browser = game();
        ask(&mut browser, SessionRequest::Join(code(&host)));
        let all = &mut [&mut host, &mut desktop, &mut browser];
        run_until("the second guest", all, |games| {
            screen(games[2]) == Screen::Lobby
        });
        // They take the seats that follow the host's, whichever way they came.
        assert_eq!((own_seat(&desktop), own_seat(&browser)), (1, 2));

        ask(&mut host, SessionRequest::Start);
        let all = &mut [&mut host, &mut desktop, &mut browser];
        run_until("the run", all, |games| {
            games.iter_mut().all(|game| seats(game) == [0, 1, 2])
        });
        // Each guest drives its own tank, and each sees the other's go.
        let (first, second) = (place(&mut host, 1), place(&mut host, 2));
        let all = &mut [&mut host, &mut desktop, &mut browser];
        run_until("the drive", all, |games| {
            let drive = |drive| TankCommand { drive, ..default() };
            order(games[1], drive(-Vec2::X));
            order(games[2], drive(Vec2::Y));
            games.iter_mut().all(|game| {
                let west = first.x - place(game, 1).x;
                let north = place(game, 2).y - second.y;
                west > 1.0 && north > 1.0
            })
        });
        assert!((place(&mut host, 1).y - first.y).abs() < 0.5);
        assert!((place(&mut host, 2).x - second.x).abs() < 0.5);
    }

    #[test]
    fn a_run_in_progress_takes_nobody_the_way_browsers_come_either() {
        let mut host = host_with(free_port(), Some(free_port()));
        ask(&mut host, SessionRequest::Start);
        run_until("the run", &mut [&mut host], |games| {
            screen(games[0]) == Screen::Playing && seats(games[0]) == [0]
        });
        let mut late = game();
        ask(&mut late, SessionRequest::Join(code(&host)));
        run_until("the refusal", &mut [&mut host, &mut late], |games| {
            games[1].world().resource::<Notice>().0.is_some()
        });
        let told = late.world().resource::<Notice>().0.clone().unwrap();
        assert!(told.contains("its run has started"), "{told}");
        assert_eq!(*late.world().resource::<Session>(), Session::Solo);
        assert_eq!(seats(&mut host), [0]);
    }

    #[test]
    fn a_host_that_opens_a_game_again_hands_out_what_it_did_before() {
        let (port, web) = (free_port(), free_port());
        let mut host = host_with(port, Some(web));
        let handed = code(&host);
        // The game closes, and lets go of both its ports.
        ask(&mut host, SessionRequest::Leave);
        run_until("the ports", &mut [&mut host], |_| {
            [port, web]
                .iter()
                .all(|port| UdpSocket::bind(("0.0.0.0", *port)).is_ok())
        });
        ask(&mut host, SessionRequest::Host(default()));
        run_until("the lobby", &mut [&mut host], |games| {
            games[0].world().get_resource::<Hosting>().is_some()
        });
        assert_eq!(code(&host), handed);
        // Whoever was given it before joins the new game with it.
        let mut guest = game();
        ask(&mut guest, SessionRequest::Join(handed));
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        assert_eq!(own_seat(&guest), 1);
    }

    #[test]
    fn a_host_knows_at_once_that_a_guest_left_whichever_way_it_came() {
        for by_link in [false, true] {
            let port = free_port();
            let mut host = host_with(port, Some(free_port()));
            let mut guest = game();
            // With the patience of a real game, a host that isn't told waits 5 s.
            for game in [&mut host, &mut guest] {
                game.insert_resource(Patience(5));
            }
            let asked = if by_link {
                code(&host)
            } else {
                format!("127.0.0.1:{port}")
            };
            ask(&mut guest, SessionRequest::Join(asked));
            run_until("the lobby", &mut [&mut host, &mut guest], |games| {
                screen(games[1]) == Screen::Lobby
            });
            ask(&mut guest, SessionRequest::Leave);
            let left = std::time::Instant::now();
            run_until("the departure", &mut [&mut host, &mut guest], |games| {
                let world = games[0].world_mut();
                world.query::<&Seat>().iter(world).count() == 0
            });
            let took = left.elapsed();
            assert!(
                took < Duration::from_secs(2),
                "{took:?}, by a link: {by_link}"
            );
            // The guest has nothing left of the game, though its last words took a moment.
            let world = guest.world_mut();
            assert_eq!(world.query::<&Lobby>().iter(world).count(), 0);
        }
    }

    /// Why the connections of a game ended, in the order in which they did.
    #[derive(Resource, Default)]
    struct Endings(Vec<DisconnectedReason>);

    fn note_endings(
        trigger: On<Add, Disconnected>,
        ended: Query<&Disconnected>,
        mut endings: ResMut<Endings>,
    ) {
        if let Ok(ended) = ended.get(trigger.entity) {
            endings.0.push(ended.reason.clone());
        }
    }

    #[test]
    fn a_host_that_closes_its_game_tells_its_guests_before_it_stops_listening() {
        for by_link in [false, true] {
            let port = free_port();
            let mut host = host_with(port, Some(free_port()));
            let mut guest = game();
            guest.init_resource::<Endings>().add_observer(note_endings);
            let asked = if by_link {
                code(&host)
            } else {
                format!("127.0.0.1:{port}")
            };
            ask(&mut guest, SessionRequest::Join(asked));
            run_until("the lobby", &mut [&mut host, &mut guest], |games| {
                screen(games[1]) == Screen::Lobby
            });
            ask(&mut host, SessionRequest::Leave);
            run_until("the way home", &mut [&mut host, &mut guest], |games| {
                games[1].world().resource::<Notice>().0.is_some()
            });
            let told = guest.world().resource::<Notice>().0.clone().unwrap();
            assert_eq!(told, CLOSED, "by a link: {by_link}");
            // The guest left on the word of its host. Its connection had not been ended for
            // it, which is all that a guest in a browser would have seen of a host that
            // stopped listening at once.
            let endings = &guest.world().resource::<Endings>().0;
            let ended_for_it = |reason: &DisconnectedReason| {
                matches!(
                    reason,
                    DisconnectedReason::ByPeer(_) | DisconnectedReason::LinkFailed(_)
                )
            };
            assert!(!endings.iter().any(ended_for_it), "{endings:?}");
        }
    }

    #[test]
    fn a_guest_whose_lobby_goes_knows_that_its_host_closed_the_game() {
        let port = free_port();
        let mut host = host(port);
        let mut guest = guest(port);
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        // The lobby goes, and nothing else does: the host still listens.
        let lobby = host.world().resource::<Hosting>().game();
        host.world_mut().despawn(lobby);
        run_until("the way home", &mut [&mut host, &mut guest], |games| {
            games[1].world().resource::<Notice>().0.is_some()
        });
        let told = guest.world().resource::<Notice>().0.clone().unwrap();
        assert_eq!(told, CLOSED);
        assert_eq!(screen(&guest), Screen::Menu);
        assert_eq!(*guest.world().resource::<Session>(), Session::Solo);
        // The host is told that the guest left, as of any guest who leaves.
        run_until("the departure", &mut [&mut host, &mut guest], |games| {
            let world = games[0].world_mut();
            world.query::<&Seat>().iter(world).count() == 0
        });
    }

    #[test]
    fn a_print_that_is_not_the_one_of_the_game_joins_nothing() {
        let mut host = host_with(free_port(), Some(free_port()));
        let port = host.world().resource::<Hosting>().web().unwrap().port;
        let mut guest = game();
        let other = web_code("127.0.0.1", port, &"ab".repeat(PRINT_LENGTH / 2));
        ask(&mut guest, SessionRequest::Join(other));
        run_until("an answer", &mut [&mut host, &mut guest], |games| {
            let refused = games[1].world().resource::<Notice>().0.is_some();
            refused || screen(games[1]) == Screen::Lobby
        });
        assert_eq!(screen(&guest), Screen::Menu);
        assert_eq!(*guest.world().resource::<Session>(), Session::Solo);
        assert_eq!(seats(&mut host), Vec::<u8>::new());
        // The player is told where nothing answered, and what to ask the host for.
        let told = guest.world().resource::<Notice>().0.clone().unwrap();
        let wanted = format!("No game answers at 127.0.0.1:{port}. ");
        assert!(told.starts_with(&wanted), "{told}");
        assert!(
            told.ends_with("the one that its lobby shows now."),
            "{told}"
        );
    }

    /// Makes what a game receives come late, unevenly, and sometimes not at all: each way
    /// takes so many milliseconds, give or take some, and loses a share of what is sent.
    pub(crate) fn line(game: &mut App, late: u64, uneven: u64, lost: f32) {
        let line = LinkConditionerConfig::default()
            .with_incoming_latency(Duration::from_millis(late))
            .with_incoming_jitter(Duration::from_millis(uneven))
            .with_fixed_loss(lost);
        game.add_systems(First, move |mut links: Query<&mut Link>| {
            for mut link in &mut links {
                if link.recv.conditioner.is_none() {
                    link.recv.conditioner = Some(RecvLinkConditioner::new(line.clone()));
                }
            }
        });
    }

    #[test]
    fn a_run_holds_on_a_slow_line_that_loses_packets() {
        let port = free_port();
        // A line that takes 120 ms there and back.
        let mut host = host(port);
        line(&mut host, 60, 10, 0.02);
        let mut guest = game();
        line(&mut guest, 60, 10, 0.02);
        ask(
            &mut guest,
            SessionRequest::Join(format!("127.0.0.1:{port}")),
        );
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        ask(&mut host, SessionRequest::Start);
        run_until("the run", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Playing && seats(games[1]) == [0, 1]
        });

        // The guest drives, and sees its tank where the host has it.
        let start = place(&mut host, 1);
        run_until("the drive", &mut [&mut host, &mut guest], |games| {
            let north = TankCommand {
                drive: Vec2::Y,
                ..default()
            };
            order(games[1], north);
            place(games[0], 1).y > start.y + 3.0
        });
        order(&mut guest, TankCommand::default());
        run_until("the stop", &mut [&mut host, &mut guest], |games| {
            let (there, here) = (place(games[0], 1), place(games[1], 1));
            there.distance(here) < 0.01
        });

        // The run ends for both, and the guest leaves.
        wreck_everyone(&mut host);
        run_until("the end", &mut [&mut host, &mut guest], |games| {
            over(games[0]) && over(games[1])
        });
        ask(&mut guest, SessionRequest::Leave);
        run_until("the departure", &mut [&mut host, &mut guest], |games| {
            let world = games[0].world_mut();
            let guests = world.query::<&Seat>().iter(world).count();
            guests == 0 && seats(games[1]).is_empty()
        });
    }

    /// What a guest that was tampered with does: it asks the tank of the host to drive north,
    /// as if that tank were its own.
    fn usurp(mut commands: Commands, mut tanks: Query<(Entity, &Player, &mut TankCommand)>) {
        for (entity, player, mut command) in &mut tanks {
            if player.seat == 0 {
                command.drive = Vec2::Y;
                commands
                    .entity(entity)
                    .insert(InputMarker::<TankCommand>::default());
            }
        }
    }

    #[test]
    fn a_guest_commands_no_tank_but_its_own() {
        let port = free_port();
        let mut host = host(port);
        let mut guest = guest(port);
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        ask(&mut host, SessionRequest::Start);
        run_until("the run", &mut [&mut host, &mut guest], |games| {
            seats(games[1]) == [0, 1]
        });
        guest.add_systems(FixedPreUpdate, usurp.before(send_commands));

        // The guest's own tank obeys, which shows that its commands arrive. The host's stays.
        let (host_start, guest_start) = (place(&mut host, 0), place(&mut host, 1));
        run_until("the drive", &mut [&mut host, &mut guest], |games| {
            let east = TankCommand {
                drive: Vec2::X,
                ..default()
            };
            order(games[1], east);
            place(games[0], 1).x > guest_start.x + 2.0
        });
        assert!(place(&mut host, 0).distance(host_start) < 0.1);
    }

    #[test]
    fn nobody_joins_a_run_in_progress_and_a_closed_game_sends_its_guests_home() {
        let port = free_port();
        let mut host = host(port);
        let mut first = guest(port);
        run_until("the lobby", &mut [&mut host, &mut first], |games| {
            screen(games[1]) == Screen::Lobby
        });
        ask(&mut host, SessionRequest::Start);
        run_until("the run", &mut [&mut host, &mut first], |games| {
            screen(games[1]) == Screen::Playing
        });

        // Someone who comes during the run is turned away, and told why.
        let mut late = guest(port);
        run_until(
            "the refusal",
            &mut [&mut host, &mut first, &mut late],
            |games| games[2].world().resource::<Notice>().0.is_some(),
        );
        let told = late.world().resource::<Notice>().0.clone().unwrap();
        assert!(told.contains("its run has started"), "{told}");
        assert_eq!(*late.world().resource::<Session>(), Session::Solo);
        assert_eq!(seats(&mut host), [0, 1]);

        // The run ends when both tanks are wrecks, and the guest sees it end.
        wreck_everyone(&mut host);
        run_until("the end", &mut [&mut host, &mut first], |games| {
            let world = games[1].world_mut();
            let wrecks = world.query::<&Wreck>().iter(world).count();
            over(games[0]) && over(games[1]) && wrecks == 2
        });

        // The host closes the game: the guest is back at the menu, with nothing left of it.
        ask(&mut host, SessionRequest::Leave);
        run_until("the way home", &mut [&mut host, &mut first], |games| {
            screen(games[1]) == Screen::Menu && seats(games[1]).is_empty()
        });
        let told = first.world().resource::<Notice>().0.clone().unwrap();
        assert_eq!(told, "The host closed the game.");
        assert_eq!(*first.world().resource::<Session>(), Session::Solo);
        assert!(first.world().get_resource::<Run>().is_none());
        assert_eq!(*host.world().resource::<Session>(), Session::Solo);
        assert!(host.world().get_resource::<Hosting>().is_none());
    }

    #[test]
    fn a_game_that_is_not_there_does_not_answer() {
        let mut guest = guest(free_port());
        run_until("the silence", &mut [&mut guest], |games| {
            games[0].world().resource::<Notice>().0.is_some()
        });
        let told = guest.world().resource::<Notice>().0.clone().unwrap();
        assert!(told.starts_with("No game answers at 127.0.0.1:"), "{told}");
        assert_eq!(*guest.world().resource::<Session>(), Session::Solo);
        // What is not an address is said to be none at once.
        ask(&mut guest, SessionRequest::Join("over there".to_string()));
        guest.update();
        let told = guest.world().resource::<Notice>().0.clone().unwrap();
        assert!(told.starts_with("That is not an address"), "{told}");
    }

    #[test]
    fn a_port_serves_one_game_at_a_time() {
        let port = free_port();
        let _host = host(port);
        let mut second = game();
        second.insert_resource(HostPort(port));
        ask(&mut second, SessionRequest::Host(default()));
        second.update();
        let told = second.world().resource::<Notice>().0.clone().unwrap();
        assert!(told.contains("is taken on this machine"), "{told}");
        assert_eq!(*second.world().resource::<Session>(), Session::Solo);
        assert_eq!(screen(&second), Screen::Menu);
    }
}
