//! The music as it is heard today, to the bit. A change of `music.score` that is heard changes
//! the values here: the checks that hold for any piece are in `checks.rs`.

mod common;

use common::{arrangements, fingerprint, play, restless};
use tanku_audio::{SAMPLE_RATE, music, music_from};

#[test]
fn the_music_is_stable() {
    // Golden values: a change here changes what the music sounds like. They are in the order
    // of the arrangements: the intro, the five threats of a corridor, the bridge, then the
    // five threats of an arena.
    let golden: [u64; 12] = [
        0x0F8E_4AE9_5C2E_C9EB,
        0xC9A3_AA76_F6C2_6242,
        0x6906_5710_EAFB_C2C1,
        0xA735_CF6C_69F9_569D,
        0x2896_188F_1387_63D6,
        0xCA00_951D_7AE8_9D5B,
        0x9938_7853_498F_12D4,
        0x97F8_882E_8183_384D,
        0x117D_1DA4_D89F_5B2E,
        0xACC9_1BD1_0426_4271,
        0x4576_D8D5_25E1_09DE,
        0xB658_F269_1A91_3B3D,
    ];
    // The first 32 seconds of each.
    let heard: Vec<u64> = arrangements()
        .iter()
        .map(|arrangement| {
            let mut music = music_from(arrangement.context).0;
            fingerprint(&play(&mut music, 32 * SAMPLE_RATE as usize))
        })
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
