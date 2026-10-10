//! The state of the simulation, which the server owns and the client reads.

use bevy::prelude::*;

use crate::{map::SectorLayout, stats::Genome};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Team {
    Players,
    Enemies,
}

/// Position on the ground plane: `x` is east and `y` is north.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Position(pub Vec2);

/// Position one tick ago, for smooth rendering between ticks.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct PreviousPosition(pub Vec2);

#[derive(Component, Clone, Debug)]
#[require(Position, PreviousPosition, Motion, TankCommand, Shield)]
pub struct Tank {
    pub genome: Genome,
    pub team: Team,
    /// The sector the tank is in.
    pub sector: u32,
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Motion {
    pub velocity: Vec2,
    /// Angle of the hull, counterclockwise from east.
    pub hull: f32,
    /// Angle of the turret, counterclockwise from east.
    pub turret: f32,
}

/// What a player or a brain asks of its tank this tick.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct TankCommand {
    /// Direction to drive in, no longer than 1.
    pub drive: Vec2,
    /// The point to aim at.
    pub aim: Vec2,
    pub fire: bool,
    pub mine: bool,
    pub shield: bool,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Gun {
    pub rounds: u8,
    /// Time spent reloading the next round.
    pub reload: f32,
    /// Time before the next shot.
    pub cooldown: f32,
    /// Time before the next mine.
    pub mine_cooldown: f32,
}

impl Gun {
    pub fn loaded(genome: &Genome) -> Self {
        Self {
            rounds: genome.magazine(),
            reload: 0.0,
            cooldown: 0.0,
            mine_cooldown: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ShieldState {
    #[default]
    Ready,
    /// The bubble is up for this much longer.
    Active(f32),
    /// The tank is stunned for this much longer.
    Overloaded(f32),
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Shield {
    pub state: ShieldState,
    /// Time before the shield can be raised again.
    pub cooldown: f32,
}

impl Shield {
    pub fn is_active(&self) -> bool {
        matches!(self.state, ShieldState::Active(_))
    }

    pub fn is_overloaded(&self) -> bool {
        matches!(self.state, ShieldState::Overloaded(_))
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct Player;

#[derive(Component, Clone, Copy, Debug)]
pub struct Enemy {
    /// Enemies are dormant until a player enters their sector.
    pub awake: bool,
}

#[derive(Component, Clone, Copy, Debug)]
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MineState {
    /// Harmless to drive over for this much longer.
    Arming(f32),
    /// Waiting, with this much fuse left.
    Armed(f32),
    /// An opposing tank came close: the mine blows after this much longer.
    Triggered(f32),
}

#[derive(Component, Clone, Copy, Debug)]
#[require(Position)]
pub struct Mine {
    pub owner: Entity,
    pub team: Team,
    pub sector: u32,
    pub state: MineState,
}

/// A destructible block. Its position is its center.
#[derive(Component, Clone, Copy, Debug)]
#[require(Position)]
pub struct Block {
    pub sector: u32,
}

/// An explosive crate. Its position is its center.
#[derive(Component, Clone, Copy, Debug)]
#[require(Position)]
pub struct ExplosiveCrate {
    pub sector: u32,
}

/// A loaded sector: its walls and its floor.
#[derive(Component, Clone, Debug)]
pub struct Sector {
    pub layout: SectorLayout,
}

/// The energy wall at the end of a sector.
#[derive(Component, Clone, Copy, Debug)]
pub struct Barrier {
    pub sector: u32,
    pub from: Vec2,
    pub to: Vec2,
    /// Unit vector that points into the next sector.
    pub outward: Vec2,
    pub unlocked: bool,
}

/// The flag of a sector, near its barrier. A solo player who captures it comes back there.
#[derive(Component, Clone, Copy, Debug)]
#[require(Position)]
pub struct Flag {
    pub sector: u32,
    pub captured: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RunPhase {
    Playing,
    /// The player was destroyed and comes back after this much longer.
    Respawning(f32),
    Over,
}

/// The run in progress.
#[derive(Resource, Clone, Debug)]
pub struct Run {
    pub seed_text: String,
    pub seed: u64,
    /// The sector the player is in. The start bay is sector 0.
    pub sector: u32,
    /// The score: the furthest sector cleared.
    pub cleared: u32,
    /// The sector of the last flag captured, or 0 for the start bay.
    pub checkpoint: u32,
    pub lives: u32,
    pub kills: u32,
    /// Simulation ticks played.
    pub ticks: u64,
    pub phase: RunPhase,
}

/// What the client asks of the server.
#[derive(Message, Clone, Debug)]
pub enum RunRequest {
    Start { seed: String },
    Abandon,
}

/// Things that happened in the simulation, for effects, sounds and the interface.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub enum SimEvent {
    RunStarted,
    RunOver,
    Shot { at: Vec2, team: Team },
    Ricochet { at: Vec2 },
    BulletDestroyed { at: Vec2 },
    MinePlaced { at: Vec2 },
    MineTriggered { at: Vec2 },
    Blast { at: Vec2 },
    BlockDestroyed { at: Vec2 },
    ShieldRaised { at: Vec2 },
    ShieldBlocked { at: Vec2 },
    ShieldOverloaded { at: Vec2 },
    TankDestroyed { at: Vec2, team: Team },
    PlayerRespawned { at: Vec2 },
    SectorCleared { sector: u32 },
    FlagCaptured { at: Vec2 },
    BarrierUnlocked { sector: u32 },
    SectorEntered { sector: u32 },
}
