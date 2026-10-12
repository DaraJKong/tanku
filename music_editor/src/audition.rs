//! Listening to one thing of a piece: a motif, a pattern, a section, an instrument, the
//! scale.
//!
//! The game only plays whole arrangements, so the editor writes one for the occasion. It
//! takes the lines of the document, rewrites the few that say what plays when, and hands the
//! result to the reader like any other file. Nothing of it is ever saved.

use std::sync::Arc;

use tanku_audio::{
    Context, Place,
    notation::{Instrument, Part, Piece, Scene, Source, THREATS, read},
    score::Score,
};

use crate::lines;

/// What the play button plays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Audition {
    /// The piece as the game plays it.
    Game,
    /// A motif alone, on the lead.
    Motif(u8),
    /// A pattern alone, over the chords of a section that has it.
    Pattern(Instrument, String),
    /// One section, in the arrangement of the scene that the transport is in.
    Section(String),
    /// The piece as the game plays it, with one instrument only.
    Solo(Instrument),
    /// The scale of the mode, up and down, on the lead.
    Scale,
}

impl Audition {
    /// The context that the audition is heard in, if it isn't the one of the transport: a
    /// motif and a pattern are played without a fight, so that no roll comes before them.
    pub fn context(&self) -> Option<Context> {
        matches!(self, Self::Motif(_) | Self::Pattern(..)).then(|| Context {
            place: Place::Bay,
            ..Context::default()
        })
    }
}

const SCENES: [&str; 4] = ["menu", "calm", "corridor", "arena"];

/// A name for the section that an audition adds, which no section of the piece has.
fn free_name(piece: &Piece) -> String {
    let names = (1..).map(|number| match number {
        1 => "audition".to_string(),
        number => format!("audition{number}"),
    });
    let free = |name: &String| piece.passage(name).is_none();
    names.take(9).find(free).unwrap_or_default()
}

/// A letter for the motif that an audition adds, which no motif of the piece has.
fn free_letter(piece: &Piece) -> Result<char, String> {
    let free = |letter: &char| piece.motif(*letter as u8).is_none();
    let letter = ('A'..='Z').rev().find(free);
    letter.ok_or_else(|| "every letter names a motif already".to_string())
}

/// The lines without what says what plays when, and where that was.
fn without_arrangement(lines: &[String]) -> (Vec<String>, usize) {
    let arranges = |line: &String| {
        lines::is_record(line) && matches!(lines::words(line).first(), Some(&"play" | &"threat"))
    };
    let at = lines.iter().position(arranges).unwrap_or(lines.len());
    let kept = lines.iter().filter(|line| !arranges(line)).cloned();
    (kept.collect(), at)
}

/// The octave in which a piece plays a pattern, and the first section of the scene that has
/// it. A pattern that nothing plays is heard over the first section of the piece, low for
/// the bass and in the middle for the pluck.
fn where_played(piece: &Piece, index: u8, instrument: Instrument) -> (String, u8) {
    let has = |source: Source| match source {
        Source::Rhythm {
            index: other,
            octave,
        } if other == index => Some(octave),
        _ => None,
    };
    for scene in Scene::ALL {
        let play = piece.play(scene);
        let layers = match play.part(instrument) {
            Part::Silent => Vec::new(),
            Part::Plays(layer) => vec![layer],
            Part::ByThreat => (0..THREATS as u8)
                .filter_map(|threat| piece.threat(threat, instrument))
                .collect(),
        };
        for layer in layers {
            let octave = has(layer.first).or_else(|| layer.second.and_then(has));
            if let Some(octave) = octave {
                let section = piece.section(play.sections()[0]).name.as_str().to_string();
                return (section, octave);
            }
        }
    }
    let first = piece.passages()[0].name.as_str().to_string();
    (first, if instrument == Instrument::Bass { 2 } else { 3 })
}

/// The lines of the piece that an audition plays.
pub fn lines_of(
    lines: &[String],
    piece: &Piece,
    audition: &Audition,
) -> Result<Vec<String>, String> {
    match audition {
        Audition::Game => Ok(lines.to_vec()),
        Audition::Section(name) => {
            let mut out = lines.to_vec();
            for at in lines::of_kind(lines, "play") {
                out[at] = lines::with_word(&out[at], 2, name);
            }
            Ok(out)
        }
        Audition::Solo(instrument) => {
            let mut out = lines.to_vec();
            for at in lines::of_kind(lines, "mix") {
                if lines::words(&out[at]).get(1) != Some(&instrument.name()) {
                    let silent = lines::with_word(&out[at], 2, "0%");
                    out[at] = lines::with_word(&silent, 3, "0%");
                }
            }
            Ok(out)
        }
        Audition::Pattern(instrument, name) => {
            let mut rhythms = piece.rhythms().iter();
            let place = rhythms
                .position(|rhythm| rhythm.instrument == *instrument && rhythm.name.is(name))
                .ok_or("the piece has no such pattern")?;
            let (section, octave) = where_played(piece, place as u8, *instrument);
            let part = if instrument.is_drum() {
                format!("{}={name}", instrument.name())
            } else {
                format!("{}={name}@{octave}", instrument.name())
            };
            let (mut out, at) = without_arrangement(lines);
            let plays = SCENES.map(|scene| format!("play {scene} {section} {part}"));
            out.splice(at..at, plays);
            Ok(out)
        }
        Audition::Motif(letter) => {
            let motif = piece.motif(*letter).ok_or("the piece has no such motif")?;
            let (letter, bars) = (*letter as char, motif.bars().len());
            let mut statements = vec![letter.to_string(); 8 / bars];
            let mut added = Vec::new();
            if 8 % bars != 0 {
                // Three bars go twice into eight, with two bars of silence after them.
                let filler = free_letter(piece)?;
                let silence = "---- ---- ---- ----";
                added.push(format!("motif {filler} {silence} | {silence}"));
                statements.push(filler.to_string());
            }
            alone(lines, piece, added, &statements, &["menu"])
        }
        Audition::Scale => {
            let letter = free_letter(piece)?;
            let scale = format!("motif {letter} 1.2. 3.4. 5.6. 7.8. | 8.7. 6.5. 4.3. 2.1.");
            let statements = vec![letter.to_string(); 4];
            // In every scene, so that a fight in an arena gives the scale of its mode.
            alone(lines, piece, vec![scale], &statements, &SCENES)
        }
    }
}

/// The lines of a piece that only plays one melody, on the lead, in some of its scenes.
fn alone(
    lines: &[String],
    piece: &Piece,
    mut added: Vec<String>,
    statements: &[String],
    scenes: &[&str],
) -> Result<Vec<String>, String> {
    let name = free_name(piece);
    added.push(lines::section(&name, &[1; 8], statements, None));
    for scene in SCENES {
        let lead = if scenes.contains(&scene) {
            " lead=melody@4"
        } else {
            ""
        };
        added.push(format!("play {scene} {name}{lead}"));
    }
    let (mut out, at) = without_arrangement(lines);
    out.splice(at..at, added);
    Ok(out)
}

/// The piece that an audition plays, with every line that the rules derive from it.
pub fn score(lines: &[String], piece: &Piece, audition: &Audition) -> Result<Arc<Score>, String> {
    let lines = lines_of(lines, piece, audition)?;
    let mut text = lines.join("\n");
    text.push('\n');
    let piece =
        read(&text).map_err(|fault| format!("this can't be played alone: {}", fault.what))?;
    Ok(Arc::new(Score::new(piece)))
}

#[cfg(test)]
mod tests {
    use tanku_audio::{
        director::{Director, Step},
        notation::Form,
        score::PIECE,
    };

    use super::*;

    const FILE: &str = include_str!("../../tanku_audio/music.score");

    fn lines() -> Vec<String> {
        FILE.lines().map(str::to_string).collect()
    }

    fn heard(audition: &Audition) -> Arc<Score> {
        score(&lines(), &PIECE, audition).unwrap()
    }

    /// Two bars of a piece in a context: what starts on each step.
    fn two_bars(score: &Arc<Score>, context: Context) -> Vec<Step> {
        let mut director = Director::settled_in(score.clone(), context);
        (0..32).map(|_| director.step(context)).collect()
    }

    fn bay() -> Context {
        Context {
            place: Place::Bay,
            ..Context::default()
        }
    }

    fn fight(place: Place, threat: u8) -> Context {
        Context {
            place,
            fight: true,
            threat,
            ..Context::default()
        }
    }

    #[test]
    fn the_game_is_the_piece_as_it_is() {
        assert_eq!(*heard(&Audition::Game), Score::new(PIECE));
        assert_eq!(Audition::Game.context(), None);
    }

    #[test]
    fn a_motif_is_heard_alone_on_the_lead() {
        let score = heard(&Audition::Motif(b'B'));
        let context = Audition::Motif(b'B').context().unwrap();
        let bars = two_bars(&score, context);
        // The first note of motif B is degree 8, the A above the tonic of the lead, and its
        // second bar starts on degree 5.
        assert_eq!(bars[0].lead[0].unwrap().pitch, 81);
        assert_eq!(bars[16].lead[0].unwrap().pitch, 76);
        let quiet = |step: &Step| {
            step.kick == 0
                && step.hat == 0
                && step.snare == 0
                && step.bass.is_none()
                && step.pluck.is_none()
        };
        assert!(bars.iter().all(quiet));
        // The piece has one section more, which is that motif four times.
        assert_eq!(score.piece.passages().len(), 5);
        let melody = score.piece.passage("audition").unwrap().melody;
        let motif = PIECE.motif(b'B').unwrap();
        assert_eq!(melody[6..], motif.bars()[..]);

        // A motif of one bar is played eight times.
        let call = heard(&Audition::Motif(b'C'));
        let melody = call.piece.passage("audition").unwrap().melody;
        assert!(
            melody
                .iter()
                .all(|bar| *bar == PIECE.motif(b'C').unwrap().bars()[0])
        );
    }

    #[test]
    fn a_motif_of_three_bars_is_followed_by_silence() {
        let mut lines = lines();
        let at = lines::find(&lines, &["motif", "C"]).unwrap();
        lines.insert(
            at + 1,
            "motif D 1... .... .... .... | 2... .... .... .... | 3... .... .... ....".into(),
        );
        let text = lines.join("\n") + "\n";
        let piece = read(&text).unwrap();
        let score = score(&lines, &piece, &Audition::Motif(b'D')).unwrap();
        let melody = score.piece.passage("audition").unwrap().melody;
        let degrees: Vec<u8> = melody.iter().map(|bar| bar[0].degree).collect();
        assert_eq!(degrees, [1, 2, 3, 1, 2, 3, 0, 0]);
    }

    #[test]
    fn a_pattern_is_heard_alone_where_the_piece_plays_it() {
        // The leaping bass, in the octave of the fights and over the chords of the verse.
        let audition = Audition::Pattern(Instrument::Bass, "leaping".into());
        let score = heard(&audition);
        let bars = two_bars(&score, audition.context().unwrap());
        let pitches: Vec<u8> = bars[..16]
            .iter()
            .filter_map(|step| step.bass)
            .map(|tone| tone.pitch)
            .collect();
        assert_eq!(pitches, [45, 57, 45, 45, 57, 52]);
        assert!(
            bars.iter()
                .all(|step| step.pluck.is_none() && step.lead[0].is_none() && step.kick == 0)
        );
        assert_eq!(score.piece.play(Scene::Menu).sections(), [1]);

        // A drum has no octave, and a pattern that nothing plays is heard over the first
        // section.
        let score = heard(&Audition::Pattern(Instrument::Snare, "arena".into()));
        let bars = two_bars(&score, bay());
        let hits: Vec<usize> = (0..16).filter(|&step| bars[step].snare > 0).collect();
        assert_eq!(hits, [4, 12]);
        let mut lines = lines();
        lines::insert(
            &mut lines,
            "pat pluck spare    x... .... x... .... 90".into(),
        );
        let piece = read(&(lines.join("\n") + "\n")).unwrap();
        let spare = Audition::Pattern(Instrument::Pluck, "spare".into());
        let score = super::score(&lines, &piece, &spare).unwrap();
        assert_eq!(score.piece.play(Scene::Menu).sections(), [0]);
        let bars = two_bars(&score, bay());
        assert_eq!(bars[0].pluck.unwrap().level, 90);
    }

    #[test]
    fn a_section_is_heard_in_the_arrangement_of_the_moment() {
        let score = heard(&Audition::Section("chorus".into()));
        for scene in Scene::ALL {
            assert_eq!(score.piece.play(scene).sections(), [2]);
        }
        // A fight in a corridor at threat 2: the chorus with its kick and its lead, which
        // starts on degree 8.
        let bars = two_bars(&score, fight(Place::Corridor, 2));
        assert!(bars[0].kick > 0 && bars[0].bass.is_some());
        assert_eq!(bars[0].lead[0].unwrap().pitch, 81);
    }

    #[test]
    fn an_instrument_is_heard_alone() {
        let score = heard(&Audition::Solo(Instrument::Lead));
        for instrument in Instrument::ALL {
            let alone = instrument == Instrument::Lead;
            let level = score.piece.mix(instrument);
            assert_eq!(level != [0.0, 0.0], alone, "{instrument:?}");
        }
        assert_eq!(
            score.piece.mix(Instrument::Lead),
            PIECE.mix(Instrument::Lead)
        );
        // Nothing else of the piece moved.
        assert_eq!(score.piece.play(Scene::Arena), PIECE.play(Scene::Arena));
    }

    #[test]
    fn the_scale_is_heard_in_the_mode_of_the_scene() {
        let score = heard(&Audition::Scale);
        let scale = |context: Context| -> Vec<u8> {
            let bars = two_bars(&score, context);
            bars[..16]
                .iter()
                .filter_map(|step| step.lead[0])
                .map(|tone| tone.pitch - 69)
                .collect()
        };
        // Dorian in the menu, and Aeolian in a fight of an arena: the sixth degree is a
        // semitone lower there.
        assert_eq!(scale(bay()), [0, 2, 3, 5, 7, 9, 10, 12]);
        assert_eq!(scale(fight(Place::Arena, 0)), [0, 2, 3, 5, 7, 8, 10, 12]);
        assert_eq!(score.line(4, Form::Melody, 1)[0].degree, 8);
    }
}
