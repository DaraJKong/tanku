//! Chooses what plays. The game gives a context, and on every step the director says what each
//! instrument starts.
//!
//! What plays is the arrangement that the file of the piece writes: a line for each scene,
//! with the sections that play in turn and what each instrument plays in them, and a line for
//! each threat. The director reads the context as a scene and plays its line.
//!
//! Three rules make the music answer fast and stay in time. A change within a fight, such as
//! a new threat, waits for the bar line. A fight has a way in and a way out, which last two
//! beats each: a roll announces it at once, and a connection leads its tune, from the next
//! beat on, to where the bridge begins. And what follows a roll or a connection starts on its
//! own first beat, wherever the bar was: the bridge has a tempo of its own, so nothing has to
//! line up.
//!
//! All of it is integer logic, so it gives the same answer on every platform.

use crate::{
    notation::{Instrument, Layer, Part, Passage, Play, Rhythm, Scene, Source, THREATS},
    score::{self, BARS, BEAT, Line, Mode, SCORE, STEPS, Score},
};

/// Where the run has got to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    /// No run is on: the main menu or the game over screen.
    #[default]
    Menu,
    Bay,
    Corridor,
    Arena,
}

/// All that the music knows about the game.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub place: Place,
    /// At least one enemy is awake.
    pub fight: bool,
    /// The highest brain tier among the enemies that are awake, from 0 to 4.
    pub threat: u8,
    /// The player's tank is destroyed and not back yet.
    pub down: bool,
    /// A menu is open over the game.
    pub muffled: bool,
    /// The warning of a mine sounds: the music makes room for it. The director has no say in
    /// this, which can't wait for a step.
    pub ducked: bool,
}

const HIGHEST_THREAT: u8 = THREATS as u8 - 1;

/// A note to start: its pitch in semitones, where 69 is the A of 440 Hz, its length, and how
/// loud it is, up to 255.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    pub pitch: u8,
    pub steps: u8,
    pub level: u8,
}

/// What starts on one step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Step {
    /// How long the step lasts, in half frames. It depends on the tempo of the section.
    pub length: u32,
    /// How hard each drum is hit, from 0 for no hit to 255.
    pub kick: u8,
    pub hat: u8,
    pub snare: u8,
    pub bass: Option<Tone>,
    pub pluck: Option<Tone>,
    /// The note of the pluck gets an echo, as the notes of its arpeggio do. Its melodies stay
    /// dry.
    pub echo: bool,
    /// The lead plays one note, or two an octave apart.
    pub lead: [Option<Tone>; 2],
    /// The section changed on this step: the notes that were sounding let go.
    pub fresh: bool,
    /// The tank is destroyed or the run is over: everything stops but the bass, which holds
    /// its note until the next bar line.
    pub hush: bool,
    pub muffled: bool,
}

/// How loud a note is that no line of the file gives a level: the note that the bass holds
/// while a tank is destroyed, and the notes of a connection.
const HIT: u8 = 255;
/// A roll lasts a few beats, at the tempo of the fight it announces.
pub const ROLL_HITS: u8 = score::ROLL.beats * BEAT as u8;
/// Bars in a phrase. A fight opens on the phrase after the one where the last fight ended.
const BARS_IN_PHRASE: u8 = 4;

/// The roll that announces a fight.
#[derive(Clone, Copy, Debug)]
struct Rolling {
    /// The drum that rolls: one for an arena, another elsewhere.
    drum: Instrument,
    done: u8,
}

/// The connection that leads from a fight into what follows.
#[derive(Clone, Copy, Debug)]
struct Connection {
    /// The way from the note the lead was on to the degree on which the bridge stays.
    line: Line,
    /// Whether the lead goes that way with the pluck and the bass: it does if it was playing.
    lead: bool,
    /// How long a step of the fight lasted, in half frames.
    from: u32,
    /// How hard the kick of the fight was hit, or 0 if the fight had none.
    kick: u8,
    done: u8,
}

/// A note that a line starts: whether it has an echo, and its degree if the line is a melody.
type Start = (Tone, bool, Option<u8>);

#[derive(Clone, Debug)]
pub struct Director {
    score: &'static Score,
    /// The scene whose line of the arrangement plays.
    scene: Scene,
    /// The bar of that line that plays, counted through all its sections.
    bar: u8,
    /// The step of the bar that comes next.
    step: u8,
    mode: Mode,
    /// The threat that the bar that plays was arranged for.
    threat: u8,
    /// The phrase that the next fight opens on.
    next_phrase: u8,
    roll: Option<Rolling>,
    connection: Option<Connection>,
    /// The degree of the last note that the lead started in the fight, if the lead is playing.
    lead_on: Option<u8>,
    /// Everything has stopped but the bass, until the next bar line.
    hush: bool,
    /// Whether the last step was outside a run, to tell when one starts.
    in_menu: bool,
}

/// A director at the start of the piece of the game, in the main menu.
impl Default for Director {
    fn default() -> Self {
        Self::of(&SCORE)
    }
}

impl Director {
    /// A director at the start of a piece, in the main menu.
    pub fn of(score: &'static Score) -> Self {
        Self {
            score,
            scene: Scene::Menu,
            bar: 0,
            step: 0,
            mode: score.piece.key.home,
            threat: 0,
            next_phrase: 0,
            roll: None,
            connection: None,
            lead_on: None,
            hush: false,
            in_menu: true,
        }
    }

    /// A director for a game that has been in this context for a long time: it is in the
    /// section of the context, and nothing announces it.
    pub fn settled(context: Context) -> Self {
        Self::settled_in(&SCORE, context)
    }

    /// The same, for any piece.
    pub fn settled_in(score: &'static Score, context: Context) -> Self {
        let mut director = Self {
            in_menu: context.place == Place::Menu,
            ..Self::of(score)
        };
        if is_fight(context) {
            director.enter_fight(context);
        } else {
            director.enter(calm(context));
        }
        director
    }

    /// The name of the section that plays.
    pub fn section(&self) -> &'static str {
        self.passage().name.as_str()
    }

    /// The bar of the section that plays, from 0 to 7.
    pub fn bar(&self) -> u8 {
        self.bar % BARS as u8
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// What starts on the next step, given the context at that moment.
    pub fn step(&mut self, context: Context) -> Step {
        let mut out = Step {
            muffled: context.muffled,
            ..Step::default()
        };
        let playing = context.place != Place::Menu;
        if playing && self.in_menu {
            // A run starts: its first fight opens the verse.
            self.next_phrase = 0;
        }
        self.in_menu = !playing;
        let fight = is_fight(context);
        let (roll_hits, link_steps) = (self.roll_hits(), self.link_steps());

        // A roll that is over gives way to the fight, and a connection to what follows the
        // fight, wherever the bar was.
        if self.roll.is_some_and(|roll| roll.done == roll_hits) {
            self.roll = None;
            if fight {
                self.enter_fight(context);
                out.fresh = true;
            }
        }
        if self.connection.is_some_and(|link| link.done == link_steps) {
            self.connection = None;
            self.enter(calm(context));
            out.fresh = true;
        }
        let between = self.roll.is_some() || self.connection.is_some();

        // On a bar line, what was decided since the last one takes effect. A tank that is
        // still destroyed keeps everything waiting, and so do a roll and a connection: the
        // section under them stands still.
        if self.step == 0 && !context.down && !between {
            let calm = calm(context);
            let elsewhere = !self.scene.is_fight() && self.scene != calm && !fight;
            if self.hush || elsewhere {
                self.hush = false;
                self.enter(calm);
                out.fresh = true;
            }
            // A fight that goes on takes the threat of the moment. One that has just ended
            // keeps its own for its connection.
            if self.scene.is_fight() && fight {
                self.arrange(context);
            }
        }

        // What can't wait for the bar line.
        let run_left = !playing && self.scene != Scene::Menu;
        if context.down || run_left {
            if !self.hush {
                if self.scene.is_fight() && self.connection.is_none() {
                    self.end_fight();
                }
                self.hush = true;
                self.roll = None;
                self.connection = None;
                out.bass = Some(self.root());
            }
        } else if self.scene.is_fight() && !fight {
            // The fight plays on to its next beat, so that the kick never stumbles. The
            // connection starts there, from the note the lead is on.
            let on_a_beat = self.step.is_multiple_of(BEAT as u8);
            if !self.hush && self.connection.is_none() && on_a_beat {
                self.end_fight();
                let playing = self.layer(Instrument::Lead).is_some();
                let lead = self.lead_on.filter(|_| playing);
                self.connection = Some(Connection {
                    line: score::connection(lead, self.score.piece.link),
                    lead: lead.is_some(),
                    from: self.pace(),
                    kick: self.kick(),
                    done: 0,
                });
            }
        } else if !self.scene.is_fight() && fight && !self.hush {
            if self.roll.is_none() {
                let roll = self.score.piece.roll;
                let arena = context.place == Place::Arena;
                self.roll = Some(Rolling {
                    drum: if arena { roll.arena } else { roll.drum },
                    done: 0,
                });
            }
        } else if !fight {
            // The fight was called off before it started: the section goes on from where it
            // stood.
            self.roll = None;
        }
        out.hush = self.hush;

        // A roll is at the tempo of the section that its fight opens on.
        let opening = self.opening(context);
        if let Some(rolling) = &mut self.roll {
            // Nothing else starts under the roll, which goes from one level to another.
            let roll = self.score.piece.roll;
            let (from, to) = (roll.from as i32, roll.to as i32);
            let level = (from + (to - from) * (rolling.done as i32 + 1) / roll_hits as i32) as u8;
            match rolling.drum {
                Instrument::Kick => out.kick = level,
                Instrument::Snare => out.snare = level,
                _ => out.hat = level,
            }
            rolling.done += 1;
            out.length = opening;
            return out;
        }
        if let Some(link) = self.connection {
            self.connect(link, calm(context), &mut out);
            return out;
        }

        if !self.hush {
            self.play(&mut out);
        }
        out.length = self.pace();

        self.step += 1;
        if self.step == STEPS as u8 {
            self.step = 0;
            self.bar = (self.bar + 1) % self.bars();
        }
        out
    }

    /// One step of a connection. The kick keeps the beat while it slows down, and the bass,
    /// the pluck and the lead go together, each in its own octave, to the degree on which the
    /// bridge stays.
    fn connect(&mut self, link: Connection, next: Scene, out: &mut Step) {
        let score = self.score;
        if link.done.is_multiple_of(BEAT as u8) {
            out.kick = link.kick;
        }
        let cell = link.line[link.done as usize];
        if cell.degree != 0 {
            let home = score.piece.key.home;
            let tone = |octave: u8| {
                Some(Tone {
                    pitch: score.tonic(octave) + home.semitones(cell.degree),
                    steps: cell.steps,
                    level: HIT,
                })
            };
            // A line that rises starts low in the scale, where the bass is an octave higher
            // so as not to go below its range.
            let rising = link.line[0].degree < score.piece.link.degree;
            out.bass = tone(if rising { 2 } else { 1 });
            out.pluck = tone(3);
            if link.lead {
                out.lead[0] = tone(4);
            }
        }
        // Each step is a little nearer the tempo of what follows than the one before.
        let first = score.piece.play(next).sections()[0];
        let (from, to) = (link.from as i64, score.pace(first) as i64);
        let length = from + (to - from) * (link.done as i64 + 1) / self.link_steps() as i64;
        out.length = length as u32;
        self.connection = Some(Connection {
            done: link.done + 1,
            ..link
        });
    }

    /// Starts the line of a scene without a fight from its first bar.
    fn enter(&mut self, scene: Scene) {
        self.scene = scene;
        self.bar = 0;
        self.step = 0;
        self.mode = self.score.piece.key.home;
        // Nobody fights: what such a scene leaves to the threat is what the lowest one has.
        self.threat = 0;
    }

    /// Starts the fight on the phrase after the one where the last fight ended.
    fn enter_fight(&mut self, context: Context) {
        self.scene = fight_scene(context);
        self.bar = self.next_phrase * BARS_IN_PHRASE % self.bars();
        self.step = 0;
        self.lead_on = None;
        self.arrange(context);
    }

    /// Notes where the fight ended, for the next one to go on from there.
    fn end_fight(&mut self) {
        let bars = self.bars();
        // On a bar line, the bar that the count shows has not played.
        let last = if self.step == 0 {
            (self.bar + bars - 1) % bars
        } else {
            self.bar
        };
        self.next_phrase = (last / BARS_IN_PHRASE + 1) % (bars / BARS_IN_PHRASE);
    }

    /// Sets what a bar of a fight is arranged for.
    fn arrange(&mut self, context: Context) {
        self.scene = fight_scene(context);
        // The two fights may not have as many sections.
        self.bar %= self.bars();
        self.threat = context.threat.min(HIGHEST_THREAT);
        if self.layer(Instrument::Lead).is_none() {
            // The lead is silent.
            self.lead_on = None;
        }
        let key = self.score.piece.key;
        self.mode = if self.scene == Scene::Arena {
            key.arena
        } else {
            key.home
        };
    }

    /// The line of the arrangement that plays.
    fn line(&self) -> &'static Play {
        self.score.piece.play(self.scene)
    }

    /// Bars in that line: eight for each of its sections.
    fn bars(&self) -> u8 {
        (self.line().sections().len() * BARS) as u8
    }

    /// The section that plays, by its place among those of the piece.
    fn index(&self) -> u8 {
        let sections = self.line().sections();
        sections[self.bar as usize / BARS % sections.len()]
    }

    fn passage(&self) -> &'static Passage {
        self.score.piece.section(self.index())
    }

    /// How long a step of the section that plays lasts, in half frames.
    fn pace(&self) -> u32 {
        self.score.pace(self.index())
    }

    /// How long a step lasts in the section that a fight would open on in this context.
    fn opening(&self, context: Context) -> u32 {
        let sections = self.score.piece.play(fight_scene(context)).sections();
        let bar = (self.next_phrase * BARS_IN_PHRASE) as usize;
        self.score.pace(sections[bar / BARS % sections.len()])
    }

    /// Steps that a roll and a connection last.
    fn roll_hits(&self) -> u8 {
        self.score.piece.roll.beats * BEAT as u8
    }

    fn link_steps(&self) -> u8 {
        self.score.piece.link.beats * BEAT as u8
    }

    /// The degree that the chord of the bar is built on. The bridge has no chords: it stays
    /// on one degree.
    fn chord(&self) -> u8 {
        let written = self.passage().chords[self.bar() as usize];
        self.mode.chord(written)
    }

    /// The root of the chord of the bar on the bass, for a whole bar.
    fn root(&self) -> Tone {
        Tone {
            pitch: score::bass_root(self.mode, self.chord(), self.score.tonic(2)),
            steps: STEPS as u8,
            level: HIT,
        }
    }

    /// What an instrument plays in the scene, at the threat of the bar.
    fn layer(&self, instrument: Instrument) -> Option<Layer> {
        match self.line().part(instrument) {
            Part::Silent => None,
            Part::ByThreat => self.score.piece.threat(self.threat, instrument),
            Part::Plays(layer) => Some(layer),
        }
    }

    /// The pattern that a drum plays in the scene.
    fn drum(&self, instrument: Instrument) -> Option<&'static Rhythm> {
        match self.layer(instrument)?.first {
            Source::Rhythm { index, .. } => Some(self.score.piece.pattern(index)),
            Source::Tune { .. } => None,
        }
    }

    /// How hard the kick of the scene is hit, or 0 if it has none.
    fn kick(&self) -> u8 {
        self.drum(Instrument::Kick).map_or(0, |rhythm| rhythm.level)
    }

    /// Starts what each instrument has on this step in the line of the arrangement.
    fn play(&mut self, out: &mut Step) {
        let level = |drum: Instrument| {
            let pattern = self.drum(drum);
            pattern.map_or(0, |rhythm| hit(rhythm, self.step))
        };
        out.kick = level(Instrument::Kick);
        out.hat = level(Instrument::Hat);
        out.snare = level(Instrument::Snare);

        if let Some(layer) = self.layer(Instrument::Bass) {
            out.bass = self.first(layer, Instrument::Bass).map(|(tone, ..)| tone);
        }
        if let Some(layer) = self.layer(Instrument::Pluck)
            && let Some((tone, echo, _)) = self.first(layer, Instrument::Pluck)
        {
            out.pluck = Some(tone);
            out.echo = echo;
        }
        if let Some(layer) = self.layer(Instrument::Lead)
            && let Some((tone, _, degree)) = self.first(layer, Instrument::Lead)
        {
            out.lead[0] = Some(tone);
            // The second line of the lead follows the first: it starts a note where the
            // first does.
            if let (true, Some(second)) = (layer.together, layer.second) {
                let second = self.start(second, Instrument::Lead);
                out.lead[1] = second.map(|(tone, ..)| tone);
            }
            if self.scene.is_fight() {
                self.lead_on = degree;
            }
        }
    }

    /// The note that a layer starts on this step, if it starts one: that of its first line,
    /// or that of a second line that sounds where the first starts nothing.
    fn first(&self, layer: Layer, instrument: Instrument) -> Option<Start> {
        let first = self.start(layer.first, instrument);
        match layer.second {
            Some(second) if !layer.together => first.or_else(|| self.start(second, instrument)),
            _ => first,
        }
    }

    /// The note that a line starts on this step, if it starts one.
    fn start(&self, source: Source, instrument: Instrument) -> Option<Start> {
        let score = self.score;
        match source {
            Source::Tune {
                form,
                octave,
                level,
            } => {
                let line = score.line(self.index(), form, self.bar() as usize);
                let cell = line[self.step as usize];
                (cell.degree != 0).then(|| {
                    let tone = Tone {
                        pitch: score.tonic(octave) + self.mode.semitones(cell.degree),
                        steps: cell.steps,
                        level,
                    };
                    (tone, false, Some(cell.degree))
                })
            }
            Source::Rhythm { index, octave } => {
                let rhythm = score.piece.pattern(index);
                let tonic = score.tonic(octave);
                let chord = if instrument == Instrument::Bass {
                    // The chord of the bar, built on the root that the bass has for it.
                    let root = score::bass_root(self.mode, self.chord(), tonic);
                    let [low, third, fifth] = score::triad(self.mode, self.chord(), 0);
                    [root, root + third - low, root + fifth - low]
                } else {
                    score::triad(self.mode, self.chord(), tonic)
                };
                let tone = self.voice(rhythm, chord, tonic)?;
                Some((tone, rhythm.echo, None))
            }
        }
    }

    /// The note of a pattern that starts on this step, if one does. A hit is the next note of
    /// `chord`, whose notes go up in turn. `R`, `O` and `V` are the root of that chord, that
    /// root an octave higher and its fifth, and a digit is a degree of the scale above
    /// `tonic`. The note lasts until the next one, or until a dash.
    fn voice(&self, rhythm: &Rhythm, chord: [u8; 3], tonic: u8) -> Option<Tone> {
        let step = self.step as usize;
        let symbol = rhythm.grid[step];
        let pitch = match symbol {
            b'x' | b'o' => {
                let before = rhythm.grid[..step].iter();
                chord[before
                    .filter(|symbol| matches!(symbol, b'x' | b'o'))
                    .count()
                    % 3]
            }
            b'R' => chord[0],
            b'O' => chord[0] + 12,
            b'V' => chord[2],
            b'1'..=b'9' => tonic + self.mode.semitones(symbol - b'0'),
            _ => return None,
        };
        let rest = rhythm.grid[step + 1..].iter();
        let steps = 1 + rest.take_while(|&&symbol| symbol == b'.').count() as u8;
        let level = if symbol == b'o' {
            rhythm.soft
        } else {
            rhythm.level
        };
        Some(Tone {
            pitch,
            steps,
            level,
        })
    }
}

/// Whether the context asks for the sections of a fight.
fn is_fight(context: Context) -> bool {
    context.fight && !context.down && context.place != Place::Menu
}

/// The scene of a context when nobody fights.
fn calm(context: Context) -> Scene {
    match context.place {
        Place::Menu | Place::Bay => Scene::Menu,
        Place::Corridor | Place::Arena => Scene::Calm,
    }
}

/// The scene of a context in which a fight is on.
fn fight_scene(context: Context) -> Scene {
    match context.place {
        Place::Arena => Scene::Arena,
        _ => Scene::Corridor,
    }
}

/// How hard a drum is hit on a step of its pattern.
fn hit(rhythm: &Rhythm, step: u8) -> u8 {
    match rhythm.grid[step as usize] {
        b'x' => rhythm.level,
        b'o' => rhythm.soft,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        notation::read,
        score::{CONNECTION_STEPS, HOME, PIECE},
    };

    const BAR: usize = STEPS;
    /// Steps that a roll and a connection last.
    const ROLL: usize = ROLL_HITS as usize;
    const LINK: usize = CONNECTION_STEPS as usize;
    /// How long a step lasts in a fight and in the bridge, in half frames.
    const STEP: u32 = pace("verse");
    const BRIDGE_STEP: u32 = pace("bridge");
    /// How hard the kick of a fight and the hi-hat of a corridor are hit, and how loud the
    /// tick of the bridge is.
    const KICK: u8 = pattern(Instrument::Kick, "fight").level;
    const HAT: u8 = pattern(Instrument::Hat, "corridor").level;
    const TICK: u8 = pattern(Instrument::Pluck, "tick").level;

    /// How long a step of a section of the piece lasts, in half frames.
    const fn pace(section: &str) -> u32 {
        match PIECE.passage(section) {
            Some(passage) => score::step(passage.tempo),
            None => panic!("the piece has no such section"),
        }
    }

    /// A pattern of the piece.
    const fn pattern(instrument: Instrument, name: &str) -> Rhythm {
        match PIECE.rhythm(instrument, name) {
            Some(rhythm) => rhythm,
            None => panic!("the piece has no such pattern"),
        }
    }

    fn at(place: Place) -> Context {
        Context {
            place,
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

    fn play(director: &mut Director, context: Context, steps: usize) -> Vec<Step> {
        (0..steps).map(|_| director.step(context)).collect()
    }

    fn drums(step: &Step) -> bool {
        step.kick > 0 || step.hat > 0 || step.snare > 0
    }

    fn pitched(step: &Step) -> bool {
        step.bass.is_some() || step.pluck.is_some() || step.lead[0].is_some()
    }

    /// The pitches that an instrument starts over some steps.
    fn pitches(steps: &[Step], tone: fn(&Step) -> Option<Tone>) -> Vec<u8> {
        steps.iter().flat_map(tone).map(|tone| tone.pitch).collect()
    }

    /// The steps of a bar on which something is true.
    fn on(bar: &[Step], is: fn(&Step) -> bool) -> Vec<usize> {
        (0..bar.len()).filter(|&step| is(&bar[step])).collect()
    }

    #[test]
    fn the_menu_and_the_bay_have_the_intro_without_drums() {
        for place in [Place::Menu, Place::Bay] {
            let mut director = Director::default();
            let steps = play(&mut director, at(place), 2 * BARS * BAR);
            assert_eq!(director.section(), "intro");
            assert!(steps.iter().all(|step| !drums(step)));
            assert!(steps.iter().all(|step| step.lead == [None; 2]));
            // Its melody has no echo, and it is at the tempo of a fight.
            assert!(steps.iter().all(|step| !step.echo && step.length == STEP));
            // The pluck has motif A and the bass holds the root of each bar.
            assert_eq!(
                pitches(&steps[..BAR], |step| step.pluck),
                [69, 71, 72, 76, 72, 71]
            );
            for (index, step) in steps.iter().enumerate() {
                assert_eq!(step.bass.is_some(), index % BAR == 0);
            }
            let bass = steps[0].bass.unwrap();
            assert_eq!((bass.pitch, bass.steps), (45, 16));
            // The loop is eight bars long.
            assert_eq!(steps[..BARS * BAR], steps[BARS * BAR..]);
        }
    }

    #[test]
    fn a_roll_of_two_beats_announces_a_fight() {
        for offset in 0..BAR {
            let mut director = Director::settled(at(Place::Corridor));
            let before = play(&mut director, at(Place::Corridor), BAR + offset);
            assert!(before.iter().all(|step| step.length == BRIDGE_STEP));
            let steps = play(&mut director, fight(Place::Corridor, 2), ROLL + BAR);
            // The roll is heard on the step where the fight starts. It grows louder on every
            // step, on the hi-hat alone, at the tempo of the fight.
            let roll: Vec<u8> = steps[..ROLL].iter().map(|step| step.hat).collect();
            assert!(roll[0] > 0, "offset {offset}");
            assert!(roll.windows(2).all(|pair| pair[0] < pair[1]), "{roll:?}");
            assert_eq!(roll[ROLL - 1], score::ROLL.to);
            for step in &steps[..ROLL] {
                assert!(step.kick == 0 && step.snare == 0 && !pitched(step) && !step.fresh);
                assert_eq!(step.length, STEP);
            }
            // Then the verse starts on its first beat, wherever the bar of the bridge was.
            let verse = &steps[ROLL..];
            assert!(verse[0].fresh, "offset {offset}");
            assert_eq!(on(verse, |step| step.kick == KICK), [0, 4, 8, 12]);
            assert_eq!(on(verse, |step| step.hat == HAT), [2, 6, 10, 14]);
            assert!(verse.iter().all(|step| step.length == STEP));
            assert_eq!(
                pitches(verse, |step| step.lead[0]),
                [69, 71, 72, 76, 72, 71]
            );
        }
    }

    #[test]
    fn the_intro_stops_under_the_roll_out_of_the_bay() {
        let mut director = Director::default();
        play(&mut director, at(Place::Bay), BAR + 14);
        let roll = play(&mut director, fight(Place::Corridor, 0), ROLL);
        assert!(roll.iter().all(|step| step.hat > 0 && !pitched(step)));
        let first = director.step(fight(Place::Corridor, 0));
        assert!(first.fresh);
        assert_eq!(first.kick, KICK);
        assert_eq!(director.section(), "verse");
    }

    #[test]
    fn an_arena_rolls_on_the_snare_and_darkens() {
        let mut director = Director::settled(at(Place::Arena));
        assert_eq!(director.mode(), HOME);
        play(&mut director, at(Place::Arena), 5);
        let roll = play(&mut director, fight(Place::Arena, 2), ROLL);
        assert!(roll.iter().all(|step| step.snare > 0 && step.hat == 0));
        assert_eq!(director.mode(), HOME);

        let bars = play(&mut director, fight(Place::Arena, 2), 2 * BAR);
        assert_eq!(director.mode(), Mode::Aeolian);
        assert_eq!(director.section(), "verse");
        // The second bar of motif A, with the F of Aeolian where Dorian has F sharp.
        assert_eq!(pitches(&bars[BAR..], |step| step.lead[0]), [72, 76, 77, 76]);
        // The snare is on beats 2 and 4 and the hi-hat on every step.
        assert_eq!(on(&bars[..BAR], |step| step.snare > 0), [4, 12]);
        assert!(bars.iter().all(|step| step.hat > 0));
        assert!(bars[2].hat > bars[1].hat);
    }

    #[test]
    fn each_mode_has_its_notes_and_its_chords() {
        // The second bar of the lead and the roots of the bass over the eight bars of the
        // verse.
        let verse = |place: Place| {
            let mut director = Director::settled(fight(place, 2));
            let steps = play(&mut director, fight(place, 2), BARS * BAR);
            let lead = pitches(&steps[BAR..2 * BAR], |step| step.lead[0]);
            let downbeats: Vec<Step> = steps.iter().step_by(BAR).copied().collect();
            (lead, pitches(&downbeats, |step| step.bass))
        };
        // A, G, D and E under the piece, and F sharp in its tune.
        let piece = (vec![72, 76, 78, 76], vec![45, 45, 43, 50, 45, 45, 43, 40]);
        assert_eq!(verse(Place::Corridor), piece);
        // An arena has F for both: in the tune, and in place of G under it.
        let arena = (vec![72, 76, 77, 76], vec![45, 45, 41, 50, 45, 45, 41, 40]);
        assert_eq!(verse(Place::Arena), arena);
    }

    #[test]
    fn a_connection_starts_on_the_next_beat_and_leads_into_the_bridge() {
        // Wherever the fight ends in its bar. Its lead is silent, so the connection is the
        // fall, on the pluck and the bass.
        for offset in 0..BAR {
            let mut director = Director::settled(fight(Place::Corridor, 0));
            play(&mut director, fight(Place::Corridor, 0), BAR + offset);
            let wait = (4 - offset % 4) % 4;
            let steps = play(&mut director, at(Place::Corridor), wait + LINK + BAR);
            // The fight plays on to its next beat, with its drums.
            for (index, step) in steps[..wait].iter().enumerate() {
                let at = (offset + index) as u8;
                assert_eq!(
                    (step.kick, step.hat),
                    (0, hit(&pattern(Instrument::Hat, "corridor"), at))
                );
                assert!(!step.fresh && step.length == STEP, "offset {offset}");
            }
            // The connection starts on that beat: its kick is where the kick of the fight
            // would be, and nothing else drums.
            let link = &steps[wait..wait + LINK];
            assert_eq!(
                on(link, |step| step.kick == KICK),
                [0, 4],
                "offset {offset}"
            );
            assert!(link.iter().all(|step| step.hat == 0 && step.snare == 0));
            assert!(link.iter().all(|step| !step.fresh && !step.hush));
            assert_eq!(pitches(link, |step| step.pluck), [69, 67, 66, 64]);
            assert_eq!(pitches(link, |step| step.bass), [45, 43, 42, 40]);
            assert!(link.iter().all(|step| step.lead == [None; 2]));
            // It slows down from the tempo of the fight to the tempo of the bridge.
            let lengths: Vec<u32> = link.iter().map(|step| step.length).collect();
            assert!(lengths[0] > STEP && lengths[LINK - 1] == BRIDGE_STEP);
            assert!(
                lengths.windows(2).all(|pair| pair[0] < pair[1]),
                "{lengths:?}"
            );
            // Then the bridge starts on its first beat.
            let bridge = &steps[wait + LINK..];
            assert!(bridge[0].fresh, "offset {offset}");
            assert_eq!(bridge[0].kick, pattern(Instrument::Kick, "heart").level);
            assert!(bridge.iter().all(|step| step.length == BRIDGE_STEP));
            assert_eq!(director.section(), "bridge");
            assert_eq!(director.mode(), HOME);
        }
    }

    #[test]
    fn a_connection_starts_from_the_note_the_lead_is_on() {
        // What the lead, the pluck and the bass play in the connection, and the steps on which
        // the lead starts a note, for a fight that is seen to be over after some steps.
        let link = |place: Place, threat: u8, fought: usize| {
            let mut director = Director::settled(fight(place, threat));
            play(&mut director, fight(place, threat), fought);
            let wait = (4 - fought % 4) % 4;
            let steps = play(&mut director, at(place), wait + LINK);
            let link = &steps[wait..];
            (
                pitches(link, |step| step.lead[0]),
                on(link, |step| step.lead[0].is_some()),
                pitches(link, |step| step.pluck),
                pitches(link, |step| step.bass),
            )
        };
        let (verse, chorus) = (0, BARS * BAR);
        let corridor = Place::Corridor;

        // The verse has motif A: A, B, C, E, C, B, then C, E, F sharp, E. From C, the lead
        // rises to E by steps of the scale, and the pluck and the bass with it.
        let rise = (
            vec![72, 74, 76],
            vec![0, 2, 4],
            vec![60, 62, 64],
            vec![48, 50, 52],
        );
        assert_eq!(link(corridor, 2, verse + 8), rise);
        // A fight that ends between two beats gets the same: it plays on to the beat.
        assert_eq!(link(corridor, 2, verse + 7), rise);
        // From B it rises through four notes, and from the tonic it starts on that B too.
        let long_rise = (
            vec![71, 72, 74, 76],
            vec![0, 2, 4, 6],
            vec![59, 60, 62, 64],
            vec![47, 48, 50, 52],
        );
        assert_eq!(link(corridor, 2, verse + 4), long_rise);
        assert_eq!(link(corridor, 1, verse + 4), long_rise);
        // From E, the lead is there already: it stays for the two beats.
        let stay = (vec![76], vec![0], vec![64], vec![40]);
        assert_eq!(link(corridor, 2, verse + BAR + 4), stay);
        // From F sharp, one step down, a beat each.
        let step_down = (vec![78, 76], vec![0, 4], vec![66, 64], vec![42, 40]);
        assert_eq!(link(corridor, 2, verse + BAR + 8), step_down);
        // In an arena the lead is on F there, and the F sharp of the piece comes back first.
        assert_eq!(link(Place::Arena, 2, verse + BAR + 8), step_down);

        // The chorus has motif B, which starts on the upper A and touches the B above it:
        // from either, the fall.
        let fall = (
            vec![81, 79, 78, 76],
            vec![0, 2, 4, 6],
            vec![69, 67, 66, 64],
            vec![45, 43, 42, 40],
        );
        assert_eq!(link(corridor, 2, chorus + 4), fall);
        assert_eq!(link(corridor, 2, chorus + 8), fall);
    }

    #[test]
    fn the_next_fight_goes_on_from_where_the_last_one_ended() {
        let mut director = Director::settled(fight(Place::Corridor, 1));
        play(&mut director, fight(Place::Corridor, 1), 2 * BAR);
        // The two bars that played were in the first phrase, so the next fight opens the
        // second.
        director.step(at(Place::Corridor));
        assert_eq!(director.next_phrase, 1);
    }

    #[test]
    fn a_bar_of_the_bridge_is_four_pairs_over_a_tick() {
        let mut director = Director::settled(at(Place::Corridor));
        let steps = play(&mut director, at(Place::Corridor), 2 * BARS * BAR);
        assert_eq!(director.section(), "bridge");
        // It is slower than the rest, and nothing has an echo.
        assert!(
            steps
                .iter()
                .all(|step| step.length == BRIDGE_STEP && !step.echo)
        );
        assert!(steps.iter().all(|step| step.hat == 0 && step.snare == 0));
        for bar in steps.chunks(BAR) {
            // One pair per beat: the heart, the call, the bass and the answer.
            assert_eq!(on(bar, |step| step.kick > 0), [0, 2]);
            assert!(bar[0].kick > bar[2].kick);
            assert_eq!(
                on(bar, |step| step.pluck.is_some_and(|tone| tone.level == HIT)),
                [4, 6]
            );
            assert_eq!(on(bar, |step| step.bass.is_some()), [8, 10]);
            assert_eq!(pitches(bar, |step| step.bass), [40, 40]);
            assert_eq!(on(bar, |step| step.lead[0].is_some()), [12, 14]);
            // Under them the pluck ticks on every half beat, between E and the B above it.
            let ticks: Vec<(usize, u8)> = (0..BAR)
                .filter_map(|step| Some((step, bar[step].pluck?)))
                .filter(|(_, tone)| tone.level == TICK)
                .map(|(step, tone)| (step, tone.pitch))
                .collect();
            assert_eq!(
                ticks,
                [(0, 52), (2, 59), (8, 52), (10, 59), (12, 52), (14, 59)]
            );
        }
        // The call starts on E and goes to a note that changes from bar to bar, and the lead
        // gives the two back in the other order.
        let loud = |step: &Step| step.pluck.filter(|tone| tone.level == HIT);
        let calls = pitches(&steps[..BARS * BAR], loud);
        assert_eq!(
            calls,
            [
                64, 69, 64, 67, 64, 69, 64, 71, 64, 69, 64, 67, 64, 69, 64, 62
            ]
        );
        let answers = pitches(&steps[..2 * BAR], |step| step.lead[0]);
        assert_eq!(answers, [81, 76, 79, 76]);
        assert_eq!(steps[12].lead[0].map(|tone| tone.level), Some(150));
        assert_eq!(steps[..BARS * BAR], steps[BARS * BAR..]);
    }

    #[test]
    fn the_lead_follows_the_threat() {
        let lead = |threat: u8| {
            let mut director = Director::settled(fight(Place::Corridor, threat));
            let bar = play(&mut director, fight(Place::Corridor, threat), BAR);
            let notes = bar.iter().filter(|step| step.lead[0].is_some()).count();
            let octaves = bar.iter().any(|step| step.lead[1].is_some());
            let pluck = bar.iter().filter(|step| step.pluck.is_some()).count();
            // Every note of the arpeggio has its echo.
            assert!(bar.iter().all(|step| step.echo == step.pluck.is_some()));
            (notes, octaves, pitches(&bar, |step| step.bass), pluck)
        };
        let plain = vec![45; 6];
        let leaping = vec![45, 57, 45, 45, 57, 52];
        // Silent, the skeleton, the motif, its ornaments, and its ornaments in octaves.
        assert_eq!(lead(0), (0, false, plain.clone(), 8));
        assert_eq!(lead(1), (2, false, plain, 8));
        assert_eq!(lead(2), (6, false, leaping.clone(), 8));
        assert_eq!(lead(3), (14, false, leaping.clone(), 8));
        assert_eq!(lead(4), (14, true, leaping, 16));
    }

    #[test]
    fn the_threat_changes_on_the_bar_line() {
        let mut director = Director::settled(fight(Place::Corridor, 0));
        play(&mut director, fight(Place::Corridor, 0), 3);
        let rest = play(&mut director, fight(Place::Corridor, 2), BAR - 3);
        assert!(rest.iter().all(|step| step.lead == [None; 2]));
        let next = play(&mut director, fight(Place::Corridor, 2), BAR);
        assert!(next[0].lead[0].is_some());
        assert!(!next[0].fresh);
    }

    #[test]
    fn fights_walk_through_the_piece() {
        let mut director = Director::default();
        play(&mut director, at(Place::Bay), BAR);
        let mut opened = Vec::new();
        // Six short fights of two bars, each after its roll, with a calm between them.
        for _ in 0..6 {
            let steps = play(&mut director, fight(Place::Corridor, 2), ROLL + 2 * BAR);
            assert_eq!(steps.iter().position(|step| step.fresh), Some(ROLL));
            opened.push((director.section(), director.bar()));
            play(&mut director, at(Place::Corridor), BAR);
        }
        // Each fight plays the first two bars of the next phrase, so the bar that comes after
        // them is the third of that phrase.
        let (verse, chorus) = ("verse", "chorus");
        assert_eq!(
            opened,
            [
                (verse, 2),
                (verse, 6),
                (chorus, 2),
                (chorus, 6),
                (verse, 2),
                (verse, 6)
            ]
        );

        // A long fight plays the verse and the chorus in turn.
        let mut director = Director::settled(fight(Place::Corridor, 2));
        let mut sections = Vec::new();
        for _ in 0..4 {
            sections.push(director.section());
            play(&mut director, fight(Place::Corridor, 2), BARS * BAR);
        }
        assert_eq!(sections, [verse, chorus, verse, chorus]);
    }

    #[test]
    fn a_run_starts_over_with_the_verse() {
        let mut director = Director::default();
        play(&mut director, at(Place::Bay), BAR);
        play(&mut director, fight(Place::Corridor, 1), ROLL + 2 * BAR);
        // The run ends and another starts: the intro goes on, and nothing marks it.
        play(&mut director, at(Place::Menu), 2 * BAR);
        assert_eq!(director.section(), "intro");
        let steps = play(&mut director, at(Place::Bay), BAR);
        assert!(steps.iter().all(|step| !step.fresh && !step.hush));
        play(&mut director, fight(Place::Corridor, 1), ROLL);
        director.step(fight(Place::Corridor, 1));
        assert_eq!((director.section(), director.bar()), ("verse", 0));
    }

    #[test]
    fn a_destroyed_tank_leaves_the_bass_alone() {
        let mut director = Director::settled(fight(Place::Corridor, 2));
        play(&mut director, fight(Place::Corridor, 2), 5);
        let down = Context {
            down: true,
            ..fight(Place::Corridor, 2)
        };
        // The tank is gone for a bar and a half: nothing starts but one note of the bass.
        let steps = play(&mut director, down, BAR + 8);
        assert!(steps[0].hush);
        assert!(steps[0].bass.is_some());
        assert!(steps[1..].iter().all(|step| !drums(step) && !pitched(step)));
        assert!(steps.iter().all(|step| step.hush));
        // It comes back at its checkpoint, where nobody fights: the bridge, on the bar line.
        let steps = play(&mut director, at(Place::Corridor), BAR);
        let start = steps.iter().position(|step| step.fresh).unwrap();
        assert_eq!(start, 3);
        assert!(
            steps[..start]
                .iter()
                .all(|step| !drums(step) && !pitched(step))
        );
        assert_eq!(director.section(), "bridge");
    }

    #[test]
    fn the_end_of_a_run_leads_to_the_intro() {
        let mut director = Director::settled(fight(Place::Arena, 4));
        play(&mut director, fight(Place::Arena, 4), 6);
        let steps = play(&mut director, at(Place::Menu), BAR);
        assert!(steps[0].hush);
        assert!(steps[10].fresh);
        assert_eq!(director.section(), "intro");
        assert_eq!(director.mode(), HOME);
    }

    #[test]
    fn a_fight_that_is_called_off_stops_its_roll() {
        let mut director = Director::settled(at(Place::Corridor));
        play(&mut director, at(Place::Corridor), 2);
        let steps = play(&mut director, fight(Place::Corridor, 1), 4);
        assert!(steps.iter().all(|step| step.hat > 0));
        // The bridge goes on from where it stood: the second beat of its heart comes next.
        let steps = play(&mut director, at(Place::Corridor), 2 * BAR);
        assert!(steps.iter().all(|step| step.hat == 0 && !step.fresh));
        assert_eq!(steps[0].kick, pattern(Instrument::Kick, "heart").soft);
        assert_eq!(director.section(), "bridge");
    }

    #[test]
    fn a_menu_muffles_at_once() {
        let mut director = Director::settled(fight(Place::Corridor, 1));
        let paused = Context {
            muffled: true,
            ..fight(Place::Corridor, 1)
        };
        assert!(!director.step(fight(Place::Corridor, 1)).muffled);
        assert!(director.step(paused).muffled);
    }

    /// A piece of its own, which arranges what the piece of the game doesn't: a lead in the
    /// menu, two sections in turn when nobody fights, a drum that follows the threat, a line of
    /// the bass under another, two fights of different lengths, and a roll that dies away.
    const OTHER: &str = "tanku-music 1\n\
                         tempo 120\n\
                         key A dorian aeolian\n\
                         motif A 1... .... .... ....\n\
                         motif B 1..2 .... 3-.. .... | 5... .... .... 6...\n\
                         pat kick four x... x... x... x... 200\n\
                         pat hat off ..x. ..x. ..x. ..x. 90\n\
                         pat snare back .... x... .... x... 150\n\
                         pat bass roots R... .... R... .... 250\n\
                         pat bass walk 1.2. 3.5. 8.5. 3.2. 240\n\
                         pat pluck up x.x. x.x. x.x. x.x. 120 echo\n\
                         pat pluck drone 5... .... 5... .... 80\n\
                         sec one 1111 1111 A A A A A A A A\n\
                         sec two 4455 4411 B B B B 90\n\
                         play menu one bass=roots@2 lead=melody@4\n\
                         play calm two,one kick=four hat=* pluck=melody@3/drone@2\n\
                         play corridor one,two snare=back hat=* bass=roots@2/walk@1 lead=*\n\
                         play arena two bass=roots@2 pluck=up@3 lead=melody@4+skeleton@5:90\n\
                         threat 0 hat=off\n\
                         threat 1 lead=skeleton@4\n\
                         threat 2 hat=off lead=melody@4\n\
                         threat 3 lead=ornate@4\n\
                         threat 4 hat=off lead=ornate@4:200\n\
                         roll 1 kick kick 255 51\n\
                         link 2 5 4\n";

    /// That piece, as a director plays it.
    fn other() -> &'static Score {
        Box::leak(Box::new(Score::new(read(OTHER).unwrap())))
    }

    /// What an instrument starts on each step where it starts a note: the step, the pitch,
    /// the length and the level.
    fn notes(steps: &[Step], tone: fn(&Step) -> Option<Tone>) -> Vec<(usize, u8, u8, u8)> {
        let starts = steps.iter().enumerate();
        let note = |(step, tone): (usize, Tone)| (step, tone.pitch, tone.steps, tone.level);
        starts
            .filter_map(|(at, step)| Some((at, tone(step)?)))
            .map(note)
            .collect()
    }

    #[test]
    fn a_scene_plays_what_its_line_says() {
        // The menu has a lead, which the menu of the game doesn't, and no pluck.
        let mut director = Director::of(other());
        let bar = play(&mut director, at(Place::Menu), BAR);
        assert_eq!(director.section(), "one");
        assert_eq!(notes(&bar, |step| step.lead[0]), [(0, 69, 16, 255)]);
        assert_eq!(
            notes(&bar, |step| step.bass),
            [(0, 45, 8, 250), (8, 45, 8, 250)]
        );
        assert!(bar.iter().all(|step| !drums(step) && step.pluck.is_none()));

        // A fight in a corridor has a snare and no kick. Its bass has its roots, and walks
        // where they start nothing: a tone above the tonic, then up the scale and back.
        let fight = |threat: u8| fight(Place::Corridor, threat);
        let mut director = Director::settled_in(other(), fight(0));
        let bar = play(&mut director, fight(0), BAR);
        assert_eq!(on(&bar, |step| step.snare == 150), [4, 12]);
        assert!(bar.iter().all(|step| step.kick == 0));
        let bass = notes(&bar, |step| step.bass);
        let pitches: Vec<u8> = bass.iter().map(|note| note.1).collect();
        assert_eq!(pitches, [45, 35, 36, 40, 45, 40, 36, 35]);
        assert_eq!((bass[0], bass[1]), ((0, 45, 8, 250), (2, 35, 2, 240)));
    }

    #[test]
    fn the_threat_sets_what_a_scene_leaves_to_it() {
        // The hi-hat of a fight in a corridor plays at every other threat, and the lead of
        // the highest threat has a level of its own.
        let heard = |threat: u8| {
            let fight = fight(Place::Corridor, threat);
            let mut director = Director::settled_in(other(), fight);
            let bar = play(&mut director, fight, BAR);
            let lead = notes(&bar, |step| step.lead[0]);
            (on(&bar, |step| step.hat == 90), lead)
        };
        let off = vec![2, 6, 10, 14];
        assert_eq!(heard(0), (off.clone(), vec![]));
        assert_eq!(heard(1), (vec![], vec![(0, 69, 16, 255)]));
        assert_eq!(heard(2), (off.clone(), vec![(0, 69, 16, 255)]));
        // The ornament of a long note: the degree above it, then the note again.
        let ornate = |level| vec![(0, 69, 14, level), (14, 71, 1, level), (15, 69, 1, level)];
        assert_eq!(heard(3), (vec![], ornate(255)));
        assert_eq!(heard(4), (off.clone(), ornate(200)));

        // When nobody fights, the threat is the lowest one, whatever the last fight was.
        let mut director = Director::settled_in(other(), fight(Place::Corridor, 3));
        play(&mut director, fight(Place::Corridor, 3), BAR);
        let calm = play(&mut director, at(Place::Corridor), LINK + 2 * BAR);
        let bar = &calm[LINK + BAR..];
        assert_eq!(on(bar, |step| step.hat == 90), off);
    }

    #[test]
    fn a_scene_plays_its_sections_in_turn_each_at_its_tempo() {
        let mut director = Director::settled_in(other(), at(Place::Corridor));
        let mut heard = Vec::new();
        for _ in 0..4 {
            let section = director.section();
            let bars = play(&mut director, at(Place::Corridor), BARS * BAR);
            heard.push((section, bars[0].length, bars[BARS * BAR - 1].length));
        }
        // Eight bars of one section at 90 beats per minute, then eight of the other at 120.
        let (slow, fast) = (score::step(90), score::step(120));
        let turn = [("two", slow, slow), ("one", fast, fast)];
        assert_eq!(heard, [turn[0], turn[1], turn[0], turn[1]]);

        // The pluck has the melody, and its other line where the melody starts nothing: in
        // the second bar of the motif, on the third beat.
        let mut director = Director::settled_in(other(), at(Place::Corridor));
        let bars = play(&mut director, at(Place::Corridor), 2 * BAR);
        let melody = [(0, 57, 3, 255), (3, 59, 5, 255), (8, 60, 1, 255)];
        assert_eq!(notes(&bars[..BAR], |step| step.pluck), melody);
        let under = [(0, 64, 12, 255), (8, 52, 8, 80), (12, 66, 4, 255)];
        assert_eq!(notes(&bars[BAR..], |step| step.pluck), under);
        // Neither is a pattern with an echo.
        assert!(bars.iter().all(|step| !step.echo));
    }

    #[test]
    fn the_second_line_of_the_lead_follows_the_first() {
        // In the arena, the lead has the melody and its skeleton an octave higher: the
        // skeleton starts a note where it has one and the melody starts one too.
        let arena = fight(Place::Arena, 2);
        let mut director = Director::settled_in(other(), arena);
        let bar = play(&mut director, arena, BAR);
        assert_eq!(on(&bar, |step| step.lead[0].is_some()), [0, 3, 8]);
        let second = notes(&bar, |step| step.lead[1]);
        assert_eq!(second, [(0, 81, 8, 90), (8, 84, 8, 90)]);
        // The arpeggio of the pluck has its echo.
        assert_eq!(on(&bar, |step| step.echo), [0, 2, 4, 6, 8, 10, 12, 14]);
        assert_eq!(director.mode(), Mode::Aeolian);
    }

    #[test]
    fn a_roll_and_a_connection_follow_what_the_file_says_of_them() {
        // A roll of one beat on the kick, which dies away, at the tempo of the fight.
        let mut director = Director::settled_in(other(), at(Place::Corridor));
        play(&mut director, at(Place::Corridor), 3);
        let roll = play(&mut director, fight(Place::Corridor, 2), 4);
        let levels: Vec<u8> = roll.iter().map(|step| step.kick).collect();
        assert_eq!(levels, [204, 153, 102, 51]);
        assert!(roll.iter().all(|step| step.hat == 0 && step.snare == 0));
        assert!(roll.iter().all(|step| step.length == score::step(120)));
        let first = director.step(fight(Place::Corridor, 2));
        assert!(first.fresh);
        assert_eq!(director.section(), "one");

        // The fight has no kick, so its connection has none either. It slows down to the
        // tempo of the first section of the calm, which is the slow one.
        play(&mut director, fight(Place::Corridor, 2), 3);
        let link = play(&mut director, at(Place::Corridor), LINK);
        assert!(link.iter().all(|step| !drums(step)));
        // The lead was on the tonic: the connection rises to degree 5 from the note above.
        assert_eq!(pitches(&link, |step| step.lead[0]), [71, 72, 74, 76]);
        let lengths: Vec<u32> = link.iter().map(|step| step.length).collect();
        assert!(lengths.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(lengths[LINK - 1], score::step(90));
        let next = director.step(at(Place::Corridor));
        assert!(next.fresh);
        assert_eq!((director.section(), next.length), ("two", score::step(90)));
    }

    #[test]
    fn two_fights_can_have_different_lengths() {
        // A fight in a corridor has sixteen bars, and one in an arena has eight.
        let mut director = Director::settled_in(other(), fight(Place::Corridor, 0));
        // It ends in its fourth phrase, so the next fight opens on the first one.
        play(&mut director, fight(Place::Corridor, 0), 13 * BAR);
        assert_eq!(director.section(), "two");
        play(&mut director, at(Place::Corridor), LINK + BAR);
        play(&mut director, fight(Place::Arena, 0), 4);
        director.step(fight(Place::Arena, 0));
        assert_eq!((director.section(), director.bar()), ("two", 0));

        // A fight in an arena that ends in its second phrase leaves the count on a phrase
        // that the corridor has too.
        play(&mut director, fight(Place::Arena, 0), 5 * BAR);
        play(&mut director, at(Place::Arena), LINK + BAR);
        play(&mut director, fight(Place::Corridor, 0), 4);
        director.step(fight(Place::Corridor, 0));
        assert_eq!((director.section(), director.bar()), ("one", 0));
        // Whatever the game does, nothing fails: the two fights in turn, bar by bar.
        for bar in 0..40 {
            let place = [Place::Corridor, Place::Arena][bar % 2];
            play(&mut director, fight(place, (bar % 5) as u8), BAR);
        }
    }
}
