//! The network of a game that several players share, for a machine that joins one.
//!
//! A guest simulates nothing: it receives the lobby, the run and the simulation of its host,
//! and sends what its player asks of their tank. A desktop game joins at the address of its
//! host. A browser can't: it only speaks to a server that shows a certificate, and takes the
//! one that a host makes for itself only with the print of that certificate. So a browser
//! joins by what its host hands out, an address and a print. Hosting is in `host`, which a
//! browser doesn't have.

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
    map::Corridor,
    protocol::{Lobby, Member, PORT, ProtocolPlugin, RunState, WEB_PORT, protocol_id},
    rules::TICK_RATE,
    seed::VERSION,
    sim::{Player, Run, RunPhase, Sector, SectorIndex, SimEvent, TankCommand},
};
use tanku_server::field::Field;

use crate::{
    Overlay, Screen,
    crew::{LocalSeat, Own},
    mirror,
    session::{Notice, Session, SessionRequest},
};

/// How long a guest stays connected without a seat before it takes the game for full.
const SEAT_WAIT: f32 = 3.0;

/// How many seconds a guest and its host wait for each other before they call the connection
/// lost. It is also how long a guest tries to reach a game that doesn't answer.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Patience(pub i32);

/// The game that this machine joined, or is trying to.
#[derive(Resource)]
struct Joined {
    client: Entity,
    /// The number that this machine goes by in that game.
    id: u64,
    /// Where the game is, as the player is told.
    address: String,
    /// Whether it is joined the way browsers join, by what its host handed out.
    by_link: bool,
    /// Whether the game gave this machine a seat.
    seated: bool,
    /// Seconds spent connected without a seat.
    waited: f32,
}

/// The name of a host being looked up, before its game can be joined.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource)]
struct Lookup {
    name: String,
    found: Task<Option<SocketAddr>>,
}

/// A connection that the player ended. It goes once the host was told.
#[derive(Component)]
struct Leaving;

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
    address: SocketAddr,
    print: Option<String>,
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
        by_link,
        seated: false,
        waited: 0.0,
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
            .add_systems(Update, direct)
            .add_systems(
                FixedPreUpdate,
                send_commands.in_set(InputSystems::WriteClientInputs),
            )
            .add_systems(
                PreUpdate,
                (
                    watch,
                    (
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
) {
    for request in requests.read() {
        match (request, *session) {
            (SessionRequest::Join(text), Session::Solo) => {
                notice.0 = None;
                let started = match target(text) {
                    Some(Target::Address(address)) => {
                        join(&mut commands, address, None, *patience, &mut notice)
                    }
                    Some(Target::Web(address, print)) => {
                        join(&mut commands, address, Some(print), *patience, &mut notice)
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    Some(Target::Name(name, port)) => {
                        let host = name.clone();
                        let found = IoTaskPool::get().spawn(async move {
                            let found = (host.as_str(), port).to_socket_addrs().ok()?;
                            found.into_iter().find(SocketAddr::is_ipv4)
                        });
                        commands.insert_resource(Lookup { name, found });
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
                    commands.entity(joined.client).insert(Leaving);
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
        Some(address) => join(&mut commands, address, None, *patience, &mut notice),
        None => {
            notice.0 = Some(format!("No machine is known as {}.", lookup.name));
            false
        }
    };
    if !joined {
        *session = Session::Solo;
    }
}

/// Why a connection ended, as the player is told.
fn farewell(reason: &DisconnectedReason, joined: &Joined) -> String {
    let address = &joined.address;
    match reason {
        // A host that closes its game says so. Where browsers are taken, its line may close
        // before its word is through, which says the same.
        DisconnectedReason::ByPeer(_) | DisconnectedReason::LinkFailed(UnlinkReason::ByPeer(_)) => {
            "The host closed the game.".to_string()
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

/// Watches the connection to the host: takes the seat that the host gives, and goes back to
/// the menus when the connection ends.
fn watch(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut joined: ResMut<Joined>,
    clients: Query<(Has<Connected>, Option<&Disconnected>)>,
    lobbies: Query<&Lobby>,
    (mut seat, mut session, mut notice): (ResMut<LocalSeat>, ResMut<Session>, ResMut<Notice>),
    screen: Res<State<Screen>>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut next_overlay: ResMut<NextState<Overlay>>,
) {
    let Ok((connected, disconnected)) = clients.get(joined.client) else {
        return;
    };
    let seated = lobbies
        .single()
        .ok()
        .and_then(|lobby| lobby.seat_of(Member::Guest(joined.id)));
    let ended = match (disconnected, connected, seated) {
        (Some(disconnected), ..) => Some(farewell(&disconnected.reason, &joined)),
        (None, true, Some(given)) => {
            // The lobby is where a guest comes in.
            if !joined.seated {
                next_screen.set(Screen::Lobby);
            }
            joined.seated = true;
            joined.waited = 0.0;
            seat.set_if_neq(LocalSeat(given));
            None
        }
        // A host that has no seat to give says so by giving none.
        (None, true, None) => {
            joined.waited += time.delta_secs();
            (joined.waited > SEAT_WAIT).then(|| {
                let denied = DisconnectedReason::TransportError("Denied".to_string());
                farewell(&denied, &joined)
            })
        }
        (None, false, _) => None,
    };
    let Some(told) = ended else {
        return;
    };
    notice.0 = Some(told);
    // The connection may still be up when the host gave no seat.
    commands.trigger(Disconnect {
        entity: joined.client,
    });
    commands.entity(joined.client).insert(Leaving);
    commands.remove_resource::<Joined>();
    commands.remove_resource::<Run>();
    commands.remove_resource::<Field>();
    *session = Session::Solo;
    seat.0 = 0;
    // Whoever was still asking to join can ask again where they are.
    if *screen.get() != Screen::Join {
        next_screen.set(Screen::Menu);
    }
    next_overlay.set(Overlay::None);
}

/// A connection that ended goes once the frame has sent what was left to say. Everything it
/// received goes with it.
fn let_go(mut commands: Commands, gone: Query<Entity, With<Leaving>>) {
    for client in &gone {
        commands.entity(client).despawn();
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
    let wanted = match (*screen.get(), run.is_some()) {
        (Screen::Lobby | Screen::GameOver, _) if in_progress => Screen::Playing,
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
    use tanku_core::{
        CorePlugin,
        sim::{Position, Tank, Wreck},
    };
    use tanku_core::{discovery::Reach, sim::RunRequest};
    use tanku_server::{
        ServerPlugin,
        net::{Hosting, Seat},
    };

    use super::*;
    use crate::{
        crew,
        host::{HostPort, WebPort},
        session,
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

    /// A port that is free on this machine.
    pub(crate) fn free_port() -> u16 {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.local_addr().unwrap().port()
    }

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
        ask(&mut game, SessionRequest::Host);
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
        ask(&mut host, SessionRequest::Host);
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
        ask(&mut second, SessionRequest::Host);
        second.update();
        let told = second.world().resource::<Notice>().0.clone().unwrap();
        assert!(told.contains("is taken on this machine"), "{told}");
        assert_eq!(*second.world().resource::<Session>(), Session::Solo);
        assert_eq!(screen(&second), Screen::Menu);
    }
}
