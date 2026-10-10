//! The report of a run that is over: the drawing of the tank that ended it.
//!
//! The tank stands large on the sheet. Its parts are numbered on the drawing and listed next
//! to it with what each one gives, so that the player knows that tank the next time.

use std::f32::consts::TAU;

use bevy::prelude::*;
use tanku_core::{
    sim::{Blow, Run, Team, Weapon},
    stats::{Genome, SpendingGroup, Stat},
};

use crate::{
    Screen,
    camera::{self, OnScreen, View},
    exhibit::{self, Exhibit, Show, Stand},
    menu, models,
    scene::Paints,
    theme,
};

/// Meters of ground that the report shows from the bottom to the top of the screen.
const SPAN: f32 = 9.0;
/// Where the tank stands on the screen, in meters from its middle.
const PLACE: Vec2 = Vec2::new(2.6, 0.9);
/// Where its turret points, from the way its hull faces.
const TURRET: f32 = 0.45;
/// Radius of a balloon, in pixels.
const BALLOON: f32 = 15.0;
/// How far from the middle of the tank the balloons stand, in meters.
const CIRCLE: f32 = 1.9;
/// The least angle between two balloons, seen from the middle of the tank.
const LEAST_GAP: f32 = 0.3;
/// Size of the dot at the end of a leader, on the part it points at.
const DOT: f32 = 6.0;

/// A part of a tank, as the list tells it.
pub struct Part {
    /// The stat and its level.
    pub name: String,
    /// What that level gives.
    pub gives: String,
    /// The points it costs.
    pub cost: u32,
}

/// What points at the parts of the tank on show: where the tank stands, and where each part
/// is in the world.
#[derive(Resource)]
struct Callouts {
    stand: Stand,
    parts: Vec<Vec3>,
}

/// A piece of the balloon of a part: the line to the part, the dot on it and the ring with
/// its number.
#[derive(Component, Clone, Copy)]
enum Piece {
    Leader(usize),
    Dot(usize),
    Ring(usize),
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnExit(Screen::GameOver), put_away)
        .add_systems(
            Update,
            (stage, point.after(camera::follow))
                .chain()
                .run_if(in_state(Screen::GameOver)),
        );
}

/// A number without the zeros that say nothing: 4, 2.5, 1.25.
fn plain(number: f32) -> String {
    let text = format!("{number:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn count(number: u8, thing: &str) -> String {
    let plural = if number == 1 { "" } else { "s" };
    format!("{number} {thing}{plural}")
}

fn stat_name(stat: Stat) -> &'static str {
    match stat {
        Stat::Engine => "Engine",
        Stat::Tracks => "Tracks",
        Stat::Turret => "Turret",
        Stat::Muzzle => "Muzzle",
        Stat::Magazine => "Magazine",
        Stat::Loader => "Loader",
        Stat::Ricochet => "Ricochet",
        Stat::Mines => "Mines",
        Stat::Shield => "Shield",
        Stat::Piercing => "Piercing",
        Stat::Brain => "Brain",
        Stat::Ballistics => "Ballistics",
    }
}

/// What the level of a stat gives a tank, in the words of the specification.
fn gives(genome: &Genome, stat: Stat) -> String {
    match stat {
        Stat::Engine if genome.top_speed() == 0.0 => "Fixed".to_string(),
        Stat::Engine => format!("{} m/s", plain(genome.top_speed())),
        Stat::Tracks => format!("{:.0}\u{b0}/s", genome.hull_turn_rate().to_degrees()),
        Stat::Turret => format!("{:.0}\u{b0}/s", genome.turret_turn_rate().to_degrees()),
        Stat::Muzzle => format!("{} m/s", plain(genome.bullet_speed())),
        Stat::Magazine => count(genome.magazine(), "round"),
        Stat::Loader => format!("{} s", plain(genome.reload_time())),
        Stat::Ricochet => count(genome.bounces(), "bounce"),
        Stat::Mines => count(genome.mines(), "mine"),
        Stat::Shield => match genome.shield() {
            Some((stun, _)) => format!("{} s stun", plain(stun)),
            None => "None".to_string(),
        },
        Stat::Piercing => count(genome.piercing(), "bullet"),
        Stat::Brain => {
            let tiers = ["Sentry", "Grunt", "Soldier", "Veteran", "Ace"];
            tiers[genome.brain() as usize].to_string()
        }
        Stat::Ballistics => {
            let tiers = [
                "Aims straight",
                "Leads targets",
                "One bounce",
                "Two bounces",
            ];
            tiers[genome.ballistics() as usize].to_string()
        }
    }
}

/// The points that the level of a stat costs a tank.
fn cost(genome: &Genome, stat: Stat) -> u32 {
    let level = genome.level(stat);
    if stat.is_software() {
        stat.share(level, genome.budget as u32)
    } else {
        stat.price(level)
    }
}

/// The parts of the tank behind a blow that the drawing points at, in the order of their
/// numbers. A player's turret follows the cursor, so it tells nothing.
fn anchors(blow: &Blow) -> Vec<models::Anchor> {
    let mut anchors = models::anchors(&blow.genome);
    anchors.retain(|anchor| blow.team == Team::Enemies || anchor.stat != Stat::Turret);
    anchors
}

/// The list of the parts of the tank behind a blow.
pub fn parts(blow: &Blow) -> Vec<Part> {
    let genome = &blow.genome;
    let listed = |anchor: &models::Anchor| {
        let stat = anchor.stat;
        Part {
            name: format!("{} {}", stat_name(stat), genome.level(stat)),
            gives: gives(genome, stat),
            cost: cost(genome, stat),
        }
    };
    anchors(blow).iter().map(listed).collect()
}

/// What kind of tank it was: where most of its points went, which is what its hue tells.
pub fn build(blow: &Blow) -> &'static str {
    match (blow.team, blow.genome.main_group()) {
        (Team::Players, _) => "Your own tank",
        (_, SpendingGroup::Mobility) => "Mobility build",
        (_, SpendingGroup::Firepower) => "Firepower build",
        (_, SpendingGroup::Utility) => "Mines and shield build",
        (_, SpendingGroup::Software) => "Software build",
    }
}

/// How the tank did it.
pub fn means(blow: &Blow) -> &'static str {
    match (blow.team, blow.weapon) {
        (Team::Enemies, Weapon::Bullet) => "With a bullet",
        (Team::Enemies, Weapon::Mine) => "With a mine",
        (Team::Enemies, Weapon::Crate) => "With a crate that it set off",
        (Team::Players, Weapon::Bullet) => "With a bullet that bounced back",
        (Team::Players, Weapon::Mine) => "With a mine of your own",
        (Team::Players, Weapon::Crate) => "With a crate that you set off",
    }
}

/// The color of the tank behind a blow.
pub fn color(blow: &Blow) -> Color {
    match blow.team {
        Team::Players => theme::PLAYER,
        Team::Enemies => models::enemy_color(&blow.genome),
    }
}

/// Moves angles apart until no two are closer than `least`, without changing their order
/// around the circle.
fn spread(angles: &[f32], least: f32) -> Vec<f32> {
    let count = angles.len();
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by(|&a, &b| angles[a].total_cmp(&angles[b]));
    let mut sorted: Vec<f32> = order.iter().map(|&index| angles[index]).collect();
    for _ in 0..64 {
        let mut moved = false;
        for here in 0..count {
            let next = (here + 1) % count;
            if next == here {
                break;
            }
            // The last one is followed by the first, a turn further.
            let turn = if next == 0 { TAU } else { 0.0 };
            let gap = sorted[next] + turn - sorted[here];
            if gap < least - 1e-4 {
                let push = (least - gap) / 2.0;
                sorted[here] -= push;
                sorted[next] += push;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    let mut spread = vec![0.0; count];
    for (rank, &index) in order.iter().enumerate() {
        spread[index] = sorted[rank];
    }
    spread
}

/// Puts the tank that ended the run on the sheet, with a balloon for each of its parts.
///
/// It waits for the report to be on screen, and does nothing once the sheet is laid.
fn stage(
    mut commands: Commands,
    exhibits: Query<(), With<Exhibit>>,
    run: Option<Res<Run>>,
    paints: Res<Paints>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut view: ResMut<View>,
) {
    if !exhibits.is_empty() {
        return;
    }
    let Some(blow) = run.and_then(|run| run.lost_to) else {
        // Nothing to draw: the report stands on an empty sheet.
        exhibit::sheet(
            &mut commands,
            &mut meshes,
            &paints,
            &mut view,
            Screen::GameOver,
            SPAN,
        );
        return;
    };
    let show = Show {
        genome: blow.genome,
        color: color(&blow),
        span: SPAN,
        place: PLACE,
        turret: TURRET,
        sways: false,
        shield: false,
    };
    let stand = exhibit::put(
        &mut commands,
        &mut meshes,
        &paints,
        &mut view,
        Screen::GameOver,
        &show,
    );
    let anchors = anchors(&blow);
    let parts = anchors
        .iter()
        .map(|anchor| stand.point(anchor.on_turret, anchor.at))
        .collect();
    commands.insert_resource(Callouts { stand, parts });

    let piece = |piece: Piece, node: Node| {
        (
            piece,
            Node {
                position_type: PositionType::Absolute,
                ..node
            },
            UiTransform::IDENTITY,
            Visibility::Hidden,
        )
    };
    commands
        .spawn((
            DespawnOnExit(Screen::GameOver),
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|sheet| {
            // The leaders lie under the dots, and the dots under the rings.
            for index in 0..anchors.len() {
                let line = Node {
                    height: px(1),
                    ..default()
                };
                sheet.spawn((
                    piece(Piece::Leader(index), line),
                    BackgroundColor(theme::TEXT_DIM),
                ));
            }
            for index in 0..anchors.len() {
                let dot = Node {
                    width: px(DOT),
                    height: px(DOT),
                    border_radius: BorderRadius::MAX,
                    ..default()
                };
                sheet.spawn((piece(Piece::Dot(index), dot), BackgroundColor(theme::TEXT)));
            }
            for index in 0..anchors.len() {
                let ring = Node {
                    width: px(2.0 * BALLOON),
                    height: px(2.0 * BALLOON),
                    border: UiRect::all(px(1.5)),
                    border_radius: BorderRadius::MAX,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                };
                sheet.spawn((
                    piece(Piece::Ring(index), ring),
                    BorderColor::all(theme::TEXT),
                    BackgroundColor(theme::FLOOR),
                    children![theme::figure((index + 1).to_string(), 14.0, theme::TEXT)],
                ));
            }
        });
}

/// Where the pieces of one balloon go on the screen: the ends of its leader, from the ring
/// to the part.
struct Pointer {
    ring: Vec2,
    part: Vec2,
}

/// Lays out the balloons around the tank: each one stands on a circle, on the side of its
/// part, and no two stand on each other.
fn pointers(callouts: &Callouts, screen: &OnScreen) -> Option<Vec<Pointer>> {
    let middle = callouts.stand.point(false, Vec3::Y * models::TURRET_HEIGHT);
    let center = screen.point(middle)?;
    let radius = screen.point(middle + Vec3::X)?.distance(center) * CIRCLE;
    let mut parts = Vec::with_capacity(callouts.parts.len());
    for part in &callouts.parts {
        parts.push(screen.point(*part)?);
    }
    let sides: Vec<f32> = parts
        .iter()
        .map(|part| (*part - center).to_angle())
        .collect();
    let pointers = parts.iter().zip(spread(&sides, LEAST_GAP));
    Some(
        pointers
            .map(|(&part, side)| Pointer {
                ring: center + Vec2::from_angle(side) * radius,
                part,
            })
            .collect(),
    )
}

/// Keeps the balloons on the parts they point at.
fn point(
    screen: OnScreen,
    callouts: Option<Res<Callouts>>,
    mut pieces: Query<(&Piece, &mut Node, &mut UiTransform, &mut Visibility)>,
) {
    let pointers = callouts.and_then(|callouts| pointers(&callouts, &screen));
    for (piece, mut node, mut transform, mut visibility) in &mut pieces {
        let index = match *piece {
            Piece::Leader(index) | Piece::Dot(index) | Piece::Ring(index) => index,
        };
        let pointer = pointers.as_ref().and_then(|pointers| pointers.get(index));
        let wanted = if pointer.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        let Some(pointer) = pointer else {
            continue;
        };
        let corner = match *piece {
            Piece::Ring(_) => pointer.ring - Vec2::splat(BALLOON),
            Piece::Dot(_) => pointer.part - Vec2::splat(DOT / 2.0),
            Piece::Leader(_) => {
                // A line as long as the way from the ring to the part, turned around its
                // middle to lie along that way.
                let along = pointer.part - pointer.ring;
                let length = px(along.length().round());
                if node.width != length {
                    node.width = length;
                }
                let turn = Rot2::radians(along.to_angle());
                if transform.rotation != turn {
                    transform.rotation = turn;
                }
                (pointer.ring + pointer.part) / 2.0 - Vec2::new(along.length() / 2.0, 0.5)
            }
        };
        let held = Val2::px(corner.x.round(), corner.y.round());
        if transform.translation != held {
            transform.translation = held;
        }
    }
}

/// The game takes the view back.
fn put_away(mut commands: Commands, mut view: ResMut<View>) {
    commands.remove_resource::<Callouts>();
    view.release();
}

/// One row of the list of parts.
fn row(list: &mut ChildSpawnerCommands, number: usize, part: &Part) {
    let cell = |width: f32| Node {
        width: px(width),
        ..default()
    };
    list.spawn((
        Node {
            height: px(32),
            column_gap: px(10),
            align_items: AlignItems::Center,
            border: UiRect::bottom(px(1)),
            ..default()
        },
        BorderColor::all(theme::RULE_DIM),
        children![
            (
                cell(20.0),
                children![theme::figure(number.to_string(), 14.0, theme::TEXT)]
            ),
            (
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                children![theme::title(part.name.clone(), 15.0, theme::TEXT)],
            ),
            theme::text(part.gives.clone(), 16.0, theme::TEXT_DIM),
            (
                Node {
                    justify_content: JustifyContent::End,
                    ..cell(26.0)
                },
                children![theme::figure(part.cost.to_string(), 14.0, theme::TEXT)],
            ),
        ],
    ));
}

/// What the report says about the tank next to its drawing: what it was and how it did it,
/// then the list of its parts. They stand along the right edge of the sheet.
pub fn panels(sheet: &mut ChildSpawnerCommands, blow: &Blow) {
    let genome = &blow.genome;
    let points = format!("{} of {} points", genome.cost(), genome.budget);
    sheet.spawn((
        menu::ruled(Node {
            position_type: PositionType::Absolute,
            right: px(48),
            top: px(90),
            width: px(300),
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: UiRect::axes(px(14), px(12)),
            ..default()
        }),
        children![
            theme::label("Destroyed by", 12.0, theme::TEXT_DIM),
            theme::title(build(blow), 22.0, theme::TEXT),
            (
                Node {
                    column_gap: px(10),
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![
                    (
                        Node {
                            width: px(14),
                            height: px(14),
                            ..default()
                        },
                        BackgroundColor(color(blow)),
                    ),
                    theme::figure(points, 14.0, theme::TEXT_DIM),
                ],
            ),
            theme::text(means(blow), 17.0, theme::TEXT),
        ],
    ));

    let parts = parts(blow);
    // Two columns, read down the first one and then down the second.
    let half = parts.len().div_ceil(2);
    sheet
        .spawn(menu::ruled(Node {
            position_type: PositionType::Absolute,
            right: px(48),
            bottom: px(48),
            width: px(700),
            flex_direction: FlexDirection::Column,
            padding: UiRect::new(px(14), px(14), px(10), px(6)),
            ..default()
        }))
        .with_children(|list| {
            list.spawn((
                Node {
                    justify_content: JustifyContent::SpaceBetween,
                    margin: UiRect::bottom(px(4)),
                    ..default()
                },
                children![
                    theme::label("Parts and their level", 12.0, theme::TEXT_DIM),
                    theme::label("What they give, and their points", 12.0, theme::TEXT_DIM),
                ],
            ));
            list.spawn(Node {
                column_gap: px(28),
                ..default()
            })
            .with_children(|columns| {
                for (first, column) in [(1, &parts[..half]), (half + 1, &parts[half..])] {
                    columns
                        .spawn(Node {
                            flex_grow: 1.0,
                            flex_basis: px(0),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        })
                        .with_children(|list| {
                            for (place, part) in column.iter().enumerate() {
                                row(list, first + place, part);
                            }
                        });
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blow(team: Team, genome: Genome) -> Blow {
        Blow {
            weapon: Weapon::Bullet,
            genome,
            team,
        }
    }

    /// The burster of the specification: 4 m/s, 3 rounds, a soldier's brain.
    fn burster() -> Genome {
        let mut genome = Genome::empty(33);
        let levels = [
            (Stat::Engine, 3),
            (Stat::Tracks, 2),
            (Stat::Turret, 2),
            (Stat::Muzzle, 2),
            (Stat::Magazine, 2),
            (Stat::Loader, 3),
            (Stat::Brain, 2),
        ];
        for (stat, level) in levels {
            genome.set(stat, level);
        }
        genome
    }

    #[test]
    fn the_list_tells_each_part_with_what_it_gives_and_costs() {
        let listed = parts(&blow(Team::Enemies, burster()));
        let told: Vec<(&str, &str, u32)> = listed
            .iter()
            .map(|part| (part.name.as_str(), part.gives.as_str(), part.cost))
            .collect();
        assert_eq!(
            told,
            [
                ("Engine 3", "4 m/s", 6),
                ("Tracks 2", "180\u{b0}/s", 2),
                ("Turret 2", "90\u{b0}/s", 2),
                ("Muzzle 2", "9 m/s", 2),
                ("Magazine 2", "3 rounds", 6),
                ("Loader 3", "2 s", 4),
                ("Brain 2", "Soldier", 6),
            ]
        );
        // The list adds up to what the tank spent.
        let spent: u32 = listed.iter().map(|part| part.cost).sum();
        assert_eq!(spent, burster().cost());
    }

    #[test]
    fn the_list_of_a_player_s_tank_leaves_its_turret_out() {
        let listed = parts(&blow(Team::Players, Genome::PLAYER));
        assert!(listed.iter().all(|part| !part.name.starts_with("Turret")));
        let spent: u32 = listed.iter().map(|part| part.cost).sum();
        assert_eq!(spent, Genome::PLAYER.cost());
        // One number on the drawing for each row of the list.
        let own = blow(Team::Players, Genome::PLAYER);
        assert_eq!(anchors(&own).len(), listed.len());
    }

    #[test]
    fn numbers_are_written_without_idle_zeros() {
        assert_eq!(plain(4.0), "4");
        assert_eq!(plain(2.5), "2.5");
        assert_eq!(plain(1.25), "1.25");
        assert_eq!(plain(0.8), "0.8");
        assert_eq!(count(1, "round"), "1 round");
        assert_eq!(count(5, "round"), "5 rounds");
    }

    #[test]
    fn the_report_tells_what_the_tank_was_and_how_it_did_it() {
        let enemy = blow(Team::Enemies, burster());
        assert_eq!(build(&enemy), "Firepower build");
        assert_eq!(means(&enemy), "With a bullet");
        let own = Blow {
            weapon: Weapon::Mine,
            ..blow(Team::Players, Genome::PLAYER)
        };
        assert_eq!(build(&own), "Your own tank");
        assert_eq!(means(&own), "With a mine of your own");
        assert_eq!(color(&own), theme::PLAYER);
    }

    #[test]
    fn balloons_make_room_for_each_other() {
        let apart = |angles: &[f32]| {
            let mut sorted = angles.to_vec();
            sorted.sort_by(f32::total_cmp);
            let around = sorted[0] + TAU - sorted[sorted.len() - 1];
            let between = sorted.windows(2).map(|pair| pair[1] - pair[0]);
            between.fold(around, f32::min)
        };
        // Parts that are far apart keep their balloons where they are.
        let far = [0.0, 1.5, 3.0, -1.5];
        assert_eq!(spread(&far, LEAST_GAP), far);
        // Parts on the same side push their balloons apart, in the same order.
        let close = [0.5, 0.52, 0.48, -2.0, 0.51];
        let spread_out = spread(&close, LEAST_GAP);
        assert!(apart(&spread_out) > LEAST_GAP - 1e-3);
        assert!(spread_out[2] < spread_out[0] && spread_out[0] < spread_out[4]);
        assert!(spread_out[4] < spread_out[1]);
        assert_eq!(spread_out[3], -2.0);
        // A full tank has twelve parts, and its balloons all fit around it.
        let crowded: Vec<f32> = (0..12).map(|part| 0.01 * part as f32).collect();
        assert!(apart(&spread(&crowded, LEAST_GAP)) > LEAST_GAP - 1e-2);
    }
}
