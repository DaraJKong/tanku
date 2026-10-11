//! How a game of several players is found on the local network.
//!
//! A machine that hosts a game announces it by multicast DNS, the way printers and speakers
//! announce themselves, and a machine whose player wants to join browses for games the same
//! way. The crate `mdns-sd` does the talking, on every network that the machine is on. What
//! a host says of its game, besides its address and its port, travels as the text of the
//! announcement.
//!
//! Nothing here is needed to join a game: its address does that. This only saves typing it.

use std::net::{IpAddr, Ipv4Addr, UdpSocket};

use bevy::prelude::*;

use crate::{protocol::REVISION, rules::MAX_PLAYERS, seed::VERSION};

/// The kind of service that a game is on the local network.
pub const SERVICE: &str = "_tanku._udp.local.";
/// The longest name of a version that is read.
const VERSION_LENGTH: usize = 16;

/// What a host says of its game to those who look for one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Announcement {
    /// A number of its own, by which a game is told from every other.
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

    /// The text of the announcement, as names and what goes by them. The port is not in it:
    /// an announcement carries the port of its service apart.
    pub fn text(&self) -> [(&'static str, String); 5] {
        let door = if self.open { "open" } else { "closed" };
        [
            ("game", format!("{:x}", self.game)),
            ("version", self.version.clone()),
            ("revision", self.revision.to_string()),
            ("players", self.players.to_string()),
            ("door", door.to_string()),
        ]
    }

    /// Reads what a host says of its game, from the port of its service and from what its
    /// text has by each name. Anything can announce itself on a network: what doesn't read
    /// as a game is none. A later build may say more, which is left unread.
    pub fn read<'a>(port: u16, said: impl Fn(&str) -> Option<&'a str>) -> Option<Self> {
        let named = |version: &&str| {
            (1..=VERSION_LENGTH).contains(&version.len())
                && version.chars().all(|letter| letter.is_ascii_graphic())
        };
        Some(Self {
            game: u64::from_str_radix(said("game")?, 16).ok()?,
            version: said("version").filter(named)?.to_string(),
            revision: said("revision")?.parse().ok()?,
            port: Some(port).filter(|port| *port != 0)?,
            players: said("players")?.parse().ok()?,
            open: match said("door")? {
                "open" => true,
                "closed" => false,
                _ => return None,
            },
        })
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

/// The address at which a machine whose own is given reaches a host, among those that the
/// host has: one on the same network rather than on another, and any of them rather than the
/// one that every machine has for itself. Two networks are taken for one when their first
/// three numbers are the same, as most networks at home go.
pub fn nearest(machines: &[Ipv4Addr], own: Option<Ipv4Addr>) -> Option<Ipv4Addr> {
    let network = |address: &Ipv4Addr| address.octets()[..3].to_vec();
    let rank = |address: &Ipv4Addr| {
        if address.is_loopback() {
            2
        } else if own.is_some_and(|own| network(&own) == network(address)) {
            0
        } else {
            1
        }
    };
    machines
        .iter()
        .copied()
        .min_by_key(|address| (rank(address), *address))
}

/// Where games are announced and looked for.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reach {
    /// On every network that this machine is on.
    #[default]
    Network,
    /// On this machine alone, on a port of its own: what a test announces, only that test
    /// finds.
    Here(u16),
    /// Nowhere: nothing is announced and nothing is found.
    Nowhere,
}

#[cfg(not(target_arch = "wasm32"))]
pub use on_the_network::{Announcer, Browser, Sighting};

#[cfg(not(target_arch = "wasm32"))]
mod on_the_network {
    use std::time::Duration;

    use mdns_sd::{IfKind, Receiver, ServiceDaemon, ServiceEvent, ServiceInfo};

    use super::*;

    /// Seconds between two times that a machine asks the network afresh for its games, and
    /// each game that it knows whether it is still there. Left to itself, the crate asks less
    /// and less often, and keeps a host that was unplugged for two minutes.
    const ASK: f32 = 5.0;
    /// How long a game has to answer that it is still there.
    const ANSWER: Duration = Duration::from_secs(4);
    /// How long a host waits for its goodbye to be on its way when it closes its game.
    const GOODBYE: Duration = Duration::from_millis(250);

    impl Reach {
        /// What talks to the network, in a thread of its own.
        pub(super) fn daemon(self) -> Option<ServiceDaemon> {
            let daemon = match self {
                Reach::Network => ServiceDaemon::new(),
                Reach::Here(port) => ServiceDaemon::new_with_port(port),
                Reach::Nowhere => return None,
            }
            .ok()?;
            if let Reach::Here(_) = self {
                daemon.disable_interface(IfKind::All).ok()?;
                daemon.enable_interface(IfKind::LoopbackV4).ok()?;
            } else {
                // A game takes its guests at the older kind of address only.
                daemon.disable_interface(IfKind::IPv6).ok()?;
            }
            Some(daemon)
        }
    }

    /// Announces a hosted game for as long as it lives. Dropping it says goodbye.
    pub struct Announcer {
        pub(super) daemon: ServiceDaemon,
        /// The name that the game goes by on the network, which no other game has.
        label: String,
        told: Option<Announcement>,
    }

    impl Announcer {
        /// Starts to announce the game of a number where games are looked for. There is
        /// none if nothing is to be announced, or if this machine can't.
        pub fn new(reach: Reach, game: u64) -> Option<Self> {
            Some(Self {
                daemon: reach.daemon()?,
                label: format!("tanku-{game:016x}"),
                told: None,
            })
        }

        /// Says what the game is like, if that is news.
        pub fn tell(&mut self, told: &Announcement) {
            if self.told.as_ref() == Some(told) {
                return;
            }
            // The machine goes by the name of the game, so that no two games claim one name.
            // Its addresses are filled in on each network that it is on.
            let machine = format!("{}.local.", self.label);
            let text = told.text();
            let service =
                ServiceInfo::new(SERVICE, &self.label, &machine, "", told.port, &text[..])
                    .map(ServiceInfo::enable_addr_auto);
            if service.is_ok_and(|service| self.daemon.register(service).is_ok()) {
                self.told = Some(told.clone());
            }
        }
    }

    impl Drop for Announcer {
        fn drop(&mut self) {
            // The goodbye is sent before the thread ends. A game that closes with its
            // window waits for that, or the window would take the thread along first.
            if let Ok(done) = self.daemon.shutdown() {
                let _ = done.recv_timeout(GOODBYE);
            }
        }
    }

    /// What a machine that looks for games learns.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Sighting {
        /// A game is there, by this name on the network, as its host tells it and at these
        /// addresses of its machine.
        Game {
            name: String,
            told: Announcement,
            machines: Vec<Ipv4Addr>,
        },
        /// The game of this name is gone: its host closed it, or answers no more.
        Gone { name: String },
    }

    /// Looks for the games of the local network for as long as it lives.
    pub struct Browser {
        daemon: ServiceDaemon,
        events: Receiver<ServiceEvent>,
        /// The games that are there, by name, each with whether the network told of it
        /// since it was last asked afresh.
        known: Vec<(String, bool)>,
        /// Seconds until the network is asked afresh.
        wait: f32,
    }

    impl Browser {
        /// Starts to look where games are announced. There is none if nothing is to be
        /// found, or if this machine can't look.
        pub fn new(reach: Reach) -> Option<Self> {
            let daemon = reach.daemon()?;
            let events = daemon.browse(SERVICE).ok()?;
            Some(Self {
                daemon,
                events,
                known: Vec::new(),
                wait: ASK,
            })
        }

        /// What was learned since the last time, so many seconds ago. It is asked for in
        /// every frame: the thread that listens waits for its news to be taken.
        pub fn hear(&mut self, seconds: f32) -> Vec<Sighting> {
            let mut seen = Vec::new();
            while let Ok(event) = self.events.try_recv() {
                match event {
                    ServiceEvent::ServiceResolved(found) => {
                        let said = |name: &str| found.get_property_val_str(name);
                        let Some(told) = Announcement::read(found.port, said) else {
                            continue;
                        };
                        let mut machines: Vec<Ipv4Addr> =
                            found.get_addresses_v4().into_iter().collect();
                        machines.sort();
                        let name = found.fullname.clone();
                        self.known.retain(|(known, _)| *known != name);
                        self.known.push((name.clone(), true));
                        seen.push(Sighting::Game {
                            name,
                            told,
                            machines,
                        });
                    }
                    ServiceEvent::ServiceRemoved(_, name) => {
                        self.known.retain(|(known, _)| *known != name);
                        seen.push(Sighting::Gone { name });
                    }
                    _ => {}
                }
            }
            self.wait -= seconds;
            if self.wait <= 0.0 {
                self.wait = ASK;
                self.ask_afresh(&mut seen);
            }
            seen
        }

        /// Asks the network for its games as if for the first time, which brings what is
        /// known of each again, and asks each game whether it is still there. A game that
        /// the last asking didn't bring is gone, whatever became of the news of it.
        fn ask_afresh(&mut self, seen: &mut Vec<Sighting>) {
            let (there, gone): (Vec<_>, Vec<_>) =
                self.known.drain(..).partition(|(_, told_of)| *told_of);
            seen.extend(gone.into_iter().map(|(name, _)| Sighting::Gone { name }));
            for (name, _) in there {
                let _ = self.daemon.verify(name.clone(), ANSWER);
                self.known.push((name, false));
            }
            if let Ok(events) = self.daemon.browse(SERVICE) {
                self.events = events;
            }
        }
    }

    impl Drop for Browser {
        fn drop(&mut self) {
            let _ = self.daemon.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn game() -> Announcement {
        Announcement::of(0xFEED_5EED, 28405, 2, true)
    }

    /// Reads back what an announcement says, with some of its text changed.
    fn read_with(told: &Announcement, changes: &[(&str, Option<&str>)]) -> Option<Announcement> {
        let mut text: HashMap<String, String> = told
            .text()
            .into_iter()
            .map(|(name, said)| (name.to_string(), said))
            .collect();
        for (name, said) in changes {
            match said {
                Some(said) => text.insert(name.to_string(), said.to_string()),
                None => text.remove(*name),
            };
        }
        Announcement::read(told.port, |name| text.get(name).map(String::as_str))
    }

    #[test]
    fn what_a_host_says_of_its_game_is_read_back() {
        let told = game();
        assert_eq!(read_with(&told, &[]), Some(told.clone()));
        let closed = Announcement {
            open: false,
            players: 4,
            game: u64::MAX,
            ..told.clone()
        };
        assert_eq!(read_with(&closed, &[]), Some(closed));
        // A later build may say more.
        assert_eq!(read_with(&told, &[("mode", Some("hard"))]), Some(told));
        // A name and what goes by it fit the 255 letters that an announcement gives them.
        let longest = Announcement {
            version: "9".repeat(VERSION_LENGTH),
            revision: u32::MAX,
            ..game()
        };
        assert!(
            longest
                .text()
                .iter()
                .all(|(name, said)| name.len() + said.len() < 64)
        );
    }

    #[test]
    fn what_does_not_read_as_a_game_is_none() {
        let told = game();
        let long = "9".repeat(VERSION_LENGTH + 1);
        let junk: [(&str, Option<&str>); 9] = [
            ("game", None),
            ("game", Some("not a number")),
            ("version", Some("")),
            ("version", Some("two words")),
            ("version", Some(&long)),
            ("revision", Some("-1")),
            ("players", Some("many")),
            ("door", Some("ajar")),
            ("door", None),
        ];
        for change in junk {
            assert_eq!(read_with(&told, &[change]), None, "{change:?}");
        }
        // Neither does a game without a port.
        let nowhere = Announcement { port: 0, ..told };
        assert_eq!(read_with(&nowhere, &[]), None);
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
    fn a_host_is_reached_at_the_address_that_is_nearest() {
        let at = |text: &str| text.parse::<Ipv4Addr>().unwrap();
        let own = Some(at("192.168.0.21"));
        let (here, near, far) = (at("127.0.0.1"), at("192.168.0.13"), at("10.8.0.2"));
        assert_eq!(nearest(&[], own), None);
        // On the same network rather than on another, whatever order they come in.
        assert_eq!(nearest(&[far, near, here], own), Some(near));
        assert_eq!(nearest(&[here, near, far], own), Some(near));
        // Any network rather than the address that every machine has for itself.
        assert_eq!(nearest(&[here, far], own), Some(far));
        assert_eq!(nearest(&[here, far], None), Some(far));
        // A game on this very machine is known by the address that others know it by.
        assert_eq!(nearest(&[here, at("192.168.0.21")], own), own);
        // And by its own when it has no other.
        assert_eq!(nearest(&[here], None), Some(here));
    }

    /// What follows talks over this machine alone, on a port of its own: what one test
    /// announces, no other test finds, and nothing of it leaves the machine.
    #[cfg(not(target_arch = "wasm32"))]
    mod on_this_machine {
        use std::time::{Duration, Instant};

        use mdns_sd::{IfKind, ServiceDaemon, ServiceInfo};

        use super::super::*;

        fn here() -> Reach {
            let free = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
            Reach::Here(free.local_addr().unwrap().port())
        }

        /// Takes what a browser learns until something is true of all that it learned.
        fn until(
            browser: &mut Browser,
            seen: &mut Vec<Sighting>,
            what: &str,
            done: impl Fn(&[Sighting]) -> bool,
        ) {
            let start = Instant::now();
            while !done(seen) {
                assert!(
                    start.elapsed() < Duration::from_secs(20),
                    "it never came: {what}. What did: {seen:?}"
                );
                std::thread::sleep(Duration::from_millis(10));
                seen.extend(browser.hear(0.01));
            }
        }

        /// What was last learned of a game, if it is still there.
        fn last(seen: &[Sighting]) -> Option<&Announcement> {
            match seen.last()? {
                Sighting::Game { told, .. } => Some(told),
                Sighting::Gone { .. } => None,
            }
        }

        #[test]
        fn a_game_is_found_kept_up_and_gone_when_its_host_says_goodbye() {
            let reach = here();
            let mut announcer = Announcer::new(reach, 0xFEED).unwrap();
            let told = Announcement::of(0xFEED, 28405, 1, true);
            announcer.tell(&told);
            let (mut browser, mut seen) = (Browser::new(reach).unwrap(), Vec::new());
            until(&mut browser, &mut seen, "the game", |seen| {
                last(seen) == Some(&told)
            });
            let Some(Sighting::Game { name, machines, .. }) = seen.last() else {
                unreachable!();
            };
            assert!(name.starts_with("tanku-000000000000feed."), "{name}");
            assert_eq!(machines, &[Ipv4Addr::LOCALHOST]);

            // A player joined it and its run started: those who look are told.
            let later = Announcement {
                players: 2,
                open: false,
                ..told.clone()
            };
            announcer.tell(&later);
            until(&mut browser, &mut seen, "its news", |seen| {
                last(seen) == Some(&later)
            });

            // Its host closes it, and says so.
            drop(announcer);
            let closed_at = Instant::now();
            until(&mut browser, &mut seen, "its end", |seen| {
                matches!(seen.last(), Some(Sighting::Gone { .. }))
            });
            assert!(closed_at.elapsed() < Duration::from_secs(3));
        }

        #[test]
        fn a_host_that_goes_without_a_word_is_missed_before_long() {
            let reach = here();
            let mut announcer = Announcer::new(reach, 0xDEAD).unwrap();
            let told = Announcement::of(0xDEAD, 28405, 1, true);
            announcer.tell(&told);
            let (mut browser, mut seen) = (Browser::new(reach).unwrap(), Vec::new());
            until(&mut browser, &mut seen, "the game", |seen| {
                last(seen) == Some(&told)
            });
            // The cable of the host is pulled: it hears nothing and says nothing any more.
            announcer.daemon.disable_interface(IfKind::All).unwrap();
            let gone_at = Instant::now();
            until(&mut browser, &mut seen, "its end", |seen| {
                matches!(seen.last(), Some(Sighting::Gone { .. }))
            });
            // Nobody said goodbye, so it took asking, and no answer.
            let waited = gone_at.elapsed().as_secs_f32();
            assert!(
                (3.0..15.0).contains(&waited),
                "it was missed after {waited} s"
            );
            // It stays gone.
            let mut later = Vec::new();
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(6) {
                std::thread::sleep(Duration::from_millis(10));
                later.extend(browser.hear(0.01));
            }
            assert!(last(&later).is_none(), "{later:?}");
        }

        #[test]
        fn what_announces_itself_as_a_game_and_reads_as_none_is_left_out() {
            let reach = here();
            // Something else takes the name of the service, with a text of its own.
            let stranger = reach.daemon().unwrap();
            let text = [("game", "not a number"), ("door", "open")];
            let machine = "stranger.local.";
            let service = ServiceInfo::new(SERVICE, "stranger", machine, "", 9, &text[..])
                .unwrap()
                .enable_addr_auto();
            stranger.register(service).unwrap();
            // A game of another version is a game all the same: it is told as it is.
            let mut announcer = Announcer::new(reach, 0xBEEF).unwrap();
            let other = Announcement {
                revision: REVISION + 1,
                ..Announcement::of(0xBEEF, 4000, 3, true)
            };
            announcer.tell(&other);
            let (mut browser, mut seen) = (Browser::new(reach).unwrap(), Vec::new());
            until(&mut browser, &mut seen, "the game", |seen| {
                last(seen) == Some(&other)
            });
            assert_eq!(other.standing(), Standing::Other);
            // The stranger had the time to be heard, and was not taken for a game.
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(2) {
                std::thread::sleep(Duration::from_millis(10));
                seen.extend(browser.hear(0.01));
            }
            let games = seen.iter().filter_map(|sighting| match sighting {
                Sighting::Game { name, .. } => Some(name.as_str()),
                Sighting::Gone { .. } => None,
            });
            assert!(games.clone().count() > 0);
            assert!(games.clone().all(|name| name.starts_with("tanku-")));
            let _ = stranger.shutdown();
        }

        #[test]
        fn nothing_is_announced_or_found_nowhere() {
            assert!(Announcer::new(Reach::Nowhere, 1).is_none());
            assert!(Browser::new(Reach::Nowhere).is_none());
            let _: Option<ServiceDaemon> = Reach::Nowhere.daemon();
        }
    }

    #[test]
    fn games_are_looked_for_on_the_network_unless_a_test_says_otherwise() {
        assert_eq!(Reach::default(), Reach::Network);
        assert!(SERVICE.ends_with("._udp.local."));
        // The name of the service is short enough for an announcement.
        let name = SERVICE.split('.').next().unwrap();
        assert!(name.starts_with('_') && name.len() <= 15, "{name}");
    }
}
