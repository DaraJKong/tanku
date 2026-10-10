//! Chooses what plays. The game gives a context, and on every step the director says what each
//! instrument starts.
//!
//! Two rules make the music answer fast and stay in time. Every change of section, of mode or
//! of pattern happens on a bar line. What can't wait for the bar line is said at once: a roll
//! announces a fight, and the drums stop the moment a fight ends.
//!
//! All of it is integer logic, so it gives the same answer on every platform.

use crate::score::{
    self, A3, A4, BARS, BASS_LEAPING, BASS_LONG, BASS_PLAIN, BRIDGE, BRIDGE_CHORDS, CHORUS,
    CHORUS_CHORDS, HAT_ARENA, HAT_CORRIDOR, KICK_CALM, KICK_FIGHT, Line, Mode, PLUCK_ARPEGGIO,
    Pattern, SNARE_ARENA, STEPS, VERSE, VERSE_CHORDS,
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
}

const HIGHEST_THREAT: u8 = 4;

/// Eight bars with their chords and their melody.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Intro,
    Verse,
    Chorus,
    Bridge,
}

/// What stopped at once and waits for the bar line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Hush {
    #[default]
    None,
    /// A fight ended: the drums stop, and the notes that sound are held.
    Held,
    /// The tank is destroyed or the run is over: everything stops but the bass, which holds
    /// its note.
    BassOnly,
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
    /// How hard each drum is hit, from 0 for no hit to 255.
    pub kick: u8,
    pub hat: u8,
    pub snare: u8,
    pub bass: Option<Tone>,
    pub pluck: Option<Tone>,
    /// The note of the pluck gets an echo, as the notes of its arpeggio do. Its melodies stay
    /// dry.
    pub echo: bool,
    /// Up to three notes at once.
    pub lead: [Option<Tone>; 3],
    /// The lead comes in softly, as it does in the bridge.
    pub soft: bool,
    /// The section changed on this step: the notes that were sounding let go.
    pub fresh: bool,
    pub hush: Hush,
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
/// The kick of the calm is softer than the kick of a fight.
const CALM_KICK: u8 = 180;
const HAT: u8 = 190;
/// The hi-hat of an arena plays every step, and louder where the corridor's plays.
const ARENA_HATS: (u8, u8) = (100, 200);
const SNARE: u8 = 230;
/// A roll starts this loud and ends at full strength.
const ROLL_START: u16 = 40;
/// A roll lasts one beat at least.
const SHORTEST_ROLL: u8 = 4;
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
}

/// The roll that announces a fight.
#[derive(Clone, Copy, Debug)]
struct Roll {
    /// On the snare in an arena, on the hi-hat elsewhere.
    snare: bool,
    done: u8,
    hits: u8,
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
    hush: Hush,
    /// Whether the last step was outside a run, to tell when one starts.
    in_menu: bool,
}

impl Default for Director {
    fn default() -> Self {
        Self {
            part: Part::Intro,
            bar: 0,
            step: 0,
            mode: Mode::Ionian,
            arena: false,
            threat: 0,
            next_phrase: 0,
            roll: None,
            hush: Hush::None,
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

        // On a bar line, what was decided since the last one takes effect. A tank that is
        // still destroyed keeps everything waiting.
        if self.step == 0 && !context.down {
            let calm = calm_part(context);
            let rolled = self.roll.is_some_and(|roll| roll.done == roll.hits);
            if self.hush != Hush::None {
                self.hush = Hush::None;
                self.enter(calm);
                out.fresh = true;
            } else if rolled && fight {
                self.roll = None;
                self.enter_fight(context);
                out.fresh = true;
            } else if self.part == Part::Fight && !fight {
                // The fight ended right on the bar line.
                self.end_fight();
                self.enter(calm);
                out.fresh = true;
            } else if self.part != Part::Fight && self.part != calm && !fight {
                self.enter(calm);
                out.fresh = true;
            }
            if self.part == Part::Fight {
                self.arrange(context);
            }
        }

        // What can't wait for the bar line.
        let run_left = !playing && self.part != Part::Intro;
        if context.down || run_left {
            if self.hush != Hush::BassOnly {
                if self.part == Part::Fight && self.hush == Hush::None {
                    self.end_fight();
                }
                self.hush = Hush::BassOnly;
                self.roll = None;
                out.bass = Some(self.root());
            }
        } else if self.part == Part::Fight && !fight {
            if self.hush == Hush::None {
                self.end_fight();
                self.hush = Hush::Held;
                // The bass lands on the root and stays there.
                out.bass = Some(self.root());
            }
        } else if self.part != Part::Fight && fight && self.hush == Hush::None {
            if self.roll.is_none() {
                let left = STEPS as u8 - self.step;
                self.roll = Some(Roll {
                    snare: context.place == Place::Arena,
                    done: 0,
                    // When the bar line is closer than a beat, the roll runs to the one after.
                    hits: if left >= SHORTEST_ROLL {
                        left
                    } else {
                        left + STEPS as u8
                    },
                });
            }
        } else if !fight {
            // The fight was called off before it started.
            self.roll = None;
        }

        if self.hush == Hush::None {
            match self.part {
                Part::Intro => self.intro(&mut out),
                Part::Bridge => self.bridge(&mut out),
                Part::Fight => self.fight(&mut out),
            }
        }
        if let Some(roll) = &mut self.roll {
            let rise = (HIT as u16 - ROLL_START) * (roll.done as u16 + 1) / roll.hits as u16;
            let level = (ROLL_START + rise) as u8;
            if roll.snare {
                out.snare = level;
            } else {
                out.hat = level;
            }
            roll.done += 1;
        }
        out.hush = self.hush;

        self.step += 1;
        if self.step == STEPS as u8 {
            self.step = 0;
            self.bar = (self.bar + 1) % self.part.bars();
        }
        out
    }

    /// Starts the intro or the bridge from its first bar.
    fn enter(&mut self, part: Part) {
        self.part = part;
        self.bar = 0;
        self.mode = Mode::Ionian;
    }

    /// Starts the fight on the phrase after the one where the last fight ended.
    fn enter_fight(&mut self, context: Context) {
        self.part = Part::Fight;
        self.bar = self.next_phrase * BARS_IN_PHRASE;
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
        self.mode = if self.arena {
            Mode::Aeolian
        } else {
            Mode::Ionian
        };
    }

    /// The degree that the chord of the bar is built on.
    fn chord(&self) -> u8 {
        let bar = self.bar() as usize;
        match self.section() {
            Section::Intro | Section::Verse => VERSE_CHORDS[bar],
            Section::Chorus => CHORUS_CHORDS[bar],
            Section::Bridge => BRIDGE_CHORDS[bar],
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
        out.pluck = self.melody(&VERSE, A4);
    }

    fn bridge(&self, out: &mut Step) {
        out.kick = hit(&KICK_CALM, self.step, CALM_KICK);
        out.bass = self.bass(&BASS_LONG);
        out.pluck = self.melody(&BRIDGE, A3);
        // The lead holds each chord for as long as it lasts.
        let bar = self.bar as usize;
        let chord = BRIDGE_CHORDS[bar];
        if self.step == 0 && (bar == 0 || BRIDGE_CHORDS[bar - 1] != chord) {
            let bars = BRIDGE_CHORDS[bar..]
                .iter()
                .take_while(|&&next| next == chord);
            let steps = (bars.count() * STEPS) as u8;
            let held = score::close_triad(self.mode, chord, A4);
            out.lead = held.map(|pitch| {
                Some(Tone {
                    pitch,
                    steps,
                    level: HIT,
                })
            });
            out.soft = true;
        }
    }

    fn fight(&self, out: &mut Step) {
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
            if let Some(tone) = self.melody(lines, A4) {
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

    /// The note of a melody that starts on this step, if one does, with degree 1 at `tonic`.
    fn melody(&self, lines: &[Line; BARS], tonic: u8) -> Option<Tone> {
        let cell = lines[self.bar() as usize][self.step as usize];
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

    #[test]
    fn the_menu_and_the_bay_have_the_intro_without_drums() {
        for place in [Place::Menu, Place::Bay] {
            let mut director = Director::default();
            let steps = play(&mut director, at(place), 2 * BARS * BAR);
            assert_eq!(director.section(), Section::Intro);
            assert!(steps.iter().all(|step| !drums(step)));
            assert!(steps.iter().all(|step| step.lead == [None; 3]));
            // Its melody has no echo.
            assert!(steps.iter().all(|step| !step.echo));
            // The pluck has motif A and the bass holds the root of each bar.
            let melody: Vec<u8> = steps[..BAR]
                .iter()
                .flat_map(|step| step.pluck)
                .map(|tone| tone.pitch)
                .collect();
            assert_eq!(melody, [69, 71, 73, 76, 73, 71]);
            for (index, step) in steps.iter().enumerate() {
                assert_eq!(step.bass.is_some(), index % BAR == 0);
            }
            assert_eq!(
                steps[0].bass,
                Some(Tone {
                    pitch: 45,
                    steps: 16,
                    level: HIT
                })
            );
            // The loop is eight bars long.
            assert_eq!(steps[..BARS * BAR], steps[BARS * BAR..]);
        }
    }

    #[test]
    fn a_fight_waits_for_the_bar_line_behind_a_roll() {
        for offset in 0..BAR {
            let mut director = Director::settled(at(Place::Corridor));
            play(&mut director, at(Place::Corridor), BAR + offset);
            let steps = play(&mut director, fight(Place::Corridor, 2), 3 * BAR);
            // The roll is heard on the step where the fight starts.
            assert!(steps[0].hat > 0, "offset {offset}");
            let start = steps.iter().position(|step| step.fresh).unwrap();
            assert_eq!((offset + start) % BAR, 0, "offset {offset}");
            assert!(start >= SHORTEST_ROLL as usize, "offset {offset}");
            assert!(start < BAR + SHORTEST_ROLL as usize, "offset {offset}");
            // It grows louder on every step, on the hi-hat, over the bridge.
            let roll: Vec<u8> = steps[..start].iter().map(|step| step.hat).collect();
            assert!(roll.windows(2).all(|pair| pair[0] < pair[1]), "{roll:?}");
            assert_eq!(roll[start - 1], HIT);
            assert!(steps[..start].iter().all(|step| step.snare == 0));
            // Then the verse starts, with the kick on every beat.
            assert_eq!(steps[start].kick, HIT);
            assert_eq!(steps[start + 4].kick, HIT);
            assert_eq!(steps[start].hat, 0);
            assert_eq!(steps[start + 2].hat, HAT);
        }
    }

    #[test]
    fn the_intro_goes_on_under_the_roll_out_of_the_bay() {
        // The barrier of the bay is crossed two steps before a bar line, so the roll runs
        // through the next bar. The intro plays under it to the end, and no bridge comes in.
        let mut director = Director::default();
        play(&mut director, at(Place::Bay), BAR + 14);
        let steps = play(&mut director, fight(Place::Corridor, 0), 2 + BAR);
        assert!(steps.iter().all(|step| step.hat > 0 && step.kick == 0));
        assert!(
            steps
                .iter()
                .all(|step| !step.fresh && step.lead == [None; 3])
        );
        assert!(steps[2].pluck.is_some());
        let first = director.step(fight(Place::Corridor, 0));
        assert!(first.fresh);
        assert_eq!(first.kick, HIT);
        assert_eq!(director.section(), Section::Verse);
    }

    #[test]
    fn an_arena_rolls_on_the_snare_and_turns_minor() {
        let mut director = Director::settled(at(Place::Arena));
        assert_eq!(director.mode(), Mode::Ionian);
        play(&mut director, at(Place::Arena), 5);
        let steps = play(&mut director, fight(Place::Arena, 2), BAR - 5);
        assert!(steps.iter().all(|step| step.snare > 0 && step.hat == 0));
        assert_eq!(director.mode(), Mode::Ionian);

        let bar = play(&mut director, fight(Place::Arena, 2), BAR);
        assert_eq!(director.mode(), Mode::Aeolian);
        assert_eq!(director.section(), Section::Verse);
        // Motif A, with the C of the minor mode where the major has C sharp.
        let melody: Vec<u8> = bar
            .iter()
            .flat_map(|step| step.lead[0])
            .map(|tone| tone.pitch)
            .collect();
        assert_eq!(melody, [69, 71, 72, 76, 72, 71]);
        // The snare is on beats 2 and 4 and the hi-hat on every step.
        let snare: Vec<usize> = (0..BAR).filter(|&step| bar[step].snare > 0).collect();
        assert_eq!(snare, [4, 12]);
        assert!(bar.iter().all(|step| step.hat > 0));
        assert!(bar[2].hat > bar[1].hat);
    }

    #[test]
    fn the_end_of_a_fight_stops_the_drums_at_once() {
        for offset in 1..BAR {
            let mut director = Director::settled(fight(Place::Arena, 3));
            play(&mut director, fight(Place::Arena, 3), BAR + offset);
            let steps = play(&mut director, at(Place::Arena), 2 * BAR);
            let line = BAR - offset;
            // Up to the bar line nothing starts but the bass, which lands on the root.
            assert_eq!(steps[0].hush, Hush::Held);
            assert_eq!(
                steps[0].bass,
                Some(Tone {
                    pitch: 45,
                    steps: 16,
                    level: HIT
                })
            );
            assert!(steps[..line].iter().all(|step| !drums(step)));
            assert!(steps[1..line].iter().all(|step| !pitched(step)));
            // On the bar line the bridge starts from its first bar, in major.
            assert!(steps[line].fresh);
            assert_eq!(steps[line].hush, Hush::None);
            assert_eq!(steps[line].kick, CALM_KICK);
            assert!(steps[line].soft);
            assert_eq!(
                steps[line].lead.map(|tone| tone.unwrap().pitch),
                [74, 78, 69]
            );
            assert_eq!(director.section(), Section::Bridge);
            assert_eq!(director.mode(), Mode::Ionian);
        }
    }

    #[test]
    fn a_fight_that_ends_on_the_bar_line_needs_no_marker() {
        let mut director = Director::settled(fight(Place::Corridor, 1));
        play(&mut director, fight(Place::Corridor, 1), 2 * BAR);
        let step = director.step(at(Place::Corridor));
        assert!(step.fresh);
        assert_eq!(step.hush, Hush::None);
        assert_eq!(director.section(), Section::Bridge);
        // The two bars that played were in the first phrase, so the next fight opens the second.
        assert_eq!(director.next_phrase, 1);
    }

    #[test]
    fn the_bridge_is_calm() {
        let mut director = Director::settled(at(Place::Corridor));
        let steps = play(&mut director, at(Place::Corridor), 2 * BARS * BAR);
        assert_eq!(director.section(), Section::Bridge);
        for (index, step) in steps.iter().enumerate() {
            // The kick is on beats 1 and 3, and nothing else drums.
            assert_eq!(step.kick > 0, index % 8 == 0);
            assert!(step.hat == 0 && step.snare == 0);
            // The lead strikes a chord when the chord changes, every two bars.
            assert_eq!(step.lead[0].is_some(), index % (2 * BAR) == 0);
        }
        assert_eq!(steps[0].lead[0].unwrap().steps, 32);
        // Motif B at half speed, an octave below the lead.
        let melody: Vec<(u8, u8)> = steps[..2 * BAR]
            .iter()
            .flat_map(|step| step.pluck)
            .map(|tone| (tone.pitch, tone.steps))
            .collect();
        assert_eq!(melody, [(69, 12), (71, 4), (69, 8), (66, 8)]);
        assert!(steps.iter().all(|step| !step.echo));
        assert_eq!(steps[..BARS * BAR], steps[BARS * BAR..]);
    }

    #[test]
    fn the_lead_follows_the_threat() {
        let lead = |threat: u8| {
            let mut director = Director::settled(fight(Place::Corridor, threat));
            let bar = play(&mut director, fight(Place::Corridor, threat), BAR);
            let notes = bar.iter().filter(|step| step.lead[0].is_some()).count();
            let octaves = bar.iter().any(|step| step.lead[1].is_some());
            let bass: Vec<u8> = bar
                .iter()
                .flat_map(|step| step.bass)
                .map(|t| t.pitch)
                .collect();
            let pluck = bar.iter().filter(|step| step.pluck.is_some()).count();
            // Every note of the arpeggio has its echo.
            assert!(bar.iter().all(|step| step.echo == step.pluck.is_some()));
            (notes, octaves, bass, pluck)
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
        assert!(rest.iter().all(|step| step.lead == [None; 3]));
        let next = play(&mut director, fight(Place::Corridor, 2), BAR);
        assert!(next[0].lead[0].is_some());
        assert!(!next[0].fresh);
    }

    #[test]
    fn fights_walk_through_the_piece() {
        let mut director = Director::default();
        play(&mut director, at(Place::Bay), BAR);
        let mut opened = Vec::new();
        // Six short fights, each of a bar and a half, with a calm between them.
        for _ in 0..6 {
            let steps = play(&mut director, fight(Place::Corridor, 2), 3 * BAR);
            let start = steps.iter().position(|step| step.fresh).unwrap();
            assert_eq!(start, BAR);
            opened.push((director.section(), director.bar()));
            play(&mut director, at(Place::Corridor), BAR);
        }
        // The roll takes the first bar. Each fight then plays the first two bars of the next
        // phrase, so the bar that comes after them is the third of that phrase.
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
        play(&mut director, fight(Place::Corridor, 1), 3 * BAR);
        // The run ends and another starts: the intro goes on, and nothing marks it.
        play(&mut director, at(Place::Menu), 2 * BAR);
        assert_eq!(director.section(), Section::Intro);
        let steps = play(&mut director, at(Place::Bay), BAR);
        assert!(
            steps
                .iter()
                .all(|step| !step.fresh && step.hush == Hush::None)
        );
        play(&mut director, fight(Place::Corridor, 1), BAR);
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
        assert_eq!(steps[0].hush, Hush::BassOnly);
        assert!(steps[0].bass.is_some());
        assert!(steps[1..].iter().all(|step| !drums(step) && !pitched(step)));
        assert!(steps.iter().all(|step| step.hush == Hush::BassOnly));
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
        assert_eq!(steps[0].hush, Hush::BassOnly);
        assert!(steps[10].fresh);
        assert_eq!(director.section(), Section::Intro);
        assert_eq!(director.mode(), Mode::Ionian);
    }

    #[test]
    fn a_fight_that_is_called_off_stops_its_roll() {
        let mut director = Director::settled(at(Place::Corridor));
        play(&mut director, at(Place::Corridor), 2);
        let steps = play(&mut director, fight(Place::Corridor, 1), 4);
        assert!(steps.iter().all(|step| step.hat > 0));
        let steps = play(&mut director, at(Place::Corridor), 2 * BAR);
        assert!(steps.iter().all(|step| step.hat == 0 && !step.fresh));
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
