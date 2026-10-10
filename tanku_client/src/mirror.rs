//! A guest's picture of the run that its host simulates.
//!
//! The host tells where tanks and bullets are as often as it can. What it tells comes a little
//! late and not always evenly, so a guest doesn't show a tank as it comes: it shows the tank a
//! moment in the past, between two tellings, so that the tank moves evenly. A bullet is shown
//! where the host last told that it was.

use std::collections::VecDeque;

use bevy::prelude::*;
use lightyear::prelude::*;
use tanku_core::{
    geom,
    protocol::Told,
    rules::TICK_RATE,
    sim::{Bullet, Motion, Position, PreviousPosition, Tank},
};

use crate::session::Session;

/// How far in the past the tanks are shown, in ticks: enough for the next telling to be there
/// in time, most of the time.
const DELAY: f64 = 3.0;
/// A telling that comes after a longer silence than this, in ticks, says that the tank stood
/// still meanwhile. A shorter silence is a telling that got lost.
const HOLD: u32 = 3;
/// How far the picture of the past may be from where it should be, in ticks, before it starts
/// over from what is told.
const SLACK: f64 = 6.0;
/// How much of what the picture of the past is off by is taken up in a second.
const PULL: f64 = 2.0;
/// The tellings of a tank that are remembered.
const MEMORY: usize = 64;

/// When the picture of a guest is made, in each frame: after what the host told has come in.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Picture;

/// What the host told of a tank at one of its ticks.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fix {
    tick: u32,
    position: Vec2,
    motion: Motion,
}

/// What the host told of a tank the last few times.
#[derive(Component, Debug, Default)]
struct Track {
    told: VecDeque<Fix>,
}

impl Track {
    /// Takes a telling in. After a long silence, the tank stood still until just before.
    fn hear(&mut self, fix: Fix) {
        match self.told.back().copied() {
            Some(last) if fix.tick <= last.tick => return,
            Some(last) if fix.tick - last.tick > HOLD => self.told.push_back(Fix {
                tick: fix.tick - 1,
                ..last
            }),
            _ => {}
        }
        self.told.push_back(fix);
        while self.told.len() > MEMORY {
            self.told.pop_front();
        }
    }

    /// Where the tank was at a moment between two tellings. Before the first, it is where the
    /// first says, and after the last, where the last says.
    fn at(&self, tick: f64) -> Option<(Vec2, Motion)> {
        let after = self.told.partition_point(|fix| (fix.tick as f64) <= tick);
        let before = after.checked_sub(1).map(|index| self.told[index]);
        let (from, to) = match (before, self.told.get(after).copied()) {
            (Some(from), Some(to)) => (from, to),
            (Some(only), None) | (None, Some(only)) => return Some((only.position, only.motion)),
            (None, None) => return None,
        };
        let along = ((tick - from.tick as f64) / (to.tick - from.tick) as f64) as f32;
        let turned = |from: f32, to: f32| from + geom::angle_difference(from, to) * along;
        let motion = Motion {
            velocity: from.motion.velocity.lerp(to.motion.velocity, along),
            hull: turned(from.motion.hull, to.motion.hull),
            turret: turned(from.motion.turret, to.motion.turret),
        };
        Some((from.position.lerp(to.position, along), motion))
    }

    /// Forgets what was told long before a moment.
    fn forget(&mut self, tick: f64) {
        while self.told.len() > 2 && (self.told[1].tick as f64) < tick {
            self.told.pop_front();
        }
    }
}

/// The moment of the host's time at which the tanks are shown: a little behind the last tick
/// that the host told of.
#[derive(Resource, Debug, Default)]
struct Past {
    tick: f64,
    newest: u32,
}

impl Past {
    /// Moves on by a frame. The picture keeps the pace of the frames and drifts toward where
    /// it should be, so that an uneven arrival doesn't show.
    fn advance(&mut self, seconds: f64) {
        let wanted = self.newest as f64 - DELAY;
        self.tick += seconds * TICK_RATE;
        let off = wanted - self.tick;
        if off.abs() > SLACK {
            self.tick = wanted;
        } else {
            self.tick += off * (seconds * PULL).min(1.0);
        }
    }
}

/// The tick of the host that what a guest knows of a thing is true of. That is the last tick
/// at which the host told of the thing itself, or a later one that the guest has all the
/// tellings of: the host then had nothing new to tell of it.
fn heard(checkpoints: &ReplicationCheckpointMap, confirmed: &ConfirmHistory) -> Option<u32> {
    let of_it = checkpoints.get(confirmed.last_tick());
    let of_all = checkpoints.last_confirmed_tick();
    of_it.max(of_all).map(|tick| tick.0)
}

pub fn plugin(app: &mut App) {
    let guest = |session: Res<Session>| *session == Session::Guest;
    app.init_resource::<Past>().add_systems(
        PreUpdate,
        (hear_tanks, show_tanks, show_bullets)
            .chain()
            .in_set(Picture)
            .run_if(guest),
    );
}

/// Takes in what the host told of the tanks. A tank that is new stands where it is told to.
fn hear_tanks(
    mut commands: Commands,
    checkpoints: Res<ReplicationCheckpointMap>,
    mut past: ResMut<Past>,
    mut tanks: Query<
        (
            Entity,
            &Told<Position>,
            &Told<Motion>,
            &ConfirmHistory,
            Option<&mut Track>,
        ),
        (With<Tank>, With<Remote>),
    >,
) {
    for (entity, position, motion, confirmed, track) in &mut tanks {
        let Some(tick) = heard(&checkpoints, confirmed) else {
            continue;
        };
        let fix = Fix {
            tick,
            position: position.0.0,
            motion: motion.0,
        };
        // The host of another game counts its ticks from elsewhere.
        if fix.tick + 2 * (MEMORY as u32) < past.newest {
            *past = Past::default();
        }
        past.newest = past.newest.max(fix.tick);
        match track {
            Some(mut track) => track.hear(fix),
            None => {
                let mut track = Track::default();
                track.hear(fix);
                let place = (Position(fix.position), PreviousPosition(fix.position));
                commands.entity(entity).insert((track, place, fix.motion));
            }
        }
    }
}

/// Shows the tanks where they were a moment ago.
fn show_tanks(
    time: Res<Time<Real>>,
    mut past: ResMut<Past>,
    mut tanks: Query<(
        &mut Track,
        &mut Position,
        &mut PreviousPosition,
        &mut Motion,
    )>,
) {
    past.advance(time.delta_secs_f64());
    for (mut track, mut position, mut previous, mut motion) in &mut tanks {
        let Some((place, moving)) = track.at(past.tick) else {
            continue;
        };
        position.set_if_neq(Position(place));
        previous.set_if_neq(PreviousPosition(place));
        motion.set_if_neq(moving);
        track.forget(past.tick);
    }
}

/// Shows the bullets where the host last told that they were.
fn show_bullets(
    mut commands: Commands,
    mut told: Query<
        (
            Entity,
            &Told<Position>,
            &Told<Bullet>,
            Option<(&mut Bullet, &mut Position, &mut PreviousPosition)>,
        ),
        With<Remote>,
    >,
) {
    for (entity, told_position, told_bullet, shown) in &mut told {
        let place = told_position.0.0;
        match shown {
            Some((mut bullet, mut position, mut previous)) => {
                bullet.set_if_neq(told_bullet.0);
                position.set_if_neq(Position(place));
                previous.set_if_neq(PreviousPosition(place));
            }
            None => {
                let place = (Position(place), PreviousPosition(place));
                commands.entity(entity).insert((told_bullet.0, place));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, f32::consts::PI};

    use tanku_core::sim::TankCommand;

    use super::*;
    use crate::{
        Screen,
        net::tests::{ask, free_port, game, host, line, order, place, run_until, screen, seats},
        session::SessionRequest,
    };

    fn fix(tick: u32, x: f32) -> Fix {
        Fix {
            tick,
            position: Vec2::new(x, 0.0),
            motion: Motion::default(),
        }
    }

    fn tick(game: &App) -> u32 {
        game.world().resource::<LocalTimeline>().tick().0
    }

    /// How many ticks a game is past a tick. A host is behind its guest's clock, so this can
    /// be less than none.
    fn past(game: &App, since: u32) -> i64 {
        tick(game) as i64 - since as i64
    }

    /// A host and a guest in a run of two, on a line that takes so long each way. The guest
    /// has had the time to set its clock by the host's.
    fn pair(late: u64, uneven: u64, lost: f32) -> (App, App) {
        let port = free_port();
        let mut host = host(port);
        let mut guest = game();
        for game in [&mut host, &mut guest] {
            line(game, late, uneven, lost);
        }
        ask(
            &mut guest,
            SessionRequest::Join(format!("127.0.0.1:{port}")),
        );
        run_until("the lobby", &mut [&mut host, &mut guest], |games| {
            screen(games[1]) == Screen::Lobby
        });
        ask(&mut host, SessionRequest::Start);
        run_until("the run", &mut [&mut host, &mut guest], |games| {
            seats(games[1]) == [0, 1]
        });
        let since = tick(&guest);
        run_until("a settled clock", &mut [&mut host, &mut guest], |games| {
            past(games[1], since) > 90
        });
        (host, guest)
    }

    #[test]
    fn a_tank_is_shown_between_two_tellings() {
        let mut track = Track::default();
        assert_eq!(track.at(5.0), None);
        for (tick, x) in [(10, 1.0), (11, 2.0), (12, 4.0)] {
            track.hear(fix(tick, x));
        }
        let place = |track: &Track, tick: f64| track.at(tick).unwrap().0.x;
        assert_eq!(place(&track, 10.5), 1.5);
        assert_eq!(place(&track, 11.25), 2.5);
        // Before the first telling and after the last, the tank is where they say.
        assert_eq!(place(&track, 3.0), 1.0);
        assert_eq!(place(&track, 20.0), 4.0);
        // What was told long before is forgotten, and what is needed stays.
        track.forget(11.5);
        assert_eq!(track.told.len(), 2);
        assert_eq!(place(&track, 11.5), 3.0);
    }

    #[test]
    fn a_turret_turns_the_short_way_between_two_tellings() {
        let turned = |hull: f32| Motion { hull, ..default() };
        let mut track = Track::default();
        track.hear(Fix {
            motion: turned(PI - 0.1),
            ..fix(1, 0.0)
        });
        track.hear(Fix {
            motion: turned(-PI + 0.1),
            ..fix(2, 0.0)
        });
        let hull = track.at(1.5).unwrap().1.hull;
        assert!((hull.abs() - PI).abs() < 1e-5, "{hull}");
    }

    #[test]
    fn a_tank_that_was_not_told_of_for_long_stood_still() {
        let mut track = Track::default();
        track.hear(fix(10, 1.0));
        // Nothing for a second, then the tank has moved: it started just before.
        track.hear(fix(70, 2.0));
        assert_eq!(track.at(40.0).unwrap().0.x, 1.0);
        assert_eq!(track.at(69.0).unwrap().0.x, 1.0);
        assert_eq!(track.at(69.5).unwrap().0.x, 1.5);
        // A telling or two that got lost is no stop: the tank went on meanwhile.
        track.hear(fix(73, 5.0));
        assert_eq!(track.at(71.5).unwrap().0.x, 3.5);
        // What comes late changes nothing.
        track.hear(fix(72, 9.0));
        assert_eq!(track.told.back().unwrap().tick, 73);
    }

    #[test]
    fn the_picture_of_the_past_keeps_an_even_pace() {
        let frame = 1.0 / TICK_RATE;
        // It starts from what is told, a little behind.
        let mut past = Past {
            newest: 1000,
            ..default()
        };
        past.advance(frame);
        assert_eq!(past.tick, 1000.0 - DELAY);
        // Tellings that come two at a time and then not at all don't show: the picture moves
        // on by about a tick in each frame.
        for frame_number in 1..=600u32 {
            past.newest = 1000 + frame_number + frame_number % 2;
            let before = past.tick;
            past.advance(frame);
            let pace = past.tick - before;
            assert!((pace - 1.0).abs() < 0.1, "frame {frame_number}: {pace}");
        }
        let behind = past.newest as f64 - past.tick;
        assert!((behind - DELAY).abs() < 1.0, "{behind}");
    }

    #[test]
    fn a_guest_shows_the_other_tanks_at_an_even_pace() {
        // What the host tells comes unevenly, and some of it is lost.
        let (mut host, mut guest) = pair(30, 12, 0.03);
        let north = TankCommand {
            drive: Vec2::Y,
            ..default()
        };
        let since = tick(&host);
        let seen = RefCell::new((place(&mut guest, 0), Vec::new()));
        run_until("the drive", &mut [&mut host, &mut guest], |games| {
            order(games[0], north);
            let ticks = past(games[0], since);
            let (now, frame) = (
                place(games[1], 0),
                games[1].world().resource::<Time<Real>>(),
            );
            let mut seen = seen.borrow_mut();
            // Once the tank is at its top speed.
            if ticks > 30 && frame.delta_secs() > 0.0 {
                let pace = now.distance(seen.0) / frame.delta_secs();
                seen.1.push(pace);
            }
            seen.0 = now;
            ticks > 120
        });
        let mut paces = seen.into_inner().1;
        paces.sort_by(f32::total_cmp);
        let (count, top) = (paces.len(), 5.0);
        let middle = paces[count / 2];
        assert!(
            (middle - top).abs() < 0.25,
            "the middle pace is {middle} m/s"
        );
        // Hardly a frame shows the tank standing still or leaping.
        let even = paces
            .iter()
            .filter(|pace| (*pace - top).abs() < 1.5)
            .count();
        assert!(
            even * 100 >= count * 95,
            "{even} even frames of {count}: {paces:?}"
        );
    }
}
