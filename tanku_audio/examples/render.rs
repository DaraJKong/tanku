//! Writes the music to sound files, to listen to it without the game:
//!
//! ```bash
//! cargo run -p tanku_audio --example render
//! ```
//!
//! The files go to `target/music`, or to the directory given after `--`. The piece is written
//! once in each of the two modes it may get, in the directories `dorian` and `mixolydian`:
//!
//! - `intro.wav` and `bridge.wav`: two turns of each loop;
//! - `corridor_threat_0.wav` to `corridor_threat_4.wav`: the verse and the chorus of a fight
//!   in a corridor at each threat;
//! - `tour.wav`: three minutes of a made-up run.
//!
//! Next to them, `arena_threat_0.wav` to `arena_threat_4.wav` are the fights of an arena,
//! which sound the same whatever the mode of the piece, and `tour.txt` tells what happens in
//! the tour and when.

use std::{error::Error, fmt::Write, fs, path::PathBuf};

use tanku_audio::{CHANNELS, Context, Mode, Music, Place, SAMPLE_RATE, music, music_from, wav};

/// Seconds in a bar.
const BAR: f32 = 2.0;

/// The made-up run of the tour: when each thing happens, in seconds, and the context from
/// then on. The times are off the beat on purpose, as they are in a game.
const TOUR: [(f32, &str, Context); 19] = [
    (0.0, "The main menu", at(Place::Menu)),
    (9.2, "A run starts, in the bay", at(Place::Bay)),
    (
        19.3,
        "Sector 1, a corridor: a sentry (threat 0)",
        fight(Place::Corridor, 0),
    ),
    (31.6, "Sector 1 is cleared", at(Place::Corridor)),
    (
        40.2,
        "Sector 2: a grunt (threat 1)",
        fight(Place::Corridor, 1),
    ),
    (54.9, "Sector 2 is cleared", at(Place::Corridor)),
    (
        62.4,
        "Sector 6: a soldier and two grunts (threat 2)",
        fight(Place::Corridor, 2),
    ),
    (
        78.1,
        "The soldier is destroyed (threat 1)",
        fight(Place::Corridor, 1),
    ),
    (86.5, "Sector 6 is cleared", at(Place::Corridor)),
    (
        95.8,
        "Sector 7, an arena: soldiers (threat 2)",
        fight(Place::Arena, 2),
    ),
    (122.3, "The arena is cleared", at(Place::Arena)),
    (
        131.0,
        "Sector 11, a corridor: a veteran (threat 3)",
        fight(Place::Corridor, 3),
    ),
    (
        143.7,
        "The player's tank is destroyed",
        down(Place::Corridor, 3),
    ),
    (
        145.2,
        "It comes back at its checkpoint",
        at(Place::Corridor),
    ),
    (
        152.6,
        "Sector 16: an ace (threat 4)",
        fight(Place::Corridor, 4),
    ),
    (
        166.4,
        "The menu opens over the game",
        paused(Place::Corridor, 4),
    ),
    (169.9, "The menu closes", fight(Place::Corridor, 4)),
    (
        178.3,
        "The tank is destroyed for the last time: game over",
        at(Place::Menu),
    ),
    (190.0, "The end of the tour", at(Place::Menu)),
];

const fn at(place: Place) -> Context {
    Context {
        place,
        fight: false,
        threat: 0,
        down: false,
        muffled: false,
    }
}

const fn fight(place: Place, threat: u8) -> Context {
    Context {
        fight: true,
        threat,
        ..at(place)
    }
}

const fn down(place: Place, threat: u8) -> Context {
    Context {
        down: true,
        ..fight(place, threat)
    }
}

const fn paused(place: Place, threat: u8) -> Context {
    Context {
        muffled: true,
        ..fight(place, threat)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let directory = std::env::args().nth(1).unwrap_or("target/music".into());
    let directory = PathBuf::from(directory);

    for (name, home) in [("dorian", Mode::Dorian), ("mixolydian", Mode::Mixolydian)] {
        let directory = directory.join(name);
        fs::create_dir_all(&directory)?;
        // Two turns of each loop, and the 16 bars of a fight.
        arrangement(home, at(Place::Bay), directory.join("intro.wav"))?;
        arrangement(home, at(Place::Corridor), directory.join("bridge.wav"))?;
        for threat in 0..=4 {
            let file = directory.join(format!("corridor_threat_{threat}.wav"));
            arrangement(home, fight(Place::Corridor, threat), file)?;
        }

        let (mut music, conductor) = music(home);
        let mut frames = Vec::new();
        for pair in TOUR.windows(2) {
            conductor.set(pair[0].2);
            play(&mut music, pair[1].0 - pair[0].0, &mut frames);
        }
        wav::write(directory.join("tour.wav"), &frames)?;
    }

    // An arena sounds the same whatever the mode of the piece.
    for threat in 0..=4 {
        let file = directory.join(format!("arena_threat_{threat}.wav"));
        arrangement(Mode::Dorian, fight(Place::Arena, threat), file)?;
    }
    let mut notes = String::new();
    for (start, label, _) in &TOUR[..TOUR.len() - 1] {
        let (minutes, seconds) = ((start / 60.0) as u32, start % 60.0);
        writeln!(notes, "{minutes}:{seconds:04.1}  {label}")?;
    }
    fs::write(directory.join("tour.txt"), notes)?;

    println!("Wrote the music to {}", directory.display());
    Ok(())
}

/// Writes 16 bars of the music of a game that stays in one context.
fn arrangement(home: Mode, context: Context, file: PathBuf) -> Result<(), Box<dyn Error>> {
    let (mut music, _conductor) = music_from(home, context);
    let mut frames = Vec::new();
    play(&mut music, 16.0 * BAR, &mut frames);
    Ok(wav::write(file, &frames)?)
}

/// Adds some seconds of music to what was played so far.
fn play(music: &mut Music, seconds: f32, frames: &mut Vec<f32>) {
    let start = frames.len();
    let samples = (seconds * SAMPLE_RATE as f32).round() as usize * CHANNELS as usize;
    frames.resize(start + samples, 0.0);
    music.render(&mut frames[start..]);
}
