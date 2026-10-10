//! Tank stats: their levels, what each level is worth and what it costs.

/// Total budget of the player tank.
pub const PLAYER_BUDGET: u16 = 59;

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
    Brain,
    Ballistics,
}

/// What a tank spends its points on, which decides its hue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpendingGroup {
    Mobility,
    Firepower,
    Utility,
    Software,
}

impl Stat {
    pub const COUNT: usize = 12;
    pub const HARDWARE: [Stat; 10] = [
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
    ];

    /// Cumulative price of each hardware level. Software is priced by [`Stat::share`].
    const fn prices(self) -> &'static [u8] {
        match self {
            Stat::Engine => &[0, 2, 4, 6, 8, 11, 14, 17],
            Stat::Tracks => &[0, 1, 2, 3, 4, 5, 6],
            Stat::Turret => &[0, 1, 2, 3, 5, 7, 10],
            Stat::Muzzle => &[0, 1, 3, 6, 9, 12, 16, 21],
            Stat::Magazine => &[0, 3, 6, 9, 12, 15],
            Stat::Loader => &[0, 1, 2, 4, 6, 9, 12, 16],
            Stat::Ricochet => &[0, 4, 10, 18],
            Stat::Mines => &[0, 4, 6, 8, 10],
            Stat::Shield => &[0, 6, 9, 12, 15, 18],
            Stat::Piercing => &[0, 4],
            Stat::Brain | Stat::Ballistics => &[],
        }
    }

    /// Share of the budget, in percent, that each software tier costs.
    const fn shares(self) -> &'static [u8] {
        match self {
            Stat::Brain => &[0, 8, 16, 24, 32],
            Stat::Ballistics => &[0, 5, 10, 15],
            _ => &[],
        }
    }

    pub const fn is_software(self) -> bool {
        matches!(self, Stat::Brain | Stat::Ballistics)
    }

    pub const fn max_level(self) -> u8 {
        let levels = if self.is_software() {
            self.shares().len()
        } else {
            self.prices().len()
        };
        levels as u8 - 1
    }

    /// Total price of a hardware level.
    pub const fn price(self, level: u8) -> u32 {
        self.prices()[level as usize] as u32
    }

    /// Price of a software tier for a tank with this budget.
    pub const fn share(self, tier: u8, budget: u32) -> u32 {
        (budget * self.shares()[tier as usize] as u32).div_ceil(100)
    }

    pub const fn group(self) -> SpendingGroup {
        match self {
            Stat::Engine | Stat::Tracks => SpendingGroup::Mobility,
            Stat::Turret
            | Stat::Muzzle
            | Stat::Magazine
            | Stat::Loader
            | Stat::Ricochet
            | Stat::Piercing => SpendingGroup::Firepower,
            Stat::Mines | Stat::Shield => SpendingGroup::Utility,
            Stat::Brain | Stat::Ballistics => SpendingGroup::Software,
        }
    }
}

/// A tank's full list of stat levels, with the budget that paid for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Genome {
    levels: [u8; Stat::COUNT],
    pub budget: u16,
}

impl Genome {
    /// The fixed tank of a player: 59 points and no software.
    pub const PLAYER: Genome = Genome {
        //       Eng Trk Tur Muz Mag Ldr Ric Min Shd Prc Brn Bal
        levels: [4, 5, 0, 2, 4, 6, 1, 2, 2, 0, 0, 0],
        budget: PLAYER_BUDGET,
    };

    /// A tank with every stat at level 0.
    pub const fn empty(budget: u16) -> Self {
        Self {
            levels: [0; Stat::COUNT],
            budget,
        }
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

    /// The group that received the most points.
    pub fn main_group(&self) -> SpendingGroup {
        let spent = |group| -> u32 {
            Stat::HARDWARE
                .iter()
                .filter(|stat| stat.group() == group)
                .map(|&stat| stat.price(self.level(stat)))
                .sum()
        };
        let candidates = [
            (spent(SpendingGroup::Mobility), SpendingGroup::Mobility),
            (spent(SpendingGroup::Firepower), SpendingGroup::Firepower),
            (spent(SpendingGroup::Utility), SpendingGroup::Utility),
            (self.software_cost(), SpendingGroup::Software),
        ];
        // The first group wins a tie, so the order above is the tie-break.
        let mut best = candidates[0];
        for candidate in candidates {
            if candidate.0 > best.0 {
                best = candidate;
            }
        }
        best.1
    }

    /// Top speed in m/s.
    pub fn top_speed(&self) -> f32 {
        [0.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0][self.level(Stat::Engine) as usize]
    }

    /// Hull turn rate in rad/s.
    pub fn hull_turn_rate(&self) -> f32 {
        (90.0 + 45.0 * self.level(Stat::Tracks) as f32).to_radians()
    }

    /// Turret turn rate in rad/s. Player turrets ignore this stat.
    pub fn turret_turn_rate(&self) -> f32 {
        let degrees: f32 =
            [45.0, 60.0, 90.0, 120.0, 180.0, 270.0, 360.0][self.level(Stat::Turret) as usize];
        degrees.to_radians()
    }

    /// Bullet speed in m/s. Each level above the one of the players' tank adds as much.
    pub fn bullet_speed(&self) -> f32 {
        [6.0, 7.5, 9.0, 11.0, 13.0, 15.0, 17.0, 19.0][self.level(Stat::Muzzle) as usize]
    }

    /// Rounds the magazine holds.
    pub fn magazine(&self) -> u8 {
        1 + self.level(Stat::Magazine)
    }

    /// Seconds needed to reload one round.
    pub fn reload_time(&self) -> f32 {
        [4.0, 3.2, 2.5, 2.0, 1.6, 1.25, 1.0, 0.8][self.level(Stat::Loader) as usize]
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
            level => Some(11.0 - level as f32),
        }
    }

    /// Normal bullets that a bullet of this tank destroys before one destroys it.
    pub fn piercing(&self) -> u8 {
        self.level(Stat::Piercing)
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
    fn full_hardware_costs_135_points() {
        let total: u32 = Stat::HARDWARE
            .iter()
            .map(|stat| stat.price(stat.max_level()))
            .sum();
        assert_eq!(total, 135);
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

        use crate::progression::{ballistics_cap, brain_cap, muzzle_cap, sector_budget};
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
                sector_budget(*sector) >= budget as u32
                    && brain_cap(*sector) >= genome.brain()
                    && ballistics_cap(*sector) >= genome.ballistics()
                    && muzzle_cap(*sector) >= genome.level(Muzzle)
            };
            assert_eq!((1..).find(possible), Some(sector), "{name}");
        }
        let hunter = build(96, builds[7].3);
        assert_eq!(hunter.main_group(), SpendingGroup::Software);
    }
}
