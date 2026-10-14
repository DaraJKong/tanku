//! The waves of an arena.
//!
//! An arena is too small for its whole group at once: every tank would have the players in
//! range. So an arena is empty when the players come in, and its enemies appear in waves: the
//! first a moment after the players, each of the others when the one before it is destroyed.
//! The place of each tank is marked for a moment before the tank is there.
//!
//! A group of arena mode is held back the same way, in the same room as the one before it.
//! It doesn't come in waves: half of it comes at first, or the whole of a group of two, and
//! then a tank for each one that is destroyed, so that the room stays as full until nothing
//! is held back.

use bevy::prelude::*;
use tanku_core::{
    rules::{APPEAR_APART, APPEAR_ROOM, APPEAR_TIME, TICK},
    sim::{Enemy, Player, Position, Run, RunPhase, SimEvent, Tank, Wreck},
};

use crate::field::{Field, Fuse, Order};

/// Brings the tanks that the arena that the players are in holds back.
pub fn waves(
    mut commands: Commands,
    mut field: ResMut<Field>,
    run: Res<Run>,
    enemies: Query<(&Tank, &Position), With<Enemy>>,
    players: Query<&Position, (With<Player>, Without<Wreck>)>,
    mut events: MessageWriter<SimEvent>,
) {
    let front = run.sector;
    let Some(mut arena) = field.arenas.remove(&front) else {
        return;
    };
    let lit = arena.fuse != Fuse::Out;
    arena.fuse = match arena.fuse {
        Fuse::Burning(left) if left > TICK => Fuse::Burning(left - TICK),
        Fuse::Burning(_) => Fuse::Out,
        fuse => fuse,
    };
    if arena.fuse == Fuse::Out {
        let there: Vec<Vec2> = enemies
            .iter()
            .filter(|(tank, _)| tank.sector == front)
            .map(|(_, position)| position.0)
            .collect();
        match arena.order {
            // The next wave starts when nothing is left of the one before it.
            Order::Waves(waves) => {
                if there.is_empty()
                    && arena.incoming.is_empty()
                    && let Some(wave) = arena.waiting.pop_front()
                {
                    let wave_number = waves - arena.waiting.len() as u32;
                    events.write(SimEvent::WaveIncoming {
                        wave: wave_number,
                        of: waves,
                    });
                    arena.incoming = wave.into_iter().map(|recruit| (recruit, None)).collect();
                }
            }
            // A group of arena mode is announced once, when it comes. From then on, the next
            // tank is on its way as soon as one is missing. Nothing comes for a single player
            // who fell: what is there goes back into reserve when they come back.
            Order::Refill(most) => {
                if lit {
                    events.write(SimEvent::WaveIncoming { wave: 1, of: 1 });
                }
                while run.phase == RunPhase::Playing
                    && there.len() + arena.incoming.len() < most as usize
                    && let Some(recruit) = arena.draw()
                {
                    arena.incoming.push((recruit, None));
                }
            }
        }
        let toward = field.way_in(front);
        // In arena mode, the tanks were shown in reserve, and a mark tells which one comes.
        let told = field.room.is_some();
        // In arena mode, a group has places to spare.
        let mut spare = match field.room.as_mut() {
            Some(state) => std::mem::take(&mut state.spare),
            None => Vec::new(),
        };
        arena.incoming.retain_mut(|(recruit, left)| {
            match *left {
                // The place of a tank is marked once the players leave it room, and the tanks
                // of its group that are there. In the room of arena mode, where nobody knows
                // that a tank waits for them to move, the tank takes a place that has room
                // instead.
                None => {
                    let has_room = |at: Vec2| {
                        let apart = |player: &Position| player.0.distance(at) > APPEAR_ROOM;
                        let clear = |tank: &Vec2| tank.distance(at) >= APPEAR_APART;
                        players.iter().all(apart) && there.iter().all(clear)
                    };
                    if !has_room(recruit.at)
                        && let Some(other) = spare.iter().position(|place| has_room(*place))
                    {
                        std::mem::swap(&mut recruit.at, &mut spare[other]);
                    }
                    if has_room(recruit.at) {
                        *left = Some(APPEAR_TIME);
                        events.write(SimEvent::TankIncoming {
                            at: recruit.at,
                            tank: told.then_some(recruit.genome),
                        });
                    }
                    true
                }
                Some(time) if time > TICK => {
                    *left = Some(time - TICK);
                    true
                }
                Some(_) => {
                    field.enlist(&mut commands, (front, toward), *recruit, true);
                    events.write(SimEvent::TankAppeared {
                        at: recruit.at,
                        tank: told.then_some(recruit.genome),
                    });
                    false
                }
            }
        });
        if let Some(state) = field.room.as_mut() {
            state.spare = spare;
        }
    }
    field.arenas.insert(front, arena);
}
