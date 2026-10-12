//! The words of the file that have something inside them: what an instrument plays in a line
//! of the arrangement, the motif that a melody states, and a number with its unit.
//!
//! The reader of `tanku_audio` is the judge of all of them. What is here only takes a word
//! apart far enough to change one piece of it and write it again.

use tanku_audio::notation::{Form, Instrument, Piece};

use crate::lines;

/// One line of an instrument: a pattern or a form of the melody, the octave of its tonic if
/// the instrument has a pitch, and the level of a melody that isn't as loud as can be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Voice {
    pub name: String,
    pub octave: Option<u8>,
    pub level: Option<u8>,
}

impl Voice {
    fn parse(text: &str) -> Self {
        let (text, level) = match text.split_once(':') {
            Some((text, level)) => (text, level.parse().ok()),
            None => (text, None),
        };
        let (name, octave) = match text.split_once('@') {
            Some((name, octave)) => (name, octave.parse().ok()),
            None => (text, None),
        };
        Self {
            name: name.to_string(),
            octave,
            level,
        }
    }

    fn write(&self) -> String {
        let mut text = self.name.clone();
        if let Some(octave) = self.octave {
            text.push_str(&format!("@{octave}"));
        }
        if let Some(level) = self.level {
            text.push_str(&format!(":{level}"));
        }
        text
    }

    /// Whether the line is a form of the melody, which has a level, and not a pattern.
    pub fn is_tune(&self) -> bool {
        Form::ALL.iter().any(|form| form.name() == self.name)
    }

    /// The level of the line as the game plays it: a melody without one is as loud as can be.
    pub fn loud(&self) -> u8 {
        self.level.unwrap_or(255)
    }
}

/// What an instrument has in a line of the arrangement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartText {
    Silent,
    /// `*`: what the line of the threat says.
    ByThreat,
    Plays {
        first: Voice,
        /// A second line: under the first, where the first starts nothing, or together with
        /// it.
        second: Option<Voice>,
        together: bool,
    },
}

impl PartText {
    /// What comes after the `=` of an instrument.
    pub fn parse(text: &str) -> Self {
        if text == "*" {
            return Self::ByThreat;
        }
        let (first, second, together) = match text.split_once('/') {
            Some((first, second)) => (first, Some(second), false),
            None => match text.split_once('+') {
                Some((first, second)) => (first, Some(second), true),
                None => (text, None, false),
            },
        };
        Self::Plays {
            first: Voice::parse(first),
            second: second.map(Voice::parse),
            together,
        }
    }

    /// What is written after the `=`, or nothing for an instrument that the line leaves out.
    pub fn write(&self) -> Option<String> {
        match self {
            Self::Silent => None,
            Self::ByThreat => Some("*".to_string()),
            Self::Plays {
                first,
                second,
                together,
            } => {
                let mut text = first.write();
                if let Some(second) = second {
                    text.push(if *together { '+' } else { '/' });
                    text.push_str(&second.write());
                }
                Some(text)
            }
        }
    }

    /// The part of an instrument in a line of the arrangement.
    pub fn of(line: &str, instrument: Instrument) -> Self {
        match part_word(line, instrument) {
            Some((_, text)) => Self::parse(text),
            None => Self::Silent,
        }
    }
}

/// The word of a line that says what an instrument plays: its place, and what comes after
/// its `=`.
pub fn part_word(line: &str, instrument: Instrument) -> Option<(usize, &str)> {
    let words = lines::words(line);
    let (place, word) = words.into_iter().enumerate().find(|(_, word)| {
        word.split_once('=')
            .is_some_and(|(name, _)| name == instrument.name())
    })?;
    Some((place, word.split_once('=')?.1))
}

/// A line of the arrangement with another part for an instrument. A part that is new goes
/// among the others in the order of the instruments.
pub fn with_part(line: &str, instrument: Instrument, part: &PartText) -> String {
    let written = part
        .write()
        .map(|text| format!("{}={text}", instrument.name()));
    match (part_word(line, instrument), written) {
        (Some((place, _)), Some(word)) => lines::with_word(line, place, &word),
        (Some((place, _)), None) => lines::without_word(line, place),
        (None, None) => line.to_string(),
        (None, Some(word)) => {
            // Before the first part of an instrument that comes later.
            let words = lines::words(line);
            let later = |word: &&str| {
                let name = word.split_once('=').map(|(name, _)| name);
                let place = Instrument::ALL
                    .iter()
                    .position(|other| Some(other.name()) == name);
                place.is_some_and(|place| place > instrument as usize)
            };
            match words.iter().position(later) {
                Some(place) => lines::with_word(line, place, &format!("{word} {}", words[place])),
                None => lines::with_last(line, &word),
            }
        }
    }
}

/// What a line of an instrument can be: its patterns, then the forms of the melody if it has
/// a pitch. The lead has no pattern, and a drum plays no melody.
pub fn voices(piece: &Piece, instrument: Instrument) -> Vec<String> {
    let patterns = piece.rhythms().iter();
    let mut names: Vec<String> = patterns
        .filter(|rhythm| rhythm.instrument == instrument)
        .map(|rhythm| rhythm.name.as_str().to_string())
        .collect();
    if !instrument.is_drum() {
        names.extend(Form::ALL.iter().map(|form| form.name().to_string()));
    }
    names
}

/// The octave that a line of an instrument starts in when nothing says otherwise.
pub fn usual_octave(instrument: Instrument) -> Option<u8> {
    match instrument {
        Instrument::Bass => Some(2),
        Instrument::Pluck => Some(3),
        Instrument::Lead => Some(4),
        _ => None,
    }
}

/// A motif as a word of a melody states it: as it is written, with its turn in place of its
/// last bar, or with its last note on another degree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Statement {
    pub letter: u8,
    pub turned: bool,
    pub last: Option<u8>,
}

impl Statement {
    pub fn parse(word: &str) -> Option<Self> {
        let bytes = word.as_bytes();
        let letter = *bytes.first()?;
        if !letter.is_ascii_alphabetic() {
            return None;
        }
        let last = match bytes {
            [_] => None,
            [_, b':', digit @ b'1'..=b'9'] => Some(digit - b'0'),
            _ => return None,
        };
        Some(Self {
            letter: letter.to_ascii_uppercase(),
            turned: letter.is_ascii_lowercase(),
            last,
        })
    }

    pub fn write(&self) -> String {
        let letter = self.letter as char;
        match (self.turned, self.last) {
            (_, Some(degree)) => format!("{letter}:{degree}"),
            (true, None) => letter.to_ascii_lowercase().to_string(),
            (false, None) => letter.to_string(),
        }
    }
}

/// A `sec` line, taken apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionLine {
    pub name: String,
    pub chords: [u8; 8],
    pub statements: Vec<Statement>,
    pub tempo: Option<u16>,
}

impl SectionLine {
    pub fn parse(line: &str) -> Option<Self> {
        let words = lines::words(line);
        let [_, name, first, second, rest @ ..] = words.as_slice() else {
            return None;
        };
        let mut chords = [1; 8];
        for (place, symbol) in first.bytes().chain(second.bytes()).take(8).enumerate() {
            chords[place] = match symbol {
                b'C' => 0,
                digit => digit.wrapping_sub(b'0'),
            };
        }
        let tempo = rest.last().and_then(|word| word.parse::<u16>().ok());
        let stated = &rest[..rest.len() - tempo.is_some() as usize];
        let statements = stated.iter().map(|word| Statement::parse(word));
        Some(Self {
            name: name.to_string(),
            chords,
            statements: statements.collect::<Option<Vec<_>>>()?,
            tempo,
        })
    }

    pub fn write(&self) -> String {
        let statements: Vec<String> = self.statements.iter().map(Statement::write).collect();
        lines::section(&self.name, &self.chords, &statements, self.tempo)
    }

    /// The statement that a bar of the section belongs to: its place, and the bars that it
    /// covers.
    pub fn stating(&self, piece: &Piece, bar: usize) -> Option<(usize, usize, usize)> {
        let mut from = 0;
        for (place, statement) in self.statements.iter().enumerate() {
            let bars = piece.motif(statement.letter)?.bars().len();
            if bar < from + bars {
                return Some((place, from, bars));
            }
            from += bars;
        }
        None
    }
}

/// The unit that a number of a `sound` line is written with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Ms,
    Hz,
    Percent,
    /// A plain count: a number of steps, or of times.
    Count,
}

impl Unit {
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Ms => "ms",
            Self::Hz => "Hz",
            Self::Percent => "%",
            Self::Count => "",
        }
    }
}

/// One number of a setting: what it is, its unit and its bounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knob {
    pub label: &'static str,
    pub unit: Unit,
    pub least: f32,
    pub most: f32,
}

const fn knob(label: &'static str, unit: Unit, least: f32, most: f32) -> Knob {
    Knob {
        label,
        unit,
        least,
        most,
    }
}

const fn time(label: &'static str) -> Knob {
    knob(label, Unit::Ms, 0.1, 10_000.0)
}

const fn pitch(label: &'static str) -> Knob {
    knob(label, Unit::Hz, 20.0, 20_000.0)
}

/// The settings of the sound of an instrument, by the word that names each in its `sound`
/// line, with the numbers that follow that word. `docs/MUSIC.md` says what they do.
pub fn settings(instrument: Instrument) -> &'static [(&'static str, &'static [Knob])] {
    const KICK: &[(&str, &[Knob])] = &[
        (
            "pitch",
            &[pitch("Starts at"), pitch("Goes to"), time("Gets there in")],
        ),
        ("decay", &[time("Dies away in")]),
    ];
    const HAT: &[(&str, &[Knob])] = &[
        ("above", &[pitch("Noise kept above")]),
        ("decay", &[time("Dies away in")]),
    ];
    const SNARE: &[(&str, &[Knob])] = &[
        (
            "band",
            &[
                knob("Noise around", Unit::Hz, 20.0, 5000.0),
                pitch("Width of that band"),
            ],
        ),
        ("decay", &[time("Noise dies away in")]),
        (
            "tone",
            &[
                pitch("Tone"),
                time("Tone dies away in"),
                knob("Level of the tone", Unit::Percent, 0.0, 100.0),
            ],
        ),
    ];
    const BASS: &[(&str, &[Knob])] = &[
        ("rise", &[time("Rise")]),
        (
            "filter",
            &[
                pitch("Filter starts at"),
                pitch("Filter goes to"),
                time("Gets there in"),
            ],
        ),
    ];
    const PLUCK: &[(&str, &[Knob])] = &[
        ("ring", &[knob("Rings for", Unit::Ms, 100.0, 10_000.0)]),
        ("smooth", &[knob("Attack smoothed", Unit::Count, 0.0, 8.0)]),
        (
            "echo",
            &[
                knob("Echo after, in steps", Unit::Count, 1.0, 8.0),
                knob("Level of the echo", Unit::Percent, 0.0, 100.0),
            ],
        ),
    ];
    const LEAD: &[(&str, &[Knob])] = &[
        (
            "width",
            &[knob("Width of the pulse", Unit::Percent, 1.0, 99.0)],
        ),
        ("rise", &[time("Rise")]),
        ("below", &[pitch("Heard below")]),
        (
            "vibrato",
            &[
                time("Vibrato comes in after"),
                knob("Depth of the vibrato", Unit::Percent, 0.0, 10.0),
                knob("Rate of the vibrato", Unit::Hz, 0.1, 20.0),
            ],
        ),
    ];
    match instrument {
        Instrument::Kick => KICK,
        Instrument::Hat => HAT,
        Instrument::Snare => SNARE,
        Instrument::Bass => BASS,
        Instrument::Pluck => PLUCK,
        Instrument::Lead => LEAD,
    }
}

/// A number of a `sound` line as the window shows it: where its word is in the line, what
/// it is and what it is worth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dial {
    pub word: usize,
    pub knob: Knob,
    pub value: f32,
}

/// The numbers of the `sound` line of an instrument, in the order of its settings.
pub fn dials(line: &str, instrument: Instrument) -> Vec<Dial> {
    let words = lines::words(line);
    let mut dials = Vec::new();
    for (key, knobs) in settings(instrument) {
        let Some(at) = words.iter().position(|word| word == key) else {
            continue;
        };
        for (offset, knob) in knobs.iter().enumerate() {
            let word = at + 1 + offset;
            let value = words.get(word).and_then(|text| value(text, knob.unit));
            if let Some(value) = value {
                dials.push(Dial {
                    word,
                    knob: *knob,
                    value,
                });
            }
        }
    }
    dials
}

/// The number of a word that ends with a unit.
pub fn value(word: &str, unit: Unit) -> Option<f32> {
    word.strip_suffix(unit.suffix())?.parse().ok()
}

/// A number with its unit, as the file writes it.
pub fn written(value: f32, unit: Unit) -> String {
    format!("{}{}", lines::number(value), unit.suffix())
}

/// The number as the window shows it, with a space before a unit that is a word.
pub fn shown(value: f32, unit: Unit) -> String {
    match unit {
        Unit::Ms => format!("{} ms", lines::number(value)),
        Unit::Hz => format!("{} Hz", lines::number(value)),
        Unit::Percent => format!("{}%", lines::number(value)),
        Unit::Count => lines::number(value),
    }
}

/// The next value of a number, up or down. A time, a frequency and a share move by a step
/// that fits their size, a tenth of it or so, and land on round numbers. A count moves by
/// one.
pub fn stepped(dial: &Dial, up: bool) -> f32 {
    let Knob {
        unit, least, most, ..
    } = dial.knob;
    if unit == Unit::Count {
        let next = dial.value + if up { 1.0 } else { -1.0 };
        return next.clamp(least, most);
    }
    // The largest of 1, 2 and 5 times a power of ten that is a tenth of the value at most,
    // and no finer than the file writes.
    let size = (dial.value as f64).max(least.max(0.01) as f64);
    let mut step = 0.0001;
    'search: for power in -4..5 {
        for lead in [1.0, 2.0, 5.0] {
            let candidate = lead * 10f64.powi(power);
            // A value of the file is a 32-bit number, a hair off the one that is written.
            if candidate > size / 10.0 * (1.0 + 1e-6) {
                break 'search;
            }
            step = candidate;
        }
    }
    let value = dial.value as f64;
    let next = if up {
        ((value / step) + 1e-6).floor() * step + step
    } else {
        ((value / step) - 1e-6).ceil() * step - step
    };
    (next as f32).clamp(least, most)
}

/// How far a number is between its bounds, from 0 to 1, for a bar that shows it. Times and
/// frequencies are spread by their size, as the ear takes them.
pub fn share(dial: &Dial) -> f32 {
    let Knob {
        unit, least, most, ..
    } = dial.knob;
    let share = match unit {
        Unit::Ms | Unit::Hz => (dial.value / least).ln() / (most / least).ln(),
        Unit::Percent | Unit::Count => (dial.value - least) / (most - least),
    };
    share.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use tanku_audio::score::PIECE;

    use super::*;

    const FILE: &str = include_str!("../../tanku_audio/music.score");

    fn line(head: &str) -> &'static str {
        FILE.lines().find(|line| line.starts_with(head)).unwrap()
    }

    #[test]
    fn a_part_is_taken_apart_and_written_again() {
        let calm = line("play calm");
        let pluck = PartText::of(calm, Instrument::Pluck);
        let PartText::Plays {
            first,
            second,
            together,
        } = &pluck
        else {
            panic!("the pluck plays in the calm");
        };
        assert_eq!(
            (first.name.as_str(), first.octave, first.loud()),
            ("melody", Some(3), 255)
        );
        assert_eq!(second.as_ref().unwrap().write(), "tick@2");
        assert!(!together && first.is_tune() && !second.as_ref().unwrap().is_tune());
        assert_eq!(pluck.write().as_deref(), Some("melody@3/tick@2"));
        assert_eq!(
            PartText::of(calm, Instrument::Lead).write().as_deref(),
            Some("answer@4:150")
        );
        assert_eq!(
            PartText::of(calm, Instrument::Kick).write().as_deref(),
            Some("heart")
        );
        assert_eq!(PartText::of(calm, Instrument::Hat), PartText::Silent);
        assert_eq!(
            PartText::of(line("play arena"), Instrument::Bass),
            PartText::ByThreat
        );
        let lead = PartText::of(line("threat 4"), Instrument::Lead);
        assert_eq!(lead.write().as_deref(), Some("ornate@4+ornate@5:130"));
        // Every part of the file comes back as it is written.
        for row in FILE
            .lines()
            .filter(|line| line.starts_with("play") || line.starts_with("threat"))
        {
            for instrument in Instrument::ALL {
                if let Some((_, text)) = part_word(row, instrument) {
                    assert_eq!(PartText::parse(text).write().as_deref(), Some(text));
                }
            }
        }
    }

    #[test]
    fn a_part_changes_in_its_line() {
        let menu = line("play menu");
        let silent = with_part(menu, Instrument::Pluck, &PartText::Silent);
        assert_eq!(silent, "play menu     intro         bass=long@2");
        // A part that is new stands among the others in the order of the instruments.
        let kick = PartText::parse("heart");
        assert_eq!(
            with_part(menu, Instrument::Kick, &kick),
            "play menu     intro         kick=heart bass=long@2 pluck=melody@4"
        );
        let lead = PartText::parse("melody@4:100");
        assert_eq!(
            with_part(menu, Instrument::Lead, &lead),
            "play menu     intro         bass=long@2 pluck=melody@4 lead=melody@4:100"
        );
        let star = with_part(menu, Instrument::Bass, &PartText::ByThreat);
        assert_eq!(star, "play menu     intro         bass=* pluck=melody@4");
        // The first part of a line that has none.
        assert_eq!(
            with_part(
                "play calm bridge",
                Instrument::Hat,
                &PartText::parse("arena")
            ),
            "play calm bridge hat=arena"
        );
    }

    #[test]
    fn an_instrument_plays_its_patterns_or_the_melody() {
        assert_eq!(voices(&PIECE, Instrument::Kick), ["heart", "fight"]);
        assert_eq!(
            voices(&PIECE, Instrument::Pluck),
            [
                "arpeggio", "fast", "tick", "melody", "skeleton", "ornate", "answer"
            ]
        );
        assert_eq!(voices(&PIECE, Instrument::Lead).len(), 4);
    }

    #[test]
    fn a_section_is_taken_apart_and_written_again() {
        for head in ["sec intro", "sec verse", "sec chorus", "sec bridge"] {
            let written = line(head);
            assert_eq!(SectionLine::parse(written).unwrap().write(), written);
        }
        let bridge = SectionLine::parse(line("sec bridge")).unwrap();
        assert_eq!((bridge.tempo, bridge.statements.len()), (Some(105), 8));
        assert_eq!(bridge.statements[1].write(), "C:7");
        assert_eq!(bridge.stating(&PIECE, 3), Some((3, 3, 1)));
        let verse = SectionLine::parse(line("sec verse")).unwrap();
        assert_eq!(verse.chords, [1, 1, 0, 4, 1, 1, 0, 5]);
        // Motif A has two bars: the last two of the verse are its fourth statement, which
        // ends on its turn.
        assert_eq!(verse.stating(&PIECE, 7), Some((3, 6, 2)));
        assert!(verse.statements[3].turned && !verse.statements[2].turned);
    }

    #[test]
    fn the_numbers_of_a_sound_are_found_in_its_line() {
        let lead = dials(line("sound lead"), Instrument::Lead);
        let values: Vec<f32> = lead.iter().map(|dial| dial.value).collect();
        assert_eq!(values, [30.0, 4.0, 3000.0, 136.0, 0.4, 5.5]);
        let words: Vec<usize> = lead.iter().map(|dial| dial.word).collect();
        assert_eq!(words, [3, 5, 7, 9, 10, 11]);
        assert_eq!(shown(lead[3].value, lead[3].knob.unit), "136 ms");
        assert_eq!(written(lead[4].value, lead[4].knob.unit), "0.4%");
        // Every number of every instrument is found, and none is out of its bounds.
        let counts = Instrument::ALL.map(|instrument| {
            let head = format!("sound {}", instrument.name());
            let dials = dials(line(&head), instrument);
            assert!(
                dials
                    .iter()
                    .all(|dial| dial.knob.least <= dial.value && dial.value <= dial.knob.most)
            );
            dials.len()
        });
        assert_eq!(counts, [4, 2, 6, 4, 4, 6]);
    }

    #[test]
    fn a_number_moves_by_a_step_of_its_size() {
        let dial = |value: f32, unit: Unit, least: f32, most: f32| Dial {
            word: 0,
            knob: knob("", unit, least, most),
            value,
        };
        let up = |dial: Dial| lines::number(stepped(&dial, true));
        let down = |dial: Dial| lines::number(stepped(&dial, false));
        // To the next round number, then from round number to round number.
        assert_eq!(up(dial(136.0, Unit::Ms, 0.1, 10_000.0)), "140");
        assert_eq!(up(dial(140.0, Unit::Ms, 0.1, 10_000.0)), "150");
        assert_eq!(down(dial(136.0, Unit::Ms, 0.1, 10_000.0)), "130");
        assert_eq!(up(dial(3000.0, Unit::Hz, 20.0, 20_000.0)), "3200");
        assert_eq!(down(dial(3000.0, Unit::Hz, 20.0, 20_000.0)), "2800");
        assert_eq!(up(dial(45.0, Unit::Ms, 0.1, 10_000.0)), "46");
        assert_eq!(up(dial(5.5, Unit::Hz, 0.1, 20.0)), "6");
        assert_eq!(down(dial(0.4, Unit::Percent, 0.0, 10.0)), "0.38");
        assert_eq!(up(dial(33.0, Unit::Percent, 0.0, 100.0)), "34");
        // Never past a bound, and a count moves by one.
        assert_eq!(up(dial(20_000.0, Unit::Hz, 20.0, 20_000.0)), "20000");
        assert_eq!(down(dial(0.0, Unit::Percent, 0.0, 10.0)), "0");
        assert_eq!(up(dial(0.0, Unit::Percent, 0.0, 10.0)), "0.001");
        assert_eq!(up(dial(3.0, Unit::Count, 1.0, 8.0)), "4");
        assert_eq!(up(dial(8.0, Unit::Count, 1.0, 8.0)), "8");
        // The bar of a frequency is spread by octaves: 3000 Hz is most of the way up.
        let bar = share(&dial(3000.0, Unit::Hz, 20.0, 20_000.0));
        assert!((bar - 0.725).abs() < 0.01, "{bar}");
    }
}
