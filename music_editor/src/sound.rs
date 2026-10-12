//! The sound of the editor: the piece as the game would play it, on a thread of its own.
//!
//! The editor hands orders to that thread and never waits for it: a piece to play from its
//! start, another state of the piece to go on with, or silence. What the thread is done with
//! comes back to be let go of here, so that it frees no memory itself. A meter tells where
//! the music is and how loud, for the display.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
    mpsc::{Receiver, Sender, channel},
};

use bevy::prelude::Resource;
use tanku_audio::{
    CHANNELS, Conductor, Context, Music, Spot, music_of, notation::Scene, score::Score,
};

/// Frames that the music computes at a time: 3 ms of it.
const BLOCK_FRAMES: usize = 128;
const BLOCK: usize = BLOCK_FRAMES * CHANNELS as usize;
/// Blocks over which a music that is stopped dies away, so that it ends without a click.
const FADE: u32 = 4;
/// What the meter keeps of a peak from one block to the next: it falls by 40 dB in half a
/// second.
const FALL: f32 = 0.973;

/// What the editor asks of the sound thread.
enum Order {
    /// A music from its start, in place of the one that plays.
    Play(Box<Music>),
    /// Another state of the piece, for the music that plays to go on with.
    Rescore(Arc<Score>),
    Stop,
}

/// What the sound thread is done with.
enum Spent {
    Music(#[expect(dead_code, reason = "sent back to be freed")] Box<Music>),
    Score(#[expect(dead_code, reason = "sent back to be freed")] Arc<Score>),
}

/// Where the music is and how loud, as the sound thread last told.
#[derive(Default)]
struct Meter {
    left: AtomicU32,
    right: AtomicU32,
    /// A spot as one number, or 0 while nothing plays.
    spot: AtomicU32,
}

/// What the display shows of the music.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Heard {
    /// The peak on each side, from 0 to 1.
    pub level: [f32; 2],
    /// Where the music is, while it plays.
    pub spot: Option<Spot>,
}

fn pack(spot: Spot) -> u32 {
    1 | (spot.scene as u32) << 1
        | (spot.section as u32) << 3
        | (spot.bar as u32) << 7
        | (spot.step as u32) << 10
        | (spot.between as u32) << 14
}

fn unpack(bits: u32) -> Option<Spot> {
    (bits & 1 != 0).then(|| Spot {
        scene: Scene::ALL[(bits >> 1) as usize & 3],
        section: (bits >> 3) as u8 & 15,
        bar: (bits >> 7) as u8 & 7,
        step: (bits >> 10) as u8 & 15,
        between: bits >> 14 & 1 != 0,
    })
}

/// The end of the sound that the editor holds.
#[derive(Resource)]
pub struct Engine {
    orders: Sender<Order>,
    /// Only the editor reads it: the lock is there for the engine, which shares what it holds
    /// between its threads.
    spent: Mutex<Receiver<Spent>>,
    meter: Arc<Meter>,
    /// Tells the music that plays what happens in the game.
    conductor: Conductor,
    playing: bool,
}

/// The end of the sound that makes it: the sound library reads it one sample at a time.
pub struct Stream {
    orders: Receiver<Order>,
    spent: Sender<Spent>,
    meter: Arc<Meter>,
    music: Option<Box<Music>>,
    /// A music that was stopped or replaced, and the blocks that it still dies away over.
    tail: Option<(Box<Music>, u32)>,
    block: [f32; BLOCK],
    /// The sample of the block that comes next.
    next: usize,
    peak: [f32; 2],
}

/// The two ends of the sound.
pub fn engine() -> (Engine, Stream) {
    let (orders, taken) = channel();
    let (given, spent) = channel();
    let meter = Arc::new(Meter::default());
    let engine = Engine {
        orders,
        spent: Mutex::new(spent),
        meter: meter.clone(),
        conductor: Conductor::default(),
        playing: false,
    };
    let stream = Stream {
        orders: taken,
        spent: given,
        meter,
        music: None,
        tail: None,
        block: [0.0; BLOCK],
        // Nothing is computed before it is asked for.
        next: BLOCK,
        peak: [0.0; 2],
    };
    (engine, stream)
}

impl Engine {
    /// Plays a piece from the start of the section of a context.
    pub fn play(&mut self, score: Arc<Score>, context: Context) {
        let (music, conductor) = music_of(score, context);
        self.conductor = conductor;
        self.playing = true;
        // The stream is only gone when the program ends.
        let _ = self.orders.send(Order::Play(Box::new(music)));
    }

    /// Hands the music that plays another state of its piece: it goes on from where it is.
    pub fn rescore(&self, score: Arc<Score>) {
        if self.playing {
            let _ = self.orders.send(Order::Rescore(score));
        }
    }

    pub fn stop(&mut self) {
        self.playing = false;
        let _ = self.orders.send(Order::Stop);
    }

    /// Tells the music what happens in the game.
    pub fn tell(&self, context: Context) {
        self.conductor.set(context);
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    /// What the music was last heard to do. Also lets go of what the sound thread is done
    /// with.
    pub fn heard(&self) -> Heard {
        if let Ok(spent) = self.spent.lock() {
            while spent.try_recv().is_ok() {}
        }
        let level = |side: &AtomicU32| f32::from_bits(side.load(Ordering::Relaxed));
        Heard {
            level: [level(&self.meter.left), level(&self.meter.right)],
            spot: unpack(self.meter.spot.load(Ordering::Relaxed)),
        }
    }
}

impl Stream {
    /// Does what the editor asked for since the last block, then computes the next one.
    fn fill(&mut self) {
        while let Ok(order) = self.orders.try_recv() {
            match order {
                Order::Play(music) => {
                    let old = self.music.replace(music);
                    self.retire(old);
                }
                Order::Rescore(score) => {
                    let old = match &mut self.music {
                        Some(music) => music.rescore(score),
                        None => score,
                    };
                    let _ = self.spent.send(Spent::Score(old));
                }
                Order::Stop => {
                    let old = self.music.take();
                    self.retire(old);
                }
            }
        }
        match &mut self.music {
            Some(music) => music.render(&mut self.block),
            None => self.block.fill(0.0),
        }
        if let Some((mut tail, left)) = self.tail.take() {
            let mut dying = [0.0; BLOCK];
            tail.render(&mut dying);
            // From the level of the block before it down to that of the next one.
            let (from, to) = (left as f32 / FADE as f32, (left - 1) as f32 / FADE as f32);
            for (frame, (out, dying)) in self.block.chunks_mut(2).zip(dying.chunks(2)).enumerate() {
                let level = from + (to - from) * frame as f32 / BLOCK_FRAMES as f32;
                out[0] += dying[0] * level;
                out[1] += dying[1] * level;
            }
            if left > 1 {
                self.tail = Some((tail, left - 1));
            } else {
                let _ = self.spent.send(Spent::Music(tail));
            }
        }
        self.measure();
    }

    /// Lets a music die away, and lets go of the one that was dying away.
    fn retire(&mut self, music: Option<Box<Music>>) {
        let Some(music) = music else {
            return;
        };
        if let Some((old, _)) = self.tail.replace((music, FADE)) {
            let _ = self.spent.send(Spent::Music(old));
        }
    }

    /// Tells the meter how loud the block is, and where the music has got to.
    fn measure(&mut self) {
        for (side, peak) in self.peak.iter_mut().enumerate() {
            let samples = self.block.iter().skip(side).step_by(2);
            let loudest = samples.fold(0.0f32, |loudest, sample| loudest.max(sample.abs()));
            *peak = loudest.max(*peak * FALL);
        }
        let [left, right] = self.peak;
        self.meter.left.store(left.to_bits(), Ordering::Relaxed);
        self.meter.right.store(right.to_bits(), Ordering::Relaxed);
        let spot = self.music.as_ref().map_or(0, |music| pack(music.spot()));
        self.meter.spot.store(spot, Ordering::Relaxed);
    }
}

impl Iterator for Stream {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.next == BLOCK {
            self.fill();
            self.next = 0;
        }
        let sample = self.block[self.next];
        self.next += 1;
        Some(sample)
    }
}

#[cfg(test)]
mod tests {
    use tanku_audio::{Place, notation::read, score::game};

    use super::*;

    fn fight() -> Context {
        Context {
            place: Place::Arena,
            fight: true,
            threat: 4,
            ..Context::default()
        }
    }

    /// The next frames of a stream, as samples.
    fn listen(stream: &mut Stream, frames: usize) -> Vec<f32> {
        stream.by_ref().take(2 * frames).collect()
    }

    #[test]
    fn nothing_plays_until_the_editor_asks() {
        let (mut engine, mut stream) = engine();
        // Eight blocks of silence: an order is taken where a block ends.
        let silence = listen(&mut stream, 8 * BLOCK_FRAMES);
        assert!(silence.iter().all(|sample| *sample == 0.0));
        assert_eq!(engine.heard(), Heard::default());
        assert!(!engine.playing());

        engine.play(game(), fight());
        let heard = listen(&mut stream, 44_100);
        assert!(heard.iter().any(|sample| sample.abs() > 0.1));
        // It is the music of the game, to the sample.
        let (mut music, _) = music_of(game(), fight());
        let mut frames = vec![0.0; 2 * 44_100];
        music.render(&mut frames);
        assert!(heard == frames);
        // The meter follows: a fight in an arena, in the first of its sections.
        let told = engine.heard();
        assert!(told.level[0] > 0.05 && told.level[1] > 0.05);
        let spot = told.spot.unwrap();
        assert_eq!((spot.scene, spot.section, spot.bar), (Scene::Arena, 1, 0));
        assert!(engine.playing());
    }

    #[test]
    fn a_music_that_stops_dies_away_in_a_moment() {
        let (mut engine, mut stream) = engine();
        engine.play(game(), fight());
        listen(&mut stream, 20 * BLOCK_FRAMES);
        engine.stop();
        // Four blocks later it is silent, and nothing jumped on the way.
        let dying = listen(&mut stream, FADE as usize * BLOCK_FRAMES);
        assert!(dying[..BLOCK].iter().any(|sample| *sample != 0.0));
        let jumps = dying.chunks(2).zip(dying.chunks(2).skip(1));
        let jump = jumps.map(|(one, next)| (next[0] - one[0]).abs());
        assert!(jump.fold(0.0, f32::max) < 0.3);
        let silent = listen(&mut stream, 2000);
        assert!(silent.iter().all(|sample| *sample == 0.0));
        assert_eq!(engine.heard().spot, None);
        assert!(!engine.playing());
    }

    #[test]
    fn another_state_of_the_piece_goes_on_from_where_the_music_is() {
        let text = include_str!("../../tanku_audio/music.score")
            .replace("mix kick  32% 32%", "mix kick  0% 0%");
        let other = Arc::new(Score::new(read(&text).unwrap()));
        let (mut engine, mut stream) = engine();
        engine.play(game(), fight());
        listen(&mut stream, 2 * 44_100);
        let before = engine.heard().spot.unwrap();
        engine.rescore(other);
        listen(&mut stream, BLOCK_FRAMES);
        let after = engine.heard().spot.unwrap();
        // Two seconds are a bar of a fight: the music is in its second bar, before and after.
        assert_eq!((before.bar, after.bar), (1, 1));
        assert_eq!(after.section, before.section);
        // What it is done with came back, and the editor has let go of it.
        assert!(engine.spent.lock().unwrap().try_recv().is_err());
    }

    #[test]
    fn a_spot_survives_the_trip_between_the_threads() {
        for scene in Scene::ALL {
            for (section, bar, step) in [(0, 0, 0), (7, 7, 15), (3, 4, 9)] {
                for between in [false, true] {
                    let spot = Spot {
                        scene,
                        section,
                        bar,
                        step,
                        between,
                    };
                    assert_eq!(unpack(pack(spot)), Some(spot));
                }
            }
        }
        assert_eq!(unpack(0), None);
    }
}
