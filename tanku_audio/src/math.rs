//! The few lines of math that every sound of the crate is made with.
//!
//! They add, subtract, multiply and divide 32-bit floats, which gives the same result on every
//! platform. A library function such as `sin` or `exp` would not: its last bit differs from
//! one platform to the next.

use std::f32::consts::TAU;

/// The sine of an angle given in turns, between 0 and 1.
pub(crate) const fn sine(turns: f32) -> f32 {
    // Folded onto a quarter turn either side of zero, where a short series is exact to a few
    // millionths.
    let folded = if turns < 0.25 {
        turns
    } else if turns < 0.75 {
        0.5 - turns
    } else {
        turns - 1.0
    };
    let x = folded * TAU;
    let x2 = x * x;
    x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0))))
}

/// The frequency of a pitch in Hz. Pitches count semitones, and 69 is the A of 440 Hz.
pub(crate) fn frequency(pitch: u8) -> f32 {
    // The 12 semitones from that A up. The other octaves are these doubled or halved, which
    // is exact.
    const OCTAVE: [f32; 12] = [
        440.0, 466.163_76, 493.883_3, 523.251_16, 554.365_23, 587.329_5, 622.253_97, 659.255_1,
        698.456_5, 739.988_83, 783.990_84, 830.609_4,
    ];
    let above = pitch as i32 - 69;
    let octave = above.div_euclid(12);
    let doubled = (1u32 << octave.unsigned_abs()) as f32;
    let hz = OCTAVE[above.rem_euclid(12) as usize];
    if octave < 0 {
        hz / doubled
    } else {
        hz * doubled
    }
}

/// The number e to the power of minus `x`, for an `x` that isn't negative: what is left, after
/// a time, of a level that dies away.
pub(crate) const fn decay(x: f32) -> f32 {
    // Halved four times, which leaves a short series exact to a few billionths, then squared
    // as many times.
    let y = x / 16.0;
    let mut series = 1.0;
    let mut term = 12;
    while term > 0 {
        series = 1.0 - y / term as f32 * series;
        term -= 1;
    }
    let mut left = series;
    let mut squared = 0;
    while squared < 4 {
        left *= left;
        squared += 1;
    }
    left
}

/// White noise from a generator of its own.
pub(crate) struct Noise(pub(crate) u32);

impl Noise {
    pub(crate) fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1 << 23) as f32 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sine_is_exact_to_a_few_millionths() {
        for index in 0..=10_000 {
            let turns = index as f32 / 10_000.0;
            let exact = (turns as f64 * std::f64::consts::TAU).sin();
            assert!((sine(turns) as f64 - exact).abs() < 5e-6, "{turns}");
        }
    }

    #[test]
    fn pitches_are_those_of_equal_temperament() {
        assert_eq!(frequency(69), 440.0);
        assert_eq!(frequency(45), 110.0);
        assert_eq!(frequency(93), 1760.0);
        for pitch in 36..=100u8 {
            let exact = 440.0 * 2f64.powf((pitch as f64 - 69.0) / 12.0);
            assert!(
                (frequency(pitch) as f64 / exact - 1.0).abs() < 1e-6,
                "{pitch}"
            );
        }
    }

    #[test]
    fn a_decay_is_exact_to_a_few_millionths() {
        assert_eq!(decay(0.0), 1.0);
        // Up to the longest decay of a cue, and well past it.
        for index in 0..=20_000 {
            let x = index as f32 / 1000.0;
            let exact = (-(x as f64)).exp();
            assert!((decay(x) as f64 - exact).abs() < 2e-6, "{x}");
            assert!((decay(x) as f64 / exact - 1.0).abs() < 1e-5, "{x}");
        }
    }
}
