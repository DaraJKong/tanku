//! What must hold for any piece that `music.score` writes: the rules of `docs/MUSIC.md` that a
//! change of a note, of a pattern or of a level could break. Nothing here says what the piece
//! is, so these are the tests to run after a change of the file:
//!
//! ```bash
//! cargo test -p tanku_audio --test checks
//! ```
//!
//! The checks themselves are in the crate, for a program that changes a piece to run them
//! too.

use tanku_audio::{checks::Check, score::game};

/// Fails with what a check finds wrong with the piece of the game, each in its sentence.
fn holds(check: Check) {
    let found = check.run(&game());
    assert!(found.is_empty(), "{}", found.join("\n"));
}

#[test]
fn the_melodies_of_the_fights_use_five_degrees() {
    holds(Check::FiveDegrees);
}

#[test]
fn every_chord_is_major_or_minor() {
    holds(Check::Chords);
}

#[test]
fn no_note_rubs_against_the_root_of_its_chord() {
    holds(Check::Rubs);
}

#[test]
fn the_calm_has_no_mode() {
    holds(Check::Calm);
}

#[test]
fn every_instrument_stays_in_its_range() {
    holds(Check::Ranges);
}

#[test]
fn every_bar_makes_sound() {
    holds(Check::Sounding);
}

#[test]
fn nothing_is_too_loud() {
    holds(Check::Loudness);
}

#[test]
fn loops_are_seamless() {
    holds(Check::Seamless);
}
