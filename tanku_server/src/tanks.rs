//! How tanks move, aim, shoot, place mines and shield.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;
use tanku_core::{
    geom,
    rules::{
        FIRE_COOLDOWN, MINE_ARMING_TIME, MINE_COOLDOWN, MUZZLE_DISTANCE, PIVOT_ANGLE,
        PLAYER_TURRET_RATE, SHIELD_DURATION, TANK_ACCELERATION_TIME, TANK_RADIUS, TICK,
    },
    sim::{
        Bullet, Enemy, Gun, Mine, MineState, Motion, Player, Position, PreviousPosition, Shield,
        ShieldState, SimEvent, Tank, TankCommand, Team,
    },
};

use crate::{
    Casualties, Maker,
    bullets::{self, Flight},
    field::{Field, InSector},
};

/// Runs the shields: the bubble, then the overload, then the cooldown.
pub fn shields(
    mut tanks: Query<(&Tank, &Position, &TankCommand, &mut Shield)>,
    mut events: MessageWriter<SimEvent>,
) {
    for (tank, position, command, mut shield) in &mut tanks {
        let Some((stun, cooldown)) = tank.genome.shield() else {
            continue;
        };
        match shield.state {
            ShieldState::Ready => {
                shield.cooldown = (shield.cooldown - TICK).max(0.0);
                if command.shield && shield.cooldown == 0.0 {
                    shield.state = ShieldState::Active(SHIELD_DURATION);
                    events.write(SimEvent::ShieldRaised { at: position.0 });
                }
            }
            ShieldState::Active(left) if left > TICK => {
                shield.state = ShieldState::Active(left - TICK);
            }
            ShieldState::Active(_) => {
                shield.state = ShieldState::Overloaded(stun);
                shield.cooldown = cooldown;
                events.write(SimEvent::ShieldOverloaded { at: position.0 });
            }
            ShieldState::Overloaded(left) => {
                shield.cooldown = (shield.cooldown - TICK).max(0.0);
                shield.state = if left > TICK {
                    ShieldState::Overloaded(left - TICK)
                } else {
                    ShieldState::Ready
                };
            }
        }
    }
}

/// Moves the hulls and turns the turrets.
pub fn drive(
    field: Res<Field>,
    mut tanks: Query<(
        &Tank,
        &mut Position,
        &mut PreviousPosition,
        &mut Motion,
        &TankCommand,
        &Shield,
        Has<Player>,
    )>,
) {
    for (tank, mut position, mut previous, mut motion, command, shield, is_player) in &mut tanks {
        previous.0 = position.0;
        let top_speed = tank.genome.top_speed();
        let drive = if shield.is_overloaded() {
            Vec2::ZERO
        } else {
            command.drive.clamp_length_max(1.0)
        };

        // A tank drives along its hull only, forward or in reverse, whichever turns less.
        let mut wanted = Vec2::ZERO;
        if top_speed > 0.0 && drive.length_squared() > 0.01 {
            let heading = drive.to_angle();
            let reverse = geom::angle_difference(motion.hull, heading).abs() > FRAC_PI_2;
            let target = if reverse { heading + PI } else { heading };
            motion.hull =
                geom::rotate_toward(motion.hull, target, tank.genome.hull_turn_rate() * TICK);
            if geom::angle_difference(motion.hull, target).abs() <= PIVOT_ANGLE {
                let direction = if reverse { -1.0 } else { 1.0 };
                wanted = Vec2::from_angle(motion.hull) * direction * top_speed * drive.length();
            }
        }
        let acceleration = top_speed / TANK_ACCELERATION_TIME * TICK;
        let change = wanted - motion.velocity;
        motion.velocity += change.clamp_length_max(acceleration);
        position.0 += motion.velocity * TICK;
        let mut velocity = motion.velocity;
        field.push_out(tank, &mut position.0, &mut velocity);
        motion.velocity = velocity;

        if !shield.is_overloaded() && command.aim.distance_squared(position.0) > 0.01 {
            let rate = if is_player {
                PLAYER_TURRET_RATE
            } else {
                tank.genome.turret_turn_rate()
            };
            let target = (command.aim - position.0).to_angle();
            motion.turret = geom::rotate_toward(motion.turret, target, rate * TICK);
        }
    }
}

/// Tanks are solid to each other. A tank without an engine doesn't give way.
pub fn separate(field: Res<Field>, mut tanks: Query<(&Tank, &mut Position, &mut Motion)>) {
    let mut pairs = tanks.iter_combinations_mut();
    while let Some([(tank_a, mut a, _), (tank_b, mut b, _)]) = pairs.fetch_next() {
        let offset = b.0 - a.0;
        let distance = offset.length();
        if distance >= 2.0 * TANK_RADIUS || distance < 1e-5 {
            continue;
        }
        let push = offset / distance * (2.0 * TANK_RADIUS - distance);
        let (a_moves, b_moves) = (
            tank_a.genome.top_speed() > 0.0,
            tank_b.genome.top_speed() > 0.0,
        );
        match (a_moves, b_moves) {
            (true, false) => a.0 -= push,
            (false, true) => b.0 += push,
            _ => {
                a.0 -= push / 2.0;
                b.0 += push / 2.0;
            }
        }
    }
    for (tank, mut position, mut motion) in &mut tanks {
        let mut velocity = motion.velocity;
        field.push_out(tank, &mut position.0, &mut velocity);
        motion.velocity = velocity;
    }
}

/// Reloads the guns, fires them and places mines.
pub fn operate(
    mut commands: Commands,
    field: Res<Field>,
    mut casualties: ResMut<Casualties>,
    mut tanks: Query<(
        Entity,
        &Tank,
        &Position,
        &Motion,
        &TankCommand,
        &Shield,
        &mut Gun,
    )>,
    mines: Query<&Mine>,
    mut events: MessageWriter<SimEvent>,
) {
    for (entity, tank, position, motion, command, shield, mut gun) in &mut tanks {
        let genome = &tank.genome;
        gun.cooldown = (gun.cooldown - TICK).max(0.0);
        gun.mine_cooldown = (gun.mine_cooldown - TICK).max(0.0);
        if gun.rounds < genome.magazine() {
            gun.reload += TICK;
            if gun.reload >= genome.reload_time() {
                gun.reload = 0.0;
                gun.rounds += 1;
            }
        }
        if shield.is_overloaded() {
            continue;
        }

        if command.fire && gun.rounds > 0 && gun.cooldown == 0.0 {
            gun.rounds -= 1;
            gun.cooldown = FIRE_COOLDOWN;
            let direction = Vec2::from_angle(motion.turret);
            let mut bullet = Bullet {
                owner: entity,
                team: tank.team,
                sector: tank.sector,
                velocity: direction * genome.bullet_speed(),
                bounces_left: genome.bounces(),
                pierces_left: genome.piercing(),
                bounced: false,
            };
            events.write(SimEvent::Shot {
                at: position.0 + direction * MUZZLE_DISTANCE,
                team: tank.team,
            });
            // The bullet starts in the turret and travels down the barrel, so that a tank
            // against a wall can't shoot through it.
            let mut at = position.0;
            let flight = bullets::fly_past_walls(&field, &mut at, &mut bullet, MUZZLE_DISTANCE);
            for bounce in &flight.bounces {
                events.write(SimEvent::Ricochet { at: *bounce });
            }
            match flight.end {
                Flight::Flying => {
                    commands.spawn((
                        bullet,
                        Maker(*genome),
                        Position(at),
                        PreviousPosition(at),
                        InSector(tank.sector),
                    ));
                }
                // A shot that ends in the barrel hits what stopped it all the same.
                Flight::Stopped(solid) => {
                    bullets::strike(&mut casualties, solid, Some((*genome, tank.team)));
                    events.write(SimEvent::BulletDestroyed { at });
                }
            }
        }

        let armed = mines.iter().filter(|mine| mine.owner == entity).count();
        if command.mine && gun.mine_cooldown == 0.0 && armed < genome.mines() as usize {
            gun.mine_cooldown = MINE_COOLDOWN;
            commands.spawn((
                Mine {
                    owner: entity,
                    team: tank.team,
                    sector: tank.sector,
                    state: MineState::Arming(MINE_ARMING_TIME),
                },
                Maker(*genome),
                Position(position.0),
                InSector(tank.sector),
            ));
            events.write(SimEvent::MinePlaced { at: position.0 });
        }
    }
}

/// Dormant enemies and tanks without anyone at the controls do nothing.
pub fn idle(mut tanks: Query<(&mut TankCommand, &Enemy)>) {
    for (mut command, enemy) in &mut tanks {
        if !enemy.awake {
            *command = TankCommand::default();
        }
    }
}

/// Whether a bullet destroys a tank it touches.
pub fn hits(bullet: &Bullet, target: Entity, team: Team) -> bool {
    match bullet.team {
        // A player's bullet destroys any tank, its owner included once it has bounced.
        Team::Players => target != bullet.owner || bullet.bounced,
        // An enemy's bullet destroys only players and passes through enemies.
        Team::Enemies => team == Team::Players,
    }
}
