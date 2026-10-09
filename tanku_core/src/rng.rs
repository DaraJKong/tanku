//! Deterministic random streams.
//!
//! Generation must give the same result on every platform, so a stream only does fixed-width
//! integer arithmetic, and each purpose draws from its own stream.

/// What a stream is used for. Two streams with different purposes never share values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Maneuver = 1,
    Shape = 2,
    Obstacles = 3,
    Enemy = 4,
    Brain = 5,
}

#[derive(Clone, Debug)]
pub struct Stream(u64);

impl Stream {
    /// A stream for one purpose, identified by up to two numbers such as a sector and an index.
    pub fn new(seed: u64, purpose: Purpose, a: u32, b: u32) -> Self {
        let key = ((purpose as u64) << 56) ^ ((a as u64) << 28) ^ b as u64;
        Self(mix(seed ^ mix(key)))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// A value in `0..n`. `n` must not be zero.
    pub fn below(&mut self, n: u32) -> u32 {
        ((self.next_u32() as u64 * n as u64) >> 32) as u32
    }

    /// A value in `lo..=hi`.
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        debug_assert!(lo <= hi);
        lo + self.below((hi - lo) as u32 + 1) as i32
    }

    /// True `percent` times out of 100.
    pub fn chance(&mut self, percent: u32) -> bool {
        self.below(100) < percent
    }

    pub fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.below(items.len() as u32) as usize]
    }

    /// An index drawn in proportion to its weight. The weights must not all be zero.
    pub fn weighted(&mut self, weights: &[u32]) -> usize {
        let total: u32 = weights.iter().sum();
        let mut roll = self.below(total);
        for (index, &weight) in weights.iter().enumerate() {
            if roll < weight {
                return index;
            }
            roll -= weight;
        }
        unreachable!("the roll is below the total weight")
    }

    /// A value in `0.0..1.0`. Only for behavior and effects: generation stays on integers.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1 << 24) as f32
    }

    /// A value in `-1.0..1.0`.
    pub fn signed_unit(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

/// The finalizer of SplitMix64.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_are_stable() {
        // Golden values: a change here changes what every seed generates.
        let mut stream = Stream::new(42, Purpose::Shape, 7, 3);
        let drawn: Vec<u32> = (0..4).map(|_| stream.below(1000)).collect();
        assert_eq!(drawn, [515, 475, 989, 981]);
    }

    #[test]
    fn purposes_do_not_share_values() {
        let mut a = Stream::new(1, Purpose::Shape, 0, 0);
        let mut b = Stream::new(1, Purpose::Enemy, 0, 0);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn ranges_are_inclusive() {
        let mut stream = Stream::new(9, Purpose::Enemy, 0, 0);
        let drawn: Vec<i32> = (0..200).map(|_| stream.range(-2, 2)).collect();
        assert!(drawn.iter().all(|v| (-2..=2).contains(v)));
        assert!(drawn.contains(&-2) && drawn.contains(&2));
    }

    #[test]
    fn weights_of_zero_are_never_drawn() {
        let mut stream = Stream::new(5, Purpose::Enemy, 0, 0);
        assert!((0..100).all(|_| stream.weighted(&[0, 3, 0, 1]) % 2 == 1));
    }
}
