//! What the server of a game and its clients tell each other.
//!
//! The server sends the lobby, the run and the state of the simulation: every component listed
//! here goes to the clients as it changes. A client sends what its player asks of their tank,
//! tick by tick.
//!
//! What a client shows in a time of its own is told apart from what it shows. A client doesn't
//! show a tank or a bullet where the server last said it was: it shows the other tanks a
//! moment late, between two tellings, and the bullets where they are by now. And its own tank
//! does at once what its player asks: it drives, fires, lays mines and shields ahead of the
//! server's word. So the place and the motion of tanks and bullets travel as [`Told`], and so
//! do the guns and the shields: each side keeps its own `Position`, `Motion`, `Bullet`, `Gun`
//! and `Shield`.

use bevy::{ecs::entity::MapEntities, prelude::*};
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    balance::FINGERPRINT,
    lock::{Challenge, Proof},
    modes::Difficulty,
    rules::MAX_PLAYERS,
    seed::VERSION,
    sim::{
        Barrier, Block, Bullet, Enemy, ExplosiveCrate, Gun, Mine, Motion, Planned, Player,
        Position, Protected, Reserve, Run, SectorIndex, Shield, SimEvent, Tank, TankCommand, Wreck,
    },
    stats::Genome,
};

/// Counts the changes to what the server and its clients tell each other since the version of
/// the game was last raised. Two builds that don't count the same don't speak the same, so
/// they don't join each other: raise this with every such change.
pub const REVISION: u32 = 1;

/// The port a game listens on unless it is told otherwise.
pub const PORT: u16 = 28405;
/// The port on which it listens for browsers, which don't speak as desktop games do.
pub const WEB_PORT: u16 = 28406;
/// The longest name that a host gives a game, in letters.
pub const NAME_LENGTH: usize = 24;

/// Whether a letter can be part of what a player types for others to read: the lettering of
/// the interface draws it. That is the Latin alphabet with its accents, figures and signs,
/// as the font file of the lettering lists them. Whatever a player types then shows as it
/// was typed, on every machine.
pub fn lettered(letter: char) -> bool {
    matches!(
        letter,
        ' '..='~'
            | '\u{a1}'..='\u{ac}'
            | '\u{ae}'..='\u{113}'
            | '\u{116}'..='\u{12b}'
            | '\u{12d}'..='\u{131}'
            | '\u{133}'..='\u{137}'
            | '\u{139}'..='\u{13e}'
            | '\u{140}'..='\u{148}'
            | '\u{14a}'..='\u{14d}'
            | '\u{14f}'..='\u{17e}'
    )
}

/// The name of a game as it is kept and shown, from what a host typed or from what a
/// machine on the network claims: only letters that the interface draws, no space at its
/// ends, and no more letters than a name has.
pub fn named(text: &str) -> String {
    let written: String = text.chars().filter(|letter| lettered(*letter)).collect();
    // The spaces in front of a name take none of its room.
    let name: String = written.trim_start().chars().take(NAME_LENGTH).collect();
    name.trim_end().to_string()
}

/// What tells one build of the game from another on the network: players must run the same
/// version, builds that speak the same, and builds of the same balance. Each machine works out
/// the tank of its own player by its balance: two that differ would never agree on it.
pub fn protocol_id() -> u64 {
    told_apart(VERSION, REVISION, FINGERPRINT)
}

/// The number that the builds of a version, of a revision and of a balance go by.
fn told_apart(version: &str, revision: u32, balance: u64) -> u64 {
    let bytes = version.bytes().chain(revision.to_le_bytes());
    bytes
        .chain(balance.to_le_bytes())
        .fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
            (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01B3)
        })
}

/// Who sits in a seat of a game that several players share.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Member {
    /// The player whose machine runs the game.
    Host,
    /// A player who joined, by the number that their machine goes by on the network.
    Guest(u64),
}

/// The lobby of a game that several players share: who is there, and what their next run is.
/// The host decides, and the guests are told.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lobby {
    /// Who sits where. A seat is the number of a player in a run.
    pub seats: [Option<Member>; MAX_PLAYERS as usize],
    /// The name that the host gave the game, or nothing.
    pub name: String,
    /// Whether the game has a password, which whoever joins must type.
    pub locked: bool,
    /// Whether the game is a playtest of its host: its runs set no record and earn no
    /// achievement, for anyone in them.
    pub playtest: bool,
    pub seed: String,
    /// Whether the host typed the seed, which makes it a chosen one: a run on it earns no
    /// achievement.
    pub typed: bool,
    /// How hard the next run is.
    pub difficulty: Difficulty,
    /// Whether the next run is in free mode, in which each player has the tank that they
    /// built in their own garage.
    pub free: bool,
    /// Whether the next run is in arena mode: in one room, against one group after another.
    pub arena: bool,
    /// Who has said that they are ready for the run, by seat.
    pub ready: [bool; MAX_PLAYERS as usize],
    /// The seconds until the run starts by itself, once everyone is ready.
    pub countdown: Option<u8>,
}

/// What a host tells one guest who knocks at a game that has a password.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Door {
    /// The game has a password: this is what a proof of it must answer.
    Locked(Challenge),
    /// The proof was not the one of the password.
    Refused,
}

/// What a guest tells its host, besides what its player asks of their tank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Word {
    /// The proof that its player typed the password of the game.
    Proof(Proof),
    /// The tank that its player built in their garage, for a run in free mode.
    Tank(Genome),
    /// Whether its player is ready for the run.
    Ready(bool),
}

impl Lobby {
    /// How many players are there.
    pub fn players(&self) -> u32 {
        self.seats.iter().flatten().count() as u32
    }

    /// The seat of a member, if they have one.
    pub fn seat_of(&self, member: Member) -> Option<u8> {
        let seat = self.seats.iter().position(|taken| *taken == Some(member));
        seat.map(|seat| seat as u8)
    }

    /// Whether everyone who is there has said that they are ready.
    pub fn all_ready(&self) -> bool {
        let mut seated = self.seats.iter().zip(self.ready);
        self.players() > 0 && seated.all(|(seat, ready)| seat.is_none() || ready)
    }

    /// Sits everyone anew. Whoever was ready is still ready in their new seat, and whoever
    /// has just come is not.
    pub fn reseat(&mut self, seats: [Option<Member>; MAX_PLAYERS as usize]) {
        let was_ready = |member: Member| {
            let seat = self.seat_of(member);
            seat.is_some_and(|seat| self.ready[seat as usize])
        };
        let ready = seats.map(|member| member.is_some_and(was_ready));
        (self.seats, self.ready) = (seats, ready);
    }
}

/// What the server tells of a part of a tank or of a bullet: where it is, how it moves, what
/// its gun holds and how its shield is.
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Told<T: Send + Sync + 'static>(pub T);

/// The run, as the server tells it to the clients.
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct RunState(pub Run);

/// The channel of what must arrive, in the order it was sent.
pub struct Reliable;

// A command names no entity.
impl MapEntities for TankCommand {
    fn map_entities<M: EntityMapper>(&mut self, _: &mut M) {}
}

/// Registers what goes over the network. It comes after the plugins of the network library,
/// and is the same on both sides.
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(input::native::InputPlugin::<TankCommand>::default());

        app.register_message::<SimEvent>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<Door>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<Word>()
            .add_direction(NetworkDirection::ClientToServer);
        app.add_channel::<Reliable>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);

        app.component::<Lobby>().replicate();
        app.component::<RunState>().replicate();
        app.component::<SectorIndex>().replicate();
        // What stays where it is has its place told as it is.
        app.component::<Position>()
            .replicate_filtered::<(Without<Tank>, Without<Bullet>)>();
        app.component::<Told<Position>>().replicate();
        app.component::<Told<Motion>>().replicate();
        app.component::<Told<Bullet>>().replicate();
        app.component::<Told<Gun>>().replicate();
        app.component::<Told<Shield>>().replicate();
        app.component::<Tank>().replicate();
        app.component::<Player>().replicate();
        app.component::<Enemy>().replicate();
        app.component::<Wreck>().replicate();
        app.component::<Protected>().replicate();
        app.component::<Mine>().replicate();
        app.component::<Block>().replicate();
        app.component::<ExplosiveCrate>().replicate();
        app.component::<Barrier>().replicate();
        app.component::<Reserve>().replicate();
        app.component::<Planned>().replicate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_that_speak_differently_tell_each_other_apart() {
        // The same build always goes by the same number.
        assert_eq!(protocol_id(), protocol_id());
        // Another version, another revision of what is told or another balance goes by
        // another.
        assert_eq!(protocol_id(), told_apart(VERSION, REVISION, FINGERPRINT));
        assert_ne!(
            protocol_id(),
            told_apart(VERSION, REVISION + 1, FINGERPRINT)
        );
        assert_ne!(protocol_id(), told_apart("0.0.0", REVISION, FINGERPRINT));
        assert_ne!(
            protocol_id(),
            told_apart(VERSION, REVISION, FINGERPRINT + 1)
        );
        // The balance of the game as it was before one of its numbers changed.
        let before = crate::balance::fingerprint("tanku-balance 1\nblast 3m\n");
        let after = crate::balance::fingerprint("tanku-balance 1\nblast 2.5m\n");
        assert_ne!(
            told_apart(VERSION, REVISION, before),
            told_apart(VERSION, REVISION, after)
        );
    }

    #[test]
    fn a_name_keeps_what_is_written_and_no_more_than_it_has_room_for() {
        assert_eq!(named("  Friday night \n"), "Friday night");
        assert_eq!(named("Soir\u{e9}e\tchez\u{0} Dara"), "Soir\u{e9}echez Dara");
        // A name is counted in letters, not in bytes.
        let long = "\u{e9}".repeat(NAME_LENGTH + 6);
        assert_eq!(named(&long).chars().count(), NAME_LENGTH);
        // What is cut at a space ends before it.
        let cut = format!("{} tail", "a".repeat(NAME_LENGTH - 1));
        assert_eq!(named(&cut), "a".repeat(NAME_LENGTH - 1));
        assert_eq!(named(" \t "), "");
        // The spaces in front of a name take none of its room.
        let indented = format!("   {}", "a".repeat(NAME_LENGTH));
        assert_eq!(named(&indented), "a".repeat(NAME_LENGTH));
        // What the interface has no letter for is left out: it would show as nothing.
        assert_eq!(
            named("Tanks \u{1F600} \u{418}\u{433}\u{440}\u{430} 4"),
            "Tanks   4"
        );
        assert!(lettered('\u{17e}') && !lettered('\u{a0}') && !lettered('\u{7f}'));
    }

    #[test]
    fn a_lobby_tells_who_sits_where() {
        let mut lobby = Lobby::default();
        assert_eq!(lobby.players(), 0);
        lobby.seats[0] = Some(Member::Host);
        lobby.seats[2] = Some(Member::Guest(7));
        assert_eq!(lobby.players(), 2);
        assert_eq!(lobby.seat_of(Member::Host), Some(0));
        assert_eq!(lobby.seat_of(Member::Guest(7)), Some(2));
        assert_eq!(lobby.seat_of(Member::Guest(8)), None);
    }

    #[test]
    fn a_lobby_knows_when_everyone_is_ready() {
        let mut lobby = Lobby::default();
        // Nobody is there: nobody is ready.
        assert!(!lobby.all_ready());
        lobby.seats[0] = Some(Member::Host);
        lobby.seats[1] = Some(Member::Guest(7));
        assert!(!lobby.all_ready());
        lobby.ready[0] = true;
        assert!(!lobby.all_ready());
        lobby.ready[1] = true;
        assert!(lobby.all_ready());
        // A seat that nobody has waits for nobody, whatever was left on it.
        lobby.ready[3] = true;
        assert!(lobby.all_ready());
        lobby.ready[0] = false;
        assert!(!lobby.all_ready());
    }

    #[test]
    fn whoever_was_ready_stays_so_in_a_new_seat_and_whoever_comes_is_not() {
        let (host, first, second, third) = (
            Some(Member::Host),
            Some(Member::Guest(7)),
            Some(Member::Guest(8)),
            Some(Member::Guest(9)),
        );
        let mut lobby = Lobby {
            seats: [host, first, second, None],
            ready: [true, false, true, false],
            ..default()
        };
        // The first guest leaves: the second one moves up, and is still ready.
        lobby.reseat([host, second, None, None]);
        assert_eq!(lobby.seats, [host, second, None, None]);
        assert_eq!(lobby.ready, [true, true, false, false]);
        // A third one comes, and takes the seat that the second one had: they are not ready.
        lobby.reseat([host, second, third, None]);
        assert_eq!(lobby.ready, [true, true, false, false]);
        assert!(!lobby.all_ready());
        // Nothing changes for those who stay where they are.
        lobby.ready[2] = true;
        lobby.reseat([host, second, third, None]);
        assert_eq!(lobby.ready, [true, true, true, false]);
    }
}
