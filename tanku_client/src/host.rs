//! Hosting a game of several players. This machine runs the simulation, as in a solo game,
//! and its server sends it to the guests: to the games of other machines, and to browsers.
//! A browser hosts nothing, because nothing can reach it.

use bevy::prelude::*;
use tanku_core::{
    discovery::{Reach, local_address},
    modes::Difficulty,
    protocol::{Lobby, PORT, WEB_PORT},
    sim::{Run, RunRequest},
};
use tanku_server::net::{self as server, Credentials, Hosting, Ports, Seat};

use crate::{
    Overlay, Screen,
    cli::Options,
    crew::LocalSeat,
    net::{web_code, written},
    session::{Drawn, Invitation, Notice, Session, SessionRequest},
};

/// The port on which this machine hosts a game.
#[derive(Resource, Clone, Copy, Debug)]
pub struct HostPort(pub u16);

/// The port on which a game that this machine hosts listens for browsers, if it takes them.
#[derive(Resource, Clone, Copy, Debug)]
pub struct WebPort(pub Option<u16>);

/// Comes after the plugins of the network library and the protocol.
pub fn plugin(app: &mut App) {
    app.add_plugins(server::NetPlugin)
        // A game that was given no command line brings nothing of its own to a run.
        .init_resource::<Options>()
        .insert_resource(HostPort(PORT))
        .insert_resource(WebPort(Some(WEB_PORT)))
        .add_systems(Update, direct)
        .add_systems(
            OnEnter(Screen::Lobby),
            reopen.run_if(resource_exists::<Hosting>),
        );
}

/// Does what the menus ask of a host: to open a game, to start its run, and to close it.
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
    (hosting, run, options): (Option<Res<Hosting>>, Option<Res<Run>>, Res<Options>),
    mut seats: Query<&mut Seat>,
    lobbies: Query<&Lobby>,
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
                let sign = (sign.name.as_str(), sign.password.as_str());
                let opened = server::open(&mut commands, ports, &reach, &mut credentials, sign);
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
            (SessionRequest::Start, Session::Host) => {
                let lobby = hosting.as_ref().and_then(|hosting| {
                    let lobby = lobbies.get(hosting.game()).ok()?;
                    Some((hosting, lobby))
                });
                if let Some((hosting, lobby)) = lobby {
                    let players = server::muster(hosting, &mut seats);
                    // The next run on this seed is one on a seed that was played.
                    let (seed, drawn) = (lobby.seed.clone(), std::mem::take(&mut drawn.0));
                    // A host sets no difficulty yet: the lobby has no options so far.
                    run_requests.write(RunRequest::Start {
                        seed,
                        players,
                        drawn,
                        difficulty: Difficulty::Normal,
                        // The host has the first seat, and the tank of a playtest is theirs.
                        free: options.tank.is_some(),
                        tanks: options.tanks(),
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
