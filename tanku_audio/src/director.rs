//! Chooses what plays. The game gives a context, and on every step the director says what each
//! instrument starts.
//!
//! Three rules make the music answer fast and stay in time. A change within a fight, such as
//! a new threat, waits for the bar line. A fight has a way in and a way out, which last two
//! beats each: a roll announces it at once, and a connection leads its tune, from the next
//! beat on, to where the bridge begins. And what follows a roll or a connection starts on its
//! own first beat, wherever the bar was: the bridge has a tempo of its own, so nothing has to
//! line up.
//!
//! All of it is integer logic, so it gives the same answer on every platform.

use crate::score::{
    self, A1, A2, A3, A4, BARS, BASS_LEAPING, BASS_LONG, BASS_PLAIN, BRIDGE, BRIDGE_ANSWERS,
    BRIDGE_BASS, BRIDGE_STEP, CHORUS, CONNECTION_STEPS, HAT_ARENA, HAT_CORRIDOR, KICK_FIGHT,
    KICK_HEART, Line, Mode, PEDAL, PLUCK_ARPEGGIO, Pattern, SNARE_ARENA, STEP, STEPS, TICK, VERSE,
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

const HIGHEST_THREAT: u8 = 4;

/// The mode of the piece, which every section is in but the fights of an arena.
const HOME: Mode = Mode::Dorian;

/// Eight bars with their chords and their melody.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Intro,
    Verse,
    Chorus,
    Bridge,
}

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

/// What the lead plays in the verse and in the chorus, for each threat from 1 to 4. At threat
/// 4 it also plays the same line an octave higher.
static LEAD: [[[Line; BARS]; 2]; 4] = [
    [score::skeletons(VERSE), score::skeletons(CHORUS)],
    [VERSE, CHORUS],
    [score::ornamented(VERSE), score::ornamented(CHORUS)],
    [score::ornamented(VERSE), score::ornamented(CHORUS)],
];

const HIT: u8 = 255;
/// The arpeggio of the pluck stays under the melody.
const ARPEGGIO: u8 = 150;
/// The octave that the lead adds at the highest threat is quieter than the line itself.
const OCTAVE: u8 = 130;
/// The two beats of the heart in the bridge: the kick is softer there than in a fight, and
/// its second beat is softer than its first.
const HEART: (u8, u8) = (180, 135);
/// The tick of the bridge stays far under its call, and the answer of the lead under both
/// the call and the lines that the lead has in a fight.
const TICK_LEVEL: u8 = 110;
const ANSWER: u8 = 150;
const HAT: u8 = 190;
/// The hi-hat of an arena plays every step, and louder where the corridor's plays.
const ARENA_HATS: (u8, u8) = (100, 200);
const SNARE: u8 = 230;
/// A roll starts this loud and ends at full strength.
const ROLL_START: u16 = 40;
/// A roll lasts two beats, at the tempo of the fight it announces.
pub const ROLL_HITS: u8 = 8;
/// Phrases that the fights walk through: two for the verse and two for the chorus.
const PHRASES: u8 = 4;
const BARS_IN_PHRASE: u8 = 4;

/// What loops: the intro, the bridge, or the verse followed by the chorus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Intro,
    Bridge,
    Fight,
}

impl Part {
    fn bars(self) -> u8 {
        match self {
            Part::Intro | Part::Bridge => BARS as u8,
            Part::Fight => 2 * BARS as u8,
        }
    }

    /// How long a step of the part lasts, in half frames.
    fn step(self) -> u32 {
        match self {
            Part::Intro | Part::Fight => STEP,
            Part::Bridge => BRIDGE_STEP,
        }
    }
}

/// The roll that announces a fight.
#[derive(Clone, Copy, Debug)]
struct Roll {
    /// On the snare in an arena, on the hi-hat elsewhere.
    snare: bool,
    done: u8,
}

/// The connection that leads from a fight into what follows.
#[derive(Clone, Copy, Debug)]
struct Connection {
    /// The way from the note the lead was on to the degree on which the bridge stays.
    line: Line,
    /// Whether the lead goes that way with the pluck and the bass: it does if it was playing.
    lead: bool,
    done: u8,
}

#[derive(Clone, Debug)]
pub struct Director {
    part: Part,
    /// The bar of the part that plays. A fight counts the 8 bars of the verse, then the 8 of
    /// the chorus.
    bar: u8,
    /// The step of the bar that comes next.
    step: u8,
    mode: Mode,
    /// What the bar that plays was arranged for.
    arena: bool,
    threat: u8,
    /// The phrase that the next fight opens on.
    next_phrase: u8,
    roll: Option<Roll>,
    connection: Option<Connection>,
    /// The degree of the last note that the lead started in the fight, if the lead is playing.
    lead_on: Option<u8>,
    /// Everything has stopped but the bass, until the next bar line.
    hush: bool,
    /// Whether the last step was outside a run, to tell when one starts.
    in_menu: bool,
}

/// A director at the start of the piece, in the main menu.
impl Default for Director {
    fn default() -> Self {
        Self {
            part: Part::Intro,
            bar: 0,
            step: 0,
            mode: HOME,
            arena: false,
            threat: 0,
            next_phrase: 0,
            roll: None,
            connection: None,
            lead_on: None,
            hush: false,
            in_menu: true,
        }
    }
}

impl Director {
    /// A director for a game that has been in this context for a long time: it is in the
    /// section of the context, and nothing announces it.
    pub fn settled(context: Context) -> Self {
        let mut director = Self {
            in_menu: context.place == Place::Menu,
            ..Self::default()
        };
        if is_fight(context) {
            director.enter_fight(context);
        } else {
            director.enter(calm_part(context));
        }
        director
    }

    pub fn section(&self) -> Section {
        match self.part {
            Part::Intro => Section::Intro,
            Part::Bridge => Section::Bridge,
            Part::Fight if self.bar < BARS as u8 => Section::Verse,
            Part::Fight => Section::Chorus,
        }
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

        // A roll that is over gives way to the fight, and a connection to what follows the
        // fight, wherever the bar was.
        if self.roll.is_some_and(|roll| roll.done == ROLL_HITS) {
            self.roll = None;
            if fight {
                self.enter_fight(context);
                out.fresh = true;
            }
        }
        if self
            .connection
            .is_some_and(|link| link.done == CONNECTION_STEPS)
        {
            self.connection = None;
            self.enter(calm_part(context));
            out.fresh = true;
        }
        let between = self.roll.is_some() || self.connection.is_some();

        // On a bar line, what was decided since the last one takes effect. A tank that is
        // still destroyed keeps everything waiting, and so do a roll and a connection: the
        // section under them stands still.
        if self.step == 0 && !context.down && !between {
            let calm = calm_part(context);
            let elsewhere = self.part != Part::Fight && self.part != calm && !fight;
            if self.hush || elsewhere {
                self.hush = false;
                self.enter(calm);
                out.fresh = true;
            }
            // A fight that goes on takes the threat of the moment. One that has just ended
            // keeps its own for its connection.
            if self.part == Part::Fight && fight {
                self.arrange(context);
            }
        }

        // What can't wait for the bar line.
        let run_left = !playing && self.part != Part::Intro;
        if context.down || run_left {
            if !self.hush {
                if self.part == Part::Fight && self.connection.is_none() {
                    self.end_fight();
                }
                self.hush = true;
                self.roll = None;
                self.connection = None;
                out.bass = Some(self.root());
            }
        } else if self.part == Part::Fight && !fight {
            // The fight plays on to its next beat, so that the kick never stumbles. The
            // connection starts there, from the note the lead is on.
            let on_a_beat = self.step.is_multiple_of(4);
            if !self.hush && self.connection.is_none() && on_a_beat {
                self.end_fight();
                let lead = self.lead_on.filter(|_| self.threat > 0);
                self.connection = Some(Connection {
                    line: score::connection(lead),
                    lead: lead.is_some(),
                    done: 0,
                });
            }
        } else if self.part != Part::Fight && fight && !self.hush {
            if self.roll.is_none() {
                self.roll = Some(Roll {
                    snare: context.place == Place::Arena,
                    done: 0,
                });
            }
        } else if !fight {
            // The fight was called off before it started: the section goes on from where it
            // stood.
            self.roll = None;
        }
        out.hush = self.hush;

        if let Some(roll) = &mut self.roll {
            // Nothing else starts under the roll, which is at the tempo of the fight.
            let rise = (HIT as u16 - ROLL_START) * (roll.done as u16 + 1) / ROLL_HITS as u16;
            let level = (ROLL_START + rise) as u8;
            if roll.snare {
                out.snare = level;
            } else {
                out.hat = level;
            }
            roll.done += 1;
            out.length = STEP;
            return out;
        }
        if let Some(link) = self.connection {
            self.connect(link, calm_part(context), &mut out);
            return out;
        }

        if !self.hush {
            match self.part {
                Part::Intro => self.intro(&mut out),
                Part::Bridge => self.bridge(&mut out),
                Part::Fight => self.fight(&mut out),
            }
        }
        out.length = self.part.step();

        self.step += 1;
        if self.step == STEPS as u8 {
            self.step = 0;
            self.bar = (self.bar + 1) % self.part.bars();
        }
        out
    }

    /// One step of a connection. The kick keeps the beat while it slows down, and the bass,
    /// the pluck and the lead go together, each in its own octave, to the degree on which the
    /// bridge stays.
    fn connect(&mut self, link: Connection, next: Part, out: &mut Step) {
        if link.done.is_multiple_of(4) {
            out.kick = HIT;
        }
        let cell = link.line[link.done as usize];
        if cell.degree != 0 {
            let tone = |tonic: u8| {
                Some(Tone {
                    pitch: tonic + HOME.semitones(cell.degree),
                    steps: cell.steps,
                    level: HIT,
                })
            };
            // A line that rises starts low in the scale, where the bass is an octave higher
            // so as not to go below its range.
            let rising = link.line[0].degree < PEDAL;
            out.bass = tone(if rising { A2 } else { A1 });
            out.pluck = tone(A3);
            if link.lead {
                out.lead[0] = tone(A4);
            }
        }
        // Each step is a little longer than the one before, up to the tempo of what follows.
        let slower = next.step() - STEP;
        out.length = STEP + slower * (link.done as u32 + 1) / CONNECTION_STEPS as u32;
        self.connection = Some(Connection {
            done: link.done + 1,
            ..link
        });
    }

    /// Starts the intro or the bridge from its first bar.
    fn enter(&mut self, part: Part) {
        self.part = part;
        self.bar = 0;
        self.step = 0;
        self.mode = HOME;
    }

    /// Starts the fight on the phrase after the one where the last fight ended.
    fn enter_fight(&mut self, context: Context) {
        self.part = Part::Fight;
        self.bar = self.next_phrase * BARS_IN_PHRASE;
        self.step = 0;
        self.lead_on = None;
        self.arrange(context);
    }

    /// Notes where the fight ended, for the next one to go on from there.
    fn end_fight(&mut self) {
        // On a bar line, the bar that the count shows has not played.
        let last = if self.step == 0 {
            (self.bar + Part::Fight.bars() - 1) % Part::Fight.bars()
        } else {
            self.bar
        };
        self.next_phrase = (last / BARS_IN_PHRASE + 1) % PHRASES;
    }

    /// Sets what a bar of a fight is arranged for.
    fn arrange(&mut self, context: Context) {
        self.arena = context.place == Place::Arena;
        self.threat = context.threat.min(HIGHEST_THREAT);
        if self.threat == 0 {
            // The lead is silent.
            self.lead_on = None;
        }
        self.mode = if self.arena { Mode::Aeolian } else { HOME };
    }

    /// The degree that the chord of the bar is built on. The bridge has no chords: it stays
    /// on one degree.
    fn chord(&self) -> u8 {
        let bar = self.bar() as usize;
        match self.section() {
            Section::Intro | Section::Verse => self.mode.verse_chords()[bar],
            Section::Chorus => self.mode.chorus_chords()[bar],
            Section::Bridge => PEDAL,
        }
    }

    /// The root of the chord of the bar on the bass, for a whole bar.
    fn root(&self) -> Tone {
        Tone {
            pitch: score::bass_root(self.mode, self.chord()),
            steps: STEPS as u8,
            level: HIT,
        }
    }

    fn intro(&self, out: &mut Step) {
        out.bass = self.bass(&BASS_LONG);
        out.pluck = self.note(&VERSE[self.bar as usize], A4);
    }

    /// A bar of the bridge is four pairs of hits, one per beat: the two beats of a heart on the
    /// kick, the call of the pluck, two notes of the bass and the answer of the lead. Under
    /// them the pluck ticks, an octave below its call.
    fn bridge(&self, out: &mut Step) {
        let bar = self.bar as usize;
        out.kick = match KICK_HEART[self.step as usize] {
            b'x' => HEART.0,
            b'o' => HEART.1,
            _ => 0,
        };
        let tick = self.note(&TICK, A2).map(|tone| Tone {
            level: TICK_LEVEL,
            ..tone
        });
        out.pluck = self.note(&BRIDGE[bar], A3).or(tick);
        out.bass = self.note(&BRIDGE_BASS, A1);
        out.lead[0] = self.note(&BRIDGE_ANSWERS[bar], A4).map(|tone| Tone {
            level: ANSWER,
            ..tone
        });
    }

    fn fight(&mut self, out: &mut Step) {
        out.kick = hit(&KICK_FIGHT, self.step, HIT);
        if self.arena {
            let (soft, loud) = ARENA_HATS;
            out.hat = hit(&HAT_ARENA, self.step, soft).max(hit(&HAT_CORRIDOR, self.step, loud));
            out.snare = hit(&SNARE_ARENA, self.step, SNARE);
        } else {
            out.hat = hit(&HAT_CORRIDOR, self.step, HAT);
        }
        out.bass = self.bass(if self.threat >= 2 {
            &BASS_LEAPING
        } else {
            &BASS_PLAIN
        });

        // The pluck goes up the chord, twice as fast at the highest threat.
        let chord = score::triad(self.mode, self.chord(), A3);
        let step = self.step as usize;
        if self.threat == HIGHEST_THREAT {
            out.pluck = Some(Tone {
                pitch: chord[step % 3],
                steps: 1,
                level: ARPEGGIO,
            });
        } else if PLUCK_ARPEGGIO[step] != b'.' {
            out.pluck = Some(Tone {
                pitch: chord[step / 2 % 3],
                steps: 2,
                level: ARPEGGIO,
            });
        }
        out.echo = out.pluck.is_some();

        if self.threat > 0 {
            let chorus = self.section() == Section::Chorus;
            let lines = &LEAD[self.threat as usize - 1][chorus as usize];
            let line = &lines[self.bar() as usize];
            if let Some(tone) = self.note(line, A4) {
                self.lead_on = Some(line[self.step as usize].degree);
                out.lead[0] = Some(tone);
                if self.threat == HIGHEST_THREAT {
                    out.lead[1] = Some(Tone {
                        pitch: tone.pitch + 12,
                        level: OCTAVE,
                        ..tone
                    });
                }
            }
        }
    }

    /// The note of a bar of melody that starts on this step, if one does, with degree 1 at
    /// `tonic`.
    fn note(&self, line: &Line, tonic: u8) -> Option<Tone> {
        let cell = line[self.step as usize];
        (cell.degree != 0).then(|| Tone {
            pitch: tonic + self.mode.semitones(cell.degree),
            steps: cell.steps,
            level: HIT,
        })
    }

    /// The note of a bass pattern that starts on this step, if one does. It lasts until the
    /// next note of the pattern.
    fn bass(&self, pattern: &Pattern) -> Option<Tone> {
        let step = self.step as usize;
        let root = score::bass_root(self.mode, self.chord());
        let pitch = match pattern[step] {
            b'R' => root,
            b'O' => root + 12,
            b'5' => root + 7,
            _ => return None,
        };
        let rest = pattern[step + 1..].iter();
        let steps = 1 + rest.take_while(|&&symbol| symbol == b'.').count() as u8;
        Some(Tone {
            pitch,
            steps,
            level: HIT,
        })
    }
}

/// Whether the context asks for the sections of a fight.
fn is_fight(context: Context) -> bool {
    context.fight && !context.down && context.place != Place::Menu
}

/// The part that plays when nobody fights.
fn calm_part(context: Context) -> Part {
    match context.place {
        Place::Menu | Place::Bay => Part::Intro,
        Place::Corridor | Place::Arena => Part::Bridge,
    }
}

/// How hard a drum is hit on a step of its pattern.
fn hit(pattern: &Pattern, step: u8, level: u8) -> u8 {
    if pattern[step as usize] == b'x' {
        level
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAR: usize = STEPS;
    const ROLL: usize = ROLL_HITS as usize;
    const LINK: usize = CONNECTION_STEPS as usize;

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
            assert_eq!(director.section(), Section::Intro);
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
            assert_eq!(roll[ROLL - 1], HIT);
            for step in &steps[..ROLL] {
                assert!(step.kick == 0 && step.snare == 0 && !pitched(step) && !step.fresh);
                assert_eq!(step.length, STEP);
            }
            // Then the verse starts on its first beat, wherever the bar of the bridge was.
            let verse = &steps[ROLL..];
            assert!(verse[0].fresh, "offset {offset}");
            assert_eq!(on(verse, |step| step.kick == HIT), [0, 4, 8, 12]);
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
        assert_eq!(first.kick, HIT);
        assert_eq!(director.section(), Section::Verse);
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
        assert_eq!(director.section(), Section::Verse);
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
                assert_eq!((step.kick, step.hat), (0, hit(&HAT_CORRIDOR, at, HAT)));
                assert!(!step.fresh && step.length == STEP, "offset {offset}");
            }
            // The connection starts on that beat: its kick is where the kick of the fight
            // would be, and nothing else drums.
            let link = &steps[wait..wait + LINK];
            assert_eq!(on(link, |step| step.kick == HIT), [0, 4], "offset {offset}");
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
            assert_eq!(bridge[0].kick, HEART.0);
            assert!(bridge.iter().all(|step| step.length == BRIDGE_STEP));
            assert_eq!(director.section(), Section::Bridge);
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
        assert_eq!(director.section(), Section::Bridge);
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
                .filter(|(_, tone)| tone.level == TICK_LEVEL)
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
        assert_eq!(steps[12].lead[0].map(|tone| tone.level), Some(ANSWER));
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
        let (verse, chorus) = (Section::Verse, Section::Chorus);
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
        assert_eq!(director.section(), Section::Intro);
        let steps = play(&mut director, at(Place::Bay), BAR);
        assert!(steps.iter().all(|step| !step.fresh && !step.hush));
        play(&mut director, fight(Place::Corridor, 1), ROLL);
        director.step(fight(Place::Corridor, 1));
        assert_eq!((director.section(), director.bar()), (Section::Verse, 0));
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
        assert_eq!(director.section(), Section::Bridge);
    }

    #[test]
    fn the_end_of_a_run_leads_to_the_intro() {
        let mut director = Director::settled(fight(Place::Arena, 4));
        play(&mut director, fight(Place::Arena, 4), 6);
        let steps = play(&mut director, at(Place::Menu), BAR);
        assert!(steps[0].hush);
        assert!(steps[10].fresh);
        assert_eq!(director.section(), Section::Intro);
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
        assert_eq!(steps[0].kick, HEART.1);
        assert_eq!(director.section(), Section::Bridge);
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
}
