//! The music on a sound device. The sound library of the engine reads the piece one sample at a
//! time, on a thread of its own, and mixes it with the cues.

use std::time::Duration;

use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source, Volume},
    prelude::*,
};
use tanku_audio::{CHANNELS, Conductor, Music, SAMPLE_RATE};

use super::{Baton, loudness};
use crate::{audio::Muted, settings::Settings};

/// Samples that the music computes at a time: 3 ms of it. The sound library reads them one
/// by one.
const BLOCK: usize = 128 * CHANNELS as usize;

/// The piece, as something that the sound library can play.
#[derive(Asset, TypePath)]
struct Piece(Conductor);

impl Decodable for Piece {
    type Decoder = Stream;

    fn decoder(&self) -> Stream {
        Stream::new(self.0.music())
    }
}

/// The music as the sound library reads it: one sample at a time, without an end.
struct Stream {
    music: Music,
    block: [f32; BLOCK],
    /// The sample of the block that comes next.
    next: usize,
}

impl Stream {
    fn new(music: Music) -> Self {
        Self {
            music,
            block: [0.0; BLOCK],
            // Nothing is computed before it is asked for.
            next: BLOCK,
        }
    }
}

impl Iterator for Stream {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.next == BLOCK {
            self.music.render(&mut self.block);
            self.next = 0;
        }
        let sample = self.block[self.next];
        self.next += 1;
        Some(sample)
    }
}

impl Source for Stream {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(CHANNELS).unwrap()
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub fn plugin(app: &mut App) {
    app.add_audio_source::<Piece>()
        .add_systems(Startup, start)
        .add_systems(Update, set_volume);
}

/// Starts the music, which never stops from then on. A game that must stay silent has none.
fn start(
    mut commands: Commands,
    mut pieces: ResMut<Assets<Piece>>,
    baton: Res<Baton>,
    settings: Res<Settings>,
    muted: Option<Res<Muted>>,
) {
    if muted.is_some() {
        return;
    }
    commands.spawn((
        AudioPlayer(pieces.add(Piece(baton.0.clone()))),
        // The music has no end, so it is played once and not in a loop, which would keep
        // every sample of it.
        PlaybackSettings::ONCE.with_volume(Volume::Linear(loudness(&settings))),
    ));
}

/// Keeps the music as loud as the settings say. The volume of the game reaches a sound only
/// when it starts, and the music starts once.
fn set_volume(settings: Res<Settings>, mut sinks: Query<&mut AudioSink, With<AudioPlayer<Piece>>>) {
    let wanted = Volume::Linear(settings.volume * loudness(&settings));
    for mut sink in &mut sinks {
        if sink.volume() != wanted {
            sink.set_volume(wanted);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::audio::AudioPlugin;
    use tanku_audio::{Context, Place};

    use super::*;
    use crate::{Overlay, Screen};

    #[test]
    fn the_sound_library_reads_the_music_as_it_is() {
        let fight = Context {
            place: Place::Arena,
            fight: true,
            threat: 4,
            ..default()
        };
        let (mut music, conductor) = tanku_audio::music_from(fight);
        // Three seconds, which don't end on a block.
        let mut frames = vec![0.0; 3 * SAMPLE_RATE as usize * CHANNELS as usize + 14];
        music.render(&mut frames);

        let (music, _) = tanku_audio::music_from(conductor.context());
        let mut stream = Stream::new(music);
        assert_eq!(stream.channels().get(), 2);
        assert_eq!(stream.sample_rate().get(), 44_100);
        // It has no end, and nothing tells the library to look at it again.
        assert_eq!(stream.current_span_len(), None);
        assert_eq!(stream.total_duration(), None);
        let read: Vec<f32> = stream.by_ref().take(frames.len()).collect();
        assert!(read.iter().any(|sample| *sample != 0.0));
        assert_eq!(read, frames);
    }

    #[test]
    fn the_piece_starts_from_its_first_bar_when_it_is_played() {
        // The game has told the music where it is before the sound library asks for it.
        let piece = Piece(Conductor::default());
        let (mut music, _) = tanku_audio::music();
        let mut frames = vec![0.0; 2 * BLOCK];
        music.render(&mut frames);
        let read: Vec<f32> = piece.decoder().take(frames.len()).collect();
        assert_eq!(read, frames);
    }

    /// Plays the piece on the sound device of the machine for half a second, with the volume
    /// of the game at nothing:
    /// `cargo test -p tanku_client reaches_the_sound_device -- --ignored`
    #[test]
    #[ignore = "needs a sound device"]
    fn the_music_reaches_the_sound_device() {
        let silent = Volume::Linear(0.0);
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AudioPlugin {
                global_volume: silent.into(),
                ..default()
            },
        ))
        .init_resource::<Settings>()
        .insert_resource(State::new(Screen::Menu))
        .insert_resource(State::new(Overlay::None))
        .add_plugins(super::super::plugin);
        app.world_mut().resource_mut::<Settings>().volume = 0.0;
        for _ in 0..25 {
            app.update();
            std::thread::sleep(Duration::from_millis(20));
        }
        let world = app.world_mut();
        let mut sinks = world.query_filtered::<&AudioSink, With<AudioPlayer<Piece>>>();
        let sink = sinks.single(world).expect("the piece is not playing");
        assert_eq!(sink.volume(), silent);
        // The device has asked for the first moments of the piece, and goes on asking.
        let played = sink.position();
        assert!(played > Duration::from_millis(100), "{played:?}");
        assert!(!sink.empty() && !sink.is_paused());
    }
}
