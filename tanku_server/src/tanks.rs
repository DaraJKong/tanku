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
        ShieldState, SimEvent, Tank, TankCommand, Team, Wreck,
    },
};

use crate::{
    Casualties, Maker,
    bullets::{self, Flight},
    field::{Field, InSector},
    repair::Repairers,
};

/// Runs the shields: the bubble, then the cooldown.
pub fn shields(
    repairers: Res<Repairers>,
    mut tanks: Query<(Entity, &Tank, &Position, &TankCommand, &mut Shield), Without<Wreck>>,
    mut events: MessageWriter<SimEvent>,
) {
    for (entity, tank, position, command, mut shield) in &mut tanks {
        let Some(cooldown) = tank.genome.shield_cooldown() else {
            continue;
        };
        match shield.state {
            ShieldState::Ready => {
                // What doesn't change is left alone: guests are told of what was touched.
                if shield.cooldown > 0.0 {
                    shield.cooldown = (shield.cooldown - TICK).max(0.0);
                }
                // A tank that repairs a wreck has its hands full.
                let free = !repairers.0.contains(&entity);
                if command.shield && free && shield.cooldown == 0.0 {
                    shield.state = ShieldState::Active(SHIELD_DURATION);
                    events.write(SimEvent::ShieldRaised { at: position.0 });
                }
            }
            ShieldState::Active(left) if left > TICK => {
                shield.state = ShieldState::Active(left - TICK);
            }
            // The cooldown is all that a shield costs its tank.
            ShieldState::Active(_) => {
                shield.state = ShieldState::Ready;
                shield.cooldown = cooldown;
                events.write(SimEvent::ShieldDropped { at: position.0 });
            }
        }
    }
}

/// One tick of a tank: its hull turns and drives as it is asked, and its turret turns to where
/// it is asked to aim. The host does this for every tank, and a guest for the tank of its own
/// player, ahead of what the host tells.
pub fn step(
    field: &Field,
    tank: &Tank,
    is_player: bool,
    command: &TankCommand,
    position: &mut Vec2,
    motion: &mut Motion,
) {
    let top_speed = tank.genome.top_speed();
    let drive = command.drive.clamp_length_max(1.0);

    // A tank drives along its hull only, forward or in reverse, whichever turns less.
    let mut wanted = Vec2::ZERO;
    if top_speed > 0.0 && drive.length_squared() > 0.01 {
        let heading = drive.to_angle();
        let reverse = geom::angle_difference(motion.hull, heading).abs() > FRAC_PI_2;
        let target = if reverse { heading + PI } else { heading };
        motion.hull = geom::rotate_toward(motion.hull, target, tank.genome.hull_turn_rate() * TICK);
        if geom::angle_difference(motion.hull, target).abs() <= PIVOT_ANGLE {
            let direction = if reverse { -1.0 } else { 1.0 };
            wanted = Vec2::from_angle(motion.hull) * direction * top_speed * drive.length();
        }
    }
    let acceleration = top_speed / TANK_ACCELERATION_TIME * TICK;
    let change = wanted - motion.velocity;
    motion.velocity += change.clamp_length_max(acceleration);
    *position += motion.velocity * TICK;
    field.push_out(tank, position, &mut motion.velocity);

    if command.aim.distance_squared(*position) > 0.01 {
        let rate = if is_player {
            PLAYER_TURRET_RATE
        } else {
            tank.genome.turret_turn_rate()
        };
        let target = (command.aim - *position).to_angle();
        motion.turret = geom::rotate_toward(motion.turret, target, rate * TICK);
    }
}

/// Moves the hulls and turns the turrets. A wreck stays as it is.
pub fn drive(
    field: Res<Field>,
    mut tanks: Query<
        (
            &Tank,
            &mut Position,
            &mut PreviousPosition,
            &mut Motion,
            &TankCommand,
            Has<Player>,
        ),
        Without<Wreck>,
    >,
) {
    for (tank, mut position, mut previous, mut motion, command, is_player) in &mut tanks {
        previous.0 = position.0;
        step(
            &field,
            tank,
            is_player,
            command,
            &mut position.0,
            &mut motion,
        );
    }
}

/// How far the second of two tanks must go from the first for them not to overlap.
fn overlap(a: Vec2, b: Vec2) -> Option<Vec2> {
    let offset = b - a;
    let distance = offset.length();
    let overlaps = (1e-5..2.0 * TANK_RADIUS).contains(&distance);
    overlaps.then(|| offset / distance * (2.0 * TANK_RADIUS - distance))
}

/// Moves a tank out of the others, which stay where they are. Each is given with whether it
/// gives way: the tank then only goes half of the way out, as it does when the host moves
/// both. A guest does this for the tank of its own player.
pub fn give_way(position: &mut Vec2, others: impl Iterator<Item = (Vec2, bool)>) {
    for (other, moves) in others {
        if let Some(push) = overlap(*position, other) {
            *position -= if moves { push / 2.0 } else { push };
        }
    }
}

/// Tanks are solid to each other. A tank without an engine doesn't give way, and neither does
/// a wreck.
pub fn separate(
    field: Res<Field>,
    mut tanks: Query<(&Tank, &mut Position, &mut Motion, Has<Wreck>)>,
) {
    let mut pairs = tanks.iter_combinations_mut();
    while let Some([(tank_a, mut a, _, wreck_a), (tank_b, mut b, _, wreck_b)]) = pairs.fetch_next()
    {
        let Some(push) = overlap(a.0, b.0) else {
            continue;
        };
        let (a_moves, b_moves) = (
            tank_a.genome.top_speed() > 0.0 && !wreck_a,
            tank_b.genome.top_speed() > 0.0 && !wreck_b,
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
    for (tank, mut position, mut motion, _) in &mut tanks {
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
    repairers: Res<Repairers>,
    mut tanks: Query<(Entity, &Tank, &Position, &Motion, &TankCommand, &mut Gun), Without<Wreck>>,
    mines: Query<&Mine>,
    mut events: MessageWriter<SimEvent>,
) {
    for (entity, tank, position, motion, command, mut gun) in &mut tanks {
        let genome = &tank.genome;
        // What doesn't change is left alone: guests are told of what was touched.
        if gun.cooldown > 0.0 {
            gun.cooldown = (gun.cooldown - TICK).max(0.0);
        }
        if gun.mine_cooldown > 0.0 {
            gun.mine_cooldown = (gun.mine_cooldown - TICK).max(0.0);
        }
        let laid = mines.iter().filter(|mine| mine.owner == entity).count() as u8;
        if gun.mines != laid {
            gun.mines = laid;
        }
        if gun.rounds < genome.magazine() {
            gun.reload += TICK;
            if gun.reload >= genome.reload_time() {
                gun.reload = 0.0;
                gun.rounds += 1;
            }
        }
        // A tank that repairs a wreck has its hands full.
        if repairers.0.contains(&entity) {
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
                    let team = tank.team;
                    bullets::strike(&mut casualties, solid, Some((*genome, team)));
                    events.write(SimEvent::BulletDestroyed { at, team });
                }
            }
        }

        if command.mine && gun.mine_cooldown == 0.0 && gun.mines < genome.mines() {
            gun.mine_cooldown = MINE_COOLDOWN;
            gun.mines += 1;
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

/// Dormant enemies do nothing.
pub fn idle(mut tanks: Query<(&mut TankCommand, &Enemy, &Position, &Motion)>) {
    for (mut command, enemy, position, motion) in &mut tanks {
        if !enemy.awake {
            // Aiming where the turret already points keeps it still.
            *command = TankCommand {
                aim: position.0 + Vec2::from_angle(motion.turret),
                ..default()
            };
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
