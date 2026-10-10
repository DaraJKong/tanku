//! A guest's picture of the run that its host simulates.
//!
//! The host tells where tanks and bullets are as often as it can. What it tells comes a little
//! late and not always evenly, so a guest shows none of it as it comes:
//!
//! - The other tanks are shown a moment in the past, between two tellings, so that they move
//!   evenly.
//! - The tank of the guest's own player does at once what the player asks. The guest moves it
//!   as the host will. When the host tells where the tank was, the guest starts again from
//!   there and does again what was asked since.
//! - A bullet flies straight and bounces off what stands still, so the guest knows where it
//!   is by now. It shows the bullet there, in the time of its player's tank, and not where it
//!   was when the host told: what comes at the tank is where the screen shows it.

use std::collections::VecDeque;

use bevy::prelude::*;
use lightyear::prelude::*;
use tanku_core::{
    geom,
    protocol::Told,
    rules::{TICK, TICK_RATE},
    sim::{
        Barrier, Block, Bullet, ExplosiveCrate, Motion, Position, PreviousPosition, Run, Sector,
        Tank, TankCommand, Wreck,
    },
};
use tanku_server::{
    bullets::{self, Flight},
    field::{self, Field},
    tanks,
};

use crate::{crew::Own, scene::Nudge, session::Session};

/// How far in the past the other tanks are shown, in ticks: enough for the next telling to be
/// there in time, most of the time.
const DELAY: f64 = 3.0;
/// A telling that comes after a longer silence than this, in ticks, says that the tank stood
/// still meanwhile. A shorter silence is a telling that got lost.
const HOLD: u32 = 3;
/// How far the picture of the past may be from where it should be, in ticks, before it starts
/// over from what is told.
const SLACK: f64 = 6.0;
/// How much of what the picture of the past is off by is taken up in a second.
const PULL: f64 = 2.0;
/// The ticks of orders that are remembered, and the furthest that a guest runs ahead of what
/// it was told. Beyond that, its clock is not the host's yet, and it shows what it is told.
const MEMORY: usize = 64;
const REACH: u32 = 40;
/// Seconds in which a tank that was not where the guest thought is brought to its place, and
/// the distance beyond which it is put there at once.
const EASE: f32 = 0.1;
const JUMP: f32 = 3.0;

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

/// The moment of the host's time at which the other tanks are shown: a little behind the last
/// tick that the host told of.
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

/// What the player asked of their tank at each of the last ticks.
#[derive(Resource, Debug, Default)]
struct Orders {
    asked: VecDeque<(u32, TankCommand)>,
}

impl Orders {
    fn note(&mut self, tick: u32, command: TankCommand) {
        self.asked.retain(|(noted, _)| *noted < tick);
        self.asked.push_back((tick, command));
        while self.asked.len() > MEMORY {
            self.asked.pop_front();
        }
    }

    /// What was asked at a tick. A tick that nothing was noted for takes what was asked last
    /// before it, as the host does when an order doesn't reach it in time.
    fn at(&self, tick: u32) -> TankCommand {
        let noted = self.asked.iter().rev().find(|(noted, _)| *noted <= tick);
        noted.map_or_else(TankCommand::default, |(_, command)| *command)
    }
}

/// The tank of this guest's player, which the guest moves ahead of the host.
#[derive(Component, Debug, Default)]
struct Ahead {
    /// The tick of the telling that the tank was last moved on from.
    heard: Option<u32>,
}

/// A bullet that the guest flies ahead of the host.
#[derive(Component, Debug)]
struct Flown {
    /// The tick of the telling that the bullet was last flown on from.
    heard: u32,
    /// Whether the bullet flew into something that ends it. The host will take it away.
    spent: bool,
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
    app.init_resource::<Past>()
        .init_resource::<Orders>()
        .add_observer(shift_orders)
        .add_systems(
            PreUpdate,
            (
                keep_field,
                field::rebuild_colliders.run_if(resource_exists::<Field>),
                (take_the_helm, hear_tanks),
                show_tanks,
            )
                .chain()
                .in_set(Picture)
                .run_if(guest),
        )
        .add_systems(
            FixedUpdate,
            (foresee, fly)
                .run_if(guest)
                .run_if(resource_exists::<Field>),
        )
        .add_systems(Update, ease.before(crate::scene::place));
}

/// A guest keeps its own account of what is solid, from the sectors that it laid out and from
/// what the host told of the blocks, the crates and the barriers.
fn keep_field(
    mut commands: Commands,
    run: Option<Res<Run>>,
    field: Option<ResMut<Field>>,
    fresh: Query<
        (),
        Or<(
            Added<Sector>,
            Added<Block>,
            Added<ExplosiveCrate>,
            Changed<Barrier>,
        )>,
    >,
    (mut sectors, mut blocks, mut crates, mut barriers): (
        RemovedComponents<Sector>,
        RemovedComponents<Block>,
        RemovedComponents<ExplosiveCrate>,
        RemovedComponents<Barrier>,
    ),
) {
    let gone = sectors.read().count() + blocks.read().count();
    let gone = gone + crates.read().count() + barriers.read().count();
    match (run, field) {
        (Some(run), Some(mut field)) if field.seed == run.seed => {
            if gone > 0 || !fresh.is_empty() {
                field.stale = true;
            }
        }
        // A new run, maybe of another host: what was asked in the last one is of no use.
        (Some(run), _) => {
            commands.insert_resource(Field::new(run.seed, run.players));
            commands.insert_resource(Orders::default());
        }
        (None, Some(_)) => commands.remove_resource::<Field>(),
        (None, None) => {}
    }
}

/// The tank of the guest's own player is the one that the guest moves ahead.
fn take_the_helm(
    mut commands: Commands,
    tanks: Query<(Entity, Has<Own>, Has<Ahead>), (With<Tank>, With<Remote>)>,
) {
    for (entity, own, ahead) in &tanks {
        match (own, ahead) {
            (true, false) => {
                commands
                    .entity(entity)
                    .insert((Ahead::default(), Nudge::default()));
            }
            (false, true) => {
                commands.entity(entity).remove::<(Ahead, Nudge)>();
            }
            _ => {}
        }
    }
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

/// Shows the tanks that the guest doesn't move itself where they were a moment ago.
fn show_tanks(
    time: Res<Time<Real>>,
    mut past: ResMut<Past>,
    mut tanks: Query<
        (
            &mut Track,
            &mut Position,
            &mut PreviousPosition,
            &mut Motion,
        ),
        Without<Ahead>,
    >,
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

/// When the clock of the guest is set anew, the ticks of what was asked move with it.
fn shift_orders(shift: On<LocalTimelineShift>, mut orders: ResMut<Orders>) {
    for (tick, _) in &mut orders.asked {
        *tick = tick.wrapping_add_signed(shift.delta);
    }
}

/// Moves the tank of the guest's own player by a tick, as the host will when the order of this
/// tick reaches it. When the host has told where the tank was, the tank starts again from
/// there and does again what was asked since.
fn foresee(
    timeline: Res<LocalTimeline>,
    checkpoints: Res<ReplicationCheckpointMap>,
    field: Res<Field>,
    mut orders: ResMut<Orders>,
    mut own: Query<
        (
            &Tank,
            &TankCommand,
            (&Told<Position>, &Told<Motion>, &ConfirmHistory),
            (&mut Position, &mut PreviousPosition, &mut Motion),
            &mut Ahead,
            &mut Nudge,
            Has<Wreck>,
        ),
        With<Own>,
    >,
    others: Query<(&Position, &Tank, Has<Wreck>), Without<Own>>,
) {
    let now = timeline.tick().0;
    for (tank, command, told, shown, mut ahead, mut nudge, wreck) in &mut own {
        let (told_position, told_motion, confirmed) = told;
        let (mut position, mut previous, mut motion) = shown;
        orders.note(now, *command);
        let told_at = heard(&checkpoints, confirmed);
        let ahead_by = told_at.map(|told_at| now.wrapping_sub(told_at));
        // A wreck stays where the host has it, and so does a tank while the clock of the guest
        // is not yet the host's.
        if wreck || ahead_by.is_none_or(|ticks| ticks == 0 || ticks > REACH) {
            position.0 = told_position.0.0;
            previous.0 = position.0;
            *motion = told_motion.0;
            *ahead = Ahead::default();
            nudge.0 = Vec2::ZERO;
            continue;
        }
        let step = |place: &mut Vec2, moving: &mut Motion, asked: &TankCommand| {
            tanks::step(&field, tank, true, asked, place, moving);
            let others = others.iter().map(|(other, tank, wreck)| {
                let gives_way = tank.genome.top_speed() > 0.0 && !wreck;
                (other.0, gives_way)
            });
            tanks::give_way(place, others);
            field.push_out(tank, place, &mut moving.velocity);
        };
        let (mut place, mut moving) = (position.0, *motion);
        previous.0 = place;
        step(&mut place, &mut moving, command);
        if let Some(told_at) = told_at.filter(|told_at| ahead.heard != Some(*told_at)) {
            ahead.heard = Some(told_at);
            let thought = place;
            (place, moving) = (told_position.0.0, told_motion.0);
            for tick in 1..now.wrapping_sub(told_at) {
                step(
                    &mut place,
                    &mut moving,
                    &orders.at(told_at.wrapping_add(tick)),
                );
            }
            previous.0 = place;
            step(&mut place, &mut moving, command);
            // The tank is drawn where the guest thought it was, and brought to its place.
            let off = thought - place;
            nudge.0 = if off.length() > JUMP {
                Vec2::ZERO
            } else {
                nudge.0 + off
            };
        }
        position.0 = place;
        *motion = moving;
    }
}

/// Flies a bullet for some ticks. Returns where it was a tick before the end, and whether it
/// flew into something that ends it.
fn flown(field: &Field, position: &mut Vec2, bullet: &mut Bullet, ticks: u32) -> (Vec2, bool) {
    let mut before = *position;
    for _ in 0..ticks {
        before = *position;
        let distance = bullet.velocity.length() * TICK;
        let flight = bullets::fly_past_walls(field, position, bullet, distance);
        if flight.end != Flight::Flying {
            return (before, true);
        }
    }
    (before, false)
}

/// Flies the bullets to where they are by now, from where the host last told that they were.
fn fly(
    mut commands: Commands,
    timeline: Res<LocalTimeline>,
    checkpoints: Res<ReplicationCheckpointMap>,
    field: Res<Field>,
    mut told: Query<
        (
            Entity,
            &Told<Position>,
            &Told<Bullet>,
            &ConfirmHistory,
            Option<(
                &mut Bullet,
                &mut Position,
                &mut PreviousPosition,
                &mut Flown,
            )>,
        ),
        With<Remote>,
    >,
) {
    let now = timeline.tick().0;
    for (entity, told_position, told_bullet, confirmed, shown) in &mut told {
        let Some(told_at) = heard(&checkpoints, confirmed) else {
            continue;
        };
        // From what the host told, as many ticks as the guest is ahead of it.
        let afresh = || {
            let (mut place, mut bullet) = (told_position.0.0, told_bullet.0);
            let ticks = Some(now.wrapping_sub(told_at)).filter(|ticks| *ticks <= REACH);
            let (before, spent) = flown(&field, &mut place, &mut bullet, ticks.unwrap_or(0));
            (bullet, place, before, spent)
        };
        let Some((mut bullet, mut position, mut previous, mut state)) = shown else {
            let (bullet, place, before, spent) = afresh();
            let flown = Flown {
                heard: told_at,
                spent,
            };
            let hidden = hidden(spent);
            let place = (Position(place), PreviousPosition(before));
            commands
                .entity(entity)
                .insert((bullet, place, flown, hidden));
            continue;
        };
        if state.heard != told_at {
            let was_spent = state.spent;
            let flying;
            (flying, position.0, previous.0, state.spent) = afresh();
            bullet.set_if_neq(flying);
            state.heard = told_at;
            if state.spent != was_spent {
                commands.entity(entity).insert(hidden(state.spent));
            }
        } else if !state.spent {
            let mut flying = *bullet;
            (previous.0, state.spent) = flown(&field, &mut position.0, &mut flying, 1);
            bullet.set_if_neq(flying);
            if state.spent {
                commands.entity(entity).insert(hidden(true));
            }
        }
    }
}

/// A bullet that flew into something on this side is not shown any more.
fn hidden(spent: bool) -> Visibility {
    if spent {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    }
}

/// Brings what is drawn off its place back to it.
fn ease(time: Res<Time<Real>>, mut nudged: Query<&mut Nudge>) {
    let kept = (-time.delta_secs() / EASE).exp();
    for mut nudge in &mut nudged {
        if nudge.0 != Vec2::ZERO {
            nudge.0 = if nudge.0.length() < 1e-3 {
                Vec2::ZERO
            } else {
                nudge.0 * kept
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap, f32::consts::PI};

    use tanku_core::sim::Player;

    use super::*;
    use crate::{
        Screen,
        net::tests::{ask, free_port, game, host, line, order, place, run_until, screen, seats},
        session::SessionRequest,
    };

    /// What a game has at each of its ticks. A test keeps it to compare what a guest shows at
    /// a tick with what its host has once it gets to that tick.
    #[derive(Resource, Default)]
    struct Log {
        tanks: HashMap<(u32, u8), Vec2>,
        bullets: HashMap<u32, Vec<Vec2>>,
    }

    fn keep_log(
        timeline: Res<LocalTimeline>,
        mut log: ResMut<Log>,
        tanks: Query<(&Player, &Position)>,
        bullets: Query<(&Position, Option<&Flown>), With<Bullet>>,
    ) {
        let tick = timeline.tick().0;
        for (player, position) in &tanks {
            log.tanks.insert((tick, player.seat), position.0);
        }
        let flying = bullets
            .iter()
            .filter(|(_, flown)| flown.is_none_or(|flown| !flown.spent));
        let places = flying.map(|(position, _)| position.0).collect();
        log.bullets.insert(tick, places);
    }

    fn tick(game: &App) -> u32 {
        game.world().resource::<LocalTimeline>().tick().0
    }

    /// How many ticks a game is past a tick. A host is behind its guest's clock, so this can
    /// be less than none.
    fn past(game: &App, since: u32) -> i64 {
        tick(game) as i64 - since as i64
    }

    /// A host and a guest in a run of two, on a line that takes so long each way. Both keep a
    /// log, and the guest has had the time to set its clock by the host's.
    fn pair(late: u64, uneven: u64, lost: f32) -> (App, App) {
        let port = free_port();
        let mut host = host(port);
        let mut guest = game();
        for game in [&mut host, &mut guest] {
            line(game, late, uneven, lost);
            game.init_resource::<Log>().add_systems(FixedLast, keep_log);
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
            let world = games[1].world_mut();
            let mut helms = world.query::<&Ahead>();
            let ahead = helms.iter(world).any(|ahead| ahead.heard.is_some());
            ahead && past(games[1], since) > 90
        });
        (host, guest)
    }

    /// How far apart the two games have something, at the ticks that both have a log of.
    fn apart(host: &App, guest: &App, seat: u8, ticks: std::ops::Range<u32>) -> Vec<f32> {
        let (there, here) = (
            host.world().resource::<Log>(),
            guest.world().resource::<Log>(),
        );
        let both = ticks.filter_map(|tick| {
            let (there, here) = (
                there.tanks.get(&(tick, seat))?,
                here.tanks.get(&(tick, seat))?,
            );
            Some(there.distance(*here))
        });
        both.collect()
    }

    #[test]
    fn a_guest_moves_its_own_tank_at_once_and_to_where_the_host_will_have_it() {
        // A line that takes 120 ms there and back.
        let (mut host, mut guest) = pair(60, 0, 0.0);
        let drive = |drive: Vec2| TankCommand {
            drive,
            aim: Vec2::new(40.0, 40.0),
            ..default()
        };

        // The tank moves on the guest's screen before the host has heard of the order.
        let start = place(&mut guest, 1);
        order(&mut guest, drive(Vec2::Y));
        let asked_at = std::time::Instant::now();
        run_until("a first move", &mut [&mut host, &mut guest], |games| {
            place(games[1], 1).y > start.y + 1e-3
        });
        assert!(
            asked_at.elapsed().as_millis() < 60,
            "{:?}",
            asked_at.elapsed()
        );
        assert_eq!(place(&mut host, 1), start);

        // North, then east, then a stop. All the way, the guest has the tank where the host
        // gets to have it at that tick, and nothing has to be put right.
        let since = tick(&guest);
        let off = RefCell::new(0.0f32);
        run_until("the drive", &mut [&mut host, &mut guest], |games| {
            let ticks = past(games[1], since);
            let asked = match ticks {
                ..40 => drive(Vec2::Y),
                40..80 => drive(Vec2::X),
                _ => drive(Vec2::ZERO),
            };
            order(games[1], asked);
            let world = games[1].world_mut();
            let nudge = world.query::<&Nudge>().single(world).unwrap().0.length();
            off.replace_with(|worst| worst.max(nudge));
            past(games[0], since) > 110
        });
        let apart = apart(&host, &guest, 1, since + 2..since + 100);
        let worst = apart.iter().copied().fold(0.0, f32::max);
        assert!(apart.len() > 90, "{} ticks to compare", apart.len());
        assert!(worst < 1e-3, "the tank was up to {worst} m from its place");
        assert!(
            *off.borrow() < 1e-3,
            "it was put right by {} m",
            off.borrow()
        );
        // It did drive, and around a corner.
        let end = place(&mut host, 1);
        assert!(
            end.y > start.y + 2.0 && end.x > start.x + 2.0,
            "{start} to {end}"
        );
    }

    #[test]
    fn a_tank_that_stood_still_for_long_starts_at_once() {
        let (mut host, mut guest) = pair(60, 0, 0.0);
        // Nothing moves for six seconds: the host has nothing to tell of the tanks.
        let since = tick(&guest);
        let told = RefCell::new((None, 0));
        run_until("the wait", &mut [&mut host, &mut guest], |games| {
            let world = games[1].world_mut();
            let mut tanks = world.query_filtered::<&ConfirmHistory, With<Player>>();
            let last: Vec<_> = tanks.iter(world).map(ConfirmHistory::last_tick).collect();
            let last = Some(last);
            let mut told = told.borrow_mut();
            if told.0 != last {
                *told = (last, told.1 + 1);
            }
            past(games[1], since) > 360
        });
        let tellings = told.borrow().1;
        assert!(tellings < 10, "the host told of them {tellings} times");
        let start = place(&mut guest, 1);
        let north = TankCommand {
            drive: Vec2::Y,
            ..default()
        };
        order(&mut guest, north);
        let asked_at = std::time::Instant::now();
        run_until("a first move", &mut [&mut host, &mut guest], |games| {
            place(games[1], 1).y > start.y + 1e-3
        });
        assert!(
            asked_at.elapsed().as_millis() < 60,
            "{:?}",
            asked_at.elapsed()
        );
        // And it goes where the host will have it.
        let since = tick(&guest);
        run_until("the drive", &mut [&mut host, &mut guest], |games| {
            past(games[0], since) > 60
        });
        let apart = apart(&host, &guest, 1, since + 2..since + 50);
        let worst = apart.iter().copied().fold(0.0, f32::max);
        assert!(apart.len() > 40, "{} ticks to compare", apart.len());
        assert!(worst < 1e-3, "the tank was up to {worst} m from its place");
    }

    #[test]
    fn a_guest_stops_its_own_tank_at_a_wall_as_the_host_does() {
        let (mut host, mut guest) = pair(60, 0, 0.0);
        let east = TankCommand {
            drive: Vec2::X,
            ..default()
        };
        // Three seconds eastward end at the wall of the bay.
        let since = tick(&guest);
        run_until("the wall", &mut [&mut host, &mut guest], |games| {
            order(games[1], east);
            past(games[0], since) > 190
        });
        let apart = apart(&host, &guest, 1, since + 2..since + 180);
        let worst = apart.iter().copied().fold(0.0, f32::max);
        assert!(apart.len() > 170, "{} ticks to compare", apart.len());
        assert!(worst < 1e-3, "the tank was up to {worst} m from its place");
        // It stands against the wall on both sides, a tank's radius from it.
        let log = guest.world().resource::<Log>();
        let (late, later) = (log.tanks[&(since + 150, 1)], log.tanks[&(since + 170, 1)]);
        assert_eq!(late, later);
        let field = guest.world().resource::<Field>();
        assert!(!field.sees(late, late + Vec2::X * 1.0));
        assert!(field.sees(late, late + Vec2::X * 0.8));
    }

    #[test]
    fn a_guest_shows_a_bullet_where_the_host_will_have_it() {
        let (mut host, mut guest) = pair(60, 0, 0.0);
        // The host's player aims west, at a wall of the bay and away from the guest's tank,
        // then fires one shot. It comes back off the wall and passes north of both tanks.
        let at = place(&mut host, 0);
        let aim = |fire: bool| TankCommand {
            aim: at + Vec2::new(-30.0, 5.0),
            fire,
            ..default()
        };
        let bullets = |game: &mut App| {
            let world = game.world_mut();
            world.query::<&Bullet>().iter(world).count()
        };
        let since = tick(&host);
        run_until("the aim", &mut [&mut host, &mut guest], |games| {
            order(games[0], aim(false));
            past(games[0], since) > 30
        });
        run_until("the shot", &mut [&mut host, &mut guest], |games| {
            let fired = bullets(games[0]) > 0;
            order(games[0], aim(!fired));
            fired
        });
        run_until("the flight", &mut [&mut host, &mut guest], |games| {
            bullets(games[0]) == 0 && bullets(games[1]) == 0
        });

        // At each tick that both saw the bullet fly, they had it at the same place: before it
        // bounced off the wall, and after.
        let (there, here) = (
            host.world().resource::<Log>(),
            guest.world().resource::<Log>(),
        );
        let mut same: Vec<(u32, Vec2, f32)> = Vec::new();
        for (tick, places) in &here.bullets {
            if let (&[here], Some(&[there])) =
                (&places[..], there.bullets.get(tick).map(Vec::as_slice))
            {
                same.push((*tick, here, here.distance(there)));
            }
        }
        same.sort_by_key(|(tick, ..)| *tick);
        let worst = same.iter().map(|(.., apart)| *apart).fold(0.0, f32::max);
        assert!(same.len() > 30, "{} ticks to compare", same.len());
        assert!(
            worst < 1e-3,
            "the bullet was up to {worst} m from its place"
        );
        let west = same
            .iter()
            .map(|(_, place, _)| place.x)
            .fold(f32::MAX, f32::min);
        let last = same.last().unwrap().1.x;
        assert!(
            last - west > 1.0,
            "it came back {} m from the wall",
            last - west
        );
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

    fn fix(tick: u32, x: f32) -> Fix {
        Fix {
            tick,
            position: Vec2::new(x, 0.0),
            motion: Motion::default(),
        }
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
    fn what_was_asked_is_remembered_by_tick() {
        let drive = |x: f32| TankCommand {
            drive: Vec2::new(x, 0.0),
            ..default()
        };
        let mut orders = Orders::default();
        assert_eq!(orders.at(5), TankCommand::default());
        orders.note(5, drive(1.0));
        orders.note(6, drive(2.0));
        orders.note(8, drive(3.0));
        assert_eq!(orders.at(6), drive(2.0));
        // A tick with no order takes the one before it.
        assert_eq!(orders.at(7), drive(2.0));
        assert_eq!(orders.at(9), drive(3.0));
        // A tick that comes again replaces what was noted for it and after it.
        orders.note(6, drive(4.0));
        assert_eq!(orders.at(8), drive(4.0));
        for tick in 9..200 {
            orders.note(tick, drive(0.0));
        }
        assert_eq!(orders.asked.len(), MEMORY);
    }
}
