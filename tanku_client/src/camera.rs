//! The camera: it looks down at the player's tank and turns in quarter turns.
//!
//! Once a sector is quiet, the view runs ahead of a tank that comes close to its barrier, so
//! that the players can study the next group before they cross.

use std::f32::consts::FRAC_PI_2;

use bevy::{
    core_pipeline::tonemapping::Tonemapping, ecs::system::SystemParam, prelude::*,
    window::PrimaryWindow,
};
use tanku_core::{
    geom,
    sim::{Barrier, Bullet, Enemy, Player, Run, Tank, Team, Wreck},
};

use crate::{crew::Own, scene, settings::Settings, theme};

/// Angle between the ground and the line of sight.
const PITCH: f32 = 65.0;
/// A narrow field of view, to keep the perspective mild.
const FIELD_OF_VIEW: f32 = 20.0;
/// Meters of ground visible from the bottom to the top of the screen.
const VISIBLE_GROUND: f32 = 30.0;
/// How far the view shifts toward the cursor, as a share of the distance and in meters at most.
const LOOK_AHEAD: (f32, f32) = (0.03125, 0.75);
/// How far past the barrier of a quiet sector the view runs ahead of a tank that waits there,
/// in meters. The tank stays on the screen, near the edge it has its back to.
const PAST_BARRIER: f32 = 10.0;
/// How far from that barrier the tank is when the view starts to run ahead, and when it is
/// all the way there.
const APPROACH: (f32, f32) = (13.0, 5.0);
/// How fast the view runs ahead, and how fast it comes back to the tank: the shares of the way
/// that are left after a second are `e` to the power of minus these.
const PAST_RATES: (f32, f32) = (1.0, 0.8);

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource, Default)]
pub struct View {
    /// Quarter turns of the view. North is at the top of the screen at 0 and on the right at 1.
    pub turns: i32,
    /// The point on the ground under the cursor.
    pub aim: Vec2,
    /// Meters of ground from the bottom to the top of the screen, when a script zooms.
    pub span: Option<f32>,
    yaw: f32,
    focus: Vec2,
    /// How far the view has run ahead of the tank, past the barrier of its sector.
    past: Vec2,
    /// Time during which the view stays where it was told to look.
    hold: f32,
    shake: f32,
    noise: u32,
}

impl View {
    /// The direction on the ground of a direction on the screen.
    pub fn to_ground(&self, screen: Vec2) -> Vec2 {
        Vec2::from_angle(self.turns as f32 * FRAC_PI_2).rotate(screen)
    }

    /// Shakes the camera, by an amount from 0 to 1.
    pub fn shake(&mut self, amount: f32) {
        self.shake = (self.shake + amount).min(1.0);
    }

    /// The point on the ground the camera looks at.
    pub fn focus(&self) -> Vec2 {
        self.focus
    }

    /// Shows a place, from closer or from further than the game does if a span is given: the
    /// meters of ground from the bottom to the top of the screen. The view stays there, whatever
    /// the tanks do, until it is given back to the game.
    pub fn show(&mut self, point: Vec2, span: Option<f32>) {
        self.focus = point;
        self.span = span;
        self.hold = f32::INFINITY;
    }

    /// Gives the view back to the game: it follows the player's tank, from the distance of
    /// the game.
    pub fn release(&mut self) {
        self.span = None;
        self.hold = 0.0;
    }

    /// How far the view is turned right now, in radians: it turns smoothly from one quarter
    /// to the next.
    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// Looks at a point for a while instead of following the player.
    #[cfg(feature = "script")]
    pub fn look_at(&mut self, point: Vec2, seconds: f32) {
        self.focus = point;
        self.hold = seconds;
    }

    fn jitter(&mut self) -> f32 {
        self.noise = self
            .noise
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.noise >> 8) as f32 / (1 << 23) as f32 - 1.0
    }
}

/// Where the world is on the screen, for what the interface draws over the game: the tags
/// of the barriers and the ruler.
///
/// It reads the camera where [`follow`] has just put it. The engine only works that out once
/// the systems of a frame have run, which is a frame late for a mark that must stay on what it
/// marks while the view moves. A system that uses it runs after [`follow`].
#[derive(SystemParam)]
pub struct OnScreen<'w, 's> {
    camera: Single<'w, 's, (&'static Camera, &'static Transform), With<MainCamera>>,
    scale: Res<'w, UiScale>,
}

impl OnScreen<'_, '_> {
    /// The place of a point of the world, in the pixels of the interface.
    pub fn point(&self, world: Vec3) -> Option<Vec2> {
        let (camera, transform) = *self.camera;
        let seen_from = GlobalTransform::from(*transform);
        let at = camera.world_to_viewport(&seen_from, world).ok()?;
        Some(at / self.scale.0)
    }

    /// The size of the screen, in the pixels of the interface.
    pub fn size(&self) -> Option<Vec2> {
        Some(self.camera.0.logical_viewport_size()? / self.scale.0)
    }

    /// The point of the ground under a point of the screen.
    pub fn ground(&self, screen: Vec2) -> Option<Vec2> {
        let (camera, transform) = *self.camera;
        let seen_from = GlobalTransform::from(*transform);
        let ray = camera
            .viewport_to_world(&seen_from, screen * self.scale.0)
            .ok()?;
        (ray.direction.y < -1e-4).then(|| {
            let reach = -ray.origin.y / ray.direction.y;
            scene::to_ground(ray.origin + *ray.direction * reach)
        })
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<View>()
        .insert_resource(ClearColor(theme::VOID))
        .add_systems(Startup, setup)
        .add_systems(Update, follow.after(scene::place));
}

fn setup(mut commands: Commands) {
    // Nothing is lit, and no tone mapping comes between the colors of the theme and the screen.
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        Tonemapping::None,
        Projection::Perspective(PerspectiveProjection {
            fov: FIELD_OF_VIEW.to_radians(),
            ..default()
        }),
        Transform::from_xyz(0.0, 70.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Finds the point on the ground under the cursor.
pub fn aim(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut view: ResMut<View>,
) {
    let (camera, transform) = *camera;
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(transform, cursor) else {
        return;
    };
    if ray.direction.y < -1e-4 {
        let ground = ray.origin + *ray.direction * (-ray.origin.y / ray.direction.y);
        view.aim = scene::to_ground(ground);
    }
}

/// How far past a barrier the view runs ahead of a tank that is this far from it: not at
/// all from far away, then more and more as the tank comes close.
fn run_ahead(distance: f32) -> f32 {
    let (far, near) = APPROACH;
    let closeness = ((far - distance) / (far - near)).clamp(0.0, 1.0);
    PAST_BARRIER * closeness * closeness * (3.0 - 2.0 * closeness)
}

/// Where the camera stands to look at a point of the ground, with this many meters of
/// ground from the bottom to the top of the screen.
fn eye(focus: Vec2, yaw: f32, span: f32) -> Transform {
    let pitch = PITCH.to_radians();
    let distance = span * pitch.sin() / (2.0 * (FIELD_OF_VIEW.to_radians() / 2.0).tan());
    let offset = Quat::from_rotation_y(yaw) * Vec3::new(0.0, pitch.sin(), pitch.cos()) * distance;
    let focus = scene::to_world(focus, 0.0);
    Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y)
}

pub fn follow(
    time: Res<Time>,
    settings: Res<Settings>,
    run: Option<Res<Run>>,
    mut view: ResMut<View>,
    players: Query<(&Transform, &Player, Has<Own>, Has<Wreck>), Without<MainCamera>>,
    barriers: Query<&Barrier>,
    enemies: Query<&Tank, With<Enemy>>,
    bullets: Query<&Bullet>,
    mut camera: Single<&mut Transform, With<MainCamera>>,
) {
    let seconds = time.delta_secs();
    view.hold = (view.hold - seconds).max(0.0);
    // The view follows the tank of the player at this screen. While that tank is a wreck, it
    // follows a teammate whose tank still runs.
    let own = players.iter().find(|(_, _, own, _)| *own);
    let running = players.iter().filter(|(.., wreck)| !wreck);
    let watched = match own {
        Some((transform, _, _, false)) => Some(transform),
        _ => {
            let teammate = running.min_by_key(|(_, player, ..)| player.seat);
            teammate.or(own).map(|(transform, ..)| transform)
        }
    };
    if let Some(player) = watched
        && view.hold == 0.0
    {
        let at = scene::to_ground(player.translation);
        let ahead = ((view.aim - at) * LOOK_AHEAD.0).clamp_length_max(LOOK_AHEAD.1);
        // The way out of the sector the tank is in, once the sector is quiet: no enemy is
        // left there, and none of their bullets still flies.
        let sector = run.as_ref().map(|run| run.sector);
        let quiet = sector.filter(|sector| {
            let mut shots = bullets.iter().filter(|bullet| bullet.team == Team::Enemies);
            enemies.iter().all(|tank| tank.sector != *sector)
                && shots.all(|bullet| bullet.sector != *sector)
        });
        let way_out = quiet.and_then(|sector| {
            let mut barriers = barriers.iter();
            barriers.find(|barrier| barrier.sector == sector)
        });
        let past = way_out.map_or(Vec2::ZERO, |barrier| {
            let distance = geom::distance_to_segment(at, barrier.from, barrier.to);
            barrier.outward * run_ahead(distance)
        });
        let rate = if past.length() > view.past.length() {
            PAST_RATES.0
        } else {
            PAST_RATES.1
        };
        view.past = view.past.lerp(past, 1.0 - (-rate * seconds).exp());
        let target = at + ahead + view.past;
        // A jump means a new run or a respawn: go there at once.
        view.focus = if view.focus.distance(target) > 40.0 {
            target
        } else {
            view.focus.lerp(target, 1.0 - (-8.0 * seconds).exp())
        };
    }
    let target_yaw = view.turns as f32 * FRAC_PI_2;
    view.yaw += (target_yaw - view.yaw) * (1.0 - (-12.0 * seconds).exp());
    view.shake = (view.shake - 2.0 * seconds).max(0.0);
    let strength = if settings.camera_shake {
        view.shake * view.shake * 0.7
    } else {
        0.0
    };
    let jitter = Vec2::new(view.jitter(), view.jitter()) * strength;

    let span = view.span.unwrap_or(VISIBLE_GROUND);
    **camera = eye(view.focus + jitter, view.yaw, span);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where the tank of the test stands.
    const TANK: Vec2 = Vec2::new(0.0, 80.0);

    /// A world with a camera and a player's tank.
    fn world_with_a_tank() -> App {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<View>()
            .insert_resource(Settings::default())
            .add_systems(Update, follow);
        app.world_mut().spawn((MainCamera, Transform::default()));
        app.world_mut().spawn((
            Player { seat: 0 },
            Transform::from_translation(scene::to_world(TANK, 0.0)),
        ));
        app
    }

    /// A run in which the player is in this sector.
    fn run_in(sector: u32) -> Run {
        Run {
            seed_text: "TEST".into(),
            seed: 1,
            difficulty: default(),
            players: 1,
            team: 1,
            solo: true,
            sector,
            cleared: 0,
            checkpoint: 0,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase: tanku_core::sim::RunPhase::Playing,
            lost_to: None,
            playtest: false,
            earns: true,
            free: false,
        }
    }

    /// The barrier of the test: it closes sector 3 to the north, 20 m wide, and the tank of
    /// the test waits 3 m in front of its middle.
    fn barrier(sector: u32, north: f32) -> Barrier {
        Barrier {
            sector,
            from: Vec2::new(-10.0, north),
            to: Vec2::new(10.0, north),
            outward: Vec2::Y,
            unlocked: false,
        }
    }

    /// A world in which the tank waits at the barrier of its sector, which is quiet.
    fn world_at_a_barrier() -> App {
        let mut app = world_with_a_tank();
        app.insert_resource(run_in(3));
        // The cursor is on the tank, so that the view doesn't lean toward it.
        app.world_mut().resource_mut::<View>().aim = TANK;
        app.world_mut().spawn(barrier(3, TANK.y + 3.0));
        // The barrier behind the tank, and the one of the next sector, far ahead.
        app.world_mut().spawn(barrier(2, TANK.y - 60.0));
        app.world_mut().spawn(barrier(4, TANK.y + 63.0));
        app
    }

    /// Lets this much time pass, a frame at a time.
    fn wait(app: &mut App, seconds: f32) {
        let frame = std::time::Duration::from_secs_f32(1.0 / 60.0);
        for _ in 0..(seconds * 60.0).round() as u32 {
            app.world_mut().resource_mut::<Time>().advance_by(frame);
            app.update();
        }
    }

    /// Long enough for the view to get where it is going.
    const A_WHILE: f32 = 8.0;

    /// How far ahead of the tank the view looks, to the north.
    fn ahead(app: &App) -> f32 {
        app.world().resource::<View>().focus().y - TANK.y
    }

    fn near(found: f32, wanted: f32) -> bool {
        (found - wanted).abs() < 0.05 * wanted.abs().max(1.0)
    }

    #[test]
    fn the_view_stays_on_a_tank_that_is_far_from_a_barrier() {
        let (far, near) = APPROACH;
        assert_eq!(run_ahead(far), 0.0);
        assert_eq!(run_ahead(far + 30.0), 0.0);
        assert_eq!(run_ahead(near), PAST_BARRIER);
        assert_eq!(run_ahead(0.0), PAST_BARRIER);
        // It runs further ahead as the tank comes close.
        for step in 0..20 {
            let distance = near + (far - near) * step as f32 / 20.0;
            assert!(
                run_ahead(distance) > run_ahead(distance + 0.4),
                "{distance}"
            );
        }
    }

    #[test]
    fn the_view_runs_ahead_of_a_tank_that_waits_at_the_barrier_of_a_quiet_sector() {
        let mut app = world_at_a_barrier();
        wait(&mut app, A_WHILE);
        assert!(near(ahead(&app), PAST_BARRIER), "{}", ahead(&app));
        // The tank stays on the screen, well inside its edge, and as much ground shows.
        let world = app.world_mut();
        let mut camera = world.query_filtered::<&Transform, With<MainCamera>>();
        let eye = *camera.single(world).unwrap();
        assert!(shows(&eye, 16.0 / 9.0, TANK - Vec2::Y * 3.0));
        let focus = scene::to_world(TANK + Vec2::Y * ahead(&app), 0.0);
        let afar = super::eye(Vec2::ZERO, 0.0, VISIBLE_GROUND)
            .translation
            .length();
        assert!(near(eye.translation.distance(focus), afar));
    }

    #[test]
    fn the_view_stays_on_a_tank_that_still_has_enemies_around() {
        let mut app = world_at_a_barrier();
        let enemy = Tank {
            genome: tanku_core::stats::Genome::empty(10),
            team: Team::Enemies,
            sector: 3,
        };
        let enemy = app.world_mut().spawn((enemy, Enemy { awake: true })).id();
        wait(&mut app, A_WHILE);
        assert!(ahead(&app).abs() < 0.05);
        // Enemies of other sectors don't hold it back: the dormant ones are what it shows.
        app.world_mut().get_mut::<Tank>(enemy).unwrap().sector = 4;
        wait(&mut app, A_WHILE);
        assert!(near(ahead(&app), PAST_BARRIER));
    }

    #[test]
    fn the_view_stays_on_a_tank_while_a_bullet_of_the_enemies_still_flies() {
        let mut app = world_at_a_barrier();
        let bullet = |team: Team| Bullet {
            owner: Entity::PLACEHOLDER,
            team,
            sector: 3,
            velocity: Vec2::X,
            bounces_left: 1,
            pierces_left: 0,
            bounced: false,
        };
        let last_shot = app.world_mut().spawn(bullet(Team::Enemies)).id();
        // The player's own bullets are theirs to watch.
        app.world_mut().spawn(bullet(Team::Players));
        wait(&mut app, A_WHILE);
        assert!(ahead(&app).abs() < 0.05);
        app.world_mut().despawn(last_shot);
        wait(&mut app, A_WHILE);
        assert!(near(ahead(&app), PAST_BARRIER));
    }

    #[test]
    fn the_view_stays_on_a_tank_whose_barrier_is_far() {
        let mut app = world_at_a_barrier();
        // The barrier is now further than the view starts to care.
        let world = app.world_mut();
        let mut barriers = world.query::<&mut Barrier>();
        for mut barrier in barriers.iter_mut(world) {
            if barrier.sector == 3 {
                barrier.from.y += APPROACH.0;
                barrier.to.y += APPROACH.0;
            }
        }
        wait(&mut app, A_WHILE);
        assert!(ahead(&app).abs() < 0.05);
    }

    #[test]
    fn the_view_comes_back_slowly_once_the_tank_is_through() {
        let mut app = world_at_a_barrier();
        wait(&mut app, A_WHILE);
        // The tank is now in the next sector, whose barrier is far.
        app.world_mut().resource_mut::<Run>().sector = 4;
        // Half a second later it has started back, and has most of the way left.
        wait(&mut app, 0.5);
        let left = ahead(&app) / PAST_BARRIER;
        assert!(left > 0.5 && left < 0.95, "{left}");
        wait(&mut app, 2.0 * A_WHILE);
        assert!(ahead(&app) < 0.05);
    }

    /// Whether a point of the ground is on a screen of this shape, its width over its
    /// height, for a camera that stands there.
    fn shows(eye: &Transform, shape: f32, point: Vec2) -> bool {
        let world = scene::to_world(point, 0.0);
        let seen = eye.compute_affine().inverse().transform_point3(world);
        let reach = (FIELD_OF_VIEW.to_radians() / 2.0).tan() * -seen.z;
        seen.z < 0.0 && seen.y.abs() <= reach && seen.x.abs() <= reach * shape
    }

    /// What a tank sees of the next group while it waits at a barrier, with the view on the
    /// tank and then with the view run ahead: for north at the top of the screen, then for
    /// the view turned so that the way out is on the right. Each is the share of the enemies
    /// that show, the share of the groups of which at least one enemy shows, and the share
    /// of the groups that show whole.
    fn what_shows_past_barriers(seeds: u64, sectors: u32) -> [[[f32; 3]; 2]; 2] {
        use tanku_core::{
            map::{Corridor, SectorKind},
            progression,
        };
        // A wide screen, and a tank 4 m in front of the barrier of its sector.
        const SHAPE: f32 = 16.0 / 9.0;
        const WAIT: f32 = 4.0;
        let mut seen = [[[0u32; 3]; 2]; 2];
        let (mut enemies, mut groups) = (0, 0);
        for seed in 0..seeds {
            let mut corridor = Corridor::new(seed);
            for index in 0..sectors {
                let plan = corridor.plan(index).clone();
                let next = corridor.layout(index + 1);
                let arena = next.plan.kind == SectorKind::Arena;
                let count = progression::enemy_count(index + 1, 1, arena) as usize;
                let spawns = next.spawns.iter().take(count);
                let group: Vec<Vec2> = spawns.map(|at| at.as_vec2()).collect();
                let (from, to) = (plan.gate.0.as_vec2(), plan.gate.1.as_vec2());
                let outward = (to - from).normalize().perp();
                let tank = (from + to) / 2.0 - outward * WAIT;
                let distance = geom::distance_to_segment(tank, from, to);
                let ahead = tank + outward * run_ahead(distance);
                enemies += group.len() as u32;
                groups += 1;
                // The turn of the view that puts the way out on the right of the screen.
                let right = (outward.to_angle() / FRAC_PI_2).round() * FRAC_PI_2;
                for (focus, counts) in [tank, ahead].into_iter().zip(&mut seen) {
                    for (yaw, counts) in [0.0, right].into_iter().zip(counts) {
                        let eye = eye(focus, yaw, VISIBLE_GROUND);
                        let on_screen = group.iter().filter(|at| shows(&eye, SHAPE, **at));
                        let on_screen = on_screen.count();
                        counts[0] += on_screen as u32;
                        counts[1] += (on_screen > 0) as u32;
                        counts[2] += (on_screen == group.len()) as u32;
                    }
                }
            }
        }
        let shares = |counts: [u32; 3]| {
            [
                counts[0] as f32 / enemies as f32,
                counts[1] as f32 / groups as f32,
                counts[2] as f32 / groups as f32,
            ]
        };
        seen.map(|counts| counts.map(shares))
    }

    /// Prints what shows past barriers, over many corridors:
    /// `cargo test -p tanku_client past_barriers -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn print_what_shows_past_barriers() {
        let seen = what_shows_past_barriers(300, 30);
        for (view, turns) in ["on the tank", "run ahead"].iter().zip(seen) {
            for (turn, shares) in ["north at the top", "the way out on the right"]
                .iter()
                .zip(turns)
            {
                let [enemies, some, all] = shares.map(|share| 100.0 * share);
                println!(
                    "{view:>11}, {turn:>24}: {enemies:5.1}% of the enemies, a part of {some:5.1}% of \
                     the groups, all of {all:5.1}%"
                );
            }
        }
    }

    #[test]
    fn the_view_shows_more_of_the_next_group_when_it_runs_ahead() {
        let [on_the_tank, run_ahead] = what_shows_past_barriers(20, 30);
        for turn in 0..2 {
            assert!(on_the_tank[turn][0] < run_ahead[turn][0]);
        }
        // With north at the top, the view on the tank shows next to nothing of the next
        // group, which is why the view runs ahead.
        assert!(on_the_tank[0][0] < 0.06, "{}", on_the_tank[0][0]);
        assert!(run_ahead[0][0] > 0.2, "{}", run_ahead[0][0]);
    }

    #[test]
    fn a_place_that_is_shown_holds_the_view_until_it_is_released() {
        let mut app = world_with_a_tank();
        let place = Vec2::new(0.0, -500.0);
        let mut view = app.world_mut().resource_mut::<View>();
        view.show(place, Some(7.5));
        // The tank of a run that was just left is still there for a moment: it must not take
        // the view back.
        app.update();
        assert_eq!(app.world().resource::<View>().focus(), place);

        app.world_mut().resource_mut::<View>().release();
        app.update();
        let view = app.world().resource::<View>();
        assert!(view.focus().distance(TANK) < 1.0);
        assert_eq!(view.span, None);
    }
}
