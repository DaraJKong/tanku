//! The sound cues. The crate `tanku_audio` computes their waveforms when the game starts, so
//! the game ships without sound files. This module plays them: it knows where a sound comes
//! from and how loud it is.

use std::time::Duration;

use bevy::{audio::Volume, prelude::*};
use tanku_audio::{
    cues::{self, Cue},
    wav,
};
use tanku_core::sim::{Run, SimEvent, Team};

use crate::{
    camera::View,
    music::{self, DuckedUntil},
};

/// Present when the game must stay silent, whatever the volume is set to.
#[derive(Resource)]
pub struct Muted;

/// Where the cues of a frame are played: what asks for one comes before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Playing;

/// Asks for a cue that no event of the simulation plays by itself, at full volume: what the
/// display tells of the run, such as the loss of a teammate.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sound(pub Cue);

/// Distance at which a sound is at its quietest.
const HEARING_RANGE: f32 = 50.0;
/// How fast what still sounds of a blast falls, each second: as the blast itself dies away.
const BLAST_FALL: f32 = 7.0;
/// The share of a blast under which one more adds nothing that is heard.
const BLAST_FAINT: f32 = 0.2;

/// What still sounds of the blasts that went off: how loud the last one was made, with what
/// was left of those before it, and when.
#[derive(Default)]
struct Rumble {
    level: f32,
    at: Duration,
}

/// Every cue as a sound that is ready to play, in the order of the cues.
#[derive(Resource)]
struct Cues(Vec<Handle<AudioSource>>);

impl Cues {
    fn sound(&self, cue: Cue) -> Handle<AudioSource> {
        self.0[cue as usize].clone()
    }
}

pub fn plugin(app: &mut App) {
    // The music hears of a cue in the frame that plays it.
    app.add_message::<Sound>()
        .add_systems(Startup, setup)
        .add_systems(Update, (play.in_set(Playing).before(music::tell), click));
}

/// Starts a cue, this loud. A game that logs what is for debugging lists the cues as they
/// play, with `RUST_LOG=tanku_client::audio=debug`: it is how to know which ones a run made.
fn start(commands: &mut Commands, cues: &Cues, cue: Cue, volume: f32) {
    debug!("cue {cue:?} at {volume:.2}");
    commands.spawn((
        AudioPlayer::new(cues.sound(cue)),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
    ));
}

/// Computes the cues, each as a WAV file in memory.
fn setup(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let sound = |cue: Cue| AudioSource {
        bytes: wav::encode_as(&cue.samples(), 1, cues::SAMPLE_RATE).into(),
    };
    let sounds = Cue::ALL.map(|cue| sources.add(sound(cue)));
    commands.insert_resource(Cues(sounds.to_vec()));
}

fn play(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    mut asked: MessageReader<Sound>,
    cues: Res<Cues>,
    view: Res<View>,
    time: Res<Time<Real>>,
    mut ducked: ResMut<DuckedUntil>,
    run: Option<Res<Run>>,
    mut rumble: Local<Rumble>,
) {
    // A sector of the corridor wakes with a sound. A group of arena mode is announced by its
    // own.
    let wakes = run.is_some_and(|run| !run.arena);
    let (mut appeared, mut blast) = (false, None::<f32>);
    for event in events.read() {
        let (cue, at) = match *event {
            SimEvent::Shot {
                at,
                team: Team::Players,
            } => (Cue::Shot, Some(at)),
            SimEvent::Shot {
                at,
                team: Team::Enemies,
            } => (Cue::EnemyShot, Some(at)),
            // A bullet that splits sounds as one that bounces: it leaves another way.
            SimEvent::Ricochet { at } | SimEvent::BulletSplit { at } => (Cue::Ricochet, Some(at)),
            SimEvent::BulletDestroyed { at, .. } => (Cue::Pop, Some(at)),
            SimEvent::MinePlaced { at } => (Cue::MinePlaced, Some(at)),
            SimEvent::MineTriggered { at } => (Cue::MineTriggered, Some(at)),
            SimEvent::Blast { at } | SimEvent::TankDestroyed { at, .. } => (Cue::Blast, Some(at)),
            SimEvent::ShieldRaised { at } => (Cue::ShieldRaised, Some(at)),
            SimEvent::ShieldBlocked { at } => (Cue::ShieldBlocked, Some(at)),
            SimEvent::ShieldDropped { at } => (Cue::ShieldDropped, Some(at)),
            // The end of a fight is told with its last enemy. Bullets that still fly are no
            // fight: a cue that waited for them came seconds late, as an answer to nothing.
            SimEvent::SectorCleared { sector } if sector > 0 => (Cue::Cleared, None),
            SimEvent::LifeTaken { .. } => (Cue::LifeTaken, None),
            SimEvent::BarrierUnlocked { sector } if sector > 0 => (Cue::Unlocked, None),
            SimEvent::SectorEntered { .. } if wakes => (Cue::Entered, None),
            // A tank that comes back sounds the same, whatever brought it back.
            SimEvent::PlayerRespawned { .. } | SimEvent::TankRepaired { .. } => {
                (Cue::Respawned, None)
            }
            SimEvent::RunOver => (Cue::RunOver, None),
            SimEvent::WaveIncoming { .. } => (Cue::Wave, None),
            // The tanks of a wave appear together, and sound as one.
            SimEvent::TankAppeared { at, .. } if !appeared => {
                appeared = true;
                (Cue::Appeared, Some(at))
            }
            _ => continue,
        };
        // What happens far from the middle of the screen is quieter, but never silent: sound
        // tells what the eyes can miss.
        let nearness = at.map_or(1.0, |at| 1.0 - at.distance(view.focus()) / HEARING_RANGE);
        // The music makes room for the warning of a mine that is within hearing range, for
        // as long as the warning sounds.
        if cue == Cue::MineTriggered && nearness > 0.0 {
            ducked.0 = time.elapsed() + Duration::from_secs_f32(cue.seconds());
        }
        let volume = nearness.clamp(0.25, 1.0);
        if cue == Cue::Blast {
            blast = Some(blast.map_or(volume, |loudest| loudest.max(volume)));
        } else {
            start(&mut commands, &cues, cue, volume);
        }
    }
    for Sound(cue) in asked.read() {
        start(&mut commands, &cues, *cue, 1.0);
    }
    // Blasts that go off together sound as one, as loud as the nearest of them. A mine that
    // takes a tank with it is a blast and a tank that is destroyed, and a sound that is
    // started twice at once is twice as loud: three of them are past what a loudspeaker
    // takes. One that follows while another still sounds adds what that one has lost, and
    // no more, so however many go off, they are never louder than one.
    if let Some(wanted) = blast {
        let now = time.elapsed();
        let since = now.saturating_sub(rumble.at).as_secs_f32();
        // Nothing is left of a blast that has ended.
        let left = if since < Cue::Blast.seconds() {
            rumble.level * (-BLAST_FALL * since).exp()
        } else {
            0.0
        };
        if wanted - left > BLAST_FAINT * wanted {
            start(&mut commands, &cues, Cue::Blast, wanted - left);
            *rumble = Rumble {
                level: wanted,
                at: now,
            };
        }
    }
}

fn click(
    mut commands: Commands,
    buttons: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
    cues: Res<Cues>,
) {
    if buttons
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        start(&mut commands, &cues, Cue::Click, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use bevy::audio::Source;
    use tanku_core::sim::RunPhase;

    use super::*;
    use crate::{Overlay, Screen, music::Baton};

    /// A game in the middle of a run, which plays cues and tells the music. The middle of the
    /// screen is the point (0, 0) of the map.
    fn game() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<AudioSource>>()
            .init_resource::<View>()
            .init_resource::<Baton>()
            .init_resource::<DuckedUntil>()
            .init_resource::<Time<Real>>()
            .insert_resource(State::new(Screen::Playing))
            .insert_resource(State::new(Overlay::None))
            .insert_resource(Run {
                seed_text: "TEST".into(),
                seed: 1,
                difficulty: default(),
                players: 1,
                team: 1,
                solo: true,
                sector: 1,
                cleared: 0,
                checkpoint: 0,
                lives: 3,
                kills: 0,
                ticks: 0,
                phase: RunPhase::Playing,
                lost_to: None,
                playtest: false,
                earns: true,
                free: false,
                arena: false,
                lull: None,
                paused: false,
            })
            .add_message::<SimEvent>()
            .add_message::<Sound>()
            .add_systems(Startup, setup)
            .add_systems(Update, (play.before(music::tell), music::tell));
        app.update();
        app
    }

    /// The cues that the game started to play since the last time it was asked, each with
    /// how loud.
    fn played(app: &mut App) -> Vec<(Cue, f32)> {
        let world = app.world_mut();
        let mut players = world.query::<(Entity, &AudioPlayer, &PlaybackSettings)>();
        let found: Vec<_> = players
            .iter(world)
            .map(|(entity, player, settings)| (entity, player.0.clone(), settings.volume))
            .collect();
        let cues = world.resource::<Cues>();
        let name = |sound: &Handle<AudioSource>| {
            let mut all = Cue::ALL.into_iter();
            all.find(|cue| cues.sound(*cue) == *sound).unwrap()
        };
        let heard = found
            .iter()
            .map(|(_, sound, volume)| (name(sound), volume.to_linear()));
        let heard = heard.collect();
        for (entity, ..) in found {
            world.despawn(entity);
        }
        heard
    }

    #[test]
    fn every_event_that_has_a_sound_plays_its_cue() {
        let mut app = game();
        // Next to the middle of the screen, and further than a sound is heard from.
        let (near, far) = (Vec2::new(3.0, 4.0), Vec2::new(0.0, 80.0));
        let (players, enemies) = (Team::Players, Team::Enemies);
        let sounds = |at: Vec2| {
            [
                (SimEvent::Shot { at, team: players }, Cue::Shot),
                (SimEvent::Shot { at, team: enemies }, Cue::EnemyShot),
                (SimEvent::Ricochet { at }, Cue::Ricochet),
                (SimEvent::BulletDestroyed { at, team: enemies }, Cue::Pop),
                (SimEvent::MinePlaced { at }, Cue::MinePlaced),
                (SimEvent::MineTriggered { at }, Cue::MineTriggered),
                (SimEvent::Blast { at }, Cue::Blast),
                (SimEvent::TankDestroyed { at, team: enemies }, Cue::Blast),
                (SimEvent::ShieldRaised { at }, Cue::ShieldRaised),
                (SimEvent::ShieldBlocked { at }, Cue::ShieldBlocked),
                (SimEvent::ShieldDropped { at }, Cue::ShieldDropped),
            ]
        };
        // Each of them is alone: a second after the one before it.
        let alone = |app: &mut App, event: SimEvent| {
            let mut clock = app.world_mut().resource_mut::<Time<Real>>();
            clock.update_with_duration(Duration::from_secs(1));
            app.world_mut().write_message(event);
            app.update();
            played(app)
        };
        // What happens somewhere is at its loudest next to the middle of the screen, and a
        // quarter as loud far from it: never silent.
        for (event, cue) in sounds(near) {
            assert_eq!(alone(&mut app, event), [(cue, 0.9)], "{event:?}");
        }
        for (event, cue) in sounds(far) {
            assert_eq!(alone(&mut app, event), [(cue, 0.25)], "{event:?}");
        }
        // What happens to the run is as loud wherever the players are.
        let told = [
            (SimEvent::SectorCleared { sector: 3 }, Cue::Cleared),
            (SimEvent::LifeTaken { at: far }, Cue::LifeTaken),
            (SimEvent::BarrierUnlocked { sector: 3 }, Cue::Unlocked),
            (SimEvent::PlayerRespawned { at: far }, Cue::Respawned),
            (SimEvent::TankRepaired { at: far }, Cue::Respawned),
            (SimEvent::RunOver, Cue::RunOver),
        ];
        for (event, cue) in told {
            app.world_mut().write_message(event);
            app.update();
            assert_eq!(played(&mut app), [(cue, 1.0)], "{event:?}");
        }
        // These have no sound: the start bay is cleared and open from the start, and a block
        // that is destroyed has the blast that destroyed it.
        let silent = [
            SimEvent::RunStarted,
            SimEvent::SectorCleared { sector: 0 },
            SimEvent::BarrierUnlocked { sector: 0 },
            SimEvent::BlockDestroyed { at: near },
        ];
        for event in silent {
            app.world_mut().write_message(event);
            app.update();
            assert_eq!(played(&mut app), [], "{event:?}");
        }
    }

    #[test]
    fn a_cue_that_is_asked_for_plays_at_full_volume() {
        let mut app = game();
        for cue in [Cue::Last, Cue::Stamp] {
            app.world_mut().write_message(Sound(cue));
            app.update();
            assert_eq!(played(&mut app), [(cue, 1.0)], "{cue:?}");
        }
    }

    #[test]
    fn a_sector_of_the_corridor_wakes_with_a_sound() {
        let mut app = game();
        let entered = SimEvent::SectorEntered { sector: 3 };
        let heard = |app: &mut App| {
            app.world_mut().write_message(entered);
            app.update();
            played(app)
        };
        // The cue is as loud wherever the players are.
        assert_eq!(heard(&mut app), [(Cue::Entered, 1.0)]);
        // A group of arena mode comes with a sound of its own.
        app.world_mut().resource_mut::<Run>().arena = true;
        assert_eq!(heard(&mut app), []);
    }

    #[test]
    fn blasts_that_go_off_together_sound_as_one() {
        let mut app = game();
        // What happens in a frame, this long after the frame before it.
        let frame = |app: &mut App, events: &[SimEvent], seconds: f32| {
            let mut clock = app.world_mut().resource_mut::<Time<Real>>();
            clock.update_with_duration(Duration::from_secs_f32(seconds));
            for event in events {
                app.world_mut().write_message(*event);
            }
            app.update();
            played(app)
        };
        let (near, far) = (Vec2::new(3.0, 4.0), Vec2::new(0.0, 80.0));
        let lost = |at: Vec2| SimEvent::TankDestroyed {
            at,
            team: Team::Enemies,
        };
        let (mine, tank) = (SimEvent::Blast { at: near }, lost(near));
        let (shell, wreck) = (SimEvent::Blast { at: far }, lost(far));
        let one = |volume: f32| vec![(Cue::Blast, volume)];

        // A mine takes a tank with it, or two: one blast is heard, as loud as one alone, and
        // not two or three on top of each other.
        assert_eq!(frame(&mut app, &[mine, tank], 1.0), one(0.9));
        assert_eq!(frame(&mut app, &[mine, tank, tank], 1.0), one(0.9));
        // It is as loud as the nearest of them, whichever is told first.
        assert_eq!(frame(&mut app, &[shell, mine, wreck], 1.0), one(0.9));
        assert_eq!(frame(&mut app, &[shell, wreck], 1.0), one(0.25));

        // A blast in the next frame, as when a mine sets off the crate next to it, is heard
        // in the one that sounds, and so is one far away while that one is louder than it.
        assert_eq!(frame(&mut app, &[mine], 1.0), one(0.9));
        assert_eq!(frame(&mut app, &[mine, tank], 1.0 / 60.0), []);
        assert_eq!(frame(&mut app, &[shell], 0.05), []);

        // One that follows a tenth of a second later adds what the first has lost by then,
        // which is half of it: the two are never louder than one.
        assert_eq!(frame(&mut app, &[mine], 1.0), one(0.9));
        let more = frame(&mut app, &[mine], 0.1);
        assert!(matches!(more[..], [(Cue::Blast, more)] if (more - 0.453).abs() < 0.005));
        // And a blast that comes once the last has ended is alone.
        assert_eq!(frame(&mut app, &[tank], 0.8), one(0.9));

        // The other cues are as they were: two shots in a frame are two shots.
        let shot = SimEvent::Shot {
            at: near,
            team: Team::Enemies,
        };
        let shots = frame(&mut app, &[shot, shot, mine], 1.0);
        assert_eq!(shots.len(), 3);
        assert_eq!(
            shots
                .iter()
                .filter(|(cue, _)| *cue == Cue::EnemyShot)
                .count(),
            2
        );
    }

    #[test]
    fn the_end_of_a_fight_is_told_with_its_last_enemy() {
        let mut app = game();
        // The last enemy of a sector is destroyed: the cue comes in that frame, with the
        // blast, and once. It waits for nothing, whatever still flies in the sector.
        let last = SimEvent::TankDestroyed {
            at: Vec2::new(3.0, 4.0),
            team: Team::Enemies,
        };
        app.world_mut().write_message(last);
        app.world_mut()
            .write_message(SimEvent::SectorCleared { sector: 3 });
        app.update();
        let heard = played(&mut app);
        assert_eq!(heard.len(), 2);
        assert!(heard.contains(&(Cue::Blast, 0.9)) && heard.contains(&(Cue::Cleared, 1.0)));
        app.update();
        assert_eq!(played(&mut app), []);
    }

    #[test]
    fn the_music_makes_room_for_the_warning_of_a_mine() {
        let mut app = game();
        let ducked = |app: &App| app.world().resource::<Baton>().0.context().ducked;
        let frame = |app: &mut App, event: Option<SimEvent>, seconds: f32| {
            let time = Duration::from_secs_f32(seconds);
            let mut clock = app.world_mut().resource_mut::<Time<Real>>();
            clock.update_with_duration(time);
            if let Some(event) = event {
                app.world_mut().write_message(event);
            }
            app.update();
        };
        frame(&mut app, None, 0.016);
        assert!(!ducked(&app));

        // Another cue leaves the music alone, and so does a warning too far away to matter.
        let near = Vec2::new(3.0, 4.0);
        frame(&mut app, Some(SimEvent::MinePlaced { at: near }), 0.016);
        assert!(!ducked(&app));
        let far = Vec2::new(0.0, HEARING_RANGE + 1.0);
        frame(&mut app, Some(SimEvent::MineTriggered { at: far }), 0.016);
        assert!(!ducked(&app));

        // A warning within hearing range ducks the music in the frame that plays it, for as
        // long as it sounds.
        frame(&mut app, Some(SimEvent::MineTriggered { at: near }), 0.016);
        assert!(ducked(&app));
        frame(&mut app, None, 0.5);
        assert!(ducked(&app));
        frame(&mut app, None, 0.2);
        assert!(!ducked(&app));
    }

    #[test]
    fn the_sound_library_reads_every_cue_back() {
        let mut app = App::new();
        app.init_resource::<Assets<AudioSource>>()
            .add_systems(Startup, setup);
        app.update();
        let cues = app.world().resource::<Cues>();
        let sources = app.world().resource::<Assets<AudioSource>>();
        for cue in Cue::ALL {
            let sound = sources.get(&cues.sound(cue)).unwrap().decoder();
            assert_eq!(sound.channels().get(), 1, "{cue:?}");
            assert_eq!(sound.sample_rate().get(), cues::SAMPLE_RATE, "{cue:?}");
            // What it reads is the cue, to a step of a file of 16 bits.
            let read: Vec<f32> = sound.collect();
            let computed = cue.samples();
            assert_eq!(read.len(), computed.len(), "{cue:?}");
            for (read, computed) in read.iter().zip(&computed) {
                assert!((read - computed).abs() < 2.0 / i16::MAX as f32, "{cue:?}");
            }
        }
    }
}
