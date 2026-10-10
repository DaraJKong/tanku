//! The players of a run, as this client sees them: which tank is the one of the player at this
//! screen.

use bevy::prelude::*;
use tanku_core::sim::Player;

/// The seat of the player at this screen.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalSeat(pub u8);

/// The tank of the player at this screen.
#[derive(Component)]
pub struct Own;

pub fn plugin(app: &mut App) {
    app.init_resource::<LocalSeat>()
        .add_systems(PreUpdate, claim);
}

/// Marks the tank of the player at this screen, whenever tanks appear or the seat changes.
pub fn claim(
    mut commands: Commands,
    seat: Res<LocalSeat>,
    tanks: Query<(Entity, &Player, Has<Own>)>,
) {
    for (entity, player, own) in &tanks {
        match (player.seat == seat.0, own) {
            (true, false) => {
                commands.entity(entity).insert(Own);
            }
            (false, true) => {
                commands.entity(entity).remove::<Own>();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tank_of_the_local_seat_is_the_player_s_own() {
        let mut app = App::new();
        app.add_plugins(plugin);
        let tanks = [0, 1, 2].map(|seat| app.world_mut().spawn(Player { seat }).id());
        let own = |app: &mut App| {
            let world = app.world_mut();
            let mut marked = world.query_filtered::<Entity, With<Own>>();
            marked.iter(world).collect::<Vec<Entity>>()
        };
        app.update();
        assert_eq!(own(&mut app), [tanks[0]]);
        app.insert_resource(LocalSeat(2));
        app.update();
        assert_eq!(own(&mut app), [tanks[2]]);
    }
}
