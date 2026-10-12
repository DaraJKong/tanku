//! The sound that Tanku computes. No sound file ships with the game.
//!
//! The music is one piece that never stops and is the same on every platform. The game holds
//! a [`Conductor`] and tells it what happens, and whoever plays the sound holds the [`Music`]
//! and asks it for frames. Its specification is in `docs/MUSIC.md`.
//!
//! The sound cues are in [`cues`]: short waveforms that the game computes when it starts and
//! plays itself.

pub mod checks;
pub mod cues;
pub mod director;
mod math;
pub mod notation;
pub mod score;
pub mod synth;
pub mod wav;

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use director::Director;
pub use director::{Context, Place, Spot};
use score::Score;
use synth::Synth;

/// Frames of music per second.
pub const SAMPLE_RATE: u32 = 44_100;
/// How loud the music is against the cues when the game has the volume of its music all the
/// way up. Halfway up, where that setting starts, the loudest arrangement peaks 12 dB below a
/// shot fired next to the player.
pub const MUSIC_LEVEL: f32 = 0.54;
/// How loud a ducked music is, against what it was. Whoever computes the music ahead of time,
/// as the web build does, turns it down this much where it is played instead: the music
/// itself would do it too late.
pub const DUCKED: f32 = 0.5;
/// Frames between two looks at what can't wait for the next step of the music: a millisecond
/// and a half.
const WATCH: u64 = 64;
/// The bit of a packed context that says that the music is ducked.
const DUCKED_BIT: u32 = 1 << 8;
/// The music is in stereo: a frame is a sample for the left, then one for the right.
pub const CHANNELS: u16 = 2;

/// The music from its start, in the main menu, and the conductor that goes with it.
pub fn music() -> (Music, Conductor) {
    let conductor = Conductor::default();
    (conductor.music(), conductor)
}

/// The music of a game that has been in a context for a long time: it starts in the section
/// of that context, and nothing announces it.
pub fn music_from(context: Context) -> (Music, Conductor) {
    music_of(score::game(), context)
}

/// The same for another piece than that of the game: one that a program read while it runs.
pub fn music_of(score: Arc<Score>, context: Context) -> (Music, Conductor) {
    let conductor = Conductor(Arc::new(AtomicU32::new(pack(context))));
    let synth = Synth::of(&score);
    let director = Director::settled_in(score, context);
    (conductor.follow(director, synth), conductor)
}

/// The end of the music that the game holds, to tell it what happens. It starts in the main
/// menu.
#[derive(Clone)]
pub struct Conductor(Arc<AtomicU32>);

impl Default for Conductor {
    fn default() -> Self {
        Self(Arc::new(AtomicU32::new(pack(Context::default()))))
    }
}

impl Conductor {
    /// Tells the music the context of the game. It never waits: the music reads the context
    /// at the start of its next step.
    pub fn set(&self, context: Context) {
        self.0.store(pack(context), Ordering::Relaxed);
    }

    /// What the music was last told.
    pub fn context(&self) -> Context {
        unpack(self.0.load(Ordering::Relaxed))
    }

    /// The music that this conductor leads, from the start of the piece. Whoever plays the
    /// sound asks for it, which may be after the game has started to tell what happens.
    pub fn music(&self) -> Music {
        self.follow(Director::default(), Synth::default())
    }

    fn follow(&self, director: Director, synth: Synth) -> Music {
        Music {
            context: self.0.clone(),
            director,
            synth,
            frame: 0,
            next_step: 0,
        }
    }
}

/// The end of the music that makes the sound.
pub struct Music {
    context: Arc<AtomicU32>,
    director: Director,
    synth: Synth,
    /// Frames rendered so far.
    frame: u64,
    /// Where the next step starts, in half frames. Steps are counted in half frames because
    /// one lasts 5512 frames and a half at 120 beats per minute: nothing is rounded, so
    /// nothing drifts.
    next_step: u64,
}

impl Music {
    /// Plays another piece from the next step on, without starting over, and gives back the
    /// one that it played: a program that changes a piece while it plays hands each new
    /// state of it this way. What sounds goes on sounding. Unlike [`Music::render`], this
    /// takes memory when the echo of the new piece comes later than any before it.
    pub fn rescore(&mut self, score: Arc<Score>) -> Arc<Score> {
        self.synth.retune(&score);
        self.director.rescore(score)
    }

    /// Where the music is in its piece.
    pub fn spot(&self) -> Spot {
        self.director.spot()
    }

    /// Fills `frames` with the music that comes next. It never allocates, locks or waits, so
    /// a sound thread can call it.
    pub fn render(&mut self, frames: &mut [f32]) {
        for frame in frames.as_chunks_mut::<{ CHANNELS as usize }>().0 {
            if self.frame.is_multiple_of(WATCH) {
                let ducked = self.context.load(Ordering::Relaxed) & DUCKED_BIT != 0;
                self.synth.duck(ducked);
            }
            if self.frame == self.next_step / 2 {
                let context = unpack(self.context.load(Ordering::Relaxed));
                let step = self.director.step(context);
                self.synth.play(&step, self.next_step);
                self.next_step += step.length as u64;
            }
            *frame = self.synth.frame();
            self.frame += 1;
        }
    }
}

/// A context as one number, which is how it goes from the game to the sound thread.
fn pack(context: Context) -> u32 {
    context.place as u32
        | (context.fight as u32) << 2
        | (context.down as u32) << 3
        | (context.muffled as u32) << 4
        | (context.threat.min(7) as u32) << 5
        | if context.ducked { DUCKED_BIT } else { 0 }
}

fn unpack(bits: u32) -> Context {
    Context {
        place: [Place::Menu, Place::Bay, Place::Corridor, Place::Arena][bits as usize & 3],
        fight: bits & 1 << 2 != 0,
        down: bits & 1 << 3 != 0,
        muffled: bits & 1 << 4 != 0,
        threat: (bits >> 5) as u8 & 7,
        ducked: bits & DUCKED_BIT != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_context_survives_the_trip_between_the_threads() {
        for place in [Place::Menu, Place::Bay, Place::Corridor, Place::Arena] {
            for bits in 0..128u8 {
                let context = Context {
                    place,
                    fight: bits & 1 != 0,
                    down: bits & 2 != 0,
                    muffled: bits & 4 != 0,
                    ducked: bits & 8 != 0,
                    threat: bits >> 4,
                };
                assert_eq!(unpack(pack(context)), context);
                let conductor = Conductor::default();
                conductor.set(context);
                assert_eq!(conductor.context(), context);
            }
        }
    }

    #[test]
    fn a_duck_doesnt_wait_for_a_step() {
        let fight = Context {
            place: Place::Corridor,
            fight: true,
            threat: 2,
            ..Context::default()
        };
        let (mut plain, _) = music_from(fight);
        let (mut ducked, conductor) = music_from(fight);
        let mut listen = |frames: usize| {
            let mut heard = [vec![0.0; 2 * frames], vec![0.0; 2 * frames]];
            plain.render(&mut heard[0]);
            ducked.render(&mut heard[1]);
            heard
        };
        // Half a second in, which is in the middle of a step.
        let [whole, same] = listen(22_000);
        assert_eq!(whole, same);
        conductor.set(Context {
            ducked: true,
            ..fight
        });
        // The music is half as loud 0.02 s later, where a step lasts 0.125 s. Below the knee
        // of the limit, half is exact.
        let [whole, half] = listen(2000);
        let settled = whole.iter().zip(&half).skip(2 * 1000);
        let mut compared = 0;
        for (whole, half) in settled.filter(|(whole, _)| whole.abs() < 0.5) {
            assert_eq!(*half, 0.5 * whole);
            compared += 1;
        }
        assert!(compared > 1000);
    }

    #[test]
    fn the_music_can_be_asked_for_after_the_game_has_started() {
        // Both ends are made at once, or the sound end later: the frames are the same.
        let fight = Context {
            place: Place::Corridor,
            fight: true,
            threat: 2,
            ..Context::default()
        };
        let (mut at_once, conductor) = music();
        conductor.set(fight);
        let later = Conductor::default();
        later.set(fight);
        let mut later = later.music();
        let mut frames = [vec![0.0; 2 * 44_100], vec![0.0; 2 * 44_100]];
        at_once.render(&mut frames[0]);
        later.render(&mut frames[1]);
        assert!(frames[0].iter().any(|sample| *sample != 0.0));
        assert_eq!(frames[0], frames[1]);
    }

    #[test]
    fn the_settings_of_an_instrument_can_only_sound_bad() {
        // Every instrument at one end of what its `sound` line can say, then at the other,
        // and all of them as loud as the mix can make them. The music may be ugly: it is
        // never out of range and never NaN.
        let ends = [
            [
                "sound kick pitch 20000Hz 20Hz 0.1ms decay 10000ms",
                "sound hat above 20Hz decay 10000ms",
                "sound snare band 5000Hz 500Hz decay 10000ms tone 20000Hz 10000ms 100%",
                "sound bass rise 0.1ms filter 20000Hz 20Hz 0.1ms",
                "sound pluck ring 10000ms smooth 0 echo 1 100%",
                "sound lead width 1% rise 0.1ms below 20000Hz vibrato 0.1ms 10% 20Hz",
            ],
            [
                "sound kick pitch 20Hz 20000Hz 10000ms decay 0.1ms",
                "sound hat above 20000Hz decay 0.1ms",
                "sound snare band 5000Hz 10000Hz decay 0.1ms tone 20Hz 0.1ms 0%",
                "sound bass rise 10000ms filter 20Hz 20000Hz 10000ms",
                "sound pluck ring 100ms smooth 8 echo 8 100%",
                "sound lead width 99% rise 10000ms below 20Hz vibrato 10000ms 0% 0.1Hz",
            ],
        ];
        for end in ends {
            // The piece of the game, with these lines in place of its own.
            let mut text = String::new();
            for line in include_str!("../music.score").lines() {
                let mut words = line.split_whitespace();
                let line = match (words.next(), words.next()) {
                    (Some("mix"), Some(name)) => format!("mix {name} 100% 100%"),
                    (Some("sound"), Some(name)) => {
                        let named = |new: &&&str| new.split_whitespace().nth(1) == Some(name);
                        end.iter().find(named).unwrap().to_string()
                    }
                    _ => line.to_string(),
                };
                text += &format!("{line}\n");
            }
            let score = Arc::new(Score::new(notation::read(&text).unwrap()));
            let fight = Context {
                place: Place::Arena,
                fight: true,
                threat: 4,
                ..Context::default()
            };
            let (mut music, _) = music_of(score, fight);
            let mut frames = vec![0.0; 2 * 8 * 44_100];
            music.render(&mut frames);
            assert!(frames.iter().all(|sample| sample.abs() <= 1.0), "{end:?}");
            assert!(frames.iter().any(|sample| sample.abs() > 0.5), "{end:?}");
        }
    }

    #[test]
    fn another_piece_is_played_as_its_file_says() {
        // The piece of the game, with every instrument but the kick mixed down to nothing,
        // and the kick on the left alone.
        let mut text = include_str!("../music.score").to_string();
        for (name, levels) in [
            ("kick ", "32% 32%"),
            ("hat  ", "75% 45%"),
            ("snare", "28% 28%"),
            ("bass ", "10% 10%"),
            ("pluck", "20% 10%"),
            ("lead ", " 5%  8%"),
        ] {
            let (line, silent) = (format!("mix {name} {levels}"), format!("mix {name} 0% 0%"));
            assert!(text.contains(&line), "{line}");
            let left = format!("mix {name} 50% 0%");
            text = text.replace(&line, if name == "kick " { &left } else { &silent });
        }
        let score = Arc::new(Score::new(notation::read(&text).unwrap()));
        let fight = Context {
            place: Place::Arena,
            fight: true,
            threat: 4,
            ..Context::default()
        };
        let (mut music, _) = music_of(score, fight);
        let mut frames = vec![0.0; 2 * 44_100];
        music.render(&mut frames);
        let (left, right): (Vec<f32>, Vec<f32>) =
            frames.chunks(2).map(|frame| (frame[0], frame[1])).unzip();
        assert!(left.iter().any(|sample| sample.abs() > 0.2));
        assert!(right.iter().all(|sample| *sample == 0.0));
        // Nothing else is heard: before the kick of the second beat, the one of the first has
        // died away.
        assert!(left[20_000..22_000].iter().all(|sample| *sample == 0.0));

        // The piece of the game is the same music either way.
        let (mut game, _) = music_from(fight);
        let (mut same, _) = music_of(score::game(), fight);
        let mut heard = [vec![0.0; 2 * 44_100], vec![0.0; 2 * 44_100]];
        game.render(&mut heard[0]);
        same.render(&mut heard[1]);
        assert_eq!(heard[0], heard[1]);
        assert!(heard[0] != frames);
    }

    #[test]
    fn a_music_takes_another_piece_without_starting_over() {
        let fight = Context {
            place: Place::Arena,
            fight: true,
            threat: 4,
            ..Context::default()
        };
        // The piece of the game without its lead, with an echo that comes a step later, at
        // another level.
        let text = include_str!("../music.score")
            .replace("mix lead   5%  8%", "mix lead   0%  0%")
            .replace("echo 3 33%", "echo 4 50%");
        let other = Arc::new(Score::new(notation::read(&text).unwrap()));
        let listen = |music: &mut Music, frames: usize| {
            let mut heard = vec![0.0; 2 * frames];
            music.render(&mut heard);
            heard
        };

        // A bar and a little more of the game's piece, then the other piece.
        let (mut music, _) = music_of(score::game(), fight);
        let before = listen(&mut music, 100_000);
        let spot = music.spot();
        let old = music.rescore(other.clone());
        assert!(Arc::ptr_eq(&old, &score::game()));
        assert_eq!(music.spot(), spot);
        let after = listen(&mut music, 200_000);
        assert!(after.iter().any(|sample| sample.abs() > 0.1));
        assert!(after.iter().all(|sample| sample.abs() < 1.0));

        // The piece of the game went on to the same place by itself, so the two are the same
        // music up to the change and another music after it.
        let (mut game, _) = music_of(score::game(), fight);
        assert_eq!(listen(&mut game, 100_000), before);
        assert_eq!(game.spot(), spot);
        assert!(listen(&mut game, 200_000) != after);

        // A piece that only mixes the lead away sounds, from the change on, as if it had
        // played from the start: nothing else of the music has moved.
        let text = include_str!("../music.score").replace("mix lead   5%  8%", "mix lead   0%  0%");
        let muted = Arc::new(Score::new(notation::read(&text).unwrap()));
        let (mut changed, _) = music_of(score::game(), fight);
        let (mut whole, _) = music_of(muted.clone(), fight);
        listen(&mut changed, 100_000);
        listen(&mut whole, 100_000);
        changed.rescore(muted);
        assert_eq!(listen(&mut changed, 200_000), listen(&mut whole, 200_000));
    }
}
