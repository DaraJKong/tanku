//! Hosting a game of several players. This machine runs the simulation, as in a solo game,
//! and its server sends it to the guests: to the games of other machines, and to browsers.
//! A browser hosts nothing, because nothing can reach it.
//!
//! The host also keeps the rules of the lobby: who can be ready, what takes a player's word
//! back, and the countdown to the run once everyone is ready.

use bevy::prelude::*;
use tanku_core::{
    discovery::{Reach, local_address},
    modes::Difficulty,
    protocol::{Lobby, PORT, WEB_PORT},
    rules::COUNTDOWN,
    sim::{Run, RunRequest},
    stats::Genome,
};
use tanku_server::net::{self as server, Built, Credentials, Hosting, Ports, Seat};

use crate::{
    Overlay, Screen,
    cli::Options,
    crew::LocalSeat,
    garage::Garage,
    menu::held_back,
    net::{web_code, written},
    session::{Drawn, Invitation, Notice, Session, SessionRequest},
    unlocks::Unlocks,
};

/// The port on which this machine hosts a game.
#[derive(Resource, Clone, Copy, Debug)]
pub struct HostPort(pub u16);

/// The port on which a game that this machine hosts listens for browsers, if it takes them.
#[derive(Resource, Clone, Copy, Debug)]
pub struct WebPort(pub Option<u16>);

/// How long the lobby of a game that this machine hosts counts down, once everyone is ready,
/// before its run starts.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Countdown(pub f32);

/// Comes after the plugins of the network library and the protocol.
pub fn plugin(app: &mut App) {
    app.add_plugins(server::NetPlugin)
        // A game that was given no command line brings nothing of its own to a run, one
        // without a garage has the standard tank in it, and one that keeps no achievements
        // has unlocked nothing.
        .init_resource::<Options>()
        .init_resource::<Garage>()
        .init_resource::<Unlocks>()
        .insert_resource(HostPort(PORT))
        .insert_resource(WebPort(Some(WEB_PORT)))
        .insert_resource(Countdown(COUNTDOWN))
        // The run that a countdown starts is started in the same frame, so that the lobby
        // counts down once.
        .add_systems(
            Update,
            (
                (settle, count_down).run_if(resource_exists::<Hosting>),
                direct,
            )
                .chain(),
        )
        .add_systems(
            OnEnter(Screen::Lobby),
            reopen.run_if(resource_exists::<Hosting>),
        );
}

/// What a host's game knew of its lobby a frame ago, to tell what has changed since.
#[derive(Default)]
struct Known {
    /// What the run was set to.
    set: Option<(Difficulty, String, bool, bool)>,
    /// The tank that the host had for a run in free mode.
    tank: Option<Genome>,
}

/// Keeps who is ready as the rules of the lobby have it. Nobody is ready during a run, nor
/// once the host has changed what the next one is set to. A host who changes their tank is
/// no longer ready, as any player. And a host can't be ready for a run that can't start: one
/// without a seed, or one that a single player has not unlocked while the host is alone.
fn settle(
    hosting: Res<Hosting>,
    (options, garage, unlocks): (Res<Options>, Res<Garage>, Res<Unlocks>),
    mut lobbies: Query<&mut Lobby>,
    mut known: Local<Known>,
) {
    let Ok(mut lobby) = lobbies.get_mut(hosting.game()) else {
        return;
    };
    let mut ready = lobby.ready;
    if !hosting.is_open() {
        ready = default();
    }
    let set = (
        lobby.difficulty,
        lobby.seed.clone(),
        lobby.typed,
        lobby.free,
    );
    if known.set.as_ref().is_some_and(|known| *known != set) {
        ready = default();
    }
    known.set = Some(set);
    // Outside free mode, the tank of a player is not theirs to change.
    let tank = lobby.free.then(|| options.tank.unwrap_or(garage.tank));
    if tank.is_some() && known.tank.is_some() && tank != known.tank {
        ready[0] = false;
    }
    known.tank = tank;
    if held_back(&lobby, &unlocks, &options).is_some() {
        ready[0] = false;
    }
    if lobby.ready != ready {
        lobby.ready = ready;
    }
}

/// Counts down while everyone in the lobby is ready, and starts the run when the count is
/// over. The count stops as soon as someone is not ready, and starts over the next time.
fn count_down(
    time: Res<Time<Real>>,
    hosting: Res<Hosting>,
    length: Res<Countdown>,
    mut lobbies: Query<&mut Lobby>,
    mut left: Local<Option<f32>>,
    mut requests: MessageWriter<SessionRequest>,
) {
    let Ok(mut lobby) = lobbies.get_mut(hosting.game()) else {
        return;
    };
    let counting = hosting.is_open() && lobby.all_ready();
    *left = match (*left, counting) {
        (_, false) => None,
        (None, true) => Some(length.0),
        (Some(left), true) => Some(left - time.delta_secs()),
    };
    if left.is_some_and(|left| left <= 0.0) {
        *left = None;
        requests.write(SessionRequest::Start);
    }
    // The lobby shows whole seconds: 3, 2, 1.
    let shown = left.map(|left| left.ceil().max(1.0) as u8);
    if lobby.countdown != shown {
        lobby.countdown = shown;
    }
}

/// Does what the menus ask of a host: to open a game, to say that they are ready for its
/// run, to start a run, and to close the game.
fn direct(
    mut commands: Commands,
    mut requests: MessageReader<SessionRequest>,
    (mut session, mut notice, mut invitation, mut seat): (
        ResMut<Session>,
        ResMut<Notice>,
        ResMut<Invitation>,
        ResMut<LocalSeat>,
    ),
    (port, web, reach, mut credentials): (
        Res<HostPort>,
        Res<WebPort>,
        Res<Reach>,
        ResMut<Credentials>,
    ),
    (hosting, run, options, garage): (
        Option<Res<Hosting>>,
        Option<Res<Run>>,
        Res<Options>,
        Res<Garage>,
    ),
    mut seats: Query<(&mut Seat, Option<&Built>)>,
    mut lobbies: Query<&mut Lobby>,
    mut run_requests: MessageWriter<RunRequest>,
    mut drawn: ResMut<Drawn>,
    mut next_screen: ResMut<NextState<Screen>>,
    mut next_overlay: ResMut<NextState<Overlay>>,
) {
    for request in requests.read() {
        match (request, *session) {
            (SessionRequest::Host(sign), Session::Solo) => {
                let ports = Ports {
                    game: port.0,
                    web: web.0,
                };
                // The lobby opens on the run that the host set the game up for.
                let game = Lobby {
                    name: sign.name.clone(),
                    difficulty: sign.difficulty,
                    free: sign.free,
                    playtest: options.playtest,
                    ..default()
                };
                let game = (game, sign.password.as_str());
                let opened = server::open(&mut commands, ports, &reach, &mut credentials, game);
                let Ok(web) = opened else {
                    notice.0 = Some(format!(
                        "Port {} is taken on this machine. Is another game open?",
                        port.0
                    ));
                    continue;
                };
                *session = Session::Host;
                notice.0 = None;
                let machine = local_address();
                // A machine without a network still takes a browser of its own.
                let reached_at = machine.unwrap_or(std::net::Ipv4Addr::LOCALHOST);
                *invitation = Invitation {
                    address: machine.map(|machine| written(machine, port.0)),
                    port: port.0,
                    web: web.map(|web| web_code(reached_at, web.port, &web.print)),
                    web_port: ports.web,
                };
                if let Some(code) = &invitation.web {
                    info!(
                        "A browser joins this game with #{code} at the end of the page's address."
                    );
                }
                seat.0 = 0;
                next_screen.set(Screen::Lobby);
            }
            (SessionRequest::Leave, Session::Host) => {
                if let Some(hosting) = &hosting {
                    if run.is_some() {
                        run_requests.write(RunRequest::Abandon);
                    }
                    server::close(&mut commands, hosting);
                }
                *session = Session::Solo;
                *invitation = Invitation::default();
            }
            // The host is a player of the lobby as the others are. `settle` takes the
            // word back if the run can't start.
            (SessionRequest::Ready(ready), Session::Host) => {
                let open = hosting.as_ref().filter(|hosting| hosting.is_open());
                let lobby = open.and_then(|hosting| lobbies.get_mut(hosting.game()).ok());
                if let Some(mut lobby) = lobby.filter(|lobby| lobby.ready[0] != *ready) {
                    lobby.ready[0] = *ready;
                }
            }
            (SessionRequest::Start, Session::Host) => {
                let lobby = hosting.as_ref().and_then(|hosting| {
                    let lobby = lobbies.get(hosting.game()).ok()?;
                    Some((hosting, lobby))
                });
                if let Some((hosting, lobby)) = lobby {
                    let guests = server::muster(hosting, &mut seats);
                    let players = guests.len() as u32 + 1;
                    // The next run on this seed is one on a seed that was played, and a
                    // seed that the host typed is none that the game drew.
                    let (seed, fresh) = (lobby.seed.clone(), std::mem::take(&mut drawn.0));
                    // In free mode, each player has the tank that they built: the host has
                    // the first seat. Outside it, only a playtest brings a tank, and that
                    // one is its host's alone.
                    let tanks: Vec<Genome> = if lobby.free {
                        let own = options.tank.unwrap_or(garage.tank);
                        std::iter::once(own).chain(guests).collect()
                    } else {
                        options.tanks()
                    };
                    run_requests.write(RunRequest::Start {
                        seed,
                        players,
                        drawn: fresh && !lobby.typed,
                        difficulty: lobby.difficulty,
                        free: lobby.free || options.tank.is_some(),
                        arena: false,
                        tanks,
                    });
                    next_screen.set(Screen::Playing);
                    next_overlay.set(Overlay::None);
                }
            }
            // The rest is not a host's to do: joining and leaving a game are a guest's.
            _ => {}
        }
    }
}

/// Players can join a hosted game again whenever its host is back in the lobby.
fn reopen(hosting: Res<Hosting>) {
    hosting.reopen();
}
