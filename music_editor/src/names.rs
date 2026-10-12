//! The names that the window gives to what the file writes as numbers and symbols: a note, a
//! chord, an instrument, where a pattern is played, and what a step of it sounds over a chord.

use tanku_audio::{
    notation::{Instrument, Layer, Part, Piece, Rhythm, STEPS, Scene, Source, THREATS},
    score::{Mode, Score, bass_root, triad},
};

use crate::{actions::TONICS, parts};

/// The note of a degree in a mode of the piece.
pub fn note(piece: &Piece, mode: Mode, degree: u8) -> &'static str {
    TONICS[(piece.key.tonic + mode.semitones(degree)) as usize % 12]
}

/// The chord that a section writes, as musicians name it in a mode: `Am`, `D`, or `F#dim`.
pub fn chord(piece: &Piece, mode: Mode, written: u8) -> String {
    let [root, third, fifth] = triad(mode, mode.chord(written), piece.key.tonic);
    let kind = match (third - root, fifth - root) {
        (3, 6) => "dim",
        (3, _) => "m",
        (4, 8) => "aug",
        _ => "",
    };
    format!("{}{kind}", TONICS[root as usize % 12])
}

/// An instrument as the window names it.
pub const fn instrument(instrument: Instrument) -> &'static str {
    match instrument {
        Instrument::Kick => "Kick",
        Instrument::Hat => "Hi-hat",
        Instrument::Snare => "Snare",
        Instrument::Bass => "Bass",
        Instrument::Pluck => "Pluck",
        Instrument::Lead => "Lead",
    }
}

/// A scene as the window names it.
pub const fn scene(scene: Scene) -> &'static str {
    match scene {
        Scene::Menu => "Menu or start bay",
        Scene::Calm => "Nobody fights",
        Scene::Corridor => "Fight in a corridor",
        Scene::Arena => "Fight in an arena",
    }
}

/// The brain tier that sets a threat.
pub const fn brain(threat: u8) -> &'static str {
    match threat {
        0 => "Sentry",
        1 => "Grunt",
        2 => "Soldier",
        3 => "Veteran",
        _ => "Ace",
    }
}

/// Where the arrangement plays a pattern: the scenes whose line names it, and the threats
/// whose line does, if a scene leaves the instrument to them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Uses {
    pub scenes: Vec<Scene>,
    pub threats: Vec<u8>,
    /// The scenes that leave the instrument to the threat.
    pub left: Vec<Scene>,
    /// Whether it only ever plays under a line of the melody.
    pub under: bool,
    /// The octave that it is first played in, on an instrument that has a pitch.
    pub octave: Option<u8>,
}

/// Where a pattern is played.
pub fn uses(piece: &Piece, instrument: Instrument, name: &str) -> Uses {
    let mut uses = Uses::default();
    let mut rhythms = piece.rhythms().iter();
    let Some(place) =
        rhythms.position(|rhythm| rhythm.instrument == instrument && rhythm.name.is(name))
    else {
        return uses;
    };
    let octave = |source: Source| match source {
        Source::Rhythm { index, octave } if index as usize == place => Some(octave),
        _ => None,
    };
    // The octave of the pattern in a line, and whether it is the second line there.
    let found = |layer: Layer| match (octave(layer.first), layer.second.and_then(octave)) {
        (Some(octave), _) => Some((octave, false)),
        (None, Some(octave)) => Some((octave, matches!(layer.first, Source::Tune { .. }))),
        (None, None) => None,
    };
    let mut under = Vec::new();
    for scene in Scene::ALL {
        match piece.play(scene).part(instrument) {
            Part::Silent => {}
            Part::ByThreat => uses.left.push(scene),
            Part::Plays(layer) => {
                if let Some((octave, second)) = found(layer) {
                    uses.scenes.push(scene);
                    uses.octave = uses.octave.or(Some(octave));
                    under.push(second);
                }
            }
        }
    }
    if !uses.left.is_empty() {
        for threat in 0..THREATS as u8 {
            if let Some((octave, second)) = piece.threat(threat, instrument).and_then(found) {
                uses.threats.push(threat);
                uses.octave = uses.octave.or(Some(octave));
                under.push(second);
            }
        }
    }
    uses.under = !under.is_empty() && under.iter().all(|second| *second);
    if instrument.is_drum() {
        uses.octave = None;
    }
    uses
}

/// A few numbers in a sentence: `2`, `0 and 1`, `2 to 4`, or `0, 2 and 4`.
fn listed(numbers: &[u8]) -> String {
    let run = numbers.windows(2).all(|pair| pair[1] == pair[0] + 1);
    match numbers {
        [] => String::new(),
        [one] => one.to_string(),
        [first, second] => format!("{first} and {second}"),
        [first, .., last] if run => format!("{first} to {last}"),
        [rest @ .., last] => {
            let rest: Vec<String> = rest.iter().map(u8::to_string).collect();
            format!("{} and {last}", rest.join(", "))
        }
    }
}

/// Where a pattern is played, in a few words.
pub fn played_in(piece: &Piece, instrument: Instrument, name: &str) -> String {
    let uses = uses(piece, instrument, name);
    let mut places: Vec<String> = Vec::new();
    let has = |scenes: &[Scene], scene: Scene| scenes.contains(&scene);
    let fights = |scenes: &[Scene]| match (has(scenes, Scene::Corridor), has(scenes, Scene::Arena))
    {
        (true, true) => Some("fights"),
        (true, false) => Some("fights in a corridor"),
        (false, true) => Some("fights in an arena"),
        (false, false) => None,
    };
    if has(&uses.scenes, Scene::Menu) {
        places.push("the menu".to_string());
    }
    if has(&uses.scenes, Scene::Calm) {
        places.push("the calm".to_string());
    }
    match fights(&uses.scenes) {
        Some("fights") => places.push("every fight".to_string()),
        Some(one) => places.push(one.to_string()),
        None => {}
    }
    if !uses.threats.is_empty() {
        let threats = if uses.threats.len() == 1 {
            "threat"
        } else {
            "threats"
        };
        let whose = fights(&uses.left).unwrap_or("the calm");
        places.push(format!("{whose} at {threats} {}", listed(&uses.threats)));
    }
    if places.is_empty() {
        return "Nowhere yet".to_string();
    }
    let mut words = places.join(", ");
    if uses.under {
        words.push_str(", under its melody");
    }
    let mut letters = words.chars();
    let first = letters.next().map(|first| first.to_ascii_uppercase());
    first.into_iter().chain(letters).collect()
}

/// The pitch that each step of a pattern starts over a chord, as the game plays it: `written`
/// is the chord of a bar as its section writes it. A drum has no pitch.
pub fn heard(score: &Score, rhythm: &Rhythm, mode: Mode, written: u8) -> [Option<u8>; STEPS] {
    if rhythm.instrument.is_drum() {
        return [None; STEPS];
    }
    let uses = uses(&score.piece, rhythm.instrument, rhythm.name.as_str());
    let octave = uses.octave.or(parts::usual_octave(rhythm.instrument));
    let tonic = score.tonic(octave.unwrap_or(3));
    let degree = mode.chord(written);
    let chord = if rhythm.instrument == Instrument::Bass {
        let root = bass_root(mode, degree, tonic);
        let [low, third, fifth] = triad(mode, degree, 0);
        [root, root + third - low, root + fifth - low]
    } else {
        triad(mode, degree, tonic)
    };
    let mut hits = 0;
    std::array::from_fn(|step| match rhythm.grid[step] {
        b'x' | b'o' => {
            hits += 1;
            Some(chord[(hits - 1) % 3])
        }
        b'R' => Some(chord[0]),
        b'O' => Some(chord[0] + 12),
        b'V' => Some(chord[2]),
        digit @ b'1'..=b'9' => Some(tonic + mode.semitones(digit - b'0')),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use tanku_audio::checks;

    use super::*;
    use crate::fixture::{PIECE, SCORE};

    #[test]
    fn a_chord_has_the_name_of_its_root_and_of_its_third() {
        let name = |mode: Mode, written: u8| chord(&PIECE, mode, written);
        let dorian: Vec<String> = [1, 0, 4, 5].map(|chord| name(Mode::Dorian, chord)).into();
        assert_eq!(dorian, ["Am", "G", "D", "Em"]);
        let aeolian: Vec<String> = [1, 0, 4, 5].map(|chord| name(Mode::Aeolian, chord)).into();
        assert_eq!(aeolian, ["Am", "F", "Dm", "Em"]);
        // The chord that neither mode of the game has, and the degrees as notes.
        assert_eq!(name(Mode::Locrian, 1), "Adim");
        assert_eq!(name(Mode::Ionian, 7), "G#dim");
        assert_eq!(note(&PIECE, Mode::Dorian, 6), "F#");
        assert_eq!(note(&PIECE, Mode::Aeolian, 6), "F");
        assert_eq!(note(&PIECE, Mode::Aeolian, 9), "B");
    }

    #[test]
    fn a_pattern_says_where_it_is_played() {
        let played = |instrument: Instrument, name: &str| played_in(&PIECE, instrument, name);
        assert_eq!(played(Instrument::Kick, "heart"), "The calm");
        assert_eq!(played(Instrument::Kick, "fight"), "Every fight");
        assert_eq!(played(Instrument::Hat, "corridor"), "Fights in a corridor");
        assert_eq!(played(Instrument::Snare, "arena"), "Fights in an arena");
        assert_eq!(played(Instrument::Bass, "long"), "The menu");
        assert_eq!(
            played(Instrument::Bass, "plain"),
            "Fights at threats 0 and 1"
        );
        assert_eq!(
            played(Instrument::Bass, "leaping"),
            "Fights at threats 2 to 4"
        );
        assert_eq!(
            played(Instrument::Pluck, "arpeggio"),
            "Fights at threats 0 to 3"
        );
        assert_eq!(played(Instrument::Pluck, "fast"), "Fights at threat 4");
        assert_eq!(
            played(Instrument::Pluck, "tick"),
            "The calm, under its melody"
        );
        assert_eq!(played(Instrument::Pluck, "none"), "Nowhere yet");
        assert_eq!(listed(&[0, 2, 4]), "0, 2 and 4");
        // The octave that each is played in.
        assert_eq!(uses(&PIECE, Instrument::Pluck, "tick").octave, Some(2));
        assert_eq!(uses(&PIECE, Instrument::Bass, "pair").octave, Some(1));
        assert_eq!(uses(&PIECE, Instrument::Kick, "fight").octave, None);
    }

    #[test]
    fn a_step_of_a_pattern_has_its_note_over_a_chord() {
        let heard = |instrument: Instrument, name: &str, mode: Mode, written: u8| {
            let rhythm = PIECE.rhythm(instrument, name).unwrap();
            let pitches = super::heard(&SCORE, &rhythm, mode, written);
            let notes = pitches.map(|pitch| pitch.map_or(String::new(), checks::note));
            notes.join(" ")
        };
        // R..O ..R. R..O ..V. over the chord on the tonic, then over the color chord of
        // each mode: the bass never goes below E2.
        assert_eq!(
            heard(Instrument::Bass, "leaping", Mode::Dorian, 1),
            "A2   A3   A2  A2   A3   E3 "
        );
        assert_eq!(
            heard(Instrument::Bass, "leaping", Mode::Dorian, 0),
            "G2   G3   G2  G2   G3   D3 "
        );
        assert_eq!(
            heard(Instrument::Bass, "leaping", Mode::Aeolian, 0),
            "F2   F3   F2  F2   F3   C3 "
        );
        // The hits of the pluck go up the chord in turn, and a digit is a degree of the
        // scale whatever the chord.
        assert!(
            heard(Instrument::Pluck, "arpeggio", Mode::Dorian, 4).starts_with("D4  F#4  A4  D4 ")
        );
        assert_eq!(
            heard(Instrument::Pluck, "tick", Mode::Dorian, 4),
            "E3  B3  E3  B3  E3  B3  E3  B3 "
        );
        assert_eq!(heard(Instrument::Kick, "fight", Mode::Dorian, 1).trim(), "");
    }
}
