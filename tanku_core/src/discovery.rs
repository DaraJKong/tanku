//! How a game of several players is found on the local network.
//!
//! A machine that looks for a game asks every machine of its network at once, on a port that
//! every game knows. A machine that hosts a game answers the one that asked. It also tells
//! every machine of its game now and then, which reaches those whose question got lost on the
//! way. A question and an answer are each a line of text in a packet of its own.
//!
//! Nothing here is needed to join a game: its address does that. This only saves typing it.

use std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddrV4, UdpSocket},
};

use bevy::prelude::*;

use crate::{protocol::REVISION, rules::MAX_PLAYERS, seed::VERSION};

/// The port on which games are asked for and announced.
pub const PORT: u16 = 28406;
/// The longest packet that is read. An announcement is about a third as long.
pub const LIMIT: usize = 128;
/// The longest name of a version that is read.
const VERSION_LENGTH: usize = 16;

const QUESTION: &str = "TANKU?";
const ANSWER: &str = "TANKU!";

/// What a host says of its game to those who look for one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Announcement {
    /// A number of its own, by which a game that is heard of by two ways is known as one.
    pub game: u64,
    /// The version of the game that hosts it, and the revision of what it tells its guests. A
    /// game only takes players whose game has both.
    pub version: String,
    pub revision: u32,
    /// The port on which the game takes its guests.
    pub port: u16,
    pub players: u8,
    /// Whether the game takes players now: nobody joins a run in progress.
    pub open: bool,
}

/// What a player can do about a game that was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// It can be joined.
    Open,
    /// Every seat is taken.
    Full,
    /// Its run is in progress.
    Running,
    /// It is hosted by another version of the game.
    Other,
}

impl Announcement {
    /// What a game that this build hosts says of itself.
    pub fn of(game: u64, port: u16, players: u8, open: bool) -> Self {
        Self {
            game,
            version: VERSION.to_string(),
            revision: REVISION,
            port,
            players,
            open,
        }
    }

    /// What a player of this build can do about the game.
    pub fn standing(&self) -> Standing {
        if self.version != VERSION || self.revision != REVISION {
            Standing::Other
        } else if !self.open {
            Standing::Running
        } else if self.players as u32 >= MAX_PLAYERS {
            Standing::Full
        } else {
            Standing::Open
        }
    }

    /// The packet that says it.
    pub fn packet(&self) -> Vec<u8> {
        let door = if self.open { "open" } else { "closed" };
        let (game, version, revision) = (self.game, &self.version, self.revision);
        let (port, players) = (self.port, self.players);
        format!("{ANSWER} {game:x} {version} {revision} {port} {players} {door}").into_bytes()
    }
}

/// What a packet on the port of the games says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Heard {
    /// Someone looks for a game.
    Question,
    /// Someone hosts one.
    Answer(Announcement),
}

/// The packet that asks for the games of a network.
pub fn question() -> Vec<u8> {
    QUESTION.as_bytes().to_vec()
}

/// Reads a packet. Anything can come in on a port of a network: what is neither a question nor
/// an answer says nothing.
pub fn read(packet: &[u8]) -> Option<Heard> {
    if packet.len() > LIMIT {
        return None;
    }
    let mut words = std::str::from_utf8(packet).ok()?.split_ascii_whitespace();
    match words.next()? {
        QUESTION => Some(Heard::Question),
        ANSWER => {
            let game = u64::from_str_radix(words.next()?, 16).ok()?;
            let named = |version: &&str| {
                version.len() <= VERSION_LENGTH && version.chars().all(|c| c.is_ascii_graphic())
            };
            let version = words.next().filter(named)?.to_string();
            let revision = words.next()?.parse().ok()?;
            let port = words.next()?.parse().ok().filter(|port| *port != 0)?;
            let players = words.next()?.parse().ok()?;
            let open = match words.next()? {
                "open" => true,
                "closed" => false,
                _ => return None,
            };
            // A later build may have more to say: what follows is left unread.
            Some(Heard::Answer(Announcement {
                game,
                version,
                revision,
                port,
                players,
                open,
            }))
        }
        _ => None,
    }
}

/// The address of this machine on its network, if it has one. Nothing is sent: aiming a socket
/// at an address only picks the route to it, and with it the address that this machine has on
/// that route.
pub fn local_address() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // An address kept for documents, which no machine has.
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(address) if !address.is_loopback() && !address.is_unspecified() => Some(address),
        _ => None,
    }
}

/// The addresses that reach every machine of the local network, and this one.
///
/// The first is every machine of whichever network the system picks. A machine that is on
/// several may pick another one than the players are on, so the second is the network of the
/// address that this machine goes out by, taken to hold 256 addresses as most networks at
/// home do. The last is this machine itself, for two games on a machine that has no network.
pub fn everyone() -> Vec<Ipv4Addr> {
    let mut to = vec![Ipv4Addr::BROADCAST];
    if let Some(own) = local_address() {
        let [a, b, c, _] = own.octets();
        to.push(Ipv4Addr::new(a, b, c, 255));
    }
    to.push(Ipv4Addr::LOCALHOST);
    to
}

/// Where games are asked for and announced.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct Reach {
    pub port: u16,
    /// The machines that are told. Without a list, every machine of the local network is.
    pub to: Option<Vec<Ipv4Addr>>,
}

impl Default for Reach {
    fn default() -> Self {
        Self {
            port: PORT,
            to: None,
        }
    }
}

impl Reach {
    /// Nobody is asked or told, and nothing is heard. Tests that aren't about finding games
    /// keep to themselves this way.
    pub fn nobody() -> Self {
        Self {
            port: 0,
            to: Some(Vec::new()),
        }
    }

    /// The addresses that a question or an announcement goes to.
    pub fn addresses(&self) -> Vec<SocketAddrV4> {
        let machines = self.to.clone().unwrap_or_else(everyone);
        let on_the_port = |machine| SocketAddrV4::new(machine, self.port);
        machines.into_iter().map(on_the_port).collect()
    }

    /// A socket to ask and to announce with, which never waits. It is on the port of the
    /// games if that port is free on this machine: it then hears what is said to every
    /// machine. If another game has the port, it is on a port of its own, and hears what is
    /// said to it alone. Between two machines either will do, and between two games on one
    /// machine it takes one of each.
    pub fn socket(&self) -> io::Result<UdpSocket> {
        let any = Ipv4Addr::UNSPECIFIED;
        let socket = UdpSocket::bind((any, self.port)).or_else(|_| UdpSocket::bind((any, 0)))?;
        socket.set_nonblocking(true)?;
        socket.set_broadcast(true)?;
        Ok(socket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Announcement {
        Announcement::of(0xFEED_5EED, 28405, 2, true)
    }

    #[test]
    fn what_a_host_says_of_its_game_is_read_back() {
        let told = game();
        assert_eq!(read(&told.packet()), Some(Heard::Answer(told.clone())));
        let closed = Announcement {
            open: false,
            players: 4,
            ..told
        };
        assert_eq!(read(&closed.packet()), Some(Heard::Answer(closed)));
        assert_eq!(read(&question()), Some(Heard::Question));
        // Both fit in a packet with room to spare.
        let longest = Announcement {
            game: u64::MAX,
            version: "9".repeat(VERSION_LENGTH),
            revision: u32::MAX,
            port: u16::MAX,
            players: u8::MAX,
            open: false,
        };
        assert!(longest.packet().len() < LIMIT, "{}", longest.packet().len());
        assert_eq!(read(&longest.packet()), Some(Heard::Answer(longest)));
    }

    #[test]
    fn a_later_build_can_say_more() {
        let mut packet = game().packet();
        packet.extend(b" and more that this build knows nothing of");
        assert_eq!(read(&packet), Some(Heard::Answer(game())));
    }

    #[test]
    fn what_is_neither_a_question_nor_an_answer_says_nothing() {
        let told = String::from_utf8(game().packet()).unwrap();
        let cut = &told[..told.len() - 5];
        let junk: [&[u8]; 9] = [
            b"",
            b" ",
            b"TANKU",
            b"tanku?",
            b"HELLO? TANKU?",
            b"TANKU! feed5eed 0.4.0 3 0 2 open",
            b"TANKU! feed5eed 0.4.0 3 28405 two open",
            b"TANKU! feed5eed 0.4.0 3 28405 2 ajar",
            &[0xFF, 0xFE, 0x00, 0x54],
        ];
        for packet in junk.into_iter().chain([cut.as_bytes()]) {
            assert_eq!(read(packet), None, "{:?}", String::from_utf8_lossy(packet));
        }
        // Neither does a packet that is too long, even one that starts well.
        let mut long = game().packet();
        long.resize(LIMIT + 1, b' ');
        assert_eq!(read(&long), None);
        let unnamed = format!("TANKU! 1 {} 3 28405 2 open", "9".repeat(VERSION_LENGTH + 1));
        assert_eq!(read(unnamed.as_bytes()), None);
    }

    #[test]
    fn a_player_is_told_what_they_can_do_about_a_game() {
        assert_eq!(game().standing(), Standing::Open);
        let with = |change: fn(&mut Announcement)| {
            let mut told = game();
            change(&mut told);
            told.standing()
        };
        assert_eq!(with(|told| told.players = 4), Standing::Full);
        assert_eq!(with(|told| told.open = false), Standing::Running);
        assert_eq!(with(|told| told.revision += 1), Standing::Other);
        assert_eq!(with(|told| told.version = "0.0.1".into()), Standing::Other);
        // A game of another version is that before anything else: its seats are no use.
        let other = with(|told| {
            told.revision += 1;
            told.open = false;
        });
        assert_eq!(other, Standing::Other);
    }

    #[test]
    fn every_machine_is_told_on_the_port_of_the_games() {
        let everyone = Reach::default().addresses();
        assert!(everyone.iter().all(|address| address.port() == PORT));
        let machines: Vec<Ipv4Addr> = everyone.iter().map(|address| *address.ip()).collect();
        assert_eq!(machines.first(), Some(&Ipv4Addr::BROADCAST));
        assert_eq!(machines.last(), Some(&Ipv4Addr::LOCALHOST));
        // A test tells its own machine, or nobody.
        let here = Reach {
            port: 4000,
            to: Some(vec![Ipv4Addr::LOCALHOST]),
        };
        assert_eq!(here.addresses(), ["127.0.0.1:4000".parse().unwrap()]);
        assert!(Reach::nobody().addresses().is_empty());
    }

    #[test]
    fn a_second_socket_on_a_machine_takes_a_port_of_its_own() {
        // A port that is free on this machine.
        let free = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = free.local_addr().unwrap().port();
        drop(free);
        let reach = Reach {
            port,
            to: Some(vec![Ipv4Addr::LOCALHOST]),
        };
        let first = reach.socket().unwrap();
        let second = reach.socket().unwrap();
        assert_eq!(first.local_addr().unwrap().port(), port);
        assert_ne!(second.local_addr().unwrap().port(), port);
        // Neither waits for a packet that isn't there.
        let mut packet = [0; 8];
        assert!(first.recv_from(&mut packet).is_err());
        // What the second says to every machine reaches the first, and the answer finds it.
        second.send_to(&question(), reach.addresses()[0]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let (length, from) = first.recv_from(&mut packet).unwrap();
        assert_eq!(read(&packet[..length]), Some(Heard::Question));
        assert_eq!(from.port(), second.local_addr().unwrap().port());
    }
}
