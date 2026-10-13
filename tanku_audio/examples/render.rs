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
//!   it and when;
//! - `connections.wav`: eight short fights that each end with the lead on another note, to
//!   hear every way from a fight into the bridge, and `connections.txt`, which tells them
//!   apart;
//! - `cues.wav`: every sound cue alone, then over the fullest music, as loud as the game plays
//!   both with the settings it starts with, and `cues.txt`, which tells them apart.

use std::{
    error::Error,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

use tanku_audio::{
    CHANNELS, Context, MUSIC_LEVEL, Music, Place, SAMPLE_RATE,
    cues::{self, Cue},
    director::{Director, ROLL_HITS},
    music, music_from,
    score::{CONNECTION_STEPS, STEPS},
    synth::Synth,
    wav,
};

/// Seconds in a bar, and in a bar of the bridge, which is slower.
const BAR: f32 = 2.0;
const BRIDGE_BAR: f32 = 16.0 / 7.0;

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

/// The fights of the file of connections: where each is, its threat, how many steps it lasts
/// once its roll is over, and the note its lead is on when it ends. Each fight opens on the
/// next phrase of the piece, and none lasts longer than its phrase.
const CONNECTIONS: [(Place, u8, usize, &str); 8] = [
    (
        Place::Corridor,
        2,
        40,
        "The lead is on C: it rises, C, D, E",
    ),
    (
        Place::Corridor,
        2,
        24,
        "The lead is on F sharp: one step down, a beat each",
    ),
    (
        Place::Corridor,
        2,
        40,
        "The lead is on the B above the upper A: the fall, A, G, F sharp, E",
    ),
    (Place::Corridor, 2, 24, "The lead is on E: it stays there"),
    (
        Place::Corridor,
        2,
        36,
        "The lead is on B: it rises, B, C, D, E",
    ),
    (
        Place::Corridor,
        1,
        36,
        "The lead is on the lower A: it rises from the B above it",
    ),
    (
        Place::Corridor,
        0,
        40,
        "The lead is silent: the fall, on the pluck and the bass",
    ),
    (
        Place::Arena,
        2,
        48,
        "An arena, where the lead is on F: F sharp, then E",
    ),
];

/// The settings that the game starts with: its volume, and the volume of its music.
const VOLUME: f32 = 0.7;
const MUSIC_VOLUME: f32 = 0.5;
/// How loud the game plays a cue that sounds as far away as a cue gets.
const FAR: f32 = 0.25;
/// Seconds of the file of cues before its music starts.
const ALONE: f32 = 26.0;
/// The music of the file of cues: the fullest there is.
const FULLEST: Context = fight(Place::Arena, 4);

/// The cues that sound where something happens, which the game plays quieter the further that
/// is from the middle of the screen. The others are as loud wherever the players are.
const PLACED: [Cue; 10] = [
    Cue::Shot,
    Cue::EnemyShot,
    Cue::Ricochet,
    Cue::Pop,
    Cue::MinePlaced,
    Cue::MineTriggered,
    Cue::Blast,
    Cue::ShieldRaised,
    Cue::ShieldBlocked,
    Cue::ShieldDropped,
];

/// What a cue tells.
fn told(cue: Cue) -> &'static str {
    match cue {
        Cue::Shot => "A player fires",
        Cue::EnemyShot => "An enemy fires",
        Cue::Ricochet => "A bullet bounces",
        Cue::Pop => "A bullet is destroyed",
        Cue::MinePlaced => "A mine is placed",
        Cue::MineTriggered => "The warning of a mine",
        Cue::Blast => "A blast",
        Cue::ShieldRaised => "A shield is raised",
        Cue::ShieldBlocked => "A shield destroys a bullet",
        Cue::ShieldDropped => "The bubble of a shield is gone",
        Cue::Cleared => "A sector is cleared",
        Cue::LifeTaken => "A life is taken",
        Cue::Unlocked => "A barrier unlocks",
        Cue::Respawned => "The player's tank is back",
        Cue::RunOver => "The run is over",
        Cue::Click => "A button is pressed",
        Cue::Wave => "A wave of an arena is on its way",
        Cue::Appeared => "The tanks of a wave are there",
        Cue::Entered => "A sector is entered, in the experiment of a playtest",
    }
}

const fn at(place: Place) -> Context {
    Context {
        place,
        fight: false,
        threat: 0,
        down: false,
        muffled: false,
        ducked: false,
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
    connections(&directory)?;
    cues(&directory)?;

    println!("Wrote the music to {}", directory.display());
    Ok(())
}

/// Writes the file of connections. The game is played step by step here, so that each fight
/// ends on the very note it should.
fn connections(directory: &Path) -> Result<(), Box<dyn Error>> {
    let mut director = Director::settled(at(Place::Corridor));
    let mut synth = Synth::default();
    let (mut frames, mut notes) = (Vec::new(), String::new());
    // Where the next step starts, in half frames.
    let mut next = 0u64;
    let mut run = |context: Context, steps: usize, frames: &mut Vec<f32>| {
        for _ in 0..steps {
            let step = director.step(context);
            synth.play(&step, next);
            next += step.length as u64;
            while (frames.len() as u64) < next / 2 * CHANNELS as u64 {
                frames.extend(synth.frame());
            }
        }
    };

    run(at(Place::Corridor), STEPS, &mut frames);
    for (place, threat, fought, what) in CONNECTIONS {
        run(
            fight(place, threat),
            ROLL_HITS as usize + fought,
            &mut frames,
        );
        let start = (frames.len() / CHANNELS as usize) as f32 / SAMPLE_RATE as f32;
        writeln!(
            notes,
            "{}:{:04.1}  {what}",
            (start / 60.0) as u32,
            start % 60.0
        )?;
        // The connection, then two bars of the bridge.
        run(
            at(place),
            CONNECTION_STEPS as usize + 2 * STEPS,
            &mut frames,
        );
    }
    wav::write(directory.join("connections.wav"), &frames)?;
    Ok(fs::write(directory.join("connections.txt"), notes)?)
}

/// Writes the file of cues: each one alone, then over the fullest music, next to the player
/// and as far away as a cue gets. Everything is as loud as the game plays it when it starts.
fn cues(directory: &Path) -> Result<(), Box<dyn Error>> {
    // When each cue starts, and how loud the game plays it.
    let mut hits: Vec<(f32, Cue, f32)> = Vec::new();
    let mut notes = String::new();
    let mut hit = |notes: &mut String, at: f32, cue: Cue, level: f32, more: &str| {
        hits.push((at, cue, level));
        let (minutes, seconds) = ((at / 60.0) as u32, at % 60.0);
        writeln!(notes, "{minutes}:{seconds:04.1}  {}{more}", told(cue))
    };

    writeln!(notes, "Each cue alone:")?;
    for (number, cue) in Cue::ALL.into_iter().enumerate() {
        hit(&mut notes, 1.0 + 1.5 * number as f32, cue, 1.0, "")?;
    }
    // Over the music, a cue starts on the second beat of a bar, where the kick, the snare and
    // the hi-hat hit together: the music is at its fullest there.
    writeln!(
        notes,
        "\nOver the fight of an arena at the highest threat, next to the player:"
    )?;
    let near = |number: usize| 4.5 + 2.0 * number as f32;
    for (number, cue) in Cue::ALL.into_iter().enumerate() {
        let ducks = cue == Cue::MineTriggered;
        let more = if ducks { ": the music makes room" } else { "" };
        hit(&mut notes, ALONE + near(number), cue, 1.0, more)?;
    }
    writeln!(
        notes,
        "\nThe cues that sound where something happens, as far away as a cue gets:"
    )?;
    let far = |number: usize| near(Cue::ALL.len() + number);
    for (number, cue) in PLACED.into_iter().enumerate() {
        let stays = cue == Cue::MineTriggered;
        let more = if stays {
            ": out of hearing range, so the music stays as it is"
        } else {
            ""
        };
        hit(&mut notes, ALONE + far(number), cue, FAR, more)?;
    }

    // The music, which makes room for the warning that sounds next to the player.
    let (mut music, conductor) = music_from(FULLEST);
    let mut played = Vec::new();
    let warning = near(Cue::MineTriggered as usize);
    play(&mut music, warning, &mut played);
    conductor.set(Context {
        ducked: true,
        ..FULLEST
    });
    play(&mut music, Cue::MineTriggered.seconds(), &mut played);
    conductor.set(FULLEST);
    let rest = far(PLACED.len()) + 1.5 - warning - Cue::MineTriggered.seconds();
    play(&mut music, rest, &mut played);

    let frames_in = |seconds: f32| (seconds * SAMPLE_RATE as f32).round() as usize;
    let mut frames = vec![0.0; frames_in(ALONE) * CHANNELS as usize];
    frames.extend(
        played
            .iter()
            .map(|level| level * MUSIC_VOLUME * MUSIC_LEVEL),
    );
    for (at, cue, level) in hits {
        // A cue has half the frames of the music, and is in the middle of the stereo.
        assert_eq!(SAMPLE_RATE, 2 * cues::SAMPLE_RATE);
        let samples = cue.samples();
        let start = frames_in(at);
        for (index, &sample) in samples.iter().enumerate() {
            let next = samples.get(index + 1).copied().unwrap_or(0.0);
            for (half, heard) in [sample, (sample + next) / 2.0].into_iter().enumerate() {
                let frame = (start + 2 * index + half) * CHANNELS as usize;
                frames[frame] += level * heard;
                frames[frame + 1] += level * heard;
            }
        }
    }
    for frame in &mut frames {
        *frame *= VOLUME;
    }
    wav::write(directory.join("cues.wav"), &frames)?;
    Ok(fs::write(directory.join("cues.txt"), notes)?)
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
