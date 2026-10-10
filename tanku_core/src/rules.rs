//! Constants of the game rules. Lengths are in meters, times in seconds, angles in radians.

use std::f32::consts::PI;

/// Simulation ticks per second.
pub const TICK_RATE: f64 = 60.0;
/// Duration of one simulation tick.
pub const TICK: f32 = 1.0 / TICK_RATE as f32;

pub const TANK_RADIUS: f32 = 0.9;
/// Time a tank needs to reach its top speed.
pub const TANK_ACCELERATION_TIME: f32 = 0.15;
/// A tank pivots in place while its hull is further than this from the wanted direction.
pub const PIVOT_ANGLE: f32 = PI / 4.0;
/// Turn rate of a player's turret.
pub const PLAYER_TURRET_RATE: f32 = 4.0 * PI;
/// Distance from the center of a tank to the tip of its barrel.
pub const MUZZLE_DISTANCE: f32 = 1.3;

pub const BULLET_RADIUS: f32 = 0.225;
pub const FIRE_COOLDOWN: f32 = 0.25;

pub const MINE_RADIUS: f32 = 0.35;
pub const MINE_ARMING_TIME: f32 = 1.0;
/// Shortest time between two mines of the same tank.
pub const MINE_COOLDOWN: f32 = 0.5;
pub const MINE_FUSE: f32 = 10.0;
pub const MINE_TRIGGER_RADIUS: f32 = 2.0;
/// How long a mine warns before it blows, once a tank has set it off or when its fuse is
/// about to run out.
pub const MINE_WARNING: f32 = 0.6;
pub const BLAST_RADIUS: f32 = 3.0;

pub const SHIELD_DURATION: f32 = 0.5;
pub const SHIELD_RADIUS: f32 = 1.4;

/// Side of a destructible block or an explosive crate.
pub const BLOCK_SIZE: f32 = 2.0;
/// A barrier unlocks when every living player is this close to it.
pub const BARRIER_GATHER_DISTANCE: f32 = 8.0;

/// A flag is captured by a player who comes this close to it.
pub const FLAG_CAPTURE_RADIUS: f32 = 2.0;

pub const SOLO_LIVES: u32 = 3;
/// Delay between the destruction of the solo player and the respawn.
pub const RESPAWN_DELAY: f32 = 1.5;
/// A player's tank can't shoot for this long after it appears.
pub const SPAWN_FIRE_DELAY: f32 = 0.5;

/// The furthest sector a playtest can skip to.
pub const PLAYTEST_LAST_SECTOR: u32 = 999;
