//! Small text files kept on the player's machine: one `key = value` pair per line.
//!
//! The web build keeps them in the local storage of the browser.

use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

/// Set for a run that must leave the player's files alone.
static KEEPS_NOTHING: AtomicBool = AtomicBool::new(false);

/// Whether the player's files are left alone: in a run that asked for it, and in every test.
fn keeps_nothing() -> bool {
    cfg!(test) || KEEPS_NOTHING.load(Ordering::Relaxed)
}

/// Makes this run read and write nothing: it starts from the defaults and leaves no trace.
#[cfg(feature = "script")]
pub fn keep_nothing() {
    KEEPS_NOTHING.store(true, Ordering::Relaxed);
}

#[cfg(not(target_arch = "wasm32"))]
use tanku_core::files::directory;

/// Reads a file of pairs. A missing or unreadable file reads as empty.
pub fn load(name: &str) -> BTreeMap<String, String> {
    if keeps_nothing() {
        return BTreeMap::new();
    }
    parse(&read(name).unwrap_or_default())
}

pub fn save(name: &str, pairs: &BTreeMap<String, String>) {
    if keeps_nothing() {
        return;
    }
    let text: String = pairs
        .iter()
        .map(|(key, value)| format!("{key} = {value}\n"))
        .collect();
    write(name, &text);
}

/// Adds a line to a file of the player that is kept line by line, such as the journal of the
/// runs. A file that is new starts with its heading. A page keeps no such file: nothing
/// outside the browser could read it.
#[cfg(not(target_arch = "wasm32"))]
pub fn append(name: &str, heading: &str, line: &str) {
    if keeps_nothing() {
        return;
    }
    if let Some(directory) = directory() {
        append_to(&directory.join(name), heading, line);
    }
}

/// Adds a line to a file that whoever started the game named. That file is not one of the
/// player's own: a run that leaves those alone still writes it.
#[cfg(not(target_arch = "wasm32"))]
pub fn append_to(path: &std::path::Path, heading: &str, line: &str) {
    if let Err(error) = add_to(path, heading, line) {
        bevy::log::warn!("could not add to {}: {error}", path.display());
    }
}

fn parse(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn read(name: &str) -> Option<String> {
    std::fs::read_to_string(directory()?.join(name)).ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn write(name: &str, text: &str) {
    let Some(directory) = directory() else {
        return;
    };
    let written = std::fs::create_dir_all(&directory)
        .and_then(|()| std::fs::write(directory.join(name), text));
    if let Err(error) = written {
        bevy::log::warn!("could not save {name}: {error}");
    }
}

/// Adds a line at the end of a file, after its heading if the file is new or empty.
#[cfg(not(target_arch = "wasm32"))]
fn add_to(path: &std::path::Path, heading: &str, line: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let text = match file.metadata()?.len() {
        0 => format!("{heading}\n{line}\n"),
        _ => format!("{line}\n"),
    };
    // One write, so that two games on a machine don't cut each other's lines.
    file.write_all(text.as_bytes())
}

/// The local storage of the page, if the browser allows one.
#[cfg(target_arch = "wasm32")]
fn browser() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
fn read(name: &str) -> Option<String> {
    browser()?.get_item(&format!("tanku/{name}")).ok()?
}

#[cfg(target_arch = "wasm32")]
fn write(name: &str, text: &str) {
    let written = browser().map(|storage| storage.set_item(&format!("tanku/{name}"), text));
    if !matches!(written, Some(Ok(()))) {
        bevy::log::warn!("could not save {name}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_test_leaves_the_files_of_the_player_alone() {
        assert!(keeps_nothing());
        assert!(load("settings.txt").is_empty());
    }

    #[test]
    fn a_line_is_added_at_the_end_of_a_file_that_starts_with_its_heading() {
        // The test writes to a file of its own, since it leaves the player's files alone.
        let path = std::env::temp_dir().join(format!("tanku-append-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        append("journal.txt", "heading", "never written");
        add_to(&path, "heading 1", "first").unwrap();
        add_to(&path, "heading 1", "second").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(text, "heading 1\nfirst\nsecond\n");
    }

    #[test]
    fn pairs_are_parsed_and_junk_is_skipped() {
        let pairs = parse("volume = 0.5\n\nnot a pair\n bind.up=KeyW, ArrowUp \n");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs["volume"], "0.5");
        assert_eq!(pairs["bind.up"], "KeyW, ArrowUp");
        // What a player typed may hold the sign that parts a pair.
        let pairs = parse("host.name = 2 + 2 = 5\n");
        assert_eq!(pairs["host.name"], "2 + 2 = 5");
    }
}
