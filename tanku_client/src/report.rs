//! The report of a run that is over: the drawing of the tank that ended it.
//!
//! The tank stands large on the sheet. Its parts are numbered on the drawing and listed next
//! to it with what each one gives, so that the player knows that tank the next time.

use bevy::prelude::*;
use tanku_core::{
    sim::{Blow, Run, Team, Weapon},
    stats::{Class, Genome, Stat},
};

use crate::{
    Screen,
    camera::View,
    drawing,
    exhibit::{self, Exhibit, Show},
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

/// A part of a tank, as the list tells it.
pub struct Part {
    /// The stat and its level.
    pub name: String,
    /// What that level gives.
    pub gives: String,
    /// The points it costs.
    pub cost: u32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(OnExit(Screen::GameOver), put_away)
        .add_systems(Update, stage.run_if(in_state(Screen::GameOver)));
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

pub fn stat_name(stat: Stat) -> &'static str {
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
        Stat::Seeking => "Seeking",
        Stat::Splitting => "Splitting",
        Stat::Exploding => "Exploding",
        Stat::Brain => "Brain",
        Stat::Ballistics => "Ballistics",
    }
}

/// The part of a tank that tells a stat, as its parts list names it.
pub fn part_name(stat: Stat) -> &'static str {
    match stat {
        Stat::Engine => "Tracks",
        Stat::Tracks => "Hull",
        Stat::Turret => "Turret",
        Stat::Muzzle => "Barrel",
        Stat::Magazine => "Drum",
        Stat::Loader => "Vents",
        Stat::Ricochet => "Rings",
        Stat::Mines => "Rack",
        Stat::Shield => "Disk",
        Stat::Piercing => "Tip",
        Stat::Seeking => "Chevrons",
        Stat::Splitting => "Fork",
        Stat::Exploding => "Bell",
        Stat::Brain => "Antennas",
        Stat::Ballistics => "Sight",
    }
}

/// The parts that a drawing of a tank names, in the order of their numbers: a player's turret
/// follows the cursor, so it tells nothing.
pub fn named_parts(genome: &Genome, team: Team) -> Vec<models::Anchor> {
    let mut anchors = models::anchors(genome);
    anchors.retain(|anchor| team == Team::Enemies || anchor.stat != Stat::Turret);
    anchors
}

/// The parts list of a tank drawn as an assembly, in the order of the numbers of its balloons.
pub fn assembly(genome: &Genome, team: Team) -> Vec<drawing::Part> {
    let listed = |anchor: &models::Anchor| {
        let stat = anchor.stat;
        drawing::Part {
            part: part_name(stat).to_string(),
            stat: stat_name(stat).to_string(),
            level: genome.level(stat).to_string(),
            gives: gives(genome, stat),
            points: cost(genome, stat).to_string(),
        }
    };
    named_parts(genome, team).iter().map(listed).collect()
}

/// What the level of a stat gives a tank, in the words of the specification.
pub fn gives(genome: &Genome, stat: Stat) -> String {
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
        Stat::Shield => match genome.shield_cooldown() {
            Some(cooldown) => format!("{} s cooldown", plain(cooldown)),
            None => "None".to_string(),
        },
        Stat::Piercing => count(genome.piercing(), "bullet"),
        // The three that a tank has or doesn't have say what its bullets do.
        Stat::Seeking | Stat::Splitting | Stat::Exploding if genome.level(stat) == 0 => {
            "No".to_string()
        }
        Stat::Seeking => "At each bounce".to_string(),
        Stat::Splitting => "In two, after 1 s".to_string(),
        Stat::Exploding => "Blast of 3 m".to_string(),
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
/// numbers.
fn anchors(blow: &Blow) -> Vec<models::Anchor> {
    named_parts(&blow.genome, blow.team)
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

/// The name of a class, as the players read it.
pub fn class_name(class: Class) -> &'static str {
    match class {
        Class::None => "All-rounder",
        Class::Sniper => "Sniper",
        Class::Scout => "Scout",
        Class::Tactician => "Tactician",
        Class::Trickster => "Trickster",
        Class::Lancer => "Lancer",
        Class::Sapper => "Sapper",
        Class::Gunner => "Gunner",
        Class::Seeker => "Seeker",
        Class::Flanker => "Flanker",
        Class::Grenadier => "Grenadier",
    }
}

/// What kind of tank it was: its class, which is what its hue tells. Among several players,
/// the run doesn't tell whose tank a player's tank was.
pub fn build(blow: &Blow, solo: bool) -> &'static str {
    match blow.team {
        Team::Players if solo => "Your own tank",
        Team::Players => "A player's tank",
        Team::Enemies => class_name(blow.genome.class),
    }
}

/// How the tank did it. A single player's own weapons are told as theirs.
pub fn means(blow: &Blow, solo: bool) -> &'static str {
    match (blow.team, blow.weapon) {
        (Team::Players, Weapon::Bullet) if solo => "With a bullet that bounced back",
        (Team::Players, Weapon::Mine) if solo => "With a mine of your own",
        (Team::Players, Weapon::Crate) if solo => "With a crate that you set off",
        (_, Weapon::Bullet) => "With a bullet",
        (_, Weapon::Mine) => "With a mine",
        (_, Weapon::Crate) => "With a crate that it set off",
    }
}

/// The color of the tank behind a blow.
pub fn color(blow: &Blow) -> Color {
    match blow.team {
        Team::Players => theme::PLAYER,
        Team::Enemies => models::enemy_color(&blow.genome),
    }
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
    exhibit::call_out(&mut commands, stand, &anchors(&blow), Screen::GameOver);
}

/// The game takes the view back.
fn put_away(mut commands: Commands, mut view: ResMut<View>) {
    commands.remove_resource::<exhibit::Callouts>();
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
                children![theme::old::figure(number.to_string(), 14.0, theme::TEXT)]
            ),
            (
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                children![theme::old::title(part.name.clone(), 15.0, theme::TEXT)],
            ),
            theme::old::text(part.gives.clone(), 16.0, theme::TEXT_DIM),
            (
                Node {
                    justify_content: JustifyContent::End,
                    ..cell(26.0)
                },
                children![theme::old::figure(part.cost.to_string(), 14.0, theme::TEXT)],
            ),
        ],
    ));
}

/// What the report says about the tank next to its drawing: what it was and how it did it,
/// then the list of its parts. They stand along the right edge of the sheet.
pub fn panels(sheet: &mut ChildSpawnerCommands, blow: &Blow, solo: bool) {
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
            theme::old::label("Destroyed by", 12.0, theme::TEXT_DIM),
            theme::old::title(build(blow, solo), 22.0, theme::TEXT),
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
                    theme::old::figure(points, 14.0, theme::TEXT_DIM),
                ],
            ),
            theme::old::text(means(blow, solo), 17.0, theme::TEXT),
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
                    theme::old::label("Parts and their level", 12.0, theme::TEXT_DIM),
                    theme::old::label("What they give, and their points", 12.0, theme::TEXT_DIM),
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
        let mut genome = Genome::empty(34);
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
                ("Muzzle 2", "9 m/s", 3),
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
        // Its shield is told by what it costs to raise: the wait before the next time.
        let shield = listed.iter().find(|part| part.name == "Shield 2").unwrap();
        assert_eq!((shield.gives.as_str(), shield.cost), ("9 s cooldown", 9));
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
        assert_eq!(build(&enemy, true), "All-rounder");
        let mut gunner = burster();
        gunner.class = Class::Gunner;
        assert_eq!(build(&blow(Team::Enemies, gunner), true), "Gunner");
        assert_eq!(means(&enemy, true), "With a bullet");
        assert_eq!(means(&enemy, false), "With a bullet");
        let own = Blow {
            weapon: Weapon::Mine,
            ..blow(Team::Players, Genome::PLAYER)
        };
        assert_eq!(build(&own, true), "Your own tank");
        assert_eq!(means(&own, true), "With a mine of your own");
        // Among several players, the tank may be a teammate's.
        assert_eq!(build(&own, false), "A player's tank");
        assert_eq!(means(&own, false), "With a mine");
        assert_eq!(color(&own), theme::PLAYER);
    }
}
