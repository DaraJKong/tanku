//! The sound cues: short waveforms that tell what happens in a fight and in a run. The game
//! computes each of them once, when it starts, so no sound file ships.
//!
//! They are made with the same math as the instruments, so a cue is the same samples on every
//! platform, and those that play notes take them from the table of the music: the tonic of
//! the piece and its fifth, A and E, which are in tune with whatever the music plays. Playing
//! the cues is left to the game, which knows where a sound comes from and how loud it is.

use crate::{
    math::{Noise, decay, frequency, sine},
    score::{A3, A4},
};

/// Samples of a cue per second. A cue is in mono.
pub const SAMPLE_RATE: u32 = 22_050;
const RATE: f32 = SAMPLE_RATE as f32;
/// Seconds over which a cue fades out at its end, which avoids a click.
const FADE: f32 = 0.01;
/// Where the noise of every cue starts.
const SEED: u32 = 12_345;

const E4: u8 = A4 - 5;
const E5: u8 = A4 + 7;
const A5: u8 = A4 + 12;
const E6: u8 = A5 + 7;

/// Something the game tells with a sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cue {
    /// A player fires.
    Shot,
    /// An enemy fires.
    EnemyShot,
    /// A bullet bounces.
    Ricochet,
    /// A bullet is destroyed.
    Pop,
    MinePlaced,
    /// The warning of a mine that is about to blow.
    MineTriggered,
    Blast,
    ShieldRaised,
    /// A shield destroys a bullet.
    ShieldBlocked,
    ShieldOverloaded,
    /// The last enemy of a sector is destroyed.
    Cleared,
    /// A flag is captured.
    Captured,
    /// A barrier unlocks.
    Unlocked,
    /// A player's tank is back.
    Respawned,
    RunOver,
    /// A button of a menu is pressed.
    Click,
}

impl Cue {
    /// Every cue, in the order of their numbers.
    pub const ALL: [Cue; 16] = [
        Cue::Shot,
        Cue::EnemyShot,
        Cue::Ricochet,
        Cue::Pop,
        Cue::MinePlaced,
        Cue::MineTriggered,
        Cue::Blast,
        Cue::ShieldRaised,
        Cue::ShieldBlocked,
        Cue::ShieldOverloaded,
        Cue::Cleared,
        Cue::Captured,
        Cue::Unlocked,
        Cue::Respawned,
        Cue::RunOver,
        Cue::Click,
    ];

    /// How long the cue lasts.
    pub fn seconds(self) -> f32 {
        match self {
            Cue::Shot => 0.16,
            Cue::EnemyShot => 0.18,
            Cue::Ricochet => 0.2,
            Cue::Pop => 0.07,
            Cue::MinePlaced => 0.09,
            Cue::MineTriggered => 0.6,
            Cue::Blast => 0.8,
            Cue::ShieldRaised => 0.25,
            Cue::ShieldBlocked => 0.16,
            Cue::ShieldOverloaded => 0.45,
            Cue::Cleared => 0.5,
            Cue::Captured => 0.6,
            Cue::Unlocked => 0.45,
            Cue::Respawned => 0.4,
            Cue::RunOver => 1.0,
            Cue::Click => 0.04,
        }
    }

    /// The samples of the cue, each from -1 to 1.
    pub fn samples(self) -> Vec<f32> {
        let seconds = self.seconds();
        let count = (seconds * RATE) as u32;
        let mut noise = Noise(SEED);
        let sample = |index: u32| {
            let t = index as f32 / RATE;
            let fade = ((seconds - t) / FADE).min(1.0);
            (self.wave(t, &mut noise) * fade).clamp(-1.0, 1.0)
        };
        (0..count).map(sample).collect()
    }

    /// The level of the cue after `t` seconds, before it fades out.
    fn wave(self, t: f32, noise: &mut Noise) -> f32 {
        // What is left of a line that falls to nothing over the length of the cue.
        let left = 1.0 - t / self.seconds();
        let glide = |from: f32, to: f32| sweep(t, from, to, self.seconds());
        match self {
            Cue::Shot => {
                0.5 * glide(420.0, 90.0) * decay(t * 22.0) + 0.3 * noise.next() * decay(t * 60.0)
            }
            Cue::EnemyShot => {
                0.5 * glide(300.0, 60.0) * decay(t * 20.0) + 0.3 * noise.next() * decay(t * 50.0)
            }
            Cue::Ricochet => 0.35 * glide(1900.0, 2900.0) * decay(t * 24.0),
            Cue::Pop => 0.3 * noise.next() * decay(t * 70.0),
            Cue::MinePlaced => 0.4 * tone(t, frequency(E5)) * decay(t * 40.0),
            // Beeps of a twentieth of a second, as far apart. Their pitch is between two notes
            // on purpose: a warning stands apart from the music.
            Cue::MineTriggered if t % 0.1 < 0.05 => 0.4 * tone(t, 1500.0),
            Cue::MineTriggered => 0.0,
            Cue::Blast => {
                0.6 * tone(t, 52.0) * decay(t * 7.0) + 0.5 * noise.next() * decay(t * 9.0)
            }
            Cue::ShieldRaised => 0.35 * glide(400.0, 1800.0) * left,
            Cue::ShieldBlocked => {
                0.3 * (tone(t, frequency(A5)) + tone(t, frequency(E6))) * decay(t * 20.0)
            }
            Cue::ShieldOverloaded => (0.35 * glide(700.0, 120.0) + 0.15 * noise.next()) * left,
            Cue::Cleared => 0.3 * (note(t, 0.0, E5) + note(t, 0.14, A5)),
            Cue::Captured => 0.3 * (note(t, 0.0, E5) + note(t, 0.1, A5) + note(t, 0.2, E6)),
            Cue::Unlocked => 0.3 * (note(t, 0.0, A5) + note(t, 0.12, E6)),
            Cue::Respawned => 0.3 * glide(300.0, 900.0) * left,
            Cue::RunOver => 0.3 * (note(t, 0.0, A4) + note(t, 0.25, E4) + note(t, 0.5, A3)),
            Cue::Click => 0.3 * tone(t, 1000.0) * decay(t * 80.0),
        }
    }
}

/// A sine wave of some frequency in Hz, after `t` seconds.
fn tone(t: f32, hz: f32) -> f32 {
    turn(hz * t)
}

/// A sine wave whose frequency slides from one value to another over some seconds.
fn sweep(t: f32, from: f32, to: f32, seconds: f32) -> f32 {
    turn(from * t + (to - from) * t * t / (2.0 * seconds))
}

/// The sine of an angle given in turns, of which there can be many.
fn turn(turns: f32) -> f32 {
    sine(turns - (turns as u32) as f32)
}

/// A note of the music that starts at some moment and dies away.
fn note(t: f32, start: f32, pitch: u8) -> f32 {
    if t < start {
        0.0
    } else {
        tone(t, frequency(pitch)) * decay((t - start) * 9.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_are_numbered_in_their_order() {
        for (number, cue) in Cue::ALL.into_iter().enumerate() {
            assert_eq!(cue as usize, number);
        }
    }

    #[test]
    fn a_cue_lasts_as_long_as_it_says_and_ends_without_a_click() {
        for cue in Cue::ALL {
            let samples = cue.samples();
            let seconds = samples.len() as f32 / RATE;
            assert!((seconds - cue.seconds()).abs() < 1.0 / RATE, "{cue:?}");
            assert!(samples.iter().all(|sample| sample.abs() <= 1.0), "{cue:?}");
            assert!(samples.iter().any(|sample| sample.abs() > 0.1), "{cue:?}");
            // The last sample is next to nothing.
            assert!(samples[samples.len() - 1].abs() < 0.01, "{cue:?}");
        }
    }

    /// How much of a frequency there is in a cue.
    fn amount(samples: &[f32], hz: f32) -> f64 {
        let turn = |index: usize| std::f64::consts::TAU * hz as f64 * index as f64 / RATE as f64;
        let part = |wave: fn(f64) -> f64| -> f64 {
            let samples = samples.iter().enumerate();
            samples
                .map(|(index, &sample)| sample as f64 * wave(turn(index)))
                .sum()
        };
        part(f64::sin).hypot(part(f64::cos))
    }

    #[test]
    fn cues_play_the_tonic_of_the_music_and_its_fifth() {
        let played: [(Cue, &[u8]); 6] = [
            (Cue::MinePlaced, &[E5]),
            (Cue::ShieldBlocked, &[A5, E6]),
            (Cue::Cleared, &[E5, A5]),
            (Cue::Captured, &[E5, A5, E6]),
            (Cue::Unlocked, &[A5, E6]),
            (Cue::RunOver, &[A3, E4, A4]),
        ];
        for (cue, notes) in played {
            // Of all the notes from two octaves below the A of 440 Hz to two above, the
            // strongest in the cue are the ones it is meant to play.
            let samples = cue.samples();
            let mut heard: Vec<u8> = (A4 - 24..=A4 + 24).collect();
            let strength = |pitch: &u8| amount(&samples, frequency(*pitch));
            heard.sort_by(|a, b| strength(b).total_cmp(&strength(a)));
            heard.truncate(notes.len());
            heard.sort();
            assert_eq!(heard, notes, "{cue:?}");
            // Each is an A or an E.
            assert!(notes.iter().all(|pitch| [9, 4].contains(&(pitch % 12))));
        }
    }

    #[test]
    fn the_warning_of_a_mine_is_between_two_notes() {
        let samples = Cue::MineTriggered.samples();
        // Its neighbors are an F sharp and the G above it.
        let (below, above) = (frequency(A4 + 21), frequency(A4 + 22));
        assert!(below < 1500.0 && 1500.0 < above);
        let own = amount(&samples, 1500.0);
        assert!(own > 2.0 * amount(&samples, below) && own > 2.0 * amount(&samples, above));
    }

    #[test]
    fn a_cue_is_the_same_every_time() {
        for cue in Cue::ALL {
            assert_eq!(cue.samples(), cue.samples(), "{cue:?}");
        }
    }
}
