//! The loaded part of the corridor and everything solid in it.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use bevy::prelude::*;
use tanku_core::{
    enemy,
    geom::{self, Hit},
    map::{Corridor, SectorKind, SectorPlan},
    modes::Difficulty,
    progression,
    rng::{Purpose, Stream},
    rules::{BLOCK_SIZE, TANK_RADIUS},
    sim::{
        Barrier, Block, Enemy, ExplosiveCrate, Gun, Motion, Position, PreviousPosition, Sector,
        SectorIndex, Tank, Team,
    },
    stats::Genome,
};

use crate::brain::Brain;

/// What a collider belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Solid {
    Wall,
    Block(Entity),
    Crate(Entity),
    Barrier { sector: u32, unlocked: bool },
}

impl Solid {
    /// Whether a bullet bounces off it. Anything else stops the bullet.
    pub fn bounces(self) -> bool {
        matches!(self, Solid::Wall | Solid::Block(_))
    }

    /// Whether it stops a blast. Blocks and crates are destroyed instead.
    pub fn stops_blasts(self) -> bool {
        matches!(self, Solid::Wall | Solid::Barrier { .. })
    }

    /// Whether a tank is stopped by it. Only players cross barriers, and only the unlocked
    /// barrier of the sector they are in.
    pub fn stops(self, tank: &Tank) -> bool {
        match self {
            Solid::Barrier { sector, unlocked } => {
                !(unlocked && tank.team == Team::Players && tank.sector == sector)
            }
            _ => true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Collider {
    pub a: Vec2,
    pub b: Vec2,
    pub solid: Solid,
}

/// Every simulated entity belongs to a sector and is removed with it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct InSector(pub u32);

/// Where an enemy started, and which tank of its group it is. It goes back there, as it was,
/// when a single player who fell comes back to its sector.
#[derive(Component, Clone, Copy, Debug)]
pub struct Home {
    pub number: u32,
    pub at: Vec2,
    pub hull: f32,
    pub turret: f32,
}

/// An enemy of an arena that isn't there yet: which tank of its group it is, what it is made
/// of and where it will appear.
#[derive(Clone, Copy, Debug)]
pub struct Recruit {
    pub number: u32,
    pub genome: Genome,
    pub at: Vec2,
}

/// How far an arena is with its waves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fuse {
    /// No player has come in yet.
    Unlit,
    /// The players are in, and the first wave is announced after this much longer.
    Burning(f32),
    /// The waves are on.
    Out,
}

/// The enemies that an arena still holds back. An arena is empty when the players come in:
/// its enemies appear in waves, each when the one before it is destroyed, at places that are
/// marked for a moment first.
#[derive(Clone, Debug)]
pub struct Arena {
    /// The waves that have not started, the next one first.
    pub waiting: VecDeque<Vec<Recruit>>,
    /// The tanks of the wave in progress that are not there yet, each with the time until it
    /// appears. A tank whose place a player stands too close to has none: it waits for room.
    pub incoming: Vec<(Recruit, Option<f32>)>,
    pub fuse: Fuse,
    /// How many waves the arena has in all.
    pub waves: u32,
}

impl Arena {
    /// Whether enemies are still to come.
    pub fn holds_more(&self) -> bool {
        !self.waiting.is_empty() || !self.incoming.is_empty()
    }
}

#[derive(Resource)]
pub struct Field {
    pub seed: u64,
    /// How hard the enemies of the run are. The corridor is the same for every difficulty.
    pub difficulty: Difficulty,
    /// The seats of the players in the run. Sectors are loaded for as many: enemy groups grow
    /// with their number.
    pub seats: Vec<u8>,
    /// Whether the run started with a single player, who has lives.
    pub solo: bool,
    pub corridor: Corridor,
    pub colliders: Vec<Collider>,
    /// Set when something solid appeared or disappeared.
    pub stale: bool,
    pub loaded: BTreeSet<u32>,
    /// Sectors whose enemies are all destroyed.
    pub cleared: BTreeSet<u32>,
    /// What the loaded arenas still hold back, by sector.
    pub arenas: BTreeMap<u32, Arena>,
}

impl Field {
    pub fn new(seed: u64, players: u32, difficulty: Difficulty) -> Self {
        Self {
            seed,
            difficulty,
            seats: (0..players as u8).collect(),
            solo: players == 1,
            corridor: Corridor::new(seed),
            colliders: Vec::new(),
            stale: true,
            loaded: BTreeSet::new(),
            cleared: BTreeSet::new(),
            arenas: BTreeMap::new(),
        }
    }

    /// How many players the run has.
    pub fn players(&self) -> u32 {
        self.seats.len() as u32
    }

    /// First thing a circle touches when it moves from `origin` by `delta`.
    pub fn cast(&self, origin: Vec2, delta: Vec2, radius: f32) -> Option<(Hit, Solid)> {
        let reach = delta.abs() / 2.0 + Vec2::splat(radius);
        let center = origin + delta / 2.0;
        let mut first: Option<(Hit, Solid)> = None;
        for collider in &self.colliders {
            // A cheap test first: most colliders are nowhere near the path.
            let middle = (collider.a + collider.b) / 2.0;
            let extent = (collider.b - collider.a).abs() / 2.0;
            if ((middle - center).abs() - extent - reach).max_element() > 0.0 {
                continue;
            }
            let hit = geom::sweep_circle_segment(origin, delta, radius, collider.a, collider.b);
            if let Some(hit) = hit
                && first.is_none_or(|(best, _)| hit.t < best.t)
            {
                first = Some((hit, collider.solid));
            }
        }
        first
    }

    /// Whether anything that matches `stops` lies between two points.
    pub fn blocked(&self, from: Vec2, to: Vec2, stops: impl Fn(Solid) -> bool) -> bool {
        self.colliders.iter().any(|collider| {
            stops(collider.solid) && geom::segments_intersect(from, to, collider.a, collider.b)
        })
    }

    /// Whether two points see each other.
    pub fn sees(&self, from: Vec2, to: Vec2) -> bool {
        !self.blocked(from, to, |_| true)
    }

    /// Moves a tank out of everything solid it overlaps.
    pub fn push_out(&self, tank: &Tank, position: &mut Vec2, velocity: &mut Vec2) {
        push_out(&self.colliders, tank, position, velocity);
    }

    /// Loads a sector: its walls, its barrier, its obstacles and, unless it is cleared, its
    /// dormant enemies, which are as many as the players call for. An arena is loaded empty:
    /// its enemies are held back, and come in waves.
    pub fn load(&mut self, commands: &mut Commands, index: u32) {
        if !self.loaded.insert(index) {
            return;
        }
        self.stale = true;
        let layout = self.corridor.layout(index);
        spawn_barrier(commands, &layout.plan);
        let center = |corner: &IVec2| Position(corner.as_vec2() + Vec2::splat(BLOCK_SIZE / 2.0));
        for corner in &layout.blocks {
            commands.spawn((Block { sector: index }, center(corner), InSector(index)));
        }
        for corner in &layout.crates {
            commands.spawn((
                ExplosiveCrate { sector: index },
                center(corner),
                InSector(index),
            ));
        }

        if layout.plan.kind != SectorKind::Bay && !self.cleared.contains(&index) {
            let arena = layout.plan.kind == SectorKind::Arena;
            let count = progression::enemy_count(index, self.players().max(1), arena);
            // The group shares its budget among as many tanks as the players call for. A
            // sector that is too small for all of them holds as many as fit.
            let genomes = enemy::group(self.seed, self.difficulty, index, count);
            let group = layout.spawns.iter().zip(genomes).enumerate();
            let mut group = group.map(|(number, (spawn, genome))| Recruit {
                number: number as u32,
                genome,
                at: spawn.as_vec2(),
            });
            if arena {
                let sizes = progression::wave_sizes(group.len() as u32);
                let waves = sizes
                    .iter()
                    .map(|&size| group.by_ref().take(size as usize).collect());
                self.arenas.insert(
                    index,
                    Arena {
                        waiting: waves.collect(),
                        incoming: Vec::new(),
                        fuse: Fuse::Unlit,
                        waves: sizes.len() as u32,
                    },
                );
            } else {
                for recruit in group {
                    self.enlist(commands, &layout.plan, recruit, false);
                }
            }
        } else {
            self.cleared.insert(index);
        }
        commands.spawn((Sector { layout }, SectorIndex(index), InSector(index)));
    }

    /// Puts an enemy in its sector: asleep until the players come in, or awake at once when it
    /// appears in an arena that they are in.
    pub fn enlist(
        &self,
        commands: &mut Commands,
        plan: &SectorPlan,
        recruit: Recruit,
        awake: bool,
    ) {
        let Recruit { number, genome, at } = recruit;
        let entry = (plan.entry.0 + plan.entry.1).as_vec2() / 2.0;
        let (brain, hull) = self.brain(plan.index, number);
        let turret = (entry - at).to_angle();
        commands.spawn((
            Tank {
                genome,
                team: Team::Enemies,
                sector: plan.index,
            },
            Enemy { awake },
            Home {
                number,
                at,
                hull,
                turret,
            },
            Position(at),
            PreviousPosition(at),
            Motion {
                hull,
                turret,
                ..default()
            },
            Gun::loaded(&genome),
            brain,
            InSector(plan.index),
        ));
    }

    /// The brain of an enemy as it is when the enemy is put in its sector, and the way its
    /// hull is turned then.
    pub fn brain(&self, sector: u32, number: u32) -> (Brain, f32) {
        let mut rng = Stream::new(self.seed, Purpose::Brain, sector, number);
        let hull = rng.unit() * std::f32::consts::TAU;
        (Brain::new(rng, sector, self.difficulty), hull)
    }

    /// The barrier in front of a sector, for a sector that is loaded without the one before it.
    pub fn load_entry_barrier(&mut self, commands: &mut Commands, index: u32) {
        if let Some(previous) = index.checked_sub(1) {
            let plan = self.corridor.plan(previous).clone();
            spawn_barrier(commands, &plan);
            self.stale = true;
        }
    }
}

/// Moves a tank out of those of these colliders that it overlaps.
pub fn push_out(colliders: &[Collider], tank: &Tank, position: &mut Vec2, velocity: &mut Vec2) {
    for _ in 0..3 {
        let mut moved = false;
        for collider in colliders {
            if !collider.solid.stops(tank) {
                continue;
            }
            let closest = geom::closest_point_on_segment(*position, collider.a, collider.b);
            let offset = *position - closest;
            let distance = offset.length();
            if distance < TANK_RADIUS && distance > 1e-5 {
                let normal = offset / distance;
                *position += normal * (TANK_RADIUS - distance);
                *velocity -= normal * velocity.dot(normal).min(0.0);
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
}

/// A barrier lives as long as the sector behind it, so that this sector stays closed.
fn spawn_barrier(commands: &mut Commands, plan: &SectorPlan) {
    let (from, to) = (plan.gate.0.as_vec2(), plan.gate.1.as_vec2());
    commands.spawn((
        Barrier {
            sector: plan.index,
            from,
            to,
            outward: (to - from).normalize().perp(),
            unlocked: false,
        },
        InSector(plan.index + 1),
    ));
}

/// Where a player starts in a sector: a few meters behind its entry.
pub fn entry_position(plan: &SectorPlan) -> Vec2 {
    let middle = (plan.entry.0 + plan.entry.1).as_vec2() / 2.0;
    middle + plan.heading.forward().as_vec2() * 5.0
}

pub fn rebuild_colliders(
    mut field: ResMut<Field>,
    sectors: Query<&Sector>,
    blocks: Query<(Entity, &Position), With<Block>>,
    crates: Query<(Entity, &Position), With<ExplosiveCrate>>,
    barriers: Query<&Barrier>,
) {
    if !field.stale {
        return;
    }
    field.stale = false;
    let mut colliders = Vec::new();
    for sector in &sectors {
        for &(a, b) in &sector.layout.plan.walls {
            colliders.push(Collider {
                a: a.as_vec2(),
                b: b.as_vec2(),
                solid: Solid::Wall,
            });
        }
    }
    let mut square = |center: Vec2, solid: Solid| {
        let half = BLOCK_SIZE / 2.0;
        let corners = [
            center + Vec2::new(-half, -half),
            center + Vec2::new(half, -half),
            center + Vec2::new(half, half),
            center + Vec2::new(-half, half),
        ];
        for i in 0..4 {
            colliders.push(Collider {
                a: corners[i],
                b: corners[(i + 1) % 4],
                solid,
            });
        }
    };
    for (entity, position) in &blocks {
        square(position.0, Solid::Block(entity));
    }
    for (entity, position) in &crates {
        square(position.0, Solid::Crate(entity));
    }
    for barrier in &barriers {
        colliders.push(Collider {
            a: barrier.from,
            b: barrier.to,
            solid: Solid::Barrier {
                sector: barrier.sector,
                unlocked: barrier.unlocked,
            },
        });
    }
    field.colliders = colliders;
}

/// Removes everything that belongs to a sector that is no longer loaded.
pub fn unload(
    mut commands: Commands,
    mut field: ResMut<Field>,
    entities: Query<(Entity, &InSector, Option<&Barrier>)>,
) {
    let loaded = field.loaded.clone();
    field.arenas.retain(|sector, _| loaded.contains(sector));
    let mut removed = false;
    for (entity, sector, barrier) in &entities {
        // A barrier stays as long as the sector on either side of it.
        let kept = field.loaded.contains(&sector.0)
            || barrier.is_some_and(|barrier| field.loaded.contains(&barrier.sector));
        if !kept {
            commands.entity(entity).try_despawn();
            removed = true;
        }
    }
    field.stale |= removed;
}
