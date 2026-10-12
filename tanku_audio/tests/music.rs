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
