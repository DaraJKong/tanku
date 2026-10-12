//! Help for the tests of this crate and of the others, which take it with the feature
//! `testing`.

use std::{
    net::{Ipv4Addr, TcpListener, UdpSocket},
    sync::{
        Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

/// The first port that tests take, and how many they take theirs from. They lie below the
/// ports that a system hands out to whoever asks for any free one: those start at 32768 on
/// Linux, and at 49152 on Windows and macOS.
const FIRST: u32 = 10_000;
const PORTS: u32 = 15_000;
/// How far apart the rows of two processes start when their numbers follow each other: more
/// ports than the tests of one process take.
const ROW: u32 = 128;

/// A port that is free on this machine, for a test to open a game on or to announce one on.
///
/// Asking the system for any free port and letting go of it won't do. The system hands that
/// port to whoever asks next, be it another test or the socket of a guest, and now and then
/// it does so before the game of the test has taken the port: the test then finds its port
/// taken. So the tests take their ports from a row that the system hands to nobody, and each
/// call gives the next port of the row.
///
/// Two runs of the tests at once must not take the same port either. The row of a process
/// starts where the number of the process says, which keeps most runs apart. And a process
/// keeps each port that it takes, for as long as it lives, by listening on the TCP port of
/// the same number, which no test uses: the system lets one process do that, and the others
/// pass that port over.
pub fn free_port() -> u16 {
    static TAKEN: AtomicU32 = AtomicU32::new(0);
    static KEPT: Mutex<Vec<TcpListener>> = Mutex::new(Vec::new());
    let start = std::process::id().wrapping_mul(ROW);
    loop {
        let next = TAKEN.fetch_add(1, Ordering::Relaxed);
        let port = (FIRST + start.wrapping_add(next) % PORTS) as u16;
        let Ok(kept) = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)) else {
            continue;
        };
        // A port that another program holds for itself is passed over too.
        if UdpSocket::bind((Ipv4Addr::UNSPECIFIED, port)).is_ok() {
            KEPT.lock().unwrap().push(kept);
            return port;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_call_gives_another_port_that_the_system_hands_to_nobody() {
        let ports: Vec<u16> = (0..100).map(|_| free_port()).collect();
        let distinct: BTreeSet<&u16> = ports.iter().collect();
        assert_eq!(distinct.len(), ports.len());
        let row = FIRST..FIRST + PORTS;
        assert!(ports.iter().all(|port| row.contains(&(*port as u32))));
        // Each is this process's own: nothing else can keep it, and a game can take it.
        for port in &ports {
            assert!(TcpListener::bind((Ipv4Addr::UNSPECIFIED, *port)).is_err());
            assert!(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, *port)).is_ok());
        }
        // On Linux, the system says which ports it hands out: the row must lie below them,
        // or a test may find its port taken now and then.
        if let Ok(range) = std::fs::read_to_string("/proc/sys/net/ipv4/ip_local_port_range") {
            let lowest = range
                .split_whitespace()
                .next()
                .and_then(|low| low.parse().ok());
            let lowest: u32 = lowest.expect("the lowest port that the system hands out");
            assert!(
                row.end <= lowest,
                "this system hands out ports from {lowest} on, among those of the tests"
            );
        }
    }
}
