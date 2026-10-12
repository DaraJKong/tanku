//! What the tests of the music share: the arrangements of the piece, and how they are heard.

#![allow(dead_code)]

use tanku_audio::{
    CHANNELS, Conductor, Context, Music, Place, SAMPLE_RATE,
    notation::{Passage, Scene},
    score::{BARS, SCORE, STEPS, step},
};

/// Frames in a bar of a section. A step is counted in half frames.
pub fn bar(passage: &Passage) -> usize {
    STEPS * step(passage.tempo) as usize / 2
}

/// The sections that a scene plays in turn, each with its place among those of the piece.
pub fn sections(scene: Scene) -> Vec<(u8, &'static Passage)> {
    let places = SCORE.piece.play(scene).sections().iter();
    places
        .map(|&index| (index, SCORE.piece.section(index)))
        .collect()
}

pub fn at(place: Place) -> Context {
    Context {
        place,
        ..Context::default()
    }
}

pub fn fight(place: Place, threat: u8) -> Context {
    Context {
        place,
        fight: true,
        threat,
        ..Context::default()
    }
}

/// An arrangement of the piece.
pub struct Arrangement {
    pub name: String,
    pub context: Context,
    /// Frames in its first bar, and in one turn of its loop.
    pub bar: usize,
    pub turn: usize,
}

/// Every arrangement of the piece: the menu, the fights of a corridor at each threat, the
/// calm, and the fights of an arena at each threat.
pub fn arrangements() -> Vec<Arrangement> {
    let arrangement = |name: String, scene: Scene, context: Context| {
        let bars = sections(scene).into_iter().map(|(_, passage)| bar(passage));
        let bars: Vec<usize> = bars.collect();
        Arrangement {
            name,
            context,
            bar: bars[0],
            turn: BARS * bars.iter().sum::<usize>(),
        }
    };
    let mut all = vec![arrangement("menu".into(), Scene::Menu, at(Place::Bay))];
    for threat in 0..=4 {
        let name = format!("corridor at threat {threat}");
        all.push(arrangement(
            name,
            Scene::Corridor,
            fight(Place::Corridor, threat),
        ));
    }
    all.push(arrangement("calm".into(), Scene::Calm, at(Place::Corridor)));
    for threat in 0..=4 {
        let name = format!("arena at threat {threat}");
        all.push(arrangement(name, Scene::Arena, fight(Place::Arena, threat)));
    }
    all
}

/// The next frames of a music.
pub fn play(music: &mut Music, frames: usize) -> Vec<f32> {
    let mut frames = vec![0.0; frames * CHANNELS as usize];
    music.render(&mut frames);
    frames
}

/// A number that changes if one bit of the frames does.
pub fn fingerprint(frames: &[f32]) -> u64 {
    frames.iter().fold(0xCBF2_9CE4_8422_2325, |hash, frame| {
        (hash ^ frame.to_bits() as u64).wrapping_mul(0x0100_0000_01B3)
    })
}

/// How loud some frames are on average.
pub fn loudness(frames: &[f32]) -> f32 {
    (frames.iter().map(|frame| frame * frame).sum::<f32>() / frames.len() as f32).sqrt()
}

/// Contexts that change at random moments, from a small generator of its own.
pub fn restless(music: &mut Music, conductor: &Conductor, seconds: usize) -> Vec<f32> {
    let mut state = 12_345u32;
    let mut draw = |below: u32| {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) % below
    };
    let places = [Place::Menu, Place::Bay, Place::Corridor, Place::Arena];
    let mut frames = Vec::new();
    while frames.len() < seconds * SAMPLE_RATE as usize * CHANNELS as usize {
        conductor.set(Context {
            place: places[draw(4) as usize],
            fight: draw(2) == 0,
            threat: draw(5) as u8,
            down: draw(6) == 0,
            muffled: draw(6) == 0,
            ducked: draw(6) == 0,
        });
        // Anything from a few frames to three seconds.
        let mut part = vec![0.0; (1 + draw(3 * SAMPLE_RATE) as usize) * CHANNELS as usize];
        music.render(&mut part);
        frames.extend(part);
    }
    frames
}
