//! The music in a browser. The page hands it to the browser itself, a few pieces ahead of
//! time, so that a frame that takes long leaves no hole in it: see `ahead`.
//!
//! The cues stay with the sound library of the engine, which hands sound over one piece at a
//! time. A hole there is short, and a cue is shorter still.

use bevy::prelude::*;
use tanku_audio::{CHANNELS, DUCKED, Music, SAMPLE_RATE};
use web_sys::{
    AudioContext, AudioContextOptions, AudioContextState, GainNode, js_sys::Reflect,
    wasm_bindgen::JsValue,
};

use super::{
    Baton, DuckedUntil,
    ahead::{Ahead, PIECE},
    loudness,
};
use crate::settings::Settings;

const RATE: f64 = SAMPLE_RATE as f64;
/// Seconds in which the music gets most of the way to a lower loudness, and to a higher one:
/// it is ducked in 0.02 s and comes back in 0.2 s, as on a desktop.
const DOWN: f64 = 0.007;
const UP: f64 = 0.07;
/// A page that drew no frame for this many seconds wasn't shown.
const AWAY: f32 = 1.0;

/// The music as the browser plays it.
struct Played {
    /// A source of sound of its own, next to that of the cues.
    sound: AudioContext,
    /// How loud the music is, which is set where it is played: what is computed ahead of
    /// time would be turned down too late.
    level: GainNode,
    heard: f32,
    music: Music,
    ahead: Ahead,
    /// A piece of music as it is computed, the left and the right in turn, and one side of it.
    piece: Vec<f32>,
    side: Vec<f32>,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, start)
        .add_systems(Update, feed.after(super::tell));
}

/// Gets the music ready. The browser plays nothing before the first click or key, so the
/// piece starts then.
fn start(world: &mut World) {
    let music = world.resource::<Baton>().0.music();
    match prepare(music) {
        Ok(played) => world.insert_non_send(played),
        Err(error) => warn!("no music: {error:?}"),
    }
}

fn prepare(music: Music) -> Result<Played, JsValue> {
    let options = AudioContextOptions::new();
    options.set_sample_rate(SAMPLE_RATE as f32);
    let sound = AudioContext::new_with_context_options(&options)?;
    // The page that measures the sound tells the music from the cues by this name.
    Reflect::set(&sound, &"tanku".into(), &"music".into())?;
    Reflect::set(&sound, &"ahead".into(), &Ahead::default().reach().into())?;
    let level = sound.create_gain()?;
    level.gain().set_value(0.0);
    level.connect_with_audio_node(&sound.destination())?;
    Ok(Played {
        sound,
        level,
        heard: 0.0,
        music,
        ahead: Ahead::default(),
        piece: vec![0.0; PIECE as usize * CHANNELS as usize],
        side: vec![0.0; PIECE as usize],
    })
}

/// Hands the browser the music that is due, and keeps it as loud as the settings say.
fn feed(
    mut commands: Commands,
    played: Option<NonSendMut<Played>>,
    settings: Res<Settings>,
    ducked: Res<DuckedUntil>,
    time: Res<Time<Real>>,
) {
    let Some(mut played) = played else {
        return;
    };
    if played.sound.state() != AudioContextState::Running {
        return;
    }
    let ducked = time.elapsed() < ducked.0;
    let away = time.delta_secs() > AWAY;
    if let Err(error) = played.hand_over(&settings, ducked, away) {
        warn!("the music stopped: {error:?}");
        commands.queue(|world: &mut World| {
            world.remove_non_send::<Played>();
        });
    }
}

impl Played {
    fn hand_over(&mut self, settings: &Settings, ducked: bool, away: bool) -> Result<(), JsValue> {
        let now = self.sound.current_time();

        let wanted = settings.volume * loudness(settings) * if ducked { DUCKED } else { 1.0 };
        if wanted != self.heard {
            let pace = if wanted < self.heard { DOWN } else { UP };
            self.level.gain().set_target_at_time(wanted, now, pace)?;
            self.heard = wanted;
        }

        let reach = self.ahead.reach();
        let (first, pieces) = self.ahead.due((now * RATE).ceil() as u64, away);
        if self.ahead.reach() != reach {
            // For the page that measures the sound: how far ahead the music is kept.
            Reflect::set(&self.sound, &"ahead".into(), &self.ahead.reach().into())?;
        }
        for piece in 0..pieces as u64 {
            self.music.render(&mut self.piece);
            let buffer =
                self.sound
                    .create_buffer(CHANNELS as u32, PIECE as u32, SAMPLE_RATE as f32)?;
            let (frames, _) = self.piece.as_chunks::<{ CHANNELS as usize }>();
            for channel in 0..CHANNELS as usize {
                for (sample, frame) in self.side.iter_mut().zip(frames) {
                    *sample = frame[channel];
                }
                buffer.copy_to_channel(&self.side, channel as i32)?;
            }
            let source = self.sound.create_buffer_source()?;
            source.set_buffer(Some(&buffer));
            source.connect_with_audio_node(&self.level)?;
            source.start_with_when((first + piece * PIECE) as f64 / RATE)?;
        }
        Ok(())
    }
}
