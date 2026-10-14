//! A run in arena mode: one room, and one group after another without end.
//!
//! The run never leaves the room, so nothing unlocks and nothing is crossed. A group is the
//! group of the sector of its number. Half of it comes when the group does, or both tanks of
//! a group of two, and then a tank for each one that is destroyed: the room stays as full
//! until the group has no more to send. Between two groups there is a short lull: the room is laid out anew, and the next
//! group stands in reserve outside the south wall, where the players read it as they read a
//! group through a barrier.

use std::collections::VecDeque;

use bevy::prelude::*;
use tanku_core::{
    enemy,
    map::Room,
    progression,
    rules::{
        ARENA_DELAY, BLOCK_SIZE, BULLET_RADIUS, LIFE_GROUPS, LULL, MINE_RADIUS, RISE_TIME,
        SOLO_LIVES, TANK_RADIUS, TICK,
    },
    sim::{
        Block, Bullet, Enemy, ExplosiveCrate, Mine, Planned, Player, Position, Reserve, Run,
        RunPhase, Sector, SectorIndex, SimEvent, Tank,
    },
};

use crate::field::{Arena, Field, Fuse, InSector, Order, Recruit};

/// The room of a run in arena mode, and how the run stands in it.
#[derive(Clone, Debug)]
pub struct RoomState {
    pub room: Room,
    /// The seconds left before the next group is announced, between two groups.
    pub lull: Option<f32>,
    /// The group that comes when the lull is over, in its two parts: the tanks that come
    /// with it, and those that each wait for one of the group to be destroyed.
    pub next: VecDeque<Vec<Recruit>>,
    /// How many tanks each of these parts has, for the group that is on, or for the next one
    /// during a lull. A tank keeps its place in the reserve by it.
    pub split: Vec<u32>,
    /// The places of that group that none of its tanks has: a tank whose own place has a
    /// player too close, or a tank of its group on it, takes one of them, so that a tank
    /// never waits for a player to move.
    pub spare: Vec<Vec2>,
    /// What stands in the room as it was last laid out: the lowest corners of its blocks, and
    /// those of its crates.
    pub blocks: Vec<IVec2>,
    pub crates: Vec<IVec2>,
}

/// The seconds until a planned block or crate stands, if it has room then.
#[derive(Component, Clone, Copy, Debug)]
pub struct Due(pub f32);

impl RoomState {
    pub fn new(seed: u64) -> Self {
        Self {
            room: Room::new(seed),
            lull: None,
            next: VecDeque::new(),
            split: Vec::new(),
            spare: Vec::new(),
            blocks: Vec::new(),
            crates: Vec::new(),
        }
    }

    /// Where a tank of the group that is on, or of the next one during a lull, waits in
    /// reserve: with its part of the group, the tanks that come first nearest the room.
    fn reserve(&self, number: u32) -> Vec2 {
        let places = self.room.reserve(&self.split);
        let fallback = self.room.start().as_vec2();
        places.get(number as usize).copied().unwrap_or(fallback)
    }

    /// Lays the room out for a level of its terrain. What it holds is there at once when a
    /// run opens the room. Between two groups, its place is inked in first.
    fn lay(&mut self, commands: &mut Commands, level: u32, at_once: bool) {
        (self.blocks, self.crates) = self.room.obstacles(level);
        let blocks = self.blocks.iter().map(|corner| (corner, false));
        let crates = self.crates.iter().map(|corner| (corner, true));
        for (corner, explosive) in blocks.chain(crates) {
            let at = center(*corner);
            if at_once {
                stand(commands, at, explosive);
            } else {
                plan(commands, at, explosive);
            }
        }
    }
}

/// The middle of a block or of a crate, from its lowest corner.
fn center(corner: IVec2) -> Vec2 {
    corner.as_vec2() + Vec2::splat(BLOCK_SIZE / 2.0)
}

/// Puts a block or a crate in the room.
fn stand(commands: &mut Commands, at: Vec2, explosive: bool) {
    if explosive {
        commands.spawn((ExplosiveCrate { sector: 0 }, Position(at), InSector(0)));
    } else {
        commands.spawn((Block { sector: 0 }, Position(at), InSector(0)));
    }
}

/// Plans a block or a crate: its place is inked in, and it stands there after a moment.
fn plan(commands: &mut Commands, at: Vec2, explosive: bool) {
    commands.spawn((
        Planned { explosive },
        Due(RISE_TIME),
        Position(at),
        InSector(0),
    ));
}

/// Draws up a group: the tanks of the sector of its number, as many as the players call for,
/// each with the place where it appears, in the order they come: those that come with the
/// group, then those that each wait for one of them to be destroyed.
fn muster(field: &Field, state: &mut RoomState, group: u32) -> VecDeque<Vec<Recruit>> {
    let count = progression::enemy_count(group, field.players().max(1), false);
    let genomes = enemy::group(field.seed, field.difficulty, group, count);
    let places = state.room.places(group, &state.blocks, &state.crates);
    let tanks = places.iter().zip(genomes).enumerate();
    let mut tanks = tanks.map(|(number, (place, genome))| Recruit {
        number: number as u32,
        genome,
        at: place.as_vec2(),
    });
    let left_over = places.iter().skip(tanks.len());
    state.spare = left_over.map(|place| place.as_vec2()).collect();
    let (first, held) = progression::room_parts(tanks.len() as u32);
    state.split = [first, held].into_iter().filter(|&part| part > 0).collect();
    let sizes = state.split.iter();
    sizes
        .map(|&size| tanks.by_ref().take(size as usize).collect())
        .collect()
}

/// Opens the room for a run that has cleared this many groups: the room with what stands in
/// it, the next group in reserve, and a lull before it comes.
pub fn open(commands: &mut Commands, field: &mut Field, cleared: u32) {
    let Some(mut state) = field.room.take() else {
        return;
    };
    // Nothing is ever unloaded: the room is all there is.
    field.loaded.extend(0..=cleared);
    field.stale = true;
    let layout = state.room.shell();
    commands.spawn((Sector { layout }, SectorIndex(0), InSector(0)));
    state.lay(commands, cleared + 1, true);
    state.next = muster(field, &mut state, cleared + 1);
    state.lull = Some(LULL);
    field.room = Some(state);
}

/// Runs the groups of a room: the lull before each one, its start, and its end.
pub fn advance(
    mut commands: Commands,
    mut field: ResMut<Field>,
    mut run: ResMut<Run>,
    mut players: Query<(&mut Tank, &Position), With<Player>>,
    enemies: Query<&Tank, (With<Enemy>, Without<Player>)>,
    standing: Query<Entity, Or<(With<Block>, With<ExplosiveCrate>, With<Planned>)>>,
    mut events: MessageWriter<SimEvent>,
) {
    let Some(mut state) = field.room.take() else {
        return;
    };
    let front = run.sector;
    match state.lull {
        // The lull waits for a single player who fell.
        Some(_) if run.phase != RunPhase::Playing => {}
        Some(left) if left > TICK => state.lull = Some(left - TICK),
        // The next group is on. It is announced at once, with the tanks that come first.
        Some(_) => {
            state.lull = None;
            let group = front + 1;
            run.sector = group;
            field.loaded.insert(group);
            for (mut tank, _) in &mut players {
                tank.sector = group;
            }
            let waiting = std::mem::take(&mut state.next);
            // As many tanks stay in the room as come first, for as long as any waits.
            let most = state.split.first().copied().unwrap_or(0);
            field.arenas.clear();
            field.arenas.insert(
                group,
                Arena {
                    waiting,
                    incoming: Vec::new(),
                    fuse: Fuse::Burning(0.0),
                    order: Order::Refill(most),
                },
            );
            events.write(SimEvent::SectorEntered { sector: group });
        }
        None => {
            let fought = enemies.iter().any(|tank| tank.sector == front)
                || field.arenas.get(&front).is_some_and(Arena::holds_more);
            if !fought && field.cleared.insert(front) {
                run.cleared = run.cleared.max(front);
                // A single player whose tank is destroyed finds the run where it is.
                run.checkpoint = front;
                events.write(SimEvent::SectorCleared { sector: front });
                // A single player who is short of a life gets one back every few groups.
                if run.solo && front.is_multiple_of(LIFE_GROUPS) && run.lives < SOLO_LIVES {
                    run.lives += 1;
                    let own = players.iter().next().map(|(_, position)| position.0);
                    let at = own.unwrap_or(state.room.start().as_vec2());
                    events.write(SimEvent::LifeTaken { at });
                }
                // The room is laid out anew for the next group, which goes into reserve.
                for entity in &standing {
                    commands.entity(entity).try_despawn();
                }
                field.stale = true;
                state.lay(&mut commands, front + 1, false);
                state.next = muster(&field, &mut state, front + 1);
                state.lull = Some(LULL);
            }
            // A single player who comes back finds the tanks that were left in reserve. They
            // come again after a moment.
            if let Some(arena) = field.arenas.get_mut(&front)
                && arena.fuse == Fuse::Unlit
                && run.phase == RunPhase::Playing
            {
                arena.fuse = Fuse::Burning(ARENA_DELAY);
            }
        }
    }
    let told = state.lull.map(|left| left.ceil().max(1.0) as u8);
    if run.lull != told {
        run.lull = told;
    }
    field.room = Some(state);
}

/// Keeps in reserve the tanks that have not come yet: the group that a lull announces, or
/// what the group that is on still holds back, each for a tank to be destroyed. A tank leaves
/// the reserve when its place in the room is marked.
pub fn reserve(
    mut commands: Commands,
    field: Res<Field>,
    run: Res<Run>,
    reserves: Query<(Entity, &Reserve)>,
) {
    let Some(state) = &field.room else {
        return;
    };
    let (group, waiting): (u32, Vec<&Recruit>) = match (state.lull, field.arenas.get(&run.sector)) {
        (Some(_), _) => (run.sector + 1, state.next.iter().flatten().collect()),
        (None, Some(arena)) => {
            let unmarked = arena.incoming.iter().filter(|(_, left)| left.is_none());
            let held = arena.waiting.iter().flatten();
            let waiting = held.chain(unmarked.map(|(recruit, _)| recruit));
            (run.sector, waiting.collect())
        }
        (None, None) => (run.sector, Vec::new()),
    };
    let stays = |reserve: &Reserve| {
        reserve.group == group
            && waiting
                .iter()
                .any(|recruit| recruit.number == reserve.number)
    };
    for (entity, reserve) in &reserves {
        if !stays(reserve) {
            commands.entity(entity).try_despawn();
        }
    }
    for recruit in waiting {
        let there = |(_, reserve): (Entity, &Reserve)| {
            reserve.group == group && reserve.number == recruit.number
        };
        if reserves.iter().any(there) {
            continue;
        }
        commands.spawn((
            Reserve {
                genome: recruit.genome,
                group,
                number: recruit.number,
            },
            Position(state.reserve(recruit.number)),
            InSector(0),
        ));
    }
}

/// How far a point is from a block or a crate that stands around this center.
fn gap(point: Vec2, center: Vec2) -> f32 {
    let outside = (point - center).abs() - Vec2::splat(BLOCK_SIZE / 2.0);
    outside.max(Vec2::ZERO).length()
}

/// Raises the blocks and the crates that are planned, each once its time has come and
/// nothing is in its place: no tank, no wreck and no mine. A bullet that flies there ends.
pub fn raise(
    mut commands: Commands,
    mut field: ResMut<Field>,
    mut planned: Query<(Entity, &Planned, &mut Due, &Position)>,
    tanks: Query<&Position, With<Tank>>,
    mines: Query<&Position, With<Mine>>,
    bullets: Query<(Entity, &Position, &Bullet)>,
    mut events: MessageWriter<SimEvent>,
) {
    if field.room.is_none() {
        return;
    }
    let mut raised = false;
    for (entity, marked, mut due, position) in &mut planned {
        due.0 = (due.0 - TICK).max(0.0);
        if due.0 > 0.0 {
            continue;
        }
        let at = position.0;
        let in_the_way = tanks.iter().any(|tank| gap(tank.0, at) < TANK_RADIUS)
            || mines.iter().any(|mine| gap(mine.0, at) < MINE_RADIUS);
        if in_the_way {
            continue;
        }
        for (bullet, flying, shot) in &bullets {
            if gap(flying.0, at) < BULLET_RADIUS {
                commands.entity(bullet).try_despawn();
                let (at, team) = (flying.0, shot.team);
                events.write(SimEvent::BulletDestroyed { at, team });
            }
        }
        commands.entity(entity).try_despawn();
        stand(&mut commands, at, marked.explosive);
        raised = true;
    }
    field.stale |= raised;
}
