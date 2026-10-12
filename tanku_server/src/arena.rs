//! The waves of an arena.
//!
//! An arena is too small for its whole group at once: every tank would have the players in
//! range. So an arena is empty when the players come in, and its enemies appear in waves: the
//! first a moment after the players, each of the others when the one before it is destroyed.
//! The place of each tank is marked for a moment before the tank is there.
//!
//! The groups of arena mode come the same way, one after another in the same room.

use bevy::prelude::*;
use tanku_core::{
    rules::{APPEAR_ROOM, APPEAR_TIME, TICK},
    sim::{Enemy, Player, Position, Run, SimEvent, Tank, Wreck},
};

use crate::field::{Field, Fuse};

/// Runs the waves of the arena that the players are in.
pub fn waves(
    mut commands: Commands,
    mut field: ResMut<Field>,
    run: Res<Run>,
    enemies: Query<&Tank, With<Enemy>>,
    players: Query<&Position, (With<Player>, Without<Wreck>)>,
    mut events: MessageWriter<SimEvent>,
) {
    let front = run.sector;
    let Some(mut arena) = field.arenas.remove(&front) else {
        return;
    };
    arena.fuse = match arena.fuse {
        Fuse::Burning(left) if left > TICK => Fuse::Burning(left - TICK),
        Fuse::Burning(_) => Fuse::Out,
        fuse => fuse,
    };
    if arena.fuse == Fuse::Out {
        // The next wave starts when nothing is left of the one before it.
        let fought = enemies.iter().any(|tank| tank.sector == front);
        if !fought
            && arena.incoming.is_empty()
            && let Some(wave) = arena.waiting.pop_front()
        {
            let wave_number = arena.waves - arena.waiting.len() as u32;
            events.write(SimEvent::WaveIncoming {
                wave: wave_number,
                of: arena.waves,
            });
            arena.incoming = wave.into_iter().map(|recruit| (recruit, None)).collect();
        }
        let toward = field.way_in(front);
        // In arena mode, a group has places to spare.
        let mut spare = match field.room.as_mut() {
            Some(state) => std::mem::take(&mut state.spare),
            None => Vec::new(),
        };
        arena.incoming.retain_mut(|(recruit, left)| {
            match *left {
                // The place of a tank is marked once the players leave it room. In the room
                // of arena mode, where nobody knows that a tank waits for them to move, the
                // tank takes a place that has room instead.
                None => {
                    let has_room = |at: Vec2| {
                        let apart = |player: &Position| player.0.distance(at) > APPEAR_ROOM;
                        players.iter().all(apart)
                    };
                    if !has_room(recruit.at)
                        && let Some(other) = spare.iter().position(|place| has_room(*place))
                    {
                        std::mem::swap(&mut recruit.at, &mut spare[other]);
                    }
                    if has_room(recruit.at) {
                        *left = Some(APPEAR_TIME);
                        events.write(SimEvent::TankIncoming { at: recruit.at });
                    }
                    true
                }
                Some(time) if time > TICK => {
                    *left = Some(time - TICK);
                    true
                }
                Some(_) => {
                    field.enlist(&mut commands, (front, toward), *recruit, true);
                    events.write(SimEvent::TankAppeared { at: recruit.at });
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
