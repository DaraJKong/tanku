//! The tank that the player builds for free mode. It is kept on their machine, and the garage
//! opens on it the next time.

use std::collections::BTreeMap;

use bevy::prelude::*;
use tanku_core::stats::{Ammo, Genome, PLAYER_BUDGET, Stat};

use crate::storage;

const GARAGE_FILE: &str = "garage.txt";

/// The stats that the garage lists, each with its levels: every stat of a player's tank but
/// the four that are the type of its bullets. A tank fires one type, so that is one choice
/// among five and not four levels.
pub const LISTED: [Stat; 8] = [
    Stat::Engine,
    Stat::Tracks,
    Stat::Muzzle,
    Stat::Magazine,
    Stat::Loader,
    Stat::Ricochet,
    Stat::Mines,
    Stat::Shield,
];

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

    /// The tank with this type of bullet in place of the one that it fires, if a player may
    /// build that: the points pay for the type, less what the other one cost.
    pub fn loaded(&self, ammo: Ammo) -> Option<Genome> {
        let mut tank = self.tank;
        tank.load(ammo);
        tank.is_a_players().then_some(tank)
    }

    /// What the tank paid for and can't use, if anything: a bullet that explodes never
    /// bounces, and a bullet seeks at its bounces alone.
    pub fn wasted(&self) -> Option<&'static str> {
        match (self.tank.ammo(), self.tank.bounces()) {
            (Ammo::Exploding, 1..) => Some("Its bullets never bounce: Ricochet buys nothing."),
            (Ammo::Seeking, 0) => Some("Its bullets can't bounce: Seeking buys nothing."),
            _ => None,
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
        for stat in LISTED {
            assert_eq!(garage.changed(stat, 1), None, "{stat:?}");
        }
        // Giving up the shield frees its 9 points, which pay for two levels of Muzzle, with
        // a point left.
        garage.change(Stat::Shield, -1);
        garage.change(Stat::Shield, -1);
        assert_eq!((garage.tank.shield_cooldown(), garage.left()), (None, 9));
        assert_eq!(garage.changed(Stat::Shield, -1), None);
        garage.change(Stat::Muzzle, 1);
        garage.change(Stat::Muzzle, 1);
        assert_eq!((garage.tank.bullet_speed(), garage.left()), (13.0, 1));
        // A level that costs more than what is left changes nothing, and neither does a
        // type of bullet.
        let before = garage;
        garage.change(Stat::Muzzle, 1);
        assert_eq!(garage, before);
        assert_eq!(garage.loaded(Ammo::Piercing), None);
        // What holds an enemy back doesn't hold a player: piercing bullets, without a class.
        garage.change(Stat::Mines, -2);
        garage.tank = garage.loaded(Ammo::Piercing).unwrap();
        assert_eq!((garage.tank.piercing(), garage.left()), (1, 1));
    }

    #[test]
    fn a_tank_fires_one_type_of_bullet() {
        // A tank with 11 points left, which pay for any type.
        let mut garage = Garage::default();
        garage.change(Stat::Shield, -2);
        garage.change(Stat::Mines, -1);
        assert_eq!((garage.tank.ammo(), garage.left()), (Ammo::Normal, 11));
        for (ammo, left) in [
            (Ammo::Piercing, 5),
            (Ammo::Seeking, 1),
            (Ammo::Splitting, 2),
            (Ammo::Exploding, 1),
            (Ammo::Normal, 11),
        ] {
            // A type takes the place of the one before it, which gives its points back.
            garage.tank = garage.loaded(ammo).unwrap();
            assert_eq!((garage.tank.ammo(), garage.left()), (ammo, left));
            let types = Ammo::STATS.into_iter();
            let fired = types.filter(|stat| garage.tank.level(*stat) > 0).count();
            assert_eq!(fired, (ammo != Ammo::Normal) as usize);
            // The type that the tank fires is always its to keep.
            assert_eq!(garage.loaded(ammo), Some(garage.tank));
        }
        // What is left pays for a type, less what the type of the tank cost: a tank with 3
        // points left and piercing bullets has 9 for another type.
        garage.tank = garage.loaded(Ammo::Piercing).unwrap();
        garage.change(Stat::Mines, 1);
        assert_eq!(garage.left(), 3);
        assert!(garage.loaded(Ammo::Splitting).is_some());
        assert_eq!(garage.loaded(Ammo::Seeking), None);
        assert_eq!(garage.loaded(Ammo::Exploding), None);
        assert!(garage.loaded(Ammo::Normal).is_some());
        // The tank is kept with its type, and comes back with it.
        assert_eq!(Garage::from_pairs(&garage.to_pairs()), garage);
        // A file that holds two types holds no tank that a player may build.
        let mut pairs = garage.to_pairs();
        pairs.insert("seeking".to_string(), "1".to_string());
        assert_eq!(Garage::from_pairs(&pairs), Garage::default());
    }

    #[test]
    fn the_garage_tells_what_a_tank_paid_for_and_can_not_use() {
        // A tank with 11 points left, which pay for any type.
        let mut garage = Garage::default();
        garage.change(Stat::Shield, -2);
        garage.change(Stat::Mines, -1);
        // The standard tank has a bounce: seeking bullets use it, and exploding ones can't.
        assert_eq!(garage.wasted(), None);
        for ammo in [Ammo::Piercing, Ammo::Seeking, Ammo::Splitting] {
            garage.tank = garage.loaded(ammo).unwrap();
            assert_eq!(garage.wasted(), None, "{ammo:?}");
        }
        garage.tank = garage.loaded(Ammo::Exploding).unwrap();
        assert!(garage.wasted().unwrap().contains("Ricochet buys nothing"));
        garage.change(Stat::Ricochet, -1);
        assert_eq!(garage.wasted(), None);
        // Without a bounce, seeking bullets are normal bullets.
        garage.tank = garage.loaded(Ammo::Seeking).unwrap();
        assert!(garage.wasted().unwrap().contains("Seeking buys nothing"));
        garage.change(Stat::Ricochet, 1);
        assert_eq!(garage.wasted(), None);
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
