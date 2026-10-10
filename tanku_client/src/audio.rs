//! The sound cues. The crate `tanku_audio` computes their waveforms when the game starts, so
//! the game ships without sound files. This module plays them: it knows where a sound comes
//! from and how loud it is.

use bevy::{audio::Volume, prelude::*};
use tanku_audio::{
    cues::{self, Cue},
    wav,
};
use tanku_core::sim::{SimEvent, Team};

use crate::camera::View;

/// Present when the game must stay silent, whatever the volume is set to.
#[derive(Resource)]
pub struct Muted;

/// Distance at which a sound is at its quietest.
const HEARING_RANGE: f32 = 50.0;

/// Every cue as a sound that is ready to play, in the order of the cues.
#[derive(Resource)]
struct Cues(Vec<Handle<AudioSource>>);

impl Cues {
    fn sound(&self, cue: Cue) -> Handle<AudioSource> {
        self.0[cue as usize].clone()
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (play, click));
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
) {
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
            SimEvent::Ricochet { at } => (Cue::Ricochet, Some(at)),
            SimEvent::BulletDestroyed { at, .. } => (Cue::Pop, Some(at)),
            SimEvent::MinePlaced { at } => (Cue::MinePlaced, Some(at)),
            SimEvent::MineTriggered { at } => (Cue::MineTriggered, Some(at)),
            SimEvent::Blast { at } | SimEvent::TankDestroyed { at, .. } => (Cue::Blast, Some(at)),
            SimEvent::ShieldRaised { at } => (Cue::ShieldRaised, Some(at)),
            SimEvent::ShieldBlocked { at } => (Cue::ShieldBlocked, Some(at)),
            SimEvent::ShieldOverloaded { at } => (Cue::ShieldOverloaded, Some(at)),
            SimEvent::SectorCleared { sector } if sector > 0 => (Cue::Cleared, None),
            SimEvent::FlagCaptured { .. } => (Cue::Captured, None),
            SimEvent::BarrierUnlocked { sector } if sector > 0 => (Cue::Unlocked, None),
            SimEvent::PlayerRespawned { .. } => (Cue::Respawned, None),
            SimEvent::RunOver => (Cue::RunOver, None),
            _ => continue,
        };
        // What happens far from the middle of the screen is quieter, but never silent: sound
        // tells what the eyes can miss.
        let nearness = at.map_or(1.0, |at| 1.0 - at.distance(view.focus()) / HEARING_RANGE);
        let volume = Volume::Linear(nearness.clamp(0.25, 1.0));
        commands.spawn((
            AudioPlayer::new(cues.sound(cue)),
            PlaybackSettings::DESPAWN.with_volume(volume),
        ));
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
        commands.spawn((
            AudioPlayer::new(cues.sound(Cue::Click)),
            PlaybackSettings::DESPAWN,
        ));
    }
}

#[cfg(test)]
mod tests {
    use bevy::audio::Source;

    use super::*;

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
