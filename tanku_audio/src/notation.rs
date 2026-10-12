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
//!
//! What plays when is written too: a `play` line for each scene that the arrangement tells
//! apart, and a `threat` line for each threat. In them an instrument has its line, as in
//! `bass=long@2` or `lead=answer@4:150`: a pattern, or a form of the melody of the section,
//! then the octave of its tonic after an `@`, and the level of a melody after a `:`.
//!
//! How the instruments sound is written last: a `mix` line for each, with its level on the
//! left and on the right, and a `sound` line for each, with the settings of its sound. A
//! number of these lines is written right before its unit, as in `45ms`, `5.5Hz` or `33%`.

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
pub(crate) const PASSAGES: usize = 8;
/// Sections that a scene can play in turn.
const TURNS: usize = 4;
/// Threats: the brain tiers of the enemies, from 0 to 4.
pub const THREATS: usize = 5;
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
        let mut text = [0; 400];
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

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.letters()).unwrap_or_default()
    }

    pub const fn is(&self, name: &str) -> bool {
        same(self.letters(), name.as_bytes())
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

/// The instruments of the piece. The lead has no pattern: it plays the melodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instrument {
    Kick,
    Hat,
    Snare,
    Bass,
    Pluck,
    Lead,
}

impl Instrument {
    /// The six instruments, in the order in which a line of the arrangement keeps them.
    pub const ALL: [Self; 6] = [
        Self::Kick,
        Self::Hat,
        Self::Snare,
        Self::Bass,
        Self::Pluck,
        Self::Lead,
    ];

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
        } else if same(name, b"lead") {
            Some(Self::Lead)
        } else {
            None
        }
    }

    /// Whether the instrument is a drum, which has no pitch.
    pub const fn is_drum(self) -> bool {
        matches!(self, Self::Kick | Self::Hat | Self::Snare)
    }
}

/// What the game is doing, as far as the arrangement tells it apart. Each scene has a `play`
/// line in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// No run is on, or the run is in its start bay.
    Menu,
    /// Nobody fights, in a corridor or in an arena.
    Calm,
    /// A fight in a corridor.
    Corridor,
    /// A fight in an arena.
    Arena,
}

impl Scene {
    pub const ALL: [Self; 4] = [Self::Menu, Self::Calm, Self::Corridor, Self::Arena];

    /// The name of the scene in the file.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::Calm => "calm",
            Self::Corridor => "corridor",
            Self::Arena => "arena",
        }
    }

    const fn named(name: &[u8]) -> Option<Self> {
        let mut index = 0;
        while index < Self::ALL.len() {
            if same(name, Self::ALL[index].name().as_bytes()) {
                return Some(Self::ALL[index]);
            }
            index += 1;
        }
        None
    }

    /// Whether the scene is a fight.
    pub const fn is_fight(self) -> bool {
        matches!(self, Self::Corridor | Self::Arena)
    }
}

/// The melody of a section, or a line that the rules derive from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// The melody as it is written.
    Melody,
    /// Its notes that start on a beat, each held until the next.
    Skeleton,
    /// The melody with its ornaments.
    Ornate,
    /// The answer to each of its bars, which is a call.
    Answer,
}

impl Form {
    pub const ALL: [Self; 4] = [Self::Melody, Self::Skeleton, Self::Ornate, Self::Answer];

    /// The name of the form in the file.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Melody => "melody",
            Self::Skeleton => "skeleton",
            Self::Ornate => "ornate",
            Self::Answer => "answer",
        }
    }

    const fn named(name: &[u8]) -> Option<Self> {
        let mut index = 0;
        while index < Self::ALL.len() {
            if same(name, Self::ALL[index].name().as_bytes()) {
                return Some(Self::ALL[index]);
            }
            index += 1;
        }
        None
    }
}

/// What a line of an instrument is made from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The melody of the section that plays, or a line derived from it. Degree 1 is the tonic
    /// in an octave, from 1 to 5, and every note has the same level.
    Tune { form: Form, octave: u8, level: u8 },
    /// A pattern, by its place among those of the piece. On an instrument that has a pitch,
    /// it has an octave too.
    Rhythm { index: u8, octave: u8 },
}

/// What an instrument plays: one line, or two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layer {
    pub first: Source,
    /// A second line. It sounds where the first starts nothing, or together with the first,
    /// which only the lead can do.
    pub second: Option<Source>,
    pub together: bool,
}

/// What an instrument has in a line of the arrangement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Silent,
    /// What the line of the threat says.
    ByThreat,
    Plays(Layer),
}

/// A line of the arrangement: the sections that a scene plays in turn, and what each
/// instrument plays in them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Play {
    sections: [u8; TURNS],
    count: u8,
    parts: [Part; 6],
}

impl Play {
    const EMPTY: Self = Self {
        sections: [0; TURNS],
        count: 0,
        parts: [Part::Silent; 6],
    };

    /// The sections, each by its place among those of the piece.
    pub const fn sections(&self) -> &[u8] {
        self.sections.split_at(self.count as usize).0
    }

    pub const fn part(&self, instrument: Instrument) -> Part {
        self.parts[instrument as usize]
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
    /// Whether its notes have the echo of the pluck.
    pub echo: bool,
}

impl Rhythm {
    const EMPTY: Self = Self {
        instrument: Instrument::Kick,
        name: Word::EMPTY,
        grid: [b'.'; STEPS],
        level: 0,
        soft: 0,
        echo: false,
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

/// How the kick sounds: a sine that falls fast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KickSound {
    /// The frequency that its pitch starts at and the one that it goes to, in Hz.
    pub from: f32,
    pub to: f32,
    /// Seconds in which the pitch covers about two thirds of what is left of the way.
    pub sweep: f32,
    /// Seconds in which the level loses about two thirds of what is left of it.
    pub decay: f32,
}

/// How the hi-hat sounds: a short burst of the top of white noise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HatSound {
    /// The frequency above which the noise is kept, in Hz.
    pub above: f32,
    /// Seconds in which the level loses about two thirds of what is left of it.
    pub decay: f32,
}

/// How the snare sounds: a burst of the middle of white noise over a short tone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnareSound {
    /// The frequency that the noise is around, and how wide that band is, in Hz.
    pub center: f32,
    pub width: f32,
    /// Seconds in which the noise loses about two thirds of what is left of it.
    pub decay: f32,
    /// The frequency of the tone in Hz, the seconds of its own decay, and how loud it is
    /// against the noise.
    pub tone: f32,
    pub tone_decay: f32,
    pub tone_level: f32,
}

/// How the bass sounds: a square wave through a filter that moves at each attack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BassSound {
    /// Seconds that a note takes to rise.
    pub rise: f32,
    /// The filter lets through what is below a frequency, which starts here at each attack
    /// and goes there, in Hz.
    pub from: f32,
    pub to: f32,
    /// Seconds in which the filter covers about two thirds of what is left of the way.
    pub sweep: f32,
}

/// How the pluck sounds: a plucked string, and its echo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PluckSound {
    /// Seconds that a string takes to fall by 60 dB.
    pub ring: f32,
    /// Times that the burst of noise that plucks a string is smoothed, which takes the edge
    /// off its attack.
    pub smooth: u32,
    /// Steps that the echo comes after, at the tempo of the piece, and how loud it is against
    /// the note.
    pub echo_steps: u32,
    pub echo_level: f32,
}

/// How the lead sounds: a pulse wave with a slow vibrato.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeadSound {
    /// The part of its period that the wave spends up.
    pub width: f32,
    /// Seconds that a note takes to rise.
    pub rise: f32,
    /// The frequency below which the lead is heard, in Hz.
    pub below: f32,
    /// Seconds after which the vibrato comes in: it is whole after as long again.
    pub vibrato_after: f32,
    /// How far the vibrato bends the pitch, as a part of it, and how many times per second.
    pub vibrato_depth: f32,
    pub vibrato_rate: f32,
}

/// How each instrument sounds: the settings of its recipe, which is in the synthesizer. The
/// file writes them in ms, in Hz and in percent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sounds {
    pub kick: KickSound,
    pub hat: HatSound,
    pub snare: SnareSound,
    pub bass: BassSound,
    pub pluck: PluckSound,
    pub lead: LeadSound,
}

impl Sounds {
    /// Before the file says anything.
    const UNSET: Self = Self {
        kick: KickSound {
            from: 0.0,
            to: 0.0,
            sweep: 0.0,
            decay: 0.0,
        },
        hat: HatSound {
            above: 0.0,
            decay: 0.0,
        },
        snare: SnareSound {
            center: 0.0,
            width: 0.0,
            decay: 0.0,
            tone: 0.0,
            tone_decay: 0.0,
            tone_level: 0.0,
        },
        bass: BassSound {
            rise: 0.0,
            from: 0.0,
            to: 0.0,
            sweep: 0.0,
        },
        pluck: PluckSound {
            ring: 0.0,
            smooth: 0,
            echo_steps: 0,
            echo_level: 0.0,
        },
        lead: LeadSound {
            width: 0.0,
            rise: 0.0,
            below: 0.0,
            vibrato_after: 0.0,
            vibrato_depth: 0.0,
            vibrato_rate: 0.0,
        },
    };
}

/// The piece as the file writes it.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// The line of the arrangement of each scene.
    plays: [Play; 4],
    /// What each instrument plays at each threat, where a scene leaves it to the threat.
    threats: [[Option<Layer>; 6]; THREATS],
    /// The line of the file that each threat is on, or 0.
    threat_lines: [u32; THREATS],
    pub roll: Roll,
    pub link: Link,
    /// How loud each instrument is on the left and on the right, where 1 is as loud as a
    /// sound can be, and whether the file says so.
    mix: [[f32; 2]; 6],
    mixed: [bool; 6],
    /// How each instrument sounds, and whether the file says so.
    pub sounds: Sounds,
    sounded: [bool; 6],
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
        plays: [Play::EMPTY; 4],
        threats: [[None; 6]; THREATS],
        threat_lines: [0; THREATS],
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
        mix: [[0.0; 2]; 6],
        mixed: [false; 6],
        sounds: Sounds::UNSET,
        sounded: [false; 6],
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

    /// Where the pattern of an instrument is among the patterns, by its name.
    const fn rhythm_at(&self, instrument: Instrument, name: &[u8]) -> Option<usize> {
        let mut index = 0;
        while index < self.rhythm_count {
            let rhythm = &self.rhythms[index];
            if rhythm.instrument as u8 == instrument as u8 && same(rhythm.name.letters(), name) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    /// The pattern of an instrument by its name.
    pub const fn rhythm(&self, instrument: Instrument, name: &str) -> Option<Rhythm> {
        match self.rhythm_at(instrument, name.as_bytes()) {
            Some(index) => Some(self.rhythms[index]),
            None => None,
        }
    }

    /// The pattern at a place among the patterns.
    pub const fn pattern(&self, index: u8) -> &Rhythm {
        &self.rhythms[index as usize]
    }

    /// Where a section is among the sections, by its name.
    const fn passage_at(&self, name: &[u8]) -> Option<usize> {
        let mut index = 0;
        while index < self.passage_count {
            if same(self.passages[index].name.letters(), name) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    /// The section of a name.
    pub const fn passage(&self, name: &str) -> Option<Passage> {
        match self.passage_at(name.as_bytes()) {
            Some(index) => Some(self.passages[index]),
            None => None,
        }
    }

    /// The section at a place among the sections.
    pub const fn section(&self, index: u8) -> &Passage {
        &self.passages[index as usize]
    }

    /// The line of the arrangement of a scene.
    pub const fn play(&self, scene: Scene) -> &Play {
        &self.plays[scene as usize]
    }

    /// What an instrument plays at a threat, where a scene leaves it to the threat.
    pub const fn threat(&self, threat: u8, instrument: Instrument) -> Option<Layer> {
        self.threats[threat as usize][instrument as usize]
    }

    /// How loud an instrument is on the left and on the right, where 1 is as loud as a sound
    /// can be.
    pub const fn mix(&self, instrument: Instrument) -> [f32; 2] {
        self.mix[instrument as usize]
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

/// A number of the file that can have a point, such as `5.5` or `0.4`: its digits as one whole
/// number, and how many of them come after the point.
#[derive(Clone, Copy)]
struct Decimal {
    digits: u32,
    places: u32,
}

impl Decimal {
    /// The number over a power of ten: over a hundred for a number of percent. It is one
    /// division of two whole numbers, so it is the nearest value to what the file writes.
    const fn over(self, tens: u32) -> f32 {
        let mut scale = 1u32;
        let mut power = 0;
        while power < self.places + tens {
            scale *= 10;
            power += 1;
        }
        self.digits as f32 / scale as f32
    }
}

/// A measure: a number right before its unit, as in `136ms`, `5.5Hz` or `30%`. It has seven
/// digits at most, of which four after its point.
const fn measure(word: &[u8], unit: &[u8]) -> Option<Decimal> {
    if word.len() <= unit.len() {
        return None;
    }
    let (number, ending) = word.split_at(word.len() - unit.len());
    if !same(ending, unit) {
        return None;
    }
    let (mut digits, mut count, mut places) = (0, 0, 0);
    let mut point = false;
    let mut index = 0;
    while index < number.len() {
        let byte = number[index];
        if byte == b'.' {
            // One point, between two digits.
            if point || index == 0 || index + 1 == number.len() {
                return None;
            }
            point = true;
        } else if byte.is_ascii_digit() && count < 7 {
            digits = 10 * digits + (byte - b'0') as u32;
            count += 1;
            places += point as u32;
        } else {
            return None;
        }
        index += 1;
    }
    if places > 4 {
        return None;
    }
    Some(Decimal { digits, places })
}

/// The bound of a measure counts ten-thousandths of its unit, which is as fine as a measure is
/// written.
const WHOLE: u64 = 10_000;

/// The next word as a measure in a unit, if it is one between two bounds.
const fn measured(words: &mut Words, unit: &[u8], least: u64, most: u64) -> Option<Decimal> {
    let Some(word) = words.next() else {
        return None;
    };
    let Some(found) = measure(word, unit) else {
        return None;
    };
    let mut fine = found.digits as u64;
    let mut places = found.places;
    while places < 4 {
        fine *= 10;
        places += 1;
    }
    if fine >= least && fine <= most {
        Some(found)
    } else {
        None
    }
}

/// The next word as a share of the whole, from its number of percent, between two bounds.
const fn share(words: &mut Words, least: u64, most: u64) -> Option<f32> {
    match measured(words, b"%", least, most) {
        Some(share) => Some(share.over(2)),
        None => None,
    }
}

/// The next word as a share of the whole, from its number of percent, up to 100%.
const fn percent(words: &mut Words) -> Option<f32> {
    share(words, 0, 100 * WHOLE)
}

/// The next word as a time in seconds, from its number of milliseconds: a tenth of one at
/// least, which is four frames, and ten seconds at most.
const fn time(words: &mut Words) -> Result<f32, &'static str> {
    match measured(words, b"ms", WHOLE / 10, 10_000 * WHOLE) {
        Some(time) => Ok(time.over(3)),
        None => Err(TIME),
    }
}

/// The next word as a frequency in Hz, among those that are heard.
const fn frequency(words: &mut Words) -> Result<f32, &'static str> {
    match measured(words, b"Hz", 20 * WHOLE, 20_000 * WHOLE) {
        Some(frequency) => Ok(frequency.over(0)),
        None => Err(FREQUENCY),
    }
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
const KINDS: &str = "a line starts with `tempo`, `key`, `motif`, `turn`, `pat`, `sec`, `play`, \
                     `threat`, `roll`, `link`, `mix` or `sound`";
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
const INSTRUMENT: &str = "a pattern is for the `kick`, the `hat`, the `snare`, the `bass` or \
                          the `pluck`: the lead plays the melodies";
const TAKEN: &str = "`melody`, `skeleton`, `ornate` and `answer` are the lines of a section: a \
                     pattern has another name";
const ENDING: &str = "after its levels, a pattern can only have `echo`, which the pluck has";
const SCENE: &str = "a `play` line is for the `menu`, the `calm`, a fight in a `corridor` or a \
                     fight in an `arena`";
const TURN_BY_TURN: &str = "after its scene, a `play` line has the sections that play in turn, \
                            which are written above it, with a comma between two: `verse,chorus`";
const PART: &str = "what an instrument plays is written as in `bass=long@2`: the `kick`, the \
                    `hat`, the `snare`, the `bass`, the `pluck` or the `lead`, then `=`, then \
                    its line";
const BEAT_OF_DRUM: &str = "a drum plays one of its patterns, by its name alone";
const OCTAVE: &str = "a line that has a pitch has its octave, from 1 to 5, after an `@`, as in \
                      `melody@4`";
const LINE: &str = "a line is `melody`, `skeleton`, `ornate` or `answer`, or a pattern of that \
                    instrument, which is written above";
const LOUD: &str = "after a `:`, a melody has its level, from 1 to 255, and a pattern has the \
                    levels of its own line";
const TOGETHER: &str = "only the lead plays two lines at once, with a `+` between them";
const THREAT: &str = "a `threat` line has a threat, from 0 to 4, then what the instruments play \
                      at that threat";
const STAR: &str = "`*` leaves an instrument to the threat, in a `play` line";
const LEFT: &str = "an instrument is left to the threat, and a threat from 0 to 4 has no \
                    `threat` line";
const NOT_LEFT: &str = "no `play` line leaves this instrument to the threat, with `*`";
const MIX: &str = "a `mix` line has an instrument, then how loud it is on the left and on the \
                   right, each from 0% to 100%";
const SOUND: &str = "a `sound` line has an instrument, then each of its settings by its name";
const TIME: &str = "a time is a number of ms, from 0.1ms to 10000ms, as in `45ms`";
const FREQUENCY: &str = "a frequency is a number of Hz, from 20Hz to 20000Hz, as in `3000Hz`";
const KICK: &str = "the kick has `pitch`, with the frequency that it starts at, the one that it \
                    goes to and the time of the way, and `decay`, with the time that it dies \
                    away in, each once";
const HAT: &str = "the hat has `above`, with the frequency above which its noise is kept, and \
                   `decay`, with the time that it dies away in, each once";
const SNARE: &str = "the snare has `band`, with the frequency that its noise is around and the \
                     width of that band, `decay`, with the time that the noise dies away in, and \
                     `tone`, with a frequency, the time that it dies away in and its level, from \
                     0% to 100%, each once";
const BAND: &str = "the noise of the snare is around a frequency of 5000Hz at most, and its \
                    band is as wide as a tenth of that frequency at least and as twice that \
                    frequency at most";
const BASS: &str = "the bass has `rise`, with the time that a note takes to rise, and `filter`, \
                    with the frequency that its filter starts at, the one that it goes to and \
                    the time of the way, each once";
const PLUCK: &str = "the pluck has `ring`, with the time that a string rings, `smooth`, with how \
                     many times its attack is smoothed, and `echo`, with the steps that its echo \
                     comes after and its level, each once";
const RING: &str = "a string of the pluck rings for 100ms to 10000ms, as in `1000ms`";
const SMOOTH: &str = "the attack of the pluck is smoothed 0 to 8 times";
const ECHO: &str = "the echo of the pluck comes 1 to 8 steps after the note, and is from 0% to \
                    100% as loud, as in `echo 3 33%`";
const LEAD: &str = "the lead has `width`, with the part of its period that its wave is up for, \
                    `rise`, with the time that a note takes to rise, `below`, with the frequency \
                    below which it is heard, and `vibrato`, with the time that it comes in \
                    after, how far it bends the pitch and how many times per second, each once";
const WIDTH: &str = "the wave of the lead is up for 1% to 99% of its period, as in `30%`";
const DEPTH: &str = "the vibrato of the lead bends the pitch by 0% to 10%, as in `0.4%`";
const RATE: &str = "the vibrato of the lead comes 0.1 to 20 times per second, as in `5.5Hz`";
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
        if let Err(what) = record(&mut piece, &mut found, kind, &mut words, line) {
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
    if let Err(fault) = arrangement(&piece, line) {
        return Err(fault);
    }
    let mut instrument = 0;
    while instrument < Instrument::ALL.len() {
        if !piece.mixed[instrument] {
            let what = match Instrument::ALL[instrument] {
                Instrument::Kick => "the file has no `mix kick` line",
                Instrument::Hat => "the file has no `mix hat` line",
                Instrument::Snare => "the file has no `mix snare` line",
                Instrument::Bass => "the file has no `mix bass` line",
                Instrument::Pluck => "the file has no `mix pluck` line",
                Instrument::Lead => "the file has no `mix lead` line",
            };
            return Err(Fault { line, what });
        }
        if !piece.sounded[instrument] {
            let what = match Instrument::ALL[instrument] {
                Instrument::Kick => "the file has no `sound kick` line",
                Instrument::Hat => "the file has no `sound hat` line",
                Instrument::Snare => "the file has no `sound snare` line",
                Instrument::Bass => "the file has no `sound bass` line",
                Instrument::Pluck => "the file has no `sound pluck` line",
                Instrument::Lead => "the file has no `sound lead` line",
            };
            return Err(Fault { line, what });
        }
        instrument += 1;
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

/// Checks the arrangement of a piece once its file is read: every scene has its line, and the
/// threat has what the scenes leave to it. `last` is the last line of the file.
const fn arrangement(piece: &Piece, last: u32) -> Result<(), Fault> {
    let mut scene = 0;
    while scene < Scene::ALL.len() {
        if piece.plays[scene].count == 0 {
            let what = match Scene::ALL[scene] {
                Scene::Menu => "the file has no `play menu` line",
                Scene::Calm => "the file has no `play calm` line",
                Scene::Corridor => "the file has no `play corridor` line",
                Scene::Arena => "the file has no `play arena` line",
            };
            return Err(Fault { line: last, what });
        }
        scene += 1;
    }
    let mut instrument = 0;
    while instrument < Instrument::ALL.len() {
        let (mut left, mut scene) = (false, 0);
        while scene < Scene::ALL.len() {
            left |= matches!(piece.plays[scene].parts[instrument], Part::ByThreat);
            scene += 1;
        }
        let mut threat = 0;
        while threat < THREATS {
            let line = piece.threat_lines[threat];
            if left && line == 0 {
                return Err(Fault {
                    line: last,
                    what: LEFT,
                });
            }
            if !left && piece.threats[threat][instrument].is_some() {
                let what = NOT_LEFT;
                return Err(Fault { line, what });
            }
            threat += 1;
        }
        instrument += 1;
    }
    Ok(())
}

/// Reads one line into the piece, from the word that says what the line is.
const fn record(
    piece: &mut Piece,
    found: &mut Found,
    kind: &[u8],
    words: &mut Words,
    line: u32,
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
    } else if same(kind, b"play") {
        tri!(play(piece, words));
    } else if same(kind, b"threat") {
        tri!(threat(piece, words, line));
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
    } else if same(kind, b"mix") {
        tri!(mix(piece, words));
    } else if same(kind, b"sound") {
        tri!(sound(piece, words));
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
    if matches!(instrument, Instrument::Lead) {
        return Err(INSTRUMENT);
    }
    let name = need!(name(words), NAME);
    if Form::named(name.letters()).is_some() {
        return Err(TAKEN);
    }
    if piece.rhythm_at(instrument, name.letters()).is_some() {
        return Err("this pattern is written twice");
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
    // Then the level of a soft hit, if it has some, and the echo, if it has one.
    let (mut soft, mut echo) = (0, false);
    while let Some(word) = words.next() {
        if echo {
            return Err(ENDING);
        } else if same(word, b"echo") {
            if !matches!(instrument, Instrument::Pluck) {
                return Err(ENDING);
            }
            echo = true;
        } else if soft == 0 {
            soft = match number(word) {
                Some(soft) if soft >= 1 && soft <= 255 => soft,
                _ => return Err(LEVEL),
            };
        } else {
            return Err(ENDING);
        }
    }
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
        echo,
    };
    piece.rhythm_count += 1;
    Ok(())
}

const fn passage(piece: &mut Piece, words: &mut Words) -> Result<(), &'static str> {
    let name = need!(name(words), NAME);
    if piece.passage_at(name.letters()).is_some() {
        return Err("this section is written twice");
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

/// Where a byte first is in a word.
const fn find(word: &[u8], byte: u8) -> Option<usize> {
    let mut index = 0;
    while index < word.len() {
        if word[index] == byte {
            return Some(index);
        }
        index += 1;
    }
    None
}

/// A word in two, around the first of a byte that it has.
const fn halves(word: &[u8], byte: u8) -> Option<(&[u8], &[u8])> {
    match find(word, byte) {
        Some(at) => Some((word.split_at(at).0, word.split_at(at + 1).1)),
        None => None,
    }
}

const fn play(piece: &mut Piece, words: &mut Words) -> Result<(), &'static str> {
    let scene = match words.next() {
        Some(name) => Scene::named(name),
        None => None,
    };
    let scene = need!(scene, SCENE) as usize;
    if piece.plays[scene].count > 0 {
        return Err("this scene is written twice");
    }
    let mut play = Play::EMPTY;
    let mut rest = need!(words.next(), TURN_BY_TURN);
    loop {
        let (name, more) = match halves(rest, b',') {
            Some((name, more)) => (name, Some(more)),
            None => (rest, None),
        };
        if play.count as usize == TURNS {
            return Err("a scene plays four sections at most");
        }
        play.sections[play.count as usize] = need!(piece.passage_at(name), TURN_BY_TURN) as u8;
        play.count += 1;
        match more {
            Some(more) => rest = more,
            None => break,
        }
    }
    tri!(parts(piece, words, &mut play.parts, true));
    piece.plays[scene] = play;
    Ok(())
}

const fn threat(piece: &mut Piece, words: &mut Words, line: u32) -> Result<(), &'static str> {
    let threat = need!(amount(words, 0, THREATS as u32 - 1), THREAT) as usize;
    if piece.threat_lines[threat] != 0 {
        return Err("this threat is written twice");
    }
    let mut played = [Part::Silent; 6];
    tri!(parts(piece, words, &mut played, false));
    let mut instrument = 0;
    while instrument < played.len() {
        if let Part::Plays(layer) = played[instrument] {
            piece.threats[threat][instrument] = Some(layer);
        }
        instrument += 1;
    }
    piece.threat_lines[threat] = line;
    Ok(())
}

/// Reads what the instruments play, to the end of a line. Only a scene can leave an
/// instrument to the threat.
const fn parts(
    piece: &Piece,
    words: &mut Words,
    parts: &mut [Part; 6],
    of_a_scene: bool,
) -> Result<(), &'static str> {
    while let Some(word) = words.next() {
        let (name, line) = need!(halves(word, b'='), PART);
        let instrument = need!(Instrument::named(name), PART);
        if !matches!(parts[instrument as usize], Part::Silent) {
            return Err("this instrument is written twice");
        }
        parts[instrument as usize] = if same(line, b"*") {
            if !of_a_scene {
                return Err(STAR);
            }
            Part::ByThreat
        } else {
            Part::Plays(tri!(layer(piece, instrument, line)))
        };
    }
    Ok(())
}

/// What an instrument plays: a line, a line with another where it starts nothing, after a
/// `/`, or two lines together, around a `+`.
const fn layer(piece: &Piece, instrument: Instrument, text: &[u8]) -> Result<Layer, &'static str> {
    let (first, second, together) = match halves(text, b'/') {
        Some((first, second)) => (first, Some(second), false),
        None => match halves(text, b'+') {
            Some((first, second)) => (first, Some(second), true),
            None => (text, None, false),
        },
    };
    if instrument.is_drum() && second.is_some() {
        return Err(BEAT_OF_DRUM);
    }
    if together && !matches!(instrument, Instrument::Lead) {
        return Err(TOGETHER);
    }
    let first = tri!(source(piece, instrument, first));
    let second = match second {
        Some(second) => Some(tri!(source(piece, instrument, second))),
        None => None,
    };
    Ok(Layer {
        first,
        second,
        together,
    })
}

/// One line of an instrument: a name, an octave after an `@` if the instrument has a pitch,
/// and a level after a `:` if the line is a melody.
const fn source(
    piece: &Piece,
    instrument: Instrument,
    text: &[u8],
) -> Result<Source, &'static str> {
    let (text, level) = match halves(text, b':') {
        Some((text, level)) => (text, Some(level)),
        None => (text, None),
    };
    let (name, octave) = match halves(text, b'@') {
        Some((name, octave)) => (name, Some(octave)),
        None => (text, None),
    };
    if instrument.is_drum() {
        if octave.is_some() || level.is_some() {
            return Err(BEAT_OF_DRUM);
        }
        let index = need!(piece.rhythm_at(instrument, name), LINE) as u8;
        return Ok(Source::Rhythm { index, octave: 0 });
    }
    let octave = match need!(octave, OCTAVE) {
        [digit @ b'1'..=b'5'] => *digit - b'0',
        _ => return Err(OCTAVE),
    };
    match Form::named(name) {
        Some(form) => {
            let level = match level {
                Some(level) => match number(level) {
                    Some(level) if level >= 1 && level <= 255 => level as u8,
                    _ => return Err(LOUD),
                },
                None => 255,
            };
            Ok(Source::Tune {
                form,
                octave,
                level,
            })
        }
        None => {
            if level.is_some() {
                return Err(LOUD);
            }
            let index = need!(piece.rhythm_at(instrument, name), LINE) as u8;
            Ok(Source::Rhythm { index, octave })
        }
    }
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

const fn mix(piece: &mut Piece, words: &mut Words) -> Result<(), &'static str> {
    let instrument = match words.next() {
        Some(name) => Instrument::named(name),
        None => None,
    };
    let instrument = need!(instrument, MIX) as usize;
    if piece.mixed[instrument] {
        return Err("this instrument is mixed twice");
    }
    piece.mix[instrument] = [need!(percent(words), MIX), need!(percent(words), MIX)];
    piece.mixed[instrument] = true;
    Ok(())
}

const fn sound(piece: &mut Piece, words: &mut Words) -> Result<(), &'static str> {
    let instrument = match words.next() {
        Some(name) => Instrument::named(name),
        None => None,
    };
    let instrument = need!(instrument, SOUND);
    if piece.sounded[instrument as usize] {
        return Err("the sound of this instrument is written twice");
    }
    match instrument {
        Instrument::Kick => piece.sounds.kick = tri!(kick(words)),
        Instrument::Hat => piece.sounds.hat = tri!(hat(words)),
        Instrument::Snare => piece.sounds.snare = tri!(snare(words)),
        Instrument::Bass => piece.sounds.bass = tri!(bass(words)),
        Instrument::Pluck => piece.sounds.pluck = tri!(pluck(words)),
        Instrument::Lead => piece.sounds.lead = tri!(lead(words)),
    }
    piece.sounded[instrument as usize] = true;
    Ok(())
}

const fn kick(words: &mut Words) -> Result<KickSound, &'static str> {
    let (mut pitch, mut decay) = (None, None);
    while let Some(setting) = words.next() {
        if same(setting, b"pitch") && pitch.is_none() {
            let (from, to) = (tri!(frequency(words)), tri!(frequency(words)));
            pitch = Some((from, to, tri!(time(words))));
        } else if same(setting, b"decay") && decay.is_none() {
            decay = Some(tri!(time(words)));
        } else {
            return Err(KICK);
        }
    }
    match (pitch, decay) {
        (Some((from, to, sweep)), Some(decay)) => Ok(KickSound {
            from,
            to,
            sweep,
            decay,
        }),
        _ => Err(KICK),
    }
}

const fn hat(words: &mut Words) -> Result<HatSound, &'static str> {
    let (mut above, mut decay) = (None, None);
    while let Some(setting) = words.next() {
        if same(setting, b"above") && above.is_none() {
            above = Some(tri!(frequency(words)));
        } else if same(setting, b"decay") && decay.is_none() {
            decay = Some(tri!(time(words)));
        } else {
            return Err(HAT);
        }
    }
    match (above, decay) {
        (Some(above), Some(decay)) => Ok(HatSound { above, decay }),
        _ => Err(HAT),
    }
}

const fn snare(words: &mut Words) -> Result<SnareSound, &'static str> {
    let (mut band, mut decay, mut tone) = (None, None, None);
    while let Some(setting) = words.next() {
        if same(setting, b"band") && band.is_none() {
            let (center, width) = (tri!(frequency(words)), tri!(frequency(words)));
            // The filter of the band is steady within these bounds, and not past them.
            if center > 5000.0 || width < center / 10.0 || width > 2.0 * center {
                return Err(BAND);
            }
            band = Some((center, width));
        } else if same(setting, b"decay") && decay.is_none() {
            decay = Some(tri!(time(words)));
        } else if same(setting, b"tone") && tone.is_none() {
            let (frequency, fall) = (tri!(frequency(words)), tri!(time(words)));
            tone = Some((frequency, fall, need!(percent(words), SNARE)));
        } else {
            return Err(SNARE);
        }
    }
    match (band, decay, tone) {
        (Some((center, width)), Some(decay), Some((tone, tone_decay, tone_level))) => {
            Ok(SnareSound {
                center,
                width,
                decay,
                tone,
                tone_decay,
                tone_level,
            })
        }
        _ => Err(SNARE),
    }
}

const fn bass(words: &mut Words) -> Result<BassSound, &'static str> {
    let (mut rise, mut filter) = (None, None);
    while let Some(setting) = words.next() {
        if same(setting, b"rise") && rise.is_none() {
            rise = Some(tri!(time(words)));
        } else if same(setting, b"filter") && filter.is_none() {
            let (from, to) = (tri!(frequency(words)), tri!(frequency(words)));
            filter = Some((from, to, tri!(time(words))));
        } else {
            return Err(BASS);
        }
    }
    match (rise, filter) {
        (Some(rise), Some((from, to, sweep))) => Ok(BassSound {
            rise,
            from,
            to,
            sweep,
        }),
        _ => Err(BASS),
    }
}

const fn pluck(words: &mut Words) -> Result<PluckSound, &'static str> {
    let (mut ring, mut smooth, mut echo) = (None, None, None);
    while let Some(setting) = words.next() {
        if same(setting, b"ring") && ring.is_none() {
            // A string that rings for less would lose more than itself at each turn.
            ring = match measured(words, b"ms", 100 * WHOLE, 10_000 * WHOLE) {
                Some(time) => Some(time.over(3)),
                None => return Err(RING),
            };
        } else if same(setting, b"smooth") && smooth.is_none() {
            smooth = Some(need!(amount(words, 0, 8), SMOOTH));
        } else if same(setting, b"echo") && echo.is_none() {
            let steps = need!(amount(words, 1, 8), ECHO);
            echo = Some((steps, need!(percent(words), ECHO)));
        } else {
            return Err(PLUCK);
        }
    }
    match (ring, smooth, echo) {
        (Some(ring), Some(smooth), Some((echo_steps, echo_level))) => Ok(PluckSound {
            ring,
            smooth,
            echo_steps,
            echo_level,
        }),
        _ => Err(PLUCK),
    }
}

const fn lead(words: &mut Words) -> Result<LeadSound, &'static str> {
    let (mut width, mut rise, mut below, mut vibrato) = (None, None, None, None);
    while let Some(setting) = words.next() {
        if same(setting, b"width") && width.is_none() {
            width = Some(need!(share(words, WHOLE, 99 * WHOLE), WIDTH));
        } else if same(setting, b"rise") && rise.is_none() {
            rise = Some(tri!(time(words)));
        } else if same(setting, b"below") && below.is_none() {
            below = Some(tri!(frequency(words)));
        } else if same(setting, b"vibrato") && vibrato.is_none() {
            let after = tri!(time(words));
            let depth = need!(share(words, 0, 10 * WHOLE), DEPTH);
            let rate = match measured(words, b"Hz", WHOLE / 10, 20 * WHOLE) {
                Some(rate) => rate.over(0),
                None => return Err(RATE),
            };
            vibrato = Some((after, depth, rate));
        } else {
            return Err(LEAD);
        }
    }
    match (width, rise, below, vibrato) {
        (Some(width), Some(rise), Some(below), Some((after, depth, rate))) => Ok(LeadSound {
            width,
            rise,
            below,
            vibrato_after: after,
            vibrato_depth: depth,
            vibrato_rate: rate,
        }),
        _ => Err(LEAD),
    }
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

    /// The lines that a file has once: its heading, a tempo, a key, a roll and a connection.
    const SMALL: &str = "tanku-music 1\n\
                         tempo 96\n\
                         key C# ionian mixolydian\n\
                         roll 1 kick snare 20 200\n\
                         link 3 2 6\n";
    /// The shortest arrangement: one section of one motif, which every scene plays in
    /// silence.
    const SILENCE: &str = "motif Z 1... .... .... ....\n\
                           sec rest 1111 1111 Z Z Z Z Z Z Z Z\n\
                           play menu rest\n\
                           play calm rest\n\
                           play corridor rest\n\
                           play arena rest\n";
    /// The mix of the six instruments.
    const LEVELS: &str = "mix kick 30% 30%\n\
                          mix hat 70% 40%\n\
                          mix snare 25% 25%\n\
                          mix bass 10% 10%\n\
                          mix pluck 20% 15%\n\
                          mix lead 5.5% 12.25%\n";
    /// How the six instruments sound.
    const SOUNDS: &str = "sound kick pitch 120Hz 50Hz 25ms decay 60ms\n\
                          sound hat above 8000Hz decay 12.5ms\n\
                          sound snare band 1800Hz 900Hz decay 40ms tone 180Hz 20ms 50%\n\
                          sound bass rise 2ms filter 2000Hz 250Hz 80ms\n\
                          sound pluck ring 750ms smooth 2 echo 4 25%\n\
                          sound lead width 25% rise 5ms below 4000Hz vibrato 200ms 0.5% 6Hz\n";
    /// A motif of one bar and a motif of two, with its turn.
    const MOTIFS: &str = "motif A 1... .... .... ....\n\
                          motif B 1..2 .... 3-.. .... | 5... .... .... 6...\n\
                          turn B 8... .... .... ....\n";
    /// Some patterns, two sections, and an arrangement of them.
    const PATTERNS: &str = "pat kick four x... x... x... x... 200\n\
                            pat hat off ..x. ..x. ..x. ..x. 90\n\
                            pat snare back .... x... .... x... 150\n\
                            pat bass roots R... .... R... .... 250\n\
                            pat bass walk 1.2. 3.5. 8.5. 3.2. 240\n\
                            pat pluck up x.x. x.x. x.x. x.x. 120 echo\n\
                            pat pluck drone 5... .... 5... .... 80\n";
    const SECTIONS: &str = "sec one 1111 1111 A A A A A A A A\n\
                            sec two 4455 4411 B B B b\n";
    const ARRANGEMENT: &str = "play menu one bass=roots@2 pluck=melody@4\n\
                               play calm two,one kick=four pluck=melody@3/drone@2 \
                               lead=answer@4:150\n\
                               play corridor one,two kick=four hat=* bass=* lead=*\n\
                               play arena two snare=back bass=walk@1 pluck=up@3 \
                               lead=ornate@4+skeleton@5:90\n\
                               threat 0 bass=roots@2\n\
                               threat 1 hat=off bass=roots@2 lead=skeleton@4\n\
                               threat 2 hat=off bass=walk@1 lead=melody@4\n\
                               threat 3 hat=off bass=walk@1 lead=ornate@4\n\
                               threat 4 hat=off bass=walk@1 lead=ornate@4:200\n";

    /// The line on which a file can't be read, and why.
    fn fault(text: &str) -> (u32, &'static str) {
        let fault = read(text).unwrap_err();
        (fault.line, fault.what)
    }

    /// The same for a file that is the small one and some more lines.
    fn fault_after(lines: &str) -> (u32, &'static str) {
        fault(&format!("{SMALL}{lines}"))
    }

    /// A file that reads: the small one, some more lines, the shortest arrangement, a mix and
    /// the sounds of the instruments.
    fn whole(lines: &str) -> String {
        format!("{SMALL}{lines}{SILENCE}{LEVELS}{SOUNDS}")
    }

    /// The same for an arrangement of its own, over the motifs, the patterns and the two
    /// sections that such a file has.
    fn arranged(arrangement: &str) -> String {
        format!("{SMALL}{MOTIFS}{PATTERNS}{SECTIONS}{arrangement}{LEVELS}{SOUNDS}")
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
        let piece = read(&whole("")).unwrap();
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
        // One motif and one section, which every scene plays without an instrument.
        assert_eq!((piece.motifs().len(), piece.passages().len()), (1, 1));
        assert!(piece.rhythms().is_empty());
        for scene in Scene::ALL {
            let play = piece.play(scene);
            assert_eq!(play.sections(), [0]);
            for instrument in Instrument::ALL {
                assert_eq!(play.part(instrument), Part::Silent);
                assert_eq!(piece.threat(4, instrument), None);
            }
        }
        // Without them, the file has no arrangement.
        assert_eq!(fault(SMALL), (5, "the file has no `play menu` line"));
        let calm = "the file has no `play calm` line";
        assert_eq!(
            fault(&whole("").replace("play calm rest\n", "")),
            (22, calm)
        );
        let arena = "the file has no `play arena` line";
        assert_eq!(
            fault(&whole("").replace("play arena rest\n", "")),
            (22, arena)
        );
    }

    #[test]
    fn comments_empty_lines_and_line_endings_change_nothing() {
        let dressed = "# A comment.\r\n\r\n  tanku-music   1\r\n#tempo 60\r\ntempo 96\r\n\
                       \tkey C# ionian mixolydian\n\nroll 1 kick snare 20 200\nlink 3 2 6";
        let whole_dressed = format!("{dressed}\n{SILENCE}{LEVELS}{SOUNDS}");
        assert_eq!(read(&whole_dressed), read(&whole("")));
        assert!(read(&whole("")).is_ok());
    }

    #[test]
    fn a_key_is_named_by_its_tonic() {
        let offset = |note: &str| {
            let key = read(&whole("").replace("C#", note)).unwrap().key;
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
                let text = whole("").replace("ionian mixolydian", &modes);
                let key = read(&text).unwrap().key;
                assert_eq!((key.home, key.arena), (home, arena));
            }
        }
    }

    #[test]
    fn motifs_and_patterns_read_bar_by_bar() {
        let lines = "pat kick heart x.o. .... .... .... 180 135\n\
                     pat bass heart R..O ..V. 5-5. x... 255\n";
        let piece = read(&whole(&format!("{MOTIFS}{lines}"))).unwrap();
        let [a, b, _] = piece.motifs() else {
            panic!("two motifs, and the one of the arrangement");
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
        assert_eq!((bass.level, bass.soft, bass.echo), (255, 0, false));
        assert!(bass.name.is("heart") && !bass.name.is("hear"));
        assert_eq!(bass.name.as_str(), "heart");
        assert_eq!(piece.rhythm(Instrument::Hat, "heart"), None);
        assert_eq!(piece.rhythm(Instrument::Kick, "fight"), None);
    }

    #[test]
    fn a_section_states_its_motifs() {
        let lines = "sec verse 14C5 7621 A B b A:9 B:4 100\nsec calm 1111 1111 b b b b\n";
        let piece = read(&whole(&format!("{MOTIFS}{lines}"))).unwrap();
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
    fn an_arrangement_reads_scene_by_scene() {
        let piece = read(&arranged(ARRANGEMENT)).unwrap();
        // A line is a melody or one of its forms, or a pattern, by its place among the seven.
        let tune = |form, octave, level| Source::Tune {
            form,
            octave,
            level,
        };
        let rhythm = |index, octave| Source::Rhythm { index, octave };
        let alone = |first| Layer {
            first,
            second: None,
            together: false,
        };
        let plays = |first| Part::Plays(alone(first));

        let menu = piece.play(Scene::Menu);
        assert_eq!(menu.sections(), [0]);
        assert_eq!(menu.part(Instrument::Bass), plays(rhythm(3, 2)));
        assert_eq!(
            menu.part(Instrument::Pluck),
            plays(tune(Form::Melody, 4, 255))
        );
        assert_eq!(menu.part(Instrument::Kick), Part::Silent);
        assert_eq!(menu.part(Instrument::Lead), Part::Silent);

        // Two sections in turn. A drum has no octave, and a melody can have a level.
        let calm = piece.play(Scene::Calm);
        assert_eq!(calm.sections(), [1, 0]);
        assert_eq!(calm.part(Instrument::Kick), plays(rhythm(0, 0)));
        assert_eq!(
            calm.part(Instrument::Lead),
            plays(tune(Form::Answer, 4, 150))
        );
        // A line where another starts nothing.
        let under = Layer {
            first: tune(Form::Melody, 3, 255),
            second: Some(rhythm(6, 2)),
            together: false,
        };
        assert_eq!(calm.part(Instrument::Pluck), Part::Plays(under));

        let corridor = piece.play(Scene::Corridor);
        assert_eq!(corridor.sections(), [0, 1]);
        for instrument in [Instrument::Hat, Instrument::Bass, Instrument::Lead] {
            assert_eq!(corridor.part(instrument), Part::ByThreat);
        }
        assert_eq!(corridor.part(Instrument::Pluck), Part::Silent);

        // Two lines of the lead together.
        let arena = piece.play(Scene::Arena);
        let both = Layer {
            first: tune(Form::Ornate, 4, 255),
            second: Some(tune(Form::Skeleton, 5, 90)),
            together: true,
        };
        assert_eq!(arena.part(Instrument::Lead), Part::Plays(both));
        assert_eq!(arena.part(Instrument::Snare), plays(rhythm(2, 0)));
        assert_eq!(arena.part(Instrument::Bass), plays(rhythm(4, 1)));
        assert_eq!(arena.part(Instrument::Pluck), plays(rhythm(5, 3)));

        // What the threat gives to the instruments that a scene leaves to it.
        assert_eq!(piece.threat(0, Instrument::Bass), Some(alone(rhythm(3, 2))));
        assert_eq!(piece.threat(0, Instrument::Hat), None);
        assert_eq!(piece.threat(0, Instrument::Lead), None);
        assert_eq!(piece.threat(3, Instrument::Hat), Some(alone(rhythm(1, 0))));
        let loud = alone(tune(Form::Ornate, 4, 200));
        assert_eq!(piece.threat(4, Instrument::Lead), Some(loud));
        assert_eq!(piece.threat(4, Instrument::Pluck), None);

        // A pattern of the pluck has an echo if its line asks for one.
        let echo = |name: &str| piece.rhythm(Instrument::Pluck, name).unwrap().echo;
        assert!(echo("up") && !echo("drone"));
        assert_eq!(piece.pattern(5).name.as_str(), "up");
        assert_eq!(piece.section(1).name.as_str(), "two");
    }

    #[test]
    fn a_fault_in_the_arrangement_says_what_is_wrong() {
        // The arrangement starts on line 18: its four scenes, then its five threats.
        let with = |old: &str, new: &str| {
            let text = arranged(ARRANGEMENT);
            assert!(text.contains(old), "{old}");
            fault(&text.replace(old, new))
        };
        // A scene and its sections.
        assert_eq!(with("play menu one", "play bay one"), (18, SCENE));
        assert_eq!(with("play menu one", "play"), (18, SCENE));
        let turns = TURN_BY_TURN;
        assert_eq!(
            with("play menu one bass=roots@2", "play menu bass=roots@2"),
            (18, turns)
        );
        assert_eq!(with("play menu one", "play menu three"), (18, turns));
        assert_eq!(with("play calm two,one", "play calm two,"), (19, turns));
        let twice = "this scene is written twice";
        assert_eq!(with("play calm two,one", "play menu two,one"), (19, twice));
        let none = "the file has no `play corridor` line";
        assert_eq!(
            with("play corridor one,two kick=four hat=* bass=* lead=*\n", "").1,
            none
        );

        // What an instrument plays.
        assert_eq!(with("bass=roots@2 pluck", "bass:roots@2 pluck"), (18, PART));
        assert_eq!(
            with("bass=roots@2 pluck", "organ=roots@2 pluck"),
            (18, PART)
        );
        let twice = "this instrument is written twice";
        assert_eq!(
            with("bass=roots@2 pluck", "bass=roots@2 bass=walk@1 pluck"),
            (18, twice)
        );
        for octave in ["", "@", "@0", "@6", "@22", "@x"] {
            let line = format!("bass=roots{octave} pluck");
            assert_eq!(with("bass=roots@2 pluck", &line), (18, OCTAVE), "{octave}");
        }
        // A pattern of another instrument, a pattern that isn't written, and a level for a
        // pattern, which has its own.
        assert_eq!(with("bass=roots@2 pluck", "bass=up@2 pluck"), (18, LINE));
        assert_eq!(with("bass=roots@2 pluck", "bass=run@2 pluck"), (18, LINE));
        assert_eq!(
            with("bass=roots@2 pluck", "bass=roots@2:9 pluck"),
            (18, LOUD)
        );
        for level in [":", ":0", ":256", ":loud"] {
            let line = format!("pluck=melody@4{level}\n");
            assert_eq!(with("pluck=melody@4\n", &line), (18, LOUD), "{level}");
        }
        // A drum has one pattern of its own, and nothing else.
        assert_eq!(
            with("kick=four pluck", "kick=four@1 pluck"),
            (19, BEAT_OF_DRUM)
        );
        assert_eq!(
            with("kick=four pluck", "kick=four/four pluck"),
            (19, BEAT_OF_DRUM)
        );
        assert_eq!(with("kick=four pluck", "kick=off pluck"), (19, LINE));
        assert_eq!(with("kick=four pluck", "kick=melody pluck"), (19, LINE));
        // Only the lead has two lines at once, and it has no pattern.
        let both = "pluck=melody@3+drone@2";
        assert_eq!(with("pluck=melody@3/drone@2", both), (19, TOGETHER));
        assert_eq!(with("lead=answer@4:150", "lead=drone@2"), (19, LINE));

        // The threat.
        assert_eq!(with("threat 4 hat=off", "threat 5 hat=off"), (26, THREAT));
        assert_eq!(with("threat 4 hat=off", "threat hat=off"), (26, THREAT));
        let twice = "this threat is written twice";
        assert_eq!(with("threat 4 hat=off", "threat 3 hat=off"), (26, twice));
        assert_eq!(with("threat 2 hat=off", "threat 2 hat=*"), (24, STAR));
        // A scene leaves instruments to the threat, and one threat has no line.
        let third = "threat 3 hat=off bass=walk@1 lead=ornate@4\n";
        assert_eq!(with(third, ""), (37, LEFT));
        // A threat gives an instrument that no scene leaves to it.
        assert_eq!(
            with("threat 1 hat=off", "threat 1 pluck=up@3 hat=off"),
            (23, NOT_LEFT)
        );
    }

    #[test]
    fn the_mix_reads_in_percent() {
        let piece = read(&whole("")).unwrap();
        assert_eq!(piece.mix(Instrument::Kick), [0.3, 0.3]);
        assert_eq!(piece.mix(Instrument::Hat), [0.7, 0.4]);
        assert_eq!(piece.mix(Instrument::Pluck), [0.2, 0.15]);
        // A number of percent can have a point, and it is the nearest value to what is
        // written.
        assert_eq!(piece.mix(Instrument::Lead), [0.055, 0.1225]);
        // The mix starts on line 12, with the kick.
        let with = |old: &str, new: &str| {
            let text = whole("");
            assert!(text.contains(old), "{old}");
            read(&text.replace(old, new))
        };
        let silent = with("mix kick 30% 30%", "mix kick 0% 100%").unwrap();
        assert_eq!(silent.mix(Instrument::Kick), [0.0, 1.0]);
        let bad = [
            "30 30%",
            "30% 30",
            "30%",
            "30% 101%",
            "30% 100.1%",
            "-3% 30%",
            "30% .5%",
            "30% 5.%",
            "30% 1.2.3%",
            "30% 0.12345%",
            "30% 12345678%",
            "30% %",
            "30% 30 %",
        ];
        for levels in bad {
            let fault = with("mix kick 30% 30%", &format!("mix kick {levels}")).unwrap_err();
            assert_eq!((fault.line, fault.what), (12, MIX), "{levels}");
        }
        let fault = |old: &str, new: &str| {
            let fault = with(old, new).unwrap_err();
            (fault.line, fault.what)
        };
        assert_eq!(fault("mix kick 30% 30%", "mix organ 30% 30%"), (12, MIX));
        let extra = "this line has a word too many";
        assert_eq!(
            fault("mix kick 30% 30%", "mix kick 30% 30% 30%"),
            (12, extra)
        );
        let twice = "this instrument is mixed twice";
        assert_eq!(fault("mix hat 70% 40%", "mix kick 70% 40%"), (13, twice));
        // Every instrument has its line.
        let none = "the file has no `mix lead` line";
        assert_eq!(fault("mix lead 5.5% 12.25%\n", ""), (22, none));
        let none = "the file has no `mix bass` line";
        assert_eq!(fault("mix bass 10% 10%\n", ""), (22, none));
    }

    #[test]
    fn the_sounds_read_in_ms_in_hz_and_in_percent() {
        let sounds = read(&whole("")).unwrap().sounds;
        // A time is in seconds from then on, and a share is a part of the whole: each is the
        // nearest value to what is written.
        let kick = KickSound {
            from: 120.0,
            to: 50.0,
            sweep: 0.025,
            decay: 0.06,
        };
        assert_eq!(sounds.kick, kick);
        let hat = HatSound {
            above: 8000.0,
            decay: 0.0125,
        };
        assert_eq!(sounds.hat, hat);
        let snare = SnareSound {
            center: 1800.0,
            width: 900.0,
            decay: 0.04,
            tone: 180.0,
            tone_decay: 0.02,
            tone_level: 0.5,
        };
        assert_eq!(sounds.snare, snare);
        let bass = BassSound {
            rise: 0.002,
            from: 2000.0,
            to: 250.0,
            sweep: 0.08,
        };
        assert_eq!(sounds.bass, bass);
        let pluck = PluckSound {
            ring: 0.75,
            smooth: 2,
            echo_steps: 4,
            echo_level: 0.25,
        };
        assert_eq!(sounds.pluck, pluck);
        let lead = LeadSound {
            width: 0.25,
            rise: 0.005,
            below: 4000.0,
            vibrato_after: 0.2,
            vibrato_depth: 0.005,
            vibrato_rate: 6.0,
        };
        assert_eq!(sounds.lead, lead);

        // The sounds start on line 18, with the kick, and the lead is on line 23.
        let with = |old: &str, new: &str| {
            let text = whole("");
            assert!(text.contains(old), "{old}");
            read(&text.replace(old, new))
        };
        // The settings of an instrument come in any order, and a pitch or a filter can go
        // either way.
        let turned = "sound snare tone 180Hz 20ms 50% decay 40ms band 1800Hz 900Hz";
        let snare = "sound snare band 1800Hz 900Hz decay 40ms tone 180Hz 20ms 50%";
        assert_eq!(with(snare, turned), read(&whole("")));
        let rising = with("pitch 120Hz 50Hz", "pitch 50Hz 120Hz").unwrap().sounds;
        assert_eq!((rising.kick.from, rising.kick.to), (50.0, 120.0));
        let opening = with("filter 2000Hz 250Hz", "filter 250Hz 2000Hz").unwrap();
        assert_eq!(
            (opening.sounds.bass.from, opening.sounds.bass.to),
            (250.0, 2000.0)
        );

        // Every setting has its bounds, and both ends of them read.
        let ends = [
            ("decay 60ms", "decay 0.1ms"),
            ("decay 60ms", "decay 10000ms"),
            ("above 8000Hz", "above 20Hz"),
            ("above 8000Hz", "above 20000Hz"),
            ("band 1800Hz 900Hz", "band 5000Hz 500Hz"),
            ("band 1800Hz 900Hz", "band 5000Hz 10000Hz"),
            ("band 1800Hz 900Hz", "band 20Hz 40Hz"),
            ("tone 180Hz 20ms 50%", "tone 180Hz 20ms 0%"),
            ("tone 180Hz 20ms 50%", "tone 180Hz 20ms 100%"),
            ("ring 750ms", "ring 100ms"),
            ("ring 750ms", "ring 10000ms"),
            ("smooth 2", "smooth 0"),
            ("smooth 2", "smooth 8"),
            ("echo 4 25%", "echo 1 0%"),
            ("echo 4 25%", "echo 8 100%"),
            ("width 25%", "width 1%"),
            ("width 25%", "width 99%"),
            ("vibrato 200ms 0.5% 6Hz", "vibrato 0.1ms 0% 0.1Hz"),
            ("vibrato 200ms 0.5% 6Hz", "vibrato 10000ms 10% 20Hz"),
        ];
        for (old, new) in ends {
            assert!(with(old, new).is_ok(), "{new}");
        }
        let fault = |old: &str, new: &str| {
            let fault = with(old, new).unwrap_err();
            (fault.line, fault.what)
        };
        let past = [
            // A time, with its unit.
            ("decay 60ms", "decay 60", 18, TIME),
            ("decay 60ms", "decay 60Hz", 18, TIME),
            ("decay 60ms", "decay 60 ms", 18, TIME),
            ("decay 60ms", "decay 0.09ms", 18, TIME),
            ("decay 60ms", "decay 10001ms", 18, TIME),
            ("decay 60ms", "decay", 18, TIME),
            ("decay 12.5ms", "decay 12.50001ms", 19, TIME),
            ("rise 2ms", "rise 0ms", 21, TIME),
            ("rise 5ms", "rise -5ms", 23, TIME),
            // A frequency, with its unit.
            ("above 8000Hz", "above 8000", 19, FREQUENCY),
            ("above 8000Hz", "above 8kHz", 19, FREQUENCY),
            ("above 8000Hz", "above 8000hz", 19, FREQUENCY),
            ("above 8000Hz", "above 19.9Hz", 19, FREQUENCY),
            ("above 8000Hz", "above 20001Hz", 19, FREQUENCY),
            ("pitch 120Hz 50Hz 25ms", "pitch 120Hz 25ms", 18, FREQUENCY),
            ("below 4000Hz", "below 4000ms", 23, FREQUENCY),
            // The band of the snare.
            ("band 1800Hz 900Hz", "band 5001Hz 900Hz", 20, BAND),
            ("band 1800Hz 900Hz", "band 1800Hz 179Hz", 20, BAND),
            ("band 1800Hz 900Hz", "band 1800Hz 3601Hz", 20, BAND),
            ("tone 180Hz 20ms 50%", "tone 180Hz 20ms 101%", 20, SNARE),
            ("tone 180Hz 20ms 50%", "tone 180Hz 20ms 0.5", 20, SNARE),
            // The string and its echo.
            ("ring 750ms", "ring 99.9ms", 22, RING),
            ("ring 750ms", "ring 10001ms", 22, RING),
            ("ring 750ms", "ring 1s", 22, RING),
            ("smooth 2", "smooth 9", 22, SMOOTH),
            ("smooth 2", "smooth 2.5", 22, SMOOTH),
            ("echo 4 25%", "echo 0 25%", 22, ECHO),
            ("echo 4 25%", "echo 9 25%", 22, ECHO),
            ("echo 4 25%", "echo 4 101%", 22, ECHO),
            ("echo 4 25%", "echo 500ms 25%", 22, ECHO),
            ("echo 4 25%", "echo 4", 22, ECHO),
            // The wave of the lead and its vibrato.
            ("width 25%", "width 0.9%", 23, WIDTH),
            ("width 25%", "width 100%", 23, WIDTH),
            ("width 25%", "width 0.25", 23, WIDTH),
            ("0.5% 6Hz", "10.1% 6Hz", 23, DEPTH),
            ("0.5% 6Hz", "6Hz 0.5%", 23, DEPTH),
            ("0.5% 6Hz", "0.5% 20.5Hz", 23, RATE),
            ("0.5% 6Hz", "0.5% 0.09Hz", 23, RATE),
            ("0.5% 6Hz", "0.5%", 23, RATE),
            // A setting that an instrument doesn't have, one that is missing and one that is
            // there twice.
            ("decay 60ms", "decay 60ms ring 750ms", 18, KICK),
            (" decay 60ms", "", 18, KICK),
            ("decay 60ms", "decay 60ms decay 60ms", 18, KICK),
            ("above 8000Hz decay", "below 8000Hz decay", 19, HAT),
            ("sound hat above 8000Hz ", "sound hat ", 19, HAT),
            (" tone 180Hz 20ms 50%", "", 20, SNARE),
            ("band 1800Hz 900Hz", "band 1800Hz 900Hz 40ms", 20, SNARE),
            ("rise 2ms", "rise 2ms width 25%", 21, BASS),
            ("rise 2ms ", "", 21, BASS),
            ("smooth 2 ", "", 22, PLUCK),
            ("ring 750ms", "ring 750ms 800ms", 22, PLUCK),
            ("below 4000Hz", "pass 4000Hz", 23, LEAD),
            ("width 25% ", "", 23, LEAD),
            ("sound lead width 25%", "sound lead 25%", 23, LEAD),
        ];
        for (old, new, line, what) in past {
            assert_eq!(fault(old, new), (line, what), "{new}");
        }

        // An instrument that the game doesn't have, and one whose sound is written twice.
        assert_eq!(fault("sound kick pitch", "sound organ pitch"), (18, SOUND));
        assert_eq!(
            fault(&SOUNDS[..SOUNDS.find('\n').unwrap()], "sound"),
            (18, SOUND)
        );
        let twice = "the sound of this instrument is written twice";
        let again = "sound kick pitch 120Hz 50Hz 25ms decay 60ms";
        assert_eq!(
            fault("sound hat above 8000Hz decay 12.5ms", again),
            (19, twice)
        );
        // Every instrument has its line.
        let lead = SOUNDS.lines().last().unwrap();
        let none = "the file has no `sound lead` line";
        assert_eq!(fault(&format!("{lead}\n"), ""), (22, none));
        let none = "the file has no `sound snare` line";
        let snare = format!("{snare}\n");
        assert_eq!(fault(&snare, ""), (22, none));
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
        assert_eq!(pattern("bass melody R... .... .... .... 100"), (6, TAKEN));
        assert_eq!(pattern("pluck answer x... .... .... .... 100"), (6, TAKEN));
        // Only the pluck has an echo, which a pattern asks for after its levels.
        assert_eq!(
            pattern("kick heart x... .... .... .... 100 echo"),
            (6, ENDING)
        );
        assert_eq!(
            pattern("pluck up x... .... .... .... 100 echo 50"),
            (6, ENDING)
        );
        assert_eq!(
            pattern("pluck up x... .... .... .... 100 echo echo"),
            (6, ENDING)
        );
        assert_eq!(
            pattern("pluck up x.o. .... .... .... 100 50 60"),
            (6, ENDING)
        );
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
        // With the motif of the shortest arrangement, eight.
        assert!(read(&whole(&lines(7, motif))).is_ok());
        let full = fault_after(&lines(9, motif));
        assert_eq!(full, (14, "a piece has eight motifs at most"));
        let long = "motif A 1... .... .... ....".to_string() + &" | 1... .... .... ....".repeat(4);
        assert_eq!(fault_after(&long).1, "a motif has four bars at most");

        let pattern = |index: usize| format!("pat kick k{index} x... .... .... .... 9\n");
        assert!(read(&whole(&lines(16, pattern))).is_ok());
        let full = fault_after(&lines(17, pattern));
        assert_eq!(full, (22, "a piece has sixteen patterns at most"));

        let section = |index: usize| format!("sec s{index} 1111 1111 A A A A A A A A\n");
        let sections = |count: usize| format!("{}{}", motif(0), lines(count, section));
        assert!(read(&whole(&sections(7))).is_ok());
        let full = fault_after(&sections(9));
        assert_eq!(full, (15, "a piece has eight sections at most"));

        // A scene plays four sections in turn at most.
        let turns = |sections: &str| whole("").replace("play menu rest", sections);
        assert!(read(&turns("play menu rest,rest,rest,rest")).is_ok());
        let full = fault(&turns("play menu rest,rest,rest,rest,rest"));
        assert_eq!(full, (8, "a scene plays four sections at most"));
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

    #[test]
    #[should_panic(expected = "and how many times per second, each once")]
    fn the_longest_fault_is_whole_when_it_stops_the_build() {
        let fault = Fault {
            line: 1234,
            what: LEAD,
        };
        fault.stop();
    }
}
