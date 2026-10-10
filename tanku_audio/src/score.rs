//! The piece as it is written by hand: two motifs, the chords of each section and the rhythm
//! patterns, with the rules that derive every other line from them.
//!
//! Melodies are written in degrees of the scale and not in notes, so each of them exists in
//! both modes. A grid is one bar of 16 steps in four beats. In a melody, a digit starts a note
//! on that degree and a dot holds it. In a pattern, `x` is a hit, `R` is the root of the chord
//! of the bar, `O` is that root an octave higher, `5` is its fifth, and a dot starts nothing.

/// Steps in a bar. A step is a quarter of a beat.
pub const STEPS: usize = 16;
/// Bars in a section.
pub const BARS: usize = 8;

/// The A of 440 Hz. Pitches count semitones, and this one is the tonic of the lead.
pub const A4: u8 = 69;
pub const A3: u8 = A4 - 12;
pub const A2: u8 = A3 - 12;

/// The scale in use. Both have A as their tonic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Major.
    Ionian,
    /// Minor.
    Aeolian,
}

impl Mode {
    /// Semitones from the tonic up to a degree. Degree 1 is the tonic, and degree 8 is the
    /// tonic an octave higher.
    pub const fn semitones(self, degree: u8) -> u8 {
        let scale = match self {
            Mode::Ionian => [0, 2, 4, 5, 7, 9, 11],
            Mode::Aeolian => [0, 2, 3, 5, 7, 8, 10],
        };
        let index = degree as usize - 1;
        12 * (index / 7) as u8 + scale[index % 7]
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
/// The melody of the bridge: motif B twice, at half speed.
pub const BRIDGE: [Line; BARS] = {
    let [a, b, c, d] = half_speed(MOTIF_B);
    [a, b, c, d, a, b, c, d]
};

/// The chords of each section, one per bar, each named by the degree it is built on.
pub const VERSE_CHORDS: [u8; BARS] = [1, 1, 6, 4, 1, 1, 6, 5];
pub const CHORUS_CHORDS: [u8; BARS] = [4, 5, 1, 6, 4, 5, 1, 1];
pub const BRIDGE_CHORDS: [u8; BARS] = [4, 4, 1, 1, 4, 4, 5, 5];

pub const KICK_CALM: Pattern = pattern("x... .... x... ....");
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

/// The degrees that the motifs use, within one octave. In Ionian they make the major
/// pentatonic scale, and in Aeolian the scale known as hirajoshi.
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

/// Makes every note of a motif twice as long, so that its two bars become four.
pub const fn half_speed(motif: [Line; 2]) -> [Line; 4] {
    let mut out = [[REST; STEPS]; 4];
    let mut bar = 0;
    while bar < motif.len() {
        let mut step = 0;
        while step < STEPS {
            let Cell { degree, steps } = motif[bar][step];
            if degree != 0 {
                let at = 2 * (bar * STEPS + step);
                out[at / STEPS][at % STEPS] = Cell {
                    degree,
                    steps: 2 * steps,
                };
            }
            step += 1;
        }
        bar += 1;
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

/// The three notes of the chord on a degree, each in the 12 semitones that start at `lowest`.
/// Every chord then lies close to the one before it.
pub const fn close_triad(mode: Mode, chord: u8, lowest: u8) -> [u8; 3] {
    let [root, third, fifth] = triad(mode, chord, 0);
    [lowest + root % 12, lowest + third % 12, lowest + fifth % 12]
}

/// The root of a chord for the bass, between E2 and D3.
pub const fn bass_root(mode: Mode, chord: u8) -> u8 {
    let pitch = A2 + mode.semitones(chord);
    if pitch > A2 + 5 { pitch - 12 } else { pitch }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODES: [Mode; 2] = [Mode::Ionian, Mode::Aeolian];

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
    fn the_modes_are_those_of_the_specification() {
        let scale = |mode: Mode| (1..=7).map(move |degree| name(A4 + mode.semitones(degree)));
        assert!(scale(Mode::Ionian).eq(["A", "B", "C#", "D", "E", "F#", "G#"]));
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
    }

    #[test]
    fn the_motifs_use_five_degrees() {
        for line in VERSE.iter().chain(&CHORUS).chain(&BRIDGE) {
            for cell in line.iter().filter(|cell| cell.degree != 0) {
                assert!(FIVE.contains(&((cell.degree - 1) % 7 + 1)), "{cell:?}");
            }
        }
        // The major pentatonic scale in one mode and hirajoshi in the other.
        let five = |mode: Mode| FIVE.map(|degree| name(A4 + mode.semitones(degree)));
        assert_eq!(five(Mode::Ionian), ["A", "B", "C#", "E", "F#"]);
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
    fn chords_are_major_or_minor_in_both_modes() {
        let chords = |mode: Mode| {
            [1, 4, 5, 6].map(|chord| {
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
        assert_eq!(chords(Mode::Ionian), ["A", "D", "E", "F#m"]);
        assert_eq!(chords(Mode::Aeolian), ["Am", "Dm", "Em", "F"]);
        for chord in VERSE_CHORDS
            .iter()
            .chain(&CHORUS_CHORDS)
            .chain(&BRIDGE_CHORDS)
        {
            assert!([1, 4, 5, 6].contains(chord));
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

        let slow = half_speed(MOTIF_B).map(|line| grid(&line));
        assert_eq!(
            slow,
            [
                "8... .... .... 9...",
                "8... .... 6... ....",
                "5... ..6. .... 5...",
                "3... ..2. .... 1...",
            ]
        );
        assert_eq!(BRIDGE[0][0].steps, 12);
        assert_eq!(grid(&VERSE[7]), "3..5 ..2. .... 5...");
    }

    #[test]
    fn no_note_rubs_against_the_root_of_its_chord() {
        for mode in MODES {
            for (lines, chords) in [(VERSE, VERSE_CHORDS), (CHORUS, CHORUS_CHORDS)] {
                assert_eq!(rubs(mode, &lines, &chords), 0, "{mode:?}");
                assert_eq!(rubs(mode, &skeletons(lines), &chords), 0, "{mode:?}");
                assert_eq!(rubs(mode, &ornamented(lines), &chords), 0, "{mode:?}");
            }
        }
        // The bridge is only played in Ionian.
        assert_eq!(rubs(Mode::Ionian, &BRIDGE, &BRIDGE_CHORDS), 0);
    }

    #[test]
    fn the_turnaround_keeps_the_verse_sound_in_minor() {
        // Without it, the last bar of the verse holds F over E minor.
        let mut plain = VERSE;
        plain[7] = MOTIF_A[1];
        assert_eq!(rubs(Mode::Ionian, &plain, &VERSE_CHORDS), 0);
        assert_eq!(rubs(Mode::Aeolian, &plain, &VERSE_CHORDS), 1);
    }

    #[test]
    fn every_instrument_stays_in_its_range() {
        let (e2, d3, d4, a5, b6) = (40, 50, 62, 81, 95);
        for mode in MODES {
            for chord in [1, 4, 5, 6] {
                let root = bass_root(mode, chord);
                assert!((e2..=d3).contains(&root), "bass on {chord}");
                // The leaping bass goes an octave above its root.
                assert!(root + 12 <= d4);
                assert!(
                    triad(mode, chord, A3)
                        .iter()
                        .all(|pitch| (A3..=a5).contains(pitch))
                );
                let held = close_triad(mode, chord, A4);
                assert!(held.iter().all(|pitch| (A4..A4 + 12).contains(pitch)));
            }
            // The lead, with its ornaments and an octave higher.
            for line in ornamented(VERSE).iter().chain(&ornamented(CHORUS)) {
                for cell in line.iter().filter(|cell| cell.degree != 0) {
                    assert!(A4 + mode.semitones(cell.degree) + 12 <= b6);
                }
            }
        }
        // The pluck has the melody of the intro from A4 and that of the bridge from A3.
        let highest = |lines: &[Line; BARS]| {
            let cells = lines.iter().flatten();
            cells.map(|cell| cell.degree).max().unwrap()
        };
        assert!(A4 + Mode::Ionian.semitones(highest(&VERSE)) <= a5);
        assert!(A3 + Mode::Ionian.semitones(highest(&BRIDGE)) <= a5);
    }
}
