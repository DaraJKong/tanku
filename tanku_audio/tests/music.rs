//! The music as it is heard: whole bars of it, rendered and checked.

use tanku_audio::{CHANNELS, Conductor, Context, Music, Place, SAMPLE_RATE, music, music_from};

/// Frames in a bar, which lasts two seconds.
const BAR: usize = 2 * SAMPLE_RATE as usize;
/// Frames in a bar of the bridge, which is slower.
const BRIDGE_BAR: usize = 100_800;

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

/// An arrangement of the piece: its name, its context and the frames of its loop.
type Arrangement = (String, Context, usize);

/// Every arrangement of the piece: the intro, the fights of a corridor at each threat, the
/// bridge, and the fights of an arena at each threat.
fn arrangements() -> Vec<Arrangement> {
    let mut all = vec![("intro".to_string(), at(Place::Bay), 8 * BAR)];
    for threat in 0..=4 {
        let name = format!("corridor at threat {threat}");
        all.push((name, fight(Place::Corridor, threat), 16 * BAR));
    }
    all.push(("bridge".to_string(), at(Place::Corridor), 8 * BRIDGE_BAR));
    for threat in 0..=4 {
        let name = format!("arena at threat {threat}");
        all.push((name, fight(Place::Arena, threat), 16 * BAR));
    }
    all
}

/// The next frames of a music.
fn play(music: &mut Music, frames: usize) -> Vec<f32> {
    let mut frames = vec![0.0; frames * CHANNELS as usize];
    music.render(&mut frames);
    frames
}

/// The first 32 seconds of an arrangement.
fn listen((_, context, _): &Arrangement) -> Vec<f32> {
    play(&mut music_from(*context).0, 16 * BAR)
}

/// A number that changes if one bit of the frames does.
fn fingerprint(frames: &[f32]) -> u64 {
    frames.iter().fold(0xCBF2_9CE4_8422_2325, |hash, frame| {
        (hash ^ frame.to_bits() as u64).wrapping_mul(0x0100_0000_01B3)
    })
}

/// How loud some frames are on average.
fn loudness(frames: &[f32]) -> f32 {
    (frames.iter().map(|frame| frame * frame).sum::<f32>() / frames.len() as f32).sqrt()
}

/// Contexts that change at random moments, from a small generator of its own.
fn restless(music: &mut Music, conductor: &Conductor, seconds: usize) -> Vec<f32> {
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
            place_only: draw(6) == 0,
        });
        // Anything from a few frames to three seconds.
        let mut part = vec![0.0; (1 + draw(3 * SAMPLE_RATE) as usize) * CHANNELS as usize];
        music.render(&mut part);
        frames.extend(part);
    }
    frames
}

#[test]
fn the_music_is_stable() {
    // Golden values: a change here changes what the music sounds like. They are in the order
    // of the arrangements: the intro, the five threats of a corridor, the bridge, then the
    // five threats of an arena.
    let golden: [u64; 12] = [
        0x0F8E_4AE9_5C2E_C9EB,
        0xB707_E705_EDA8_4A93,
        0x5AD8_C9AE_6783_73FA,
        0x8B59_7837_D1A8_D34B,
        0xDB76_65F3_DD0C_8733,
        0x2BA7_3B84_D898_B3BD,
        0xF687_5E7B_4823_6191,
        0x62F5_550B_A741_2DB3,
        0xF424_7194_4520_A5D2,
        0xF29D_D23F_4D42_2478,
        0xE78D_6423_02D7_1900,
        0x5936_2A65_08B9_2618,
    ];
    let heard: Vec<u64> = arrangements()
        .iter()
        .map(|arrangement| fingerprint(&listen(arrangement)))
        .collect();
    assert_eq!(heard, golden, "{heard:#018X?}");
}

#[test]
fn the_same_events_give_the_same_frames() {
    let run = || {
        let (mut music, conductor) = music();
        fingerprint(&restless(&mut music, &conductor, 60))
    };
    assert_eq!(run(), run());
}

#[test]
fn every_bar_makes_sound() {
    for arrangement in arrangements() {
        let frames = listen(&arrangement);
        for (bar, frames) in frames.chunks(BAR * CHANNELS as usize).enumerate() {
            assert!(
                loudness(frames) > 0.01,
                "bar {bar} of the {}",
                arrangement.0
            );
        }
    }
    // The music never stops, whatever happens.
    let (mut music, conductor) = music();
    let frames = restless(&mut music, &conductor, 120);
    for (bar, frames) in frames.chunks(BAR * CHANNELS as usize).enumerate() {
        assert!(loudness(frames) > 0.002, "bar {bar}");
    }
}

#[test]
fn nothing_is_too_loud() {
    let in_range = |frames: &[f32]| frames.iter().all(|frame| frame.abs() < 1.0);
    for arrangement in arrangements() {
        let frames = listen(&arrangement);
        assert!(in_range(&frames), "the {}", arrangement.0);
        // Loud enough to listen to, and far from the top. The bridge is the quietest: a good
        // part of it is silence.
        let loud = loudness(&frames);
        assert!((0.03..0.2).contains(&loud), "the {}: {loud}", arrangement.0);
    }
    let (mut music, conductor) = music();
    assert!(in_range(&restless(&mut music, &conductor, 120)));
}

#[test]
fn loops_are_seamless() {
    // The third turn of a loop is the second one again, bit for bit: nothing drifts.
    for (name, context, frames) in arrangements() {
        let mut music = music_from(context).0;
        play(&mut music, frames);
        let second = play(&mut music, frames);
        let third = play(&mut music, frames);
        assert!(second == third, "the {name}");
    }
}
