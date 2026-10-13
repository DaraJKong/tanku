//! The sound cues. The crate `tanku_audio` computes their waveforms when the game starts, so
//! the game ships without sound files. This module plays them: it knows where a sound comes
//! from and how loud it is.

use std::time::Duration;

use bevy::{audio::Volume, prelude::*};
use tanku_audio::{
    cues::{self, Cue},
    wav,
};
use tanku_core::sim::{Bullet, Run, SimEvent, Team};

use crate::{
    camera::View,
    music::{self, DuckedUntil},
};

/// Present when the game must stay silent, whatever the volume is set to.
#[derive(Resource)]
pub struct Muted;

/// Distance at which a sound is at its quietest.
const HEARING_RANGE: f32 = 50.0;
/// Seconds between the last enemy of a sector and the cue that tells the end of the fight, at
/// the least: the cue answers the blast, and is no part of it.
const ENDING_PAUSE: f32 = 0.35;

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
    app.add_systems(Startup, setup)
        .add_systems(Update, (play.before(music::tell), tell_end, click));
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
    cues: Res<Cues>,
    view: Res<View>,
    time: Res<Time<Real>>,
    mut ducked: ResMut<DuckedUntil>,
    run: Option<Res<Run>>,
) {
    // A sector of the corridor wakes with a sound. A group of arena mode is announced by its
    // own.
    let wakes = run.is_some_and(|run| !run.arena);
    let mut appeared = false;
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
        start(&mut commands, &cues, cue, nearness.clamp(0.25, 1.0));
    }
}

/// Tells the end of a fight, once it is over: when the last enemy of a sector is destroyed,
/// their bullets may still fly, and one of them can still destroy a tank. The cue waits until
/// none of them is left in flight, as the view does before it runs ahead of a tank at a
/// barrier, and a moment after the last blast at the least: that of the last tank, or that
/// of a shell or of a mine that goes off after it.
///
/// Each machine of a game tells it from what it shows, so a guest may hear it a moment
/// before or after its host.
fn tell_end(
    mut commands: Commands,
    mut events: MessageReader<SimEvent>,
    // The sector whose end is still to tell, with the seconds that the cue waits at the
    // least, and whether a single player is without a tank.
    (mut ending, mut down): (Local<Option<(u32, f32)>>, Local<bool>),
    cues: Res<Cues>,
    time: Res<Time>,
    run: Option<Res<Run>>,
    bullets: Query<&Bullet>,
) {
    let solo = run.as_ref().is_none_or(|run| run.solo);
    let (mut cleared, mut dropped, mut blasted) = (None, false, false);
    for event in events.read() {
        match *event {
            // The start bay is cleared when a run starts, and no fight ends there.
            SimEvent::SectorCleared { sector } if sector > 0 => cleared = Some(sector),
            SimEvent::Blast { .. } => blasted = true,
            // An end that was not told yet is told no more once the next fight starts, or
            // once the run is over.
            SimEvent::SectorEntered { .. } | SimEvent::RunOver => dropped = true,
            SimEvent::RunStarted => (dropped, *down) = (true, false),
            // A single player whose tank is lost hears of that, and of the tank that comes
            // back. No end is told meanwhile: not the one that was still to tell when a
            // bullet that still flew took the tank, and not the one of a fight that a mine
            // of that tank ends after it.
            SimEvent::TankDestroyed {
                team: Team::Players,
                ..
            } if solo => *down = true,
            SimEvent::PlayerRespawned { .. } => *down = false,
            _ => {}
        }
    }
    if dropped || *down {
        *ending = None;
    } else if let Some(sector) = cleared {
        *ending = Some((sector, ENDING_PAUSE));
    } else if let Some((_, wait)) = ending.as_mut().filter(|_| blasted) {
        // The last shell of the enemies goes off, or a mine: the cue answers that blast as
        // it answers the one of the last tank.
        *wait = ENDING_PAUSE;
    }
    // While the host of a game holds the run, nothing moves, and nothing ends.
    let held = run.is_some_and(|run| run.paused);
    let Some((sector, wait)) = ending.as_mut().filter(|_| !held) else {
        return;
    };
    *wait -= time.delta_secs();
    let mut shots = bullets.iter().filter(|bullet| bullet.team == Team::Enemies);
    if *wait > 0.0 || shots.any(|bullet| bullet.sector == *sector) {
        return;
    }
    start(&mut commands, &cues, Cue::Cleared, 1.0);
    *ending = None;
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
    use tanku_core::sim::{Kind, RunPhase};

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
            .init_resource::<Time>()
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
            .add_systems(Startup, setup)
            .add_systems(Update, (play.before(music::tell), tell_end, music::tell));
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
        // What happens somewhere is at its loudest next to the middle of the screen, and a
        // quarter as loud far from it: never silent.
        for (event, cue) in sounds(near) {
            app.world_mut().write_message(event);
            app.update();
            assert_eq!(played(&mut app), [(cue, 0.9)], "{event:?}");
        }
        for (event, cue) in sounds(far) {
            app.world_mut().write_message(event);
            app.update();
            assert_eq!(played(&mut app), [(cue, 0.25)], "{event:?}");
        }
        // What happens to the run is as loud wherever the players are.
        let told = [
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
    fn the_end_of_a_fight_is_told_once_it_is_over() {
        let mut app = game();
        let frame = |app: &mut App, event: Option<SimEvent>, seconds: f32| {
            let time = Duration::from_secs_f32(seconds);
            app.world_mut().resource_mut::<Time>().advance_by(time);
            if let Some(event) = event {
                app.world_mut().write_message(event);
            }
            app.update();
            played(app)
        };
        let shot = |app: &mut App, team: Team, sector: u32| {
            let bullet = Bullet {
                owner: Entity::PLACEHOLDER,
                team,
                sector,
                velocity: Vec2::Y,
                bounces_left: 0,
                pierces_left: 0,
                bounced: false,
                kind: Kind::Plain,
            };
            app.world_mut().spawn(bullet).id()
        };
        let cleared = Some(SimEvent::SectorCleared { sector: 3 });
        let told = [(Cue::Cleared, 1.0)];

        // The last enemy of a sector is destroyed, and nothing of theirs flies: the cue
        // answers the blast a moment later, once, as loud wherever the players are.
        assert_eq!(frame(&mut app, cleared, 0.016), []);
        assert_eq!(frame(&mut app, None, 0.3), []);
        assert_eq!(frame(&mut app, None, 0.05), told);
        assert_eq!(frame(&mut app, None, 1.0), []);

        // A bullet of those enemies still flies: the fight is not over, however long it
        // takes. The bullets of the players hold nothing, and neither do those of another
        // sector.
        let last = shot(&mut app, Team::Enemies, 3);
        shot(&mut app, Team::Players, 3);
        shot(&mut app, Team::Enemies, 4);
        assert_eq!(frame(&mut app, cleared, 0.016), []);
        assert_eq!(frame(&mut app, None, 3.0), []);
        app.world_mut().despawn(last);
        assert_eq!(frame(&mut app, None, 0.016), told);

        // The last of those bullets is a shell that goes off: the cue answers its blast, as
        // it answers the blast of the last tank, and is no part of it.
        let last = shot(&mut app, Team::Enemies, 3);
        assert_eq!(frame(&mut app, cleared, 0.016), []);
        assert_eq!(frame(&mut app, None, 2.0), []);
        app.world_mut().despawn(last);
        let shell = Some(SimEvent::Blast { at: Vec2::ZERO });
        assert_eq!(frame(&mut app, shell, 0.016), [(Cue::Blast, 1.0)]);
        assert_eq!(frame(&mut app, None, 0.3), []);
        assert_eq!(frame(&mut app, None, 0.06), told);

        // While the host of a game holds the run, nothing ends.
        assert_eq!(frame(&mut app, cleared, 0.016), []);
        app.world_mut().resource_mut::<Run>().paused = true;
        assert_eq!(frame(&mut app, None, 2.0), []);
        app.world_mut().resource_mut::<Run>().paused = false;
        assert_eq!(frame(&mut app, None, 0.3), []);
        assert_eq!(frame(&mut app, None, 0.05), told);

        // The players are through the next barrier while a bullet still flies behind them:
        // the next fight has started, and the end of the last one is told no more.
        let last = shot(&mut app, Team::Enemies, 3);
        assert_eq!(frame(&mut app, cleared, 0.016), []);
        let entered = Some(SimEvent::SectorEntered { sector: 4 });
        assert_eq!(frame(&mut app, entered, 0.5), [(Cue::Entered, 1.0)]);
        app.world_mut().despawn(last);
        assert_eq!(frame(&mut app, None, 1.0), []);

        // A single player whose tank is lost to what still flew hears no end either, in
        // whichever order the two are told. Among several players, the others do.
        let lost = SimEvent::TankDestroyed {
            at: Vec2::ZERO,
            team: Team::Players,
        };
        assert_eq!(frame(&mut app, cleared, 0.016), []);
        assert_eq!(frame(&mut app, Some(lost), 0.1), [(Cue::Blast, 1.0)]);
        assert_eq!(frame(&mut app, None, 1.0), []);
        app.world_mut().write_message(lost);
        assert_eq!(frame(&mut app, cleared, 1.0), [(Cue::Blast, 1.0)]);
        assert_eq!(frame(&mut app, None, 1.0), []);
        // The same holds for a fight that ends while that player has no tank, as when a
        // mine of theirs takes the last enemy after them: the tank that comes back is what
        // they hear of.
        assert_eq!(frame(&mut app, cleared, 0.2), []);
        let back = Some(SimEvent::PlayerRespawned { at: Vec2::ZERO });
        assert_eq!(frame(&mut app, back, 0.3), [(Cue::Respawned, 1.0)]);
        assert_eq!(frame(&mut app, None, 1.0), []);
        // With their tank back, the next end is told as any other.
        assert_eq!(frame(&mut app, cleared, 0.016), []);
        assert_eq!(frame(&mut app, None, 0.4), told);
        app.world_mut().resource_mut::<Run>().solo = false;
        assert_eq!(frame(&mut app, cleared, 0.016), []);
        assert_eq!(frame(&mut app, Some(lost), 0.1), [(Cue::Blast, 1.0)]);
        assert_eq!(frame(&mut app, None, 0.3), told);

        // A run that ends tells its own end.
        assert_eq!(frame(&mut app, cleared, 0.016), []);
        assert_eq!(
            frame(&mut app, Some(SimEvent::RunOver), 0.1),
            [(Cue::RunOver, 1.0)]
        );
        assert_eq!(frame(&mut app, None, 1.0), []);
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
