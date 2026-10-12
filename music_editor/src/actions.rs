//! What a press does: to the view, or to the lines of the document.
//!
//! Every change of the piece is a change of some words of the file, which the document only
//! takes if the reader of `tanku_audio` reads the result. So nothing here checks a bound for
//! its own sake: it keeps numbers in the ranges that the window offers, and the reader says
//! the rest, in its own words, which the window shows.

use tanku_audio::notation::{
    Cell, Form, Instrument, Line, Part, Piece, Rhythm, STEPS, Scene, THREATS,
};

use crate::{
    doc::Doc,
    lines,
    parts::{self, PartText, SectionLine, Statement, Voice},
    state::{Ending, Field, Picked, Press, Row, Tool, Typing, View, Way},
};

/// The tonics, from A up, as the file names them.
pub const TONICS: [&str; 12] = [
    "A", "A#", "B", "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#",
];
const DRUMS: [Instrument; 3] = [Instrument::Kick, Instrument::Hat, Instrument::Snare];

/// A number moved by a step and kept between two bounds.
fn moved(value: i64, by: i64, least: i64, most: i64) -> i64 {
    (value + by).clamp(least, most)
}

/// Does what a press asks. `big` is for the larger step of a number: the Shift key is held.
pub fn apply(press: &Press, big: bool, doc: &mut Doc, view: &mut View) {
    doc.refusal = None;
    // A click anywhere else ends what was being typed, as Enter does.
    if view.typing.is_some() && !matches!(press, Press::Type(_)) {
        commit(doc, view);
    }
    let piece = *doc.piece();
    let scale = if big { 10 } else { 1 };
    match press {
        Press::Tab(tab) => view.tab = *tab,
        Press::Undo => {
            doc.undo();
        }
        Press::Redo => {
            doc.redo();
        }
        Press::Save => {
            if let Err(why) = doc.save() {
                doc.refusal = Some(why);
            }
        }
        Press::Revert => {
            doc.revert();
        }
        // The clipboard belongs to the window: the press is taken there.
        Press::Copy => {}
        Press::Play => view.playing = true,
        Press::Stop => view.playing = false,
        Press::Plays(choice) => view.plays[view.tab as usize] = *choice,
        Press::Place(place) => view.place = *place,
        Press::Fight(fight) => view.fight = *fight,
        Press::Threat(threat) => view.threat = (*threat).min(THREATS as u8 - 1),

        Press::Motif(letter) => {
            view.motif = *letter;
            view.turn = false;
            view.note = None;
        }
        Press::NewMotif => new_motif(doc, view, &piece),
        Press::RemoveMotif => {
            if let Some(letter) = view.shown_motif(&piece) {
                let letter = (letter as char).to_string();
                let removed = doc.change(|lines| {
                    for kind in ["turn", "motif"] {
                        if let Some(at) = lines::find(lines, &[kind, &letter]) {
                            lines::remove(lines, at);
                        }
                    }
                });
                if removed {
                    view.note = None;
                    view.turn = false;
                }
            }
        }
        Press::MotifBars(by) => {
            if let Some((letter, mut bars)) = shown_bars(view, &piece) {
                let count = moved(bars.len() as i64, *by as i64, 1, 4) as usize;
                bars.resize(count, [Cell::default(); STEPS]);
                if write_motif(doc, letter, &bars) {
                    view.note = view.note.filter(|(bar, _)| *bar < count);
                }
            }
        }
        Press::Turn(shown) => {
            let has = view
                .shown_motif(&piece)
                .and_then(|letter| piece.motif(letter));
            view.turn = *shown && has.is_some_and(|motif| motif.turn.is_some());
            view.note = None;
        }
        Press::HasTurn(has) => {
            if let Some((letter, bars)) = shown_bars(view, &piece) {
                let name = (letter as char).to_string();
                let done = if *has {
                    // It starts as the last bar that it takes the place of.
                    let last = bars[bars.len() - 1];
                    doc.set_line(&["turn", &name], lines::turn(letter as char, &last))
                } else {
                    doc.change(|lines| {
                        if let Some(at) = lines::find(lines, &["turn", &name]) {
                            lines::remove(lines, at);
                        }
                    })
                };
                if done {
                    view.turn = *has;
                    view.note = None;
                }
            }
        }
        Press::Tool(tool) => view.tool = *tool,
        Press::Cell(bar, step, degree) => cell(doc, view, &piece, (*bar, *step), *degree),
        Press::NoteDegree(by) => {
            change_note(doc, view, &piece, |line, start| {
                line[start].degree = moved(line[start].degree as i64, *by as i64, 1, 9) as u8;
            });
        }
        Press::NoteLength(by) => {
            let mut length = None;
            change_note(doc, view, &piece, |line, start| {
                let room = lines::room(line, start) as i64;
                line[start].steps = moved(line[start].steps as i64, *by as i64, 1, room) as u8;
                length = Some(line[start].steps);
            });
            view.length = length.unwrap_or(view.length);
        }

        Press::PatternOf(instrument) => {
            view.pattern_of = *instrument;
            let first = piece
                .rhythms()
                .iter()
                .find(|rhythm| rhythm.instrument == *instrument);
            if let Some(rhythm) = first {
                view.pattern = Some((*instrument, rhythm.name.as_str().to_string()));
            }
        }
        Press::Pattern(instrument, name) => {
            view.pattern = Some((*instrument, name.clone()));
            view.pattern_of = *instrument;
        }
        Press::NewPattern => new_pattern(doc, view, &piece),
        Press::RemovePattern => {
            if let Some((instrument, name)) = view.shown_pattern(&piece) {
                let head = ["pat", instrument.name(), &name];
                if let Some(at) = doc.find(&head) {
                    doc.change(|lines| lines::remove(lines, at));
                }
            }
        }
        Press::Brush(symbol) => view.brush = *symbol,
        Press::Step(instrument, name, step) => {
            view.pattern = Some((*instrument, name.clone()));
            view.pattern_of = *instrument;
            if let Some(mut rhythm) = piece.rhythm(*instrument, name) {
                let symbol = &mut rhythm.grid[*step];
                *symbol = if *symbol == view.brush {
                    b'.'
                } else {
                    view.brush
                };
                write_pattern(doc, &rhythm);
            }
        }
        Press::Level(by) | Press::Soft(by) => {
            let shown = view.shown_pattern(&piece);
            if let Some(mut rhythm) =
                shown.and_then(|(instrument, name)| piece.rhythm(instrument, &name))
            {
                let by = *by as i64 * if big { 5 } else { 1 };
                let level = if matches!(press, Press::Level(_)) {
                    &mut rhythm.level
                } else {
                    &mut rhythm.soft
                };
                *level = moved(*level as i64, by, 1, 255) as u8;
                write_pattern(doc, &rhythm);
            }
        }
        Press::Echo => {
            let shown = view.shown_pattern(&piece);
            if let Some(mut rhythm) =
                shown.and_then(|(instrument, name)| piece.rhythm(instrument, &name))
            {
                rhythm.echo = !rhythm.echo;
                write_pattern(doc, &rhythm);
            }
        }
        Press::Over(bar) => view.over = (*bar).min(7),

        Press::Section(name) => {
            view.section = name.clone();
            view.bar = 0;
        }
        Press::NewSection => new_section(doc, view, &piece),
        Press::RemoveSection => {
            if let Some(name) = view.shown_section(&piece)
                && let Some(at) = doc.find(&["sec", &name])
            {
                doc.change(|lines| lines::remove(lines, at));
            }
        }
        Press::Bar(bar) => view.bar = (*bar).min(7),
        Press::Chord(chord) => {
            change_section(doc, view, &piece, |section, _| {
                section.chords[view.bar] = *chord;
            });
        }
        Press::States(letter) => {
            let bar = view.bar;
            change_section(doc, view, &piece, |section, piece| {
                restate(section, piece, bar, *letter);
            });
        }
        Press::Ends(ending) => {
            let bar = view.bar;
            change_section(doc, view, &piece, |section, piece| {
                let Some((place, ..)) = section.stating(piece, bar) else {
                    return;
                };
                let statement = &mut section.statements[place];
                match ending {
                    Ending::Written => (statement.turned, statement.last) = (false, None),
                    Ending::Turned => (statement.turned, statement.last) = (true, None),
                    Ending::Degree(by) => {
                        let written = last_degree(piece, statement.letter).unwrap_or(1);
                        let from = statement.last.unwrap_or(written) as i64;
                        statement.turned = false;
                        statement.last = Some(moved(from, *by as i64, 1, 9) as u8);
                    }
                }
            });
        }
        Press::OwnTempo(own) => {
            let tempo = piece.tempo;
            change_section(doc, view, &piece, |section, _| {
                section.tempo = own.then_some(section.tempo.unwrap_or(tempo));
            });
        }
        Press::SectionTempo(by) => {
            change_section(doc, view, &piece, |section, _| {
                if let Some(tempo) = &mut section.tempo {
                    *tempo = moved(*tempo as i64, (*by * scale) as i64, 30, 300) as u16;
                }
            });
        }
        Press::Pick(picked) => {
            view.picked = Some(*picked);
            view.second = false;
        }
        Press::Second(second) => view.second = *second,
        Press::PartLine(by) => change_part(doc, view, &piece, |part, row, instrument, second| {
            next_line(part, &piece, row, instrument, second, *by);
        }),
        Press::PartOctave(by) => change_part(doc, view, &piece, |part, _, _, second| {
            if let Some(Voice {
                octave: Some(octave),
                ..
            }) = voice_of(part, second)
            {
                *octave = moved(*octave as i64, *by as i64, 1, 5) as u8;
            }
        }),
        Press::PartLevel(by) => change_part(doc, view, &piece, |part, _, _, second| {
            if let Some(voice) = voice_of(part, second)
                && voice.is_tune()
            {
                let by = *by as i64 * if big { 25 } else { 5 };
                let level = moved(voice.loud() as i64, by, 1, 255) as u8;
                // A melody that is as loud as can be says nothing of its level.
                voice.level = (level < 255).then_some(level);
            }
        }),
        Press::Together(together) => change_part(doc, view, &piece, |part, _, _, _| {
            if let PartText::Plays { together: now, .. } = part {
                *now = *together;
            }
        }),
        Press::Turns(place, by) => {
            if let Some(Picked::Sections(scene)) = view.picked {
                turns(doc, &piece, scene, *place, *by);
            }
        }

        Press::Way(way) => view.way = *way,
        Press::WayNumber(word, by) => {
            let head = if view.way == Way::Roll {
                "roll"
            } else {
                "link"
            };
            let words = doc.record(&[head]).unwrap_or_default();
            let numbers: Vec<i64> = words.iter().map(|word| word.parse().unwrap_or(0)).collect();
            if let Some(&value) = numbers.get(*word) {
                // The levels of a roll move faster than its beats.
                let level = view.way == Way::Roll && *word >= 4;
                let by = *by as i64 * if level { if big { 25 } else { 5 } } else { 1 };
                let most = match (view.way, *word) {
                    (_, 1) => 4,
                    (Way::Link, 2) => 9,
                    (Way::Link, _) => 2 * numbers[1],
                    (Way::Roll, _) => 255,
                };
                let next = moved(value, by, 1, most);
                doc.change(|lines| {
                    let Some(at) = lines::find(lines, &[head]) else {
                        return;
                    };
                    lines[at] = lines::with_word(&lines[at], *word, &next.to_string());
                    // A connection that gets shorter has room for fewer notes.
                    if head == "link" && *word == 1 && numbers[3] > 2 * next {
                        lines[at] = lines::with_word(&lines[at], 3, &(2 * next).to_string());
                    }
                });
            }
        }
        Press::WayDrum(word) => {
            let words = doc.record(&["roll"]).unwrap_or_default();
            let now = words.get(*word).copied().unwrap_or_default();
            let place = DRUMS
                .iter()
                .position(|drum| drum.name() == now)
                .unwrap_or(0);
            let next = DRUMS[(place + 1) % DRUMS.len()].name();
            doc.set_word(&["roll"], *word, next);
        }

        Press::Instrument(instrument) => view.instrument = *instrument,
        Press::Setting(word, by) => {
            let head = ["sound", view.instrument.name()];
            let line = doc
                .find(&head)
                .map(|at| doc.lines()[at].clone())
                .unwrap_or_default();
            let dials = parts::dials(&line, view.instrument);
            if let Some(dial) = dials.iter().find(|dial| dial.word == *word) {
                let next = parts::stepped(dial, *by > 0);
                doc.set_word(&head, *word, &parts::written(next, dial.knob.unit));
            }
        }
        Press::Mix(side, by) => {
            let head = ["mix", view.instrument.name()];
            let words = doc.record(&head).unwrap_or_default();
            let level = words
                .get(2 + side)
                .and_then(|word| parts::value(word, parts::Unit::Percent));
            if let Some(level) = level {
                let by = *by as f32 * if big { 5.0 } else { 1.0 };
                let next = parts::written((level + by).clamp(0.0, 100.0), parts::Unit::Percent);
                let mut levels = [words[2].to_string(), words[3].to_string()];
                levels[*side] = next;
                doc.set_line(&head, lines::mix(view.instrument, &levels[0], &levels[1]));
            }
        }

        Press::Tempo(by) => {
            let tempo = moved(piece.tempo as i64, (*by * scale) as i64, 30, 300);
            doc.set_word(&["tempo"], 1, &tempo.to_string());
        }
        Press::Tonic(tonic) => {
            doc.set_word(&["key"], 1, TONICS[*tonic as usize % 12]);
        }
        Press::Home(mode) => {
            doc.set_word(&["key"], 2, mode.name());
        }
        Press::Arena(mode) => {
            doc.set_word(&["key"], 3, mode.name());
        }

        Press::Type(field) => {
            if view
                .typing
                .as_ref()
                .is_some_and(|typing| typing.field != *field)
            {
                commit(doc, view);
            }
            let text = match field {
                Field::PatternName => view.shown_pattern(&piece).map(|(_, name)| name),
                Field::SectionName => view.shown_section(&piece),
            };
            view.typing = text.map(|text| Typing {
                field: *field,
                text,
            });
        }
    }
}

/// What a key does to the text that is typed. Enter ends it, and Escape gives it up.
pub fn typed(doc: &mut Doc, view: &mut View, key: Key) {
    let Some(typing) = &mut view.typing else {
        return;
    };
    match key {
        Key::Letter(letter) => {
            // A name of the file: twelve letters at most, without a space.
            let fits = letter.is_ascii_alphanumeric() || letter == '_';
            if fits && typing.text.len() < 12 {
                typing.text.push(letter.to_ascii_lowercase());
            }
        }
        Key::Back => {
            typing.text.pop();
        }
        Key::Enter => commit(doc, view),
        Key::Escape => view.typing = None,
    }
}

/// A key of a text field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Letter(char),
    Back,
    Enter,
    Escape,
}

/// Gives the thing that was typed its new name, wherever the file names it.
fn commit(doc: &mut Doc, view: &mut View) {
    let Some(Typing { field, text }) = view.typing.take() else {
        return;
    };
    let piece = *doc.piece();
    if text.is_empty() {
        return;
    }
    match field {
        Field::PatternName => {
            let Some((instrument, old)) = view.shown_pattern(&piece) else {
                return;
            };
            if old == text {
                return;
            }
            let renamed = doc.change(|lines| {
                let Some(at) = lines::find(lines, &["pat", instrument.name(), &old]) else {
                    return;
                };
                let Some(rhythm) = piece.rhythm(instrument, &old) else {
                    return;
                };
                let levels = (rhythm.level, rhythm.soft);
                lines[at] = lines::pattern(instrument, &text, &rhythm.grid, levels, rhythm.echo);
                // Every line of the arrangement that plays it.
                for kind in ["play", "threat"] {
                    for at in lines::of_kind(lines, kind) {
                        let mut part = PartText::of(&lines[at], instrument);
                        if let PartText::Plays { first, second, .. } = &mut part {
                            for voice in std::iter::once(first).chain(second) {
                                if voice.name == old {
                                    voice.name = text.clone();
                                }
                            }
                            lines[at] = parts::with_part(&lines[at], instrument, &part);
                        }
                    }
                }
            });
            if renamed {
                view.pattern = Some((instrument, text));
            }
        }
        Field::SectionName => {
            let Some(old) = view.shown_section(&piece) else {
                return;
            };
            if old == text {
                return;
            }
            let renamed = doc.change(|lines| {
                let Some(at) = lines::find(lines, &["sec", &old]) else {
                    return;
                };
                let Some(mut section) = SectionLine::parse(&lines[at]) else {
                    return;
                };
                section.name = text.clone();
                lines[at] = section.write();
                // Every scene that plays it.
                for at in lines::of_kind(lines, "play") {
                    let words = lines::words(&lines[at]);
                    let Some(played) = words.get(2) else {
                        continue;
                    };
                    let names = played
                        .split(',')
                        .map(|name| if name == old { &text } else { name });
                    let played = names.collect::<Vec<_>>().join(",");
                    lines[at] = lines::with_word(&lines[at], 2, &played);
                }
            });
            if renamed {
                view.section = text;
            }
        }
    }
}

/// The motif that is shown and its bars.
fn shown_bars(view: &View, piece: &Piece) -> Option<(u8, Vec<Line>)> {
    let letter = view.shown_motif(piece)?;
    Some((letter, piece.motif(letter)?.bars().to_vec()))
}

fn write_motif(doc: &mut Doc, letter: u8, bars: &[Line]) -> bool {
    let name = (letter as char).to_string();
    doc.set_line(&["motif", &name], lines::motif(letter as char, bars))
}

fn new_motif(doc: &mut Doc, view: &mut View, piece: &Piece) {
    let free = ('A'..='Z').find(|letter| piece.motif(*letter as u8).is_none());
    let Some(letter) = free else {
        doc.refusal = Some("every letter names a motif already".to_string());
        return;
    };
    let silence = [Cell::default(); STEPS];
    if write_motif(doc, letter as u8, &[silence]) {
        view.motif = letter as u8;
        view.turn = false;
        view.note = None;
    }
}

/// The bar that the grid of the motif shows: one of its bars, or its turn.
fn shown_line(view: &View, piece: &Piece, bar: usize) -> Option<(u8, Line)> {
    let letter = view.shown_motif(piece)?;
    let motif = piece.motif(letter)?;
    let line = if view.turn {
        motif.turn?
    } else {
        *motif.bars().get(bar)?
    };
    Some((letter, line))
}

/// Writes a bar of the motif that is shown, or its turn.
fn write_line(doc: &mut Doc, view: &View, piece: &Piece, bar: usize, line: Line) -> bool {
    let Some((letter, mut bars)) = shown_bars(view, piece) else {
        return false;
    };
    if view.turn {
        let name = (letter as char).to_string();
        return doc.set_line(&["turn", &name], lines::turn(letter as char, &line));
    }
    bars[bar] = line;
    write_motif(doc, letter, &bars)
}

/// A click on the grid of a motif.
fn cell(doc: &mut Doc, view: &mut View, piece: &Piece, (bar, step): (usize, usize), degree: u8) {
    let Some((_, mut line)) = shown_line(view, piece, bar) else {
        return;
    };
    let under = lines::note_at(&line, step).filter(|start| line[*start].degree == degree);
    match (view.tool, under) {
        // A click on a note picks it, whatever the tool but the one that takes notes away.
        (Tool::Draw | Tool::Select, Some(start)) => {
            view.note = Some((bar, start));
            view.length = line[start].steps;
        }
        (Tool::Select, None) => view.note = None,
        (Tool::Erase, Some(start)) => {
            lines::erase(&mut line, start);
            if write_line(doc, view, piece, bar, line) {
                view.note = None;
            }
        }
        (Tool::Erase, None) => {}
        (Tool::Draw, None) => {
            if line[step].degree != 0 {
                // Another note starts here: it moves to this degree.
                line[step].degree = degree;
            } else {
                let next = (step + 1..STEPS).find(|&later| line[later].degree != 0);
                let room = next.unwrap_or(STEPS) - step;
                lines::place(&mut line, step, degree, view.length.min(room as u8));
            }
            if write_line(doc, view, piece, bar, line) {
                view.note = Some((bar, step));
            }
        }
    }
}

/// Changes the note that is selected.
fn change_note(
    doc: &mut Doc,
    view: &mut View,
    piece: &Piece,
    change: impl FnOnce(&mut Line, usize),
) {
    let Some((bar, start)) = view.note else {
        return;
    };
    let Some((_, mut line)) = shown_line(view, piece, bar) else {
        return;
    };
    if line[start].degree == 0 {
        view.note = None;
        return;
    }
    change(&mut line, start);
    write_line(doc, view, piece, bar, line);
}

fn write_pattern(doc: &mut Doc, rhythm: &Rhythm) -> bool {
    let name = rhythm.name.as_str();
    // A pattern that gets its first soft hit starts it at half the level of a hit.
    let soft = if rhythm.soft == 0 {
        (rhythm.level / 2).max(1)
    } else {
        rhythm.soft
    };
    let line = lines::pattern(
        rhythm.instrument,
        name,
        &rhythm.grid,
        (rhythm.level, soft),
        rhythm.echo,
    );
    doc.set_line(&["pat", rhythm.instrument.name(), name], line)
}

/// A name that nothing of a kind has yet: `new`, then `new2`, and so on.
fn free_name(taken: impl Fn(&str) -> bool) -> String {
    let names = (1..).map(|number| match number {
        1 => "new".to_string(),
        number => format!("new{number}"),
    });
    names.take(99).find(|name| !taken(name)).unwrap_or_default()
}

fn new_pattern(doc: &mut Doc, view: &mut View, piece: &Piece) {
    let instrument = match view.pattern_of {
        Instrument::Lead => Instrument::Pluck,
        instrument => instrument,
    };
    let name = free_name(|name| piece.rhythm(instrument, name).is_some());
    let line = lines::pattern(instrument, &name, &[b'.'; STEPS], (200, 0), false);
    if doc.change(|lines| lines::insert(lines, line)) {
        view.pattern = Some((instrument, name));
    }
}

fn new_section(doc: &mut Doc, view: &mut View, piece: &Piece) {
    // The first motif that fills eight bars by itself.
    let fills = |motif: &&tanku_audio::notation::Motif| 8 % motif.bars().len() == 0;
    let Some(motif) = piece.motifs().iter().find(fills) else {
        doc.refusal = Some("no motif fills the eight bars of a section by itself".to_string());
        return;
    };
    let name = free_name(|name| piece.passage(name).is_some());
    let statement = (motif.letter as char).to_string();
    let statements = vec![statement; 8 / motif.bars().len()];
    let line = lines::section(&name, &[1; 8], &statements, None);
    if doc.change(|lines| lines::insert(lines, line)) {
        view.section = name;
        view.bar = 0;
    }
}

/// Changes the section that is shown.
fn change_section(
    doc: &mut Doc,
    view: &View,
    piece: &Piece,
    change: impl FnOnce(&mut SectionLine, &Piece),
) {
    let Some(name) = view.shown_section(piece) else {
        return;
    };
    let head = ["sec", name.as_str()];
    let Some(at) = doc.find(&head) else {
        return;
    };
    let Some(mut section) = SectionLine::parse(&doc.lines()[at]) else {
        return;
    };
    let before = section.clone();
    change(&mut section, piece);
    if section == before {
        return;
    }
    // A tempo that comes or goes rewrites the line, and anything else changes its words
    // where they stand.
    if section.tempo.is_some() != before.tempo.is_some()
        || section.statements.len() != before.statements.len()
    {
        doc.set_line(&head, section.write());
        return;
    }
    let now = lines::words(&section.write())
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    doc.change(|lines| {
        let then: Vec<String> = lines::words(&lines[at])
            .into_iter()
            .map(str::to_string)
            .collect();
        for (place, (word, was)) in now.iter().zip(&then).enumerate() {
            if word != was {
                lines[at] = lines::with_word(&lines[at], place, word);
            }
        }
    });
}

/// The degree of the last note of a motif, as it is written.
pub fn last_degree(piece: &Piece, letter: u8) -> Option<u8> {
    let motif = piece.motif(letter)?;
    let mut cells = motif.bars().iter().flatten();
    cells.rfind(|cell| cell.degree != 0).map(|cell| cell.degree)
}

/// Gives the statement of a bar another motif. A shorter motif is stated as many times as it
/// takes to fill the bars of the one that goes, and a longer one takes the statements after
/// it that it covers.
fn restate(section: &mut SectionLine, piece: &Piece, bar: usize, letter: u8) {
    let Some((place, _, old)) = section.stating(piece, bar) else {
        return;
    };
    let Some(motif) = piece.motif(letter) else {
        return;
    };
    let new = motif.bars().len();
    let statement = Statement {
        letter,
        turned: section.statements[place].turned && motif.turn.is_some(),
        last: None,
    };
    let plain = Statement {
        turned: false,
        ..statement
    };
    if new <= old && old % new == 0 {
        // The last of them keeps the turn, which ends a section.
        let mut stated = vec![plain; old / new];
        stated[old / new - 1] = statement;
        section.statements.splice(place..=place, stated);
        return;
    }
    // The statements that the longer motif covers, if they end where it does.
    let mut covered = 0;
    let mut end = place;
    while covered < new && end < section.statements.len() {
        let bars = piece
            .motif(section.statements[end].letter)
            .map_or(1, |motif| motif.bars().len());
        covered += bars;
        end += 1;
    }
    // If they don't, the reader says that the melody no longer fills its eight bars.
    section.statements.splice(place..end, [statement]);
}

/// The line of a part that the panel shows: its first one, or its second.
fn voice_of(part: &mut PartText, second: bool) -> Option<&mut Voice> {
    match part {
        PartText::Plays {
            first,
            second: other,
            ..
        } => {
            if second {
                other.as_mut()
            } else {
                Some(first)
            }
        }
        _ => None,
    }
}

/// Changes the part that is selected, and keeps the lines of the threats in step with what
/// the scenes leave to them.
fn change_part(
    doc: &mut Doc,
    view: &View,
    piece: &Piece,
    change: impl FnOnce(&mut PartText, Row, Instrument, bool),
) {
    let Some(Picked::Part(row, instrument)) = view.picked else {
        return;
    };
    let head = row.head();
    let head = [head[0].as_str(), head[1].as_str()];
    let line = doc.find(&head).map(|at| doc.lines()[at].clone());
    let mut part = line
        .as_deref()
        .map_or(PartText::Silent, |line| PartText::of(line, instrument));
    let before = part.clone();
    change(&mut part, row, instrument, view.second);
    if part == before {
        return;
    }
    let left = |piece: &Piece, but: Option<Scene>| {
        Scene::ALL
            .into_iter()
            .filter(|scene| Some(*scene) != but)
            .any(|scene| piece.play(scene).part(instrument) == Part::ByThreat)
    };
    doc.change(|lines| {
        match lines::find(lines, &head) {
            Some(at) => lines[at] = parts::with_part(&lines[at], instrument, &part),
            // A threat whose line isn't written yet.
            None => {
                let line = parts::with_part(&format!("{} {}", head[0], head[1]), instrument, &part);
                lines::insert(lines, line);
            }
        }
        let Row::Scene(scene) = row else {
            return;
        };
        if part == PartText::ByThreat {
            // Every threat has its line, even if it says nothing yet.
            for threat in 0..THREATS {
                if lines::find(lines, &["threat", &threat.to_string()]).is_none() {
                    lines::insert(lines, format!("threat {threat}"));
                }
            }
        } else if before == PartText::ByThreat && !left(piece, Some(scene)) {
            // No scene leaves the instrument to the threat any more: what the threats gave
            // it goes with it.
            for at in lines::of_kind(lines, "threat") {
                lines[at] = parts::with_part(&lines[at], instrument, &PartText::Silent);
            }
        }
    });
}

/// The next thing that a line of a part can be, or the one before.
fn next_line(
    part: &mut PartText,
    piece: &Piece,
    row: Row,
    instrument: Instrument,
    second: bool,
    by: i8,
) {
    let voices = parts::voices(piece, instrument);
    let voice = |name: &String, old: Option<&Voice>| {
        let tune = Form::ALL.iter().any(|form| form.name() == name);
        Voice {
            name: name.clone(),
            octave: old
                .and_then(|old| old.octave)
                .or(parts::usual_octave(instrument)),
            level: old.and_then(|old| old.level).filter(|_| tune),
        }
    };
    let turn =
        |now: usize, count: usize| (now as i64 + by as i64).rem_euclid(count as i64) as usize;
    if second {
        // Nothing, then every line that the instrument can play.
        let PartText::Plays {
            first,
            second: other,
            ..
        } = part
        else {
            return;
        };
        if instrument.is_drum() {
            return;
        }
        let now = match other {
            None => 0,
            Some(voice) => {
                1 + voices
                    .iter()
                    .position(|name| *name == voice.name)
                    .unwrap_or(0)
            }
        };
        let next = turn(now, voices.len() + 1);
        *other = (next > 0).then(|| voice(&voices[next - 1], other.as_ref().or(Some(first))));
        return;
    }
    // Silence, the threat if the line is that of a scene, then every line.
    let star = matches!(row, Row::Scene(_)) as usize;
    let now = match part {
        PartText::Silent => 0,
        PartText::ByThreat => 1,
        PartText::Plays { first, .. } => {
            1 + star
                + voices
                    .iter()
                    .position(|name| *name == first.name)
                    .unwrap_or(0)
        }
    };
    let next = turn(now, voices.len() + 1 + star);
    *part = match next {
        0 => PartText::Silent,
        1 if star == 1 => PartText::ByThreat,
        place => {
            let name = &voices[place - 1 - star];
            match part {
                PartText::Plays {
                    first,
                    second,
                    together,
                } => PartText::Plays {
                    first: voice(name, Some(first)),
                    second: second.clone(),
                    together: *together,
                },
                _ => PartText::Plays {
                    first: voice(name, None),
                    second: None,
                    together: false,
                },
            }
        }
    };
}

/// Changes a place of the list of sections that a scene plays in turn: the next section, the
/// one before, or none, which ends the list there.
fn turns(doc: &mut Doc, piece: &Piece, scene: Scene, place: usize, by: i8) {
    let head = ["play", scene.name()];
    let Some(words) = doc.record(&head) else {
        return;
    };
    let mut played: Vec<String> = words[2].split(',').map(str::to_string).collect();
    let names: Vec<&str> = piece
        .passages()
        .iter()
        .map(|passage| passage.name.as_str())
        .collect();
    if place > played.len() || place >= 4 {
        return;
    }
    // The first place always has a section.
    let none = (place > 0) as usize;
    let now = match played.get(place) {
        Some(name) => none + names.iter().position(|other| other == name).unwrap_or(0),
        None => 0,
    };
    let next = (now as i64 + by as i64).rem_euclid((names.len() + none) as i64) as usize;
    played.truncate(place);
    if next >= none {
        played.push(names[next - none].to_string());
        // The places after it stay as they were.
        played.extend(words[2].split(',').skip(place + 1).map(str::to_string));
    }
    doc.set_word(&head, 2, &played.join(","));
}

#[cfg(test)]
mod tests {
    use tanku_audio::{Place, score::Mode};

    use super::*;
    use crate::state::Tab;

    const FILE: &str = crate::fixture::FILE;

    fn open() -> (Doc, View) {
        let doc = Doc::of(FILE, None).unwrap();
        let view = View::of(doc.piece());
        (doc, view)
    }

    fn press(doc: &mut Doc, view: &mut View, press: Press) {
        apply(&press, false, doc, view);
    }

    /// The line of the document that starts with these words.
    fn line(doc: &Doc, head: &[&str]) -> String {
        doc.lines()[doc.find(head).unwrap()].clone()
    }

    #[test]
    fn the_transport_follows_its_buttons() {
        let (mut doc, mut view) = open();
        press(&mut doc, &mut view, Press::Tab(Tab::Sections));
        press(&mut doc, &mut view, Press::Plays(1));
        press(&mut doc, &mut view, Press::Place(Place::Arena));
        press(&mut doc, &mut view, Press::Threat(9));
        press(&mut doc, &mut view, Press::Play);
        assert_eq!(
            (view.tab, view.plays[Tab::Sections as usize]),
            (Tab::Sections, 1)
        );
        assert_eq!(
            (view.context().place, view.context().threat),
            (Place::Arena, 4)
        );
        assert!(view.playing && view.context().fight);
        press(&mut doc, &mut view, Press::Fight(false));
        press(&mut doc, &mut view, Press::Stop);
        assert!(!view.playing && !view.context().fight);
        assert_eq!(doc.revision(), 0);
    }

    #[test]
    fn a_note_is_drawn_picked_moved_and_taken_away() {
        let (mut doc, mut view) = open();
        // The second bar of motif A, 3..5 ..6. .... 5..., gets a note in the middle of its
        // long one, as long as the last note that was touched.
        press(&mut doc, &mut view, Press::Cell(1, 8, 2));
        assert_eq!(
            line(&doc, &["motif", "A"]),
            "motif A 1..2 ..3. 5..3 ..2. | 3..5 ..6. 2.-- 5..."
        );
        assert_eq!(view.note, Some((1, 8)));
        // It goes up a degree and gets longer, up to the next note and no further.
        press(&mut doc, &mut view, Press::NoteDegree(1));
        for _ in 0..5 {
            press(&mut doc, &mut view, Press::NoteLength(1));
        }
        assert_eq!(
            line(&doc, &["motif", "A"]),
            "motif A 1..2 ..3. 5..3 ..2. | 3..5 ..6. 3... 5..."
        );
        assert_eq!(view.length, 4);
        // A click on another degree where a note starts moves that note.
        press(&mut doc, &mut view, Press::Cell(1, 12, 9));
        assert!(line(&doc, &["motif", "A"]).ends_with("3... 9..."));
        // A click on a note picks it, in the middle of it too.
        press(&mut doc, &mut view, Press::Cell(1, 4, 5));
        assert_eq!((view.note, view.length), (Some((1, 3)), 3));
        // The eraser takes a note away, and only where there is one.
        press(&mut doc, &mut view, Press::Tool(Tool::Erase));
        press(&mut doc, &mut view, Press::Cell(1, 5, 2));
        press(&mut doc, &mut view, Press::Cell(1, 5, 5));
        assert_eq!(
            line(&doc, &["motif", "A"]),
            "motif A 1..2 ..3. 5..3 ..2. | 3..- --6. 3... 9..."
        );
        assert_eq!(view.note, None);
        // The intro and the verse state the motif, so they have the change too.
        assert_eq!(
            doc.piece().passage("verse").unwrap().melody[1][12].degree,
            9
        );
        // Selecting changes nothing.
        let revision = doc.revision();
        press(&mut doc, &mut view, Press::Tool(Tool::Select));
        press(&mut doc, &mut view, Press::Cell(0, 0, 1));
        press(&mut doc, &mut view, Press::Cell(0, 0, 7));
        assert_eq!((doc.revision(), view.note), (revision, None));
    }

    #[test]
    fn a_motif_gets_bars_and_a_turn() {
        let (mut doc, mut view) = open();
        // A new motif takes the first letter that is free and starts as one bar of silence.
        press(&mut doc, &mut view, Press::NewMotif);
        assert_eq!(view.motif, b'D');
        assert_eq!(line(&doc, &["motif", "D"]), "motif D ---- ---- ---- ----");
        assert_eq!(doc.find(&["motif", "D"]), Some(9));
        press(&mut doc, &mut view, Press::MotifBars(1));
        press(&mut doc, &mut view, Press::Cell(1, 4, 6));
        assert_eq!(
            line(&doc, &["motif", "D"]),
            "motif D ---- ---- ---- ---- | ---- 6.-- ---- ----"
        );
        // Its turn starts as its last bar, and is drawn on like a bar.
        press(&mut doc, &mut view, Press::HasTurn(true));
        assert!(view.turn);
        assert_eq!(line(&doc, &["turn", "D"]), "turn  D ---- 6.-- ---- ----");
        press(&mut doc, &mut view, Press::Cell(0, 0, 1));
        assert_eq!(line(&doc, &["turn", "D"]), "turn  D 1.-- 6.-- ---- ----");
        press(&mut doc, &mut view, Press::HasTurn(false));
        assert_eq!((doc.find(&["turn", "D"]), view.turn), (None, false));
        // A bar fewer, and no fewer than one.
        press(&mut doc, &mut view, Press::MotifBars(-1));
        press(&mut doc, &mut view, Press::MotifBars(-1));
        assert_eq!(line(&doc, &["motif", "D"]), "motif D ---- ---- ---- ----");
        press(&mut doc, &mut view, Press::RemoveMotif);
        assert_eq!(doc.text(), FILE);

        // A motif that a section states stays, and so does its turn: the reader says why.
        press(&mut doc, &mut view, Press::Motif(b'A'));
        press(&mut doc, &mut view, Press::RemoveMotif);
        assert_eq!(
            doc.refusal.as_deref(),
            Some("line 23: a section comes after the motifs that it plays")
        );
        press(&mut doc, &mut view, Press::HasTurn(false));
        assert_eq!(
            doc.refusal.as_deref(),
            Some("line 24: this motif has no turn")
        );
        // A bar fewer for motif A, and the verse no longer fills its eight bars.
        press(&mut doc, &mut view, Press::MotifBars(-1));
        assert!(doc.refusal.is_some());
        assert_eq!(doc.text(), FILE);
    }

    #[test]
    fn a_pattern_is_painted_step_by_step() {
        let (mut doc, mut view) = open();
        // A hit of the kick on the third step of its heart, then away again.
        press(
            &mut doc,
            &mut view,
            Press::Step(Instrument::Kick, "heart".into(), 3),
        );
        assert_eq!(
            line(&doc, &["pat", "kick", "heart"]),
            "pat kick  heart    x.ox .... .... .... 180 135"
        );
        assert_eq!(
            view.shown_pattern(doc.piece()),
            Some((Instrument::Kick, "heart".into()))
        );
        press(
            &mut doc,
            &mut view,
            Press::Step(Instrument::Kick, "heart".into(), 3),
        );
        assert_eq!(doc.text(), FILE);
        // The first soft hit of a pattern starts at half the level of a hit, and the level
        // goes with the last one.
        press(&mut doc, &mut view, Press::Brush(b'o'));
        press(
            &mut doc,
            &mut view,
            Press::Step(Instrument::Kick, "fight".into(), 2),
        );
        assert_eq!(
            line(&doc, &["pat", "kick", "fight"]),
            "pat kick  fight    x.o. x... x... x... 255 127"
        );
        press(&mut doc, &mut view, Press::Soft(-7));
        press(&mut doc, &mut view, Press::Level(-5));
        assert!(line(&doc, &["pat", "kick", "fight"]).ends_with("250 120"));
        press(
            &mut doc,
            &mut view,
            Press::Step(Instrument::Kick, "fight".into(), 2),
        );
        assert!(line(&doc, &["pat", "kick", "fight"]).ends_with("x... x... x... x... 250"));
        // A drum has no root: the reader says what a drum has.
        press(&mut doc, &mut view, Press::Brush(b'R'));
        press(
            &mut doc,
            &mut view,
            Press::Step(Instrument::Kick, "fight".into(), 2),
        );
        assert_eq!(
            doc.refusal.as_deref(),
            Some("line 13: on a drum, a step is `x` for a hit, `o` for a soft one or a dot")
        );
        // The pluck has an echo to take or to leave.
        press(
            &mut doc,
            &mut view,
            Press::Pattern(Instrument::Pluck, "tick".into()),
        );
        press(&mut doc, &mut view, Press::Echo);
        assert!(line(&doc, &["pat", "pluck", "tick"]).ends_with("110 echo"));
    }

    #[test]
    fn a_pattern_is_made_named_and_removed() {
        let (mut doc, mut view) = open();
        press(&mut doc, &mut view, Press::PatternOf(Instrument::Bass));
        assert_eq!(
            view.shown_pattern(doc.piece()),
            Some((Instrument::Bass, "long".into()))
        );
        press(&mut doc, &mut view, Press::NewPattern);
        assert_eq!(
            line(&doc, &["pat", "bass", "new"]),
            "pat bass  new      .... .... .... .... 200"
        );
        // After the last pattern of the file.
        assert_eq!(doc.find(&["pat", "bass", "new"]), Some(23));
        press(&mut doc, &mut view, Press::Type(Field::PatternName));
        for key in [Key::Back, Key::Back, Key::Back] {
            typed(&mut doc, &mut view, key);
        }
        for letter in "Deep one".chars() {
            typed(&mut doc, &mut view, Key::Letter(letter));
        }
        typed(&mut doc, &mut view, Key::Enter);
        assert_eq!(view.typing, None);
        assert!(doc.find(&["pat", "bass", "deepone"]).is_some());
        press(&mut doc, &mut view, Press::RemovePattern);
        assert_eq!(doc.text(), FILE);

        // A pattern that the arrangement plays keeps its place in it under its new name.
        press(
            &mut doc,
            &mut view,
            Press::Pattern(Instrument::Pluck, "tick".into()),
        );
        press(&mut doc, &mut view, Press::Type(Field::PatternName));
        typed(&mut doc, &mut view, Key::Letter('s'));
        typed(&mut doc, &mut view, Key::Enter);
        assert!(line(&doc, &["play", "calm"]).contains("pluck=melody@3/ticks@2 lead=answer@4:150"));
        assert_eq!(view.pattern, Some((Instrument::Pluck, "ticks".into())));
        // And one that is played can't go: the reader says where it is missed.
        press(&mut doc, &mut view, Press::RemovePattern);
        assert!(
            doc.refusal
                .as_deref()
                .unwrap()
                .starts_with("line 30: a line is")
        );
        // Escape gives up what was typed.
        press(&mut doc, &mut view, Press::Type(Field::PatternName));
        typed(&mut doc, &mut view, Key::Letter('x'));
        typed(&mut doc, &mut view, Key::Escape);
        assert!(doc.find(&["pat", "pluck", "ticks"]).is_some());
    }

    #[test]
    fn a_section_gets_chords_statements_and_a_tempo() {
        let (mut doc, mut view) = open();
        press(&mut doc, &mut view, Press::Section("verse".into()));
        press(&mut doc, &mut view, Press::Bar(7));
        press(&mut doc, &mut view, Press::Chord(4));
        assert_eq!(
            line(&doc, &["sec", "verse"]),
            "sec verse  11C4 11C4  A A A a"
        );
        press(&mut doc, &mut view, Press::Chord(0));
        assert_eq!(
            line(&doc, &["sec", "verse"]),
            "sec verse  11C4 11CC  A A A a"
        );
        // Its last statement as it is written, then with its last note on another degree.
        press(&mut doc, &mut view, Press::Ends(Ending::Written));
        assert_eq!(
            line(&doc, &["sec", "verse"]),
            "sec verse  11C4 11CC  A A A A"
        );
        press(&mut doc, &mut view, Press::Ends(Ending::Degree(1)));
        assert_eq!(
            line(&doc, &["sec", "verse"]),
            "sec verse  11C4 11CC  A A A A:6"
        );
        press(&mut doc, &mut view, Press::Ends(Ending::Turned));
        assert!(line(&doc, &["sec", "verse"]).ends_with("A A A a"));
        // Motif C has one bar: two of it take the place of one statement of A, and A takes
        // them back.
        press(&mut doc, &mut view, Press::Bar(2));
        press(&mut doc, &mut view, Press::States(b'C'));
        assert_eq!(
            line(&doc, &["sec", "verse"]),
            "sec verse  11C4 11CC  A C C A a"
        );
        press(&mut doc, &mut view, Press::States(b'A'));
        assert_eq!(
            line(&doc, &["sec", "verse"]),
            "sec verse  11C4 11CC  A A A a"
        );
        // Motif B has no turn, so the last statement loses it.
        press(&mut doc, &mut view, Press::Bar(6));
        press(&mut doc, &mut view, Press::States(b'B'));
        assert!(line(&doc, &["sec", "verse"]).ends_with("A A A B"));
        // A tempo of its own starts as that of the piece.
        press(&mut doc, &mut view, Press::OwnTempo(true));
        press(&mut doc, &mut view, Press::SectionTempo(-3));
        assert!(line(&doc, &["sec", "verse"]).ends_with("A A A B  117"));
        assert_eq!(doc.piece().passage("verse").unwrap().tempo, 117);
        press(&mut doc, &mut view, Press::OwnTempo(false));
        assert!(line(&doc, &["sec", "verse"]).ends_with("A A A B"));
    }

    #[test]
    fn a_section_is_made_named_and_removed() {
        let (mut doc, mut view) = open();
        press(&mut doc, &mut view, Press::NewSection);
        assert_eq!(view.section, "new");
        assert_eq!(line(&doc, &["sec", "new"]), "sec new    1111 1111  A A A A");
        press(&mut doc, &mut view, Press::RemoveSection);
        assert_eq!(doc.text(), FILE);
        // The fights play the verse under whatever it is called.
        press(&mut doc, &mut view, Press::Section("verse".into()));
        press(&mut doc, &mut view, Press::Type(Field::SectionName));
        for _ in 0..5 {
            typed(&mut doc, &mut view, Key::Back);
        }
        for letter in "first".chars() {
            typed(&mut doc, &mut view, Key::Letter(letter));
        }
        // Another press ends the typing as Enter does.
        press(&mut doc, &mut view, Press::Bar(1));
        assert_eq!(view.section, "first");
        assert!(
            line(&doc, &["play", "corridor"]).starts_with("play corridor first,chorus  kick=fight")
        );
        assert!(
            line(&doc, &["play", "arena"]).starts_with("play arena    first,chorus  kick=fight")
        );
        press(&mut doc, &mut view, Press::RemoveSection);
        assert!(doc.refusal.is_some());
    }

    #[test]
    fn a_part_of_the_arrangement_changes_in_its_line() {
        let (mut doc, mut view) = open();
        let pick = |row: Row, instrument: Instrument| Press::Pick(Picked::Part(row, instrument));
        // The pluck of the calm: its melody an octave higher, quieter, over its tick.
        press(
            &mut doc,
            &mut view,
            pick(Row::Scene(Scene::Calm), Instrument::Pluck),
        );
        press(&mut doc, &mut view, Press::PartOctave(1));
        press(&mut doc, &mut view, Press::PartLevel(-1));
        assert!(line(&doc, &["play", "calm"]).contains("pluck=melody@4:250/tick@2 "));
        // Its second line, an octave lower, then gone.
        press(&mut doc, &mut view, Press::Second(true));
        press(&mut doc, &mut view, Press::PartOctave(-1));
        assert!(line(&doc, &["play", "calm"]).contains("pluck=melody@4:250/tick@1 "));
        press(&mut doc, &mut view, Press::PartLine(1));
        assert!(line(&doc, &["play", "calm"]).contains("pluck=melody@4:250/melody@1 "));
        for _ in 0..4 {
            press(&mut doc, &mut view, Press::PartLine(1));
        }
        assert!(line(&doc, &["play", "calm"]).contains("pluck=melody@4:250 "));
        // The first line turns through silence, the threat, the patterns and the melodies.
        press(&mut doc, &mut view, Press::Second(false));
        press(&mut doc, &mut view, Press::PartLine(-1));
        assert!(line(&doc, &["play", "calm"]).contains("pluck=tick@4 "));
        // The snare of the calm, which was silent, gets its only pattern after two turns.
        press(
            &mut doc,
            &mut view,
            pick(Row::Scene(Scene::Calm), Instrument::Snare),
        );
        press(&mut doc, &mut view, Press::PartLine(-1));
        assert!(line(&doc, &["play", "calm"]).contains("kick=heart snare=arena bass=pair@1"));
        press(&mut doc, &mut view, Press::PartLine(1));
        assert!(line(&doc, &["play", "calm"]).contains("kick=heart bass=pair@1"));
        // The lead at threat 4 plays its two lines together or one under the other.
        press(&mut doc, &mut view, pick(Row::Threat(4), Instrument::Lead));
        press(&mut doc, &mut view, Press::Together(false));
        assert!(line(&doc, &["threat", "4"]).ends_with("lead=ornate@4/ornate@5:130"));
    }

    #[test]
    fn the_threats_keep_up_with_what_the_scenes_leave_them() {
        let (mut doc, mut view) = open();
        let pick = |row: Row, instrument: Instrument| Press::Pick(Picked::Part(row, instrument));
        // The hi-hat of a corridor is left to the threat: nothing else has to change, since
        // every threat has its line.
        press(
            &mut doc,
            &mut view,
            pick(Row::Scene(Scene::Corridor), Instrument::Hat),
        );
        press(&mut doc, &mut view, Press::PartLine(-1));
        assert!(line(&doc, &["play", "corridor"]).contains("kick=fight hat=* bass=*"));
        assert_eq!(
            doc.piece().play(Scene::Corridor).part(Instrument::Hat),
            Part::ByThreat
        );
        // At threat 3 it plays the pattern of the arena.
        press(&mut doc, &mut view, pick(Row::Threat(3), Instrument::Hat));
        press(&mut doc, &mut view, Press::PartLine(-1));
        assert_eq!(
            line(&doc, &["threat", "3"]),
            "threat 3  hat=arena bass=leaping@2 pluck=arpeggio@3  lead=ornate@4"
        );
        // The scene takes its hi-hat back: what the threats gave it goes with the star.
        press(
            &mut doc,
            &mut view,
            pick(Row::Scene(Scene::Corridor), Instrument::Hat),
        );
        press(&mut doc, &mut view, Press::PartLine(1));
        assert!(line(&doc, &["play", "corridor"]).contains("kick=fight hat=corridor bass=*"));
        assert!(!line(&doc, &["threat", "3"]).contains("hat"));
        assert_eq!(doc.refusal, None);

        // A piece without any line of threat gets all five with its first star.
        let text: String = FILE
            .lines()
            .filter(|line| !line.starts_with("threat"))
            .map(|line| format!("{}\n", line.replace("=*", "=melody@4")))
            .collect();
        let mut doc = Doc::of(&text, None).unwrap();
        let mut view = View::of(doc.piece());
        press(
            &mut doc,
            &mut view,
            pick(Row::Scene(Scene::Menu), Instrument::Lead),
        );
        press(&mut doc, &mut view, Press::PartLine(1));
        assert!(line(&doc, &["play", "menu"]).ends_with("lead=*"));
        assert_eq!(lines::of_kind(doc.lines(), "threat").len(), 5);
        assert_eq!(doc.refusal, None);
    }

    #[test]
    fn a_scene_plays_other_sections_in_turn() {
        let (mut doc, mut view) = open();
        press(
            &mut doc,
            &mut view,
            Press::Pick(Picked::Sections(Scene::Corridor)),
        );
        // The second place goes from the chorus to the bridge, then to nothing.
        press(&mut doc, &mut view, Press::Turns(1, 1));
        assert!(line(&doc, &["play", "corridor"]).starts_with("play corridor verse,bridge  kick"));
        press(&mut doc, &mut view, Press::Turns(1, 1));
        assert!(line(&doc, &["play", "corridor"]).starts_with("play corridor verse         kick"));
        // A place after the last one adds a section, and the first place always has one.
        press(&mut doc, &mut view, Press::Turns(1, 1));
        assert!(line(&doc, &["play", "corridor"]).starts_with("play corridor verse,intro   kick"));
        press(&mut doc, &mut view, Press::Turns(0, -1));
        assert!(line(&doc, &["play", "corridor"]).starts_with("play corridor intro,intro   kick"));
        press(&mut doc, &mut view, Press::Turns(3, 1));
        assert!(line(&doc, &["play", "corridor"]).starts_with("play corridor intro,intro   kick"));
    }

    #[test]
    fn the_ways_in_and_out_of_a_fight_have_their_numbers() {
        let (mut doc, mut view) = open();
        // The connection: its degree, its beats and its notes, which fit its beats.
        press(&mut doc, &mut view, Press::WayNumber(2, -1));
        assert_eq!(line(&doc, &["link"]), "link 2 4 4");
        press(&mut doc, &mut view, Press::WayNumber(3, 1));
        assert_eq!(line(&doc, &["link"]), "link 2 4 4");
        press(&mut doc, &mut view, Press::WayNumber(1, -1));
        assert_eq!(line(&doc, &["link"]), "link 1 4 2");
        // The roll: its beats, its drums and its two levels.
        press(&mut doc, &mut view, Press::Way(Way::Roll));
        press(&mut doc, &mut view, Press::WayNumber(1, 1));
        press(&mut doc, &mut view, Press::WayDrum(2));
        press(&mut doc, &mut view, Press::WayDrum(3));
        press(&mut doc, &mut view, Press::WayNumber(4, 2));
        press(&mut doc, &mut view, Press::WayNumber(5, -1));
        assert_eq!(line(&doc, &["roll"]), "roll 3 snare kick 50 250");
    }

    #[test]
    fn an_instrument_has_its_numbers_and_its_mix() {
        let (mut doc, mut view) = open();
        // The vibrato of the lead comes in a little later, and its pulse gets narrower.
        press(&mut doc, &mut view, Press::Setting(9, 1));
        press(&mut doc, &mut view, Press::Setting(3, -1));
        assert_eq!(
            line(&doc, &["sound", "lead"]),
            "sound lead  width 28%  rise 4ms  below 3000Hz  vibrato 140ms 0.4% 5.5Hz"
        );
        assert_eq!(doc.piece().sounds.lead.vibrato_after, 0.14);
        press(&mut doc, &mut view, Press::Mix(0, 2));
        press(&mut doc, &mut view, Press::Mix(1, -1));
        assert_eq!(line(&doc, &["mix", "lead"]), "mix lead   7%  7%");
        // Another instrument, and a word that is no number of its line.
        press(&mut doc, &mut view, Press::Instrument(Instrument::Kick));
        press(&mut doc, &mut view, Press::Setting(6, 1));
        press(&mut doc, &mut view, Press::Setting(2, 1));
        assert!(line(&doc, &["sound", "kick"]).ends_with("decay 45ms"));
        press(&mut doc, &mut view, Press::Setting(7, 1));
        assert_eq!(
            line(&doc, &["sound", "kick"]),
            "sound kick  pitch 110Hz 55Hz 30ms  decay 46ms"
        );
    }

    #[test]
    fn the_piece_has_its_tempo_and_its_key() {
        let (mut doc, mut view) = open();
        press(&mut doc, &mut view, Press::Tempo(-4));
        apply(&Press::Tempo(1), true, &mut doc, &mut view);
        assert_eq!(doc.piece().tempo, 126);
        press(&mut doc, &mut view, Press::Tonic(4));
        press(&mut doc, &mut view, Press::Home(Mode::Phrygian));
        press(&mut doc, &mut view, Press::Arena(Mode::Locrian));
        assert_eq!(line(&doc, &["key"]), "key C# phrygian locrian");
        let key = doc.piece().key;
        assert_eq!(
            (key.tonic, key.home, key.arena),
            (4, Mode::Phrygian, Mode::Locrian)
        );
        // The tempo stays where a piece can have it.
        apply(&Press::Tempo(100), true, &mut doc, &mut view);
        assert_eq!(doc.piece().tempo, 300);
    }

    #[test]
    fn what_is_done_is_undone_and_saved() {
        let (mut doc, mut view) = open();
        press(&mut doc, &mut view, Press::Tempo(1));
        press(&mut doc, &mut view, Press::Undo);
        assert_eq!(doc.text(), FILE);
        press(&mut doc, &mut view, Press::Redo);
        assert_eq!(doc.piece().tempo, 121);
        // This document has no file.
        press(&mut doc, &mut view, Press::Save);
        assert_eq!(
            doc.refusal.as_deref(),
            Some("this document has no file to be saved in")
        );
        press(&mut doc, &mut view, Press::Revert);
        assert_eq!((doc.text().as_str(), doc.refusal.as_deref()), (FILE, None));
    }
}
