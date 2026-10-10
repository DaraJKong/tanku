//! The loaded part of the corridor and everything solid in it.

use std::collections::BTreeSet;

use bevy::prelude::*;
use tanku_core::{
    enemy,
    geom::{self, Hit},
    map::{Corridor, SectorKind, SectorPlan},
    progression,
    rng::{Purpose, Stream},
    rules::{BLOCK_SIZE, TANK_RADIUS},
    sim::{
        Barrier, Block, Enemy, ExplosiveCrate, Flag, Gun, Motion, Position, PreviousPosition,
        Sector, SectorIndex, Tank, Team,
    },
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

#[derive(Resource)]
pub struct Field {
    pub seed: u64,
    /// The seats of the players in the run. Sectors are loaded for as many: enemy groups grow
    /// with their number.
    pub seats: Vec<u8>,
    /// Whether the run started with a single player, whose sectors have flags.
    pub solo: bool,
    pub corridor: Corridor,
    pub colliders: Vec<Collider>,
    /// Set when something solid appeared or disappeared.
    pub stale: bool,
    pub loaded: BTreeSet<u32>,
    /// Sectors whose enemies are all destroyed.
    pub cleared: BTreeSet<u32>,
}

impl Field {
    pub fn new(seed: u64, players: u32) -> Self {
        Self {
            seed,
            seats: (0..players as u8).collect(),
            solo: players == 1,
            corridor: Corridor::new(seed),
            colliders: Vec::new(),
            stale: true,
            loaded: BTreeSet::new(),
            cleared: BTreeSet::new(),
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
        for _ in 0..3 {
            let mut moved = false;
            for collider in &self.colliders {
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

    /// Loads a sector: its walls, its barrier, its flag if it has one, its obstacles and,
    /// unless it is cleared, its dormant enemies. `captured` tells whether its flag already is.
    /// Only a single player has flags, and the enemies are as many as the players call for.
    pub fn load(&mut self, commands: &mut Commands, index: u32, captured: bool) {
        if !self.loaded.insert(index) {
            return;
        }
        self.stale = true;
        let layout = self.corridor.layout(index);
        spawn_barrier(commands, &layout.plan);
        if self.solo && progression::is_checkpoint(index) {
            commands.spawn((
                Flag {
                    sector: index,
                    captured,
                },
                Position(layout.plan.flag_point.as_vec2()),
                InSector(index),
            ));
        }
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
            let count = progression::enemy_count(index, self.players().max(1), arena) as usize;
            let entry = (layout.plan.entry.0 + layout.plan.entry.1).as_vec2() / 2.0;
            for (number, spawn) in layout.spawns.iter().take(count).enumerate() {
                let genome = enemy::generate(self.seed, index, number as u32);
                let position = spawn.as_vec2();
                let mut rng = Stream::new(self.seed, Purpose::Brain, index, number as u32);
                commands.spawn((
                    Tank {
                        genome,
                        team: Team::Enemies,
                        sector: index,
                    },
                    Enemy { awake: false },
                    Position(position),
                    PreviousPosition(position),
                    Motion {
                        hull: rng.unit() * std::f32::consts::TAU,
                        turret: (entry - position).to_angle(),
                        ..default()
                    },
                    Gun::loaded(&genome),
                    Brain::new(rng, index),
                    InSector(index),
                ));
            }
        } else {
            self.cleared.insert(index);
        }
        commands.spawn((Sector { layout }, SectorIndex(index), InSector(index)));
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
