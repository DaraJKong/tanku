//! The music as it is heard: whole bars of it, rendered and checked.

use tanku_audio::{
    CHANNELS, Conductor, Context, Mode, Music, Place, SAMPLE_RATE, music, music_from,
};

/// Frames in a bar, which lasts two seconds.
const BAR: usize = 2 * SAMPLE_RATE as usize;

/// The two candidates for the mode of the piece.
const HOMES: [Mode; 2] = [Mode::Dorian, Mode::Mixolydian];

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

/// An arrangement of the piece: its name, the mode of the piece, its context and the bars of
/// its loop.
type Arrangement = (String, Mode, Context, usize);

/// What depends on the mode of the piece: the intro, the bridge and the fights in a corridor.
fn at_home(home: Mode) -> Vec<Arrangement> {
    let mut all = vec![
        (format!("intro in {home:?}"), home, at(Place::Bay), 8),
        (format!("bridge in {home:?}"), home, at(Place::Corridor), 8),
    ];
    for threat in 0..=4 {
        let name = format!("corridor at threat {threat} in {home:?}");
        all.push((name, home, fight(Place::Corridor, threat), 16));
    }
    all
}

/// The fights in an arena, which are in a mode of their own.
fn in_an_arena(home: Mode) -> Vec<Arrangement> {
    let arena = |threat| {
        let name = format!("arena at threat {threat}");
        (name, home, fight(Place::Arena, threat), 16)
    };
    (0..=4).map(arena).collect()
}

/// Every arrangement of the piece: in each of its two modes, then in an arena.
fn arrangements() -> Vec<Arrangement> {
    let mut all: Vec<Arrangement> = HOMES.into_iter().flat_map(at_home).collect();
    all.extend(in_an_arena(HOMES[0]));
    all
}

/// The next bars of a music.
fn play(music: &mut Music, bars: usize) -> Vec<f32> {
    let mut frames = vec![0.0; bars * BAR * CHANNELS as usize];
    music.render(&mut frames);
    frames
}

/// The first 16 bars of an arrangement.
fn listen((_, home, context, _): &Arrangement) -> Vec<f32> {
    play(&mut music_from(*home, *context).0, 16)
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
    // of the arrangements: the intro, the bridge and the five threats of a corridor in Dorian,
    // the same in Mixolydian, then the five threats of an arena.
    let golden: [u64; 19] = [
        0x0F8E_4AE9_5C2E_C9EB,
        0x7EAE_DE25_8D3D_6572,
        0xB707_E705_EDA8_4A93,
        0x5AD8_C9AE_6783_73FA,
        0x8B59_7837_D1A8_D34B,
        0xDB76_65F3_DD0C_8733,
        0x2BA7_3B84_D898_B3BD,
        0x5E9B_66FD_4F15_00C1,
        0x6714_0BA2_F53D_95E6,
        0x21DF_E194_7AE5_8997,
        0x5108_D941_242A_5E6D,
        0xC36C_1144_A2CA_5CF2,
        0xA8D8_5445_186B_4FBB,
        0x3E6D_AD35_B3FC_325A,
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
fn an_arena_sounds_the_same_in_both_modes_of_the_piece() {
    let [dorian, mixolydian] = HOMES.map(in_an_arena);
    for (one, other) in dorian.iter().zip(&mixolydian) {
        assert!(listen(one) == listen(other), "the {}", one.0);
    }
}

#[test]
fn the_two_modes_of_the_piece_sound_different() {
    let [dorian, mixolydian] = HOMES.map(at_home);
    for (one, other) in dorian.iter().zip(&mixolydian) {
        assert!(listen(one) != listen(other), "the {}", one.0);
    }
}

#[test]
fn the_same_events_give_the_same_frames() {
    let run = || {
        let (mut music, conductor) = music(Mode::Dorian);
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
    for home in HOMES {
        let (mut music, conductor) = music(home);
        let frames = restless(&mut music, &conductor, 120);
        for (bar, frames) in frames.chunks(BAR * CHANNELS as usize).enumerate() {
            assert!(loudness(frames) > 0.002, "bar {bar}");
        }
    }
}

#[test]
fn nothing_is_too_loud() {
    let in_range = |frames: &[f32]| frames.iter().all(|frame| frame.abs() < 1.0);
    for arrangement in arrangements() {
        let frames = listen(&arrangement);
        assert!(in_range(&frames), "the {}", arrangement.0);
        // Loud enough to listen to, and far from the top.
        let loud = loudness(&frames);
        assert!((0.04..0.2).contains(&loud), "the {}: {loud}", arrangement.0);
    }
    for home in HOMES {
        let (mut music, conductor) = music(home);
        assert!(in_range(&restless(&mut music, &conductor, 120)));
    }
}

#[test]
fn loops_are_seamless() {
    // The third turn of a loop is the second one again, bit for bit: nothing drifts.
    for (name, home, context, bars) in arrangements() {
        let mut music = music_from(home, context).0;
        play(&mut music, bars);
        let second = play(&mut music, bars);
        let third = play(&mut music, bars);
        assert!(second == third, "the {name}");
    }
}
