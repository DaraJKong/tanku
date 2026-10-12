//! The checks that the window shows: those that hold for any piece, from `tanku_audio`, and a
//! few of the editor's own about the tempo, the cues and the cost of the sound.
//!
//! Some are told at once. The others listen to minutes of music, so they run on a thread of
//! their own after each change, and the window shows them as running until they are back.

use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, channel},
    },
    time::Instant,
};

use bevy::prelude::Resource;
use tanku_audio::{
    CHANNELS, Context, MUSIC_LEVEL, Place, SAMPLE_RATE,
    checks::{self, Check},
    cues::Cue,
    music_of,
    score::{Score, step},
};

use crate::state::Tab;

/// How a check stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Standing {
    Running,
    Holds,
    /// What it found wrong, each in a sentence.
    Fails(Vec<String>),
}

/// A check of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    /// One that holds for any piece.
    Piece(Check),
    /// The timings of the game are whole numbers of steps at the tempo of the piece.
    Beats,
    /// A step is a whole number of half frames at each tempo.
    Steps,
    /// The cues of the game play notes of the piece.
    Cues,
    /// The music is as far under the cues as the game sets it.
    UnderCues,
    /// The sound is cheap enough to compute while it plays.
    Cost,
    /// The piece sounds as the file that was last saved does.
    SameSound,
}

/// The rules of the game that the music is set against, and the seconds that each lasts.
/// `docs/PROJECT.md` has them.
pub const RULES: [(&str, f32); 6] = [
    ("Time between two shots of a gun", 0.25),
    ("Shield, time between two mines", 0.5),
    ("Reload of one round, arming of a mine", 1.0),
    ("Respawn", 1.5),
    ("Wait before the first wave, repair", 2.0),
    ("Fuse of a mine", 10.0),
];

/// Whether so many seconds are a whole number of steps at a tempo.
pub fn on_a_step(seconds: f32, tempo: u16) -> bool {
    let step = 60.0 / tempo as f32 / 4.0;
    ((seconds / step).round() * step - seconds).abs() <= 1e-4
}
/// The decibels that the game keeps the loudest music under a shot, and how far from them a
/// piece can be.
const UNDER: f32 = 12.0;
const UNDER_SLACK: f32 = 0.5;
/// Milliseconds that a second of the fullest music may take to compute.
const COST_LIMIT: f32 = 10.0;

impl Item {
    /// The checks of a tab, in the order in which its panel tells them.
    pub fn of(tab: Tab) -> &'static [Self] {
        use Check::*;
        match tab {
            Tab::Motifs => &[
                Self::Piece(FiveDegrees),
                Self::Piece(Rubs),
                Self::Piece(Ranges),
            ],
            Tab::Patterns => &[Self::Piece(Calm), Self::Piece(Ranges)],
            Tab::Sections => &[
                Self::Piece(Chords),
                Self::Piece(Rubs),
                Self::Piece(Calm),
                Self::Piece(Sounding),
            ],
            Tab::Transitions => &[
                Self::Piece(Ranges),
                Self::Piece(Sounding),
                Self::Piece(Seamless),
            ],
            Tab::Instruments => &[Self::Piece(Loudness), Self::UnderCues, Self::Cost],
            Tab::Piece => &[Self::Beats, Self::Steps, Self::Cues, Self::Piece(Chords)],
            Tab::File => &ALL,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Piece(check) => check.title(),
            Self::Beats => "The rules land on beats",
            Self::Steps => "Nothing is rounded",
            Self::Cues => "Cues in tune",
            Self::UnderCues => "Under the cues",
            Self::Cost => "Cost",
            Self::SameSound => "Same sound as the saved file",
        }
    }

    /// What holds when the check does.
    pub fn promise(self) -> &'static str {
        match self {
            Self::Piece(check) => check.promise(),
            Self::Beats => "The six timings of the game are whole numbers of steps at this tempo.",
            Self::Steps => "A step is a whole number of half frames at every tempo of the piece.",
            Self::Cues => "The cues play A and E, which both modes of the piece have.",
            Self::UnderCues => "The loudest arrangement peaks 12 dB below a shot.",
            Self::Cost => "A second of the fullest arrangement is computed in under 10 ms.",
            Self::SameSound => "Every arrangement gives the samples that the saved file gives.",
        }
    }

    /// Whether the check is told at once, or runs on its own thread.
    fn quick(self) -> bool {
        match self {
            Self::Piece(check) => !matches!(check, Check::Ranges) && Check::QUICK.contains(&check),
            Self::Beats | Self::Steps | Self::Cues => true,
            Self::UnderCues | Self::Cost | Self::SameSound => false,
        }
    }
}

/// Every check, in the order in which the file tab counts them.
const ALL: [Item; 14] = [
    Item::Piece(Check::FiveDegrees),
    Item::Piece(Check::Chords),
    Item::Piece(Check::Rubs),
    Item::Piece(Check::Calm),
    Item::Piece(Check::Ranges),
    Item::Piece(Check::Sounding),
    Item::Piece(Check::Loudness),
    Item::Piece(Check::Seamless),
    Item::Beats,
    Item::Steps,
    Item::Cues,
    Item::UnderCues,
    Item::Cost,
    Item::SameSound,
];

/// Runs a check on a piece. `saved` is the piece of the file as it was last saved.
fn run(item: Item, score: &Arc<Score>, saved: &Arc<Score>) -> Standing {
    let found = match item {
        Item::Piece(check) => check.run(score),
        Item::Beats => beats(score),
        Item::Steps => steps(score),
        Item::Cues => cues(score),
        Item::UnderCues => under_cues(score),
        Item::Cost => cost(score),
        Item::SameSound => same_sound(score, saved),
    };
    if found.is_empty() {
        Standing::Holds
    } else {
        Standing::Fails(found)
    }
}

fn beats(score: &Score) -> Vec<String> {
    let tempo = score.piece.tempo;
    let off = |rule: &&(&str, f32)| !on_a_step(rule.1, tempo);
    match RULES.iter().filter(off).count() {
        0 => Vec::new(),
        count => vec![format!(
            "At {tempo} beats per minute, {count} of the six timings of the game end between \
             two steps."
        )],
    }
}

fn steps(score: &Score) -> Vec<String> {
    let piece = &score.piece;
    let mut tempos: Vec<u16> = piece
        .passages()
        .iter()
        .map(|passage| passage.tempo)
        .collect();
    tempos.push(piece.tempo);
    tempos.sort_unstable();
    tempos.dedup();
    let minute = 60 * 2 * SAMPLE_RATE;
    let rounded = |tempo: &&u16| !minute.is_multiple_of(4 * **tempo as u32);
    tempos
        .iter()
        .filter(rounded)
        .map(|tempo| {
            let length = step(*tempo);
            format!(
                "At {tempo} beats per minute a step is rounded to {length} half frames: the \
                 tempo is a hair off."
            )
        })
        .collect()
}

fn cues(score: &Score) -> Vec<String> {
    let key = score.piece.key;
    let mut found = Vec::new();
    for mode in [key.home, key.arena] {
        let notes: Vec<u8> = (1..=7)
            .map(|degree| (key.tonic + mode.semitones(degree)) % 12)
            .collect();
        // A and E, in semitones from A.
        for (name, note) in [("A", 0), ("E", 7)] {
            if !notes.contains(&note) {
                found.push(format!(
                    "The cues play {name}, which {mode:?} doesn't have in this key."
                ));
            }
        }
    }
    found.dedup();
    found
}

fn under_cues(score: &Arc<Score>) -> Vec<String> {
    let peak = checks::peak(score);
    let shot = Cue::Shot
        .samples()
        .into_iter()
        .map(f32::abs)
        .fold(0.0, f32::max);
    // The level of the music in the game, with its setting where it starts: halfway up.
    let under = 20.0 * (shot / (peak * 0.5 * MUSIC_LEVEL)).log10();
    if (under - UNDER).abs() <= UNDER_SLACK {
        Vec::new()
    } else {
        vec![format!(
            "The loudest arrangement peaks {under:.1} dB below a shot. The game sets it at 12."
        )]
    }
}

fn cost(score: &Arc<Score>) -> Vec<String> {
    let fight = Context {
        place: Place::Arena,
        fight: true,
        threat: 4,
        ..Context::default()
    };
    let seconds = 10;
    let mut music = music_of(score.clone(), fight).0;
    let mut frames = vec![0.0; seconds * SAMPLE_RATE as usize * CHANNELS as usize];
    let start = Instant::now();
    music.render(&mut frames);
    let each = start.elapsed().as_secs_f32() * 1000.0 / seconds as f32;
    if each < COST_LIMIT {
        Vec::new()
    } else {
        vec![format!(
            "A second of the fullest arrangement takes {each:.1} ms. The limit is 10."
        )]
    }
}

fn same_sound(score: &Arc<Score>, saved: &Arc<Score>) -> Vec<String> {
    if Arc::ptr_eq(score, saved) || **score == **saved {
        return Vec::new();
    }
    let (now, then) = (checks::arrangements(score), checks::arrangements(saved));
    let mut different = 0;
    for (now, then) in now.iter().zip(&then) {
        let heard = |score: &Arc<Score>, bar: usize| {
            let mut music = music_of(score.clone(), now.context).0;
            checks::play(&mut music, 8 * bar)
        };
        if now.bar != then.bar || heard(score, now.bar) != heard(saved, then.bar) {
            different += 1;
        }
    }
    match different {
        0 => Vec::new(),
        count => vec![format!(
            "{count} of {} arrangements sound different: the values that the tests store change \
             with them.",
            now.len()
        )],
    }
}

/// What the thread of the slow checks sends back: the state of the piece that it listened to,
/// and how each check stands.
type Report = (u64, Vec<(Item, Standing)>);

#[derive(Resource)]
pub struct Checks {
    standing: Vec<(Item, Standing)>,
    /// The state of the document that the checks are about.
    revision: Option<u64>,
    /// The thread that listens, if one does. The lock is there for the engine, which shares
    /// what it holds between its threads.
    listening: Option<Mutex<Receiver<Report>>>,
}

impl Default for Checks {
    fn default() -> Self {
        Self {
            standing: ALL.map(|item| (item, Standing::Running)).to_vec(),
            revision: None,
            listening: None,
        }
    }
}

impl Checks {
    pub fn standing(&self, item: Item) -> &Standing {
        let found = self.standing.iter().find(|(other, _)| *other == item);
        found.map_or(&Standing::Running, |(_, standing)| standing)
    }

    /// How many of some checks hold, and whether any is still running.
    pub fn count(&self, items: &[Item]) -> (usize, bool) {
        let holds = |item: &&Item| *self.standing(**item) == Standing::Holds;
        let running = |item: &Item| *self.standing(*item) == Standing::Running;
        (
            items.iter().filter(holds).count(),
            items.iter().any(running),
        )
    }

    /// Follows the document: tells at once the checks that only read the piece, and sends
    /// the others to listen to it. Says whether anything it tells has changed.
    pub fn follow(&mut self, revision: u64, score: &Arc<Score>, saved: &Arc<Score>) -> bool {
        let mut changed = false;
        if self.revision != Some(revision) {
            self.revision = Some(revision);
            for (item, standing) in &mut self.standing {
                *standing = if item.quick() {
                    run(*item, score, saved)
                } else {
                    Standing::Running
                };
            }
            changed = true;
            // A thread that listens to an older state is let go: its report is not read.
            self.listening = None;
        }
        match &self.listening {
            None if self
                .standing
                .iter()
                .any(|(_, standing)| *standing == Standing::Running) =>
            {
                let (report, listening) = channel();
                let (score, saved) = (score.clone(), saved.clone());
                std::thread::spawn(move || {
                    let slow = ALL.into_iter().filter(|item| !item.quick());
                    let told = slow.map(|item| (item, run(item, &score, &saved))).collect();
                    // Nobody waits for it if the piece has changed since.
                    let _ = report.send((revision, told));
                });
                self.listening = Some(Mutex::new(listening));
            }
            Some(listening) => {
                let report = listening
                    .lock()
                    .ok()
                    .and_then(|report| report.try_recv().ok());
                if let Some((of, told)) = report {
                    self.listening = None;
                    if Some(of) == self.revision {
                        for (item, standing) in told {
                            if let Some(place) =
                                self.standing.iter_mut().find(|(other, _)| *other == item)
                            {
                                place.1 = standing;
                            }
                        }
                        changed = true;
                    }
                }
            }
            None => {}
        }
        changed
    }

    /// Waits for the checks that listen, for a test or for a script that wants them told.
    #[cfg(test)]
    pub fn settle(&mut self, revision: u64, score: &Arc<Score>, saved: &Arc<Score>) {
        self.follow(revision, score, saved);
        while self.count(&ALL).1 {
            std::thread::sleep(std::time::Duration::from_millis(5));
            self.follow(revision, score, saved);
        }
    }
}

#[cfg(test)]
mod tests {
    use tanku_audio::notation::read;

    use super::*;

    fn with(old: &str, new: &str) -> Arc<Score> {
        let text = crate::fixture::FILE;
        assert!(text.contains(old), "{old}");
        Arc::new(Score::new(read(&text.replace(old, new)).unwrap()))
    }

    #[test]
    fn a_sound_piece_holds_every_check() {
        let mut checks = Checks::default();
        // Before anything is told, everything is running.
        assert_eq!(checks.count(&ALL), (0, true));
        checks.settle(0, &crate::fixture::score(), &crate::fixture::score());
        // The cost is a time, which a busy machine makes longer: a test only asks that it
        // was told, whatever it says.
        let others: Vec<Item> = ALL.into_iter().filter(|item| *item != Item::Cost).collect();
        for item in &others {
            assert_eq!(*checks.standing(*item), Standing::Holds, "{item:?}");
        }
        assert_eq!(checks.count(&others), (13, false));
        assert_ne!(*checks.standing(Item::Cost), Standing::Running);
        // Every tab has its checks, and each of them its words.
        for tab in Tab::ALL {
            let items = Item::of(tab);
            assert!(!items.is_empty());
            assert!(
                items
                    .iter()
                    .all(|item| !item.title().is_empty() && item.promise().ends_with('.'))
            );
        }
    }

    #[test]
    fn the_quick_checks_are_told_at_once() {
        let mut checks = Checks::default();
        let odd = with("tempo 120", "tempo 97");
        assert!(checks.follow(1, &odd, &crate::fixture::score()));
        let Standing::Fails(found) = checks.standing(Item::Beats) else {
            panic!("the timings of the game fall between the steps of this tempo");
        };
        assert_eq!(
            found[0],
            "At 97 beats per minute, 6 of the six timings of the game end between two steps."
        );
        assert!(
            matches!(checks.standing(Item::Steps), Standing::Fails(found) if found[0].starts_with("At 97 beats per minute a step is rounded to 13639 half frames"))
        );
        assert_eq!(
            *checks.standing(Item::Piece(Check::Chords)),
            Standing::Holds
        );
        // The ones that listen are on their way.
        assert_eq!(*checks.standing(Item::SameSound), Standing::Running);
        assert_eq!(
            *checks.standing(Item::Piece(Check::Ranges)),
            Standing::Running
        );

        // In another key the cues are out of it.
        let mut checks = Checks::default();
        checks.follow(
            1,
            &with("key A dorian", "key C dorian"),
            &crate::fixture::score(),
        );
        assert_eq!(
            *checks.standing(Item::Cues),
            Standing::Fails(vec![
                "The cues play E, which Dorian doesn't have in this key.".into(),
                "The cues play A, which Aeolian doesn't have in this key.".into(),
                "The cues play E, which Aeolian doesn't have in this key.".into()
            ])
        );
    }

    #[test]
    fn the_checks_that_listen_come_back_for_the_state_they_heard() {
        let mut checks = Checks::default();
        // A lead that is ten times as loud. Three arrangements have no lead: the menu, and a
        // fight at threat 0 in a corridor and in an arena.
        let loud = with("mix lead   5%  8%", "mix lead  50% 80%");
        checks.settle(3, &loud, &crate::fixture::score());
        assert!(
            matches!(checks.standing(Item::UnderCues), Standing::Fails(found) if found[0].starts_with("The loudest arrangement peaks"))
        );
        assert_eq!(
            *checks.standing(Item::SameSound),
            Standing::Fails(vec![
                "9 of 12 arrangements sound different: the values that the tests store change with them.".into()
            ])
        );
        assert_ne!(*checks.standing(Item::Cost), Standing::Running);
        // A change while a thread listens: its report is for a state that is gone, and the
        // window never shows it.
        let mut checks = Checks::default();
        checks.follow(1, &loud, &crate::fixture::score());
        checks.settle(2, &crate::fixture::score(), &crate::fixture::score());
        assert_eq!(*checks.standing(Item::UnderCues), Standing::Holds);
        assert_eq!(*checks.standing(Item::SameSound), Standing::Holds);
    }
}
