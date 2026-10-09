//! Small text files kept on the player's machine: one `key = value` pair per line.

use std::collections::BTreeMap;

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
    parse(&read(name).unwrap_or_default())
}

pub fn save(name: &str, pairs: &BTreeMap<String, String>) {
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

// The web build keeps nothing yet: browser storage needs a dependency the project doesn't have.
#[cfg(target_arch = "wasm32")]
fn read(_name: &str) -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
fn write(_name: &str, _text: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_are_parsed_and_junk_is_skipped() {
        let pairs = parse("volume = 0.5\n\nnot a pair\n bind.up=KeyW, ArrowUp \n");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs["volume"], "0.5");
        assert_eq!(pairs["bind.up"], "KeyW, ArrowUp");
    }
}
