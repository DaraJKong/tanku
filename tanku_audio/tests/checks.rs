//! What must hold for any piece that `music.score` writes: the rules of `docs/MUSIC.md` that a
//! change of a note, of a pattern or of a level could break. Nothing here says what the piece
//! is, so these are the tests to run after a change of the file:
//!
//! ```bash
//! cargo test -p tanku_audio --test checks
//! ```

mod common;

use common::{arrangements, at, bar, fight, loudness, play, restless, sections};
use tanku_audio::{
    CHANNELS, Context, Place,
    director::{Director, Step},
    music, music_from,
    notation::{Form, Instrument, Layer, Part, Scene, Source, THREATS},
    score::{
        ARENA, BARS, CONNECTION_STEPS, FIVE, HOME, LINK, Line, Mode, SCORE, STEPS, rubs, triad,
    },
};

/// The mode that a scene is heard in: that of the piece, or that of the fights of an arena.
fn mode(scene: Scene) -> Mode {
    if scene == Scene::Arena { ARENA } else { HOME }
}

/// The lines that the instruments play in a scene, at any threat.
fn lines(scene: Scene) -> Vec<Source> {
    let piece = &SCORE.piece;
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
fn forms(scene: Scene) -> Vec<Form> {
    let played = |form: &Form| {
        let lines = lines(scene);
        lines
            .iter()
            .any(|line| matches!(line, Source::Tune { form: other, .. } if other == form))
    };
    Form::ALL.into_iter().filter(played).collect()
}

/// The melody of a section in a form, by the place of the section among those of the piece.
fn melody(section: u8, form: Form) -> [Line; BARS] {
    std::array::from_fn(|bar| *SCORE.line(section, form, bar))
}

/// A pitch as the name of its note, such as E2 for pitch 40.
fn note(pitch: u8) -> String {
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!("{}{}", names[pitch as usize % 12], pitch as i32 / 12 - 1)
}

#[test]
fn the_melodies_of_the_fights_use_five_degrees() {
    // Degrees 1, 2, 3, 5 and 6, in any octave: a tune of these holds in both modes, and its
    // ornaments stay among them.
    for scene in [Scene::Corridor, Scene::Arena] {
        for (_, passage) in sections(scene) {
            let cells = passage.melody.iter().flatten();
            for cell in cells.filter(|cell| cell.degree != 0) {
                let within = (cell.degree - 1) % 7 + 1;
                assert!(
                    FIVE.contains(&within),
                    "degree {} in the {:?}",
                    cell.degree,
                    passage.name
                );
            }
        }
    }
}

#[test]
fn every_chord_is_major_or_minor() {
    for scene in Scene::ALL {
        let mode = mode(scene);
        for (_, passage) in sections(scene) {
            for chord in mode.chords(passage.chords) {
                let [root, third, fifth] = triad(mode, chord, 0);
                assert!(
                    matches!(third - root, 3 | 4) && fifth - root == 7,
                    "the chord on degree {chord} of the {:?} in {mode:?}",
                    passage.name
                );
            }
        }
    }
}

#[test]
fn no_note_rubs_against_the_root_of_its_chord() {
    // Every section in the mode of each scene that plays it, in every form of its melody
    // that the scene plays: as it is written, as its skeleton, with its ornaments.
    for scene in Scene::ALL {
        let mode = mode(scene);
        for (index, passage) in sections(scene) {
            let chords = mode.chords(passage.chords);
            for form in forms(scene) {
                let count = rubs(mode, &melody(index, form), &chords);
                assert!(
                    count == 0,
                    "notes of the {} of the {:?} that stay a semitone above the root of their \
                     chord in {mode:?}: {count}",
                    form.name(),
                    passage.name
                );
            }
        }
    }
}

#[test]
fn the_calm_has_no_mode() {
    // What is heard when nobody fights, between two fights of either kind: the melody of its
    // sections in the forms that are played, and the degrees that its patterns write.
    let mut heard = Vec::new();
    for (index, _) in sections(Scene::Calm) {
        for form in forms(Scene::Calm) {
            let cells = melody(index, form).into_iter().flatten();
            heard.extend(cells.map(|cell| cell.degree));
        }
    }
    for line in lines(Scene::Calm) {
        if let Source::Rhythm { index, .. } = line {
            let digits = SCORE.piece.pattern(index).grid.into_iter();
            heard.extend(digits.filter(u8::is_ascii_digit).map(|digit| digit - b'0'));
        }
    }
    heard.retain(|&degree| degree != 0);
    assert!(!heard.is_empty());
    for degree in heard {
        // The same note in both modes, and not the third degree, which tells major from
        // minor.
        let [home, arena] = [HOME, ARENA].map(|mode| mode.semitones(degree));
        assert!(
            home == arena,
            "degree {degree} of the calm is another note in {HOME:?} than in {ARENA:?}"
        );
        let within = (degree - 1) % 7 + 1;
        assert!(within != 3, "the calm has the third degree");
        // Never a semitone above the degree that a connection leads to, on which it stays.
        let above = (home + 12 - HOME.semitones(LINK.degree)) % 12;
        assert!(
            above != 1,
            "degree {degree} of the calm is a semitone above the degree it stays on"
        );
    }
}

#[test]
fn every_instrument_stays_in_its_range() {
    // From E2 to D4 for the bass, from E3 to A5 for the pluck and from A4 to B6 for the lead.
    let ranges = [("bass", 40, 62), ("pluck", 52, 81), ("lead", 69, 95)];
    let mut heard = [(u8::MAX, u8::MIN); 3];
    let mut listen = |step: &Step| {
        let tones = [step.bass, step.pluck, step.lead[0], step.lead[1]];
        for (index, tone) in tones.into_iter().enumerate() {
            if let Some(tone) = tone {
                let (lowest, highest) = &mut heard[index.min(2)];
                *lowest = tone.pitch.min(*lowest);
                *highest = tone.pitch.max(*highest);
            }
        }
    };
    // Two turns of every arrangement.
    for arrangement in arrangements() {
        let mut director = Director::settled(arrangement.context);
        for _ in 0..4 * BARS * STEPS {
            listen(&director.step(arrangement.context));
        }
    }
    // Then every way out of a fight, wherever it ends: a connection, or the note that the
    // bass holds while a tank is destroyed.
    for place in [Place::Corridor, Place::Arena] {
        for threat in 0..=4 {
            let fight = fight(place, threat);
            let down = Context {
                down: true,
                ..fight
            };
            for fought in 0..2 * BARS * STEPS {
                for after in [at(place), down] {
                    let mut director = Director::settled(fight);
                    for _ in 0..fought {
                        director.step(fight);
                    }
                    for _ in 0..STEPS + CONNECTION_STEPS as usize {
                        listen(&director.step(after));
                    }
                }
            }
        }
    }
    for ((name, low, high), (lowest, highest)) in ranges.into_iter().zip(heard) {
        assert!(
            low <= lowest && highest <= high,
            "the {name} plays from {} to {}, and its range is from {} to {}",
            note(lowest),
            note(highest),
            note(low),
            note(high)
        );
    }
}

#[test]
fn every_bar_makes_sound() {
    for arrangement in arrangements() {
        let frames = play(&mut music_from(arrangement.context).0, 16 * arrangement.bar);
        let bars = frames.chunks(arrangement.bar * CHANNELS as usize);
        for (bar, frames) in bars.enumerate() {
            assert!(
                loudness(frames) > 0.01,
                "bar {bar} of the {}",
                arrangement.name
            );
        }
    }
    // The music never stops, whatever happens.
    let (mut music, conductor) = music();
    let frames = restless(&mut music, &conductor, 120);
    let fight = bar(sections(Scene::Corridor)[0].1);
    let bars = frames.chunks(fight * CHANNELS as usize);
    for (bar, frames) in bars.enumerate() {
        assert!(loudness(frames) > 0.002, "bar {bar}");
    }
}

#[test]
fn nothing_is_too_loud() {
    let in_range = |frames: &[f32]| frames.iter().all(|frame| frame.abs() < 1.0);
    for arrangement in arrangements() {
        let frames = play(&mut music_from(arrangement.context).0, 16 * arrangement.bar);
        assert!(in_range(&frames), "the {}", arrangement.name);
        // Loud enough to listen to, and far from the top. The bridge is the quietest: a good
        // part of it is silence.
        let loud = loudness(&frames);
        assert!(
            (0.03..0.2).contains(&loud),
            "the {}: {loud}",
            arrangement.name
        );
    }
    let (mut music, conductor) = music();
    assert!(in_range(&restless(&mut music, &conductor, 120)));
}

#[test]
fn loops_are_seamless() {
    // The third turn of a loop is the second one again, bit for bit: nothing drifts.
    for arrangement in arrangements() {
        let mut music = music_from(arrangement.context).0;
        play(&mut music, arrangement.turn);
        let second = play(&mut music, arrangement.turn);
        let third = play(&mut music, arrangement.turn);
        assert!(second == third, "the {}", arrangement.name);
    }
}
