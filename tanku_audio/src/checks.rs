//! What must hold for any piece that a file writes: the rules of `docs/MUSIC.md` that a change
//! of a note, of a pattern or of a level could break. Nothing here says what the piece is.
//!
//! The tests of the crate run them on the piece of the game, and a program that changes a
//! piece runs them on each state of it. A check gives the list of what it found wrong, each
//! in a sentence, and an empty list when it holds.
//!
//! Five of them only read the piece and are done in a moment. The other three listen to it:
//! they render minutes of music, which takes about a second.

use std::sync::Arc;

use crate::{
    CHANNELS, Conductor, Context, Music, Place, SAMPLE_RATE,
    director::{Director, Step},
    music_of,
    notation::{Form, Instrument, Layer, Part, Passage, Scene, Source, THREATS},
    score::{BARS, BEAT, FIVE, Line, Mode, STEPS, Score, rubs, step, triad},
};

/// The checks, in the order in which they are told.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Check {
    FiveDegrees,
    Chords,
    Rubs,
    Calm,
    Ranges,
    Sounding,
    Loudness,
    Seamless,
}

impl Check {
    /// The checks that only read the piece.
    pub const QUICK: [Self; 5] = [
        Self::FiveDegrees,
        Self::Chords,
        Self::Rubs,
        Self::Calm,
        Self::Ranges,
    ];
    /// The checks that listen to the piece.
    pub const SLOW: [Self; 3] = [Self::Sounding, Self::Loudness, Self::Seamless];

    /// What the check is called, in a few words.
    pub const fn title(self) -> &'static str {
        match self {
            Self::FiveDegrees => "Five degrees",
            Self::Chords => "Major or minor chords",
            Self::Rubs => "No note rubs against a root",
            Self::Calm => "The calm has no mode",
            Self::Ranges => "In range",
            Self::Sounding => "No bar is silent",
            Self::Loudness => "Never too loud",
            Self::Seamless => "Seamless loops",
        }
    }

    /// What holds when the check does.
    pub const fn promise(self) -> &'static str {
        match self {
            Self::FiveDegrees => "The melodies of the fights only use degrees 1, 2, 3, 5 and 6.",
            Self::Chords => "Every chord is a plain triad in the mode that it is heard in.",
            Self::Rubs => "No note stays a semitone above the root of its chord.",
            Self::Calm => "What plays when nobody fights is the same notes in both modes.",
            Self::Ranges => {
                "The bass, the pluck and the lead stay in their ranges, whatever happens."
            }
            Self::Sounding => "Every bar of every arrangement makes sound.",
            Self::Loudness => "No frame of any arrangement goes past full scale.",
            Self::Seamless => "The third turn of a loop is its second one again, to the bit.",
        }
    }

    /// What the check finds wrong with a piece: nothing when it holds.
    pub fn run(self, score: &Arc<Score>) -> Vec<String> {
        let mut found = match self {
            Self::FiveDegrees => five_degrees(score),
            Self::Chords => chords(score),
            Self::Rubs => rubbing(score),
            Self::Calm => calm(score),
            Self::Ranges => ranges(score),
            Self::Sounding => sounding(score),
            Self::Loudness => loudness_of_all(score),
            Self::Seamless => seamless(score),
        };
        // A section that two scenes play is found wrong twice.
        found.dedup();
        found
    }
}

/// The mode that a scene is heard in: that of the piece, or that of the fights of an arena.
pub fn mode(score: &Score, scene: Scene) -> Mode {
    let key = score.piece.key;
    if scene == Scene::Arena {
        key.arena
    } else {
        key.home
    }
}

/// The sections that a scene plays in turn, each with its place among those of the piece.
pub fn sections(score: &Score, scene: Scene) -> Vec<(u8, &Passage)> {
    let places = score.piece.play(scene).sections().iter();
    places
        .map(|&index| (index, score.piece.section(index)))
        .collect()
}

/// The lines that the instruments play in a scene, at any threat.
pub fn lines(score: &Score, scene: Scene) -> Vec<Source> {
    let piece = &score.piece;
    let mut lines = Vec::new();
    for instrument in Instrument::ALL {
        let layers: Vec<Layer> = match piece.play(scene).part(instrument) {
            Part::Silent => Vec::new(),
            Part::Plays(layer) => vec![layer],
            Part::ByThreat => {
                let threats = 0..THREATS as u8;
                threats
                    .filter_map(|threat| piece.threat(threat, instrument))
                    .collect()
            }
        };
        for layer in layers {
            lines.push(layer.first);
            lines.extend(layer.second);
        }
    }
    lines
}

/// The forms of the melody that a scene plays.
pub fn forms(score: &Score, scene: Scene) -> Vec<Form> {
    let lines = lines(score, scene);
    let played = |form: &Form| {
        lines
            .iter()
            .any(|line| matches!(line, Source::Tune { form: other, .. } if other == form))
    };
    Form::ALL.into_iter().filter(played).collect()
}

/// The melody of a section in a form, by the place of the section among those of the piece.
fn melody(score: &Score, section: u8, form: Form) -> [Line; BARS] {
    std::array::from_fn(|bar| *score.line(section, form, bar))
}

/// A pitch as the name of its note, such as E2 for pitch 40.
pub fn note(pitch: u8) -> String {
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", names[pitch as usize % 12], pitch as i32 / 12 - 1)
}

/// Degrees 1, 2, 3, 5 and 6, in any octave: a tune of these holds in both modes, and its
/// ornaments stay among them.
fn five_degrees(score: &Score) -> Vec<String> {
    let mut found = Vec::new();
    for scene in [Scene::Corridor, Scene::Arena] {
        for (_, passage) in sections(score, scene) {
            let cells = passage.melody.iter().flatten();
            for cell in cells.filter(|cell| cell.degree != 0) {
                let within = (cell.degree - 1) % 7 + 1;
                if !FIVE.contains(&within) {
                    found.push(format!("degree {} in the {:?}", cell.degree, passage.name));
                }
            }
        }
    }
    found.sort();
    found
}

fn chords(score: &Score) -> Vec<String> {
    let mut found = Vec::new();
    for scene in Scene::ALL {
        let mode = mode(score, scene);
        for (_, passage) in sections(score, scene) {
            for chord in mode.chords(passage.chords) {
                let [root, third, fifth] = triad(mode, chord, 0);
                if !(matches!(third - root, 3 | 4) && fifth - root == 7) {
                    found.push(format!(
                        "the chord on degree {chord} of the {:?} in {mode:?}",
                        passage.name
                    ));
                }
            }
        }
    }
    found
}

/// Every section in the mode of each scene that plays it, in every form of its melody that the
/// scene plays: as it is written, as its skeleton, with its ornaments.
fn rubbing(score: &Score) -> Vec<String> {
    let mut found = Vec::new();
    for scene in Scene::ALL {
        let mode = mode(score, scene);
        for (index, passage) in sections(score, scene) {
            let chords = mode.chords(passage.chords);
            for form in forms(score, scene) {
                let count = rubs(mode, &melody(score, index, form), &chords);
                if count != 0 {
                    found.push(format!(
                        "notes of the {} of the {:?} that stay a semitone above the root of \
                         their chord in {mode:?}: {count}",
                        form.name(),
                        passage.name
                    ));
                }
            }
        }
    }
    found
}

/// What is heard when nobody fights, between two fights of either kind: the melody of its
/// sections in the forms that are played, and the degrees that its patterns write.
fn calm(score: &Score) -> Vec<String> {
    let key = score.piece.key;
    let mut heard = Vec::new();
    for (index, _) in sections(score, Scene::Calm) {
        for form in forms(score, Scene::Calm) {
            let cells = melody(score, index, form).into_iter().flatten();
            heard.extend(cells.map(|cell| cell.degree));
        }
    }
    for line in lines(score, Scene::Calm) {
        if let Source::Rhythm { index, .. } = line {
            let digits = score.piece.pattern(index).grid.into_iter();
            heard.extend(digits.filter(u8::is_ascii_digit).map(|digit| digit - b'0'));
        }
    }
    heard.retain(|&degree| degree != 0);
    heard.sort_unstable();
    heard.dedup();
    let mut found = Vec::new();
    if heard.is_empty() {
        found.push("the calm writes no note".to_string());
    }
    for degree in heard {
        // The same note in both modes, and not the third degree, which tells major from
        // minor.
        let [home, arena] = [key.home, key.arena].map(|mode| mode.semitones(degree));
        if home != arena {
            found.push(format!(
                "degree {degree} of the calm is another note in {:?} than in {:?}",
                key.home, key.arena
            ));
        }
        if (degree - 1) % 7 + 1 == 3 {
            found.push("the calm has the third degree".to_string());
        }
        // Never a semitone above the degree that a connection leads to, on which it stays.
        let above = (home + 12 - key.home.semitones(score.piece.link.degree)) % 12;
        if above == 1 {
            found.push(format!(
                "degree {degree} of the calm is a semitone above the degree it stays on"
            ));
        }
    }
    found
}

/// The ranges of the instruments that have a pitch: from E2 to D4 for the bass, from E3 to A5
/// for the pluck and from A4 to B6 for the lead.
pub const RANGES: [(Instrument, u8, u8); 3] = [
    (Instrument::Bass, 40, 62),
    (Instrument::Pluck, 52, 81),
    (Instrument::Lead, 69, 95),
];

/// The lowest and the highest note that the bass, the pluck and the lead play: in two turns
/// of every arrangement, then on every way out of a fight, wherever it ends. An instrument
/// that plays nothing has `None`.
pub fn heard(score: &Arc<Score>) -> [Option<(u8, u8)>; 3] {
    let mut heard: [Option<(u8, u8)>; 3] = [None; 3];
    let mut listen = |step: &Step| {
        let tones = [step.bass, step.pluck, step.lead[0], step.lead[1]];
        for (index, tone) in tones.into_iter().enumerate() {
            if let Some(tone) = tone {
                let (lowest, highest) = heard[index.min(2)].unwrap_or((u8::MAX, u8::MIN));
                heard[index.min(2)] = Some((tone.pitch.min(lowest), tone.pitch.max(highest)));
            }
        }
    };
    for arrangement in arrangements(score) {
        let mut director = Director::settled_in(score.clone(), arrangement.context);
        let steps = 2 * arrangement.turn_bars * STEPS;
        for _ in 0..steps {
            listen(&director.step(arrangement.context));
        }
    }
    // A connection, or the note that the bass holds while a tank is destroyed.
    let link = score.piece.link.beats as usize * BEAT;
    for place in [Place::Corridor, Place::Arena] {
        for threat in 0..THREATS as u8 {
            let fight = fight(place, threat);
            let down = Context {
                down: true,
                ..fight
            };
            for fought in 0..2 * BARS * STEPS {
                for after in [at(place), down] {
                    let mut director = Director::settled_in(score.clone(), fight);
                    for _ in 0..fought {
                        director.step(fight);
                    }
                    for _ in 0..STEPS + link {
                        listen(&director.step(after));
                    }
                }
            }
        }
    }
    heard
}

fn ranges(score: &Arc<Score>) -> Vec<String> {
    let mut found = Vec::new();
    for ((instrument, low, high), heard) in RANGES.into_iter().zip(heard(score)) {
        let Some((lowest, highest)) = heard else {
            continue;
        };
        if lowest < low || highest > high {
            found.push(format!(
                "the {} plays from {} to {}, and its range is from {} to {}",
                instrument.name(),
                note(lowest),
                note(highest),
                note(low),
                note(high)
            ));
        }
    }
    found
}

pub fn at(place: Place) -> Context {
    Context {
        place,
        ..Context::default()
    }
}

pub fn fight(place: Place, threat: u8) -> Context {
    Context {
        place,
        fight: true,
        threat,
        ..Context::default()
    }
}

/// Frames in a bar of a section. A step is counted in half frames.
pub fn bar(passage: &Passage) -> usize {
    STEPS * step(passage.tempo) as usize / 2
}

/// An arrangement of a piece.
#[derive(Clone, Debug)]
pub struct Arrangement {
    pub name: String,
    pub context: Context,
    /// Frames in its first bar, and in one turn of its loop.
    pub bar: usize,
    pub turn: usize,
    /// Bars in one turn of its loop.
    pub turn_bars: usize,
}

/// Every arrangement of a piece: the menu, the fights of a corridor at each threat, the calm,
/// and the fights of an arena at each threat.
pub fn arrangements(score: &Score) -> Vec<Arrangement> {
    let arrangement = |name: String, scene: Scene, context: Context| {
        let bars = sections(score, scene).into_iter();
        let bars: Vec<usize> = bars.map(|(_, passage)| bar(passage)).collect();
        Arrangement {
            name,
            context,
            bar: bars[0],
            turn: BARS * bars.iter().sum::<usize>(),
            turn_bars: BARS * bars.len(),
        }
    };
    let mut all = vec![arrangement("menu".into(), Scene::Menu, at(Place::Bay))];
    for threat in 0..THREATS as u8 {
        let name = format!("corridor at threat {threat}");
        all.push(arrangement(
            name,
            Scene::Corridor,
            fight(Place::Corridor, threat),
        ));
    }
    all.push(arrangement("calm".into(), Scene::Calm, at(Place::Corridor)));
    for threat in 0..THREATS as u8 {
        let name = format!("arena at threat {threat}");
        all.push(arrangement(name, Scene::Arena, fight(Place::Arena, threat)));
    }
    all
}

/// The next frames of a music.
pub fn play(music: &mut Music, frames: usize) -> Vec<f32> {
    let mut frames = vec![0.0; frames * CHANNELS as usize];
    music.render(&mut frames);
    frames
}

/// How loud some frames are on average.
pub fn loudness(frames: &[f32]) -> f32 {
    (frames.iter().map(|frame| frame * frame).sum::<f32>() / frames.len() as f32).sqrt()
}

/// Contexts that change at random moments, from a small generator of its own.
pub fn restless(music: &mut Music, conductor: &Conductor, seconds: usize) -> Vec<f32> {
    let mut state = 12_345u32;
    let mut draw = |below: u32| {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) % below
    };
    let places = [Place::Menu, Place::Bay, Place::Corridor, Place::Arena];
    let mut frames = Vec::new();
    while frames.len() < seconds * SAMPLE_RATE as usize * CHANNELS as usize {
        conductor.set(Context {
            place: places[draw(4) as usize],
            fight: draw(2) == 0,
            threat: draw(5) as u8,
            down: draw(6) == 0,
            muffled: draw(6) == 0,
            ducked: draw(6) == 0,
        });
        // Anything from a few frames to three seconds.
        let mut part = vec![0.0; (1 + draw(3 * SAMPLE_RATE) as usize) * CHANNELS as usize];
        music.render(&mut part);
        frames.extend(part);
    }
    frames
}

/// A music at the start of a piece, in the main menu, and its conductor.
fn from_the_start(score: &Arc<Score>) -> (Music, Conductor) {
    music_of(score.clone(), Context::default())
}

fn sounding(score: &Arc<Score>) -> Vec<String> {
    let mut found = Vec::new();
    for arrangement in arrangements(score) {
        let mut music = music_of(score.clone(), arrangement.context).0;
        let frames = play(&mut music, 16 * arrangement.bar);
        let bars = frames.chunks(arrangement.bar * CHANNELS as usize);
        for (bar, frames) in bars.enumerate() {
            if loudness(frames) <= 0.01 {
                found.push(format!(
                    "bar {} of the {} is silent",
                    bar + 1,
                    arrangement.name
                ));
            }
        }
    }
    // The music never stops, whatever happens.
    let (mut music, conductor) = from_the_start(score);
    let frames = restless(&mut music, &conductor, 120);
    let fight = bar(sections(score, Scene::Corridor)[0].1);
    let bars = frames.chunks(fight * CHANNELS as usize);
    let silent = bars.filter(|frames| loudness(frames) <= 0.002).count();
    if silent > 0 {
        found.push(format!(
            "{silent} bars are silent while the game goes from one thing to another"
        ));
    }
    found
}

fn loudness_of_all(score: &Arc<Score>) -> Vec<String> {
    let in_range = |frames: &[f32]| frames.iter().all(|frame| frame.abs() < 1.0);
    let mut found = Vec::new();
    for arrangement in arrangements(score) {
        let mut music = music_of(score.clone(), arrangement.context).0;
        let frames = play(&mut music, 16 * arrangement.bar);
        if !in_range(&frames) {
            found.push(format!("the {} goes past full scale", arrangement.name));
        }
        // Loud enough to listen to, and far from the top. The bridge is the quietest: a good
        // part of it is silence.
        let loud = loudness(&frames);
        if loud < 0.03 {
            found.push(format!("the {} is too quiet: {loud:.3}", arrangement.name));
        } else if loud >= 0.2 {
            found.push(format!("the {} is too loud: {loud:.3}", arrangement.name));
        }
    }
    let (mut music, conductor) = from_the_start(score);
    if !in_range(&restless(&mut music, &conductor, 120)) {
        found.push("a change of the game goes past full scale".to_string());
    }
    found
}

/// The third turn of a loop is the second one again, bit for bit: nothing drifts.
fn seamless(score: &Arc<Score>) -> Vec<String> {
    let mut found = Vec::new();
    for arrangement in arrangements(score) {
        let mut music = music_of(score.clone(), arrangement.context).0;
        play(&mut music, arrangement.turn);
        let second = play(&mut music, arrangement.turn);
        let third = play(&mut music, arrangement.turn);
        if second != third {
            found.push(format!("the {} doesn't repeat", arrangement.name));
        }
    }
    found
}

/// The peak of the loudest arrangement of a piece, over sixteen bars of each, as a share of
/// full scale: what the levels of the game are set against.
pub fn peak(score: &Arc<Score>) -> f32 {
    let mut peak = 0.0f32;
    for arrangement in arrangements(score) {
        let mut music = music_of(score.clone(), arrangement.context).0;
        let frames = play(&mut music, 16 * arrangement.bar);
        peak = frames
            .iter()
            .fold(peak, |peak, frame| peak.max(frame.abs()));
    }
    peak
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{notation::read, score::game};

    #[test]
    fn the_piece_of_the_game_holds_what_only_reads_it() {
        for check in Check::QUICK {
            assert_eq!(check.run(&game()), Vec::<String>::new(), "{check:?}");
        }
    }

    #[test]
    fn a_check_says_what_is_wrong_in_a_sentence() {
        let with = |old: &str, new: &str| {
            let text = include_str!("../music.score");
            assert!(text.contains(old), "{old}");
            Arc::new(Score::new(read(&text.replace(old, new)).unwrap()))
        };
        // A fourth degree in the chorus.
        let fourth = with("motif B 8... ..9.", "motif B 4... ..9.");
        let found = Check::FiveDegrees.run(&fourth);
        assert_eq!(found, ["degree 4 in the chorus"]);
        // A chord on degree 6, which is diminished in Dorian.
        let sixth = with("sec chorus 451C", "sec chorus 4516");
        let found = Check::Chords.run(&sixth);
        assert_eq!(found, ["the chord on degree 6 of the chorus in Dorian"]);
        // A lead two octaves down.
        let low = with("lead=melody@4", "lead=melody@2");
        let found = Check::Ranges.run(&low);
        assert_eq!(
            found,
            ["the lead plays from A2 to B6, and its range is from A4 to B6"]
        );
        // A calm with the third degree, which is the same note in both modes of the piece
        // and tells a major key from a minor one.
        let third = with("pat pluck tick     5.9.", "pat pluck tick     3.9.");
        assert_eq!(Check::Calm.run(&third), ["the calm has the third degree"]);
    }

    #[test]
    fn every_check_has_its_words() {
        let all = Check::QUICK.into_iter().chain(Check::SLOW);
        for check in all {
            assert!(!check.title().is_empty() && check.promise().ends_with('.'));
        }
    }
}
