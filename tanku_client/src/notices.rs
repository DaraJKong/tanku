//! The messages of a run that must not be missed.
//!
//! A tank of the players that is destroyed, or that comes back, is told by an alert under the
//! tank that the player at this screen watches, where their eyes are: few words, no panel that
//! would hide a bullet, and gone in 2.2 s. The frame of the sheet flashes red with a loss, the
//! circle of the wreck is inked in red for a moment, the squares of the display blink, and a
//! loss that leaves one tank running, or one life, has a cue of its own.
//!
//! What a run earns, an achievement or a record, is stamped on the sheet above the tank, as the
//! game over screen stamps a run that is lost, with a cue and a green flash of the frame. It
//! comes with the last kill of a sector, when the fight is over. The routine news of a run stays
//! on the line at the top of the screen.

use bevy::prelude::*;
use tanku_audio::cues::Cue;
use tanku_core::{
    modes::Unlock,
    rules::{MAX_PLAYERS, REPAIR_RADIUS},
    sim::{Blow, Player, Playtest, Position, Run, SimEvent, Team, Weapon, Wreck},
};

use crate::{
    Screen,
    audio::{self, Sound},
    camera::{self, MainCamera},
    crew::Own,
    draft::{Draft, LIFT},
    hud::{self, SHEET_EDGE},
    report, scene,
    settings::{League, Records},
    theme::{self, Face},
    unlocks::{self, Unlocked},
};

/// How long an alert shows, and the end of that time over which it fades.
pub const ALERT_TIME: f32 = 2.2;
const ALERT_FADE: f32 = 0.4;
/// Two losses this close together are told by one alert.
const MERGE_TIME: f32 = 1.0;
/// How far under the middle of a tank its alert starts, in meters of ground: under the arc of
/// its rounds.
const ALERT_DROP: f32 = 2.65;
/// How far an alert keeps from the edges of the screen, in pixels of the interface: its middle
/// from the sides, and its top from the top and from the bottom.
const ALERT_ROOM: Vec2 = Vec2::new(240.0, 170.0);
/// How long a square of the display or a marker blinks for a loss, and each step of it.
pub const BLINK_TIME: f32 = 0.9;
const BLINK_STEP: f32 = 0.15;
/// The flash of the frame of the sheet: how strong it starts, how long it fades, and how wide
/// its line is.
const FLASH: f32 = 0.9;
const FLASH_TIME: f32 = 0.6;
const FLASH_WIDTH: f32 = 2.0;
/// The circle of a fresh wreck inked in red: how long it stays, and the ring that leaves it,
/// how long that takes and how much it grows.
const INK_TIME: f32 = 0.9;
const RING_TIME: f32 = 0.6;
const RING_GROWTH: f32 = 0.22;
/// The width of the line of that circle, in meters.
const INK_WIDTH: f32 = 0.1;
/// How long a stamp shows, how long it takes to land, and the end of its time over which it
/// fades.
const STAMP_TIME: f32 = 4.2;
const STAMP_LANDING: f32 = 0.15;
const STAMP_FADE: f32 = 0.5;
/// How much larger a stamp is as it comes down, and how far it is turned: as the stamp of the
/// game over screen.
const STAMP_DROP: f32 = 1.4;
const STAMP_TURN: f32 = -3.0;
/// Where the middle of the stamps stands, as a share of the height of the screen: above the
/// tank that the view follows, which is in the middle.
const STAMP_HEIGHT: f32 = 28.0;
/// Each stamp of a moment lands this long after the one before it.
const STAMP_STEP: f32 = 0.25;
/// How dark the inside of a stamp is: enough to read it over a block, little enough to see a
/// bullet through it.
const STAMP_FILL: f32 = 0.5;

/// The losses that the display tells for a moment: how long ago the tank of each seat was
/// destroyed, and which life of a single player was lost, with how long ago.
#[derive(Resource, Default)]
pub struct Alarms {
    pub lost: [Option<f32>; MAX_PLAYERS as usize],
    pub life: Option<(u32, f32)>,
}

impl Alarms {
    /// Whether something that blinks for a loss this long ago is lit at this moment, while it
    /// blinks.
    pub fn blink(since: Option<f32>) -> Option<bool> {
        let since = since.filter(|since| *since < BLINK_TIME)?;
        Some(((since / BLINK_STEP) as u32).is_multiple_of(2))
    }

    /// How long ago the tank of a seat was destroyed, if it was a moment ago.
    pub fn lost(&self, seat: u8) -> Option<f32> {
        self.lost.get(seat as usize).copied().flatten()
    }
}

/// Who is left alone to run, after a loss.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Last {
    /// The player at this screen.
    Me,
    /// A teammate, by seat.
    Mate(u8),
}

/// What an alert tells.
#[derive(Clone, Debug, PartialEq)]
enum Told {
    /// Tanks of teammates are destroyed, by seat, and the last of them lies there. A tank that
    /// is left alone to run is told too.
    Lost {
        seats: Vec<u8>,
        wreck: Vec2,
        last: Option<Last>,
    },
    /// The tank of a teammate is back, or the player's own.
    Back(Option<u8>),
    /// The player's own tank is destroyed: by what, if the run knows, and what is left. A single
    /// player has lives, which a playtest may not count. Among several players, the teammates
    /// whose tanks run can repair it.
    Own {
        blow: Option<Blow>,
        solo: bool,
        lives: Option<u32>,
        menders: Vec<u8>,
    },
}

/// Where an alert stands.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Anchor {
    /// Under the tank that the view follows, wherever it goes on the screen.
    Watched,
    /// Under the place on the screen where the player's own tank was destroyed. The view
    /// leaves that place, and the alert stays there.
    Pinned { ground: Vec2, screen: Option<Vec2> },
}

/// The alert on the screen, if there is one.
#[derive(Resource, Default)]
struct Shown(Option<Alert>);

struct Alert {
    told: Told,
    anchor: Anchor,
    age: f32,
    root: Entity,
}

/// How much of a message shows, from nothing to all of it.
#[derive(Component)]
struct Notice(f32);

/// A part of a message, with its colors when the message shows whole.
#[derive(Component, Clone, Copy)]
struct Inked {
    root: Entity,
    text: Option<Color>,
    back: Option<Color>,
    border: Option<Color>,
    outline: Option<Color>,
}

impl Inked {
    fn new(root: Entity) -> Self {
        Self {
            root,
            text: None,
            back: None,
            border: None,
            outline: None,
        }
    }

    fn text(root: Entity, color: Color) -> Self {
        Self {
            text: Some(color),
            ..Self::new(root)
        }
    }
}

/// The arrow of an alert, which points to the last wreck it tells of.
#[derive(Component)]
struct Pointer;

/// How far that wreck is, in an alert.
#[derive(Component)]
struct Distance;

/// The flash of the frame of the sheet: its color, and how long ago it started.
#[derive(Resource)]
struct Flash {
    color: Color,
    age: f32,
}

impl Default for Flash {
    fn default() -> Self {
        Self {
            color: theme::DANGER,
            age: FLASH_TIME,
        }
    }
}

/// The line that flashes over the frame of the sheet.
#[derive(Component)]
struct FlashFrame;

/// A ring drawn on the ground for a moment over the circle of a fresh wreck: one that stays
/// and fades, or one that leaves it as it grows.
#[derive(Component)]
struct Ink {
    age: f32,
    life: f32,
    grows: bool,
}

#[derive(Resource)]
struct Rings(Handle<Mesh>);

/// What a stamp says: what it is about, what was earned, and a word more.
#[derive(Clone, Debug, PartialEq)]
struct StampWords {
    about: String,
    earned: String,
    more: String,
}

/// The stamps of a moment, which come down one after the other, and those that wait for them to
/// be gone.
#[derive(Resource, Default)]
struct Stamps {
    waiting: Vec<StampWords>,
    /// The column that holds the stamps on the screen, how long ago the first one came, and
    /// how many there are.
    column: Option<(Entity, f32, usize)>,
}

/// A stamp on the screen, and how long after the first of its moment it comes.
#[derive(Component)]
struct StampCard(f32);

/// The record that a run started with, for its league and the size of its team, and whether
/// the run has gone past it yet.
#[derive(Resource, Default)]
struct Best {
    run: Option<(u64, u64)>,
    sector: Option<u32>,
    told: bool,
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Shown>()
        .init_resource::<Alarms>()
        .init_resource::<Flash>()
        .init_resource::<Stamps>()
        .init_resource::<Best>()
        .add_systems(Startup, setup)
        .add_systems(OnEnter(Screen::Playing), (build, forget))
        .add_systems(
            Update,
            (
                tell.before(audio::Playing),
                // An achievement is told in the frame that earns it, before the record that
                // comes with it.
                (earn.after(unlocks::earn), stamp)
                    .chain()
                    .before(audio::Playing),
                place.after(tell).after(camera::follow),
                fade.after(place).after(stamp),
                flash,
                ink,
            )
                .run_if(in_state(Screen::Playing)),
        );
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let mut ring = Draft::default();
    ring.ring(
        Vec3::ZERO,
        (REPAIR_RADIUS - INK_WIDTH, REPAIR_RADIUS),
        (1, 1.0),
        Color::WHITE,
    );
    commands.insert_resource(Rings(meshes.add(ring.build())));
}

/// The frame that flashes, over the fine line that frames the sheet.
fn build(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(Screen::Playing),
        FlashFrame,
        Node {
            position_type: PositionType::Absolute,
            left: px(SHEET_EDGE),
            right: px(SHEET_EDGE),
            top: px(SHEET_EDGE),
            bottom: px(SHEET_EDGE),
            border: UiRect::all(px(FLASH_WIDTH)),
            ..default()
        },
        BorderColor::all(Color::NONE),
        Pickable::IGNORE,
    ));
}

/// A run that starts has nothing to tell yet.
fn forget(
    (mut shown, mut alarms, mut flash): (ResMut<Shown>, ResMut<Alarms>, ResMut<Flash>),
    (mut stamps, mut best): (ResMut<Stamps>, ResMut<Best>),
) {
    shown.0 = None;
    *alarms = Alarms::default();
    *flash = Flash::default();
    *stamps = Stamps::default();
    *best = Best::default();
}

/// The cue of a loss of the players, by the tanks that still run after it: one more loss ends
/// the run when one is left. None is left when the run is over, which has a cue of its own.
fn loss_cue(running: usize) -> Option<Cue> {
    match running {
        0 => None,
        1 => Some(Cue::Last),
        _ => Some(Cue::Down),
    }
}

/// The cue of a single player's loss: the last life has one of its own.
fn life_cue(lives: Option<u32>) -> Option<Cue> {
    (lives == Some(1)).then_some(Cue::Last)
}

/// How a player is named in an alert.
fn named(seat: u8) -> String {
    hud::short_name(seat)
}

/// A tank of a class, with the article that its name takes.
fn one_of(blow: &Blow) -> String {
    let name = report::class_name(blow.genome.class).to_lowercase();
    let article = if name.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    format!("{article} {name}")
}

/// What destroyed a tank, said in a line. A single player's own weapons are told as theirs.
fn cause(blow: &Blow, solo: bool) -> String {
    let weapon = match blow.weapon {
        Weapon::Bullet => "bullet",
        Weapon::Mine => "mine",
        Weapon::Crate => "crate",
    };
    match (blow.team, blow.weapon) {
        (Team::Players, Weapon::Bullet) if solo => "By a bullet of your own".to_string(),
        (Team::Players, Weapon::Mine) if solo => "By a mine of your own".to_string(),
        (Team::Players, Weapon::Crate) if solo => "By a crate that you set off".to_string(),
        (Team::Players, Weapon::Crate) => "By a crate that a player set off".to_string(),
        (Team::Players, _) => format!("By the {weapon} of a player"),
        (Team::Enemies, Weapon::Crate) => format!("By a crate that {} set off", one_of(blow)),
        (Team::Enemies, _) => format!("By the {weapon} of {}", one_of(blow)),
    }
}

/// What is left to a single player, and in which color: nothing is said when the run is over.
fn lives_words(lives: u32) -> Option<(String, Color)> {
    match lives {
        0 => None,
        1 => Some(("Last life".to_string(), theme::DANGER)),
        lives => Some((format!("{lives} lives left"), theme::TEXT)),
    }
}

/// Who can repair a tank among several players: the teammates whose tanks run.
fn menders_words(menders: &[u8]) -> Option<String> {
    match menders {
        [] => None,
        [one] => Some(format!("{} can repair it", named(*one))),
        _ => Some("A teammate can repair it".to_string()),
    }
}

/// What an alert says of the tank that is left alone to run.
fn last_words(last: Last) -> String {
    match last {
        Last::Me => "Last tank running".to_string(),
        Last::Mate(seat) => format!("{} is the last tank", named(seat)),
    }
}

/// An alert of a loss that comes while another one shows, a moment after it, joins it.
fn merged(shown: &Told, age: f32, told: &Told) -> Option<Told> {
    let (
        Told::Lost { seats: before, .. },
        Told::Lost {
            seats, wreck, last, ..
        },
    ) = (shown, told)
    else {
        return None;
    };
    if age >= MERGE_TIME {
        return None;
    }
    let mut seats: Vec<u8> = before.iter().chain(seats).copied().collect();
    seats.sort();
    seats.dedup();
    Some(Told::Lost {
        seats,
        wreck: *wreck,
        last: *last,
    })
}

/// Tells what happened to the tanks of the players: it shows an alert, flashes the frame,
/// inks the circle of a fresh wreck, notes the loss for the display, and asks for its cue.
fn tell(
    mut commands: Commands,
    (run, playtest): (Option<Res<Run>>, Option<Res<Playtest>>),
    mut events: MessageReader<SimEvent>,
    wrecked: Query<(Entity, &Player, &Position, Has<Own>), Added<Wreck>>,
    mut repaired: RemovedComponents<Wreck>,
    players: Query<(&Player, Has<Own>, Has<Wreck>)>,
    (mut shown, mut alarms, mut flash): (ResMut<Shown>, ResMut<Alarms>, ResMut<Flash>),
    mut sounds: MessageWriter<Sound>,
    (rings, mut materials): (Res<Rings>, ResMut<Assets<StandardMaterial>>),
) {
    let Some(run) = run else {
        events.clear();
        repaired.clear();
        return;
    };
    let infinite_lives = playtest.is_some_and(|playtest| playtest.infinite_lives);
    // What there is to tell this frame, the last one first to be shown.
    let mut told: Vec<(Told, Anchor)> = Vec::new();
    let mut cue: Option<Cue> = None;
    // A single player's tank is gone when it is destroyed: the run says what is left.
    for event in events.read() {
        if let SimEvent::TankDestroyed {
            at,
            team: Team::Players,
        } = *event
            && run.solo
        {
            let lives = (!infinite_lives).then_some(run.lives);
            if let Some(lives) = lives {
                alarms.life = Some((lives + 1, 0.0));
            }
            cue = cue.or(life_cue(lives));
            let anchor = Anchor::Pinned {
                ground: at,
                screen: None,
            };
            let own = Told::Own {
                blow: run.lost_to,
                solo: true,
                lives,
                menders: Vec::new(),
            };
            told.push((own, anchor));
        }
    }
    // Among several players, a destroyed tank is a wreck.
    let running: Vec<(u8, bool)> = players
        .iter()
        .filter(|(.., wreck)| !wreck)
        .map(|(player, own, _)| (player.seat, own))
        .collect();
    let last = match running.as_slice() {
        [(_, true)] => Some(Last::Me),
        [(seat, false)] => Some(Last::Mate(*seat)),
        _ => None,
    };
    let mut lost: Vec<(u8, Vec2)> = Vec::new();
    let mut own_loss: Option<Vec2> = None;
    for (entity, player, position, own) in &wrecked {
        if let Some(since) = alarms.lost.get_mut(player.seat as usize) {
            *since = Some(0.0);
        }
        let ink = |life: f32, grows: bool, height: f32| {
            (
                ChildOf(entity),
                Ink {
                    age: 0.0,
                    life,
                    grows,
                },
                Mesh3d(rings.0.clone()),
                Transform::from_xyz(0.0, height * LIFT, 0.0),
            )
        };
        let paint = |materials: &mut Assets<StandardMaterial>| {
            materials.add(StandardMaterial {
                base_color: theme::DANGER,
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            })
        };
        commands.spawn((
            ink(INK_TIME, false, 7.0),
            MeshMaterial3d(paint(&mut materials)),
        ));
        commands.spawn((
            ink(RING_TIME, true, 8.0),
            MeshMaterial3d(paint(&mut materials)),
        ));
        if own {
            own_loss = Some(position.0);
        } else {
            lost.push((player.seat, position.0));
        }
    }
    if let Some(&(_, wreck)) = lost.last() {
        cue = cue.or(loss_cue(running.len()));
        let seats = lost.iter().map(|(seat, _)| *seat).collect();
        let told_lost = Told::Lost { seats, wreck, last };
        told.push((told_lost, Anchor::Watched));
    }
    // One's own loss is told before a teammate's that comes with it.
    if let Some(ground) = own_loss {
        let menders: Vec<u8> = running.iter().map(|(seat, _)| *seat).collect();
        if menders.len() == 1 {
            cue = cue.or(Some(Cue::Last));
        }
        let own = Told::Own {
            blow: run.lost_to,
            solo: false,
            lives: None,
            menders,
        };
        let anchor = Anchor::Pinned {
            ground,
            screen: None,
        };
        told.push((own, anchor));
    }
    if !told.is_empty() {
        *flash = Flash {
            color: theme::DANGER,
            age: 0.0,
        };
    }
    // A tank that is no wreck anymore, and still there, is back.
    for entity in repaired.read() {
        if let Ok((player, own, false)) = players.get(entity) {
            let back = Told::Back((!own).then_some(player.seat));
            told.push((back, Anchor::Watched));
        }
    }
    if let Some(cue) = cue {
        sounds.write(Sound(cue));
    }
    let Some((told, anchor)) = told.pop() else {
        return;
    };
    let (told, anchor) = match &shown.0 {
        Some(alert) => match merged(&alert.told, alert.age, &told) {
            Some(joined) => (joined, alert.anchor),
            None => (told, anchor),
        },
        None => (told, anchor),
    };
    if let Some(alert) = shown.0.take() {
        commands.entity(alert.root).despawn();
    }
    let root = spawn(&mut commands, &told);
    shown.0 = Some(Alert {
        told,
        anchor,
        age: 0.0,
        root,
    });
}

/// A row of the parts of an alert, side by side.
fn row(commands: &mut Commands, column: Entity, gap: f32) -> Entity {
    let row = Node {
        column_gap: px(gap),
        align_items: AlignItems::Center,
        ..default()
    };
    commands.spawn((ChildOf(column), row)).id()
}

/// Words of an alert, with their dark edge.
fn words(
    commands: &mut Commands,
    (parent, root): (Entity, Entity),
    (face, size): (Face, f32),
    content: impl Into<String>,
    color: Color,
) {
    let edged = theme::edged(Inked::text(root, color), face, content, size, color);
    commands.spawn((ChildOf(parent), edged));
}

/// The tag of a player in an alert: the one that stands over their tank, larger.
fn tag(commands: &mut Commands, (parent, root): (Entity, Entity), seat: u8) {
    let tag = commands
        .spawn((
            ChildOf(parent),
            Inked {
                border: Some(theme::PLAYER),
                back: Some(theme::VOID),
                ..Inked::new(root)
            },
            Node {
                border: UiRect::all(px(2)),
                padding: UiRect::new(px(8), px(5), px(0), px(0)),
                ..default()
            },
            BorderColor::all(theme::PLAYER),
            BackgroundColor(theme::VOID),
        ))
        .id();
    commands.spawn((
        ChildOf(tag),
        Inked::text(root, theme::TEXT),
        theme::title(named(seat), 22.0, theme::TEXT),
    ));
}

/// A small square of a color, with a dark edge: the color of the tank behind a blow, or a life.
fn square(
    commands: &mut Commands,
    (parent, root): (Entity, Entity),
    size: f32,
    (fill, border): (Color, Option<Color>),
) {
    let width = if border.is_some() { 1.0 } else { 0.0 };
    commands.spawn((
        ChildOf(parent),
        Inked {
            back: Some(fill),
            border,
            outline: Some(theme::VOID),
            ..Inked::new(root)
        },
        Node {
            width: px(size),
            height: px(size),
            border: UiRect::all(px(width)),
            ..default()
        },
        BackgroundColor(fill),
        BorderColor::all(border.unwrap_or(Color::NONE)),
        Outline::new(px(2), px(0), theme::VOID),
    ));
}

/// The arrow of an alert: a needle with its head, pointing up until it is turned toward a
/// wreck.
fn arrow(commands: &mut Commands, (parent, root): (Entity, Entity)) {
    let arrow = commands
        .spawn((
            ChildOf(parent),
            Pointer,
            Node {
                width: px(18),
                height: px(18),
                ..default()
            },
            UiTransform::IDENTITY,
        ))
        .id();
    let part = |node: Node, turn: f32| {
        (
            ChildOf(arrow),
            Inked {
                back: Some(theme::TEXT),
                outline: Some(theme::VOID),
                ..Inked::new(root)
            },
            Node {
                position_type: PositionType::Absolute,
                ..node
            },
            BackgroundColor(theme::TEXT),
            Outline::new(px(1.5), px(0), theme::VOID),
            UiTransform::from_rotation(Rot2::degrees(turn)),
        )
    };
    commands.spawn(part(
        Node {
            left: px(8),
            top: px(4),
            width: px(2),
            height: px(13),
            ..default()
        },
        0.0,
    ));
    commands.spawn(part(
        Node {
            left: px(5),
            top: px(1),
            width: px(8),
            height: px(8),
            ..default()
        },
        45.0,
    ));
}

/// Draws an alert, at no place yet: [`place`] puts it under its tank.
fn spawn(commands: &mut Commands, told: &Told) -> Entity {
    let root = commands
        .spawn((
            DespawnOnExit(Screen::Playing),
            Notice(1.0),
            Node {
                position_type: PositionType::Absolute,
                width: px(0),
                height: px(0),
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
        ))
        .id();
    let column = commands
        .spawn((
            ChildOf(root),
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(7),
                ..default()
            },
            // Its middle under the point it hangs from.
            UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
        ))
        .id();
    let heading = match told {
        Told::Back(_) => theme::ACCENT_HOVERED,
        _ => theme::DANGER,
    };
    let rule = heading.with_alpha(0.75);
    commands.spawn((
        ChildOf(column),
        Inked {
            back: Some(rule),
            ..Inked::new(root)
        },
        Node {
            width: px(168),
            height: px(1),
            ..default()
        },
        BackgroundColor(rule),
    ));
    let big = (Face::Title, 30.0);
    let line = (Face::Title, 16.0);
    let first = row(commands, column, 12.0);
    match told {
        Told::Lost { seats, last, .. } => {
            for seat in seats {
                tag(commands, (first, root), *seat);
            }
            words(commands, (first, root), big, "Destroyed", heading);
            let second = row(commands, column, 9.0);
            arrow(commands, (second, root));
            let distance = theme::edged(
                (Distance, Inked::text(root, theme::TEXT)),
                Face::Figure,
                "",
                17.0,
                theme::TEXT,
            );
            commands.spawn((ChildOf(second), distance));
            if let Some(last) = last {
                square(commands, (second, root), 3.0, (theme::TEXT_DIM, None));
                words(commands, (second, root), line, last_words(*last), heading);
            }
        }
        Told::Back(seat) => {
            match seat {
                Some(seat) => {
                    tag(commands, (first, root), *seat);
                    words(commands, (first, root), big, "Is back", heading);
                }
                None => words(commands, (first, root), big, "Your tank is back", heading),
            };
        }
        Told::Own {
            blow,
            solo,
            lives,
            menders,
        } => {
            let title = if *solo {
                "Tank destroyed"
            } else {
                "Your tank is destroyed"
            };
            words(commands, (first, root), big, title, heading);
            if let Some(blow) = blow {
                let second = row(commands, column, 9.0);
                square(commands, (second, root), 11.0, (report::color(blow), None));
                words(
                    commands,
                    (second, root),
                    line,
                    cause(blow, *solo),
                    theme::TEXT,
                );
            }
            if let Some((left, (said, color))) = lives.and_then(|n| Some((n, lives_words(n)?))) {
                let third = row(commands, column, 7.0);
                for life in 1..=tanku_core::rules::SOLO_LIVES {
                    let look = if life <= left {
                        (theme::PLAYER, None)
                    } else if life == left + 1 {
                        (theme::DANGER_FILL, Some(theme::DANGER))
                    } else {
                        (theme::VOID, Some(theme::RULE))
                    };
                    square(commands, (third, root), 14.0, look);
                }
                words(commands, (third, root), line, said, color);
            }
            if let Some(said) = menders_words(menders) {
                let third = row(commands, column, 7.0);
                words(commands, (third, root), line, said, theme::TEXT);
            }
        }
    }
    root
}

/// Where a point of the ground is on the screen, in pixels of the interface, and how long a
/// meter of ground is there.
fn on_screen(
    (camera, transform): (&Camera, &GlobalTransform),
    scale: f32,
    point: Vec3,
) -> Option<(Vec2, f32)> {
    let aside = point + transform.rotation() * Vec3::X;
    let here = camera.world_to_viewport(transform, point).ok()?;
    let there = camera.world_to_viewport(transform, aside).ok()?;
    Some((here / scale, here.distance(there) / scale))
}

/// The turn of an arrow that points up, to point from one place of the screen to another.
fn bearing(from: Vec2, to: Vec2) -> Rot2 {
    let way = to - from;
    if way.length_squared() < 1.0 {
        return Rot2::IDENTITY;
    }
    // The screen counts down: up is the way of negative y.
    Rot2::radians(way.x.atan2(-way.y))
}

/// Ages the alert, and puts it under the tank that the view follows, or where the player's own
/// tank was destroyed. An alert of a loss points to the wreck, and says how far it is.
fn place(
    time: Res<Time>,
    ui_scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    tanks: Query<(&Player, &GlobalTransform, Has<Own>, Has<Wreck>)>,
    mut shown: ResMut<Shown>,
    mut alarms: ResMut<Alarms>,
    mut roots: Query<(&mut Node, &mut Notice, &mut Visibility)>,
    mut pointers: Query<&mut UiTransform, With<Pointer>>,
    mut distances: Query<&mut Text, With<Distance>>,
    mut commands: Commands,
) {
    let seconds = time.delta_secs();
    for since in alarms.lost.iter_mut() {
        *since = since
            .map(|since| since + seconds)
            .filter(|since| *since < ALERT_TIME);
    }
    if let Some((life, since)) = alarms.life {
        alarms.life = Some((life, since + seconds)).filter(|(_, since)| *since < ALERT_TIME);
    }
    let Some(alert) = shown.0.as_mut() else {
        return;
    };
    alert.age += seconds;
    let Ok((mut node, mut notice, mut visibility)) = roots.get_mut(alert.root) else {
        shown.0 = None;
        return;
    };
    if alert.age >= ALERT_TIME {
        commands.entity(alert.root).despawn();
        shown.0 = None;
        return;
    }
    notice.0 = ((ALERT_TIME - alert.age) / ALERT_FADE).min(1.0);
    let (camera, transform) = *camera;
    let scale = ui_scale.0;
    // The tank that the view follows: the player's own while it runs, or else a teammate's.
    let own = tanks.iter().find(|(.., own, _)| *own);
    let watched = match own {
        Some((_, at, _, false)) => Some(at),
        _ => {
            let running = tanks.iter().filter(|(.., wreck)| !wreck);
            let teammate = running.min_by_key(|(player, ..)| player.seat);
            teammate.or(own).map(|(_, at, ..)| at)
        }
    };
    let watched = watched.map(|at| at.translation());
    let under = |point: Vec3| {
        on_screen((camera, transform), scale, point)
            .map(|(at, meter)| at + Vec2::Y * ALERT_DROP * meter)
    };
    let wanted = match &mut alert.anchor {
        Anchor::Watched => watched.and_then(under),
        Anchor::Pinned { ground, screen } => {
            if screen.is_none() {
                *screen = under(scene::to_world(*ground, 0.0));
            }
            *screen
        }
    };
    let Some(wanted) = wanted else {
        *visibility = Visibility::Hidden;
        return;
    };
    let size = camera.logical_viewport_size().unwrap_or(Vec2::ONE) / scale;
    let high = (size - ALERT_ROOM).max(ALERT_ROOM);
    let at = wanted
        .clamp(Vec2::new(ALERT_ROOM.x, ALERT_ROOM.y / 3.0), high)
        .round();
    (node.left, node.top) = (px(at.x), px(at.y));
    *visibility = Visibility::Inherited;
    // An alert of a loss points from the tank that the view follows to the last wreck.
    if let (Told::Lost { wreck, .. }, Some(from)) = (&alert.told, watched) {
        let wreck = scene::to_world(*wreck, 0.0);
        let ends = (
            on_screen((camera, transform), scale, from),
            on_screen((camera, transform), scale, wreck),
        );
        if let (Some((from_at, _)), Some((to_at, _))) = ends {
            for mut pointer in &mut pointers {
                pointer.rotation = bearing(from_at, to_at);
            }
        }
        let meters = scene::to_ground(from)
            .distance(scene::to_ground(wreck))
            .round();
        for mut text in &mut distances {
            let said = format!("{meters} m");
            if text.0 != said {
                text.0 = said;
            }
        }
    }
}

/// Fades the parts of the messages with their messages.
fn fade(
    notices: Query<&Notice>,
    mut parts: Query<(
        &Inked,
        Option<&mut TextColor>,
        Option<&mut BackgroundColor>,
        Option<&mut BorderColor>,
        Option<&mut Outline>,
    )>,
) {
    for (inked, text, back, border, outline) in &mut parts {
        let shown = notices.get(inked.root).map_or(0.0, |notice| notice.0);
        let faded = |color: Color| color.with_alpha(color.alpha() * shown);
        if let (Some(color), Some(mut text)) = (inked.text, text) {
            text.set_if_neq(TextColor(faded(color)));
        }
        if let (Some(color), Some(mut back)) = (inked.back, back) {
            back.set_if_neq(BackgroundColor(faded(color)));
        }
        if let (Some(color), Some(mut border)) = (inked.border, border) {
            border.set_if_neq(BorderColor::all(faded(color)));
        }
        if let (Some(color), Some(mut outline)) = (inked.outline, outline) {
            outline.color = faded(color);
        }
    }
}

/// Flashes the frame of the sheet, and lets the flash fade.
fn flash(
    time: Res<Time>,
    mut flash: ResMut<Flash>,
    mut frame: Single<&mut BorderColor, With<FlashFrame>>,
) {
    flash.age += time.delta_secs();
    let left = (1.0 - flash.age / FLASH_TIME).max(0.0);
    let wanted = BorderColor::all(flash.color.with_alpha(FLASH * left * left));
    frame.set_if_neq(wanted);
}

/// Fades the rings over the circle of a fresh wreck, grows the one that leaves it, and takes
/// them away.
fn ink(
    mut commands: Commands,
    time: Res<Time>,
    mut rings: Query<(
        Entity,
        &mut Ink,
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut ink, mut transform, material) in &mut rings {
        ink.age += time.delta_secs();
        let progress = ink.age / ink.life;
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let alpha = if ink.grows {
            transform.scale = Vec3::splat(1.0 + RING_GROWTH * (1.0 - (1.0 - progress).powi(2)));
            1.0 - progress
        } else {
            // It stays whole, then fades over the last part of its time.
            ((1.0 - progress) / 0.4).min(1.0)
        };
        if let Some(mut paint) = materials.get_mut(&material.0) {
            paint.base_color.set_alpha(alpha);
        }
    }
}

/// The words of the stamp of an achievement: the sector that earned it, what it unlocks, and
/// where to find it. The host of a game has every option already: an achievement is for solo.
fn unlock_words(unlock: Unlock, solo: bool) -> StampWords {
    let hard = if unlock.in_hard_mode() {
        " in hard mode"
    } else {
        ""
    };
    let more = if !unlocks::built(unlock) {
        "Not in the game yet"
    } else if solo {
        "New on the solo screen"
    } else {
        "For your solo runs"
    };
    StampWords {
        about: format!(
            "Achievement \u{b7} sector {} cleared{hard}",
            unlock.sector()
        ),
        earned: format!("{} unlocked", unlocks::name(unlock)),
        more: more.to_string(),
    }
}

/// The words of the stamp of a record: its league, and how far past the best the run is.
fn record_words(run: &Run, best: u32) -> StampWords {
    let league = League::of(run);
    let team = match run.team {
        1 => "Solo".to_string(),
        players => format!("{players} players"),
    };
    let kind = league.words().unwrap_or_else(|| "normal".to_string());
    StampWords {
        about: format!("{team} \u{b7} {kind}"),
        earned: "New record".to_string(),
        more: format!(
            "{} {}s cleared \u{b7} the best was {best}",
            run.cleared,
            league.unit()
        ),
    }
}

/// Whether a run is past the record that it started with. A first run has none to beat: every
/// sector it clears would be one.
fn past(best: Option<u32>, cleared: u32) -> bool {
    best.is_some_and(|best| cleared > best)
}

/// Finds what a run earns: each achievement that it unlocks, and the first sector that it
/// clears past its record. A playtest keeps no record.
fn earn(
    run: Option<Res<Run>>,
    records: Res<Records>,
    mut unlocked: MessageReader<Unlocked>,
    mut best: ResMut<Best>,
    mut stamps: ResMut<Stamps>,
) {
    let Some(run) = run else {
        unlocked.clear();
        return;
    };
    // A new run, or the same seed again, starts from the record as it stands.
    let this = (run.seed, run.ticks);
    let new = best
        .run
        .is_none_or(|(seed, ticks)| seed != this.0 || ticks > this.1);
    best.run = Some(this);
    if new {
        let record = records.best(League::of(&run), run.team);
        best.sector = record.map(|record| record.sector);
        best.told = false;
    }
    for Unlocked(unlock) in unlocked.read() {
        stamps.waiting.push(unlock_words(*unlock, run.solo));
    }
    if !best.told && !run.playtest && past(best.sector, run.cleared) {
        best.told = true;
        let words = record_words(&run, best.sector.unwrap_or_default());
        stamps.waiting.push(words);
    }
}

/// A stamp, which comes down in its column after this many seconds.
fn card(commands: &mut Commands, column: Entity, said: &StampWords, delay: f32) {
    let fill = theme::VOID.with_alpha(STAMP_FILL);
    let card = commands
        .spawn((
            ChildOf(column),
            StampCard(delay),
            Notice(0.0),
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(3),
                border: UiRect::all(px(2)),
                padding: UiRect::new(px(30), px(26), px(10), px(14)),
                ..default()
            },
            BorderColor::all(theme::GOOD),
            BackgroundColor(fill),
            UiTransform {
                scale: Vec2::splat(STAMP_DROP),
                rotation: Rot2::degrees(STAMP_TURN),
                ..UiTransform::IDENTITY
            },
        ))
        .id();
    commands.entity(card).insert(Inked {
        back: Some(fill),
        border: Some(theme::GOOD),
        ..Inked::new(card)
    });
    let parts = (card, card);
    words(
        commands,
        parts,
        (Face::Label, 13.0),
        &said.about,
        theme::GOOD,
    );
    words(
        commands,
        parts,
        (Face::Title, 40.0),
        &said.earned,
        theme::GOOD,
    );
    words(
        commands,
        parts,
        (Face::Label, 15.0),
        &said.more,
        theme::TEXT,
    );
}

/// Brings the stamps down: what waits comes once the stamps on the screen are gone, all of it
/// at once, or joins them if they have only just come. Each moment of stamps has its cue and
/// flashes the frame green.
fn stamp(
    mut commands: Commands,
    time: Res<Time>,
    mut stamps: ResMut<Stamps>,
    mut flash: ResMut<Flash>,
    mut sounds: MessageWriter<Sound>,
    mut cards: Query<(&StampCard, &mut Notice, &mut UiTransform)>,
) {
    let seconds = time.delta_secs();
    if let Some((column, age, count)) = stamps.column {
        let age = age + seconds;
        if age >= STAMP_TIME + STAMP_STEP * count.saturating_sub(1) as f32 {
            commands.entity(column).despawn();
            stamps.column = None;
        } else {
            stamps.column = Some((column, age, count));
        }
    }
    if !stamps.waiting.is_empty() {
        let waiting = std::mem::take(&mut stamps.waiting);
        match stamps.column {
            None => {
                let column = commands
                    .spawn((
                        DespawnOnExit(Screen::Playing),
                        Node {
                            position_type: PositionType::Absolute,
                            left: percent(50),
                            top: percent(STAMP_HEIGHT),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: px(14),
                            ..default()
                        },
                        // Its middle on that point.
                        UiTransform::from_translation(Val2::percent(-50.0, -50.0)),
                        Pickable::IGNORE,
                    ))
                    .id();
                for (index, words) in waiting.iter().enumerate() {
                    card(&mut commands, column, words, STAMP_STEP * index as f32);
                }
                stamps.column = Some((column, 0.0, waiting.len()));
                sounds.write(Sound(Cue::Stamp));
                *flash = Flash {
                    color: theme::GOOD,
                    age: 0.0,
                };
            }
            Some((column, age, count)) if age < STAMP_STEP => {
                for (index, words) in waiting.iter().enumerate() {
                    let delay = STAMP_STEP * (count + index) as f32;
                    card(&mut commands, column, words, delay);
                }
                stamps.column = Some((column, age, count + waiting.len()));
            }
            Some(_) => stamps.waiting = waiting,
        }
    }
    let age = stamps.column.map_or(0.0, |(_, age, _)| age);
    for (card, mut notice, mut transform) in &mut cards {
        let (shown, scale) = landing(age - card.0);
        notice.0 = shown;
        transform.scale = Vec2::splat(scale);
    }
}

/// How much of a stamp shows, and how large it is, this long after it starts to come down: it
/// lands, holds, and fades.
fn landing(since: f32) -> (f32, f32) {
    if since < 0.0 {
        (0.0, STAMP_DROP)
    } else if since < STAMP_LANDING {
        let landed = since / STAMP_LANDING;
        (landed, STAMP_DROP - (STAMP_DROP - 1.0) * landed)
    } else {
        (((STAMP_TIME - since) / STAMP_FADE).clamp(0.0, 1.0), 1.0)
    }
}

#[cfg(test)]
mod tests {
    use tanku_core::{
        sim::RunPhase,
        stats::{Class, Genome},
    };

    use super::*;

    fn blow(team: Team, weapon: Weapon, class: Class) -> Blow {
        let mut genome = Genome::PLAYER;
        genome.class = class;
        Blow {
            weapon,
            genome,
            team,
        }
    }

    #[test]
    fn an_alert_says_what_destroyed_a_tank() {
        let enemy = |weapon, class| cause(&blow(Team::Enemies, weapon, class), true);
        assert_eq!(
            enemy(Weapon::Bullet, Class::Trickster),
            "By the bullet of a trickster"
        );
        assert_eq!(
            enemy(Weapon::Mine, Class::Sapper),
            "By the mine of a sapper"
        );
        assert_eq!(
            enemy(Weapon::Crate, Class::Gunner),
            "By a crate that a gunner set off"
        );
        // A class whose name starts with a vowel takes its article.
        assert_eq!(
            enemy(Weapon::Bullet, Class::None),
            "By the bullet of an all-rounder"
        );
        // A single player's own weapons are told as theirs, and among several players, the
        // run doesn't know whose they were.
        let player = |weapon, solo| cause(&blow(Team::Players, weapon, Class::None), solo);
        assert_eq!(player(Weapon::Bullet, true), "By a bullet of your own");
        assert_eq!(player(Weapon::Crate, true), "By a crate that you set off");
        assert_eq!(player(Weapon::Mine, false), "By the mine of a player");
        assert_eq!(
            player(Weapon::Crate, false),
            "By a crate that a player set off"
        );
    }

    #[test]
    fn an_alert_says_what_is_left() {
        assert_eq!(
            lives_words(2),
            Some(("2 lives left".to_string(), theme::TEXT))
        );
        assert_eq!(
            lives_words(1),
            Some(("Last life".to_string(), theme::DANGER))
        );
        // A run that is over says no more.
        assert_eq!(lives_words(0), None);
        assert_eq!(menders_words(&[]), None);
        assert_eq!(menders_words(&[1]).as_deref(), Some("P2 can repair it"));
        assert_eq!(
            menders_words(&[1, 3]).as_deref(),
            Some("A teammate can repair it")
        );
        assert_eq!(last_words(Last::Me), "Last tank running");
        assert_eq!(last_words(Last::Mate(2)), "P3 is the last tank");
    }

    #[test]
    fn a_loss_that_leaves_one_tank_running_has_the_cue_of_the_last() {
        assert_eq!(loss_cue(3), Some(Cue::Down));
        assert_eq!(loss_cue(2), Some(Cue::Down));
        assert_eq!(loss_cue(1), Some(Cue::Last));
        // The end of a run has its own.
        assert_eq!(loss_cue(0), None);
        assert_eq!(life_cue(Some(2)), None);
        assert_eq!(life_cue(Some(1)), Some(Cue::Last));
        assert_eq!(life_cue(Some(0)), None);
        // A playtest that doesn't count lives has no last one.
        assert_eq!(life_cue(None), None);
    }

    #[test]
    fn losses_a_moment_apart_are_one_alert() {
        let lost = |seats: Vec<u8>, last| Told::Lost {
            seats,
            wreck: Vec2::new(3.0, 0.0),
            last,
        };
        let first = lost(vec![2], None);
        let second = lost(vec![0], Some(Last::Me));
        // Within a second, the alert names both, and says what the second one left.
        assert_eq!(
            merged(&first, 0.5, &second),
            Some(lost(vec![0, 2], Some(Last::Me)))
        );
        // Later, the second has an alert of its own.
        assert_eq!(merged(&first, MERGE_TIME, &second), None);
        // Other alerts never join.
        assert_eq!(merged(&Told::Back(Some(1)), 0.1, &second), None);
    }

    #[test]
    fn what_blinks_for_a_loss_blinks_for_a_moment() {
        assert_eq!(Alarms::blink(None), None);
        assert_eq!(Alarms::blink(Some(0.0)), Some(true));
        assert_eq!(Alarms::blink(Some(BLINK_STEP * 1.5)), Some(false));
        assert_eq!(Alarms::blink(Some(BLINK_STEP * 2.5)), Some(true));
        assert_eq!(Alarms::blink(Some(BLINK_TIME)), None);
    }

    #[test]
    fn an_arrow_points_the_way_to_a_wreck() {
        let turned = |to: Vec2| bearing(Vec2::ZERO, to).as_degrees().round();
        // Up the screen, the way it is drawn, then to the right, down and to the left.
        assert_eq!(turned(Vec2::new(0.0, -10.0)), 0.0);
        assert_eq!(turned(Vec2::new(10.0, 0.0)), 90.0);
        assert_eq!(turned(Vec2::new(0.0, 10.0)).abs(), 180.0);
        assert_eq!(turned(Vec2::new(-10.0, 0.0)), -90.0);
    }

    /// A run of several players with the tanks of these seats, the one of the first seat being
    /// the player's own: the alerts of a run as far as they go without a screen.
    fn game(seats: &[u8]) -> (App, Vec<Entity>) {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Shown>()
            .init_resource::<Alarms>()
            .init_resource::<Flash>()
            .add_message::<SimEvent>()
            .add_message::<Sound>()
            .insert_resource(Run {
                seed_text: "TEST".into(),
                seed: 1,
                difficulty: default(),
                players: seats.len() as u32,
                team: seats.len() as u32,
                solo: seats.len() == 1,
                sector: 3,
                cleared: 2,
                checkpoint: 2,
                lives: 3,
                kills: 0,
                ticks: 0,
                phase: RunPhase::Playing,
                lost_to: None,
                playtest: false,
                earns: true,
                free: false,
                arena: false,
                lull: None,
                paused: false,
            })
            .add_systems(Startup, setup)
            .add_systems(Update, tell);
        let tanks = seats
            .iter()
            .map(|&seat| {
                let tank = (Player { seat }, Position(Vec2::new(seat as f32 * 5.0, 0.0)));
                let mut tank = app.world_mut().spawn(tank);
                if seat == seats[0] {
                    tank.insert(Own);
                }
                tank.id()
            })
            .collect();
        app.update();
        (app, tanks)
    }

    /// The cues asked for since the last time, and what the alert tells.
    fn heard(app: &mut App) -> (Vec<Cue>, Option<Told>) {
        let world = app.world_mut();
        let mut asked = world.resource_mut::<Messages<Sound>>();
        let sounds: Vec<Cue> = asked.drain().map(|Sound(cue)| cue).collect();
        let told = world
            .resource::<Shown>()
            .0
            .as_ref()
            .map(|alert| alert.told.clone());
        (sounds, told)
    }

    #[test]
    fn the_loss_of_a_teammate_is_told_with_its_cue() {
        let (mut app, tanks) = game(&[0, 1, 2]);
        assert_eq!(heard(&mut app), (vec![], None));
        // Player 3's tank is destroyed: two tanks still run.
        app.world_mut()
            .entity_mut(tanks[2])
            .insert(Wreck::default());
        app.update();
        let lost = |seats: Vec<u8>, last| Told::Lost {
            seats,
            wreck: Vec2::new(10.0, 0.0),
            last,
        };
        assert_eq!(
            heard(&mut app),
            (vec![Cue::Down], Some(lost(vec![2], None)))
        );
        assert_eq!(app.world().resource::<Alarms>().lost(2), Some(0.0));
        assert_eq!(app.world().resource::<Flash>().age, 0.0);
        // Then player 2's: the player at this screen is the last tank that runs.
        app.world_mut()
            .entity_mut(tanks[1])
            .insert(Wreck::default());
        app.update();
        let (cues, told) = heard(&mut app);
        assert_eq!(cues, [Cue::Last]);
        let Some(Told::Lost { seats, last, .. }) = told else {
            panic!("{told:?}");
        };
        // A moment apart, the alert names both.
        assert_eq!((seats, last), (vec![1, 2], Some(Last::Me)));
        // Repaired, player 2's tank is back.
        app.world_mut().entity_mut(tanks[1]).remove::<Wreck>();
        app.update();
        assert_eq!(heard(&mut app), (vec![], Some(Told::Back(Some(1)))));
    }

    #[test]
    fn one_s_own_loss_says_who_can_repair_it() {
        let (mut app, tanks) = game(&[0, 1]);
        app.world_mut()
            .entity_mut(tanks[0])
            .insert(Wreck::default());
        app.update();
        let (cues, told) = heard(&mut app);
        // The teammate is the last tank that runs.
        assert_eq!(cues, [Cue::Last]);
        let Some(Told::Own { solo, menders, .. }) = told else {
            panic!("{told:?}");
        };
        assert!(!solo);
        assert_eq!(menders, [1]);
        // Repaired, the player's own tank is back.
        app.world_mut().entity_mut(tanks[0]).remove::<Wreck>();
        app.update();
        assert_eq!(heard(&mut app), (vec![], Some(Told::Back(None))));
    }

    #[test]
    fn a_single_player_s_loss_says_what_is_left() {
        let (mut app, _) = game(&[0]);
        let lost = SimEvent::TankDestroyed {
            at: Vec2::ZERO,
            team: Team::Players,
        };
        // The run has taken a life already when its event is read.
        app.world_mut().resource_mut::<Run>().lives = 1;
        app.world_mut().write_message(lost);
        app.update();
        let (cues, told) = heard(&mut app);
        assert_eq!(cues, [Cue::Last]);
        let Some(Told::Own { solo, lives, .. }) = told else {
            panic!("{told:?}");
        };
        assert_eq!((solo, lives), (true, Some(1)));
        // The square of the second life blinks on the display.
        assert_eq!(app.world().resource::<Alarms>().life, Some((2, 0.0)));
        // An enemy's tank is no loss.
        let enemy = SimEvent::TankDestroyed {
            at: Vec2::ZERO,
            team: Team::Enemies,
        };
        app.world_mut().write_message(enemy);
        app.update();
        assert_eq!(heard(&mut app).0, []);
    }

    #[test]
    fn an_achievement_is_stamped_with_where_to_find_it() {
        let solo = unlock_words(Unlock::ArenaMode, true);
        assert_eq!(solo.about, "Achievement \u{b7} sector 10 cleared");
        assert_eq!(solo.earned, "Arena mode unlocked");
        assert_eq!(solo.more, "New on the solo screen");
        // Among several players, the host has every option already.
        assert_eq!(
            unlock_words(Unlock::FreeMode, false).more,
            "For your solo runs"
        );
        // What the game doesn't have yet is said so.
        let impossible = unlock_words(Unlock::ImpossibleMode, true);
        assert!(impossible.about.ends_with("in hard mode"));
        assert_eq!(impossible.more, "Not in the game yet");
    }

    #[test]
    fn a_stamp_lands_holds_and_fades() {
        assert_eq!(landing(-0.1), (0.0, STAMP_DROP));
        assert_eq!(landing(0.0), (0.0, STAMP_DROP));
        let (shown, scale) = landing(STAMP_LANDING / 2.0);
        assert!((shown - 0.5).abs() < 1e-5 && scale < STAMP_DROP && scale > 1.0);
        assert_eq!(landing(STAMP_LANDING), (1.0, 1.0));
        assert_eq!(landing(STAMP_TIME - STAMP_FADE), (1.0, 1.0));
        assert_eq!(landing(STAMP_TIME), (0.0, 1.0));
    }

    /// A run with the records of this machine, as far as its stamps go without a screen.
    fn earning(records: Records) -> App {
        let mut app = App::new();
        app.init_resource::<Stamps>()
            .init_resource::<Best>()
            .init_resource::<Flash>()
            .init_resource::<Time>()
            .insert_resource(records)
            .add_message::<Unlocked>()
            .add_message::<Sound>()
            .insert_resource(Run {
                seed_text: "TEST".into(),
                seed: 1,
                difficulty: default(),
                players: 1,
                team: 1,
                solo: true,
                sector: 3,
                cleared: 2,
                checkpoint: 2,
                lives: 3,
                kills: 0,
                ticks: 0,
                phase: RunPhase::Playing,
                lost_to: None,
                playtest: false,
                earns: true,
                free: false,
                arena: false,
                lull: None,
                paused: false,
            })
            .add_systems(Update, (earn, stamp).chain());
        app
    }

    /// What the stamps say, those on the screen and those that wait, and the cues asked for.
    fn stamped(app: &mut App) -> (usize, Vec<StampWords>, Vec<Cue>) {
        let world = app.world_mut();
        let shown = world.query::<&StampCard>().iter(world).count();
        let waiting = world.resource::<Stamps>().waiting.clone();
        let mut asked = world.resource_mut::<Messages<Sound>>();
        let cues: Vec<Cue> = asked.drain().map(|Sound(cue)| cue).collect();
        (shown, waiting, cues)
    }

    #[test]
    fn a_record_is_stamped_once_past_the_best_that_the_run_started_with() {
        let mut records = Records::default();
        records.submit(League::default(), 1, 3, "OLD");
        let mut app = earning(records);
        app.update();
        assert_eq!(stamped(&mut app), (0, vec![], vec![]));
        // Level with the record is no record.
        app.world_mut().resource_mut::<Run>().cleared = 3;
        app.update();
        assert_eq!(stamped(&mut app).0, 0);
        // Past it, the stamp comes with its cue, and says by how much.
        app.world_mut().resource_mut::<Run>().cleared = 4;
        app.update();
        assert_eq!(stamped(&mut app), (1, vec![], vec![Cue::Stamp]));
        let words = record_words(app.world().resource::<Run>(), 3);
        assert_eq!(words.about, "Solo \u{b7} normal");
        assert_eq!(words.more, "4 sectors cleared \u{b7} the best was 3");
        // Once a run.
        app.world_mut().resource_mut::<Run>().cleared = 5;
        app.update();
        assert_eq!(stamped(&mut app), (1, vec![], vec![]));
        // An achievement in the same moment joins the stamp on the screen.
        app.world_mut().write_message(Unlocked(Unlock::ArenaMode));
        app.update();
        assert_eq!(stamped(&mut app), (2, vec![], vec![]));
    }

    #[test]
    fn a_first_run_and_a_playtest_beat_no_record() {
        assert!(!past(None, 7));
        assert!(past(Some(6), 7) && !past(Some(7), 7));
        let mut app = earning(Records::default());
        app.world_mut().resource_mut::<Run>().cleared = 9;
        app.update();
        assert_eq!(stamped(&mut app), (0, vec![], vec![]));
        let mut records = Records::default();
        records.submit(League::default(), 1, 3, "OLD");
        let mut app = earning(records);
        app.world_mut().resource_mut::<Run>().playtest = true;
        app.world_mut().resource_mut::<Run>().cleared = 9;
        app.update();
        assert_eq!(stamped(&mut app), (0, vec![], vec![]));
    }
}
