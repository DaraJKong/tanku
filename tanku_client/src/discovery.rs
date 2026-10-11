//! Looks for the games that are hosted on the local network, while the player is on the join
//! screen. What is found shows there, and a click joins it: nobody has to type an address
//! that a machine of the same network could tell by itself.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use bevy::prelude::*;
use tanku_core::discovery::{self, Announcement, Heard, Reach};

use crate::{
    Screen,
    net::written,
    session::{Found, Game},
};

/// Seconds between two times that every machine is asked.
const ASK: f32 = 1.0;
/// A game that wasn't heard of for this many seconds is gone, or its run has started and its
/// machine was turned off. A host tells of its game every second.
const FORGOTTEN: f64 = 3.5;
/// How many packets are read in a frame, and how many games are shown.
const READ: usize = 64;
const SHOWN: usize = 5;

/// The search for games, while it goes on.
#[derive(Resource)]
struct Search {
    socket: UdpSocket,
    /// Seconds until every machine is asked again.
    wait: f32,
    /// When each game was last heard of, by its number.
    heard: Vec<(u64, f64)>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Screen::Join), start)
        .add_systems(OnExit(Screen::Join), stop)
        .add_systems(Update, look.run_if(resource_exists::<Search>));
}

fn start(mut commands: Commands, reach: Res<Reach>, mut found: ResMut<Found>) {
    found.0.clear();
    // Without a socket, the player types the address as before.
    if let Ok(socket) = reach.socket() {
        commands.insert_resource(Search {
            socket,
            wait: 0.0,
            heard: Vec::new(),
        });
    }
}

fn stop(mut commands: Commands, mut found: ResMut<Found>) {
    commands.remove_resource::<Search>();
    found.0.clear();
}

/// Takes in what a host said of its game, from the machine that the word came from.
///
/// A game on this machine is heard of twice, as every machine hears it and as this machine
/// alone does. It is one game, and it keeps the address by which the other machines know it,
/// which is the one that its host's lobby shows.
fn hear(games: &mut Vec<Game>, told: &Announcement, machine: Ipv4Addr) {
    let (players, standing) = (told.players, told.standing());
    let address = written(machine, told.port);
    if let Some(game) = games.iter_mut().find(|game| game.id == told.game) {
        if game.machine.is_loopback() && !machine.is_loopback() {
            (game.machine, game.address) = (machine, address);
        }
        (game.players, game.standing) = (players, standing);
    } else if games.len() < SHOWN {
        games.push(Game {
            id: told.game,
            machine,
            address,
            players,
            standing,
        });
    }
}

/// Asks every machine for its game now and then, and keeps the list of those that answer.
fn look(
    time: Res<Time<Real>>,
    reach: Res<Reach>,
    mut search: ResMut<Search>,
    mut found: ResMut<Found>,
) {
    let now = time.elapsed_secs_f64();
    search.wait -= time.delta_secs();
    if search.wait <= 0.0 {
        search.wait = ASK;
        for everyone in reach.addresses() {
            let _ = search.socket.send_to(&discovery::question(), everyone);
        }
    }
    let mut games = found.0.clone();
    // A packet that fills this is longer than any that says something.
    let mut packet = [0; discovery::LIMIT + 1];
    for _ in 0..READ {
        let Ok((length, from)) = search.socket.recv_from(&mut packet) else {
            break;
        };
        let (Some(Heard::Answer(told)), IpAddr::V4(machine)) =
            (discovery::read(&packet[..length]), from.ip())
        else {
            continue;
        };
        hear(&mut games, &told, machine);
        if games.iter().any(|game| game.id == told.game) {
            search.heard.retain(|(game, _)| *game != told.game);
            search.heard.push((told.game, now));
        }
    }
    search.heard.retain(|(_, at)| now - at <= FORGOTTEN);
    games.retain(|game| search.heard.iter().any(|(heard, _)| *heard == game.id));
    // In an order that holds from one frame to the next.
    games.sort_by(|a, b| (&a.address, a.id).cmp(&(&b.address, b.id)));
    if found.0 != games {
        found.0 = games;
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::{discovery::Standing, protocol::REVISION};

    use super::*;
    use crate::{
        net::{
            HostPort,
            tests::{ask, free_port, game, run_until, screen},
        },
        session::SessionRequest,
    };

    /// Where the games of a test are looked for: on its own machine, on a port that is free.
    fn here() -> Reach {
        Reach {
            port: free_port(),
            to: Some(vec![Ipv4Addr::LOCALHOST]),
        }
    }

    /// A game that looks for games there, from the join screen.
    fn looking(reach: &Reach) -> App {
        let mut game = game();
        game.insert_resource(reach.clone());
        let mut next = game.world_mut().resource_mut::<NextState<Screen>>();
        next.set(Screen::Join);
        run_until("the join screen", &mut [&mut game], |games| {
            games[0].world().contains_resource::<Search>()
        });
        game
    }

    /// A game that hosts one, announced there.
    fn hosting(reach: &Reach, port: u16) -> App {
        let mut game = game();
        game.insert_resource(reach.clone())
            .insert_resource(HostPort(port));
        ask(&mut game, SessionRequest::Host);
        run_until("a lobby", &mut [&mut game], |games| {
            screen(games[0]) == Screen::Lobby
        });
        game
    }

    fn found(game: &App) -> Vec<Game> {
        game.world().resource::<Found>().0.clone()
    }

    /// The port that a game looks for games from.
    fn asks_from(game: &App) -> u16 {
        let search = game.world().resource::<Search>();
        search.socket.local_addr().unwrap().port()
    }

    #[test]
    fn a_player_on_the_join_screen_finds_a_game_and_joins_it() {
        let (reach, port) = (here(), free_port());
        let mut host = hosting(&reach, port);
        let mut guest = looking(&reach);
        // The host has the port of the games, so the guest asks from a port of its own, and
        // is answered there.
        assert_ne!(asks_from(&guest), reach.port);
        run_until("a game", &mut [&mut host, &mut guest], |games| {
            !found(games[1]).is_empty()
        });
        let games = found(&guest);
        assert_eq!(games.len(), 1);
        let address = format!("127.0.0.1:{port}");
        assert_eq!(games[0].address, address);
        assert_eq!((games[0].players, games[0].standing), (1, Standing::Open));

        // What was found is where the game is joined.
        ask(&mut guest, SessionRequest::Join(address.clone()));
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        // The search ends with the join screen.
        assert!(found(&guest).is_empty());
        assert!(!guest.world().contains_resource::<Search>());

        // Someone else who looks sees two players there, then that the run has started,
        // then nothing once the host has closed the game.
        let mut other = looking(&reach);
        let all = &mut [&mut host, &mut guest, &mut other];
        run_until("two players", all, |games| {
            found(games[2]).first().map(|game| game.players) == Some(2)
        });
        ask(&mut host, SessionRequest::Start);
        let all = &mut [&mut host, &mut guest, &mut other];
        run_until("a run in progress", all, |games| {
            found(games[2]).first().map(|game| game.standing) == Some(Standing::Running)
        });
        ask(&mut host, SessionRequest::Leave);
        let closed_at = std::time::Instant::now();
        let all = &mut [&mut host, &mut guest, &mut other];
        run_until("an empty list", all, |games| found(games[2]).is_empty());
        let waited = closed_at.elapsed().as_secs_f64();
        assert!(
            (2.0..FORGOTTEN + 1.0).contains(&waited),
            "the game was listed for {waited} s more"
        );
    }

    #[test]
    fn a_game_that_opens_after_the_search_started_on_its_machine_is_found_too() {
        let reach = here();
        // The game that looks came first and has the port of the games. The host that
        // comes second can't hear the question: what it tells every machine by itself is
        // what gets through.
        let mut guest = looking(&reach);
        assert_eq!(asks_from(&guest), reach.port);
        let port = free_port();
        let mut host = hosting(&reach, port);
        run_until("a game", &mut [&mut host, &mut guest], |games| {
            !found(games[1]).is_empty()
        });
        assert_eq!(found(&guest)[0].address, format!("127.0.0.1:{port}"));
    }

    #[test]
    fn a_game_of_another_version_shows_as_one_and_what_says_nothing_is_left_out() {
        let reach = here();
        let mut guest = looking(&reach);
        let to = (Ipv4Addr::LOCALHOST, asks_from(&guest));
        let teller = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let other = Announcement {
            revision: REVISION + 1,
            ..Announcement::of(9, 28405, 2, true)
        };
        let mut long = Announcement::of(10, 28405, 1, true).packet();
        long.resize(discovery::LIMIT + 40, b' ');
        for packet in [&b"TANKU! what"[..], &long, &other.packet()] {
            teller.send_to(packet, to).unwrap();
        }
        run_until("a game", &mut [&mut guest], |games| {
            !found(games[0]).is_empty()
        });
        let games = found(&guest);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].address, "127.0.0.1");
        assert_eq!(games[0].standing, Standing::Other);
        // It is forgotten once nothing is heard of it.
        run_until("an empty list", &mut [&mut guest], |games| {
            found(games[0]).is_empty()
        });
    }

    #[test]
    fn a_game_that_is_heard_of_by_two_ways_is_one_game() {
        let here = Ipv4Addr::LOCALHOST;
        let there = Ipv4Addr::new(192, 168, 0, 21);
        let told = Announcement::of(7, 28405, 1, true);
        let mut games = Vec::new();
        hear(&mut games, &told, here);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].address, "127.0.0.1");
        // The address by which other machines know it takes over, and stays.
        hear(&mut games, &told, there);
        hear(&mut games, &told, here);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].address, "192.168.0.21");
        // What it says of itself is kept up.
        let later = Announcement {
            players: 3,
            open: false,
            ..told
        };
        hear(&mut games, &later, there);
        assert_eq!(
            (games[0].players, games[0].standing),
            (3, Standing::Running)
        );
        // Another game is another row, with its port when it isn't the usual one.
        hear(&mut games, &Announcement::of(8, 4000, 1, true), there);
        assert_eq!(games.len(), 2);
        assert_eq!(games[1].address, "192.168.0.21:4000");
        // A network that claims to hold a crowd of games shows a few.
        for game in 100..200 {
            hear(&mut games, &Announcement::of(game, 28405, 1, true), there);
        }
        assert_eq!(games.len(), SHOWN);
    }
}
