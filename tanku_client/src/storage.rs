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

/// The directory where the game keeps its files, if the platform has one.
#[cfg(not(target_arch = "wasm32"))]
fn directory() -> Option<std::path::PathBuf> {
    use std::{env::var_os, path::PathBuf};
    let base = if cfg!(target_os = "windows") {
        var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    };
    base.map(|base| base.join("tanku"))
}

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
