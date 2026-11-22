//! Seeded pseudorandom sampling.

use std::ops::Range;

/// A reproducible SplitMix64 generator for simulations and procedural content.
///
/// The same seed and sequence of calls reproduce the same output.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Initializes the generator with any 64-bit seed.
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next 64-bit sample.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mixed = (self.state ^ (self.state >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        let mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    /// Returns the high 32 bits of the next sample.
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Samples uniformly from `start..end`. Empty or reversed ranges return `None`.
    pub fn range_u32(&mut self, range: Range<u32>) -> Option<u32> {
        let width = range.end.checked_sub(range.start)?;
        if width == 0 {
            return None;
        }

        // Discard the incomplete residue block to avoid modulo bias.
        let threshold = width.wrapping_neg() % width;
        loop {
            let sample = self.next_u32();
            if sample >= threshold {
                return Some(range.start + sample % width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_reference_sequences() {
        for (seed, expected) in [
            (
                0,
                [
                    0xe220_a839_7b1d_cdaf,
                    0x6e78_9e6a_a1b9_65f4,
                    0x06c4_5d18_8009_454f,
                    0xf88b_b8a8_724c_81ec,
                ],
            ),
            (
                u64::MAX,
                [
                    0xe4d9_7177_1b65_2c20,
                    0xe99f_f867_dbf6_82c9,
                    0x382f_f84c_b272_81e9,
                    0x6d1d_b36c_cba9_82d2,
                ],
            ),
        ] {
            let mut rng = Rng::from_seed(seed);
            assert_eq!(expected.map(|_| rng.next_u64()), expected);
        }
    }

    #[test]
    fn cloning_reproduces_the_remaining_stream() {
        let mut rng = Rng::from_seed(81);
        rng.next_u64();
        let mut replay = rng.clone();

        for _ in 0..16 {
            assert_eq!(rng.next_u64(), replay.next_u64());
        }
    }

    #[test]
    fn samples_repeatable_integers_with_an_exclusive_upper_bound() {
        let mut rng = Rng::from_seed(0);

        let expected = [26, 19, 21, 21];
        assert_eq!(expected.map(|_| rng.range_u32(17..29)), expected.map(Some));

        for _ in 0..128 {
            assert!((12..15).contains(&rng.range_u32(12..15).unwrap()));
        }
        assert_eq!(rng.range_u32(u32::MAX - 1..u32::MAX), Some(u32::MAX - 1));
    }

    #[test]
    fn integer_range_discards_values_below_the_rejection_threshold() {
        let mut rng = Rng::from_seed(3);

        assert_eq!(rng.range_u32(0..0x8000_0001), Some(860_254_089));
        assert_eq!(rng.next_u64(), 0x9ceb_e8a6_d050_dd01);
    }

    #[test]
    fn invalid_integer_ranges_leave_the_stream_unchanged() {
        let mut rng = Rng::from_seed(19);
        let mut replay = rng.clone();
        for (start, end) in [(7, 7), (9, 5)] {
            assert_eq!(rng.range_u32(start..end), None);
        }
        assert_eq!(rng.next_u64(), replay.next_u64());
    }
}
