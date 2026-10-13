//! Where the game keeps its files on a player's machine: its settings, its records and the
//! journal of its runs. The game writes them, and the trials read the journal.

use std::{env::var_os, path::PathBuf};

/// The directory where the game keeps its files, if the platform has one.
pub fn directory() -> Option<PathBuf> {
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
