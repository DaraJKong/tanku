//! The scars that a fight leaves on the floor, as marks on the drawing.
//!
//! Where a blast went off, a scalloped outline stays around a darker patch, the way a change
//! is circled on a plan. A block that the blast took away stays as a square in dashes, and a
//! cross marks every place where a tank was destroyed. The marks belong to their sector and
//! go when it does. In arena mode, where the room never goes, they go with each group.

use std::f32::consts::TAU;

use bevy::prelude::*;
use tanku_core::{
    rules::{BLAST_RADIUS, BLOCK_SIZE},
    sim::{Run, Sector, SimEvent},
};

use crate::{
    draft::{Draft, LIFT},
    scene::{self, Paints},
    theme,
};

/// The scallops around a blast, and the radius of the arc of each one.
const SCALLOPS: usize = 16;
const SCALLOP_RADIUS: f32 = 0.68;
/// The straight strokes that draw the arc of one scallop.
const SCALLOP_STROKES: usize = 6;
/// Radius of the darker patch inside the outline, and how dark it is.
const SCORCH_RADIUS: f32 = 2.3;
const SCORCH: f32 = 0.2;
/// A dash of the outline of a block that is gone, and the gap after it: 12 and 3 widths of its
/// line, as ISO 128-20 has them.
const DASH: (f32, f32) = (12.0 * WIDTHS.1, 3.0 * WIDTHS.1);
/// Length of an arm of the cross that marks a destroyed tank, from end to end.
const CROSS: f32 = 1.0;
/// Widths of the three kinds of line.
const WIDTHS: (f32, f32, f32) = (0.035, 0.03, 0.05);
/// How strong the outline of a blast and the dashes of a block are over the floor. A cross
/// is at full strength.
const STRENGTHS: (f32, f32) = (0.75, 0.7);
/// A sector keeps this many blasts on its floor. The oldest go first.
const MOST_BLASTS: usize = 24;

/// A mark on the floor of a sector. A blast carries its number, which tells how old it is.
#[derive(Component)]
struct Scar {
    blast: Option<u32>,
}

/// What the darker patch of a blast is painted with.
#[derive(Resource)]
struct Scorch(Handle<StandardMaterial>);

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(Update, mark);
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    let scorch = scene::paint(&mut materials, Color::BLACK.with_alpha(SCORCH));
    commands.insert_resource(Scorch(scorch));
}

/// Whether a point is inside an outline.
fn inside(outline: &[Vec2], point: Vec2) -> bool {
    let count = outline.len();
    let crossings = (0..count).filter(|&corner| {
        let (a, b) = (outline[corner], outline[(corner + 1) % count]);
        // The edge passes the height of the point, to the right of it.
        (a.y > point.y) != (b.y > point.y)
            && point.x < a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x)
    });
    crossings.count() % 2 == 1
}

/// The outline of a blast: scallops that bulge out of the circle that the blast reached,
/// as strokes from one point of the ground to the next. `turn` is where the first one starts.
fn scallops(center: Vec2, turn: f32) -> Vec<(Vec2, Vec2)> {
    let arc = TAU / SCALLOPS as f32;
    let on_circle = |angle: f32| center + Vec2::from_angle(angle) * BLAST_RADIUS;
    let mut strokes = Vec::with_capacity(SCALLOPS * SCALLOP_STROKES);
    for scallop in 0..SCALLOPS {
        let start = turn + scallop as f32 * arc;
        let (from, to) = (on_circle(start), on_circle(start + arc));
        // The arc of a scallop turns around a point inside the circle, behind the middle
        // of its chord.
        let (middle, half_chord) = ((from + to) / 2.0, from.distance(to) / 2.0);
        let back = (SCALLOP_RADIUS * SCALLOP_RADIUS - half_chord * half_chord).sqrt();
        let hub = middle - (middle - center).normalize() * back;
        let (first, last) = ((from - hub).to_angle(), (to - hub).to_angle());
        let sweep = (last - first).rem_euclid(TAU);
        let point = |stroke: usize| {
            let angle = first + sweep * stroke as f32 / SCALLOP_STROKES as f32;
            hub + Vec2::from_angle(angle) * SCALLOP_RADIUS
        };
        strokes.extend((0..SCALLOP_STROKES).map(|stroke| (point(stroke), point(stroke + 1))));
    }
    strokes
}

/// A line in dashes, from one point to another. It starts and ends with a dash, so that every
/// corner of a square is drawn, and its dashes are stretched a little to fit.
fn dashes(from: Vec2, to: Vec2) -> Vec<(Vec2, Vec2)> {
    let (length, along) = (from.distance(to), (to - from).normalize_or_zero());
    let (dash, gap) = DASH;
    let count = (((length + gap) / (dash + gap)).round() as usize).max(1);
    let stretch = length / (count as f32 * dash + (count - 1) as f32 * gap);
    (0..count)
        .map(|index| {
            let at = index as f32 * (dash + gap) * stretch;
            (
                from + along * at,
                from + along * (at + dash * stretch).min(length),
            )
        })
        .collect()
}

/// Lays strokes on the floor of a sector. What falls outside the sector is left out: a
/// blast stops at walls and barriers, and the void has no floor to mark.
fn lay(draft: &mut Draft, strokes: &[(Vec2, Vec2)], outline: &[Vec2], width: f32, color: Color) {
    for &(from, to) in strokes {
        if inside(outline, (from + to) / 2.0) {
            draft.line(
                scene::to_world(from, 0.0),
                scene::to_world(to, 0.0),
                width,
                color,
            );
        }
    }
}

/// The darker patch of a blast, cut into small faces so that the walls of the sector can
/// cut it too.
fn scorch(center: Vec2, outline: &[Vec2]) -> Draft {
    const WEDGES: usize = 24;
    const RINGS: usize = 4;
    let mut draft = Draft::default();
    let corner = |ring: usize, wedge: usize| {
        let reach = SCORCH_RADIUS * ring as f32 / RINGS as f32;
        center + Vec2::from_angle(TAU * wedge as f32 / WEDGES as f32) * reach
    };
    for ring in 0..RINGS {
        for wedge in 0..WEDGES {
            let mut corners = vec![
                corner(ring + 1, wedge),
                corner(ring + 1, wedge + 1),
                corner(ring, wedge + 1),
            ];
            if ring > 0 {
                corners.push(corner(ring, wedge));
            }
            let middle = corners.iter().sum::<Vec2>() / corners.len() as f32;
            if inside(outline, middle) {
                let corners: Vec<Vec3> = corners
                    .iter()
                    .map(|&corner| scene::to_world(corner, 0.0))
                    .collect();
                draft.face(&corners, Color::WHITE);
            }
        }
    }
    draft
}

/// The lines of a mark: the outline of a blast, the square of a block or the cross of a tank.
fn lines(event: &SimEvent, outline: &[Vec2], turn: f32) -> Draft {
    let mut draft = Draft::default();
    let over_the_floor = |strength: f32| theme::over(theme::FLOOR, theme::SCAR, strength);
    match *event {
        SimEvent::Blast { at } => {
            let color = over_the_floor(STRENGTHS.0);
            lay(&mut draft, &scallops(at, turn), outline, WIDTHS.0, color);
        }
        SimEvent::BlockDestroyed { at } => {
            let half = BLOCK_SIZE / 2.0;
            let corners = [(-half, -half), (half, -half), (half, half), (-half, half)]
                .map(|(x, y)| at + Vec2::new(x, y));
            for side in 0..4 {
                let strokes = dashes(corners[side], corners[(side + 1) % 4]);
                let color = over_the_floor(STRENGTHS.1);
                lay(&mut draft, &strokes, outline, WIDTHS.1, color);
            }
        }
        SimEvent::TankDestroyed { at, .. } => {
            let arm = Vec2::splat(CROSS / 2.0 / std::f32::consts::SQRT_2);
            let strokes = [(at - arm, at + arm), (at - arm.perp(), at + arm.perp())];
            lay(&mut draft, &strokes, outline, WIDTHS.2, theme::SCAR);
        }
        _ => {}
    }
    draft
}

/// Marks the floor where something blew up or was destroyed.
fn mark(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    run: Option<Res<Run>>,
    sectors: Query<(Entity, &Sector)>,
    scars: Query<(Entity, &Scar, &ChildOf)>,
    paints: Res<Paints>,
    scorched: Res<Scorch>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut blasts: Local<u32>,
) {
    let events: Vec<SimEvent> = events.read().copied().collect();
    // In arena mode, the floor is wiped when a group is cleared: the room is laid out anew,
    // and the marks of a run without end would cover it.
    let in_a_room = run.is_some_and(|run| run.arena);
    let cleared = |event: &SimEvent| matches!(event, SimEvent::SectorCleared { .. });
    if in_a_room && events.iter().any(cleared) {
        for (entity, ..) in &scars {
            commands.entity(entity).despawn();
        }
        return;
    }
    let mut blasted = false;
    for event in &events {
        let at = match *event {
            SimEvent::Blast { at }
            | SimEvent::BlockDestroyed { at }
            | SimEvent::TankDestroyed { at, .. } => at,
            _ => continue,
        };
        // The mark belongs to the sector it happened in, and stops at its edges.
        let outline_of = |sector: &Sector| -> Vec<Vec2> {
            let corners = sector.layout.plan.outline.iter();
            corners.map(|corner| corner.as_vec2()).collect()
        };
        let mut floors = sectors
            .iter()
            .map(|(entity, sector)| (entity, outline_of(sector)));
        let Some((sector, outline)) = floors.find(|(_, outline)| inside(outline, at)) else {
            continue;
        };
        let is_blast = matches!(event, SimEvent::Blast { .. });
        if is_blast {
            *blasts += 1;
            blasted = true;
        }
        // No two blasts are circled quite the same way.
        let turn = *blasts as f32 * 2.4;
        let scar = Scar {
            blast: is_blast.then_some(*blasts),
        };
        let scar = commands
            .spawn((
                scar,
                Transform::default(),
                Visibility::default(),
                ChildOf(sector),
            ))
            .id();
        let drawn = lines(event, &outline, turn);
        if !drawn.is_empty() {
            commands.spawn((
                Mesh3d(meshes.add(drawn.build())),
                MeshMaterial3d(paints.ink.clone()),
                Transform::from_xyz(0.0, 0.6 * LIFT, 0.0),
                ChildOf(scar),
            ));
        }
        let patch = if is_blast {
            scorch(at, &outline)
        } else {
            Draft::default()
        };
        if !patch.is_empty() {
            commands.spawn((
                Mesh3d(meshes.add(patch.build())),
                MeshMaterial3d(scorched.0.clone()),
                Transform::from_xyz(0.0, 0.3 * LIFT, 0.0),
                ChildOf(scar),
            ));
        }
    }
    if !blasted {
        return;
    }
    // A long fight doesn't cover the floor: the oldest blasts of a sector are rubbed out.
    // Those of this frame aren't counted yet, so a sector may keep a few more for a moment.
    let mut kept: Vec<(Entity, u32, Entity)> = scars
        .iter()
        .filter_map(|(entity, scar, sector)| Some((sector.parent(), scar.blast?, entity)))
        .collect();
    kept.sort();
    for of_sector in kept.chunk_by(|a, b| a.0 == b.0) {
        let surplus = of_sector.len().saturating_sub(MOST_BLASTS);
        for (_, _, entity) in &of_sector[..surplus] {
            commands.entity(*entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::{map::Corridor, sim::Team};

    use super::*;

    /// A room of 20 m, around the origin.
    fn room() -> Vec<Vec2> {
        [(-10.0, -10.0), (-10.0, 10.0), (10.0, 10.0), (10.0, -10.0)]
            .map(Vec2::from)
            .to_vec()
    }

    #[test]
    fn a_point_is_inside_an_outline_or_not() {
        let room = room();
        assert!(inside(&room, Vec2::ZERO));
        assert!(inside(&room, Vec2::new(9.9, -9.9)));
        assert!(!inside(&room, Vec2::new(10.1, 0.0)));
        assert!(!inside(&room, Vec2::new(0.0, -30.0)));
        // An outline with a notch: the notch is outside.
        let notched = [
            (0, 0),
            (0, 10),
            (4, 10),
            (4, 4),
            (8, 4),
            (8, 10),
            (12, 10),
            (12, 0),
        ]
        .map(|(x, y)| Vec2::new(x as f32, y as f32));
        assert!(inside(&notched, Vec2::new(2.0, 8.0)));
        assert!(!inside(&notched, Vec2::new(6.0, 8.0)));
        assert!(inside(&notched, Vec2::new(6.0, 2.0)));
    }

    #[test]
    fn the_scallops_of_a_blast_close_around_what_it_reached() {
        let center = Vec2::new(3.0, -2.0);
        let strokes = scallops(center, 0.7);
        assert_eq!(strokes.len(), SCALLOPS * SCALLOP_STROKES);
        for (index, &(from, to)) in strokes.iter().enumerate() {
            // Each stroke starts where the one before it ended, all the way around.
            let before = strokes[(index + strokes.len() - 1) % strokes.len()].1;
            assert!(before.distance(from) < 1e-3, "stroke {index}");
            // The scallops bulge out of the circle of the blast, by less than a meter.
            for point in [from, to] {
                let reach = point.distance(center);
                assert!((BLAST_RADIUS - 1e-3..BLAST_RADIUS + 0.5).contains(&reach));
            }
        }
    }

    #[test]
    fn dashes_fill_a_line_from_corner_to_corner() {
        let (from, to) = (Vec2::new(1.0, 1.0), Vec2::new(3.0, 1.0));
        let strokes = dashes(from, to);
        assert_eq!(strokes.len(), 5);
        // A dash at each end, so that the corners of a square are drawn, and each dash as
        // long as the others, a little more than 12 widths of the line.
        let (first, last) = (strokes[0], strokes[strokes.len() - 1]);
        assert!(first.0.distance(from) < 1e-4 && last.1.distance(to) < 1e-4);
        let length = first.1.x - first.0.x;
        assert!((length - DASH.0).abs() < 0.03);
        for (start, end) in strokes {
            assert!((end.x - start.x - length).abs() < 1e-4);
        }
    }

    #[test]
    fn a_blast_near_a_wall_leaves_no_mark_beyond_it() {
        let room = room();
        // One meter from the east wall: a third of the outline would be past it.
        let blast = SimEvent::Blast {
            at: Vec2::new(9.0, 0.0),
        };
        let drawn = lines(&blast, &room, 0.0);
        let (lowest, highest) = drawn.bounds();
        assert!(highest.x < 10.3 && lowest.x > 5.5);
        let whole = lines(&SimEvent::Blast { at: Vec2::ZERO }, &room, 0.0);
        let color = theme::over(theme::FLOOR, theme::SCAR, STRENGTHS.0);
        assert!(drawn.area(color) < 0.8 * whole.area(color));
        // The same for the darker patch.
        let (_, highest) = scorch(Vec2::new(9.0, 0.0), &room).bounds();
        assert!(highest.x < 10.3);
    }

    /// A world that marks the floor as the game does, with the start bay loaded.
    fn world_with_a_bay() -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .add_message::<SimEvent>()
            .add_systems(PreStartup, scene::setup)
            .add_systems(Startup, setup)
            .add_systems(Update, mark);
        let layout = Corridor::new(1).layout(0);
        let bay = app.world_mut().spawn(Sector { layout }).id();
        (app, bay)
    }

    fn scars_of(app: &mut App, sector: Entity) -> usize {
        let world = app.world_mut();
        let mut scars = world.query::<(&Scar, &ChildOf)>();
        let of_sector = scars
            .iter(world)
            .filter(|(_, parent)| parent.parent() == sector);
        of_sector.count()
    }

    #[test]
    fn marks_belong_to_the_sector_they_happened_in() {
        let (mut app, bay) = world_with_a_bay();
        let inside_the_bay = Vec2::new(0.0, 8.0);
        app.world_mut()
            .write_message(SimEvent::Blast { at: inside_the_bay });
        app.world_mut().write_message(SimEvent::TankDestroyed {
            at: inside_the_bay,
            team: Team::Enemies,
        });
        // Nothing is loaded there: no floor to mark.
        app.world_mut().write_message(SimEvent::Blast {
            at: Vec2::new(0.0, 500.0),
        });
        app.update();
        assert_eq!(scars_of(&mut app, bay), 2);
        // The marks go with their sector.
        app.world_mut().entity_mut(bay).despawn();
        let world = app.world_mut();
        assert_eq!(world.query::<&Scar>().iter(world).count(), 0);
    }

    /// A run that is at this sector, in arena mode or in the corridor.
    fn run_at(sector: u32, arena: bool) -> Run {
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
            earns: !arena,
            free: false,
            arena,
            lull: None,
            paused: false,
        }
    }

    #[test]
    fn the_floor_of_arena_mode_is_wiped_when_a_group_is_cleared() {
        let at = Vec2::new(0.0, 8.0);
        let fight = |app: &mut App| {
            app.world_mut().write_message(SimEvent::Blast { at });
            let team = Team::Enemies;
            app.world_mut()
                .write_message(SimEvent::TankDestroyed { at, team });
            app.world_mut()
                .write_message(SimEvent::BlockDestroyed { at });
        };
        let (mut app, room) = world_with_a_bay();
        app.insert_resource(run_at(3, true));
        fight(&mut app);
        app.update();
        assert_eq!(scars_of(&mut app, room), 3);
        // The last tank of the group leaves no mark either: the floor is wiped with it.
        fight(&mut app);
        app.world_mut()
            .write_message(SimEvent::SectorCleared { sector: 3 });
        app.update();
        app.update();
        assert_eq!(scars_of(&mut app, room), 0);
        // The next fight marks it again.
        fight(&mut app);
        app.update();
        assert_eq!(scars_of(&mut app, room), 3);

        // In the corridor, the marks of a sector stay when it is cleared.
        let (mut app, bay) = world_with_a_bay();
        app.insert_resource(run_at(3, false));
        fight(&mut app);
        app.world_mut()
            .write_message(SimEvent::SectorCleared { sector: 3 });
        app.update();
        app.update();
        assert_eq!(scars_of(&mut app, bay), 3);
    }

    #[test]
    fn a_sector_keeps_its_latest_blasts_only() {
        let (mut app, bay) = world_with_a_bay();
        for _ in 0..MOST_BLASTS + 8 {
            let at = Vec2::new(0.0, 8.0);
            app.world_mut().write_message(SimEvent::Blast { at });
            app.update();
        }
        // The blast of the last frame isn't counted yet when the oldest are rubbed out.
        assert!(scars_of(&mut app, bay) <= MOST_BLASTS + 1);
        // The crosses of destroyed tanks all stay.
        for _ in 0..3 {
            app.world_mut().write_message(SimEvent::TankDestroyed {
                at: Vec2::new(0.0, 8.0),
                team: Team::Enemies,
            });
        }
        app.update();
        assert!(scars_of(&mut app, bay) >= MOST_BLASTS + 3);
    }
}
