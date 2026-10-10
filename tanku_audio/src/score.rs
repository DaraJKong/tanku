//! The piece as it is written by hand: three motifs, the chords of each section and the rhythm
//! patterns, with the rules that derive every other line from them.
//!
//! Melodies are written in degrees of the scale and not in notes, so each of them exists in
//! every mode. A grid is one bar of 16 steps in four beats. In a melody, a digit starts a note
//! on that degree, a dot holds it and a dash is silence. In a pattern, `x` is a hit, `o` is a
//! softer one, `R` is the root of the chord of the bar, `O` is that root an octave higher, `5`
//! is its fifth, and a dot starts nothing.

/// Steps in a bar. A step is a quarter of a beat.
pub const STEPS: usize = 16;
/// Bars in a section.
pub const BARS: usize = 8;

/// How long a step lasts, in half frames of the music. At 120 beats per minute a step is 5512
/// frames and a half.
pub const STEP: u32 = 11_025;
/// The bridge is slower, at 90 beats per minute: its steps are 7350 frames long.
pub const BRIDGE_STEP: u32 = 14_700;

/// The A of 440 Hz. Pitches count semitones, and this one is the tonic of the lead.
pub const A4: u8 = 69;
pub const A3: u8 = A4 - 12;
pub const A2: u8 = A3 - 12;
pub const A1: u8 = A2 - 12;

/// The scale in use. Both have A as their tonic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Minor, with a major sixth. It is the mode of the piece.
    Dorian,
    /// Minor. It is the mode of the fights in an arena.
    Aeolian,
}

impl Mode {
    /// Semitones from the tonic up to a degree. Degree 1 is the tonic, and degree 8 is the
    /// tonic an octave higher.
    pub const fn semitones(self, degree: u8) -> u8 {
        let scale = match self {
            Mode::Dorian => [0, 2, 3, 5, 7, 9, 10],
            Mode::Aeolian => [0, 2, 3, 5, 7, 8, 10],
        };
        let index = degree as usize - 1;
        12 * (index / 7) as u8 + scale[index % 7]
    }

    /// The degree of the chord that gives the mode its color, next to the chords on 1, 4 and
    /// 5: the major chord a tone below the tonic in Dorian, and the major chord on the sixth
    /// degree in Aeolian.
    pub const fn color(self) -> u8 {
        match self {
            Mode::Dorian => 7,
            Mode::Aeolian => 6,
        }
    }

    /// The chords of the intro and of the verse, one per bar, each named by the degree it is
    /// built on.
    pub const fn verse_chords(self) -> [u8; BARS] {
        let color = self.color();
        [1, 1, color, 4, 1, 1, color, 5]
    }

    /// The chords of the chorus.
    pub const fn chorus_chords(self) -> [u8; BARS] {
        [4, 5, 1, self.color(), 4, 5, 1, 1]
    }
}

/// What starts on one step of a melody: a note on a degree that lasts a number of steps, or
/// nothing when the degree is 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub degree: u8,
    pub steps: u8,
}

const REST: Cell = Cell {
    degree: 0,
    steps: 0,
};

/// One bar of a melody.
pub type Line = [Cell; STEPS];

/// One bar of a rhythm: the symbols of its grid.
pub type Pattern = [u8; STEPS];

/// Reads one bar of a melody from its grid.
pub const fn line(grid: &str) -> Line {
    let symbols = pattern(grid);
    let mut line = [REST; STEPS];
    let mut last = STEPS;
    let mut step = 0;
    while step < STEPS {
        let symbol = symbols[step];
        if symbol == b'.' {
            if last < STEPS {
                line[last].steps += 1;
            }
        } else if symbol == b'-' {
            last = STEPS;
        } else {
            assert!(symbol.is_ascii_digit() && symbol != b'0');
            line[step] = Cell {
                degree: symbol - b'0',
                steps: 1,
            };
            last = step;
        }
        step += 1;
    }
    line
}

/// Reads one bar of a rhythm from its grid.
pub const fn pattern(grid: &str) -> Pattern {
    let bytes = grid.as_bytes();
    let mut pattern = [b'.'; STEPS];
    let mut step = 0;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b' ' {
            assert!(step < STEPS, "a bar has 16 steps");
            pattern[step] = bytes[index];
            step += 1;
        }
        index += 1;
    }
    assert!(step == STEPS, "a bar has 16 steps");
    pattern
}

/// Motif A, the question: it climbs and stops on degree 5.
pub const MOTIF_A: [Line; 2] = [line("1..2 ..3. 5..3 ..2."), line("3..5 ..6. .... 5...")];
/// Motif B, the answer: it comes down from the high tonic to the tonic.
pub const MOTIF_B: [Line; 2] = [line("8... ..9. 8... 6..."), line("5..6 ..5. 3..2 ..1.")];
/// The second bar of motif A with degree 2, a note of chord 5, in place of its long degree 6.
pub const TURNAROUND: Line = line("3..5 ..2. .... 5...");

/// The melody of the intro and of the verse: motif A four times, the last time with the
/// turnaround.
pub const VERSE: [Line; BARS] = {
    let [first, second] = MOTIF_A;
    [
        first, second, first, second, first, second, first, TURNAROUND,
    ]
};
/// The melody of the chorus: motif B four times.
pub const CHORUS: [Line; BARS] = {
    let [first, second] = MOTIF_B;
    [first, second, first, second, first, second, first, second]
};

/// Motif C, the call of the bridge: two notes in a row, the second a fourth above the first
/// and left to ring.
pub const MOTIF_C: Line = line("---- 5.8. ---- ----");
/// The melody of the bridge: the call in every bar. Its second note changes from one bar to
/// the next, and the last call of the eight falls where the others rise.
pub const BRIDGE: [Line; BARS] = [
    MOTIF_C,
    line("---- 5.7. ---- ----"),
    MOTIF_C,
    line("---- 5.9. ---- ----"),
    MOTIF_C,
    line("---- 5.7. ---- ----"),
    MOTIF_C,
    line("---- 5.4. ---- ----"),
];
/// What the bass plays in every bar of the bridge: two short notes in a row on the degree that
/// the bridge never leaves.
pub const BRIDGE_BASS: Line = line("---- ---- 5-5- ----");
pub const PEDAL: u8 = 5;
/// The kick of the bridge: the two beats of a heart. A bar of the bridge is these two hits,
/// the call of the pluck, the two notes of the bass, and a beat of silence.
pub const KICK_HEART: Pattern = pattern("x.o. .... .... ....");
/// The degrees of the bridge. They are the same notes in every mode, so the bridge has no
/// mode: it is neither major nor minor.
pub const NEUTRAL: [u8; 5] = [1, 2, 4, 5, 7];

pub const KICK_FIGHT: Pattern = pattern("x... x... x... x...");
pub const HAT_CORRIDOR: Pattern = pattern("..x. ..x. ..x. ..x.");
/// Louder on the steps of the corridor pattern.
pub const HAT_ARENA: Pattern = pattern("xxxx xxxx xxxx xxxx");
pub const SNARE_ARENA: Pattern = pattern(".... x... .... x...");
pub const BASS_LONG: Pattern = pattern("R... .... .... ....");
pub const BASS_PLAIN: Pattern = pattern("R..R ..R. R..R ..R.");
pub const BASS_LEAPING: Pattern = pattern("R..O ..R. R..O ..5.");
/// The root, the third and the fifth of the chord in turn, rising.
pub const PLUCK_ARPEGGIO: Pattern = pattern("x.x. x.x. x.x. x.x.");

/// The degrees that the motifs use, within one octave. They make a scale of five notes in each
/// mode: in Aeolian the scale known as hirajoshi, and in Dorian the same with its sixth degree
/// a semitone higher.
pub const FIVE: [u8; 5] = [1, 2, 3, 5, 6];

/// The next of the five degrees above one of them.
const fn above(degree: u8) -> u8 {
    let octave = (degree - 1) / 7;
    let within = (degree - 1) % 7 + 1;
    let mut index = 0;
    while FIVE[index] != within {
        index += 1;
    }
    if index + 1 < FIVE.len() {
        7 * octave + FIVE[index + 1]
    } else {
        7 * (octave + 1) + 1
    }
}

/// Keeps the notes that start on a beat and holds each until the next.
pub const fn skeleton(line: Line) -> Line {
    let mut out = [REST; STEPS];
    let mut last = STEPS;
    let mut step = 0;
    while step < STEPS {
        if step % 4 == 0 && line[step].degree != 0 {
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

/// The root of a chord for the bass, between E2 and D3.
pub const fn bass_root(mode: Mode, chord: u8) -> u8 {
    let pitch = A2 + mode.semitones(chord);
    if pitch > A2 + 5 { pitch - 12 } else { pitch }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODES: [Mode; 2] = [Mode::Dorian, Mode::Aeolian];

    /// A bar of melody as its grid.
    fn grid(line: &Line) -> String {
        let mut grid = String::new();
        for (step, cell) in line.iter().enumerate() {
            if step > 0 && step % 4 == 0 {
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

    /// Notes that lie a semitone above the root of their chord and don't pass quickly enough:
    /// in one step if they start on a beat, in three at most otherwise.
    fn rubs(mode: Mode, lines: &[Line; BARS], chords: &[u8; BARS]) -> usize {
        let mut count = 0;
        for (line, &chord) in lines.iter().zip(chords) {
            for (step, cell) in line.iter().enumerate() {
                if cell.degree == 0 {
                    continue;
                }
                let note = mode.semitones(cell.degree) as i32;
                let above_root = (note - mode.semitones(chord) as i32).rem_euclid(12) == 1;
                let allowed = if step % 4 == 0 { 1 } else { 3 };
                count += (above_root && cell.steps > allowed) as usize;
            }
        }
        count
    }

    #[test]
    fn steps_have_the_length_of_their_tempo() {
        // Four steps to a beat, and a minute of music in half frames.
        let minute = 60 * 2 * crate::SAMPLE_RATE;
        assert_eq!(4 * 120 * STEP, minute);
        assert_eq!(4 * 90 * BRIDGE_STEP, minute);
        // A bar lasts two seconds, and 2.67 s in the bridge.
        assert_eq!(STEPS as u32 * STEP / 2, 88_200);
        assert_eq!(STEPS as u32 * BRIDGE_STEP / 2, 117_600);
    }

    #[test]
    fn the_modes_are_those_of_the_specification() {
        let scale = |mode: Mode| (1..=7).map(move |degree| name(A4 + mode.semitones(degree)));
        assert!(scale(Mode::Dorian).eq(["A", "B", "C", "D", "E", "F#", "G"]));
        assert!(scale(Mode::Aeolian).eq(["A", "B", "C", "D", "E", "F", "G"]));
        for mode in MODES {
            assert_eq!(mode.semitones(8), 12);
            assert_eq!(mode.semitones(9), 14);
        }
    }

    #[test]
    fn grids_read_as_they_are_written() {
        let [first, second] = MOTIF_A;
        assert_eq!(grid(&first), "1..2 ..3. 5..3 ..2.");
        assert_eq!(grid(&second), "3..5 ..6. .... 5...");
        let cell = |degree, steps| Cell { degree, steps };
        assert_eq!(first[0], cell(1, 3));
        assert_eq!(first[14], cell(2, 2));
        assert_eq!(second[6], cell(6, 6));
        assert_eq!(first[1], cell(0, 0));
        assert_eq!(&BASS_LEAPING, b"R..O..R.R..O..5.");
        // A dash ends the note before it.
        assert_eq!(starts(&MOTIF_C), [4, 6]);
        assert_eq!(MOTIF_C[6], cell(8, 2));
        assert_eq!(BRIDGE_BASS[10], cell(5, 1));
    }

    #[test]
    fn the_motifs_use_five_degrees() {
        for line in VERSE.iter().chain(&CHORUS) {
            for cell in line.iter().filter(|cell| cell.degree != 0) {
                assert!(FIVE.contains(&((cell.degree - 1) % 7 + 1)), "{cell:?}");
            }
        }
        // Hirajoshi in Aeolian, and the same with a higher sixth degree in Dorian.
        let five = |mode: Mode| FIVE.map(|degree| name(A4 + mode.semitones(degree)));
        assert_eq!(five(Mode::Dorian), ["A", "B", "C", "E", "F#"]);
        assert_eq!(five(Mode::Aeolian), ["A", "B", "C", "E", "F"]);
    }

    #[test]
    fn the_motifs_share_the_rhythm_of_the_bass() {
        let rhythm = [0, 3, 6, 8, 11, 14];
        assert_eq!(starts(&MOTIF_A[0]), rhythm);
        assert_eq!(starts(&MOTIF_B[1]), rhythm);
        let bass: Vec<usize> = (0..STEPS)
            .filter(|&step| BASS_PLAIN[step] != b'.')
            .collect();
        assert_eq!(bass, rhythm);
    }

    #[test]
    fn chords_are_major_or_minor_in_every_mode() {
        // The chords on 1, 4 and 5, then the one that gives the mode its color.
        let chords = |mode: Mode| {
            [1, 4, 5, mode.color()].map(|chord| {
                let [root, third, fifth] = triad(mode, chord, A4);
                assert_eq!(fifth - root, 7);
                let minor = match third - root {
                    3 => "m",
                    4 => "",
                    other => panic!("a third of {other} semitones"),
                };
                format!("{}{minor}", name(root))
            })
        };
        assert_eq!(chords(Mode::Dorian), ["Am", "D", "Em", "G"]);
        assert_eq!(chords(Mode::Aeolian), ["Am", "Dm", "Em", "F"]);
        for mode in MODES {
            let chords = mode.verse_chords().into_iter().chain(mode.chorus_chords());
            for chord in chords {
                assert!([1, 4, 5, mode.color()].contains(&chord));
            }
        }
    }

    #[test]
    fn derived_lines_read_as_expected() {
        let [a, b] = MOTIF_A;
        assert_eq!(grid(&skeleton(a)), "1... .... 5... ....");
        assert_eq!(grid(&skeleton(b)), "3... .... .... 5...");
        assert_eq!(skeleton(a)[0].steps, 8);
        assert_eq!(skeleton(b)[0].steps, 12);
        assert_eq!(skeleton(b)[12].steps, 4);
        assert_eq!(grid(&ornaments(a)), "1212 323. 5653 532.");
        assert_eq!(grid(&ornaments(MOTIF_B[0])), "8... 989. 8.98 6.86");
        // A note keeps its length less the two steps of its ornament.
        assert_eq!(ornaments(b)[6].steps, 4);
        assert_eq!(grid(&ornaments(b)), "3535 656. ..86 5.65");

        assert_eq!(grid(&VERSE[7]), "3..5 ..2. .... 5...");
    }

    #[test]
    fn no_note_rubs_against_the_root_of_its_chord() {
        for mode in MODES {
            let sections = [(VERSE, mode.verse_chords()), (CHORUS, mode.chorus_chords())];
            for (lines, chords) in sections {
                assert_eq!(rubs(mode, &lines, &chords), 0, "{mode:?}");
                assert_eq!(rubs(mode, &skeletons(lines), &chords), 0, "{mode:?}");
                assert_eq!(rubs(mode, &ornamented(lines), &chords), 0, "{mode:?}");
            }
        }
    }

    #[test]
    fn the_bridge_has_no_mode() {
        let notes = || {
            let cells = BRIDGE.iter().chain([&BRIDGE_BASS]).flatten();
            cells.map(|cell| cell.degree).filter(|&degree| degree != 0)
        };
        assert_eq!(notes().count(), 2 * BARS + 2);
        for degree in notes() {
            assert!(NEUTRAL.contains(&((degree - 1) % 7 + 1)), "degree {degree}");
            // The same note whatever the mode, and never a semitone above the pedal.
            let [dorian, aeolian] = MODES.map(|mode| mode.semitones(degree));
            assert_eq!(dorian, aeolian, "degree {degree}");
            assert_ne!((dorian + 12 - Mode::Dorian.semitones(PEDAL)) % 12, 1);
        }
        // Every call starts on the pedal, and only the last one falls.
        let calls: Vec<u8> = BRIDGE.iter().map(|line| line[6].degree).collect();
        assert!(BRIDGE.iter().all(|line| line[4].degree == PEDAL));
        assert_eq!(calls, [8, 7, 8, 9, 8, 7, 8, 4]);
    }

    #[test]
    fn the_turnaround_keeps_the_verse_sound_in_minor() {
        // Without it, the last bar of the verse holds F over E minor.
        let mut plain = VERSE;
        plain[7] = MOTIF_A[1];
        assert_eq!(rubs(Mode::Dorian, &plain, &Mode::Dorian.verse_chords()), 0);
        assert_eq!(
            rubs(Mode::Aeolian, &plain, &Mode::Aeolian.verse_chords()),
            1
        );
    }

    #[test]
    fn every_instrument_stays_in_its_range() {
        let (e2, d3, d4, a5, b6) = (40, 50, 62, 81, 95);
        for mode in MODES {
            for chord in [1, 4, 5, mode.color()] {
                let root = bass_root(mode, chord);
                assert!((e2..=d3).contains(&root), "bass on {chord}");
                // The leaping bass goes an octave above its root.
                assert!(root + 12 <= d4);
                assert!(
                    triad(mode, chord, A3)
                        .iter()
                        .all(|pitch| (A3..=a5).contains(pitch))
                );
            }
            // The lead, with its ornaments and an octave higher.
            for line in ornamented(VERSE).iter().chain(&ornamented(CHORUS)) {
                for cell in line.iter().filter(|cell| cell.degree != 0) {
                    assert!(A4 + mode.semitones(cell.degree) + 12 <= b6);
                }
            }
        }
        // The pluck has the melody of the intro from A4 and that of the bridge from A3, and
        // the bass has its notes of the bridge from A1.
        let highest = |lines: &[Line; BARS]| {
            let cells = lines.iter().flatten();
            cells.map(|cell| cell.degree).max().unwrap()
        };
        assert!(A4 + Mode::Dorian.semitones(highest(&VERSE)) <= a5);
        assert!(A3 + Mode::Dorian.semitones(highest(&BRIDGE)) <= a5);
        assert_eq!(A1 + Mode::Dorian.semitones(PEDAL), e2);
    }
}
