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
        arena.incoming.retain_mut(|(recruit, left)| {
            match *left {
                // The place of a tank is marked once the players leave it room.
                None => {
                    let room = |player: &Position| player.0.distance(recruit.at) > APPEAR_ROOM;
                    if players.iter().all(room) {
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
    }
    field.arenas.insert(front, arena);
}
