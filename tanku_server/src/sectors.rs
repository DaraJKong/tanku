//! The life of a sector: dormant, fought, cleared, left behind.

use bevy::prelude::*;
use tanku_core::{
    geom, progression,
    rules::{BARRIER_GATHER_DISTANCE, TANK_RADIUS},
    sim::{Barrier, Enemy, Player, Position, Run, SimEvent, Tank},
};

use crate::field::Field;

pub fn advance(
    mut commands: Commands,
    mut field: ResMut<Field>,
    mut run: ResMut<Run>,
    mut player: Query<(&mut Tank, &Position), With<Player>>,
    mut enemies: Query<(&Tank, &mut Enemy), Without<Player>>,
    mut barriers: Query<&mut Barrier>,
    mut events: MessageWriter<SimEvent>,
) {
    let sector = run.sector;
    let fought = enemies.iter().any(|(tank, _)| tank.sector == sector);
    if !fought && field.cleared.insert(sector) {
        run.cleared = run.cleared.max(sector);
        if progression::is_checkpoint(sector) {
            run.checkpoint = sector;
        }
        events.write(SimEvent::SectorCleared { sector });
    }

    let Ok((mut tank, position)) = player.single_mut() else {
        return;
    };
    let Some(mut barrier) = barriers.iter_mut().find(|barrier| barrier.sector == sector) else {
        return;
    };
    if !barrier.unlocked {
        let distance = geom::distance_to_segment(position.0, barrier.from, barrier.to);
        if field.cleared.contains(&sector) && distance <= BARRIER_GATHER_DISTANCE {
            barrier.unlocked = true;
            field.stale = true;
            events.write(SimEvent::BarrierUnlocked { sector });
        }
        return;
    }

    // The player is through once the whole tank is past the barrier.
    if (position.0 - barrier.from).dot(barrier.outward) > TANK_RADIUS {
        let entered = sector + 1;
        tank.sector = entered;
        run.sector = entered;
        for (enemy_tank, mut enemy) in &mut enemies {
            if enemy_tank.sector == entered {
                enemy.awake = true;
            }
        }
        // The sector just left stays in view, and the next one waits behind its barrier.
        field.load(&mut commands, entered + 1);
        field.loaded.retain(|&loaded| loaded + 1 >= entered);
        events.write(SimEvent::SectorEntered { sector: entered });
    }
}
