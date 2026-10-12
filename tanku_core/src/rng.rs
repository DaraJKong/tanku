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
    /// How the enemies of a sector share the budget of their group.
    Group = 6,
    /// The room of arena mode: its walls, what stands in it for each group, and where the
    /// tanks of each group appear.
    Room = 7,
}

/// A SplitMix64 generator, which starts from a state that depends on the seed and on its key.
#[derive(Clone, Debug)]
pub struct Stream(u64);

impl Stream {
    /// A stream for one purpose, identified by up to two numbers such as a sector and an index.
    pub fn new(seed: u64, purpose: Purpose, a: u32, b: u32) -> Self {
        // Each part of the key has bits of its own, so two streams of a seed never coincide.
        debug_assert!(a < 1 << 28 && b < 1 << 28);
        let key = ((purpose as u64) << 56) | ((a as u64) << 28) | b as u64;
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

/// The output function of SplitMix64.
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
    fn the_generator_is_splitmix64() {
        // The published reference values for a state of 1234567.
        let mut stream = Stream(1_234_567);
        let drawn: Vec<u64> = (0..5).map(|_| stream.next_u64()).collect();
        assert_eq!(
            drawn,
            [
                6_457_827_717_110_365_317,
                3_203_168_211_198_807_973,
                9_817_491_932_198_370_423,
                4_593_380_528_125_082_431,
                16_408_922_859_458_223_821,
            ]
        );
    }

    #[test]
    fn values_are_spread_evenly() {
        let mut stream = Stream::new(1, Purpose::Enemy, 0, 0);
        let mut counts = [0u32; 10];
        for _ in 0..100_000 {
            counts[stream.below(10) as usize] += 1;
        }
        // Chi-squared with 9 degrees of freedom: a fair generator exceeds 27.9 once in a
        // thousand tries.
        let chi_squared: f64 = counts
            .iter()
            .map(|&count| (count as f64 - 10_000.0).powi(2) / 10_000.0)
            .sum();
        assert!(chi_squared < 27.9, "{chi_squared}");
    }

    #[test]
    fn neighboring_streams_are_unrelated() {
        // Two sectors in a row, or two enemies in a row, start streams that have nothing in
        // common: half of the 64 bits of their first values differ on average.
        let first =
            |sector: u32, index: u32| Stream::new(42, Purpose::Enemy, sector, index).next_u64();
        let mut differing = 0;
        for sector in 0..2000 {
            differing += (first(sector, 0) ^ first(sector + 1, 0)).count_ones();
            differing += (first(sector, 0) ^ first(sector, 1)).count_ones();
        }
        let average = differing as f64 / 4000.0;
        assert!((average - 32.0).abs() < 0.5, "{average}");
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
