//! The lines of the file, as the editor writes them.
//!
//! The editor never holds a piece that it could write wrong: it holds the lines of the file,
//! changes a word of one of them, and lets the reader of `tanku_audio` say what the lines
//! mean. So everything here is about words: where they are in a line, how a word is replaced
//! without moving the ones after it, and how a record is written from nothing.
//!
//! `docs/MUSIC.md` describes the records.

use tanku_audio::notation::{Cell, Instrument, Line, STEPS};

/// The kinds of record, in the order in which a file has them. A record that is new goes
/// after the last one of its kind, or after those of the kinds before it.
pub const ORDER: [&str; 13] = [
    "tanku-music",
    "tempo",
    "key",
    "motif",
    "turn",
    "pat",
    "sec",
    "play",
    "threat",
    "roll",
    "link",
    "mix",
    "sound",
];

/// The kinds that share a block of the file with the kind before them: no empty line comes
/// between the two.
const SAME_BLOCK: [&str; 4] = ["tempo", "key", "turn", "link"];

/// The parts that the window lists a file in, each with the kinds of its records.
pub const PARTS: [(&str, &[&str]); 8] = [
    ("Piece", &["tanku-music", "tempo", "key"]),
    ("Motifs", &["motif", "turn"]),
    ("Patterns", &["pat"]),
    ("Sections", &["sec"]),
    ("Arrangement", &["play", "threat"]),
    ("Transitions", &["roll", "link"]),
    ("Mix", &["mix"]),
    ("Sounds", &["sound"]),
];

/// The part that each line of a file belongs to: a record by its kind, and a comment with the
/// record that follows it, or with the last one if none does. An empty line belongs to none.
pub fn parts(lines: &[String]) -> Vec<Option<usize>> {
    let part = |line: &String| {
        let kind = words(line).first().copied()?;
        PARTS.iter().position(|(_, kinds)| kinds.contains(&kind))
    };
    let mut parts: Vec<Option<usize>> = vec![None; lines.len()];
    let mut next = None;
    for (at, line) in lines.iter().enumerate().rev() {
        if is_record(line) {
            next = part(line).or(next);
            parts[at] = next;
        } else if !line.trim().is_empty() {
            parts[at] = next;
        }
    }
    // What comes after the last record goes with it.
    let mut last = None;
    for (at, line) in lines.iter().enumerate() {
        last = parts[at].or(last);
        if parts[at].is_none() && !line.trim().is_empty() {
            parts[at] = last;
        }
    }
    parts
}

/// The words that tell a record from the others of its kind: `pat hat arena`, `sec verse`,
/// or `tempo` alone.
pub fn name(line: &str) -> String {
    let words = words(line);
    let count = match words.first() {
        Some(&"pat") => 3,
        Some(&("motif" | "turn" | "sec" | "play" | "threat" | "mix" | "sound")) => 2,
        _ => 1,
    };
    words[..count.min(words.len())].join(" ")
}

/// Whether a line is a record: neither empty nor a comment.
pub fn is_record(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty() && !line.starts_with('#')
}

/// The words of a line.
pub fn words(line: &str) -> Vec<&str> {
    line.split_whitespace().collect()
}

/// Where each word of a line starts and ends.
fn spans(line: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (at, letter) in line.char_indices() {
        match (letter.is_whitespace(), start) {
            (false, None) => start = Some(at),
            (true, Some(from)) => {
                spans.push((from, at));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        spans.push((from, line.len()));
    }
    spans
}

/// The words of a line, each with the column where it starts: how many characters stand
/// before it.
pub fn columns(line: &str) -> Vec<(usize, &str)> {
    let column = |start: usize| line[..start].chars().count();
    let spans = spans(line).into_iter();
    spans
        .map(|(start, end)| (column(start), &line[start..end]))
        .collect()
}

/// Whether a word is made of figures and of the symbols of a bar only: it is then read
/// character by character, each in its column.
pub fn is_figures(word: &str) -> bool {
    word.chars()
        .all(|letter| "0123456789.-xoROV|%".contains(letter))
}

/// A line with one of its words replaced. Where more than one space came after the word, the
/// next word was lined up by hand: it stays in its column if the new word leaves room for a
/// space before it.
pub fn with_word(line: &str, index: usize, word: &str) -> String {
    let spans = spans(line);
    let Some(&(start, end)) = spans.get(index) else {
        return line.to_string();
    };
    let mut out = format!("{}{word}", &line[..start]);
    if let Some(&(next, _)) = spans.get(index + 1) {
        let gap = if next - end > 1 {
            next.saturating_sub(start + word.len()).max(1)
        } else {
            1
        };
        out.push_str(&" ".repeat(gap));
        out.push_str(&line[next..]);
    }
    out
}

/// A line without one of its words, and without the space before it.
pub fn without_word(line: &str, index: usize) -> String {
    let spans = spans(line);
    let Some(&(start, end)) = spans.get(index) else {
        return line.to_string();
    };
    let before = line[..start].trim_end();
    format!("{before}{}", &line[end..]).trim_end().to_string()
}

/// A line with a word more at its end.
pub fn with_last(line: &str, word: &str) -> String {
    format!("{} {word}", line.trim_end())
}

/// The line that a record starts, among some lines: the first one whose first words are
/// those of `head`, such as `["pat", "bass", "leaping"]`.
pub fn find(lines: &[String], head: &[&str]) -> Option<usize> {
    lines.iter().position(|line| {
        is_record(line) && {
            let words = words(line);
            words.len() >= head.len() && words[..head.len()] == *head
        }
    })
}

/// Every line of a kind, by its place among the lines.
pub fn of_kind(lines: &[String], kind: &str) -> Vec<usize> {
    let is = |line: &String| is_record(line) && words(line).first() == Some(&kind);
    (0..lines.len()).filter(|&at| is(&lines[at])).collect()
}

/// Puts a new record among the lines: after the last one of its kind, or else after the
/// records of the kinds that come before it, in a block of its own.
pub fn insert(lines: &mut Vec<String>, record: String) {
    let kind = words(&record)
        .first()
        .copied()
        .unwrap_or_default()
        .to_string();
    if let Some(&last) = of_kind(lines, &kind).last() {
        lines.insert(last + 1, record);
        return;
    }
    let place = ORDER.iter().position(|known| *known == kind);
    let before = &ORDER[..place.unwrap_or(ORDER.len())];
    let last = before
        .iter()
        .filter_map(|kind| of_kind(lines, kind).last().copied())
        .max();
    match last {
        Some(last) if SAME_BLOCK.contains(&kind.as_str()) => lines.insert(last + 1, record),
        Some(last) => {
            lines.insert(last + 1, String::new());
            lines.insert(last + 2, record);
        }
        None => lines.push(record),
    }
}

/// Takes a record away, and the empty line that it leaves if it was alone in its block.
pub fn remove(lines: &mut Vec<String>, at: usize) {
    lines.remove(at);
    let empty = |line: Option<&String>| line.is_none_or(|line| line.trim().is_empty());
    // Two empty lines in a row, or one at the end of the file.
    if at > 0 && empty(lines.get(at - 1)) && empty(lines.get(at)) {
        lines.remove(at - 1);
    }
}

/// One bar as the file writes it: four words of four steps.
pub fn bar(grid: &[u8; STEPS]) -> String {
    let beats = grid.chunks(4).map(|beat| String::from_utf8_lossy(beat));
    beats.collect::<Vec<_>>().join(" ")
}

/// The symbols of a bar of melody: a digit where a note starts, dots while it lasts, and a
/// dash wherever nothing sounds.
pub fn symbols(line: &Line) -> [u8; STEPS] {
    let mut grid = [b'-'; STEPS];
    for (step, cell) in line.iter().enumerate() {
        if cell.degree != 0 {
            grid[step] = b'0' + cell.degree;
            let end = (step + cell.steps as usize).min(STEPS);
            grid[step + 1..end].fill(b'.');
        }
    }
    grid
}

/// The note that sounds on a step of a bar, by the step where it starts.
pub fn note_at(line: &Line, step: usize) -> Option<usize> {
    let starts = (0..=step).rev().find(|&from| line[from].degree != 0)?;
    (starts + line[starts].steps as usize > step).then_some(starts)
}

/// Puts a note into a bar of melody. A melody has one note at a time: the note that sounded
/// there is cut short, and those that started under the new one are gone. The new note ends
/// with its bar at the latest.
pub fn place(line: &mut Line, step: usize, degree: u8, steps: u8) {
    let steps = (steps.max(1) as usize).min(STEPS - step);
    if let Some(before) = note_at(line, step)
        && before < step
    {
        line[before].steps = (step - before) as u8;
    }
    for cell in &mut line[step..step + steps] {
        *cell = Cell::default();
    }
    line[step] = Cell {
        degree,
        steps: steps as u8,
    };
}

/// Takes away the note that sounds on a step, if one does.
pub fn erase(line: &mut Line, step: usize) {
    if let Some(start) = note_at(line, step) {
        line[start] = Cell::default();
    }
}

/// Steps that a note could last from where it starts: up to the next note, or to the end of
/// its bar.
pub fn room(line: &Line, start: usize) -> u8 {
    let next = (start + 1..STEPS).find(|&step| line[step].degree != 0);
    (next.unwrap_or(STEPS) - start) as u8
}

/// A motif as its line: its letter, then its bars with a `|` between two.
pub fn motif(letter: char, bars: &[Line]) -> String {
    let bars = bars.iter().map(|line| bar(&symbols(line)));
    format!("motif {letter} {}", bars.collect::<Vec<_>>().join(" | "))
}

/// The turn of a motif as its line.
pub fn turn(letter: char, line: &Line) -> String {
    format!("turn  {letter} {}", bar(&symbols(line)))
}

/// A pattern as its line: instrument, name, bar, the level of a hit, then that of a soft hit
/// if the bar has some, and `echo` for a pattern of the pluck that has its echo.
pub fn pattern(
    instrument: Instrument,
    name: &str,
    grid: &[u8; STEPS],
    (level, soft): (u8, u8),
    echo: bool,
) -> String {
    let mut line = format!(
        "pat {:<5} {name:<8} {} {level}",
        instrument.name(),
        bar(grid)
    );
    if grid.contains(&b'o') {
        line.push_str(&format!(" {}", soft.max(1)));
    }
    if echo && instrument == Instrument::Pluck {
        line.push_str(" echo");
    }
    line
}

/// A section as its line: its name, a chord per bar in two words of four, the motifs that its
/// melody states, and its tempo if it has one of its own.
pub fn section(name: &str, chords: &[u8; 8], statements: &[String], tempo: Option<u16>) -> String {
    let chord = |chord: &u8| match chord {
        0 => 'C',
        degree => (b'0' + degree) as char,
    };
    let (first, second): (String, String) = (
        chords[..4].iter().map(chord).collect(),
        chords[4..].iter().map(chord).collect(),
    );
    let mut line = format!("sec {name:<6} {first} {second}  {}", statements.join(" "));
    if let Some(tempo) = tempo {
        line.push_str(&format!("  {tempo}"));
    }
    line
}

/// The mix of an instrument as its line: its level on the left and on the right, in percent,
/// each in its column.
pub fn mix(instrument: Instrument, left: &str, right: &str) -> String {
    format!("mix {:<5} {left:>3} {right:>3}", instrument.name())
}

/// A number as the file writes it before a unit: without a point if it is whole, and with
/// four digits after its point at most.
pub fn number(value: f32) -> String {
    let rounded = (value as f64 * 10_000.0).round() / 10_000.0;
    let text = format!("{rounded:.4}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    text.to_string()
}

#[cfg(test)]
mod tests {
    use tanku_audio::score::PIECE;

    #[test]
    fn every_line_of_the_file_has_its_part() {
        let lines: Vec<String> = FILE.lines().map(str::to_string).collect();
        let parts = parts(&lines);
        let count = |part: usize| parts.iter().filter(|other| **other == Some(part)).count();
        let counts: Vec<usize> = (0..PARTS.len()).map(count).collect();
        // The comment at the top goes with the heading, and the nine empty lines with none.
        assert_eq!(counts, [4, 4, 12, 4, 9, 2, 6, 6]);
        assert_eq!(
            parts.iter().filter(|part| part.is_none()).count(),
            lines.len() - 47
        );
        assert_eq!((parts[0], parts[1], parts[2]), (Some(0), Some(0), None));
        // A comment before a record is of its part, and one after the last record too.
        let lines: Vec<String> = ["tempo 120", "", "# the tunes", "motif A 1...", "# the end"]
            .map(str::to_string)
            .into();
        assert_eq!(
            super::parts(&lines),
            [Some(0), None, Some(1), Some(1), Some(1)]
        );
        assert_eq!(
            name("pat hat   arena    ooxo ooxo ooxo ooxo 200 100"),
            "pat hat arena"
        );
        assert_eq!(name("sec verse  11C4 11C5  A A A a"), "sec verse");
        assert_eq!(name("tempo 120"), "tempo");
        assert_eq!(name(""), "");
        // The words of a line stand in its columns, and a bar is read symbol by symbol.
        let words = columns("pat hat   arena    ooxo ooxo");
        assert_eq!(
            words,
            [
                (0, "pat"),
                (4, "hat"),
                (10, "arena"),
                (19, "ooxo"),
                (24, "ooxo")
            ]
        );
        assert!(
            ["ooxo", "R..O", "5-5.", "|", "255", "0.4%"]
                .into_iter()
                .all(is_figures)
        );
        assert!(
            !["hat", "A", "11C4", "110Hz", "C:7", "bass=*"]
                .into_iter()
                .any(is_figures)
        );
    }

    use super::*;

    fn lines(text: &str) -> Vec<String> {
        text.lines().map(str::to_string).collect()
    }

    /// The file of the game.
    const FILE: &str = include_str!("../../tanku_audio/music.score");

    #[test]
    fn a_word_is_replaced_where_it_stands() {
        let line = "pat bass  leaping  R..O ..R. R..O ..V. 255";
        // A word between two single spaces: nothing is lined up there.
        assert_eq!(
            with_word("roll 2 hat snare 40 255", 2, "kick"),
            "roll 2 kick snare 40 255"
        );
        // The same length: nothing else moves.
        assert_eq!(
            with_word(line, 3, "R..R"),
            "pat bass  leaping  R..R ..R. R..O ..V. 255"
        );
        // A shorter name leaves the bar in its column, and a longer one pushes it by as
        // little as it can.
        assert_eq!(
            with_word(line, 2, "low"),
            "pat bass  low      R..O ..R. R..O ..V. 255"
        );
        assert_eq!(
            with_word(line, 2, "leapingest"),
            "pat bass  leapingest R..O ..R. R..O ..V. 255"
        );
        // The last word, and a word that isn't there.
        assert_eq!(with_word("tempo 120", 1, "96"), "tempo 96");
        assert_eq!(with_word("tempo 120", 4, "96"), "tempo 120");
    }

    #[test]
    fn a_word_goes_and_a_word_comes() {
        let line = "threat 1  bass=plain@2    pluck=arpeggio@3  lead=skeleton@4";
        assert_eq!(
            without_word(line, 4),
            "threat 1  bass=plain@2    pluck=arpeggio@3"
        );
        assert_eq!(
            without_word(line, 2),
            "threat 1    pluck=arpeggio@3  lead=skeleton@4"
        );
        assert_eq!(
            with_last("play menu intro  ", "kick=fight"),
            "play menu intro kick=fight"
        );
    }

    #[test]
    fn records_are_found_by_their_first_words() {
        let lines = lines(FILE);
        // The comment of the first line is no record.
        assert_eq!(find(&lines, &["tanku-music"]), Some(1));
        assert_eq!(find(&lines, &["motif", "B"]), Some(7));
        assert_eq!(find(&lines, &["pat", "hat", "arena"]), Some(14));
        assert_eq!(find(&lines, &["pat", "snare", "arena"]), Some(15));
        assert_eq!(find(&lines, &["sound", "lead"]), Some(55));
        assert_eq!(find(&lines, &["pat", "lead", "arena"]), None);
        assert_eq!(of_kind(&lines, "play"), [29, 30, 31, 32]);
        assert_eq!(of_kind(&lines, "turn"), [9]);
    }

    #[test]
    fn a_new_record_goes_with_those_of_its_kind() {
        let mut file = lines(FILE);
        insert(&mut file, "motif D 1... .... .... ....".into());
        // After the three motifs, before the turn.
        assert_eq!(find(&file, &["motif", "D"]), Some(9));
        assert_eq!(find(&file, &["turn", "A"]), Some(10));

        // The first record of its kind stands after the kinds before it: a turn right under
        // the motifs, a pattern in a block of its own.
        let mut small = lines("tanku-music 1\ntempo 96\n\nmotif A 1... .... .... ....\n");
        insert(&mut small, "turn  A 2... .... .... ....".into());
        insert(&mut small, "pat kick four x... x... x... x... 200".into());
        insert(&mut small, "key A dorian aeolian".into());
        assert_eq!(
            small,
            [
                "tanku-music 1",
                "tempo 96",
                "key A dorian aeolian",
                "",
                "motif A 1... .... .... ....",
                "turn  A 2... .... .... ....",
                "",
                "pat kick four x... x... x... x... 200",
            ]
        );
        // A record that goes leaves no hole.
        let pattern = find(&small, &["pat"]).unwrap();
        remove(&mut small, pattern);
        assert_eq!(small.len(), 6);
        assert_eq!(small.last().unwrap(), "turn  A 2... .... .... ....");
    }

    #[test]
    fn the_records_of_the_game_are_written_as_its_file_has_them() {
        let file = lines(FILE);
        // Motifs and their turn.
        for motif in PIECE.motifs() {
            let letter = motif.letter as char;
            let written = &file[find(&file, &["motif", &letter.to_string()]).unwrap()];
            assert_eq!(&super::motif(letter, motif.bars()), written);
            if let Some(turned) = &motif.turn {
                let written = &file[find(&file, &["turn", &letter.to_string()]).unwrap()];
                assert_eq!(&turn(letter, turned), written);
            }
        }
        // Patterns.
        for rhythm in PIECE.rhythms() {
            let (instrument, name) = (rhythm.instrument, rhythm.name.as_str());
            let written = &file[find(&file, &["pat", instrument.name(), name]).unwrap()];
            let levels = (rhythm.level, rhythm.soft);
            assert_eq!(
                &pattern(instrument, name, &rhythm.grid, levels, rhythm.echo),
                written
            );
        }
        // Sections, whose statements the line itself gives back.
        for passage in PIECE.passages() {
            let name = passage.name.as_str();
            let written = &file[find(&file, &["sec", name]).unwrap()];
            let words = words(written);
            let own = words.last().and_then(|word| word.parse::<u16>().ok());
            let last = words.len() - own.is_some() as usize;
            let statements: Vec<String> =
                words[4..last].iter().map(|word| word.to_string()).collect();
            assert_eq!(&section(name, &passage.chords, &statements, own), written);
        }
    }

    #[test]
    fn the_mix_of_the_game_is_written_as_its_file_has_it() {
        let file = lines(FILE);
        for instrument in Instrument::ALL {
            let written = &file[find(&file, &["mix", instrument.name()]).unwrap()];
            let levels: Vec<&str> = words(written)[2..].to_vec();
            assert_eq!(&mix(instrument, levels[0], levels[1]), written);
        }
        assert_eq!(
            mix(Instrument::Lead, "12.5%", "100%"),
            "mix lead  12.5% 100%"
        );
    }

    #[test]
    fn a_melody_has_one_note_at_a_time() {
        // The second bar of motif A: 3..5 ..6. .... 5...
        let mut line = PIECE.motif(b'A').unwrap().bars()[1];
        assert_eq!(bar(&symbols(&line)), "3..5 ..6. .... 5...");
        assert_eq!(note_at(&line, 0), Some(0));
        assert_eq!(note_at(&line, 5), Some(3));
        assert_eq!(note_at(&line, 11), Some(6));
        assert_eq!(room(&line, 6), 6);

        // A note in the middle of the long one cuts it short, and lasts as long as it is
        // asked to.
        place(&mut line, 8, 2, 2);
        assert_eq!(bar(&symbols(&line)), "3..5 ..6. 2.-- 5...");
        // A note over two others takes their place, and stops at the end of the bar.
        place(&mut line, 10, 9, 20);
        assert_eq!(bar(&symbols(&line)), "3..5 ..6. 2.9. ....");
        assert_eq!(note_at(&line, 15), Some(10));
        // A note that goes leaves silence.
        erase(&mut line, 4);
        assert_eq!(bar(&symbols(&line)), "3..- --6. 2.9. ....");
        erase(&mut line, 4);
        assert_eq!(note_at(&line, 4), None);
    }

    #[test]
    fn a_number_is_written_as_short_as_it_is() {
        assert_eq!(number(120.0), "120");
        assert_eq!(number(5.5), "5.5");
        assert_eq!(number(0.4), "0.4");
        assert_eq!(number(12.25), "12.25");
        assert_eq!(number(0.1), "0.1");
        assert_eq!(number(33.0), "33");
    }
}
