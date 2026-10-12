//! The tank that the player builds for free mode. It is kept on their machine, and the garage
//! opens on it the next time.

use std::collections::BTreeMap;

use bevy::prelude::*;
use tanku_core::stats::{Genome, PLAYER_BUDGET, Stat};

use crate::storage;

const GARAGE_FILE: &str = "garage.txt";

/// The tank of the player's garage: the one that they built last, or the standard tank until
/// they build one.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Garage {
    pub tank: Genome,
}

impl Default for Garage {
    fn default() -> Self {
        Self {
            tank: Genome::PLAYER,
        }
    }
}

impl Garage {
    /// The points that the tank leaves unspent.
    pub fn left(&self) -> u32 {
        (PLAYER_BUDGET as u32).saturating_sub(self.tank.cost())
    }

    /// The tank with so many levels more or less of a stat, if a player may build that: the
    /// stat has such a level, the points pay for it, and the tank keeps an engine.
    pub fn changed(&self, stat: Stat, by: i8) -> Option<Genome> {
        let wanted = self.tank.level(stat).checked_add_signed(by)?;
        let level = |own: Stat| {
            if own == stat {
                wanted
            } else {
                self.tank.level(own)
            }
        };
        let levels = Stat::GARAGE.map(|own| (own, level(own)));
        Genome::of_the_garage(&levels).ok()
    }

    /// Gives the tank so many levels more or less of a stat, if a player may build that.
    #[cfg(test)]
    fn change(&mut self, stat: Stat, by: i8) {
        if let Some(tank) = self.changed(stat, by) {
            self.tank = tank;
        }
    }

    /// The tank that a file holds: the standard one if the file holds none that a player may
    /// build.
    fn from_pairs(pairs: &BTreeMap<String, String>) -> Self {
        let level = |stat: Stat| pairs.get(stat.name())?.parse::<u8>().ok();
        let levels: Vec<(Stat, u8)> = Stat::GARAGE
            .into_iter()
            .filter_map(|stat| Some((stat, level(stat)?)))
            .collect();
        Self {
            tank: Genome::of_the_garage(&levels).unwrap_or(Genome::PLAYER),
        }
    }

    fn to_pairs(self) -> BTreeMap<String, String> {
        let pair = |stat: Stat| (stat.name().to_string(), self.tank.level(stat).to_string());
        Stat::GARAGE.into_iter().map(pair).collect()
    }
}

pub fn plugin(app: &mut App) {
    app.insert_resource(Garage::from_pairs(&storage::load(GARAGE_FILE)))
        .add_systems(Update, keep.run_if(resource_changed::<Garage>));
}

/// Keeps the tank whenever its player changes it, and not as it was just read.
fn keep(garage: Res<Garage>, mut read: Local<bool>) {
    if *read {
        storage::save(GARAGE_FILE, &garage.to_pairs());
    }
    *read = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_garage_opens_on_the_standard_tank_and_keeps_the_one_that_was_built() {
        assert_eq!(Garage::default().tank, Genome::PLAYER);
        assert_eq!(Garage::from_pairs(&BTreeMap::new()), Garage::default());
        let mut garage = Garage::default();
        garage.change(Stat::Shield, -2);
        garage.change(Stat::Muzzle, 3);
        assert_ne!(garage, Garage::default());
        assert_eq!(Garage::from_pairs(&garage.to_pairs()), garage);
        // A file that holds no tank that a player may build gives the standard one.
        let mut pairs = garage.to_pairs();
        pairs.insert("muzzle".to_string(), "7".to_string());
        pairs.insert("loader".to_string(), "7".to_string());
        pairs.insert("ricochet".to_string(), "3".to_string());
        assert_eq!(Garage::from_pairs(&pairs), Garage::default());
        pairs.insert("muzzle".to_string(), "fast".to_string());
        pairs.insert("engine".to_string(), "40".to_string());
        assert_eq!(Garage::from_pairs(&pairs), Garage::default());
    }

    #[test]
    fn a_level_is_bought_with_the_points_that_are_left() {
        let mut garage = Garage::default();
        // The standard tank spends every point: nothing more fits.
        assert_eq!(garage.left(), 0);
        for stat in Stat::GARAGE {
            assert_eq!(garage.changed(stat, 1), None, "{stat:?}");
        }
        // Giving up the shield frees its 9 points, which pay for three levels of Muzzle.
        garage.change(Stat::Shield, -1);
        garage.change(Stat::Shield, -1);
        assert_eq!((garage.tank.shield_cooldown(), garage.left()), (None, 9));
        assert_eq!(garage.changed(Stat::Shield, -1), None);
        garage.change(Stat::Muzzle, 1);
        garage.change(Stat::Muzzle, 1);
        garage.change(Stat::Muzzle, 1);
        assert_eq!((garage.tank.bullet_speed(), garage.left()), (15.0, 0));
        // A level that costs more than what is left changes nothing.
        let before = garage;
        garage.change(Stat::Muzzle, 1);
        garage.change(Stat::Piercing, 1);
        assert_eq!(garage, before);
        // What holds an enemy back doesn't hold a player: piercing bullets and a third
        // bounce, without a class.
        garage.change(Stat::Mines, -2);
        garage.change(Stat::Piercing, 1);
        assert_eq!((garage.tank.piercing(), garage.left()), (1, 2));
        assert_eq!(garage.changed(Stat::Piercing, 1), None);
    }

    #[test]
    fn a_tank_keeps_an_engine() {
        let mut garage = Garage::default();
        for _ in 0..10 {
            garage.change(Stat::Engine, -1);
        }
        assert_eq!(garage.tank.level(Stat::Engine), 1);
        assert_eq!(garage.tank.top_speed(), 2.0);
        assert_eq!(garage.changed(Stat::Engine, -1), None);
        // Every other stat goes down to nothing.
        for stat in Stat::GARAGE {
            for _ in 0..10 {
                garage.change(stat, -1);
            }
        }
        assert_eq!((garage.tank.cost(), garage.left()), (2, 57));
        assert!(garage.tank.is_a_players());
    }
}
