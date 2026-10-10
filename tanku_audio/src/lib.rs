//! The sound that Tanku computes. No sound file ships with the game.
//!
//! The music is one piece that never stops and is the same on every platform. The game holds
//! a [`Conductor`] and tells it what happens, and whoever plays the sound holds the [`Music`]
//! and asks it for frames. Its specification is in `docs/MUSIC.md`.
//!
//! The sound cues are in [`cues`]: short waveforms that the game computes when it starts and
//! plays itself.

pub mod cues;
pub mod director;
mod math;
pub mod score;
pub mod synth;
pub mod wav;

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use director::Director;
pub use director::{Context, Place};
use synth::Synth;

/// Frames of music per second.
pub const SAMPLE_RATE: u32 = 44_100;
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
    let conductor = Conductor(Arc::new(AtomicU32::new(pack(context))));
    (conductor.follow(Director::settled(context)), conductor)
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
        self.follow(Director::default())
    }

    fn follow(&self, director: Director) -> Music {
        Music {
            context: self.0.clone(),
            director,
            synth: Synth::default(),
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
    /// Fills `frames` with the music that comes next. It never allocates, locks or waits, so
    /// a sound thread can call it.
    pub fn render(&mut self, frames: &mut [f32]) {
        for frame in frames.as_chunks_mut::<{ CHANNELS as usize }>().0 {
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
}

fn unpack(bits: u32) -> Context {
    Context {
        place: [Place::Menu, Place::Bay, Place::Corridor, Place::Arena][bits as usize & 3],
        fight: bits & 1 << 2 != 0,
        down: bits & 1 << 3 != 0,
        muffled: bits & 1 << 4 != 0,
        threat: (bits >> 5) as u8 & 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_context_survives_the_trip_between_the_threads() {
        for place in [Place::Menu, Place::Bay, Place::Corridor, Place::Arena] {
            for bits in 0..64u8 {
                let context = Context {
                    place,
                    fight: bits & 1 != 0,
                    down: bits & 2 != 0,
                    muffled: bits & 4 != 0,
                    threat: bits >> 3,
                };
                assert_eq!(unpack(pack(context)), context);
                let conductor = Conductor::default();
                conductor.set(context);
                assert_eq!(conductor.context(), context);
            }
        }
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
}
