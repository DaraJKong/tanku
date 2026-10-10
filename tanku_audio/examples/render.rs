//! Writes the music to sound files, to listen to it without the game:
//!
//! ```bash
//! cargo run -p tanku_audio --example render
//! ```
//!
//! The files go to `target/music`, or to the directory given after `--`:
//!
//! - `intro.wav` and `bridge.wav`: two turns of each loop;
//! - `corridor_threat_0.wav` to `corridor_threat_4.wav`, and the same for `arena`: the verse
//!   and the chorus of a fight at each threat;
//! - `tour.wav`: three minutes of a made-up run, and `tour.txt`, which tells what happens in
//!   it and when.

use std::{error::Error, fmt::Write, fs, path::PathBuf};

use tanku_audio::{CHANNELS, Context, Music, Place, SAMPLE_RATE, music, music_from, wav};

/// Seconds in a bar, and in a bar of the bridge, which is slower.
const BAR: f32 = 2.0;
const BRIDGE_BAR: f32 = 8.0 / 3.0;

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

    fs::create_dir_all(&directory)?;

    // Two turns of each loop, and the 16 bars of a fight.
    let file = |name: &str| directory.join(format!("{name}.wav"));
    arrangement(at(Place::Bay), 16.0 * BAR, file("intro"))?;
    arrangement(at(Place::Corridor), 16.0 * BRIDGE_BAR, file("bridge"))?;
    for (name, place) in [("corridor", Place::Corridor), ("arena", Place::Arena)] {
        for threat in 0..=4 {
            let file = file(&format!("{name}_threat_{threat}"));
            arrangement(fight(place, threat), 16.0 * BAR, file)?;
        }
    }

    let (mut music, conductor) = music();
    let mut frames = Vec::new();
    for pair in TOUR.windows(2) {
        conductor.set(pair[0].2);
        play(&mut music, pair[1].0 - pair[0].0, &mut frames);
    }
    wav::write(file("tour"), &frames)?;

    let mut notes = String::new();
    for (start, label, _) in &TOUR[..TOUR.len() - 1] {
        let (minutes, seconds) = ((start / 60.0) as u32, start % 60.0);
        writeln!(notes, "{minutes}:{seconds:04.1}  {label}")?;
    }
    fs::write(directory.join("tour.txt"), notes)?;

    println!("Wrote the music to {}", directory.display());
    Ok(())
}

/// Writes some seconds of the music of a game that stays in one context.
fn arrangement(context: Context, seconds: f32, file: PathBuf) -> Result<(), Box<dyn Error>> {
    let (mut music, _conductor) = music_from(context);
    let mut frames = Vec::new();
    play(&mut music, seconds, &mut frames);
    Ok(wav::write(file, &frames)?)
}

/// Adds some seconds of music to what was played so far.
fn play(music: &mut Music, seconds: f32, frames: &mut Vec<f32>) {
    let start = frames.len();
    let samples = (seconds * SAMPLE_RATE as f32).round() as usize * CHANNELS as usize;
    frames.resize(start + samples, 0.0);
    music.render(&mut frames[start..]);
}
