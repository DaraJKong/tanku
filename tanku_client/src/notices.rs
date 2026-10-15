//! The messages of a run that must not be missed.
//!
//! A tank of the players that is destroyed, or that comes back, is told by an alert where the
//! player's eyes are, shaped as the leader note of a drawing (ISO 128-22): a leader from a dot
//! on what it tells of, or from the balloon of a teammate, to a reference line with the words
//! on it and under it. Few words, no panel that would hide a bullet, and gone in 2.2 s. The
//! frame of the sheet flashes red with a loss, the circle of the wreck is inked in red for a
//! moment, the balloons and the squares of the display tell it too, and a loss that leaves one
//! tank running, or one life, has a cue of its own.
//!
//! What a run earns, an achievement or a record, is stamped on the sheet above the tank, as a
//! drawing that is released is stamped: a double frame, turned a little, with the day of its
//! release. It comes with a cue and a green flash of the frame, with the last kill of a sector,
//! when the fight is over. The routine news of a run stays on the line at the top of the
//! screen.

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
    balloons,
    camera::{self, MainCamera},
    crew::Own,
    draft::{Draft, LIFT},
    hud::{self, Heads, SHEET_EDGE},
    lines::{self, Line},
    report, scene,
    settings::{League, Records, Settings},
    sheet,
    tags::TagMarks,
    theme::{self, Face},
    unlocks::{self, Unlocked},
};

/// How long an alert shows, and the end of that time over which it fades.
pub const ALERT_TIME: f32 = 2.2;
const ALERT_FADE: f32 = 0.4;
/// Two losses this close together are told by one alert.
const MERGE_TIME: f32 = 1.0;
/// How far under the middle of a tank the balloon of an alert stands when its player has none
/// on the screen, in meters of ground: under the arc of its rounds.
const ALERT_DROP: f32 = 2.65;
/// How long a square of the display or a marker blinks for a loss, and each step of it.
pub const BLINK_TIME: f32 = 0.9;
/// How long the square of a player who left the game stays, struck out, before it goes.
pub const LEFT_TIME: f32 = 3.0;
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
/// The double frame of a stamp: its outer line, the gap, and its inner line, which also parts
/// its two fields.
const STAMP_FRAME: (f32, f32, f32) = (3.0, 3.0, 1.0);
/// How far over a tag of a barrier the stamps stand, when one is under where they stand, and
/// how far from the top of the screen they keep at least.
const STAMP_CLEAR: (f32, f32) = (16.0, 80.0);
/// How far down the screen the stamps reach at most, as a share of its height, when they stand
/// under the tags of a barrier: over the tank that the view follows, which is in the middle.
const STAMP_LOWEST: f32 = 44.0;

/// What an alert says, in these faces and sizes: its headline over its reference line, and its
/// lines under it.
const HEADLINE: (Face, f32) = (Face::Title, theme::ALERT);
const LINE: (Face, f32) = (Face::Label, theme::LABEL);
/// What a stamp says: what it is about, what was earned, large or in brief, what more, and the
/// day of its release.
const ABOUT: (Face, f32) = (Face::Label, theme::LABEL);
const EARNED: (Face, f32) = (Face::Title, theme::ALERT);
const EARNED_BRIEF: (Face, f32) = (Face::Title, theme::VALUE);
const MORE: (Face, f32) = (Face::Label, theme::LABEL);
const DAY: (Face, f32) = (Face::Figure, theme::VALUE);
/// Every size that a notice says something in, and the messages of the display: see
/// `rehearse`. Words are in capitals.
const SAID: [f32; 3] = [theme::LABEL, theme::VALUE, theme::ALERT];
const CAPITALS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,:;!?'-+%/()\u{b7}";
/// The leader of an alert: from the dot on what it tells of, at 60° to the edges of the
/// screen, to the bend where its reference line starts, and the dot.
const LEADER: Vec2 = Vec2::new(54.0, 94.0);
const DOT: f32 = 8.0;
/// The reference line: its width, how far in from its start its words stand, how far the
/// baseline of the headline stands over it, and how far the capitals of the lines under it
/// stand under it.
const REFERENCE: f32 = 2.0;
const WORDS_INSET: f32 = 8.0;
const OVER_LINE: f32 = 10.0;
const UNDER_LINE: f32 = 10.0;
/// How far from the middle of a balloon the reference line that hangs from it starts: past
/// the red ring that closes on it.
const HEAD_REACH: f32 = hud::HEAD_WIDTH / 2.0 + 7.0;
/// Where a note points from on the screen, as shares of its size: past these, it turns to
/// the left, or points up from under what it tells of.
const TURN_LEFT: f32 = 0.6;
const TURN_UP: f32 = 0.62;
/// How far the words and the leader of an alert keep from a tag of a barrier or its caption.
const TAG_CLEARANCE: f32 = 8.0;

/// The losses that the display tells for a moment: how long ago the tank of each seat was
/// destroyed, and which life of a single player was lost, with how long ago. And the life
/// that a single player took back, with how long ago.
#[derive(Resource, Default)]
pub struct Alarms {
    pub lost: [Option<f32>; MAX_PLAYERS as usize],
    pub life: Option<(u32, f32)>,
    pub back: Option<(u32, f32)>,
    /// How long ago the player of each seat left the game, if it was a moment ago.
    pub left: [Option<f32>; MAX_PLAYERS as usize],
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

    /// How long ago the player of a seat left the game, if it was a moment ago.
    pub fn left(&self, seat: u8) -> Option<f32> {
        self.left.get(seat as usize).copied().flatten()
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
    /// A teammate left the game, and the next sectors, or the next groups of arena mode, are
    /// for so many players.
    Left { seat: u8, players: u32, arena: bool },
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

/// What an alert points at, or hangs from.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Anchor {
    /// The tank that the view follows, wherever it goes on the screen: a dot on it.
    Watched,
    /// The place on the ground where the player's own tank was destroyed: a dot on it, which
    /// stays on that place as the view moves.
    Pinned(Vec2),
    /// The balloon of the player of a seat, over its tank or at the edge of the screen. Where
    /// it doesn't show, the alert stands under the tank that the view follows, headed by a
    /// balloon of its own.
    Balloon(u8),
}

/// The alert on the screen, if there is one.
#[derive(Resource, Default)]
struct Shown(Option<Alert>);

struct Alert {
    told: Told,
    anchor: Anchor,
    /// The way that it stands from the way it prefers, if it points at a place.
    flip: Flip,
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
    line: Option<Color>,
    /// Whether it casts the shadow of what is written over the game.
    shadow: bool,
}

impl Inked {
    fn new(root: Entity) -> Self {
        Self {
            root,
            text: None,
            back: None,
            border: None,
            line: None,
            shadow: false,
        }
    }

    fn text(root: Entity, color: Color) -> Self {
        Self {
            text: Some(color),
            ..Self::new(root)
        }
    }
}

/// The parts of an alert, which [`place`] lays out around what it tells of.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum Piece {
    /// The leader from the dot to the bend of the reference line.
    Leader,
    Dot,
    /// The balloon that heads an alert whose player has none on the screen.
    Head,
    /// The reference line, from the bend of the leader as far as the words go: it is placed
    /// where the leader ends, so that the two meet however the words are laid out.
    Reference,
    /// The words, over and under the room of the reference line.
    Words,
}

/// How far the wreck of a teammate is, in an alert.
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

/// What a stamp says: what it is about, what was earned, and a word more. A record has a
/// shorter word too, for the smaller stamp that it takes next to an achievement.
#[derive(Clone, Debug, PartialEq)]
struct StampWords {
    about: String,
    earned: String,
    more: String,
    brief: Option<String>,
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
        .add_systems(Update, rehearse)
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

/// Lays out what a notice can say ahead of the first one, one face and size in a frame and
/// out of sight, from the main menu on. The engine draws the letters of a face at a size the
/// first time that it lays them out, and the first alert of a run, in the frame of a loss,
/// took 3 ms more for it (measured 2026-10-14). The words stay laid out where nothing shows,
/// and are laid out again when the interface changes size, for its new size.
fn rehearse(mut commands: Commands, mut rehearsed: Local<(usize, Option<Entity>)>) {
    let (done, root) = &mut *rehearsed;
    let Some(&size) = SAID.get(*done) else {
        return;
    };
    let root = *root.get_or_insert_with(|| {
        let aside = Node {
            position_type: PositionType::Absolute,
            left: px(-10_000),
            flex_direction: FlexDirection::Column,
            ..default()
        };
        commands.spawn((aside, Visibility::Hidden)).id()
    });
    let said = Face::Label.set(CAPITALS.to_string(), size, theme::TEXT);
    commands.spawn((ChildOf(root), said));
    *done += 1;
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

/// The cue of a loss of the players, by the tanks that still run after it. Only a loss after
/// which one more ends the run has one, when a single tank is left: the others are told by
/// what shows and by their blast. None is left when the run is over, which has a cue of its
/// own.
fn loss_cue(running: usize) -> Option<Cue> {
    (running == 1).then_some(Cue::Last)
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

/// What an alert says of the run that a player left: what it is played for from then on.
fn left_words(players: u32, arena: bool) -> String {
    let unit = if arena { "groups" } else { "sectors" };
    let players = match players {
        1 => "1 player".to_string(),
        players => format!("{players} players"),
    };
    format!("The next {unit} are for {players}")
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
    mut before: Local<Option<(u64, u64, u32, Vec<u8>)>>,
) {
    let Some(run) = run else {
        events.clear();
        repaired.clear();
        *before = None;
        return;
    };
    let infinite_lives = playtest.is_some_and(|playtest| playtest.infinite_lives);
    // What there is to tell this frame, the last one first to be shown.
    let mut told: Vec<(Told, Anchor)> = Vec::new();
    let mut cue: Option<Cue> = None;
    // A single player's tank is gone when it is destroyed: the run says what is left. A life
    // that they take back is told on the display.
    for event in events.read() {
        if matches!(event, SimEvent::LifeTaken { .. }) && run.solo {
            alarms.back = Some((run.lives, 0.0));
        }
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
            let anchor = Anchor::Pinned(at);
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
        let seats: Vec<u8> = lost.iter().map(|(seat, _)| *seat).collect();
        // It hangs from the balloon of the last of them.
        let anchor = Anchor::Balloon(seats[seats.len() - 1]);
        let told_lost = Told::Lost { seats, wreck, last };
        told.push((told_lost, anchor));
    }
    // One's own loss is told before a teammate's that comes with it.
    if let Some(ground) = own_loss {
        let menders: Vec<u8> = running.iter().map(|(seat, _)| *seat).collect();
        cue = cue.or(loss_cue(menders.len()));
        let own = Told::Own {
            blow: run.lost_to,
            solo: false,
            lives: None,
            menders,
        };
        let anchor = Anchor::Pinned(ground);
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
            let anchor = if own {
                Anchor::Watched
            } else {
                Anchor::Balloon(player.seat)
            };
            told.push((back, anchor));
        }
    }
    // A player who leaves the run takes their tank away, and the run counts one player less.
    let mut seats: Vec<u8> = players.iter().map(|(player, ..)| player.seat).collect();
    seats.sort();
    if let Some((seed, ticks, count, ref was)) = *before
        && seed == run.seed
        && ticks <= run.ticks
        && run.players < count
    {
        for &seat in was.iter().filter(|seat| !seats.contains(seat)) {
            if let Some(since) = alarms.left.get_mut(seat as usize) {
                *since = Some(0.0);
            }
            let left = Told::Left {
                seat,
                players: run.players,
                arena: run.arena,
            };
            told.push((left, Anchor::Balloon(seat)));
        }
    }
    *before = Some((run.seed, run.ticks, run.players, seats));
    if let Some(cue) = cue {
        sounds.write(Sound(cue));
    }
    let Some((told, anchor)) = told.pop() else {
        return;
    };
    // An alert that joins the one on the screen stands where that one stood.
    let (told, anchor, flip) = match &shown.0 {
        Some(alert) => match merged(&alert.told, alert.age, &told) {
            Some(joined) => (joined, alert.anchor, alert.flip),
            None => (told, anchor, Flip::default()),
        },
        None => (told, anchor, Flip::default()),
    };
    if let Some(alert) = shown.0.take() {
        commands.entity(alert.root).despawn();
    }
    let root = spawn(&mut commands, &told);
    shown.0 = Some(Alert {
        told,
        anchor,
        flip,
        age: 0.0,
        root,
    });
}

/// A row of the lines under the reference line of an alert, side by side.
fn row(commands: &mut Commands, column: Entity, gap: f32) -> Entity {
    let row = Node {
        column_gap: px(gap),
        align_items: AlignItems::Center,
        margin: UiRect::horizontal(px(WORDS_INSET)),
        ..default()
    };
    commands.spawn((ChildOf(column), row)).id()
}

/// Words of an alert, with their shadow.
fn words(
    commands: &mut Commands,
    (parent, root): (Entity, Entity),
    (face, size): (Face, f32),
    content: impl Into<String>,
    color: Color,
) {
    let shaded = theme::shaded(Inked::text(root, color), face, content, size, color);
    commands.spawn((ChildOf(parent), shaded));
}

/// A small square of a color, with the shadow of the words beside it: the color of the tank
/// behind a blow, or a life.
fn square(
    commands: &mut Commands,
    (parent, root): (Entity, Entity),
    size: f32,
    (fill, border): (Color, Option<Color>),
) {
    let width = if border.is_some() { 1.5 } else { 0.0 };
    commands.spawn((
        ChildOf(parent),
        Inked {
            back: Some(fill),
            border,
            shadow: true,
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
        theme::shadow(1.0),
    ));
}

/// What an alert says over its reference line, and in which color: red for a loss, blue for a
/// return.
fn headline(told: &Told) -> (&'static str, Color) {
    match told {
        Told::Lost { .. } => ("Destroyed", theme::DANGER),
        Told::Back(Some(_)) => ("Is back", theme::ACCENT_HOVERED),
        Told::Back(None) => ("Your tank is back", theme::ACCENT_HOVERED),
        Told::Left { .. } => ("Left the game", theme::TEXT),
        Told::Own { solo: true, .. } => ("Tank destroyed", theme::DANGER),
        Told::Own { solo: false, .. } => ("Your tank is destroyed", theme::DANGER),
    }
}

/// The balloon that heads an alert whose player has none on the screen: the one of a player
/// who left, struck out, or of a teammate out of view.
fn head_state(told: &Told) -> Option<(u8, balloons::State)> {
    match told {
        Told::Left { seat, .. } => Some((*seat, balloons::State::Left)),
        Told::Lost { seats, .. } => Some((seats[seats.len() - 1], balloons::State::Lost(0.0))),
        Told::Back(Some(seat)) => Some((*seat, balloons::State::Runs)),
        _ => None,
    }
}

/// Draws an alert, at no place yet: [`place`] lays it out around what it tells of.
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
    let (said, ink) = headline(told);
    // The leader, its dot and the reference line stand over the words, whose shadows would
    // dim them.
    let piece = |piece: Piece| {
        let over = matches!(piece, Piece::Leader | Piece::Dot | Piece::Reference);
        (
            ChildOf(root),
            piece,
            UiTransform::IDENTITY,
            Visibility::Hidden,
            ZIndex(i32::from(over)),
        )
    };
    let leader = Line::new(REFERENCE, ink);
    commands.spawn((
        piece(Piece::Leader),
        Inked {
            line: Some(ink),
            ..Inked::new(root)
        },
        Node {
            position_type: PositionType::Absolute,
            width: px(LEADER.length()),
            height: px(leader.across()),
            ..default()
        },
        leader,
    ));
    commands.spawn((
        piece(Piece::Dot),
        Inked {
            back: Some(ink),
            ..Inked::new(root)
        },
        Node {
            position_type: PositionType::Absolute,
            width: px(DOT),
            height: px(DOT),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(ink),
    ));
    commands.spawn((
        piece(Piece::Reference),
        Inked {
            back: Some(ink),
            ..Inked::new(root)
        },
        Node {
            position_type: PositionType::Absolute,
            width: px(0),
            height: px(REFERENCE),
            ..default()
        },
        BackgroundColor(ink),
    ));
    if let Some((seat, state)) = head_state(told) {
        let head = commands
            .spawn((
                piece(Piece::Head),
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
            ))
            .id();
        let balloon = balloons::balloon(seat, hud::HEAD_WIDTH, theme::VALUE);
        commands.spawn((ChildOf(head), balloon)).insert(state);
    }
    // The words, with the room of the reference line between them: as long as the longest of
    // them, and the line as long as they are.
    let column = commands
        .spawn((
            piece(Piece::Words),
            Node {
                position_type: PositionType::Absolute,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Start,
                ..default()
            },
        ))
        .id();
    let over = commands
        .spawn((
            ChildOf(column),
            Node {
                margin: UiRect {
                    left: px(WORDS_INSET),
                    right: px(WORDS_INSET),
                    // The baseline of the headline stands so far over the line.
                    bottom: px(OVER_LINE - REFERENCE / 2.0 - capitals_room(HEADLINE.1)),
                    ..default()
                },
                ..default()
            },
        ))
        .id();
    words(commands, (over, root), HEADLINE, said, ink);
    commands.spawn((
        ChildOf(column),
        Node {
            height: px(REFERENCE),
            margin: UiRect::bottom(px(UNDER_LINE - REFERENCE / 2.0 - capitals_room(LINE.1))),
            ..default()
        },
    ));
    match told {
        Told::Lost { last, .. } => {
            let first = row(commands, column, 9.0);
            let distance = theme::shaded(
                (Distance, Inked::text(root, theme::LINE)),
                Face::Label,
                "",
                LINE.1,
                theme::LINE,
            );
            commands.spawn((ChildOf(first), distance));
            if let Some(last) = last {
                let second = row(commands, column, 9.0);
                words(commands, (second, root), LINE, last_words(*last), ink);
            }
        }
        Told::Back(_) => {}
        Told::Left { players, arena, .. } => {
            let first = row(commands, column, 9.0);
            let said = left_words(*players, *arena);
            words(commands, (first, root), LINE, said, theme::LINE);
        }
        Told::Own {
            blow,
            solo,
            lives,
            menders,
        } => {
            // What destroyed it, and among several players who can repair it, on one line.
            let menders = menders_words(menders);
            if blow.is_some() || menders.is_some() {
                let first = row(commands, column, 9.0);
                if let Some(blow) = blow {
                    square(commands, (first, root), 11.0, (report::color(blow), None));
                    let cause = cause(blow, *solo);
                    words(commands, (first, root), LINE, cause, theme::LINE);
                }
                if let Some(said) = menders {
                    if blow.is_some() {
                        square(commands, (first, root), 3.0, (theme::TEXT_DIM, None));
                    }
                    words(commands, (first, root), LINE, said, theme::LINE);
                }
            }
            if let Some((left, (said, color))) = lives.and_then(|n| Some((n, lives_words(n)?))) {
                let lives = row(commands, column, 6.0);
                for life in 1..=tanku_core::rules::SOLO_LIVES {
                    let look = if life <= left {
                        (theme::PLAYER, None)
                    } else if life == left + 1 {
                        (theme::DANGER_FILL, Some(theme::DANGER))
                    } else {
                        (theme::VOID, Some(theme::RULE))
                    };
                    square(commands, (lives, root), 15.0, look);
                }
                words(commands, (lives, root), LINE, said, color);
            }
        }
    }
    root
}

/// The room between the capitals of a line of capitals and the edges of its box, over and
/// under them: their box is higher than they are.
fn capitals_room(size: f32) -> f32 {
    (theme::capitals_height(size) - size) / 2.0
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

/// How an alert stands on the screen: the dot and the bend of its leader, if it has one, the
/// middle of the balloon that heads it, if it draws one, where its reference line starts, and
/// which way it runs across.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Layout {
    leader: Option<(Vec2, Vec2)>,
    head: Option<Vec2>,
    start: Vec2,
    way: f32,
}

/// Which way an alert that points at a place stands, from the way that it prefers there:
/// turned over, up instead of down or down instead of up, and to the other side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Flip {
    over: bool,
    aside: bool,
}

impl Flip {
    /// Every way, the one that an alert prefers first. Turned over comes before turned aside:
    /// the two tags of a barrier stand level with each other at its two ends, unless the view
    /// is turned.
    const ALL: [Flip; 4] = [
        Flip {
            over: false,
            aside: false,
        },
        Flip {
            over: true,
            aside: false,
        },
        Flip {
            over: false,
            aside: true,
        },
        Flip {
            over: true,
            aside: true,
        },
    ];
}

/// The layout of an alert with a dot on a point of the screen: its leader goes down at 60°
/// to the right of it, or to the left past the most of the screen, and up from the bottom of
/// the screen, unless it is flipped.
fn pointing(at: Vec2, screen: Vec2, flip: Flip) -> Layout {
    // On whole pixels, as the leader is: the reference line, laid out on whole pixels, then
    // runs through the end of the leader.
    let at = at.round();
    let way = if at.x > screen.x * TURN_LEFT {
        -1.0
    } else {
        1.0
    };
    let down = if at.y > screen.y * TURN_UP { -1.0 } else { 1.0 };
    let way = if flip.aside { -way } else { way };
    let down = if flip.over { -down } else { down };
    let bend = at + Vec2::new(LEADER.x * way, LEADER.y * down);
    Layout {
        leader: Some((at, bend)),
        head: None,
        start: bend,
        way,
    }
}

/// The layout of an alert that hangs from a balloon: its reference line runs from past the
/// balloon, the way that it is given.
fn hanging(head: Vec2, way: f32, drawn: bool) -> Layout {
    Layout {
        leader: None,
        head: drawn.then_some(head),
        start: head + Vec2::X * HEAD_REACH * way,
        way,
    }
}

/// Where the reference line of an alert goes, by its top left corner, when it is this long:
/// from where it starts, the way that it runs, with its middle on that point.
fn reference_corner(layout: &Layout, length: f32) -> Vec2 {
    let left = if layout.way > 0.0 {
        layout.start.x
    } else {
        layout.start.x - length
    };
    Vec2::new(left, layout.start.y - REFERENCE / 2.0)
}

/// How far the top of the words of an alert stands over the middle of its reference line: the
/// box of the headline, and from its capitals down to the middle of the line.
fn words_over() -> f32 {
    theme::capitals_height(HEADLINE.1) + OVER_LINE - capitals_room(HEADLINE.1)
}

/// The room that the words of an alert take in a layout, when they are this large.
fn words_room(layout: &Layout, words: Vec2) -> Rect {
    let top = layout.start.y - words_over();
    let left = if layout.way > 0.0 {
        layout.start.x
    } else {
        layout.start.x - words.x
    };
    Rect::new(left, top, left + words.x, top + words.y)
}

/// Whether an alert keeps clear of the tags of the barriers and of their captions: its words,
/// and its leader past its dot, which stays on what the alert tells of.
fn keeps_clear(layout: &Layout, words: Rect, tags: &[Rect]) -> bool {
    let tags: Vec<Rect> = tags.iter().map(|tag| tag.inflate(TAG_CLEARANCE)).collect();
    let leader = layout.leader.is_none_or(|(dot, bend)| {
        let steps = (dot.distance(bend) / 4.0).ceil().max(1.0) as usize;
        (1..=steps).all(|step| {
            let point = dot.lerp(bend, step as f32 / steps as f32);
            tags.iter().all(|tag| !tag.contains(point))
        })
    });
    leader && tags.iter().all(|tag| tag.intersect(words).is_empty())
}

/// Whether words stay on the screen: inside the frame of the sheet, and over the blocks at
/// the bottom of the display.
fn on_sheet(words: Rect, screen: Vec2) -> bool {
    let room = Rect::new(
        SHEET_EDGE,
        SHEET_EDGE,
        screen.x - SHEET_EDGE,
        screen.y - hud::BLOCKS_HIGH,
    );
    room.contains(words.min) && room.contains(words.max)
}

/// The way of an alert that points at a point of the screen, and whose words take this room:
/// the way that it has, as long as it keeps clear of the tags of the barriers, or else the
/// first of the others that does, from the way that it prefers. A way that it doesn't prefer
/// must keep its words on the sheet too. Where none is clear, it keeps the way that it has.
fn clear_flip(at: Vec2, screen: Vec2, words: Vec2, tags: &[Rect], now: Flip) -> Flip {
    let clear = |flip: Flip| {
        let layout = pointing(at, screen, flip);
        let room = words_room(&layout, words);
        let shown = flip == Flip::default() || on_sheet(room, screen);
        shown && keeps_clear(&layout, room, tags)
    };
    let mut ways = std::iter::once(now).chain(Flip::ALL);
    ways.find(|flip| clear(*flip)).unwrap_or(now)
}

/// Ages the alert, and lays it out around what it tells of: a dot on the tank that the view
/// follows or on the place where the player's own tank was destroyed, or the balloon of a
/// teammate. An alert of a teammate's loss says how far the wreck is.
fn place(
    time: Res<Time>,
    ui_scale: Res<UiScale>,
    camera: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    tanks: Query<(&Player, &GlobalTransform, Has<Own>, Has<Wreck>)>,
    heads: Res<Heads>,
    marks: Option<Res<TagMarks>>,
    mut shown: ResMut<Shown>,
    mut alarms: ResMut<Alarms>,
    mut roots: Query<(&mut Notice, &mut Visibility), Without<Piece>>,
    mut pieces: Query<(
        &Piece,
        &ChildOf,
        &mut Node,
        &mut UiTransform,
        &mut Visibility,
        Option<&Line>,
        &ComputedNode,
    )>,
    mut distances: Query<&mut Text, With<Distance>>,
    mut commands: Commands,
) {
    let seconds = time.delta_secs();
    for since in alarms.lost.iter_mut() {
        *since = since
            .map(|since| since + seconds)
            .filter(|since| *since < ALERT_TIME);
    }
    let alarms = &mut *alarms;
    for since in alarms.left.iter_mut() {
        *since = since
            .map(|since| since + seconds)
            .filter(|since| *since < LEFT_TIME);
    }
    for told in [&mut alarms.life, &mut alarms.back] {
        if let Some((life, since)) = *told {
            *told = Some((life, since + seconds)).filter(|(_, since)| *since < ALERT_TIME);
        }
    }
    let Some(alert) = shown.0.as_mut() else {
        return;
    };
    alert.age += seconds;
    let Ok((mut notice, mut visibility)) = roots.get_mut(alert.root) else {
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
    let screen = camera.logical_viewport_size().unwrap_or(Vec2::ONE) / scale;
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
    let seen = |point: Vec3| on_screen((camera, transform), scale, point);
    // The room that the words take, once they are laid out: a frame after the alert comes.
    let words = pieces
        .iter()
        .find(|(piece, parent, ..)| **piece == Piece::Words && parent.parent() == alert.root)
        .map(|(.., computed)| computed.size() * computed.inverse_scale_factor())
        .unwrap_or_default();
    // An alert shows once its words are laid out: an alert that points at a place must know
    // whether they keep clear of the tags of the barriers there, and its reference line is as
    // long as they are.
    if words == Vec2::ZERO {
        *visibility = Visibility::Hidden;
        return;
    }
    let tags = marks.as_deref().map_or(&[][..], |marks| &marks.0[..]);
    let pointed = |at: Vec2, flip: &mut Flip| {
        *flip = clear_flip(at, screen, words, tags, *flip);
        pointing(at, screen, *flip)
    };
    let layout = match alert.anchor {
        Anchor::Watched => watched
            .and_then(seen)
            .map(|(at, _)| pointed(at, &mut alert.flip)),
        Anchor::Pinned(ground) => {
            let at = seen(scene::to_world(ground, 0.0));
            at.map(|(at, _)| pointed(at, &mut alert.flip))
        }
        Anchor::Balloon(seat) => match heads.of(seat) {
            Some((head, way)) => Some(hanging(head, way, false)),
            // Under the tank that the view follows, with a balloon of its own.
            None => watched.and_then(seen).map(|(at, meter)| {
                let head = at + Vec2::new(-HEAD_REACH, ALERT_DROP * meter);
                hanging(head, 1.0, true)
            }),
        },
    };
    let Some(layout) = layout else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Inherited;
    for (piece, parent, mut node, mut moved, mut shows, line, _) in &mut pieces {
        if parent.parent() != alert.root {
            continue;
        }
        let (at, turn, wanted) = match *piece {
            Piece::Leader => match (layout.leader, line) {
                (Some((dot, bend)), Some(line)) => {
                    let along = bend - dot;
                    let corner = line.corner((dot + bend) / 2.0, along.length());
                    (corner, along.to_angle(), true)
                }
                _ => (Vec2::ZERO, 0.0, false),
            },
            Piece::Dot => match layout.leader {
                Some((dot, _)) => (dot - Vec2::splat(DOT / 2.0), 0.0, true),
                None => (Vec2::ZERO, 0.0, false),
            },
            Piece::Head => match layout.head {
                Some(head) => (head - Vec2::splat(hud::HEAD_WIDTH / 2.0), 0.0, true),
                None => (Vec2::ZERO, 0.0, false),
            },
            Piece::Reference => {
                // On whole pixels, as the leader is, so that its end stays on the bend.
                let length = words.x.round();
                if node.width != px(length) {
                    node.width = px(length);
                }
                (reference_corner(&layout, length), 0.0, true)
            }
            Piece::Words => {
                // The reference line runs through the point where it starts, and the words
                // stand on the side it runs to.
                let (align, left) = if layout.way > 0.0 {
                    (AlignItems::Start, percent(0))
                } else {
                    (AlignItems::End, percent(-100))
                };
                if node.align_items != align {
                    node.align_items = align;
                }
                let held = Val2::new(left, px(0));
                if moved.translation.x != held.x {
                    moved.translation.x = held.x;
                }
                (layout.start - Vec2::Y * words_over(), 0.0, true)
            }
        };
        let (left, top) = (px(at.x.round()), px(at.y.round()));
        if node.left != left || node.top != top {
            (node.left, node.top) = (left, top);
        }
        let turn = Rot2::radians(turn);
        if moved.rotation != turn {
            moved.rotation = turn;
        }
        let visible = if wanted {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        shows.set_if_neq(visible);
    }
    // An alert of a teammate's loss says how far the wreck is from the tank that the view
    // follows.
    if let (Told::Lost { wreck, .. }, Some(from)) = (&alert.told, watched) {
        let meters = scene::to_ground(from).distance(*wreck).round();
        for mut text in &mut distances {
            let said = format!("{meters} m from you").to_uppercase();
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
        Option<&mut BoxShadow>,
        Option<&mut Line>,
    )>,
) {
    for (inked, text, back, border, shadow, line) in &mut parts {
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
        if let (true, Some(mut shadow)) = (inked.shadow, shadow) {
            shadow.set_if_neq(theme::shadow(shown));
        }
        if let (Some(color), Some(mut line)) = (inked.line, line) {
            let ink = faded(color);
            if line.ink != ink {
                line.ink = ink;
            }
        }
    }
}

/// Flashes the frame of the sheet, and lets the flash fade. A setting of accessibility turns
/// the flash off.
fn flash(
    time: Res<Time>,
    settings: Res<Settings>,
    mut flash: ResMut<Flash>,
    mut frame: Single<&mut BorderColor, With<FlashFrame>>,
) {
    flash.age += time.delta_secs();
    let on = if settings.flash_frame { 1.0 } else { 0.0 };
    let left = on * (1.0 - flash.age / FLASH_TIME).max(0.0);
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
        brief: None,
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
    let (cleared, unit) = (run.cleared, league.unit());
    StampWords {
        about: format!("{team} \u{b7} {kind}"),
        earned: "New record".to_string(),
        more: format!("{cleared} {unit}s cleared \u{b7} the best was {best}"),
        brief: Some(format!("{cleared} {unit}s \u{b7} the best was {best}")),
    }
}

/// Whether a run is past the record that it started with. A first run has none to beat: every
/// sector it clears would be one.
fn past(best: Option<u32>, cleared: u32) -> bool {
    best.is_some_and(|best| cleared > best)
}

/// Finds what a run earns: each achievement that it unlocks, and the first sector that it
/// clears past its record. A playtest keeps no record, unless it fakes them.
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
    // A new run, or the same seed again, starts from the record as it stands, and so do the
    // records that a playtest swaps in or out.
    let this = (run.seed, run.ticks);
    let new = best
        .run
        .is_none_or(|(seed, ticks)| seed != this.0 || ticks > this.1);
    best.run = Some(this);
    if new || records.is_changed() {
        let record = records.best(League::of(&run), run.team);
        best.sector = record.map(|record| record.sector);
        best.told = false;
    }
    for Unlocked(unlock) in unlocked.read() {
        stamps.waiting.push(unlock_words(*unlock, run.solo));
    }
    let kept = !run.playtest || records.fake;
    if !best.told && kept && past(best.sector, run.cleared) {
        best.told = true;
        let words = record_words(&run, best.sector.unwrap_or_default());
        stamps.waiting.push(words);
    }
}

/// A stamp, which comes down in its column after this many seconds: an approval stamp, its
/// double frame turned a little, with what was earned in one field and its release, and the
/// day of it, in the other. A record next to an achievement takes a smaller stamp, on one
/// line.
fn card(commands: &mut Commands, column: Entity, said: &StampWords, delay: f32, beside: bool) {
    let brief = said.brief.as_ref().filter(|_| beside);
    let fill = theme::VOID.with_alpha(STAMP_FILL);
    let card = commands
        .spawn((
            ChildOf(column),
            StampCard(delay),
            Notice(0.0),
            Node {
                border: UiRect::all(px(STAMP_FRAME.0)),
                padding: UiRect::all(px(STAMP_FRAME.1)),
                ..default()
            },
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
        ..Inked::new(card)
    });
    // The frames are turned with the card: each is drawn in lines over the border that holds
    // its room, and fades with the card.
    let line = Inked {
        line: Some(theme::GOOD),
        ..Inked::new(card)
    };
    let ruled = |commands: &mut Commands, parent: Entity, node: Node| {
        let widths = [
            node.border.top,
            node.border.right,
            node.border.bottom,
            node.border.left,
        ]
        .map(|width| match width {
            Val::Px(width) => width,
            _ => 0.0,
        });
        let ruled = commands.spawn((ChildOf(parent), node)).id();
        let border = lines::border(widths, theme::GOOD, line);
        commands.spawn((ChildOf(ruled), border));
        ruled
    };
    let border = lines::border([STAMP_FRAME.0; 4], theme::GOOD, line);
    commands.spawn((ChildOf(card), border));
    let inner = ruled(
        commands,
        card,
        Node {
            border: UiRect::all(px(STAMP_FRAME.2)),
            ..default()
        },
    );
    let field = ruled(
        commands,
        inner,
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            padding: UiRect::new(px(16), px(16), px(8), px(9)),
            border: UiRect::right(px(STAMP_FRAME.2)),
            ..default()
        },
    );
    let parts = (field, card);
    if let Some(brief) = brief {
        let line = commands
            .spawn((
                ChildOf(field),
                Node {
                    column_gap: px(14),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ))
            .id();
        words(
            commands,
            (line, card),
            EARNED_BRIEF,
            &said.earned,
            theme::GOOD,
        );
        words(commands, (line, card), MORE, brief, theme::LINE);
    } else {
        words(commands, parts, ABOUT, &said.about, theme::GOOD);
        words(commands, parts, EARNED, &said.earned, theme::GOOD);
        words(commands, parts, MORE, &said.more, theme::LINE);
    }
    let release = commands
        .spawn((
            ChildOf(inner),
            Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                row_gap: px(8),
                padding: UiRect::axes(px(14), px(8)),
                ..default()
            },
        ))
        .id();
    words(commands, (release, card), ABOUT, "Released", theme::GOOD);
    words(
        commands,
        (release, card),
        DAY,
        sheet::day_of_issue(),
        theme::GOOD,
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
    (marks, mut columns): (
        Option<Res<TagMarks>>,
        Query<(&mut Node, &ComputedNode, &UiGlobalTransform), Without<StampCard>>,
    ),
    (ui_scale, camera): (
        Option<Res<UiScale>>,
        Option<Single<&Camera, With<MainCamera>>>,
    ),
) {
    let seconds = time.delta_secs();
    // The stamps move up out of the way of a tag of a barrier that stands under them.
    if let Some((column, ..)) = stamps.column
        && let Ok((mut node, computed, at)) = columns.get_mut(column)
        && let (Some(marks), Some(scale), Some(camera)) = (marks, ui_scale, camera)
        && let Some(screen) = camera.logical_viewport_size()
    {
        let scale = scale.0;
        let half = computed.size / scale / 2.0;
        let screen = screen / scale;
        let home = Vec2::new(at.translation.x / scale, screen.y * STAMP_HEIGHT / 100.0);
        let lowest = screen.y * STAMP_LOWEST / 100.0;
        let wanted = clear_of(home, half, &marks.0, lowest);
        if node.top != wanted {
            node.top = wanted;
        }
    }
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
                let beside = waiting.len() > 1;
                for (index, words) in waiting.iter().enumerate() {
                    let delay = STAMP_STEP * index as f32;
                    card(&mut commands, column, words, delay, beside);
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
                    card(&mut commands, column, words, delay, true);
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

/// Where the middle of the stamps stands from the top of the screen, from where they stand when
/// nothing is in their way: there, or over the tags of barriers that stand under them, by
/// their middle and half their size. They keep clear of the top of the screen. Where that
/// leaves them on a tag, they stand under the tags instead, if their bottom stays over
/// `lowest`.
fn clear_of(home: Vec2, half: Vec2, tags: &[Rect], lowest: f32) -> Val {
    let room = |middle: f32| Rect::from_center_half_size(Vec2::new(home.x, middle), half);
    let in_way = |middle: f32| {
        let room = room(middle).inflate(STAMP_CLEAR.0);
        tags.iter()
            .filter(move |tag| !room.intersect(**tag).is_empty())
            .copied()
    };
    // A tag that the stamps are moved past may be in the way of another one.
    let past = |toward: f32| {
        let mut middle = home.y;
        for _ in 0..tags.len() {
            let edges = in_way(middle).map(|tag| if toward < 0.0 { tag.min.y } else { tag.max.y });
            let Some(edge) = edges.reduce(|a, b| if toward < 0.0 { a.min(b) } else { a.max(b) })
            else {
                break;
            };
            middle = edge + toward * (STAMP_CLEAR.0 + half.y + 0.5);
        }
        middle
    };
    let over = past(-1.0).max(STAMP_CLEAR.1 + half.y);
    let middle = if in_way(over).next().is_none() {
        over
    } else {
        let under = past(1.0);
        let clear = in_way(under).next().is_none() && under + half.y <= lowest;
        if clear { under } else { over }
    };
    if (middle - home.y).abs() < 0.5 {
        percent(STAMP_HEIGHT)
    } else {
        px(middle.round())
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
    fn only_a_loss_after_which_one_more_ends_the_run_has_a_cue() {
        assert_eq!(loss_cue(3), None);
        assert_eq!(loss_cue(2), None);
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
    fn an_alert_points_at_what_it_tells_of_from_a_note_that_stays_on_the_screen() {
        let screen = Vec2::new(1920.0, 1080.0);
        // Down at 60° to the right of the dot, as a leader note is drawn.
        let middle = pointing(Vec2::new(800.0, 400.0), screen, Flip::default());
        let (dot, bend) = middle.leader.unwrap();
        assert_eq!(bend - dot, LEADER);
        assert!(((bend - dot).to_angle().to_degrees() - 60.0).abs() < 0.2);
        assert_eq!((middle.start, middle.way), (bend, 1.0));
        // To the left past the most of the screen, and up from its bottom.
        let low_right = pointing(Vec2::new(1700.0, 900.0), screen, Flip::default());
        let (dot, bend) = low_right.leader.unwrap();
        assert_eq!(bend - dot, Vec2::new(-LEADER.x, -LEADER.y));
        assert_eq!(low_right.way, -1.0);
        // From a balloon, the line starts past its red ring, and draws no leader.
        let hung = hanging(Vec2::new(500.0, 500.0), -1.0, false);
        assert_eq!(hung.start, Vec2::new(500.0 - HEAD_REACH, 500.0));
        assert_eq!((hung.leader, hung.head), (None, None));
        const { assert!(HEAD_REACH > hud::HEAD_WIDTH / 2.0 + balloons::LINE) };
    }

    #[test]
    fn the_reference_line_of_an_alert_runs_on_from_the_end_of_its_leader() {
        let screen = Vec2::new(1920.0, 1080.0);
        let length = 380.0;
        // A dot between pixels, each way that an alert can stand.
        for at in [Vec2::new(800.3, 400.6), Vec2::new(1700.7, 900.2)] {
            for flip in Flip::ALL {
                let layout = pointing(at, screen, flip);
                let (_, bend) = layout.leader.unwrap();
                // The leader ends on whole pixels, where the line starts, and the line runs
                // through that point by its middle.
                assert_eq!(bend, bend.round());
                let corner = reference_corner(&layout, length);
                let start = if layout.way > 0.0 {
                    corner
                } else {
                    corner + Vec2::X * length
                };
                assert_eq!(start + Vec2::Y * REFERENCE / 2.0, bend);
            }
        }
    }

    #[test]
    fn an_alert_turns_to_keep_clear_of_the_tags_of_a_barrier() {
        let screen = Vec2::new(1920.0, 1080.0);
        let (at, words) = (Vec2::new(960.0, 540.0), Vec2::new(380.0, 70.0));
        let room = |flip: Flip| words_room(&pointing(at, screen, flip), words);
        let level = Flip::default();
        // Nothing in the way: the way that it prefers, down to the right.
        assert_eq!(clear_flip(at, screen, words, &[], level), level);
        // The tag at the east end of a barrier just under the tank, as when the tank was lost
        // right past it: the alert turns over, up from its dot.
        let under = room(level);
        let east = Rect::from_center_half_size(under.max - Vec2::splat(30.0), Vec2::splat(30.0));
        let over = clear_flip(at, screen, words, &[east], level);
        assert_eq!(over, Flip::ALL[1]);
        assert!(room(over).max.y < at.y);
        // A tag over the tank too: it turns aside instead, and down.
        let north = Rect::from_center_half_size(room(over).center(), Vec2::splat(30.0));
        assert_eq!(
            clear_flip(at, screen, words, &[east, north], level),
            Flip::ALL[2]
        );
        // Once turned, it stays so while it keeps clear, even when the tag is gone, and it
        // keeps the way that it has where none is clear.
        assert_eq!(clear_flip(at, screen, words, &[], over), over);
        let everywhere = Rect::from_center_half_size(at, Vec2::splat(1000.0));
        assert_eq!(clear_flip(at, screen, words, &[everywhere], over), over);
        // Its leader keeps clear too.
        let layout = pointing(at, screen, level);
        let (dot, bend) = layout.leader.unwrap();
        let on_leader = Rect::from_center_half_size(dot.lerp(bend, 0.5), Vec2::splat(4.0));
        assert!(!keeps_clear(&layout, room(level), &[on_leader]));
    }

    #[test]
    fn an_alert_that_turns_keeps_its_words_over_the_blocks_of_the_display() {
        // A tank low on the screen: its alert points up from it. Turned over, its words would
        // stand on the parts list or the title block.
        let screen = Vec2::new(1920.0, 1080.0);
        let (at, words) = (Vec2::new(960.0, 860.0), Vec2::new(380.0, 70.0));
        let room = |flip: Flip| words_room(&pointing(at, screen, flip), words);
        let level = Flip::default();
        assert!(room(level).max.y < at.y);
        assert!(!on_sheet(room(Flip::ALL[1]), screen));
        assert!(on_sheet(room(Flip::ALL[2]), screen));
        let tag = Rect::from_center_half_size(room(level).center(), Vec2::splat(30.0));
        assert_eq!(clear_flip(at, screen, words, &[tag], level), Flip::ALL[2]);
    }

    #[test]
    fn stamps_stand_over_the_tags_of_barriers_that_are_under_them() {
        let (home, half, lowest) = (Vec2::new(960.0, 300.0), Vec2::new(300.0, 60.0), 475.0);
        // Nothing in the way: where they stand.
        assert_eq!(clear_of(home, half, &[], lowest), percent(STAMP_HEIGHT));
        let far = Rect::from_center_half_size(Vec2::new(200.0, 300.0), Vec2::splat(40.0));
        assert_eq!(clear_of(home, half, &[far], lowest), percent(STAMP_HEIGHT));
        // A tag under them: over it, by a gap.
        let under = Rect::from_center_half_size(Vec2::new(960.0, 330.0), Vec2::splat(40.0));
        let Val::Px(middle) = clear_of(home, half, &[under], lowest) else {
            panic!("the stamps don't move");
        };
        assert!(middle + half.y + STAMP_CLEAR.0 <= under.min.y);
        // Never into the top of the screen: a tag that leaves them no room over it has them
        // under it, by a gap.
        let high = Rect::from_center_half_size(Vec2::new(960.0, 120.0), Vec2::splat(40.0));
        let home = Vec2::new(960.0, 150.0);
        let Val::Px(middle) = clear_of(home, half, &[high], lowest) else {
            panic!("the stamps don't move");
        };
        assert!(middle - half.y - STAMP_CLEAR.0 >= high.max.y);
        // Unless that would bring them down onto the tank in the middle of the screen: they
        // stay as high as they go.
        let Val::Px(middle) = clear_of(home, half, &[high], 250.0) else {
            panic!("the stamps don't move");
        };
        assert_eq!(middle, STAMP_CLEAR.1 + half.y);
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
    fn the_loss_of_a_teammate_is_told_and_heard_once_one_tank_is_left() {
        let (mut app, tanks) = game(&[0, 1, 2]);
        assert_eq!(heard(&mut app), (vec![], None));
        // Player 3's tank is destroyed: two tanks still run, and the loss is only seen.
        app.world_mut()
            .entity_mut(tanks[2])
            .insert(Wreck::default());
        app.update();
        let lost = |seats: Vec<u8>, last| Told::Lost {
            seats,
            wreck: Vec2::new(10.0, 0.0),
            last,
        };
        assert_eq!(heard(&mut app), (vec![], Some(lost(vec![2], None))));
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
    fn a_teammate_who_leaves_is_told() {
        let (mut app, tanks) = game(&[0, 1, 2]);
        app.update();
        // Player 3 leaves: their tank goes, and the run is for two players.
        app.world_mut().entity_mut(tanks[2]).despawn();
        app.world_mut().resource_mut::<Run>().players = 2;
        app.update();
        let left = Told::Left {
            seat: 2,
            players: 2,
            arena: false,
        };
        assert_eq!(heard(&mut app), (vec![], Some(left)));
        assert_eq!(app.world().resource::<Alarms>().left(2), Some(0.0));
        assert_eq!(left_words(2, false), "The next sectors are for 2 players");
        assert_eq!(left_words(1, true), "The next groups are for 1 player");
        // A new run on another seed is no leaving.
        app.world_mut().entity_mut(tanks[1]).despawn();
        let mut run = app.world_mut().resource_mut::<Run>();
        (run.seed, run.players) = (2, 1);
        app.update();
        assert_eq!(app.world().resource::<Alarms>().left(1), None);
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
        // A life that is taken back blinks too, as the third.
        app.world_mut().resource_mut::<Run>().lives = 3;
        app.world_mut()
            .write_message(SimEvent::LifeTaken { at: Vec2::ZERO });
        app.update();
        assert_eq!(app.world().resource::<Alarms>().back, Some((3, 0.0)));
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
        // Next to an achievement, it takes a smaller stamp with fewer words.
        assert_eq!(
            words.brief.as_deref(),
            Some("4 sectors \u{b7} the best was 3")
        );
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
        // A playtest that fakes records has them stamped.
        let mut fake = Records::fake();
        fake.submit(League::default(), 1, 3, "FAKE");
        let mut app = earning(fake);
        app.world_mut().resource_mut::<Run>().playtest = true;
        app.world_mut().resource_mut::<Run>().cleared = 4;
        app.update();
        assert_eq!(stamped(&mut app), (1, vec![], vec![Cue::Stamp]));
    }
}
