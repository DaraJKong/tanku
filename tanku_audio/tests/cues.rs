//! The sound cues as they are heard.

use tanku_audio::cues::Cue;

/// A number that changes if one bit of the samples does.
fn fingerprint(samples: &[f32]) -> u64 {
    samples.iter().fold(0xCBF2_9CE4_8422_2325, |hash, sample| {
        (hash ^ sample.to_bits() as u64).wrapping_mul(0x0100_0000_01B3)
    })
}

#[test]
fn the_cues_are_stable() {
    // Golden values: a change here changes what a cue sounds like. They are in the order of
    // the cues.
    let golden: [u64; 16] = [
        0x5E6C_A888_83FA_893F,
        0x8995_BBCE_6C99_3098,
        0xE7CA_389B_D3FA_4AEC,
        0x61BB_AC26_645E_76D9,
        0x230F_AEC0_9E20_1917,
        0x160A_82D3_76A8_62D9,
        0xCC9F_5ACD_7E9F_D20B,
        0x15D6_CBBA_D225_A386,
        0xAFDD_CCC6_8F9D_E863,
        0x5C0A_1595_5DF7_25A9,
        0xB904_489E_CF00_C10A,
        0x336F_39FB_955E_ECF4,
        0x02B1_2B55_65ED_C940,
        0x5FDA_56E3_F03A_5A27,
        0x79B5_EA22_D6B8_FA44,
        0x8399_C49A_1ECB_CB54,
    ];
    let heard: Vec<u64> = Cue::ALL
        .iter()
        .map(|cue| fingerprint(&cue.samples()))
        .collect();
    assert_eq!(heard, golden, "{heard:#018X?}");
}
