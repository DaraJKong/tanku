//! What a special bullet is about to do, drawn ahead of it.
//!
//! A line in dashes stands for what isn't there yet. From a moment before a bullet that seeks
//! bounces, one runs from the bullet to the wall, and a second from there to the tank that the
//! bullet will leave toward. From a moment before a bullet that explodes ends on something
//! that stands still, one runs from the bullet to that place, and a circle shows what its
//! blast will reach. A bullet that splits carries two lines ahead of it, one for each way
//! that it will go: in dashes at first, then whole and flashing just before it splits.
//!
//! All of it is one drawing, made again at every frame: these marks move with the bullets.

use bevy::prelude::*;
use tanku_core::{
    rules::{BLAST_RADIUS, BULLET_RADIUS, SPLIT_ANGLE, SPLIT_WARNING, TANK_RADIUS},
    sim::{Bullet, Enemy, Kind, Position, Tank, Wreck},
};
use tanku_server::{
    bullets::{self, Mark, Quarry},
    field::Field,
};

use crate::{
    draft::{self, Draft, LIFT},
    scene::{self, Paints},
    theme,
};

/// How far ahead of the middle of a bullet the line of its flight starts: a little past its
/// nose.
const NOSE: f32 = 2.0 * BULLET_RADIUS + 0.1;
/// The two lines ahead of a bullet that splits: how far from its middle they start and end,
/// and how wide they are.
const FORK: (f32, f32) = (0.55, 1.7);
const FORK_WIDTH: f32 = 0.05;
/// How many times those lines show or hide while they flash, each for as long: an odd
/// number, so that they show at first and show again when the bullet splits.
const FLASHES: u32 = 5;
/// The drawing of what the bullets are about to do.
#[derive(Component)]
struct Ahead;

/// One thing that is drawn ahead of a bullet.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Sign {
    /// The way of a bullet to the place where it bounces or ends, in dashes that stand still
    /// while the bullet comes.
    Flight { from: Vec2, to: Vec2 },
    /// The way out of a bullet that seeks: from where it bounces to the tank that it seeks.
    WayOut { from: Vec2, to: Vec2 },
    /// What the blast of a bullet that explodes will reach.
    Blast { at: Vec2 },
    /// The two ways of a bullet that splits, from where it is: in dashes, or whole when it
    /// is about to split.
    Fork { at: Vec2, heading: f32, whole: bool },
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, draw.after(scene::place));
}

/// The drawing has no mesh until there is something to draw: the engine has no room for
/// a mesh without a corner, and says so.
fn setup(mut commands: Commands, paints: Res<Paints>) {
    commands.spawn((
        Ahead,
        MeshMaterial3d(paints.ink.clone()),
        Transform::IDENTITY,
        Visibility::Hidden,
    ));
}

/// Whether the two lines of a bullet that splits show, this long before it does: always
/// while they are in dashes, and every other moment once they are whole.
fn fork_shows(fuse: f32) -> bool {
    let flash = ((1.0 - fuse / SPLIT_WARNING) * FLASHES as f32) as u32;
    fuse > SPLIT_WARNING || flash.min(FLASHES - 1).is_multiple_of(2)
}

/// What is drawn ahead of a bullet that is at a place.
fn signs(field: &Field, bullet: &Bullet, at: Vec2, quarry: &[Quarry]) -> Vec<Sign> {
    let Some(direction) = bullet.velocity.try_normalize() else {
        return Vec::new();
    };
    if let Kind::Splitting { fuse, .. } = bullet.kind {
        let fork = Sign::Fork {
            at,
            heading: direction.to_angle(),
            whole: fuse <= SPLIT_WARNING,
        };
        return fork_shows(fuse).then_some(fork).into_iter().collect();
    }
    // The line stops where the nose of the bullet will.
    let flight = |end: Vec2| {
        let (from, to) = (at + direction * NOSE, end + direction * BULLET_RADIUS);
        ((to - from).dot(direction) > 0.0).then_some(Sign::Flight { from, to })
    };
    match bullets::mark(field, bullet, at, quarry) {
        None => Vec::new(),
        Some(Mark::Blast { at: end }) => flight(end)
            .into_iter()
            .chain([Sign::Blast { at: end }])
            .collect(),
        Some(Mark::Bounce { at: end, toward }) => {
            // The way out stops at the edge of the tank.
            let way_out = toward.and_then(|tank| {
                let reach = tank.distance(end) - TANK_RADIUS;
                let to = end + (tank - end).normalize_or_zero() * reach;
                (reach > 0.0).then_some(Sign::WayOut { from: end, to })
            });
            flight(end).into_iter().chain(way_out).collect()
        }
    }
}

/// A line in dashes from a point toward another. Its dashes are counted from the first
/// point: they stay where they are when the second point moves along the line.
fn dashes(draft: &mut Draft, anchor: Vec3, toward: Vec3, width: f32, color: Color) {
    let (dash, gap) = draft::DASH;
    let length = anchor.distance(toward);
    let Some(along) = (toward - anchor).try_normalize() else {
        return;
    };
    let mut start = 0.0;
    while start < length {
        let end = (start + dash).min(length);
        draft.line(anchor + along * start, anchor + along * end, width, color);
        start += dash + gap;
    }
}

/// The drawing of these signs. Lines are at the height at which bullets fly, and the circle
/// of a blast lies on the floor, as those of a mine do.
fn drawing(signs: &[Sign]) -> Draft {
    let mut draft = Draft::default();
    let raised = |point: Vec2| scene::to_world(point, scene::BULLET_HEIGHT);
    let (white, red) = (theme::BULLET, theme::DANGER);
    for sign in signs {
        match *sign {
            // The dashes are counted from the far end, which doesn't move.
            Sign::Flight { from, to } => {
                dashes(&mut draft, raised(to), raised(from), draft::LINE, white);
            }
            Sign::WayOut { from, to } => {
                dashes(&mut draft, raised(from), raised(to), draft::LINE, white);
            }
            Sign::Blast { at } => {
                let ring = (BLAST_RADIUS - draft::LINE, BLAST_RADIUS);
                let center = scene::to_world(at, 3.0 * LIFT);
                draft.patterned_ring(center, ring, &draft::PHANTOM, red);
            }
            Sign::Fork { at, heading, whole } => {
                for side in [-1.0, 1.0] {
                    let way = Vec2::from_angle(heading + side * SPLIT_ANGLE);
                    let (from, to) = (raised(at + way * FORK.0), raised(at + way * FORK.1));
                    if whole {
                        draft.line(from, to, FORK_WIDTH, white);
                    } else {
                        dashes(&mut draft, from, to, FORK_WIDTH, white);
                    }
                }
            }
        }
    }
    draft
}

/// Draws what every special bullet is about to do, from where the bullet is drawn.
fn draw(
    mut commands: Commands,
    field: Option<Res<Field>>,
    bullets: Query<(&Bullet, &Transform, Option<&Visibility>)>,
    tanks: Query<(&Tank, &Position, Option<&Enemy>, Has<Wreck>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut drawings: Query<(Entity, Option<&Mesh3d>, &mut Visibility), (With<Ahead>, Without<Bullet>)>,
) {
    let Ok((entity, mesh, mut visibility)) = drawings.single_mut() else {
        return;
    };
    // A guest hides a bullet that it has seen end before its host tells so.
    let shown = |(bullet, _, look): &(&Bullet, &Transform, Option<&Visibility>)| {
        bullet.kind != Kind::Plain && look.is_none_or(|look| *look != Visibility::Hidden)
    };
    let special: Vec<(&Bullet, Vec2)> = bullets
        .iter()
        .filter(shown)
        .map(|(bullet, transform, _)| (bullet, scene::to_ground(transform.translation)))
        .collect();
    let mut found = Vec::new();
    if let Some(field) = field.filter(|_| !special.is_empty()) {
        let quarry = bullets::quarry(tanks.iter());
        for (bullet, at) in special {
            found.extend(signs(&field, bullet, at, &quarry));
        }
    }
    let wanted = if found.is_empty() {
        Visibility::Hidden
    } else {
        // The drawing takes the place of the one before it.
        let drawn = drawing(&found).build();
        match mesh {
            Some(mesh) => {
                let _ = meshes.insert(&mesh.0, drawn);
            }
            None => {
                commands.entity(entity).insert(Mesh3d(meshes.add(drawn)));
            }
        }
        Visibility::Inherited
    };
    if *visibility != wanted {
        *visibility = wanted;
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::{
        rules::MARK_TIME,
        sim::Team,
        stats::{Ammo, Genome},
    };
    use tanku_server::field::{Collider, Solid};

    use super::*;

    /// A field with one wall across it, 10 m north of the middle of the map.
    fn field() -> Field {
        let mut field = Field::new(1, 1, default());
        field.colliders = vec![Collider {
            a: Vec2::new(-20.0, 10.0),
            b: Vec2::new(20.0, 10.0),
            solid: Solid::Wall,
        }]
        .into();
        field
    }

    /// A bullet of the enemies, as a tank that fires this type sends it north.
    fn bullet(ammo: Ammo, bounces: u8) -> Bullet {
        let mut genome = Genome::empty(60);
        genome.set(tanku_core::stats::Stat::Ricochet, bounces);
        genome.load(ammo);
        let tank = Tank {
            genome,
            team: Team::Enemies,
            sector: 1,
        };
        Bullet::fired(Entity::PLACEHOLDER, &tank, Vec2::Y * genome.bullet_speed())
    }

    fn close(found: Vec2, wanted: Vec2) -> bool {
        found.distance(wanted) < 1e-3
    }

    #[test]
    fn the_drawing_is_there_while_a_bullet_has_something_to_show() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .insert_resource(field())
            .add_systems(PreStartup, scene::setup)
            .add_systems(Startup, setup)
            .add_systems(Update, draw);
        app.update();
        let drawing = |app: &mut App| -> (Option<usize>, Visibility) {
            let world = app.world_mut();
            let mut drawings =
                world.query_filtered::<(Option<&Mesh3d>, &Visibility), With<Ahead>>();
            let (mesh, visibility) = drawings.single(world).unwrap();
            let (mesh, visibility) = (mesh.map(|mesh| mesh.0.clone()), *visibility);
            let meshes = world.resource::<Assets<Mesh>>();
            let corners = mesh.map(|mesh| meshes.get(&mesh).unwrap().count_vertices());
            (corners, visibility)
        };
        // Nothing to draw yet: the drawing has no mesh, not one without a corner.
        assert_eq!(drawing(&mut app), (None, Visibility::Hidden));
        // A normal bullet changes nothing.
        let flying = |ammo: Ammo, at: Vec2| {
            let place = Transform::from_translation(scene::to_world(at, scene::BULLET_HEIGHT));
            (bullet(ammo, 0), place)
        };
        let near = Vec2::new(0.0, 6.0);
        app.world_mut().spawn(flying(Ammo::Normal, near));
        app.update();
        assert_eq!(drawing(&mut app), (None, Visibility::Hidden));
        // A bullet that is about to explode is drawn its way and its blast.
        let exploding = app.world_mut().spawn(flying(Ammo::Exploding, near)).id();
        app.update();
        app.update();
        let (corners, visibility) = drawing(&mut app);
        assert_eq!(visibility, Visibility::Inherited);
        let whole = corners.unwrap();
        assert!(whole > 0);
        // The drawing follows the bullet: nearer to the wall, its way is shorter.
        let nearer = scene::to_world(Vec2::new(0.0, 9.0), scene::BULLET_HEIGHT);
        let mut entity = app.world_mut().entity_mut(exploding);
        entity.get_mut::<Transform>().unwrap().translation = nearer;
        app.update();
        let (corners, visibility) = drawing(&mut app);
        assert_eq!(visibility, Visibility::Inherited);
        assert!(corners.unwrap() > 0 && corners.unwrap() < whole);
        // A bullet that a guest has hidden shows nothing, and neither does one that is gone.
        app.world_mut()
            .entity_mut(exploding)
            .insert(Visibility::Hidden);
        app.update();
        assert_eq!(drawing(&mut app).1, Visibility::Hidden);
        app.world_mut()
            .entity_mut(exploding)
            .insert(Visibility::Inherited);
        app.update();
        assert_eq!(drawing(&mut app).1, Visibility::Inherited);
        app.world_mut().despawn(exploding);
        app.update();
        assert_eq!(drawing(&mut app).1, Visibility::Hidden);
    }

    #[test]
    fn a_normal_bullet_and_a_piercing_one_show_nothing() {
        let field = field();
        for ammo in [Ammo::Normal, Ammo::Piercing] {
            for from in [0.0, 5.0, 9.0] {
                let at = Vec2::new(0.0, from);
                assert_eq!(signs(&field, &bullet(ammo, 2), at, &[]), [], "{ammo:?}");
            }
        }
    }

    #[test]
    fn a_bullet_that_explodes_shows_its_way_and_its_blast_a_moment_before() {
        let field = field();
        let bullet = bullet(Ammo::Exploding, 0);
        let reach = bullet.velocity.length() * MARK_TIME;
        // The middle of the bullet stops a radius short of the wall.
        let end = Vec2::new(0.0, 10.0 - BULLET_RADIUS);
        // Too far from the wall, nothing shows yet.
        let far = Vec2::new(0.0, end.y - reach - 0.5);
        assert_eq!(signs(&field, &bullet, far, &[]), []);
        // Then a line runs from its nose to the wall, and a circle shows its blast around
        // the place where it will end.
        for back in [reach - 0.5, 3.0, 1.0] {
            let at = Vec2::new(0.0, end.y - back);
            let found = signs(&field, &bullet, at, &[]);
            let [Sign::Flight { from, to }, Sign::Blast { at: blast }] = found[..] else {
                panic!("{found:?}");
            };
            assert!(close(from, at + Vec2::Y * NOSE), "{from}");
            assert!(close(to, Vec2::new(0.0, 10.0)), "{to}");
            assert!(close(blast, end), "{blast}");
        }
        // Against the wall, the circle alone is left.
        let at = end - Vec2::Y * 0.2;
        assert_eq!(signs(&field, &bullet, at, &[]), [Sign::Blast { at: end }]);
    }

    #[test]
    fn a_bullet_that_seeks_shows_its_way_to_the_wall_and_its_way_out() {
        let field = field();
        let end = Vec2::new(0.0, 10.0 - BULLET_RADIUS);
        let at = Vec2::new(0.0, 4.0);
        let players = |place: Vec2| vec![(place, Team::Players)];
        // With a bounce left, a line runs to the wall, and one from there to the edge of
        // the tank that it will leave toward.
        let seeking = bullet(Ammo::Seeking, 1);
        let tank = Vec2::new(6.0, 2.0);
        let found = signs(&field, &seeking, at, &players(tank));
        let [Sign::Flight { to, .. }, Sign::WayOut { from, to: out }] = found[..] else {
            panic!("{found:?}");
        };
        assert!(close(to, Vec2::new(0.0, 10.0)) && close(from, end));
        assert!((out.distance(tank) - TANK_RADIUS).abs() < 1e-3, "{out}");
        assert!((out - end).perp_dot(tank - end).abs() < 1e-3);
        // The way out follows the tank.
        let moved = Vec2::new(-5.0, 6.0);
        let found = signs(&field, &seeking, at, &players(moved));
        let Some(Sign::WayOut { to: out, .. }) = found.last().copied() else {
            panic!("{found:?}");
        };
        assert!((out.distance(moved) - TANK_RADIUS).abs() < 1e-3);
        // It seeks no tank of its own side: its way to the wall shows alone, and it will
        // leave at the mirror angle.
        let allies = vec![(tank, Team::Enemies)];
        let found = signs(&field, &seeking, at, &allies);
        assert!(matches!(found[..], [Sign::Flight { .. }]), "{found:?}");
        // Without a bounce left it is a normal bullet, which shows nothing.
        let spent = bullet(Ammo::Seeking, 0);
        assert_eq!(signs(&field, &spent, at, &players(tank)), []);
        // Nothing shows far from the wall.
        let far = Vec2::new(0.0, -8.0);
        assert_eq!(signs(&field, &seeking, far, &players(tank)), []);
    }

    #[test]
    fn a_bullet_that_splits_carries_its_two_ways_which_flash_before_it_does() {
        let field = field();
        let at = Vec2::new(1.0, 2.0);
        let mut splitting = bullet(Ammo::Splitting, 1);
        let fork = |bullet: &Bullet| signs(&field, bullet, at, &[]);
        // In dashes for as long as the split is far, wherever the bullet is.
        let heading = std::f32::consts::FRAC_PI_2;
        let dashed = Sign::Fork {
            at,
            heading,
            whole: false,
        };
        assert_eq!(fork(&splitting), [dashed]);
        // Whole and flashing for as long as a mine warns: there at first, and there when
        // the bullet splits.
        let whole = Sign::Fork {
            at,
            heading,
            whole: true,
        };
        let mut shown = Vec::new();
        for step in 0..=60 {
            let fuse = SPLIT_WARNING * (1.0 - step as f32 / 60.0);
            let Kind::Splitting { bounces, .. } = splitting.kind else {
                panic!("it splits");
            };
            splitting.kind = Kind::Splitting { fuse, bounces };
            let found = fork(&splitting);
            assert!(found.is_empty() || found == [whole], "{found:?}");
            shown.push(!found.is_empty());
        }
        assert!(shown[0] && shown[60]);
        let changes = shown.windows(2).filter(|pair| pair[0] != pair[1]).count();
        assert_eq!(changes, 4, "{shown:?}");
        let there = shown.iter().filter(|shown| **shown).count();
        assert!((34..=38).contains(&there), "{there}");
    }

    #[test]
    fn the_dashes_of_a_way_stand_still_while_the_bullet_comes() {
        let (white, red) = (theme::BULLET, theme::DANGER);
        let to = Vec2::new(0.0, 10.0);
        let way = |from: f32| {
            let from = Vec2::new(0.0, from);
            drawing(&[Sign::Flight { from, to }])
        };
        // The line reaches the wall, and loses its dashes from the side of the bullet.
        let (long, short) = (way(2.0), way(6.03));
        for draft in [&long, &short] {
            let (lowest, highest) = draft.bounds();
            assert!((scene::to_ground(lowest).y - 10.0).abs() < 1e-4);
            assert!((highest.y - scene::BULLET_HEIGHT).abs() < 1e-4);
            assert!((lowest.y - scene::BULLET_HEIGHT).abs() < 1e-4);
        }
        assert!((scene::to_ground(long.bounds().1).y - 2.0).abs() < 0.09);
        assert!((scene::to_ground(short.bounds().1).y - 6.03).abs() < 0.09);
        // Dashes and gaps keep their lengths: a line half as long has half the ink.
        let ink = |draft: &Draft| draft.area(white) / draft::LINE;
        let (dash, gap) = draft::DASH;
        let share = dash / (dash + gap);
        assert!((ink(&long) - 8.0 * share).abs() < dash);
        assert!((ink(&short) - 3.97 * share).abs() < dash);

        // The circle of a blast is red, on the floor, in the phantom line of a reach: 27 of
        // its 36 widths are inked.
        let blast = drawing(&[Sign::Blast { at: to }]);
        let whole = std::f32::consts::PI * (2.0 * BLAST_RADIUS - draft::LINE) * draft::LINE;
        assert!((blast.area(red) / whole - 0.75).abs() < 0.03);
        assert!(blast.bounds().1.y < 0.05);
        // The two ways of a bullet that splits leave its nose at the angle of the split.
        let fork = |whole: bool| {
            drawing(&[Sign::Fork {
                at: Vec2::ZERO,
                heading: 0.0,
                whole,
            }])
        };
        let (dashed, whole) = (fork(false), fork(true));
        let length = FORK.1 - FORK.0;
        assert!((whole.area(white) - 2.0 * length * FORK_WIDTH).abs() < 1e-4);
        // In dashes, a gap of 3 widths after each dash of 12.
        assert!(dashed.area(white) < 0.9 * whole.area(white));
        let reach = scene::to_ground(whole.bounds().1);
        assert!((reach.x - FORK.1 * SPLIT_ANGLE.cos()).abs() < FORK_WIDTH);
        assert!((reach.y.abs() - FORK.1 * SPLIT_ANGLE.sin()).abs() < FORK_WIDTH);
    }
}
