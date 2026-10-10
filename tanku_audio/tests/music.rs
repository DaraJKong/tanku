//! The music as it is heard: whole bars of it, rendered and checked.

use tanku_audio::{CHANNELS, Context, Music, Place, SAMPLE_RATE, music, music_from};

/// Frames in a bar, which lasts two seconds.
const BAR: usize = 2 * SAMPLE_RATE as usize;

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

/// Every arrangement of the piece, and the bars of its loop.
fn arrangements() -> Vec<(String, Context, usize)> {
    let mut all = vec![
        ("intro".to_string(), at(Place::Bay), 8),
        ("bridge".to_string(), at(Place::Corridor), 8),
    ];
    for (name, place) in [("corridor", Place::Corridor), ("arena", Place::Arena)] {
        for threat in 0..=4 {
            all.push((
                format!("{name} at threat {threat}"),
                fight(place, threat),
                16,
            ));
        }
    }
    all
}

/// The next bars of a music.
fn play(music: &mut Music, bars: usize) -> Vec<f32> {
    let mut frames = vec![0.0; bars * BAR * CHANNELS as usize];
    music.render(&mut frames);
    frames
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
fn restless(music: &mut Music, conductor: &tanku_audio::Conductor, seconds: usize) -> Vec<f32> {
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
    // Golden values: a change here changes what the music sounds like.
    let golden: [u64; 12] = [
        0x13B1_7353_D38D_66D1,
        0x160D_5305_8BDF_02F2,
        0xD348_9EB7_C5C1_E65A,
        0x24C1_9911_D581_493D,
        0xD35F_4476_D092_DA6B,
        0x13C0_69D4_9B87_DC6F,
        0x077F_EDC6_7C8E_B94C,
        0x62F5_550B_A741_2DB3,
        0xF424_7194_4520_A5D2,
        0xF29D_D23F_4D42_2478,
        0xE78D_6423_02D7_1900,
        0x5936_2A65_08B9_2618,
    ];
    let heard: Vec<u64> = arrangements()
        .into_iter()
        .map(|(_, context, _)| fingerprint(&play(&mut music_from(context).0, 16)))
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
    for (name, context, _) in arrangements() {
        let frames = play(&mut music_from(context).0, 16);
        for (bar, frames) in frames.chunks(BAR * CHANNELS as usize).enumerate() {
            assert!(loudness(frames) > 0.01, "bar {bar} of the {name}");
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
    for (name, context, _) in arrangements() {
        let frames = play(&mut music_from(context).0, 16);
        assert!(in_range(&frames), "the {name}");
        // Loud enough to listen to, and far from the top.
        let loud = loudness(&frames);
        assert!((0.04..0.2).contains(&loud), "the {name}: {loud}");
    }
    let (mut music, conductor) = music();
    assert!(in_range(&restless(&mut music, &conductor, 120)));
}

#[test]
fn loops_are_seamless() {
    // The third turn of a loop is the second one again, bit for bit: nothing drifts.
    for (name, context, bars) in arrangements() {
        let mut music = music_from(context).0;
        play(&mut music, bars);
        let second = play(&mut music, bars);
        let third = play(&mut music, bars);
        assert!(second == third, "the {name}");
    }
}
