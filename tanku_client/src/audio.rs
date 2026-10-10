//! The sound cues. Each one is a short waveform computed when the game starts, so the game
//! ships without sound files.

use std::f32::consts::TAU;

use bevy::{audio::Volume, prelude::*};
use tanku_core::sim::{SimEvent, Team};

use crate::camera::View;

/// Present when the game must stay silent, whatever the volume is set to.
#[derive(Resource)]
pub struct Muted;

const SAMPLE_RATE: u32 = 22_050;
/// Distance at which a sound is at its quietest.
const HEARING_RANGE: f32 = 50.0;

#[derive(Resource)]
struct Cues {
    shot: Handle<AudioSource>,
    enemy_shot: Handle<AudioSource>,
    ricochet: Handle<AudioSource>,
    pop: Handle<AudioSource>,
    mine_placed: Handle<AudioSource>,
    mine_triggered: Handle<AudioSource>,
    blast: Handle<AudioSource>,
    shield_raised: Handle<AudioSource>,
    shield_blocked: Handle<AudioSource>,
    shield_overloaded: Handle<AudioSource>,
    cleared: Handle<AudioSource>,
    captured: Handle<AudioSource>,
    unlocked: Handle<AudioSource>,
    respawned: Handle<AudioSource>,
    run_over: Handle<AudioSource>,
    click: Handle<AudioSource>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (play, click));
}

/// White noise from a small generator of its own, so that every build makes the same sounds.
struct Noise(u32);

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1 << 23) as f32 - 1.0
    }
}

/// Renders a waveform to a WAV file in memory: mono, 16 bits.
fn render(seconds: f32, wave: impl Fn(f32, &mut Noise) -> f32) -> AudioSource {
    let count = (seconds * SAMPLE_RATE as f32) as u32;
    let mut noise = Noise(12345);
    let data_size = count * 2;
    let mut bytes = Vec::with_capacity(44 + data_size as usize);
    bytes.extend(b"RIFF");
    bytes.extend((36 + data_size).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes()); // PCM
    bytes.extend(1u16.to_le_bytes()); // one channel
    bytes.extend(SAMPLE_RATE.to_le_bytes());
    bytes.extend((SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data_size.to_le_bytes());
    for index in 0..count {
        let t = index as f32 / SAMPLE_RATE as f32;
        // A short fade at the end avoids a click.
        let fade = ((seconds - t) / 0.01).min(1.0);
        let sample = (wave(t, &mut noise) * fade).clamp(-1.0, 1.0);
        bytes.extend(((sample * i16::MAX as f32) as i16).to_le_bytes());
    }
    AudioSource {
        bytes: bytes.into(),
    }
}

/// A sine whose pitch slides from one frequency to another over a duration.
fn sweep(t: f32, from: f32, to: f32, seconds: f32) -> f32 {
    (TAU * (from * t + (to - from) * t * t / (2.0 * seconds))).sin()
}

fn setup(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let mut cue =
        |seconds: f32, wave: &dyn Fn(f32, &mut Noise) -> f32| sources.add(render(seconds, wave));
    let decay = |t: f32, rate: f32| (-t * rate).exp();
    let note = |t: f32, start: f32, pitch: f32| {
        if t < start {
            0.0
        } else {
            (TAU * pitch * t).sin() * decay(t - start, 9.0)
        }
    };
    commands.insert_resource(Cues {
        shot: cue(0.16, &|t, noise| {
            0.5 * sweep(t, 420.0, 90.0, 0.16) * decay(t, 22.0) + 0.3 * noise.next() * decay(t, 60.0)
        }),
        enemy_shot: cue(0.18, &|t, noise| {
            0.5 * sweep(t, 300.0, 60.0, 0.18) * decay(t, 20.0) + 0.3 * noise.next() * decay(t, 50.0)
        }),
        ricochet: cue(0.2, &|t, _| {
            0.35 * sweep(t, 1900.0, 2900.0, 0.2) * decay(t, 24.0)
        }),
        pop: cue(0.07, &|t, noise| 0.3 * noise.next() * decay(t, 70.0)),
        mine_placed: cue(0.09, &|t, _| 0.4 * (TAU * 620.0 * t).sin() * decay(t, 40.0)),
        mine_triggered: cue(0.6, &|t, _| {
            if t % 0.1 < 0.05 {
                0.4 * (TAU * 1500.0 * t).sin()
            } else {
                0.0
            }
        }),
        blast: cue(0.8, &|t, noise| {
            let rumble = (TAU * 52.0 * t).sin() * decay(t, 7.0);
            0.6 * rumble + 0.5 * noise.next() * decay(t, 9.0)
        }),
        shield_raised: cue(0.25, &|t, _| {
            0.35 * sweep(t, 400.0, 1800.0, 0.25) * (1.0 - t / 0.25)
        }),
        shield_blocked: cue(0.16, &|t, _| {
            0.3 * ((TAU * 900.0 * t).sin() + (TAU * 1350.0 * t).sin()) * decay(t, 20.0)
        }),
        shield_overloaded: cue(0.45, &|t, noise| {
            (0.35 * sweep(t, 700.0, 120.0, 0.45) + 0.15 * noise.next()) * (1.0 - t / 0.45)
        }),
        cleared: cue(0.5, &|t, _| {
            0.3 * (note(t, 0.0, 660.0) + note(t, 0.14, 880.0))
        }),
        captured: cue(0.6, &|t, _| {
            0.3 * (note(t, 0.0, 660.0) + note(t, 0.1, 880.0) + note(t, 0.2, 1320.0))
        }),
        unlocked: cue(0.45, &|t, _| {
            0.3 * (note(t, 0.0, 880.0) + note(t, 0.12, 1320.0))
        }),
        respawned: cue(0.4, &|t, _| {
            0.3 * sweep(t, 300.0, 900.0, 0.4) * (1.0 - t / 0.4)
        }),
        run_over: cue(1.0, &|t, _| {
            0.3 * (note(t, 0.0, 440.0) + note(t, 0.25, 330.0) + note(t, 0.5, 220.0))
        }),
        click: cue(0.04, &|t, _| {
            0.3 * (TAU * 1000.0 * t).sin() * decay(t, 80.0)
        }),
    });
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
            } => (&cues.shot, Some(at)),
            SimEvent::Shot {
                at,
                team: Team::Enemies,
            } => (&cues.enemy_shot, Some(at)),
            SimEvent::Ricochet { at } => (&cues.ricochet, Some(at)),
            SimEvent::BulletDestroyed { at } => (&cues.pop, Some(at)),
            SimEvent::MinePlaced { at } => (&cues.mine_placed, Some(at)),
            SimEvent::MineTriggered { at } => (&cues.mine_triggered, Some(at)),
            SimEvent::Blast { at } | SimEvent::TankDestroyed { at, .. } => (&cues.blast, Some(at)),
            SimEvent::ShieldRaised { at } => (&cues.shield_raised, Some(at)),
            SimEvent::ShieldBlocked { at } => (&cues.shield_blocked, Some(at)),
            SimEvent::ShieldOverloaded { at } => (&cues.shield_overloaded, Some(at)),
            SimEvent::SectorCleared { sector } if sector > 0 => (&cues.cleared, None),
            SimEvent::FlagCaptured { .. } => (&cues.captured, None),
            SimEvent::BarrierUnlocked { sector } if sector > 0 => (&cues.unlocked, None),
            SimEvent::PlayerRespawned { .. } => (&cues.respawned, None),
            SimEvent::RunOver => (&cues.run_over, None),
            _ => continue,
        };
        // What happens far from the middle of the screen is quieter, but never silent: sound
        // tells what the eyes can miss.
        let nearness = at.map_or(1.0, |at| 1.0 - at.distance(view.focus()) / HEARING_RANGE);
        let volume = Volume::Linear(nearness.clamp(0.25, 1.0));
        commands.spawn((
            AudioPlayer::new(cue.clone()),
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
            AudioPlayer::new(cues.click.clone()),
            PlaybackSettings::DESPAWN,
        ));
    }
}
