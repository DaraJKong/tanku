//! The document: the file of a piece as the editor holds it.
//!
//! It is the lines of the file and nothing else. A change rewrites some of them and hands
//! the whole text to the reader of `tanku_audio`, which is the only thing that says what a
//! file means. A change that the reader refuses is not made, and the document keeps why. So
//! the document always reads, a piece of it can always be played, and the lines that no
//! change touched stay as they were written, comments and columns included.

use std::{path::PathBuf, sync::Arc};

use bevy::prelude::Resource;
use tanku_audio::{
    notation::{Piece, read},
    score::Score,
};

use crate::lines;

/// States of the lines that undoing can go back to.
const UNDO_DEPTH: usize = 500;

#[derive(Resource)]
pub struct Doc {
    /// Where the file is, if the document has somewhere to be saved.
    pub path: Option<PathBuf>,
    lines: Vec<String>,
    /// The lines as they are in the file.
    saved: Vec<String>,
    piece: Piece,
    score: Arc<Score>,
    saved_score: Arc<Score>,
    undone: Vec<Vec<String>>,
    redone: Vec<Vec<String>>,
    /// Goes up with every change of the lines.
    revision: u64,
    /// Why the last change was not made, in the words of the reader.
    pub refusal: Option<String>,
    /// While it is set, a change goes on with the one before it, and undoing takes both back
    /// at once: a button that is held moves a number by many steps, which are one change.
    pub joined: bool,
}

fn split(text: &str) -> Vec<String> {
    text.lines().map(str::to_string).collect()
}

fn join(lines: &[String]) -> String {
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

impl Doc {
    /// The document of a text, or the line on which the text can't be read and why.
    pub fn of(text: &str, path: Option<PathBuf>) -> Result<Self, String> {
        let piece = read(text).map_err(|fault| fault.to_string())?;
        let score = Arc::new(Score::new(piece));
        let lines = split(text);
        Ok(Self {
            path,
            saved: lines.clone(),
            lines,
            piece,
            saved_score: score.clone(),
            score,
            undone: Vec::new(),
            redone: Vec::new(),
            revision: 0,
            refusal: None,
            joined: false,
        })
    }

    /// The document of a file.
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{} can't be read: {error}", path.display()))?;
        let name = path.display().to_string();
        Self::of(&text, Some(path)).map_err(|fault| format!("{name}, {fault}"))
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn saved(&self) -> &[String] {
        &self.saved
    }

    pub fn text(&self) -> String {
        join(&self.lines)
    }

    /// The piece that the lines write.
    pub fn piece(&self) -> &Piece {
        &self.piece
    }

    /// That piece with every line that the rules derive from it, to play it.
    pub fn score(&self) -> &Arc<Score> {
        &self.score
    }

    /// The piece of the file as it was last saved.
    pub fn saved_score(&self) -> &Arc<Score> {
        &self.saved_score
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The line of a record, by the words that it starts with.
    pub fn find(&self, head: &[&str]) -> Option<usize> {
        lines::find(&self.lines, head)
    }

    /// The words of a record, by the words that it starts with.
    pub fn record(&self, head: &[&str]) -> Option<Vec<&str>> {
        self.find(head).map(|at| lines::words(&self.lines[at]))
    }

    /// Changes the lines, if what they then write is a piece. A change that the reader
    /// refuses is not made: the lines stay as they were, and `refusal` says why.
    pub fn change(&mut self, change: impl FnOnce(&mut Vec<String>)) -> bool {
        let mut lines = self.lines.clone();
        change(&mut lines);
        if lines == self.lines {
            return false;
        }
        match read(&join(&lines)) {
            Ok(piece) => {
                let before = std::mem::replace(&mut self.lines, lines);
                if !self.joined || self.undone.is_empty() {
                    self.undone.push(before);
                }
                if self.undone.len() > UNDO_DEPTH {
                    self.undone.remove(0);
                }
                self.redone.clear();
                self.take(piece);
                true
            }
            Err(fault) => {
                self.refusal = Some(fault.to_string());
                false
            }
        }
    }

    /// Replaces one word of a record, by the words that the record starts with.
    pub fn set_word(&mut self, head: &[&str], index: usize, word: &str) -> bool {
        let Some(at) = self.find(head) else {
            return false;
        };
        self.change(|lines| lines[at] = lines::with_word(&lines[at], index, word))
    }

    /// Replaces a record by another line, or puts the line in if no record starts so.
    pub fn set_line(&mut self, head: &[&str], line: String) -> bool {
        match self.find(head) {
            Some(at) => self.change(|lines| lines[at] = line),
            None => self.change(|lines| lines::insert(lines, line)),
        }
    }

    fn take(&mut self, piece: Piece) {
        self.piece = piece;
        self.score = Arc::new(Score::new(piece));
        self.revision += 1;
        self.refusal = None;
    }

    /// The lines as they are, read again. They always read: they did when they were set.
    fn reread(&mut self) {
        if let Ok(piece) = read(&join(&self.lines)) {
            self.take(piece);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undone.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redone.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(lines) = self.undone.pop() else {
            return false;
        };
        self.redone.push(std::mem::replace(&mut self.lines, lines));
        self.reread();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(lines) = self.redone.pop() else {
            return false;
        };
        self.undone.push(std::mem::replace(&mut self.lines, lines));
        self.reread();
        true
    }

    /// Goes back to the file as it was last saved. It can be undone.
    pub fn revert(&mut self) -> bool {
        let saved = self.saved.clone();
        self.change(|lines| *lines = saved)
    }

    /// Writes the lines to the file.
    pub fn save(&mut self) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Err("this document has no file to be saved in".to_string());
        };
        std::fs::write(path, self.text())
            .map_err(|error| format!("{} can't be written: {error}", path.display()))?;
        self.saved = self.lines.clone();
        self.saved_score = self.score.clone();
        self.revision += 1;
        Ok(())
    }

    /// Whether a line is not in the file as it was last saved.
    pub fn is_changed(&self, at: usize) -> bool {
        let line = &self.lines[at];
        let count = |lines: &[String]| lines.iter().filter(|other| *other == line).count();
        count(&self.lines) > count(&self.saved)
    }

    /// The lines that are not in the file as it was last saved, and how many of its lines are
    /// gone.
    pub fn changes(&self) -> (Vec<usize>, usize) {
        let changed = (0..self.lines.len()).filter(|&at| self.is_changed(at));
        let gone = |line: &&String| {
            let count = |lines: &[String]| lines.iter().filter(|other| other == line).count();
            count(&self.saved) > count(&self.lines)
        };
        (changed.collect(), self.saved.iter().filter(gone).count())
    }

    /// How many changes are not saved: a line that changed counts once, not as a line that
    /// went and a line that came.
    pub fn unsaved(&self) -> usize {
        let (changed, gone) = self.changes();
        changed.len().max(gone)
    }
}

#[cfg(test)]
mod tests {
    use tanku_audio::notation::Instrument;

    use super::*;

    const FILE: &str = include_str!("../../tanku_audio/music.score");

    fn doc() -> Doc {
        Doc::of(FILE, None).unwrap()
    }

    #[test]
    fn the_file_of_the_game_opens_as_it_is() {
        let doc = doc();
        assert_eq!(doc.text(), FILE);
        assert_eq!(doc.piece(), &tanku_audio::score::PIECE);
        assert_eq!(doc.lines().len(), 56);
        assert_eq!((doc.unsaved(), doc.revision()), (0, 0));
        assert!(!doc.can_undo() && !doc.can_redo());
    }

    #[test]
    fn a_change_is_read_before_it_is_made() {
        let mut doc = doc();
        assert!(doc.set_word(&["tempo"], 1, "96"));
        assert_eq!(doc.piece().tempo, 96);
        assert_eq!((doc.unsaved(), doc.revision()), (1, 1));
        assert_eq!(doc.record(&["tempo"]).unwrap(), ["tempo", "96"]);
        // The line above it is the heading still, and nothing else moved.
        assert_eq!(doc.lines()[1], "tanku-music 1");
        assert_eq!(doc.lines()[4], "key A dorian aeolian");

        // A tempo that no piece has: the reader says why, and nothing changes.
        assert!(!doc.set_word(&["tempo"], 1, "960"));
        assert_eq!(
            doc.refusal.as_deref(),
            Some("line 4: a tempo is a number of beats per minute, from 30 to 300")
        );
        assert_eq!((doc.piece().tempo, doc.revision()), (96, 1));
        // The next change that is made clears it.
        assert!(doc.set_word(&["key"], 1, "C#"));
        assert_eq!((doc.refusal.as_deref(), doc.piece().key.tonic), (None, 4));
        // A change that changes nothing is not one.
        assert!(!doc.set_word(&["key"], 1, "C#"));
        assert_eq!(doc.revision(), 2);
    }

    #[test]
    fn changes_are_undone_and_done_again() {
        let mut doc = doc();
        doc.set_word(&["tempo"], 1, "96");
        doc.set_word(&["mix", "kick"], 2, "40%");
        assert_eq!(doc.piece().mix(Instrument::Kick), [0.4, 0.32]);
        assert!(doc.undo());
        assert_eq!(doc.piece().mix(Instrument::Kick), [0.32, 0.32]);
        assert_eq!(doc.piece().tempo, 96);
        assert!(doc.undo() && !doc.undo());
        assert_eq!(doc.text(), FILE);
        assert_eq!(doc.unsaved(), 0);
        assert!(doc.redo() && doc.redo() && !doc.redo());
        assert_eq!(doc.piece().mix(Instrument::Kick), [0.4, 0.32]);
        // A new change forgets what was undone.
        doc.undo();
        doc.set_word(&["tempo"], 1, "100");
        assert!(!doc.can_redo());
        assert_eq!(doc.piece().tempo, 100);
        // Every state has its own number, so that whoever follows the document sees it move.
        assert_eq!(doc.revision(), 8);
    }

    #[test]
    fn the_steps_of_a_held_button_are_one_change() {
        let mut doc = doc();
        doc.set_word(&["tempo"], 1, "121");
        doc.joined = true;
        doc.set_word(&["tempo"], 1, "122");
        doc.set_word(&["tempo"], 1, "123");
        doc.joined = false;
        assert_eq!(doc.piece().tempo, 123);
        assert!(doc.undo() && !doc.can_undo());
        assert_eq!(doc.text(), FILE);
        // Done again, it is the last of them.
        assert!(doc.redo());
        assert_eq!(doc.piece().tempo, 123);
        // Without a change before it, a change that goes on is a change of its own.
        let mut doc = self::doc();
        doc.joined = true;
        doc.set_word(&["tempo"], 1, "96");
        assert!(doc.undo());
    }

    #[test]
    fn what_is_not_saved_is_counted_by_line() {
        let mut doc = doc();
        doc.set_word(&["mix", "kick"], 2, "40%");
        doc.set_word(&["mix", "kick"], 3, "40%");
        doc.set_line(&["motif", "D"], "motif D 1... .... .... ....".into());
        // One line that changed twice and one that is new.
        assert_eq!(doc.unsaved(), 2);
        let (changed, gone) = doc.changes();
        assert_eq!((changed.as_slice(), gone), ([9, 44].as_slice(), 1));
        assert!(doc.is_changed(9) && !doc.is_changed(8));
        // The file as it was comes back in one change, which is undone like any other.
        assert!(doc.revert());
        assert_eq!((doc.text().as_str(), doc.unsaved()), (FILE, 0));
        assert!(doc.undo());
        assert_eq!(doc.unsaved(), 2);
    }

    #[test]
    fn a_document_is_saved_where_it_was_opened() {
        let directory = std::env::temp_dir().join(format!("music_editor_{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("saved.score");
        std::fs::write(&path, FILE).unwrap();

        let mut doc = Doc::open(path.clone()).unwrap();
        doc.set_word(&["tempo"], 1, "96");
        assert_eq!(doc.unsaved(), 1);
        doc.save().unwrap();
        assert_eq!(doc.unsaved(), 0);
        let written = std::fs::read_to_string(&path).unwrap();
        assert_eq!(written, FILE.replace("tempo 120", "tempo 96"));
        assert!(Arc::ptr_eq(doc.saved_score(), doc.score()));
        // A change can still be undone after it was saved: the file is then behind again.
        assert!(doc.undo());
        assert_eq!(doc.unsaved(), 1);

        // A document without a file says so, and a file that isn't a piece says where.
        assert!(Doc::of(FILE, None).unwrap().save().is_err());
        std::fs::write(&path, "tanku-music 1\ntempo fast\n").unwrap();
        let fault = Doc::open(path.clone()).err().unwrap();
        assert!(fault.ends_with(
            "saved.score, line 2: a tempo is a number of beats per minute, from 30 to 300"
        ));
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
