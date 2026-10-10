//! The music. The crate `tanku_audio` computes it while it plays. This module hands it to the
//! sound library and tells it, every frame, what happens in the game.

use std::time::Duration;

use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source, Volume},
    prelude::*,
};
use tanku_audio::{CHANNELS, Conductor, Context, Music, Place, SAMPLE_RATE};
use tanku_core::{
    map::SectorKind,
    sim::{Enemy, Run, RunPhase, Sector},
};

use crate::{Overlay, Screen, audio::Muted, settings::Settings};

/// How loud the music is against the cues with its volume all the way up. Halfway up, where
/// the setting starts, the loudest arrangement peaks 12 dB below a shot fired next to the
/// player.
const LOUDEST: f32 = 0.54;
/// The threat of every fight for now: the lead plays the motif as it is written. The brains
/// of the enemies that are awake will set it.
const THREAT: u8 = 2;
/// Samples that the music computes at a time: 3 ms of it. The sound library reads them one
/// by one.
const BLOCK: usize = 128 * CHANNELS as usize;

/// The end of the music that the game holds, to tell it what happens.
#[derive(Resource, Default)]
pub struct Baton(pub Conductor);

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
        .init_resource::<Baton>()
        .add_systems(Startup, start)
        .add_systems(Update, (tell, set_volume));
}

/// How loud the music is, under the volume of the game.
fn loudness(settings: &Settings) -> f32 {
    settings.music * LOUDEST
}

/// Starts the music, which never stops from then on. A game that must stay silent has none.
fn start(
    mut commands: Commands,
    mut pieces: ResMut<Assets<Piece>>,
    baton: Res<Baton>,
    settings: Res<Settings>,
    muted: Option<Res<Muted>>,
) {
    // Not on the web yet. A browser mixes the sound between two frames of the game, so a long
    // frame leaves a gap, and nobody has listened for gaps there.
    if muted.is_some() || cfg!(target_arch = "wasm32") {
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

/// What the music is told about the game: where the run has got to, given the kind of its
/// sector if that sector is loaded, and whether an enemy is awake.
fn context(
    screen: Screen,
    overlay: Overlay,
    run: Option<&Run>,
    kind: Option<SectorKind>,
    awake: bool,
) -> Context {
    // The main menu and the game over screen. A run that is over is no place either: its
    // last moments are still on the screen, and so is the start of the run that follows it.
    let on = |run: &&Run| screen == Screen::Playing && run.phase != RunPhase::Over;
    let Some(run) = run.filter(on) else {
        return Context::default();
    };
    Context {
        place: match kind {
            Some(SectorKind::Bay) => Place::Bay,
            Some(SectorKind::Corridor) => Place::Corridor,
            Some(SectorKind::Arena) => Place::Arena,
            // The sector isn't loaded yet.
            None if run.sector == 0 => Place::Bay,
            None => Place::Corridor,
        },
        fight: awake,
        threat: THREAT,
        down: run.phase != RunPhase::Playing,
        // The settings leave the music clear, so that its volume is set by ear.
        muffled: overlay == Overlay::Pause,
    }
}

/// Tells the music what happens in the game.
fn tell(
    baton: Res<Baton>,
    screen: Res<State<Screen>>,
    overlay: Res<State<Overlay>>,
    run: Option<Res<Run>>,
    sectors: Query<&Sector>,
    enemies: Query<&Enemy>,
) {
    let run = run.as_deref();
    let reached = run.map_or(0, |run| run.sector);
    let mut plans = sectors.iter().map(|sector| &sector.layout.plan);
    let kind = plans
        .find(|plan| plan.index == reached)
        .map(|plan| plan.kind);
    let awake = enemies.iter().any(|enemy| enemy.awake);
    baton
        .0
        .set(context(*screen.get(), *overlay.get(), run, kind, awake));
}

#[cfg(test)]
mod tests {
    use bevy::audio::AudioPlugin;
    use tanku_core::map::Corridor;

    use super::*;

    fn run(sector: u32, phase: RunPhase) -> Run {
        Run {
            seed_text: "TEST".into(),
            seed: 1,
            sector,
            cleared: 0,
            checkpoint: 0,
            lives: 3,
            kills: 0,
            ticks: 0,
            phase,
            lost_to: None,
        }
    }

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
        .add_plugins(plugin);
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

    #[test]
    fn the_music_is_twelve_decibels_under_a_shot() {
        // The loudest arrangement, for the verse and the chorus of each mode.
        let mut peak = 0.0f32;
        for (place, threat) in [(Place::Arena, 1), (Place::Arena, 4), (Place::Corridor, 4)] {
            let (mut music, _) = tanku_audio::music_from(Context {
                place,
                fight: true,
                threat,
                ..default()
            });
            let mut frames = vec![0.0; 32 * SAMPLE_RATE as usize * CHANNELS as usize];
            music.render(&mut frames);
            peak = frames
                .iter()
                .fold(peak, |peak, sample| peak.max(sample.abs()));
        }
        let shot = crate::audio::loudest_shot();
        let settings = Settings::default();
        // The volume of the game sets both, so it doesn't count.
        let under = 20.0 * (shot / (peak * loudness(&settings))).log10();
        assert!((11.5..12.5).contains(&under), "{under} dB");
    }

    #[test]
    fn the_music_is_told_where_the_run_is() {
        let told = |screen, run: &Run, kind| context(screen, Overlay::None, Some(run), kind, false);
        let playing = run(3, RunPhase::Playing);
        // No run, or a screen that isn't the game: the menu.
        let menu = context(Screen::Menu, Overlay::None, None, None, false);
        assert_eq!(menu, Context::default());
        assert_eq!(menu.place, Place::Menu);
        assert_eq!(told(Screen::Menu, &playing, Some(SectorKind::Arena)), menu);
        assert_eq!(told(Screen::GameOver, &playing, None), menu);

        let place = |kind| told(Screen::Playing, &playing, kind).place;
        assert_eq!(place(Some(SectorKind::Bay)), Place::Bay);
        assert_eq!(place(Some(SectorKind::Corridor)), Place::Corridor);
        assert_eq!(place(Some(SectorKind::Arena)), Place::Arena);
        // Until the sector is loaded, the number of the sector tells the bay from the rest.
        assert_eq!(place(None), Place::Corridor);
        let bay = run(0, RunPhase::Playing);
        assert_eq!(told(Screen::Playing, &bay, None).place, Place::Bay);
    }

    #[test]
    fn the_music_is_told_of_fights_and_of_losses() {
        let corridor = Some(SectorKind::Corridor);
        let told = |overlay, run: &Run, awake| {
            context(Screen::Playing, overlay, Some(run), corridor, awake)
        };
        let playing = run(3, RunPhase::Playing);
        let calm = told(Overlay::None, &playing, false);
        assert!(!calm.fight && !calm.down && !calm.muffled);
        let fight = told(Overlay::None, &playing, true);
        assert!(fight.fight && !fight.down);
        assert_eq!(fight.threat, THREAT);

        // A tank that is on its way back is down, and the enemies are still awake.
        let back_soon = told(Overlay::None, &run(3, RunPhase::Respawning(1.0)), true);
        assert!(back_soon.down && back_soon.fight);
        assert_eq!(back_soon.place, Place::Corridor);
        // A run that is over has left its place, though the game is still on the screen.
        let over = told(Overlay::None, &run(3, RunPhase::Over), true);
        assert_eq!(over, Context::default());

        // The pause menu muffles the music, and the settings don't.
        assert!(told(Overlay::Pause, &playing, true).muffled);
        assert!(!told(Overlay::Settings, &playing, true).muffled);
        let settings_of_the_menu = context(Screen::Menu, Overlay::Settings, None, None, false);
        assert!(!settings_of_the_menu.muffled);
    }

    #[test]
    fn the_game_tells_the_music_every_frame() {
        let mut app = App::new();
        app.init_resource::<Baton>()
            .insert_resource(State::new(Screen::Playing))
            .insert_resource(State::new(Overlay::None))
            .add_systems(Update, tell);
        let baton = app.world().resource::<Baton>().0.clone();
        app.update();
        // The screen changes a moment before the run is there.
        assert_eq!(baton.context().place, Place::Menu);

        // The bay, the first sector and the first arena of a seed are loaded.
        let mut corridor = Corridor::new(1);
        let arena = (1..).find(|&index| corridor.plan(index).kind == SectorKind::Arena);
        let arena = arena.unwrap();
        for index in [0, 1, arena] {
            let layout = corridor.layout(index);
            app.world_mut().spawn(Sector { layout });
        }
        let asleep = app.world_mut().spawn(Enemy { awake: false }).id();
        app.insert_resource(run(0, RunPhase::Playing));
        app.update();
        assert_eq!(baton.context().place, Place::Bay);
        assert!(!baton.context().fight);

        // The player crosses into the first sector, whose enemy wakes up.
        app.world_mut().resource_mut::<Run>().sector = 1;
        app.world_mut()
            .entity_mut(asleep)
            .insert(Enemy { awake: true });
        app.update();
        let context = baton.context();
        assert_eq!((context.place, context.fight), (Place::Corridor, true));
        // It is destroyed.
        app.world_mut().entity_mut(asleep).despawn();
        app.update();
        assert_eq!(
            (baton.context().place, baton.context().fight),
            (Place::Corridor, false)
        );

        app.world_mut().resource_mut::<Run>().sector = arena;
        app.update();
        assert_eq!(baton.context().place, Place::Arena);
    }
}
