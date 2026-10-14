//! The editor of the music of Tanku: it opens `music.score`, shows it as motifs, patterns,
//! sections and instruments, plays it as the game does, and saves it.
//!
//! ```bash
//! cargo run -p music_editor
//! ```
//!
//! The file of the game is the one it opens, unless another is named after `--`.

mod actions;
mod audition;
mod checks;
mod dev;
mod doc;
#[cfg(test)]
mod fixture;
mod lines;
mod names;
mod parts;
mod plot;
mod sound;
mod state;
mod theme;
mod ui;

use std::{path::PathBuf, sync::Mutex};

use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source, Volume},
    prelude::*,
};
use tanku_audio::{CHANNELS, MUSIC_LEVEL, SAMPLE_RATE};

use crate::{doc::Doc, sound::Stream, state::View};

/// The file of the game, which the editor opens when it is given no other.
const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tanku_audio/music.score");

/// The channels and the rate of the music as the sound library takes them: numbers that are
/// not zero. They are worked out while the editor is compiled, so a zero would stop the build.
const CHANNEL_COUNT: ChannelCount = ChannelCount::new(CHANNELS).unwrap();
const RATE: SampleRate = SampleRate::new(SAMPLE_RATE).unwrap();

const USAGE: &str = "\
Usage: music_editor [FILE]

Opens a piece of music of Tanku. Without a file, it opens the one of the game,
tanku_audio/music.score.

  -h, --help  Show this help.";

/// The sound of the editor, for the sound library to read. It is read once: the editor has
/// one sound, which never ends.
#[derive(Asset, TypePath)]
struct Feed(Mutex<Option<Stream>>);

impl Decodable for Feed {
    type Decoder = Reader;

    fn decoder(&self) -> Reader {
        Reader(self.0.lock().ok().and_then(|mut stream| stream.take()))
    }
}

/// The stream as the sound library reads it: without an end, at the rate of the music.
struct Reader(Option<Stream>);

impl Iterator for Reader {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.0.as_mut()?.next()
    }
}

impl Source for Reader {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        CHANNEL_COUNT
    }

    fn sample_rate(&self) -> SampleRate {
        RATE
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// The sound, until the sound library has it.
#[derive(Resource)]
struct Fed(Handle<Feed>);

/// Starts the sound, which plays nothing until the editor asks.
fn start_the_sound(mut commands: Commands, fed: Res<Fed>) {
    commands.spawn((
        AudioPlayer(fed.0.clone()),
        // As loud as the game plays its music with that setting all the way up. The sound
        // has no end, so it is played once and not in a loop.
        PlaybackSettings::ONCE.with_volume(Volume::Linear(MUSIC_LEVEL)),
    ));
}

fn main() {
    let mut path = None;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            other if other.starts_with('-') => {
                eprintln!("unknown option: {other}\n\n{USAGE}");
                std::process::exit(2);
            }
            file => path = Some(PathBuf::from(file)),
        }
    }
    let scripted = dev::scripted();
    // A script only saves into a file that it was given.
    let named = path.is_some();
    let doc = match Doc::open(path.unwrap_or_else(|| PathBuf::from(FILE))) {
        Ok(mut doc) => {
            if scripted && !named {
                doc.path = None;
            }
            doc
        }
        Err(why) => {
            eprintln!("{why}");
            std::process::exit(1);
        }
    };

    let mut app = App::new();
    if scripted {
        dev::add_engine(&mut app);
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Tanku music editor".into(),
                resolution: (ui::SHEET.x as u32, ui::SHEET.y as u32).into(),
                ..default()
            }),
            ..default()
        }));
    }
    let (engine, stream) = sound::engine();
    app.insert_resource(View::of(doc.piece()))
        .insert_resource(doc)
        .insert_resource(engine)
        .add_plugins((theme::plugin, ui::plugin));
    if scripted {
        dev::plugin(&mut app, stream);
    } else {
        app.add_audio_source::<Feed>();
        let feeds = &mut app.world_mut().resource_mut::<Assets<Feed>>();
        let fed = Fed(feeds.add(Feed(Mutex::new(Some(stream)))));
        app.insert_resource(fed)
            .add_systems(Startup, start_the_sound);
    }
    app.run();
}
