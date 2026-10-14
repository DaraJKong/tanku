//! The report of a run that is over: the drawing of the tank that ended it.
//!
//! The tank is drawn as an assembly on the right of the sheet, its view titled over it with its
//! class, its points and what it did. Its parts are numbered by balloons on the drawing and
//! listed above the title block with what each one gives, so that the player knows that tank
//! the next time.

use bevy::prelude::*;
use tanku_core::{
    sim::{Blow, Run, Team, Weapon},
    stats::{Class, Genome, Stat},
};

use crate::{
    Screen,
    camera::{MainCamera, View},
    drawing,
    exhibit::{self, Exhibit, Show},
    hud::SHEET_EDGE,
    menu, models,
    scene::Paints,
    sheet::{self, FRAME_WIDTH},
    theme,
};

/// Meters of ground that the report shows from the bottom to the top of the screen when it
/// has no tank to draw.
const SPAN: f32 = 9.0;
/// Where the turret of the tank points, from the way its hull faces.
const TURRET: f32 = 0.45;
/// Where the right of the sheet starts from its top, and how high the title of the view is,
/// with the gap under it.
pub const VIEW_TOP: f32 = 56.0;
pub const VIEW_TITLE: f32 = 64.0;
/// The room that the view of the tank keeps, which the rows of its parts list make room for as
/// far as they can, from 32 px high down to 24.
const LEAST_VIEW: f32 = 220.0;
const ROWS: (f32, f32) = (32.0, 24.0);

/// How the right of the sheet of a report is laid out, for a sheet this large in the pixels of
/// the interface and a tank of this many parts: how high the rows of its parts list are, and
/// the room of its view, between its title and the parts list.
pub fn layout(sheet: Vec2, parts: usize) -> (f32, Rect) {
    let width = drawing::title_width_of(&menu::PART_COLUMNS);
    let bottom = sheet.y - SHEET_EDGE - drawing::title_height() + FRAME_WIDTH;
    let left_for_list = bottom - (VIEW_TOP + VIEW_TITLE + LEAST_VIEW);
    let row = (left_for_list / (parts + 1) as f32).clamp(ROWS.1, ROWS.0);
    let top = bottom - drawing::parts_height(parts, row) + FRAME_WIDTH;
    let right = sheet.x - SHEET_EDGE;
    let room = Rect::new(right - width, VIEW_TOP + VIEW_TITLE, right, top - 8.0);
    (row, room)
}

/// The title of the view of the tank that ended a run: what it was with its points, and what it
/// did, where and how.
pub fn view_title(run: &Run, blow: &Blow, solo: bool) -> (String, String) {
    let genome = &blow.genome;
    let what = format!(
        "{} \u{b7} {} of {} points",
        build(blow, solo),
        genome.cost(),
        genome.budget
    );
    let place = match (run.arena, run.sector, run.lull) {
        (false, 0, _) => "in the start bay".to_string(),
        (false, sector, _) => format!("in sector {sector}"),
        // Between two groups, a player has only their own shots and mines to fear.
        (true, cleared, Some(_)) => format!("before group {}", cleared + 1),
        (true, group, None) => format!("in group {group}"),
    };
    let who = if solo {
        "Destroyed you"
    } else {
        "Ended the run"
    };
    let did = format!("{who} {place}, {}", means(blow, solo).to_lowercase());
    (what, did)
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

/// The parts that a drawing names, each with the number of its balloon: the numbers of a parts
/// list, in its order.
pub fn numbered(anchors: Vec<models::Anchor>) -> Vec<(usize, models::Anchor)> {
    anchors
        .into_iter()
        .enumerate()
        .map(|(index, anchor)| (index + 1, anchor))
        .collect()
}

/// The parts of the tank behind a blow that the drawing points at, with their numbers.
fn anchors(blow: &Blow) -> Vec<(usize, models::Anchor)> {
    numbered(named_parts(&blow.genome, blow.team))
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
    (camera, ui_scale): (Single<&Camera, With<MainCamera>>, Res<UiScale>),
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
    let sheet = camera.logical_viewport_size().unwrap_or(sheet::LEAST) / ui_scale.0;
    let anchors = anchors(&blow);
    let (_, room) = layout(sheet, anchors.len());
    let (span, place) = menu::stage_in(room, sheet);
    let show = Show {
        genome: blow.genome,
        color: color(&blow),
        span,
        place,
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
    exhibit::call_out(&mut commands, (stand, room), &anchors, Screen::GameOver);
}

/// The game takes the view back.
fn put_away(mut commands: Commands, mut view: ResMut<View>) {
    commands.remove_resource::<exhibit::Callouts>();
    view.release();
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
        let listed = assembly(&burster(), Team::Enemies);
        let told: Vec<(&str, &str, &str, &str, &str)> = listed
            .iter()
            .map(|part| {
                let words = (
                    &part.part,
                    &part.stat,
                    &part.level,
                    &part.gives,
                    &part.points,
                );
                (
                    words.0.as_str(),
                    words.1.as_str(),
                    words.2.as_str(),
                    words.3.as_str(),
                    words.4.as_str(),
                )
            })
            .collect();
        assert_eq!(
            told,
            [
                ("Tracks", "Engine", "3", "4 m/s", "6"),
                ("Hull", "Tracks", "2", "180\u{b0}/s", "2"),
                ("Turret", "Turret", "2", "90\u{b0}/s", "2"),
                ("Barrel", "Muzzle", "2", "9 m/s", "3"),
                ("Drum", "Magazine", "2", "3 rounds", "6"),
                ("Vents", "Loader", "3", "2 s", "4"),
                ("Antennas", "Brain", "2", "Soldier", "6"),
            ]
        );
        // The list adds up to what the tank spent.
        let spent: u32 = listed
            .iter()
            .map(|part| part.points.parse::<u32>().unwrap())
            .sum();
        assert_eq!(spent, burster().cost());
    }

    #[test]
    fn the_list_of_a_player_s_tank_leaves_its_turret_out() {
        let listed = assembly(&Genome::PLAYER, Team::Players);
        assert!(listed.iter().all(|part| part.stat != "Turret"));
        let spent: u32 = listed
            .iter()
            .map(|part| part.points.parse::<u32>().unwrap())
            .sum();
        assert_eq!(spent, Genome::PLAYER.cost());
        // Its shield is told by what it costs to raise: the wait before the next time.
        let shield = listed.iter().find(|part| part.stat == "Shield").unwrap();
        assert_eq!(
            (shield.gives.as_str(), shield.points.as_str()),
            ("9 s cooldown", "9")
        );
        // One number on the drawing for each row of the list.
        let own = blow(Team::Players, Genome::PLAYER);
        assert_eq!(anchors(&own).len(), listed.len());
    }

    #[test]
    fn the_view_of_the_tank_is_titled_with_what_it_did() {
        let run = Run {
            seed_text: "TEST".into(),
            seed: 1,
            difficulty: default(),
            players: 1,
            team: 1,
            solo: true,
            sector: 6,
            cleared: 5,
            checkpoint: 5,
            lives: 0,
            kills: 0,
            ticks: 0,
            phase: tanku_core::sim::RunPhase::Over,
            lost_to: None,
            playtest: false,
            earns: true,
            free: false,
            arena: false,
            lull: None,
            paused: false,
        };
        let (what, did) = view_title(&run, &blow(Team::Enemies, burster()), true);
        assert_eq!(what, "All-rounder \u{b7} 29 of 34 points");
        assert_eq!(did, "Destroyed you in sector 6, with a bullet");
        let (_, did) = view_title(&run, &blow(Team::Enemies, burster()), false);
        assert_eq!(did, "Ended the run in sector 6, with a bullet");
    }

    #[test]
    fn the_view_of_the_tank_keeps_room_over_a_long_parts_list() {
        // On the least sheet, a tank with every part keeps a room for its view, over rows that
        // grow as thin as they get.
        let (row, room) = layout(sheet::LEAST, 15);
        assert_eq!(row, ROWS.1);
        assert!(room.height() >= 190.0, "{room:?}");
        // With fewer parts, the rows give the view the room that it keeps.
        let (row, room) = layout(sheet::LEAST, 11);
        assert!(row > ROWS.1 && row < ROWS.0);
        assert!((room.height() - (LEAST_VIEW - 8.0)).abs() < 4.0, "{room:?}");
        // A tank of a few parts has rows of their full height.
        let (row, _) = layout(sheet::LEAST, 6);
        assert_eq!(row, ROWS.0);
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
