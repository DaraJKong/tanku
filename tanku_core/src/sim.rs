//! The state of the simulation, which the server owns and the client reads.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    map::SectorLayout,
    modes::Difficulty,
    rules::SPLIT_TIME,
    stats::{Ammo, Genome},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Team {
    Players,
    Enemies,
}

/// Position on the ground plane: `x` is east and `y` is north.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Position(pub Vec2);

/// Position one tick ago, for smooth rendering between ticks.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PreviousPosition(pub Vec2);

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
#[require(Position, PreviousPosition, Motion, TankCommand, Shield)]
pub struct Tank {
    pub genome: Genome,
    pub team: Team,
    /// The sector the tank is in.
    pub sector: u32,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Motion {
    pub velocity: Vec2,
    /// Angle of the hull, counterclockwise from east.
    pub hull: f32,
    /// Angle of the turret, counterclockwise from east.
    pub turret: f32,
}

/// What a player or a brain asks of its tank this tick.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
pub struct TankCommand {
    /// Direction to drive in, no longer than 1.
    pub drive: Vec2,
    /// The point to aim at.
    pub aim: Vec2,
    pub fire: bool,
    pub mine: bool,
    pub shield: bool,
    /// Repair the wreck of a teammate that is within reach.
    pub repair: bool,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gun {
    pub rounds: u8,
    /// Time spent reloading the next round.
    pub reload: f32,
    /// Time before the next shot.
    pub cooldown: f32,
    /// Time before the next mine.
    pub mine_cooldown: f32,
    /// Mines of this tank that lie on the ground.
    pub mines: u8,
}

impl Gun {
    pub fn loaded(genome: &Genome) -> Self {
        Self {
            rounds: genome.magazine(),
            reload: 0.0,
            cooldown: 0.0,
            mine_cooldown: 0.0,
            mines: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum ShieldState {
    /// The bubble is down. It can be raised once the cooldown is over.
    #[default]
    Ready,
    /// The bubble is up for this much longer.
    Active(f32),
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Shield {
    pub state: ShieldState,
    /// Time before the shield can be raised again.
    pub cooldown: f32,
}

impl Shield {
    pub fn is_active(&self) -> bool {
        matches!(self.state, ShieldState::Active(_))
    }
}

/// The tank of a player. Seats number the players of a run from 0.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub seat: u8,
}

/// A player's tank that was destroyed in a run of several players. It stays where it is until
/// a teammate has repaired it, and is in nobody's way: tanks drive over it and bullets fly
/// over it, so that it gives no cover to the tank that repairs it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Wreck {
    /// Seconds of repair done so far.
    pub repair: f32,
    /// The seats of the players whose tanks are repairing it, one bit for each. A tank
    /// repairs one wreck at a time, so a seat is in one wreck at most.
    pub repairers: u8,
}

impl Wreck {
    /// Whether the tank of the player of a seat is repairing it.
    pub fn repaired_by(&self, seat: u8) -> bool {
        self.repairers & (1 << seat) != 0
    }
}

/// A tank that was just repaired: nothing destroys it for this much longer.
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Protected(pub f32);

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Enemy {
    /// Enemies are dormant until a player enters their sector.
    pub awake: bool,
}

/// What a bullet does besides flying straight and bouncing. Piercing is counted apart, in
/// what a bullet has left to pierce.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    #[default]
    Plain,
    /// Each of its bounces sends it toward a tank that its side fights, not off at the mirror
    /// angle.
    Seeking,
    /// It becomes two plain bullets once its time is up, and doesn't bounce before that.
    Splitting {
        /// Seconds before it splits.
        fuse: f32,
        /// The bounces that each of the two has.
        bounces: u8,
    },
    /// It never bounces, and goes off in a blast where it ends.
    Exploding,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[require(Position, PreviousPosition)]
pub struct Bullet {
    pub owner: Entity,
    pub team: Team,
    pub sector: u32,
    pub velocity: Vec2,
    pub bounces_left: u8,
    /// Normal bullets it still destroys without being destroyed.
    pub pierces_left: u8,
    /// A player's bullet can't hit its owner before it has bounced.
    pub bounced: bool,
    pub kind: Kind,
}

impl Bullet {
    /// A bullet as it leaves the gun of a tank, with what the tank's stats give it.
    pub fn fired(owner: Entity, tank: &Tank, velocity: Vec2) -> Self {
        let genome = &tank.genome;
        let ammo = genome.ammo();
        let (bounces_left, kind) = match ammo {
            Ammo::Normal | Ammo::Piercing => (genome.bounces(), Kind::Plain),
            Ammo::Seeking => (genome.bounces(), Kind::Seeking),
            // The bounces of the tank are for the two bullets that it becomes.
            Ammo::Splitting => {
                let (fuse, bounces) = (SPLIT_TIME, genome.bounces());
                (0, Kind::Splitting { fuse, bounces })
            }
            Ammo::Exploding => (0, Kind::Exploding),
        };
        Self {
            owner,
            team: tank.team,
            sector: tank.sector,
            velocity,
            bounces_left,
            pierces_left: (ammo == Ammo::Piercing) as u8,
            bounced: false,
            kind,
        }
    }

    /// One of the two bullets that a splitting bullet becomes: a plain one, that flies this
    /// way with these bounces.
    pub fn half(&self, velocity: Vec2, bounces: u8) -> Self {
        Self {
            velocity,
            bounces_left: bounces,
            pierces_left: 0,
            bounced: false,
            kind: Kind::Plain,
            ..*self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MineState {
    /// Harmless to drive over for this much longer.
    Arming(f32),
    /// Waiting, with this much fuse left.
    Armed(f32),
    /// An opposing tank came close, or the fuse is about to run out: the mine warns, and blows
    /// after this much longer.
    Triggered(f32),
}

#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
#[require(Position)]
pub struct Mine {
    pub owner: Entity,
    pub team: Team,
    pub sector: u32,
    pub state: MineState,
}

/// A destructible block. Its position is its center.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
#[require(Position)]
pub struct Block {
    pub sector: u32,
}

/// An explosive crate. Its position is its center.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
#[require(Position)]
pub struct ExplosiveCrate {
    pub sector: u32,
}

/// A loaded sector: its walls and its floor.
#[derive(Component, Clone, Debug)]
pub struct Sector {
    pub layout: SectorLayout,
}

/// Which sector a loaded sector is. That is all a server tells of it: the seed gives the rest,
/// which is the same on every machine.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectorIndex(pub u32);

/// The energy wall at the end of a sector.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Barrier {
    pub sector: u32,
    pub from: Vec2,
    pub to: Vec2,
    /// Unit vector that points into the next sector.
    pub outward: Vec2,
    pub unlocked: bool,
}

/// A tank of arena mode that has not come yet. It waits in reserve outside the room, where the
/// players read it, until its place in the room is marked.
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[require(Position)]
pub struct Reserve {
    pub genome: Genome,
    /// The group that it is part of, and which tank of that group it is.
    pub group: u32,
    pub number: u32,
}

/// A block or a crate of arena mode that is on its way. Its place is inked in on the floor,
/// and it stands there once its time has come and nothing is in its place. Its position is
/// its center.
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[require(Position)]
pub struct Planned {
    /// Whether it is a crate. It is a block otherwise.
    pub explosive: bool,
}

/// A life that an arena left on its floor when it was cleared. A single player who is short
/// of one takes it by driving over it, and it is lost once the player has left the arena.
#[derive(Component, Clone, Copy, Debug, Serialize, Deserialize)]
#[require(Position)]
pub struct Life {
    pub sector: u32,
}

/// What destroys a tank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Weapon {
    Bullet,
    /// The blast of a mine.
    Mine,
    /// The blast of an explosive crate.
    Crate,
}

/// The blow that destroyed a tank: the weapon, and the tank behind it. That is the tank that
/// fired the bullet, the one that laid the mine, or the one that set the crate off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blow {
    pub weapon: Weapon,
    pub genome: Genome,
    pub team: Team,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RunPhase {
    Playing,
    /// The player was destroyed and comes back after this much longer.
    Respawning(f32),
    Over,
}

/// The run in progress.
#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub seed_text: String,
    pub seed: u64,
    /// How hard the enemies of the run are.
    pub difficulty: Difficulty,
    /// Players in the run. Sectors are generated for this many.
    pub players: u32,
    /// Players at the start of the run. Its record is the one of a team of this size.
    pub team: u32,
    /// Whether the run started with a single player, who has lives and checkpoints. Several
    /// players have wrecks and repairs instead.
    pub solo: bool,
    /// The furthest sector that a player has entered. The start bay is sector 0.
    pub sector: u32,
    /// The score: the furthest sector cleared.
    pub cleared: u32,
    /// The last sector cleared, or 0 for the start bay: a single player whose tank is
    /// destroyed comes back in front of its barrier.
    pub checkpoint: u32,
    pub lives: u32,
    pub kills: u32,
    /// Simulation ticks played.
    pub ticks: u64,
    pub phase: RunPhase,
    /// What destroyed a player's tank the last time, if the simulation knows whose doing it
    /// was. When the run is over, this is what ended it.
    pub lost_to: Option<Blow>,
    /// Whether the run is the one of a playtest, which sets no record.
    pub playtest: bool,
    /// Whether the run earns achievements: it is the first on a seed that the game drew for
    /// it, in the standard tank, and no playtest. A seed that a player chose or plays again
    /// can be practiced, and a tank can be built for a seed.
    pub earns: bool,
    /// Whether the run is in free mode, in which the players build their own tanks. Its
    /// record is one of free mode.
    pub free: bool,
    /// Whether the run is in arena mode: it is fought in one room, against one group after
    /// another without end. Its sector is then the number of the group that is on, and its
    /// score the number of groups cleared. Its record is one of arena mode.
    pub arena: bool,
    /// In arena mode, between two groups: the seconds until the next one is announced, in
    /// whole seconds.
    pub lull: Option<u8>,
}

/// Present when the game was started as a playtest, which lets the player cheat. The runs of
/// a playtest set no record. Whoever starts the game inserts it and sets its options, and the
/// server follows them.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Playtest {
    /// A destroyed player comes back without losing a life.
    pub infinite_lives: bool,
}

/// What the client asks of the server.
#[derive(Message, Clone, Debug, Serialize, Deserialize)]
pub enum RunRequest {
    Start {
        seed: String,
        /// How many players the run has, from 1 to 4.
        players: u32,
        /// Whether the game has just drawn that seed for this run, which then earns
        /// achievements unless it is a playtest.
        drawn: bool,
        difficulty: Difficulty,
        /// Whether the run is in free mode, in which the players build their own tanks.
        free: bool,
        /// Whether the run is in arena mode, in one room and without a corridor.
        arena: bool,
        /// The tank of each seat, from the first one. A seat that has none here has the
        /// standard tank, and so has one whose tank no player may have built.
        tanks: Vec<Genome>,
    },
    Abandon,
    /// A player leaves a run of several players, and their tank goes with them.
    Leave {
        seat: u8,
    },
    /// In a playtest: puts the player in front of a sector, as if every sector before it was
    /// cleared.
    SkipTo {
        sector: u32,
    },
}

/// Things that happened in the simulation, for effects, sounds and the interface.
#[derive(Message, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum SimEvent {
    RunStarted,
    RunOver,
    Shot {
        at: Vec2,
        team: Team,
    },
    Ricochet {
        at: Vec2,
    },
    BulletDestroyed {
        at: Vec2,
        team: Team,
    },
    MinePlaced {
        at: Vec2,
    },
    MineTriggered {
        at: Vec2,
    },
    Blast {
        at: Vec2,
    },
    BlockDestroyed {
        at: Vec2,
    },
    ShieldRaised {
        at: Vec2,
    },
    ShieldBlocked {
        at: Vec2,
    },
    ShieldDropped {
        at: Vec2,
    },
    TankDestroyed {
        at: Vec2,
        team: Team,
    },
    TankRepaired {
        at: Vec2,
    },
    PlayerRespawned {
        at: Vec2,
    },
    SectorCleared {
        sector: u32,
    },
    /// An arena that was cleared left a life on its floor.
    LifeDropped {
        at: Vec2,
    },
    /// A single player took a life from the floor.
    LifeTaken {
        at: Vec2,
    },
    BarrierUnlocked {
        sector: u32,
    },
    SectorEntered {
        sector: u32,
    },
    /// A wave of an arena is on its way: which one, of how many. A group of arena mode is
    /// told as the one wave of one: the tanks that come later, each for one that is destroyed,
    /// are told by their marks alone.
    WaveIncoming {
        wave: u32,
        of: u32,
    },
    /// The place where a tank of an arena is about to appear is marked.
    TankIncoming {
        at: Vec2,
    },
    /// A tank of an arena appeared where its place was marked.
    TankAppeared {
        at: Vec2,
    },
    /// A splitting bullet became two.
    BulletSplit {
        at: Vec2,
    },
}
