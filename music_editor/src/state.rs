//! What the editor shows and what is selected in it: the tab, the motif, the note, the
//! pattern, the bar, the part, and what the play button plays.

use bevy::prelude::{Component, Resource};
use tanku_audio::{
    Context, Place,
    notation::{Instrument, Piece, Scene},
    score::Mode,
};

use crate::audition::Audition;

/// The tabs of the window, in their order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tab {
    #[default]
    Motifs,
    Patterns,
    Sections,
    Transitions,
    Instruments,
    Piece,
    File,
}

impl Tab {
    pub const ALL: [Self; 7] = [
        Self::Motifs,
        Self::Patterns,
        Self::Sections,
        Self::Transitions,
        Self::Instruments,
        Self::Piece,
        Self::File,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Motifs => "Motifs",
            Self::Patterns => "Patterns",
            Self::Sections => "Sections",
            Self::Transitions => "Transitions",
            Self::Instruments => "Instruments",
            Self::Piece => "Piece",
            Self::File => "File",
        }
    }
}

/// What a click does on the grid of a motif.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Draw,
    Select,
    Erase,
}

/// A line of the arrangement: that of a scene, or that of a threat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Scene(Scene),
    Threat(u8),
}

impl Row {
    /// The words that the line of the file starts with.
    pub fn head(self) -> [String; 2] {
        match self {
            Self::Scene(scene) => ["play".to_string(), scene.name().to_string()],
            Self::Threat(threat) => ["threat".to_string(), threat.to_string()],
        }
    }
}

/// What is selected in the arrangement: what an instrument plays in a line, or the sections
/// that a scene plays in turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picked {
    Part(Row, Instrument),
    Sections(Scene),
}

/// The two ways between a fight and what is around it that the file writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Way {
    Roll,
    #[default]
    Link,
}

/// A text that is typed into: the name of a pattern or of a section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    PatternName,
    SectionName,
}

/// What the keyboard writes into, and what it holds so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Typing {
    pub field: Field,
    pub text: String,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct View {
    pub tab: Tab,

    /// The motif that is shown, by its letter.
    pub motif: u8,
    /// Whether the grid shows the turn of the motif instead of its bars.
    pub turn: bool,
    /// The note that is selected: its bar, and the step where it starts.
    pub note: Option<(usize, usize)>,
    pub tool: Tool,
    /// Steps that the next note lasts, if it has the room: as many as the last one.
    pub length: u8,

    /// The pattern that is shown, and the instrument that a new one is for.
    pub pattern: Option<(Instrument, String)>,
    pub pattern_of: Instrument,
    /// The symbol that a click writes into a pattern.
    pub brush: u8,
    /// The chord over which the notes of the pattern are told: a bar of the section.
    pub over: usize,

    pub section: String,
    /// The bar of the section that is selected, from 0 to 7.
    pub bar: usize,
    pub picked: Option<Picked>,
    /// Whether the panel of the part shows its second line.
    pub second: bool,

    pub way: Way,
    pub instrument: Instrument,

    /// For each tab, what the play button plays: the thing of the tab, or the game.
    pub plays: [usize; 7],
    pub playing: bool,
    pub place: Place,
    pub fight: bool,
    pub threat: u8,

    pub typing: Option<Typing>,
}

impl View {
    /// The view of a piece that was just opened: its first motif, its first section.
    pub fn of(piece: &Piece) -> Self {
        let motif = piece.motifs().first().map_or(b'A', |motif| motif.letter);
        let section = piece.passages().first();
        Self {
            tab: Tab::Motifs,
            motif,
            turn: false,
            note: None,
            tool: Tool::Draw,
            length: 2,
            pattern: None,
            pattern_of: Instrument::Kick,
            brush: b'x',
            over: 0,
            section: section.map_or(String::new(), |passage| passage.name.as_str().to_string()),
            bar: 0,
            picked: None,
            second: false,
            way: Way::Link,
            instrument: Instrument::Lead,
            plays: [0; 7],
            playing: false,
            place: Place::Corridor,
            fight: true,
            threat: 2,
            typing: None,
        }
    }

    /// The context of the game that the transport is in.
    pub fn context(&self) -> Context {
        Context {
            place: self.place,
            fight: self.fight,
            threat: self.threat,
            ..Context::default()
        }
    }

    /// The pattern that is shown: the one that was picked if the piece still has it, or the
    /// first one of the piece.
    pub fn shown_pattern(&self, piece: &Piece) -> Option<(Instrument, String)> {
        let has =
            |(instrument, name): &(Instrument, String)| piece.rhythm(*instrument, name).is_some();
        let first = piece.rhythms().first();
        self.pattern
            .clone()
            .filter(has)
            .or_else(|| first.map(|rhythm| (rhythm.instrument, rhythm.name.as_str().to_string())))
    }

    /// The motif that is shown: the one that was picked if the piece still has it.
    pub fn shown_motif(&self, piece: &Piece) -> Option<u8> {
        let first = piece.motifs().first().map(|motif| motif.letter);
        piece.motif(self.motif).map(|motif| motif.letter).or(first)
    }

    /// The section that is shown.
    pub fn shown_section(&self, piece: &Piece) -> Option<String> {
        let first = piece.passages().first();
        let first = first.map(|passage| passage.name.as_str().to_string());
        piece
            .passage(&self.section)
            .map(|_| self.section.clone())
            .or(first)
    }

    /// The names of what the play button can play on a tab. The last one is the game.
    pub fn choices(tab: Tab) -> &'static [&'static str] {
        match tab {
            Tab::Motifs => &["This motif", "Its section", "As in the game"],
            Tab::Patterns => &["This pattern", "Its section", "As in the game"],
            Tab::Sections => &["This section", "As in the game"],
            Tab::Transitions => &["As in the game"],
            Tab::Instruments => &["This instrument alone", "As in the game"],
            Tab::Piece => &["The scale", "As in the game"],
            Tab::File => &["As in the game"],
        }
    }

    /// What the play button plays now.
    pub fn audition(&self, piece: &Piece) -> Audition {
        let choice = self.plays[self.tab as usize];
        let section_of = |name: Option<String>| name.map_or(Audition::Game, Audition::Section);
        match (self.tab, choice) {
            (Tab::Motifs, 0) => self
                .shown_motif(piece)
                .map_or(Audition::Game, Audition::Motif),
            (Tab::Motifs, 1) => {
                let motif = self.shown_motif(piece);
                section_of(motif.and_then(|letter| stating(piece, letter).into_iter().next()))
            }
            (Tab::Patterns, 0) => self
                .shown_pattern(piece)
                .map_or(Audition::Game, |(instrument, name)| {
                    Audition::Pattern(instrument, name)
                }),
            (Tab::Patterns, 1) => section_of(
                self.shown_pattern(piece)
                    .and_then(|(instrument, name)| playing(piece, instrument, &name)),
            ),
            (Tab::Sections, 0) => section_of(self.shown_section(piece)),
            (Tab::Instruments, 0) => Audition::Solo(self.instrument),
            (Tab::Piece, 0) => Audition::Scale,
            _ => Audition::Game,
        }
    }
}

/// The sections whose melody states a motif, in the order of the piece: those that have its
/// first bar, wherever it is and however the statement ends.
pub fn stating(piece: &Piece, letter: u8) -> Vec<String> {
    let Some(motif) = piece.motif(letter) else {
        return Vec::new();
    };
    let first = motif.bars()[0];
    let passages = piece.passages().iter();
    passages
        .filter(|passage| passage.melody.contains(&first))
        .map(|passage| passage.name.as_str().to_string())
        .collect()
}

/// The first section of the first scene that plays a pattern.
pub fn playing(piece: &Piece, instrument: Instrument, name: &str) -> Option<String> {
    use tanku_audio::notation::{Part, Source, THREATS};
    let place = piece
        .rhythms()
        .iter()
        .position(|rhythm| rhythm.instrument == instrument && rhythm.name.is(name))?;
    let has =
        |source: Source| matches!(source, Source::Rhythm { index, .. } if index as usize == place);
    for scene in Scene::ALL {
        let play = piece.play(scene);
        let layers = match play.part(instrument) {
            Part::Silent => Vec::new(),
            Part::Plays(layer) => vec![layer],
            Part::ByThreat => (0..THREATS as u8)
                .filter_map(|threat| piece.threat(threat, instrument))
                .collect(),
        };
        if layers
            .iter()
            .any(|layer| has(layer.first) || layer.second.is_some_and(has))
        {
            return Some(piece.section(play.sections()[0]).name.as_str().to_string());
        }
    }
    None
}

/// Everything of the window that a click presses.
#[derive(Component, Clone, Debug, PartialEq)]
pub enum Press {
    Tab(Tab),
    Undo,
    Redo,
    Save,
    Revert,
    Copy,
    Play,
    Stop,
    Plays(usize),
    Place(Place),
    Fight(bool),
    Threat(u8),

    Motif(u8),
    NewMotif,
    RemoveMotif,
    /// One bar more or one bar fewer.
    MotifBars(i8),
    /// Shows the bars of the motif, or its turn.
    Turn(bool),
    /// Gives the motif a turn, or takes its turn away.
    HasTurn(bool),
    Tool(Tool),
    /// A cell of the grid of a motif: a bar, a step and a degree.
    Cell(usize, usize, u8),
    NoteDegree(i8),
    NoteLength(i8),

    PatternOf(Instrument),
    Pattern(Instrument, String),
    NewPattern,
    RemovePattern,
    Brush(u8),
    /// A step of a pattern.
    Step(Instrument, String, usize),
    Level(i16),
    Soft(i16),
    Echo,
    Over(usize),

    Section(String),
    NewSection,
    RemoveSection,
    Bar(usize),
    Chord(u8),
    /// The motif that the statement of the selected bar plays.
    States(u8),
    /// How that statement ends: as written, with its turn, or on a degree.
    Ends(Ending),
    OwnTempo(bool),
    SectionTempo(i16),
    Pick(Picked),
    /// Shows the first line of the part, or its second one.
    Second(bool),
    /// The next or the last thing that the line of the part can be.
    PartLine(i8),
    PartOctave(i8),
    PartLevel(i16),
    /// Whether the second line sounds together with the first, which the lead can do.
    Together(bool),
    /// The next or the last section in a place of the list of a scene.
    Turns(usize, i8),

    Way(Way),
    /// A number of the roll or of the connection, by its place in the line, and how far it
    /// moves.
    WayNumber(usize, i16),
    /// The drum that rolls, or the one that rolls in an arena: the next one.
    WayDrum(usize),

    Instrument(Instrument),
    /// A number of the `sound` line of the instrument, by the place of its word.
    Setting(usize, i8),
    /// The level of the instrument on a side.
    Mix(usize, i16),

    Tempo(i16),
    Tonic(u8),
    Home(Mode),
    Arena(Mode),

    Type(Field),
}

impl Press {
    /// Whether the press goes on while its button is held: it moves a number by a step.
    pub fn repeats(&self) -> bool {
        matches!(
            self,
            Self::NoteDegree(_)
                | Self::NoteLength(_)
                | Self::Level(_)
                | Self::Soft(_)
                | Self::SectionTempo(_)
                | Self::PartLevel(_)
                | Self::WayNumber(..)
                | Self::Setting(..)
                | Self::Mix(..)
                | Self::Tempo(_)
        )
    }
}

/// How a statement of a melody ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    Written,
    Turned,
    /// One degree higher or lower for its last note.
    Degree(i8),
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::fixture::PIECE;

    #[test]
    fn the_play_button_plays_the_thing_of_the_tab() {
        let mut view = View::of(&PIECE);
        assert_eq!(view.audition(&PIECE), Audition::Motif(b'A'));
        view.plays[Tab::Motifs as usize] = 1;
        assert_eq!(view.audition(&PIECE), Audition::Section("intro".into()));
        view.motif = b'B';
        assert_eq!(view.audition(&PIECE), Audition::Section("chorus".into()));
        view.plays[Tab::Motifs as usize] = 2;
        assert_eq!(view.audition(&PIECE), Audition::Game);

        view.tab = Tab::Patterns;
        assert_eq!(
            view.audition(&PIECE),
            Audition::Pattern(Instrument::Kick, "heart".into())
        );
        view.pattern = Some((Instrument::Bass, "leaping".into()));
        view.plays[Tab::Patterns as usize] = 1;
        assert_eq!(view.audition(&PIECE), Audition::Section("verse".into()));

        view.tab = Tab::Instruments;
        assert_eq!(view.audition(&PIECE), Audition::Solo(Instrument::Lead));
        view.tab = Tab::File;
        assert_eq!(view.audition(&PIECE), Audition::Game);
        // Every tab ends its choices with the game.
        for tab in Tab::ALL {
            assert_eq!(View::choices(tab).last(), Some(&"As in the game"));
        }
    }

    #[test]
    fn a_selection_that_the_piece_lost_falls_back() {
        let mut view = View::of(&PIECE);
        view.motif = b'Q';
        view.section = "gone".into();
        view.pattern = Some((Instrument::Hat, "gone".into()));
        assert_eq!(view.shown_motif(&PIECE), Some(b'A'));
        assert_eq!(view.shown_section(&PIECE).as_deref(), Some("intro"));
        assert_eq!(
            view.shown_pattern(&PIECE),
            Some((Instrument::Kick, "heart".into()))
        );
    }
}
