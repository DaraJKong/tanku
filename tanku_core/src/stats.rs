//! Tank stats: their levels, what each level is worth and what it costs.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::balance::{BALANCE, ClassRule, LEVELS};

/// Total budget of the player tank: the points of a tank of the garage.
pub const PLAYER_BUDGET: u16 = BALANCE.garage;

/// What each level of a stat gives, where that isn't the level itself, and what it costs:
/// both by stat, then by level.
const GIVES: &[[f32; LEVELS]; Stat::COUNT] = &BALANCE.stats.gives;
const COSTS: &[[u8; LEVELS]; Stat::COUNT] = &BALANCE.stats.costs;
/// What the balance says of each class.
const CLASSES: &[ClassRule; Class::ALL.len()] = &BALANCE.classes;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stat {
    Engine,
    Tracks,
    Turret,
    Muzzle,
    Magazine,
    Loader,
    Ricochet,
    Mines,
    Shield,
    Piercing,
    Seeking,
    Splitting,
    Exploding,
    Brain,
    Ballistics,
}

/// The type of a tank's bullets: what they do besides flying straight. A tank fires one type,
/// and each of the special ones is a stat of a single level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Ammo {
    #[default]
    Normal,
    /// Destroys the first normal bullet that it touches and flies on.
    Piercing,
    /// Leaves a wall toward a tank, not at the mirror angle.
    Seeking,
    /// Becomes two normal bullets after a moment.
    Splitting,
    /// Never bounces, and goes off where it ends.
    Exploding,
}

impl Ammo {
    pub const ALL: [Ammo; 5] = [
        Ammo::Normal,
        Ammo::Piercing,
        Ammo::Seeking,
        Ammo::Splitting,
        Ammo::Exploding,
    ];

    /// The stats of the special types. A tank has one of them at most.
    pub const STATS: [Stat; 4] = [
        Stat::Piercing,
        Stat::Seeking,
        Stat::Splitting,
        Stat::Exploding,
    ];

    /// The stat that a tank has when it fires this type. Normal bullets need none.
    pub const fn stat(self) -> Option<Stat> {
        match self {
            Ammo::Normal => None,
            Ammo::Piercing => Some(Stat::Piercing),
            Ammo::Seeking => Some(Stat::Seeking),
            Ammo::Splitting => Some(Stat::Splitting),
            Ammo::Exploding => Some(Stat::Exploding),
        }
    }

    /// What a tank pays to fire this type.
    pub const fn price(self) -> u32 {
        match self.stat() {
            Some(stat) => stat.price(1),
            None => 0,
        }
    }
}

/// The class of an enemy tank: the stats that it favors when it spends its points, which is
/// also what its hue tells. The tank of a player has none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Class {
    /// A bit of everything.
    #[default]
    None,
    /// A fast bullet and a turret that follows.
    Sniper,
    /// A tank that is hard to catch.
    Scout,
    /// The best brain and the best aim that its sector allows.
    Tactician,
    /// Bullets that bounce again and again.
    Trickster,
    /// Bullets that pierce: no other class has them.
    Lancer,
    /// Mines and a shield.
    Sapper,
    /// Many rounds, reloaded fast.
    Gunner,
    /// Bullets that seek at their bounces: no other class has them.
    Seeker,
    /// Bullets that split: no other class has them.
    Flanker,
    /// Bullets that explode: no other class has them.
    Grenadier,
}

impl Class {
    pub const ALL: [Class; 11] = [
        Class::None,
        Class::Sniper,
        Class::Scout,
        Class::Tactician,
        Class::Trickster,
        Class::Lancer,
        Class::Sapper,
        Class::Gunner,
        Class::Seeker,
        Class::Flanker,
        Class::Grenadier,
    ];

    /// The name that a class goes by where the balance is written down.
    pub const fn name(self) -> &'static str {
        match self {
            Class::None => "all-rounder",
            Class::Sniper => "sniper",
            Class::Scout => "scout",
            Class::Tactician => "tactician",
            Class::Trickster => "trickster",
            Class::Lancer => "lancer",
            Class::Sapper => "sapper",
            Class::Gunner => "gunner",
            Class::Seeker => "seeker",
            Class::Flanker => "flanker",
            Class::Grenadier => "grenadier",
        }
    }

    /// The stats that a tank of this class favors, the one it takes first in front.
    pub const fn favorites(self) -> &'static [Stat] {
        CLASSES[self as usize].favorites()
    }

    /// The highest level of a stat that a tank of this class can have. Some things are kept
    /// for the classes that are about them. Each special bullet is the one of the class that
    /// favors it, and of no other. And the balance says how much each class has of ricochet
    /// and of ballistics: bullets that bounce twice and the bank shots that need them are
    /// for a few.
    pub const fn highest(self, stat: Stat) -> u8 {
        let rule = &CLASSES[self as usize];
        match stat {
            Stat::Piercing | Stat::Seeking | Stat::Splitting | Stat::Exploding => {
                if rule.favors(stat) {
                    stat.max_level()
                } else {
                    0
                }
            }
            Stat::Ricochet => rule.ricochet,
            Stat::Ballistics => rule.ballistics,
            _ => stat.max_level(),
        }
    }
}

impl Stat {
    pub const COUNT: usize = 15;
    /// Every stat, in the order of their numbers.
    pub const ALL: [Stat; Stat::COUNT] = [
        Stat::Engine,
        Stat::Tracks,
        Stat::Turret,
        Stat::Muzzle,
        Stat::Magazine,
        Stat::Loader,
        Stat::Ricochet,
        Stat::Mines,
        Stat::Shield,
        Stat::Piercing,
        Stat::Seeking,
        Stat::Splitting,
        Stat::Exploding,
        Stat::Brain,
        Stat::Ballistics,
    ];
    pub const HARDWARE: [Stat; 13] = [
        Stat::Engine,
        Stat::Tracks,
        Stat::Turret,
        Stat::Muzzle,
        Stat::Magazine,
        Stat::Loader,
        Stat::Ricochet,
        Stat::Mines,
        Stat::Shield,
        Stat::Piercing,
        Stat::Seeking,
        Stat::Splitting,
        Stat::Exploding,
    ];

    /// The stats of a tank that a player builds: every piece of hardware but the turret,
    /// which follows the cursor on a player's tank.
    pub const GARAGE: [Stat; 12] = [
        Stat::Engine,
        Stat::Tracks,
        Stat::Muzzle,
        Stat::Magazine,
        Stat::Loader,
        Stat::Ricochet,
        Stat::Mines,
        Stat::Shield,
        Stat::Piercing,
        Stat::Seeking,
        Stat::Splitting,
        Stat::Exploding,
    ];

    /// The name that a stat goes by where a tank is written down.
    pub const fn name(self) -> &'static str {
        match self {
            Stat::Engine => "engine",
            Stat::Tracks => "tracks",
            Stat::Turret => "turret",
            Stat::Muzzle => "muzzle",
            Stat::Magazine => "magazine",
            Stat::Loader => "loader",
            Stat::Ricochet => "ricochet",
            Stat::Mines => "mines",
            Stat::Shield => "shield",
            Stat::Piercing => "piercing",
            Stat::Seeking => "seeking",
            Stat::Splitting => "splitting",
            Stat::Exploding => "exploding",
            Stat::Brain => "brain",
            Stat::Ballistics => "ballistics",
        }
    }

    /// Cumulative price of each hardware level. Software is priced by [`Stat::share`].
    const fn prices(self) -> &'static [u8] {
        if self.is_software() {
            return &[];
        }
        COSTS[self as usize].split_at(self.levels()).0
    }

    /// Share of the budget, in percent, that each software tier costs.
    const fn shares(self) -> &'static [u8] {
        if !self.is_software() {
            return &[];
        }
        COSTS[self as usize].split_at(self.levels()).0
    }

    pub const fn is_software(self) -> bool {
        matches!(self, Stat::Brain | Stat::Ballistics)
    }

    /// How many levels the stat has, level 0 included. The balance says what each of them
    /// gives and costs, and not how many there are: the parts of a tank are drawn for these.
    pub const fn levels(self) -> usize {
        match self {
            Stat::Engine | Stat::Muzzle | Stat::Loader => 8,
            Stat::Tracks | Stat::Turret => 7,
            Stat::Magazine | Stat::Shield => 6,
            Stat::Mines | Stat::Brain => 5,
            Stat::Ricochet | Stat::Ballistics => 4,
            Stat::Piercing | Stat::Seeking | Stat::Splitting | Stat::Exploding => 2,
        }
    }

    pub const fn max_level(self) -> u8 {
        self.levels() as u8 - 1
    }

    /// Total price of a hardware level.
    pub const fn price(self, level: u8) -> u32 {
        self.prices()[level as usize] as u32
    }

    /// Price of a software tier for a tank with this budget.
    pub const fn share(self, tier: u8, budget: u32) -> u32 {
        (budget * self.shares()[tier as usize] as u32).div_ceil(100)
    }
}

/// Why a tank is none that a player may build.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flaw {
    /// It has a stat that a player's tank doesn't have: its turret follows the cursor, and
    /// its player is its brain.
    NotAPlayers(Stat),
    /// A stat is above its highest level, which is this one.
    TooHigh(Stat, u8),
    /// It can't drive, so it would never come within reach of a barrier.
    NoEngine,
    /// It has two of the special types of bullet, and a tank fires one type.
    TwoAmmos(Stat, Stat),
    /// It costs this many points, which is more than a player has.
    TooDear(u32),
}

impl fmt::Display for Flaw {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Flaw::NotAPlayers(stat) => {
                write!(formatter, "the tank of a player has no {}", stat.name())
            }
            Flaw::TooHigh(stat, highest) => {
                write!(formatter, "{} goes up to {highest}", stat.name())
            }
            Flaw::NoEngine => {
                write!(
                    formatter,
                    "it needs engine 1 at least, to reach the barriers"
                )
            }
            Flaw::TwoAmmos(one, other) => write!(
                formatter,
                "a tank fires one type of bullet, not {} and {}",
                one.name(),
                other.name()
            ),
            Flaw::TooDear(cost) => write!(
                formatter,
                "it costs {cost} points, and a player has {PLAYER_BUDGET}"
            ),
        }
    }
}

/// A tank's full list of stat levels, with the budget that paid for them and the class that
/// chose them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Genome {
    levels: [u8; Stat::COUNT],
    pub budget: u16,
    pub class: Class,
}

impl Genome {
    /// The fixed tank of a player, which the balance calls the standard tank. It has no
    /// software.
    pub const PLAYER: Genome = Genome {
        levels: BALANCE.standard,
        budget: PLAYER_BUDGET,
        class: Class::None,
    };

    /// A tank with every stat at level 0, and no class.
    pub const fn empty(budget: u16) -> Self {
        Self {
            levels: [0; Stat::COUNT],
            budget,
            class: Class::None,
        }
    }

    /// The tank that a player builds with the points of the standard tank: the levels that
    /// they chose, and nothing of the rest. What holds an enemy back doesn't hold it: it
    /// has any piece of hardware at any level, and needs no brain for mines or a shield.
    /// Points may be left over.
    pub fn of_the_garage(levels: &[(Stat, u8)]) -> Result<Self, Flaw> {
        let mut genome = Self::empty(PLAYER_BUDGET);
        for &(stat, level) in levels {
            if level > stat.max_level() {
                return Err(Flaw::TooHigh(stat, stat.max_level()));
            }
            genome.set(stat, level);
        }
        match genome.flaw() {
            Some(flaw) => Err(flaw),
            None => Ok(genome),
        }
    }

    /// What keeps this tank from being one that a player built, if anything does. It says
    /// nothing of where the tank comes from: one that another machine sent is checked
    /// with it too.
    pub fn flaw(&self) -> Option<Flaw> {
        let above = |&stat: &Stat| self.level(stat) > stat.max_level();
        let every = Stat::HARDWARE
            .iter()
            .chain(&[Stat::Brain, Stat::Ballistics]);
        if let Some(&stat) = every.clone().find(|stat| above(stat)) {
            return Some(Flaw::TooHigh(stat, stat.max_level()));
        }
        let own = |stat: &&Stat| !Stat::GARAGE.contains(stat) && self.level(**stat) > 0;
        if let Some(&stat) = every.clone().find(own) {
            return Some(Flaw::NotAPlayers(stat));
        }
        let mut ammos = Ammo::STATS.into_iter().filter(|stat| self.level(*stat) > 0);
        if let (Some(one), Some(other)) = (ammos.next(), ammos.next()) {
            return Some(Flaw::TwoAmmos(one, other));
        }
        if self.level(Stat::Engine) == 0 {
            return Some(Flaw::NoEngine);
        }
        let cost = self.cost();
        (cost > PLAYER_BUDGET as u32).then_some(Flaw::TooDear(cost))
    }

    /// Whether a player may have built this tank: it has the budget of a player's tank, no
    /// class and no flaw.
    pub fn is_a_players(&self) -> bool {
        self.budget == PLAYER_BUDGET && self.class == Class::None && self.flaw().is_none()
    }

    pub const fn level(&self, stat: Stat) -> u8 {
        self.levels[stat as usize]
    }

    pub fn set(&mut self, stat: Stat, level: u8) {
        debug_assert!(level <= stat.max_level());
        self.levels[stat as usize] = level;
    }

    pub fn hardware_cost(&self) -> u32 {
        Stat::HARDWARE
            .iter()
            .map(|&stat| stat.price(self.level(stat)))
            .sum()
    }

    pub fn software_cost(&self) -> u32 {
        let budget = self.budget as u32;
        Stat::Brain.share(self.level(Stat::Brain), budget)
            + Stat::Ballistics.share(self.level(Stat::Ballistics), budget)
    }

    pub fn cost(&self) -> u32 {
        self.hardware_cost() + self.software_cost()
    }

    /// What the level of a stat gives this tank, for a stat whose levels give something
    /// else than themselves.
    fn gives(&self, stat: Stat) -> f32 {
        GIVES[stat as usize][..stat.levels()][self.level(stat) as usize]
    }

    /// Top speed in m/s.
    pub fn top_speed(&self) -> f32 {
        self.gives(Stat::Engine)
    }

    /// Hull turn rate in rad/s.
    pub fn hull_turn_rate(&self) -> f32 {
        self.gives(Stat::Tracks).to_radians()
    }

    /// Turret turn rate in rad/s. Player turrets ignore this stat.
    pub fn turret_turn_rate(&self) -> f32 {
        self.gives(Stat::Turret).to_radians()
    }

    /// Bullet speed in m/s.
    pub fn bullet_speed(&self) -> f32 {
        self.gives(Stat::Muzzle)
    }

    /// Rounds the magazine holds.
    pub fn magazine(&self) -> u8 {
        1 + self.level(Stat::Magazine)
    }

    /// Seconds needed to reload one round.
    pub fn reload_time(&self) -> f32 {
        self.gives(Stat::Loader)
    }

    /// Times a bullet bounces before the next impact destroys it.
    pub fn bounces(&self) -> u8 {
        self.level(Stat::Ricochet)
    }

    /// Mines the tank can have armed at once.
    pub fn mines(&self) -> u8 {
        self.level(Stat::Mines)
    }

    /// Seconds before the shield can be raised again, if the tank has one.
    pub fn shield_cooldown(&self) -> Option<f32> {
        match self.level(Stat::Shield) {
            0 => None,
            _ => Some(self.gives(Stat::Shield)),
        }
    }

    /// Normal bullets that a bullet of this tank destroys before one destroys it.
    pub fn piercing(&self) -> u8 {
        self.level(Stat::Piercing)
    }

    /// The type of its bullets. No tank that the game builds has two special types, and a
    /// tank that was given two fires the first of them.
    pub fn ammo(&self) -> Ammo {
        let special = |ammo: &Ammo| ammo.stat().is_some_and(|stat| self.level(stat) > 0);
        Ammo::ALL.into_iter().find(special).unwrap_or_default()
    }

    /// Gives the tank this type of bullet, and none of the others.
    pub fn load(&mut self, ammo: Ammo) {
        for stat in Ammo::STATS {
            self.set(stat, (ammo.stat() == Some(stat)) as u8);
        }
    }

    pub fn brain(&self) -> u8 {
        self.level(Stat::Brain)
    }

    pub fn ballistics(&self) -> u8 {
        self.level(Stat::Ballistics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(budget: u16, levels: &[(Stat, u8)]) -> Genome {
        let mut genome = Genome::empty(budget);
        for &(stat, level) in levels {
            genome.set(stat, level);
        }
        genome
    }

    #[test]
    fn player_tank_matches_the_specification() {
        let player = Genome::PLAYER;
        assert_eq!(player.cost(), PLAYER_BUDGET as u32);
        assert_eq!(player.top_speed(), 5.0);
        assert_eq!(player.hull_turn_rate().to_degrees().round(), 315.0);
        assert_eq!(player.bullet_speed(), 9.0);
        assert_eq!(player.magazine(), 5);
        assert_eq!(player.reload_time(), 1.0);
        assert_eq!(player.bounces(), 1);
        assert_eq!(player.mines(), 2);
        assert_eq!(player.shield_cooldown(), Some(9.0));
        assert_eq!(player.piercing(), 0);
    }

    #[test]
    fn a_player_builds_a_tank_within_the_rules_of_the_garage() {
        use Stat::*;
        // The standard tank is one that a player may build.
        let standard = [
            (Engine, 4),
            (Tracks, 5),
            (Muzzle, 2),
            (Magazine, 4),
            (Loader, 6),
            (Ricochet, 1),
            (Mines, 2),
            (Shield, 2),
        ];
        assert_eq!(Genome::of_the_garage(&standard), Ok(Genome::PLAYER));
        assert!(Genome::PLAYER.is_a_players());
        // What holds an enemy back doesn't hold a player: the fastest gun, piercing bullets
        // and three bounces, mines and a shield without a brain, at their price. Points
        // may be left over.
        let parts = [
            (Engine, 1),
            (Muzzle, 7),
            (Ricochet, 3),
            (Piercing, 1),
            (Mines, 1),
            (Shield, 1),
        ];
        let gun = Genome::of_the_garage(&parts).unwrap();
        assert_eq!(
            (gun.bullet_speed(), gun.bounces(), gun.piercing()),
            (19.0, 3, 1)
        );
        assert_eq!((gun.mines(), gun.shield_cooldown()), (1, Some(10.0)));
        assert_eq!((gun.cost(), gun.budget), (55, PLAYER_BUDGET));
        assert_eq!((gun.class, gun.brain()), (Class::None, 0));
        assert!(gun.is_a_players());
        // A tank that can't drive would never come within reach of a barrier.
        assert_eq!(Genome::of_the_garage(&[(Muzzle, 7)]), Err(Flaw::NoEngine));
        // No tank costs more than the standard one.
        let dear = [(Engine, 7), (Muzzle, 7), (Loader, 7), (Magazine, 2)];
        assert_eq!(Genome::of_the_garage(&dear), Err(Flaw::TooDear(60)));
        let fits = [
            (Engine, 7),
            (Muzzle, 7),
            (Loader, 7),
            (Magazine, 1),
            (Tracks, 2),
        ];
        assert_eq!(Genome::of_the_garage(&fits).map(|tank| tank.cost()), Ok(59));
        // The turret of a player's tank follows the cursor, and its player is its brain.
        for stat in [Turret, Brain, Ballistics] {
            let tank = Genome::of_the_garage(&[(Engine, 1), (stat, 1)]);
            assert_eq!(tank, Err(Flaw::NotAPlayers(stat)));
        }
        // No stat goes above its highest level.
        let fast = Genome::of_the_garage(&[(Engine, 8)]);
        assert_eq!(fast, Err(Flaw::TooHigh(Engine, 7)));
        let sharp = Genome::of_the_garage(&[(Engine, 1), (Piercing, 2)]);
        assert_eq!(sharp, Err(Flaw::TooHigh(Piercing, 1)));
        // A tank fires one type of bullet: any of the special ones, at its price, and never
        // two of them.
        for (ammo, price) in [
            (Ammo::Normal, 0),
            (Ammo::Piercing, 4),
            (Ammo::Seeking, 4),
            (Ammo::Splitting, 6),
            (Ammo::Exploding, 8),
        ] {
            let mut tank = Genome::of_the_garage(&[(Engine, 1)]).unwrap();
            tank.load(ammo);
            assert_eq!((tank.ammo(), tank.cost()), (ammo, 2 + price));
            assert_eq!(ammo.price(), price);
            assert!(tank.is_a_players());
        }
        let both = Genome::of_the_garage(&[(Engine, 1), (Seeking, 1), (Exploding, 1)]);
        assert_eq!(both, Err(Flaw::TwoAmmos(Seeking, Exploding)));
        // Loading a type unloads the one that the tank had.
        let mut tank = Genome::PLAYER;
        tank.load(Ammo::Splitting);
        tank.load(Ammo::Piercing);
        assert_eq!((tank.ammo(), tank.piercing()), (Ammo::Piercing, 1));
        assert_eq!(tank.level(Splitting), 0);
        tank.load(Ammo::Normal);
        assert_eq!(tank, Genome::PLAYER);
    }

    #[test]
    fn a_tank_from_elsewhere_is_checked_as_one_from_the_garage() {
        // A tank that another machine sent may hold anything. Its levels are checked before
        // anything is made of them.
        let mut levels = [0; Stat::COUNT];
        levels[Stat::Engine as usize] = 200;
        let broken = Genome {
            levels,
            budget: PLAYER_BUDGET,
            class: Class::None,
        };
        assert_eq!(broken.flaw(), Some(Flaw::TooHigh(Stat::Engine, 7)));
        assert!(!broken.is_a_players());
        // The tank of an enemy is none of a player's, whatever it is made of.
        let mut enemy = Genome::PLAYER;
        enemy.class = Class::Scout;
        assert!(enemy.flaw().is_none() && !enemy.is_a_players());
        let mut rich = Genome::PLAYER;
        rich.budget = PLAYER_BUDGET + 1;
        assert!(!rich.is_a_players());
        let mut smart = Genome::PLAYER;
        smart.set(Stat::Brain, 1);
        assert_eq!(smart.flaw(), Some(Flaw::NotAPlayers(Stat::Brain)));
    }

    #[test]
    fn a_flaw_says_what_is_wrong_with_a_tank() {
        let said = |flaw: Flaw| flaw.to_string();
        assert_eq!(
            said(Flaw::NotAPlayers(Stat::Turret)),
            "the tank of a player has no turret"
        );
        assert_eq!(said(Flaw::TooHigh(Stat::Muzzle, 7)), "muzzle goes up to 7");
        assert_eq!(
            said(Flaw::NoEngine),
            "it needs engine 1 at least, to reach the barriers"
        );
        assert_eq!(
            said(Flaw::TooDear(63)),
            "it costs 63 points, and a player has 59"
        );
        assert_eq!(
            said(Flaw::TwoAmmos(Stat::Piercing, Stat::Splitting)),
            "a tank fires one type of bullet, not piercing and splitting"
        );
        // A stat goes by one name, and no two by the same.
        let every = Stat::HARDWARE
            .iter()
            .chain(&[Stat::Brain, Stat::Ballistics]);
        let mut names: Vec<&str> = every.map(|stat| stat.name()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), Stat::COUNT);
        // The garage has every piece of hardware but the turret.
        let hardware = Stat::HARDWARE
            .into_iter()
            .filter(|stat| *stat != Stat::Turret);
        assert!(hardware.eq(Stat::GARAGE));
    }

    #[test]
    fn bullets_gain_2_m_per_second_with_each_level_above_the_players_own() {
        let speed = |level: u8| build(0, &[(Stat::Muzzle, level)]).bullet_speed();
        let own = Genome::PLAYER.level(Stat::Muzzle);
        assert_eq!((speed(0), speed(1), speed(own)), (6.0, 7.5, 9.0));
        for level in own + 1..=Stat::Muzzle.max_level() {
            assert_eq!(speed(level) - speed(level - 1), 2.0);
        }
        assert_eq!(speed(Stat::Muzzle.max_level()), 19.0);
    }

    #[test]
    fn full_hardware_costs_139_points() {
        // Every stat at its highest level, with the dearest of the types of bullet: a tank
        // has one type.
        let plain: u32 = Stat::HARDWARE
            .iter()
            .filter(|stat| !Ammo::STATS.contains(stat))
            .map(|stat| stat.price(stat.max_level()))
            .sum();
        let dearest = Ammo::ALL.map(Ammo::price).into_iter().max();
        assert_eq!((plain, dearest), (131, Some(8)));
    }

    #[test]
    fn the_special_bullets_are_priced_by_what_they_add() {
        let price = |ammo: Ammo| ammo.price();
        assert_eq!(price(Ammo::Piercing), price(Ammo::Seeking));
        assert!(price(Ammo::Seeking) < price(Ammo::Splitting));
        assert!(price(Ammo::Splitting) < price(Ammo::Exploding));
        // Each is one class's alone, and no class has two.
        for class in Class::ALL {
            let own = Ammo::STATS
                .into_iter()
                .filter(|stat| class.highest(*stat) > 0)
                .count();
            let special = matches!(
                class,
                Class::Lancer | Class::Seeker | Class::Flanker | Class::Grenadier
            );
            assert_eq!(own, special as usize, "{class:?}");
        }
        // A bullet that explodes never bounces, and a bullet seeks at each of its bounces.
        assert_eq!(Class::Grenadier.highest(Stat::Ricochet), 0);
        assert_eq!(Class::Seeker.highest(Stat::Ricochet), 3);
        assert_eq!(Class::Flanker.highest(Stat::Ricochet), 1);
        for class in [Class::Seeker, Class::Flanker, Class::Grenadier] {
            assert_eq!(class.highest(Stat::Ballistics), 1, "{class:?}");
        }
    }

    #[test]
    fn no_level_is_cheaper_than_the_one_before_it() {
        for stat in Stat::HARDWARE {
            let step = |level: u8| stat.price(level) - stat.price(level - 1);
            for level in 2..=stat.max_level() {
                // The first level of these buys a new ability, which the next levels only
                // improve.
                let entry_fee = matches!(stat, Stat::Mines | Stat::Shield) && level == 2;
                assert!(
                    entry_fee || step(level) >= step(level - 1),
                    "{stat:?} {level}"
                );
            }
        }
    }

    #[test]
    fn shield_levels_match_the_specification() {
        let cooldowns = [None, Some(10.0), Some(9.0), Some(8.0), Some(7.0), Some(6.0)];
        for (level, cooldown) in (0..).zip(cooldowns) {
            let genome = build(0, &[(Stat::Shield, level)]);
            assert_eq!(genome.shield_cooldown(), cooldown);
        }
    }

    #[test]
    fn reference_builds_cost_what_the_specification_says() {
        use Stat::*;

        use crate::{
            modes::Difficulty::Normal,
            progression::{ballistics_cap, brain_cap, muzzle_cap, sector_budget},
        };
        // Each build with its budget and the first sector that can have it.
        let builds: [(&str, u16, u32, &[(Stat, u8)]); 8] = [
            (
                "sentry",
                10,
                1,
                &[(Turret, 2), (Muzzle, 2), (Loader, 1), (Ricochet, 1)],
            ),
            (
                "rover",
                15,
                2,
                &[
                    (Engine, 1),
                    (Tracks, 1),
                    (Turret, 2),
                    (Muzzle, 2),
                    (Loader, 1),
                    (Ricochet, 1),
                    (Brain, 1),
                ],
            ),
            (
                "sniper",
                19,
                6,
                &[
                    (Engine, 1),
                    (Tracks, 1),
                    (Turret, 3),
                    (Muzzle, 3),
                    (Loader, 2),
                    (Brain, 2),
                    (Ballistics, 1),
                ],
            ),
            (
                "burster",
                34,
                6,
                &[
                    (Engine, 3),
                    (Tracks, 2),
                    (Turret, 3),
                    (Muzzle, 2),
                    (Magazine, 2),
                    (Loader, 3),
                    (Ricochet, 1),
                    (Brain, 2),
                ],
            ),
            (
                "miner",
                39,
                7,
                &[
                    (Engine, 5),
                    (Tracks, 3),
                    (Turret, 2),
                    (Muzzle, 2),
                    (Loader, 2),
                    (Ricochet, 1),
                    (Mines, 4),
                    (Brain, 1),
                ],
            ),
            (
                "banker",
                42,
                13,
                &[
                    (Turret, 4),
                    (Muzzle, 4),
                    (Magazine, 1),
                    (Loader, 3),
                    (Ricochet, 2),
                    (Brain, 1),
                    (Ballistics, 3),
                ],
            ),
            (
                "raider",
                72,
                19,
                &[
                    (Engine, 5),
                    (Tracks, 3),
                    (Turret, 4),
                    (Muzzle, 2),
                    (Magazine, 4),
                    (Loader, 4),
                    (Ricochet, 1),
                    (Mines, 2),
                    (Brain, 3),
                    (Ballistics, 1),
                ],
            ),
            (
                "hunter",
                96,
                46,
                &[
                    (Engine, 7),
                    (Tracks, 5),
                    (Turret, 5),
                    (Muzzle, 6),
                    (Magazine, 1),
                    (Loader, 4),
                    (Mines, 2),
                    (Brain, 4),
                    (Ballistics, 1),
                ],
            ),
        ];
        for (name, budget, sector, levels) in builds {
            let genome = build(budget, levels);
            assert_eq!(genome.cost(), budget as u32, "{name}");
            let possible = |sector: &u32| {
                sector_budget(*sector, Normal) >= budget as u32
                    && brain_cap(*sector, Normal) >= genome.brain()
                    && ballistics_cap(*sector, Normal) >= genome.ballistics()
                    && muzzle_cap(*sector, Normal) >= genome.level(Muzzle)
            };
            assert_eq!((1..).find(possible), Some(sector), "{name}");
        }
    }
}
