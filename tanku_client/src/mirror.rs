//! A guest's picture of the run that its host simulates.
//!
//! The host tells where tanks and bullets are as often as it can. What it tells comes a little
//! late and not always evenly, so a guest shows none of it as it comes:
//!
//! - The other tanks are shown a moment in the past, between two tellings, so that they move
//!   evenly.
//! - The tank of the guest's own player does at once what the player asks: it drives, fires,
//!   lays mines and raises its shield. The guest does all of it as the host will. When the
//!   host tells how the tank was, the guest starts again from there and does again what was
//!   asked since. The bullet or the mine that the guest showed meanwhile makes way for the
//!   one that the host tells of, and what the host says of a shot that the guest has sounded
//!   already is not sounded again.
//! - A bullet flies straight and bounces off what stands still, so the guest knows where it
//!   is by now. It shows the bullet there, in the time of its player's tank, and not where it
//!   was when the host told: what comes at the tank is where the screen shows it, and ends
//!   where it meets the tank or its bubble.

use std::{collections::VecDeque, mem::discriminant};

use bevy::prelude::*;
use lightyear::prelude::*;
use tanku_core::{
    geom,
    protocol::Told,
    rules::{BULLET_RADIUS, MINE_ARMING_TIME, SHIELD_RADIUS, TANK_RADIUS, TICK, TICK_RATE},
    sim::{
        Barrier, Block, Bullet, Enemy, ExplosiveCrate, Gun, Kind, Mine, MineState, Motion,
        Position, PreviousPosition, Run, Sector, Shield, SimEvent, Tank, TankCommand, Wreck,
    },
};
use tanku_server::{
    bullets::{self, Flight, Quarry},
    field::{self, Field},
    tanks::{self, Bubble},
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
/// How many ticks apart a shot is in two accounts of what the tank did, and is still the same
/// shot. An order that reaches the host late is obeyed a tick or two late.
const LATITUDE: u32 = 2;
/// How long the guest waits for the host to tell of what it has shown already, in ticks, and
/// how near to it the host's word must be, in meters.
const AWAITED: u32 = 120;
const NEAR: f32 = 1.5;

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

/// Whether a tick comes after another.
fn after(tick: u32, other: u32) -> bool {
    (tick.wrapping_sub(other) as i32) > 0
}

/// Where something happened, when the host tells where.
fn place_of(event: &SimEvent) -> Option<Vec2> {
    match *event {
        SimEvent::Shot { at, .. }
        | SimEvent::Ricochet { at }
        | SimEvent::BulletDestroyed { at, .. }
        | SimEvent::MinePlaced { at }
        | SimEvent::ShieldRaised { at }
        | SimEvent::ShieldBlocked { at }
        | SimEvent::ShieldDropped { at } => Some(at),
        _ => None,
    }
}

/// What the guest has shown and sounded ahead of its host. The host tells of it a moment
/// later, and that must not show a second time.
#[derive(Resource, Debug, Default)]
pub struct Echoes {
    awaited: Vec<(u32, SimEvent)>,
}

impl Echoes {
    /// Notes what the guest has just shown, at a tick of its clock.
    fn expect(&mut self, now: u32, event: SimEvent) {
        self.awaited
            .retain(|(shown_at, _)| now.wrapping_sub(*shown_at) <= AWAITED);
        self.awaited.push((now, event));
    }

    /// Whether the host tells here of something that the guest has shown already: the same
    /// kind of thing at about the same place, not long ago. What was shown answers for one
    /// telling.
    pub fn claims(&mut self, now: u32, told: &SimEvent) -> bool {
        let Some(there) = place_of(told) else {
            return false;
        };
        let answers = |(shown_at, shown): &(u32, SimEvent)| {
            now.wrapping_sub(*shown_at) <= AWAITED
                && discriminant(shown) == discriminant(told)
                && place_of(shown).is_some_and(|here| here.distance(there) <= NEAR)
        };
        let found = self.awaited.iter().position(answers);
        found.map(|index| self.awaited.remove(index)).is_some()
    }
}

/// Something that the tank of the guest's own player did at a tick, ahead of the host's word.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Deed {
    /// A shot from where the tank stood, with its turret turned so.
    Shot {
        from: Vec2,
        turret: f32,
    },
    Mine {
        at: Vec2,
    },
    Raised {
        at: Vec2,
    },
    Dropped {
        at: Vec2,
    },
}

/// A deed that the guest has shown and sounded, and the bullet or the mine that it shows for
/// it until the host's word of that tick is in.
#[derive(Debug)]
struct Shown {
    tick: u32,
    deed: Deed,
    thing: Option<Entity>,
}

impl Shown {
    /// Whether this is a deed of an account of what the tank did: the same kind of deed, at
    /// about the same tick.
    fn is(&self, tick: u32, deed: &Deed) -> bool {
        discriminant(&self.deed) == discriminant(deed) && self.tick.abs_diff(tick) <= LATITUDE
    }
}

/// The tank of this guest's player, which the guest runs ahead of the host.
#[derive(Component, Debug, Default)]
struct Ahead {
    /// The tick of the telling that the tank was last run on from.
    heard: Option<u32>,
    /// What the tank did since, which the host has yet to tell of.
    shown: Vec<Shown>,
}

/// How the tank of the guest's own player is at a tick.
#[derive(Clone, Copy, Debug)]
struct Helm {
    place: Vec2,
    moving: Motion,
    gun: Gun,
    shield: Shield,
}

/// A bullet or a mine that the guest shows ahead of the host's word.
#[derive(Component, Debug)]
struct Foreseen;

/// A bullet that the guest flies ahead of the host.
#[derive(Component, Debug)]
struct Flown {
    /// The tick of the telling that the bullet was last flown on from. For a bullet that the
    /// guest shows ahead of the host's word, the tick of its shot.
    heard: u32,
    /// Whether the bullet flew into something that ends it. The host will take it away.
    spent: bool,
    /// The tick at which the bullet came upon the tank of the guest's own player, or upon its
    /// bubble, which ends it too. That holds until the host tells otherwise. It is the tick at
    /// which the host gets to end the bullet, whatever the clock of the guest says by then.
    stopped: Option<u32>,
}

impl Flown {
    /// Whether the bullet is at its end as far as the guest can tell.
    fn gone(&self) -> bool {
        self.spent || self.stopped.is_some()
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
    // While the host holds the run, nothing moves there, and nothing moves here ahead of it.
    let held = |run: Option<Res<Run>>| run.is_some_and(|run| run.paused);
    app.init_resource::<Past>()
        .init_resource::<Orders>()
        .init_resource::<Echoes>()
        .add_observer(shift_ticks)
        .add_observer(take_back)
        .add_systems(
            PreUpdate,
            (
                keep_field,
                field::rebuild_colliders.run_if(resource_exists::<Field>),
                (take_the_helm, hear_tanks, hear_shields),
                show_tanks,
            )
                .chain()
                .in_set(Picture)
                .run_if(guest),
        )
        .add_systems(
            FixedUpdate,
            (foresee, fly)
                .chain()
                .run_if(guest)
                .run_if(not(held))
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
        // A new run, maybe of another host: what was asked and shown in the last one is of
        // no use.
        (Some(run), _) => {
            commands.insert_resource(Field::new(run.seed, run.players, run.difficulty));
            commands.insert_resource(Orders::default());
            commands.insert_resource(Echoes::default());
        }
        (None, Some(_)) => commands.remove_resource::<Field>(),
        (None, None) => {}
    }
}

/// The tank of the guest's own player is the one that the guest runs ahead. Its gun is the
/// one that the guest shows.
fn take_the_helm(
    mut commands: Commands,
    tanks: Query<(Entity, Has<Own>, Has<Ahead>, &Told<Gun>), (With<Tank>, With<Remote>)>,
) {
    for (entity, own, ahead, gun) in &tanks {
        match (own, ahead) {
            (true, false) => {
                commands
                    .entity(entity)
                    .insert((Ahead::default(), Nudge::default(), gun.0));
            }
            (false, true) => {
                commands.entity(entity).remove::<(Ahead, Nudge)>();
            }
            _ => {}
        }
    }
}

/// A tank that the guest no longer runs ahead takes back what it had shown of it.
fn take_back(gone: On<Remove, Ahead>, helms: Query<&Ahead>, mut commands: Commands) {
    let things = helms.get(gone.entity).map(|ahead| &ahead.shown);
    for shown in things.into_iter().flatten() {
        if let Some(thing) = shown.thing {
            commands.entity(thing).try_despawn();
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

/// The shields of the tanks that the guest doesn't run itself are as the host tells them.
fn hear_shields(mut tanks: Query<(&Told<Shield>, &mut Shield), Without<Ahead>>) {
    for (told, mut shield) in &mut tanks {
        shield.set_if_neq(told.0);
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

/// When the clock of the guest is set anew, the ticks of what was asked and shown move with
/// it. The tick at which a bullet ended on the tank of the guest's own player stays: the
/// host ends the bullet at that tick whatever the clock of the guest says, and the guest has
/// shown its end already.
fn shift_ticks(
    shift: On<LocalTimelineShift>,
    mut orders: ResMut<Orders>,
    mut echoes: ResMut<Echoes>,
    mut helms: Query<&mut Ahead>,
) {
    let moved = |tick: &mut u32| *tick = tick.wrapping_add_signed(shift.delta);
    orders.asked.iter_mut().for_each(|(tick, _)| moved(tick));
    echoes.awaited.iter_mut().for_each(|(tick, _)| moved(tick));
    for mut ahead in &mut helms {
        ahead
            .shown
            .iter_mut()
            .for_each(|shown| moved(&mut shown.tick));
    }
}

/// Runs the tank of the guest's own player by a tick, as the host will when the order of this
/// tick reaches it: the tank drives, aims, shields, fires and lays mines. When the host has
/// told how the tank was, the tank starts again from there and does again what was asked
/// since.
///
/// What the tank does shows at once: its bullet and its mine are the guest's own until the
/// host's word of that tick is in. From then on the host's are shown, or nothing, if the host
/// had it otherwise.
fn foresee(
    mut commands: Commands,
    timeline: Res<LocalTimeline>,
    checkpoints: Res<ReplicationCheckpointMap>,
    field: Res<Field>,
    (mut orders, mut echoes): (ResMut<Orders>, ResMut<Echoes>),
    mut events: MessageWriter<SimEvent>,
    mut own: Query<
        (
            Entity,
            &Tank,
            &TankCommand,
            (
                &Told<Position>,
                &Told<Motion>,
                &Told<Gun>,
                &Told<Shield>,
                &ConfirmHistory,
            ),
            (
                &mut Position,
                &mut PreviousPosition,
                &mut Motion,
                &mut Gun,
                &mut Shield,
            ),
            &mut Ahead,
            &mut Nudge,
            Has<Wreck>,
        ),
        With<Own>,
    >,
    others: Query<(&Position, &Tank, Option<&Enemy>), (Without<Own>, Without<Wreck>)>,
    mut bullets: Query<
        (
            &mut Bullet,
            &mut Position,
            &mut PreviousPosition,
            &mut Flown,
        ),
        (With<Foreseen>, Without<Tank>),
    >,
) {
    let now = timeline.tick().0;
    // The tanks that a seeking bullet of the player may leave a wall toward, where the guest
    // shows them.
    let quarry: Vec<Quarry> = others
        .iter()
        .filter(|(.., enemy)| enemy.is_none_or(|enemy| enemy.awake))
        .map(|(position, tank, _)| (position.0, tank.team))
        .collect();
    for (entity, tank, command, told, shown, mut ahead, mut nudge, wreck) in &mut own {
        let (told_position, told_motion, told_gun, told_shield, confirmed) = told;
        let (mut position, mut previous, mut motion, mut gun, mut shield) = shown;
        orders.note(now, *command);
        let told = Helm {
            place: told_position.0.0,
            moving: told_motion.0,
            gun: told_gun.0,
            shield: told_shield.0,
        };
        let told_at = heard(&checkpoints, confirmed);
        let ahead_by = told_at.map(|told_at| now.wrapping_sub(told_at));
        // A wreck is as the host has it, and so is a tank while the clock of the guest is not
        // yet the host's.
        if wreck || ahead_by.is_none_or(|ticks| ticks == 0 || ticks > REACH) {
            (position.0, previous.0) = (told.place, told.place);
            *motion = told.moving;
            gun.set_if_neq(told.gun);
            shield.set_if_neq(told.shield);
            for shown in std::mem::take(&mut *ahead).shown {
                if let Some(thing) = shown.thing {
                    commands.entity(thing).try_despawn();
                }
            }
            nudge.0 = Vec2::ZERO;
            continue;
        }

        let step = |helm: &mut Helm, asked: &TankCommand, tick: u32, did: &mut Vec<(u32, Deed)>| {
            if let Some(cooldown) = tank.genome.shield_cooldown() {
                let at = helm.place;
                match tanks::step_shield(&mut helm.shield, cooldown, asked.shield) {
                    Some(Bubble::Raised) => did.push((tick, Deed::Raised { at })),
                    Some(Bubble::Dropped) => did.push((tick, Deed::Dropped { at })),
                    None => {}
                }
            }
            tanks::step(&field, tank, true, asked, &mut helm.place, &mut helm.moving);
            // The tanks that run are in its way, and the wrecks are not.
            let others = others
                .iter()
                .map(|(other, tank, _)| (other.0, tank.genome.top_speed() > 0.0));
            tanks::give_way(&mut helm.place, others);
            field.push_out(tank, &mut helm.place, &mut helm.moving.velocity);
            let worked = tanks::step_gun(&mut helm.gun, &tank.genome, asked);
            if worked.fired {
                let (from, turret) = (helm.place, helm.moving.turret);
                did.push((tick, Deed::Shot { from, turret }));
            }
            if worked.laid {
                did.push((tick, Deed::Mine { at: helm.place }));
            }
        };

        let mut helm = Helm {
            place: position.0,
            moving: *motion,
            gun: *gun,
            shield: *shield,
        };
        let mut did: Vec<(u32, Deed)> = Vec::new();
        previous.0 = helm.place;
        step(&mut helm, command, now, &mut did);
        let again = told_at.filter(|told_at| ahead.heard != Some(*told_at));
        if let Some(told_at) = again {
            ahead.heard = Some(told_at);
            let thought = helm.place;
            helm = told;
            did.clear();
            for tick in 1..now.wrapping_sub(told_at) {
                let tick = told_at.wrapping_add(tick);
                step(&mut helm, &orders.at(tick), tick, &mut did);
            }
            previous.0 = helm.place;
            step(&mut helm, command, now, &mut did);
            // The tank is drawn where the guest thought it was, and brought to its place.
            let off = thought - helm.place;
            nudge.0 = if off.length() > JUMP {
                Vec2::ZERO
            } else {
                nudge.0 + off
            };
            // The host's word of a tick settles what the tank did in it: its bullet or its
            // mine is there as the host tells it, or there is none. So what the guest showed
            // goes once it is not in this new account of the ticks since that word.
            ahead.shown.retain(|shown| {
                let stands = did.iter().any(|(tick, deed)| shown.is(*tick, deed));
                if let (false, Some(thing)) = (stands, shown.thing) {
                    commands.entity(thing).try_despawn();
                }
                stands
            });
        }
        position.0 = helm.place;
        *motion = helm.moving;
        gun.set_if_neq(helm.gun);
        shield.set_if_neq(helm.shield);

        // The bullets that the guest shows for the tank fly on.
        for shown in &ahead.shown {
            let Some(Ok(bullet)) = shown.thing.map(|thing| bullets.get_mut(thing)) else {
                continue;
            };
            let (mut bullet, mut place, mut before, mut state) = bullet;
            if state.spent {
                continue;
            }
            let mut flying = *bullet;
            let flew = flown(&field, &mut place.0, &mut flying, 1, &quarry);
            before.0 = flew.way[0];
            bullet.set_if_neq(flying);
            if flew.spent {
                state.spent = true;
                commands.entity(shown.thing.unwrap()).insert(hidden(true));
            }
        }

        // What the tank did and the guest has not shown yet shows now, and sounds.
        let team = tank.team;
        for (tick, deed) in did {
            if ahead.shown.iter().any(|shown| shown.is(tick, &deed)) {
                continue;
            }
            let mut tell = |event: SimEvent| {
                events.write(event);
                echoes.expect(now, event);
            };
            let thing = match deed {
                Deed::Shot { from, turret } => {
                    let shot = tanks::shoot(&field, entity, tank, from, turret);
                    tell(SimEvent::Shot {
                        at: shot.muzzle,
                        team,
                    });
                    match shot.end {
                        Flight::Stopped(_) => {
                            tell(SimEvent::BulletDestroyed { at: shot.at, team });
                            None
                        }
                        Flight::Flying => {
                            let (mut at, mut bullet) = (shot.at, shot.bullet);
                            // The bullet flies on in the tick of its shot, and in each since.
                            let ticks = 1 + now.wrapping_sub(tick);
                            let flew = flown(&field, &mut at, &mut bullet, ticks, &quarry);
                            let state = Flown {
                                heard: tick,
                                spent: flew.spent,
                                stopped: None,
                            };
                            let place = (Position(at), PreviousPosition(flew.way[0]));
                            let look = hidden(flew.spent);
                            Some(commands.spawn((Foreseen, bullet, place, state, look)).id())
                        }
                    }
                }
                Deed::Mine { at } => {
                    tell(SimEvent::MinePlaced { at });
                    let mine = Mine {
                        owner: entity,
                        team,
                        sector: tank.sector,
                        state: MineState::Arming(MINE_ARMING_TIME),
                    };
                    Some(commands.spawn((Foreseen, mine, Position(at))).id())
                }
                Deed::Raised { at } => {
                    tell(SimEvent::ShieldRaised { at });
                    None
                }
                Deed::Dropped { at } => {
                    tell(SimEvent::ShieldDropped { at });
                    None
                }
            };
            ahead.shown.push(Shown { tick, deed, thing });
        }
    }
}

/// How a bullet flew for some ticks.
struct Flew {
    /// The way it took in the last of them: where it was before, where it bounced, where it
    /// is now.
    way: Vec<Vec2>,
    /// Whether it flew into something that ends it.
    spent: bool,
}

/// Flies a bullet for some ticks, past what stands still. A seeking bullet leaves a wall
/// toward the tank that it seeks, where the guest shows that tank. A splitting bullet is at
/// its end once its time is up: the host tells of the two that it becomes.
fn flown(
    field: &Field,
    position: &mut Vec2,
    bullet: &mut Bullet,
    ticks: u32,
    quarry: &[Quarry],
) -> Flew {
    let mut flew = Flew {
        way: vec![*position],
        spent: false,
    };
    for _ in 0..ticks {
        if let Kind::Splitting { fuse, bounces } = bullet.kind {
            if fuse <= TICK {
                flew.spent = true;
                break;
            }
            let fuse = fuse - TICK;
            bullet.kind = Kind::Splitting { fuse, bounces };
        }
        let from = *position;
        let distance = bullet.velocity.length() * TICK;
        let flight = bullets::fly_past_walls(field, position, bullet, distance, quarry);
        flew.way.clear();
        flew.way.push(from);
        flew.way.extend(&flight.bounces);
        flew.way.push(*position);
        if flight.end != Flight::Flying {
            flew.spent = true;
            break;
        }
    }
    flew
}

/// Where a bullet that went this way came upon a circle, if it did.
fn met(way: &[Vec2], center: Vec2, radius: f32) -> Option<Vec2> {
    way.windows(2).find_map(|ends| {
        let (from, delta) = (ends[0], ends[1] - ends[0]);
        let along = geom::sweep_circle_point(from, delta, center, radius + BULLET_RADIUS)?;
        Some(from + delta * along)
    })
}

/// Flies the bullets to where they are by now, from where the host last told that they were.
///
/// A bullet that comes upon the tank of the guest's own player ends there, or on its bubble:
/// the guest has both where the host will have them. It doesn't wait for the host to say so,
/// and sounds the bubble at once. A bullet flies over that tank once it is a wreck.
fn fly(
    mut commands: Commands,
    timeline: Res<LocalTimeline>,
    checkpoints: Res<ReplicationCheckpointMap>,
    field: Res<Field>,
    mut echoes: ResMut<Echoes>,
    mut events: MessageWriter<SimEvent>,
    own: Query<(&Position, &Shield), (With<Ahead>, Without<Told<Bullet>>, Without<Wreck>)>,
    tanks: Query<(&Tank, &Position, Option<&Enemy>, Has<Wreck>), Without<Told<Bullet>>>,
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
    let quarry = bullets::quarry(tanks.iter());
    let own = own.single().ok().map(|(position, shield)| {
        let radius = if shield.is_active() {
            SHIELD_RADIUS
        } else {
            TANK_RADIUS
        };
        (position.0, radius, shield.is_active())
    });
    // Ends a bullet that came upon the tank of the guest's own player in its last tick.
    let mut meet = |state: &mut Flown, flew: &Flew, bullet: &Bullet| {
        let Some((center, radius, shielded)) = own.filter(|_| !state.gone()) else {
            return;
        };
        let Some(at) = met(&flew.way, center, radius) else {
            return;
        };
        state.stopped = Some(now);
        if shielded {
            let team = bullet.team;
            for event in [
                SimEvent::ShieldBlocked { at: center },
                SimEvent::BulletDestroyed { at, team },
            ] {
                events.write(event);
                echoes.expect(now, event);
            }
        }
    };
    for (entity, told_position, told_bullet, confirmed, shown) in &mut told {
        let Some(told_at) = heard(&checkpoints, confirmed) else {
            continue;
        };
        // From what the host told, as many ticks as the guest is ahead of it.
        let afresh = || {
            let (mut place, mut bullet) = (told_position.0.0, told_bullet.0);
            let ticks = Some(now.wrapping_sub(told_at)).filter(|ticks| *ticks <= REACH);
            let flew = flown(&field, &mut place, &mut bullet, ticks.unwrap_or(0), &quarry);
            (bullet, place, flew)
        };
        let Some((mut bullet, mut position, mut previous, mut state)) = shown else {
            let (bullet, place, flew) = afresh();
            let mut state = Flown {
                heard: told_at,
                spent: flew.spent,
                stopped: None,
            };
            meet(&mut state, &flew, &bullet);
            let place = (Position(place), PreviousPosition(flew.way[0]));
            let look = hidden(state.gone());
            commands.entity(entity).insert((bullet, place, state, look));
            continue;
        };
        let was_gone = state.gone();
        if state.heard != told_at {
            let (flying, place, flew) = afresh();
            (position.0, previous.0) = (place, flew.way[0]);
            bullet.set_if_neq(flying);
            state.heard = told_at;
            state.spent = flew.spent;
            // The host still has the bullet after the tick at which the guest saw it end on
            // its player's tank: it didn't end there.
            if state
                .stopped
                .is_some_and(|stopped| !after(stopped, told_at))
            {
                state.stopped = None;
            }
            meet(&mut state, &flew, &flying);
        } else if !state.spent {
            let mut flying = *bullet;
            let flew = flown(&field, &mut position.0, &mut flying, 1, &quarry);
            previous.0 = flew.way[0];
            bullet.set_if_neq(flying);
            state.spent = flew.spent;
            meet(&mut state, &flew, &flying);
        }
        if state.gone() != was_gone {
            commands.entity(entity).insert(hidden(state.gone()));
        }
    }
}

/// A bullet that is at its end on this side is not shown any more.
fn hidden(gone: bool) -> Visibility {
    if gone {
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
        net::tests::{
            ask, code, free_port, game, host_with, line, order, place, run_until, screen, seats,
        },
        session::SessionRequest,
    };

    /// What a game has at each of its ticks, and what it showed and sounded. A test keeps it to
    /// compare what a guest shows at a tick with what its host has once it gets to that tick.
    #[derive(Resource, Default)]
    struct Log {
        tanks: HashMap<(u32, u8), Vec2>,
        bullets: HashMap<u32, Vec<Vec2>>,
        mines: HashMap<u32, Vec<Vec2>>,
        /// The gun and the shield of the tank of a seat, where the game has both.
        arms: HashMap<(u32, u8), (Gun, Shield)>,
        news: Vec<(u32, SimEvent)>,
    }

    fn keep_log(
        timeline: Res<LocalTimeline>,
        mut log: ResMut<Log>,
        tanks: Query<(&Player, &Position, Option<&Gun>, &Shield)>,
        bullets: Query<(&Position, Option<&Flown>), With<Bullet>>,
        mines: Query<&Position, With<Mine>>,
    ) {
        let tick = timeline.tick().0;
        for (player, position, gun, shield) in &tanks {
            log.tanks.insert((tick, player.seat), position.0);
            if let Some(gun) = gun {
                log.arms.insert((tick, player.seat), (*gun, *shield));
            }
        }
        let flying = bullets
            .iter()
            .filter(|(_, flown)| flown.is_none_or(|flown| !flown.gone()));
        let places = flying.map(|(position, _)| position.0).collect();
        log.bullets.insert(tick, places);
        let places = mines.iter().map(|position| position.0).collect();
        log.mines.insert(tick, places);
    }

    fn keep_news(
        timeline: Res<LocalTimeline>,
        mut log: ResMut<Log>,
        mut events: MessageReader<SimEvent>,
    ) {
        let tick = timeline.tick().0;
        log.news.extend(events.read().map(|event| (tick, *event)));
    }

    /// How many times a game showed and sounded something.
    fn news(game: &App, of: impl Fn(&SimEvent) -> bool) -> usize {
        let log = game.world().resource::<Log>();
        log.news.iter().filter(|(_, event)| of(event)).count()
    }

    /// The bullets that a game shows.
    fn bullets(game: &mut App) -> usize {
        let world = game.world_mut();
        let mut bullets = world.query_filtered::<Option<&Flown>, With<Bullet>>();
        let shown = bullets
            .iter(world)
            .filter(|flown| flown.is_none_or(|flown| !flown.gone()));
        shown.count()
    }

    /// The ticks at which a game had some of a thing, from the first to the last, and how
    /// many it had at most and at least in between.
    fn span(had: &HashMap<u32, Vec<Vec2>>) -> Option<(u32, u32, usize, usize)> {
        let ticks = had.iter().filter(|(_, places)| !places.is_empty());
        let (first, last) = (
            ticks.clone().map(|(tick, _)| *tick).min()?,
            ticks.map(|(tick, _)| *tick).max()?,
        );
        let counts = (first..=last).filter_map(|tick| had.get(&tick).map(Vec::len));
        let (most, least) = (counts.clone().max()?, counts.min()?);
        Some((first, last, most, least))
    }

    /// How far apart two games have the one thing of a kind that each has, at the ticks at
    /// which each has one.
    fn apart_at_each_tick(
        there: &HashMap<u32, Vec<Vec2>>,
        here: &HashMap<u32, Vec<Vec2>>,
    ) -> Vec<f32> {
        let both = here.iter().filter_map(|(tick, here)| {
            match (&here[..], there.get(tick).map(Vec::as_slice)) {
                (&[here], Some(&[there])) => Some(here.distance(there)),
                _ => None,
            }
        });
        both.collect()
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
        pair_by(late, uneven, lost, false, false)
    }

    /// The same, with a host that takes browsers too if it is asked to, and a guest that
    /// joins the way they do if it is.
    fn pair_by(late: u64, uneven: u64, lost: f32, web: bool, as_browser: bool) -> (App, App) {
        let port = free_port();
        let mut host = host_with(port, web.then(free_port));
        let mut guest = game();
        for game in [&mut host, &mut guest] {
            line(game, late, uneven, lost);
            game.init_resource::<Log>()
                .add_systems(FixedLast, keep_log)
                .add_systems(Last, keep_news);
        }
        let at = if as_browser {
            code(&host)
        } else {
            format!("127.0.0.1:{port}")
        };
        ask(&mut guest, SessionRequest::Join(at));
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
        drive_around_a_corner(&mut host, &mut guest, start);
    }

    /// Drives the tank of the guest's player north, then east, then stops it, and checks
    /// that the guest had it where its host got to have it at each tick.
    fn drive_around_a_corner(host: &mut App, guest: &mut App, start: Vec2) {
        let drive = |drive: Vec2| TankCommand {
            drive,
            aim: Vec2::new(40.0, 40.0),
            ..default()
        };
        let since = tick(guest);
        let off = RefCell::new(0.0f32);
        run_until("the drive", &mut [&mut *host, &mut *guest], |games| {
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
        let apart = apart(host, guest, 1, since + 2..since + 100);
        let worst = apart.iter().copied().fold(0.0, f32::max);
        assert!(apart.len() > 90, "{} ticks to compare", apart.len());
        assert!(worst < 1e-3, "the tank was up to {worst} m from its place");
        assert!(
            *off.borrow() < 1e-3,
            "it was put right by {} m",
            off.borrow()
        );
        // It did drive, and around a corner.
        let end = place(host, 1);
        assert!(
            end.y > start.y + 2.0 && end.x > start.x + 2.0,
            "{start} to {end}"
        );
    }

    #[test]
    fn a_guest_has_its_tank_where_a_host_that_takes_browsers_will_have_it() {
        // The host listens for desktop games and for browsers at once. A guest that came
        // either way has its tank where the host gets to have it, on a line that takes
        // 120 ms there and back.
        for as_browser in [false, true] {
            let (mut host, mut guest) = pair_by(60, 0, 0.0, true, as_browser);
            let start = place(&mut guest, 1);
            drive_around_a_corner(&mut host, &mut guest, start);
        }
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
        let apart = apart_at_each_tick(&there.bullets, &here.bullets);
        let worst = apart.iter().copied().fold(0.0, f32::max);
        assert!(apart.len() > 30, "{} ticks to compare", apart.len());
        assert!(
            worst < 1e-3,
            "the bullet was up to {worst} m from its place"
        );
        let shown = here.bullets.values().flatten();
        let west = shown.clone().map(|place| place.x).fold(f32::MAX, f32::min);
        let (_, last, ..) = span(&here.bullets).unwrap();
        let back = here.bullets[&last][0].x - west;
        assert!(back > 1.0, "it came back {back} m from the wall");
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

    /// A host and a guest whose player has turned their turret east, at a wall of the bay and
    /// away from the tank of the host. Returns what the guest's player asks to keep it there.
    fn aiming_east(late: u64) -> (App, App, TankCommand) {
        let (mut host, mut guest) = pair(late, 0, 0.0);
        let aim = TankCommand {
            aim: place(&mut guest, 1) + Vec2::new(30.0, 5.0),
            ..default()
        };
        let since = tick(&guest);
        run_until("the aim", &mut [&mut host, &mut guest], |games| {
            order(games[1], aim);
            past(games[1], since) > 30
        });
        (host, guest, aim)
    }

    #[test]
    fn a_guest_fires_at_once_and_shows_its_bullet_where_the_host_will_have_it() {
        // A line that takes 120 ms there and back.
        let (mut host, mut guest, aim) = aiming_east(60);
        let fire = TankCommand { fire: true, ..aim };
        let shots = |game: &App| news(game, |event| matches!(event, SimEvent::Shot { .. }));

        // The shot shows and sounds before the host has heard of the order.
        order(&mut guest, fire);
        let asked_at = std::time::Instant::now();
        run_until("a shot", &mut [&mut host, &mut guest], |games| {
            bullets(games[1]) > 0
        });
        assert!(
            asked_at.elapsed().as_millis() < 60,
            "{:?}",
            asked_at.elapsed()
        );
        assert_eq!((bullets(&mut host), shots(&host)), (0, 0));
        guest.update();
        assert_eq!(shots(&guest), 1);
        // The round is out of the gun that the guest shows.
        let world = guest.world_mut();
        let gun = world.query_filtered::<&Gun, With<Own>>().single(world);
        assert_eq!(gun.unwrap().rounds, 4);

        let fired_at = tick(&guest);
        order(&mut guest, aim);
        run_until("the flight", &mut [&mut host, &mut guest], |games| {
            let flown = past(games[0], fired_at) > 30;
            flown && bullets(games[0]) == 0 && bullets(games[1]) == 0
        });
        let (there, here) = (
            host.world().resource::<Log>(),
            guest.world().resource::<Log>(),
        );
        // The guest showed one bullet all along, from the tick at which the host fired it to
        // the tick at which the host saw it end: its own at first, then the one that the
        // host told of.
        let (first, last, most, least) = span(&here.bullets).unwrap();
        assert_eq!((most, least), (1, 1), "from tick {first} to tick {last}");
        let (fired, ended, ..) = span(&there.bullets).unwrap();
        assert_eq!((first, last), (fired, ended));
        // It was where the host had it at each tick, before it bounced off the wall and after.
        let apart = apart_at_each_tick(&there.bullets, &here.bullets);
        let worst = apart.iter().copied().fold(0.0, f32::max);
        assert!(apart.len() > 60, "{} ticks to compare", apart.len());
        assert!(
            worst < 1e-3,
            "the bullet was up to {worst} m from its place"
        );
        // What the host told of the shot didn't sound a second time.
        assert_eq!((shots(&host), shots(&guest)), (1, 1));
    }

    /// Lets go of the buttons once a tick has taken what was asked, so that a test asks for
    /// one shot, however many ticks its frame holds.
    fn let_go_after_a_tick(mut tanks: Query<&mut TankCommand, With<Own>>) {
        for mut asked in &mut tanks {
            if asked.fire || asked.mine || asked.shield {
                (asked.fire, asked.mine, asked.shield) = (false, false, false);
            }
        }
    }

    #[test]
    fn a_shot_is_one_shot_though_the_clock_of_the_guest_is_set_anew() {
        // lightyear sets the clock of a guest anew, by whole ticks, once it is more than ten
        // ticks off: ahead after the guest stood still for a moment, back after its host
        // did, and both ways when the line changes. When it will can't be told, so the clock
        // is set by hand here, by less than lightyear would put right at once.
        for by in [6, -6] {
            let (mut host, mut guest, aim) = aiming_east(60);
            guest.add_systems(FixedLast, let_go_after_a_tick);
            let shots = |game: &App| news(game, |event| matches!(event, SimEvent::Shot { .. }));

            // The guest shows and sounds its shot, and its clock is set anew before the host
            // has heard of the order.
            order(&mut guest, TankCommand { fire: true, ..aim });
            run_until("a shot", &mut [&mut host, &mut guest], |games| {
                bullets(games[1]) > 0
            });
            assert_eq!((bullets(&mut host), shots(&host)), (0, 0));
            let (fired_at, since) = (tick(&guest), tick(&host));
            guest.world_mut().trigger(LocalTimelineShift { delta: by });
            assert_eq!(past(&guest, fired_at), by as i64);
            // Its player raises the shield at once. With a clock that was set back, the guest
            // shows the bubble at a tick before the one of the shot, and the shot is still
            // one that it waits for the host to tell of.
            order(
                &mut guest,
                TankCommand {
                    shield: true,
                    ..aim
                },
            );
            run_until("the host's word", &mut [&mut host, &mut guest], |games| {
                past(games[0], since) > 45
            });

            // The host fired once. The guest took what its player had asked for the same
            // shot as before, and the host's word of it for the shot that it had sounded.
            assert_eq!(
                (shots(&host), shots(&guest)),
                (1, 1),
                "with a clock set {by} ticks from where it was"
            );
        }
    }

    /// By how many ticks lightyear set the clock of a game anew, each time it did.
    #[derive(Resource, Default)]
    struct Resets(Vec<i32>);

    fn note_resets(reset: On<LocalTimelineShift>, mut resets: ResMut<Resets>) {
        resets.0.push(reset.delta);
    }

    /// Prints how far a guest has its tank from where its host gets to have it once lightyear
    /// itself has set its clock anew, after each of five hitches of half a second, or of as
    /// many milliseconds as `TANKU_HITCH` says:
    /// `cargo test -p tanku_client after_a_hitch -- --ignored --nocapture`.
    ///
    /// While the guest stands still, its clock falls some fifteen ticks behind. What its
    /// player asks then reaches the host about ten ticks late, and lightyear sets the clock
    /// ten ticks ahead a quarter of a second later. The player drives back and forth all the
    /// while. What was asked moves with the clock, to the ticks at which the host took it: in
    /// most hitches the tank is where the host has it from the first tick on, and in the
    /// others the host took an order a tick off, which puts the tank 0.2 m off at most. With
    /// nothing moved it is 0.3 to 0.6 m off for some five ticks.
    ///
    /// Nothing is checked here, because none of it is steady: a hitch of 450 ms is followed
    /// by no such jump, and after one of 600 ms the orders still come late once the clock is
    /// set, so that the tank is up to 0.2 m off for a quarter of a second either way.
    #[test]
    #[ignore = "prints a list"]
    fn print_where_a_guest_has_its_tank_after_a_hitch() {
        let hitch_for = std::env::var("TANKU_HITCH").map_or(500, |text| text.parse().unwrap());
        let drive = |north: f32| TankCommand {
            drive: Vec2::Y * north,
            ..default()
        };
        for hitch in 1..=5 {
            let (mut host, mut guest) = pair(60, 0, 0.0);
            guest.init_resource::<Resets>().add_observer(note_resets);
            let since = tick(&host);
            run_until("the drive", &mut [&mut host, &mut guest], |games| {
                order(games[1], drive(1.0));
                past(games[0], since) > 40
            });
            let stood_still = std::time::Instant::now();
            while stood_still.elapsed().as_millis() < hitch_for {
                host.update();
                std::thread::sleep(std::time::Duration::from_millis(4));
            }
            // The guest's tick once its clock was set, and how long after the hitch.
            let (back_at, set) = (tick(&host), RefCell::new(None));
            run_until("the clock", &mut [&mut host, &mut guest], |games| {
                let ticks = past(games[0], back_at);
                let north = if (ticks / 10) % 2 == 0 { -1.0 } else { 1.0 };
                order(games[1], drive(north));
                let reset = !games[1].world().resource::<Resets>().0.is_empty();
                if reset && set.borrow().is_none() {
                    set.replace(Some((tick(games[1]), ticks)));
                }
                ticks > 90
            });
            let Some((at, after)) = set.into_inner() else {
                println!("hitch {hitch}: the clock was not set anew");
                continue;
            };
            let by = &guest.world().resource::<Resets>().0;
            let apart = apart(&host, &guest, 1, at + 1..at + 21);
            let worst = apart.iter().copied().fold(0.0, f32::max);
            let off = apart.iter().filter(|apart| **apart > 1e-3).count();
            println!(
                "hitch {hitch}: the clock was set by {by:?} ticks, {after} ticks after the \
                 hitch; the tank was up to {worst:.3} m off, at {off} of the {} ticks after",
                apart.len()
            );
        }
    }

    #[test]
    fn a_guest_lays_a_mine_at_once_where_the_host_will_have_it() {
        let (mut host, mut guest, aim) = aiming_east(60);
        let mine = TankCommand { mine: true, ..aim };
        let laid = |game: &App| news(game, |event| matches!(event, SimEvent::MinePlaced { .. }));
        let mines = |game: &mut App| {
            let world = game.world_mut();
            world.query::<&Mine>().iter(world).count()
        };

        order(&mut guest, mine);
        let asked_at = std::time::Instant::now();
        run_until("a mine", &mut [&mut host, &mut guest], |games| {
            mines(games[1]) > 0
        });
        assert!(
            asked_at.elapsed().as_millis() < 60,
            "{:?}",
            asked_at.elapsed()
        );
        assert_eq!(mines(&mut host), 0);
        guest.update();
        assert_eq!(laid(&guest), 1);

        let laid_at = tick(&guest);
        order(&mut guest, aim);
        run_until("the host's word", &mut [&mut host, &mut guest], |games| {
            past(games[0], laid_at) > 30
        });
        let (there, here) = (
            host.world().resource::<Log>(),
            guest.world().resource::<Log>(),
        );
        // One mine all along, from the tick at which the host laid it, and at the same place.
        let (first, _, most, least) = span(&here.mines).unwrap();
        assert_eq!((most, least), (1, 1));
        assert_eq!(first, span(&there.mines).unwrap().0);
        let apart = apart_at_each_tick(&there.mines, &here.mines);
        assert!(apart.len() > 30, "{} ticks to compare", apart.len());
        assert!(apart.iter().all(|apart| *apart < 1e-3), "{apart:?}");
        assert_eq!((laid(&host), laid(&guest)), (1, 1));
    }

    #[test]
    fn a_guest_raises_its_shield_at_once_and_for_as_long_as_the_host_will() {
        let (mut host, mut guest, aim) = aiming_east(60);
        let shield = TankCommand {
            shield: true,
            ..aim
        };
        let raised =
            |game: &App| news(game, |event| matches!(event, SimEvent::ShieldRaised { .. }));
        let dropped = |game: &App| {
            news(game, |event| {
                matches!(event, SimEvent::ShieldDropped { .. })
            })
        };
        let up = |game: &mut App, seat: u8| {
            let world = game.world_mut();
            let mut tanks = world.query::<(&Player, &Shield)>();
            let tank = tanks.iter(world).find(|(player, _)| player.seat == seat);
            tank.unwrap().1.is_active()
        };

        let since = tick(&guest);
        order(&mut guest, shield);
        let asked_at = std::time::Instant::now();
        run_until("a bubble", &mut [&mut host, &mut guest], |games| {
            up(games[1], 1)
        });
        assert!(
            asked_at.elapsed().as_millis() < 60,
            "{:?}",
            asked_at.elapsed()
        );
        assert!(!up(&mut host, 1));
        guest.update();
        assert_eq!(raised(&guest), 1);

        order(&mut guest, aim);
        run_until("the cooldown", &mut [&mut host, &mut guest], |games| {
            past(games[0], since) > 90
        });
        // The bubble was up on the guest's screen at the ticks at which the host had it up.
        let (there, here) = (
            host.world().resource::<Log>(),
            guest.world().resource::<Log>(),
        );
        let ticks = (since..since + 80).filter_map(|tick| {
            let (there, here) = (there.arms.get(&(tick, 1))?, here.arms.get(&(tick, 1))?);
            Some((tick, there.1, here.1))
        });
        let ticks: Vec<(u32, Shield, Shield)> = ticks.collect();
        let wrong: Vec<_> = ticks
            .iter()
            .filter(|(_, there, here)| there != here)
            .collect();
        assert!(ticks.len() > 70, "{} ticks to compare", ticks.len());
        assert!(wrong.is_empty(), "{wrong:?}");
        let up_for = ticks.iter().filter(|(_, there, _)| there.is_active());
        assert_eq!(up_for.count(), 30);
        // It rose and fell once, in sound as in sight.
        assert_eq!((raised(&guest), dropped(&guest)), (1, 1));
    }

    #[test]
    fn the_gun_of_a_guest_is_as_the_host_will_have_it_at_each_tick() {
        let (mut host, mut guest, aim) = aiming_east(60);
        // Three seconds of holding every button: the magazine empties and reloads, two mines
        // go down and the shield goes up and comes down.
        let all = TankCommand {
            fire: true,
            mine: true,
            shield: true,
            ..aim
        };
        let since = tick(&guest);
        run_until("the barrage", &mut [&mut host, &mut guest], |games| {
            let busy = past(games[1], since) < 180;
            order(games[1], if busy { all } else { aim });
            past(games[0], since) > 200
        });
        let (there, here) = (
            host.world().resource::<Log>(),
            guest.world().resource::<Log>(),
        );
        let ticks = (since + 2..since + 195).filter_map(|tick| {
            let (there, here) = (there.arms.get(&(tick, 1))?, here.arms.get(&(tick, 1))?);
            Some((tick, *there, *here))
        });
        let ticks: Vec<(u32, (Gun, Shield), (Gun, Shield))> = ticks.collect();
        let wrong: Vec<_> = ticks
            .iter()
            .filter(|(_, there, here)| there != here)
            .collect();
        assert!(ticks.len() > 180, "{} ticks to compare", ticks.len());
        assert!(
            wrong.is_empty(),
            "{} ticks differ: {:?}",
            wrong.len(),
            wrong.first()
        );
        // The gun did work meanwhile.
        let shots = |game: &App| news(game, |event| matches!(event, SimEvent::Shot { .. }));
        assert_eq!((shots(&host), shots(&guest)), (7, 7));
        let mines = |game: &App| news(game, |event| matches!(event, SimEvent::MinePlaced { .. }));
        assert_eq!((mines(&host), mines(&guest)), (2, 2));
        let emptied = ticks.iter().any(|(_, (gun, _), _)| gun.rounds == 0);
        assert!(emptied);
    }

    #[test]
    fn a_shot_that_the_host_does_not_fire_goes_away() {
        let (mut host, mut guest, aim) = aiming_east(60);
        // The gun of the guest's tank is empty on the host, and the guest has yet to hear of
        // it when its player fires.
        let world = host.world_mut();
        let mut guns = world.query::<(&Player, &mut Gun)>();
        let (_, mut gun) = guns
            .iter_mut(world)
            .find(|(player, _)| player.seat == 1)
            .unwrap();
        gun.rounds = 0;
        order(&mut guest, TankCommand { fire: true, ..aim });
        run_until("a shot", &mut [&mut host, &mut guest], |games| {
            bullets(games[1]) > 0
        });
        let fired_at = tick(&guest);
        order(&mut guest, aim);
        run_until("the host's word", &mut [&mut host, &mut guest], |games| {
            bullets(games[1]) == 0
        });
        // The bullet went with the host's word of that tick, not later.
        let waited = past(&guest, fired_at);
        assert!(waited < 25, "the bullet stayed for {waited} ticks");
        run_until("a while", &mut [&mut host, &mut guest], |games| {
            past(games[0], fired_at) > 20
        });
        assert!(span(&host.world().resource::<Log>().bullets).is_none());
        // And the gun shows what the host has.
        let world = guest.world_mut();
        let gun = world.query_filtered::<&Gun, With<Own>>().single(world);
        assert_eq!(gun.unwrap().rounds, 0);
    }

    #[test]
    fn a_shot_that_the_host_fires_a_tick_late_is_still_one_shot() {
        let (mut host, mut guest, aim) = aiming_east(60);
        let shots = |game: &App| news(game, |event| matches!(event, SimEvent::Shot { .. }));
        // The gun of the guest's tank is not quite ready on the host at the tick at which
        // the guest's player fires, and the guest has yet to hear of it.
        let ahead = past(&guest, tick(&host)) as f32;
        let world = host.world_mut();
        let mut guns = world.query::<(&Player, &mut Gun)>();
        let (_, mut gun) = guns
            .iter_mut(world)
            .find(|(player, _)| player.seat == 1)
            .unwrap();
        gun.cooldown = (ahead + 1.5) * TICK;
        // The order is given before either game moves on: it is the order of the guest's
        // next tick, whenever the frames of this test fall.
        let since = tick(&guest);
        order(&mut guest, TankCommand { fire: true, ..aim });
        run_until("the flight", &mut [&mut host, &mut guest], |games| {
            let held = past(games[1], since) < 6;
            order(games[1], TankCommand { fire: held, ..aim });
            past(games[0], since) > 40
        });
        let (there, here) = (
            host.world().resource::<Log>(),
            guest.world().resource::<Log>(),
        );
        // The guest fired a tick before the host did.
        let (first, _, most, least) = span(&here.bullets).unwrap();
        let (fired, ..) = span(&there.bullets).unwrap();
        assert_eq!((first + 1, most, least), (fired, 1, 1));
        // Its bullet was a tick ahead until the host's took its place, and the shot sounded
        // once.
        let apart = apart_at_each_tick(&there.bullets, &here.bullets);
        let ahead = apart.iter().filter(|apart| **apart > 1e-3);
        let worst = ahead.clone().copied().fold(0.0, f32::max);
        assert!(apart.len() > 30, "{} ticks to compare", apart.len());
        assert!(
            (1..20).contains(&ahead.count()) && worst < 0.16,
            "{apart:?}"
        );
        assert_eq!((shots(&host), shots(&guest)), (1, 1));
    }

    #[test]
    fn a_guest_fires_as_often_as_its_host_on_a_line_that_loses_packets() {
        // What the host tells comes unevenly, and some of it is lost, each way.
        let (mut host, mut guest) = pair(30, 12, 0.03);
        let aim = TankCommand {
            aim: place(&mut guest, 1) + Vec2::new(30.0, 5.0),
            ..default()
        };
        let since = tick(&guest);
        run_until("the barrage", &mut [&mut host, &mut guest], |games| {
            let ticks = past(games[1], since);
            let busy = (30..270).contains(&ticks);
            let all = TankCommand {
                fire: true,
                mine: true,
                shield: true,
                ..aim
            };
            order(games[1], if busy { all } else { aim });
            past(games[0], since) > 330
        });
        // Every shot, mine and bubble showed and sounded once on the guest's screen.
        for (what, of) in [
            (
                "shots",
                SimEvent::Shot {
                    at: Vec2::ZERO,
                    team: tanku_core::sim::Team::Players,
                },
            ),
            ("mines", SimEvent::MinePlaced { at: Vec2::ZERO }),
            ("bubbles", SimEvent::ShieldRaised { at: Vec2::ZERO }),
        ] {
            let count = |game: &App| news(game, |event| discriminant(event) == discriminant(&of));
            assert_eq!(count(&guest), count(&host), "{what}");
            assert!(count(&host) > 0, "{what}");
        }
        // Nothing of the guest's own is left on its screen, and its gun is the host's.
        let world = guest.world_mut();
        assert_eq!(world.query::<&Foreseen>().iter(world).count(), 0);
        let (there, here) = (
            host.world().resource::<Log>(),
            guest.world().resource::<Log>(),
        );
        let last = since + 325;
        assert_eq!(here.arms.get(&(last, 1)), there.arms.get(&(last, 1)));
        assert!(here.arms.contains_key(&(last, 1)));
    }

    /// A host whose player has fired a shot at the tank of the guest, which stands well to
    /// the north of its own, and a guest whose player has raised the shield as the bullet
    /// came close on their screen.
    fn a_shot_at_a_bubble() -> (App, App) {
        let (mut host, mut guest) = pair(60, 0, 0.0);
        // The guest's tank drives north, away from the host's.
        let since = tick(&guest);
        run_until("the drive", &mut [&mut host, &mut guest], |games| {
            let north = TankCommand {
                drive: Vec2::Y,
                ..default()
            };
            let driving = past(games[1], since) < 80;
            order(games[1], if driving { north } else { default() });
            past(games[0], since) > 100
        });
        // The host's player fires at it.
        let target = place(&mut host, 1);
        assert!(target.distance(place(&mut host, 0)) > 5.0);
        let aim = |fire: bool| TankCommand {
            aim: target,
            fire,
            ..default()
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
        // The guest's player raises the shield when the bullet is close on their screen.
        run_until("the bubble", &mut [&mut host, &mut guest], |games| {
            let world = games[1].world_mut();
            let mut shown = world.query_filtered::<&Position, With<Bullet>>();
            let close = shown
                .iter(world)
                .any(|bullet| bullet.0.distance(target) < 3.5);
            let asked = TankCommand {
                shield: close,
                ..default()
            };
            order(games[1], asked);
            close
        });
        (host, guest)
    }

    #[test]
    fn a_bullet_ends_on_the_bubble_of_a_guest_as_soon_as_it_gets_there() {
        let (mut host, mut guest) = a_shot_at_a_bubble();
        run_until("the end", &mut [&mut host, &mut guest], |games| {
            bullets(games[0]) == 0 && bullets(games[1]) == 0
        });
        let ended_at = tick(&host);
        run_until("the host's word", &mut [&mut host, &mut guest], |games| {
            past(games[0], ended_at) > 30
        });

        // The tank is whole, and the bubble took the bullet.
        let world = host.world_mut();
        assert_eq!(world.query::<&Wreck>().iter(world).count(), 0);
        let blocked = |game: &App| {
            news(game, |event| {
                matches!(event, SimEvent::ShieldBlocked { .. })
            })
        };
        assert_eq!((blocked(&host), blocked(&guest)), (1, 1));
        // The guest saw the bullet end at the tick at which the host ended it, without
        // waiting for the host to say so: by then the bullet would have flown on for as long
        // as the line takes there and back.
        let (there, here) = (
            host.world().resource::<Log>(),
            guest.world().resource::<Log>(),
        );
        let (_, ended, ..) = span(&there.bullets).unwrap();
        let (_, last, most, _) = span(&here.bullets).unwrap();
        assert_eq!((last, most), (ended, 1));
        let burst = |log: &Log| {
            let ended = |event: &SimEvent| matches!(event, SimEvent::BulletDestroyed { .. });
            let mut ticks = log.news.iter().filter(|(_, event)| ended(event));
            (ticks.clone().count(), ticks.next().map(|(tick, _)| *tick))
        };
        let ((there_count, there_tick), (here_count, here_tick)) = (burst(there), burst(here));
        assert_eq!((there_count, here_count), (1, 1));
        let late = here_tick.unwrap() as i64 - there_tick.unwrap() as i64;
        assert!(late.abs() <= 1, "the bullet burst {late} ticks late");
    }

    #[test]
    fn a_bullet_that_ended_on_a_bubble_stays_ended_though_the_clock_of_the_guest_is_set_back() {
        let (mut host, mut guest) = a_shot_at_a_bubble();
        // The bullet ends on the bubble on the guest's screen, and the host still has it.
        run_until(
            "the end on the guest's screen",
            &mut [&mut host, &mut guest],
            |games| bullets(games[1]) == 0,
        );
        assert_eq!(bullets(&mut host), 1);
        // The clock of the guest is set back by hand, as lightyear sets it when the host
        // stood still for a moment: to before the tick at which the bullet came upon the
        // bubble.
        let since = tick(&host);
        guest.world_mut().trigger(LocalTimelineShift { delta: -6 });
        let back = RefCell::new(false);
        run_until("the host's word", &mut [&mut host, &mut guest], |games| {
            *back.borrow_mut() |= bullets(games[1]) > 0;
            past(games[0], since) > 45
        });

        // The bullet didn't show again, and it burst on the bubble once, in sound as in
        // sight: the host ended it at the tick at which the guest had seen it end.
        assert!(!back.into_inner(), "the bullet showed again");
        let count =
            |game: &App, of: &SimEvent| news(game, |event| discriminant(event) == discriminant(of));
        let (blocked, burst) = (
            SimEvent::ShieldBlocked { at: Vec2::ZERO },
            SimEvent::BulletDestroyed {
                at: Vec2::ZERO,
                team: tanku_core::sim::Team::Players,
            },
        );
        assert_eq!((count(&host, &blocked), count(&guest, &blocked)), (1, 1));
        assert_eq!((count(&host, &burst), count(&guest, &burst)), (1, 1));
        let world = host.world_mut();
        assert_eq!(world.query::<&Wreck>().iter(world).count(), 0);
    }

    /// Destroys the tank of the host's own player, and waits for the guest to have the wreck.
    fn wreck_the_host(host: &mut App, guest: &mut App) {
        let world = host.world_mut();
        let mut tanks = world.query::<(Entity, &Player)>();
        let (wrecked, _) = tanks
            .iter(world)
            .find(|(_, player)| player.seat == 0)
            .unwrap();
        let mut casualties = world.resource_mut::<tanku_server::Casualties>();
        casualties.tanks.push(wrecked);
        run_until("the wreck", &mut [host, guest], |games| {
            let world = games[1].world_mut();
            world.query::<&Wreck>().iter(world).count() == 1
        });
    }

    #[test]
    fn a_guest_drives_its_own_tank_over_a_wreck_as_the_host_has_it() {
        let (mut host, mut guest) = pair(60, 0, 0.0);
        wreck_the_host(&mut host, &mut guest);
        // The wreck lies next to the guest's tank, which drives straight at it and on.
        let (wreck, start) = (place(&mut guest, 0), place(&mut guest, 1));
        assert!(wreck.distance(start) < 4.0);
        let across = TankCommand {
            drive: (wreck - start).normalize(),
            ..default()
        };
        let since = tick(&guest);
        run_until("the other side", &mut [&mut host, &mut guest], |games| {
            order(games[1], across);
            past(games[0], since) > 75
        });
        // It went over the middle of the wreck, on its own screen and on the host's.
        let log = guest.world().resource::<Log>();
        let way = (since..since + 70).filter_map(|tick| log.tanks.get(&(tick, 1)));
        let nearest = way.map(|at| at.distance(wreck)).fold(f32::MAX, f32::min);
        assert!(nearest < 0.2, "the tank came no nearer than {nearest} m");
        assert!(place(&mut host, 1).distance(start) > wreck.distance(start) + 1.0);
        let apart = apart(&host, &guest, 1, since + 2..since + 70);
        let worst = apart.iter().copied().fold(0.0, f32::max);
        assert!(apart.len() > 60, "{} ticks to compare", apart.len());
        assert!(worst < 1e-3, "the tank was up to {worst} m from its place");
    }

    #[test]
    fn a_guest_that_repairs_keeps_its_hands_free() {
        let (mut host, mut guest, aim) = aiming_east(60);
        // The tank of the host's player is destroyed, next to the guest's.
        wreck_the_host(&mut host, &mut guest);

        // While its player holds the repair key, the guest's tank fires, lays a mine and
        // shields as it always does: on its own screen at once, and on the host's.
        let busy = TankCommand {
            fire: true,
            mine: true,
            shield: true,
            repair: true,
            ..aim
        };
        let deeds = |game: &App| {
            [
                news(game, |event| matches!(event, SimEvent::Shot { .. })),
                news(game, |event| matches!(event, SimEvent::MinePlaced { .. })),
                news(game, |event| matches!(event, SimEvent::ShieldRaised { .. })),
            ]
        };
        let before = (deeds(&host), deeds(&guest));
        run_until("the deeds", &mut [&mut host, &mut guest], |games| {
            order(games[1], busy);
            let now = deeds(games[1]);
            (0..3).all(|kind| now[kind] > before.1[kind])
        });
        assert_eq!(deeds(&host), before.0);
        let since = tick(&guest);
        run_until("the repair", &mut [&mut host, &mut guest], |games| {
            order(games[1], busy);
            past(games[0], since) > 60
        });
        // The repair went on all the while.
        let world = host.world_mut();
        let repair = world.query::<&Wreck>().single(world).unwrap().repair;
        assert!(repair > 0.5, "{repair} s of repair");
        // Once the host has heard everything, it has the same shots, the same mines and the
        // same bubble as the guest showed.
        let since = tick(&guest);
        run_until("the host's turn", &mut [&mut host, &mut guest], |games| {
            order(games[1], aim);
            past(games[0], since) > 30
        });
        let (there, here) = (deeds(&host), deeds(&guest));
        assert!(there[0] > before.0[0] + 1, "{there:?}");
        assert_eq!(there, here);
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

    #[test]
    fn what_the_guest_has_shown_answers_for_one_telling_of_the_host() {
        let shot = |x: f32| SimEvent::Shot {
            at: Vec2::new(x, 0.0),
            team: tanku_core::sim::Team::Players,
        };
        let mut echoes = Echoes::default();
        // Nothing was shown: what the host tells shows.
        assert!(!echoes.claims(100, &shot(0.0)));
        echoes.expect(100, shot(0.0));
        // Another kind of thing, or the same kind elsewhere, is not what was shown.
        assert!(!echoes.claims(107, &SimEvent::Ricochet { at: Vec2::ZERO }));
        assert!(!echoes.claims(107, &shot(5.0)));
        // The host's word of it comes a little off, and only once.
        assert!(echoes.claims(107, &shot(0.4)));
        assert!(!echoes.claims(108, &shot(0.4)));
        // What the host never told of is forgotten: a later shot from there is another one.
        echoes.expect(200, shot(0.0));
        assert!(!echoes.claims(200 + AWAITED + 1, &shot(0.0)));
        // What happens nowhere in particular is never the guest's doing.
        echoes.expect(400, SimEvent::RunOver);
        assert!(!echoes.claims(401, &SimEvent::RunOver));
    }

    #[test]
    fn a_deed_is_the_same_a_tick_or_two_later() {
        let at = Vec2::ZERO;
        let shown = Shown {
            tick: 50,
            deed: Deed::Mine { at },
            thing: None,
        };
        assert!(shown.is(50, &Deed::Mine { at: Vec2::X }));
        assert!(shown.is(52, &Deed::Mine { at }));
        assert!(!shown.is(53, &Deed::Mine { at }));
        assert!(!shown.is(50, &Deed::Raised { at }));
        assert!(after(5, 4) && !after(4, 4) && !after(3, 4));
        // Ticks wrap around.
        assert!(after(2, u32::MAX));
    }

    #[test]
    fn a_bullet_comes_upon_a_circle_on_its_way() {
        let center = Vec2::new(5.0, 0.0);
        let reach = 1.0 + BULLET_RADIUS;
        // Straight at it: it stops where it touches.
        let at = met(&[Vec2::ZERO, Vec2::new(10.0, 0.0)], center, 1.0).unwrap();
        assert!((at.x - (5.0 - reach)).abs() < 1e-4, "{at}");
        // Past it, or short of it, or on a way that bends away before it: not at all.
        assert!(met(&[Vec2::new(0.0, 2.0), Vec2::new(10.0, 2.0)], center, 1.0).is_none());
        assert!(met(&[Vec2::ZERO, Vec2::new(3.0, 0.0)], center, 1.0).is_none());
        let bent = [Vec2::ZERO, Vec2::new(3.0, 0.0), Vec2::new(3.0, 9.0)];
        assert!(met(&bent, center, 1.0).is_none());
        // After a bend toward it.
        let bent = [
            Vec2::new(0.0, 4.0),
            Vec2::new(5.0, 4.0),
            Vec2::new(5.0, 0.0),
        ];
        let at = met(&bent, center, 1.0).unwrap();
        assert!((at.y - reach).abs() < 1e-4, "{at}");
        // One that starts inside and flies out is not stopped: a tank's own shot leaves.
        assert!(
            met(
                &[center + Vec2::X * 0.5, center + Vec2::X * 3.0],
                center,
                1.0
            )
            .is_none()
        );
        // A bullet that didn't move went no way.
        assert!(met(&[Vec2::ZERO], Vec2::ZERO, 1.0).is_none());
    }
}
