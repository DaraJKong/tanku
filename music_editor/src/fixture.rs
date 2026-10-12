//! A piece that stands still, for the tests of the editor.
//!
//! They press the buttons of the window and read the lines that come out, so they need a
//! file whose lines they know. It can't be the piece of the game, which the editor is there
//! to change: this is that piece as it was when the tests were written.

use std::sync::{Arc, LazyLock};

use tanku_audio::{
    notation::{self, Piece},
    score::Score,
};

pub const FILE: &str = include_str!("../fixtures/piece.score");

/// The piece that the file writes.
pub static PIECE: LazyLock<Piece> =
    LazyLock::new(|| notation::read(FILE).expect("the piece of the tests reads"));

/// That piece with every line that the rules derive from it.
pub static SCORE: LazyLock<Arc<Score>> = LazyLock::new(|| Arc::new(Score::new(*PIECE)));

/// The same, for whoever plays it.
pub fn score() -> Arc<Score> {
    SCORE.clone()
}
