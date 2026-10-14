//! Sieves that run through many segments at once, with the vector instructions of the
//! processor.
//!
//! Most questions that the simulation asks of the walls are about a few of them: where a
//! bullet first meets one, whether one stands between two tanks, which ones a tank leans on.
//! A sieve tells which segments those few may be, eight or four at a time with the widest
//! instructions that the processor has, and the question is then answered exactly for the
//! segments that it kept. A sieve keeps every segment that matters and a few that don't, so
//! what it keeps changes no answer: the game is the same on every processor, with or without
//! these instructions.
//!
//! The crate is compiled with LLVM and optimized in every profile, as the music is (see
//! `.cargo/config.toml`): Cranelift, which compiles the other crates of a development build,
//! lacks many of these instructions.

use std::{ops::Range, sync::OnceLock};

use fearless_simd::{Level, Simd, SimdBase, SimdFloat, SimdMask, dispatch};

/// The widest vector that a processor may have, in numbers. The arrays of the segments are
/// padded to a whole number of them.
const WIDEST: usize = 16;

/// How much further than a test asks a sieve keeps a segment, in meters: far more than the
/// rounding of numbers moves anything in the tests of the simulation, and a hair next to the
/// size of a tank.
pub const SLACK: f32 = 0.05;

/// Where the padding of the arrays lies: so far that no sieve keeps it.
const NOWHERE: f32 = 1e30;

/// The words of the bits of a sieve.
const WORDS: usize = 32;

/// How many segments a sieve tells apart. Beyond that, it keeps all of them.
pub const CAPACITY: usize = 64 * WORDS;

/// Segments, laid out as vector instructions read them: an array for each number of them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Segments {
    count: usize,
    /// The two ends.
    ax: Vec<f32>,
    ay: Vec<f32>,
    bx: Vec<f32>,
    by: Vec<f32>,
    /// The middle of the box around each one, and half the size of that box.
    mx: Vec<f32>,
    my: Vec<f32>,
    ex: Vec<f32>,
    ey: Vec<f32>,
}

impl Segments {
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Adds a segment that runs from one end to the other.
    pub fn push(&mut self, a: [f32; 2], b: [f32; 2]) {
        if self.count.is_multiple_of(WIDEST) {
            // The room for the next ones, padded with segments that lie nowhere.
            for array in [
                &mut self.ax,
                &mut self.ay,
                &mut self.bx,
                &mut self.by,
                &mut self.mx,
                &mut self.my,
            ] {
                array.extend([NOWHERE; WIDEST]);
            }
            self.ex.extend([0.0; WIDEST]);
            self.ey.extend([0.0; WIDEST]);
        }
        let at = self.count;
        (self.ax[at], self.ay[at], self.bx[at], self.by[at]) = (a[0], a[1], b[0], b[1]);
        self.mx[at] = (a[0] + b[0]) / 2.0;
        self.my[at] = (a[1] + b[1]) / 2.0;
        self.ex[at] = (b[0] - a[0]).abs() / 2.0;
        self.ey[at] = (b[1] - a[1]).abs() / 2.0;
        self.count += 1;
    }

    /// The segments that may lie within reach: every one whose box comes within the box of
    /// the reach, and that doesn't lie wholly on one side of its band, if it has one. Each
    /// test keeps a segment that misses it by `SLACK` or less.
    pub fn sift(&self, reach: &Reach) -> Kept {
        let mut kept = Kept::new(self.count);
        if self.count <= CAPACITY {
            let used = self.count.div_ceil(64);
            let words = &mut kept.words[..used];
            dispatch!(level(), simd => sieve(simd, self, reach, words));
        }
        kept
    }
}

/// Where a question may find something: within a box, and within a band if it has one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    /// The middle of the box.
    pub center: [f32; 2],
    /// Half the size of the box.
    pub half: [f32; 2],
    pub band: Option<Band>,
}

/// A band along a line: a segment that lies wholly on one side of the line, further from it
/// than half the width of the band, is out of reach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    /// A point of the line.
    pub point: [f32; 2],
    /// The unit vector across the line.
    pub normal: [f32; 2],
    /// How far from the line the band reaches on each side.
    pub half_width: f32,
}

/// The segments that a sieve kept, by their number in the order they were pushed, from the
/// first one.
#[derive(Clone, Debug)]
pub struct Kept {
    words: [u64; WORDS],
    /// How many segments the sieve ran through. Beyond the capacity, it kept them all.
    count: usize,
    /// The word that is being read, and what is left of it.
    word: usize,
    left: u64,
    /// The next segment, when the sieve kept them all.
    every: Range<usize>,
}

impl Kept {
    fn new(count: usize) -> Self {
        Self {
            words: [0; WORDS],
            count,
            word: 0,
            left: 0,
            every: if count > CAPACITY { 0..count } else { 0..0 },
        }
    }
}

impl Iterator for Kept {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        if self.count > CAPACITY {
            return self.every.next();
        }
        while self.left == 0 {
            if self.word == self.count.div_ceil(64) {
                return None;
            }
            self.left = self.words[self.word];
            self.word += 1;
        }
        let bit = self.left.trailing_zeros() as usize;
        self.left &= self.left - 1;
        Some((self.word - 1) * 64 + bit)
    }
}

/// The best instructions of this processor, which are looked for once.
fn level() -> Level {
    static LEVEL: OnceLock<Level> = OnceLock::new();
    *LEVEL.get_or_init(Level::new)
}

#[inline(always)]
fn sieve<S: Simd>(simd: S, segments: &Segments, reach: &Reach, words: &mut [u64]) {
    let lanes = S::f32s::N;
    debug_assert!(WIDEST.is_multiple_of(lanes));
    let splat = |value: f32| S::f32s::splat(simd, value);
    let load =
        |array: &[f32], start: usize| S::f32s::from_slice(simd, &array[start..start + lanes]);
    let (cx, cy) = (splat(reach.center[0]), splat(reach.center[1]));
    let (hx, hy) = (splat(reach.half[0]), splat(reach.half[1]));
    let slack = splat(SLACK);
    // Without a band, one that is infinitely wide: no segment lies beyond it.
    let band = reach.band.unwrap_or(Band {
        point: [0.0; 2],
        normal: [0.0; 2],
        half_width: f32::INFINITY,
    });
    let (px, py) = (splat(band.point[0]), splat(band.point[1]));
    let (nx, ny) = (splat(band.normal[0]), splat(band.normal[1]));
    let (above, below) = (
        splat(band.half_width + SLACK),
        splat(-(band.half_width + SLACK)),
    );
    for start in (0..segments.ax.len()).step_by(lanes) {
        // How far the box of each segment is from the box of the reach, along each axis.
        let off_x = (load(&segments.mx, start) - cx).abs() - load(&segments.ex, start) - hx;
        let off_y = (load(&segments.my, start) - cy).abs() - load(&segments.ey, start) - hy;
        let boxed = off_x.simd_le(slack) & off_y.simd_le(slack);
        // Most segments are far from most questions: their boxes are enough to tell.
        if !boxed.any_true() {
            continue;
        }
        let side_a = (load(&segments.ax, start) - px) * nx + (load(&segments.ay, start) - py) * ny;
        let side_b = (load(&segments.bx, start) - px) * nx + (load(&segments.by, start) - py) * ny;
        let beyond = (side_a.simd_gt(above) & side_b.simd_gt(above))
            | (side_a.simd_lt(below) & side_b.simd_lt(below));
        let kept = boxed & !beyond;
        if !kept.any_true() {
            continue;
        }
        for (lane, &mark) in kept.as_slice().iter().enumerate() {
            let index = start + lane;
            if mark != 0 && index < segments.count {
                words[index / 64] |= 1 << (index % 64);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Numbers that look random and are the same on every run.
    struct Numbers(u64);

    impl Numbers {
        fn next(&mut self) -> f32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 40) as f32 / (1u64 << 24) as f32
        }

        fn within(&mut self, low: f32, high: f32) -> f32 {
            low + (high - low) * self.next()
        }
    }

    /// How far a segment is from a reach, by the two tests of a sieve: the larger of how far
    /// its box is from the box of the reach along each axis, and how far beyond the band it
    /// lies. A segment that the sieve must keep is no further than 0, and one that it may keep
    /// no further than `SLACK`.
    fn distance(a: [f32; 2], b: [f32; 2], reach: &Reach) -> f32 {
        let mut far = f32::MIN;
        for axis in 0..2 {
            let middle = (a[axis] + b[axis]) / 2.0;
            let extent = (b[axis] - a[axis]).abs() / 2.0;
            far = far.max((middle - reach.center[axis]).abs() - extent - reach.half[axis]);
        }
        if let Some(band) = reach.band {
            let side = |p: [f32; 2]| {
                (p[0] - band.point[0]) * band.normal[0] + (p[1] - band.point[1]) * band.normal[1]
            };
            let (side_a, side_b) = (side(a), side(b));
            let beyond = if side_a.signum() == side_b.signum() {
                side_a.abs().min(side_b.abs()) - band.half_width
            } else {
                f32::MIN
            };
            far = far.max(beyond);
        }
        far
    }

    #[test]
    fn a_sieve_keeps_every_segment_within_reach_and_none_far_from_it() {
        let mut numbers = Numbers(0x9E37_79B9_7F4A_7C15);
        for count in [0, 1, 7, 8, 9, 16, 33, 140, 300] {
            let mut segments = Segments::default();
            let mut ends = Vec::new();
            for _ in 0..count {
                let a = [numbers.within(-50.0, 50.0), numbers.within(-50.0, 50.0)];
                let length = numbers.within(0.0, 20.0);
                let angle = numbers.within(0.0, std::f32::consts::TAU);
                let b = [a[0] + length * angle.cos(), a[1] + length * angle.sin()];
                segments.push(a, b);
                ends.push((a, b));
            }
            assert_eq!(segments.len(), count);
            for _ in 0..200 {
                let center = [numbers.within(-50.0, 50.0), numbers.within(-50.0, 50.0)];
                let half = [numbers.within(0.0, 15.0), numbers.within(0.0, 15.0)];
                let band = (numbers.next() < 0.7).then(|| {
                    let angle = numbers.within(0.0, std::f32::consts::TAU);
                    Band {
                        point: center,
                        normal: [angle.cos(), angle.sin()],
                        half_width: numbers.within(0.0, 2.0),
                    }
                });
                let reach = Reach { center, half, band };
                let kept: Vec<usize> = segments.sift(&reach).collect();
                assert!(kept.windows(2).all(|pair| pair[0] < pair[1]));
                for (index, &(a, b)) in ends.iter().enumerate() {
                    let far = distance(a, b, &reach);
                    if far <= 0.0 {
                        assert!(kept.contains(&index), "{index} at {far} from {reach:?}");
                    }
                    if far > 2.0 * SLACK {
                        assert!(!kept.contains(&index), "{index} at {far} from {reach:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_sieve_keeps_everything_beyond_its_capacity() {
        let mut segments = Segments::default();
        for step in 0..CAPACITY + 3 {
            let x = step as f32 * 10.0;
            segments.push([x, 0.0], [x + 1.0, 0.0]);
        }
        let reach = Reach {
            center: [0.0, 0.0],
            half: [1.0, 1.0],
            band: None,
        };
        assert_eq!(segments.sift(&reach).count(), CAPACITY + 3);
        segments.clear();
        assert!(segments.is_empty());
        assert_eq!(segments.sift(&reach).count(), 0);
    }
}
