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

const A2: u8 = A3 - 12;
const E3: u8 = A3 - 5;
const E4: u8 = A4 - 5;
const E5: u8 = A4 + 7;
const A5: u8 = A4 + 12;
const E6: u8 = A5 + 7;
const A6: u8 = A5 + 12;

/// Seconds between two notes of the fall of a loss.
const LOSS_STEP: f32 = 0.15;
/// When the knock of a stamp is heard: as the stamp lands on the sheet.
const STAMP_LANDS: f32 = 0.08;

/// Seconds that the glide of a sector that is entered takes to climb, before its clack.
const ENTERED_CLIMB: f32 = 0.3;
/// How loud the cue of a cleared sector is, next to its notes as they are written. It is as
/// loud as the two plucked notes that it took the place of, with the bass counted: its low A
/// carries further than its level tells.
const CLEARED_LEVEL: f32 = 0.75;
/// How loud the cue of a sector that is entered is, next to its glide and its clack as they
/// are written. It sounds at every crossing and tells the players what they have just done,
/// so it is one of the quietest cues: about as loud as a mine that is placed.
const ENTERED_LEVEL: f32 = 0.4;

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
    /// The bubble of a shield is gone.
    ShieldDropped,
    /// The last enemy of a sector is destroyed.
    Cleared,
    /// A single player takes a life from the floor.
    LifeTaken,
    /// A barrier unlocks.
    Unlocked,
    /// A player's tank is back.
    Respawned,
    RunOver,
    /// A button of a menu is pressed.
    Click,
    /// A wave of an arena is on its way.
    Wave,
    /// The tanks of a wave are there.
    Appeared,
    /// The first player is through a barrier, and the sector behind it wakes. An experiment
    /// of a playtest: the music marks that moment otherwise, with its roll.
    Entered,
    /// One more loss ends the run: a loss leaves the last tank that runs, or a single player
    /// their last life. The tonic, the fifth under it and the tonic under that, plucked so
    /// that they are heard over a blast. The end of a run falls the same way, an octave lower
    /// and slower.
    Last,
    /// A run has earned something: a knock as a stamp lands, then two notes that go on up from
    /// where the cue of a cleared sector stops.
    Stamp,
}

impl Cue {
    /// Every cue, in the order of their numbers.
    pub const ALL: [Cue; 21] = [
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
        Cue::Cleared,
        Cue::LifeTaken,
        Cue::Unlocked,
        Cue::Respawned,
        Cue::RunOver,
        Cue::Click,
        Cue::Wave,
        Cue::Appeared,
        Cue::Entered,
        Cue::Last,
        Cue::Stamp,
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
            Cue::ShieldDropped => 0.45,
            Cue::Cleared => 0.9,
            Cue::LifeTaken => 0.6,
            Cue::Unlocked => 0.45,
            Cue::Respawned => 0.4,
            Cue::RunOver => 1.0,
            Cue::Click => 0.04,
            Cue::Wave => 0.9,
            Cue::Appeared => 0.3,
            Cue::Entered => 0.52,
            Cue::Last => 0.95,
            Cue::Stamp => 1.4,
        }
    }

    /// The samples of the cue, each from -1 to 1.
    pub fn samples(self) -> Vec<f32> {
        let seconds = self.seconds();
        let count = (seconds * RATE) as u32;
        let mut noise = Hiss {
            noise: Noise(SEED),
            dulled: 0.0,
        };
        let sample = |index: u32| {
            let t = index as f32 / RATE;
            let fade = ((seconds - t) / FADE).min(1.0);
            (self.wave(t, &mut noise) * fade).clamp(-1.0, 1.0)
        };
        (0..count).map(sample).collect()
    }

    /// The level of the cue after `t` seconds, before it fades out.
    fn wave(self, t: f32, noise: &mut Hiss) -> f32 {
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
            Cue::ShieldDropped => (0.35 * glide(700.0, 120.0) + 0.15 * noise.next()) * left,
            // The end of a fight: an E then an A that ring on, over a low A and its octave
            // that hold for most of a second. Nothing of it is noise: the blast was.
            Cue::Cleared => {
                let ground = 0.30 * tone(t, frequency(A2)) * decay(t * 5.0)
                    + 0.14 * tone(t, frequency(A3)) * decay(t * 7.0);
                CLEARED_LEVEL * (ground + 0.30 * (ring(t, 0.06, E5, 7.0) + ring(t, 0.24, A5, 6.0)))
            }
            Cue::LifeTaken => 0.3 * (note(t, 0.0, E5) + note(t, 0.1, A5) + note(t, 0.2, E6)),
            Cue::Unlocked => 0.3 * (note(t, 0.0, A5) + note(t, 0.12, E6)),
            Cue::Respawned => 0.3 * glide(300.0, 900.0) * left,
            Cue::RunOver => 0.3 * (note(t, 0.0, A4) + note(t, 0.25, E4) + note(t, 0.5, A3)),
            Cue::Click => 0.3 * tone(t, 1000.0) * decay(t * 80.0),
            // A siren that climbs three times, between two notes like the warning of a mine.
            Cue::Wave => 0.3 * sweep(t % 0.3, 330.0, 760.0, 0.3) * (1.0 - (t % 0.3) / 0.6),
            Cue::Appeared => {
                0.45 * glide(240.0, 70.0) * decay(t * 12.0) + 0.25 * noise.next() * decay(t * 30.0)
            }
            // A sector powers up: a glide from the tonic up to its fifth, with its octave,
            // that swells under a dull rush of noise.
            Cue::Entered if t < ENTERED_CLIMB => {
                let swell = (t / ENTERED_CLIMB) * (t / ENTERED_CLIMB);
                let climb =
                    |from: u8, to: u8| sweep(t, frequency(from), frequency(to), ENTERED_CLIMB);
                let rush = 0.5 * noise.dull(0.25);
                ENTERED_LEVEL * swell * (0.34 * climb(A2, E4) + 0.14 * climb(A3, E5) + rush)
            }
            // Then a dry clack on that fifth, an octave lower, which dies away: the music is
            // left waiting for its tonic. Nothing of it is as high as the roll of the hi-hat.
            Cue::Entered => {
                let since = t - ENTERED_CLIMB;
                ENTERED_LEVEL
                    * (0.42 * tone(since, frequency(E3)) * decay(since * 16.0)
                        + 0.18 * tone(since, frequency(E4)) * decay(since * 24.0)
                        + 0.55 * noise.dull(0.5) * decay(since * 45.0))
            }
            Cue::Last => {
                let fall =
                    pluck(t, 0.0, A5) + pluck(t, LOSS_STEP, E5) + pluck(t, 2.0 * LOSS_STEP, A4);
                0.3 * fall + 0.22 * knock(t, 0.0, noise)
            }
            Cue::Stamp => {
                let notes = ring(t, 0.42, E6, 6.0) + ring(t, 0.6, A6, 5.0);
                0.5 * knock(t, STAMP_LANDS, noise) + 0.24 * notes
            }
        }
    }
}

/// A note that starts at some moment, with a third of its third harmonic, which makes it
/// hollow: it cuts through a fight better than a pure note does.
fn pluck(t: f32, start: f32, pitch: u8) -> f32 {
    if t < start {
        return 0.0;
    }
    let (since, hz) = (t - start, frequency(pitch));
    0.8 * (tone(since, hz) + 0.33 * tone(since, 3.0 * hz)) * decay(since * 9.0)
}

/// A dull knock on the fifth of the music, two octaves down, that starts at some moment. Its
/// noise is drawn at every sample, so that the knock is the same wherever it starts.
fn knock(t: f32, start: f32, noise: &mut Hiss) -> f32 {
    let rush = noise.dull(0.35);
    if t < start {
        return 0.0;
    }
    let since = t - start;
    tone(since, frequency(E3)) * decay(since * 26.0) + 0.8 * rush * decay(since * 45.0)
}

/// The noise of a cue, as it is or with its top taken off.
struct Hiss {
    noise: Noise,
    /// What the dull noise was at the last sample.
    dulled: f32,
}

impl Hiss {
    fn next(&mut self) -> f32 {
        self.noise.next()
    }

    /// Noise that follows the noise by this share at each sample: a small share leaves a
    /// dull rush, and a share of 1 leaves the noise as it is.
    fn dull(&mut self, share: f32) -> f32 {
        self.dulled += share * (self.noise.next() - self.dulled);
        self.dulled
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

/// A note of the music that starts at some moment, from the middle of its wave, and dies
/// away as fast as it is told to: it rings on where [`note`] is plucked.
fn ring(t: f32, start: f32, pitch: u8, rate: f32) -> f32 {
    if t < start {
        0.0
    } else {
        tone(t - start, frequency(pitch)) * decay((t - start) * rate)
    }
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
        let played: [(Cue, &[u8]); 8] = [
            (Cue::MinePlaced, &[E5]),
            (Cue::ShieldBlocked, &[A5, E6]),
            (Cue::Cleared, &[A2, E5, A5]),
            (Cue::LifeTaken, &[E5, A5, E6]),
            (Cue::Unlocked, &[A5, E6]),
            (Cue::RunOver, &[A3, E4, A4]),
            (Cue::Last, &[E3, A4, E5, A5]),
            (Cue::Stamp, &[E3, E6, A6]),
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
    fn the_cue_of_a_sector_that_is_entered_ends_on_the_fifth_of_the_music() {
        let samples = Cue::Entered.samples();
        // Once its glide is over, it holds two Es: the strongest note of each of its two
        // octaves.
        let clack = &samples[(ENTERED_CLIMB * RATE) as usize..];
        let strongest = |notes: std::ops::RangeInclusive<u8>| {
            let strength = |pitch: &u8| amount(clack, frequency(*pitch));
            notes.max_by(|a, b| strength(a).total_cmp(&strength(b)))
        };
        assert_eq!(strongest(A2..=A3), Some(E3));
        assert_eq!(strongest(A3 + 1..=A4), Some(E4));
        // It leaves the top to the roll of the hi-hat, which the music plays over it: what
        // is above 7 kHz is a small part of it.
        let power = |hz: f32| amount(&samples, hz).powi(2);
        let steps = (1..=220).map(|step| step as f32 * 50.0);
        let (high, all) = steps.fold((0.0, 0.0), |(high, all), hz| {
            let power = power(hz);
            (if hz > 7000.0 { high + power } else { high }, all + power)
        });
        assert!(high < 0.03 * all, "{}", high / all);
        let loudness = |samples: &[f32]| samples.iter().map(|s| (s * s) as f64).sum::<f64>();
        // It swells: its first tenth of a second is far quieter than its third.
        let tenth = (0.1 * RATE) as usize;
        assert!(loudness(&samples[..tenth]) * 20.0 < loudness(&samples[2 * tenth..3 * tenth]));
    }

    #[test]
    fn a_sector_that_is_entered_sounds_quieter_than_a_barrier_that_unlocks() {
        // How loud a cue is where it is loudest: its power over a twentieth of a second.
        let loudest = |cue: Cue| {
            let samples = cue.samples();
            let window = (0.05 * RATE) as usize;
            let power = |part: &[f32]| part.iter().map(|sample| sample * sample).sum::<f32>();
            let parts = samples.windows(window).step_by(window / 4);
            parts.map(power).fold(0.0, f32::max)
        };
        // It sounds at every crossing and tells nothing that the eyes miss, so it stays
        // under the two cues that it comes between: a barrier that unlocks, and a sector
        // that is cleared.
        let entered = loudest(Cue::Entered);
        assert!(entered > 0.0);
        for cue in [Cue::Unlocked, Cue::Cleared] {
            assert!(entered < loudest(cue), "{cue:?}");
        }
    }

    #[test]
    fn the_last_loss_falls_to_the_a_where_the_end_of_a_run_starts() {
        // The strongest of the notes of a fall in each step of it.
        let strongest = |cue: Cue, step: f32| {
            let samples = cue.samples();
            let part =
                &samples[(step * LOSS_STEP * RATE) as usize..][..(LOSS_STEP * RATE) as usize];
            let strength = |pitch: &u8| amount(part, frequency(*pitch));
            [A4, E5, A5]
                .into_iter()
                .max_by(|a, b| strength(a).total_cmp(&strength(b)))
        };
        // From the tonic to the fifth under it, then to the A under them, where the end of a
        // run starts its own fall.
        assert_eq!(strongest(Cue::Last, 0.0), Some(A5));
        assert_eq!(strongest(Cue::Last, 1.0), Some(E5));
        assert_eq!(strongest(Cue::Last, 2.0), Some(A4));
        // It is heard over a blast: most of it is above the blast's rumble, where the blast
        // has little.
        let power = |cue: Cue, from: f32, to: f32| {
            let samples = cue.samples();
            let steps = (1..=200).map(|step| step as f32 * 25.0);
            steps
                .filter(|hz| (from..to).contains(hz))
                .map(|hz| amount(&samples, hz).powi(2))
                .sum::<f64>()
        };
        let high = |cue: Cue| power(cue, 400.0, 5000.0) / power(cue, 0.0, 5000.0);
        assert!(high(Cue::Last) > 0.6, "{}", high(Cue::Last));
        assert!(high(Cue::Blast) < 0.2, "{}", high(Cue::Blast));
    }

    #[test]
    fn a_stamp_goes_on_up_from_where_the_cue_of_a_cleared_sector_stops() {
        // Both come in the frame of the last kill of a sector. The cleared sector rings an E
        // then an A, and the stamp knocks as it lands, then rings the E and the A above them,
        // as far apart: one figure of four notes that climbs.
        let strongest = |cue: Cue, from: f32, to: f32| {
            let samples = cue.samples();
            let part = &samples[(from * RATE) as usize..(to * RATE) as usize];
            let strength = |pitch: &u8| amount(part, frequency(*pitch));
            [E3, E5, A5, E6, A6]
                .into_iter()
                .max_by(|a, b| strength(a).total_cmp(&strength(b)))
        };
        assert_eq!(strongest(Cue::Cleared, 0.06, 0.24), Some(E5));
        assert_eq!(strongest(Cue::Cleared, 0.24, 0.42), Some(A5));
        assert_eq!(strongest(Cue::Stamp, STAMP_LANDS, 0.2), Some(E3));
        assert_eq!(strongest(Cue::Stamp, 0.42, 0.6), Some(E6));
        assert_eq!(strongest(Cue::Stamp, 0.6, 0.78), Some(A6));
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
