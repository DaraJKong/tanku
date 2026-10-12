//! The notation of `music.score`, the file in which the piece is written by hand, and the
//! reader that turns its text into a [`Piece`].
//!
//! The crate reads the file while it is compiled: a file that can't be read stops the build,
//! and the error says on which line. The reader is a plain function, so a program can also
//! read a piece while it runs.
//!
//! The file is text, with one record per line and its words apart by spaces. A line that
//! starts with `#` is a comment. `docs/MUSIC.md` describes the records.
//!
//! A bar is written beat by beat: four words of four steps. In a melody, a digit starts a note
//! on that degree of the scale, a dot holds it and a dash is silence. In a pattern, `x` is a
//! hit and `o` a softer one. On an instrument that has a pitch, a hit is the next note of the
//! chord of the bar, whose notes go up in turn, `R` is the root of that chord, `O` that root an
//! octave higher and `V` its fifth, a digit is a degree of the scale, a dot holds the note and
//! a dash ends it.

use std::fmt;

use crate::score::Mode;

/// Steps in a beat. A step is a quarter of a beat.
pub const BEAT: usize = 4;
/// Steps in a bar, which has four beats.
pub const STEPS: usize = 4 * BEAT;
/// Bars in a section.
pub const BARS: usize = 8;
/// Bars in a phrase. The chords of a section are written a phrase at a time.
const PHRASE: usize = 4;

/// Motifs that a piece can have, and bars that a motif can have.
const MOTIFS: usize = 8;
const MOTIF_BARS: usize = 4;
/// Patterns and sections that a piece can have.
const RHYTHMS: usize = 16;
const PASSAGES: usize = 8;
/// Letters that a name can have.
const WORD: usize = 12;

/// In the chords of a section, the chord that gives the mode its color, which the file writes
/// as `C`. The other chords are the degree they are built on.
pub const COLOR: u8 = 0;

/// What starts on one step of a melody: a note on a degree that lasts a number of steps, or
/// nothing when the degree is 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    pub degree: u8,
    pub steps: u8,
}

pub(crate) const REST: Cell = Cell {
    degree: 0,
    steps: 0,
};

/// One bar of a melody.
pub type Line = [Cell; STEPS];

/// One bar of a rhythm: the symbols of its grid.
pub type Pattern = [u8; STEPS];

/// Why a file can't be read, and on which of its lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fault {
    pub line: u32,
    pub what: &'static str,
}

impl Fault {
    /// Stops the build with the line of the file and what is wrong with it.
    pub const fn stop(self) -> ! {
        let mut text = [0; 200];
        let mut len = append(&mut text, 0, b"music.score, line ");
        let mut digits = 1;
        while self.line / digits >= 10 {
            digits *= 10;
        }
        while digits > 0 {
            len = append(&mut text, len, &[b'0' + (self.line / digits % 10) as u8]);
            digits /= 10;
        }
        len = append(&mut text, len, b": ");
        len = append(&mut text, len, self.what.as_bytes());
        match std::str::from_utf8(text.split_at(len).0) {
            Ok(message) => panic!("{}", message),
            Err(_) => panic!("music.score can't be read"),
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.what)
    }
}

/// Writes some bytes into a text from a place on, as far as they fit, and tells where they
/// end.
const fn append(text: &mut [u8], mut len: usize, bytes: &[u8]) -> usize {
    let mut index = 0;
    while index < bytes.len() && len < text.len() {
        text[len] = bytes[index];
        len += 1;
        index += 1;
    }
    len
}

/// A name of the file: that of a pattern or of a section.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Word {
    letters: [u8; WORD],
    len: u8,
}

impl Word {
    const EMPTY: Self = Self {
        letters: [0; WORD],
        len: 0,
    };

    const fn new(letters: &[u8]) -> Option<Self> {
        if letters.len() > WORD {
            return None;
        }
        let mut word = Self::EMPTY;
        word.len = append(&mut word.letters, 0, letters) as u8;
        Some(word)
    }

    const fn letters(&self) -> &[u8] {
        self.letters.split_at(self.len as usize).0
    }

    pub const fn is(&self, name: &str) -> bool {
        same(self.letters(), name.as_bytes())
    }

    const fn equals(&self, other: &Self) -> bool {
        same(self.letters(), other.letters())
    }
}

impl fmt::Debug for Word {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let letters = &self.letters[..self.len as usize];
        write!(formatter, "{}", String::from_utf8_lossy(letters))
    }
}

/// Whether two texts are the same.
pub(crate) const fn same(text: &[u8], other: &[u8]) -> bool {
    if text.len() != other.len() {
        return false;
    }
    let mut index = 0;
    while index < text.len() {
        if text[index] != other[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// The instruments that a line of the file can name. The lead has no pattern: it plays the
/// melodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instrument {
    Kick,
    Hat,
    Snare,
    Bass,
    Pluck,
}

impl Instrument {
    const fn named(name: &[u8]) -> Option<Self> {
        if same(name, b"kick") {
            Some(Self::Kick)
        } else if same(name, b"hat") {
            Some(Self::Hat)
        } else if same(name, b"snare") {
            Some(Self::Snare)
        } else if same(name, b"bass") {
            Some(Self::Bass)
        } else if same(name, b"pluck") {
            Some(Self::Pluck)
        } else {
            None
        }
    }

    /// Whether the instrument is a drum, which has no pitch.
    pub const fn is_drum(self) -> bool {
        matches!(self, Self::Kick | Self::Hat | Self::Snare)
    }
}

/// The key of the piece: its tonic, the mode of the piece and the mode of the fights of an
/// arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    /// Semitones from A up to the tonic, from 0 to 11.
    pub tonic: u8,
    pub home: Mode,
    pub arena: Mode,
}

impl Key {
    /// Semitones from A to the nearest tonic, which is below it for the keys from E to G
    /// sharp.
    pub const fn offset(self) -> i8 {
        if self.tonic > 6 {
            self.tonic as i8 - 12
        } else {
            self.tonic as i8
        }
    }
}

/// A motif: a melody of a few bars, named by a capital letter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Motif {
    pub letter: u8,
    bars: [Line; MOTIF_BARS],
    len: u8,
    /// The bar that the motif ends a section with, in place of its last one.
    pub turn: Option<Line>,
}

impl Motif {
    const EMPTY: Self = Self {
        letter: 0,
        bars: [[REST; STEPS]; MOTIF_BARS],
        len: 0,
        turn: None,
    };

    pub const fn bars(&self) -> &[Line] {
        self.bars.split_at(self.len as usize).0
    }
}

/// A pattern: one bar of rhythm for an instrument, with how loud its hits are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rhythm {
    pub instrument: Instrument,
    pub name: Word,
    pub grid: Pattern,
    /// How loud a hit is, up to 255, and a soft one. A pattern without soft hits has 0 for
    /// them.
    pub level: u8,
    pub soft: u8,
}

impl Rhythm {
    const EMPTY: Self = Self {
        instrument: Instrument::Kick,
        name: Word::EMPTY,
        grid: [b'.'; STEPS],
        level: 0,
        soft: 0,
    };
}

/// A section as the file writes it: eight bars with their chords and their melody.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Passage {
    pub name: Word,
    /// One chord per bar: the degree it is built on, or [`COLOR`].
    pub chords: [u8; BARS],
    pub melody: [Line; BARS],
    /// Beats per minute: its own tempo, or that of the piece.
    pub tempo: u16,
}

impl Passage {
    const EMPTY: Self = Self {
        name: Word::EMPTY,
        chords: [1; BARS],
        melody: [[REST; STEPS]; BARS],
        tempo: 0,
    };
}

/// The roll that announces a fight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roll {
    pub beats: u8,
    /// The drum that rolls, and the one that rolls in an arena.
    pub drum: Instrument,
    pub arena: Instrument,
    /// How loud the roll starts and how loud it ends, up to 255.
    pub from: u8,
    pub to: u8,
}

/// The connection that leads from a fight into what follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    pub beats: u8,
    /// The degree that the tune goes to.
    pub degree: u8,
    /// Notes that the way there has at most.
    pub notes: u8,
}

/// The piece as the file writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    /// Beats per minute.
    pub tempo: u16,
    pub key: Key,
    motifs: [Motif; MOTIFS],
    motif_count: usize,
    rhythms: [Rhythm; RHYTHMS],
    rhythm_count: usize,
    passages: [Passage; PASSAGES],
    passage_count: usize,
    pub roll: Roll,
    pub link: Link,
}

impl Piece {
    const EMPTY: Self = Self {
        tempo: 0,
        key: Key {
            tonic: 0,
            home: Mode::Dorian,
            arena: Mode::Dorian,
        },
        motifs: [Motif::EMPTY; MOTIFS],
        motif_count: 0,
        rhythms: [Rhythm::EMPTY; RHYTHMS],
        rhythm_count: 0,
        passages: [Passage::EMPTY; PASSAGES],
        passage_count: 0,
        roll: Roll {
            beats: 0,
            drum: Instrument::Hat,
            arena: Instrument::Hat,
            from: 0,
            to: 0,
        },
        link: Link {
            beats: 0,
            degree: 0,
            notes: 0,
        },
    };

    pub const fn motifs(&self) -> &[Motif] {
        self.motifs.split_at(self.motif_count).0
    }

    pub const fn rhythms(&self) -> &[Rhythm] {
        self.rhythms.split_at(self.rhythm_count).0
    }

    pub const fn passages(&self) -> &[Passage] {
        self.passages.split_at(self.passage_count).0
    }

    /// Where the motif of a letter is among the motifs.
    const fn motif_at(&self, letter: u8) -> Option<usize> {
        let mut index = 0;
        while index < self.motif_count {
            if self.motifs[index].letter == letter {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    pub const fn motif(&self, letter: u8) -> Option<Motif> {
        match self.motif_at(letter) {
            Some(index) => Some(self.motifs[index]),
            None => None,
        }
    }

    /// The pattern of an instrument by its name.
    pub const fn rhythm(&self, instrument: Instrument, name: &str) -> Option<Rhythm> {
        let mut index = 0;
        while index < self.rhythm_count {
            let rhythm = self.rhythms[index];
            if rhythm.instrument as u8 == instrument as u8 && rhythm.name.is(name) {
                return Some(rhythm);
            }
            index += 1;
        }
        None
    }

    /// The section of a name.
    pub const fn passage(&self, name: &str) -> Option<Passage> {
        let mut index = 0;
        while index < self.passage_count {
            if self.passages[index].name.is(name) {
                return Some(self.passages[index]);
            }
            index += 1;
        }
        None
    }
}

/// The words of one line, one after the other.
#[derive(Clone, Copy)]
struct Words<'a> {
    text: &'a [u8],
    at: usize,
    end: usize,
}

impl<'a> Words<'a> {
    const fn next(&mut self) -> Option<&'a [u8]> {
        while self.at < self.end && is_space(self.text[self.at]) {
            self.at += 1;
        }
        if self.at == self.end {
            return None;
        }
        let start = self.at;
        while self.at < self.end && !is_space(self.text[self.at]) {
            self.at += 1;
        }
        Some(self.text.split_at(self.at).0.split_at(start).1)
    }

    /// Whether the line has no word left.
    const fn is_over(&self) -> bool {
        let mut ahead = *self;
        ahead.next().is_none()
    }
}

const fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r')
}

/// A whole number from its digits, if the word is one of five digits at most.
const fn number(word: &[u8]) -> Option<u32> {
    if word.is_empty() || word.len() > 5 {
        return None;
    }
    let (mut value, mut index) = (0, 0);
    while index < word.len() {
        if !word[index].is_ascii_digit() {
            return None;
        }
        value = 10 * value + (word[index] - b'0') as u32;
        index += 1;
    }
    Some(value)
}

/// The next word as a number, if it is one between two bounds.
const fn amount(words: &mut Words, least: u32, most: u32) -> Option<u32> {
    let Some(word) = words.next() else {
        return None;
    };
    match number(word) {
        Some(value) if value >= least && value <= most => Some(value),
        _ => None,
    }
}

/// Gives what was found, or leaves with what is wrong.
macro_rules! need {
    ($found:expr, $what:expr) => {
        match $found {
            Some(value) => value,
            None => return Err($what),
        }
    };
}

/// Gives the value that was read, or leaves with what is wrong.
macro_rules! tri {
    ($read:expr) => {
        match $read {
            Ok(value) => value,
            Err(what) => return Err(what),
        }
    };
}

/// The records that a file has once at most, and whether each was found.
struct Found {
    heading: bool,
    tempo: bool,
    key: bool,
    roll: bool,
    link: bool,
}

const HEADING: &str = "the file starts with the line `tanku-music 1`";
const KINDS: &str =
    "a line starts with `tempo`, `key`, `motif`, `turn`, `pat`, `sec`, `roll` or `link`";
const TEMPO: &str = "a tempo is a number of beats per minute, from 30 to 300";
const TONIC: &str = "a key starts with its tonic, such as `A`, `C#` or `Eb`";
const MODES: &str = "a key has two modes, that of the piece and that of the fights of an arena: \
                     `ionian`, `dorian`, `phrygian`, `lydian`, `mixolydian`, `aeolian` or \
                     `locrian`";
const BAR: &str = "a bar is written beat by beat, as four words of four steps, such as \
                   `x... x... x... x...`";
const MELODY: &str =
    "in a melody, a step is a degree from 1 to 9, a dot that holds it or a dash for silence";
const LETTER: &str = "a motif is named by one capital letter";
const NAME: &str = "a name has twelve letters at most";
const INSTRUMENT: &str =
    "a pattern is for the `kick`, the `hat`, the `snare`, the `bass` or the `pluck`";
const DRUM: &str = "on a drum, a step is `x` for a hit, `o` for a soft one or a dot";
const PITCHED: &str = "on the bass and the pluck, a step is `x`, `o`, `R`, `O`, `V`, a degree \
                       from 1 to 9, a dot or a dash";
const LEVEL: &str = "a level is a number from 1 to 255";
const CHORDS: &str = "the chords of a section are two words of four, one chord per bar: a \
                      degree from 1 to 7, or `C` for the chord that gives the mode its color";
const STATEMENT: &str = "a melody is made of motifs: `A`, `a` for A with its turn, or `C:7` for \
                         C with its last note on degree 7";
const FILLS: &str = "the melody of a section is eight bars long";
const AFTER: &str = "after its eight bars of melody, a section can only have a tempo of its \
                     own, from 30 to 300";
const ROLL: &str = "a roll has its beats, from 1 to 4, the drum that rolls, the drum that rolls \
                    in an arena, and how loud it starts and ends, from 1 to 255";
const LINK: &str = "a connection has its beats, from 1 to 4, the degree it goes to, from 1 to \
                    9, and how many notes it has at most, which is two per beat or fewer";

/// Reads a piece from the text of its file.
pub const fn read(text: &str) -> Result<Piece, Fault> {
    let text = text.as_bytes();
    let mut piece = Piece::EMPTY;
    let mut found = Found {
        heading: false,
        tempo: false,
        key: false,
        roll: false,
        link: false,
    };
    let (mut at, mut line) = (0, 0);
    while at < text.len() {
        let mut end = at;
        while end < text.len() && text[end] != b'\n' {
            end += 1;
        }
        let mut words = Words { text, at, end };
        at = end + 1;
        line += 1;
        let Some(kind) = words.next() else {
            continue;
        };
        if kind[0] == b'#' {
            continue;
        }
        if let Err(what) = record(&mut piece, &mut found, kind, &mut words) {
            return Err(Fault { line, what });
        }
        if !words.is_over() {
            let what = "this line has a word too many";
            return Err(Fault { line, what });
        }
    }

    let missing = if !found.heading {
        Some(HEADING)
    } else if !found.tempo {
        Some("the file has no `tempo` line")
    } else if !found.key {
        Some("the file has no `key` line")
    } else if !found.roll {
        Some("the file has no `roll` line")
    } else if !found.link {
        Some("the file has no `link` line")
    } else {
        None
    };
    if let Some(what) = missing {
        return Err(Fault { line, what });
    }
    // A section without a tempo of its own has that of the piece.
    let mut index = 0;
    while index < piece.passage_count {
        if piece.passages[index].tempo == 0 {
            piece.passages[index].tempo = piece.tempo;
        }
        index += 1;
    }
    Ok(piece)
}

/// Reads one line into the piece, from the word that says what the line is.
const fn record(
    piece: &mut Piece,
    found: &mut Found,
    kind: &[u8],
    words: &mut Words,
) -> Result<(), &'static str> {
    if !found.heading {
        found.heading = true;
        let version = match words.next() {
            Some(word) => number(word),
            None => None,
        };
        return if same(kind, b"tanku-music") && matches!(version, Some(1)) {
            Ok(())
        } else {
            Err(HEADING)
        };
    }
    if same(kind, b"tempo") {
        if found.tempo {
            return Err("the tempo is written twice");
        }
        found.tempo = true;
        piece.tempo = tri!(tempo(words));
    } else if same(kind, b"key") {
        if found.key {
            return Err("the key is written twice");
        }
        found.key = true;
        piece.key = tri!(key(words));
    } else if same(kind, b"motif") {
        tri!(motif(piece, words));
    } else if same(kind, b"turn") {
        tri!(turn(piece, words));
    } else if same(kind, b"pat") {
        tri!(rhythm(piece, words));
    } else if same(kind, b"sec") {
        tri!(passage(piece, words));
    } else if same(kind, b"roll") {
        if found.roll {
            return Err("the roll is written twice");
        }
        found.roll = true;
        piece.roll = tri!(roll(words));
    } else if same(kind, b"link") {
        if found.link {
            return Err("the connection is written twice");
        }
        found.link = true;
        piece.link = tri!(link(words));
    } else {
        return Err(KINDS);
    }
    Ok(())
}

const fn tempo(words: &mut Words) -> Result<u16, &'static str> {
    match amount(words, 30, 300) {
        Some(tempo) => Ok(tempo as u16),
        None => Err(TEMPO),
    }
}

const fn key(words: &mut Words) -> Result<Key, &'static str> {
    let tonic = match words.next() {
        Some(note) => tonic(note),
        None => None,
    };
    let tonic = need!(tonic, TONIC);
    let home = need!(mode(words), MODES);
    let arena = need!(mode(words), MODES);
    Ok(Key { tonic, home, arena })
}

/// Semitones from A up to a note, from its name.
const fn tonic(note: &[u8]) -> Option<u8> {
    if note.is_empty() || note.len() > 2 {
        return None;
    }
    let natural = match note[0] {
        b'A' => 0,
        b'B' => 2,
        b'C' => 3,
        b'D' => 5,
        b'E' => 7,
        b'F' => 8,
        b'G' => 10,
        _ => return None,
    };
    let bent = if note.len() == 1 {
        12
    } else {
        match note[1] {
            b'#' => 13,
            b'b' => 11,
            _ => return None,
        }
    };
    Some((natural + bent) % 12)
}

const fn mode(words: &mut Words) -> Option<Mode> {
    match words.next() {
        Some(name) => Mode::named(name),
        None => None,
    }
}

/// Reads a bar: four beats, one word each.
const fn bar(words: &mut Words) -> Result<Pattern, &'static str> {
    let mut grid = [b'.'; STEPS];
    let mut step = 0;
    while step < STEPS {
        let beat = need!(words.next(), BAR);
        if beat.len() != BEAT {
            return Err(BAR);
        }
        let mut index = 0;
        while index < BEAT {
            grid[step] = beat[index];
            step += 1;
            index += 1;
        }
    }
    Ok(grid)
}

/// A bar of melody from its grid.
const fn melody(grid: Pattern) -> Result<Line, &'static str> {
    let mut line = [REST; STEPS];
    let mut last = STEPS;
    let mut step = 0;
    while step < STEPS {
        match grid[step] {
            b'.' => {
                if last < STEPS {
                    line[last].steps += 1;
                }
            }
            b'-' => last = STEPS,
            symbol @ b'1'..=b'9' => {
                line[step] = Cell {
                    degree: symbol - b'0',
                    steps: 1,
                };
                last = step;
            }
            _ => return Err(MELODY),
        }
        step += 1;
    }
    Ok(line)
}

/// The next word as the letter of a motif.
const fn letter(words: &mut Words) -> Option<u8> {
    match words.next() {
        Some(word) if word.len() == 1 && word[0].is_ascii_uppercase() => Some(word[0]),
        _ => None,
    }
}

const fn motif(piece: &mut Piece, words: &mut Words) -> Result<(), &'static str> {
    let letter = need!(letter(words), LETTER);
    if piece.motif_at(letter).is_some() {
        return Err("this motif is written twice");
    }
    if piece.motif_count == MOTIFS {
        return Err("a piece has eight motifs at most");
    }
    let mut motif = Motif {
        letter,
        ..Motif::EMPTY
    };
    loop {
        if motif.len as usize == MOTIF_BARS {
            return Err("a motif has four bars at most");
        }
        motif.bars[motif.len as usize] = tri!(melody(tri!(bar(words))));
        motif.len += 1;
        // Another bar follows a `|`.
        let mut ahead = *words;
        match ahead.next() {
            None => break,
            Some(word) if same(word, b"|") => *words = ahead,
            Some(_) => return Err("the bars of a motif are apart by `|`"),
        }
    }
    piece.motifs[piece.motif_count] = motif;
    piece.motif_count += 1;
    Ok(())
}

const fn turn(piece: &mut Piece, words: &mut Words) -> Result<(), &'static str> {
    let letter = need!(letter(words), LETTER);
    let index = need!(
        piece.motif_at(letter),
        "a turn comes after the motif that it ends"
    );
    if piece.motifs[index].turn.is_some() {
        return Err("this turn is written twice");
    }
    piece.motifs[index].turn = Some(tri!(melody(tri!(bar(words)))));
    Ok(())
}

/// The next word as a name.
const fn name(words: &mut Words) -> Option<Word> {
    match words.next() {
        Some(letters) => Word::new(letters),
        None => None,
    }
}

const fn rhythm(piece: &mut Piece, words: &mut Words) -> Result<(), &'static str> {
    let instrument = match words.next() {
        Some(name) => Instrument::named(name),
        None => None,
    };
    let instrument = need!(instrument, INSTRUMENT);
    let name = need!(name(words), NAME);
    let mut index = 0;
    while index < piece.rhythm_count {
        let other = piece.rhythms[index];
        if other.instrument as u8 == instrument as u8 && other.name.equals(&name) {
            return Err("this pattern is written twice");
        }
        index += 1;
    }
    if piece.rhythm_count == RHYTHMS {
        return Err("a piece has sixteen patterns at most");
    }

    let grid = tri!(bar(words));
    let mut soft_hits = false;
    let mut step = 0;
    while step < STEPS {
        let symbol = grid[step];
        if instrument.is_drum() {
            if !matches!(symbol, b'x' | b'o' | b'.') {
                return Err(DRUM);
            }
        } else if !matches!(
            symbol,
            b'x' | b'o' | b'R' | b'O' | b'V' | b'1'..=b'9' | b'.' | b'-'
        ) {
            return Err(PITCHED);
        }
        soft_hits |= symbol == b'o';
        step += 1;
    }
    let level = need!(amount(words, 1, 255), LEVEL);
    let soft = if words.is_over() {
        0
    } else {
        match amount(words, 1, 255) {
            Some(soft) => soft,
            None => return Err(LEVEL),
        }
    };
    if soft_hits && soft == 0 {
        return Err("a pattern with soft hits says how loud they are, after the level of a hit");
    }
    if !soft_hits && soft != 0 {
        return Err("this pattern has no soft hit, so it has one level");
    }

    piece.rhythms[piece.rhythm_count] = Rhythm {
        instrument,
        name,
        grid,
        level: level as u8,
        soft: soft as u8,
    };
    piece.rhythm_count += 1;
    Ok(())
}

const fn passage(piece: &mut Piece, words: &mut Words) -> Result<(), &'static str> {
    let name = need!(name(words), NAME);
    let mut index = 0;
    while index < piece.passage_count {
        if piece.passages[index].name.equals(&name) {
            return Err("this section is written twice");
        }
        index += 1;
    }
    if piece.passage_count == PASSAGES {
        return Err("a piece has eight sections at most");
    }
    let mut passage = Passage {
        name,
        ..Passage::EMPTY
    };

    let mut bar = 0;
    while bar < BARS {
        let phrase = need!(words.next(), CHORDS);
        if phrase.len() != PHRASE {
            return Err(CHORDS);
        }
        let mut index = 0;
        while index < PHRASE {
            passage.chords[bar] = match phrase[index] {
                b'C' => COLOR,
                symbol @ b'1'..=b'7' => symbol - b'0',
                _ => return Err(CHORDS),
            };
            bar += 1;
            index += 1;
        }
    }

    let mut bars = 0;
    while bars < BARS {
        let token = need!(words.next(), FILLS);
        let played = tri!(statement(piece, token));
        let lines = played.bars();
        if bars + lines.len() > BARS {
            return Err(FILLS);
        }
        let mut index = 0;
        while index < lines.len() {
            passage.melody[bars] = lines[index];
            bars += 1;
            index += 1;
        }
    }
    if !words.is_over() {
        passage.tempo = match amount(words, 30, 300) {
            Some(tempo) => tempo as u16,
            None => return Err(AFTER),
        };
    }

    piece.passages[piece.passage_count] = passage;
    piece.passage_count += 1;
    Ok(())
}

/// A motif as a word of a melody states it: as it is written, with its turn in place of its
/// last bar, or with its last note on another degree.
const fn statement(piece: &Piece, token: &[u8]) -> Result<Motif, &'static str> {
    if token[0].is_ascii_digit() {
        // The melody is over before its eight bars.
        return Err(FILLS);
    }
    if !token[0].is_ascii_alphabetic() || (token.len() != 1 && token.len() != 3) {
        return Err(STATEMENT);
    }
    let turned = token[0].is_ascii_lowercase();
    let index = need!(
        piece.motif_at(token[0].to_ascii_uppercase()),
        "a section comes after the motifs that it plays"
    );
    let mut motif = piece.motifs[index];
    let last = motif.len as usize - 1;
    if token.len() == 3 {
        if turned || token[1] != b':' || !matches!(token[2], b'1'..=b'9') {
            return Err(STATEMENT);
        }
        // The last note of the motif, looked for from its end.
        let (mut bar, mut step) = (last, STEPS);
        loop {
            if step == 0 {
                if bar == 0 {
                    return Err("a motif without a note has no last note to move");
                }
                bar -= 1;
                step = STEPS;
            }
            step -= 1;
            if motif.bars[bar][step].degree != 0 {
                break;
            }
        }
        motif.bars[bar][step].degree = token[2] - b'0';
    } else if turned {
        let turn = need!(motif.turn, "this motif has no turn");
        motif.bars[last] = turn;
    }
    Ok(motif)
}

/// The next word as a drum.
const fn drum(words: &mut Words) -> Option<Instrument> {
    let Some(name) = words.next() else {
        return None;
    };
    match Instrument::named(name) {
        Some(instrument) if instrument.is_drum() => Some(instrument),
        _ => None,
    }
}

const fn roll(words: &mut Words) -> Result<Roll, &'static str> {
    let beats = need!(amount(words, 1, 4), ROLL);
    let [elsewhere, arena] = [need!(drum(words), ROLL), need!(drum(words), ROLL)];
    let from = need!(amount(words, 1, 255), ROLL);
    let to = need!(amount(words, 1, 255), ROLL);
    Ok(Roll {
        beats: beats as u8,
        drum: elsewhere,
        arena,
        from: from as u8,
        to: to as u8,
    })
}

const fn link(words: &mut Words) -> Result<Link, &'static str> {
    let beats = need!(amount(words, 1, 4), LINK);
    let degree = need!(amount(words, 1, 9), LINK);
    let notes = need!(amount(words, 1, 2 * beats), LINK);
    Ok(Link {
        beats: beats as u8,
        degree: degree as u8,
        notes: notes as u8,
    })
}

/// One bar of a melody from its grid, for a test that writes a bar of its own.
#[cfg(test)]
pub(crate) const fn line(grid: &str) -> Line {
    let text = grid.as_bytes();
    let mut words = Words {
        text,
        at: 0,
        end: text.len(),
    };
    let grid = match bar(&mut words) {
        Ok(grid) => grid,
        Err(what) => panic!("{}", what),
    };
    match melody(grid) {
        Ok(line) => line,
        Err(what) => panic!("{}", what),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shortest file that reads: its heading, a tempo, a key, a roll and a connection.
    const SMALL: &str = "tanku-music 1\n\
                         tempo 96\n\
                         key C# ionian mixolydian\n\
                         roll 1 kick snare 20 200\n\
                         link 3 2 6\n";
    /// A motif of one bar and a motif of two, with its turn.
    const MOTIFS: &str = "motif A 1... .... .... ....\n\
                          motif B 1..2 .... 3-.. .... | 5... .... .... 6...\n\
                          turn B 8... .... .... ....\n";

    /// The line on which a file can't be read, and why.
    fn fault(text: &str) -> (u32, &'static str) {
        let fault = read(text).unwrap_err();
        (fault.line, fault.what)
    }

    /// The same for a file that is the small one and some more lines.
    fn fault_after(lines: &str) -> (u32, &'static str) {
        fault(&format!("{SMALL}{lines}"))
    }

    #[test]
    fn the_file_of_the_piece_reads() {
        let piece = read(include_str!("../music.score")).unwrap();
        assert_eq!(piece, crate::score::PIECE);
        assert_eq!(piece.motifs().len(), 3);
        assert_eq!(piece.rhythms().len(), 12);
        assert_eq!(piece.passages().len(), 4);
    }

    #[test]
    fn a_small_file_reads() {
        let piece = read(SMALL).unwrap();
        assert_eq!(piece.tempo, 96);
        let key = piece.key;
        assert_eq!(
            (key.tonic, key.home, key.arena),
            (4, Mode::Ionian, Mode::Mixolydian)
        );
        let roll = piece.roll;
        assert_eq!((roll.beats, roll.from, roll.to), (1, 20, 200));
        assert_eq!(
            (roll.drum, roll.arena),
            (Instrument::Kick, Instrument::Snare)
        );
        let link = piece.link;
        assert_eq!((link.beats, link.degree, link.notes), (3, 2, 6));
        assert!(piece.motifs().is_empty() && piece.rhythms().is_empty());
        assert!(piece.passages().is_empty());
    }

    #[test]
    fn comments_empty_lines_and_line_endings_change_nothing() {
        let dressed = "# A comment.\r\n\r\n  tanku-music   1\r\n#tempo 60\r\ntempo 96\r\n\
                       \tkey C# ionian mixolydian\n\nroll 1 kick snare 20 200\nlink 3 2 6";
        assert_eq!(read(dressed), read(SMALL));
    }

    #[test]
    fn a_key_is_named_by_its_tonic() {
        let offset = |note: &str| {
            let key = read(&SMALL.replace("C#", note)).unwrap().key;
            (key.tonic, key.offset())
        };
        assert_eq!(offset("A"), (0, 0));
        assert_eq!(offset("A#"), (1, 1));
        assert_eq!(offset("Bb"), (1, 1));
        assert_eq!(offset("C"), (3, 3));
        // The tonic is the nearest one to A: up to E flat above it, and down to E below it.
        assert_eq!(offset("Eb"), (6, 6));
        assert_eq!(offset("E"), (7, -5));
        assert_eq!(offset("G#"), (11, -1));
        assert_eq!(offset("Ab"), (11, -1));
        for note in ["H", "a", "A##", "Ax"] {
            assert_eq!(fault(&SMALL.replace("C#", note)), (3, TONIC), "{note}");
        }
    }

    #[test]
    fn a_key_names_two_of_the_seven_modes() {
        for home in Mode::ALL {
            for arena in Mode::ALL {
                let modes = format!("{} {}", home.name(), arena.name());
                let key = read(&SMALL.replace("ionian mixolydian", &modes))
                    .unwrap()
                    .key;
                assert_eq!((key.home, key.arena), (home, arena));
            }
        }
    }

    #[test]
    fn motifs_and_patterns_read_bar_by_bar() {
        let lines = "pat kick heart x.o. .... .... .... 180 135\n\
                     pat bass heart R..O ..V. 5-5. x... 255\n";
        let piece = read(&format!("{SMALL}{MOTIFS}{lines}")).unwrap();
        let [a, b] = piece.motifs() else {
            panic!("two motifs");
        };
        assert_eq!((a.letter, a.bars().len(), a.turn), (b'A', 1, None));
        assert_eq!(
            a.bars()[0][0],
            Cell {
                degree: 1,
                steps: 16
            }
        );
        assert_eq!((b.letter, b.bars().len()), (b'B', 2));
        // A dot holds a note, and a dash ends it.
        let cell = |degree, steps| Cell { degree, steps };
        assert_eq!(b.bars()[0][0], cell(1, 3));
        assert_eq!(b.bars()[0][3], cell(2, 5));
        assert_eq!(b.bars()[0][8], cell(3, 1));
        assert_eq!(b.bars()[1][12], cell(6, 4));
        assert_eq!(b.turn.unwrap()[0], cell(8, 16));
        assert_eq!(piece.motif(b'B'), Some(*b));
        assert_eq!(piece.motif(b'C'), None);

        // Two instruments can have a pattern of the same name.
        let kick = piece.rhythm(Instrument::Kick, "heart").unwrap();
        assert_eq!(&kick.grid, b"x.o.............");
        assert_eq!((kick.level, kick.soft), (180, 135));
        let bass = piece.rhythm(Instrument::Bass, "heart").unwrap();
        assert_eq!(&bass.grid, b"R..O..V.5-5.x...");
        assert_eq!((bass.level, bass.soft), (255, 0));
        assert!(bass.name.is("heart") && !bass.name.is("hear"));
        assert_eq!(piece.rhythm(Instrument::Hat, "heart"), None);
        assert_eq!(piece.rhythm(Instrument::Kick, "fight"), None);
    }

    #[test]
    fn a_section_states_its_motifs() {
        let lines = "sec verse 14C5 7621 A B b A:9 B:4 100\nsec calm 1111 1111 b b b b\n";
        let piece = read(&format!("{SMALL}{MOTIFS}{lines}")).unwrap();
        let verse = piece.passage("verse").unwrap();
        assert_eq!(verse.chords, [1, 4, COLOR, 5, 7, 6, 2, 1]);
        let [a, b] = (*b"AB").map(|letter| piece.motif(letter).unwrap());
        let (first, second, turn) = (b.bars()[0], b.bars()[1], b.turn.unwrap());
        // The motif as it is written, then with its turn in place of its last bar.
        assert_eq!(verse.melody[..5], [a.bars()[0], first, second, first, turn]);
        // Then each motif with its last note on another degree.
        let cell = |degree, steps| Cell { degree, steps };
        assert_eq!(verse.melody[5][0], cell(9, 16));
        assert_eq!(verse.melody[6], first);
        assert_eq!(verse.melody[7][0], second[0]);
        assert_eq!(verse.melody[7][12], cell(4, 4));
        // A section has its own tempo, or that of the piece.
        assert_eq!(verse.tempo, 100);
        let calm = piece.passage("calm").unwrap();
        assert_eq!(calm.tempo, 96);
        assert_eq!(
            calm.melody,
            [first, turn, first, turn, first, turn, first, turn]
        );
        assert_eq!(piece.passage("chorus"), None);
    }

    #[test]
    fn a_fault_says_its_line_and_what_is_wrong() {
        // The heading and the lines that a file has once.
        assert_eq!(fault("tempo 96\n"), (1, HEADING));
        assert_eq!(fault("# No more.\n"), (1, HEADING));
        assert_eq!(fault("\ntanku-music 2\n"), (2, HEADING));
        assert_eq!(fault("tanku-music 1\n").1, "the file has no `tempo` line");
        assert_eq!(
            fault("tanku-music 1\ntempo 120\n").1,
            "the file has no `key` line"
        );
        assert_eq!(
            fault_after("tempo 100\n"),
            (6, "the tempo is written twice")
        );
        assert_eq!(
            fault_after("key A dorian aeolian\n").1,
            "the key is written twice"
        );
        assert_eq!(fault_after("chord 1\n"), (6, KINDS));
        for tempo in ["20", "301", "fast", "", "1e2"] {
            assert_eq!(fault(&SMALL.replace("96", tempo)), (2, TEMPO), "{tempo}");
        }
        for modes in ["ionian", "ionian blues", "Ionian aeolian"] {
            let text = SMALL.replace("ionian mixolydian", modes);
            assert_eq!(fault(&text), (3, MODES), "{modes}");
        }
        for roll in ["roll 5 kick snare 20 200", "roll 1 bass snare 20 200"] {
            let text = SMALL.replace("roll 1 kick snare 20 200", roll);
            assert_eq!(fault(&text), (4, ROLL), "{roll}");
        }
        assert_eq!(fault(&SMALL.replace("20 200", "20 256")), (4, ROLL));
        // A connection has two notes per beat at most.
        for link in ["link 0 2 1", "link 3 0 6", "link 3 2 7", "link 3 2"] {
            assert_eq!(
                fault(&SMALL.replace("link 3 2 6", link)),
                (5, LINK),
                "{link}"
            );
        }
        let extra = "this line has a word too many";
        assert_eq!(
            fault(&SMALL.replace("link 3 2 6", "link 3 2 6 1")),
            (5, extra)
        );

        // Motifs.
        assert_eq!(fault_after("motif a 1... .... .... ....\n"), (6, LETTER));
        assert_eq!(fault_after("motif AB 1... .... .... ....\n"), (6, LETTER));
        assert_eq!(fault_after("motif A 1... .... ....\n"), (6, BAR));
        assert_eq!(fault_after("motif A 1... ... .... ....\n"), (6, BAR));
        assert_eq!(fault_after("motif A 1... ..... .... ....\n"), (6, BAR));
        assert_eq!(fault_after("motif A 1... .x.. .... ....\n"), (6, MELODY));
        assert_eq!(fault_after("motif A 1... .0.. .... ....\n"), (6, MELODY));
        let apart = "the bars of a motif are apart by `|`";
        let two = "motif A 1... .... .... .... 2... .... .... ....\n";
        assert_eq!(fault_after(two), (6, apart));
        assert_eq!(fault_after("motif A 1... .... .... .... |\n"), (6, BAR));
        let twice = "this motif is written twice";
        assert_eq!(fault_after(&format!("{MOTIFS}{MOTIFS}")), (9, twice));
        let before = "a turn comes after the motif that it ends";
        assert_eq!(fault_after("turn A 1... .... .... ....\n"), (6, before));
        let turned = format!("{MOTIFS}turn B 1... .... .... ....\n");
        assert_eq!(fault_after(&turned), (9, "this turn is written twice"));

        // Patterns.
        let pattern = |words: &str| fault_after(&format!("pat {words}\n"));
        assert_eq!(pattern("kick heart x.R. .... .... .... 100"), (6, DRUM));
        assert_eq!(pattern("hat heart x.-. .... .... .... 100"), (6, DRUM));
        assert_eq!(pattern("bass low R.T. .... .... .... 100"), (6, PITCHED));
        assert_eq!(pattern("bass low R.0. .... .... .... 100"), (6, PITCHED));
        assert_eq!(pattern("kick heart x... .... .... ...."), (6, LEVEL));
        assert_eq!(pattern("kick heart x... .... .... .... 0"), (6, LEVEL));
        assert_eq!(pattern("kick heart x... .... .... .... 256"), (6, LEVEL));
        assert_eq!(pattern("kick heart x.o. .... .... .... 9 loud"), (6, LEVEL));
        assert_eq!(pattern("kick heart x... .... .... 100"), (6, BAR));
        assert_eq!(
            pattern("kick aheartthatbeats x... .... .... .... 100"),
            (6, NAME)
        );
        assert_eq!(
            pattern("lead solo x... .... .... .... 100"),
            (6, INSTRUMENT)
        );
        let soft = pattern("kick heart x.o. .... .... .... 100").1;
        assert!(soft.starts_with("a pattern with soft hits"), "{soft}");
        let hard = pattern("kick heart x... .... .... .... 100 50").1;
        assert!(hard.starts_with("this pattern has no soft hit"), "{hard}");
        let twice = "kick heart x... .... .... .... 100\npat kick heart x... x... .... .... 9";
        assert_eq!(pattern(twice), (7, "this pattern is written twice"));

        // Sections.
        let section = |words: &str| fault_after(&format!("{MOTIFS}sec {words}\n"));
        assert_eq!(section("verse 1111 111 A"), (9, CHORDS));
        assert_eq!(section("verse 1111 1181 A"), (9, CHORDS));
        assert_eq!(section("verse 1111 A"), (9, CHORDS));
        assert_eq!(section("verse 1111 1111 A A A"), (9, FILLS));
        assert_eq!(section("verse 1111 1111 B B B 100"), (9, FILLS));
        // Seven bars, and a motif of two.
        assert_eq!(section("verse 1111 1111 B B B A B"), (9, FILLS));
        assert_eq!(section("verse 1111 1111 B B B B A"), (9, AFTER));
        assert_eq!(section("verse 1111 1111 B B B B 20"), (9, AFTER));
        assert_eq!(section("verse 1111 1111 B B B B 100 90"), (9, extra));
        for word in ["A:0", "A:10", "b:5", "A-5", "AB", "?"] {
            assert_eq!(section(&format!("verse 1111 1111 {word}")), (9, STATEMENT));
        }
        let unknown = "a section comes after the motifs that it plays";
        assert_eq!(section("verse 1111 1111 C"), (9, unknown));
        assert_eq!(section("verse 1111 1111 a").1, "this motif has no turn");
        assert_eq!(section("averyverylongname 1111 1111 A"), (9, NAME));
        let twice = "verse 1111 1111 B B B B\nsec verse 1111 1111 B B B B";
        assert_eq!(section(twice), (10, "this section is written twice"));
    }

    #[test]
    fn a_piece_has_room_for_so_much() {
        let lines =
            |count: usize, line: fn(usize) -> String| -> String { (0..count).map(line).collect() };
        let motif = |index: usize| {
            let letter = (b'A' + index as u8) as char;
            format!("motif {letter} 1... .... .... ....\n")
        };
        assert!(read(&format!("{SMALL}{}", lines(8, motif))).is_ok());
        let full = fault_after(&lines(9, motif));
        assert_eq!(full, (14, "a piece has eight motifs at most"));
        let long = "motif A 1... .... .... ....".to_string() + &" | 1... .... .... ....".repeat(4);
        assert_eq!(fault_after(&long).1, "a motif has four bars at most");

        let pattern = |index: usize| format!("pat kick k{index} x... .... .... .... 9\n");
        assert!(read(&format!("{SMALL}{}", lines(16, pattern))).is_ok());
        let full = fault_after(&lines(17, pattern));
        assert_eq!(full, (22, "a piece has sixteen patterns at most"));

        let section = |index: usize| format!("sec s{index} 1111 1111 A A A A A A A A\n");
        let sections = |count: usize| format!("{SMALL}{}{}", motif(0), lines(count, section));
        assert!(read(&sections(8)).is_ok());
        assert_eq!(
            fault(&sections(9)),
            (15, "a piece has eight sections at most")
        );
    }

    #[test]
    fn a_fault_reads_well() {
        let fault = Fault {
            line: 12,
            what: LEVEL,
        };
        assert_eq!(
            fault.to_string(),
            "line 12: a level is a number from 1 to 255"
        );
    }

    #[test]
    #[should_panic(expected = "music.score, line 104: a level is a number from 1 to 255")]
    fn a_fault_stops_the_build_with_its_line() {
        let fault = Fault {
            line: 104,
            what: LEVEL,
        };
        fault.stop();
    }
}
