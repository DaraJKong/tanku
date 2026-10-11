//! Looks for the games that are hosted on the local network, while the player is on the join
//! screen. What is found shows there, and a click joins it: nobody has to type an address
//! that a machine of the same network could tell by itself.

use std::net::Ipv4Addr;

use bevy::prelude::*;
use tanku_core::discovery::{Announcement, Browser, Reach, Sighting, local_address, nearest};

use crate::{
    Screen,
    net::written,
    session::{Found, Game},
};

/// How many games are shown.
const SHOWN: usize = 5;

/// The search for games, while it goes on.
#[derive(Resource)]
struct Search {
    browser: Browser,
    /// The name that each game goes by on the network, with its number.
    names: Vec<(String, u64)>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Screen::Join), start)
        .add_systems(OnExit(Screen::Join), stop)
        .add_systems(Update, look.run_if(resource_exists::<Search>));
}

fn start(mut commands: Commands, reach: Res<Reach>, mut found: ResMut<Found>) {
    found.0.clear();
    // Without a search, the player types the address as before.
    if let Some(browser) = Browser::new(*reach) {
        commands.insert_resource(Search {
            browser,
            names: Vec::new(),
        });
    }
}

fn stop(mut commands: Commands, mut found: ResMut<Found>) {
    commands.remove_resource::<Search>();
    found.0.clear();
}

/// Takes in what a host says of its game, with the address at which this machine reaches it.
/// A game that is told of again is the same game, by its number.
fn hear(games: &mut Vec<Game>, told: &Announcement, machine: Ipv4Addr) {
    let game = Game {
        id: told.game,
        machine,
        address: written(machine, told.port),
        players: told.players,
        standing: told.standing(),
    };
    if let Some(known) = games.iter_mut().find(|known| known.id == told.game) {
        *known = game;
    } else if games.len() < SHOWN {
        games.push(game);
    }
}

/// Keeps the list of the games that the network tells of.
fn look(time: Res<Time<Real>>, mut search: ResMut<Search>, mut found: ResMut<Found>) {
    let seen = search.browser.hear(time.delta_secs());
    if seen.is_empty() {
        return;
    }
    let (mut games, own) = (found.0.clone(), local_address());
    for sighting in seen {
        match sighting {
            Sighting::Game {
                name,
                told,
                machines,
            } => {
                let Some(machine) = nearest(&machines, own) else {
                    continue;
                };
                hear(&mut games, &told, machine);
                search.names.retain(|(known, _)| *known != name);
                search.names.push((name, told.game));
            }
            Sighting::Gone { name } => {
                search.names.retain(|(known, _)| *known != name);
                // A game goes with the last name that it went by.
                games.retain(|game| search.names.iter().any(|(_, id)| *id == game.id));
            }
        }
    }
    // In an order that holds from one frame to the next.
    games.sort_by(|a, b| (&a.address, a.id).cmp(&(&b.address, b.id)));
    if found.0 != games {
        found.0 = games;
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::discovery::Standing;

    use super::*;
    use crate::{
        net::{
            HostPort,
            tests::{ask, free_port, game, run_until, screen},
        },
        session::SessionRequest,
    };

    /// Where the games of a test are announced and looked for: on its own machine, on a port
    /// that nothing else listens on.
    fn here() -> Reach {
        Reach::Here(free_port())
    }

    /// A game that looks for games there, from the join screen.
    fn looking(reach: Reach) -> App {
        let mut game = game();
        game.insert_resource(reach);
        let mut next = game.world_mut().resource_mut::<NextState<Screen>>();
        next.set(Screen::Join);
        run_until("the join screen", &mut [&mut game], |games| {
            games[0].world().contains_resource::<Search>()
        });
        game
    }

    /// A game that hosts one, announced there.
    fn hosting(reach: Reach, port: u16) -> App {
        let mut game = game();
        game.insert_resource(reach).insert_resource(HostPort(port));
        ask(&mut game, SessionRequest::Host);
        run_until("a lobby", &mut [&mut game], |games| {
            screen(games[0]) == Screen::Lobby
        });
        game
    }

    fn found(game: &App) -> Vec<Game> {
        game.world().resource::<Found>().0.clone()
    }

    #[test]
    fn a_player_on_the_join_screen_finds_a_game_and_joins_it() {
        let (reach, port) = (here(), free_port());
        let mut host = hosting(reach, port);
        let mut guest = looking(reach);
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
        let mut other = looking(reach);
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
        // The host said goodbye: nobody had to wait for it to be missed.
        let waited = closed_at.elapsed().as_secs_f64();
        assert!(waited < 3.0, "the game was listed for {waited} s more");
    }

    #[test]
    fn a_game_that_opens_while_a_player_looks_is_found_too() {
        let reach = here();
        let mut guest = looking(reach);
        let since = std::time::Instant::now();
        run_until("a while", &mut [&mut guest], |_| {
            since.elapsed().as_secs_f64() > 1.5
        });
        assert!(found(&guest).is_empty());
        let port = free_port();
        let mut host = hosting(reach, port);
        run_until("a game", &mut [&mut host, &mut guest], |games| {
            !found(games[1]).is_empty()
        });
        assert_eq!(found(&guest)[0].address, format!("127.0.0.1:{port}"));
    }

    #[test]
    fn a_game_that_is_told_of_again_is_the_same_game() {
        let here = Ipv4Addr::LOCALHOST;
        let there = Ipv4Addr::new(192, 168, 0, 21);
        let told = Announcement::of(7, 28405, 1, true);
        let mut games = Vec::new();
        hear(&mut games, &told, here);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].address, "127.0.0.1");
        // What it says of itself is kept up, and so is where it is reached.
        let later = Announcement {
            players: 3,
            open: false,
            ..told
        };
        hear(&mut games, &later, there);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].address, "192.168.0.21");
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
