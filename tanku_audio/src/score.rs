//! The piece that the game plays, as `music.score` writes it, and the rules that derive every
//! other line from what is written there.
//!
//! Melodies are written in degrees of the scale and not in notes, so each of them exists in
//! every mode. [`notation`] says how the file writes a bar.

use crate::notation::{self, COLOR, Instrument, Link, Passage, Piece, REST, Rhythm, Roll, same};
pub use crate::notation::{BARS, BEAT, Cell, Line, Pattern, STEPS};

/// The piece as its file writes it. The file is read while the crate is compiled: one that
/// can't be read stops the build, and the error says on which line.
pub const PIECE: Piece = match notation::read(include_str!("../music.score")) {
    Ok(piece) => piece,
    Err(fault) => fault.stop(),
};

/// Half frames in a minute of music.
const MINUTE: u32 = 60 * 2 * crate::SAMPLE_RATE;

/// How long a step lasts at a tempo in beats per minute, in half frames, to the nearest one.
/// At the 120 beats per minute of the piece a step is 5512 frames and a half, and at the 105
/// of its bridge it is 6300 frames: both are whole numbers of half frames.
pub const fn step(tempo: u16) -> u32 {
    let steps = BEAT as u32 * tempo as u32;
    (MINUTE + steps / 2) / steps
}

/// The A of 440 Hz. Pitches count semitones. The cues play this A, the A below it and their
/// fifths.
pub const A4: u8 = 69;
pub const A3: u8 = A4 - 12;
pub const A2: u8 = A3 - 12;
pub const A1: u8 = A2 - 12;

/// The tonic of the piece in the octaves of those four, from the bass up to the lead. In the
/// key of A they are those four, and another key is at most six semitones away.
pub const T4: u8 = tonic(A4);
pub const T3: u8 = tonic(A3);
pub const T2: u8 = tonic(A2);
pub const T1: u8 = tonic(A1);

const fn tonic(a: u8) -> u8 {
    (a as i8 + PIECE.key.offset()) as u8
}

/// The mode of the piece, which every section is in but the fights of an arena.
pub const HOME: Mode = PIECE.key.home;
/// The mode of the fights of an arena.
pub const ARENA: Mode = PIECE.key.arena;

/// The scale in use: one of the seven modes of the major scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Major.
    Ionian,
    /// Minor, with a major sixth. It is the mode of the piece.
    Dorian,
    /// Minor, with a minor second.
    Phrygian,
    /// Major, with an augmented fourth.
    Lydian,
    /// Major, with a minor seventh.
    Mixolydian,
    /// Minor. It is the mode of the fights in an arena.
    Aeolian,
    /// Minor, with a minor second and a diminished fifth.
    Locrian,
}

impl Mode {
    /// The seven modes. Each is the scale of the one before it, started one degree higher.
    pub const ALL: [Mode; 7] = [
        Mode::Ionian,
        Mode::Dorian,
        Mode::Phrygian,
        Mode::Lydian,
        Mode::Mixolydian,
        Mode::Aeolian,
        Mode::Locrian,
    ];

    /// The name of the mode in the file.
    pub const fn name(self) -> &'static str {
        match self {
            Mode::Ionian => "ionian",
            Mode::Dorian => "dorian",
            Mode::Phrygian => "phrygian",
            Mode::Lydian => "lydian",
            Mode::Mixolydian => "mixolydian",
            Mode::Aeolian => "aeolian",
            Mode::Locrian => "locrian",
        }
    }

    /// The mode of a name of the file.
    pub(crate) const fn named(name: &[u8]) -> Option<Self> {
        let mut index = 0;
        while index < Self::ALL.len() {
            let mode = Self::ALL[index];
            if same(name, mode.name().as_bytes()) {
                return Some(mode);
            }
            index += 1;
        }
        None
    }

    /// Semitones from the tonic up to a degree. Degree 1 is the tonic, and degree 8 is the
    /// tonic an octave higher.
    pub const fn semitones(self, degree: u8) -> u8 {
        let scale = match self {
            Mode::Ionian => [0, 2, 4, 5, 7, 9, 11],
            Mode::Dorian => [0, 2, 3, 5, 7, 9, 10],
            Mode::Phrygian => [0, 1, 3, 5, 7, 8, 10],
            Mode::Lydian => [0, 2, 4, 6, 7, 9, 11],
            Mode::Mixolydian => [0, 2, 4, 5, 7, 9, 10],
            Mode::Aeolian => [0, 2, 3, 5, 7, 8, 10],
            Mode::Locrian => [0, 1, 3, 5, 6, 8, 10],
        };
        let index = degree as usize - 1;
        12 * (index / 7) as u8 + scale[index % 7]
    }

    /// The degree of the chord that gives the mode its color, next to the chords on 1, 4 and
    /// 5. It is a major chord in every mode but Ionian: a tone below the tonic in Dorian and
    /// in Mixolydian, on the sixth degree in Aeolian, and on the second degree in Phrygian, in
    /// Lydian and in Locrian. In Ionian it is the minor chord on the sixth degree.
    pub const fn color(self) -> u8 {
        match self {
            Mode::Ionian | Mode::Aeolian => 6,
            Mode::Dorian | Mode::Mixolydian => 7,
            Mode::Phrygian | Mode::Lydian | Mode::Locrian => 2,
        }
    }

    /// The degree that a chord of a section is built on in this mode: the one the file
    /// writes, or the one that gives the mode its color.
    pub const fn chord(self, written: u8) -> u8 {
        if written == COLOR {
            self.color()
        } else {
            written
        }
    }

    /// The same for the chords of a section, one per bar.
    pub fn chords(self, written: [u8; BARS]) -> [u8; BARS] {
        written.map(|chord| self.chord(chord))
    }
}

/// A section of the file by its name.
macro_rules! passage {
    ($name:literal) => {
        match PIECE.passage($name) {
            Some(passage) => passage,
            None => panic!(concat!("music.score has no line `sec ", $name, "`")),
        }
    };
}

/// The four sections: eight bars each, with their chords, their melody and their tempo. The
/// intro and the verse have the same melody, on the pluck for one and on the lead for the
/// other.
pub const INTRO: Passage = passage!("intro");
pub const VERSE: Passage = passage!("verse");
pub const CHORUS: Passage = passage!("chorus");
/// The bridge has no chords: its bass stays on one degree. Its melody is a call in every bar.
pub const BRIDGE: Passage = passage!("bridge");
/// What the lead answers to each call of the bridge.
pub const BRIDGE_ANSWERS: [Line; BARS] = answers(BRIDGE.melody);

/// A pattern of the file by its instrument and its name.
macro_rules! rhythm {
    ($instrument:ident, $written:literal, $name:literal) => {
        match PIECE.rhythm(Instrument::$instrument, $name) {
            Some(rhythm) => rhythm,
            None => panic!(concat!(
                "music.score has no line `pat ",
                $written,
                " ",
                $name,
                "`"
            )),
        }
    };
}

/// The kick of the bridge: the two beats of a heart. A bar of the bridge is these two hits,
/// the call of the pluck, the two notes of the bass and the answer of the lead, one beat each,
/// over the tick.
pub const KICK_HEART: Rhythm = rhythm!(Kick, "kick", "heart");
pub const KICK_FIGHT: Rhythm = rhythm!(Kick, "kick", "fight");
pub const HAT_CORRIDOR: Rhythm = rhythm!(Hat, "hat", "corridor");
/// Every step, and louder on the steps of the corridor.
pub const HAT_ARENA: Rhythm = rhythm!(Hat, "hat", "arena");
pub const SNARE_ARENA: Rhythm = rhythm!(Snare, "snare", "arena");
pub const BASS_LONG: Rhythm = rhythm!(Bass, "bass", "long");
pub const BASS_PLAIN: Rhythm = rhythm!(Bass, "bass", "plain");
pub const BASS_LEAPING: Rhythm = rhythm!(Bass, "bass", "leaping");
/// What the bass plays in every bar of the bridge: two notes in a row on the degree that the
/// bridge never leaves, the second held under the answer of the lead.
pub const BASS_PAIR: Rhythm = rhythm!(Bass, "bass", "pair");
/// The root, the third and the fifth of the chord in turn, rising.
pub const PLUCK_ARPEGGIO: Rhythm = rhythm!(Pluck, "pluck", "arpeggio");
/// The same, twice as fast.
pub const PLUCK_FAST: Rhythm = rhythm!(Pluck, "pluck", "fast");
/// The tick of the bridge: under its call, the pluck rocks on every half beat between the
/// degree that the bridge never leaves and the fifth above it.
pub const PLUCK_TICK: Rhythm = rhythm!(Pluck, "pluck", "tick");

/// Notes of a melody that lie a semitone above the root of their chord and don't pass
/// quickly enough: in one step if they start on a beat, in three at most otherwise. `chords`
/// has the degree that the chord of each bar is built on. This is the check that guards every
/// change of a note.
pub fn rubs(mode: Mode, lines: &[Line; BARS], chords: &[u8; BARS]) -> usize {
    let mut count = 0;
    for (line, &chord) in lines.iter().zip(chords) {
        for (step, cell) in line.iter().enumerate() {
            if cell.degree == 0 {
                continue;
            }
            let note = mode.semitones(cell.degree) as i32;
            let above_root = (note - mode.semitones(chord) as i32).rem_euclid(12) == 1;
            let allowed = if step % BEAT == 0 { 1 } else { 3 };
            count += (above_root && cell.steps > allowed) as usize;
        }
    }
    count
}

// What plays when is in the code, which asks for each section and each pattern by its name: a
// file can change them, and can't add one.
const _: () = assert!(
    PIECE.passages().len() <= 4,
    "music.score has a section that nothing plays: the game plays `intro`, `verse`, `chorus` \
     and `bridge`"
);
const _: () = assert!(
    PIECE.rhythms().len() <= 12,
    "music.score has a pattern that nothing plays: the game plays twelve patterns, by their \
     names"
);

/// The roll that announces a fight.
pub const ROLL: Roll = PIECE.roll;
/// The connection that leads from a fight into what follows.
pub const LINK: Link = PIECE.link;
/// Steps that a connection lasts. They get longer one after the other, from the tempo of the
/// fight to the tempo of what follows.
pub const CONNECTION_STEPS: u8 = LINK.beats * BEAT as u8;

/// The connection, which leads from a fight into the bridge: the tune goes by steps of the
/// scale from the note the lead is on to the degree of the connection, on which the bridge
/// stays, and never turns back on the way. From the upper tonic down to degree 5 that is the
/// four notes of a fall, and from lower down it is a rise. `lead` is the degree the lead is
/// on, or `None` if it is silent: the connection then starts from the upper tonic.
///
/// The notes are half a beat apart, or a beat apart when there are two of them and room for
/// it, and the last one takes what is left. A walk of more notes than the connection has
/// starts further on, where that many are left.
pub const fn connection(lead: Option<u8>, link: Link) -> Line {
    let from = match lead {
        Some(degree) => degree,
        None => 8,
    };
    let rising = from < link.degree;
    let mut count = if rising {
        (link.degree - from) as usize + 1
    } else {
        (from - link.degree) as usize + 1
    };
    if count > link.notes as usize {
        count = link.notes as usize;
    }
    let steps = link.beats as usize * BEAT;
    let apart = if count == 2 && steps > BEAT {
        BEAT
    } else {
        BEAT / 2
    };
    let mut out = [REST; STEPS];
    let mut note = 0;
    while note < count {
        // The notes are counted back from the last one, which is the degree of the connection.
        let before = (count - 1 - note) as u8;
        let start = apart * note;
        out[start] = Cell {
            degree: if rising {
                link.degree - before
            } else {
                link.degree + before
            },
            steps: if note + 1 == count {
                (steps - start) as u8
            } else {
                apart as u8
            },
        };
        note += 1;
    }
    out
}

/// The degrees that the motifs of the piece use, within one octave. They make a scale of five
/// notes in each mode: in Aeolian the scale known as hirajoshi, and in Dorian the same with
/// its sixth degree a semitone higher.
pub const FIVE: [u8; 5] = [1, 2, 3, 5, 6];

/// The next of the five degrees above a degree.
const fn above(degree: u8) -> u8 {
    let octave = (degree - 1) / 7;
    let within = (degree - 1) % 7 + 1;
    let mut index = 0;
    while index < FIVE.len() {
        if FIVE[index] > within {
            return 7 * octave + FIVE[index];
        }
        index += 1;
    }
    7 * (octave + 1) + 1
}

/// Keeps the notes that start on a beat and holds each until the next.
pub const fn skeleton(line: Line) -> Line {
    let mut out = [REST; STEPS];
    let mut last = STEPS;
    let mut step = 0;
    while step < STEPS {
        if step % BEAT == 0 && line[step].degree != 0 {
            if last < STEPS {
                out[last].steps = (step - last) as u8;
            }
            out[step].degree = line[step].degree;
            last = step;
        }
        step += 1;
    }
    if last < STEPS {
        out[last].steps = (STEPS - last) as u8;
    }
    out
}

/// Gives the last two steps of each note of three steps or more to the degree above it,
/// among the five, and to the note again.
pub const fn ornaments(line: Line) -> Line {
    let mut out = line;
    let mut step = 0;
    while step < STEPS {
        let Cell { degree, steps } = line[step];
        if degree != 0 && steps >= 3 {
            let turn = step + steps as usize - 2;
            out[step].steps = steps - 2;
            out[turn] = Cell {
                degree: above(degree),
                steps: 1,
            };
            out[turn + 1] = Cell { degree, steps: 1 };
        }
        step += 1;
    }
    out
}

/// The answer to a call: its first two notes in the other order, two beats later, and short.
/// A call of one note gets no answer.
pub const fn answer(call: Line) -> Line {
    let mut notes = [REST; 2];
    let mut steps = [0; 2];
    let mut found = 0;
    let mut step = 0;
    while step < STEPS && found < 2 {
        if call[step].degree != 0 {
            notes[found] = call[step];
            steps[found] = step;
            found += 1;
        }
        step += 1;
    }
    let mut out = [REST; STEPS];
    if found == 2 {
        out[(steps[0] + STEPS / 2) % STEPS] = Cell {
            degree: notes[1].degree,
            steps: 1,
        };
        out[(steps[1] + STEPS / 2) % STEPS] = Cell {
            degree: notes[0].degree,
            steps: 1,
        };
    }
    out
}

/// The answer to every call of a section.
pub const fn answers(mut calls: [Line; BARS]) -> [Line; BARS] {
    let mut bar = 0;
    while bar < BARS {
        calls[bar] = answer(calls[bar]);
        bar += 1;
    }
    calls
}

/// The skeleton of every bar of a melody.
pub const fn skeletons(mut lines: [Line; BARS]) -> [Line; BARS] {
    let mut bar = 0;
    while bar < BARS {
        lines[bar] = skeleton(lines[bar]);
        bar += 1;
    }
    lines
}

/// Every bar of a melody with its ornaments.
pub const fn ornamented(mut lines: [Line; BARS]) -> [Line; BARS] {
    let mut bar = 0;
    while bar < BARS {
        lines[bar] = ornaments(lines[bar]);
        bar += 1;
    }
    lines
}

/// The three notes of the chord on a degree, from its root upward. The root is in the 12
/// semitones that start at `lowest`.
pub const fn triad(mode: Mode, chord: u8, lowest: u8) -> [u8; 3] {
    [
        lowest + mode.semitones(chord),
        lowest + mode.semitones(chord + 2),
        lowest + mode.semitones(chord + 4),
    ]
}

/// The root of a chord for the bass, in the twelve semitones from five below its tonic: in
/// the key of A, from E2 up to the E flat above it, so that the bass never goes below E2.
pub const fn bass_root(mode: Mode, chord: u8) -> u8 {
    let pitch = T2 + mode.semitones(chord);
    if pitch > T2 + 6 { pitch - 12 } else { pitch }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notation::line;

    /// The degrees of the bridge. They are the same notes in both modes, and the third degree
    /// is not among them, so the bridge has no mode: it is neither major nor minor.
    const NEUTRAL: [u8; 5] = [1, 2, 4, 5, 7];

    /// A bar of melody as its grid, with a dot wherever no note starts.
    fn grid(line: &Line) -> String {
        let mut grid = String::new();
        for (step, cell) in line.iter().enumerate() {
            if step > 0 && step % BEAT == 0 {
                grid.push(' ');
            }
            match cell.degree {
                0 => grid.push('.'),
                degree => grid.push((b'0' + degree) as char),
            }
        }
        grid
    }

    fn name(pitch: u8) -> &'static str {
        [
            "A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#",
        ][(pitch as usize + 3) % 12]
    }

    /// The steps of a bar on which something starts.
    fn starts(line: &Line) -> Vec<usize> {
        (0..STEPS).filter(|&step| line[step].degree != 0).collect()
    }

    /// The degrees that a pattern writes.
    fn degrees(rhythm: &Rhythm) -> Vec<u8> {
        let digits = rhythm.grid.iter().filter(|symbol| symbol.is_ascii_digit());
        digits.map(|digit| digit - b'0').collect()
    }

    #[test]
    fn steps_have_the_length_of_their_tempo() {
        // Four steps to a beat, and a minute of music in half frames.
        let minute = 60 * 2 * crate::SAMPLE_RATE;
        assert_eq!(4 * 120 * step(120), minute);
        assert_eq!(4 * 105 * step(105), minute);
        // A bar lasts two seconds, and 2.29 s in the bridge.
        assert_eq!(STEPS as u32 * step(120) / 2, 88_200);
        assert_eq!(STEPS as u32 * step(105) / 2, 100_800);
        // The bridge alone has a tempo of its own.
        assert_eq!(PIECE.tempo, 120);
        let tempos = [INTRO, VERSE, CHORUS, BRIDGE].map(|passage| passage.tempo);
        assert_eq!(tempos, [120, 120, 120, 105]);
        // At another tempo a step is rounded to the nearest half frame: 12,027.27 of them at
        // 110 beats per minute, and 11,707.96 at 113.
        assert_eq!((step(110), step(113)), (12_027, 11_708));
    }

    #[test]
    fn the_modes_are_those_of_the_specification() {
        assert_eq!((HOME, ARENA), (Mode::Dorian, Mode::Aeolian));
        assert_eq!([T1, T2, T3, T4], [A1, A2, A3, A4]);
        let scale = |mode: Mode| (1..=7).map(move |degree| name(A4 + mode.semitones(degree)));
        assert!(scale(Mode::Dorian).eq(["A", "B", "C", "D", "E", "F#", "G"]));
        assert!(scale(Mode::Aeolian).eq(["A", "B", "C", "D", "E", "F", "G"]));
        // The two that were tried for the piece and set aside.
        assert!(scale(Mode::Ionian).eq(["A", "B", "C#", "D", "E", "F#", "G#"]));
        assert!(scale(Mode::Mixolydian).eq(["A", "B", "C#", "D", "E", "F#", "G"]));
        // And the three others that a file can name.
        assert!(scale(Mode::Phrygian).eq(["A", "A#", "C", "D", "E", "F", "G"]));
        assert!(scale(Mode::Lydian).eq(["A", "B", "C#", "D#", "E", "F#", "G#"]));
        assert!(scale(Mode::Locrian).eq(["A", "A#", "C", "D", "D#", "F", "G"]));

        // Each mode is the major scale, started one degree higher than the mode before it.
        let steps = |mode: Mode| -> Vec<u8> {
            let step = |degree: u8| mode.semitones(degree + 1) - mode.semitones(degree);
            (1..=7).map(step).collect()
        };
        for (first, mode) in Mode::ALL.into_iter().enumerate() {
            let mut major = steps(Mode::Ionian);
            major.rotate_left(first);
            assert_eq!(steps(mode), major, "{mode:?}");
            // Past the octave, the scale starts over.
            assert_eq!(mode.semitones(8), 12);
            assert_eq!(mode.semitones(9), 12 + mode.semitones(2));
            // The file names each of them, in small letters.
            assert_eq!(Mode::named(mode.name().as_bytes()), Some(mode));
        }
        assert_eq!(Mode::named(b"Dorian"), None);
        assert_eq!(Mode::named(b"minor"), None);
        // The five degrees of the motifs are hirajoshi in Aeolian, and the same with a higher
        // sixth degree in Dorian.
        let five = |mode: Mode| FIVE.map(|degree| name(A4 + mode.semitones(degree)));
        assert_eq!(five(Mode::Dorian), ["A", "B", "C", "E", "F#"]);
        assert_eq!(five(Mode::Aeolian), ["A", "B", "C", "E", "F"]);
    }

    #[test]
    fn the_piece_is_written_as_the_specification_has_it() {
        let [a, b, c] = (*b"ABC").map(|letter| PIECE.motif(letter).unwrap());
        assert_eq!(PIECE.motifs().len(), 3);
        let [first, second] = [a.bars()[0], a.bars()[1]];
        assert_eq!(grid(&first), "1..2 ..3. 5..3 ..2.");
        assert_eq!(grid(&second), "3..5 ..6. .... 5...");
        assert_eq!(grid(&a.turn.unwrap()), "3..5 ..2. .... 5...");
        assert_eq!(grid(&b.bars()[0]), "8... ..9. 8... 6...");
        assert_eq!(grid(&b.bars()[1]), "5..6 ..5. 3..2 ..1.");
        let cell = |degree, steps| Cell { degree, steps };
        assert_eq!(first[0], cell(1, 3));
        assert_eq!(first[14], cell(2, 2));
        assert_eq!(second[6], cell(6, 6));
        assert_eq!(first[1], cell(0, 0));
        // A dash ends the note before it.
        let [call] = c.bars() else {
            panic!("the call is one bar long");
        };
        assert_eq!(starts(call), [4, 6]);
        assert_eq!(call[6], cell(8, 2));

        // The intro and the verse have motif A four times, the last time with its turn, and
        // the chorus has motif B four times.
        let verse = [first, second, first, second, first, second, first];
        assert_eq!(VERSE.melody[..7], verse);
        assert_eq!(VERSE.melody[7], a.turn.unwrap());
        assert_eq!(INTRO.melody, VERSE.melody);
        assert_eq!(CHORUS.melody[..2], [b.bars()[0], b.bars()[1]]);
        assert_eq!(CHORUS.melody[..2], CHORUS.melody[6..]);
        assert_eq!(VERSE.chords, [1, 1, COLOR, 4, 1, 1, COLOR, 5]);
        assert_eq!(INTRO.chords, VERSE.chords);
        assert_eq!(CHORUS.chords, [4, 5, 1, COLOR, 4, 5, 1, 1]);
        assert_eq!(BRIDGE.chords, [5; BARS]);

        assert_eq!(&BASS_LEAPING.grid, b"R..O..R.R..O..V.");
        assert_eq!(&BASS_PAIR.grid, b"--------5-5.....");
        assert_eq!(&HAT_ARENA.grid, b"ooxoooxoooxoooxo");
        assert_eq!((HAT_ARENA.level, HAT_ARENA.soft), (200, 100));
        assert_eq!((KICK_HEART.level, KICK_HEART.soft), (180, 135));
        assert_eq!((PLUCK_ARPEGGIO.level, PLUCK_TICK.level), (150, 110));
        assert_eq!((ROLL.beats, ROLL.from, ROLL.to), (2, 40, 255));
        assert_eq!(
            (ROLL.drum, ROLL.arena),
            (Instrument::Hat, Instrument::Snare)
        );
        assert_eq!((LINK.beats, LINK.degree, LINK.notes), (2, 5, 4));
    }

    #[test]
    fn the_motifs_share_the_rhythm_of_the_bass() {
        let rhythm = [0, 3, 6, 8, 11, 14];
        assert_eq!(starts(&VERSE.melody[0]), rhythm);
        assert_eq!(starts(&CHORUS.melody[1]), rhythm);
        let bass: Vec<usize> = (0..STEPS)
            .filter(|&step| BASS_PLAIN.grid[step] != b'.')
            .collect();
        assert_eq!(bass, rhythm);
    }

    #[test]
    fn every_mode_has_its_chords() {
        // The chords on 1, 4 and 5, then the one that gives the mode its color.
        let named = |mode: Mode| {
            [1, 4, 5, mode.color()].map(|chord| {
                let [root, third, fifth] = triad(mode, chord, A4);
                let kind = match (third - root, fifth - root) {
                    (4, 7) => "",
                    (3, 7) => "m",
                    (3, 6) => "dim",
                    other => panic!("a chord of {other:?} semitones"),
                };
                format!("{}{kind}", name(root))
            })
        };
        assert_eq!(named(Mode::Dorian), ["Am", "D", "Em", "G"]);
        assert_eq!(named(Mode::Aeolian), ["Am", "Dm", "Em", "F"]);
        assert_eq!(named(Mode::Ionian), ["A", "D", "E", "F#m"]);
        assert_eq!(named(Mode::Mixolydian), ["A", "D", "Em", "G"]);
        // Three modes have a chord on 1, 4 or 5 that is neither major nor minor. The chord of
        // their color is B flat in Phrygian and in Locrian, which also has E flat on 5.
        assert_eq!(named(Mode::Phrygian), ["Am", "Dm", "Edim", "A#"]);
        assert_eq!(named(Mode::Lydian), ["A", "D#dim", "E", "B"]);
        assert_eq!(named(Mode::Locrian), ["Adim", "Dm", "D#", "A#"]);
        // The chord of a color is none of the other three, and it is major or minor.
        for mode in Mode::ALL {
            assert!(![1, 4, 5].contains(&mode.color()), "{mode:?}");
            assert!(!named(mode)[3].ends_with("dim"), "{mode:?}");
        }
        // The chords of the piece are these and no other, in both of its modes.
        for mode in [HOME, ARENA] {
            let written = [INTRO, VERSE, CHORUS].map(|passage| mode.chords(passage.chords));
            for chord in written.into_iter().flatten() {
                assert!([1, 4, 5, mode.color()].contains(&chord));
            }
            assert!(named(mode).iter().all(|chord| !chord.ends_with("dim")));
        }
    }

    #[test]
    fn the_bass_has_the_root_of_every_chord_from_e2_up() {
        for mode in Mode::ALL {
            for chord in 1..=7 {
                let root = bass_root(mode, chord);
                assert!((T2 - 5..=T2 + 6).contains(&root), "{mode:?}, chord {chord}");
                // It is the root of the chord, in one octave or another.
                assert_eq!((root + 12 - T2) % 12, mode.semitones(chord), "{mode:?}");
            }
        }
        assert_eq!(T2 - 5, 40);
        // The chord of Locrian on its diminished fifth: E flat 3, not the one below E2.
        assert_eq!(bass_root(Mode::Locrian, 5), 51);
        assert_eq!(bass_root(Mode::Dorian, 5), 40);
    }

    #[test]
    fn derived_lines_read_as_expected() {
        let (a, b) = (line("1..2 ..3. 5..3 ..2."), line("3..5 ..6. .... 5..."));
        assert_eq!(grid(&skeleton(a)), "1... .... 5... ....");
        assert_eq!(grid(&skeleton(b)), "3... .... .... 5...");
        assert_eq!(skeleton(a)[0].steps, 8);
        assert_eq!(skeleton(b)[0].steps, 12);
        assert_eq!(skeleton(b)[12].steps, 4);
        assert_eq!(grid(&ornaments(a)), "1212 323. 5653 532.");
        let high = line("8... ..9. 8... 6...");
        assert_eq!(grid(&ornaments(high)), "8... 989. 8.98 6.86");
        // A note keeps its length less the two steps of its ornament.
        assert_eq!(ornaments(b)[6].steps, 4);
        assert_eq!(grid(&ornaments(b)), "3535 656. ..86 5.65");
        // A degree that is not among the five takes the next of the five above it.
        let others = line("4... 7... .... ....");
        assert_eq!(grid(&ornaments(others)), "4.54 7... .... ..87");
        // A melody without a note on a beat has no skeleton.
        assert_eq!(skeleton(line("-1.. -2.. -3.. -5..")), [REST; STEPS]);
    }

    #[test]
    fn the_bridge_stays_on_one_degree() {
        // The degree that the bridge never leaves is the one a connection leads to: the bass
        // has it, the tick starts on it, and so does every call.
        let pedal = LINK.degree;
        assert_eq!(degrees(&BASS_PAIR), [pedal; 2]);
        assert_eq!(
            degrees(&PLUCK_TICK),
            [pedal, 9, pedal, 9, pedal, 9, pedal, 9]
        );
        assert!(BRIDGE.melody.iter().all(|line| line[4].degree == pedal));
        // Only the last call falls.
        let calls: Vec<u8> = BRIDGE.melody.iter().map(|line| line[6].degree).collect();
        assert_eq!(calls, [8, 7, 8, 9, 8, 7, 8, 4]);
        // Its notes are five degrees, which are the same notes in both modes. The third degree
        // is not among them, so the bridge has no mode: it is neither major nor minor.
        let lines = BRIDGE.melody.iter().chain(&BRIDGE_ANSWERS).flatten();
        let mut heard: Vec<u8> = lines.map(|cell| cell.degree).collect();
        heard.extend(degrees(&PLUCK_TICK));
        heard.retain(|&degree| degree != 0);
        assert_eq!(heard.len(), 4 * BARS + 8);
        assert!(
            heard
                .iter()
                .all(|degree| NEUTRAL.contains(&((degree - 1) % 7 + 1)))
        );
    }

    #[test]
    fn the_lead_answers_each_call_backwards() {
        // Two beats after the call, its second note and then its first, one step each.
        assert_eq!(starts(&BRIDGE_ANSWERS[0]), [12, 14]);
        for (call, answer) in BRIDGE.melody.iter().zip(&BRIDGE_ANSWERS) {
            assert_eq!(answer[12].degree, call[6].degree);
            assert_eq!(answer[14].degree, call[4].degree);
            assert_eq!((answer[12].steps, answer[14].steps), (1, 1));
        }
        // A call late in its bar is answered early in it, and a call of more than two notes
        // by its first two.
        assert_eq!(
            grid(&answer(line("---- ---- 5.8. ----"))),
            "8.5. .... .... ...."
        );
        assert_eq!(
            grid(&answer(line("5.8. 9... ---- ----"))),
            ".... .... 8.5. ...."
        );
        // One note is no call.
        assert_eq!(answer(line("---- 5... ---- ----")), [REST; STEPS]);
    }

    #[test]
    fn a_connection_goes_to_the_degree_of_the_bridge_without_turning_back() {
        // What the connection plays in its two beats, for each note the lead can be on.
        let walk = |lead: Option<u8>| grid(&connection(lead, LINK))[..9].to_string();
        // From the upper tonic, and from the note above it, the four notes of a fall.
        assert_eq!(walk(Some(8)), "8.7. 6.5.");
        assert_eq!(walk(Some(9)), "8.7. 6.5.");
        // From nearer, the notes that are left, the last one for as long as it takes.
        assert_eq!(walk(Some(6)), "6... 5...");
        assert_eq!(walk(Some(5)), "5... ....");
        // From below, a rise. From the tonic it starts one step on, where four notes are left.
        assert_eq!(walk(Some(3)), "3.4. 5...");
        assert_eq!(walk(Some(2)), "2.3. 4.5.");
        assert_eq!(walk(Some(1)), "2.3. 4.5.");
        // A lead that is silent has nothing to connect from: the fall.
        assert_eq!(walk(None), "8.7. 6.5.");

        let steps = CONNECTION_STEPS;
        for lead in (1..=10).map(Some).chain([None]) {
            let line = connection(lead, LINK);
            let notes: Vec<Cell> = line
                .iter()
                .copied()
                .filter(|cell| cell.degree != 0)
                .collect();
            // It fills the two beats and nothing after them, and ends on the degree of the
            // bridge.
            assert_eq!(notes.iter().map(|cell| cell.steps).sum::<u8>(), steps);
            assert!(starts(&line).iter().all(|&step| step < steps as usize));
            assert_eq!(notes.last().unwrap().degree, LINK.degree, "{lead:?}");
            // One step of the scale at a time, always the same way.
            let degrees: Vec<i32> = notes.iter().map(|cell| cell.degree as i32).collect();
            let moves: Vec<i32> = degrees.windows(2).map(|pair| pair[1] - pair[0]).collect();
            assert!(moves.iter().all(|&by| by == 1) || moves.iter().all(|&by| by == -1));
        }
        // In the mode of the piece the fall has F sharp: A, G, F sharp, E.
        let notes = [8, 7, 6, 5].map(|degree| name(A4 + Mode::Dorian.semitones(degree)));
        assert_eq!(notes, ["A", "G", "F#", "E"]);
    }

    #[test]
    fn a_connection_follows_what_the_file_says_of_it() {
        let way = |lead: Option<u8>, beats: u8, degree: u8, notes: u8| {
            let link = Link {
                beats,
                degree,
                notes,
            };
            grid(&connection(lead, link))
        };
        // To the tonic, from the degree the piece now goes to.
        assert_eq!(way(Some(5), 2, 1, 4), "4.3. 2.1. .... ....");
        // In one beat, two notes are half a beat apart.
        assert_eq!(way(Some(8), 1, 5, 2), "6.5. .... .... ....");
        // Over three beats, with up to six notes.
        assert_eq!(way(Some(1), 3, 8, 6), "3.4. 5.6. 7.8. ....");
        // Up to the note above the upper tonic, from silence: a beat each.
        assert_eq!(way(None, 2, 9, 4), "8... 9... .... ....");
        // With one note, the connection is where it goes at once.
        assert_eq!(way(Some(2), 4, 5, 1), "5... .... .... ....");
        assert_eq!(
            connection(
                Some(2),
                Link {
                    beats: 4,
                    degree: 5,
                    notes: 1
                }
            )[0]
            .steps,
            16
        );
    }

    #[test]
    fn the_turnaround_keeps_the_verse_sound_in_minor() {
        // Without it, the last bar of the verse holds F over E minor.
        let mut plain = VERSE.melody;
        plain[7] = plain[1];
        let dorian = Mode::Dorian.chords(VERSE.chords);
        assert_eq!(rubs(Mode::Dorian, &plain, &dorian), 0);
        let aeolian = Mode::Aeolian.chords(VERSE.chords);
        assert_eq!(rubs(Mode::Aeolian, &plain, &aeolian), 1);
    }
}
