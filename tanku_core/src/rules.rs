//! Constants of the game rules. Lengths are in meters, times in seconds, angles in radians.
//!
//! Those that decide how hard the game is are written in `game.balance`, and have their
//! value from there.

use crate::balance::BALANCE;

/// Simulation ticks per second.
pub const TICK_RATE: f64 = 60.0;
/// Duration of one simulation tick.
pub const TICK: f32 = 1.0 / TICK_RATE as f32;

pub const TANK_RADIUS: f32 = 0.9;
/// The most that two tanks that overlap are moved apart in a tick. It is more than the two
/// fastest tanks close in on each other in a tick, so tanks stay solid to each other, and it
/// is little enough that a tank that comes back under another one pushes it aside without a
/// jump.
pub const TANK_PUSH: f32 = 0.3;
/// Time a tank needs to reach its top speed.
pub const TANK_ACCELERATION_TIME: f32 = BALANCE.tank.speed_up;
/// A tank pivots in place while its hull is further than this from the wanted direction.
pub const PIVOT_ANGLE: f32 = BALANCE.tank.pivot;
/// Turn rate of a player's turret.
pub const PLAYER_TURRET_RATE: f32 = BALANCE.tank.turret;
/// Distance from the center of a tank to the tip of its barrel.
pub const MUZZLE_DISTANCE: f32 = 1.3;

pub const BULLET_RADIUS: f32 = BALANCE.gun.bullet;
pub const FIRE_COOLDOWN: f32 = BALANCE.gun.cooldown;

pub const MINE_RADIUS: f32 = 0.35;
pub const MINE_ARMING_TIME: f32 = BALANCE.mine.arm;
/// Shortest time between two mines of the same tank.
pub const MINE_COOLDOWN: f32 = BALANCE.mine.every;
pub const MINE_FUSE: f32 = BALANCE.mine.fuse;
pub const MINE_TRIGGER_RADIUS: f32 = BALANCE.mine.trigger;
/// How long a mine warns before it blows, once a tank has set it off or when its fuse is
/// about to run out.
pub const MINE_WARNING: f32 = BALANCE.mine.warn;
pub const BLAST_RADIUS: f32 = BALANCE.blast;

/// A splitting bullet becomes two after this long.
pub const SPLIT_TIME: f32 = BALANCE.split.after;
/// The two leave this far to each side of its line of flight.
pub const SPLIT_ANGLE: f32 = BALANCE.split.angle;
/// For this long before it splits, the two lines ahead of it are solid and flash: as long as
/// a mine warns.
pub const SPLIT_WARNING: f32 = MINE_WARNING;
/// What a seeking bullet and an exploding one are about to do is marked this long before:
/// the bounce of the first, and the end of the second on what stands still.
pub const MARK_TIME: f32 = BALANCE.mark;

pub const SHIELD_DURATION: f32 = BALANCE.shield.lasts;
pub const SHIELD_RADIUS: f32 = BALANCE.shield.radius;

/// Side of a destructible block or an explosive crate.
pub const BLOCK_SIZE: f32 = 2.0;
/// A barrier unlocks when every living player is this close to it.
pub const BARRIER_GATHER_DISTANCE: f32 = 8.0;

/// A life on the floor is taken by a tank that comes this close to it.
pub const LIFE_REACH: f32 = BALANCE.arena.reach;

/// The most players a run can have.
pub const MAX_PLAYERS: u32 = 4;
/// A run starts by itself this long after everyone in its lobby has said that they are ready.
pub const COUNTDOWN: f32 = 3.0;
/// A player repairs the wreck of a teammate from this close, in this long.
pub const REPAIR_RADIUS: f32 = BALANCE.repair.within;
pub const REPAIR_TIME: f32 = BALANCE.repair.takes;
/// Time during which nothing destroys a tank that was just repaired.
pub const REPAIR_PROTECTION: f32 = BALANCE.repair.safe;

/// The enemies of an arena come in this many waves at most, each when the one before it is
/// destroyed.
pub const ARENA_WAVES: u32 = BALANCE.arena.waves;
/// Time between the entry of the first player into an arena and the warning of its first wave.
pub const ARENA_DELAY: f32 = BALANCE.arena.wait;
/// How long the place of a tank of an arena is marked before the tank appears there. It is
/// short, so that the players can't aim at the place and destroy the tank as it appears.
pub const APPEAR_TIME: f32 = BALANCE.arena.mark;
/// The mark of a tank of an arena waits while the tank of a player is this close to its place.
pub const APPEAR_ROOM: f32 = BALANCE.arena.clear;
/// And while a tank of its own group is this close to it: the two would stand in each other.
/// That only happens in arena mode, where a tank comes while others of its group are there.
pub const APPEAR_APART: f32 = BALANCE.room.apart;

/// In arena mode, the share of a group that comes with it, in percent and rounded up. The
/// room holds no more of the group at once: each of the others comes when one is destroyed.
pub const ROOM_AT_ONCE: u32 = BALANCE.room.at_once;
/// In arena mode, the time between the end of a group and the coming of the next one, and the
/// time before the first group of a run.
pub const LULL: f32 = BALANCE.room.lull;
/// In arena mode, a single player who is short of a life gets one back for every this many
/// groups that are cleared.
pub const LIFE_GROUPS: u32 = BALANCE.room.life_every;
/// In arena mode, how long the place of a block or of a crate is inked in before it stands
/// there. It waits for as long as something is in its place.
pub const RISE_TIME: f32 = BALANCE.room.rise;

/// The lives of a single player, which are also the most they can have.
pub const SOLO_LIVES: u32 = BALANCE.solo.lives;
/// Delay between the destruction of the solo player and the respawn.
pub const RESPAWN_DELAY: f32 = BALANCE.solo.respawn;
/// A player's tank can't shoot for this long after it appears.
pub const SPAWN_FIRE_DELAY: f32 = BALANCE.tank.hold_fire;

/// The furthest sector a playtest can skip to.
pub const PLAYTEST_LAST_SECTOR: u32 = 999;
