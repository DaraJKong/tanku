//! What the tests of the music share: the arrangements of the piece of the game, and how they
//! are heard.

#![allow(dead_code)]

#[allow(unused_imports)]
pub use tanku_audio::checks::{Arrangement, loudness, play, restless};
use tanku_audio::{checks, score::game};

/// Every arrangement of the piece of the game.
pub fn arrangements() -> Vec<Arrangement> {
    checks::arrangements(&game())
}

/// A number that changes if one bit of the frames does.
pub fn fingerprint(frames: &[f32]) -> u64 {
    frames.iter().fold(0xCBF2_9CE4_8422_2325, |hash, frame| {
        (hash ^ frame.to_bits() as u64).wrapping_mul(0x0100_0000_01B3)
    })
}
